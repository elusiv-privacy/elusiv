use crate::{
    error::ElusivError, processor::setup_child_account, proof::vkey::VerifyingKey,
    state::vkey::VKeyAccount,
};
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_types::{BorshSerDeSized, ChildAccountConfig, ElusivOption, ParentAccount};
use elusiv_utils::{
    guard, open_pda_account_with_offset, pda_account, transfer_with_system_program,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

pub const VKEY_ACCOUNT_DATA_PACKET_SIZE: usize = 964;
const MAX_NUMBER_OF_VKEYS: u32 = 1;

/// A binary data packet containing [`VKEY_ACCOUNT_DATA_PACKET_SIZE`] bytes
#[derive(BorshSerialize, BorshDeserialize)]
pub struct VKeyAccountDataPacket(pub Vec<u8>);

impl elusiv_types::BorshSerDeSized for VKeyAccountDataPacket {
    const SIZE: usize = VKEY_ACCOUNT_DATA_PACKET_SIZE + u32::SIZE;
}

/// Creates a new [`VKeyAccount`]
pub fn create_vkey_account<'a>(
    signer: &AccountInfo<'a>,
    vkey_account: &AccountInfo<'a>,

    vkey_id: u32,
    public_inputs_count: u32,
    authority: ElusivOption<Pubkey>,
) -> ProgramResult {
    guard!(
        vkey_id < MAX_NUMBER_OF_VKEYS,
        ElusivError::InvalidAccountState
    );

    open_pda_account_with_offset::<VKeyAccount>(&crate::id(), signer, vkey_account, vkey_id, None)?;

    pda_account!(mut vkey_account, VKeyAccount, vkey_account);
    vkey_account.set_authority(&authority);
    vkey_account.set_public_inputs_count(&public_inputs_count);

    Ok(())
}

pub fn create_new_vkey_version(
    signer: &AccountInfo,
    vkey_account: &mut VKeyAccount,
    vkey_binary_data_account: &AccountInfo,

    _vkey_id: u32,
) -> ProgramResult {
    verify_vkey_modification(signer, vkey_account)?;

    guard!(
        vkey_account.get_child_pubkey(1).is_none(),
        ElusivError::InvalidAccountState
    );

    let public_inputs_count = vkey_account.get_public_inputs_count() as usize;
    let binary_data_account_size =
        VerifyingKey::source_size(public_inputs_count) + ChildAccountConfig::SIZE;

    setup_child_account(
        vkey_account,
        vkey_binary_data_account,
        1,
        false, // don't care about zeroness, authority is allowed to submit any data
        Some(binary_data_account_size),
    )?;

    Ok(())
}

pub fn set_vkey_data(
    signer: &AccountInfo,
    vkey_account: &mut VKeyAccount,

    _vkey_id: u32,
    data_position: u32,
    packet: VKeyAccountDataPacket,
) -> ProgramResult {
    verify_vkey_modification(signer, vkey_account)?;

    let public_inputs_count = vkey_account.get_public_inputs_count();
    let len = VerifyingKey::source_size(public_inputs_count as usize);
    let start = data_position as usize * VKEY_ACCOUNT_DATA_PACKET_SIZE;
    let end = start + VKEY_ACCOUNT_DATA_PACKET_SIZE;
    let cutoff = if end > len { end - len } else { 0 };

    guard!(start < len, ElusivError::InvalidInstructionData);

    vkey_account.execute_on_child_account_mut(1, |data| {
        data[start..end - cutoff]
            .copy_from_slice(&packet.0[..VKEY_ACCOUNT_DATA_PACKET_SIZE - cutoff])
    })?;

    Ok(())
}

/// Updates a [`VKeyAccount`]
pub fn update_vkey_version<'a>(
    signer: &AccountInfo<'a>,
    vkey_account: &mut VKeyAccount,
    old_vkey_binary_data_account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,

    _vkey_id: u32,
) -> ProgramResult {
    verify_vkey_modification(signer, vkey_account)?;

    guard!(
        vkey_account.get_child_pubkey(1).is_some(),
        ElusivError::InvalidAccountState
    );

    // Close old vkey account
    if let Some(old_vkey_account) = vkey_account.get_child_pubkey(0) {
        guard!(
            old_vkey_account == *old_vkey_binary_data_account.key,
            ElusivError::InvalidAccount
        );

        transfer_with_system_program(
            old_vkey_binary_data_account,
            signer,
            system_program,
            old_vkey_binary_data_account.lamports(),
        )?;
    }

    // Swap child accounts
    vkey_account.set_child_pubkey(0, vkey_account.get_child_pubkey(1).into());
    vkey_account.set_child_pubkey(1, None.into());

    // Inc version
    let version = vkey_account.get_version();
    vkey_account.set_version(
        &version
            .checked_add(1)
            .ok_or(ElusivError::InvalidAccountState)?,
    );

    Ok(())
}

