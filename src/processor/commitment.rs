use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    native_token::LAMPORTS_PER_SOL,
};
use crate::commitment::poseidon_hash::TOTAL_POSEIDON_ROUNDS;
use crate::fee::{FeeAccount, CURRENT_FEE_VERSION};
use crate::macros::guard;
use crate::processor::{open_pda_account_with_offset, close_account};
use crate::state::{MT_HEIGHT, StorageAccount, program_account::ProgramAccount};
use crate::types::U256;
use super::utils::{send_with_system_program, send_from_pool};
use crate::state::queue::{
    RingQueue,
    Queue,
    CommitmentQueue, CommitmentQueueAccount, BaseCommitmentQueueAccount, BaseCommitmentQueue,
};
use crate::error::ElusivError::{
    InvalidAmount,
    InvalidAccount,
    ComputationIsNotYetFinished,
    ComputationIsAlreadyFinished,
    NonScalarValue,
    NoRoomForCommitment,
    InvalidFeeVersion,
};
use crate::fields::{try_scalar_montgomery, u256_to_big_uint};
use crate::commitment::{
    BaseCommitmentHashingAccount,
    CommitmentHashingAccount,
    poseidon_hash::{binary_poseidon_hash_partial},
    BaseCommitmentHashComputation,
    CommitmentHashComputation,
};
use elusiv_computation::{PartialComputation};
use crate::fields::{u256_to_fr, fr_to_u256_le};
use borsh::{BorshDeserialize, BorshSerialize};
use crate::bytes::BorshSerDeSized;
use crate::macros::BorshSerDeSized;
use ark_bn254::Fr;
use ark_ff::Zero;

/// Ensures that a `PartialComputation` is finished
macro_rules! partial_computation_is_finished {
    ($computation: ty, $account: ident) => {
        guard!(
            $account.get_instruction() as usize == <$computation>::INSTRUCTIONS.len(),
            ComputationIsNotYetFinished
        );
    };
}

macro_rules! partial_computation_is_not_finished {
    ($computation: ty, $account: ident) => {
        guard!(
            ($account.get_instruction() as usize) < (<$computation>::INSTRUCTIONS.len()),
            ComputationIsAlreadyFinished
        );
    };
}

pub const MIN_STORE_AMOUNT: u64 = LAMPORTS_PER_SOL / 10;
pub const MAX_STORE_AMOUNT: u64 = u64::MAX / 100;

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Debug)]
pub struct BaseCommitmentHashRequest {
    pub base_commitment: U256,
    pub amount: u64,
    pub commitment: U256,
    pub fee_version: u16,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Debug)]
pub struct CommitmentHashRequest {
    pub commitment: U256,
    pub fee_version: u16,
}

/// Stores a base commitment hash and takes the funds from the sender
/// - computation: `commitment = h(base_commitment, amount)` (https://github.com/elusiv-privacy/circuits/blob/master/circuits/commitment.circom)
pub fn store_base_commitment<'a>(
    sender: &AccountInfo<'a>,
    fee: &FeeAccount,
    pool: &AccountInfo<'a>,
    fee_collector: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    base_commitment_queue: &mut BaseCommitmentQueueAccount,

    fee_version: u64,
    request: BaseCommitmentHashRequest,
) -> ProgramResult {
    guard!(request.amount >= MIN_STORE_AMOUNT, InvalidAmount);
    guard!(request.amount <= MAX_STORE_AMOUNT, InvalidAmount);
    guard!(matches!(try_scalar_montgomery(u256_to_big_uint(&request.base_commitment)), Some(_)), NonScalarValue);
    guard!(matches!(try_scalar_montgomery(u256_to_big_uint(&request.commitment)), Some(_)), NonScalarValue);
    guard!(fee_version == request.fee_version as u64, InvalidFeeVersion);
    guard!(fee_version == CURRENT_FEE_VERSION as u64, InvalidFeeVersion);

    // Take amount + fee from sender
    let compensation_fee = fee.base_commitment_hash_fee();
    let network_fee = fee.get_base_commitment_network_fee();
    guard!(sender.lamports() >= compensation_fee + request.amount, InvalidAmount);
    send_with_system_program(
        sender,
        pool,
        system_program,
        request.amount + compensation_fee - network_fee
    )?;
    send_with_system_program(
        sender,
        fee_collector,
        system_program,
        network_fee
    )?;

    let mut queue = BaseCommitmentQueue::new(base_commitment_queue);
    queue.enqueue(request)
}

