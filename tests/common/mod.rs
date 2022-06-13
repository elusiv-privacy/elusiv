#![allow(dead_code)]
#![allow(unused_macros)]

pub mod program_setup;
pub mod log;

use elusiv_utils::get_storage_account_sub_accounts;
use solana_program::{
    pubkey::Pubkey,
    instruction::{Instruction, AccountMeta}, system_instruction, native_token::LAMPORTS_PER_SOL,
};
use solana_program_test::ProgramTestContext;
use solana_sdk::{signature::{Keypair}, transaction::Transaction, signer::Signer};
use assert_matches::assert_matches;
use std::{str::FromStr};
use ark_bn254::Fr;
use elusiv::{types::U256, instruction::{UserAccount, WritableUserAccount}, state::STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT};
use elusiv::fields::{fr_to_u256_le};
use elusiv::processor::{BaseCommitmentHashRequest};
use elusiv::state::{StorageAccount, program_account::{PDAAccount, MultiAccountAccount}};

const DEFAULT_START_BALANCE: u64 = LAMPORTS_PER_SOL;

pub struct Actor {
    pub keypair: Keypair,
    pub pubkey: Pubkey,

    // Due to the InvalidRentPayingAccount error, we need to give our client a starting balance (= zero)
    pub start_balance: u64,
}

impl Actor {
    pub async fn new(
        context: &mut ProgramTestContext,
    ) -> Self {
        let keypair = Keypair::new();
        let pubkey = keypair.pubkey();

        airdrop(&pubkey, DEFAULT_START_BALANCE, context).await;

        Actor {
            keypair,
            pubkey,
            start_balance: DEFAULT_START_BALANCE,
        }
    }

    /// Returns the account's balance - start_balance - failed_signatures * lamports_per_signature
    pub async fn balance(&self, context: &mut ProgramTestContext) -> u64 {
        get_balance(self.pubkey, context).await - self.start_balance
    }

    pub async fn airdrop(&self, lamports: u64, context: &mut ProgramTestContext) {
        airdrop(&self.pubkey, lamports, context).await
    }
}

pub async fn get_balance(pubkey: Pubkey, context: &mut ProgramTestContext) -> u64 {
    context.banks_client.get_account(pubkey).await.unwrap().unwrap().lamports
}

pub async fn account_does_exist(pubkey: Pubkey, context: &mut ProgramTestContext) -> bool {
    matches!(context.banks_client.get_account(pubkey).await.unwrap(), Some(_))
}

pub async fn account_does_not_exist(pubkey: Pubkey, context: &mut ProgramTestContext) -> bool {
    !account_does_exist(pubkey, context).await
}

pub async fn get_data(context: &mut ProgramTestContext, id: Pubkey) -> Vec<u8> {
    context.banks_client.get_account(id).await.unwrap().unwrap().data
}

pub async fn get_account_cost(context: &mut ProgramTestContext, size: usize) -> u64 {
    let rent = context.banks_client.get_rent().await.unwrap();
    rent.minimum_balance(size)
}

pub async fn airdrop(account: &Pubkey, lamports: u64, context: &mut ProgramTestContext) {
    let mut tx = Transaction::new_with_payer(
        &[
            nonce_instruction(
                system_instruction::transfer(&context.payer.pubkey(), account, lamports)
            )
        ],
        Some(&context.payer.pubkey())
    );
    tx.sign(&[&context.payer], context.last_blockhash);
    assert_matches!(context.banks_client.process_transaction(tx).await, Ok(()));
}

#[allow(deprecated)]
pub async fn lamports_per_signature(context: &mut ProgramTestContext) -> u64 {
    context.banks_client.get_fees().await.unwrap().0.lamports_per_signature
}

// Account getters
macro_rules! queue_mut {
    ($id: ident, $ty: ty, $ty_account: ty, $data: expr) => {
        let mut queue = <$ty_account>::new($data).unwrap();
        let mut $id = <$ty>::new(&mut queue);
    };
}

macro_rules! queue {
    ($id: ident, $ty: ty, $ty_account: ty, $offset: expr, $prg: ident) => {
        let mut queue = get_data(&mut $prg, <$ty_account>::find($offset).0).await;
        let mut queue = <$ty_account>::new(&mut queue[..]).unwrap();
        let $id = <$ty>::new(&mut queue);
    };
}

