mod common;

use {
    assert_matches::*,
    solana_program_test::*,
    solana_program::native_token::LAMPORTS_PER_SOL,
    /*solana_sdk::{
        signature::Keypair,
        signer::Signer
    },*/
    elusiv::state::StorageAccount,
};
use common::*;

#[tokio::test]
#[ignore]
/// Test valid deposit
async fn test_deposit() {
    let (mut banks_client, payer, recent_blockhash) = start_program_with_storage(storage_id()).await;

    for _ in 0..1 {
        let t = send_deposit_transaction(elusiv::id(), storage_id(), &payer, recent_blockhash, deposit_data(valid_commitment())).await;
        assert_matches!(banks_client.process_transaction(t).await, Ok(()));
    }
}

#[tokio::test]
#[should_panic]
#[ignore]
/// Test deposit with different kinds of wrong data
async fn test_deposit_no_data() {
    let (mut banks_client, payer, recent_blockhash) = start_program_with_storage(storage_id()).await;

    let data = vec![0];

    let t = send_deposit_transaction(elusiv::id(), storage_id(), &payer, recent_blockhash, data).await;
    banks_client.process_transaction(t).await.unwrap()
}

#[tokio::test]
#[ignore]
/// Tests for changes in the storage after two deposits
async fn test_two_valid_deposits() {
    let (mut banks_client, payer, recent_blockhash) = start_program_with_storage(storage_id()).await;

    let mut account_data_old = get_storage_data(&mut banks_client).await;
    let storage_balance = get_balance(&mut banks_client, storage_id()).await;

    let commitment1 = send_valid_deposit(&payer, &mut banks_client, recent_blockhash).await;
    let mut account_data1 = get_storage_data(&mut banks_client).await;

    let commitment2 = send_valid_deposit(&payer, &mut banks_client, recent_blockhash).await;
    let mut account_data2 = get_storage_data(&mut banks_client).await;

    // Test for storage data changes
    //assert_ne!(account_data_old, account_data1);
    //assert_ne!(account_data_old, account_data2);
    //assert_ne!(account_data1, account_data2);

    // Test for increment of leaf pointer
    let old_storage = StorageAccount::from(&mut account_data_old).unwrap();
    let new_storage1 = StorageAccount::from(&mut account_data1).unwrap();
    let new_storage2 = StorageAccount::from(&mut account_data2).unwrap();
    assert_eq!(old_storage.leaf_pointer(), 0);
    assert_eq!(new_storage1.leaf_pointer(), 1);
    assert_eq!(new_storage2.leaf_pointer(), 2);

    // Test for commitment storage
    let commitments = get_commitments(&mut account_data1);
    assert_eq!(commitments[0], commitment1);

    let commitments = get_commitments(&mut account_data2);
    assert_eq!(commitments[0], commitment1);
    assert_eq!(commitments[1], commitment2);

    // Test for balance changes
    assert_eq!(
        storage_balance + LAMPORTS_PER_SOL * 2,
        get_balance(&mut banks_client, storage_id()).await
    );
}