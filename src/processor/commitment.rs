use ark_bn254::Fr;
use ark_ff::BigInteger256;
use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    native_token::LAMPORTS_PER_SOL,
};
use crate::commitment::{commitment_hash_computation_instructions, commitments_per_batch, MAX_HT_COMMITMENTS, compute_base_commitment_hash_partial, compute_commitment_hash_partial};
use crate::macros::guard;
use crate::state::MT_COMMITMENT_COUNT;
use crate::state::{StorageAccount, program_account::ProgramAccount};
use crate::types::U256;
use super::utils::{send_with_system_program, send_from_pool, close_account, open_pda_account_with_offset};
use crate::state::{
    fee::FeeAccount,
    queue::{
        RingQueue,
        Queue,
        CommitmentQueue, CommitmentQueueAccount, BaseCommitmentQueueAccount, BaseCommitmentQueue,
    },
    governor::GovernorAccount,
};
use crate::error::ElusivError::{
    InvalidAmount,
    InvalidAccount,
    InvalidInstructionData,
    ComputationIsNotYetFinished,
    ComputationIsAlreadyFinished,
    NonScalarValue,
    NoRoomForCommitment,
    InvalidFeeVersion,
    InvalidBatchingRate,
    MerkleTreeIsNotInitialized,
};
use crate::fields::{try_scalar_montgomery, u256_to_big_uint, u256_to_fr};
use crate::commitment::{
    BaseCommitmentHashingAccount,
    CommitmentHashingAccount,
    BaseCommitmentHashComputation,
};
use elusiv_computation::PartialComputation;
use crate::fields::fr_to_u256_le;
use borsh::{BorshDeserialize, BorshSerialize};
use crate::bytes::BorshSerDeSized;
use crate::macros::BorshSerDeSized;

pub const MIN_STORE_AMOUNT: u64 = LAMPORTS_PER_SOL / 10;
pub const MAX_STORE_AMOUNT: u64 = u64::MAX / 100;

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Debug)]
pub struct BaseCommitmentHashRequest {
    pub base_commitment: U256,
    pub amount: u64,
    pub commitment: U256,   // only there for the case that we need to do duplicate checking (not atm)
    pub fee_version: u64,

    /// The minimum allowed batching rate (since the fee is precomputed with the concrete batching rate)
    pub min_batching_rate: u32,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Debug)]
pub struct CommitmentHashRequest {
    pub commitment: U256,
    pub fee_version: u64,
    pub min_batching_rate: u32,
}

const ZERO_BASE_COMMITMENT: Fr = Fr::new(BigInteger256::new([3162363550698150530, 9486080942857866267, 15374008727889305678, 621823773387469172]));

/// Stores a base commitment hash and takes the funds from the sender
/// - computation: `commitment = h(base_commitment, amount)` (https://github.com/elusiv-privacy/circuits/blob/master/circuits/commitment.circom)
#[allow(clippy::too_many_arguments)]
pub fn store_base_commitment<'a>(
    sender: &AccountInfo<'a>,
    fee: &FeeAccount,
    governor: &GovernorAccount,
    pool: &AccountInfo<'a>,
    fee_collector: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    base_commitment_queue: &mut BaseCommitmentQueueAccount,

    _base_commitment_queue_index: u64,
    request: BaseCommitmentHashRequest,
) -> ProgramResult {
    guard!(request.amount >= MIN_STORE_AMOUNT, InvalidAmount);
    guard!(request.amount <= MAX_STORE_AMOUNT, InvalidAmount);

    guard!(matches!(try_scalar_montgomery(u256_to_big_uint(&request.base_commitment)), Some(_)), NonScalarValue);
    guard!(matches!(try_scalar_montgomery(u256_to_big_uint(&request.commitment)), Some(_)), NonScalarValue);

    // Zero-commitment cannot be inserted by user
    guard!(u256_to_fr(&request.base_commitment) != ZERO_BASE_COMMITMENT, InvalidInstructionData);

    guard!(request.fee_version == governor.get_fee_version(), InvalidFeeVersion);
    guard!(request.min_batching_rate == governor.get_commitment_batching_rate(), InvalidBatchingRate);

    // Take amount + fee from sender
    let compensation_fee = fee.base_commitment_hash_fee(request.min_batching_rate);
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

    _base_commitment_queue_index: u64,
    hash_account_index: u64,
) -> ProgramResult {
    // fee_payer rents hashing_account
    open_pda_account_with_offset::<BaseCommitmentHashingAccount>(fee_payer, hashing_account, hash_account_index)?;

    let mut queue = BaseCommitmentQueue::new(base_commitment_queue);
    let request = queue.dequeue_first()?;

    // Hashing account setup
    let data = &mut hashing_account.data.borrow_mut()[..];
    let mut hashing_account = BaseCommitmentHashingAccount::new(data)?;
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
    guard!(hashing_account.get_fee_version() == fee_version, InvalidFeeVersion);

    compute_base_commitment_hash_partial(hashing_account)?;
    send_from_pool(pool, fee_payer, fee.hash_tx_compensation())
}