macro_rules! sized_account {
    ($id: ident, $ty: ty, $offset: expr, $data: ident) => {
        let $id = <$ty>::new(&mut $data).unwrap();
    };
}

macro_rules! pda_account {
    ($id: ident, $ty: ty, $offset: expr, $prg: ident) => {
        let pk = <$ty>::find($offset).0;
        let mut data = &mut get_data(&mut $prg, pk).await[..];
        let $id = <$ty>::new(&mut data).unwrap();
    };
}

macro_rules! account_info {
    ($id: ident, $pk: expr, $prg: ident) => {
        let pk = solana_program::pubkey::Pubkey::new($pk);
        let mut a = $prg.banks_client.get_account(pk).await.unwrap().unwrap();
        let (mut lamports, mut d, owner, executable, epoch) = a.get();

        let $id = solana_program::account_info::AccountInfo::new(
            &pk,
            false,
            false,
            &mut lamports,
            &mut d,
            &owner,
            executable,
            epoch
        );
    };
}

macro_rules! storage_account {
    ($id: ident, $prg: ident) => {
        let mut data = get_data(&mut $prg, StorageAccount::find(None).0).await;

        let pks = elusiv::state::program_account::MultiAccountAccountFields::<{StorageAccount::COUNT}>::new(&data).unwrap();
        let keys = pks.pubkeys;

        account_info!(acc0, &keys[0], $prg);
        account_info!(acc1, &keys[1], $prg);
        account_info!(acc2, &keys[2], $prg);
        account_info!(acc3, &keys[3], $prg);
        account_info!(acc4, &keys[4], $prg);
        account_info!(acc5, &keys[5], $prg);
        account_info!(acc6, &keys[6], $prg);

        let sub_accounts = vec![&acc0, &acc1, &acc2, &acc3, &acc4, &acc5, &acc6];

        let $id = StorageAccount::new(&mut data[..], sub_accounts).unwrap();
    };
}

macro_rules! nullifier_account {
    ($id: ident, $index: expr, $prg: ident) => {
        let mut data = get_data(&mut $prg, NullifierAccount::find(Some($index)).0).await;

        let pks = elusiv::state::program_account::MultiAccountAccountFields::<{NullifierAccount::COUNT}>::new(&data).unwrap();
        let keys = pks.pubkeys;

        account_info!(acc0, &keys[0], $prg);
        account_info!(acc1, &keys[1], $prg);
        account_info!(acc2, &keys[2], $prg);
        account_info!(acc3, &keys[3], $prg);

        let sub_accounts = vec![&acc0, &acc1, &acc2, &acc3];

        let $id = NullifierAccount::new(&mut data[..], sub_accounts).unwrap();
    };
}

#[allow(unused_imports)] pub(crate) use queue;
#[allow(unused_imports)] pub(crate) use queue_mut;
#[allow(unused_imports)] pub(crate) use pda_account;
#[allow(unused_imports)] pub(crate) use sized_account;
#[allow(unused_imports)] pub(crate) use account_info;
#[allow(unused_imports)] pub(crate) use storage_account;
#[allow(unused_imports)] pub(crate) use nullifier_account;

pub async fn storage_accounts(
    context: &mut ProgramTestContext
) ->
(
    Vec<Pubkey>,
    [UserAccount; STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT],
    [WritableUserAccount; STORAGE_ACCOUNT_SUB_ACCOUNTS_COUNT],
)
{
    let storage_data = get_data(context, StorageAccount::find(None).0).await;
    let accounts = get_storage_account_sub_accounts(&storage_data[..]).unwrap();

    let storage_accounts: Vec<UserAccount> = accounts.iter().map(|p| UserAccount(*p)).collect();
    let writable_storage_accounts: Vec<WritableUserAccount> = accounts.iter().map(|p| WritableUserAccount(*p)).collect();

    let storage_accounts: [UserAccount; StorageAccount::COUNT] = storage_accounts.try_into().unwrap();
    let writable_storage_accounts: [WritableUserAccount; StorageAccount::COUNT] = writable_storage_accounts.try_into().unwrap();

    (accounts, storage_accounts, writable_storage_accounts)
}

use self::program_setup::set_account;

