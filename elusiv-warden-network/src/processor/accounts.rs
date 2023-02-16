use crate::{
    apa::ApaProposalsAccount,
    network::{ApaWardenNetworkAccount, BasicWardenNetworkAccount},
    warden::WardensAccount,
};
use elusiv_types::UnverifiedAccountInfo;
use elusiv_utils::open_pda_account_without_offset;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

pub fn init<'a, 'b>(
    payer: &AccountInfo<'b>,
    wardens_account: UnverifiedAccountInfo<'a, 'b>,
    basic_network_account: UnverifiedAccountInfo<'a, 'b>,
    apa_network_account: UnverifiedAccountInfo<'a, 'b>,
    apa_proposals_account: UnverifiedAccountInfo<'a, 'b>,
) -> ProgramResult {
    open_pda_account_without_offset::<WardensAccount>(
        &crate::id(),
        payer,
        wardens_account.get_unsafe(),
        None,
    )?;
    open_pda_account_without_offset::<BasicWardenNetworkAccount>(
        &crate::id(),
        payer,
        basic_network_account.get_unsafe(),
        None,
    )?;
    open_pda_account_without_offset::<ApaWardenNetworkAccount>(
        &crate::id(),
        payer,
        apa_network_account.get_unsafe(),
        None,
    )?;
    open_pda_account_without_offset::<ApaProposalsAccount>(
        &crate::id(),
        payer,
        apa_proposals_account.get_unsafe(),
        None,
    )?;

    Ok(())
}

/// Closes a program owned account in devnet and localhost
///
/// # Notes
///
/// - `signer` needs to be the program's keypair
/// - `recipient` receives the accounts Lamports
#[cfg(not(feature = "mainnet"))]
pub fn close_program_account<'a>(
    signer: &AccountInfo,
    recipient: &AccountInfo<'a>,
    program_account: &AccountInfo<'a>,
) -> ProgramResult {
    assert!(!cfg!(feature = "mainnet"));
    assert_eq!(*signer.key, crate::ID);

    elusiv_utils::close_account(recipient, program_account)
}
