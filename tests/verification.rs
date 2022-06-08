//! Tests the account setup process

#[cfg(not(tarpaulin_include))]
mod common;
use common::log::*;
use common::program_setup::{start_program_solana_program_test, setup_pda_accounts, setup_queue_accounts, request_compute_units};
use elusiv::instruction::ElusivInstruction;
use solana_sdk::{signature::Signer, transaction::Transaction};
use assert_matches::assert_matches;

use solana_program_test::*;

/*#[tokio::test]
async fn test_verify_full_proof() {
    save_debug_log();

    let (mut banks_client, payer, recent_blockhash) = start_program_solana_program_test().await;
    setup_pda_accounts(&mut banks_client, &payer, recent_blockhash).await;

    let mut transaction = Transaction::new_with_payer(
        &[
            //request_compute_units(100_000),
            ElusivInstruction::setup_proof_instruction()
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

    let mut transaction = Transaction::new_with_payer(
        &[
            request_compute_units(40_000_000),
            ElusivInstruction::verify_proof_instruction()
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

    get_compute_unit_pairs_from_log();
}*/