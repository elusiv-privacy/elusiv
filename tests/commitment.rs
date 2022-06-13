//! Tests the base commitment and commitment hashing

#[cfg(not(tarpaulin_include))]
mod common;
use common::*;
use common::program_setup::*;

use elusiv::fields::{SCALAR_MODULUS, big_uint_to_u256, u256_to_fr};
use elusiv::processor::{BaseCommitmentHashRequest, MIN_STORE_AMOUNT, MAX_STORE_AMOUNT, CommitmentHashRequest};
use elusiv::state::{StorageAccount, MT_HEIGHT};
use elusiv::state::governor::{PoolAccount, FeeCollectorAccount};
use elusiv::state::program_account::{SizedAccount, MultiAccountAccountFields, MultiAccountAccount};
use elusiv::state::queue::{BaseCommitmentQueue, BaseCommitmentQueueAccount};
use elusiv::instruction::*;
use elusiv::types::U256;
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program::pubkey::Pubkey;
use solana_program_test::*;
use solana_program::account_info::Account;
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
    fee::FeeAccount,
};
use elusiv::commitment::{BaseCommitmentHashComputation, BaseCommitmentHashingAccount, CommitmentHashingAccount, CommitmentHashComputation};
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

async fn setup_commitment_tests() -> (ProgramTestContext, Actor) {
    let mut context = start_program_solana_program_test().await;
    setup_pda_accounts(&mut context).await;
    let client = Actor::new(&mut context).await;

    (context, client)
}

