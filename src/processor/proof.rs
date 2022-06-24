use ark_ff::BigInteger256;
use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    clock::Clock,
    sysvar::Sysvar,
};
use crate::macros::{guard, BorshSerDeSized};
use crate::processor::CommitmentHashRequest;
use crate::state::governor;
use crate::state::program_account::PDAAccount;
use crate::state::{
    NullifierAccount,
    StorageAccount,
    program_account::ProgramAccount,
    fee::FeeAccount,
    governor::GovernorAccount,
};
use crate::state::queue::{
    RingQueue,
    Queue,
    CommitmentQueue,CommitmentQueueAccount,
};
use crate::error::ElusivError::{
    InvalidAccount,
    InvalidMerkleRoot,
    InvalidPublicInputs,
    InvalidRecipient,
    InvalidInstructionData,
    ComputationIsNotYetFinished,
    InvalidFeePayer,
    NullifierAlreadyExists,
    InvalidTimestamp,
    InvalidFeeVersion,
    MerkleTreeIsNotInitialized,
};
use crate::proof::{
    VerificationAccount,
    //verifier::verify_partial,
    vkey::{
        SendBinaryVKey,
        MergeBinaryVKey,
        MigrateUnaryVKey,
    },
};
use crate::types::{RawProof, JoinSplitProofData, SendPublicInputs, MergePublicInputs, MigratePublicInputs, PublicInputs, JoinSplitPublicInputs};
use crate::bytes::BorshSerDeSized;
use super::utils::{send_from_pool, close_account};
use borsh::{BorshSerialize, BorshDeserialize};

macro_rules! execute_with_vkey {
    ($request: ident, $vk: ident, $b: block) => {
        match $request {
            ProofRequest::Send { .. } => {
                type $vk = SendBinaryVKey; $b
            }
            ProofRequest::Merge { .. } => {
                type $vk = MergeBinaryVKey; $b
            }
            ProofRequest::Migrate { .. } => {
                type $vk = MigrateUnaryVKey; $b
            }
        }
    };
}

#[derive(BorshSerialize, BorshDeserialize, BorshSerDeSized)]
pub enum ProofRequest {
    Send { request: SendProofRequest },
    Merge { request: MergeProofRequest },
    Migrate{ request: MigrateProofRequest }
}

impl ProofRequest {
    pub fn raw_proof(&self) -> RawProof {
        match self {
            Self::Send { request } => request.proof_data.proof,
            Self::Merge { request } => request.proof_data.proof,
            Self::Migrate { request } => request.proof_data.proof,
        }
    }

    pub fn fee_version(&self) -> u64 {
        panic!()
    }

    pub fn batching_rate(&self) -> u32 {
        panic!()
    }

    pub fn public_inputs(&self) -> Vec<BigInteger256> {
        match self {
            Self::Send { request } => request.public_inputs.public_inputs_big_integer(),
            Self::Merge { request } => request.public_inputs.public_inputs_big_integer(),
            Self::Migrate { request } => request.public_inputs.public_inputs_big_integer(),
        }
    }
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone)]
/// Sending funds from a private balance to a recipient pubkey (on Ed25519)
pub struct SendProofRequest {
    pub proof_data: JoinSplitProofData<2>,
    pub public_inputs: SendPublicInputs,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone)]
/// Merging 
pub struct MergeProofRequest {
    pub proof_data: JoinSplitProofData<2>,
    pub public_inputs: MergePublicInputs,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone)]
pub struct MigrateProofRequest {
    pub proof_data: JoinSplitProofData<1>,
    pub public_inputs: MigratePublicInputs,
}

/// Due to imprecision of the chain clock we cut off the last bits of the timestamp
const TIMESTAMP_BITS_PRUNING: usize = 5;

