use ark_bn254::Fr;
use ark_ff::BigInteger256;
use solana_program::program_error::ProgramError;
use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    native_token::LAMPORTS_PER_SOL,
};
use crate::commitment::{commitment_hash_computation_instructions, commitments_per_batch, MAX_HT_COMMITMENTS, compute_base_commitment_hash_partial, compute_commitment_hash_partial};
use crate::macros::guard;
use crate::state::MT_COMMITMENT_COUNT;
use crate::state::governor::FEE_COLLECTOR_MINIMUM_BALANCE;
use crate::state::{StorageAccount, program_account::ProgramAccount};
use crate::types::{U256, RawU256};
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
    InsufficientFunds,
    InvalidAmount,
    InvalidAccount,
    InvalidInstructionData,
    ComputationIsNotYetFinished,
    ComputationIsAlreadyFinished,
    NonScalarValue,
    NoRoomForCommitment,
    InvalidFeeVersion,
    InvalidBatchingRate,
};
use crate::fields::{is_element_scalar_field, u256_to_big_uint, u256_to_fr_skip_mr};
use crate::commitment::{
    BaseCommitmentHashingAccount,
    CommitmentHashingAccount,
    BaseCommitmentHashComputation,
};
use elusiv_computation::PartialComputation;
use crate::fields::fr_to_u256_le;
use borsh::{BorshDeserialize, BorshSerialize};
use crate::bytes::{BorshSerDeSized, u64_as_u32_safe};
use crate::macros::BorshSerDeSized;

pub const MIN_STORE_AMOUNT: u64 = LAMPORTS_PER_SOL / 10;
pub const MAX_STORE_AMOUNT: u64 = u64::MAX / 100;
pub const MATH_ERR: ProgramError = ProgramError::InvalidArgument;

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Debug)]
pub struct BaseCommitmentHashRequest {
    pub base_commitment: RawU256,
    pub amount: u64,
    pub commitment: RawU256,   // only there for the case that we need to do duplicate checking (not atm)
    pub fee_version: u64,

    /// The minimum allowed batching rate (since the fee is precomputed with the concrete batching rate)
    pub min_batching_rate: u32,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, PartialEq, Clone, Debug)]
pub struct CommitmentHashRequest {
    pub commitment: U256,
    pub fee_version: u32,
    pub min_batching_rate: u32,
}

/// poseidon(0, 0)
const ZERO_BASE_COMMITMENT: Fr = Fr::new(BigInteger256::new([3162363550698150530, 9486080942857866267, 15374008727889305678, 621823773387469172]));

/// poseidon(poseidon(0, 0), 0) in mr-form
pub const ZERO_COMMITMENT: U256 = [29,226,44,239,152,247,24,127,109,7,41,61,125,1,193,123,69,104,37,230,178,56,26,51,102,9,129,182,119,238,153,4];
pub const ZERO_COMMITMENT_RAW: U256 = [106,77,49,231,137,82,142,103,122,195,234,157,189,191,2,42,174,41,59,182,21,225,230,119,13,86,164,94,87,82,83,23];

