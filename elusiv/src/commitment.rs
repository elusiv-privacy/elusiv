pub mod poseidon_hash;

#[cfg(not(tarpaulin_include))]
mod poseidon_constants;

use crate::commitment::poseidon_hash::{binary_poseidon_hash_partial, TOTAL_POSEIDON_ROUNDS};
use crate::error::ElusivError;
use crate::error::ElusivError::{ComputationIsAlreadyFinished};
use crate::macros::{elusiv_account, elusiv_hash_compute_units, guard, two_pow};
use crate::processor::BaseCommitmentHashRequest;
use crate::state::{StorageAccount, HISTORY_ARRAY_COUNT};
use crate::types::U256;
use crate::bytes::usize_as_u32_safe;
use crate::state::program_account::PDAAccountData;
use crate::fields::{u256_to_fr_skip_mr, fr_to_u256_le};
use ark_bn254::Fr;
use ark_ff::{BigInteger256, PrimeField};
use solana_program::program_error::ProgramError;
use elusiv_computation::PartialComputation;
use self::poseidon_hash::BinarySpongeHashingState;

/// Partial computation resulting in `commitment = h(base_commitment, amount)`
pub struct BaseCommitmentHashComputation {}

elusiv_hash_compute_units!(BaseCommitmentHashComputation, 1);
const_assert_eq!(BaseCommitmentHashComputation::TX_COUNT, 2);

/// Account used for computing `commitment = h(base_commitment, amount)`
/// - https://github.com/elusiv-privacy/circuits/blob/16de8d067a9c71aa7d807cfd80a128de6df863dd/circuits/commitment.circom#L7
/// - multiple of these accounts can exist
#[elusiv_account(partial_computation: true)]
pub struct BaseCommitmentHashingAccount {
    pda_data: PDAAccountData,

    pub instruction: u32,
    round: u32,

    pub fee_version: u32,
    pub fee_payer: U256,
    pub is_active: bool,

    token_id: u16,
    pub state: BinarySpongeHashingState,
    pub min_batching_rate: u32,
}

impl<'a> BaseCommitmentHashingAccount<'a> {
    pub fn setup(
        &mut self,
        request: BaseCommitmentHashRequest,
        fee_payer: U256,
    ) -> Result<(), ProgramError> {
        self.set_is_active(&true);
        self.set_instruction(&0);
        self.set_round(&0);
        self.set_fee_payer(&fee_payer);
        self.set_fee_version(&request.fee_version);

        self.set_min_batching_rate(&request.min_batching_rate);
        self.set_token_id(&request.token_id);

        // Reset hashing state
        self.set_state(
            &BinarySpongeHashingState::new(
                u256_to_fr_skip_mr(&request.base_commitment.reduce()),
                Fr::from_repr(BigInteger256([request.amount, request.token_id as u64, 0, 0])).unwrap(),
                false,
            )
        );

        Ok(())
    }
}

pub fn compute_base_commitment_hash_partial(
    hashing_account: &mut BaseCommitmentHashingAccount,
) -> Result<(), ProgramError> {
    let instruction = hashing_account.get_instruction();
    guard!((instruction as usize) < BaseCommitmentHashComputation::IX_COUNT, ComputationIsAlreadyFinished);

    let start_round = hashing_account.get_round();
    let rounds = BaseCommitmentHashComputation::INSTRUCTION_ROUNDS[instruction as usize] as u32;

    let mut state = hashing_account.get_state();

    for round in start_round..start_round + rounds {
        guard!(round < BaseCommitmentHashComputation::TOTAL_ROUNDS, ComputationIsAlreadyFinished);
        binary_poseidon_hash_partial(round, &mut state);
    }

    hashing_account.set_state(&state);
    hashing_account.set_instruction(&(instruction + 1));
    hashing_account.set_round(&(start_round + rounds));

    Ok(())
}

