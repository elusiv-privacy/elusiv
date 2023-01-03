use elusiv_utils::open_pda_account_without_offset;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};
use crate::{apa::ApaProposalsAccount, warden::WardensAccount, network::BasicWardenNetworkAccount};

pub fn init<'a>(
    payer: &AccountInfo<'a>,
    wardens_account: &AccountInfo<'a>,
    basic_network_account: &AccountInfo<'a>,
    apa_proposals_account: &AccountInfo<'a>,
) -> ProgramResult {
    open_pda_account_without_offset::<WardensAccount>(
        &crate::id(),
        payer,
        wardens_account,
    )?;

    open_pda_account_without_offset::<BasicWardenNetworkAccount>(
        &crate::id(),
        payer,
        basic_network_account,
    )?;

    open_pda_account_without_offset::<ApaProposalsAccount>(
        &crate::id(),
        payer,
        apa_proposals_account,
    )?;

    Ok(())
}

/// Closes a program owned account in devnet and localhost
/// 
/// # Note
/// 
/// - `signer` needs to be the program's keypair
#[cfg(not(feature = "mainnet"))]
pub fn close_program_account<'a>(
    signer: &AccountInfo<'a>,
    account: &AccountInfo<'a>,
) -> ProgramResult {
    assert!(!cfg!(feature = "mainnet"));
    assert_eq!(*signer.key, crate::ID);

    elusiv_utils::close_account(signer, account)
}