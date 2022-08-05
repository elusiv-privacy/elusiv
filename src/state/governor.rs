use borsh::{BorshDeserialize, BorshSerialize};
use crate::token::{TokenAuthorityAccount, TOKENS};
use crate::{macros::elusiv_account, types::U256};
use crate::bytes::{BorshSerDeSized, ElusivOption};
use crate::state::program_account::SizedAccount;
use super::{program_account::PDAAccountData, fee::ProgramFee};

#[elusiv_account(pda_seed = b"governor")]
pub struct GovernorAccount {
    pda_data: PDAAccountData,

    /// The current fee-version (new requests are forced to use this version)
    fee_version: u32,

    /// The `ProgramFee` for the `FeeAccount` with the offset `fee_version`
    program_fee: ProgramFee,

    /// The number of commitments in a MT-root hashing batch
    commitment_batching_rate: u32,

    program_version: u32,
}

const TOKEN_COUNT: usize = TOKENS.len();

macro_rules! impl_token_authority {
    ($ty: ident) => {
        impl<'a> TokenAuthorityAccount<'a> for $ty<'a> {
            fn get_token_account(&self, token_id: u16) -> U256 {
                self.get_token_account(token_id as usize).option().unwrap()
            }
        }
    };
}

impl_token_authority!(PoolAccount);
impl_token_authority!(FeeCollectorAccount);

#[elusiv_account(pda_seed = b"pool")]
pub struct PoolAccount {
    pda_data: PDAAccountData,
    token_account: [ElusivOption<U256>; TOKEN_COUNT],
}

#[elusiv_account(pda_seed = b"fee_collector")]
pub struct FeeCollectorAccount {
    pda_data: PDAAccountData,
    token_account: [ElusivOption<U256>; TOKEN_COUNT],
}