#[tokio::test]
async fn test_base_commitment() {
    let (mut context, mut client) = setup_commitment_tests().await;
    let requests = requests();
    let lamports_per_tx = lamports_per_signature(&mut context).await;

    pda_account!(fee, FeeAccount, Some(0), context);

    let sol_pool = PoolAccount::find(None).0;
    let sol_pool_start_balance = get_balance(sol_pool, &mut context).await;

    let fee_collector = FeeCollectorAccount::find(None).0;
    let fee_collector_start_balance = get_balance(fee_collector, &mut context).await;

    let mut relayer_a = Actor::new(&mut context).await; 
    let mut relayer_b = Actor::new(&mut context).await; 

    // Request should fail: client has not enough funds
    let store_ix = ElusivInstruction::store_base_commitment_instruction(
        0,
        requests[0].clone(),
        SignerAccount(client.pubkey),
    );
    ix_should_fail(store_ix.clone(), &mut client, &mut context).await;
    assert_eq!(0, client.balance(&mut context).await);

    // Client has enough funds
    let base_commitment_hash_fee = fee.base_commitment_hash_fee();
    let network_fee = fee.get_base_commitment_network_fee();
    let amount = requests[0].amount + base_commitment_hash_fee + lamports_per_tx;
    client.airdrop(amount, &mut context).await;
    assert_eq!(amount, client.balance(&mut context).await);

    ix_should_succeed(store_ix, &mut client, &mut context).await;

    // client has: zero-balance
    assert_eq!(0, client.balance(&mut context).await);

    // pool has: requests[0].amount + base_commitment_hash_fee - network_fee
    assert_eq!(
        network_fee + fee_collector_start_balance,
        get_balance(fee_collector, &mut context).await
    );

    // fee_collector has: network_fee
    assert_eq!(
        requests[0].amount + base_commitment_hash_fee - network_fee + sol_pool_start_balance,
        get_balance(sol_pool, &mut context).await
    );

    // Check the queue for the first request
    queue!(queue, BaseCommitmentQueue, BaseCommitmentQueueAccount, Some(0), context);
    assert_eq!(queue.len(), 1);
    assert_eq!(queue.view_first().unwrap(), requests[0]);

    // Client stores the second request
    let amount = requests[1].amount + base_commitment_hash_fee + lamports_per_tx;
    client.airdrop(amount, &mut context).await;
    ix_should_succeed(ElusivInstruction::store_base_commitment_instruction(
        0,
        requests[1].clone(),
        SignerAccount(client.pubkey),
    ), &mut client, &mut context).await;

    assert_eq!(0, client.balance(&mut context).await);
    assert_eq!(
        2 * network_fee + fee_collector_start_balance,
        get_balance(fee_collector, &mut context).await
    );
    assert_eq!(
        requests[0].amount + requests[1].amount + 2 * (base_commitment_hash_fee - network_fee) + sol_pool_start_balance,
        get_balance(sol_pool, &mut context).await
    );

    // Check the queue for the second request
    queue!(queue, BaseCommitmentQueue, BaseCommitmentQueueAccount, Some(0), context);
    assert_eq!(queue.len(), 2);
    assert_eq!(queue.view_first().unwrap(), requests[0]);
    assert_eq!(queue.view(1).unwrap(), requests[1]);

    // Init through A with hash_account at 0
    let rent = get_account_cost(&mut context, BaseCommitmentHashingAccount::SIZE).await;
    relayer_a.airdrop(lamports_per_tx + rent, &mut context).await;
    ix_should_succeed(
        ElusivInstruction::init_base_commitment_hash_instruction(
            0,
            0,
            SignerAccount(relayer_a.pubkey)
        ),
        &mut relayer_a, &mut context,
    ).await;

    // A should now have lost the cost of renting
    assert_eq!(0, relayer_a.balance(&mut context).await);

    // First request has been dequeued
    queue!(queue, BaseCommitmentQueue, BaseCommitmentQueueAccount, Some(0), context);
    assert_eq!(queue.len(), 1);

    // Second init through B will fail, since the hash_account at 0 already exists
    relayer_b.airdrop(lamports_per_tx + rent, &mut context).await;
    ix_should_fail(
        ElusivInstruction::init_base_commitment_hash_instruction(
            0,
            0,
            SignerAccount(relayer_b.pubkey)
        ),
        &mut relayer_b, &mut context,
    ).await;

    // But init through B will succeed for the hash_account with offset 1
    ix_should_succeed(
        ElusivInstruction::init_base_commitment_hash_instruction(
            0,
            1,
            SignerAccount(relayer_b.pubkey)
        ),
        &mut relayer_b, &mut context,
    ).await;
    assert_eq!(0, relayer_b.balance(&mut context).await);

    // Queue should now be empty
    queue!(queue, BaseCommitmentQueue, BaseCommitmentQueueAccount, Some(0), context);
    assert_eq!(queue.len(), 0);

    // New hash_account with the request
    assert!(account_does_exist(BaseCommitmentHashingAccount::find(Some(0)).0, &mut context).await);
    pda_account!(hash_account, BaseCommitmentHashingAccount, Some(0), context);
    assert_eq!(hash_account.get_fee_version(), 0);
    assert_eq!(hash_account.get_fee_payer(), relayer_a.pubkey.to_bytes());
    assert_eq!(hash_account.get_instruction(), 0);

    let compute_ix = ElusivInstruction::compute_base_commitment_hash_instruction(
        0,
        0,
        0,
        SignerAccount(relayer_a.pubkey),
    );
    let finalize_ix = ElusivInstruction::finalize_base_commitment_hash_instruction(
        0,
        WritableUserAccount(relayer_a.pubkey)
    );

    // Compute each base_commitment_hash
    let hash_reward = fee.get_relayer_hash_tx_fee();
    for i in 0..BaseCommitmentHashComputation::INSTRUCTIONS.len() {
        // Finalization will always fail before completion
        ix_should_fail(finalize_ix.clone(), &mut relayer_a, &mut context).await;

        // Fail due to too low compute budget
        let required_compute_budget = BaseCommitmentHashComputation::INSTRUCTIONS[i].compute_units;
        if required_compute_budget > 300_000 { // include the 100k compute unit padding
            ix_should_fail(compute_ix.clone(), &mut relayer_a, &mut context).await;
        }

        // Success for correct compute budget
        tx_should_succeed(&[
            request_compute_units(required_compute_budget),
            compute_ix.clone(),
        ], &mut relayer_a, &mut context).await;

        // Check for:
        // - rewards of the last computation
        // - compensation of the signature costs (no negative signature costs)
        assert_eq!(
            (i as u64 + 1) * hash_reward,
            relayer_a.balance(&mut context).await
        );
    }

    // Additional computation will fail
    tx_should_fail(&[
        request_compute_units(1_400_000),
        compute_ix.clone(),
    ], &mut relayer_a, &mut context).await;

    // Finalize fails: B attempts to submit the wrong original_fee_payer
    ix_should_fail(
        ElusivInstruction::finalize_base_commitment_hash_instruction(
            0,
            WritableUserAccount(relayer_b.pubkey)
        ),
        &mut relayer_b, &mut context
    ).await;

    // Finalize succeeds: B supplies A as original_fee_payer
    relayer_b.airdrop(lamports_per_tx, &mut context).await;
    ix_should_succeed(
        ElusivInstruction::finalize_base_commitment_hash_instruction(
            0,
            WritableUserAccount(relayer_a.pubkey)
        ),
        &mut relayer_b, &mut context
    ).await;

    // Check that hash_account has been closed
    assert!(account_does_not_exist(BaseCommitmentHashingAccount::find(Some(0)).0, &mut context).await);

    // Second finalize will fail
    ix_should_fail(compute_ix.clone(), &mut relayer_a, &mut context).await;

    // Check commitment queue for the correct hash
    queue!(queue, CommitmentQueue, CommitmentQueueAccount, None, context);
    let commitment = queue.view_first().unwrap();
    assert_eq!(queue.len(), 1);
    assert_eq!(commitment.commitment, requests[0].commitment);
    assert_eq!(commitment.fee_version, 0);

    // Resulting balance for the relayer
    // - in the real world the relayer will combine the finalize tx with some other ix (like init commitment + hash)
    // - check that rent has been sent to A and not B, since B called finalize
    assert_eq!(
        hash_reward * BaseCommitmentHashComputation::INSTRUCTIONS.len() as u64 + rent,
        relayer_a.balance(&mut context).await
    );

    // SOL pool contains:
    // - amounts of the two requests
    // - for the first request the cost for hashing the commitment
    // - for the second request the cost for hashing base commitment + commitment
    let commitment_hash_fee = fee.commitment_hash_fee();
    assert_eq!(
        requests[0].amount + requests[1].amount + commitment_hash_fee + base_commitment_hash_fee - network_fee + sol_pool_start_balance,
        get_balance(sol_pool, &mut context).await
    );

    // Fee collector unchanged
    assert_eq!(
        2 * network_fee + fee_collector_start_balance,
        get_balance(fee_collector, &mut context).await
    );
}

