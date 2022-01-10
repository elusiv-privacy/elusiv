mod common;

use {
    assert_matches::*,
    solana_program::{
        instruction::AccountMeta,
        instruction::Instruction,
        native_token::LAMPORTS_PER_SOL,
        system_program,
    },
    solana_program_test::*,
    solana_sdk::{
        signature::Signer,
        transaction::Transaction,
    },
    poseidon::scalar,
};
use common::*;

#[tokio::test]
async fn test_deposit() {
    // Setup program and storage account
    let program_id = elusiv::id();
    let storage_id = storage_account_id();
    let (mut banks_client, payer, recent_blockhash) = start_program_with_storage(storage_id).await;

    // Generate commitment
    let commitment = valid_commitment();

    // Generate instruction data
    let mut data = vec![0];
    let amount: u64 = LAMPORTS_PER_SOL;
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&scalar::to_bytes_le(commitment));

    let mut transaction = Transaction::new_with_payer(
        &[Instruction {
            program_id,
            accounts: vec!
            [
                AccountMeta::new(payer.pubkey(), true),    // 0. [signer, writable] Depositor account
                AccountMeta::new(storage_id, false) ,    // 1. [owned, writable] Bank and storage account
                AccountMeta::new(system_program::id(), false),    // 2. [static] System program
            ],
            data,
        }],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);

    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
}

#[tokio::test]
#[should_panic]
async fn test_deposit_no_data() {
    // Setup program and storage account
    let program_id = elusiv::id();
    let storage_id = storage_account_id();
    let (mut banks_client, payer, recent_blockhash) = start_program_with_storage(storage_id).await;

    // Generate invalid instruction data
    let data = vec![0];

    let mut transaction = Transaction::new_with_payer(
        &[Instruction {
            program_id,
            accounts: vec!
            [
                AccountMeta::new(payer.pubkey(), true),    // 0. [signer, writable] Depositor account
                AccountMeta::new(storage_id, false) ,    // 1. [owned, writable] Bank and storage account
                AccountMeta::new(solana_program::sysvar::ID, false),    // 2. [static] System program
            ],
            data,
        }],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);

    banks_client.process_transaction(transaction).await.unwrap()
}