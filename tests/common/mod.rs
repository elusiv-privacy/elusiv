#![allow(dead_code)]
#![allow(unused_macros)]

pub mod program_setup;
pub mod log;

use solana_program::{
    pubkey::Pubkey,
    instruction::Instruction, system_instruction,
};
use solana_sdk::{signature::{Keypair}, transaction::Transaction, signer::Signer};
use assert_matches::assert_matches;
use program_setup::TestProgram;
use std::str::FromStr;
use ark_bn254::Fr;
use elusiv::types::U256;
use elusiv::fields::{fr_to_u256_le};
use elusiv::processor::{BaseCommitmentHashRequest};

pub async fn get_balance(pubkey: Pubkey, test_program: &mut TestProgram) -> u64 {
    test_program.banks_client.get_account(pubkey).await.unwrap().unwrap().lamports
}

pub async fn account_does_exist(pubkey: Pubkey, test_program: &mut TestProgram) -> bool {
    matches!(test_program.banks_client.get_account(pubkey).await.unwrap(), Some(_))
}

pub async fn account_does_not_exist(pubkey: Pubkey, test_program: &mut TestProgram) -> bool {
    !account_does_exist(pubkey, test_program).await
}

pub async fn get_data(test_program: &mut TestProgram, id: Pubkey) -> Vec<u8> {
    test_program.banks_client.get_account(id).await.unwrap().unwrap().data
}

pub async fn get_account_cost(test_program: &mut TestProgram, size: usize) -> u64 {
    let rent = test_program.banks_client.get_rent().await.unwrap();
    rent.minimum_balance(size)
}

pub async fn airdrop(account: &Pubkey, lamports: u64, test_program: &mut TestProgram) {
    let mut tx = Transaction::new_with_payer(
        &[
            system_instruction::transfer(&test_program.keypair.pubkey(), account, lamports)
        ],
        Some(&test_program.keypair.pubkey())
    );
    tx.sign(&[&test_program.keypair], test_program.blockhash);
    assert_matches!(test_program.banks_client.process_transaction(tx).await, Ok(()));
}

pub async fn lamports_per_signature(test_program: &mut TestProgram) -> u64 {
    test_program.banks_client.get_fees().await.unwrap().0.lamports_per_signature
}

// Account getters
macro_rules! queue {
    ($id: ident, $ty: ty, $ty_account: ty, $prg: ident) => {
        let mut queue = get_data(&mut $prg, <$ty_account>::find(None).0).await;
        let mut queue = <$ty_account>::new(&mut queue[..]).unwrap();
        let $id = <$ty>::new(&mut queue);
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
        use elusiv::state::program_account::MultiAccountProgramAccount;

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

pub(crate) use queue;
pub(crate) use pda_account;
pub(crate) use account_info;
pub(crate) use storage_account;

/// Adds random nonce bytes at the end of the ix data (no problem since the program cuts the ix data off)
pub fn nonce_instruction(ix: Instruction) -> Instruction {
    let mut ix = ix;
    for _ in 0..8 {
        ix.data.push(rand::random());
    }
    ix
}

async fn generate_and_sign_tx(
    ixs: &[Instruction],
    payer: &Pubkey,
    signers: Vec<&Keypair>,
    test_program: &mut TestProgram,
) -> Transaction {
    let ixs: Vec<Instruction> = ixs.iter().map(|ix| nonce_instruction(ix.clone())).collect();
    let mut tx = Transaction::new_with_payer(&ixs, Some(payer));
    tx.sign(&signers, test_program.banks_client.get_latest_blockhash().await.unwrap());
    tx
}

// Succesful transactions
pub async fn tx_should_succeed(
    ixs: &[Instruction],
    payer: &Pubkey,
    signers: Vec<&Keypair>,
    test_program: &mut TestProgram,
) {
    let tx = generate_and_sign_tx(ixs, payer, signers, test_program).await;
    assert_matches!(test_program.banks_client.process_transaction(tx).await, Ok(()));
}

pub async fn ix_should_succeed(
    ix: Instruction,
    payer: &Pubkey,
    signers: Vec<&Keypair>,
    test_program: &mut TestProgram,
) {
    tx_should_succeed(&[ix], payer, signers, test_program).await
}

// Failing transactions
pub async fn tx_should_fail(
    ixs: &[Instruction],
    payer: &Pubkey,
    signers: Vec<&Keypair>,
    test_program: &mut TestProgram,
) {
    let tx = generate_and_sign_tx(ixs, payer, signers, test_program).await;
    assert_matches!(test_program.banks_client.process_transaction(tx).await, Err(_));
}

pub async fn ix_should_fail(
    ix: Instruction,
    payer: &Pubkey,
    signers: Vec<&Keypair>,
    test_program: &mut TestProgram,
) {
    tx_should_fail(&[ix], payer, signers, test_program).await
}

pub fn u256_from_str(str: &str) -> U256 {
    fr_to_u256_le(&Fr::from_str(str).unwrap())
}

pub fn base_commitment_request(bc: &str, c: &str, amount: u64, fee_version: u16) -> BaseCommitmentHashRequest {
    BaseCommitmentHashRequest { base_commitment: u256_from_str(bc), commitment: u256_from_str(c), amount, fee_version }
}