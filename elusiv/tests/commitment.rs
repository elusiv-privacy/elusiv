//! Tests the base commitment and commitment hashing

mod common;
use ark_bn254::Fr;
use ark_ff::Zero;
use common::*;
use elusiv::{
    commitment::{
        commitment_hash_computation_instructions, commitments_per_batch,
        poseidon_hash::{full_poseidon2_hash, BinarySpongeHashingState},
        BaseCommitmentHashComputation, COMMITMENT_HASH_COMPUTE_BUDGET,
    },
    fields::{fr_to_u256_le, u256_to_fr_skip_mr, u64_to_scalar_skip_mr},
    instruction::{
        ElusivInstruction, SignerAccount, UserAccount, WritableSignerAccount, WritableUserAccount,
    },
    processor::{program_token_account_address, BaseCommitmentHashRequest, CommitmentHashRequest},
    state::{
        commitment::{
            BaseCommitmentHashingAccount, CommitmentHashingAccount, CommitmentQueue,
            CommitmentQueueAccount,
        },
        governor::{FeeCollectorAccount, GovernorAccount, PoolAccount},
        metadata::{CommitmentMetadata, MetadataQueue, MetadataQueueAccount},
        program_account::{PDAAccount, ProgramAccount, SizedAccount},
        queue::{Queue, RingQueue},
        storage::{StorageAccount, EMPTY_TREE, MT_HEIGHT},
    },
    token::{Lamports, Token, TokenPrice, LAMPORTS_TOKEN_ID, USDC_TOKEN_ID},
    types::{RawU256, U256},
};
use elusiv_computation::PartialComputation;
use elusiv_types::{tokens::Price, BorshSerDeSized};
use solana_program::{native_token::LAMPORTS_PER_SOL, pubkey::Pubkey, system_program};
use solana_program_test::*;

async fn enqueue_commitments(
    test: &mut ElusivProgramTest,
    requests: &[CommitmentHashRequest],
    metadata: Option<&[CommitmentMetadata]>,
) {
    test.set_pda_account::<CommitmentQueueAccount, _>(&elusiv::id(), None, None, |data| {
        queue!(mut queue, CommitmentQueue, data);

        for request in requests {
            queue.enqueue(*request).unwrap();
        }
    })
    .await;

    let metadata = if let Some(metadata) = metadata {
        assert_eq!(requests.len(), metadata.len());
        metadata.to_vec()
    } else {
        (0..requests.len())
            .map(|_| CommitmentMetadata::default())
            .collect()
    };

    test.set_pda_account::<MetadataQueueAccount, _>(&elusiv::id(), None, None, |data| {
        let mut queue = MetadataQueueAccount::new(data).unwrap();
        let mut queue = MetadataQueue::new(&mut queue);

        for metadata in metadata {
            queue.enqueue(metadata).unwrap();
        }
    })
    .await;
}

#[tokio::test]
async fn test_store_base_commitment_lamports_transfer() {
    let mut test = start_test_with_setup().await;
    let client = test.new_actor().await;
    let warden = test.new_actor().await;

    let pool = PoolAccount::find(None).0;
    let fee_collector = FeeCollectorAccount::find(None).0;

    let request = base_commitment_request(
        "8337064132573119120838379738103457054645361649757131991036638108422638197362",
        "139214303935475888711984321184227760578793579443975701453971046059378311483",
        0,
        1_000_000_000,
        LAMPORTS_TOKEN_ID,
        0,
        0,
    );
    let metadata = CommitmentMetadata::default();

    let fee = genesis_fee(&mut test).await;
    let subvention = fee.base_commitment_subvention.0;
    let computation_fee = (fee.base_commitment_hash_computation_fee()
        + fee.commitment_hash_computation_fee(request.min_batching_rate))
    .unwrap()
    .0;
    let network_fee = fee.base_commitment_network_fee.calc(request.amount);
    let hashing_account_rent = test.rent(BaseCommitmentHashingAccount::SIZE).await;

    client
        .airdrop(
            0,
            request.amount + computation_fee + network_fee - subvention,
            &mut test,
        )
        .await;
    warden
        .airdrop(0, computation_fee + hashing_account_rent.0, &mut test)
        .await;
    test.airdrop(
        &FeeCollectorAccount::find(None).0,
        Lamports(subvention).into_token_strict(),
    )
    .await;

    let hashing_account_bump = BaseCommitmentHashingAccount::find(Some(0)).1;
    let sol_price_account = test.token_to_usd_price_pyth_account(0);
    test.ix_should_succeed(
        ElusivInstruction::store_base_commitment_instruction(
            0,
            hashing_account_bump,
            request.clone(),
            metadata,
            SignerAccount(client.pubkey),
            WritableUserAccount(client.pubkey),
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(warden.pubkey),
            WritableUserAccount(pool),
            WritableUserAccount(fee_collector),
            UserAccount(sol_price_account),
            UserAccount(sol_price_account),
            UserAccount(system_program::id()),
        ),
        &[&client.keypair, &warden.keypair],
    )
    .await;

    // Client has zero lamports
    assert_eq!(0, client.lamports(&mut test).await);

    // Fee collector has network-fee as lamports
    assert_eq!(
        network_fee,
        test.pda_lamports(&fee_collector, FeeCollectorAccount::SIZE)
            .await
            .0
    );

    // Pool has request.amount + computation_fee as lamports
    assert_eq!(
        request.amount + computation_fee,
        test.pda_lamports(&pool, PoolAccount::SIZE).await.0
    );

    // Warden has computation_fee lamports
    assert_eq!(computation_fee, warden.lamports(&mut test).await);
}