/// Initialized a base commitment hash by opening a BaseCommitmentHashingAccount
pub fn init_base_commitment_hash<'a>(
    fee_payer: &AccountInfo<'a>,
    base_commitment_queue: &mut BaseCommitmentQueueAccount,
    hashing_account: &AccountInfo<'a>,

    hash_account_index: u64,
) -> ProgramResult {
    // fee_payer rents hashing_account
    open_pda_account_with_offset::<BaseCommitmentHashingAccount>(fee_payer, hashing_account, hash_account_index)?;

    let mut queue = BaseCommitmentQueue::new(base_commitment_queue);
    let request = queue.dequeue_first()?;

    // Hashing account setup
    let mut data = &mut hashing_account.data.borrow_mut()[..];
    let mut hashing_account = BaseCommitmentHashingAccount::new(&mut data)?;
    hashing_account.reset(request, fee_payer.key.to_bytes())
}

pub fn compute_base_commitment_hash<'a>(
    fee_payer: &AccountInfo<'a>,
    fee: &FeeAccount,
    pool: &AccountInfo<'a>,
    hashing_account: &mut BaseCommitmentHashingAccount,

    _hash_account_index: u64,
    fee_version: u64,
    _nonce: u64,
) -> ProgramResult {
    guard!(hashing_account.get_is_active(), ComputationIsNotYetFinished);
    guard!(hashing_account.get_fee_version() as u64 == fee_version, InvalidFeeVersion);
    partial_computation_is_not_finished!(BaseCommitmentHashComputation, hashing_account);

    let instruction = hashing_account.get_instruction();
    let start_round = BaseCommitmentHashComputation::INSTRUCTIONS[instruction as usize].start_round;
    let rounds = BaseCommitmentHashComputation::INSTRUCTIONS[instruction as usize].rounds;

    let mut state = [
        u256_to_fr(&hashing_account.get_state(0)),
        u256_to_fr(&hashing_account.get_state(1)),
        u256_to_fr(&hashing_account.get_state(2)),
    ];

    for round in start_round..start_round + rounds {
        guard!(round < BaseCommitmentHashComputation::TOTAL_ROUNDS, ComputationIsAlreadyFinished);
        binary_poseidon_hash_partial(round, &mut state);
    }

    hashing_account.set_state(0, &fr_to_u256_le(&state[0]));
    hashing_account.set_state(1, &fr_to_u256_le(&state[1]));
    hashing_account.set_state(2, &fr_to_u256_le(&state[2]));

    hashing_account.set_instruction(&(instruction + 1));

    send_from_pool(pool, fee_payer, fee.hash_tx_compensation())
}

pub fn finalize_base_commitment_hash<'a>(
    original_fee_payer: &AccountInfo<'a>,
    commitment_hash_queue: &mut CommitmentQueueAccount,
    hashing_account_info: &AccountInfo<'a>,

    _hash_account_index: u64,
) -> ProgramResult {
    let mut data = &mut hashing_account_info.data.borrow_mut()[..];
    let hashing_account = BaseCommitmentHashingAccount::new(&mut data)?;
    guard!(hashing_account.get_is_active(), ComputationIsNotYetFinished);
    guard!(hashing_account.get_fee_payer() == original_fee_payer.key.to_bytes(), InvalidAccount);
    partial_computation_is_finished!(BaseCommitmentHashComputation, hashing_account);

    //guard!(first.request.commitment == result, ComputationIsNotYetFinished); we skip duplicate checks for now
    let commitment = hashing_account.get_state(0);
    let mut commitment_queue = CommitmentQueue::new(commitment_hash_queue);
    commitment_queue.enqueue(
        CommitmentHashRequest {
            commitment,
            fee_version: hashing_account.get_fee_version(),
        }
    )?;
    
    // Close hashing account
    close_account(original_fee_payer, hashing_account_info)
}

