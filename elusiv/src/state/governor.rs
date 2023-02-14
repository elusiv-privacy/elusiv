use super::{fee::ProgramFee, program_account::PDAAccountData};
use crate::macros::elusiv_account;

#[elusiv_account(eager_type: true)]
pub struct GovernorAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,

    /// The current fee-version (new requests are forced to use this version)
    pub fee_version: u32,

    /// The `ProgramFee` for the `FeeAccount` with the offset `fee_version`
    pub program_fee: ProgramFee,

    /// The number of commitments in a MT-root hashing batch
    pub commitment_batching_rate: u32,

    program_version: u32,
}

#[elusiv_account(eager_type: true)]
pub struct PoolAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,
}

#[elusiv_account(eager_type: true)]
pub struct FeeCollectorAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,
}
