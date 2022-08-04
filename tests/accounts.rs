//! Tests the account setup process

#[cfg(not(tarpaulin_include))]
mod common;
use std::collections::HashMap;

use borsh::BorshSerialize;
use common::*;
use common::program_setup::*;

use elusiv::proof::precompute::{precompute_account_size2, VKEY_COUNT, PrecomputesAccount, VirtualPrecomputes, PrecomutedValues};
use elusiv::proof::vkey::{SendQuadraVKey, VerificationKey};
use elusiv::state::program_account::{MultiAccountAccount, SUB_ACCOUNT_ADDITIONAL_SIZE};
use elusiv::state::queue::{RingQueue, BaseCommitmentQueueAccount};
use elusiv::state::{StorageAccount, MT_COMMITMENT_COUNT, EMPTY_TREE};
use elusiv::commitment::CommitmentHashingAccount;
use elusiv::instruction::*;
use elusiv::processor::{SingleInstancePDAAccountKind, MultiInstancePDAAccountKind, CommitmentHashRequest};
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
    MT_HEIGHT,
};
use solana_sdk::signer::Signer;

macro_rules! assert_account {
    ($ty: ty, $context: expr, $offset: expr) => {
        {
            let data = get_data(&mut $context, <$ty>::find($offset).0).await;

            // Check balance and data size
            assert!(get_balance(&<$ty>::find($offset).0, &mut $context).await > 0);
            assert_eq!(data.len(), <$ty>::SIZE);

            // Check pda account fields
            let data = PDAAccountData::new(&data).unwrap();
            assert_eq!(data.bump_seed, <$ty>::find($offset).1);
            assert_eq!(data.version, 0);
            assert_eq!(data.initialized, false);
        }
    };
}

#[tokio::test]
#[ignore]
async fn test_setup_initial_accounts() {
    let mut context = start_program_solana_program_test().await;
    setup_initial_accounts(&mut context).await;

    assert_account!(GovernorAccount, context, None);
    assert_account!(FeeAccount, context, Some(0));
    assert_account!(PoolAccount, context, None);
    assert_account!(FeeCollectorAccount, context, None);
    assert_account!(PrecomputesAccount, context, None);

    assert_account!(CommitmentHashingAccount, context, None);
    assert_account!(CommitmentQueueAccount, context, None);
    assert_account!(BaseCommitmentQueueAccount, context, Some(0));
}

#[tokio::test]
#[should_panic]
#[ignore]
async fn test_setup_initial_accounts_duplicate() {
    let mut context = start_program_solana_program_test().await;
    setup_initial_accounts(&mut context).await;
    setup_initial_accounts(&mut context).await;
}

#[tokio::test]
#[ignore]
async fn test_setup_fee_account() {
    let mut context = start_program_solana_program_test().await;
    let mut payer = Actor::new(&mut context).await;

    ix_should_succeed(
        ElusivInstruction::setup_governor_account_instruction(
            WritableSignerAccount(payer.pubkey),
            WritableUserAccount(GovernorAccount::find(None).0)
        ), &mut payer, &mut context
    ).await;

    let genesis_fee = genesis_fee(lamports_per_signature(&mut context).await);
    setup_fee_account(&mut context).await;

    // Second time will fail
    ix_should_fail(
        ElusivInstruction::init_new_fee_version_instruction(0, genesis_fee.clone(), WritableSignerAccount(payer.pubkey)),
        &mut payer, &mut context
    ).await;
    
    pda_account!(fee, FeeAccount, Some(0), context);
    assert_eq!(fee.get_program_fee(), genesis_fee);

    pda_account!(governor, GovernorAccount, None, context);
    assert_eq!(governor.get_program_fee(), genesis_fee);

    // Attempting to set a version higher than genesis (0) will fail
    ix_should_fail(
        ElusivInstruction::init_new_fee_version_instruction(1, genesis_fee.clone(), WritableSignerAccount(payer.pubkey)),
        &mut payer, &mut context
    ).await;

    // But after governor allows it, fee_version 1 can be set
    set_single_pda_account!(GovernorAccount, &mut context, None, |account: &mut GovernorAccount| {
        account.set_fee_version(&1);
    });

    ix_should_succeed(
        ElusivInstruction::init_new_fee_version_instruction(1, genesis_fee, WritableSignerAccount(payer.pubkey)),
        &mut payer, &mut context
    ).await;
}

