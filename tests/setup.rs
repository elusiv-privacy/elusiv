//! Tests the account setup process

#[cfg(not(tarpaulin_include))]
mod common;
use common::*;
use common::program_setup::*;

use elusiv::fee::FeeAccount;
use elusiv::state::StorageAccount;
use elusiv::commitment::CommitmentHashingAccount;
use elusiv::instruction::*;
use elusiv::processor::SingleInstancePDAAccountKind;
use elusiv::state::pool::PoolAccount;
use elusiv::state::program_account::{PDAAccount, SizedAccount, MultiAccountAccount, ProgramAccount};
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program::account_info::Account;
use solana_program_test::*;
use solana_sdk::signature::Keypair;
use solana_sdk::signature::Signer;
use elusiv::state::queue::{CommitmentQueueAccount, BaseCommitmentQueueAccount};

macro_rules! assert_account {
    ($ty: ty, $test_program: expr, $offset: expr) => {
        {
            let mut data = get_data(&mut $test_program, <$ty>::find($offset).0).await;

            // Check balance and data size
            assert!(get_balance(<$ty>::find($offset).0, &mut $test_program).await > 0);
            assert_eq!(data.len(), <$ty>::SIZE);

            // Check bump and initialized flag
            let account = <$ty>::new(&mut data).unwrap();
            assert_eq!(account.get_bump_seed(), <$ty>::find($offset).1);
            assert_eq!(account.get_initialized(), true);
        }
    };
}

#[tokio::test]
async fn test_setup_pda_accounts() {
    let mut test_program = start_program_solana_program_test().await;
    setup_pda_accounts(&mut test_program).await;

    assert_account!(CommitmentHashingAccount, test_program, None);
    assert_account!(CommitmentQueueAccount, test_program, None);
    assert_account!(BaseCommitmentQueueAccount, test_program, None);
    assert_account!(FeeAccount, test_program, Some(0));
}

#[tokio::test]
#[should_panic]
async fn test_setup_pda_accounts_duplicate() {
    let mut test_program = start_program_solana_program_test().await;
    setup_pda_accounts(&mut test_program).await;
    setup_pda_accounts(&mut test_program).await;
}

#[tokio::test]
async fn test_setup_pool_accounts() {
    let mut test_program = start_program_solana_program_test().await;
    let (sol_pool, fee_collector) = setup_pool_accounts(&mut test_program).await;
    
    pda_account!(pool, PoolAccount, None, test_program);
    assert_eq!(pool.get_sol_pool(), sol_pool.to_bytes());
    assert_eq!(pool.get_fee_collector(), fee_collector.to_bytes());
}

#[tokio::test]
#[should_panic]
async fn test_setup_pool_accounts_duplicate() {
    let mut test_program = start_program_solana_program_test().await;
    setup_pool_accounts(&mut test_program).await;
    setup_pool_accounts(&mut test_program).await;
}

#[tokio::test]
async fn test_setup_fee_account() {
    let mut test_program = start_program_solana_program_test().await;
    let mut payer = Actor::new(&mut test_program).await;

    let ix = ElusivInstruction::init_new_fee_version_instruction(
        0,
        9999,
        111,
        222,
        1,
        2,
        333,
        SignerAccount(payer.pubkey),
    );

    ix_should_succeed(ix.clone(), &mut payer, &mut test_program).await;

    // Second time will fail
    ix_should_fail(ix.clone(), &mut payer, &mut test_program).await;
    
    pda_account!(fee, FeeAccount, Some(0), test_program);
    assert_eq!(fee.get_lamports_per_tx(), 9999);
    assert_eq!(fee.get_base_commitment_network_fee(), 111);
    assert_eq!(fee.get_proof_network_fee(), 222);
    assert_eq!(fee.get_relayer_hash_tx_fee(), 1);
    assert_eq!(fee.get_relayer_proof_tx_fee(), 2);
    assert_eq!(fee.get_relayer_proof_reward(), 333);

    // Attempting to set a version higher than genesis (0) will fail
    let ix = ElusivInstruction::init_new_fee_version_instruction(
        1,
        9999,
        111,
        222,
        1,
        2,
        333,
        SignerAccount(payer.pubkey),
    );
    ix_should_fail(ix.clone(), &mut payer, &mut test_program).await;
}

#[tokio::test]
async fn test_setup_pda_accounts_invalid_pda() {
    let mut test_program = start_program_solana_program_test().await;
    let mut payer = Actor::new(&mut test_program).await;

    ix_should_fail(
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::CommitmentQueue, 0,
            SignerAccount(payer.pubkey),
            WritableUserAccount(BaseCommitmentQueueAccount::find(None).0)
        ),
        &mut payer, &mut test_program
    ).await;
}

#[tokio::test]
async fn test_setup_storage_account() {
    let mut test_program = start_program_solana_program_test().await;
    let keys = setup_storage_account(&mut test_program).await;

    storage_account!(storage_account, test_program);
    assert!(storage_account.get_finished_setup());
    for i in 0..StorageAccount::COUNT {
        assert_eq!(storage_account.get_pubkeys(i), keys[i].to_bytes());
    }
}

#[tokio::test]
#[should_panic]
async fn test_setup_storage_account_duplicate() {
    let mut test_program = start_program_solana_program_test().await;
    setup_storage_account(&mut test_program).await;
    setup_storage_account(&mut test_program).await;
}