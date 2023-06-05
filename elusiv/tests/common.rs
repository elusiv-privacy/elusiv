#![allow(unused_macros)]
#![allow(dead_code)]

use elusiv::{
    fields::fr_to_u256_le,
    instruction::ElusivInstruction,
    proof::verifier::{CombinedMillerLoop, FinalExponentiation},
    state::{
        fee::{BasisPointFee, ProgramFee},
        metadata::MetadataAccount,
        nullifier::NullifierAccount,
        storage::StorageAccount,
    },
    types::U256,
};
use elusiv_computation::PartialComputation;
pub use elusiv_test::*;
use elusiv_types::{
    elusiv_token, Lamports, PDAAccount, PDAOffset, WritableSignerAccount, WritableUserAccount,
};
use std::str::FromStr;

pub async fn start_test() -> ElusivProgramTest {
    ElusivProgramTest::start(&[(
        String::from("elusiv"),
        elusiv::id(),
        processor!(elusiv::process_instruction),
    )])
    .await
}

pub async fn start_test_with_setup() -> ElusivProgramTest {
    let mut test = start_test().await;
    let genesis_fee = genesis_fee(&mut test).await;

    setup_initial_pdas(&mut test).await;
    setup_fee(&mut test, 0, genesis_fee).await;

    test
}

pub async fn genesis_fee(test: &mut ElusivProgramTest) -> ProgramFee {
    ProgramFee {
        lamports_per_tx: test.lamports_per_signature().await,
        base_commitment_network_fee: BasisPointFee(11),
        proof_network_fee: BasisPointFee(100),
        base_commitment_subvention: Lamports(33),
        proof_subvention: Lamports(44),
        warden_hash_tx_reward: Lamports(300),
        warden_proof_reward: Lamports(555),
        proof_base_tx_count: (CombinedMillerLoop::TX_COUNT + FinalExponentiation::TX_COUNT + 2)
            as u64,
    }
}

pub async fn setup_initial_pdas(test: &mut ElusivProgramTest) {
    let ixs = initial_single_instance_pdas(test.payer());
    test.tx_should_succeed_simple(&ixs).await;
}

pub fn initial_single_instance_pdas(payer: Pubkey) -> Vec<Instruction> {
    vec![
        ElusivInstruction::setup_governor_account_instruction(WritableSignerAccount(payer)),
        ElusivInstruction::open_single_instance_accounts_instruction(WritableSignerAccount(payer)),
        ElusivInstruction::create_new_accounts_v1_instruction(WritableSignerAccount(payer)),
    ]
}

pub async fn setup_fee(test: &mut ElusivProgramTest, fee_version: u32, program_fee: ProgramFee) {
    let ix = ElusivInstruction::init_new_fee_version_instruction(
        fee_version,
        program_fee,
        WritableSignerAccount(test.payer()),
    );
    test.ix_should_succeed_simple(ix).await;
}

macro_rules! setup_parent_account {
    ($fn_id: ident, $ty: ty, $instruction: ident) => {
        pub async fn $fn_id(test: &mut ElusivProgramTest) -> Vec<Pubkey> {
            let mut instructions = Vec::new();
            let pubkeys = test.create_parent_account::<$ty>(&elusiv::id()).await;

            for (i, p) in pubkeys.iter().enumerate() {
                instructions.push(ElusivInstruction::$instruction(
                    i as u32,
                    WritableUserAccount(*p),
                ));
            }
            test.tx_should_succeed_simple(&instructions).await;

            pubkeys
        }
    };
}

setup_parent_account!(
    setup_storage_account,
    StorageAccount,
    enable_storage_child_account_instruction
);

setup_parent_account!(
    setup_metadata_account,
    MetadataAccount,
    enable_metadata_child_account_instruction
);

pub async fn create_merkle_tree(test: &mut ElusivProgramTest, mt_index: u32) -> Vec<Pubkey> {
    let mut instructions = vec![ElusivInstruction::open_nullifier_account_instruction(
        mt_index,
        WritableSignerAccount(test.payer()),
    )];

    let pubkeys = test
        .create_parent_account::<NullifierAccount>(&elusiv::id())
        .await;
    for (i, p) in pubkeys.iter().enumerate() {
        instructions.push(
            ElusivInstruction::enable_nullifier_child_account_instruction(
                mt_index,
                i as u32,
                WritableUserAccount(*p),
            ),
        );
    }
    test.tx_should_succeed_simple(&instructions).await;

    pubkeys
}

macro_rules! child_accounts_getter_simple {
    ($fn_id: ident, $ty: ty) => {
        pub async fn $fn_id(test: &mut ElusivProgramTest) -> Vec<Pubkey> {
            let mut data = test.data(&<$ty>::find(None).0).await;
            test.child_accounts::<$ty>(&mut data).await
        }
    };
}

