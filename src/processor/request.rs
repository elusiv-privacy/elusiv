use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    native_token::LAMPORTS_PER_SOL,
    clock::Clock,
    sysvar::Sysvar,
};
use crate::macros::guard;
use crate::state::{NullifierAccount, StorageAccount};
use crate::state::program_account::PDAAccount;
use crate::types::{JoinSplitPublicInputs, JoinSplitProofData};
use super::utils::{send_with_system_program, send_from_pool};
use crate::state::queue::{
    RingQueue,
    BaseCommitmentQueue,BaseCommitmentQueueAccount,BaseCommitmentHashRequest,
    SendProofQueue,SendProofQueueAccount,
    MergeProofQueue,MergeProofQueueAccount,
    MigrateProofQueue,MigrateProofQueueAccount,
    FinalizeSendQueue,FinalizeSendQueueAccount, ProofRequest, QueueManagementAccount,
};
use crate::error::ElusivError::{
    InvalidAmount,
    InvalidAccount,
    InvalidInstructionData,
    CommitmentAlreadyExists,
    InvalidFeePayer,
    InvalidTimestamp,
    InvalidRecipient,
    InvalidMerkleRoot,
    InvalidPublicInputs,
    NullifierAlreadyExists,
    NonScalarValue,
};
use crate::fields::{try_scalar_montgomery, u256_to_big_uint};

pub const MINIMUM_STORE_AMOUNT: u64 = LAMPORTS_PER_SOL / 10;
pub const MAXIMUM_STORE_AMOUNT: u64 = u64::MAX;

/// Enqueues a base commitment hash request and takes the funds from the sender
pub fn store<'a>(
    sender: &AccountInfo<'a>,
    pool: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    q_manager: &QueueManagementAccount,
    queue: &mut BaseCommitmentQueueAccount,

    request: BaseCommitmentHashRequest,
) -> ProgramResult {
    guard!(q_manager.get_finished_setup(), InvalidAccount);
    let mut queue = BaseCommitmentQueue::new(queue);

    // Check amount (zero amounts are allowed since the user may need multiple commitments for some proofs)
    guard!(request.amount >= super::MINIMUM_STORE_AMOUNT || request.amount == 0, InvalidAmount);
    guard!(request.amount <= super::MAXIMUM_STORE_AMOUNT, InvalidAmount);

    // Transfer funds + fees
    let fee = 0;
    let lamports = request.amount + fee;
    send_with_system_program(sender, pool, system_program, lamports)?;

    // Check that `base_commitment` and `commitment` are in the scalar field
    guard!(matches!(try_scalar_montgomery(u256_to_big_uint(&request.base_commitment)), Some(_)), NonScalarValue);
    guard!(matches!(try_scalar_montgomery(u256_to_big_uint(&request.commitment)), Some(_)), NonScalarValue);

    // Enqueue request
    guard!(!queue.contains(&request), CommitmentAlreadyExists);
    queue.enqueue(request)
}

const TIMESTAMP_BITS_PRUNING: usize = 5;