/// Stores a base commitment hash and takes the funds from the sender
/// - computation: `commitment = poseidon(base_commitment, amount)` (https://github.com/elusiv-privacy/circuits/blob/master/circuits/commitment.circom)
#[allow(clippy::too_many_arguments)]
pub fn store_base_commitment<'a>(
    sender: &AccountInfo<'a>,
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

    guard!(is_element_scalar_field(u256_to_big_uint(&request.base_commitment.skip_mr())), NonScalarValue);
    guard!(is_element_scalar_field(u256_to_big_uint(&request.commitment.skip_mr())), NonScalarValue);

    // Zero-commitment cannot be inserted by user
    guard!(u256_to_fr_skip_mr(&request.base_commitment.reduce()) != ZERO_BASE_COMMITMENT, InvalidInstructionData);

    guard!(request.fee_version == governor.get_fee_version(), InvalidFeeVersion);
    guard!(request.min_batching_rate == governor.get_commitment_batching_rate(), InvalidBatchingRate);

    // Take `amount` and `fee` from `sender`
    let fee = governor.get_program_fee();
    let compensation_fee = fee.base_commitment_hash_fee(request.min_batching_rate);
    let network_fee = fee.base_commitment_network_fee;
    let subvention = if fee_collector.lamports() >= FEE_COLLECTOR_MINIMUM_BALANCE + fee.base_commitment_subvention {
        fee.base_commitment_subvention
    } else {
        0
    };

    // `final_amount = request.amount + compensation_fee - network_fee - subvention`
    let due_amount = request.amount.checked_add(compensation_fee).ok_or(MATH_ERR)?;
    guard!(sender.lamports() >= due_amount - subvention, InsufficientFunds);
    let final_amount = due_amount
        .checked_sub(network_fee).ok_or(MATH_ERR)?
        .checked_sub(subvention).ok_or(MATH_ERR)?;

    send_with_system_program(sender, fee_collector, system_program, network_fee)?;
    send_with_system_program(sender, pool, system_program, final_amount)?;
    if subvention > 0 {
        send_from_pool(fee_collector, pool, subvention)?;
    }

    let mut queue = BaseCommitmentQueue::new(base_commitment_queue);
    queue.enqueue(request)
}

/// Initialized a base commitment hash by opening a `BaseCommitmentHashingAccount`
pub fn init_base_commitment_hash<'a>(
    fee_payer: &AccountInfo<'a>,
    base_commitment_queue: &mut BaseCommitmentQueueAccount,
    hashing_account: &AccountInfo<'a>,

    _base_commitment_queue_index: u64,
    hash_account_index: u64,
) -> ProgramResult {
    // `fee_payer` rents `hashing_account`
    open_pda_account_with_offset::<BaseCommitmentHashingAccount>(fee_payer, hashing_account, hash_account_index)?;

    let mut queue = BaseCommitmentQueue::new(base_commitment_queue);
    let request = queue.dequeue_first()?;

    // Hashing account setup
    let data = &mut hashing_account.data.borrow_mut()[..];
    let mut hashing_account = BaseCommitmentHashingAccount::new(data)?;
    hashing_account.setup(request, fee_payer.key.to_bytes())
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
    send_from_pool(pool, fee_payer, fee.get_program_fee().hash_tx_compensation())
}

pub fn finalize_base_commitment_hash<'a>(
    original_fee_payer: &AccountInfo<'a>,
    commitment_hash_queue: &mut CommitmentQueueAccount,
    hashing_account_info: &AccountInfo<'a>,

    _hash_account_index: u64,
) -> ProgramResult {
    let data = &mut hashing_account_info.data.borrow_mut()[..];
    let mut hashing_account = BaseCommitmentHashingAccount::new(data)?;
    guard!(hashing_account.get_is_active(), ComputationIsNotYetFinished);
    guard!(hashing_account.get_fee_payer() == original_fee_payer.key.to_bytes(), InvalidAccount);
    guard!(
        (hashing_account.get_instruction() as usize) == BaseCommitmentHashComputation::IX_COUNT,
        ComputationIsNotYetFinished
    );

    let commitment = hashing_account.get_state().result();
    let mut commitment_queue = CommitmentQueue::new(commitment_hash_queue);
    commitment_queue.enqueue(
        CommitmentHashRequest {
            commitment: fr_to_u256_le(&commitment),
            fee_version: u64_as_u32_safe(hashing_account.get_fee_version()),
            min_batching_rate: hashing_account.get_min_batching_rate(),
        }
    )?;
    
    // Close hashing account
    hashing_account.set_is_active(&false);
    close_account(original_fee_payer, hashing_account_info)
}

