use super::metadata::CommitmentMetadata;
use super::queue::{queue_account, RingQueue};
use crate::buffer::buffer_account;
use crate::bytes::usize_as_u32_safe;
use crate::commitment::poseidon_hash::BinarySpongeHashingState;
use crate::commitment::{commitments_per_batch, MAX_HT_SIZE, MT_HEIGHT};
use crate::error::ElusivError;
use crate::fields::{fr_to_u256_le, u256_to_fr_skip_mr};
use crate::macros::{elusiv_account, guard, two_pow};
use crate::processor::{BaseCommitmentHashRequest, CommitmentHashRequest};
use crate::state::program_account::PDAAccountData;
use crate::state::storage::{StorageAccount, HISTORY_ARRAY_SIZE};
use crate::types::U256;
use ark_bn254::Fr;
use ark_ff::{BigInteger256, PrimeField};
use solana_program::program_error::ProgramError;

/// Account used for computing `commitment = h(base_commitment, amount)`
#[elusiv_account(partial_computation: true, eager_type: true)]
pub struct BaseCommitmentHashingAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,

    pub instruction: u32,
    pub(crate) round: u32,

    pub fee_version: u32,
    pub fee_payer: U256,
    pub is_active: bool,

    token_id: u16,
    pub state: BinarySpongeHashingState,
    pub min_batching_rate: u32,
    pub metadata: CommitmentMetadata,
}

impl<'a> BaseCommitmentHashingAccount<'a> {
    pub fn setup(
        &mut self,
        request: BaseCommitmentHashRequest,
        metadata: CommitmentMetadata,
        fee_payer: U256,
    ) -> Result<(), ProgramError> {
        self.set_is_active(&true);
        self.set_instruction(&0);
        self.set_round(&0);
        self.set_fee_payer(&fee_payer);
        self.set_fee_version(&request.fee_version);

        self.set_min_batching_rate(&request.min_batching_rate);
        self.set_token_id(&request.token_id);
        self.set_metadata(&metadata);

        // Reset hashing state
        self.set_state(&BinarySpongeHashingState::new(
            u256_to_fr_skip_mr(&request.base_commitment.reduce()),
            Fr::from_repr(BigInteger256([
                request.amount,
                request.token_id as u64 + ((request.recent_commitment_index as u64) << 16),
                0,
                0,
            ]))
            .unwrap(),
            false,
        ));

        Ok(())
    }
}

/// Account used for computing the hashes of a MT
#[elusiv_account(partial_computation: true, eager_type: true)]
pub struct CommitmentHashingAccount {
    #[no_getter]
    #[no_setter]
    pda_data: PDAAccountData,

    pub instruction: u32,
    pub(crate) round: u32,

    pub fee_version: u32,
    pub is_active: bool,

    pub setup: bool,
    pub finalization_ix: u32,

    pub batching_rate: u32,
    pub(crate) state: BinarySpongeHashingState,
    pub ordering: u32,
    pub siblings: [U256; MT_HEIGHT],

    // hashes in: (HT-root; MT-root]
    above_hashes: [U256; MT_HEIGHT],

    // commitments and hashes in the HT
    pub hash_tree: [U256; MAX_HT_SIZE],
}

impl<'a> CommitmentHashingAccount<'a> {
    /// Called before reset, sets the siblings
    pub fn setup(&mut self, ordering: u32, siblings: &[U256]) -> Result<(), ProgramError> {
        guard!(!self.get_is_active(), ElusivError::InvalidAccountState);

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
        guard!(!self.get_is_active(), ElusivError::InvalidAccountState);
        guard!(self.get_setup(), ElusivError::InvalidAccountState);

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

        if hash_index < sub_tree_size {
            // HT hashes
            // Ignore commitments in HT
            let commitment_count = commitments_per_batch(batching_rate);
            let mut nodes_below = 0;

            // Find the hash-tree layer for the hash (all layers except the commitment layer)
            for ht_layer in (0..batching_rate).rev() {
                let layer_size = two_pow!(ht_layer);
                if hash_index - nodes_below < layer_size {
                    let index_in_layer = hash_index - nodes_below;
                    let below_layer_size = two_pow!(ht_layer + 1);
                    let index_below =
                        commitment_count + nodes_below - below_layer_size + index_in_layer * 2;

                    return BinarySpongeHashingState::new(
                        u256_to_fr_skip_mr(&self.get_hash_tree(index_below)),
                        u256_to_fr_skip_mr(&self.get_hash_tree(index_below + 1)),
                        false,
                    );
                }
                nodes_below += layer_size;
            }

            unreachable!()
        } else if hash_index == sub_tree_size {
            // hash with the HT-root and a sibling
            let ordering = self.get_ordering() >> batching_rate;
            let ht_root_index = two_pow!(batching_rate + 1) - 2;

            BinarySpongeHashingState::new(
                u256_to_fr_skip_mr(&self.get_hash_tree(ht_root_index)),
                u256_to_fr_skip_mr(&self.get_siblings(batching_rate as usize)),
                ordering & 1 == 1,
            )
        } else {
            // hash above hashes with siblings
            let index = hash_index - sub_tree_size;
            let ordering = self.get_ordering() >> (index + batching_rate as usize);

            let a = u256_to_fr_skip_mr(&self.get_above_hashes(index - 1));
            let b = u256_to_fr_skip_mr(&self.get_siblings(batching_rate as usize + index));

            BinarySpongeHashingState::new(a, b, ordering & 1 == 1)
        }
    }

