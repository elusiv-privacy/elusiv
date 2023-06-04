use crate::error::ElusivWardenNetworkError;
use crate::{
    network::ApaWardenNetworkAccount,
    warden::{ApaWardenAccount, BasicWardenMapAccount, ElusivWardenID, QuoteEnd, QuoteStart},
};
use elusiv_types::UnverifiedAccountInfo;
use elusiv_utils::{guard, open_pda_account_with_offset, pda_account};
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

/// Initialize the [`ApaWardenAccount`], register as a member of the network, store the first half
/// of the SGX quote.
///
/// # Notes
///
/// The application phase is the first step of the Elusiv Warden Network Protocol
/// Each Warden possesses a keypair (kwi, Kwi).
/// The APAE (Autonomous Protocol Analysis Environment) of every warden generated a seed exchange
/// keypair (xi, Xi), as well as an SGX quote embedding the APA seed (Xi).
/// The quote is stored on chain and every Warden will later verify the quotes of every other
/// Warden, ensuring that all run the same code on genuine Intel CPUs.
///
/// Because the quote is too large to be sent in a single transaction, only the first half is sent
/// here, and the rest is transmitted upon call to [`complete_apa_genesis_warden_application`].
pub fn start_apa_genesis_warden_application<'b>(
    warden: &AccountInfo<'b>,
    warden_map_account: &BasicWardenMapAccount,
    mut apa_warden_account: UnverifiedAccountInfo<'_, 'b>,
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

/// Complete the APA genesis Warden application by sending the other half of the SGX quote.
///
/// See [`start_apa_genesis_warden_application`]
pub fn complete_apa_genesis_warden_application<'a>(
    _warden: &AccountInfo<'a>,
    warden_map_account: &BasicWardenMapAccount,
    apa_network_account: &mut ApaWardenNetworkAccount,

    provided_warden_id: ElusivWardenID,
    quote_end: QuoteEnd,
) -> ProgramResult {
    let warden_id = warden_map_account.get_warden_id();
    guard!(
        provided_warden_id == warden_id,
        ElusivWardenNetworkError::InvalidInstructionData
    );
    apa_network_account.complete_application(warden_id, quote_end)?;

    Ok(())
}

/// Every Warden has verified the quotes of its peers and hashed these into a confirmation message.
/// This method verifies these confirmation messages and keeps track of the list of nodes having
/// confirmed its peers.
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