#[tokio::test]
async fn test_base_commitment_store_invalid_inputs() {
    let (mut context, mut client) = setup_commitment_tests().await;
    let request = &requests()[0];

    client.airdrop(MAX_STORE_AMOUNT / 1000, &mut context).await;

    let invalid_instructions = vec![
        // Non-existent queue offset
        ElusivInstruction::store_base_commitment_instruction(
            1000,
            request.clone(),
            SignerAccount(client.pubkey),
        ),

        // Invalid fee-version
        ElusivInstruction::store_base_commitment_instruction(
            0,
            BaseCommitmentHashRequest {
                base_commitment: request.base_commitment,
                commitment: request.commitment,
                amount: MIN_STORE_AMOUNT,
                fee_version: 1,
            },
            SignerAccount(client.pubkey),
        ),

        // Amount too low
        ElusivInstruction::store_base_commitment_instruction(
            0,
            BaseCommitmentHashRequest {
                base_commitment: request.base_commitment,
                commitment: request.commitment,
                amount: MIN_STORE_AMOUNT - 1,
                fee_version: 0,
            },
            SignerAccount(client.pubkey),
        ),

        // Amount too high
        ElusivInstruction::store_base_commitment_instruction(
            0,
            BaseCommitmentHashRequest {
                base_commitment: request.base_commitment,
                commitment: request.commitment,
                amount: MAX_STORE_AMOUNT + 1,
                fee_version: 0,
            },
            SignerAccount(client.pubkey),
        ),

        // Non-scalar base-commitment
        ElusivInstruction::store_base_commitment_instruction(
            0,
            BaseCommitmentHashRequest {
                base_commitment: big_uint_to_u256(&SCALAR_MODULUS),
                commitment: request.commitment,
                amount: MIN_STORE_AMOUNT,
                fee_version: 0,
            },
            SignerAccount(client.pubkey),
        ),
        
        // Non-scalar commitment
        ElusivInstruction::store_base_commitment_instruction(
            0,
            BaseCommitmentHashRequest {
                base_commitment: request.base_commitment,
                commitment: big_uint_to_u256(&SCALAR_MODULUS),
                amount: MIN_STORE_AMOUNT,
                fee_version: 0,
            },
            SignerAccount(client.pubkey),
        ),
    ];

    for ix in invalid_instructions {
        ix_should_fail(ix, &mut client, &mut context).await;
    }

    // Valid inputs
    ix_should_succeed(
        ElusivInstruction::store_base_commitment_instruction(
            0,
            request.clone(),
            SignerAccount(client.pubkey),
        ), &mut client, &mut context
    ).await;
}