/// Reads a commitment hashing request and places it in the `CommitmentHashingAccount`
pub fn init_commitment_hash(
    queue: &mut CommitmentQueueAccount,
    hashing_account: &mut CommitmentHashingAccount,
    storage_account: &StorageAccount,
) -> ProgramResult {
    guard!(!hashing_account.get_is_active(), ComputationIsNotYetFinished);
    guard!(!storage_account.is_full(), NoRoomForCommitment);

    let queue = CommitmentQueue::new(queue);
    let request = queue.view_first()?;

    // Get hashing siblings
    let ordering = storage_account.get_next_commitment_ptr();
    let siblings = storage_account.get_mt_opening(ordering as usize);

    hashing_account.reset(
        request.commitment,
        ordering,
        siblings,
        request.fee_version,
    )
}

pub fn compute_commitment_hash<'a>(
    fee_payer: &AccountInfo<'a>,
    fee: &FeeAccount,
    pool: &AccountInfo<'a>,
    hashing_account: &mut CommitmentHashingAccount,

    fee_version: u64,
    _nonce: u64,
) -> ProgramResult {
    guard!(hashing_account.get_is_active(), ComputationIsNotYetFinished);
    guard!(hashing_account.get_fee_version() as u64 == fee_version, InvalidFeeVersion);
    partial_computation_is_not_finished!(CommitmentHashComputation, hashing_account);

    let instruction = hashing_account.get_instruction();
    let start_round = CommitmentHashComputation::INSTRUCTIONS[instruction as usize].start_round;
    let rounds = CommitmentHashComputation::INSTRUCTIONS[instruction as usize].rounds;

    let mut state = [
        u256_to_fr(&hashing_account.get_state(0)),
        u256_to_fr(&hashing_account.get_state(1)),
        u256_to_fr(&hashing_account.get_state(2)),
    ];

    // Hash computation
    for round in start_round..start_round + rounds {
        guard!(round < CommitmentHashComputation::TOTAL_ROUNDS, ComputationIsAlreadyFinished);

        binary_poseidon_hash_partial(round % TOTAL_POSEIDON_ROUNDS, &mut state);

        // A single hash is finished
        if round % TOTAL_POSEIDON_ROUNDS == 64 {
            let hash_num = round / TOTAL_POSEIDON_ROUNDS;
            let ordering = hashing_account.get_ordering();
            let offset = (ordering >> (hash_num + 1)) % 2;

            // Save hash
            let hash = state[0];
            hashing_account.set_finished_hashes(hash_num as usize, &fr_to_u256_le(&hash));

            // Reset state for next hash
            if hash_num < MT_HEIGHT - 1 {
                state[0] = Fr::zero();
                state[1 + offset as usize] = hash;
                state[2 - offset as usize] = hashing_account.get_siblings(hash_num as usize + 1).0;
            }
        }
    }

    hashing_account.set_state(0, &fr_to_u256_le(&state[0]));
    hashing_account.set_state(1, &fr_to_u256_le(&state[1]));
    hashing_account.set_state(2, &fr_to_u256_le(&state[2]));

    hashing_account.set_instruction(&(instruction + 1));

    send_from_pool(pool, fee_payer, fee.hash_tx_compensation())
}

