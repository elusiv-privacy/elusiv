use solana_program::{
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    native_token::LAMPORTS_PER_SOL,
    account_info::AccountInfo,
};
use super::state::*;
use super::instruction::ElusivInstruction;
use elusiv_account::account;

const MINIMUM_AMOUNT: u64 = LAMPORTS_PER_SOL / 10;

pub fn process(_program_id: &Pubkey, accounts: &[AccountInfo], instruction: ElusivInstruction) -> ProgramResult {
    use ElusivInstruction::*;

    let account_info_iter = &mut accounts.iter();

    match instruction {
        Store { proof_data, unbound_commitment } => {
            account!(Storage);

            store(storage_account)
        },
        Bind { proof_data, unbound_commitment, bound_commitment } => {
            bind()
        },
        Send { proof_data, recipient } => {
            send()
        },
        FinalizeSend => {
            finalize_send()
        },

        InitProof => {
            init_proof()
        },
        ComputeProof => {
            compute_proof()
        },
        FinalizeProof => {
            finalize_proof()
        },

        InitCommitment => {
            init_commitment()
        },
        ComputeCommitment => {
            compute_commitment()
        },
        FinalizeCommitment => {
            finalize_commitment()
        },
    }
}

fn store(
    storage_account: StorageAccount,
) -> ProgramResult {
    // Check public inputs
    // Add store request to queue
    // Transfer funds
    Ok(())
}

fn bind() -> ProgramResult {
    Ok(())
}

fn send() -> ProgramResult {
    Ok(())
}

fn finalize_send() -> ProgramResult {
    Ok(())
}

fn init_proof() -> ProgramResult {
    Ok(())
}

fn compute_proof() -> ProgramResult {
    Ok(())
}

fn finalize_proof() -> ProgramResult {
    Ok(())
}

fn init_commitment() -> ProgramResult {
    Ok(())
}

fn compute_commitment() -> ProgramResult {
    Ok(())
}

fn finalize_commitment() -> ProgramResult {
    Ok(())
}