#[tokio::test]
#[ignore]
async fn test_setup_pda_accounts_invalid_pda() {
    let mut context = start_program_solana_program_test().await;
    let mut payer = Actor::new(&mut context).await;

    ix_should_fail(
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::CommitmentQueueAccount,
            WritableSignerAccount(payer.pubkey),
            WritableUserAccount(BaseCommitmentQueueAccount::find(None).0)
        ),
        &mut payer, &mut context
    ).await;
}

#[tokio::test]
#[ignore]
async fn test_setup_storage_account() {
    let mut context = start_program_solana_program_test().await;
    let keys = setup_storage_account(&mut context).await;

    storage_account(&mut context, None, |storage_account| {
        let pks: Vec<Pubkey> = storage_account.get_multi_account_data().pubkeys.iter().map(|p| p.option().unwrap()).collect();
        assert_eq!(keys, pks);
    }).await;
}

#[tokio::test]
#[ignore]
async fn test_setup_storage_account_duplicate() {
    let mut context = start_program_solana_program_test().await;
    setup_storage_account(&mut context).await;
    let mut client = Actor::new(&mut context).await;

    // Cannot set a sub-account twice
    let k = create_account(&mut context).await;
    tx_should_fail(&[
        ElusivInstruction::enable_storage_sub_account_instruction(1, WritableUserAccount(k.pubkey()))
    ], &mut client, &mut context).await;

    // Cannot init storage PDA twice
    tx_should_fail(&[
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::StorageAccount,
            WritableSignerAccount(client.pubkey),
            WritableUserAccount(StorageAccount::find(None).0),
        )
    ], &mut client, &mut context).await;
}

#[tokio::test]
#[ignore]
async fn test_open_new_merkle_tree() {
    let mut context = start_program_solana_program_test().await;

    // Multiple MTs can be opened
    for mt_index in 0..3 {
        let keys = create_merkle_tree(&mut context, mt_index).await;

        nullifier_account(&mut context, Some(mt_index), |nullfier_account: &NullifierAccount| {
            let pks: Vec<Pubkey> = nullfier_account.get_multi_account_data().pubkeys.iter().map(|p| p.option().unwrap()).collect();
            assert_eq!(keys, pks);
        }).await;
    }
}

#[tokio::test]
#[ignore]
async fn test_open_new_merkle_tree_duplicate() {
    let mut context = start_program_solana_program_test().await;
    let mut client = Actor::new(&mut context).await;
    create_merkle_tree(&mut context, 0).await;

    // Cannot init MT twice
    tx_should_fail(&[
        ElusivInstruction::open_multi_instance_account_instruction(
            MultiInstancePDAAccountKind::NullifierAccount,
            0,
            WritableSignerAccount(client.pubkey),
            WritableUserAccount(StorageAccount::find(Some(0)).0),
        )
    ], &mut client, &mut context).await;

    // Cannot set sub-account twice
    let k = create_account(&mut context).await;
    tx_should_fail(&[
        ElusivInstruction::enable_nullifier_sub_account_instruction(0, 1, WritableUserAccount(k.pubkey()))
    ], &mut client, &mut context).await;
}

