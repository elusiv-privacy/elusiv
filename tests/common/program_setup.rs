use solana_program::{
    instruction::Instruction,
    system_instruction,
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
use elusiv::state::program_account::SizedAccount;

pub async fn start_program_solana_program_test() -> (BanksClient, Keypair, Hash) {
    let mut test = ProgramTest::default();
    let program_id = elusiv::id();
    test.add_program("elusiv", program_id, processor!(process_instruction));
    test.start().await
}

async fn setup_pda_accounts(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: Hash,
) {
    let mut transaction = Transaction::new_with_payer(
        &open_all_initial_accounts(payer.pubkey()),
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer], recent_blockhash);

    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
}

async fn setup_queue_accounts(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: Hash,
) {
    let base = create_account(banks_client, &payer, recent_blockhash,  BaseCommitmentQueueAccount::SIZE).await;
    let commitment = create_account(banks_client, &payer, recent_blockhash, CommitmentQueueAccount::SIZE).await;
    let send = create_account(banks_client, &payer, recent_blockhash, SendProofQueueAccount::SIZE).await;
    let merge = create_account(banks_client, &payer, recent_blockhash, MergeProofQueueAccount::SIZE).await;
    let migrate = create_account(banks_client, &payer, recent_blockhash, MigrateProofQueueAccount::SIZE).await;
    let finalize = create_account(banks_client, &payer, recent_blockhash, FinalizeSendQueueAccount::SIZE).await;

    let mut transaction = Transaction::new_with_payer(
        &[
            request_compute_units(340_000),
            ElusivInstruction::setup_queue_accounts(
                UserAccount(base.pubkey()),
                UserAccount(commitment.pubkey()),
                UserAccount(send.pubkey()),
                UserAccount(merge.pubkey()),
                UserAccount(migrate.pubkey()),
                UserAccount(finalize.pubkey()),
            )
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer], recent_blockhash);

    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
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

pub async fn create_account(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: Hash,
    account_size: usize,
) -> Keypair {
    let new_account_keypair = Keypair::new();
    let new_account_pubkey = new_account_keypair.pubkey();

    let rent_exemption_amount = banks_client.get_rent().await.unwrap().minimum_balance(account_size);

    let create_account_ix = system_instruction::create_account(
        &payer.pubkey(),
        &new_account_pubkey,
        rent_exemption_amount,
        account_size as u64,
        &elusiv::id(),
    );
    
    let transaction = Transaction::new_signed_with_payer(
        &[create_account_ix],
        Some(&payer.pubkey()),
        &[&payer, &new_account_keypair],
        recent_blockhash,
    );
    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
    
    new_account_keypair
}

// https://github.com/solana-labs/solana/blob/a1522d00242c2888a057c3d4238d902f063af9be/program-runtime/src/compute_budget.rs#L14
pub const MAX_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

// Fee for CUs: https://github.com/solana-labs/solana/blob/3d9874b95a4bda9bb99cb067f168811296d208cc/sdk/src/fee.rs
pub fn request_compute_units(count: u32) -> Instruction {
    assert!(count <= MAX_COMPUTE_UNIT_LIMIT);
    ComputeBudgetInstruction::request_units(count, 0)
}