#[tokio::test]
async fn test_store_base_commitment_token_transfer() {
    let mut test = start_test_with_setup().await;
    test.create_spl_token(USDC_TOKEN_ID).await;
    enable_program_token_account::<PoolAccount>(&mut test, USDC_TOKEN_ID, None).await;
    enable_program_token_account::<FeeCollectorAccount>(&mut test, USDC_TOKEN_ID, None).await;

    let mut client = test.new_actor().await;
    client.open_token_account(USDC_TOKEN_ID, 0, &mut test).await;

    let mut warden = test.new_actor().await;
    warden.open_token_account(USDC_TOKEN_ID, 0, &mut test).await;

    let pool_account = program_token_account_address::<PoolAccount>(USDC_TOKEN_ID, None).unwrap();
    let fee_collector_account =
        program_token_account_address::<FeeCollectorAccount>(USDC_TOKEN_ID, None).unwrap();

    let sol_usd_price = Price {
        price: 41,
        conf: 0,
        expo: 0,
    };
    let usdc_usd_price = Price {
        price: 1,
        conf: 0,
        expo: 0,
    };
    let sol_price_account = test.token_to_usd_price_pyth_account(0);
    let token_price_account = test.token_to_usd_price_pyth_account(USDC_TOKEN_ID);
    test.set_token_to_usd_price_pyth(0, sol_usd_price).await;
    test.set_token_to_usd_price_pyth(USDC_TOKEN_ID, usdc_usd_price)
        .await;

    let request = base_commitment_request(
        "8337064132573119120838379738103457054645361649757131991036638108422638197362",
        "139214303935475888711984321184227760578793579443975701453971046059378311483",
        0,
        1_000_000,
        USDC_TOKEN_ID,
        0,
        0,
    );
    let metadata = CommitmentMetadata::default();

    let price =
        TokenPrice::new_from_sol_price(sol_usd_price, usdc_usd_price, USDC_TOKEN_ID).unwrap();
    let fee = genesis_fee(&mut test).await;
    let subvention = fee
        .base_commitment_subvention
        .into_token(&price, USDC_TOKEN_ID)
        .unwrap();
    let computation_fee = (fee.base_commitment_hash_computation_fee()
        + fee.commitment_hash_computation_fee(request.min_batching_rate))
    .unwrap();
    let computation_fee_token = computation_fee.into_token(&price, USDC_TOKEN_ID).unwrap();
    let network_fee = Token::new(
        USDC_TOKEN_ID,
        fee.base_commitment_network_fee.calc(request.amount),
    );
    let hashing_account_rent = test.rent(BaseCommitmentHashingAccount::SIZE).await;

    client
        .airdrop(
            USDC_TOKEN_ID,
            request.amount + computation_fee_token.amount() + network_fee.amount()
                - subvention.amount(),
            &mut test,
        )
        .await;
    warden
        .airdrop(0, computation_fee.0 + hashing_account_rent.0, &mut test)
        .await;
    test.airdrop(&fee_collector_account, subvention).await;

    let hashing_account_bump = BaseCommitmentHashingAccount::find(Some(0)).1;
    test.ix_should_succeed(
        ElusivInstruction::store_base_commitment_instruction(
            0,
            hashing_account_bump,
            request.clone(),
            metadata,
            SignerAccount(client.pubkey),
            WritableUserAccount(client.get_token_account(USDC_TOKEN_ID)),
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(warden.get_token_account(USDC_TOKEN_ID)),
            WritableUserAccount(pool_account),
            WritableUserAccount(fee_collector_account),
            UserAccount(sol_price_account),
            UserAccount(token_price_account),
            UserAccount(spl_token::id()),
        ),
        &[&client.keypair, &warden.keypair],
    )
    .await;

    // Client has zero tokens
    assert_eq!(0, client.balance(USDC_TOKEN_ID, &mut test).await);

    // Fee collector has network-fee as tokens
    assert_eq!(
        network_fee.amount(),
        test.spl_balance(&fee_collector_account).await
    );

    // Pool has request.amount as tokens
    assert_eq!(request.amount, test.spl_balance(&pool_account).await);

    // Pool has computation_fee as lamports
    assert_eq!(
        computation_fee.0,
        test.pda_lamports(&PoolAccount::find(None).0, PoolAccount::SIZE)
            .await
            .0
    );

    // Warden has zero lamports
    assert_eq!(0, warden.lamports(&mut test).await);

    // Warden has computation_fee_token tokens
    assert_eq!(
        computation_fee_token.amount(),
        warden.balance(USDC_TOKEN_ID, &mut test).await
    );
}

