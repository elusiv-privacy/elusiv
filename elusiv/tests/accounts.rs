//! Tests the account setup process

#[cfg(not(tarpaulin_include))]
mod common;
use std::collections::HashMap;

use borsh::BorshSerialize;
use common::*;
use elusiv::proof::precompute::{precompute_account_size2, VKEY_COUNT, PrecomputesAccount, VirtualPrecomputes, PrecomutedValues};
use elusiv::proof::vkey::{SendQuadraVKey, VerificationKey};
use elusiv::state::program_account::{MultiAccountAccount, SUB_ACCOUNT_ADDITIONAL_SIZE, PDAOffset, SubAccount};
use elusiv::state::queue::RingQueue;
use elusiv::state::{StorageAccount, MT_COMMITMENT_COUNT};
use elusiv::commitment::CommitmentHashingAccount;
use elusiv::instruction::*;
use elusiv::processor::{SingleInstancePDAAccountKind, MultiInstancePDAAccountKind, CommitmentHashRequest};
use elusiv::token::SPL_TOKEN_COUNT;
use elusiv_utils::batch_instructions;
use solana_program::instruction::{Instruction, AccountMeta};
use solana_program::pubkey::Pubkey;
use solana_program_test::*;
use elusiv::state::{
    queue::{CommitmentQueue, CommitmentQueueAccount, Queue},
    program_account::{PDAAccount, SizedAccount, ProgramAccount, MultiAccountProgramAccount, PDAAccountData},
    fee::FeeAccount,
    NullifierAccount,
    governor::{GovernorAccount, PoolAccount, FeeCollectorAccount},
};
use solana_sdk::signer::Signer;

#[tokio::test]
async fn test_setup_initial_accounts() {
    let mut test = ElusivProgramTest::start_with_setup().await;

    async fn assert_account<T: PDAAccount + SizedAccount>(test: &mut ElusivProgramTest, pda_offset: PDAOffset) {
        let data = test.data(&T::find(pda_offset).0).await;
    
        // Check balance and data size
        assert_eq!(test.lamports(&T::find(pda_offset).0).await, test.rent(T::SIZE).await);
        assert_eq!(data.len(), T::SIZE);
    
        // Check pda account fields
        let data = PDAAccountData::new(&data).unwrap();
        assert_eq!(data.bump_seed, T::find(pda_offset).1);
        assert_eq!(data.version, 0);
        assert!(!data.initialized);
    }

    assert_account::<GovernorAccount>(&mut test, None).await;
    assert_account::<PoolAccount>(&mut test, None).await;
    assert_account::<FeeCollectorAccount>(&mut test, None).await;
    assert_account::<PrecomputesAccount>(&mut test, None).await;
    assert_account::<CommitmentHashingAccount>(&mut test, None).await;
    assert_account::<CommitmentQueueAccount>(&mut test, None).await;
}

#[tokio::test]
async fn test_setup_initial_accounts_duplicate() {
    let mut test = ElusivProgramTest::start().await;
    let ixs = open_all_initial_accounts(test.context().payer.pubkey());
    let mut double = ixs.clone();
    double.extend(ixs.clone());

    test.tx_should_fail_simple(&double).await;

    test.tx_should_succeed_simple(&ixs).await;

    test.tx_should_fail_simple(&double).await;
    test.tx_should_fail_simple(&ixs).await;
}

#[tokio::test]
async fn test_enable_token_account() {
    let mut test = ElusivProgramTest::start().await;
    test.setup_initial_pdas().await;

    for token_id in 1..=SPL_TOKEN_COUNT as u16 {
        test.create_spl_token(token_id, false).await;
        enable_program_token_account::<PoolAccount>(&mut test, token_id, None).await;
        enable_program_token_account::<FeeCollectorAccount>(&mut test, token_id, None).await;
    }
}