#[allow(clippy::too_many_arguments)]
pub fn init_proof<'a, 'b, 'c, 'd>(
    _fee_payer: &AccountInfo<'c>,
    _fee: &FeeAccount,
    governor: &GovernorAccount,
    _pool: &AccountInfo<'c>,
    _fee_collector: &AccountInfo<'c>,
    verification_account: &AccountInfo<'c>,
    storage_account: &StorageAccount,
    nullifier_account0: &NullifierAccount<'a, 'b, 'd>,
    nullifier_account1: &NullifierAccount<'a, 'b, 'd>,
    _system_program: &AccountInfo<'c>,

    verification_account_index: u64,
    request: ProofRequest,
    _ignore_duplicate_verifications: bool,
    tree_indices: [u64; 2],
) -> ProgramResult {
    /*guard!(*verification_account.key == VerificationAccount::find(Some(verification_account_index)).0, InvalidAccount);
    guard!(request.fee_version() == governor.get_fee_version(), InvalidFeeVersion);
    guard!(storage_account.get_initialized(), MerkleTreeIsNotInitialized);
    guard!(nullifier_account0.get_initialized(), MerkleTreeIsNotInitialized);
    guard!(nullifier_account1.get_initialized(), MerkleTreeIsNotInitialized);

    let clock = Clock::get()?;
    let current_timestamp: u64 = clock.unix_timestamp.try_into().unwrap();

    // Verify public inputs
    match request {
        ProofRequest::Send { request } => {
            check_join_split_public_inputs(
                &request.public_inputs.join_split,
                &request.proof_data,
                storage_account,
                [nullifier_account0, nullifier_account1],
            )?;
            guard!(tree_indices[0] == request.proof_data.tree_indices[0], InvalidInstructionData);
            guard!(tree_indices[1] == request.proof_data.tree_indices[1], InvalidInstructionData);

            // Time stamp verification (we prune the last byte)
            let timestamp = request.public_inputs.timestamp >> TIMESTAMP_BITS_PRUNING;
            guard!(timestamp == current_timestamp >> TIMESTAMP_BITS_PRUNING, InvalidTimestamp);
        }

        ProofRequest::Merge { request } => {
            check_join_split_public_inputs(
                &request.public_inputs.join_split,
                &request.proof_data,
                storage_account,
                [nullifier_account0, nullifier_account1],
            )?;
            guard!(tree_indices[0] == request.proof_data.tree_indices[0], InvalidInstructionData);
            guard!(tree_indices[1] == request.proof_data.tree_indices[1], InvalidInstructionData);
        }

        ProofRequest::Migrate { request } => {
            check_join_split_public_inputs(
                &request.public_inputs.join_split,
                &request.proof_data,
                storage_account,
                [nullifier_account0],
            )?;
            guard!(tree_indices[0] == request.proof_data.tree_indices[0], InvalidInstructionData);
        }
    }*/

    // Check for expected nullifier duplicates
    // - we need to allow for two verifications having the same nullifiers, since a bad relayer could attempt block users from accessing their funds
    // - but every request receives a short time-frame in which it can be completed with a guarantee of no duplicates

    // Also: Only storage account PDA required here
    todo!();

    // fee_payer rents verification_account
    /*open_pda_account_with_offset::<VerificationAccount>(
        fee_payer,
        verification_account,
        verification_account_index
    )?;

    let public_inputs = request.public_inputs();

    let compensation_fee = execute_with_vkey!(request, VKey, {
        fee.proof_fee_payer_fee::<VKey>(&public_inputs)
    });
    let network_fee = fee.get_proof_network_fee();
    send_with_system_program(
        fee_payer,
        pool,
        system_program,
        compensation_fee - network_fee
    )?;
    send_with_system_program(
        fee_payer,
        fee_collector,
        system_program,
        network_fee
    )?;

    let mut data = &verification_account.data.borrow_mut()[..];
    let mut verification_account = VerificationAccount::new(&mut data)?;

    verification_account.reset(
        &public_inputs,
        request,
        fee_payer.key.to_bytes(),
    )*/
}