#[tokio::test]
async fn test_base_commitment_lamports() {
    let mut test = start_test_with_setup().await;
    let client = test.new_actor().await;
    let warden_a = test.new_actor().await;
    let warden_b = test.new_actor().await;

    let request0 = base_commitment_request(
        "2373653605831809653325702328909530483017219552320948513277905949984497279624",
        "11354689880263756368702389324600778781911466694140676144665365316598881175238",
        0,
        5745748949,
        LAMPORTS_TOKEN_ID,
        0,
        1,
    );
    let request1 = base_commitment_request(
        "12104139889635562332812066919517710111891712867884647962184153324051811405076",
        "1648743558947166791659724723407286787130041879468443966677530652569417690417",
        0,
        16902202056,
        LAMPORTS_TOKEN_ID,
        0,
        1,
    );
    let metadata = CommitmentMetadata::default();

    let pool = PoolAccount::find(None).0;
    let fee_collector = FeeCollectorAccount::find(None).0;

    // Store fails: batching rate mismatch
    let store_ix = ElusivInstruction::store_base_commitment_sol_instruction(
        0,
        request0.clone(),
        metadata,
        client.pubkey,
        warden_a.pubkey,
    );
    test.ix_should_fail(store_ix.clone(), &[&client.keypair, &warden_a.keypair])
        .await;

    test.set_pda_account::<GovernorAccount, _>(&elusiv::id(), None, None, |data| {
        let mut account = GovernorAccount::new(data).unwrap();
        account.set_commitment_batching_rate(&1);
    })
    .await;

    // Store fails: client has not enough funds
    test.ix_should_fail(store_ix.clone(), &[&client.keypair, &warden_a.keypair])
        .await;

    let fee = genesis_fee(&mut test).await;
    let hashing_account_rent = test.rent(BaseCommitmentHashingAccount::SIZE).await;
    let subvention = fee.base_commitment_subvention.0;
    let computation_fee = (fee.base_commitment_hash_computation_fee()
        + fee.commitment_hash_computation_fee(request0.min_batching_rate))
    .unwrap()
    .0;
    let network_fee = fee.base_commitment_network_fee.calc(request0.amount);

    client
        .airdrop(
            LAMPORTS_TOKEN_ID,
            request0.amount + computation_fee + network_fee - subvention,
            &mut test,
        )
        .await;
    test.airdrop(
        &fee_collector,
        fee.base_commitment_subvention.into_token_strict(),
    )
    .await;
    warden_a
        .airdrop(LAMPORTS_TOKEN_ID, hashing_account_rent.0, &mut test)
        .await;

    let hashing_account_bump = BaseCommitmentHashingAccount::find(Some(0)).1;

    // Store fails: Invalid pool_account
    test.ix_should_fail(
        ElusivInstruction::store_base_commitment_instruction(
            0,
            hashing_account_bump,
            request0.clone(),
            metadata,
            SignerAccount(client.pubkey),
            WritableUserAccount(client.pubkey),
            WritableSignerAccount(warden_a.pubkey),
            WritableUserAccount(warden_a.pubkey),
            WritableUserAccount(fee_collector),
            WritableUserAccount(fee_collector),
            UserAccount(system_program::id()),
            UserAccount(system_program::id()),
            UserAccount(system_program::id()),
        ),
        &[&client.keypair, &warden_a.keypair],
    )
    .await;

    // Store fails: Invalid fee_collector_account
    test.ix_should_fail(
        ElusivInstruction::store_base_commitment_instruction(
            0,
            hashing_account_bump,
            request0.clone(),
            metadata,
            SignerAccount(client.pubkey),
            WritableUserAccount(client.pubkey),
            WritableSignerAccount(warden_a.pubkey),
            WritableUserAccount(warden_a.pubkey),
            WritableUserAccount(pool),
            WritableUserAccount(pool),
            UserAccount(system_program::id()),
            UserAccount(system_program::id()),
            UserAccount(system_program::id()),
        ),
        &[&client.keypair, &warden_a.keypair],
    )
    .await;

    // Correct batching rate and client has enough funds
    test.ix_should_succeed(store_ix, &[&client.keypair, &warden_a.keypair])
        .await;

    pda_account!(
        hash_account,
        BaseCommitmentHashingAccount,
        None,
        Some(0),
        test
    );
    assert_eq!(hash_account.get_fee_version(), 0);
    assert_eq!(hash_account.get_fee_payer(), warden_a.pubkey.to_bytes());
    assert_eq!(hash_account.get_instruction(), 0);

    assert_eq!(0, client.lamports(&mut test).await);
    assert_eq!(
        network_fee,
        test.pda_lamports(&fee_collector, FeeCollectorAccount::SIZE)
            .await
            .0
    );
    assert_eq!(
        request0.amount + computation_fee,
        test.pda_lamports(&pool, PoolAccount::SIZE).await.0
    );
    assert_eq!(0, warden_a.lamports(&mut test).await);

    // Client stores the second request
    let network_fee1 = fee.base_commitment_network_fee.calc(request1.amount);
    client
        .airdrop(
            LAMPORTS_TOKEN_ID,
            request1.amount + computation_fee + network_fee1 - subvention,
            &mut test,
        )
        .await;
    test.airdrop(
        &fee_collector,
        fee.base_commitment_subvention.into_token_strict(),
    )
    .await;
    warden_b
        .airdrop(LAMPORTS_TOKEN_ID, hashing_account_rent.0, &mut test)
        .await;

    // Same hash_account_index will fail
    test.ix_should_fail(
        ElusivInstruction::store_base_commitment_sol_instruction(
            0,
            request1.clone(),
            metadata,
            client.pubkey,
            warden_a.pubkey,
        ),
        &[&client.keypair, &warden_a.keypair],
    )
    .await;

    // Same request will fail due to a duplicate in the buffer
    test.ix_should_fail(
        ElusivInstruction::store_base_commitment_sol_instruction(
            1,
            request0.clone(),
            metadata,
            client.pubkey,
            warden_b.pubkey,
        ),
        &[&client.keypair, &warden_b.keypair],
    )
    .await;

    // Warden B with hash_account_index 1
    test.ix_should_succeed(
        ElusivInstruction::store_base_commitment_sol_instruction(
            1,
            request1.clone(),
            metadata,
            client.pubkey,
            warden_b.pubkey,
        ),
        &[&client.keypair, &warden_b.keypair],
    )
    .await;

    pda_account!(
        hash_account,
        BaseCommitmentHashingAccount,
        None,
        Some(1),
        test
    );
    assert_eq!(hash_account.get_fee_version(), 0);
    assert_eq!(hash_account.get_fee_payer(), warden_b.pubkey.to_bytes());
    assert_eq!(hash_account.get_instruction(), 0);

    assert_eq!(0, client.lamports(&mut test).await);
    assert_eq!(
        network_fee + network_fee1,
        test.pda_lamports(&fee_collector, FeeCollectorAccount::SIZE)
            .await
            .0
    );
    assert_eq!(
        request0.amount + request1.amount + computation_fee * 2,
        test.pda_lamports(&pool, PoolAccount::SIZE).await.0
    );
    assert_eq!(0, warden_b.lamports(&mut test).await);

    let compute_ix = ElusivInstruction::compute_base_commitment_hash_instruction(0);
    let finalize_ix = ElusivInstruction::finalize_base_commitment_hash_instruction(
        0,
        0,
        WritableUserAccount(warden_a.pubkey),
    );

    // Compute each base_commitment_hash
    for _ in 0..BaseCommitmentHashComputation::IX_COUNT {
        // Finalization will always fail before completion
        test.ix_should_fail_simple(finalize_ix.clone()).await;

        // Fail due to too low compute budget
        let required_compute_budget = BaseCommitmentHashComputation::COMPUTE_BUDGET_PER_IX;
        if required_compute_budget > 300_000 {
            // include the 100k compute unit padding
            test.ix_should_fail_simple(compute_ix.clone()).await;
        }

        // Success for correct compute budget
        test.tx_should_succeed_simple(&[
            request_compute_units(required_compute_budget),
            compute_ix.clone(),
        ])
        .await;
    }

    // No compensation for the warden
    assert_eq!(0, warden_a.lamports(&mut test).await);

    // Additional computation will fail
    test.tx_should_fail_simple(&[request_compute_units(1_400_000), compute_ix.clone()])
        .await;

    // Finalize fails: B attempts to submit the wrong original_fee_payer
    test.ix_should_fail_simple(
        ElusivInstruction::finalize_base_commitment_hash_instruction(
            0,
            0,
            WritableUserAccount(warden_b.pubkey),
        ),
    )
    .await;

    let finalize_ix = ElusivInstruction::finalize_base_commitment_hash_instruction(
        0,
        0,
        WritableUserAccount(warden_a.pubkey),
    );

    // Finalize fails: two finalize ix in a single tx
    test.tx_should_fail_simple(&[finalize_ix.clone(), finalize_ix.clone()])
        .await;

    // Finalize succeeds: B supplies A as original_fee_payer
    test.ix_should_succeed_simple(finalize_ix.clone()).await;

    assert_eq!(
        fee.base_commitment_hash_computation_fee().0 + hashing_account_rent.0,
        warden_a.lamports(&mut test).await
    );

    // Check that hash_account has been closed
    assert!(
        test.account_does_not_exist(&BaseCommitmentHashingAccount::find(Some(0)).0)
            .await
    );

    // Additional finalize will fail
    test.ix_should_fail_simple(finalize_ix).await;

    // Check commitment queue for the correct hash
    queue!(queue, CommitmentQueue, test);
    let commitment = queue.view_first().unwrap();
    assert_eq!(queue.len(), 1);
    // TODO: update hashes to use zero recent-commitment-index
    // assert_eq!(commitment.commitment, request0.commitment.reduce());
    assert_eq!(commitment.fee_version, 0);

    queue!(metadata_queue, MetadataQueue, test);
    assert_eq!(metadata_queue.len(), 1);
    assert_eq!(metadata, metadata_queue.view_first().unwrap());

    assert_eq!(
        request0.amount + request1.amount + computation_fee * 2
            - fee.base_commitment_hash_computation_fee().0,
        test.pda_lamports(&pool, PoolAccount::SIZE).await.0
    );
}

