use std::collections::HashMap;

use elusiv::proof::precompute::{VKEY_COUNT, precompute_account_size2, VirtualPrecomputes, precompute_account_size, PrecomputesAccount};
use elusiv::proof::vkey::{VerificationKey, SendQuadraVKey, MigrateUnaryVKey};
use elusiv::proof::{CombinedMillerLoop, FinalExponentiation};
use elusiv::state::fee::ProgramFee;
use elusiv::state::{StorageAccount, NullifierAccount};
use elusiv_computation::PartialComputation;
use solana_program::pubkey::Pubkey;
use solana_program::instruction::Instruction;
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
use elusiv::state::program_account::{SizedAccount, PDAAccount, MultiAccountAccount, SUB_ACCOUNT_ADDITIONAL_SIZE, MultiAccountProgramAccount};
use elusiv::processor::{SingleInstancePDAAccountKind, MultiInstancePDAAccountKind};
use crate::common::nonce_instruction;
use super::{get_account_cost, get_data, lamports_per_signature, get_balance};

/// Starts a test program without any account setup
pub async fn start_program_solana_program_test() -> ProgramTestContext {
    let mut test = ProgramTest::default();
    let program_id = elusiv::id();
    test.add_program("elusiv", program_id, processor!(process_instruction));
    test.start_with_context().await
}

async fn send_tx(ixs: &[Instruction], context: &mut ProgramTestContext) {
    let mut transaction = Transaction::new_with_payer(ixs, Some(&context.payer.pubkey()));
    transaction.sign(&[&context.payer], context.last_blockhash);
    assert_matches!(context.banks_client.process_transaction(transaction).await, Ok(()));
}

pub async fn setup_initial_accounts(context: &mut ProgramTestContext) {
    // Initial PDA accounts
    let ixs = open_all_initial_accounts(context.payer.pubkey());
    let ixs: Vec<Instruction> = ixs.iter().map(|ix| nonce_instruction(ix.clone())).collect();
    send_tx(&ixs, context).await;

    // Fee account
    setup_fee_account(context).await;
}

pub async fn setup_fee_account(context: &mut ProgramTestContext) {
    let lamports_per_tx = lamports_per_signature(context).await;
    send_tx(&[
        ElusivInstruction::init_new_fee_version_instruction(
            0,
            genesis_fee(lamports_per_tx),
            WritableSignerAccount(context.payer.pubkey())
        )
    ], context).await;
}

pub fn genesis_fee(lamports_per_tx: u64) -> ProgramFee {
    ProgramFee {
        lamports_per_tx,
        base_commitment_network_fee: 11,
        proof_network_fee: 100,
        base_commitment_subvention: 33,
        proof_subvention: 44,
        relayer_hash_tx_fee: 300,
        relayer_proof_reward: 555,
        proof_base_tx_count: (CombinedMillerLoop::TX_COUNT + FinalExponentiation::TX_COUNT + 2) as u64,
    }
}

pub async fn setup_storage_account<'a>(context: &mut ProgramTestContext) -> Vec<Pubkey> {
    send_tx(&[
        ElusivInstruction::open_single_instance_account_instruction(
            SingleInstancePDAAccountKind::StorageAccount,
            WritableSignerAccount(context.payer.pubkey()),
            WritableUserAccount(StorageAccount::find(None).0)
        )
    ], context).await;

    let mut instructions = Vec::new();
    let pubkeys = create_multi_account::<StorageAccount>(context).await;
    for (i, p) in pubkeys.iter().enumerate() {
        instructions.push(
            ElusivInstruction::enable_storage_sub_account_instruction(i as u32, WritableUserAccount(*p))
        );
    }
    send_tx(&instructions, context).await;

    pubkeys
}

pub async fn create_merkle_tree(
    context: &mut ProgramTestContext,
    mt_index: u32,
) -> Vec<Pubkey> {
    let mut instructions = vec![
        ElusivInstruction::open_multi_instance_account_instruction(
            MultiInstancePDAAccountKind::NullifierAccount,
            mt_index,
            WritableSignerAccount(context.payer.pubkey()),
            WritableUserAccount(NullifierAccount::find(Some(mt_index)).0)
        )
    ];

    let pubkeys = create_multi_account::<NullifierAccount>(context).await;
    for (i, p) in pubkeys.iter().enumerate() {
        instructions.push(
            ElusivInstruction::enable_nullifier_sub_account_instruction(mt_index, i as u32, WritableUserAccount(*p))
        );
    }
    send_tx(&instructions, context).await;

    pubkeys
}