pub const DEFAULT_COMMITMENT_BATCHING_RATE: usize = 0;
pub const MAX_COMMITMENT_BATCHING_RATE: usize = 4;

/// Commitment hashing computations with batches
/// - all commitments in a batch are hashed together in order to reduce hashing costs
/// - 2ˆbatching_rate is the amount of commitments per batch
/// - batch sizes range: `[0; MAX_COMMITMENT_BATCHING_RATE]`
struct CommitmentHashComputation<const BATCHING_RATE: usize> {}

/// Generates a `CommitmentHashComputation` with a specific `BATCHING_RATE`
/// - the macro also verifies that `$hash_count$ is valid
macro_rules! commitment_batch_hashing {
    ($batching_rate: literal, $hash_count: literal, $instruction_count: literal) => {
        elusiv_hash_compute_units!(CommitmentHashComputation<$batching_rate>, $hash_count);
        const_assert_eq!($hash_count, hash_count_per_batch($batching_rate));
        const_assert_eq!($instruction_count, <CommitmentHashComputation<$batching_rate>>::IX_COUNT);
    };
}

commitment_batch_hashing!(0, 20, 24);
commitment_batch_hashing!(1, 20, 24);
commitment_batch_hashing!(2, 21, 25);
commitment_batch_hashing!(3, 24, 29);
commitment_batch_hashing!(4, 31, 37);

macro_rules! commitment_hash_computation {
    ($batching_rate: ident, $field: ident) => {
        match $batching_rate {
            0 => &CommitmentHashComputation::<0>::$field,
            1 => &CommitmentHashComputation::<1>::$field,
            2 => &CommitmentHashComputation::<2>::$field,
            3 => &CommitmentHashComputation::<3>::$field,
            4 => &CommitmentHashComputation::<4>::$field,
            _ => { panic!() }
        }
    };
}

pub const COMMITMENT_HASH_COMPUTE_BUDGET: u32 = <CommitmentHashComputation<0>>::COMPUTE_BUDGET_PER_IX;

pub fn commitment_hash_computation_instructions<'a>(batching_rate: u32) -> &'a [u8] {
    commitment_hash_computation!(batching_rate, INSTRUCTION_ROUNDS)
}

pub fn commitment_hash_computation_rounds(batching_rate: u32) -> u32 {
    *commitment_hash_computation!(batching_rate, TOTAL_ROUNDS)
}

const MT_HEIGHT: usize = crate::state::MT_HEIGHT as usize;

/// Amount of commitments batched together to compute the MT root
pub const fn commitments_per_batch(batching_rate: u32) -> usize {
    two_pow!(batching_rate)
}

/// Amount of hashes per commitment batch
/// - the commitments in a batch form a hash-sub-tree (HT) of height `batching_rate`
/// - there are additional `MT_HEIGHT - batching_rate` hashes from the HT-root to the MT-root
/// - the HT contains the commitments and has `2ˆ{batching_rate + 1} - 1` hashes
pub const fn hash_count_per_batch(batching_rate: u32) -> usize {
    // batching_rate - 1 is the height of the sub-tree without commitments
    two_pow!(batching_rate) - 1 + MT_HEIGHT as usize - batching_rate as usize
}

/// Max amount of nodes in a HT (commitments + hashes)
const MAX_HT_SIZE: usize = two_pow!(usize_as_u32_safe(MAX_COMMITMENT_BATCHING_RATE) + 1) - 1;
pub const MAX_HT_COMMITMENTS: usize = commitments_per_batch(usize_as_u32_safe(MAX_COMMITMENT_BATCHING_RATE));

const_assert_eq!(MAX_HT_SIZE, 31);
const_assert_eq!(MAX_HT_COMMITMENTS, 16);

/// Account used for computing the hashes of a MT
/// - only one of these accounts can exist per MT
#[elusiv_account(partial_computation: true)]
pub struct CommitmentHashingAccount {
    pda_data: PDAAccountData,

