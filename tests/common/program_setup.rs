use elusiv::state::StorageAccount;
use solana_program::account_info::AccountInfo;
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
use elusiv::state::queue::{SendProofQueueAccount, MigrateProofQueueAccount, MergeProofQueueAccount, FinalizeSendQueueAccount, BaseCommitmentQueueAccount, CommitmentQueueAccount};
use elusiv::state::program_account::{SizedAccount, MultiAccountAccount, BigArrayAccount, PDAAccount};
use elusiv::processor::{MultiInstancePDAAccountKind, SingleInstancePDAAccountKind};

pub async fn start_program_solana_program_test() -> (BanksClient, Keypair, Hash) {
    let mut test = ProgramTest::default();
    let program_id = elusiv::id();
    test.add_program("elusiv", program_id, processor!(process_instruction));
    test.start().await
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
        ElusivInstruction::setup_queue_accounts(
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
            ElusivInstruction::open_single_instance_account(
                SingleInstancePDAAccountKind::Storage,
                nonce,
                SignerAccount(payer.pubkey()),
                WritableUserAccount(StorageAccount::find(None).0)
            ),
            ElusivInstruction::setup_storage_account(
                accounts.try_into().unwrap()
            ),
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer], recent_blockhash);

    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));

    result
}

pub async fn setup_all_accounts(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: Hash,
) {
    // Create PDA accounts
    setup_pda_accounts(banks_client, &payer, recent_blockhash).await;

    // Create queue accounts
    setup_queue_accounts(banks_client, payer, recent_blockhash).await;
}

pub async fn create_account_rent_exepmt(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: Hash,
    account_size: usize,
) -> Keypair {
    let amount = banks_client.get_rent().await.unwrap().minimum_balance(account_size);

    let (ix, keypair) = elusiv_setup::create_account(payer, &elusiv::id(), account_size, amount).unwrap();
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
    account!(acc0, &keys[0], vec![]);
    account!(acc1, &keys[1], vec![]);
    account!(acc2, &keys[2], vec![]);
    account!(acc3, &keys[3], vec![]);
    account!(acc4, &keys[4], vec![]);
    account!(acc5, &keys[5], vec![]);
    account!(acc6, &keys[6], vec![]);

    let sub_accounts = vec![acc0, acc1, acc2, acc3, acc4, acc5, acc6];

    let mut storage_account = super::get_data(banks_client, StorageAccount::find(None).0).await;
    let storage_account = StorageAccount::new(&mut storage_account[..], &sub_accounts[..]).unwrap();

    clousure(&storage_account)
}

// https://github.com/solana-labs/solana/blob/a1522d00242c2888a057c3d4238d902f063af9be/program-runtime/src/compute_budget.rs#L14
pub const MAX_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

// Fee for CUs: https://github.com/solana-labs/solana/blob/3d9874b95a4bda9bb99cb067f168811296d208cc/sdk/src/fee.rs
pub fn request_compute_units(count: u32) -> Instruction {
    assert!(count <= MAX_COMPUTE_UNIT_LIMIT);
    ComputeBudgetInstruction::request_units(count, 0)
}