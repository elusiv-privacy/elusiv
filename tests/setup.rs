//! Tests the account setup process

#[cfg(not(tarpaulin_include))]
mod common;

use elusiv::state::StorageAccount;
use common::program_setup::*;
use common::{
    get_balance,
    get_data,
};
use elusiv::commitment::{CommitmentHashingAccount, BaseCommitmentHashingAccount};
use elusiv::instruction::*;
use elusiv::processor::SingleInstancePDAAccountKind;
use elusiv::proof::VerificationAccount;
use elusiv::state::pool::PoolAccount;
use elusiv::state::program_account::{PDAAccount, SizedAccount, MultiAccountAccount, ProgramAccount};
use elusiv::state::queue::QueueManagementAccount;
use solana_program_test::*;
use solana_sdk::{signature::Signer, transaction::Transaction};
use assert_matches::assert_matches;
use elusiv::state::queue::CommitmentQueueAccount;

macro_rules! assert_account {
    ($ty: ty, $banks_client: ident, $offset: expr) => {
        {
            let mut data = get_data(&mut $banks_client, <$ty>::find($offset).0).await;

            // Check balance and data size
            assert!(get_balance(&mut $banks_client, <$ty>::find($offset).0).await > 0);
            assert_eq!(data.len(), <$ty>::SIZE);

            // Check bump and initialized flag
            let account = <$ty>::new(&mut data).unwrap();
            assert_eq!(account.get_bump_seed(), <$ty>::find($offset).1);
            assert_eq!(account.get_initialized(), true);
        }
    };
}

#[tokio::test]
async fn test_setup_pda_accounts() {
    let (mut banks_client, payer, recent_blockhash) = start_program_solana_program_test().await;
    setup_pda_accounts(&mut banks_client, &payer, recent_blockhash).await;

    // Single instance PDAs
    assert_account!(PoolAccount, banks_client, None);
    assert_account!(QueueManagementAccount, banks_client, None);
    assert_account!(CommitmentHashingAccount, banks_client, None);

    // Multi instance PDAs
    assert_account!(BaseCommitmentHashingAccount, banks_client, Some(0));
    assert_account!(VerificationAccount, banks_client, Some(0));
}

#[tokio::test]
async fn test_setup_all_accounts() {
    let (mut banks_client, payer, recent_blockhash) = start_program_solana_program_test().await;
    setup_pda_accounts(&mut banks_client, &payer, recent_blockhash).await;

    // Create queue accounts
    let queues = setup_queue_accounts(&mut banks_client, &payer, recent_blockhash).await;

    let mut queue_manager = get_data(&mut banks_client, QueueManagementAccount::find(None).0).await;
    let queue_manager = QueueManagementAccount::new(&mut queue_manager[..]).unwrap();

    // Check queue pubkeys
    assert_eq!(queue_manager.get_base_commitment_queue(), queues.base_commitment.to_bytes());
    assert_eq!(queue_manager.get_commitment_queue(), queues.commitment.to_bytes());
    assert_eq!(queue_manager.get_send_proof_queue(), queues.send_proof.to_bytes());
    assert_eq!(queue_manager.get_merge_proof_queue(), queues.merge_proof.to_bytes());
    assert_eq!(queue_manager.get_migrate_proof_queue(), queues.migrate_proof.to_bytes());
    assert_eq!(queue_manager.get_finalize_send_queue(), queues.finalize_send.to_bytes());

    // Finished setup flag
    assert!(queue_manager.get_finished_setup());
}

#[tokio::test]
#[should_panic]
async fn test_setup_pda_accounts_duplicate() {
    let (mut banks_client, payer, recent_blockhash) = start_program_solana_program_test().await;
    setup_pda_accounts(&mut banks_client, &payer, recent_blockhash).await;

    // Should fail because of PDAs already existing
    setup_pda_accounts(&mut banks_client, &payer, recent_blockhash).await;
}

#[tokio::test]
#[should_panic]
async fn test_setup_queue_accounts_duplicate() {
    let (mut banks_client, payer, recent_blockhash) = start_program_solana_program_test().await;
    setup_pda_accounts(&mut banks_client, &payer, recent_blockhash).await;
    setup_queue_accounts(&mut banks_client, &payer, recent_blockhash).await;

    // Should fail because of initialization flag
    setup_queue_accounts(&mut banks_client, &payer, recent_blockhash).await;
}