/// Enqueues a send proof and takes the computation fee from the relayer
pub fn request_proof_verification<'a, 'b, 'c, 'd>(
    fee_payer: &AccountInfo<'c>,
    pool: &AccountInfo<'c>,
    system_program: &AccountInfo<'c>,
    storage_account: &StorageAccount,
    nullifier_account0: &NullifierAccount<'a, 'b, 'd>,
    nullifier_account1: &NullifierAccount<'a, 'b, 'd>,
    queue: &AccountInfo,

    request: ProofRequest,
    tree_indices: [u64; 2],
) -> ProgramResult {
    let mut queue_data = &mut queue.data.borrow_mut()[..];

    match request {
        ProofRequest::Send { request } => {
            let mut queue = SendProofQueueAccount::new(&mut queue_data)?;
            let mut queue = SendProofQueue::new(&mut queue);

            // Verify public inputs
            check_join_split_public_inputs(
                &request.public_inputs.join_split,
                &request.proof_data,
                &storage_account,
                [&nullifier_account0, &nullifier_account1],
            )?;
            guard!(tree_indices[0] == request.proof_data.tree_indices[0] && tree_indices[0] == request.proof_data.tree_indices[0], InvalidInstructionData);
            guard!(request.fee_payer == fee_payer.key.to_bytes(), InvalidFeePayer);

            // Time stamp verification (we prune the last byte)
            let clock = Clock::get()?;
            let current_timestamp: u64 = clock.unix_timestamp.try_into().unwrap();
            let timestamp = request.public_inputs.timestamp >> TIMESTAMP_BITS_PRUNING;
            guard!(timestamp == current_timestamp >> TIMESTAMP_BITS_PRUNING, InvalidTimestamp);

            // Transfer funds + fees
            let fee = 0;
            send_with_system_program(fee_payer, pool, system_program, fee)?;

            // Enqueue request
            queue.enqueue(request)
        },

        ProofRequest::Merge { request } => {
            let mut queue = MergeProofQueueAccount::new(&mut queue_data)?;
            let mut queue = MergeProofQueue::new(&mut queue);

            // Verify public inputs
            check_join_split_public_inputs(
                &request.public_inputs.join_split,
                &request.proof_data,
                &storage_account,
                [&nullifier_account0, &nullifier_account1],
            )?;
            guard!(tree_indices[0] == request.proof_data.tree_indices[0] && tree_indices[0] == request.proof_data.tree_indices[0], InvalidInstructionData);
            guard!(request.fee_payer == fee_payer.key.to_bytes(), InvalidFeePayer);

            // Transfer funds + fees
            let fee = 0;
            send_with_system_program(fee_payer, pool, system_program, fee)?;

            // Enqueue request
            queue.enqueue(request)
        },

        ProofRequest::Migrate { request } => {
            let mut queue = MigrateProofQueueAccount::new(&mut queue_data)?;
            let mut queue = MigrateProofQueue::new(&mut queue);

            // Verify public inputs
            check_join_split_public_inputs(
                &request.public_inputs.join_split,
                &request.proof_data,
                &storage_account,
                [&nullifier_account0],
            )?;
            guard!(tree_indices[0] == request.proof_data.tree_indices[0], InvalidInstructionData);
            guard!(request.fee_payer == fee_payer.key.to_bytes(), InvalidFeePayer);

            // Transfer funds + fees
            let fee = 0;
            send_with_system_program(fee_payer, pool, system_program, fee)?;

            // Enqueue request
            queue.enqueue(request)
        }
    }
}

/// Transfers the funds of a send request to a recipient
pub fn finalize_send<'a>(
    recipient: &AccountInfo<'a>,
    pool: &AccountInfo<'a>,
    queue: &mut FinalizeSendQueueAccount,
) -> ProgramResult {
    let mut queue = FinalizeSendQueue::new(queue);
    let request = queue.dequeue_first()?;

    guard!(recipient.key.to_bytes() == request.request.recipient, InvalidRecipient);

    send_from_pool(pool, recipient, request.request.amount)
}

/// Verifies public inputs and the proof data for proof requests
pub fn check_join_split_public_inputs<const N: usize>(
    public_inputs: &JoinSplitPublicInputs<N>,
    proof_data: &JoinSplitProofData<N>,
    storage_account: &StorageAccount,
    nullifier_accounts: [&NullifierAccount; N],
    //commitment_queue_account: &CommitmentQueueAccount,
) -> ProgramResult {
    assert!(N <= 2);

    let uses_multiple_trees = N > 1 && proof_data.tree_indices[0] != proof_data.tree_indices[1];
    let active_tree_index = storage_account.get_trees_count();

    // Check that roots are the same if they represent the same tree
    guard!(!uses_multiple_trees || public_inputs.roots[0] == public_inputs.roots[1], InvalidMerkleRoot);

    // Check that roots are valid
    for i in 0..N {
        // For the active tree: root can either be the last root or any root from the active_mt_root_history
        if proof_data.tree_indices[i] == active_tree_index {
            guard!(storage_account.is_root_valid(public_inputs.roots[i]), InvalidMerkleRoot);
        } else { // For a non-active tree: root can only be one value
            guard!(public_inputs.roots[i] == nullifier_accounts[i].get_root(), InvalidMerkleRoot);
        }
    }

    // Check that nullifier_hashes for the same tree are different
    guard!(!uses_multiple_trees || public_inputs.nullifier_hashes[0] == public_inputs.nullifier_hashes[1], InvalidPublicInputs);

    // Check that nullifier_hashes can be inserted
    for i in 0..N {
        guard!(nullifier_accounts[i].can_insert_nullifier_hash(public_inputs.nullifier_hashes[i]), NullifierAlreadyExists);
    }

    Ok(())
}