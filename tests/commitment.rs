//! Tests the base commitment and commitment hashing

#[cfg(not(tarpaulin_include))]
mod common;

use elusiv::fields::{fr_to_u256_le, u256_to_fr};
use elusiv::state::{MT_HEIGHT, EMPTY_TREE};
use elusiv::types::U256;
use common::program_setup::*;
use common::{ get_data, };
use elusiv::instruction::{ElusivInstruction, SignerAccount, WritableUserAccount, UserAccount};
use solana_program::hash::Hash;
use solana_program::instruction::Instruction;
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program::pubkey::Pubkey;
use solana_program_test::*;
use solana_sdk::{signature::Signer, transaction::Transaction};
use assert_matches::assert_matches;
use elusiv::state::{
    queue::{
        BaseCommitmentQueueAccount, BaseCommitmentHashRequest, BaseCommitmentQueue,
        CommitmentQueueAccount, CommitmentQueue,
        RingQueue,
        Queue,
    },
    program_account::{
        PDAAccount,
        ProgramAccount,
    },
};
use std::str::FromStr;
use ark_bn254::Fr;
use elusiv::commitment::{BaseCommitmentHashComputation, CommitmentHashComputation, CommitmentHashingAccount};
use elusiv_computation::PartialComputation;

fn u256_from_str(str: &str) -> U256 {
    fr_to_u256_le(&Fr::from_str(str).unwrap())
}

async fn tx_should_succeed(ixs: &[Instruction], banks_client: &mut BanksClient, payer: &solana_sdk::signature::Keypair, recent_blockhash: Hash) {
    let mut tx = Transaction::new_with_payer(ixs, Some(&payer.pubkey()));
    tx.sign(&[payer], recent_blockhash);
    assert_matches!(banks_client.process_transaction(tx).await, Ok(()));
}

async fn tx_should_fail(ixs: &[Instruction], banks_client: &mut BanksClient, payer: &solana_sdk::signature::Keypair, recent_blockhash: Hash) {
    let mut tx = Transaction::new_with_payer(ixs, Some(&payer.pubkey()));
    tx.sign(&[payer], recent_blockhash);
    assert_matches!(banks_client.process_transaction(tx).await, Err(_));
}

macro_rules! queue {
    ($id: ident, $ty: ty, $ty_account: ty) => {
        let mut queue = <$ty_account>::new(&mut $id[..]).unwrap();
        let $id = <$ty>::new(&mut queue);
    };
}

async fn execute_on_base_queue<F>(banks_client: &mut BanksClient, key: &Pubkey, f: F) where F: Fn(&BaseCommitmentQueue) {
    let mut queue = get_data(banks_client, *key).await;
    queue!(queue, BaseCommitmentQueue, BaseCommitmentQueueAccount);
    f(&queue)
}

async fn execute_on_commitment_queue<F>(banks_client: &mut BanksClient, key: &Pubkey, f: F) where F: Fn(&CommitmentQueue) {
    let mut queue = get_data(banks_client, *key).await;
    queue!(queue, CommitmentQueue, CommitmentQueueAccount);
    f(&queue)
}

macro_rules! first_request_test {
    () => {
        BaseCommitmentHashRequest {
            base_commitment: u256_from_str("8337064132573119120838379738103457054645361649757131991036638108422638197362"),
            amount: LAMPORTS_PER_SOL,
            commitment: u256_from_str("139214303935475888711984321184227760578793579443975701453971046059378311483")
        }
    };
}

#[tokio::test]
async fn test_base_commitment() {
    let first_request = first_request_test!();
    let second_request = BaseCommitmentHashRequest {
        base_commitment: u256_from_str("8337064132573119120838379738103457054645361649757131991036638108422638197362"),
        amount: 20 * LAMPORTS_PER_SOL,
        commitment: u256_from_str("21186803555845400161937398579081414146527572885637089779856221229551142844794")
    };

    let (mut banks_client, payer, recent_blockhash) = start_program_solana_program_test().await;
    setup_pda_accounts(&mut banks_client, &payer, recent_blockhash).await;
    let keys = setup_queue_accounts(&mut banks_client, &payer, recent_blockhash).await;

    // Enqueue first and second
    tx_should_succeed(
        &[
            ElusivInstruction::store(first_request.clone(), SignerAccount(payer.pubkey()), WritableUserAccount(keys.base_commitment)),
            ElusivInstruction::store(second_request.clone(), SignerAccount(payer.pubkey()), WritableUserAccount(keys.base_commitment)),
        ],
        &mut banks_client, &payer, recent_blockhash
    ).await;

    // Check for requests in queue
    execute_on_base_queue(&mut banks_client, &keys.base_commitment, |queue| {
        assert_eq!(queue.len(), 2);

        let first = queue.view(0).unwrap();
        assert_eq!(first.is_being_processed, false);
        assert_eq!(first.request, first_request);

        let second = queue.view(1).unwrap();
        assert_eq!(second.is_being_processed, false);
        assert_eq!(second.request, second_request);
    }).await;

    // Init bas commitment hash computation
    tx_should_succeed(
        &[
            ElusivInstruction::init_base_commitment_hash(0, SignerAccount(payer.pubkey()), WritableUserAccount(keys.base_commitment))
        ],
        &mut banks_client, &payer, recent_blockhash
    ).await;

    // Check that first request has been set to `is_being_processed` (and nothing else has changed)
    execute_on_base_queue(&mut banks_client, &keys.base_commitment, |queue| {
        assert_eq!(queue.len(), 2);

        let first = queue.view(0).unwrap();
        assert_eq!(first.is_being_processed, true);
        assert_eq!(first.request, first_request);

        let second = queue.view(1).unwrap();
        assert_eq!(second.is_being_processed, false);
        assert_eq!(second.request, second_request);
    }).await;

    // Compute hash (should fail since not enough compute units)
    tx_should_fail(
        &[
            ElusivInstruction::compute_base_commitment_hash(0, 0)
        ],
        &mut banks_client, &payer, recent_blockhash
    ).await;

    // Compute hashes
    for i in 0..BaseCommitmentHashComputation::INSTRUCTIONS.len() {
        let nonce: u64 = rand::random();

        tx_should_succeed(
            &[
                request_compute_units(BaseCommitmentHashComputation::INSTRUCTIONS[i].compute_units),
                ElusivInstruction::compute_base_commitment_hash(0, nonce)
            ],
            &mut banks_client, &payer, recent_blockhash
        ).await;
    }

    // Finalize base commitment hash
    tx_should_succeed(
        &[
            ElusivInstruction::finalize_base_commitment_hash(0, WritableUserAccount(keys.base_commitment), WritableUserAccount(keys.commitment))
        ],
        &mut banks_client, &payer, recent_blockhash
    ).await;

    // Check base_commitment_queue (first element should be gone, second now at the top)
    execute_on_base_queue(&mut banks_client, &keys.base_commitment, |queue| {
        assert_eq!(queue.len(), 1);

        let first = queue.view(0).unwrap();
        assert_eq!(first.is_being_processed, false);
        assert_eq!(first.request, second_request);
    }).await;

    // Check commitment queue
    execute_on_commitment_queue(&mut banks_client, &keys.commitment, |queue| {
        assert_eq!(queue.len(), 1);

        let first = queue.view(0).unwrap();
        assert_eq!(first.is_being_processed, false);
        assert_eq!(first.request, first_request.commitment);
    }).await;

}

