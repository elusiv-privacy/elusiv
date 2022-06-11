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
use solana_sdk::{
    signature::Signer,
    transaction::Transaction,
    account::AccountSharedData,
};
use elusiv::instruction::*;
use elusiv::state::queue::{CommitmentQueueAccount, CommitmentQueue, Queue};
use elusiv::state::program_account::{SizedAccount, MultiAccountAccount, BigArrayAccount, PDAAccount, ProgramAccount, MultiAccountAccountFields};
use elusiv::processor::SingleInstancePDAAccountKind;

use crate::common::lamports_per_signature;

use super::{get_account_cost, get_data};

/// Starts a test program without any account setup
pub async fn start_program_solana_program_test() -> ProgramTestContext {
    let mut test = ProgramTest::default();
    let program_id = elusiv::id();
    test.add_program("elusiv", program_id, processor!(process_instruction));
    test.start_with_context().await
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

pub async fn setup_pda_accounts(context: &mut ProgramTestContext) {
    let nonce: u8 = rand::random();
    let lamports_per_tx = lamports_per_signature(context).await;

    let mut transaction = Transaction::new_with_payer(
        &open_all_initial_accounts(context.payer.pubkey(), nonce, lamports_per_tx),
        Some(&context.payer.pubkey()),
    );
    transaction.sign(&[&context.payer], context.last_blockhash);

    assert_matches!(context.banks_client.process_transaction(transaction).await, Ok(()));
}

pub async fn setup_storage_account<'a>(context: &mut ProgramTestContext) -> Vec<Pubkey> {
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
            &mut context.banks_client,
            &context.payer,
            context.last_blockhash,
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
                SignerAccount(context.payer.pubkey()),
                WritableUserAccount(StorageAccount::find(None).0)
            ),
            ElusivInstruction::setup_storage_account_instruction(
                &accounts.try_into().unwrap()
            ),
        ],
        Some(&context.payer.pubkey()),
    );
    transaction.sign(&[&context.payer], context.last_blockhash);

    assert_matches!(context.banks_client.process_transaction(transaction).await, Ok(()));

    result
}

pub async fn setup_pool_accounts<'a>(
    context: &mut ProgramTestContext,
) -> (Pubkey, Pubkey) {
    let sol_pool = create_account_rent_exepmt(
        &mut context.banks_client, &context.payer, context.last_blockhash, 0
    ).await;
    let fee_collector = create_account_rent_exepmt(
        &mut context.banks_client, &context.payer, context.last_blockhash, 0
    ).await;

    let mut transaction = Transaction::new_with_payer(
        &[
            ElusivInstruction::setup_pool_accounts_instruction(
                SignerAccount(context.payer.pubkey()),
                UserAccount(sol_pool.pubkey()),
                UserAccount(fee_collector.pubkey()),
            ),
        ],
        Some(&context.payer.pubkey()),
    );
    transaction.sign(&[&context.payer], context.last_blockhash);

    assert_matches!(context.banks_client.process_transaction(transaction).await, Ok(()));

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
    context: &mut ProgramTestContext,
) -> Keypair {
    let new_account_keypair = Keypair::new();
    let ix = solana_program::system_instruction::create_account(
        &context.payer.pubkey(),
        &new_account_keypair.pubkey(),
        0,
        0,
        &new_account_keypair.pubkey(),
    );

    let transaction = Transaction::new_signed_with_payer(
        &[ix],
        Some(&context.payer.pubkey()),
        &[&context.payer, &new_account_keypair],
        context.last_blockhash,
    );
    assert_matches!(context.banks_client.process_transaction(transaction).await, Ok(()));

    new_account_keypair
}

pub async fn set_pda_account<A: SizedAccount + PDAAccount, F>(
    context: &mut ProgramTestContext,
    offset: Option<u64>,
    setup: F,
)
where F: Fn(&mut [u8])
{
    let len = A::SIZE;
    let id = A::find(offset).0;
    let mut data = get_data(context, id).await;

    setup(&mut data);

    let rent_exemption = get_account_cost(context, len).await;
    set_account(context, &A::find(offset).0, data, rent_exemption).await;
}

pub async fn set_account(
    context: &mut ProgramTestContext,
    pubkey: &Pubkey,
    data: Vec<u8>,
    lamports: u64,
) {
    let mut account_shared_data = AccountSharedData::new(
        lamports,
        data.len(),
        &elusiv::id()
    );

    account_shared_data.set_data(data);
    context.set_account(
        pubkey,
        &account_shared_data
    );
}

// https://github.com/solana-labs/solana/blob/a1522d00242c2888a057c3d4238d902f063af9be/program-runtime/src/compute_budget.rs#L14
pub const MAX_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

// Fee for CUs: https://github.com/solana-labs/solana/blob/3d9874b95a4bda9bb99cb067f168811296d208cc/sdk/src/fee.rs
pub fn request_compute_units(count: u32) -> Instruction {
    //assert!(count <= MAX_COMPUTE_UNIT_LIMIT);
    ComputeBudgetInstruction::request_units(count, 0)
}