/// Partial proof verification computation
pub fn compute_proof<'a>(
    _fee_payer: &AccountInfo<'a>,
    _fee: &FeeAccount,
    _pool: &AccountInfo<'a>,
    _verification_account: &mut VerificationAccount,

    _verification_account_index: u64,
    _fee_version: u64,
    _nonce: u64,
) -> ProgramResult {
    panic!("Computation missing");
    /*guard!(verification_account.get_is_active(), ComputationIsNotYetFinished);
    guard!(verification_account.get_fee_version() == fee_version, InvalidFeeVersion);

    let instruction = verification_account.get_instruction();
    let start_round = 0;
    let rounds = 0;*/

    /*let request = verification_account.get_request();

    match execute_with_vkey!(request, VKey, {
        verify_partial::<VKey>(
            start_round as usize,
            rounds as usize,
            verification_account
        )
    }) {
        Ok(result) => match result {
            Some(final_result) => { // After last round we receive the verification result
                if final_result {
                    verification_account.set_is_verified(&true);
                } else {
                    verification_account.set_is_active(&false);
                }
            },
            None => {}
        },
        Err(e) => { // An error can only happen with flawed inputs -> cancel verification
            verification_account.set_is_active(&false);
            return Err(e.into());
        }
    }

    verification_account.serialize_lazy_fields();
    verification_account.set_instruction(&(instruction + 1));

    send_from_pool(pool, fee_payer, fee.proof_tx_compensation())*/
}

/// Proof finalization
/// - enqueue commitment, save nullifier-hashes, reward the original_fee_payer, and
/// - for Send: send amount to recipient
/// - for Migrate: update N-SMT-root
#[allow(clippy::too_many_arguments)]
pub fn finalize_proof<'a>(
    original_fee_payer: &AccountInfo<'a>,
    recipient: &AccountInfo<'a>, // can be any account for non-send-proofs
    fee: &FeeAccount,
    pool: &AccountInfo<'a>,
    fee_collector: &AccountInfo<'a>,
    verification_account_info: &AccountInfo<'a>,
    commitment_hash_queue: &mut CommitmentQueueAccount,
    nullifier_account0: &mut NullifierAccount,
    nullifier_account1: &mut NullifierAccount,

    _verification_account_index: u64,
    fee_version: u64,
    tree_indices: [u64; 2],
) -> ProgramResult {
    todo!("Resulting zero commitment is not allowed");
    /*let data = &mut verification_account_info.data.borrow_mut()[..];
    let verification_account = VerificationAccount::new(data)?;

    guard!(verification_account.get_fee_version() == fee_version, InvalidFeeVersion);
    guard!(verification_account.get_is_active(), ComputationIsNotYetFinished);
    guard!(original_fee_payer.key.to_bytes() == verification_account.get_fee_payer(), InvalidFeePayer);

    // If the proof is invalid, the verification_account closing funds flow to the fee_collector as slashing
    if !verification_account.get_is_verified() {
        close_account(fee_collector, verification_account_info)?;
        if cfg!(extended_logging) {
            solana_program::msg!(
                "Invalid proof, fee_payer {:?} is getting slashed",
                original_fee_payer.key
            );
        }
        return Ok(())
    }

    let mut commitment_queue = CommitmentQueue::new(commitment_hash_queue);
    let request = verification_account.get_request();
    let fee_version = verification_account.get_fee_version();
    let public_inputs = request.public_inputs();
    let batching_rate = 
    let proof_verification_fee = execute_with_vkey!(request, VKey, {
        fee.proof_verification_fee::<VKey>(&public_inputs, )
    });

    match request {
        ProofRequest::Send { request } => {
            guard!(tree_indices[0] == request.proof_data.tree_indices[0], InvalidAccount);
            guard!(tree_indices[1] == request.proof_data.tree_indices[1], InvalidAccount);
            nullifier_account0.insert_nullifier_hash(request.public_inputs.join_split.nullifier_hashes[0])?;
            nullifier_account1.insert_nullifier_hash(request.public_inputs.join_split.nullifier_hashes[1])?;

            commitment_queue.enqueue(
                CommitmentHashRequest {
                    commitment: request.public_inputs.join_split.commitment,
                    fee_version
                }
            )?;

            // Send amount - fees to the recipient
            guard!(recipient.key.to_bytes() == request.public_inputs.recipient, InvalidRecipient);
            send_from_pool(
                pool,
                recipient,
                request.public_inputs.amount - proof_verification_fee
            )?;
        }

        ProofRequest::Merge { request } => {
            guard!(tree_indices[0] == request.proof_data.tree_indices[0], InvalidAccount);
            guard!(tree_indices[1] == request.proof_data.tree_indices[1], InvalidAccount);
            nullifier_account0.insert_nullifier_hash(request.public_inputs.join_split.nullifier_hashes[0])?;
            nullifier_account1.insert_nullifier_hash(request.public_inputs.join_split.nullifier_hashes[1])?;

            commitment_queue.enqueue(
                CommitmentHashRequest {
                    commitment: request.public_inputs.join_split.commitment,
                    fee_version
                }
            )?;
        }

        ProofRequest::Migrate { request } => {
            guard!(tree_indices[0] == request.proof_data.tree_indices[0], InvalidAccount);
            nullifier_account0.insert_nullifier_hash(request.public_inputs.join_split.nullifier_hashes[0])?;

            commitment_queue.enqueue(
                CommitmentHashRequest {
                    commitment: request.public_inputs.join_split.commitment,
                    fee_version
                }
            )?;

            todo!("NSTM archivation system not implemented")
        }
    }

    if cfg!(extended_logging) {
        solana_program::msg!(
            "Valid proof, fee_payer {:?} is getting rewarded",
            original_fee_payer.key
        );
    }

    // Repay and reward relayer
    send_from_pool(
        pool,
        original_fee_payer,
        proof_verification_fee - fee.get_proof_network_fee()
    )?;

    // Close verification account
    close_account(original_fee_payer, verification_account_info)*/
    panic!()
}