#[tokio::test]
async fn test_setup_fee_account() {
    let mut test = ElusivProgramTest::start().await;
    let payer = test.context().payer.pubkey();

    test.ix_should_succeed_simple(
        ElusivInstruction::setup_governor_account_instruction(
            WritableSignerAccount(payer),
            WritableUserAccount(GovernorAccount::find(None).0)
        )
    ).await;
    
    let genesis_fee = test.genesis_fee().await;
    test.setup_fee(0, genesis_fee.clone()).await;

    // Second time will fail
    test.ix_should_fail_simple(
        ElusivInstruction::init_new_fee_version_instruction(0, genesis_fee.clone(), WritableSignerAccount(payer))
    ).await;
    
    pda_account!(fee, FeeAccount, Some(0), test);
    assert_eq!(fee.get_program_fee(), genesis_fee);

    pda_account!(governor, GovernorAccount, None, test);
    assert_eq!(governor.get_program_fee(), genesis_fee);

    // Attempting to set a version higher than genesis (0) will fail
    test.ix_should_fail_simple(
        ElusivInstruction::init_new_fee_version_instruction(1, genesis_fee.clone(), WritableSignerAccount(payer))
    ).await;

    // But after governor allows it, fee_version 1 can be set
    set_single_pda_account!(GovernorAccount, None, test, |account: &mut GovernorAccount| {
        account.set_fee_version(&1);
    });

    test.ix_should_succeed_simple(
        ElusivInstruction::init_new_fee_version_instruction(1, genesis_fee, WritableSignerAccount(payer))
    ).await;
}

#[tokio::test]
async fn test_setup_pda_accounts_invalid_pda() {
    let mut test = ElusivProgramTest::start().await;

    test.ix_should_fail_simple(
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::CommitmentQueueAccount,
            WritableSignerAccount(test.payer()),
            WritableUserAccount(FeeAccount::find(None).0)
        )
    ).await;
}

#[tokio::test]
async fn test_setup_storage_account() {
    let mut test = ElusivProgramTest::start().await;
    let keys = test.setup_storage_account().await;

    storage_account(None, &mut test, |storage_account| {
        let pks: Vec<Pubkey> = storage_account.get_multi_account_data().pubkeys.iter().map(|p| p.option().unwrap()).collect();
        assert_eq!(keys, pks);
    }).await;
}

#[tokio::test]
async fn test_setup_storage_account_duplicate() {
    let mut test = ElusivProgramTest::start().await;
    test.setup_storage_account().await;

    // Cannot set a sub-account twice
    let k = test.create_program_account_rent_exempt(StorageAccount::ACCOUNT_SIZE).await;
    test.ix_should_fail_simple(
        ElusivInstruction::enable_storage_sub_account_instruction(1, WritableUserAccount(k.pubkey()))
    ).await;

    // Cannot init storage PDA twice
    test.ix_should_fail_simple(
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::StorageAccount,
            WritableSignerAccount(test.payer()),
            WritableUserAccount(StorageAccount::find(None).0),
        )
    ).await;
}

#[tokio::test]
async fn test_open_new_merkle_tree() {
    let mut test = ElusivProgramTest::start().await;

    // Multiple MTs can be opened
    for mt_index in 0..3 {
        let keys = test.create_merkle_tree(mt_index).await;

        nullifier_account(Some(mt_index), &mut test, |nullfier_account: &NullifierAccount| {
            let pks: Vec<Pubkey> = nullfier_account.get_multi_account_data().pubkeys.iter().map(|p| p.option().unwrap()).collect();
            assert_eq!(keys, pks);
        }).await;
    }
}

#[tokio::test]
async fn test_open_new_merkle_tree_duplicate() {
    let mut test = ElusivProgramTest::start().await;
    test.create_merkle_tree(0).await;

    // Cannot init MT twice
    test.ix_should_fail_simple(
        ElusivInstruction::open_multi_instance_account_instruction(
            MultiInstancePDAAccountKind::NullifierAccount,
            0,
            WritableSignerAccount(test.payer()),
            WritableUserAccount(StorageAccount::find(Some(0)).0),
        )
    ).await;

    // Cannot set sub-account twice
    let k = test.create_program_account_rent_exempt(NullifierAccount::ACCOUNT_SIZE).await;
    test.ix_should_fail_simple(
        ElusivInstruction::enable_nullifier_sub_account_instruction(
            0,
            1,
            WritableUserAccount(k.pubkey()),
        )
    ).await;
}

