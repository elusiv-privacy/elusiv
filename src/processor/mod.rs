mod store;
mod send;
mod merge;
mod commitment;
mod proof;
mod accounts;
mod utils;

use store::*;
use send::*;
use merge::*;
use commitment::*;
use proof::*;
use accounts::*;

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
use crate::macros::{ account, guard };

pub fn process(_program_id: &Pubkey, accounts: &[AccountInfo], instruction: ElusivInstruction) -> ProgramResult {
    use ElusivInstruction::*;

    let account_info_iter = &mut accounts.iter();

    match instruction {
        Store { commitment_core, amount, commitment } => {

            account!(sender, signer);
            account!(Storage);
            account!(Queue);
            account!(pool, pool);
            account!(system_program, no_check);

            store(
                sender,
                &storage_account,
                &mut queue_account,
                pool,
                system_program,
                commitment_core,
                amount,
                commitment,
            )
        },

        Merge { proof_data } => {

            account!(Storage);
            account!(Archive);
            account!(nullifier_account_a, nullifier);
            account!(nullifier_account_b, nullifier);
            account!(Queue);

            merge(
                &storage_account,
                [ &nullifier_account_a, &nullifier_account_b, ],
                &mut queue_account,
                proof_data,
                amount,
                recipient,
            )
        },

        Send { proof_data, amount, recipient  } => { 

            account!(Storage);
            account!(Archive);
            account!(nullifier_account_a, nullifier);
            account!(nullifier_account_b, nullifier);
            account!(Queue);

            send(
                &storage_account,
                [ &nullifier_account_a, &nullifier_account_b, ],
                &mut queue_account,
                proof_data,
                amount,
                recipient,
            )
        },
        FinalizeSend => {

            account!(Queue);
            account!(pool, pool);
            account!(recipient, no_check);

            finalize_send(&mut queue_account, &pool, &recipient)
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

            init_commitment(&storage_account, &mut queue_account, &mut commitment_account)
        },
        ComputeCommitment => {

            account!(Commitment);

            compute_commitment(&mut commitment_account)
        },
        FinalizeCommitment => {

            account!(Storage);
            account!(Commitment);

            finalize_commitment(&mut storage_account, &mut commitment_account)
        },

        InitStorage  { bump_seed } => {

            account!(Storage);
            account!(reserve, reserve);
            account!(pda_account, no_check);

            init_storage(&mut storage_account, &reserve, &pda_account, bump_seed)
        },
        ArchiveStorage => {

            account!(Storage);
            account!(Archive);
            account!(nullifier_account, nullifier);

            archive_storage(&mut storage_account, &mut archive_account, &mut nullifier_account)
        }
    }
}