#[tokio::test]
async fn test_base_commitment_token() {
    let mut test = start_test_with_setup().await;
    let mut client = test.new_actor().await;
    let mut warden = test.new_actor().await;

    test.create_spl_token(USDC_TOKEN_ID).await;
    enable_program_token_account::<PoolAccount>(&mut test, USDC_TOKEN_ID, None).await;
    enable_program_token_account::<FeeCollectorAccount>(&mut test, USDC_TOKEN_ID, None).await;

    client.open_token_account(USDC_TOKEN_ID, 0, &mut test).await;
    warden.open_token_account(USDC_TOKEN_ID, 0, &mut test).await;

    let pool_account = program_token_account_address::<PoolAccount>(USDC_TOKEN_ID, None).unwrap();
    let fee_collector_account =
        program_token_account_address::<FeeCollectorAccount>(USDC_TOKEN_ID, None).unwrap();

    let sol_price_account = test.token_to_usd_price_pyth_account(0);
    let token_price_account = test.token_to_usd_price_pyth_account(USDC_TOKEN_ID);

    let sol_usd_price = Price {
        price: 41,
        conf: 0,
        expo: 0,
    };
    let usdc_usd_price = Price {
        price: 1,
        conf: 0,
        expo: 0,
    };
    test.set_token_to_usd_price_pyth(0, sol_usd_price).await;
    test.set_token_to_usd_price_pyth(USDC_TOKEN_ID, usdc_usd_price)
        .await;

    let request = base_commitment_request(
        "8337064132573119120838379738103457054645361649757131991036638108422638197362",
        "139214303935475888711984321184227760578793579443975701453971046059378311483",
        0,
        999_999,
        USDC_TOKEN_ID,
        0,
        0,
    );
    let metadata = CommitmentMetadata::default();

    let price =
        TokenPrice::new_from_sol_price(sol_usd_price, usdc_usd_price, USDC_TOKEN_ID).unwrap();
    let fee = genesis_fee(&mut test).await;
    let subvention = fee
        .base_commitment_subvention
        .into_token(&price, USDC_TOKEN_ID)
        .unwrap();
    let computation_fee = (fee.base_commitment_hash_computation_fee()
        + fee.commitment_hash_computation_fee(request.min_batching_rate))
    .unwrap();
    let computation_fee_token = computation_fee.into_token(&price, USDC_TOKEN_ID).unwrap();
    let network_fee = Token::new(
        USDC_TOKEN_ID,
        fee.base_commitment_network_fee.calc(request.amount),
    );
    let hashing_account_rent = test.rent(BaseCommitmentHashingAccount::SIZE).await;

    client
        .airdrop(
            USDC_TOKEN_ID,
            request.amount + computation_fee_token.amount() + network_fee.amount()
                - subvention.amount(),
            &mut test,
        )
        .await;
    warden
        .airdrop(0, computation_fee.0 + hashing_account_rent.0, &mut test)
        .await;
    test.airdrop(&fee_collector_account, subvention).await;

    let hashing_account_bump = BaseCommitmentHashingAccount::find(Some(0)).1;
    test.ix_should_succeed(
        ElusivInstruction::store_base_commitment_instruction(
            0,
            hashing_account_bump,
            request.clone(),
            metadata,
            SignerAccount(client.pubkey),
            WritableUserAccount(client.get_token_account(USDC_TOKEN_ID)),
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(warden.get_token_account(USDC_TOKEN_ID)),
            WritableUserAccount(pool_account),
            WritableUserAccount(fee_collector_account),
            UserAccount(sol_price_account),
            UserAccount(token_price_account),
            UserAccount(spl_token::id()),
        ),
        &[&client.keypair, &warden.keypair],
    )
    .await;

    for _ in 0..BaseCommitmentHashComputation::IX_COUNT {
        test.tx_should_succeed_simple(&[
            request_max_compute_units(),
            ElusivInstruction::compute_base_commitment_hash_instruction(0),
        ])
        .await;
    }

    test.ix_should_succeed_simple(
        ElusivInstruction::finalize_base_commitment_hash_instruction(
            0,
            0,
            WritableUserAccount(warden.pubkey),
        ),
    )
    .await;

    // Client has zero tokens
    assert_eq!(0, client.balance(USDC_TOKEN_ID, &mut test).await);

    // Fee collector has network-fee as tokens
    assert_eq!(
        network_fee.amount(),
        test.spl_balance(&fee_collector_account).await
    );

    // Pool has request.amount as tokens
    assert_eq!(request.amount, test.spl_balance(&pool_account).await);

    // Pool has computation_fee - base_commitment_fee as lamports
    assert_eq!(
        computation_fee.0 - fee.base_commitment_hash_computation_fee().0,
        test.pda_lamports(&PoolAccount::find(None).0, PoolAccount::SIZE)
            .await
            .0
    );

    // Warden has base_commitment_fee lamports
    assert_eq!(
        fee.base_commitment_hash_computation_fee().0 + hashing_account_rent.0,
        warden.lamports(&mut test).await
    );

    // Warden has computation_fee_token tokens
    assert_eq!(
        computation_fee_token.amount(),
        warden.balance(USDC_TOKEN_ID, &mut test).await
    );
}

