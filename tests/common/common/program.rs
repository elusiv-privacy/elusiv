use solana_program::hash::Hash;
use solana_program_test::*;
use solana_sdk::signature::Keypair;
use elusiv::entrypoint::process_instruction;

pub async fn start_program_solana_program_test() -> (solana_program_test::BanksClient, Keypair, Hash) {
    let mut test = ProgramTest::default();
    let program_id = elusiv::id();
    test.add_program("elusiv", program_id, processor!(process_instruction));
    test.start().await
}

use std::{error, path::PathBuf, str::FromStr};
use gadgets_scfs::{ScfsCriteria, ScfsMatrix, SCFS_DEVNET};
use solana_client::rpc_client::RpcClient;
use solana_program::{instruction::Instruction, message::Message, pubkey::Pubkey};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::Transaction,
};
use solana_test_validator::{ TestValidator, TestValidatorGenesis };

const LEDGER_PATH: &str = "./.ledger";
const PROG_PATH: &str = "dist/";
const PROG_NAME: &str = "elusiv";
const PROG_KEY: Pubkey = elusiv::id();
const TXWIDE_LIMITS: Pubkey = pubkey!("5ekBxc8itEnPv4NzGJtr8BVVQLNMQuLMNQQj7pHoLNZ9");

/// Setup the test validator passing features
/// you want to deactivate before running transactions
pub fn setup_validator(
    invalidate_features: Vec<Pubkey>,
) -> Result<(TestValidator, Keypair), Box<dyn error::Error>> {
    // Extend environment variable to include our program location
    std::env::set_var("BPF_OUT_DIR", PROG_PATH);

    // Instantiate the test validator
    let mut test_validator = TestValidatorGenesis::default();

    // Once instantiated, TestValidatorGenesis configuration functions follow
    // a builder pattern enabling chaining of settings function calls
    let (test_validator, kp) = test_validator
        .ledger_path(LEDGER_PATH)
        .add_program(PROG_NAME, PROG_KEY)
        .deactivate_features(&invalidate_features)
        .start();

    Ok((test_validator, kp))
}

/// Convenience function to remove existing ledger before TestValidatorGenesis setup
/// maps to `solana-test-validator ... --reset`
pub fn clean_ledger_setup_validator(
    invalidate_features: Vec<Pubkey>,
) -> Result<(TestValidator, Keypair), Box<dyn error::Error>> {
    if PathBuf::from_str(LEDGER_PATH).unwrap().exists() {
        std::fs::remove_dir_all(LEDGER_PATH).unwrap();
    }
    setup_validator(invalidate_features)
}

/// Submits a transaction with programs instruction
fn submit_transaction(
    rpc_client: &RpcClient,
    wallet_signer: &dyn Signer,
    instructions: Vec<Instruction>,
) -> Result<Signature, Box<dyn std::error::Error>> {
    let mut transaction = Transaction::new_unsigned(Message::new(&instructions, Some(&wallet_signer.pubkey())));

    let recent_blockhash = rpc_client
        .get_latest_blockhash()
        .map_err(|err| format!("error: unable to get recent blockhash: {}", err))?;

    transaction
        .try_sign(&vec![wallet_signer], recent_blockhash)
        .map_err(|err| format!("error: failed to sign transaction: {}", err))?;

    let signature = rpc_client
        .send_and_confirm_transaction(&transaction)
        .map_err(|err| format!("error: send transaction: {}", err))?;

    Ok(signature)
}