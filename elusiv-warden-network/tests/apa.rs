mod common;

use assert_matches::assert_matches;
use async_trait::async_trait;
use common::*;
use elusiv_types::{
    ElusivOption, PDAAccount, ProgramAccount, SignerAccount, UserAccount, WritableSignerAccount,
    SPL_TOKEN_COUNT,
};
use elusiv_warden_network::error::ElusivWardenNetworkError;
use elusiv_warden_network::warden::{QuoteEnd, QuoteStart};
use elusiv_warden_network::{
    apa::{ApaLevel, ApaProponentRole, ApaProposal, ApaProposalAccount, ApaProposalsAccount},
    instruction::ElusivWardenNetworkInstruction,
    network::{ApaWardenNetworkAccount, ElusivApaWardenNetwork, WardenNetwork},
};
use solana_program::{
    instruction::{Instruction, InstructionError},
    pubkey::Pubkey,
};
use solana_program_test::*;
use solana_sdk::{signature::Keypair, signer::Signer, transaction::TransactionError};

fn new_test_quote(user_data: &[u8; 32]) -> (QuoteStart, QuoteEnd) {
    let mut bytes = [0; 558];
    bytes[368 + 32..368 + 64].copy_from_slice(user_data);
    let start = QuoteStart(bytes);

    (start, QuoteEnd([0; 558]))
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

        let (quote_start, quote_end) = new_test_quote(&exchange_keypair.pubkey().to_bytes());

        // Attempt to apply with the wrong warden ID
        test.ix_fails_with_instruction_error(
            ElusivWardenNetworkInstruction::start_apa_genesis_warden_application_instruction(
                warden_id + 1,
                quote_start.clone(),
                WritableSignerAccount(warden.pubkey),
            ),
            &[&warden.keypair],
            InstructionError::InvalidSeeds,
        )
        .await;

        let start_application_instruction =
            ElusivWardenNetworkInstruction::start_apa_genesis_warden_application_instruction(
                warden_id,
                quote_start,
                WritableSignerAccount(warden.pubkey),
            );
        // Start the application
        test.ix_should_succeed(start_application_instruction.clone(), &[&warden.keypair])
            .await;
        test.ix_fails_with_invalid_signer(start_application_instruction.clone())
            .await;
        // Try starting an application again with the same warden, must fail
        if warden_id < APA_NETWORK_SIZE - 1 {
            // TODO No idea from where the `Custom(0)` comes.
            test.ix_fails_with_instruction_error(
                start_application_instruction,
                &[&warden.keypair],
                InstructionError::Custom(0),
            )
            .await;
        } else {
            test.ix_fails_with_instruction_error(
                start_application_instruction,
                &[&warden.keypair],
                InstructionError::ProgramFailedToComplete,
            )
            .await;
        }

        // Attempt to complete with the wrong warden ID
        test.ix_fails_with_warden_error(
            ElusivWardenNetworkInstruction::complete_apa_genesis_warden_application_instruction(
                warden_id + 1,
                quote_end.clone(),
                SignerAccount(warden.pubkey),
            ),
            &[&warden.keypair],
            ElusivWardenNetworkError::InvalidInstructionData,
        )
        .await;

        // Now complete the application
        let complete_application_instruction =
            ElusivWardenNetworkInstruction::complete_apa_genesis_warden_application_instruction(
                warden_id,
                quote_end,
                SignerAccount(warden.pubkey),
            );
        test.ix_should_succeed(complete_application_instruction.clone(), &[&warden.keypair])
            .await;
        test.ix_fails_with_invalid_signer(complete_application_instruction.clone())
            .await;

        let mut data = test.data(&ApaWardenNetworkAccount::find(None).0).await;
        let network_account = ApaWardenNetworkAccount::new(&mut data).unwrap();

        if warden_id < APA_NETWORK_SIZE - 1 {
            // Completing twice is actually allowed (and necessary for the tests because same signer)
            test.ix_should_succeed(complete_application_instruction, &[&warden.keypair])
                .await;
            // Confirmation before application phase is completed
            test.ix_fails_with_warden_error(
                ElusivWardenNetworkInstruction::confirm_apa_genesis_network_instruction(
                    warden_id,
                    network_account.confirmation_message(),
                    SignerAccount(exchange_keypair.pubkey()),
                ),
                &[exchange_keypair],
                ElusivWardenNetworkError::NotInConfirmationPhase,
            )
            .await;
        } else {
            // Warden ID is out of bounds
            test.ix_fails_with_instruction_error(
                ElusivWardenNetworkInstruction::confirm_apa_genesis_network_instruction(
                    warden_id + 1,
                    network_account.confirmation_message(),
                    SignerAccount(exchange_keypair.pubkey()),
                ),
                &[exchange_keypair],
                InstructionError::ProgramFailedToComplete,
            )
            .await;
        }
    }

    let mut data = test.data(&ApaWardenNetworkAccount::find(None).0).await;
    let network_account = ApaWardenNetworkAccount::new(&mut data).unwrap();
    assert!(!network_account.is_application_phase());
    assert!(!network_account.is_confirmed());

    // Try to start an application again, should fail
    {
        let exchange_keypair = exchange_keypairs.get(1).unwrap();
        let (quote_start, quote_end) = new_test_quote(&exchange_keypair.pubkey().to_bytes());
        let warden = Actor::new(&mut test).await;
        test.ix_fails_with_instruction_error(
            ElusivWardenNetworkInstruction::start_apa_genesis_warden_application_instruction(
                1,
                quote_start,
                WritableSignerAccount(warden.pubkey),
            ),
            &[&warden.keypair],
            InstructionError::ProgramFailedToComplete,
        )
        .await;
        test.ix_fails_with_instruction_error(
            ElusivWardenNetworkInstruction::complete_apa_genesis_warden_application_instruction(
                1,
                quote_end,
                SignerAccount(warden.pubkey),
            ),
            &[&warden.keypair],
            InstructionError::ProgramFailedToComplete,
        )
        .await;
    }

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
        test.ix_fails_with_invalid_signer(valid_instruction.clone())
            .await;

        // Invalid confirmation-message
        test.ix_fails_with_warden_error(
            ElusivWardenNetworkInstruction::confirm_apa_genesis_network_instruction(
                warden_id,
                invalid_confirmation_message,
                SignerAccount(exchange_keypair.pubkey()),
            ),
            &[exchange_keypair],
            ElusivWardenNetworkError::InvalidConfirmationMessage,
        )
        .await;

        // Invalid warden_id
        if warden_id < APA_NETWORK_SIZE - 1 {
            test.ix_fails_with_warden_error(
                ElusivWardenNetworkInstruction::confirm_apa_genesis_network_instruction(
                    warden_id + 1,
                    confirmation_message,
                    SignerAccount(exchange_keypair.pubkey()),
                ),
                &[exchange_keypair],
                ElusivWardenNetworkError::SignerAndWardenIdMismatch,
            )
            .await;
        } else {
            // Warden ID is out of bounds
            test.ix_fails_with_instruction_error(
                ElusivWardenNetworkInstruction::confirm_apa_genesis_network_instruction(
                    warden_id + 1,
                    confirmation_message,
                    SignerAccount(exchange_keypair.pubkey()),
                ),
                &[exchange_keypair],
                InstructionError::ProgramFailedToComplete,
            )
            .await;
        }

        test.ix_should_succeed(valid_instruction.clone(), &[exchange_keypair])
            .await;

        // Second call is rejected
        if warden_id < APA_NETWORK_SIZE - 1 {
            test.ix_fails_with_warden_error(
                valid_instruction,
                &[exchange_keypair],
                ElusivWardenNetworkError::WardenAlreadyConfirmed,
            )
            .await;
        } else {
            test.ix_fails_with_warden_error(
                valid_instruction,
                &[exchange_keypair],
                ElusivWardenNetworkError::NotInConfirmationPhase,
            )
            .await;
        }
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
    test.ix_fails_with_warden_error(
        ElusivWardenNetworkInstruction::propose_apa_proposal_instruction(
            1,
            proposal.clone(),
            WritableSignerAccount(test.payer()),
            UserAccount(Pubkey::new_unique()),
        ),
        &[],
        ElusivWardenNetworkError::ProposalError,
    )
    .await;

    // Invalid token_id
    let mut proposal_1 = proposal.clone();
    proposal_1.token_constraint = ElusivOption::Some(SPL_TOKEN_COUNT as u16 + 1);
    test.ix_fails_with_warden_error(
        ElusivWardenNetworkInstruction::propose_apa_proposal_instruction(
            0,
            proposal_1,
            WritableSignerAccount(test.payer()),
            UserAccount(Pubkey::new_unique()),
        ),
        &[],
        ElusivWardenNetworkError::ProposalError,
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

#[async_trait]
trait IxFailsWith {
    async fn ix_fails_with_warden_error(
        &mut self,
        ix: Instruction,
        signers: &[&Keypair],
        expected_error: ElusivWardenNetworkError,
    );

    async fn ix_fails_with_instruction_error(
        &mut self,
        ix: Instruction,
        signers: &[&Keypair],
        expected_error: InstructionError,
    );

    async fn ix_fails_with_transaction_error(
        &mut self,
        ix: Instruction,
        signers: &[&Keypair],
        expected_error: TransactionError,
    );

    async fn ix_fails_with_invalid_signer(&mut self, ix: Instruction);
}

#[async_trait]
impl IxFailsWith for ElusivProgramTest {
    async fn ix_fails_with_warden_error(
        &mut self,
        ix: Instruction,
        signers: &[&Keypair],
        expected_error: ElusivWardenNetworkError,
    ) {
        self.ix_fails_with_instruction_error(
            ix,
            signers,
            InstructionError::Custom(expected_error as u32),
        )
        .await
    }

    async fn ix_fails_with_instruction_error(
        &mut self,
        ix: Instruction,
        signers: &[&Keypair],
        expected_error: InstructionError,
    ) {
        self.ix_fails_with_transaction_error(
            ix,
            signers,
            TransactionError::InstructionError(0, expected_error),
        )
        .await
    }

    async fn ix_fails_with_transaction_error(
        &mut self,
        ix: Instruction,
        signers: &[&Keypair],
        expected_error: TransactionError,
    ) {
        let result = self.process_transaction_nonced(&[ix], signers).await;

        let actual_error = assert_matches!(
            result,
            Err(BanksClientError::SimulationError { err, .. }) => err
        );

        assert_eq!(actual_error, expected_error);
    }

    async fn ix_fails_with_invalid_signer(&mut self, ix: Instruction) {
        let invalid_signer = Keypair::new();
        let err = self.ix_should_fail(ix, &[&invalid_signer]).await;
        assert_matches!(err, BanksClientError::ClientError("Signature failure"));
    }
}