#[tokio::test]
async fn test_init_commitment() {
    let first_request = first_request_test!();

    let (mut banks_client, payer, recent_blockhash, keys, storage) = start_program_solana_program_test_with_accounts_setup(
        |_| {},
        |commitment_queue| {
            commitment_queue.enqueue(first_request.commitment).unwrap();
        },
        |_| {},
        |_| {},
        |_| {},
        |_| {},
    ).await;

    // Get storage account
    let user_storage = storage.iter().map(|&x| UserAccount(x)).collect::<Vec<UserAccount>>().try_into().unwrap();

    // Init commitment hash computation
    tx_should_succeed(
        &[
            ElusivInstruction::init_commitment_hash(SignerAccount(payer.pubkey()), WritableUserAccount(keys.commitment), user_storage)
        ],
        &mut banks_client, &payer, recent_blockhash
    ).await;

    // Check commitment queue
    execute_on_commitment_queue(&mut banks_client, &keys.commitment, |queue| {
        assert_eq!(queue.len(), 1);

        let first = queue.view(0).unwrap();
        assert_eq!(first.is_being_processed, true);
        assert_eq!(first.request, first_request.commitment);
    }).await;
}

#[tokio::test]
async fn test_single_commitment() {
    /*let first_request = first_request_test!();

    let (mut banks_client, payer, recent_blockhash, keys, storage) = start_program_solana_program_test_with_accounts_setup(
        |_| {},
        |commitment_queue| {
            commitment_queue.enqueue(first_request.commitment).unwrap();
            commitment_queue.process_first().unwrap();
        },
        |_| {},
        |_| {},
        |_| {},
        |_| {},
    ).await;

    // Get storage account
    let user_storage = storage.iter().map(|&x| UserAccount(x)).collect::<Vec<UserAccount>>().try_into().unwrap();*/

    // Compute hashes
    /*for i in 0..CommitmentHashComputation::INSTRUCTIONS.len() {
        let nonce: u64 = rand::random();

        tx_should_succeed(
            &[
                request_compute_units(CommitmentHashComputation::INSTRUCTIONS[i].compute_units),
                ElusivInstruction::compute_commitment_hash(nonce),
            ],
            &mut banks_client, &payer, recent_blockhash
        ).await;
    }

    // Check finished hashes
    let mut hashing_account = get_data(&mut banks_client, CommitmentHashingAccount::find(None).0).await;
    let hashing_account = CommitmentHashingAccount::new(&mut hashing_account).unwrap();

    assert_eq!(hashing_account.get_is_active(), true);
    assert_eq!(hashing_account.get_finished_hashes(MT_HEIGHT as usize - 1), u256_from_str("11500204619817968836204864831937045342731531929677521260156990135685848035575"));

    // Get storage account
    let writable_storage = storage.iter().map(|&x| WritableUserAccount(x)).collect::<Vec<WritableUserAccount>>().try_into().unwrap();

    // Finalize commitment hash
    tx_should_succeed(
        &[
            ElusivInstruction::finalize_commitment_hash(WritableUserAccount(keys.commitment), writable_storage)
        ],
        &mut banks_client, &payer, recent_blockhash
    ).await;

    // Check that merkle tree is updated
    execute_on_storage_account(&mut banks_client, &storage, |storage_account| {
        let root = storage_account.get_root();
        assert_eq!(root, u256_from_str("11500204619817968836204864831937045342731531929677521260156990135685848035575"));

        let commitment = storage_account.get_node(0, MT_HEIGHT as usize);
        assert_eq!(commitment, u256_to_fr(&first_request.commitment));

        assert_eq!(storage_account.get_next_commitment_ptr(), 1);
        assert_eq!(storage_account.get_active_mt_root_history(0), fr_to_u256_le(&EMPTY_TREE[MT_HEIGHT as usize]));
    }).await;*/
}