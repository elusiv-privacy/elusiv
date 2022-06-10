use borsh::BorshSerialize;
use elusiv::commitment::CommitmentHashingAccount;
use elusiv::state::StorageAccount;
use elusiv::state::pool::PoolAccount;
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
use elusiv::state::queue::{CommitmentQueueAccount, CommitmentQueue, Queue};
use elusiv::state::program_account::{SizedAccount, MultiAccountAccount, BigArrayAccount, PDAAccount, ProgramAccount, MultiAccountAccountFields};
use elusiv::processor::SingleInstancePDAAccountKind;

use crate::common::lamports_per_signature;

pub struct TestProgram {
    pub banks_client: BanksClient,
    pub keypair: Keypair,
    pub blockhash: Hash,
}

/// Starts a test program without any account setup
pub async fn start_program_solana_program_test() -> TestProgram {
    let mut test = ProgramTest::default();
    let program_id = elusiv::id();
    test.add_program("elusiv", program_id, processor!(process_instruction));
    let (banks_client, keypair, blockhash) = test.start().await;
    TestProgram { banks_client, keypair, blockhash }
}

macro_rules! setup_pda_account {
    ($ty: ty, $offset: expr, $test: ident, $data: expr) => {
        let mut data = match $data { Some(d) => d, None => vec![0; <$ty>::SIZE] };
        let mut acc = <$ty>::new(&mut data).unwrap();
        let (pk, bump) = <$ty>::find($offset);
        acc.set_bump_seed(&bump);
        acc.set_initialized(&true);

        $test.add_account_with_base64_data(pk, LAMPORTS_PER_SOL, elusiv::id(), &base64::encode(data))
    };
}

macro_rules! setup_pda_account_with_closure {
    ($ty: ty, $offset: expr, $test: ident, $closure: ident) => {
        let mut data = vec![0; <$ty>::SIZE];
        let mut acc = <$ty>::new(&mut data).unwrap();
        $closure(&mut acc);
        setup_pda_account!($ty, $offset, $test, Some(data))
    };
}

/// Starts a test program with all program accounts being setup
/// - use the individual closures to assign specific values to those accounts
pub async fn start_program_solana_program_test_with_accounts_setup<Q, C>(
    commitment_queue_setup: Q,
    commitment_hashing_account_setup: C,
) -> (BanksClient, Keypair, Hash, Vec<Pubkey>)
where
    Q: Fn(&mut CommitmentQueue),
    C: Fn(&mut CommitmentHashingAccount),
{
    let mut test = ProgramTest::default();

    let setup_commitment_queue = |acc: &mut CommitmentQueueAccount| {
        let mut queue = CommitmentQueue::new(acc);
        commitment_queue_setup(&mut queue)
    };
    setup_pda_account_with_closure!(CommitmentQueueAccount, None, test, setup_commitment_queue);
    setup_pda_account_with_closure!(CommitmentHashingAccount, None, test, commitment_hashing_account_setup);
    setup_pda_account!(PoolAccount, None, test, None);

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
    let (mut b, k, h) = test.start().await;

    // Genesis fee account
    let lamports_per_tx = panic!();//lamports_per_signature(test_program).await;
    let mut transaction = Transaction::new_with_payer(&[init_genesis_fee_account(k.pubkey(), lamports_per_tx)], Some(&k.pubkey()));
    transaction.sign(&[&k], h);
    assert_matches!(b.process_transaction(transaction).await, Ok(()));

    (b, k, h, storage_accounts)
}

pub async fn setup_pda_accounts(test_program: &mut TestProgram) {
    let nonce: u8 = rand::random();
    let lamports_per_tx = lamports_per_signature(test_program).await;

    let mut transaction = Transaction::new_with_payer(
        &open_all_initial_accounts(test_program.keypair.pubkey(), nonce, lamports_per_tx),
        Some(&test_program.keypair.pubkey()),
    );
    transaction.sign(&[&test_program.keypair], test_program.blockhash);

    assert_matches!(test_program.banks_client.process_transaction(transaction).await, Ok(()));
}

pub async fn setup_storage_account<'a>(test_program: &mut TestProgram) -> Vec<Pubkey> {
    let nonce: u8 = rand::random();

    let mut accounts = Vec::new();
    let mut result = Vec::new();
    for i in 0..elusiv::state::STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT {
        let account_size = if i < elusiv::state::STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT - 1 {
            StorageAccount::INTERMEDIARY_ACCOUNT_SIZE
        } else {
            StorageAccount::LAST_ACCOUNT_SIZE
        };

        let pk = create_account_rent_exepmt(
            &mut test_program.banks_client,
            &test_program.keypair,
            test_program.blockhash,
            account_size
        ).await.pubkey();
        result.push(pk);
        accounts.push(WritableUserAccount(pk));
    }

    let mut transaction = Transaction::new_with_payer(
        &[
            ElusivInstruction::open_single_instance_account_instruction(
                SingleInstancePDAAccountKind::Storage,
                nonce,
                SignerAccount(test_program.keypair.pubkey()),
                WritableUserAccount(StorageAccount::find(None).0)
            ),
            ElusivInstruction::setup_storage_account_instruction(
                accounts.try_into().unwrap()
            ),
        ],
        Some(&test_program.keypair.pubkey()),
    );
    transaction.sign(&[&test_program.keypair], test_program.blockhash);

    assert_matches!(test_program.banks_client.process_transaction(transaction).await, Ok(()));

    result
}

pub async fn setup_pool_accounts<'a>(
    test_program: &mut TestProgram,
) -> (Pubkey, Pubkey) {
    let sol_pool = create_account_rent_exepmt(&mut test_program.banks_client, &test_program.keypair, test_program.blockhash, 0).await;
    let fee_collector = create_account_rent_exepmt(&mut test_program.banks_client, &test_program.keypair, test_program.blockhash, 0).await;

    let mut transaction = Transaction::new_with_payer(
        &[
            ElusivInstruction::setup_pool_accounts_instruction(
                SignerAccount(test_program.keypair.pubkey()),
                UserAccount(sol_pool.pubkey()),
                UserAccount(fee_collector.pubkey()),
            ),
        ],
        Some(&test_program.keypair.pubkey()),
    );
    transaction.sign(&[&test_program.keypair], test_program.blockhash);

    assert_matches!(test_program.banks_client.process_transaction(transaction).await, Ok(()));

    (
        sol_pool.pubkey(),
        fee_collector.pubkey()
    )
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

pub async fn create_account(
    test_program: &mut TestProgram,
) -> Keypair {
    let new_account_keypair = Keypair::new();
    let ix = solana_program::system_instruction::create_account(
        &test_program.keypair.pubkey(),
        &new_account_keypair.pubkey(),
        0,
        0,
        &new_account_keypair.pubkey(),
    );

    let transaction = Transaction::new_signed_with_payer(
        &[ix],
        Some(&test_program.keypair.pubkey()),
        &[&test_program.keypair, &new_account_keypair],
        test_program.blockhash,
    );
    assert_matches!(test_program.banks_client.process_transaction(transaction).await, Ok(()));

    new_account_keypair
}

// https://github.com/solana-labs/solana/blob/a1522d00242c2888a057c3d4238d902f063af9be/program-runtime/src/compute_budget.rs#L14
pub const MAX_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

// Fee for CUs: https://github.com/solana-labs/solana/blob/3d9874b95a4bda9bb99cb067f168811296d208cc/sdk/src/fee.rs
pub fn request_compute_units(count: u32) -> Instruction {
    //assert!(count <= MAX_COMPUTE_UNIT_LIMIT);
    ComputeBudgetInstruction::request_units(count, 0)
}