#[tokio::test]
async fn test_base_commitment_accounts_fuzzing() {
    let (mut context, mut client) = setup_commitment_tests().await;
    let request = &requests()[0];
    let mut relayer_a = Actor::new(&mut context).await;

    // Store fuzzing
    client.airdrop(request.amount, &mut context).await;
    test_instruction_fuzzing(
        &[],
        ElusivInstruction::store_base_commitment_instruction(
            0,
            request.clone(),
            SignerAccount(client.pubkey),
        ),
        &mut client, &mut context
    ).await;

    // Init fuzzing
    test_instruction_fuzzing(
        &[],
        ElusivInstruction::init_base_commitment_hash_instruction(
            0,
            1,
            SignerAccount(relayer_a.pubkey),
        ),
        &mut relayer_a,
        &mut context
    ).await;

    // Computation fuzzing
    let valid_computation_ix = ElusivInstruction::compute_base_commitment_hash_instruction(
        1,
        0,
        0,
        SignerAccount(relayer_a.pubkey),
    );
    test_instruction_fuzzing(
        &[
            request_compute_units(1_400_000)
        ],
        valid_computation_ix.clone(),
        &mut relayer_a,
        &mut context,
    ).await;

    tx_should_succeed(&[
        request_compute_units(1_400_000),
        valid_computation_ix,
    ], &mut relayer_a, &mut context).await;

    // Finalization fuzzing
    test_instruction_fuzzing(
        &[],
        ElusivInstruction::finalize_base_commitment_hash_instruction(
            1,
            WritableUserAccount(relayer_a.pubkey),
        ),
        &mut relayer_a,
        &mut context,
    ).await;
}

#[tokio::test]
async fn test_base_commitment_full_queue() {
    let (mut context, mut client) = setup_commitment_tests().await;
    let requests = &requests();

    // Enqueue all but one
    set_pda_account::<BaseCommitmentQueueAccount, _>(&mut context, Some(0), |data| {
        queue_mut!(queue, BaseCommitmentQueue, BaseCommitmentQueueAccount, data);
        for _ in 0..BaseCommitmentQueue::CAPACITY - 1 {
            queue.enqueue(requests[0].clone()).unwrap();
        }
    }).await;

    queue!(queue, BaseCommitmentQueue, BaseCommitmentQueueAccount, Some(0), context);
    assert_eq!(queue.len(), BaseCommitmentQueue::CAPACITY - 1);
    assert_eq!(queue.empty_slots(), 1);

    // One insertion is still possible
    let ix = ElusivInstruction::store_base_commitment_instruction(
        0,
        requests[0].clone(),
        SignerAccount(client.pubkey),
    );

    client.airdrop(LAMPORTS_PER_SOL * 2, &mut context).await;
    ix_should_succeed(ix.clone(), &mut client, &mut context).await;

    // Now queue is full
    queue!(queue, BaseCommitmentQueue, BaseCommitmentQueueAccount, Some(0), context);
    assert_eq!(queue.len(), BaseCommitmentQueue::CAPACITY);
    assert_eq!(queue.empty_slots(), 0);

    client.airdrop(LAMPORTS_PER_SOL * 2, &mut context).await;
    ix_should_fail(ix, &mut client, &mut context).await;
}

