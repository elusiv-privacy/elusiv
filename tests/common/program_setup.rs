use borsh::BorshSerialize;
use elusiv::commitment::{CommitmentHashingAccount, BaseCommitmentHashingAccount, self};
use elusiv::proof::VerificationAccount;
use elusiv::state::StorageAccount;
use elusiv::state::pool::PoolAccount;
use solana_program::account_info::AccountInfo;
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program::pubkey::Pubkey;
use solana_program::{
    instruction::Instruction,
    hash::Hash,
};
use solana_program_test::*;
use solana_sdk::signature::Keypair;
use elusiv::entrypoint::process_instruction;
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use assert_matches::*;
use solana_sdk::{signature::Signer, transaction::Transaction};
use elusiv::instruction::*;
use elusiv::state::queue::{SendProofQueueAccount, MigrateProofQueueAccount, MergeProofQueueAccount, FinalizeSendQueueAccount, BaseCommitmentQueueAccount, CommitmentQueueAccount, QueueManagementAccount, CommitmentQueue, Queue, BaseCommitmentQueue, SendProofQueue, MergeProofQueue, MigrateProofQueue, FinalizeSendQueue};
use elusiv::state::program_account::{SizedAccount, MultiAccountAccount, BigArrayAccount, PDAAccount, MultiAccountProgramAccount, ProgramAccount, MultiInstanceAccount, MultiAccountAccountFields};
use elusiv::processor::{SingleInstancePDAAccountKind};

use crate::common::get_data;

/// Starts a test program without any account setup
pub async fn start_program_solana_program_test() -> (BanksClient, Keypair, Hash) {
    let mut test = ProgramTest::default();
    let program_id = elusiv::id();
    test.add_program("elusiv", program_id, processor!(process_instruction));
    test.start().await
}

macro_rules! setup_queue_account_with_data {
    ($pk: ident, $clousure: ident, $tya: ty, $ty: ty, $test: ident) => {
        let mut data = vec![0; <$tya>::SIZE];
        let mut account = <$tya>::new(&mut data).unwrap();
        let mut queue = <$ty>::new(&mut account);
        $clousure(&mut queue);

        let $pk = Pubkey::new_unique();
        $test.add_account_with_base64_data(
            $pk,
            LAMPORTS_PER_SOL,
            elusiv::id(),
            &base64::encode(data),
        )
    };
}

macro_rules! setup_pda_account {
    ($ty: ty, $offset: expr, $test: ident) => {
        let mut data = vec![0; <$ty>::SIZE];
        let mut acc = <$ty>::new(&mut data).unwrap();
        let (pk, bump) = <$ty>::find($offset);
        acc.set_bump_seed(&bump);
        acc.set_initialized(&true);

        $test.add_account_with_base64_data(pk, LAMPORTS_PER_SOL, elusiv::id(), &base64::encode(data))
    };
}