/// Places the hash siblings into the hashing account
pub fn init_commitment_hash_setup(
    hashing_account: &mut CommitmentHashingAccount,
    storage_account: &StorageAccount,
) -> ProgramResult {
    guard!(!hashing_account.get_is_active(), ComputationIsNotYetFinished);

    let ordering = storage_account.get_next_commitment_ptr();
    let siblings = storage_account.get_mt_opening(ordering as usize);

    hashing_account.setup(ordering, &siblings)
}

/// Places the next batch from the commitment queue in the `CommitmentHashingAccount`
pub fn init_commitment_hash(
    queue: &mut CommitmentQueueAccount,
    hashing_account: &mut CommitmentHashingAccount,
) -> ProgramResult {
    guard!(!hashing_account.get_is_active(), ComputationIsNotYetFinished);
    guard!(hashing_account.get_setup(), ComputationIsNotYetFinished);

    let mut queue = CommitmentQueue::new(queue);
    let (batch, batching_rate) = queue.next_batch()?;
    queue.remove(batch.len() as u64)?;

    // The fee/batch-upgrader logic has to guarantee that there are no lower fees in a batch
    let fee_version = batch.first().unwrap().fee_version;

    // Check for room for the commitment batch
    guard!(
        hashing_account.get_ordering() as usize + batch.len() <= MT_COMMITMENT_COUNT,
        NoRoomForCommitment
    );

    let mut commitments = [[0; 32]; MAX_HT_COMMITMENTS];
    for i in 0..batch.len() {
        commitments[i] = batch[i].commitment;
    }

    hashing_account.reset(batching_rate, fee_version as u64, &commitments)
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
    send_from_pool(pool, fee_payer, fee.get_program_fee().hash_tx_compensation())
}

