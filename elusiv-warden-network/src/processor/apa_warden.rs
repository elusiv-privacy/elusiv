use crate::{
    network::ApaWardenNetworkAccount,
    warden::{ApaWardenAccount, BasicWardenMapAccount, ElusivWardenID, QuoteStart, QuoteEnd},
};
use elusiv_types::UnverifiedAccountInfo;
use elusiv_utils::{guard, open_pda_account_with_offset, pda_account};
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};
use crate::error::ElusivWardenNetworkError;

pub fn start_apa_genesis_warden_application<'a, 'b>(
    warden: &AccountInfo<'b>,
    warden_map_account: &BasicWardenMapAccount,
    mut apa_warden_account: UnverifiedAccountInfo<'a, 'b>,
    apa_network_account: &mut ApaWardenNetworkAccount,
    _warden_id: ElusivWardenID,
    quote_start: QuoteStart,
) -> ProgramResult {
    let warden_id = warden_map_account.get_warden_id();
    let network_member_index = apa_network_account.start_application(warden_id, &quote_start)?;

    open_pda_account_with_offset::<ApaWardenAccount>(
        &crate::id(),
        warden,
        apa_warden_account.get_unsafe_and_set_is_verified(),
        warden_id, // this enforces equality between the two client supplied warden_id's
        None,
    )?;

    pda_account!(
        mut apa_warden_account,
        ApaWardenAccount,
        apa_warden_account.get_safe()?
    );
    apa_warden_account.set_warden_id(&warden_id);
    apa_warden_account.set_network_member_index(&network_member_index);
    // apa_warden_account.set_latest_quote(&quote);

    Ok(())
}

pub fn complete_apa_genesis_warden_application<'a>(
    _warden: &AccountInfo<'a>,
    warden_map_account: &BasicWardenMapAccount,
    apa_network_account: &mut ApaWardenNetworkAccount,
    provided_warden_id: ElusivWardenID,
    quote_end: QuoteEnd,
) -> ProgramResult {
    let warden_id = warden_map_account.get_warden_id();
    guard!(provided_warden_id == warden_id, ElusivWardenNetworkError::InvalidInstructionData);
    apa_network_account.complete_application(warden_id, quote_end)?;

    Ok(())
}

pub fn confirm_apa_genesis_network(
    exchange_key_account: &AccountInfo,
    apa_warden_account: &ApaWardenAccount,
    apa_network_account: &mut ApaWardenNetworkAccount,

    _warden_id: ElusivWardenID,
    confirmation_message: [u8; 32],
) -> ProgramResult {
    apa_network_account.confirm_others(
        apa_warden_account.get_network_member_index() as usize,
        exchange_key_account.key,
        &confirmation_message,
    )
}

pub fn complete_apa_genesis_network() -> ProgramResult {
    todo!()
}
