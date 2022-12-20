//! Tests the account setup process

mod common;

use borsh::BorshSerialize;
use common::*;
use elusiv::proof::vkey::VKeyAccountManangerAccount;
use elusiv::state::program_account::PDAOffset;
use elusiv::state::queue::RingQueue;
use elusiv::state::{StorageAccount, MT_COMMITMENT_COUNT, StorageChildAccount, NullifierChildAccount};
use elusiv::commitment::CommitmentHashingAccount;
use elusiv::instruction::*;
use elusiv::processor::{SingleInstancePDAAccountKind, MultiInstancePDAAccountKind, CommitmentHashRequest};
use elusiv::token::SPL_TOKEN_COUNT;
use elusiv_types::ChildAccount;
use solana_program::instruction::{Instruction, AccountMeta};
use solana_program::pubkey::Pubkey;
use solana_program_test::*;
use elusiv::state::{
    queue::{CommitmentQueue, CommitmentQueueAccount, Queue},
    program_account::{PDAAccount, SizedAccount, ProgramAccount, PDAAccountData},
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
    }

    assert_account::<GovernorAccount>(&mut test, None).await;
    assert_account::<PoolAccount>(&mut test, None).await;
    assert_account::<FeeCollectorAccount>(&mut test, None).await;
    assert_account::<VKeyAccountManangerAccount>(&mut test, None).await;
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

    assert_eq!(keys, test.storage_accounts().await);
}

#[tokio::test]
async fn test_setup_storage_account_duplicate() {
    let mut test = ElusivProgramTest::start().await;
    test.setup_storage_account().await;

    // Cannot set a child-account twice
    let k = test.create_program_account_rent_exempt(StorageChildAccount::SIZE).await;
    test.ix_should_fail_simple(
        ElusivInstruction::enable_storage_child_account_instruction(1, WritableUserAccount(k.pubkey()))
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
        assert_eq!(keys, test.nullifier_accounts(mt_index).await);
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

    // Cannot set child-account twice
    let k = test.create_program_account_rent_exempt(NullifierChildAccount::SIZE).await;
    test.ix_should_fail_simple(
        ElusivInstruction::enable_nullifier_child_account_instruction(
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
        let mut storage_account = StorageAccount::new(data).unwrap();
        storage_account.set_next_commitment_ptr(&(MT_COMMITMENT_COUNT as u32));
    }).await;

    // Override the root
    let root = [1; 32];
    let mut data = test.data(&root_storage_account).await;
    {
        let (_, inner_data) = StorageChildAccount::split_data_mut(&mut data).unwrap();
        inner_data[..32].copy_from_slice(&root[..32]);
    }
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
        let mut storage_account = StorageAccount::new(data).unwrap();
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
    let account = test.create_program_account_rent_exempt(NullifierChildAccount::SIZE).await;
    test.ix_should_succeed_simple(
        ElusivInstruction::enable_nullifier_child_account_instruction(
            0,
            0,
            WritableUserAccount(account.pubkey()),
        )
    ).await;

    // Setting twice at same index
    test.ix_should_fail_simple(
        ElusivInstruction::enable_nullifier_child_account_instruction(
            0,
            0,
            WritableUserAccount(account.pubkey()),
        )
    ).await;

    // Setting twice in same account (different index)
    test.ix_should_fail_simple(
        ElusivInstruction::enable_nullifier_child_account_instruction(
            0,
            1,
            WritableUserAccount(account.pubkey()),
        )
    ).await;

    // Setting in different account
    test.ix_should_fail_simple(
        ElusivInstruction::enable_nullifier_child_account_instruction(
            1,
            0,
            WritableUserAccount(account.pubkey()),
        )
    ).await;

    // Setting in storage-account
    test.ix_should_fail_simple(
        ElusivInstruction::enable_storage_child_account_instruction(
            0,
            WritableUserAccount(account.pubkey()),
        )
    ).await;

    // Setting a different account at same index should fail
    let account2 = test.create_program_account_rent_exempt(NullifierChildAccount::SIZE).await;
    test.ix_should_fail_simple(
        ElusivInstruction::enable_nullifier_child_account_instruction(
            0,
            0,
            WritableUserAccount(account2.pubkey()),
        )
    ).await;

    // Manipulate map size
    let mut data = vec![1; NullifierChildAccount::SIZE];
    data[0] = 0;
    let lamports = test.lamports(&account2.pubkey()).await;
    test.set_program_account(&account2.pubkey(), &data, lamports,).await;

    // Setting a different account at a different index should succeed
    test.ix_should_succeed_simple(
        ElusivInstruction::enable_nullifier_child_account_instruction(
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