    pub fn save_finished_hash(&mut self, hash_index: usize, state: &BinarySpongeHashingState) {
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
    pub fn update_mt(&self, storage_account: &mut StorageAccount, finalization_ix: u32) {
        let batching_rate = self.get_batching_rate();
        let ordering = self.get_ordering();

        // Insert values from the HT
        if finalization_ix <= batching_rate {
            let ht_level = batching_rate - finalization_ix;

            let mut nodes_below = 0;
            if finalization_ix > 0 {
                for i in (ht_level + 1..=batching_rate).rev() {
                    nodes_below += two_pow!(i);
                }
            }

            let mt_level = MT_HEIGHT - batching_rate as usize + ht_level as usize;
            let ht_level_size = two_pow!(ht_level);
            let ordering = ordering >> (MT_HEIGHT - mt_level);

            for i in 0..ht_level_size {
                storage_account
                    .set_node(
                        &self.get_hash_tree(nodes_below + i),
                        ordering as usize + i,
                        mt_level,
                    )
                    .unwrap();
            }
        }

        if finalization_ix == batching_rate {
            // Insert above hashes (including new root)
            for i in 0..MT_HEIGHT - batching_rate as usize {
                let mt_layer = MT_HEIGHT - batching_rate as usize - i - 1;
                let ordering = ordering as usize >> (batching_rate as usize + i + 1);

                storage_account
                    .set_node(&self.get_above_hashes(i), ordering, mt_layer)
                    .unwrap();
            }

            storage_account.set_next_commitment_ptr(
                &(ordering + usize_as_u32_safe(commitments_per_batch(batching_rate))),
            );

            // This inserts the new root into the `active_mt_root_history`
            storage_account.set_active_mt_root_history(
                ordering as usize % HISTORY_ARRAY_SIZE,
                &storage_account.get_root().unwrap(),
            );
            storage_account.set_mt_roots_count(&(storage_account.get_mt_roots_count() + 1));
        }
    }
}

pub const COMMITMENT_BUFFER_LEN: u32 = 128;

buffer_account!(
    BaseCommitmentBufferAccount,
    U256,
    COMMITMENT_BUFFER_LEN as usize,
);

buffer_account!(
    CommitmentBufferAccount,
    U256,
    COMMITMENT_BUFFER_LEN as usize,
);

pub const COMMITMENT_QUEUE_LEN: usize = 240;

// Queue used for storing commitments that should sequentially inserted into the active MT
queue_account!(
    CommitmentQueue,
    CommitmentQueueAccount,
    COMMITMENT_QUEUE_LEN,
    CommitmentHashRequest,
);

impl<'a, 'b> CommitmentQueue<'a, 'b> {
    /// Returns the next batch of commitments to be hashed together
    pub fn next_batch(&self) -> Result<(Vec<CommitmentHashRequest>, u32), ProgramError> {
        let mut requests = Vec::new();
        let mut highest_batching_rate = 0;
        let mut commitment_count: usize = u32::MAX as usize;
        let mut fee_version = None;

        while requests.len() < commitment_count {
            let request = self.view(requests.len())?;

            highest_batching_rate = std::cmp::max(highest_batching_rate, request.min_batching_rate);
            commitment_count = commitments_per_batch(highest_batching_rate);

            // Just a (hopefully always) redundant fee-check (depends on the fee upgrade logic)
            if let Some(f) = fee_version {
                guard!(f == request.fee_version, ElusivError::InvalidFeeVersion);
            }
            fee_version = Some(request.fee_version);

            requests.push(request);
        }

        if requests.is_empty() {
            return Err(ElusivError::QueueIsEmpty.into());
        }

        Ok((requests, highest_batching_rate))
    }
}

#[cfg(test)]
pub fn base_commitment_request(
    base_commitment: &str,
    commitment: &str,
    recent_commitment_index: u32,
    amount: u64,
    token_id: u16,
    fee_version: u32,
    min_batching_rate: u32,
) -> BaseCommitmentHashRequest {
    use crate::{fields::u256_from_str_skip_mr, types::RawU256};

    BaseCommitmentHashRequest {
        base_commitment: RawU256::new(u256_from_str_skip_mr(base_commitment)),
        commitment: RawU256::new(u256_from_str_skip_mr(commitment)),
        recent_commitment_index,
        amount,
        token_id,
        fee_version,
        min_batching_rate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commitment::{
        hash_count_per_batch, MAX_COMMITMENT_BATCHING_RATE, MAX_HT_COMMITMENTS,
    };
    use crate::fields::{u64_to_scalar, u64_to_scalar_skip_mr, u64_to_u256_skip_mr};
    use crate::macros::{parent_account, zero_program_account};
    use crate::state::queue::Queue;
    use crate::types::RawU256;
    use ark_bn254::Fr;
    use ark_ff::Zero;
    use elusiv_types::{BorshSerDeSized, ProgramAccount};
    use std::cmp::max;
    use std::str::FromStr;

    fn u64_to_u256(v: u64) -> U256 {
        fr_to_u256_le(&u64_to_scalar(v))
    }

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn test_next_hashing_state() {
        zero_program_account!(mut account, CommitmentHashingAccount);

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
        account
            .reset(batching_rate, fee_version, &commitments)
            .unwrap();

        // Init HT value to: 100 * level + index_in_layer
        let mut offset = 0;
        for level in (0..=batching_rate as u64).rev() {
            let level_size = two_pow!(level as u32);
            for i in 0..level_size {
                account.set_hash_tree(offset + i, &u64_to_u256(i as u64 + level * 100));
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
        zero_program_account!(mut account, CommitmentHashingAccount);

        let batching_rate = 4;
        account.set_batching_rate(&batching_rate);

        // Set hashes
        for hash_index in 0..hash_count_per_batch(batching_rate) {
            account.save_finished_hash(
                hash_index,
                &BinarySpongeHashingState([
                    u64_to_scalar(hash_index as u64),
                    Fr::zero(),
                    Fr::zero(),
                ]),
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
        zero_program_account!(mut account, CommitmentHashingAccount);
        parent_account!(mut storage_account, StorageAccount);

        let batching_rates: Vec<u32> = (0..MAX_COMMITMENT_BATCHING_RATE as u32).collect();
        let mut previous_commitments_count = 0;

        for (i, &batching_rate) in batching_rates.iter().enumerate() {
            // Set hashing account
            let ordering = storage_account.get_next_commitment_ptr();
            account.set_ordering(&ordering);
            account.set_batching_rate(&batching_rate);
            let commitments_count = commitments_per_batch(batching_rate);
            for commitment in 0..commitments_count {
                account.set_hash_tree(commitment, &u64_to_u256_skip_mr(commitment as u64));
            }
            for hash_index in 0..hash_count_per_batch(batching_rate) {
                account.save_finished_hash(
                    hash_index,
                    &BinarySpongeHashingState([
                        u64_to_scalar_skip_mr((hash_index + commitments_count) as u64),
                        Fr::zero(),
                        Fr::zero(),
                    ]),
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
                    storage_account
                        .get_node(previous_commitments_count + index, MT_HEIGHT)
                        .unwrap()
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
                        storage_account
                            .get_node(previous_offset + i, mt_level)
                            .unwrap()
                    );
                }

                offset += layer_size;
                previous_offset /= 2;
            }

            assert_eq!(
                storage_account.get_next_commitment_ptr(),
                ordering + commitments_count as u32
            );
            assert_eq!(storage_account.get_mt_roots_count(), i as u32 + 1);

            previous_commitments_count += commitments_count;
        }
    }

    #[test]
    fn test_base_commitment_account_setup() {
        zero_program_account!(mut account, BaseCommitmentHashingAccount);

        let request = BaseCommitmentHashRequest {
            base_commitment: RawU256::new([1; 32]),
            recent_commitment_index: 123,
            amount: 333,
            token_id: 22,
            commitment: RawU256::new([2; 32]),
            fee_version: 444,
            min_batching_rate: 555,
        };
        let fee_payer = [6; 32];

        account
            .setup(request.clone(), [255; CommitmentMetadata::SIZE], fee_payer)
            .unwrap();

        assert_eq!(
            account.get_state().0,
            [
                Fr::zero(),
                u256_to_fr_skip_mr(&request.base_commitment.reduce()),
                Fr::from_str("148698281640969010098995533").unwrap(), // 333 + 2^64 * 22 + 2^80 * 123 (https://www.wolframalpha.com/input?i=333+%2B+2%5E64+*+22+%2B+2%5E80+*+123)
            ]
        );
        assert_eq!(account.get_fee_payer(), fee_payer);
        assert_eq!(account.get_fee_version(), request.fee_version);
        assert_eq!(account.get_min_batching_rate(), request.min_batching_rate);
        assert_eq!(account.get_instruction(), 0);
        assert_eq!(account.get_metadata(), [255; CommitmentMetadata::SIZE]);
        assert!(account.get_is_active());
    }

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn test_commitment_account_reset() {
        zero_program_account!(mut account, CommitmentHashingAccount);

        let mut commitments = [[0; 32]; MAX_HT_COMMITMENTS];
        for i in 0..MAX_HT_COMMITMENTS {
            commitments[i] = u64_to_u256(i as u64 + 1);
        }

        let mut siblings = [[0; 32]; MT_HEIGHT];
        for i in 0..MT_HEIGHT {
            siblings[i] = u64_to_u256(666 + i as u64);
        }

        let fee_version = 222;
        let batching_rate = 4;
        let ordering = 555;

        account.setup(ordering, &siblings).unwrap();
        account
            .reset(batching_rate, fee_version, &commitments)
            .unwrap();

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
        assert_eq!(
            account.setup(ordering, &siblings),
            Err(ElusivError::InvalidAccountState.into())
        );

        // Second reset now allowed
        account.set_is_active(&false);
        account.setup(ordering, &siblings).unwrap();
        account
            .reset(batching_rate, fee_version, &commitments)
            .unwrap();
    }

    #[test]
    fn test_commitment_queue_next_batch() {
        let mut data = vec![0; <CommitmentQueueAccount as elusiv_types::SizedAccount>::SIZE];
        let mut q = CommitmentQueueAccount::new(&mut data).unwrap();
        let mut q = CommitmentQueue::new(&mut q);

        // Incomplete batch
        for _ in 0..3 {
            q.enqueue(CommitmentHashRequest {
                commitment: [0; 32],
                fee_version: 0,
                min_batching_rate: 2,
            })
            .unwrap();
        }
        assert_eq!(q.next_batch(), Err(ElusivError::InvalidQueueAccess.into()));

        // Complete batches (with variing batching rates)
        q.clear();
        for b in 0..=MAX_COMMITMENT_BATCHING_RATE {
            let c = commitments_per_batch(b as u32);
            for i in 0..c {
                q.enqueue(CommitmentHashRequest {
                    commitment: fr_to_u256_le(&u64_to_scalar(i as u64)),
                    fee_version: 0,
                    min_batching_rate: if i == 0 { b as u32 } else { 0 },
                })
                .unwrap();
            }
        }

        for b in 0..=MAX_COMMITMENT_BATCHING_RATE {
            let (batch, batching_rate) = q.next_batch().unwrap();
            for _ in 0..commitments_per_batch(batching_rate) {
                q.dequeue_first().unwrap();
            }

            assert_eq!(batching_rate as usize, b);
            for (i, c) in batch.iter().enumerate() {
                assert_eq!(c.commitment, fr_to_u256_le(&u64_to_scalar(i as u64)));
            }
        }

        // Mismatching fee
        q.clear();
        q.enqueue(CommitmentHashRequest {
            commitment: [0; 32],
            fee_version: 0,
            min_batching_rate: 1,
        })
        .unwrap();
        q.enqueue(CommitmentHashRequest {
            commitment: [0; 32],
            fee_version: 1,
            min_batching_rate: 1,
        })
        .unwrap();
        assert_eq!(q.next_batch(), Err(ElusivError::InvalidFeeVersion.into()));
    }
}
