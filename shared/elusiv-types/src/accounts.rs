use crate as elusiv_types;
use crate::bytes::{BorshSerDeSized, ElusivOption};
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_derive::BorshSerDeSized;
use solana_program::account_info::{next_account_info, AccountInfo};
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

/// An account with a fixed size
pub trait SizedAccount: Sized {
    /// The size of an [`SizedAccount`] measured in bytes
    const SIZE: usize;
}

/// A [`SizedAccount`] being owned by the program, represented by a mutable byte slice
pub trait ProgramAccount<'a>: SizedAccount {
    /// Attempts to create a new instance of [`Self`] from a buffer
    fn new(data: &'a mut [u8]) -> Result<Self, ProgramError>;
}

/// A program owned system-program account that can store data up to 10 MiB in size
///
/// # Note
///
/// - Each [`ChildAccount`] is bound to a single [`ParentAccount`].
/// - Each [`ChildAccount`]'s data starts with the [`ChildAccountConfig`].
pub trait ChildAccount: Sized {
    /// The size of [`Self`] measured in bytes (without the additional [`ChildAccountConfig::SIZE`])
    const INNER_SIZE: usize;

    /// Attempts to set the child-accounts [`ChildAccountConfig`]
    fn try_start_using_account(account: &AccountInfo) -> Result<(), ProgramError> {
        let data = &mut account.data.borrow_mut()[..];
        let (config_data, _) = split_child_account_data_mut(data)?;
        let mut config = ChildAccountConfig::try_from_slice(config_data)?;

        if config.is_in_use {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        config.is_in_use = true;

        let mut slice = &mut config_data[..ChildAccountConfig::SIZE];
        borsh::BorshSerialize::serialize(&config, &mut slice).unwrap();

        Ok(())
    }
}

/// Splits the accounts data into the [`ChildAccountConfig`] and inner-data
pub fn split_child_account_data(data: &[u8]) -> Result<(&[u8], &[u8]), ProgramError> {
    let (config, inner_data) = data.split_at(ChildAccountConfig::SIZE);
    Ok((config, inner_data))
}

/// Splits the accounts data into the [`ChildAccountConfig`] and inner-data mutably
pub fn split_child_account_data_mut(
    data: &mut [u8],
) -> Result<(&mut [u8], &mut [u8]), ProgramError> {
    let (config, inner_data) = data.split_at_mut(ChildAccountConfig::SIZE);
    Ok((config, inner_data))
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct ChildAccountConfig {
    pub is_in_use: bool,
}

pub const fn child_account_size(inner_size: usize) -> usize {
    inner_size + ChildAccountConfig::SIZE
}

impl<A: ChildAccount> SizedAccount for A {
    const SIZE: usize = child_account_size(A::INNER_SIZE);
}

/// A [`ProgramAccount`] that itself "owns" one or more [`ChildAccount`]s
///
/// # Note
///
/// - A [`ChildAccount`] can be a PDA, but will most likely be data accounts (size > 10 KiB).
pub trait ParentAccount<'a, 'b, 't>: ProgramAccount<'a> {
    /// The number of child-accounts
    const COUNT: usize;

    /// The associated [`ChildAccount`] type
    type Child: ChildAccount;

    /// Attempts to create a new instance of [`Self`] from a data-buffer and a child-accounts
    /// - this function DOES NOT verify the `child_accounts` pubkeys
    fn new_with_child_accounts(
        data: &'a mut [u8],
        child_accounts: Vec<Option<&'b AccountInfo<'t>>>,
    ) -> Result<Self, ProgramError> {
        if child_accounts.len() != Self::COUNT {
            return Err(ProgramError::InvalidArgument);
        }

        let mut s = Self::new(data)?;
        Self::set_child_accounts(&mut s, child_accounts);

        Ok(s)
    }

    /// Sets all child-accounts for this instance
    fn set_child_accounts(parent: &mut Self, child_accounts: Vec<Option<&'b AccountInfo<'t>>>);

    /// Sets a specific child-accounts [`Pubkey`] persistently
    fn set_child_pubkey(&mut self, index: usize, pubkey: ElusivOption<Pubkey>);

    /// Gets a specific child-accounts [`Pubkey`] persistently
    /// - returns [`None`] if the child-account has not been set
    fn get_child_pubkey(&self, index: usize) -> Option<Pubkey>;

    /// Returns the child-accounts [`AccountInfo`] for the specified index
    ///
    /// # Safety
    ///
    /// - Each child-account has to be serialized using the [`ChildAccount`] struct.
    /// - Modifiying/accessing without the [`ChildAccount`] struct can lead to undefined behaviour.
    /// - Use `execute_on_sub_account` instead of `get_account_unsafe` directly.
    unsafe fn get_child_account_unsafe(
        &self,
        child_index: usize,
    ) -> Result<&AccountInfo<'t>, ProgramError>;

    /// Finds all `n elem [0; COUNT]` available child-accounts in an [`AccountInfo`]-iterator
    ///
    /// # Notes
    ///
    /// - All matched accounts are consumed from the iterator.
    /// - The accounts need to match the order in which their pubkeys are stored.
    /// - Any account which pubkey has been previously set can be used.
    fn find_child_accounts<'c, 'd, I>(
        parent: &Self,
        program_id: &Pubkey,
        writable: bool,
        account_info_iter: &mut I,
    ) -> Result<Vec<Option<&'c AccountInfo<'d>>>, ProgramError>
    where
        I: Iterator<Item = &'c AccountInfo<'d>> + Clone,
    {
        let child_pubkeys: Vec<Option<Pubkey>> = (0..Self::COUNT)
            .map(|i| parent.get_child_pubkey(i))
            .collect();

        let mut accounts = vec![None; Self::COUNT];
        let mut remaining_iter = account_info_iter.clone();
        let mut i = 0;
        while i < Self::COUNT {
            match next_account_info(account_info_iter) {
                Ok(account) => {
                    #[allow(clippy::needless_range_loop)]
                    for child_index in i..Self::COUNT {
                        match child_pubkeys[child_index] {
                            Some(pubkey) => {
                                if *account.key != pubkey {
                                    continue;
                                }
                            }
                            None => continue,
                        }

                        if account.owner != program_id {
                            return Err(ProgramError::IllegalOwner);
                        }

                        if writable && !account.is_writable {
                            return Err(ProgramError::InvalidArgument);
                        }

                        accounts[child_index] = Some(account);
                        next_account_info(&mut remaining_iter)?;
                        i = child_index;

                        break;
                    }
                    i += 1;
                }
                Err(_) => break,
            }
        }

        *account_info_iter = remaining_iter;
        Ok(accounts)
    }

    /// Performs `closure` on the data of the child-account at `child_index`
    fn execute_on_child_account<T, C>(
        &self,
        child_index: usize,
        closure: C,
    ) -> Result<T, ProgramError>
    where
        C: FnOnce(&[u8]) -> T,
    {
        let account: &AccountInfo<'t> = unsafe { self.get_child_account_unsafe(child_index) }?;
        let data = &account.data.borrow()[..];
        let (_, inner_data) = split_child_account_data(data)?;
        Ok(closure(inner_data))
    }

    /// Performs `closure` on the mutable data of the child-account at `child_index`
    fn execute_on_child_account_mut<T, C>(
        &self,
        child_index: usize,
        closure: C,
    ) -> Result<T, ProgramError>
    where
        C: FnOnce(&mut [u8]) -> T,
    {
        let account: &AccountInfo<'t> = unsafe { self.get_child_account_unsafe(child_index) }?;
        let data = &mut account.data.borrow_mut()[..];
        let (_, inner_data) = split_child_account_data_mut(data)?;
        Ok(closure(inner_data))
    }
}

pub type PDAOffset = Option<u32>;

/// A [`PDAAccount`] uses a seed, an (optional) [`Pubkey`] and a [`PDAOffset`] to derive PDAs
pub trait PDAAccount {
    const PROGRAM_ID: Pubkey;
    const SEED: &'static [u8];

    /// The PDA associated with no [`Pubkey`] and the [`None`] [`PDAOffset`]
    const FIRST_PDA: (Pubkey, u8);

    #[cfg(feature = "elusiv-client")]
    const IDENT: &'static str;

    fn find(offset: PDAOffset) -> (Pubkey, u8) {
        if offset.is_none() {
            return Self::FIRST_PDA;
        }

        let seed = Self::seeds(Self::SEED, None, offset);
        let seed: Vec<&[u8]> = seed.iter().map(|x| &x[..]).collect();

        Pubkey::find_program_address(&seed, &Self::PROGRAM_ID)
    }

    fn find_with_pubkey(pubkey: Pubkey, offset: PDAOffset) -> (Pubkey, u8) {
        let seed = Self::seeds(Self::SEED, Some(pubkey), offset);
        let seed: Vec<&[u8]> = seed.iter().map(|x| &x[..]).collect();

        Pubkey::find_program_address(&seed, &Self::PROGRAM_ID)
    }

    #[cfg(feature = "elusiv-client")]
    fn find_with_pubkey_optional(pubkey: Option<Pubkey>, offset: PDAOffset) -> (Pubkey, u8) {
        match pubkey {
            Some(pubkey) => Self::find_with_pubkey(pubkey, offset),
            None => Self::find(offset),
        }
    }

    fn create(offset: PDAOffset, bump: u8) -> Result<Pubkey, ProgramError> {
        if offset.is_none() {
            return Ok(Self::FIRST_PDA.0);
        }

        let seed = Self::signers_seeds(None, offset, bump);
        let seed: Vec<&[u8]> = seed.iter().map(|x| &x[..]).collect();

        Pubkey::create_program_address(&seed, &Self::PROGRAM_ID).or(Err(ProgramError::InvalidSeeds))
    }

    fn create_with_pubkey(
        pubkey: Pubkey,
        offset: PDAOffset,
        bump: u8,
    ) -> Result<Pubkey, ProgramError> {
        let seed = Self::signers_seeds(Some(pubkey), offset, bump);
        let seed: Vec<&[u8]> = seed.iter().map(|x| &x[..]).collect();

        Pubkey::create_program_address(&seed, &Self::PROGRAM_ID).or(Err(ProgramError::InvalidSeeds))
    }

    fn seeds(seed: &[u8], pubkey: Option<Pubkey>, offset: PDAOffset) -> Vec<Vec<u8>> {
        let mut seed = vec![seed.to_vec()];

        if let Some(pubkey) = pubkey {
            seed.push(pubkey.to_bytes().to_vec());
        }

        if let Some(offset) = offset {
            seed.push(offset.to_le_bytes().to_vec());
        }

        seed
    }

    fn signers_seeds(pubkey: Option<Pubkey>, offset: PDAOffset, bump: u8) -> Vec<Vec<u8>> {
        let mut seed = Self::seeds(Self::SEED, pubkey, offset);
        seed.push(vec![bump]);
        seed
    }

    /// Extracts the bump from an [`AccountInfo`]
    ///
    /// # Note
    ///
    /// This requires the account to store [`PDAAccountData`] as the leading data
    fn get_bump(account: &AccountInfo) -> u8 {
        account.data.borrow()[0]
    }

    fn verify_account(account: &AccountInfo, offset: PDAOffset) -> ProgramResult {
        if Self::create(offset, Self::get_bump(account))? != *account.key {
            return Err(ProgramError::InvalidSeeds);
        }

        Ok(())
    }

    fn verify_account_with_pubkey(
        account: &AccountInfo,
        pubkey: Pubkey,
        offset: PDAOffset,
    ) -> ProgramResult {
        if Self::create_with_pubkey(pubkey, offset, Self::get_bump(account))? != *account.key {
            return Err(ProgramError::InvalidSeeds);
        }

        Ok(())
    }
}

pub trait ComputationAccount: PDAAccount {
    fn instruction(&self) -> u32;
    fn round(&self) -> u32;
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
#[cfg_attr(feature = "elusiv-client", derive(Clone, Debug))]
pub struct PDAAccountData {
    pub bump_seed: u8,

    /// Used for future account migrations
    pub version: u8,
}

impl PDAAccountData {
    pub fn new(data: &[u8]) -> Result<Self, std::io::Error> {
        PDAAccountData::try_from_slice(&data[..Self::SIZE])
    }
}

/// A [`ProgramAccount`] that also has a eager representation
#[cfg(feature = "elusiv-client")]
pub trait EagerAccount<'a>: ProgramAccount<'a> {
    type Repr: EagerAccountRepr;

    /// Attempts to create a new instance of the associated eager representation [`Repr`] from a buffer
    fn new_eager(data: Vec<u8>) -> Result<Self::Repr, std::io::Error> {
        Self::Repr::new(data)
    }
}

/// Eager representation of a [`ProgramAccount`]
#[cfg(feature = "elusiv-client")]
pub trait EagerAccountRepr: Sized {
    /// Attempts to create a new instance of [`Self`] from a buffer
    fn new(data: Vec<u8>) -> Result<Self, std::io::Error>;
}

/// Eager representation of a [`ParentAccount`]
#[cfg(feature = "elusiv-client")]
pub trait EagerParentAccountRepr: EagerAccountRepr {
    /// All children pubkeys
    fn child_pubkeys(&self) -> Vec<Option<Pubkey>>;
}

pub trait AccountRepr {
    fn pubkey(&self) -> Pubkey;
}

impl<'a> AccountRepr for AccountInfo<'a> {
    fn pubkey(&self) -> Pubkey {
        *self.key
    }
}

pub struct UnverifiedAccountInfo<'a, 'b> {
    account_info: &'a AccountInfo<'b>,
    is_verified: bool,
}

impl<'a, 'b> UnverifiedAccountInfo<'a, 'b> {
    pub fn new(account_info: &'a AccountInfo<'b>) -> Self {
        Self {
            account_info,
            is_verified: false,
        }
    }

    pub fn get_unsafe(&self) -> &'a AccountInfo<'b> {
        self.account_info
    }

    pub fn get_safe(&self) -> Result<&'a AccountInfo<'b>, ProgramError> {
        if self.is_verified {
            Ok(self.get_unsafe())
        } else {
            Err(ProgramError::AccountBorrowFailed)
        }
    }

    pub fn get_unsafe_and_set_is_verified(&mut self) -> &'a AccountInfo<'b> {
        self.set_is_verified();
        self.get_unsafe()
    }

    pub fn set_is_verified(&mut self) {
        self.is_verified = true;
    }
}

macro_rules! impl_user_account {
    ($ty: ident) => {
        #[cfg(feature = "elusiv-client")]
        #[derive(Debug)]
        pub struct $ty(pub Pubkey);

        #[cfg(feature = "elusiv-client")]
        impl AccountRepr for $ty {
            fn pubkey(&self) -> Pubkey {
                self.0
            }
        }

        #[cfg(feature = "elusiv-client")]
        impl From<Pubkey> for $ty {
            fn from(pk: Pubkey) -> Self {
                Self(pk)
            }
        }

        #[cfg(feature = "elusiv-client")]
        impl From<&Pubkey> for $ty {
            fn from(pk: &Pubkey) -> Self {
                Self(*pk)
            }
        }
    };
}

impl_user_account!(UserAccount);
impl_user_account!(WritableUserAccount);
impl_user_account!(SignerAccount);
impl_user_account!(WritableSignerAccount);