pub fn finalize_base_commitment_hash<'a>(
    original_fee_payer: &AccountInfo<'a>,
    commitment_hash_queue: &mut CommitmentQueueAccount,
    hashing_account_info: &AccountInfo<'a>,

    _hash_account_index: u64,
) -> ProgramResult {
    let data = &mut hashing_account_info.data.borrow_mut()[..];
    let hashing_account = BaseCommitmentHashingAccount::new(data)?;
    guard!(hashing_account.get_is_active(), ComputationIsNotYetFinished);
    guard!(hashing_account.get_fee_payer() == original_fee_payer.key.to_bytes(), InvalidAccount);
    guard!(
        (hashing_account.get_instruction() as usize) == BaseCommitmentHashComputation::INSTRUCTIONS.len(),
        ComputationIsNotYetFinished
    );

    let commitment = hashing_account.get_state().result();
    let mut commitment_queue = CommitmentQueue::new(commitment_hash_queue);
    commitment_queue.enqueue(
        CommitmentHashRequest {
            commitment: fr_to_u256_le(&commitment),
            fee_version: hashing_account.get_fee_version(),
            min_batching_rate: hashing_account.get_min_batching_rate(),
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
    guard!(storage_account.get_initialized(), MerkleTreeIsNotInitialized);

    let queue = CommitmentQueue::new(queue);
    let batch = queue.next_batch()?;
    assert!(batch.len() > 0);

    // The fee/batch-upgrader logic has to guarantee that there are no lower fees in a batch
    let fee_version = batch.first().unwrap().fee_version;

    // Check for room for the commitment batch
    guard!(
        storage_account.get_next_commitment_ptr() as usize + batch.len() <= MT_COMMITMENT_COUNT,
        NoRoomForCommitment
    );

    let ordering = storage_account.get_next_commitment_ptr();
    let siblings = storage_account.get_mt_opening(ordering as usize);
    let siblings = siblings.iter()
        .map(|s| fr_to_u256_le(&s))
        .collect::<Vec<U256>>()
        .try_into()
        .unwrap();
    let mut commitments: [U256; MAX_HT_COMMITMENTS] = [[0; 32]; MAX_HT_COMMITMENTS];
    for (i, commitment) in batch.iter().map(|r| r.commitment).enumerate() {
        commitments[i] = commitment;
    }

    hashing_account.reset(
        hashing_account.get_batching_rate(),
        commitments,
        ordering,
        siblings,
        fee_version,
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
    guard!(hashing_account.get_fee_version() == fee_version, InvalidFeeVersion);

    compute_commitment_hash_partial(hashing_account)?;

    let batching_rate = hashing_account.get_batching_rate();
    let instruction = hashing_account.get_instruction();
    let instructions = commitment_hash_computation_instructions(batching_rate);
    solana_program::msg!(
        "Commitment hash computation {} / {} for {} commitments",
        instruction + 1,
        instructions.len(),
        commitments_per_batch(batching_rate),
    );

    send_from_pool(pool, fee_payer, fee.hash_tx_compensation())
}

pub fn finalize_commitment_hash(
    queue: &mut CommitmentQueueAccount,
    hashing_account: &mut CommitmentHashingAccount,
    storage_account: &mut StorageAccount,
) -> ProgramResult {
    guard!(storage_account.get_initialized(), MerkleTreeIsNotInitialized);
    guard!(hashing_account.get_is_active(), ComputationIsNotYetFinished);

    let instruction = hashing_account.get_instruction();
    let instructions = commitment_hash_computation_instructions(hashing_account.get_batching_rate());
    guard!((instruction as usize) >= instructions.len(), ComputationIsAlreadyFinished);

    let mut commitment_queue = CommitmentQueue::new(queue);
    let commitment = commitment_queue.dequeue_first()?;
    // TODO: dequeue n commitments
    panic!();

    hashing_account.update_mt(storage_account);
    hashing_account.set_is_active(&false);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commitment::u256_from_str;
    use crate::fields::{big_uint_to_u256, SCALAR_MODULUS};
    use crate::state::program_account::{SizedAccount, PDAAccount};
    use crate::macros::{zero_account, account, test_account_info};
    use assert_matches::assert_matches;
    use solana_program::pubkey::Pubkey;

    #[test]
    #[allow(clippy::vec_init_then_push)]
    fn test_store_base_commitment() {
        zero_account!(fee, FeeAccount);
        zero_account!(mut governor, GovernorAccount);
        test_account_info!(sender, 0);
        test_account_info!(pool, 0);
        test_account_info!(fee_collector, 0);
        zero_account!(mut base_commitment_queue, BaseCommitmentQueueAccount);
        let sys_program_pk = solana_program::system_program::ID;
        account!(system_program, sys_program_pk, vec![]);

        governor.set_commitment_batching_rate(&5);
        governor.set_fee_version(&1);

        let valid_request = BaseCommitmentHashRequest {
            base_commitment: u256_from_str("1"),
            amount: LAMPORTS_PER_SOL,
            commitment: u256_from_str("1"),
            fee_version: 1,
            min_batching_rate: 5,
        };

        let mut requests = Vec::new();

        // Amount too low
        requests.push(valid_request.clone());
        requests.last_mut().unwrap().amount = MIN_STORE_AMOUNT - 1;

        // Amount too high
        requests.push(valid_request.clone());
        requests.last_mut().unwrap().amount = MAX_STORE_AMOUNT + 1;

        // Non-scalar base_commitment
        requests.push(valid_request.clone());
        requests.last_mut().unwrap().base_commitment = big_uint_to_u256(&SCALAR_MODULUS);

        // Non-scalar commitment
        requests.push(valid_request.clone());
        requests.last_mut().unwrap().commitment = big_uint_to_u256(&SCALAR_MODULUS);
        
        // Zero-commitment
        requests.push(valid_request.clone());
        requests.last_mut().unwrap().base_commitment = fr_to_u256_le(&ZERO_BASE_COMMITMENT);

        // Mismatched fee version
        requests.push(valid_request.clone());
        requests.last_mut().unwrap().fee_version = 0;

        // Invalid min_batching_rate
        requests.push(valid_request.clone());
        requests.last_mut().unwrap().min_batching_rate = 0;

        for request in requests {
            assert_matches!(
                store_base_commitment(&sender, &fee, &governor, &pool, &fee_collector, &system_program, &mut base_commitment_queue, 0, request),
                Err(_)
            );
        }

        // Base commitment queue is full
        {
            zero_account!(mut base_commitment_queue, BaseCommitmentQueueAccount);
            let mut queue = BaseCommitmentQueue::new(&mut base_commitment_queue);
            for _ in 0..BaseCommitmentQueue::CAPACITY {
                queue.enqueue(valid_request.clone()).unwrap();
            }

            assert_matches!(
                store_base_commitment(&sender, &fee, &governor, &pool, &fee_collector, &system_program, &mut base_commitment_queue, 0, valid_request.clone()),
                Err(_)
            );
        }

        store_base_commitment(&sender, &fee, &governor, &pool, &fee_collector, &system_program, &mut base_commitment_queue, 0, valid_request).unwrap();
    }

    #[test]
    fn test_init_base_commitment_hash() {
        let hashing_pda = BaseCommitmentHashingAccount::find(Some(0)).0;
        account!(hashing_account, hashing_pda, vec![0; BaseCommitmentHashingAccount::SIZE]);
        test_account_info!(fee_payer, 0);
        zero_account!(mut base_commitment_queue, BaseCommitmentQueueAccount);

        // Empty queue
        assert_matches!(
            init_base_commitment_hash(&fee_payer, &mut base_commitment_queue, &hashing_account, 0, 0),
            Err(_)
        );

        let mut queue = BaseCommitmentQueue::new(&mut base_commitment_queue);
        queue.enqueue(
            BaseCommitmentHashRequest {
                base_commitment: u256_from_str("1"),
                amount: LAMPORTS_PER_SOL,
                commitment: u256_from_str("1"),
                fee_version: 1,
                min_batching_rate: 5,
            }
        ).unwrap();

        // Mismatch between PDA and index
        assert_matches!(
            init_base_commitment_hash(&fee_payer, &mut base_commitment_queue, &hashing_account, 0, 1),
            Err(_)
        );

        init_base_commitment_hash(&fee_payer, &mut base_commitment_queue, &hashing_account, 0, 0).unwrap();
    }

    #[test]
    fn test_compute_base_commitment_hash() {
        zero_account!(mut hashing_account, BaseCommitmentHashingAccount);
        zero_account!(fee, FeeAccount);
        test_account_info!(pool, 0);
        test_account_info!(fee_payer, 0);

        // Failure for inactive account 
        assert_matches!(
            compute_base_commitment_hash(&fee_payer, &fee, &pool, &mut hashing_account, 0, 0, 0),
            Err(_)
        );

        // Failure for invalid fee_version
        hashing_account.set_is_active(&true);
        assert_matches!(
            compute_base_commitment_hash(&fee_payer, &fee, &pool, &mut hashing_account, 0, 1, 0),
            Err(_)
        );

        compute_base_commitment_hash(&fee_payer, &fee, &pool, &mut hashing_account, 0, 0, 0).unwrap();
    }

    macro_rules! hashing_account {
        ($id: ident, $ty: ty, $f: expr) => {
            let mut data = vec![0; <$ty>::SIZE];
            let mut tmp_acc = <$ty>::new(&mut data).unwrap();
            $f(&mut tmp_acc);
            let pk = <$ty>::find(Some(0)).0;
            account!($id, pk, data);
        };
    }

    #[test]
    fn test_finalize_base_commitment_hash() {
        let fee_payer_pk = Pubkey::new_unique();
        account!(fee_payer, fee_payer_pk, vec![0]);
        zero_account!(mut queue, CommitmentQueueAccount);

        // Inactive hashing account
        hashing_account!(hashing_account, BaseCommitmentHashingAccount, |hashing_account: &mut BaseCommitmentHashingAccount| {
            hashing_account.set_fee_payer(&fee_payer_pk.to_bytes());
            hashing_account.set_instruction(&(BaseCommitmentHashComputation::INSTRUCTIONS.len() as u32));
        });
        assert_matches!(finalize_base_commitment_hash(&fee_payer, &mut queue, &hashing_account, 0), Err(_));

        // Invalid original fee payer
        hashing_account!(hashing_account, BaseCommitmentHashingAccount, |hashing_account: &mut BaseCommitmentHashingAccount| {
            hashing_account.set_is_active(&true);
            hashing_account.set_instruction(&(BaseCommitmentHashComputation::INSTRUCTIONS.len() as u32));
        });
        assert_matches!(finalize_base_commitment_hash(&fee_payer, &mut queue, &hashing_account, 0), Err(_));

        // Computation not finished
        hashing_account!(hashing_account, BaseCommitmentHashingAccount, |hashing_account: &mut BaseCommitmentHashingAccount| {
            hashing_account.set_fee_payer(&fee_payer_pk.to_bytes());
            hashing_account.set_is_active(&true);
        });
        assert_matches!(finalize_base_commitment_hash(&fee_payer, &mut queue, &hashing_account, 0), Err(_));

        hashing_account!(hashing_account, BaseCommitmentHashingAccount, |hashing_account: &mut BaseCommitmentHashingAccount| {
            hashing_account.set_fee_payer(&fee_payer_pk.to_bytes());
            hashing_account.set_is_active(&true);
            hashing_account.set_instruction(&(BaseCommitmentHashComputation::INSTRUCTIONS.len() as u32));
        });
        
        // Commitment queue is full
        let mut q = CommitmentQueue::new(&mut queue);
        for _ in 0..CommitmentQueue::CAPACITY {
            q.enqueue(CommitmentHashRequest { commitment: [0; 32], min_batching_rate: 0, fee_version: 0 }).unwrap();
        }
        assert_matches!(finalize_base_commitment_hash(&fee_payer, &mut queue, &hashing_account, 0), Err(_));

        zero_account!(mut queue, CommitmentQueueAccount);
        finalize_base_commitment_hash(&fee_payer, &mut queue, &hashing_account, 0).unwrap();
    }

    #[test]
    fn test_init_commitment_hash() {
        panic!()
    }

    #[test]
    fn test_compute_commitment_hash() {
        zero_account!(mut hashing_account, CommitmentHashingAccount);
        zero_account!(fee, FeeAccount);
        test_account_info!(pool, 0);
        test_account_info!(fee_payer, 0);

        // Failure for inactive account 
        assert_matches!(
            compute_commitment_hash(&fee_payer, &fee, &pool, &mut hashing_account, 0, 0),
            Err(_)
        );

        // Failure for invalid fee_version
        hashing_account.set_is_active(&true);
        assert_matches!(
            compute_commitment_hash(&fee_payer, &fee, &pool, &mut hashing_account, 1, 0),
            Err(_)
        );

        compute_commitment_hash(&fee_payer, &fee, &pool, &mut hashing_account, 0, 0).unwrap();
    }

    #[test]
    fn test_finalize_commitment_hash() {
        panic!()
    }
}