pub fn base_commitment_request(
    base_commitment: &str,
    commitment: &str,
    recent_commitment_index: u32,
    amount: u64,
    token_id: u16,
    fee_version: u32,
    min_batching_rate: u32,
) -> BaseCommitmentHashRequest {
    BaseCommitmentHashRequest {
        base_commitment: RawU256::new(u256_from_str_skip_mr(base_commitment)),
        commitment: RawU256::new(u256_from_str_skip_mr(commitment)),
        recent_commitment_index,
        amount,
        token_id,
        fee_version,
        min_batching_rate,
    }
}

#[tokio::test]
async fn test_single_commitment() {
    let mut test = start_test_with_setup().await;
    let warden = test.new_actor().await;

    setup_storage_account(&mut test).await;
    setup_metadata_account(&mut test).await;

    let storage_accounts = storage_accounts(&mut test).await;
    let metadata_accounts = metadata_accounts(&mut test).await;

    let metadata = [3; CommitmentMetadata::SIZE];
    let request = base_commitment_request(
        "8337064132573119120838379738103457054645361649757131991036638108422638197362",
        "139214303935475888711984321184227760578793579443975701453971046059378311483",
        123,
        1_000_000_000,
        LAMPORTS_TOKEN_ID,
        0,
        0,
    );

    let fee = genesis_fee(&mut test).await;
    let pool = PoolAccount::find(None).0;

    // Add requests to commitment queue
    enqueue_commitments(
        &mut test,
        &[CommitmentHashRequest {
            commitment: request.commitment.reduce(),
            fee_version: 0,
            min_batching_rate: 0,
        }],
        Some(&[metadata]),
    )
    .await;

    let hash_tx_count = commitment_hash_computation_instructions(0).len();
    let hash_fee = fee.commitment_hash_computation_fee(0).0;
    test.airdrop_lamports(&pool, hash_fee + request.amount)
        .await;

    queue!(queue, CommitmentQueue, test);
    assert_eq!(queue.len(), 1);

    pda_account!(hashing_account, CommitmentHashingAccount, None, None, test);
    assert!(!hashing_account.get_is_active());

    // Init succeeds
    test.tx_should_succeed_simple(&[
        ElusivInstruction::init_commitment_hash_setup_instruction(false, &[]),
        ElusivInstruction::init_commitment_hash_instruction(
            false,
            &writable_user_accounts(&metadata_accounts),
        ),
    ])
    .await;

    pda_account!(hashing_account, CommitmentHashingAccount, None, None, test);
    assert!(hashing_account.get_is_active());
    assert_eq!(hashing_account.get_fee_version(), 0);
    assert_eq!(
        hashing_account.get_hash_tree(0),
        request.commitment.reduce()
    );
    assert_eq!(hashing_account.get_ordering(), 0);
    // The empty tree values are the siblings
    for i in 0..MT_HEIGHT as usize {
        assert_eq!(
            hashing_account.get_siblings(i),
            elusiv::state::storage::EMPTY_TREE[i]
        );
    }

    queue!(queue, CommitmentQueue, test);
    assert_eq!(queue.len(), 0);

    // Second init fails, since a hashing is already active
    test.tx_should_fail_simple(&[
        ElusivInstruction::init_commitment_hash_setup_instruction(false, &[]),
        ElusivInstruction::init_commitment_hash_instruction(
            false,
            &writable_user_accounts(&metadata_accounts),
        ),
    ])
    .await;

    let finalize_ix = ElusivInstruction::finalize_commitment_hash_instruction(
        &writable_user_accounts(&storage_accounts),
    );

    let compute_ix = ElusivInstruction::compute_commitment_hash_instruction(
        0,
        0,
        WritableSignerAccount(warden.pubkey),
    );

    // Computation
    for i in 0..hash_tx_count {
        // Finalization will always fail before completion
        test.ix_should_fail_simple(finalize_ix.clone()).await;

        // Fail due to too low compute budget
        let required_compute_budget = COMMITMENT_HASH_COMPUTE_BUDGET;
        if required_compute_budget > 300_000 {
            // includes the 100k compute unit padding
            test.ix_should_fail(compute_ix.clone(), &[&warden.keypair])
                .await;
        }

        // Success for correct compute budget
        test.tx_should_succeed(
            &[
                request_compute_units(required_compute_budget),
                compute_ix.clone(),
            ],
            &[&warden.keypair],
        )
        .await;

        assert_eq!(
            (i as u64 + 1) * (fee.warden_hash_tx_reward.0 + fee.lamports_per_tx.0),
            warden.lamports(&mut test).await
        );
    }

    // Additional computation fails
    test.tx_should_fail(
        &[request_max_compute_units(), compute_ix.clone()],
        &[&warden.keypair],
    )
    .await;

    // Finalization
    test.ix_should_succeed_simple(finalize_ix.clone()).await;

    // Hashing account is now inactive
    pda_account!(hashing_account, CommitmentHashingAccount, None, None, test);
    assert!(!hashing_account.get_is_active());

    assert_eq!(
        request.amount,
        test.pda_lamports(&pool, PoolAccount::SIZE).await.0
    );

    // Verify updated MT
    #[allow(clippy::needless_range_loop)]
    storage_account(None, &mut test, |s: &StorageAccount| {
        assert_eq!(
            s.get_root().unwrap(),
            u256_from_str(
                "11500204619817968836204864831937045342731531929677521260156990135685848035575"
            )
        );
        assert_eq!(
            s.get_node(0, MT_HEIGHT as usize).unwrap(),
            request.commitment.reduce()
        );
        assert_eq!(s.get_next_commitment_ptr(), 1);
        let mut hash = u256_to_fr_skip_mr(&request.commitment.reduce());
        for i in 0..MT_HEIGHT as usize {
            assert_eq!(
                fr_to_u256_le(&hash),
                s.get_node(0, MT_HEIGHT as usize - i).unwrap()
            );
            hash = full_poseidon2_hash(hash, u256_to_fr_skip_mr(&EMPTY_TREE[i]));
        }
        assert_eq!(fr_to_u256_le(&hash), s.get_root().unwrap());

        // Root should be equal to first mt_root_history value
        assert_eq!(s.get_root().unwrap(), s.get_active_mt_root_history(0));
    })
    .await;

    // Verify updated metadata
    metadata_account(None, &mut test, |m| {
        assert_eq!(m.get_commitment_metadata(0).unwrap(), metadata);
    })
    .await;
}

