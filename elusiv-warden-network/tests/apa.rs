mod common;

use common::*;
use elusiv_types::{
    ElusivOption, PDAAccount, ProgramAccount, SignerAccount, UserAccount, WritableSignerAccount,
    SPL_TOKEN_COUNT,
};
use elusiv_warden_network::{
    apa::{ApaLevel, ApaProponentRole, ApaProposal, ApaProposalAccount, ApaProposalsAccount},
    instruction::ElusivWardenNetworkInstruction,
    network::{ApaWardenNetworkAccount, ElusivApaWardenNetwork, WardenNetwork},
    warden::Quote,
};
use solana_program::pubkey::Pubkey;
use solana_program_test::*;
use solana_sdk::{signature::Keypair, signer::Signer};

fn new_test_quote(user_data: &[u8; 32]) -> Quote {
    let mut bytes = [0; 1116];
    bytes[368 + 32..368 + 64].copy_from_slice(user_data);
    Quote(bytes)
}

#[tokio::test]
async fn test_apa_inception() {
    let mut test = start_test_with_setup().await;
    const APA_NETWORK_SIZE: u32 = ElusivApaWardenNetwork::SIZE.max() as u32;

    let exchange_keypairs: Vec<_> = (0..APA_NETWORK_SIZE).map(|_| Keypair::new()).collect();
    for (i, exchange_keypair) in exchange_keypairs.iter().enumerate() {
        let warden_id = i as u32;
        let mut warden = Actor::new(&mut test).await;
        register_warden(&mut test, &mut warden).await;

        test.ix_should_succeed(
            ElusivWardenNetworkInstruction::apply_apa_genesis_warden_instruction(
                warden_id,
                new_test_quote(&exchange_keypair.pubkey().to_bytes()),
                WritableSignerAccount(warden.pubkey),
            ),
            &[&warden.keypair],
        )
        .await;

        let mut data = test.data(&ApaWardenNetworkAccount::find(None).0).await;
        let network_account = ApaWardenNetworkAccount::new(&mut data).unwrap();

        // Confirmation before application phase is completed
        if warden_id < APA_NETWORK_SIZE - 1 {
            test.ix_should_fail(
                ElusivWardenNetworkInstruction::confirm_apa_genesis_network_instruction(
                    warden_id,
                    network_account.confirmation_message(),
                    SignerAccount(exchange_keypair.pubkey()),
                ),
                &[exchange_keypair],
            )
            .await;
        }
    }

    let mut data = test.data(&ApaWardenNetworkAccount::find(None).0).await;
    let network_account = ApaWardenNetworkAccount::new(&mut data).unwrap();
    assert!(!network_account.is_application_phase());
    assert!(!network_account.is_confirmed());

    let confirmation_message = network_account.confirmation_message();

    let mut invalid_confirmation_message = confirmation_message;
    invalid_confirmation_message[0] = ((invalid_confirmation_message[0] as usize + 1) % 256) as u8;

    for (i, exchange_keypair) in exchange_keypairs.iter().enumerate() {
        let warden_id = i as u32;
        let valid_instruction =
            ElusivWardenNetworkInstruction::confirm_apa_genesis_network_instruction(
                warden_id,
                confirmation_message,
                SignerAccount(exchange_keypair.pubkey()),
            );

        // Invalid exchange-key signer
        let invalid_signer = Keypair::new();
        test.ix_should_fail(valid_instruction.clone(), &[&invalid_signer])
            .await;

        // Invalid confirmation-message
        test.ix_should_fail(
            ElusivWardenNetworkInstruction::confirm_apa_genesis_network_instruction(
                warden_id,
                invalid_confirmation_message,
                SignerAccount(exchange_keypair.pubkey()),
            ),
            &[exchange_keypair],
        )
        .await;

        // Invalid warden_id
        test.ix_should_fail(
            ElusivWardenNetworkInstruction::confirm_apa_genesis_network_instruction(
                warden_id + 1,
                confirmation_message,
                SignerAccount(exchange_keypair.pubkey()),
            ),
            &[exchange_keypair],
        )
        .await;

        test.ix_should_succeed(valid_instruction.clone(), &[exchange_keypair])
            .await;

        // Second call is rejected
        test.ix_should_fail(valid_instruction, &[exchange_keypair])
            .await;
    }

    let mut data = test.data(&ApaWardenNetworkAccount::find(None).0).await;
    let network_account = ApaWardenNetworkAccount::new(&mut data).unwrap();
    assert!(network_account.is_confirmed());
}

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
