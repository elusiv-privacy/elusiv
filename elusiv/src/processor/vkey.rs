use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_types::SUB_ACCOUNT_ADDITIONAL_SIZE;
use elusiv_utils::{guard, open_pda_account_with_offset};
use solana_program::{entrypoint::ProgramResult, account_info::AccountInfo};
use crate::{proof::vkey::{VKeyAccount, VKeyAccountManangerAccount, VerifyingKey}, error::ElusivError, processor::setup_sub_account};

pub const VKEY_ACCOUNT_DATA_PACKET_SIZE: usize = 1024;

/// A binary data packet containing [`VKEY_ACCOUNT_DATA_PACKET_SIZE`] bytes
pub struct VKeyAccountDataPacket {
    data: Vec<u8>,
}

impl elusiv_types::BorshSerDeSized for VKeyAccountDataPacket {
    const SIZE: usize = VKEY_ACCOUNT_DATA_PACKET_SIZE;
}

impl BorshSerialize for VKeyAccountDataPacket {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        todo!()
    }
}

impl BorshDeserialize for VKeyAccountDataPacket {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        todo!()
    }
}

/// Creates a new [`VKeyAccount`]
pub fn create_vkey_account<'a>(
    signer: &AccountInfo<'a>,
    vkey_manager: &mut VKeyAccountManangerAccount,
    vkey_account: &AccountInfo<'a>,
    vkey_binary_data_account: &AccountInfo<'a>,

    vkey_id: u32,
    public_inputs_count: u32,
) -> ProgramResult {
    let vkey_count = vkey_manager.get_active_vkey_count();
    guard!(vkey_count < u32::MAX, ElusivError::InvalidAccountState);
    guard!(vkey_id == vkey_count, ElusivError::InvalidPublicInputs);

    // TODO: require specific authority as signer in mainnet

    open_pda_account_with_offset::<VKeyAccount>(
        &crate::id(),
        signer,
        vkey_account,
        vkey_id,
    )?;

    let binary_data_account_size = VerifyingKey::source_size(public_inputs_count as usize) + SUB_ACCOUNT_ADDITIONAL_SIZE;
    setup_sub_account::<VKeyAccount, 1>(
        vkey_account,
        vkey_binary_data_account,
        0,
        false,  // don't care about zeroness, signer can submit any data
        Some(binary_data_account_size),
    )?;

    Ok(())
}

/// Sets the bytes of a [`VKeyAccount`] `binary_data_account`
pub fn set_vkey_account_data(
    signer: &AccountInfo,
    vkey_account: &mut VKeyAccount,

    vkey_id: u32,
    data_position: u32,
    data: VKeyAccountDataPacket,
) -> ProgramResult {
    todo!()
}

/// Initializes the `binary_data_account` check for a [`VKeyAccount`]
pub fn init_vkey_account_check(
    actor: &AccountInfo,
    vkey_account: &mut VKeyAccount,

    vkey_id: u32,
) -> ProgramResult {
    todo!()
}

/// Computes the `binary_data_account` check for a [`VKeyAccount`]
pub fn compute_vkey_account_check(
    vkey_account: &mut VKeyAccount,
    binary_data_account: &AccountInfo,

    vkey_id: u32,
) -> ProgramResult {
    todo!()
}

/// Finalizes the `binary_data_account` check for a [`VKeyAccount`]
pub fn finalize_vkey_account_check(
    vkey_account: &mut VKeyAccount,

    vkey_id: u32,
) -> ProgramResult {
    todo!()
}