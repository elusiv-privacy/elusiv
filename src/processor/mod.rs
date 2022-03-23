mod client;
mod node;

use client::*;
use node::*;

use solana_program::{
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    account_info::AccountInfo,
};
use super::state::*;
use super::queue::state::*;
use super::instruction::ElusivInstruction;
use elusiv_account::account;

pub fn process(_program_id: &Pubkey, accounts: &[AccountInfo], instruction: ElusivInstruction) -> ProgramResult {
    use ElusivInstruction::*;

    let account_info_iter = &mut accounts.iter();

    match instruction {
        Store { proof_data, unbound_commitment } => {
            account!(sender, signer);
            account!(Storage);
            account!(Queue);
            account!(pool, pool);
            account!(system_program, no_check);

            store(
                sender,
                storage_account,
                &mut queue_account,
                pool,
                system_program,
                proof_data,
                unbound_commitment
            )
        },
        Bind { proof_data, unbound_commitment, bound_commitment } => {
            account!(sender, signer);
            account!(Storage);
            account!(Queue);
            account!(pool, pool);
            account!(system_program, no_check);

            bind(
                sender,
                storage_account,
                &mut queue_account,
                pool,
                system_program,
                proof_data,
                [
                    unbound_commitment,
                    bound_commitment,
                ]
            )
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