#[tokio::test]
async fn test_reset_active_mt() {
    let mut test = ElusivProgramTest::start().await;
    test.setup_initial_pdas().await;
    test.setup_storage_account().await;

    test.create_merkle_tree(0).await;
    test.create_merkle_tree(1).await;

    let storage_accounts = test.storage_accounts().await;
    let root_storage_account = storage_accounts[0];
    let storage_accounts = writable_user_accounts(&storage_accounts);

    // Failure since active MT is not full
    test.ix_should_fail_simple(
        ElusivInstruction::reset_active_merkle_tree_instruction(0, &storage_accounts)
    ).await;

    // Set active MT as full
    test.set_pda_account::<StorageAccount, _>(None, |data| {
        let mut storage_account = StorageAccount::new(data, HashMap::new()).unwrap();
        storage_account.set_next_commitment_ptr(&(MT_COMMITMENT_COUNT as u32));
    }).await;

    // Override the root
    let root = [1; 32];
    let mut data = test.data(&root_storage_account).await;
    SubAccount::new(&mut data).data[..32].copy_from_slice(&root[..32]);
    test.set_program_account_rent_exempt(&root_storage_account, &data).await;

    // Failure since active_nullifier_account is invalid
    test.ix_should_fail_simple(
        Instruction::new_with_bytes(
            elusiv::id(),
            &ElusivInstruction::ResetActiveMerkleTree { active_mt_index: 0 }.try_to_vec().unwrap()[..],
            vec![
                AccountMeta::new(StorageAccount::find(None).0, false),
                AccountMeta::new(CommitmentQueueAccount::find(None).0, false),
                AccountMeta::new(NullifierAccount::find(Some(1)).0, false),
            ]
        )
    ).await;

    // Success
    test.ix_should_succeed_simple(
        Instruction::new_with_bytes(
            elusiv::id(),
            &ElusivInstruction::ResetActiveMerkleTree { active_mt_index: 0 }.try_to_vec().unwrap()[..],
            vec![
                AccountMeta::new(StorageAccount::find(None).0, false),
                AccountMeta::new(root_storage_account, false),
                AccountMeta::new(CommitmentQueueAccount::find(None).0, false),
                AccountMeta::new(NullifierAccount::find(Some(0)).0, false),
            ]
        )
    ).await;

    nullifier_account(Some(0), &mut test, |n: &NullifierAccount| {
        assert_eq!(n.get_root(), root);
    }).await;

    // Check active index
    storage_account(None, &mut test, |s: &StorageAccount| {
        assert_eq!(s.get_trees_count(), 1);
        assert_eq!(s.get_next_commitment_ptr(), 0);
        assert_eq!(s.get_mt_roots_count(), 0);
    }).await;

    // Too big batch will also allow for closing of MT
    test.set_pda_account::<StorageAccount, _>(None, |data| {
        let mut storage_account = StorageAccount::new(data, HashMap::new()).unwrap();
        storage_account.set_next_commitment_ptr(&(MT_COMMITMENT_COUNT as u32 - 1));
    }).await;
    set_single_pda_account!(CommitmentQueueAccount, None, test, |account: &mut CommitmentQueueAccount| {
        let mut queue = CommitmentQueue::new(account);
        queue.enqueue(CommitmentHashRequest { commitment: [0; 32], min_batching_rate: 1, fee_version: 0 }).unwrap();
        queue.enqueue(CommitmentHashRequest { commitment: [0; 32], min_batching_rate: 1, fee_version: 0 }).unwrap();
    });

    // Failure because first storage account (containing root) is missing
    test.ix_should_fail_simple(
        ElusivInstruction::reset_active_merkle_tree_instruction(1, &[])
    ).await;

    test.ix_should_succeed_simple(
        ElusivInstruction::reset_active_merkle_tree_instruction(1, &storage_accounts)
    ).await;
}