/// Verifies public inputs and the proof data for proof requests
pub fn check_join_split_public_inputs<const N: usize>(
    public_inputs: &JoinSplitPublicInputs<N>,
    proof_data: &JoinSplitProofData<N>,
    storage_account: &StorageAccount,
    nullifier_accounts: [&NullifierAccount; N],
) -> ProgramResult {
    assert!(N <= 2);

    let uses_multiple_trees = N > 1 && proof_data.tree_indices[0] != proof_data.tree_indices[1];
    let active_tree_index = storage_account.get_trees_count();

    // Check that roots are the same if they represent the same tree
    guard!(!uses_multiple_trees || public_inputs.roots[0] == public_inputs.roots[1], InvalidMerkleRoot);

    // Check that roots are valid
    for (i, nullifier_account) in nullifier_accounts.iter().enumerate() {
        // For the active tree: root can either be the last root or any root from the active_mt_root_history
        if proof_data.tree_indices[i] == active_tree_index {
            guard!(storage_account.is_root_valid(public_inputs.roots[i]), InvalidMerkleRoot);
        } else { // For a non-active tree: root can only be one value
            guard!(public_inputs.roots[i] == nullifier_account.get_root(), InvalidMerkleRoot);
        }

        // Check that nullifier_hashes can be inserted
        guard!(nullifier_account.can_insert_nullifier_hash(public_inputs.nullifier_hashes[i]), NullifierAlreadyExists);
    }

    // Check that nullifier_hashes for the same tree are different
    guard!(!uses_multiple_trees || public_inputs.nullifier_hashes[0] == public_inputs.nullifier_hashes[1], InvalidPublicInputs);

    Ok(())
}