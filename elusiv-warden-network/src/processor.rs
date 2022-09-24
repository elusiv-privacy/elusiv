use elusiv_utils::{open_pda_account_without_offset, open_pda_account_with_offset};
use solana_program::pubkey::Pubkey;
use std::net::Ipv4Addr;
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use crate::apa::{APAProposalKind, APAProposal, APAECert, APAConfig};
use crate::proposal::Vote;
use crate::warden::{ElusivFullWardenAccount, ElusivWardenID, ElusivWardensAccount, ElusivFullWarden, ElusivBasicWarden};
use crate::network::{FullWardenRegistrationAccount, FullWardenRegistrationApplication};

pub fn init<'a>(
    payer: &AccountInfo<'a>,
    warden_registration: &AccountInfo<'a>,
    wardens: &AccountInfo<'a>,
) -> ProgramResult {
    open_pda_account_without_offset::<FullWardenRegistrationAccount>(
        &crate::id(),
        payer,
        warden_registration,
    )?;

    open_pda_account_without_offset::<ElusivWardensAccount>(
        &crate::id(),
        payer,
        wardens,
    )
}

/// A Full Warden (with the corresponding APAE) applies for registration in the Genesis Full Warden Network
pub fn apply_full_genesis_warden<'a>(
    warden: &AccountInfo<'a>,
    warden_account: &AccountInfo<'a>,
    warden_registration: &mut FullWardenRegistrationAccount,
    wardens: &mut ElusivWardensAccount,

    warden_id: ElusivWardenID,
    apae_cert: APAECert,
    addr: Ipv4Addr,
) -> ProgramResult {
    let warden_key = *warden.key;
    
    warden_registration.register_applicant(
        &warden_key,
        warden_id,
        FullWardenRegistrationApplication {
            apae_cert,
        }
    )?;

    wardens.add_full_warden(
        warden,
        ElusivFullWarden {
            warden: ElusivBasicWarden {
                key: *warden.key,
                addr,
                active: true,
            },
            apae_key: Pubkey::new_from_array([0; 32]),
        },
        warden_account,
    )?;

    open_pda_account_with_offset::<ElusivFullWardenAccount>(
        &crate::id(),
        warden,
        warden_account,
        warden_id,
    )?;

    Ok(())
}

/// A registering Warden (+ each APAE) approves all other applicants
pub fn confirm_full_genesis_warden(
    warden: &AccountInfo,
    warden_account: &ElusivFullWardenAccount,
    warden_registration: &mut FullWardenRegistrationAccount,

    warden_id: ElusivWardenID,
) -> ProgramResult {
    warden_account.verify(warden)?;
    warden_registration.confirm_all_other_applications(
        warden.key,
        warden_id,
    )?;

    Ok(())
}

/// The registration leader generates the APA config
pub fn complete_full_genesis_warden(
    warden: &AccountInfo,
    warden_account: &ElusivFullWardenAccount,
    _warden_registration: &FullWardenRegistrationAccount,
    _warden_network: &AccountInfo,

    _leader_id: ElusivWardenID,
    _apa_config: APAConfig,
) -> ProgramResult {
    warden_account.verify(warden)?;

    todo!()

    // Verify apae signature
    // Create network account
}

pub fn init_apa_proposal(
    _kind: APAProposalKind,
    _proposal: APAProposal,
) -> ProgramResult {
    todo!()
}

pub fn vote_apa_proposal(
    warden: &AccountInfo,
    warden_account: &ElusivFullWardenAccount,

    _warden_id: ElusivWardenID,
    _vote: Vote,
) -> ProgramResult {
    warden_account.verify(warden)?;
    
    todo!()
}

pub fn finalize_apa_proposal(
    warden: &AccountInfo,
    warden_account: &ElusivFullWardenAccount,

    _leader_id: ElusivWardenID,
) -> ProgramResult {
    warden_account.verify(warden)?;

    todo!()
}