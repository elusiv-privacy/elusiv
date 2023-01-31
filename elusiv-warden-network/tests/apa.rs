mod common;

use common::*;
use elusiv_types::{ElusivOption, UserAccount, WritableSignerAccount, SPL_TOKEN_COUNT};
use elusiv_warden_network::{
    apa::{ApaLevel, ApaProponentRole, ApaProposal, ApaProposalAccount, ApaProposalsAccount},
    instruction::ElusivWardenNetworkInstruction,
};
use solana_program::pubkey::Pubkey;
use solana_program_test::*;

#[tokio::test]
async fn test_propose_apa_proposal() {
    let mut test = start_test_with_setup().await;

    let proposal = ApaProposal {
        proponent: Pubkey::new_from_array([0; 32]),
        timestamp: 0,
        proponent_role: ApaProponentRole::Default,
        level: ApaLevel::Outcast,
        token_constraint: ElusivOption::None,
        target: Pubkey::new_unique(),
        reason: String::new().try_into().unwrap(),
    };

    // Invalid proposal_id
    test.ix_should_fail_simple(
        ElusivWardenNetworkInstruction::propose_apa_proposal_instruction(
            1,
            proposal.clone(),
            WritableSignerAccount(test.payer()),
            UserAccount(Pubkey::new_unique()),
        ),
    )
    .await;

    // Invalid token_id
    let mut proposal_1 = proposal.clone();
    proposal_1.token_constraint = ElusivOption::Some(SPL_TOKEN_COUNT as u16 + 1);
    test.ix_should_fail_simple(
        ElusivWardenNetworkInstruction::propose_apa_proposal_instruction(
            0,
            proposal_1,
            WritableSignerAccount(test.payer()),
            UserAccount(Pubkey::new_unique()),
        ),
    )
    .await;

    for proposal_id in 0..3 {
        test.ix_should_succeed_simple(
            ElusivWardenNetworkInstruction::propose_apa_proposal_instruction(
                proposal_id,
                proposal.clone(),
                WritableSignerAccount(test.payer()),
                UserAccount(Pubkey::new_unique()),
            ),
        )
        .await;
    }

    assert_eq!(
        3,
        test.eager_account::<ApaProposalsAccount, _>(None)
            .await
            .number_of_proposals
    );

    let apa_proposal_account = test.eager_account::<ApaProposalAccount, _>(Some(0)).await;
    let mut proposal = proposal;
    proposal.timestamp = apa_proposal_account.proposal.timestamp;
    proposal.proponent = test.payer();
    assert_eq!(proposal, apa_proposal_account.proposal);
}