macro_rules! tx_should_fail {
    ($banks_client: ident, $payer: ident, $recent_blockhash: ident, $ixs: expr) => {
        {
            let mut transaction = Transaction::new_with_payer(
                &$ixs,
                Some(&$payer.pubkey()),
            );
            transaction.sign(&[&$payer], $recent_blockhash);

            assert_matches!($banks_client.process_transaction(transaction).await, Err(_));
        }
    };
}

macro_rules! queue_accounts_tx_should_fail {
    ($banks_client: ident, $payer: ident, $recent_blockhash: ident, $keys: ident, $acc: expr) => {
        let mut keys = $keys.clone();
        keys.commitment = $acc.pubkey();

        tx_should_fail!(
            $banks_client, $payer, $recent_blockhash,
            setup_queue_accounts_ix(&keys)
        );
    };
}

#[tokio::test]
#[should_panic]
/// Test function for the test macro `queue_accounts_tx_should_fail`
async fn test_setup_queue_accounts_should_fail() {
    let (mut banks_client, payer, recent_blockhash) = start_program_solana_program_test().await;
    setup_pda_accounts(&mut banks_client, &payer, recent_blockhash).await;
    let keys = create_queue_accounts(&mut banks_client, &payer, recent_blockhash).await;

    // We set the correct commitment_queue_account so that `queue_accounts_tx_should_fail` panics
    let acc = create_account_rent_exepmt(&mut banks_client, &payer, recent_blockhash, CommitmentQueueAccount::SIZE).await;
    queue_accounts_tx_should_fail!(banks_client, payer, recent_blockhash, keys, acc);
}

#[tokio::test]
async fn test_setup_queue_accounts_invalid() {
    let (mut banks_client, payer, recent_blockhash) = start_program_solana_program_test().await;
    setup_pda_accounts(&mut banks_client, &payer, recent_blockhash).await;
    let keys = create_queue_accounts(&mut banks_client, &payer, recent_blockhash).await;

    // Wrong queue account size
    let acc = create_account_rent_exepmt(&mut banks_client, &payer, recent_blockhash, 100).await;
    queue_accounts_tx_should_fail!(banks_client, payer, recent_blockhash, keys, acc);
}

#[tokio::test]
async fn test_setup_pda_accounts_invalid() {
    let (mut banks_client, payer, recent_blockhash) = start_program_solana_program_test().await;

    // Wrong PDA
    tx_should_fail!(
        banks_client, payer, recent_blockhash, vec![
            ElusivInstruction::open_single_instance_account_instruction(
                SingleInstancePDAAccountKind::Pool, 0,
                SignerAccount(payer.pubkey()),
                WritableUserAccount(QueueManagementAccount::find(None).0)
            )
        ]
    );
}

#[tokio::test]
async fn test_setup_storage_account() {
    let (mut banks_client, payer, recent_blockhash) = start_program_solana_program_test().await;

    let keys = setup_storage_account(&mut banks_client, &payer, recent_blockhash).await;

    execute_on_storage_account(&mut banks_client, &keys, |storage_account| {
        // Finished setup flag
        assert!(storage_account.get_finished_setup());

        // Check pubkeys
        for i in 0..StorageAccount::COUNT {
            assert_eq!(storage_account.get_pubkeys(i), keys[i].to_bytes());
        }
    }).await;
}

#[tokio::test]
#[should_panic]
async fn test_setup_storage_account_duplicate() {
    let (mut banks_client, payer, recent_blockhash) = start_program_solana_program_test().await;
    setup_storage_account(&mut banks_client, &payer, recent_blockhash).await;
    
    // Should panic because of initialization flag
    setup_storage_account(&mut banks_client, &payer, recent_blockhash).await;
}

#[tokio::test]
#[should_panic]
async fn test_setup_accounts_already_setup() {
    let (mut banks_client, payer, recent_blockhash, _, _) = start_program_solana_program_test_with_accounts_setup(
        |_| {},
        |_| {},
        |_| {},
        |_| {},
        |_| {},
        |_| {},
    ).await;

    setup_pda_accounts(&mut banks_client, &payer, recent_blockhash).await;
}