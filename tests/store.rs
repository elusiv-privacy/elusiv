mod common;
use common::*;

use assert_matches::*;
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program_test::*;
use solana_sdk::{signature::Signer, transaction::Transaction};
use elusiv::{instruction::*, state::queue::BaseCommitmentHashRequest};

#[tokio::test]
async fn test_store() {
    let (mut banks_client, payer, recent_blockhash) = start_program_solana_program_test().await;

    let base_commitment = [0; 32];
    let amount: u64 = LAMPORTS_PER_SOL;
    let commitment = [0; 32];

    let base_commitment_request = BaseCommitmentHashRequest {
        base_commitment,
        amount,
        commitment,
        is_active: false
    };

    let mut transaction = Transaction::new_with_payer(
        &[
            ElusivInstruction::store(base_commitment_request, SignerAccount(payer.pubkey())),
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
}

#[tokio::test]
#[ignore]
async fn test_fail() {
    let (mut banks_client, payer, recent_blockhash) = start_program_solana_program_test().await;

    let mut transaction = Transaction::new_with_payer(
        &[
            request_compute_units(1_400_000),
            ElusivInstruction::test_fail(),
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
}