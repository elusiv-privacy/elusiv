#[cfg(not(tarpaulin_include))]
mod poseidon_constants;
pub mod poseidon_hash;

use crate::{
    bytes::usize_as_u32_safe,
    commitment::poseidon_hash::{binary_poseidon_hash_partial, TOTAL_POSEIDON_ROUNDS},
    error::ElusivError,
    state::commitment::{BaseCommitmentHashingAccount, CommitmentHashingAccount},
};
use elusiv_computation::PartialComputation;
use elusiv_proc_macros::elusiv_hash_compute_units;
use elusiv_utils::{guard, two_pow};
use solana_program::program_error::ProgramError;

pub struct BaseCommitmentHashComputation;

elusiv_hash_compute_units!(BaseCommitmentHashComputation, 1, 100_000);
#[cfg(test)]
const_assert_eq!(BaseCommitmentHashComputation::TX_COUNT, 2);

pub fn compute_base_commitment_hash_partial(
    hashing_account: &mut BaseCommitmentHashingAccount,
) -> Result<(), ProgramError> {
    let instruction = hashing_account.get_instruction();
    guard!(
        (instruction as usize) < BaseCommitmentHashComputation::IX_COUNT,
        ElusivError::ComputationIsAlreadyFinished
    );

    let start_round = hashing_account.get_round();
    let rounds = BaseCommitmentHashComputation::INSTRUCTION_ROUNDS[instruction as usize] as u32;

    let mut state = hashing_account.get_state();

    for round in start_round..start_round + rounds {
        guard!(
            round < BaseCommitmentHashComputation::TOTAL_ROUNDS,
            ElusivError::ComputationIsAlreadyFinished
        );
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
///
/// # Notes
///
/// All commitments in a batch are hashed together in order to reduce hashing costs.
///
/// 2^batching_rate is the amount of commitments per batch.
///
/// Batch sizes range: `[0; MAX_COMMITMENT_BATCHING_RATE]`.
struct CommitmentHashComputation<const BATCHING_RATE: usize>;

/// Generates a [`CommitmentHashComputation`] with a specific `BATCHING_RATE`
///
/// # Note
///
/// The macro also verifies that `$hash_count` is valid.
macro_rules! commitment_batch_hashing {
    ($batching_rate: literal, $hash_count: literal, $instruction_count: literal) => {
        elusiv_hash_compute_units!(CommitmentHashComputation<$batching_rate>, $hash_count);

        #[cfg(test)]
        const_assert_eq!($hash_count, hash_count_per_batch($batching_rate));

        #[cfg(test)]
        const_assert_eq!(
            $instruction_count,
            <CommitmentHashComputation<$batching_rate>>::IX_COUNT
        );
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
            _ => {
                panic!()
            }
        }
    };
}

pub const COMMITMENT_HASH_COMPUTE_BUDGET: u32 =
    <CommitmentHashComputation<0>>::COMPUTE_BUDGET_PER_IX;

pub fn commitment_hash_computation_instructions<'a>(batching_rate: u32) -> &'a [u8] {
    commitment_hash_computation!(batching_rate, INSTRUCTION_ROUNDS)
}

pub fn commitment_hash_computation_rounds(batching_rate: u32) -> u32 {
    *commitment_hash_computation!(batching_rate, TOTAL_ROUNDS)
}

pub const MT_HEIGHT: usize = crate::state::storage::MT_HEIGHT as usize;

/// Amount of commitments batched together to compute the MT root
pub const fn commitments_per_batch(batching_rate: u32) -> usize {
    two_pow!(batching_rate)
}

/// Amount of hashes per commitment batch
///
/// # Notes
///
/// The commitments in a batch form a hash-sub-tree (HT) of height `batching_rate`.
///
/// There are additional `MT_HEIGHT - batching_rate` hashes from the HT-root to the MT-root.
///
/// The HT contains the commitments and has `2ˆ{batching_rate + 1} - 1` hashes.
pub const fn hash_count_per_batch(batching_rate: u32) -> usize {
    // batching_rate - 1 is the height of the sub-tree without commitments
    two_pow!(batching_rate) - 1 + MT_HEIGHT - batching_rate as usize
}

/// Max amount of nodes in a HT (commitments + hashes)
pub const MAX_HT_SIZE: usize = two_pow!(usize_as_u32_safe(MAX_COMMITMENT_BATCHING_RATE) + 1) - 1;
pub const MAX_HT_COMMITMENTS: usize =
    commitments_per_batch(usize_as_u32_safe(MAX_COMMITMENT_BATCHING_RATE));

#[cfg(test)]
const_assert_eq!(MAX_HT_SIZE, 31);
#[cfg(test)]
const_assert_eq!(MAX_HT_COMMITMENTS, 16);

pub fn compute_commitment_hash_partial(
    hashing_account: &mut CommitmentHashingAccount,
) -> Result<(), ProgramError> {
    let batching_rate = hashing_account.get_batching_rate();
    let instruction = hashing_account.get_instruction();
    let instructions = commitment_hash_computation_instructions(batching_rate);
    guard!(
        (instruction as usize) < instructions.len(),
        ElusivError::ComputationIsAlreadyFinished
    );

    let start_round = hashing_account.get_round();
    let rounds = instructions[instruction as usize] as u32;
    let total_rounds = commitment_hash_computation_rounds(batching_rate);
    guard!(
        start_round + rounds <= total_rounds,
        ElusivError::ComputationIsAlreadyFinished
    );

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fields::{u256_from_str, u256_to_fr_skip_mr},
        macros::zero_program_account,
        state::{
            commitment::base_commitment_request, metadata::CommitmentMetadata, storage::EMPTY_TREE,
        },
        types::U256,
    };
    use solana_program::native_token::LAMPORTS_PER_SOL;

    #[test]
    fn test_commitments_per_batch() {
        assert_eq!(commitments_per_batch(0), 1);
        assert_eq!(commitments_per_batch(1), 2);
        assert_eq!(commitments_per_batch(2), 4);
        assert_eq!(commitments_per_batch(3), 8);
    }

    #[test]
    fn test_hash_count_per_batch() {
        let n = MT_HEIGHT;

        // 1 or 2 commitments => tree height hashes
        assert_eq!(hash_count_per_batch(0), n);
        assert_eq!(hash_count_per_batch(1), n);

        // 4 commitments => 2 hashes on the lowest level, 1 above, then n - 2 hashes
        assert_eq!(hash_count_per_batch(2), 2 + 1 + n - 2);

        assert_eq!(hash_count_per_batch(3), 4 + 2 + 1 + n - 3);
    }

    #[test]
    fn test_base_commitment_hash_computation() {
        zero_program_account!(mut account, BaseCommitmentHashingAccount);

        let requests = [
            base_commitment_request(
                "8337064132573119120838379738103457054645361649757131991036638108422638197362",
                "139214303935475888711984321184227760578793579443975701453971046059378311483",
                0,
                LAMPORTS_PER_SOL,
                0,
                0,
                0,
            ),
            base_commitment_request(
                "18586133768512220936620570745912940619677854269274689475585506675881198879027",
                "21128387980949076499567732971523903199747404934809414689409667640726053688078",
                two_pow!(20) as u32 - 1,
                2,
                1,
                0,
                0,
            ),
        ];

        for request in requests {
            account
                .setup(request.clone(), CommitmentMetadata::default(), [0; 32])
                .unwrap();

            while account.get_instruction() < BaseCommitmentHashComputation::IX_COUNT as u32 {
                compute_base_commitment_hash_partial(&mut account).unwrap();
            }

            assert_eq!(
                compute_base_commitment_hash_partial(&mut account),
                Err(ElusivError::ComputationIsAlreadyFinished.into())
            );
            assert_eq!(
                account.get_state().result(),
                u256_to_fr_skip_mr(&request.commitment.reduce())
            );
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
            zero_program_account!(mut account, CommitmentHashingAccount);

            let batching_rate = request.batching_rate;
            account.setup(0, request.siblings).unwrap();
            account
                .reset(batching_rate, 0, request.commitments)
                .unwrap();

            let instructions = commitment_hash_computation_instructions(batching_rate).len() as u32;
            while account.get_instruction() < instructions {
                compute_commitment_hash_partial(&mut account).unwrap();
            }

            assert_eq!(
                compute_commitment_hash_partial(&mut account),
                Err(ElusivError::ComputationIsAlreadyFinished.into())
            );
            assert_eq!(
                account.get_state().result(),
                u256_to_fr_skip_mr(&request.valid_root)
            );
        }
    }
}