    pub instruction: u32,
    round: u32,

    pub fee_version: u32,
    pub is_active: bool,

    pub setup: bool,
    pub finalization_ix: u32,

    pub batching_rate: u32,
    state: BinarySpongeHashingState,
    pub ordering: u32,
    pub siblings: [U256; MT_HEIGHT],

    // hashes in: (HT-root; MT-root]
    above_hashes: [U256; MT_HEIGHT],

    // commitments and hashes in the HT
    pub hash_tree: [U256; MAX_HT_SIZE],
}

pub fn compute_commitment_hash_partial(
    hashing_account: &mut CommitmentHashingAccount,
) -> Result<(), ProgramError> {
    let batching_rate = hashing_account.get_batching_rate();
    let instruction = hashing_account.get_instruction();
    let instructions = commitment_hash_computation_instructions(batching_rate);
    guard!((instruction as usize) < instructions.len(), ComputationIsAlreadyFinished);

    let start_round = hashing_account.get_round();
    let rounds = instructions[instruction as usize] as u32;
    let total_rounds = commitment_hash_computation_rounds(batching_rate);
    guard!(start_round + rounds <= total_rounds, ComputationIsAlreadyFinished);

    let mut state = hashing_account.get_state();

    for round in start_round..start_round + rounds {
        binary_poseidon_hash_partial(round % TOTAL_POSEIDON_ROUNDS, &mut state);

        // A single hash is finished
        if round % TOTAL_POSEIDON_ROUNDS == 64 {
            let hash_index = round / TOTAL_POSEIDON_ROUNDS;

            // Save hash
            hashing_account.save_finished_hash(hash_index as usize, &state);

            // Reset state for next hash
            if (hash_index as usize) < hash_count_per_batch(batching_rate) - 1 {
                state = hashing_account.next_hashing_state(hash_index as usize + 1);
            }
        }
    }

    hashing_account.set_state(&state);
    hashing_account.set_instruction(&(instruction + 1));
    hashing_account.set_round(&(start_round + rounds));

    Ok(())
}

impl<'a> CommitmentHashingAccount<'a> {
    /// Called before reset, sets the siblings
    pub fn setup(
        &mut self,
        ordering: u32,
        siblings: &[U256],
    ) -> Result<(), ProgramError> {
        guard!(!self.get_is_active(), ElusivError::AccountCannotBeReset);
        self.set_setup(&true);
        self.set_instruction(&0);
        self.set_round(&0);
        self.set_ordering(&ordering);
        self.set_finalization_ix(&0);

        for (i, sibling) in siblings.iter().enumerate() {
            self.set_siblings(i, sibling);
        }

        Ok(())
    }

    /// Called after setup, sets the commitments and batching rate
    pub fn reset(
        &mut self,
        batching_rate: u32,
        fee_version: u32,
        commitments: &[U256],
    ) -> Result<(), ProgramError> {
        guard!(!self.get_is_active(), ElusivError::AccountCannotBeReset);
        guard!(self.get_setup(), ElusivError::AccountCannotBeReset);

        self.set_is_active(&true);
        self.set_fee_version(&fee_version);
        self.set_batching_rate(&batching_rate);

        assert!(commitments.len() <= MAX_HT_SIZE);
        for (i, commitment) in commitments.iter().enumerate() {
            self.set_hash_tree(i, commitment);
        }

        self.set_state(&self.next_hashing_state(0));

        Ok(())
    }

