//! Tests the base commitment and commitment hashing

#[cfg(not(tarpaulin_include))]
mod common;
use common::*;
use common::program_setup::*;

use elusiv::fee::FeeAccount;
use elusiv::processor::BaseCommitmentHashRequest;
use elusiv::state::pool::PoolAccount;
use elusiv::state::program_account::SizedAccount;
use elusiv::state::queue::{BaseCommitmentQueue, BaseCommitmentQueueAccount};
use elusiv::instruction::*;
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program::pubkey::Pubkey;
use solana_program_test::*;
use elusiv::state::{
    queue::{
        CommitmentQueueAccount, CommitmentQueue,
        RingQueue,
        Queue,
    },
    program_account::{
        PDAAccount,
        ProgramAccount,
    },
};
use elusiv::commitment::{BaseCommitmentHashComputation, BaseCommitmentHashingAccount};
use elusiv_computation::PartialComputation;

fn requests() -> Vec<BaseCommitmentHashRequest> {
    vec![
        base_commitment_request(
            "8337064132573119120838379738103457054645361649757131991036638108422638197362",
            "139214303935475888711984321184227760578793579443975701453971046059378311483",
            LAMPORTS_PER_SOL, 0,
        ),
        base_commitment_request(
            "8337064132573119120838379738103457054645361649757131991036638108422638197362",
            "21186803555845400161937398579081414146527572885637089779856221229551142844794",
            20 * LAMPORTS_PER_SOL, 0,
        ),
    ]
}

