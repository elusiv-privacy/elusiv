mod transfer;
mod commitment;
mod proof;

use transfer::*;
use commitment::*;
use proof::*;

use solana_program::{
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    account_info::AccountInfo,
};
use super::state::*;
use super::queue::state::*;
use super::proof::state::*;
use super::commitment::state::*;
use super::instruction::ElusivInstruction;
use elusiv_account::account;

pub fn process(_program_id: &Pubkey, accounts: &[AccountInfo], instruction: ElusivInstruction) -> ProgramResult {
    use ElusivInstruction::*;

    let account_info_iter = &mut accounts.iter();

    match instruction {
        Store { proof_data, unbound_commitment } => {

            account!(sender, signer);
            account!(Storage);
            account!(Archive);
            account!(nullifier_account, nullifier);
            account!(Queue);
            account!(pool, pool);
            account!(system_program, no_check);

            store(
                sender,
                &storage_account,
                &nullifier_account,
                &mut queue_account,
                pool,
                system_program,
                proof_data,
                vec![unbound_commitment]
            )
        },
        Bind { proof_data, unbound_commitment, bound_commitment } => {

            account!(sender, signer);
            account!(Storage);
            account!(Archive);
            account!(nullifier_account, nullifier);
            account!(Queue);
            account!(pool, pool);
            account!(system_program, no_check);

            store(
                sender,
                &storage_account,
                &nullifier_account,
                &mut queue_account,
                pool,
                system_program,
                proof_data,
                vec![
                    unbound_commitment,
                    bound_commitment,
                ]
            )
        },

        Send { proof_data, recipient } => {

            account!(Storage);
            account!(Archive);
            account!(nullifier_account, nullifier);
            account!(Queue);

            send(
                &storage_account,
                &nullifier_account,
                &mut queue_account,
                proof_data,
                recipient,
            )
        },
        FinalizeSend => {

            account!(Queue);
            account!(pool, pool);
            account!(recipient, no_check);

            finalize_send( &mut queue_account, &pool, &recipient )
        },

        InitProof => {

            account!(Queue);
            account!(Proof);

            init_proof(&mut queue_account, &mut proof_account)
        },
        ComputeProof => {

            account!(Proof);

            compute_proof(&mut proof_account)
        },
        FinalizeProof => {

            account!(Storage);
            account!(Archive);
            account!(nullifier_account, nullifier);
            account!(Queue);
            account!(Proof);

            finalize_proof(
                &mut storage_account,
                &mut nullifier_account,
                &mut queue_account,
                &mut proof_account
            )
        },

        InitCommitment => {

            account!(Storage);
            account!(Queue);
            account!(Commitment);

            init_commitment( &storage_account, &mut queue_account, &mut commitment_account )
        },
        ComputeCommitment => {

            account!(Commitment);

            compute_commitment( &mut commitment_account )
        },
        FinalizeCommitment => {

            account!(Storage);
            account!(Commitment);

            finalize_commitment( &mut storage_account, &mut commitment_account )
        },
    }
}