async fn create_multi_account<'a, T: MultiAccountAccount<'a>>(
    context: &mut ProgramTestContext
) -> Vec<Pubkey> {
    let mut result = Vec::new();

    for _ in 0..T::COUNT {
        let pk = create_account_rent_exempt(context, T::ACCOUNT_SIZE).await.pubkey();
        result.push(pk);
    }

    result
}

pub async fn create_account_rent_exempt(
    context: &mut ProgramTestContext,
    account_size: usize,
) -> Keypair {
    let amount = context.banks_client.get_rent().await.unwrap().minimum_balance(account_size);

    let (ix, keypair) = elusiv_utils::create_account(&context.payer, &elusiv::id(), account_size, amount).unwrap();
    let transaction = Transaction::new_signed_with_payer(
        &[ix],
        Some(&context.payer.pubkey()),
        &[&context.payer, &keypair],
        context.last_blockhash,
    );
    assert_matches!(context.banks_client.process_transaction(transaction).await, Ok(()));

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

/// `$ty: ty, $context: expr, $offset: expr, $setup: expr`
macro_rules! set_single_pda_account {
    ($ty: ty, $context: expr, $offset: expr, $setup: expr) => {
        set_pda_account::<$ty, _>($context, $offset, |data| {
            let mut account = <$ty>::new(data).unwrap();
            $setup(&mut account);
        }).await;
    };
}

#[allow(unused_imports)] pub(crate) use set_single_pda_account;

pub async fn set_pda_account<A: SizedAccount + PDAAccount, F>(
    context: &mut ProgramTestContext,
    offset: Option<u32>,
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

pub async fn setup_precomputes(
    context: &mut ProgramTestContext,
) -> Vec<UserAccount> {
    // Enable sub accounts
    let mut ixs = Vec::new();
    let mut pubkeys = Vec::new();
    for i in 0..VKEY_COUNT {
        let size = precompute_account_size2(i) + SUB_ACCOUNT_ADDITIONAL_SIZE;
        let account = create_account_rent_exempt(context, size).await;
        pubkeys.push(account.pubkey());
        ixs.push(
            ElusivInstruction::enable_precompute_sub_account_instruction(
                i as u32,
                WritableUserAccount(account.pubkey())
            )
        );
    }
    send_tx(&ixs, context).await;

    async fn setup<VKey: VerificationKey>(
        context: &mut ProgramTestContext,
        pubkey: &Pubkey,
    ) {
        let lamports = get_balance(pubkey, context).await;
        let mut d = vec![0; precompute_account_size::<VKey>()];
        let _ = VirtualPrecomputes::<VKey>::new(&mut d);
        let mut data = vec![1];
        data.extend(d);
        set_account(context, pubkey, data, lamports).await;
    }

    setup::<SendQuadraVKey>(context, &pubkeys[0]).await;
    setup::<MigrateUnaryVKey>(context, &pubkeys[1]).await;

    let pk = PrecomputesAccount::find(None).0;
    let mut data = get_data(context, pk).await;
    let mut account = PrecomputesAccount::new(&mut data, HashMap::new()).unwrap();
    account.set_is_setup(&true);
    let lamports = get_balance(&pk, context).await;
    set_account(context, &pk, data, lamports).await;

    pubkeys.iter().map(|p| UserAccount(*p)).collect()
}

// https://github.com/solana-labs/solana/blob/a1522d00242c2888a057c3d4238d902f063af9be/program-runtime/src/compute_budget.rs#L14
pub const MAX_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

// Fee for CUs: https://github.com/solana-labs/solana/blob/3d9874b95a4bda9bb99cb067f168811296d208cc/sdk/src/fee.rs
pub fn request_compute_units(count: u32) -> Instruction {
    //assert!(count <= MAX_COMPUTE_UNIT_LIMIT);
    ComputeBudgetInstruction::request_units(count, 0)
}