use solana_program::{
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    native_token::LAMPORTS_PER_SOL,
    account_info::AccountInfo,
};
use crate::macros::guard;
use crate::types::{ ProofDataBinary, U256 };
use crate::error::ElusivError::{
    InvalidAmount,
    InvalidAccount,
    InvalidRecipient,
};
use crate::state::*;
use crate::queue::state::*;
use crate::queue::proof_request::{ProofRequest, ProofRequestKind};

/// Enqueues a merge proof that should be verified
pub fn merges(
    storage_account: &StorageAccount,
    nullifier_accounts: [&NullifierAccount; 2],
    queue_account: &mut QueueAccount,
    proof_data: ProofDataBinary,
) -> ProgramResult {
    Ok(())
}