async fn set_finished_base_commitment_hash(
    hash_account_index: u32,
    commitment: &U256,
    original_fee_payer: &Pubkey,
    test: &mut ElusivProgramTest,
) {
    let mut data = vec![0; BaseCommitmentHashingAccount::SIZE];
    {
        let mut hashing_account = BaseCommitmentHashingAccount::new(&mut data).unwrap();
        hashing_account.set_instruction(&(BaseCommitmentHashComputation::IX_COUNT as u32));
        hashing_account.set_state(&BinarySpongeHashingState([
            u256_to_fr_skip_mr(commitment),
            Fr::zero(),
            Fr::zero(),
        ]));
        hashing_account.set_fee_payer(&original_fee_payer.to_bytes());
    }
    test.set_program_account_rent_exempt(
        &elusiv::id(),
        &BaseCommitmentHashingAccount::find(Some(hash_account_index)).0,
        &data,
    )
    .await;
}

#[tokio::test]
async fn test_commitment_full_queue() {
    let mut test = start_test_with_setup().await;
    let warden = test.new_actor().await;

    let request = CommitmentHashRequest {
        commitment: u256_from_str("0"),
        fee_version: 0,
        min_batching_rate: 0,
    };

    // Enqueue all
    test.set_pda_account::<CommitmentQueueAccount, _>(&elusiv::id(), None, None, |data| {
        queue!(mut queue, CommitmentQueue, data);

        for _ in 0..CommitmentQueue::CAPACITY {
            queue.enqueue(request).unwrap();
        }
    })
    .await;

    queue!(queue, CommitmentQueue, test);
    assert_eq!(queue.len(), CommitmentQueue::CAPACITY);
    assert_eq!(queue.empty_slots(), 0);

    // Add finished base_commitment to hashing account
    set_finished_base_commitment_hash(0, &request.commitment, &warden.pubkey, &mut test).await;

    // Finalization should now fail due to full queue
    test.ix_should_fail_simple(
        ElusivInstruction::finalize_base_commitment_hash_instruction(
            0,
            0,
            WritableUserAccount(warden.pubkey),
        ),
    )
    .await;
}

#[tokio::test]
async fn test_commitment_correct_storage_account_insertion() {
    let mut test = start_test_with_setup().await;

    setup_storage_account(&mut test).await;
    let storage_accounts = storage_accounts(&mut test).await;

    let len = commitment_hash_computation_instructions(0).len() as u32;
    let commitment_count = 33;

    for i in 0..commitment_count {
        test.set_pda_account::<CommitmentHashingAccount, _>(&elusiv::id(), None, None, |data| {
            let mut account = CommitmentHashingAccount::new(data).unwrap();
            account.set_is_active(&true);
            account.set_instruction(&len);
            account.set_ordering(&i);
            account.set_finalization_ix(&0);

            account.set_hash_tree(0, &fr_to_u256_le(&u64_to_scalar_skip_mr(i as u64)));
        })
        .await;

        test.ix_should_succeed_simple(ElusivInstruction::finalize_commitment_hash_instruction(
            &writable_user_accounts(&storage_accounts),
        ))
        .await;
    }

    // Check that each commitment is at the correct position
    storage_account(None, &mut test, |s: &StorageAccount| {
        for i in 0..commitment_count {
            assert_eq!(
                s.get_node(i as usize, MT_HEIGHT as usize).unwrap(),
                fr_to_u256_le(&u64_to_scalar_skip_mr(i as u64))
            );
        }
    })
    .await;
}