/// Requires `batching_rate + 1` calls
pub fn finalize_commitment_hash(
    hashing_account: &mut CommitmentHashingAccount,
    storage_account: &mut StorageAccount,
) -> ProgramResult {
    guard!(hashing_account.get_is_active(), ComputationIsNotYetFinished);

    let finalization_ix = hashing_account.get_finalization_ix();
    let batching_rate = hashing_account.get_batching_rate();
    guard!(finalization_ix <= batching_rate, ComputationIsAlreadyFinished);

    let instruction = hashing_account.get_instruction();
    let instructions = commitment_hash_computation_instructions(hashing_account.get_batching_rate());
    guard!((instruction as usize) >= instructions.len(), ComputationIsAlreadyFinished);

    guard!(
        storage_account.get_next_commitment_ptr() as usize + commitments_per_batch(batching_rate) <= MT_COMMITMENT_COUNT,
        NoRoomForCommitment
    );

    hashing_account.update_mt(storage_account, finalization_ix);
    hashing_account.set_finalization_ix(&(finalization_ix + 1));
    if finalization_ix == batching_rate {
        hashing_account.set_is_active(&false);
        hashing_account.set_setup(&false);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::commitment::poseidon_hash::full_poseidon2_hash;
    use crate::fields::{big_uint_to_u256, SCALAR_MODULUS_RAW, u256_from_str_skip_mr, fr_to_u256_le_repr};
    use crate::state::{MT_HEIGHT, EMPTY_TREE, mt_array_index};
    use crate::state::program_account::{SizedAccount, PDAAccount, MultiAccountProgramAccount, MultiAccountAccount};
    use crate::macros::{zero_account, account, test_account_info, storage_account};
    use ark_ff::Zero;
    use assert_matches::assert_matches;
    use solana_program::pubkey::Pubkey;

    // TODO: Switch to custom Result type and assert correct Err values

    #[test]
    fn test_zero_commitment() {
        assert_eq!(
            fr_to_u256_le(&Fr::from_str("10550412122474489085186760340904980499891544584677836768300371073631951867242").unwrap()),
            ZERO_COMMITMENT
        );

        assert_eq!(
            full_poseidon2_hash(full_poseidon2_hash(Fr::zero(), Fr::zero()), Fr::zero()),
            u256_to_fr_skip_mr(&ZERO_COMMITMENT)
        );

        assert_eq!(
            RawU256::new(ZERO_COMMITMENT_RAW).reduce(),
            ZERO_COMMITMENT
        );

        assert_eq!(
            full_poseidon2_hash(Fr::zero(), Fr::zero()),
            ZERO_BASE_COMMITMENT
        );
    }

    #[test]
    #[allow(clippy::vec_init_then_push)]
    fn test_store_base_commitment() {
        zero_account!(mut governor, GovernorAccount);
        test_account_info!(sender, 0);
        test_account_info!(pool, 0);
        test_account_info!(fee_collector, 0);
        zero_account!(mut base_commitment_queue, BaseCommitmentQueueAccount);
        let sys_program_pk = solana_program::system_program::ID;
        account!(system_program, sys_program_pk, vec![]);

        governor.set_commitment_batching_rate(&4);
        governor.set_fee_version(&1);

        let valid_request = BaseCommitmentHashRequest {
            base_commitment: RawU256::new(u256_from_str_skip_mr("1")),
            amount: LAMPORTS_PER_SOL,
            commitment: RawU256::new(u256_from_str_skip_mr("1")),
            fee_version: 1,
            min_batching_rate: 4,
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
        requests.last_mut().unwrap().base_commitment = RawU256::new(big_uint_to_u256(&SCALAR_MODULUS_RAW));

        // Non-scalar commitment
        requests.push(valid_request.clone());
        requests.last_mut().unwrap().commitment = RawU256::new(big_uint_to_u256(&SCALAR_MODULUS_RAW));
        
        // Zero-commitment
        requests.push(valid_request.clone());
        requests.last_mut().unwrap().base_commitment = RawU256::new(fr_to_u256_le_repr(&ZERO_BASE_COMMITMENT));

        // Mismatched fee version
        requests.push(valid_request.clone());
        requests.last_mut().unwrap().fee_version = 0;

        // Invalid min_batching_rate
        requests.push(valid_request.clone());
        requests.last_mut().unwrap().min_batching_rate = 0;

        for request in requests {
            assert_matches!(
                store_base_commitment(&sender, &governor, &pool, &fee_collector, &system_program, &mut base_commitment_queue, 0, request),
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
                store_base_commitment(&sender, &governor, &pool, &fee_collector, &system_program, &mut base_commitment_queue, 0, valid_request.clone()),
                Err(_)
            );
        }

        store_base_commitment(&sender, &governor, &pool, &fee_collector, &system_program, &mut base_commitment_queue, 0, valid_request).unwrap();
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
                base_commitment: RawU256::new(u256_from_str_skip_mr("1")),
                amount: LAMPORTS_PER_SOL,
                commitment: RawU256::new(u256_from_str_skip_mr("1")),
                fee_version: 1,
                min_batching_rate: 4,
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

    macro_rules! pda_account_info {
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
        pda_account_info!(hashing_account, BaseCommitmentHashingAccount, |hashing_account: &mut BaseCommitmentHashingAccount| {
            hashing_account.set_fee_payer(&fee_payer_pk.to_bytes());
            hashing_account.set_instruction(&(BaseCommitmentHashComputation::IX_COUNT as u32));
        });
        assert_matches!(finalize_base_commitment_hash(&fee_payer, &mut queue, &hashing_account, 0), Err(_));

        // Invalid original fee payer
        pda_account_info!(hashing_account, BaseCommitmentHashingAccount, |hashing_account: &mut BaseCommitmentHashingAccount| {
            hashing_account.set_is_active(&true);
            hashing_account.set_instruction(&(BaseCommitmentHashComputation::IX_COUNT as u32));
        });
        assert_matches!(finalize_base_commitment_hash(&fee_payer, &mut queue, &hashing_account, 0), Err(_));

        // Computation not finished
        pda_account_info!(hashing_account, BaseCommitmentHashingAccount, |hashing_account: &mut BaseCommitmentHashingAccount| {
            hashing_account.set_fee_payer(&fee_payer_pk.to_bytes());
            hashing_account.set_is_active(&true);
        });
        assert_matches!(finalize_base_commitment_hash(&fee_payer, &mut queue, &hashing_account, 0), Err(_));

        pda_account_info!(hashing_account, BaseCommitmentHashingAccount, |hashing_account: &mut BaseCommitmentHashingAccount| {
            hashing_account.set_fee_payer(&fee_payer_pk.to_bytes());
            hashing_account.set_is_active(&true);
            hashing_account.set_instruction(&(BaseCommitmentHashComputation::IX_COUNT as u32));
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
    fn test_init_commitment_hash_empty_queue() {
        storage_account!(storage_account);
        zero_account!(mut queue, CommitmentQueueAccount);
        zero_account!(mut hashing_account, CommitmentHashingAccount);

        init_commitment_hash_setup(&mut hashing_account, &storage_account).unwrap();
        assert_matches!(init_commitment_hash(&mut queue, &mut hashing_account), Err(_));
    }

    #[test]
    fn test_init_commitment_hash_active_computation() {
        zero_account!(mut queue, CommitmentQueueAccount);
        zero_account!(mut hashing_account, CommitmentHashingAccount);

        let mut q = CommitmentQueue::new(&mut queue);
        q.enqueue(CommitmentHashRequest { commitment: [0; 32], min_batching_rate: 0, fee_version: 0 }).unwrap();

        hashing_account.set_is_active(&true);
        hashing_account.set_setup(&true);
        assert_matches!(init_commitment_hash(&mut queue, &mut hashing_account), Err(_));
    }

    #[test]
    fn test_init_commitment_hash_full_storage() {
        storage_account!(mut storage_account);
        zero_account!(mut queue, CommitmentQueueAccount);
        zero_account!(mut hashing_account, CommitmentHashingAccount);

        let mut q = CommitmentQueue::new(&mut queue);
        q.enqueue(CommitmentHashRequest { commitment: [0; 32], min_batching_rate: 0, fee_version: 0 }).unwrap();

        storage_account.set_next_commitment_ptr(&(MT_COMMITMENT_COUNT as u32));
        init_commitment_hash_setup(&mut hashing_account, &storage_account).unwrap();
        assert_matches!(init_commitment_hash(&mut queue, &mut hashing_account), Err(_));
    }

    #[test]
    fn test_init_commitment_hash_incomplete_batch() {
        storage_account!(storage_account);
        zero_account!(mut queue, CommitmentQueueAccount);
        zero_account!(mut hashing_account, CommitmentHashingAccount);

        let mut q = CommitmentQueue::new(&mut queue);
        q.enqueue(CommitmentHashRequest { commitment: [0; 32], min_batching_rate: 1, fee_version: 0 }).unwrap();

        init_commitment_hash_setup(&mut hashing_account, &storage_account).unwrap();
        assert_matches!(init_commitment_hash(&mut queue, &mut hashing_account), Err(_));
    }

    #[test]
    fn test_init_commitment_hash_batch_too_big() {
        storage_account!(mut storage_account);
        zero_account!(mut queue, CommitmentQueueAccount);
        zero_account!(mut hashing_account, CommitmentHashingAccount);

        let mut q = CommitmentQueue::new(&mut queue);
        q.enqueue(CommitmentHashRequest { commitment: [0; 32], min_batching_rate: 1, fee_version: 0 }).unwrap();

        storage_account.set_next_commitment_ptr(&(MT_COMMITMENT_COUNT as u32 - 1));
        init_commitment_hash_setup(&mut hashing_account, &storage_account).unwrap();
        assert_matches!(init_commitment_hash(&mut queue, &mut hashing_account), Err(_));
    }

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn test_init_commitment_hash_valid() {
        storage_account!(storage_account);
        zero_account!(mut queue, CommitmentQueueAccount);
        zero_account!(mut hashing_account, CommitmentHashingAccount);

        let mut q = CommitmentQueue::new(&mut queue);
        q.enqueue(CommitmentHashRequest { commitment: [1; 32], min_batching_rate: 2, fee_version: 0 }).unwrap();
        q.enqueue(CommitmentHashRequest { commitment: [2; 32], min_batching_rate: 0, fee_version: 0 }).unwrap();
        q.enqueue(CommitmentHashRequest { commitment: [3; 32], min_batching_rate: 0, fee_version: 0 }).unwrap();
        q.enqueue(CommitmentHashRequest { commitment: [4; 32], min_batching_rate: 0, fee_version: 0 }).unwrap();

        init_commitment_hash_setup(&mut hashing_account, &storage_account).unwrap();
        init_commitment_hash(&mut queue, &mut hashing_account).unwrap();

        assert_eq!(hashing_account.get_batching_rate(), 2);

        // Check correct siblings
        for i in 0..MT_HEIGHT as usize {
            assert_eq!(hashing_account.get_siblings(i), EMPTY_TREE[i]);
        }

        // Check correct commitments
        for i in 0..4 {
            assert_eq!(hashing_account.get_hash_tree(i), [i as u8 + 1; 32]);
        }
    }

    #[test]
    fn test_compute_commitment_hash() {
        zero_account!(mut hashing_account, CommitmentHashingAccount);
        zero_account!(fee, FeeAccount);
        test_account_info!(pool, 0);
        test_account_info!(fee_payer, 0);

        // Inactive account 
        assert_matches!(
            compute_commitment_hash(&fee_payer, &fee, &pool, &mut hashing_account, 0, 0),
            Err(_)
        );

        // Invalid fee_version
        hashing_account.set_is_active(&true);
        assert_matches!(
            compute_commitment_hash(&fee_payer, &fee, &pool, &mut hashing_account, 1, 0),
            Err(_)
        );

        compute_commitment_hash(&fee_payer, &fee, &pool, &mut hashing_account, 0, 0).unwrap();
    }

    #[test]
    fn test_finalize_commitment_hash() {
        storage_account!(mut storage_account);
        zero_account!(mut hashing_account, CommitmentHashingAccount);

        // Computation not finished
        hashing_account.set_is_active(&true);
        hashing_account.set_instruction(&0);
        assert_matches!(finalize_commitment_hash(&mut hashing_account, &mut storage_account), Err(_));

        // Hashing account inactive
        hashing_account.set_is_active(&false);
        hashing_account.set_instruction(&(commitment_hash_computation_instructions(0).len() as u32));
        assert_matches!(finalize_commitment_hash(&mut hashing_account, &mut storage_account), Err(_));

        // Storage account is full
        hashing_account.set_is_active(&true);
        storage_account.set_next_commitment_ptr(&(MT_COMMITMENT_COUNT as u32));
        assert_matches!(finalize_commitment_hash(&mut hashing_account, &mut storage_account), Err(_));
        
        storage_account.set_next_commitment_ptr(&0);
        finalize_commitment_hash(&mut hashing_account, &mut storage_account).unwrap();
    }

    #[test]
    fn test_finalize_commitment_hash_valid() {
        storage_account!(mut storage_account);
        zero_account!(mut hashing_account, CommitmentHashingAccount);

        let batching_rate = 4;
        hashing_account.set_is_active(&true);
        hashing_account.set_batching_rate(&batching_rate);
        hashing_account.set_instruction(&(commitment_hash_computation_instructions(batching_rate).len() as u32));

        for _ in 0..=batching_rate {
            finalize_commitment_hash(&mut hashing_account, &mut storage_account).unwrap();
        }

        let commitment_count = commitments_per_batch(batching_rate);
        assert!(!hashing_account.get_is_active());
        assert_eq!(storage_account.get_next_commitment_ptr(), commitment_count as u32);

        // Check that MT is updated
        for i in 0..=MT_HEIGHT {
            for j in 0..commitment_count >> (MT_HEIGHT - i) {
                assert!(
                    storage_account.modifications.contains_key(&mt_array_index(j, i as usize))
                );
            }
        }
    }
}