use elusiv_utils::{open_pda_account_without_offset, guard};
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_program::sysvar::Sysvar;
use solana_program::{account_info::AccountInfo, clock::Clock};
use solana_program::entrypoint::ProgramResult;
use crate::warden::ElusivBasicWardenAccount;
use crate::{
    warden::{ElusivWardensAccount, ElusivWardenID, ElusivBasicWardenConfig, ElusivBasicWarden},
    network::ElusivBasicWardenNetworkAccount,
};

pub fn init<'a>(
    payer: &AccountInfo<'a>,
    wardens: &AccountInfo<'a>,
    basic_network: &AccountInfo<'a>,
) -> ProgramResult {
    open_pda_account_without_offset::<ElusivWardensAccount>(
        &crate::id(),
        payer,
        wardens,
    )?;

    open_pda_account_without_offset::<ElusivBasicWardenNetworkAccount>(
        &crate::id(),
        payer,
        basic_network,
    )?;

    Ok(())
}

pub fn register_basic_warden<'a>(
    warden: &AccountInfo<'a>,
    warden_account: &AccountInfo<'a>,
    wardens_account: &mut ElusivWardensAccount,
    basic_network_account: &mut ElusivBasicWardenNetworkAccount,

    warden_id: ElusivWardenID,
    config: ElusivBasicWardenConfig,
) -> ProgramResult {
    let current_timestamp = current_timestamp()?;
    let basic_warden = ElusivBasicWarden {
        warden_id,
        config,
        lut: Pubkey::new_from_array([0; 32]),
        is_active: false,
        activation_timestamp: current_timestamp,
        join_timestamp: current_timestamp,
    };
    wardens_account.add_basic_warden(warden, basic_warden, warden_account)?;
    basic_network_account.try_add_member(warden_id)?;
    
    Ok(())
}

pub fn update_basic_warden_state(
    warden: &AccountInfo,
    warden_account: &mut ElusivBasicWardenAccount,

    _warden_id: ElusivWardenID,
    is_active: bool,
) -> ProgramResult {
    let mut basic_warden = warden_account.get_warden();
    guard!(*warden.key == basic_warden.config.key, ProgramError::MissingRequiredSignature);

    if is_active && !basic_warden.is_active {
        basic_warden.activation_timestamp = current_timestamp()?;
    }
    basic_warden.is_active = is_active;
    warden_account.set_warden(&basic_warden);

    Ok(())
}

pub fn update_basic_warden_lut(
    warden: &AccountInfo,
    warden_account: &mut ElusivBasicWardenAccount,
    lut_account: &AccountInfo,

    _warden_id: ElusivWardenID,
) -> ProgramResult {
    // TODO: verify lut_account to be a valid, frozen LUT

    let mut basic_warden = warden_account.get_warden();
    guard!(*warden.key == basic_warden.config.key, ProgramError::MissingRequiredSignature);

    basic_warden.lut = *lut_account.key;
    warden_account.set_warden(&basic_warden);

    Ok(())
}

pub fn close_basic_warden<'a>(
    _warden: &AccountInfo<'a>,
    _warden_account: &AccountInfo<'a>,

    _warden_id: ElusivWardenID,
) -> ProgramResult {
    todo!()
}

fn current_timestamp() -> Result<u64, ProgramError> {
    if !cfg!(test) {
        let clock = Clock::get()?;
        Ok(clock.unix_timestamp.try_into().unwrap())
    } else {
        Ok(0)
    }
}