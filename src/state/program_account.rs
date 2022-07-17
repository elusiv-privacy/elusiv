//! Traits used to represent types of accounts, owned by the program

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::account_info::AccountInfo;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use crate::macros::BorshSerDeSized;
use crate::bytes::{BorshSerDeSized, ElusivOption};

pub trait SizedAccount {
    const SIZE: usize;
}

pub trait ProgramAccount<'a>: SizedAccount {
    type T: SizedAccount;

    fn new(d: &'a mut [u8]) -> Result<Self::T, ProgramError>;
}

pub trait MultiAccountProgramAccount<'a, 'b, 't>: SizedAccount {
    type T: SizedAccount;

    fn new(
        d: &'a mut [u8],
        accounts: std::collections::HashMap<usize, &'b AccountInfo<'t>>,
    ) -> Result<Self::T, ProgramError>;
}

/// This trait is used by the `elusiv_instruction` and `elusiv_accounts` macros
/// - a PDAAccount is simply a PDA with:
///     1. the leading fields specified by `PDAAccountFields`
///     2. a PDA that is derived using the following seed: `&[ &SEED, offset?, bump ]`
/// - so there are two kinds of PDAAccounts:
///     - single instance: the pda_offset is `None` -> `&[ &SEED, bump ]`
///     - multi instance: the pda_offset is `Some(offset)` -> `&[ &SEED, offset, bump ]`
pub trait PDAAccount {
    const SEED: &'static [u8];

    fn find(offset: Option<u64>) -> (Pubkey, u8) {
        let seed = Self::offset_seed(offset);
        let seed: Vec<&[u8]> = seed.iter().map(|x| &x[..]).collect();

        Pubkey::find_program_address(&seed, &crate::id())
    }

    fn pubkey(offset: Option<u64>, bump: u8) -> Result<Pubkey, ProgramError> {
        let mut seed = Self::offset_seed(offset);
        seed.push(vec![bump]);
        let seed: Vec<&[u8]> = seed.iter().map(|x| &x[..]).collect();

        match Pubkey::create_program_address(&seed, &crate::id()) {
            Ok(v) => Ok(v),
            Err(_) => Err(ProgramError::InvalidSeeds)
        }
    }

    fn offset_seed(offset: Option<u64>) -> Vec<Vec<u8>> {
        match offset {
            Some(offset) => vec![Self::SEED.to_vec(), offset.to_le_bytes().to_vec()],
            None => vec![Self::SEED.to_vec()]
        }
    }

    fn is_valid_pubkey(account: &AccountInfo, offset: Option<u64>, pubkey: &Pubkey) -> Result<bool, ProgramError> {
        let bump = account.data.borrow()[0];
        Ok(Self::pubkey(offset, bump)? == *pubkey)
    }
} 

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
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

#[derive(BorshDeserialize, BorshSerialize)]
pub struct MultiAccountAccountData<const COUNT: usize> {
    // ... PDAAccountData always before MultiAccountAccountData, since it's a PDA
     
    pub pubkeys: [ElusivOption<Pubkey>; COUNT],
}

impl<const COUNT: usize> BorshSerDeSized for MultiAccountAccountData<COUNT> {
    const SIZE: usize = COUNT * <ElusivOption<Pubkey>>::SIZE;

    fn override_slice(value: &Self, slice: &mut [u8]) -> Result<(), std::io::Error> {
        let vec = Self::try_to_vec(value)?;
        slice[PDAAccountData::SIZE..PDAAccountData::SIZE + vec.len()].copy_from_slice(&vec[..]);
        Ok(())
    }
}

impl<const COUNT: usize> MultiAccountAccountData<COUNT> {
    pub fn new(data: &[u8]) -> Result<Self, std::io::Error> {
        MultiAccountAccountData::try_from_slice(&data[PDAAccountData::SIZE..PDAAccountData::SIZE + Self::SIZE])
    }
}

/// Certain accounts, like the `VerificationAccount` can be instantiated multiple times.
/// - this allows for parallel computations/usage
/// - so we can compare this index with `MAX_INSTANCES` to check validity
pub trait MultiInstancePDAAccount: PDAAccount {
    const MAX_INSTANCES: u64;

    fn is_valid(&self, index: u64) -> bool {
        index < Self::MAX_INSTANCES
    }
}

/// Allows for storing data across multiple accounts (needed for data sized >10 MiB)
/// - these accounts can be PDAs, but will most likely be data accounts (size > 10 KiB)
/// - by default all these accounts are assumed to have the same size = `ACCOUNT_SIZE`
/// - important: `ACCOUNT_SIZE` needs to contain `SUB_ACCOUNT_ADDITIONAL_SIZE`
pub trait MultiAccountAccount<'t>: PDAAccount {
    type T: BorshSerDeSized;

    /// The count of subsidiary accounts
    const COUNT: usize;

    /// The size of subsidiary accounts
    const ACCOUNT_SIZE: usize;

    /// Returns the sub-account for the specified index
    /// 
    /// # Safety
    /// - Each sub-account has to be serialized using the `SubAccount` struct.
    /// - Modifiying/accessing without the `SubAccount` struct, can lead to undefined behaviour.
    /// - Use `execute_on_sub_account` instead of `get_account_unsafe` directly.
    unsafe fn get_account_unsafe(&self, account_index: usize) -> Result<&AccountInfo<'t>, ProgramError>;

    /// Ensures that the fields of `SubAccount` are not manipulated on a sub-account
    fn execute_on_sub_account<F, T, E>(&self, account_index: usize, f: F) -> Result<T, ProgramError> where F: Fn(&mut [u8]) -> Result<T, E> {
        let account = unsafe { self.get_account_unsafe(account_index)? };
        let data = &mut account.data.borrow_mut()[..];
        let account = SubAccount::new(data);
        f(account.data).or(Err(ProgramError::InvalidAccountData))
    }

    /// Can be used to track modifications (just important for test functions)
    fn modify(&mut self, index: usize, value: Self::T);
}

/// Size required for the `is_in_use` boolean
pub const SUB_ACCOUNT_ADDITIONAL_SIZE: usize = 1;

pub struct SubAccount<'a> {
    is_in_use: &'a mut [u8],
    pub data: &'a mut [u8],
}

impl<'a> SubAccount<'a> {
    pub fn new(data: &'a mut [u8]) -> Self {
        let (is_in_use, data) = data.split_at_mut(1);
        Self { is_in_use, data }
    }

    pub fn get_is_in_use(&self) -> bool {
        self.is_in_use[0] == 1
    }
    pub fn set_is_in_use(&mut self, value: bool) {
        self.is_in_use[0] = if value { 1 } else { 0 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PDATest { }

    impl PDAAccount for PDATest {
        const SEED: &'static [u8] = b"ABC";
    }

    #[test]
    fn test_pda_account() {
        assert_ne!(PDATest::find(None), PDATest::find(Some(0)));
        assert_ne!(PDATest::find(Some(0)), PDATest::find(Some(1)));
    }

    #[test]
    fn test_sub_account() {
        let mut data = vec![0; 100];
        let mut acc = SubAccount::new(&mut data);

        assert!(!acc.get_is_in_use());
        acc.set_is_in_use(true);
        assert!(acc.get_is_in_use());
        acc.set_is_in_use(false);
        assert!(!acc.get_is_in_use());

        assert_eq!(acc.data.len(), 99);
    }
}