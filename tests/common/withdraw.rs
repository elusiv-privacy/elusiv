use {
    solana_program::{
        instruction::Instruction,
        instruction::AccountMeta,
        hash::Hash,
        pubkey::Pubkey,
        native_token::LAMPORTS_PER_SOL,
    },
    solana_sdk::{
        signature::Signer,
        transaction::Transaction,
        signature::Keypair,
    },
    ark_ff::*,

    elusiv::scalar::*,
    elusiv::groth16,

    super::accounts::*,
    super::proof::*,
};

pub async fn withdraw_transaction(payer: &Keypair, recipient: Pubkey, recent_blockhash: Hash, data: Vec<u8>) -> Transaction {
    // Start withdrawal
    let mut instructions = Vec::new();
    instructions.push(Instruction {
        program_id: elusiv::id(),
        accounts: vec!
        [
            AccountMeta::new(program_account_id(), false),
            AccountMeta::new(withdraw_account_id(), false),
        ],
        data: data,
    });

    // Compute verification
    for _ in 0..groth16::ITERATIONS {
        instructions.push(Instruction {
            program_id: elusiv::id(),
            accounts: vec![ AccountMeta::new(withdraw_account_id(), false) ],
            data: vec![4],
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
            AccountMeta::new(recipient, true),
        ],
        data: vec![5],
    });*/

    // Sign and send transaction
    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));
    transaction.sign(&[payer], recent_blockhash);
    transaction
}

pub fn withdraw_data(proof: ProofString, inputs: &[&str]) -> Vec<u8> {
    let mut data = vec![3];

    let amount: u64 = LAMPORTS_PER_SOL;
    data.extend(amount.to_le_bytes());

    for input in inputs {
        data.extend(to_bytes_le_mont(from_str_10(input)));
    }

    proof.push_to_vec(&mut data);

    data
}