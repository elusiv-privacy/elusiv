use super::current_timestamp;
use crate::apa::{
    ApaProponentRole, ApaProposal, ApaProposalAccount, ApaProposalsAccount, ApaTargetMapAccount,
};
use crate::error::ElusivWardenNetworkError;
use elusiv_types::{elusiv_token, UnverifiedAccountInfo, SPL_TOKEN_COUNT};
use elusiv_utils::{
    guard, open_pda_account_with_associated_pubkey, open_pda_account_with_offset, pda_account,
};
use solana_program::program_error::ProgramError;
use solana_program::program_option::COption;
use solana_program::program_pack::Pack;
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

/// Processes an [`ApaProposal`]
pub fn propose_apa_proposal<'b>(
    proponent: &AccountInfo<'b>,
    mut proposal_account: UnverifiedAccountInfo<'_, 'b>,
    proposals_account: &mut ApaProposalsAccount,
    target_map_account: &AccountInfo<'b>,
    token_mint: &AccountInfo,

    proposal_id: u32,
    proposal: ApaProposal,
) -> ProgramResult {
    let proposal_count = proposals_account.get_number_of_proposals();

    guard!(
        proposal_id == proposal_count,
        ElusivWardenNetworkError::ProposalError
    );

    let mut proposal = proposal;
    proposal.timestamp = current_timestamp()?;
    proposal.proponent = *proponent.key;

    if let Some(token_id) = proposal.token_constraint.option() {
        guard!(
            token_id as usize <= SPL_TOKEN_COUNT,
            ElusivWardenNetworkError::ProposalError
        );

        match proposal.proponent_role {
            ApaProponentRole::Default => {}
            ApaProponentRole::TokenFreezingAuthority => {
                guard!(
                    *token_mint.key == elusiv_token(token_id)?.mint,
                    ElusivWardenNetworkError::ProposalError
                );
                guard!(token_id > 0, ElusivWardenNetworkError::ProposalError);

                let data = &token_mint.data.borrow()[..];
                let token_account = spl_token::state::Mint::unpack(data)?;
                match token_account.freeze_authority {
                    COption::Some(freeze_authority) => {
                        guard!(
                            freeze_authority == *proponent.key,
                            ElusivWardenNetworkError::ProposalError
                        );
                    }
                    COption::None => return Err(ElusivWardenNetworkError::ProposalError.into()),
                }
            }
        }
    }

    open_pda_account_with_offset::<ApaProposalAccount>(
        &crate::id(),
        proponent,
        proposal_account.get_unsafe_and_set_is_verified(),
        proposal_id,
        None,
    )?;

    if target_map_account.lamports() == 0 {
        open_pda_account_with_associated_pubkey::<ApaTargetMapAccount>(
            &crate::id(),
            proponent,
            target_map_account,
            &proposal.target,
            None,
            None,
        )?;
    }

    pda_account!(
        mut proposal_account,
        ApaProposalAccount,
        proposal_account.get_safe()?
    );
    proposal_account.set_proposal(&proposal);

    proposals_account.set_number_of_proposals(
        &proposal_count
            .checked_add(1)
            .ok_or_else(|| ProgramError::from(ElusivWardenNetworkError::ProposalError))?,
    );

    Ok(())
}