#[tokio::test]
async fn test_commitment_hash_multiple_commitments_zero_batch() {
    let mut test = start_test_with_setup().await;
    let warden = test.new_actor().await;

    setup_storage_account(&mut test).await;
    setup_metadata_account(&mut test).await;

    let storage_accounts = storage_accounts(&mut test).await;
    let metadata_accounts = metadata_accounts(&mut test).await;

    let pool = PoolAccount::find(None).0;
    test.airdrop_lamports(&pool, LAMPORTS_PER_SOL * 100).await;

    let commitments = vec![
        u256_from_str(
            "17695089122606640046122050453568281484908329551111425943069599106344573268591",
        ),
        u256_from_str(
            "6647356857703578745245713474272809288360618637120301827353679811066213900723",
        ),
        u256_from_str(
            "15379640546683409691976024780847698243281026803042985142030905481489858510622",
        ),
        u256_from_str(
            "9526685147941891237781527305630522288121859341465303072844645355022143819256",
        ),
        u256_from_str(
            "4912675451529070464762528188865498315454175094749833577169306500804282376621",
        ),
        u256_from_str(
            "14672838342938789129773189810958973041204269514853784121478587260372791091464",
        ),
        u256_from_str(
            "5808462669014571118534375825896524695834768083342937741019165053845945714865",
        ),
    ];

    let requests: Vec<CommitmentHashRequest> = commitments
        .iter()
        .map(|c| CommitmentHashRequest {
            commitment: *c,
            fee_version: 0,
            min_batching_rate: 0,
        })
        .collect();

    let correct_roots_afterwards = vec![
        u256_from_str(
            "9067782498943005972697481747658603367081340211439558541654633405673676102857",
        ),
        u256_from_str(
            "15301892188911160449341837174902405446602050384096489477117140364841430914614",
        ),
        u256_from_str(
            "8712848136848990562797370443371161139823751675261015848376388074182704347947",
        ),
        u256_from_str(
            "6543817352315114290363106811223879539017599496237896578152011659905900001939",
        ),
        u256_from_str(
            "7664287681500223472370483741580378590496434315208292049383954342296148132753",
        ),
        u256_from_str(
            "10008823716965287250940652746474616373356829327674075836642853586040635964761",
        ),
        u256_from_str(
            "21620303059720667189546524860541209640581655979702452251272504609177116384089",
        ),
    ];

    enqueue_commitments(
        &mut test,
        &requests,
        Some(
            &requests
                .iter()
                .enumerate()
                .map(|(i, _)| [i as u8; CommitmentMetadata::SIZE])
                .collect::<Vec<_>>(),
        ),
    )
    .await;

    // Init, compute, finalize every commitment
    for i in 0..requests.len() {
        test.tx_should_succeed_simple(&[
            ElusivInstruction::init_commitment_hash_setup_instruction(
                false,
                &user_accounts(&storage_accounts),
            ),
            ElusivInstruction::init_commitment_hash_instruction(
                false,
                &writable_user_accounts(&metadata_accounts),
            ),
        ])
        .await;

        for _ in commitment_hash_computation_instructions(0).iter() {
            test.tx_should_succeed(
                &[
                    request_compute_units(COMMITMENT_HASH_COMPUTE_BUDGET),
                    ElusivInstruction::compute_commitment_hash_instruction(
                        0,
                        0,
                        WritableSignerAccount(warden.pubkey),
                    ),
                ],
                &[&warden.keypair],
            )
            .await;
        }

        test.ix_should_succeed_simple(ElusivInstruction::finalize_commitment_hash_instruction(
            &writable_user_accounts(&storage_accounts),
        ))
        .await;

        // Verify commitment and root
        storage_account(None, &mut test, |s: &StorageAccount| {
            assert_eq!(
                s.get_node(i, MT_HEIGHT as usize).unwrap(),
                requests[i].commitment
            );
            assert_eq!(s.get_root().unwrap(), correct_roots_afterwards[i]);
        })
        .await;
    }

    // Verify all commitments
    storage_account(None, &mut test, |s: &StorageAccount| {
        for (i, request) in requests.iter().enumerate() {
            assert_eq!(
                s.get_node(i, MT_HEIGHT as usize).unwrap(),
                request.commitment
            );
        }
    })
    .await;

    // Verify all metadata
    metadata_account(None, &mut test, |m| {
        for i in 0..requests.len() {
            assert_eq!(
                m.get_commitment_metadata(i).unwrap(),
                [i as u8; CommitmentMetadata::SIZE]
            );
        }
    })
    .await;
}

async fn test_commitment_hash_with_batching_rate(
    batching_rate: u32,
    commitments: &[U256],
    root: Option<U256>,
) {
    assert_eq!(commitments.len(), commitments_per_batch(batching_rate));

    let mut test = start_test_with_setup().await;
    let warden = test.new_actor().await;

    setup_storage_account(&mut test).await;
    setup_metadata_account(&mut test).await;

    let storage_accounts = storage_accounts(&mut test).await;
    let metadata_accounts = metadata_accounts(&mut test).await;

    let pool = PoolAccount::find(None).0;
    test.airdrop_lamports(&pool, LAMPORTS_PER_SOL * 100).await;

    let requests: Vec<CommitmentHashRequest> = commitments
        .iter()
        .map(|c| CommitmentHashRequest {
            commitment: *c,
            fee_version: 0,
            min_batching_rate: batching_rate,
        })
        .collect();

    enqueue_commitments(
        &mut test,
        &requests,
        Some(
            &requests
                .iter()
                .enumerate()
                .map(|(i, _)| [i as u8; CommitmentMetadata::SIZE])
                .collect::<Vec<_>>(),
        ),
    )
    .await;

    // Init, compute, finalize every commitment
    test.tx_should_succeed_simple(&[
        ElusivInstruction::init_commitment_hash_setup_instruction(
            false,
            &user_accounts(&storage_accounts),
        ),
        ElusivInstruction::init_commitment_hash_instruction(
            false,
            &writable_user_accounts(&metadata_accounts),
        ),
    ])
    .await;

    for _ in commitment_hash_computation_instructions(batching_rate).iter() {
        test.tx_should_succeed(
            &[
                request_compute_units(COMMITMENT_HASH_COMPUTE_BUDGET),
                ElusivInstruction::compute_commitment_hash_instruction(
                    0,
                    0,
                    WritableSignerAccount(warden.pubkey),
                ),
            ],
            &[&warden.keypair],
        )
        .await;
    }

    for _ in 0..=batching_rate {
        test.ix_should_succeed_simple(ElusivInstruction::finalize_commitment_hash_instruction(
            &writable_user_accounts(&storage_accounts),
        ))
        .await;
    }

    // Verify all commitments and root
    storage_account(None, &mut test, |s: &StorageAccount| {
        for (i, request) in requests.iter().enumerate() {
            assert_eq!(
                s.get_node(i, MT_HEIGHT as usize).unwrap(),
                request.commitment
            );
        }
        if let Some(root) = root {
            assert_eq!(s.get_root().unwrap(), root);
        }
        assert_eq!(s.get_next_commitment_ptr(), commitments.len() as u32);
    })
    .await;

    // Verify all metadata
    metadata_account(None, &mut test, |m| {
        for i in 0..requests.len() {
            assert_eq!(
                m.get_commitment_metadata(i).unwrap(),
                [i as u8; CommitmentMetadata::SIZE]
            );
        }
    })
    .await;

    // Queue should be empty
    queue!(queue, CommitmentQueue, test);
    assert!(queue.is_empty());

    queue!(metadata_queue, MetadataQueue, test);
    assert!(metadata_queue.is_empty());
}