/// Adds random nonce bytes at the end of the ix data
/// - prevents rejection of previously failed ix times without repeated execution
pub fn nonce_instruction(ix: Instruction) -> Instruction {
    let mut ix = ix;
    for _ in 0..8 {
        ix.data.push(rand::random());
    }
    ix
}

/// Replaces all accounts through invalid accounts with valid data and lamports (except the signer accounts)
pub async fn invalid_accounts_fuzzing(ix: &Instruction, context: &mut ProgramTestContext) -> Vec<Instruction> {
    let mut ixs = Vec::new();
    for (i, acc) in ix.accounts.iter().enumerate() {
        if acc.is_signer { continue }
        let mut ix = ix.clone();

        // Clone data and lamports
        let id = acc.pubkey;
        let accounts_exists = account_does_exist(id, context).await;
        let data = if accounts_exists { get_data(context, id).await } else { vec![] };
        let lamports = if accounts_exists { get_balance(id, context).await } else { 100_000 };
        let new_pubkey = Pubkey::new_unique();
        set_account(context, &new_pubkey, data, lamports).await;

        if acc.is_writable {
            ix.accounts[i] = AccountMeta::new(new_pubkey, false);
        } else {
            ix.accounts[i] = AccountMeta::new_readonly(new_pubkey, false);
        }

        ixs.push(ix);
    }
    ixs
}

/// All fuzzed ix variants should fail and the original ix should afterwards succeed
/// - prefix_ixs are used to e.g. supply compute budget requests without fuzzing those ixs
pub async fn test_instruction_fuzzing(
    prefix_ixs: &[Instruction],
    valid_ix: Instruction,
    signer: &mut Actor,
    context: &mut ProgramTestContext
) {
    let invalid_instructions = invalid_accounts_fuzzing(
        &valid_ix,
        context
    ).await;

    for ix in invalid_instructions {
        let mut ixs = prefix_ixs.to_vec();
        ixs.push(ix);
        tx_should_fail(&ixs, signer, context).await;
    }

    let mut ixs = prefix_ixs.to_vec();
    ixs.push(valid_ix);
    tx_should_succeed(&ixs, signer, context).await;
}

async fn generate_and_sign_tx(
    ixs: &[Instruction],
    signer: &mut Actor,
    context: &mut ProgramTestContext,
) -> Transaction {
    let ixs: Vec<Instruction> = ixs.iter()
        .map(|ix| nonce_instruction(ix.clone()))
        .collect();
    let mut tx = Transaction::new_with_payer(
        &ixs,
        Some(&signer.pubkey)
    );
    tx.sign(
        &[&signer.keypair],
        context.banks_client.get_latest_blockhash().await.unwrap()
    );
    tx
}

// Succesful transactions
pub async fn tx_should_succeed(
    ixs: &[Instruction],
    signer: &mut Actor,
    context: &mut ProgramTestContext,
) {
    let tx = generate_and_sign_tx(ixs, signer, context).await;
    assert_matches!(context.banks_client.process_transaction(tx).await, Ok(()));
}

pub async fn ix_should_succeed(
    ix: Instruction,
    signer: &mut Actor,
    context: &mut ProgramTestContext,
) {
    tx_should_succeed(&[ix], signer, context).await
}

// Failing transactions
pub async fn tx_should_fail(
    ixs: &[Instruction],
    signer: &mut Actor,
    context: &mut ProgramTestContext,
) {
    let tx = generate_and_sign_tx(ixs, signer, context).await;
    assert_matches!(context.banks_client.process_transaction(tx).await, Err(_));

    // To compensate for failure, we airdrop
    airdrop(&signer.pubkey, lamports_per_signature(context).await, context).await;
}

pub async fn ix_should_fail(
    ix: Instruction,
    signer: &mut Actor,
    context: &mut ProgramTestContext,
) {
    tx_should_fail(&[ix], signer, context).await
}

pub fn u256_from_str(str: &str) -> U256 {
    fr_to_u256_le(&Fr::from_str(str).unwrap())
}

pub fn base_commitment_request(bc: &str, c: &str, amount: u64, fee_version: u64) -> BaseCommitmentHashRequest {
    BaseCommitmentHashRequest { base_commitment: u256_from_str(bc), commitment: u256_from_str(c), amount, fee_version }
}