use solana_program::{
    instruction::Instruction,
    instruction::AccountMeta,
    hash::Hash,
    pubkey::Pubkey,
    native_token::LAMPORTS_PER_SOL,
};
use solana_sdk::{
    signature::Signer,
    transaction::Transaction,
    signature::Keypair,
};
use elusiv::fields::scalar::*;
use elusiv::fields::utils::*;
use elusiv::groth16;
use super::accounts::*;
use super::proof::*;

pub const WITHDRAW_INSTRUCTIONS_COUNT: u64 = (groth16::ITERATIONS + 2) as u64;

pub async fn withdraw_transaction(payer: &Keypair, _recipient: Pubkey, recent_blockhash: Hash, data: Vec<u8>) -> Transaction {
    // Start withdrawal
    let mut instructions = Vec::new();
    instructions.push(Instruction {
        program_id: elusiv::id(),
        accounts: vec!
        [
            AccountMeta::new(program_account_id(), false),
            AccountMeta::new(withdraw_account_id(), false),
        ],
        data,
    });

    // Compute verification
    for _ in 0..groth16::ITERATIONS {
        instructions.push(Instruction {
            program_id: elusiv::id(),
            accounts: vec![ AccountMeta::new(withdraw_account_id(), false) ],
            data: vec![elusiv::instruction::COMPUTE_WITHDRAW],
        });
    }

    /*
    // Finalize deposit
    instructions.push(Instruction {
        program_id: elusiv::id(),
        accounts: vec!
        [
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(program_account_id(), false),
            AccountMeta::new(withdraw_account_id(), false),
            // TODO: Add relayer
            AccountMeta::new(recipient, true),
        ],
        data: vec![elusiv::Instruction::FINISH_WITHDRAW],
    });*/

    // Sign and send transaction
    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));
    transaction.sign(&[payer], recent_blockhash);
    transaction
}

pub fn withdraw_data(proof: &ProofString, inputs: &[&str]) -> Vec<u8> {
    let mut public_inputs = [[0; 32]; elusiv::instruction::PUBLIC_INPUTS_COUNT];
    for (i, input) in inputs.iter().enumerate() {
        public_inputs[i] = vec_to_array_32(to_bytes_le_mont(from_str_10(input)));
    }

    elusiv::instruction::generate_init_withdraw_data(
        Pubkey::new_unique(),
        LAMPORTS_PER_SOL,
        public_inputs,
        proof.generate_proof(),
    )
}