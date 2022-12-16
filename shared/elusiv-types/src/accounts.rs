use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::account_info::{AccountInfo, next_account_info};
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use elusiv_derive::BorshSerDeSized;
use crate::bytes::{BorshSerDeSized, ElusivOption};

/// An account with a fixed size
pub trait SizedAccount: Sized {
    /// The size of an [`SizedAccount`] measured in bytes
    const SIZE: usize;
}

/// A [`SizedAccount`] being owned by the program
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

    /// Splits the accounts data into the [`ChildAccountConfig`] and inner-data
    fn split_data(data: &[u8]) -> Result<(&[u8], &[u8]), ProgramError> {
        // TODO: disabled due to vkey having dynamic size
        /*if data.len() != Self::SIZE {
            return Err(ProgramError::InvalidAccountData)
        }*/

        let (config, inner_data) = data.split_at(ChildAccountConfig::SIZE);
        Ok((config, inner_data))
    }

    /// Splits the accounts data into the [`ChildAccountConfig`] and inner-data mutably
    fn split_data_mut(data: &mut [u8]) -> Result<(&mut [u8], &mut [u8]), ProgramError> {
        // TODO: disabled due to vkey having dynamic size
        /*if data.len() != Self::SIZE {
            return Err(ProgramError::InvalidAccountData)
        }*/

        let (config, inner_data) = data.split_at_mut(ChildAccountConfig::SIZE);
        Ok((config, inner_data))
    }

    /// Attempts to set the child-accounts [`ChildAccountConfig`]
    fn try_start_using_account(account: &AccountInfo) -> Result<(), ProgramError> {
        let data = &mut account.data.borrow_mut()[..];
        let (config_data, _) = Self::split_data_mut(data)?;
        let mut config = ChildAccountConfig::try_from_slice(config_data)?;

        if config.is_in_use {
            return Err(ProgramError::IllegalOwner)
        }
        config.is_in_use = true;
        let v = config.try_to_vec()?;

        config_data[..ChildAccountConfig::SIZE].copy_from_slice(&v);

        Ok(())
    }
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct ChildAccountConfig {
    pub is_in_use: bool,
}

impl<A: ChildAccount> SizedAccount for A {
    const SIZE: usize = A::INNER_SIZE + ChildAccountConfig::SIZE;
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
            return Err(ProgramError::InvalidArgument)
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
    unsafe fn get_child_account_unsafe(&self, child_index: usize) -> Result<&AccountInfo<'t>, ProgramError>;

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
                            Some(pubkey) => if *account.key != pubkey { continue },
                            None => continue,
                        }

                        if account.owner != program_id {
                            return Err(ProgramError::IllegalOwner);
                        }

                        if writable && !account.is_writable {
                            return Err(ProgramError::InvalidArgument)
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
    where C: FnOnce(&[u8]) -> T
    {
        let account: &AccountInfo<'t> = unsafe { self.get_child_account_unsafe(child_index) }?;
        let data = &account.data.borrow()[..];
        let (_, inner_data) = Self::Child::split_data(data)?;
        Ok(closure(inner_data))
    }

    /// Performs `closure` on the mutable data of the child-account at `child_index`
    fn execute_on_child_account_mut<T, C>(
        &self,
        child_index: usize,
        closure: C,
    ) -> Result<T, ProgramError>
    where C: FnOnce(&mut [u8]) -> T
    {
        let account: &AccountInfo<'t> = unsafe { self.get_child_account_unsafe(child_index) }?;
        let data = &mut account.data.borrow_mut()[..];
        let (_, inner_data) = Self::Child::split_data_mut(data)?;
        Ok(closure(inner_data))
    }
}

pub type PDAOffset = Option<u32>;

/// This trait is used by the [`elusiv_instruction`] and [`elusiv_accounts`] macros
/// - a PDAAccount is simply a PDA with:
///     1. the leading fields specified by [`PDAAccountFields`]
///     2. a PDA that is derived using the following seed: `&[ &SEED, offset?, bump ]`
/// - so there are two kinds of PDAAccounts:
///     - single instance: the `pda_offset` is `None` -> `&[ &SEED, bump ]`
///     - multi instance: the `pda_offset` is `Some(offset)` -> `&[ &SEED, offset, bump ]`
pub trait PDAAccount {
    const PROGRAM_ID: Pubkey;
    const SEED: &'static [u8];

    /// The [`Pubkey`] associated with the [`None`] [`PDAOffset`]
    const FIRST_PUBKEY: Pubkey;
    
    #[cfg(feature = "elusiv-client")]
    const IDENT: &'static str;

    fn find(offset: PDAOffset) -> (Pubkey, u8) {
        let seed = Self::offset_seed(Self::SEED, offset);
        let seed: Vec<&[u8]> = seed.iter().map(|x| &x[..]).collect();

        Pubkey::find_program_address(&seed, &Self::PROGRAM_ID)
    }

    fn pubkey(offset: PDAOffset, bump: u8) -> Result<Pubkey, ProgramError> {
        let mut seed = Self::offset_seed(Self::SEED, offset);
        seed.push(vec![bump]);
        let seed: Vec<&[u8]> = seed.iter().map(|x| &x[..]).collect();

        match Pubkey::create_program_address(&seed, &Self::PROGRAM_ID) {
            Ok(v) => Ok(v),
            Err(_) => Err(ProgramError::InvalidSeeds)
        }
    }

    fn offset_seed(seed: &[u8], offset: PDAOffset) -> Vec<Vec<u8>> {
        match offset {
            Some(offset) => vec![seed.to_vec(), offset.to_le_bytes().to_vec()],
            None => vec![seed.to_vec()]
        }
    }

    fn is_valid_pubkey(account: &AccountInfo, offset: PDAOffset, pubkey: &Pubkey) -> Result<bool, ProgramError> {
        if offset.is_none() {
            return Ok(Self::FIRST_PUBKEY == *pubkey)
        }

        let bump = Self::get_bump(account);
        Ok(Self::pubkey(offset, bump)? == *pubkey)
    }

    fn get_bump(account: &AccountInfo) -> u8 {
        account.data.borrow()[0]
    }

    fn signers_seeds(pda_offset: PDAOffset, bump: u8) -> Vec<Vec<u8>> {
        match pda_offset {
            Some(pda_offset) => vec![
                Self::SEED.to_vec(),
                u32::to_le_bytes(pda_offset).to_vec(),
                vec![bump]
            ],
            None => vec![
                Self::SEED.to_vec(),
                vec![bump]
            ]
        }
    }
} 

pub trait ComputationAccount: PDAAccount {
    fn instruction(&self) -> u32;
    fn round(&self) -> u32;
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Debug)]
pub struct PDAAccountData {
    pub bump_seed: u8,

    /// Used for future account migrations
    pub version: u8,

    /// In general useless, only if an account-type uses it
    pub initialized: bool,
}

impl PDAAccountData {
    pub fn new(data: &[u8]) -> Result<Self, std::io::Error> {
        PDAAccountData::try_from_slice(&data[..Self::SIZE])
    }
}