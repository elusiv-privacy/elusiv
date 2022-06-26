//! Tests the account setup process

#[cfg(not(tarpaulin_include))]
mod common;
use std::collections::HashMap;

use borsh::BorshSerialize;
use common::*;
use common::program_setup::*;

use elusiv::state::queue::{RingQueue, BaseCommitmentQueueAccount};
use elusiv::state::{StorageAccount, MT_COMMITMENT_COUNT, EMPTY_TREE};
use elusiv::commitment::CommitmentHashingAccount;
use elusiv::instruction::*;
use elusiv::processor::{SingleInstancePDAAccountKind, MultiInstancePDAAccountKind, CommitmentHashRequest};
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
            assert!(get_balance(<$ty>::find($offset).0, &mut $context).await > 0);
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
async fn test_setup_pda_accounts() {
    let mut context = start_program_solana_program_test().await;
    setup_pda_accounts(&mut context).await;

    assert_account!(GovernorAccount, context, None);
    assert_account!(FeeAccount, context, Some(0));
    assert_account!(PoolAccount, context, None);
    assert_account!(FeeCollectorAccount, context, None);

    assert_account!(CommitmentHashingAccount, context, None);
    assert_account!(CommitmentQueueAccount, context, None);
    assert_account!(BaseCommitmentQueueAccount, context, Some(0));
}

#[tokio::test]
#[should_panic]
async fn test_setup_pda_accounts_duplicate() {
    let mut context = start_program_solana_program_test().await;
    setup_pda_accounts(&mut context).await;
    setup_pda_accounts(&mut context).await;
}

#[tokio::test]
async fn test_setup_fee_account() {
    let mut context = start_program_solana_program_test().await;
    let mut payer = Actor::new(&mut context).await;

    ix_should_succeed(
        ElusivInstruction::setup_governor_account_instruction(
            SignerAccount(payer.pubkey),
            WritableUserAccount(GovernorAccount::find(None).0)
        ), &mut payer, &mut context
    ).await;

    let genesis_fee = genesis_fee(lamports_per_signature(&mut context).await);
    setup_fee_account(&mut context).await;

    // Second time will fail
    ix_should_fail(
        ElusivInstruction::init_new_fee_version_instruction(0, genesis_fee.clone(), SignerAccount(payer.pubkey)),
        &mut payer, &mut context
    ).await;
    
    pda_account!(fee, FeeAccount, Some(0), context);
    assert_eq!(fee.get_program_fee(), genesis_fee);

    // Attempting to set a version higher than genesis (0) will fail
    ix_should_fail(
        ElusivInstruction::init_new_fee_version_instruction(1, genesis_fee.clone(), SignerAccount(payer.pubkey)),
        &mut payer, &mut context
    ).await;

    // But after governor allows it, fee_version 1 can be set
    set_single_pda_account!(GovernorAccount, &mut context, None, |account: &mut GovernorAccount| {
        account.set_fee_version(&1);
    });

    ix_should_succeed(
        ElusivInstruction::init_new_fee_version_instruction(1, genesis_fee, SignerAccount(payer.pubkey)),
        &mut payer, &mut context
    ).await;
}

#[tokio::test]
async fn test_setup_pda_accounts_invalid_pda() {
    let mut context = start_program_solana_program_test().await;
    let mut payer = Actor::new(&mut context).await;

    ix_should_fail(
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::CommitmentQueueAccount,
            SignerAccount(payer.pubkey),
            WritableUserAccount(BaseCommitmentQueueAccount::find(None).0)
        ),
        &mut payer, &mut context
    ).await;
}

#[tokio::test]
async fn test_setup_storage_account() {
    let mut context = start_program_solana_program_test().await;
    let keys = setup_storage_account(&mut context).await;

    storage_account(&mut context, |storage_account| {
        let pks: Vec<Pubkey> = storage_account.get_multi_account_data().pubkeys.iter().map(|p| p.option().unwrap()).collect();
        assert_eq!(keys, pks);
    }).await;
}

#[tokio::test]
async fn test_setup_storage_account_duplicate() {
    let mut context = start_program_solana_program_test().await;
    setup_storage_account(&mut context).await;
    let mut client = Actor::new(&mut context).await;

    // Cannot set a sub-account twice
    let k = create_account(&mut context).await;
    tx_should_fail(&[
        ElusivInstruction::enable_storage_sub_account_instruction(1, UserAccount(k.pubkey()))
    ], &mut client, &mut context).await;

    // Cannot init storage PDA twice
    tx_should_fail(&[
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::StorageAccount,
            SignerAccount(client.pubkey),
            WritableUserAccount(StorageAccount::find(None).0),
        )
    ], &mut client, &mut context).await;
}

#[tokio::test]
async fn test_open_new_merkle_tree() {
    let mut context = start_program_solana_program_test().await;

    // Multiple MTs can be opened
    for mt_index in 0..3 {
        let keys = create_merkle_tree(&mut context, mt_index).await;

        nullifier_account(mt_index, &mut context, |nullfier_account: &NullifierAccount| {
            let pks: Vec<Pubkey> = nullfier_account.get_multi_account_data().pubkeys.iter().map(|p| p.option().unwrap()).collect();
            assert_eq!(keys, pks);
        }).await;
    }
}

#[tokio::test]
async fn test_open_new_merkle_tree_duplicate() {
    let mut context = start_program_solana_program_test().await;
    let mut client = Actor::new(&mut context).await;
    create_merkle_tree(&mut context, 0).await;

    // Cannot init MT twice
    tx_should_fail(&[
        ElusivInstruction::open_multi_instance_account_instruction(
            MultiInstancePDAAccountKind::NullifierAccount,
            0,
            SignerAccount(client.pubkey),
            WritableUserAccount(StorageAccount::find(Some(0)).0),
        )
    ], &mut client, &mut context).await;

    // Cannot set sub-account twice
    let k = create_account(&mut context).await;
    tx_should_fail(&[
        ElusivInstruction::enable_nullifier_sub_account_instruction(0, 1, UserAccount(k.pubkey()))
    ], &mut client, &mut context).await;
}

#[tokio::test]
async fn test_close_merkle_tree() {
    let mut context = start_program_solana_program_test().await;
    let mut client = Actor::new(&mut context).await;
    setup_pda_accounts(&mut context).await;
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

    nullifier_account(0, &mut context, |n: &NullifierAccount| {
        assert_eq!(n.get_root(), EMPTY_TREE[MT_HEIGHT as usize]);
    }).await;

    // Check active index
    storage_account(&mut context, |s: &StorageAccount| {
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