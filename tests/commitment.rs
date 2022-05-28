//! Tests the account setup process

mod common;
use elusiv::fields::fr_to_u256_le;
use elusiv::types::U256;
use common::program_setup::*;
use common::{ get_data, };
use elusiv::instruction::{ElusivInstruction, SignerAccount, WritableUserAccount};
use elusiv::state::queue::{BaseCommitmentHashRequest, BaseCommitmentQueue};
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program_test::*;
use solana_sdk::{signature::Signer, transaction::Transaction};
use assert_matches::assert_matches;
use elusiv::state::queue::BaseCommitmentQueueAccount;
use std::str::FromStr;
use ark_bn254::Fr;
use elusiv::state::queue::RingQueue;

fn u256_from_str(str: &str) -> U256 {
    fr_to_u256_le(&Fr::from_str(str).unwrap())
}

#[tokio::test]
async fn test_base_commitment() {
    let (mut banks_client, payer, recent_blockhash) = start_program_solana_program_test().await;
    setup_pda_accounts(&mut banks_client, &payer, recent_blockhash).await;
    let keys = setup_queue_accounts(&mut banks_client, &payer, recent_blockhash).await;

    // Enqueue first
    let mut transaction = Transaction::new_with_payer(
        &[
            ElusivInstruction::store(
                BaseCommitmentHashRequest {
                    base_commitment: u256_from_str("8337064132573119120838379738103457054645361649757131991036638108422638197362"),
                    amount: LAMPORTS_PER_SOL,
                    commitment: u256_from_str("139214303935475888711984321184227760578793579443975701453971046059378311483")
                },
                SignerAccount(payer.pubkey()),
                WritableUserAccount(keys.base_commitment),
            )
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

    // Enqueue second
    let mut transaction = Transaction::new_with_payer(
        &[
            ElusivInstruction::store(
                BaseCommitmentHashRequest {
                    base_commitment: u256_from_str("8337064132573119120838379738103457054645361649757131991036638108422638197362"),
                    amount: LAMPORTS_PER_SOL * 20,
                    commitment: u256_from_str("21186803555845400161937398579081414146527572885637089779856221229551142844794")
                },
                SignerAccount(payer.pubkey()),
                WritableUserAccount(keys.base_commitment),
            )
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

    let mut queue = get_data(&mut banks_client, keys.base_commitment).await;
    let mut queue = BaseCommitmentQueueAccount::new(&mut queue[..]).unwrap();
    let queue = BaseCommitmentQueue::new(&mut queue);

    // Check for requests in queue
    assert_eq!(queue.len(), 2);
    let first = queue.view_first().unwrap().request;
    assert_eq!(first.base_commitment, u256_from_str("8337064132573119120838379738103457054645361649757131991036638108422638197362"));
    assert_eq!(first.amount, LAMPORTS_PER_SOL);
    assert_eq!(first.commitment, u256_from_str("139214303935475888711984321184227760578793579443975701453971046059378311483"));

    let second = queue.view(1).unwrap().request;
    assert_eq!(second.base_commitment, u256_from_str("8337064132573119120838379738103457054645361649757131991036638108422638197362"));
    assert_eq!(second.amount, LAMPORTS_PER_SOL * 20);
    assert_eq!(second.commitment, u256_from_str("21186803555845400161937398579081414146527572885637089779856221229551142844794"));
}