/// Freezes a [`VKeyAccount`]
pub fn freeze_vkey(
    signer: &AccountInfo,
    vkey_account: &mut VKeyAccount,

    _vkey_id: u32,
) -> ProgramResult {
    verify_vkey_modification(signer, vkey_account)?;
    vkey_account.set_is_frozen(&true);

    Ok(())
}

/// Changes the modification authority of a [`VKeyAccount`]
pub fn change_vkey_authority(
    signer: &AccountInfo,
    vkey_account: &mut VKeyAccount,

    _vkey_id: u32,
    authority: Pubkey,
) -> ProgramResult {
    verify_vkey_modification(signer, vkey_account)?;
    vkey_account.set_authority(&Some(authority).into());

    Ok(())
}

fn verify_vkey_modification(signer: &AccountInfo, vkey_account: &VKeyAccount) -> ProgramResult {
    guard!(
        !vkey_account.get_is_frozen(),
        ElusivError::InvalidAccountState
    );

    if let Some(authority) = vkey_account.get_authority().option() {
        guard!(
            *signer
                .signer_key()
                .ok_or(ProgramError::MissingRequiredSignature)?
                == authority,
            ElusivError::InvalidAccount
        );
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        bytes::div_ceiling_usize,
        macros::{signing_test_account_info, test_account_info},
        processor::vkey_account,
        proof::vkey::{TestVKey, VerifyingKeyInfo},
    };

    #[test]
    fn test_create_new_vkey_version() {
        vkey_account!(vkey_account, TestVKey);
        signing_test_account_info!(signer);

        let public_inputs_count = vkey_account.get_public_inputs_count() as usize;
        let binary_data_account_size =
            VerifyingKey::source_size(public_inputs_count) + ChildAccountConfig::SIZE;

        test_account_info!(valid_vkey_binary_data_account, binary_data_account_size);
        test_account_info!(
            invalid_vkey_binary_data_account,
            binary_data_account_size - 1
        );

        vkey_account.set_child_pubkey(1, Some(*valid_vkey_binary_data_account.key).into());

        // Child already exists
        assert_eq!(
            create_new_vkey_version(
                &signer,
                &mut vkey_account,
                &valid_vkey_binary_data_account,
                0
            ),
            Err(ElusivError::InvalidAccountState.into())
        );

        vkey_account.set_child_pubkey(1, None.into());

        // Invalid size
        assert_eq!(
            create_new_vkey_version(
                &signer,
                &mut vkey_account,
                &invalid_vkey_binary_data_account,
                0
            ),
            Err(ProgramError::InvalidAccountData)
        );

        // Child account already in use
        test_account_info!(invalid_vkey_binary_data_account, binary_data_account_size);
        invalid_vkey_binary_data_account.data.borrow_mut()[0] = 1;
        assert_eq!(
            create_new_vkey_version(
                &signer,
                &mut vkey_account,
                &invalid_vkey_binary_data_account,
                0
            ),
            Err(ProgramError::AccountAlreadyInitialized)
        );

        vkey_account!(vkey_account, TestVKey);

        assert_eq!(
            create_new_vkey_version(
                &signer,
                &mut vkey_account,
                &valid_vkey_binary_data_account,
                0
            ),
            Ok(())
        );
    }

    #[test]
    fn test_set_vkey_data() {
        let data = TestVKey::verifying_key_source();
        vkey_account!(vkey_account, TestVKey);
        signing_test_account_info!(signer);

        vkey_account
            .execute_on_child_account_mut(1, |d| {
                for b in d.iter_mut() {
                    *b = 0;
                }
            })
            .unwrap();

        let positions = div_ceiling_usize(
            VerifyingKey::source_size(TestVKey::public_inputs_count()),
            VKEY_ACCOUNT_DATA_PACKET_SIZE,
        );
        for i in 0..positions {
            let slice = &data[i * VKEY_ACCOUNT_DATA_PACKET_SIZE
                ..std::cmp::min((i + 1) * VKEY_ACCOUNT_DATA_PACKET_SIZE, data.len())];
            set_vkey_data(
                &signer,
                &mut vkey_account,
                0,
                i as u32,
                VKeyAccountDataPacket(slice.to_vec()),
            )
            .unwrap();
        }

        vkey_account
            .execute_on_child_account(1, |d| {
                assert_eq!(d, data);
            })
            .unwrap();
    }

    #[test]
    fn test_update_vkey_account() {
        vkey_account!(vkey_account, TestVKey);
        signing_test_account_info!(signer);
        test_account_info!(acc);
        test_account_info!(vkey_binary_data_account);

        assert_eq!(vkey_account.get_version(), 0);
        vkey_account.set_authority(&Some(*signer.key).into());

        assert_eq!(
            update_vkey_version(&signer, &mut vkey_account, &acc, &acc, 0),
            Err(ElusivError::InvalidAccountState.into())
        );

        vkey_account.set_child_pubkey(0, None.into());
        vkey_account.set_child_pubkey(1, Some(*vkey_binary_data_account.key).into());

        assert_eq!(
            update_vkey_version(&signer, &mut vkey_account, &acc, &acc, 0),
            Ok(())
        );

        assert_eq!(vkey_account.get_version(), 1);
        assert_eq!(
            vkey_account.get_child_pubkey(0).unwrap(),
            *vkey_binary_data_account.key
        );
        assert!(vkey_account.get_child_pubkey(1).is_none());
    }

    #[test]
    fn test_freeze_vkey() {
        vkey_account!(vkey_account, TestVKey);
        signing_test_account_info!(signer);

        vkey_account.set_public_inputs_count(&TestVKey::PUBLIC_INPUTS_COUNT);
        vkey_account
            .execute_on_child_account_mut(0, |data| {
                data.copy_from_slice(&TestVKey::verifying_key_source())
            })
            .unwrap();

        freeze_vkey(&signer, &mut vkey_account, 0).unwrap();

        assert!(vkey_account.get_is_frozen());
        assert_eq!(
            freeze_vkey(&signer, &mut vkey_account, 0),
            Err(ElusivError::InvalidAccountState.into())
        );
    }

    #[test]
    fn test_change_vkey_authority() {
        vkey_account!(vkey_account, TestVKey);
        signing_test_account_info!(signer);
        signing_test_account_info!(signer2);

        assert_eq!(
            change_vkey_authority(&signer, &mut vkey_account, 0, *signer.key),
            Ok(())
        );

        assert_eq!(
            change_vkey_authority(&signer2, &mut vkey_account, 0, *signer.key),
            Err(ElusivError::InvalidAccount.into())
        );

        assert_eq!(
            change_vkey_authority(&signer, &mut vkey_account, 0, *signer2.key),
            Ok(())
        );

        assert_eq!(
            change_vkey_authority(&signer, &mut vkey_account, 0, *signer.key),
            Err(ElusivError::InvalidAccount.into())
        );
    }

    #[test]
    fn test_verify_vkey_modification() {
        vkey_account!(vkey_account, TestVKey);
        signing_test_account_info!(signer);
        signing_test_account_info!(invalid_signer);

        // Any signer allowed
        assert_eq!(
            verify_vkey_modification(&invalid_signer, &vkey_account),
            Ok(())
        );

        vkey_account.set_authority(&Some(*signer.key).into());

        // Valid signer
        assert_eq!(verify_vkey_modification(&signer, &vkey_account), Ok(()));

        // Invalid authority
        assert_eq!(
            verify_vkey_modification(&invalid_signer, &vkey_account),
            Err(ElusivError::InvalidAccount.into())
        );

        vkey_account.set_is_frozen(&true);

        // Frozen account
        assert_eq!(
            verify_vkey_modification(&signer, &vkey_account),
            Err(ElusivError::InvalidAccountState.into())
        );

        // Valid account is not signer
        vkey_account!(vkey_account, TestVKey);
        test_account_info!(signer);
        vkey_account.set_authority(&Some(*signer.key).into());
        assert_eq!(
            verify_vkey_modification(&signer, &vkey_account),
            Err(ProgramError::MissingRequiredSignature)
        );
    }
}