#[tokio::test]
#[ignore]
async fn test_close_merkle_tree() {
    let mut context = start_program_solana_program_test().await;
    let mut client = Actor::new(&mut context).await;
    setup_initial_accounts(&mut context).await;
    setup_storage_account(&mut context).await;

    create_merkle_tree(&mut context, 0).await;
    create_merkle_tree(&mut context, 1).await;

    // Failure since active MT is not full
    ix_should_fail(
        ElusivInstruction::reset_active_merkle_tree_instruction(0, &[], &[]),
        &mut client, &mut context
    ).await;

    // Set active MT as full
    set_pda_account::<StorageAccount, _>(&mut context, None, |data| {
        let mut storage_account = StorageAccount::new(data, HashMap::new()).unwrap();
        storage_account.set_next_commitment_ptr(&(MT_COMMITMENT_COUNT as u32));
    }).await;

    // Failure since active_nullifier_account is invalid
    ix_should_fail(
        Instruction::new_with_bytes(
            elusiv::id(),
            &ElusivInstruction::ResetActiveMerkleTree { active_mt_index: 0 }.try_to_vec().unwrap()[..],
            vec![
                AccountMeta::new(StorageAccount::find(None).0, false),
                AccountMeta::new(CommitmentQueueAccount::find(None).0, false),
                AccountMeta::new(NullifierAccount::find(Some(1)).0, false),
            ]
        ),
        &mut client, &mut context
    ).await;

    // Success
    ix_should_succeed(
        Instruction::new_with_bytes(
            elusiv::id(),
            &ElusivInstruction::ResetActiveMerkleTree { active_mt_index: 0 }.try_to_vec().unwrap()[..],
            vec![
                AccountMeta::new(StorageAccount::find(None).0, false),
                AccountMeta::new(CommitmentQueueAccount::find(None).0, false),
                AccountMeta::new(NullifierAccount::find(Some(0)).0, false),
            ]
        ),
        &mut client, &mut context
    ).await;

    nullifier_account(&mut context, Some(0), |n: &NullifierAccount| {
        assert_eq!(n.get_root(), EMPTY_TREE[MT_HEIGHT as usize]);
    }).await;

    // Check active index
    storage_account(&mut context, None, |s: &StorageAccount| {
        assert_eq!(s.get_trees_count(), 1);
        assert_eq!(s.get_next_commitment_ptr(), 0);
        assert_eq!(s.get_mt_roots_count(), 0);
    }).await;

    // Too big batch will also allow for closing of MT
    set_pda_account::<StorageAccount, _>(&mut context, None, |data| {
        let mut storage_account = StorageAccount::new(data, HashMap::new()).unwrap();
        storage_account.set_next_commitment_ptr(&(MT_COMMITMENT_COUNT as u32 - 1));
    }).await;
    set_single_pda_account!(CommitmentQueueAccount, &mut context, None, |account: &mut CommitmentQueueAccount| {
        let mut queue = CommitmentQueue::new(account);
        queue.enqueue(CommitmentHashRequest { commitment: [0; 32], min_batching_rate: 1, fee_version: 0 }).unwrap();
        queue.enqueue(CommitmentHashRequest { commitment: [0; 32], min_batching_rate: 1, fee_version: 0 }).unwrap();
    });

    ix_should_succeed(
        ElusivInstruction::reset_active_merkle_tree_instruction(1, &[], &[]),
        &mut client, &mut context
    ).await;
}

