use elusiv_types::ProgramAccount;
use elusiv_utils::{guard, open_pda_account_with_offset, pda_account, open_pda_account_with_associated_pubkey};
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::sysvar::instructions;
use crate::error::ElusivWardenNetworkError;
use crate::processor::current_timestamp;
use crate::warden::{BasicWardenAccount, BasicWardenStatsAccount, BasicWardenMapAccount};
use crate::{
    warden::{WardensAccount, ElusivWardenID, ElusivBasicWardenConfig, ElusivBasicWarden},
    network::BasicWardenNetworkAccount,
};
use super::get_day_and_year;

pub fn register_basic_warden<'a>(
    warden: &AccountInfo<'a>,
    warden_account: &AccountInfo<'a>,
    warden_map_account: &AccountInfo<'a>,
    wardens_account: &mut WardensAccount,
    basic_network_account: &mut BasicWardenNetworkAccount,

    warden_id: ElusivWardenID,
    config: ElusivBasicWardenConfig,
) -> ProgramResult {
    guard!(config.key == *warden.key, ProgramError::InvalidArgument);

    let current_timestamp = current_timestamp()?;
    let basic_warden = ElusivBasicWarden {
        config,
        lut: Pubkey::new_from_array([0; 32]),
        is_active: false,
        activation_timestamp: current_timestamp,
        join_timestamp: current_timestamp,
    };

    guard!(warden_id == wardens_account.get_next_warden_id(), ProgramError::InvalidArgument);
    wardens_account.set_next_warden_id(
        &warden_id.checked_add(1)
            .ok_or_else(|| ProgramError::from(ElusivWardenNetworkError::WardenRegistrationError))?
    );

    open_pda_account_with_offset::<BasicWardenAccount>(
        &crate::id(),
        warden,
        warden_account,
        warden_id,
        None,
    )?;

    pda_account!(mut warden_account, BasicWardenAccount, warden_account);
    warden_account.set_warden(&basic_warden);

    // `warden_map_account` is used to store the `warden_id` and prevent duplicate registrations
    open_pda_account_with_associated_pubkey::<BasicWardenMapAccount>(
        &crate::id(),
        warden,
        warden_map_account,
        warden.key,
        None,
        None,
    )?;

    pda_account!(mut warden_map_account, BasicWardenMapAccount, warden_map_account);
    warden_map_account.set_warden_id(&warden_id);

    basic_network_account.try_add_member(warden_id)?;
    
    Ok(())
}

pub fn update_basic_warden_state(
    warden: &AccountInfo,
    warden_account: &mut BasicWardenAccount,

    _warden_id: ElusivWardenID,
    is_active: bool,
) -> ProgramResult {
    let mut basic_warden = warden_account.get_warden();
    guard!(*warden.key == basic_warden.config.key, ProgramError::MissingRequiredSignature);

    // `activation_timestamp` is used to track all `is_active` changes
    if is_active != basic_warden.is_active {
        basic_warden.activation_timestamp = current_timestamp()?;
    }
    basic_warden.is_active = is_active;
    warden_account.set_warden(&basic_warden);

    Ok(())
}

pub fn update_basic_warden_lut(
    warden: &AccountInfo,
    warden_account: &mut BasicWardenAccount,
    lut_account: &AccountInfo,

    _warden_id: ElusivWardenID,
) -> ProgramResult {
    // TODO: verify lut_account to be a valid, frozen LUT (but not required ATM)

    let mut basic_warden = warden_account.get_warden();
    guard!(*warden.key == basic_warden.config.key, ProgramError::MissingRequiredSignature);

    basic_warden.lut = *lut_account.key;
    warden_account.set_warden(&basic_warden);

    Ok(())
}

pub fn open_basic_warden_stats_account<'a>(
    warden: &AccountInfo,
    payer: &AccountInfo<'a>,
    stats_account: &AccountInfo<'a>,

    year: u16,
) -> ProgramResult {
    open_pda_account_with_associated_pubkey::<BasicWardenStatsAccount>(
        &crate::id(),
        payer,
        stats_account,
        warden.key,
        Some(year as u32),
        None,
    )?;

    pda_account!(mut stats_account, BasicWardenStatsAccount, stats_account);
    stats_account.set_year(&year);

    Ok(())
}

const ELUSIV_PROGRAM_ID: Pubkey = crate::macros::program_id!(elusiv);

pub struct TrackableElusivInstruction {
    pub instruction_id: u8,
    pub warden_index: u8,
}

pub const TRACKABLE_ELUSIV_INSTRUCTIONS: [TrackableElusivInstruction; 3] = [
    // FinalizeBaseCommitmentHash
    TrackableElusivInstruction {
        instruction_id: 2,
        warden_index: 0,
    },

    // FinalizeVerificationTransferLamports
    TrackableElusivInstruction {
        instruction_id: 13,
        warden_index: 1,
    },

    // FinalizeVerificationTransferToken
    TrackableElusivInstruction {
        instruction_id: 14,
        warden_index: 3,
    }
];

pub fn track_basic_warden_stats(
    warden: &AccountInfo,
    stats_account: &mut BasicWardenStatsAccount,
    instructions_account: &AccountInfo,

    year: u16,
) -> ProgramResult {
    let (day, y) = get_day_and_year()?;
    guard!(y == year, ElusivWardenNetworkError::StatsError);

    guard!(stats_account.get_year() == year, ElusivWardenNetworkError::StatsError);

    let index = instructions::load_current_index_checked(instructions_account)?;
    let previous_ix = instructions::load_instruction_at_checked(
        index.checked_sub(1).ok_or(ElusivWardenNetworkError::StatsError)? as usize,
        instructions_account,
    )?;

    let ix_byte = previous_ix.data[0];
    if let Some(ix) = TRACKABLE_ELUSIV_INSTRUCTIONS.iter().find(|i| {
        i.instruction_id == ix_byte
    }) {
        guard!(previous_ix.accounts[ix.warden_index as usize].pubkey == *warden.key, ElusivWardenNetworkError::StatsError);
        guard!(previous_ix.program_id == ELUSIV_PROGRAM_ID, ProgramError::IncorrectProgramId);

        stats_account.set_store(stats_account.get_store().inc(day)?);
    } else {
        return Err(ElusivWardenNetworkError::StatsError.into())
    }

    Ok(())
}