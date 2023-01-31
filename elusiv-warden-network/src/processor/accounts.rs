use crate::{
    apa::ApaProposalsAccount,
    network::BasicWardenNetworkAccount,
    warden::{WardenRegion, WardensAccount},
};
use elusiv_utils::{open_pda_account_with_offset, open_pda_account_without_offset};
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

pub fn init<'a>(
    payer: &AccountInfo<'a>,
    wardens_account: &AccountInfo<'a>,
    basic_network_account: &AccountInfo<'a>,
    apa_proposals_account: &AccountInfo<'a>,
) -> ProgramResult {
    open_pda_account_without_offset::<WardensAccount>(&crate::id(), payer, wardens_account, None)?;

    open_pda_account_without_offset::<BasicWardenNetworkAccount>(
        &crate::id(),
        payer,
        basic_network_account,
        None,
    )?;

    open_pda_account_without_offset::<ApaProposalsAccount>(
        &crate::id(),
        payer,
        apa_proposals_account,
        None,
    )?;

    Ok(())
}

pub fn init_region_account<'a>(
    payer: &AccountInfo<'a>,
    basic_network_account: &AccountInfo<'a>,

    region: WardenRegion,
) -> ProgramResult {
    open_pda_account_with_offset::<BasicWardenNetworkAccount>(
        &crate::id(),
        payer,
        basic_network_account,
        region.pda_offset(),
        None,
    )
}

/// Closes a program owned account in devnet and localhost
///
/// # Note
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