#[tokio::test]
#[ignore]
async fn test_global_sub_account_duplicates() {
    let mut context = start_program_solana_program_test().await;
    let mut client = Actor::new(&mut context).await;
    setup_initial_accounts(&mut context).await;

    // Open storage account
    ix_should_succeed(
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::StorageAccount,
            WritableSignerAccount(client.pubkey),
            WritableUserAccount(StorageAccount::find(None).0)
        ), &mut client, &mut context
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
    ix_should_succeed(open_mt(0, client.pubkey), &mut client, &mut context).await;
    ix_should_succeed(open_mt(1, client.pubkey), &mut client, &mut context).await;

    // Setting in first MT should succeed
    let account = create_account_rent_exempt(&mut context, NullifierAccount::ACCOUNT_SIZE).await;
    ix_should_succeed(
        ElusivInstruction::enable_nullifier_sub_account_instruction(
            0,
            0,
            WritableUserAccount(account.pubkey()),
        ), &mut client, &mut context
    ).await;

    // Setting twice at same index
    ix_should_fail(
        ElusivInstruction::enable_nullifier_sub_account_instruction(
            0,
            0,
            WritableUserAccount(account.pubkey()),
        ), &mut client, &mut context
    ).await;

    // Setting twice in same account (different index)
    ix_should_fail(
        ElusivInstruction::enable_nullifier_sub_account_instruction(
            0,
            1,
            WritableUserAccount(account.pubkey()),
        ), &mut client, &mut context
    ).await;

    // Setting in different account
    ix_should_fail(
        ElusivInstruction::enable_nullifier_sub_account_instruction(
            1,
            0,
            WritableUserAccount(account.pubkey()),
        ), &mut client, &mut context
    ).await;

    // Setting in storage-account
    ix_should_fail(
        ElusivInstruction::enable_storage_sub_account_instruction(
            0,
            WritableUserAccount(account.pubkey()),
        ), &mut client, &mut context
    ).await;

    // Setting a different account at same index should fail
    let account2 = create_account_rent_exempt(&mut context, NullifierAccount::ACCOUNT_SIZE).await;
    ix_should_fail(
        ElusivInstruction::enable_nullifier_sub_account_instruction(
            0,
            0,
            WritableUserAccount(account2.pubkey()),
        ), &mut client, &mut context
    ).await;

    // Manipulate map size
    let mut data = vec![1; NullifierAccount::ACCOUNT_SIZE];
    data[0] = 0;
    let lamports = get_balance(&account2.pubkey(), &mut context).await;
    set_account(&mut context, &account2.pubkey(), data, lamports).await;

    // Setting a different account at a different index should succeed
    ix_should_succeed(
        ElusivInstruction::enable_nullifier_sub_account_instruction(
            0,
            1,
            WritableUserAccount(account2.pubkey()),
        ), &mut client, &mut context
    ).await;

    // Check map size
    let data = get_data(&mut context, account2.pubkey()).await;
    assert_eq!(data[0], 1);
    assert_eq!(&data[1..5], &[0,0,0,0]);
}

#[tokio::test]
async fn test_enable_precomputes_subaccounts() {
    let mut context = start_program_solana_program_test().await;
    let mut client = Actor::new(&mut context).await;
    setup_initial_accounts(&mut context).await;

    // Open storage account
    ix_should_succeed(
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::StorageAccount,
            WritableSignerAccount(client.pubkey),
            WritableUserAccount(StorageAccount::find(None).0)
        ), &mut client, &mut context
    ).await;

    // Invalid size
    let size = precompute_account_size2(0);
    let account = create_account_rent_exempt(&mut context, size).await;
    ix_should_fail(
        ElusivInstruction::enable_precompute_sub_account_instruction(0, WritableUserAccount(account.pubkey())),
        &mut client, &mut context
    ).await;

    // Subaccount already in use
    let size = precompute_account_size2(0) + SUB_ACCOUNT_ADDITIONAL_SIZE;
    let account = create_account_rent_exempt(&mut context, size).await;
    let mut data = vec![1];
    data.extend(vec![0; precompute_account_size2(0)]);
    let lamports = get_balance(&account.pubkey(), &mut context).await;
    set_account(&mut context, &account.pubkey(), data, lamports).await;
    ix_should_fail(
        ElusivInstruction::enable_precompute_sub_account_instruction(0, WritableUserAccount(account.pubkey())),
        &mut client, &mut context
    ).await;

    // Success
    let account = create_account_rent_exempt(&mut context, size).await;
    ix_should_succeed(
        ElusivInstruction::enable_precompute_sub_account_instruction(0, WritableUserAccount(account.pubkey())),
        &mut client, &mut context
    ).await;

    let mut data = get_data(&mut context, PrecomputesAccount::find(None).0).await;
    let precomputes = PrecomputesAccount::new(&mut data, HashMap::new()).unwrap();
    let pubkeys = precomputes.get_multi_account_data().pubkeys;
    assert_eq!(pubkeys[0].option().unwrap(), account.pubkey());
    assert!(pubkeys[1].option().is_none());

    // Index already set
    let account = create_account_rent_exempt(&mut context, size).await;
    ix_should_fail(
        ElusivInstruction::enable_precompute_sub_account_instruction(0, WritableUserAccount(account.pubkey())),
        &mut client, &mut context
    ).await;
}

