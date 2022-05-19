use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    native_token::LAMPORTS_PER_SOL,
    clock::Clock,
    sysvar::Sysvar,
};
use crate::macros::guard;
use crate::state::{NullifierAccount, StorageAccount};
use super::utils::{check_join_split_public_inputs, send_with_system_program};
use crate::state::queue::{
    RingQueue,
    BaseCommitmentQueue,BaseCommitmentQueueAccount,BaseCommitmentHashRequest,
    SendProofQueue,SendProofQueueAccount,SendProofRequest,
};
use crate::error::ElusivError::{InvalidAmount, InvalidInstructionData, CommitmentAlreadyExists, InvalidFeePayer, InvalidTimestamp};

pub const MINIMUM_STORE_AMOUNT: u64 = LAMPORTS_PER_SOL / 10;
pub const MAXIMUM_STORE_AMOUNT: u64 = u64::MAX;

/// Enqueues a base commitment hash request and takes the funds from the sender
pub fn store<'a>(
    sender: &AccountInfo<'a>,
    pool: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    queue: &mut BaseCommitmentQueueAccount,

    request: BaseCommitmentHashRequest,
) -> ProgramResult {
    let mut queue = BaseCommitmentQueue::new(queue);

    // Check amount (zero amounts are allowed since the user may need multiple commitments for some proofs)
    guard!(request.amount >= super::MINIMUM_STORE_AMOUNT || request.amount == 0, InvalidAmount);
    guard!(request.amount <= super::MAXIMUM_STORE_AMOUNT, InvalidAmount);

    // Transfer funds + fees
    let fee = 0;
    let lamports = request.amount + fee;
    send_with_system_program(sender, pool, system_program, lamports)?;

    // Enqueue request
    guard!(!request.is_active, InvalidInstructionData);
    guard!(!queue.contains(&request), CommitmentAlreadyExists);
    queue.enqueue(request)
}

const TIMESTAMP_PRUNING: usize = 4;

/// Enqueues a send proof and takes the computation fee from the relayer
pub fn send<'a, 'b>(
    relayer: &AccountInfo,
    pool: &AccountInfo,
    system_program: &AccountInfo,
    storage_account: &StorageAccount,
    nullifier_account0: &NullifierAccount<'a, 'b>,
    nullifier_account1: &NullifierAccount<'a, 'b>,
    queue: &mut SendProofQueueAccount,

    request: SendProofRequest,
) -> ProgramResult {
    let mut queue = SendProofQueue::new(queue);

    // Verify public inputs
    check_join_split_public_inputs(
        &request.public_inputs.join_split,
        &request.proof_data,
        &storage_account,
        [&nullifier_account0, &nullifier_account1],
    )?;
    guard!(request.fee_payer == relayer.key.to_bytes(), InvalidFeePayer);

    // Time stamp verification (we prune the last byte)
    let clock = Clock::get()?;
    let current_timestamp: u64 = clock.unix_timestamp.try_into().unwrap();
    let timestamp = request.public_inputs.timestamp >> TIMESTAMP_PRUNING;
    guard!(timestamp == current_timestamp >> TIMESTAMP_PRUNING, InvalidTimestamp);

    // Transfer funds + fees
    let fee = 0;
    //send_with_system_program(relayer, pool, system_program, fee)?;

    // Enqueue request
    guard!(!request.is_active, InvalidInstructionData);
    queue.enqueue(request)
}

pub fn merge() -> ProgramResult {
    Ok(())
}

pub fn migrate() -> ProgramResult {
    Ok(())
}