#[tokio::test]
async fn test_global_sub_account_duplicates() {
    let mut test = ElusivProgramTest::start().await;
    test.setup_initial_pdas().await;

    // Open storage account
    test.ix_should_succeed_simple(
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::StorageAccount,
            WritableSignerAccount(test.payer()),
            WritableUserAccount(StorageAccount::find(None).0)
        )
    ).await;

    fn open_mt(mt_index: u32, pk: Pubkey) -> Instruction {
        ElusivInstruction::open_multi_instance_account_instruction(
            MultiInstancePDAAccountKind::NullifierAccount,
            mt_index,
            WritableSignerAccount(pk),
            WritableUserAccount(NullifierAccount::find(Some(mt_index)).0)
        )
    }

    // Open two MTs
    test.ix_should_succeed_simple(open_mt(0, test.payer())).await;
    test.ix_should_succeed_simple(open_mt(1, test.payer())).await;

    // Setting in first MT should succeed
    let account = test.create_program_account_rent_exempt(NullifierAccount::ACCOUNT_SIZE).await;
    test.ix_should_succeed_simple(
        ElusivInstruction::enable_nullifier_sub_account_instruction(
            0,
            0,
            WritableUserAccount(account.pubkey()),
        )
    ).await;

    // Setting twice at same index
    test.ix_should_fail_simple(
        ElusivInstruction::enable_nullifier_sub_account_instruction(
            0,
            0,
            WritableUserAccount(account.pubkey()),
        )
    ).await;

    // Setting twice in same account (different index)
    test.ix_should_fail_simple(
        ElusivInstruction::enable_nullifier_sub_account_instruction(
            0,
            1,
            WritableUserAccount(account.pubkey()),
        )
    ).await;

    // Setting in different account
    test.ix_should_fail_simple(
        ElusivInstruction::enable_nullifier_sub_account_instruction(
            1,
            0,
            WritableUserAccount(account.pubkey()),
        )
    ).await;

    // Setting in storage-account
    test.ix_should_fail_simple(
        ElusivInstruction::enable_storage_sub_account_instruction(
            0,
            WritableUserAccount(account.pubkey()),
        )
    ).await;

    // Setting a different account at same index should fail
    let account2 = test.create_program_account_rent_exempt(NullifierAccount::ACCOUNT_SIZE).await;
    test.ix_should_fail_simple(
        ElusivInstruction::enable_nullifier_sub_account_instruction(
            0,
            0,
            WritableUserAccount(account2.pubkey()),
        )
    ).await;

    // Manipulate map size
    let mut data = vec![1; NullifierAccount::ACCOUNT_SIZE];
    data[0] = 0;
    let lamports = test.lamports(&account2.pubkey()).await;
    test.set_program_account(&account2.pubkey(), &data, lamports,).await;

    // Setting a different account at a different index should succeed
    test.ix_should_succeed_simple(
        ElusivInstruction::enable_nullifier_sub_account_instruction(
            0,
            1,
            WritableUserAccount(account2.pubkey()),
        )
    ).await;

    // Check map size
    let data = test.data(&account2.pubkey()).await;
    assert_eq!(data[0], 1);
    assert_eq!(&data[1..5], &[0,0,0,0]);
}

#[tokio::test]
async fn test_enable_precomputes_subaccounts() {
    let mut test = ElusivProgramTest::start().await;
    test.setup_initial_pdas().await;

    // Open storage account
    test.ix_should_succeed_simple(
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::StorageAccount,
            WritableSignerAccount(test.payer()),
            WritableUserAccount(StorageAccount::find(None).0)
        )
    ).await;

    // Invalid size
    let size = precompute_account_size2(0);
    let account = test.create_program_account_rent_exempt(size).await;
    test.ix_should_fail_simple(
        ElusivInstruction::enable_precompute_sub_account_instruction(0, WritableUserAccount(account.pubkey()))
    ).await;

    // Subaccount already in use
    let size = precompute_account_size2(0) + SUB_ACCOUNT_ADDITIONAL_SIZE;
    let account = test.create_program_account_rent_exempt(size).await;
    let mut data = vec![1];
    data.extend(vec![0; precompute_account_size2(0)]);
    let lamports = test.lamports(&account.pubkey()).await;
    test.set_program_account(&account.pubkey(), &data, lamports).await;
    test.ix_should_fail_simple(
        ElusivInstruction::enable_precompute_sub_account_instruction(0, WritableUserAccount(account.pubkey()))
    ).await;

    // Success
    let account = test.create_program_account_rent_exempt(size).await;
    test.ix_should_succeed_simple(
        ElusivInstruction::enable_precompute_sub_account_instruction(0, WritableUserAccount(account.pubkey()))
    ).await;

    let mut data = test.data(&PrecomputesAccount::find(None).0).await;
    let precomputes = PrecomputesAccount::new(&mut data, HashMap::new()).unwrap();
    let pubkeys = precomputes.get_multi_account_data().pubkeys;
    assert_eq!(pubkeys[0].option().unwrap(), account.pubkey());
    assert!(pubkeys[1].option().is_none());

    // Index already set
    let account = test.create_program_account_rent_exempt(size).await;
    test.ix_should_fail_simple(
        ElusivInstruction::enable_precompute_sub_account_instruction(0, WritableUserAccount(account.pubkey()))
    ).await;
}