#[tokio::test]
async fn test_commitment_hash_batching_rate_one() {
    test_commitment_hash_with_batching_rate(
        1,
        &[
            u256_from_str(
                "17695089122606640046122050453568281484908329551111425943069599106344573268591",
            ),
            u256_from_str(
                "6647356857703578745245713474272809288360618637120301827353679811066213900723",
            ),
        ],
        Some(u256_from_str(
            "15301892188911160449341837174902405446602050384096489477117140364841430914614",
        )),
    )
    .await;
}

#[tokio::test]
async fn test_commitment_hash_batching_rate_two() {
    let commitments = vec![
        u256_from_str(
            "17695089122606640046122050453568281484908329551111425943069599106344573268591",
        ),
        u256_from_str(
            "6647356857703578745245713474272809288360618637120301827353679811066213900723",
        ),
        u256_from_str(
            "15379640546683409691976024780847698243281026803042985142030905481489858510622",
        ),
        u256_from_str(
            "9526685147941891237781527305630522288121859341465303072844645355022143819256",
        ),
    ];
    let root = u256_from_str(
        "6543817352315114290363106811223879539017599496237896578152011659905900001939",
    );
    test_commitment_hash_with_batching_rate(2, &commitments, Some(root)).await;

    // Verify the const value
    let a = full_poseidon2_hash(
        u256_to_fr_skip_mr(&commitments[0]),
        u256_to_fr_skip_mr(&commitments[1]),
    );
    let b = full_poseidon2_hash(
        u256_to_fr_skip_mr(&commitments[2]),
        u256_to_fr_skip_mr(&commitments[3]),
    );

    let mut hash = full_poseidon2_hash(a, b);
    for i in 2..MT_HEIGHT {
        hash = full_poseidon2_hash(hash, u256_to_fr_skip_mr(&EMPTY_TREE[i as usize]))
    }
    assert_eq!(fr_to_u256_le(&hash), root);
}

#[tokio::test]
async fn test_commitment_hash_batching_rate_three() {
    // TODO: add correct root (atm just ignored)
    test_commitment_hash_with_batching_rate(
        3,
        &[
            u256_from_str(
                "17695089122606640046122050453568281484908329551111425943069599106344573268591",
            ),
            u256_from_str(
                "6647356857703578745245713474272809288360618637120301827353679811066213900723",
            ),
            u256_from_str(
                "15379640546683409691976024780847698243281026803042985142030905481489858510622",
            ),
            u256_from_str(
                "9526685147941891237781527305630522288121859341465303072844645355022143819256",
            ),
            u256_from_str(
                "17695089122606640046122050453568281484908329551111425943069599106344573268591",
            ),
            u256_from_str(
                "6647356857703578745245713474272809288360618637120301827353679811066213900723",
            ),
            u256_from_str(
                "15379640546683409691976024780847698243281026803042985142030905481489858510622",
            ),
            u256_from_str(
                "9526685147941891237781527305630522288121859341465303072844645355022143819256",
            ),
        ],
        None,
    )
    .await;
}

#[tokio::test]
async fn test_commitment_hash_batching_rate_four() {
    // TODO: add correct root (atm just ignored)
    test_commitment_hash_with_batching_rate(
        4,
        &[
            u256_from_str(
                "17695089122606640046122050453568281484908329551111425943069599106344573268591",
            ),
            u256_from_str(
                "6647356857703578745245713474272809288360618637120301827353679811066213900723",
            ),
            u256_from_str(
                "15379640546683409691976024780847698243281026803042985142030905481489858510622",
            ),
            u256_from_str(
                "9526685147941891237781527305630522288121859341465303072844645355022143819256",
            ),
            u256_from_str(
                "17695089122606640046122050453568281484908329551111425943069599106344573268591",
            ),
            u256_from_str(
                "6647356857703578745245713474272809288360618637120301827353679811066213900723",
            ),
            u256_from_str(
                "15379640546683409691976024780847698243281026803042985142030905481489858510622",
            ),
            u256_from_str(
                "9526685147941891237781527305630522288121859341465303072844645355022143819256",
            ),
            u256_from_str(
                "17695089122606640046122050453568281484908329551111425943069599106344573268591",
            ),
            u256_from_str(
                "6647356857703578745245713474272809288360618637120301827353679811066213900723",
            ),
            u256_from_str(
                "15379640546683409691976024780847698243281026803042985142030905481489858510622",
            ),
            u256_from_str(
                "9526685147941891237781527305630522288121859341465303072844645355022143819256",
            ),
            u256_from_str(
                "17695089122606640046122050453568281484908329551111425943069599106344573268591",
            ),
            u256_from_str(
                "6647356857703578745245713474272809288360618637120301827353679811066213900723",
            ),
            u256_from_str(
                "15379640546683409691976024780847698243281026803042985142030905481489858510622",
            ),
            u256_from_str(
                "9526685147941891237781527305630522288121859341465303072844645355022143819256",
            ),
        ],
        None,
    )
    .await;
}