#[tokio::test]
async fn test_single_commitment() {
    let (mut context, _) = setup_commitment_tests().await;
    setup_storage_account(&mut context).await;
    let requests = requests();
    let lamports_per_tx = lamports_per_signature(&mut context).await;

    pda_account!(fee, FeeAccount, Some(0), context);

    let sol_pool = PoolAccount::find(None).0;
    let sol_pool_start_balance = get_balance(sol_pool, &mut context).await;

    let mut relayer_a = Actor::new(&mut context).await;
    let mut relayer_b = Actor::new(&mut context).await;

    let storage_accounts = storage_accounts(&mut context).await;
    let writable_storage_accounts: Vec<WritableUserAccount> = storage_accounts.iter().map(|p| WritableUserAccount(*p)).collect();
    let storage_accounts: Vec<UserAccount> = storage_accounts.iter().map(|p| UserAccount(*p)).collect();

    let storage_accounts: [UserAccount; StorageAccount::COUNT] = storage_accounts.try_into().unwrap();
    let writable_storage_accounts: [WritableUserAccount; StorageAccount::COUNT] = writable_storage_accounts.try_into().unwrap();

    // Init fails, since queue is empty
    /*ix_should_fail(
        ElusivInstruction::init_commitment_hash_instruction(&storage_accounts),
        &mut relayer_a, &mut context
    ).await;*/

    // Add requests to commitment queue
    set_pda_account::<CommitmentQueueAccount, _>(&mut context, None, |data| {
        queue_mut!(queue, CommitmentQueue, CommitmentQueueAccount, data);
        queue.enqueue(
            CommitmentHashRequest {
                commitment: requests[0].commitment,
                fee_version: 0
            }
        ).unwrap();

        queue.enqueue(
            CommitmentHashRequest {
                commitment: requests[1].commitment,
                fee_version: 0
            }
        ).unwrap();
    }).await;

    // Add funds: 
    let hash_tx_count = CommitmentHashComputation::INSTRUCTIONS.len();
    let amounts = requests[0].amount + requests[1].amount;
    let hash_fee = fee.commitment_hash_fee();
    let pool_lamports = 2 * hash_fee + amounts;
    airdrop(&sol_pool, pool_lamports, &mut context).await;

    queue!(queue, CommitmentQueue, CommitmentQueueAccount, None, context);
    assert_eq!(queue.len(), 2);

    pda_account!(hashing_account, CommitmentHashingAccount, None, context);
    assert!(!hashing_account.get_is_active());

    // Init succeeds
    relayer_a.airdrop(lamports_per_tx, &mut context).await;
    ix_should_succeed(
        ElusivInstruction::init_commitment_hash_instruction(&storage_accounts),
        &mut relayer_a, &mut context
    ).await;

    pda_account!(hashing_account, CommitmentHashingAccount, None, context);
    assert!(hashing_account.get_is_active());
    assert_eq!(hashing_account.get_fee_payer(), [0; 32]);   // has no role atm
    assert_eq!(hashing_account.get_fee_version(), 0);
    assert_eq!(hashing_account.get_commitment(), requests[0].commitment);
    assert_eq!(hashing_account.get_ordering(), 0);
    // The empty tree values are the siblings
    for i in 0..MT_HEIGHT as usize {
        assert_eq!(
            hashing_account.get_siblings(i).0,
            elusiv::state::EMPTY_TREE[i]
        );
    }

    // Queue remains unchanged
    queue!(queue, CommitmentQueue, CommitmentQueueAccount, None, context);
    assert_eq!(queue.len(), 2);

    // Second init fails, since a hashing is already active
    /*ix_should_fail(
        ElusivInstruction::init_commitment_hash_instruction(&storage_accounts),
        &mut relayer_a, &mut context
    ).await;*/

    let finalize_ix = ElusivInstruction::finalize_commitment_hash_instruction(
        &writable_storage_accounts
    );

    let compute_ix = ElusivInstruction::compute_commitment_hash_instruction(
        0,
        0,
        SignerAccount(relayer_b.pubkey),
    );

    // Computation
    let single_tx_reward = fee.get_relayer_hash_tx_fee();
    for i in 0..hash_tx_count {
        // Finalization will always fail before completion
        //ix_should_fail(finalize_ix.clone(), &mut relayer_b, &mut context).await;

        // Fail due to too low compute budget
        let required_compute_budget = CommitmentHashComputation::INSTRUCTIONS[i].compute_units;
        //if required_compute_budget > 300_000 { // include the 100k compute unit padding
            //ix_should_fail(compute_ix.clone(), &mut relayer_b, &mut context).await;
        //}

        // Success for correct compute budget
        tx_should_succeed(&[
            request_compute_units(required_compute_budget),
            compute_ix.clone(),
        ], &mut relayer_b, &mut context).await;

        // Hash compensation
        // - reward per tx
        // - compensation for signature costs
        assert_eq!(
            (i as u64 + 1) * single_tx_reward,
            relayer_b.balance(&mut context).await
        );
    }
    assert_eq!(hash_fee, (single_tx_reward + lamports_per_tx) * hash_tx_count as u64);

    // Additional computation fails
    /*tx_should_fail(&[
        request_compute_units(1_400_000),
        compute_ix.clone(),
    ], &mut relayer_b, &mut context).await;*/

    // Finalization
    /*relayer_a.airdrop(lamports_per_tx, &mut context).await;
    ix_should_succeed(finalize_ix.clone(), &mut relayer_a, &mut context).await;

    // Changes in the queue
    queue!(queue, CommitmentQueue, CommitmentQueueAccount, None, context);
    let request = queue.view_first().unwrap();
    assert_eq!(queue.len(), 1);
    assert_eq!(request.commitment, requests[1].commitment);

    // Hashing account is now inactive
    pda_account!(hashing_account, CommitmentHashingAccount, None, context);
    assert!(!hashing_account.get_is_active());

    // Pool lost 1 hash_fee
    assert_eq!(
        pool_lamports - hash_fee + sol_pool_start_balance,
        get_balance(sol_pool, &mut context).await
    );

    // Check updated MT
    storage_account!(storage_account, context);
    assert_eq!(
        storage_account.get_root(),
        u256_from_str("11500204619817968836204864831937045342731531929677521260156990135685848035575")
    );
    assert_eq!(
        storage_account.get_node(0, MT_HEIGHT as usize),
        u256_to_fr(&requests[0].commitment)
    );
    assert_eq!(
        storage_account.get_next_commitment_ptr(),
        1
    );*/
}