#[tokio::test]
async fn test_base_commitment() {
    let mut test_program = start_program_solana_program_test().await;
    setup_pda_accounts(&mut test_program).await;
    setup_pool_accounts(&mut test_program).await;

    let lamports_per_tx = lamports_per_signature(&mut test_program).await;

    pda_account!(fee, FeeAccount, Some(0), test_program);
    pda_account!(pool, PoolAccount, None, test_program);

    let sol_pool = Pubkey::new(&pool.get_sol_pool());
    let sol_pool_start_balance = get_balance(sol_pool, &mut test_program).await;

    let fee_collector = Pubkey::new(&pool.get_fee_collector());
    let fee_collector_start_balance = get_balance(fee_collector, &mut test_program).await;

    let requests = requests();

    let mut client = Actor::new(&mut test_program).await; 
    let mut relayer_a = Actor::new(&mut test_program).await; 
    let mut relayer_b = Actor::new(&mut test_program).await; 

    // Request should fail: client has not enough funds
    let store_ix = ElusivInstruction::store_base_commitment_instruction(
        0,
        requests[0].clone(),
        SignerAccount(client.pubkey),
        WritableUserAccount(sol_pool),
        WritableUserAccount(fee_collector),
    );
    ix_should_fail(store_ix.clone(), &mut client, &mut test_program).await;
    assert_eq!(0, client.balance(&mut test_program).await);

    // Client has enough funds
    let base_commitment_hash_fee = fee.base_commitment_hash_fee();
    let network_fee = fee.get_base_commitment_network_fee();
    let amount = requests[0].amount + base_commitment_hash_fee + lamports_per_tx;
    client.airdrop(amount, &mut test_program).await;
    assert_eq!(amount, client.balance(&mut test_program).await);

    ix_should_succeed(store_ix, &mut client, &mut test_program).await;

    // client has: zero-balance
    assert_eq!(0, client.balance(&mut test_program).await);

    // pool has: requests[0].amount + base_commitment_hash_fee - network_fee
    assert_eq!(
        network_fee + fee_collector_start_balance,
        get_balance(fee_collector, &mut test_program).await
    );

    // fee_collector has: network_fee
    assert_eq!(
        requests[0].amount + base_commitment_hash_fee - network_fee + sol_pool_start_balance,
        get_balance(sol_pool, &mut test_program).await
    );

    // Check the queue for the first request
    queue!(queue, BaseCommitmentQueue, BaseCommitmentQueueAccount, test_program);
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.view_first().unwrap(), requests[0]);

    // Client stores the second request
    let amount = requests[1].amount + base_commitment_hash_fee + lamports_per_tx;
    client.airdrop(amount, &mut test_program).await;
    ix_should_succeed(ElusivInstruction::store_base_commitment_instruction(
        0,
        requests[1].clone(),
        SignerAccount(client.pubkey),
        WritableUserAccount(sol_pool),
        WritableUserAccount(fee_collector),
    ), &mut client, &mut test_program).await;

    assert_eq!(0, client.balance(&mut test_program).await);
    assert_eq!(
        2 * network_fee + fee_collector_start_balance,
        get_balance(fee_collector, &mut test_program).await
    );
    assert_eq!(
        requests[0].amount + requests[1].amount + 2 * (base_commitment_hash_fee - network_fee) + sol_pool_start_balance,
        get_balance(sol_pool, &mut test_program).await
    );

    // Check the queue for the second request
    queue!(queue, BaseCommitmentQueue, BaseCommitmentQueueAccount, test_program);
    assert_eq!(queue.len(), 2);
    assert_eq!(queue.view_first().unwrap(), requests[0]);
    assert_eq!(queue.view(1).unwrap(), requests[1]);

    // Init through A with hash_account at 0
    let rent = get_account_cost(&mut test_program, BaseCommitmentHashingAccount::SIZE).await;
    relayer_a.airdrop(lamports_per_tx + rent, &mut test_program).await;
    ix_should_succeed(
        ElusivInstruction::init_base_commitment_hash_instruction(
            0,
            SignerAccount(relayer_a.pubkey)
        ),
        &mut relayer_a, &mut test_program,
    ).await;

    // A should now have lost the cost of renting
    assert_eq!(0, relayer_a.balance(&mut test_program).await);

    // First request has been dequeued
    queue!(queue, BaseCommitmentQueue, BaseCommitmentQueueAccount, test_program);
    assert_eq!(queue.len(), 1);

    // Second init through B will fail, since the hash_account at 0 already exists
    relayer_b.airdrop(lamports_per_tx + rent, &mut test_program).await;
    ix_should_fail(
        ElusivInstruction::init_base_commitment_hash_instruction(
            0,
            SignerAccount(relayer_b.pubkey)
        ),
        &mut relayer_b, &mut test_program,
    ).await;

    // But init through B will succeed for the hash_account with offset 1
    ix_should_succeed(
        ElusivInstruction::init_base_commitment_hash_instruction(
            1,
            SignerAccount(relayer_b.pubkey)
        ),
        &mut relayer_b, &mut test_program,
    ).await;
    assert_eq!(0, relayer_b.balance(&mut test_program).await);

    // Queue should now be empty
    queue!(queue, BaseCommitmentQueue, BaseCommitmentQueueAccount, test_program);
    assert_eq!(queue.len(), 0);

    // New hash_account with the request
    assert!(account_does_exist(BaseCommitmentHashingAccount::find(Some(0)).0, &mut test_program).await);
    pda_account!(hash_account, BaseCommitmentHashingAccount, Some(0), test_program);
    assert_eq!(hash_account.get_fee_version(), 0);
    assert_eq!(hash_account.get_fee_payer(), relayer_a.pubkey.to_bytes());
    assert_eq!(hash_account.get_instruction(), 0);

    let compute_ix = ElusivInstruction::compute_base_commitment_hash_instruction(
        0,
        0,
        0,
        SignerAccount(relayer_a.pubkey),
        WritableUserAccount(sol_pool)
    );
    let finalize_ix = ElusivInstruction::finalize_base_commitment_hash_instruction(
        0,
        WritableUserAccount(relayer_a.pubkey)
    );

    // Compute each base_commitment_hash
    let hash_reward = fee.get_relayer_hash_tx_fee();
    for i in 0..BaseCommitmentHashComputation::INSTRUCTIONS.len() {
        // Finalization will always fail before completion
        ix_should_fail(finalize_ix.clone(), &mut relayer_a, &mut test_program).await;

        // Fail due to too low compute budget
        ix_should_fail(compute_ix.clone(), &mut relayer_a, &mut test_program).await;

        // Success for correct compute budget
        tx_should_succeed(&[
            request_compute_units(BaseCommitmentHashComputation::INSTRUCTIONS[i].compute_units),
            compute_ix.clone(),
        ], &mut relayer_a, &mut test_program).await;

        // Check for:
        // - rewards of the last computation
        // - compensation of the signature costs (no negative signature costs)
        assert_eq!(
            (i as u64 + 1) * hash_reward,
            relayer_a.balance(&mut test_program).await
        );
    }

    // Additional computation will fail
    tx_should_fail(&[
        request_compute_units(1_400_000),
        compute_ix.clone(),
    ], &mut relayer_a, &mut test_program).await;

    // Finalize fails: B attempts to submit the wrong original_fee_payer
    ix_should_fail(
        ElusivInstruction::finalize_base_commitment_hash_instruction(
            0,
            WritableUserAccount(relayer_b.pubkey)
        ),
        &mut relayer_b, &mut test_program
    ).await;

    // Finalize succeeds: B supplies A as original_fee_payer
    relayer_b.airdrop(lamports_per_tx, &mut test_program).await;
    ix_should_succeed(
        ElusivInstruction::finalize_base_commitment_hash_instruction(
            0,
            WritableUserAccount(relayer_a.pubkey)
        ),
        &mut relayer_b, &mut test_program
    ).await;

    // Check that hash_account has been closed
    assert!(account_does_not_exist(BaseCommitmentHashingAccount::find(Some(0)).0, &mut test_program).await);

    // Second finalize will fail
    ix_should_fail(compute_ix.clone(), &mut relayer_a, &mut test_program).await;

    // Check commitment queue for the correct hash
    queue!(queue, CommitmentQueue, CommitmentQueueAccount, test_program);
    let commitment = queue.view_first().unwrap();
    assert_eq!(queue.len(), 1);
    assert_eq!(commitment.commitment, requests[0].commitment);
    assert_eq!(commitment.fee_version, 0);

    // Resulting balance for the relayer
    // - in the real world the relayer will combine the finalize tx with some other ix (like init commitment + hash)
    // - check that rent has been sent to A and not B, since B called finalize
    assert_eq!(
        hash_reward * BaseCommitmentHashComputation::INSTRUCTIONS.len() as u64 + rent,
        relayer_a.balance(&mut test_program).await
    );

    // SOL pool contains:
    // - amounts of the two requests
    // - for the first request the cost for hashing the commitment
    // - for the second request the cost for hashing base commitment + commitment
    let commitment_hash_fee = fee.commitment_hash_fee();
    assert_eq!(
        requests[0].amount + requests[1].amount + commitment_hash_fee + base_commitment_hash_fee - network_fee + sol_pool_start_balance,
        get_balance(sol_pool, &mut test_program).await
    );

    // Fee collector unchanged
    assert_eq!(
        2 * network_fee + fee_collector_start_balance,
        get_balance(fee_collector, &mut test_program).await
    );
}