    /// Returns the initial state for the next hash
    /// - hashing order:
    ///     1. commitment sibling hashes on MT-layer `n`: h(c0, c1), h(c2, c3), ..
    ///     2. hashes of previous hashes till MT-layer `n - batching_rate`: h(h0, h1), ..
    ///     3. hashes of the form h(h', sibling[x]) from HT-root till MT-root
    #[allow(clippy::comparison_chain)]
    pub fn next_hashing_state(&self, hash_index: usize) -> BinarySpongeHashingState {
        let batching_rate = self.get_batching_rate();

        // Size of the ht without the commitments
        let sub_tree_size = two_pow!(batching_rate) - 1;

        if hash_index < sub_tree_size { // HT hashes
            // Ignore commitments in HT
            let commitment_count = commitments_per_batch(batching_rate);
            let mut nodes_below = 0;

            // Find the hash-tree layer for the hash (all layers except the commitment layer)
            for ht_layer in (0..batching_rate).rev() {
                let layer_size = two_pow!(ht_layer);
                if hash_index - nodes_below < layer_size {
                    let index_in_layer = hash_index - nodes_below;
                    let below_layer_size = two_pow!(ht_layer + 1);
                    let index_below = commitment_count + nodes_below - below_layer_size + index_in_layer * 2;

                    return BinarySpongeHashingState::new(
                        u256_to_fr_skip_mr(&self.get_hash_tree(index_below)),
                        u256_to_fr_skip_mr(&self.get_hash_tree(index_below + 1)),
                        false,
                    )
                }
                nodes_below += layer_size;
            }
            panic!()
        } else if hash_index == sub_tree_size { // hash with the HT-root and a sibling
            let ordering = self.get_ordering() >> batching_rate;
            let ht_root_index = two_pow!(batching_rate + 1) - 2;

            BinarySpongeHashingState::new(
                u256_to_fr_skip_mr(&self.get_hash_tree(ht_root_index)),
                u256_to_fr_skip_mr(&self.get_siblings(batching_rate as usize)),
                ordering & 1 == 1,
            )
        } else {    // hash above hashes with siblings
            let index = hash_index - sub_tree_size;
            let ordering = self.get_ordering() >> (index + batching_rate as usize);

            let a = u256_to_fr_skip_mr(&self.get_above_hashes(index - 1));
            let b = u256_to_fr_skip_mr(&self.get_siblings(batching_rate as usize + index));

            BinarySpongeHashingState::new(a, b, ordering & 1 == 1)
        }
    }

    pub fn save_finished_hash(
        &mut self,
        hash_index: usize,
        state: &BinarySpongeHashingState,
    ) {
        let batching_rate = self.get_batching_rate();
        // Size of the ht without the commitments
        let sub_tree_size = two_pow!(batching_rate) - 1;
        let result = fr_to_u256_le(&state.result());

        if hash_index < sub_tree_size {
            let commitments_count = two_pow!(batching_rate);
            self.set_hash_tree(commitments_count + hash_index, &result);
        } else {
            self.set_above_hashes(hash_index - sub_tree_size, &result)
        }
    }

    /// Updates the active MT with all finished hashes and commitments
    pub fn update_mt(
        &self,
        storage_account: &mut StorageAccount,
        finalization_ix: u32,
    ) {
        let batching_rate = self.get_batching_rate();
        let ordering = self.get_ordering();

        // Insert values from the HT
        if finalization_ix <= batching_rate {
            let ht_level = batching_rate - finalization_ix;

            let mut nodes_below = 0;
            if finalization_ix > 0 {
                for i in (ht_level + 1..=batching_rate).rev() { nodes_below += two_pow!(i); }
            }

            let mt_level = MT_HEIGHT - batching_rate as usize + ht_level as usize;
            let ht_level_size = two_pow!(ht_level);
            let ordering = ordering >> (MT_HEIGHT - mt_level);

            for i in 0..ht_level_size {
                storage_account.set_node(
                    &self.get_hash_tree(nodes_below + i),
                    ordering as usize + i,
                    mt_level,
                )
            }
        }

        if finalization_ix == batching_rate {
            // Insert above hashes (including new MT)
            for i in 0..MT_HEIGHT - batching_rate as usize {
                let mt_layer = MT_HEIGHT - batching_rate as usize - i - 1;
                let ordering = ordering as usize >> (batching_rate as usize + i + 1);

                storage_account.set_node(
                    &self.get_above_hashes(i),
                    ordering as usize,
                    mt_layer,
                )
            }

            storage_account.set_active_mt_root_history(ordering as usize % HISTORY_ARRAY_COUNT, &storage_account.get_root());
            storage_account.set_mt_roots_count(&(storage_account.get_mt_roots_count() + 1));
            storage_account.set_next_commitment_ptr(&(ordering + usize_as_u32_safe(commitments_per_batch(batching_rate))));
        }
    }
}

