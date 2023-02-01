use crate::{
    network::ApaWardenNetworkAccount,
    warden::{ApaWardenRegistrationAccount, BasicWardenMapAccount, RAQuote},
};
use elusiv_utils::{open_pda_account_with_offset, pda_account};
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

pub fn apply_apa_genesis_warden<'a>(
    warden: &AccountInfo<'a>,
    warden_map_account: &BasicWardenMapAccount,
    apa_warden_account: &AccountInfo<'a>,
    apa_network_account: &mut ApaWardenNetworkAccount,

    quote: RAQuote,
) -> ProgramResult {
    let warden_id = warden_map_account.get_warden_id();
    apa_network_account.apply(warden_id)?;

    open_pda_account_with_offset::<ApaWardenRegistrationAccount>(
        &crate::id(),
        warden,
        apa_warden_account,
        warden_id,
        None,
    )?;

    pda_account!(
        mut full_warden_account,
        ApaWardenRegistrationAccount,
        apa_warden_account
    );
    full_warden_account.set_quote(&quote);
    full_warden_account.set_warden_id(&warden_id);

    Ok(())
}

pub fn confirm_apa_genesis_network(
    warden_map_account: &BasicWardenMapAccount,
    apa_network_account: &mut ApaWardenNetworkAccount,

    member_index: u32,
    confirm: bool,
) -> ProgramResult {
    let warden_id = warden_map_account.get_warden_id();

    apa_network_account.confirm_others(warden_id, member_index as usize, confirm)
}

pub fn complete_apa_genesis_network() -> ProgramResult {
    todo!()
}
