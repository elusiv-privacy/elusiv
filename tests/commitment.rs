//! Tests the base commitment and commitment hashing

mod common;
use elusiv::fields::fr_to_u256_le;
use elusiv::types::U256;
use common::program_setup::*;
use common::{ get_data, };
use elusiv::instruction::{ElusivInstruction, SignerAccount, WritableUserAccount};
use solana_program::hash::Hash;
use solana_program::instruction::Instruction;
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program::pubkey::Pubkey;
use solana_program_test::*;
use solana_sdk::{signature::Signer, transaction::Transaction};
use assert_matches::assert_matches;
use elusiv::state::queue::{
    BaseCommitmentQueueAccount, BaseCommitmentHashRequest, BaseCommitmentQueue,
    CommitmentQueueAccount, CommitmentQueue,
};
use std::str::FromStr;
use ark_bn254::Fr;
use elusiv::state::queue::RingQueue;
use elusiv::commitment::BaseCommitmentHashComputation;
use elusiv_computation::PartialComputation;

fn u256_from_str(str: &str) -> U256 {
    fr_to_u256_le(&Fr::from_str(str).unwrap())
}

async fn tx_should_succeed(ixs: &[Instruction], banks_client: &mut BanksClient, payer: &solana_sdk::signature::Keypair, recent_blockhash: Hash) {
    let mut tx = Transaction::new_with_payer(ixs, Some(&payer.pubkey()));
    tx.sign(&[payer], recent_blockhash);
    assert_matches!(banks_client.process_transaction(tx).await, Ok(()));
}

async fn execute_on_queue<F>(banks_client: &mut BanksClient, key: &Pubkey, f: F) where F: Fn(&BaseCommitmentQueue) {
    let mut queue = get_data(banks_client, *key).await;
    let mut queue = BaseCommitmentQueueAccount::new(&mut queue[..]).unwrap();
    let queue = BaseCommitmentQueue::new(&mut queue);
    f(&queue)
}

#[tokio::test]
async fn test_base_commitment() {
    //common::log::save_debug_log();

    let first_request = BaseCommitmentHashRequest {
        base_commitment: u256_from_str("8337064132573119120838379738103457054645361649757131991036638108422638197362"),
        amount: LAMPORTS_PER_SOL,
        commitment: u256_from_str("139214303935475888711984321184227760578793579443975701453971046059378311483")
    };

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
    execute_on_queue(&mut banks_client, &keys.base_commitment, |queue| {
        assert_eq!(queue.len(), 2);

        let first = queue.view(0).unwrap();
        assert_eq!(first.is_being_processed, false);
        assert_eq!(first.request, first_request);

        let second = queue.view(1).unwrap();
        assert_eq!(second.is_being_processed, false);
        assert_eq!(second.request, second_request);
    }).await;

    // Init computation
    tx_should_succeed(
        &[
            ElusivInstruction::init_base_commitment_hash(0, SignerAccount(payer.pubkey()), WritableUserAccount(keys.base_commitment))
        ],
        &mut banks_client, &payer, recent_blockhash
    ).await;

    // Check that first request has been set to `is_being_processed` (and nothing else has changed)
    execute_on_queue(&mut banks_client, &keys.base_commitment, |queue| {
        assert_eq!(queue.len(), 2);

        let first = queue.view(0).unwrap();
        assert_eq!(first.is_being_processed, true);
        assert_eq!(first.request, first_request);

        let second = queue.view(1).unwrap();
        assert_eq!(second.is_being_processed, false);
        assert_eq!(second.request, second_request);
    }).await;

    // Compute hash (should fail since not enough compute units)
    let mut transaction = Transaction::new_with_payer(
        &[ElusivInstruction::compute_base_commitment_hash(0)],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    assert_matches!(banks_client.process_transaction(transaction).await, Err(_));

    // Compute hashes
    for i in 0..BaseCommitmentHashComputation::INSTRUCTIONS.len() {
        tx_should_succeed(
            &[
                request_compute_units(BaseCommitmentHashComputation::INSTRUCTIONS[i].compute_units),
                ElusivInstruction::compute_base_commitment_hash(0)
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
    execute_on_queue(&mut banks_client, &keys.base_commitment, |queue| {
        assert_eq!(queue.len(), 1);

        let first = queue.view(0).unwrap();
        assert_eq!(first.is_being_processed, false);
        assert_eq!(first.request, second_request);
    }).await;

    // Check commitment queue
    let mut queue = get_data(&mut banks_client, keys.commitment).await;
    let mut queue = CommitmentQueueAccount::new(&mut queue[..]).unwrap();
    let queue = CommitmentQueue::new(&mut queue);

    assert_eq!(queue.len(), 1);
    let commitment = queue.view_first().unwrap().request;
    assert_eq!(commitment, first_request.commitment);

    //common::log::get_compute_unit_pairs_from_log();
}