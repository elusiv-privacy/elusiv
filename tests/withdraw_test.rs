mod common;

use {
    assert_matches::*,
    solana_program::{
        instruction::AccountMeta,
        instruction::Instruction,
        pubkey::Pubkey,
        native_token::LAMPORTS_PER_SOL,
        system_program,
    },
    solana_program_test::*,
    solana_sdk::{
        signature::Signer,
        transaction::Transaction,
    },
    poseidon::scalar,
    elusiv::state::TOTAL_SIZE,
    elusiv::entrypoint::process_instruction,
};
use common::*;

#[tokio::test]
#[should_panic]
async fn test_withdraw_no_data() {
    // Setup program and storage account
    let program_id = elusiv::id();
    let storage_id = storage_account_id();
    let (mut banks_client, payer, recent_blockhash) = start_program_with_storage(storage_id).await;

    // Generate invalid instruction data
    let data = vec![1];

    let mut transaction = Transaction::new_with_payer(
        &[Instruction {
            program_id,
            accounts: vec!
            [ AccountMeta::new(payer.pubkey(), true), ],
            data,
        }],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);

    banks_client.process_transaction(transaction).await.unwrap()
}