#[cfg(test)]
pub fn base_commitment_request(
    bc: &str,
    c: &str,
    amount: u64,
    token_id: u16, 
    fee_version: u32,
    min_batching_rate: u32,
) -> BaseCommitmentHashRequest {
    use crate::{fields::u256_from_str_skip_mr, types::RawU256};

    BaseCommitmentHashRequest {
        base_commitment: RawU256::new(u256_from_str_skip_mr(bc)),
        commitment: RawU256::new(u256_from_str_skip_mr(c)),
        amount, token_id, fee_version, min_batching_rate
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::max;
    use std::str::FromStr;

    use super::*;
    use crate::fields::{u256_from_str, u64_to_u256_skip_mr, u64_to_scalar_skip_mr, u64_to_scalar};
    use crate::state::EMPTY_TREE;
    use crate::state::program_account::{ProgramAccount, SizedAccount, MultiAccountProgramAccount, MultiAccountAccount};
    use crate::macros::storage_account;
    use crate::types::RawU256;
    use ark_bn254::Fr;
    use ark_ff::Zero;
    use assert_matches::assert_matches;
    use solana_program::native_token::LAMPORTS_PER_SOL;

    fn u64_to_u256(v: u64) -> U256 {
        fr_to_u256_le(&u64_to_scalar(v))
    }

    #[test]
    fn test_commitments_per_batch() {
        assert_eq!(commitments_per_batch(0), 1);
        assert_eq!(commitments_per_batch(1), 2);
        assert_eq!(commitments_per_batch(2), 4);
        assert_eq!(commitments_per_batch(3), 8);
    }

    #[test]
    fn test_hash_count_per_batch() {
        let n = MT_HEIGHT as usize;

        // 1 or 2 commitments => tree height hashes
        assert_eq!(hash_count_per_batch(0), n);
        assert_eq!(hash_count_per_batch(1), n);

        // 4 commitments => 2 hashes on the lowest level, 1 above, then n - 2 hashes
        assert_eq!(hash_count_per_batch(2), 2 + 1 + n - 2);

        assert_eq!(hash_count_per_batch(3), 4 + 2 + 1 + n - 3);
    }

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn test_next_hashing_state() {
        let mut data = vec![0; CommitmentHashingAccount::SIZE];
        let mut account = CommitmentHashingAccount::new(&mut data).unwrap();

        let commitments = [[0; 32]; MAX_HT_COMMITMENTS];

        let mut siblings = [[0; 32]; MT_HEIGHT];
        for i in 0..MT_HEIGHT {
            siblings[i] = u64_to_u256(MT_HEIGHT as u64 - i as u64);
        }

        let batching_rate = 3;

        // Alternating between left and right ordering of the hashes
        let mut ordering = 0;
        for i in (0..=MT_HEIGHT).rev() {
            if i & 1 == 1 {
                ordering += two_pow!(i as u32) as u32;
            }
        }
        let fee_version = 0;

        account.setup(ordering, &siblings).unwrap();
        account.reset(batching_rate, fee_version, &commitments).unwrap();

        // Init HT value to: 100 * level + index_in_layer
        let mut offset = 0;
        for level in (0..=batching_rate as u64).rev() {
            let level_size = two_pow!(level as u32);
            for i in 0..level_size {
                account.set_hash_tree(
                    offset + i,
                    &u64_to_u256(i as u64 + level * 100)
                );
            }
            offset += level_size;
        }

        // Init above hashes to: index
        for index in 0..MT_HEIGHT - batching_rate as usize {
            account.set_above_hashes(index, &u64_to_u256(index as u64));
        }

        // Check commitment hashes
        let commitent_count = commitments_per_batch(batching_rate);
        for i in 0..commitent_count / 2 {
            assert_eq!(
                account.next_hashing_state(i),
                BinarySpongeHashingState::new(
                    u64_to_scalar(i as u64 * 2 + (batching_rate as u64) * 100),
                    u64_to_scalar(i as u64 * 2 + (batching_rate as u64) * 100 + 1),
                    false
                )
            )
        }

        // Check all HT hashes
        let mut hash_index = 0;
        for level in (1..=batching_rate as u64).rev() {
            let above_level_size = two_pow!(level as u32 - 1);
            for i in 0..above_level_size {
                assert_eq!(
                    account.next_hashing_state(hash_index + i),
                    BinarySpongeHashingState::new(
                        u64_to_scalar(i as u64 * 2 + level * 100),
                        u64_to_scalar(i as u64 * 2 + level * 100 + 1),
                        false
                    )
                )
            }
            hash_index += above_level_size;
        }

        // Check HT-root and the sibling hash at level MT_HEIGHT - batching_rate
        assert_eq!(
            account.next_hashing_state(two_pow!(batching_rate) - 1),
            BinarySpongeHashingState::new(
                u64_to_scalar(0),
                u64_to_scalar(MT_HEIGHT as u64 - batching_rate as u64),
                (ordering >> batching_rate) & 1 == 1
            )
        );

        // Check above hash states
        for index in 1..MT_HEIGHT - batching_rate as usize {
            let mt_level = batching_rate as usize + index;
            assert_eq!(
                account.next_hashing_state(two_pow!(batching_rate) - 1 + index),
                BinarySpongeHashingState::new(
                    u64_to_scalar(index as u64 - 1),
                    u64_to_scalar(MT_HEIGHT as u64 - batching_rate as u64 - index as u64),
                    (ordering >> mt_level) & 1 == 1
                )
            )
        }
    }

    #[test]
    fn test_save_finished_hash() {
        let mut data = vec![0; CommitmentHashingAccount::SIZE];
        let mut account = CommitmentHashingAccount::new(&mut data).unwrap();

        let batching_rate = 4;
        account.set_batching_rate(&batching_rate);

        // Set hashes
        for hash_index in 0..hash_count_per_batch(batching_rate) {
            account.save_finished_hash(
                hash_index,
                &BinarySpongeHashingState(
                    [
                        u64_to_scalar(hash_index as u64),
                        Fr::zero(),
                        Fr::zero()
                    ]
                )
            );
        }

        // HT hashes
        let commitment_count = two_pow!(batching_rate);
        let ht_hash_count = two_pow!(batching_rate) - 1;
        for hash_index in 0..ht_hash_count {
            assert_eq!(
                account.get_hash_tree(commitment_count + hash_index),
                u64_to_u256(hash_index as u64)
            )
        }

        // Above hashes
        for index in 0..(MT_HEIGHT - batching_rate as usize) {
            assert_eq!(
                account.get_above_hashes(index),
                u64_to_u256(index as u64 + ht_hash_count as u64)
            )
        }
    }

    #[test]
    fn test_update_mt() {
        let mut data = vec![0; CommitmentHashingAccount::SIZE];
        let mut account = CommitmentHashingAccount::new(&mut data).unwrap();
        storage_account!(mut storage_account);

        let batching_rates: Vec<u32> = (0..MAX_COMMITMENT_BATCHING_RATE as u32).collect();
        let mut previous_commitments_count = 0;

        for (i, &batching_rate) in batching_rates.iter().enumerate() {
            // Set hashing account
            let ordering = storage_account.get_next_commitment_ptr();
            account.set_ordering(&ordering);
            account.set_batching_rate(&batching_rate);
            let commitments_count = commitments_per_batch(batching_rate);
            for commitment in 0..commitments_count {
                account.set_hash_tree(
                    commitment,
                    &u64_to_u256_skip_mr(commitment as u64)
                );
            }
            for hash_index in 0..hash_count_per_batch(batching_rate) {
                account.save_finished_hash(
                    hash_index,
                    &BinarySpongeHashingState(
                        [
                            u64_to_scalar_skip_mr((hash_index + commitments_count) as u64),
                            Fr::zero(),
                            Fr::zero(),
                        ]
                    )
                );
            }

            // Update
            for i in 0..=batching_rate {
                account.update_mt(&mut storage_account, i);
            }

            // Check commitments
            for index in 0..commitments_count {
                assert_eq!(
                    u64_to_u256_skip_mr(index as u64),
                    storage_account.get_node(previous_commitments_count + index, MT_HEIGHT)
                );
            }

            // Check hashes
            let mut previous_offset = previous_commitments_count / 2;
            let mut offset = 0;
            let mut layer_size = two_pow!(batching_rate);
            for mt_level in (0..MT_HEIGHT).rev() {
                layer_size = max(layer_size / 2, 1);

                for i in 0..layer_size {
                    assert_eq!(
                        u64_to_u256_skip_mr((i + commitments_count + offset) as u64),
                        storage_account.get_node(previous_offset + i, mt_level)
                    );
                }

                offset += layer_size;
                previous_offset /= 2;
            }

            assert_eq!(storage_account.get_next_commitment_ptr(), ordering + commitments_count as u32);
            assert_eq!(storage_account.get_mt_roots_count(), i as u32 + 1);

            previous_commitments_count += commitments_count;
        }
    }

    #[test]
    fn test_base_commitment_hash_computation() {
        let mut data = vec![0; BaseCommitmentHashingAccount::SIZE];
        let mut account = BaseCommitmentHashingAccount::new(&mut data).unwrap();

        let requests = [
            base_commitment_request(
                "8337064132573119120838379738103457054645361649757131991036638108422638197362",
                "139214303935475888711984321184227760578793579443975701453971046059378311483",
                LAMPORTS_PER_SOL, 0, 0,
                0,
            ),
        ];

        for request in requests {
            account.setup(request.clone(), [0; 32]).unwrap();

            while account.get_instruction() < BaseCommitmentHashComputation::IX_COUNT as u32 {
                compute_base_commitment_hash_partial(&mut account).unwrap();
            }

            assert_matches!(compute_base_commitment_hash_partial(&mut account), Err(_));
            assert_eq!(account.get_state().result(), u256_to_fr_skip_mr(&request.commitment.reduce()));
        }
    }

    struct CommitmentBatchHashRequest<'a> {
        batching_rate: u32,
        commitments: &'a [U256],
        siblings: &'a [U256],
        valid_root: U256,
    }

    #[test]
    fn test_commitment_hash_computation() {
        let empty_siblings: Vec<U256> = EMPTY_TREE.iter().take(MT_HEIGHT).copied().collect();

        let requests = [
            CommitmentBatchHashRequest {
                batching_rate: 0,
                commitments: &[
                    u256_from_str("139214303935475888711984321184227760578793579443975701453971046059378311483"),
                ],
                siblings: &empty_siblings,
                valid_root: u256_from_str("11500204619817968836204864831937045342731531929677521260156990135685848035575"),
            },
            CommitmentBatchHashRequest {
                batching_rate: 2,
                commitments: &[
                    u256_from_str("17695089122606640046122050453568281484908329551111425943069599106344573268591"),
                    u256_from_str("6647356857703578745245713474272809288360618637120301827353679811066213900723"),
                    u256_from_str("15379640546683409691976024780847698243281026803042985142030905481489858510622"),
                    u256_from_str("9526685147941891237781527305630522288121859341465303072844645355022143819256"),
                ],
                siblings: &empty_siblings,
                valid_root: u256_from_str("6543817352315114290363106811223879539017599496237896578152011659905900001939"),
            }
        ];

        for request in requests {
            let mut data = vec![0; CommitmentHashingAccount::SIZE];
            let mut account = CommitmentHashingAccount::new(&mut data).unwrap();

            let batching_rate = request.batching_rate;
            account.setup(0, request.siblings).unwrap();
            account.reset(batching_rate, 0, request.commitments).unwrap();
            
            let instructions = commitment_hash_computation_instructions(batching_rate).len() as u32;
            while account.get_instruction() < instructions {
                compute_commitment_hash_partial(&mut account).unwrap();
            }

            assert_matches!(compute_commitment_hash_partial(&mut account), Err(_));
            assert_eq!(account.get_state().result(), u256_to_fr_skip_mr(&request.valid_root));
        }
    }

    #[test]
    fn test_base_commitment_account_setup() {
        let mut data = vec![0; BaseCommitmentHashingAccount::SIZE];
        let mut account = BaseCommitmentHashingAccount::new(&mut data).unwrap();

        let request = BaseCommitmentHashRequest {
            base_commitment: RawU256::new([1; 32]),
            amount: 333,
            token_id: 2,
            commitment: RawU256::new([2; 32]),
            fee_version: 444,
            min_batching_rate: 555,
        };
        let fee_payer = [6; 32];

        account.setup(request.clone(), fee_payer).unwrap();

        assert_eq!(account.get_state().0, [
            Fr::zero(),
            u256_to_fr_skip_mr(&request.base_commitment.reduce()),
            Fr::from_str("36893488147419103565").unwrap(), // 333 + 18446744073709551616 * 2
        ]);
        assert_eq!(account.get_fee_payer(), fee_payer);
        assert_eq!(account.get_fee_version(), request.fee_version);
        assert_eq!(account.get_min_batching_rate(), request.min_batching_rate);
        assert_eq!(account.get_instruction(), 0);
        assert!(account.get_is_active());
    }
    
    #[test]
    #[allow(clippy::needless_range_loop)]
    fn test_commitment_account_reset() {
        let mut data = vec![0; CommitmentHashingAccount::SIZE];
        let mut account = CommitmentHashingAccount::new(&mut data).unwrap();

        let mut commitments = [[0; 32]; MAX_HT_COMMITMENTS];
        for i in 0..MAX_HT_COMMITMENTS { commitments[i] = u64_to_u256(i as u64 + 1); }

        let mut siblings = [[0; 32]; MT_HEIGHT];
        for i in 0..MT_HEIGHT { siblings[i] = u64_to_u256(666 + i as u64); }

        let fee_version = 222;
        let batching_rate = 4;
        let ordering = 555;

        account.setup(ordering, &siblings).unwrap();
        account.reset(batching_rate, fee_version, &commitments).unwrap();

        for i in 0..MAX_HT_COMMITMENTS {
            assert_eq!(account.get_hash_tree(i), u64_to_u256(i as u64 + 1));
        }
        for i in 0..MT_HEIGHT {
            assert_eq!(account.get_siblings(i), u64_to_u256(666 + i as u64));
        }
        assert_eq!(account.get_fee_version(), fee_version);
        assert_eq!(account.get_batching_rate(), batching_rate);
        assert_eq!(account.get_ordering(), ordering);
        assert_eq!(account.get_instruction(), 0);
        assert!(account.get_is_active());

        // Second reset should fail
        assert_matches!(account.setup(ordering, &siblings), Err(_));

        // Second reset now allowed
        account.set_is_active(&false);
        account.setup(ordering, &siblings).unwrap();
        account.reset(batching_rate, fee_version, &commitments).unwrap();
    }
}