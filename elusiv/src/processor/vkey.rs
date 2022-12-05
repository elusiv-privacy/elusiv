use std::collections::HashMap;
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_types::{SUB_ACCOUNT_ADDITIONAL_SIZE, MultiAccountProgramAccount, ElusivOption, MultiAccountAccount};
use elusiv_utils::{guard, open_pda_account_with_offset};
use solana_program::{entrypoint::ProgramResult, account_info::AccountInfo, hash::Hash};
use crate::{proof::vkey::{VKeyAccount, VKeyAccountManangerAccount, VerifyingKey}, error::ElusivError, processor::setup_sub_account, types::U256, bytes::div_ceiling_u32};

pub const VKEY_ACCOUNT_DATA_PACKET_SIZE: usize = 1024;

/// A binary data packet containing [`VKEY_ACCOUNT_DATA_PACKET_SIZE`] bytes
#[derive(BorshSerialize)]
pub struct VKeyAccountDataPacket(pub Vec<u8>);

impl elusiv_types::BorshSerDeSized for VKeyAccountDataPacket {
    const SIZE: usize = VKEY_ACCOUNT_DATA_PACKET_SIZE;
}

impl BorshDeserialize for VKeyAccountDataPacket {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        let data = Vec::deserialize(buf)?;
        assert_eq!(data.len(), VKEY_ACCOUNT_DATA_PACKET_SIZE);
        Ok(Self(data))
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

    let binary_data_account_size = VerifyingKey::source_size(public_inputs_count as usize) + SUB_ACCOUNT_ADDITIONAL_SIZE;
    setup_sub_account::<VKeyAccount, 1>(
        vkey_account,
        vkey_binary_data_account,
        0,
        false,  // don't care about zeroness, signer can submit any data
        Some(binary_data_account_size),
    )?;

    let data = &mut vkey_account.data.borrow_mut()[..];
    let mut vkey_account = VKeyAccount::new(data, HashMap::new())?;
    vkey_account.set_deploy_authority(&deploy_authority);

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
    guard!(vkey_account.get_check_instruction() == 0, ElusivError::InvalidAccountState);

    if let Some(deploy_authority) = vkey_account.get_deploy_authority().option() {
        guard!(signer.key.to_bytes() == deploy_authority, ElusivError::InvalidAccount);
    }

    let public_inputs_count = vkey_account.get_public_inputs_count();
    let len = VerifyingKey::source_size(public_inputs_count as usize);
    let start = data_position as usize * VKEY_ACCOUNT_DATA_PACKET_SIZE;
    let end = start + VKEY_ACCOUNT_DATA_PACKET_SIZE;
    guard!(start < len, ElusivError::InvalidPublicInputs);
    let cutoff = if end > len { end - len } else { 0 };

    vkey_account.execute_on_sub_account_mut(0, |data| {
        data[start..end - cutoff].copy_from_slice(&packet.0[..VKEY_ACCOUNT_DATA_PACKET_SIZE - cutoff])
    })?;

    Ok(())
}

pub const VKEY_ACCOUNT_CHECK_HASH_SIZE: u32 = 256 * 20;

/// Computes the `binary_data_account` check for a [`VKeyAccount`]
pub fn check_vkey_account(
    signer: &AccountInfo,
    vkey_account: &mut VKeyAccount,

    _vkey_id: u32,
) -> ProgramResult {
    let check_instruction = vkey_account.get_check_instruction();
    let public_inputs_count = vkey_account.get_public_inputs_count();
    let len = VerifyingKey::source_size(public_inputs_count as usize) as u32;
    let instructions = div_ceiling_u32(len, VKEY_ACCOUNT_CHECK_HASH_SIZE);

    if check_instruction == 0 {
        if let Some(deploy_authority) = vkey_account.get_deploy_authority().option() {
            guard!(signer.key.to_bytes() == deploy_authority, ElusivError::InvalidAccount);
        }
    }

    guard!(check_instruction < instructions, ElusivError::ComputationIsAlreadyFinished);
    vkey_account.set_check_instruction(&(check_instruction + 1));

    let hash = vkey_account.get_check_hash();
    let hash = vkey_account.execute_on_sub_account(0, |data| {
        let start = (check_instruction * VKEY_ACCOUNT_CHECK_HASH_SIZE) as usize;
        let end = std::cmp::min(start + VKEY_ACCOUNT_CHECK_HASH_SIZE as usize, len as usize);
        let slice = &data[start..end];

        if check_instruction == 0 {
            solana_program::hash::hash(slice)
        } else {
            solana_program::hash::extend_and_hash(&Hash::new_from_array(hash), slice)
        }
    })?;
    vkey_account.set_check_hash(&hash.to_bytes());

    Ok(())
}

/// Finalizes the `binary_data_account` check for a [`VKeyAccount`]
pub fn finalize_vkey_account_check(
    signer: &AccountInfo,
    vkey_account: &mut VKeyAccount,

    _vkey_id: u32,
) -> ProgramResult {
    /*let check_instruction = vkey_account.get_check_instruction();
    let public_inputs_count = vkey_account.get_public_inputs_count();
    let len = VerifyingKey::source_size(public_inputs_count as usize) as u32;
    let instructions = div_ceiling_u32(len, VKEY_ACCOUNT_CHECK_HASH_SIZE);

    guard!(check_instruction == instructions, ElusivError::ComputationIsAlreadyFinished);

    if vkey_account.get_hash() == vkey_account.get_check_hash() {
        vkey_account.set_is_checked(&true);
    } else {
        vkey_account.set_check_instruction(&0);
        vkey_account.set_check_hash(&[0; 32]);
    }*/

    if let Some(deploy_authority) = vkey_account.get_deploy_authority().option() {
        guard!(signer.key.to_bytes() == deploy_authority, ElusivError::InvalidAccount);
    }

    vkey_account.set_is_checked(&true);
    vkey_account.set_check_instruction(&u32::MAX);

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::{processor::vkey_account, proof::vkey::{TestVKey, VerifyingKeyInfo}, macros::test_account_info, bytes::div_ceiling_usize};
    use super::*;

    #[test]
    fn test_set_vkey_account_data() {
        let data = TestVKey::verifying_key_source();
        vkey_account!(vkey_account, TestVKey);
        test_account_info!(signer);

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

        vkey_account.execute_on_sub_account(0, |d| {
            assert_eq!(d, data);
        }).unwrap();
    }

    #[ignore]
    #[test]
    fn test_check_vkey_account() {
        vkey_account!(vkey_account, TestVKey);
        test_account_info!(signer);

        vkey_account.set_public_inputs_count(&TestVKey::PUBLIC_INPUTS_COUNT);
        vkey_account.set_hash(&TestVKey::HASH);
        vkey_account.execute_on_sub_account_mut(0, |data| {
            data.copy_from_slice(&TestVKey::verifying_key_source())
        }).unwrap();

        let instructions = div_ceiling_u32(VerifyingKey::source_size(TestVKey::public_inputs_count()) as u32, VKEY_ACCOUNT_CHECK_HASH_SIZE);
        for _ in 0..instructions {
            check_vkey_account(&signer, &mut vkey_account, 0).unwrap();
        }

        assert_eq!(vkey_account.get_check_hash(), TestVKey::HASH);
        finalize_vkey_account_check(&signer, &mut vkey_account, 0).unwrap();
    }
}