child_accounts_getter_simple!(storage_accounts, StorageAccount);
child_accounts_getter_simple!(metadata_accounts, MetadataAccount);

pub async fn nullifier_accounts(test: &mut ElusivProgramTest, mt_index: u32) -> Vec<Pubkey> {
    let mut data = test.data(&NullifierAccount::find(Some(mt_index)).0).await;
    test.child_accounts::<NullifierAccount>(&mut data).await
}

/// mut? $id: ident, $ty: ty, $pubkey: expr, $offset: expr, $test: ident
macro_rules! pda_account {
    ($id: ident, $ty: ty, $pubkey: expr, $offset: expr, $test: expr) => {
        pda_account!(data data, $ty, $pubkey, $offset, $test);
        let $id = <$ty>::new(&mut data).unwrap();
    };
    (mut $id: ident, $ty: ty, $pubkey: expr, $offset: expr, $test: expr) => {
        pda_account!(data data, $ty, $pubkey, $offset, $test);
        let mut $id = <$ty>::new(&mut data).unwrap();
    };

    (data $data: ident, $ty: ty, $pubkey: expr, $offset: expr, $test: expr) => {
        let pk = <$ty>::find_with_pubkey_optional($pubkey, $offset).0;
        let mut $data = &mut $test.data(&pk).await[..];
    };
}

macro_rules! queue {
    ($id: ident, $ty: ty, $test: expr) => {
        pda_account!(
            mut q,
            <$ty as elusiv::state::queue::QueueAccount>::T,
            None,
            None,
            $test
        );
        let $id = <$ty as elusiv::state::queue::Queue<
            <$ty as elusiv::state::queue::QueueAccount>::T,
        >>::new(&mut q);
    };
    (mut $id: ident, $ty: ty, $data: expr) => {
        let mut q = <$ty as elusiv::state::queue::QueueAccount>::T::new($data).unwrap();
        let mut $id = <$ty>::new(&mut q);
    };
}

#[allow(unused_imports)]
pub(crate) use pda_account;
#[allow(unused_imports)]
pub(crate) use queue;

use solana_program::{instruction::Instruction, pubkey::Pubkey};
use solana_program_test::processor;
use spl_associated_token_account::instruction::create_associated_token_account;

macro_rules! parent_account {
    ($id: ident, $ty: ty) => {
        pub async fn $id<F>(pda_offset: elusiv_types::PDAOffset, test: &mut ElusivProgramTest, f: F)
        where
            F: Fn(&$ty),
        {
            let mut data = test
                .data(&<$ty as elusiv_types::PDAAccount>::find(pda_offset).0)
                .await;
            let keys = test.child_accounts::<$ty>(&mut data).await;

            let mut v = vec![];
            for &key in keys.iter() {
                let account = test
                    .context()
                    .banks_client
                    .get_account(key)
                    .await
                    .unwrap()
                    .unwrap();
                v.push(account);
            }

            let accs = v.iter_mut();
            let mut child_accounts = Vec::new();
            use solana_program::account_info::Account;

            for (i, a) in accs.enumerate() {
                let (lamports, d, owner, executable, epoch) = a.get();
                let child_account = solana_program::account_info::AccountInfo::new(
                    &keys[i], false, false, lamports, d, owner, executable, epoch,
                );
                child_accounts.push(child_account);
            }

            let account = <$ty as elusiv_types::accounts::ParentAccount>::new_with_child_accounts(
                &mut data,
                child_accounts.iter().map(|x| Some(x)).collect(),
            )
            .unwrap();

            f(&account)
        }
    };
}

parent_account!(storage_account, StorageAccount);
parent_account!(nullifier_account, NullifierAccount);
parent_account!(metadata_account, MetadataAccount);

pub fn u256_from_str(str: &str) -> U256 {
    fr_to_u256_le(&ark_bn254::Fr::from_str(str).unwrap())
}

pub fn u256_from_str_skip_mr(str: &str) -> [u8; 32] {
    let n = num::BigUint::from_str(str).unwrap();
    let bytes = n.to_bytes_le();
    let mut result = [0; 32];
    for i in 0..32 {
        if i < bytes.len() {
            result[i] = bytes[i];
        }
    }
    result
}

pub async fn enable_program_token_account<A: PDAAccount>(
    test: &mut ElusivProgramTest,
    token_id: u16,
    offset: PDAOffset,
) {
    let ix = create_associated_token_account(
        &test.payer(),
        &A::find(offset).0,
        &elusiv_token(token_id).unwrap().mint,
        &spl_token::id(),
    );
    test.process_transaction(&[ix], &[]).await.unwrap();
}
