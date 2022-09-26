use elusiv_utils::{open_pda_account_without_offset, guard, open_pda_account_with_offset, pda_account};
use elusiv_types::ProgramAccount;
use solana_program::clock::Clock;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_program::sysvar::Sysvar;
use std::net::Ipv4Addr;
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use crate::apa::{APAProposal, APAECert, APAConfig, APAProposalAccount};
use crate::proposal::{Proposal, Vote, ProposalAccount, ProposalVotingAccount};
use crate::warden::{ElusivFullWardenAccount, ElusivWardenID, ElusivWardensAccount, ElusivFullWarden, ElusivBasicWarden, ElusivBasicWardenAccount};
use crate::network::{FullWardenRegistrationAccount, FullWardenRegistrationApplication, ElusivFullWardenNetworkAccount};
use crate::error::ElusivWardenNetworkError;

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

// -------- Full Warden Genesis Network Setup --------

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
    warden_registration.register_applicant(
        warden.key,
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
    wardens: &mut ElusivWardensAccount,
    _warden_network: &AccountInfo,

    _leader_id: ElusivWardenID,
    _apa_config: APAConfig,
) -> ProgramResult {
    warden_account.verify(warden)?;

    wardens.set_full_network_configured(&true);

    todo!()

    // Verify apae signature
    // Create full network account
    // Create basic network account
    // Add members to full network
    // Add members to basic network
}

// -------- Basic Warden Genesis Network Setup --------

pub fn apply_basic_genesis_warden<'a>(
    warden: &AccountInfo<'a>,
    warden_account: &AccountInfo<'a>,
    wardens: &mut ElusivWardensAccount,

    _warden_id: ElusivWardenID,
    addr: Ipv4Addr,
) -> ProgramResult {
    wardens.add_basic_warden(
        warden,
        ElusivBasicWarden {
            key: *warden.key,
            addr,
            active: false,
        },
        warden_account,
    )
}

const BASIC_GENESIS_WARDEN_AUTHORITY: Pubkey = Pubkey::new_from_array([0; 32]);

pub fn confirm_basic_genesis_warden(
    basic_warden_authority: &AccountInfo,
    warden_account: &mut ElusivBasicWardenAccount,

    _warden_id: ElusivWardenID,
) -> ProgramResult {
    guard!(*basic_warden_authority.key == BASIC_GENESIS_WARDEN_AUTHORITY, ElusivWardenNetworkError::InvalidSignature);

    let mut warden = warden_account.get_warden();
    warden.active = true;
    warden_account.set_warden(&warden);

    Ok(())
}

// -------- APA Proposals --------

pub fn init_apa_proposal<'a>(
    proponent: &AccountInfo<'a>,
    proposal_account: &AccountInfo<'a>,
    network: &ElusivFullWardenNetworkAccount,

    proposal_id: u32,
    proposal: APAProposal,
) -> ProgramResult {
    let timestamp = current_timestamp()?;

    guard!(proposal.is_proponent_valid(None), ElusivWardenNetworkError::ProposalError);

    open_pda_account_with_offset::<APAProposalAccount>(
        &crate::id(),
        proponent,
        proposal_account,
        proposal_id,
    )?;

    pda_account!(mut proposal_account, APAProposalAccount, proposal_account);
    proposal_account.init(
        proposal,
        timestamp,
        &network.copy_members(),
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn vote_apa_proposal(
    warden: &AccountInfo,
    warden_account: &ElusivFullWardenAccount,
    proposal_account: &mut APAProposalAccount,

    warden_id: ElusivWardenID,
    _proposal_id: u32,
    vote: Vote,
) -> ProgramResult {
    warden_account.verify(warden)?;
    proposal_account.try_vote(vote, warden_id, current_timestamp()?)?;

    Ok(())
}

pub fn finalize_apa_proposal(
    warden: &AccountInfo,
    warden_account: &ElusivFullWardenAccount,
    proposal_account: &mut APAProposalAccount,

    warden_id: ElusivWardenID,
    _proposal_id: u32,
) -> ProgramResult {
    warden_account.verify(warden)?;
    guard!(proposal_account.is_consensus_reached(), ElusivWardenNetworkError::ProposalError);

    todo!()
}

fn current_timestamp() -> Result<u64, ProgramError> {
    if !cfg!(test) {
        let clock = Clock::get()?;
        clock.unix_timestamp.try_into()
            .or(Err(ProgramError::UnsupportedSysvar))
    } else {
        Ok(0)
    }
}