/// Starts a test program with all program accounts being setup
/// - use the individual closures to assign specific values to those accounts
pub async fn start_program_solana_program_test_with_accounts_setup<B, C, S, M, N, F>(
    base_commitment_queue_setup: B,
    commitment_queue_setup: C,
    send_proof_queue_setup: S,
    merge_proof_queue_setup: M,
    migrate_proof_queue_setup: N,
    finalize_send_queue_setup: F,
) -> (BanksClient, Keypair, Hash, QueueKeys, Vec<Pubkey>)
where
    B: Fn(&mut BaseCommitmentQueue),
    C: Fn(&mut CommitmentQueue),
    S: Fn(&mut SendProofQueue),
    M: Fn(&mut MergeProofQueue),
    N: Fn(&mut MigrateProofQueue),
    F: Fn(&mut FinalizeSendQueue),
{
    let mut test = ProgramTest::default();

    // Setup the queues
    setup_queue_account_with_data!(base_commitment, base_commitment_queue_setup, BaseCommitmentQueueAccount, BaseCommitmentQueue, test);
    setup_queue_account_with_data!(commitment, commitment_queue_setup, CommitmentQueueAccount, CommitmentQueue, test);
    setup_queue_account_with_data!(send_proof, send_proof_queue_setup, SendProofQueueAccount, SendProofQueue, test);
    setup_queue_account_with_data!(merge_proof, merge_proof_queue_setup, MergeProofQueueAccount, MergeProofQueue, test);
    setup_queue_account_with_data!(migrate_proof, migrate_proof_queue_setup, MigrateProofQueueAccount, MigrateProofQueue, test);
    setup_queue_account_with_data!(finalize_send, finalize_send_queue_setup, FinalizeSendQueueAccount, FinalizeSendQueue, test);

    let mut queue_management_account_data = vec![0; QueueManagementAccount::SIZE];
    {
        let mut queue_management_account = QueueManagementAccount::new(&mut queue_management_account_data).unwrap();
        queue_management_account.set_bump_seed(&QueueManagementAccount::find(None).1);
        queue_management_account.set_initialized(&true);
        queue_management_account.set_finished_setup(&true);

        queue_management_account.set_base_commitment_queue(&base_commitment.to_bytes());
        queue_management_account.set_commitment_queue(&commitment.to_bytes());
        queue_management_account.set_send_proof_queue(&send_proof.to_bytes());
        queue_management_account.set_merge_proof_queue(&merge_proof.to_bytes());
        queue_management_account.set_migrate_proof_queue(&migrate_proof.to_bytes());
        queue_management_account.set_finalize_send_queue(&finalize_send.to_bytes());
    }

    test.add_account_with_base64_data(
        QueueManagementAccount::find(None).0,
        LAMPORTS_PER_SOL,
        elusiv::id(),
        &base64::encode(queue_management_account_data),
    );

    // Other PDA accounts
    // Single instance
    setup_pda_account!(PoolAccount, None, test);
    setup_pda_account!(CommitmentHashingAccount, None, test);

    // Multi instance
    for i in 0..BaseCommitmentHashingAccount::MAX_INSTANCES { setup_pda_account!(BaseCommitmentHashingAccount, Some(i), test); }
    for i in 0..VerificationAccount::MAX_INSTANCES { setup_pda_account!(VerificationAccount, Some(i), test); }

    // Setup Storage account
    let mut storage_accounts = Vec::new();
    for i in 0..elusiv::state::STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT {
        let account_size = if i < elusiv::state::STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT - 1 {
            StorageAccount::INTERMEDIARY_ACCOUNT_SIZE
        } else {
            StorageAccount::LAST_ACCOUNT_SIZE
        };

        let pk = Pubkey::new_unique();
        storage_accounts.push(pk);
        test.add_account_with_base64_data(pk, 100 * LAMPORTS_PER_SOL, elusiv::id(), &base64::encode(vec![0u8; account_size]));
    }

    let mut storage_account_data = vec![0; StorageAccount::SIZE];
    let (pk, bump) = StorageAccount::find(None);
    let mut storage_account = MultiAccountAccountFields::<{elusiv::state::STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT}>::new(&mut storage_account_data).unwrap();
    storage_account.bump_seed = bump;
    storage_account.initialized = true;
    for i in 0..storage_account.pubkeys.len() {
        storage_account.pubkeys[i] = storage_accounts[i].to_bytes();
    }
    let serialized = storage_account.try_to_vec().unwrap();
    for i in 0..serialized.len() {
        storage_account_data[i] = serialized[i];
    }
    test.add_account_with_base64_data(pk, LAMPORTS_PER_SOL, elusiv::id(), &base64::encode(storage_account_data));

    // Start test validator
    let program_id = elusiv::id();
    test.add_program("elusiv", program_id, processor!(process_instruction));
    let (b, k, h) = test.start().await;

    (b, k, h, QueueKeys { base_commitment, commitment, send_proof, merge_proof, migrate_proof, finalize_send }, storage_accounts)
}

pub async fn setup_pda_accounts(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: Hash,
) {
    let nonce: u8 = rand::random();

    let mut transaction = Transaction::new_with_payer(
        &open_all_initial_accounts(payer.pubkey(), nonce),
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer], recent_blockhash);

    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
}

#[derive(Clone)]
pub struct QueueKeys {
    pub base_commitment: Pubkey,
    pub commitment: Pubkey,
    pub send_proof: Pubkey,
    pub merge_proof: Pubkey,
    pub migrate_proof: Pubkey,
    pub finalize_send: Pubkey,
}

pub async fn create_queue_accounts(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: Hash,
) -> QueueKeys {
    let base_commitment = create_account_rent_exepmt(banks_client, &payer, recent_blockhash,  BaseCommitmentQueueAccount::SIZE).await;
    let commitment = create_account_rent_exepmt(banks_client, &payer, recent_blockhash, CommitmentQueueAccount::SIZE).await;
    let send_proof = create_account_rent_exepmt(banks_client, &payer, recent_blockhash, SendProofQueueAccount::SIZE).await;
    let merge_proof = create_account_rent_exepmt(banks_client, &payer, recent_blockhash, MergeProofQueueAccount::SIZE).await;
    let migrate_proof = create_account_rent_exepmt(banks_client, &payer, recent_blockhash, MigrateProofQueueAccount::SIZE).await;
    let finalize_send = create_account_rent_exepmt(banks_client, &payer, recent_blockhash, FinalizeSendQueueAccount::SIZE).await;

    QueueKeys {
        base_commitment: base_commitment.pubkey(),
        commitment: commitment.pubkey(),
        send_proof: send_proof.pubkey(),
        merge_proof: merge_proof.pubkey(),
        migrate_proof: migrate_proof.pubkey(),
        finalize_send: finalize_send.pubkey(),
    }
}