async fn precompute_test() -> (ElusivProgramTest, Vec<Pubkey>) {
    let mut test = ElusivProgramTest::start().await;
    test.setup_initial_pdas().await;

    // Enable sub accounts
    let mut ixs = Vec::new();
    let mut pubkeys = Vec::new();
    for i in 0..VKEY_COUNT {
        let size = precompute_account_size2(i) + SUB_ACCOUNT_ADDITIONAL_SIZE;
        let account = test.create_program_account_rent_exempt(size).await;
        pubkeys.push(account.pubkey());
        ixs.push(
            ElusivInstruction::enable_precompute_sub_account_instruction(
                i as u32,
                WritableUserAccount(account.pubkey())
            )
        );
    }
    test.tx_should_succeed_simple(&ixs).await;

    (test, pubkeys)
}

#[tokio::test]
#[ignore]
async fn test_precompute_full() {
    // Setup requires multiple thousand tx atm -> no CI integration test possible -> ignore (we use test_precompute_partial instead and unit tests)
    let (mut test, pubkeys) = precompute_test().await;
    let precompute_accounts: Vec<WritableUserAccount> = pubkeys.iter().map(|p| WritableUserAccount(*p)).collect();

    // Init precomputing
    let ixs = [
        request_compute_units(1_400_000),
        ElusivInstruction::precompute_v_keys_instruction(&precompute_accounts),
    ];
    
    for _ in 0..SendQuadraVKey::PUBLIC_INPUTS_COUNT {
        // Init public input
        test.tx_should_succeed_simple(&ixs.clone()).await;

        for _ in 0..32 {
            // Tuples
            test.tx_should_succeed_simple(&ixs.clone()).await;

            // Quads
            test.tx_should_succeed_simple(&ixs.clone()).await;
            test.tx_should_succeed_simple(&ixs.clone()).await;
            
            // Octs
            let txs = batch_instructions(
                15 * 15,
                120_000,
                ElusivInstruction::precompute_v_keys_instruction(&precompute_accounts)
            );
            for tx in txs {
                test.tx_should_succeed_simple(&tx).await;
            }
        }
    }
}

#[tokio::test]
async fn test_precompute_partial() {
    let (mut test, pubkeys) = precompute_test().await;
    let precompute_accounts: Vec<WritableUserAccount> = pubkeys.iter().map(|p| WritableUserAccount(*p)).collect();
    let ixs = [
        request_compute_units(1_400_000),
        ElusivInstruction::precompute_v_keys_instruction(&precompute_accounts),
    ];
    test.tx_should_succeed_simple(&ixs.clone()).await;

    // Precompute the first two bytes of the first public input 
    for _ in 0..2 {
        // Tuples
        test.tx_should_succeed_simple(&ixs.clone()).await;

        // Quads
        test.tx_should_succeed_simple(&ixs.clone()).await;
        test.tx_should_succeed_simple(&ixs.clone()).await;
        
        // Octs
        let txs = batch_instructions(
            15 * 15,
            120_000,
            ElusivInstruction::precompute_v_keys_instruction(&precompute_accounts)
        );
        for tx in txs {
            test.tx_should_succeed_simple(&tx).await;
        }
    }

    let expected = 1 + 2 * (3 + 15 * 15);
    let mut data = vec![0; precompute_account_size2(0)];
    let p = VirtualPrecomputes::<SendQuadraVKey>::new(&mut data);
    fn cmp<VKey: VerificationKey, A: PrecomutedValues<VKey>, B: PrecomutedValues<VKey>>(a: &A, b: &B) {
        assert_eq!(a.point(0, 0, 1), b.point(0, 0, 1));
        assert_eq!(a.point(0, 0, 2), b.point(0, 0, 2));
    }
    precomputes_account(None, &mut test, |precomputes| {
        assert_eq!(precomputes.get_instruction(), expected);
        cmp(&p, precomputes);
    }).await;
}