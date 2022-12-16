use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_types::{ElusivOption, ChildAccountConfig, BorshSerDeSized, ProgramAccount, ParentAccount};
use elusiv_utils::{guard, open_pda_account_with_offset};
use solana_program::{entrypoint::ProgramResult, account_info::AccountInfo};
use crate::{proof::vkey::{VKeyAccount, VKeyAccountManangerAccount, VerifyingKey}, error::ElusivError, types::U256, processor::setup_child_account};

pub const VKEY_ACCOUNT_DATA_PACKET_SIZE: usize = 964;

/// A binary data packet containing [`VKEY_ACCOUNT_DATA_PACKET_SIZE`] bytes
#[derive(BorshSerialize, BorshDeserialize)]
pub struct VKeyAccountDataPacket(pub Vec<u8>);

impl elusiv_types::BorshSerDeSized for VKeyAccountDataPacket {
    const SIZE: usize = VKEY_ACCOUNT_DATA_PACKET_SIZE + u32::SIZE;
}

/// Creates a new [`VKeyAccount`]
pub fn create_vkey_account<'a>(
    signer: &AccountInfo<'a>,
    vkey_manager: &mut VKeyAccountManangerAccount,
    vkey_account: &AccountInfo<'a>,
    vkey_binary_data_account: &AccountInfo<'a>,

    vkey_id: u32,
    public_inputs_count: u32,
    deploy_authority: ElusivOption<U256>,
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

    let data = &mut vkey_account.data.borrow_mut()[..];
    let mut vkey_account = VKeyAccount::new(data)?;

    vkey_account.set_deploy_authority(&deploy_authority);
    vkey_account.set_public_inputs_count(&public_inputs_count);

    let binary_data_account_size = VerifyingKey::source_size(public_inputs_count as usize) + ChildAccountConfig::SIZE;
    setup_child_account(
        &mut vkey_account,
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

    _vkey_id: u32,
    data_position: u32,
    packet: VKeyAccountDataPacket,
) -> ProgramResult {
    guard!(!vkey_account.get_is_frozen(), ElusivError::InvalidAccountState);

    if let Some(deploy_authority) = vkey_account.get_deploy_authority().option() {
        guard!(signer.key.to_bytes() == deploy_authority, ElusivError::InvalidAccount);
    }

    let public_inputs_count = vkey_account.get_public_inputs_count();
    let len = VerifyingKey::source_size(public_inputs_count as usize);
    let start = data_position as usize * VKEY_ACCOUNT_DATA_PACKET_SIZE;
    let end = start + VKEY_ACCOUNT_DATA_PACKET_SIZE;
    let cutoff = if end > len { end - len } else { 0 };

    guard!(start < len, ElusivError::InvalidPublicInputs);

    vkey_account.execute_on_child_account_mut(0, |data| {
        data[start..end - cutoff].copy_from_slice(&packet.0[..VKEY_ACCOUNT_DATA_PACKET_SIZE - cutoff])
    })?;

    Ok(())
}

/// Freezes a [`VKeyAccount`]
pub fn freeze_vkey_account(
    signer: &AccountInfo,
    vkey_account: &mut VKeyAccount,

    _vkey_id: u32,
) -> ProgramResult {
    guard!(!vkey_account.get_is_frozen(), ElusivError::InvalidAccountState);

    if let Some(deploy_authority) = vkey_account.get_deploy_authority().option() {
        guard!(signer.key.to_bytes() == deploy_authority, ElusivError::InvalidAccount);
    }

    vkey_account.set_is_frozen(&true);

    Ok(())
}

#[cfg(test)]
mod test {
    use assert_matches::assert_matches;
    use crate::{processor::vkey_account, proof::vkey::{TestVKey, VerifyingKeyInfo}, macros::test_account_info, bytes::div_ceiling_usize};
    use super::*;

    #[test]
    fn test_set_vkey_account_data() {
        let data = TestVKey::verifying_key_source();
        vkey_account!(vkey_account, TestVKey);
        test_account_info!(signer);

        vkey_account.execute_on_child_account_mut(0, |d| {
            for b in d.iter_mut() {
                *b = 0;
            }
        }).unwrap();

        let positions = div_ceiling_usize(VerifyingKey::source_size(TestVKey::public_inputs_count()), VKEY_ACCOUNT_DATA_PACKET_SIZE);
        for i in 0..positions {
            let slice = &data[i * VKEY_ACCOUNT_DATA_PACKET_SIZE..std::cmp::min((i + 1) * VKEY_ACCOUNT_DATA_PACKET_SIZE, data.len())];
            set_vkey_account_data(
                &signer,
                &mut vkey_account,
                0,
                i as u32,
                VKeyAccountDataPacket(slice.to_vec()),
            ).unwrap();
        }

        vkey_account.execute_on_child_account(0, |d| {
            assert_eq!(d, data);
        }).unwrap();
    }

    #[test]
    fn test_freeze_vkey_account() {
        vkey_account!(vkey_account, TestVKey);
        test_account_info!(signer);

        vkey_account.set_public_inputs_count(&TestVKey::PUBLIC_INPUTS_COUNT);
        vkey_account.execute_on_child_account_mut(0, |data| {
            data.copy_from_slice(&TestVKey::verifying_key_source())
        }).unwrap();

        freeze_vkey_account(&signer, &mut vkey_account, 0).unwrap();

        assert!(vkey_account.get_is_frozen());
        assert_matches!(freeze_vkey_account(&signer, &mut vkey_account, 0), Err(_));
    }
}