async fn precompute_test() -> (ProgramTestContext, Actor, Vec<Pubkey>) {
    let mut context = start_program_solana_program_test().await;
    let mut client = Actor::new(&mut context).await;
    setup_initial_accounts(&mut context).await;

    // Enable sub accounts
    let mut ixs = Vec::new();
    let mut pubkeys = Vec::new();
    for i in 0..VKEY_COUNT {
        let size = precompute_account_size2(i) + SUB_ACCOUNT_ADDITIONAL_SIZE;
        let account = create_account_rent_exempt(&mut context, size).await;
        pubkeys.push(account.pubkey());
        ixs.push(
            ElusivInstruction::enable_precompute_sub_account_instruction(
                i as u32,
                WritableUserAccount(account.pubkey())
            )
        );
    }
    tx_should_succeed(&ixs, &mut client, &mut context).await;

    (context, client, pubkeys)
}

#[tokio::test]
#[ignore]
async fn test_precompute_full() {
    // Setup requires multiple thousand tx atm -> no CI integration test possible -> ignore (we use test_precompute_partial instead and unit tests)
    let (mut context, mut client, pubkeys) = precompute_test().await;
    let precompute_accounts: Vec<WritableUserAccount> = pubkeys.iter().map(|p| WritableUserAccount(*p)).collect();

    // Init precomputing
    let ixs = [
        request_compute_units(1_400_000),
        ElusivInstruction::precompute_v_keys_instruction(&precompute_accounts),
    ];
    
    for _ in 0..SendQuadraVKey::PUBLIC_INPUTS_COUNT {
        // Init public input
        tx_should_succeed(&ixs.clone(), &mut client, &mut context).await;

        for _ in 0..32 {
            // Tuples
            tx_should_succeed(&ixs.clone(), &mut client, &mut context).await;

            // Quads
            tx_should_succeed(&ixs.clone(), &mut client, &mut context).await;
            tx_should_succeed(&ixs.clone(), &mut client, &mut context).await;
            
            // Octs
            let txs = batch_instructions(
                15 * 15,
                120_000,
                ElusivInstruction::precompute_v_keys_instruction(&precompute_accounts)
            );
            for tx in txs {
                tx_should_succeed(&tx, &mut client, &mut context).await;
            }
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_precompute_partial() {
    let (mut context, mut client, pubkeys) = precompute_test().await;
    let precompute_accounts: Vec<WritableUserAccount> = pubkeys.iter().map(|p| WritableUserAccount(*p)).collect();
    let ixs = [
        request_compute_units(1_400_000),
        ElusivInstruction::precompute_v_keys_instruction(&precompute_accounts),
    ];
    tx_should_succeed(&ixs.clone(), &mut client, &mut context).await;

    // Precompute the first two bytes of the first public input 
    for _ in 0..2 {
        // Tuples
        tx_should_succeed(&ixs.clone(), &mut client, &mut context).await;

        // Quads
        tx_should_succeed(&ixs.clone(), &mut client, &mut context).await;
        tx_should_succeed(&ixs.clone(), &mut client, &mut context).await;
        
        // Octs
        let txs = batch_instructions(
            15 * 15,
            120_000,
            ElusivInstruction::precompute_v_keys_instruction(&precompute_accounts)
        );
        for tx in txs {
            tx_should_succeed(&tx, &mut client, &mut context).await;
        }
    }

    let expected = 1 + 2 * (3 + 15 * 15);
    let mut data = vec![0; precompute_account_size2(0)];
    let p = VirtualPrecomputes::<SendQuadraVKey>::new(&mut data);
    fn cmp<VKey: VerificationKey, A: PrecomutedValues<VKey>, B: PrecomutedValues<VKey>>(a: &A, b: &B) {
        assert_eq!(a.point(0, 0, 1), b.point(0, 0, 1));
        assert_eq!(a.point(0, 0, 2), b.point(0, 0, 2));
    }
    precomputes_account(&mut context, None, |precomputes| {
        assert_eq!(precomputes.get_instruction(), expected);
        cmp(&p, precomputes);
    }).await;
}