async fn set_finished_base_commitment_hash(
    hash_account_index: u64,
    commitment: &U256,
    original_fee_payer: &Pubkey,
    context: &mut ProgramTestContext,
) {
    let len = BaseCommitmentHashingAccount::SIZE;
    let cost = get_account_cost(context, len).await;
    let mut data = vec![0; BaseCommitmentHashingAccount::SIZE];
    {
        let mut hashing_account = BaseCommitmentHashingAccount::new(&mut data).unwrap();
        hashing_account.set_instruction(&(BaseCommitmentHashComputation::INSTRUCTIONS.len() as u32));
        hashing_account.set_state(0, commitment);
        hashing_account.set_fee_payer(&original_fee_payer.to_bytes());
    }
    set_account(
        context,
        &BaseCommitmentHashingAccount::find(Some(hash_account_index)).0,
        data,
        cost,
    ).await;
}

#[tokio::test]
async fn test_commitment_full_queue() {
    let (mut context, mut client) = setup_commitment_tests().await;

    let request = CommitmentHashRequest {
        commitment: requests()[0].commitment,
        fee_version: 0
    };

    // Enqueue all
    set_pda_account::<CommitmentQueueAccount, _>(&mut context, None, |data| {
        queue_mut!(queue, CommitmentQueue, CommitmentQueueAccount, data);
        for _ in 0..CommitmentQueue::CAPACITY {
            queue.enqueue(request.clone()).unwrap();
        }
    }).await;

    queue!(queue, CommitmentQueue, CommitmentQueueAccount, None, context);
    assert_eq!(queue.len(), CommitmentQueue::CAPACITY);
    assert_eq!(queue.empty_slots(), 0);

    // Add finished base_commitment to hashing account
    set_finished_base_commitment_hash(
        0,
        &request.commitment,
        &client.pubkey,
        &mut context,
    ).await;

    // Finalization should now fail due to full queue
    ix_should_fail(
        ElusivInstruction::finalize_base_commitment_hash_instruction(
            0,
            WritableUserAccount(client.pubkey)
        ), &mut client, &mut context
    ).await;
}