pub fn setup_queue_accounts_ix(
    keys: &QueueKeys,
) -> Vec<Instruction> {
    vec![
        request_compute_units(600_000),
        ElusivInstruction::setup_queue_accounts_instruction(
            UserAccount(keys.base_commitment),
            UserAccount(keys.commitment),
            UserAccount(keys.send_proof),
            UserAccount(keys.merge_proof),
            UserAccount(keys.migrate_proof),
            UserAccount(keys.finalize_send),
        )
    ]
}

pub async fn setup_queue_accounts(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: Hash,
) -> QueueKeys {
    let keys = create_queue_accounts(banks_client, payer, recent_blockhash).await;

    let mut transaction = Transaction::new_with_payer(
        &setup_queue_accounts_ix(&keys),
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer], recent_blockhash);

    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

    keys    
}

pub async fn setup_storage_account<'a>(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: Hash,
) -> Vec<Pubkey> {
    let nonce: u8 = rand::random();

    let mut accounts = Vec::new();
    let mut result = Vec::new();
    for i in 0..elusiv::state::STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT {
        let account_size = if i < elusiv::state::STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT - 1 {
            StorageAccount::INTERMEDIARY_ACCOUNT_SIZE
        } else {
            StorageAccount::LAST_ACCOUNT_SIZE
        };

        let pk = create_account_rent_exepmt(banks_client, payer, recent_blockhash, account_size).await.pubkey();
        result.push(pk);
        accounts.push(WritableUserAccount(pk));
    }

    let mut transaction = Transaction::new_with_payer(
        &[
            ElusivInstruction::open_single_instance_account_instruction(
                SingleInstancePDAAccountKind::Storage,
                nonce,
                SignerAccount(payer.pubkey()),
                WritableUserAccount(StorageAccount::find(None).0)
            ),
            ElusivInstruction::setup_storage_account_instruction(
                accounts.try_into().unwrap()
            ),
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer], recent_blockhash);

    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

    result
}

pub async fn create_account_rent_exepmt(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: Hash,
    account_size: usize,
) -> Keypair {
    let amount = banks_client.get_rent().await.unwrap().minimum_balance(account_size);

    let (ix, keypair) = elusiv_utils::create_account(payer, &elusiv::id(), account_size, amount).unwrap();
    let transaction = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &keypair],
        recent_blockhash,
    );
    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

    keypair
}

macro_rules! account {
    ($id: ident, $pubkey: expr, $data: expr) => {
        let mut lamports = 0;
        let mut data = $data;
        let owner = elusiv::id();
        let $id = AccountInfo::new(
            &$pubkey,
            false, false, &mut lamports,
            &mut data,
            &owner,
            false,
            0
        );
    };
}

pub async fn execute_on_storage_account<F>(
    banks_client: &mut BanksClient,
    keys: &Vec<Pubkey>,
    clousure: F,
) where F: Fn(&StorageAccount) -> () {
    account!(acc0, &keys[0], &mut get_data(banks_client, keys[0]).await);
    account!(acc1, &keys[1], &mut get_data(banks_client, keys[1]).await);
    account!(acc2, &keys[2], &mut get_data(banks_client, keys[2]).await);
    account!(acc3, &keys[3], &mut get_data(banks_client, keys[3]).await);
    account!(acc4, &keys[4], &mut get_data(banks_client, keys[4]).await);
    account!(acc5, &keys[5], &mut get_data(banks_client, keys[5]).await);
    account!(acc6, &keys[6], &mut get_data(banks_client, keys[6]).await);

    let sub_accounts = vec![&acc0, &acc1, &acc2, &acc3, &acc4, &acc5, &acc6];

    let mut storage_account = super::get_data(banks_client, StorageAccount::find(None).0).await;
    let storage_account = StorageAccount::new(&mut storage_account[..], sub_accounts).unwrap();

    clousure(&storage_account)
}

// https://github.com/solana-labs/solana/blob/a1522d00242c2888a057c3d4238d902f063af9be/program-runtime/src/compute_budget.rs#L14
pub const MAX_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

// Fee for CUs: https://github.com/solana-labs/solana/blob/3d9874b95a4bda9bb99cb067f168811296d208cc/sdk/src/fee.rs
pub fn request_compute_units(count: u32) -> Instruction {
    //assert!(count <= MAX_COMPUTE_UNIT_LIMIT);
    ComputeBudgetInstruction::request_units(count, 0)
}