pub fn finalize_commitment_hash(
    queue: &mut CommitmentQueueAccount,
    hashing_account: &mut CommitmentHashingAccount,
    storage_account: &mut StorageAccount,
) -> ProgramResult {
    guard!(hashing_account.get_is_active(), ComputationIsNotYetFinished);
    partial_computation_is_finished!(CommitmentHashComputation, hashing_account);

    let mut commitment_queue = CommitmentQueue::new(queue);
    let commitment = commitment_queue.dequeue_first()?;
    assert_eq!(commitment.commitment, hashing_account.get_commitment());

    // Insert commitment and hashes and save last root
    let mut values = vec![commitment.commitment];
    for i in 0..MT_HEIGHT as usize { values.push(hashing_account.get_finished_hashes(i)); }
    storage_account.insert_commitment(&values);

    hashing_account.set_is_active(&false);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{state::{program_account::SizedAccount, EMPTY_TREE}, fields::u64_to_scalar};
    use crate::commitment::poseidon_hash::full_poseidon2_hash;
    use crate::macros::{zero_account, account, test_account_info};
    use crate::state::MT_HEIGHT;
    use solana_program::native_token::LAMPORTS_PER_SOL;
    use std::str::FromStr;

    #[test]
    fn test_compute_base_commitment_hash() {
        zero_account!(hashing_account, BaseCommitmentHashingAccount);
        zero_account!(fee, FeeAccount);
        test_account_info!(pool, 0);
        test_account_info!(fee_payer, 0);

        // Setup hashing account
        let bc = Fr::from_str("8337064132573119120838379738103457054645361649757131991036638108422638197362").unwrap();
        let base_commitment = fr_to_u256_le(&bc);
        let c = Fr::from_str("139214303935475888711984321184227760578793579443975701453971046059378311483").unwrap();
        let commitment = fr_to_u256_le(&c);
        let amount = LAMPORTS_PER_SOL;

        // Reset values and get hashing siblings from storage account
        hashing_account.reset(BaseCommitmentHashRequest {
            base_commitment,
            amount,
            commitment,
            fee_version: 0,
        }, [0; 32]).unwrap();

        // Compute hash
        for i in 1..=BaseCommitmentHashComputation::INSTRUCTIONS.len() {
            compute_base_commitment_hash(&fee_payer, &fee, &pool, &mut hashing_account, 0, 0, 0).unwrap();
            assert_eq!(hashing_account.get_instruction(), i as u32);
        }

        let expected = full_poseidon2_hash(bc, u64_to_scalar(amount));

        // Check commitment
        let result = hashing_account.get_state(0);
        assert_eq!(u256_to_fr(&result), expected);
        assert_eq!(result, commitment);
    }

    #[test]
    fn test_compute_commitment_hash() {
        zero_account!(hashing_account, CommitmentHashingAccount);
        zero_account!(fee, FeeAccount);
        test_account_info!(pool, 0);
        test_account_info!(fee_payer, 0);

        // Setup hashing account
        let c = Fr::from_str("17943901642092756540532360474161569402553221410028090072917443956036363428842").unwrap();
        let commitment = fr_to_u256_le(&c);
        let ordering = 0;

        // Siblings are the default values of the MT
        let mut siblings = [Fr::zero(); MT_HEIGHT as usize];
        for i in 0..MT_HEIGHT as usize { siblings[i] = EMPTY_TREE[i]; } 

        // Reset values and get hashing siblings from storage account
        hashing_account.reset(commitment, ordering, siblings, 0).unwrap();

        // Compute hashes
        for i in 1..=CommitmentHashComputation::INSTRUCTIONS.len() {
            compute_commitment_hash(&fee_payer, &fee, &pool, &mut hashing_account, 0, 0).unwrap();
            assert_eq!(hashing_account.get_instruction(), i as u32);
        }

        // Check hashes with hash-function
        let mut hash = c;

        for i in 0..MT_HEIGHT as usize {
            hash = full_poseidon2_hash(hash, EMPTY_TREE[i]);
            assert_eq!(hashing_account.get_siblings(i).0, EMPTY_TREE[i]);
            assert_eq!(hashing_account.get_finished_hashes(i), fr_to_u256_le(&hash));
        }

        // Check root
        assert_eq!(
            hashing_account.get_finished_hashes(MT_HEIGHT as usize - 1),
            fr_to_u256_le(&Fr::from_str("13088350874257591466321551177903363895529460375369348286819794485219676679592").unwrap())
        );
    }
}