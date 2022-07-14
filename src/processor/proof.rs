use std::collections::HashSet;
use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    clock::Clock,
    sysvar::Sysvar,
};
use crate::fields::Wrap;
use crate::macros::{guard, BorshSerDeSized, EnumVariantIndex};
use crate::processor::{MATH_ERR, ZERO_COMMITMENT};
use crate::processor::utils::{open_pda_account_with_offset, send_with_system_program};
use crate::proof::vkey::VerificationKey;
use crate::proof::{prepare_public_inputs_instructions, verify_partial, VerificationAccountData, PendingNullifierHashesAccount, VerificationSetupState};
use crate::state::{
    NullifierAccount,
    StorageAccount,
    program_account::ProgramAccount,
    governor::GovernorAccount,
};
use crate::error::ElusivError::{
    InvalidAmount,
    InvalidAccount,
    InvalidAccountState,
    InsufficientFunds,
    InvalidMerkleRoot,
    InvalidPublicInputs,
    InvalidInstructionData,
    ComputationIsAlreadyFinished,
    NullifierAlreadyExists,
    InvalidTimestamp,
    InvalidFeeVersion,
    FeatureNotAvailable,
};
use crate::proof::{
    VerificationAccount,
    vkey::{SendQuadraVKey, MigrateUnaryVKey},
};
use crate::types::{RawProof, SendPublicInputs, MigratePublicInputs, PublicInputs, JoinSplitPublicInputs, U256, u256_to_repr, Proof};
use crate::bytes::{BorshSerDeSized, ElusivOption};
use borsh::{BorshSerialize, BorshDeserialize};

#[derive(BorshSerialize, BorshDeserialize, BorshSerDeSized, EnumVariantIndex)]
pub enum ProofRequest {
    Send(SendPublicInputs),
    Merge(SendPublicInputs),
    Migrate(MigratePublicInputs),
}

macro_rules! execute_with_vkey {
    ($index: ident, $vk: ident, $b: block) => {
        match $index {
            0 => { type $vk = SendQuadraVKey; $b }
            1 => { type $vk = SendQuadraVKey; $b }
            2 => { type $vk = MigrateUnaryVKey; $b }
            _ => panic!()
        }
    };
}

macro_rules! proof_request {
    ($self: ident, $public_inputs: ident, $e: expr) => {
        match &$self {
            ProofRequest::Send($public_inputs) => { $e },
            ProofRequest::Merge($public_inputs) => { $e },
            ProofRequest::Migrate($public_inputs) => { $e },
        }
    };
}

impl ProofRequest {
    pub fn fee_version(&self) -> u64 {
        proof_request!(self, public_inputs, public_inputs.join_split_inputs().fee_version)
    }

    /// The amount used to compute the fee
    pub fn proof_fee_amount(&self) -> u64 {
        match self {
            ProofRequest::Send(request) => request.join_split.amount,
            _ => 0
        }
    }
}

/// We only allow two distinct MTs in a join-split (merge can be used to reduce the amount of MTs)
pub const MAX_MT_COUNT: usize = 2;

#[allow(clippy::too_many_arguments)]
/// Called once to initialize a proof verification
pub fn init_verification<'a, 'b, 'c, 'd>(
    fee_payer: &AccountInfo<'c>,
    governor: &GovernorAccount,
    pool: &AccountInfo<'c>,
    fee_collector: &AccountInfo<'c>,
    verification_account: &AccountInfo<'c>,
    storage_account: &StorageAccount,
    n_acc_0: &NullifierAccount<'a, 'b, 'd>,
    n_acc_1: &NullifierAccount<'a, 'b, 'd>,
    system_program: &AccountInfo<'c>,

    verification_account_index: u64,
    tree_indices: [u64; MAX_MT_COUNT],
    request: ProofRequest,
) -> ProgramResult {
    let vkey = request.variant_index();
    let raw_public_inputs = proof_request!(
        request,
        public_inputs,
        public_inputs.public_signals()
    );
    let instructions = execute_with_vkey!(vkey, VKey, {
        prepare_public_inputs_instructions::<VKey>(
            &proof_request!(
                request,
                public_inputs,
                public_inputs.public_signals_big_integer()
            )
        )
    });

    // Compute fee
    guard!(request.fee_version() == governor.get_fee_version(), InvalidFeeVersion);
    let fee = governor.get_program_fee();
    let min_batching_rate = governor.get_commitment_batching_rate();
    let unadjusted_fee = fee.proof_verification_fee(
        instructions.len(),
        min_batching_rate,
        request.proof_fee_amount()
    );
    let subvention = fee.proof_subvention;
    let fee_total = unadjusted_fee.checked_sub(subvention).ok_or(MATH_ERR)?;

    let mut remaining_amount = 0;
    let join_split;

    // Verify public inputs
    match &request {
        ProofRequest::Send(public_inputs) => {
            guard!(public_inputs.join_split.amount >= fee_total, InvalidAmount);
            remaining_amount = public_inputs.join_split.amount - fee_total;
            guard!(public_inputs.verify_additional_constraints(), InvalidPublicInputs);
            join_split = &public_inputs.join_split;

            if cfg!(not(test)) {
                let clock = Clock::get()?;
                let current_timestamp: u64 = clock.unix_timestamp.try_into().unwrap();
                guard!(is_timestamp_valid(public_inputs.current_time, current_timestamp), InvalidTimestamp);
            }
        }
        ProofRequest::Merge(public_inputs) => {
            guard!(public_inputs.join_split.amount == fee_total, InvalidAmount);
            guard!(public_inputs.verify_additional_constraints(), InvalidPublicInputs);
            join_split = &public_inputs.join_split;
        }
        ProofRequest::Migrate(_) => {
            return Err(FeatureNotAvailable.into())
        }
    }

    check_join_split_public_inputs(
        join_split,
        storage_account,
        [n_acc_0, n_acc_1],
        &tree_indices,
    )?;

    // Send subvention to pool
    if subvention > 0 {
        guard!(fee_collector.lamports() >= subvention, InsufficientFunds);
        send_with_system_program(fee_collector, pool, system_program, subvention)?;
    }

    // Open `VerificationAccount`
    open_pda_account_with_offset::<VerificationAccount>(
        fee_payer,
        verification_account,
        verification_account_index
    )?;
    let data = &mut verification_account.data.borrow_mut()[..];
    let mut verification_account = VerificationAccount::new(data)?;

    verification_account.setup(
        &raw_public_inputs,
        &instructions,
        vkey,
        VerificationAccountData {
            fee_payer: fee_payer.key.to_bytes(),
            min_batching_rate,
            remaining_amount,
            unadjusted_fee,
            tree_indices,
            request,
        },
    )
}

/// Verifies that the `nullifier_hashes` are not already being verified in a different verification
/// - this protects the relayer against bad clients, submitting identical, valid proofs to two relayers at the same time
/// - relayers can opt-out of this protection by setting `ignore_duplicate_verifications`
/// - `ignore_duplicate_verifications` cannot introduce a race condition, since at proof finalization each `nullifier_hashes` uniqueness is checked
pub fn init_verification_validate_nullifier_hashes(
    verification_account: &mut VerificationAccount,
    pending_nullifier_hashes0: &mut PendingNullifierHashesAccount,
    pending_nullifier_hashes1: &mut PendingNullifierHashesAccount,

    _verification_account_index: u64,
    tree_indices: [u64; MAX_MT_COUNT],
    ignore_duplicate_verifications: bool,
) -> ProgramResult {
    guard!(
        matches!(verification_account.get_setup_state(), VerificationSetupState::None),
        InvalidAccountState
    );

    let data = verification_account.get_other_data();
    guard!(data.tree_indices == tree_indices, InvalidInstructionData);
    let request = data.request;
    let join_split = proof_request!(
        request,
        public_inputs,
        public_inputs.join_split_inputs()
    );

    // Check for duplicates and add `nullifier_hashes` into `pending_nullifier_hashes`
    let nullifier_hashes = group_nullifier_hashes(join_split);
    pending_nullifier_hashes0.try_insert(&nullifier_hashes[0], ignore_duplicate_verifications)?;
    if nullifier_hashes.len() == 2 {
        pending_nullifier_hashes1.try_insert(&nullifier_hashes[1], ignore_duplicate_verifications)?;
    }

    verification_account.set_setup_state(&VerificationSetupState::NullifiersChecked);

    Ok(())
}

/// Called once after `init_verification_validate_nullifier_hashes` to initialize the proof's public inputs
pub fn init_verification_public_inputs(
    verification_account: &mut VerificationAccount,
    _verification_account_index: u64,
) -> ProgramResult {
    guard!(
        matches!(verification_account.get_setup_state(), VerificationSetupState::NullifiersChecked),
        InvalidAccountState
    );
    guard!(verification_account.get_is_verified().option().is_none(), ComputationIsAlreadyFinished);

    let vkey = verification_account.get_vkey();
    let public_inputs_count = execute_with_vkey!(vkey, VKey, { VKey::PUBLIC_INPUTS_COUNT });

    for i in 0..public_inputs_count {
        let n =  verification_account.load_raw_public_input(i);
        verification_account.set_public_input(
            i,
            &Wrap(u256_to_repr(&n))
        );
    }

    verification_account.set_setup_state(&VerificationSetupState::PublicInputsSetup);

    Ok(())
}

/// Called once after `init_verification_public_inputs` to initialize the proof's public inputs
/// - Note: has to be called by the original `fee_payer`, that called `init_verification`
/// - depending on the MT-count this has to be called in a different tx than the init-tx
pub fn init_verification_proof(
    fee_payer: &AccountInfo,
    verification_account: &mut VerificationAccount,

    _verification_account_index: u64,
    proof: RawProof,
) -> ProgramResult {
    guard!(
        matches!(verification_account.get_setup_state(), VerificationSetupState::PublicInputsSetup),
        InvalidAccountState
    );
    guard!(verification_account.get_is_verified().option().is_none(), ComputationIsAlreadyFinished);
    guard!(verification_account.get_other_data().fee_payer == fee_payer.key.to_bytes(), InvalidAccount);

    let proof: Proof = proof.into();
    verification_account.a.set_serialize(&proof.a);
    verification_account.b.set_serialize(&proof.b);
    verification_account.c.set_serialize(&proof.c);

    verification_account.set_setup_state(&VerificationSetupState::ProofSetup);

    Ok(())
}

/// Partial proof verification computation
pub fn compute_verification(
    verification_account: &mut VerificationAccount,
    _verification_account_index: u64,
) -> ProgramResult {
    guard!(verification_account.get_setup_state().is_setup(), InvalidAccountState);
    guard!(verification_account.get_is_verified().option().is_none(), ComputationIsAlreadyFinished);

    let vkey = verification_account.get_vkey();
    match execute_with_vkey!(vkey, VKey, {
        verify_partial::<VKey>(verification_account)
    }) {
        Ok(result) => match result {
            Some(final_result) => { // After last round we receive the verification result
                verification_account.set_is_verified(&ElusivOption::Some(final_result));
            }
            None => {}
        }
        Err(e) => {
            match e {
                InvalidAccountState => return Err(e.into()),
                _ => { // An error (!= InvalidAccountState) can only happen with flawed inputs -> cancel verification
                    verification_account.set_is_verified(&ElusivOption::Some(false));
                    return Ok(())
                }
            }
        }
    }

    Ok(())
}

const TIMESTAMP_BITS_PRUNING: usize = 5;
fn is_timestamp_valid(asserted_time: u64, timestamp: u64) -> bool {
    (asserted_time >> TIMESTAMP_BITS_PRUNING) <= (timestamp >> TIMESTAMP_BITS_PRUNING)
}

fn is_vec_duplicate_free<T: std::cmp::Eq + std::hash::Hash + std::clone::Clone>(v: &Vec<T>) -> bool {
    (*v).clone().drain(..).collect::<HashSet<T>>().len() == v.len()
}

#[allow(clippy::needless_range_loop)]
fn check_join_split_public_inputs(
    public_inputs: &JoinSplitPublicInputs,
    storage_account: &StorageAccount,
    nullifier_accounts: [&NullifierAccount; 2],
    tree_indices: &[u64; MAX_MT_COUNT],
) -> ProgramResult {
    // Check that the resulting commitment is not the zero-commitment
    guard!(public_inputs.commitment != ZERO_COMMITMENT, InvalidPublicInputs);

    let active_tree_index = storage_account.get_trees_count();

    guard!(public_inputs.roots[0].is_some(), InvalidPublicInputs);
    guard!(
        public_inputs.nullifier_hashes.len() == public_inputs.commitment_count as usize,
        InvalidPublicInputs
    );
    guard!(
        public_inputs.roots.len() == public_inputs.commitment_count as usize,
        InvalidPublicInputs
    );

    let mut roots = Vec::new();
    let mut tree_index = vec![0; public_inputs.commitment_count as usize];
    let mut nullifier_hashes = Vec::new();
    for (i, root) in public_inputs.roots.iter().enumerate() {
        match root {
            Some(root) => {
                let index = roots.len();
                tree_index[i] = index;
                roots.push(root);
                nullifier_hashes.push(vec![public_inputs.nullifier_hashes[i]]);

                // Verify that root is valid
                if tree_indices[index] == active_tree_index { // Active tree
                    guard!(storage_account.is_root_valid(*root), InvalidMerkleRoot);
                } else { // Closed tree
                    guard!(*root == nullifier_accounts[index].get_root(), InvalidMerkleRoot);
                }
            }
            None => {
                nullifier_hashes[0].push(public_inputs.nullifier_hashes[i]);
            }
        }
    }
    guard!(!roots.is_empty() && roots.len() <= MAX_MT_COUNT, InvalidPublicInputs);
    guard!(public_inputs.roots[0].is_some(), InvalidPublicInputs);
    guard!(tree_indices.len() >= roots.len(), InvalidPublicInputs);

    // All supplied MTs (storage/nullifier-accounts) are pairwise different
    if roots.len() > 1 {
        guard!(is_vec_duplicate_free(&tree_indices.to_vec()), InvalidInstructionData);
    }

    for (i, &nullifier_hash) in public_inputs.nullifier_hashes.iter().enumerate() {
        // No duplicate nullifier-hashes for the same MT
        for j in 0..public_inputs.nullifier_hashes.len() {
            if i == j { continue }
            if nullifier_hash == public_inputs.nullifier_hashes[j] {
                guard!(tree_index[i] != tree_index[j], InvalidPublicInputs);
            }
        }

        // Check that `nullifier_hash` is new
        guard!(
            nullifier_accounts[tree_index[i]].can_insert_nullifier_hash(public_inputs.nullifier_hashes[i])?,
            NullifierAlreadyExists
        );
    }

    Ok(())
}

fn group_nullifier_hashes(
    public_inputs: &JoinSplitPublicInputs,
) -> Vec<Vec<U256>> {
    let mut nullifier_hashes = Vec::new();
    for (i, root) in public_inputs.roots.iter().enumerate() {
        match root {
            Some(_) => {
                nullifier_hashes.push(vec![public_inputs.nullifier_hashes[i]]);
            }
            None => {
                nullifier_hashes[0].push(public_inputs.nullifier_hashes[i]);
            }
        }
    }
    nullifier_hashes
}

#[cfg(test)]
mod tests {
    // Note: unit tests here allow for behaviour that is invalid on the ledger (e.g. calling pen_pda_account_with_offset twice)

    use super::*;
    use std::str::FromStr;
    use ark_bn254::Fr;
    use ark_ff::{BigInteger256, PrimeField};
    use assert_matches::assert_matches;
    use solana_program::native_token::LAMPORTS_PER_SOL;
    use solana_program::pubkey::Pubkey;
    use solana_program::system_program;
    use crate::commitment::u256_from_str;
    use crate::proof::{COMBINED_MILLER_LOOP_IXS, FINAL_EXPONENTIATION_IXS, proof_from_str, PendingNullifierHashesMap};
    use crate::proof::vkey::TestVKey;
    use crate::state::fee::ProgramFee;
    use crate::state::{EMPTY_TREE, MT_HEIGHT, NullifierMap};
    use crate::state::program_account::{SizedAccount, PDAAccount, MultiAccountProgramAccount, MultiAccountAccount};
    use crate::macros::{two_pow, zero_account, account, test_account_info, storage_account, nullifier_account, hash_map};
    use crate::types::{U256Limbed2, Proof};

    fn mutate<T: Clone, F>(v: &T, f: F) -> T where F: Fn(&mut T) {
        let mut i = v.clone();
        f(&mut i);
        i
    }

    #[test]
    fn test_init_verification() {
        test_account_info!(fee_payer, 0);
        zero_account!(mut governor, GovernorAccount);
        test_account_info!(pool, 0);
        test_account_info!(fee_collector, 0);
        let system_program_pk = system_program::ID;
        account!(system_program, system_program_pk, vec![]);
        governor.set_program_fee(&ProgramFee::default());
        let pda = VerificationAccount::find(Some(0)).0;
        account!(verification_account, pda, vec![0; VerificationAccount::SIZE]);

        let public_inputs = SendPublicInputs{
            join_split: JoinSplitPublicInputs {
                commitment_count: 1,
                roots: vec![
                    Some(EMPTY_TREE[MT_HEIGHT as usize]),
                ],
                nullifier_hashes: vec![
                    u256_from_str("1"),
                ],
                commitment: u256_from_str("1"),
                fee_version: 0,
                amount: LAMPORTS_PER_SOL,
            },
            recipient: u256_from_str("1"),
            current_time: 0,
            identifier: u256_from_str("1"),
            salt: u256_from_str("1"),
        };

        struct InitVerificationTest {
            verification_account_index: u64,
            tree_indices: [u64; MAX_MT_COUNT],
            request: ProofRequest,
            success: bool,
        }

        let tests = [
            // Send: amount is smaller than fee
            InitVerificationTest {
                verification_account_index: 0,
                tree_indices: [0, 0],
                request: ProofRequest::Send(
                    mutate(&public_inputs, |public_inputs| {
                        public_inputs.join_split.amount = 0;
                    })
                ),
                success: false,
            },

            // Merge: amount > fee
            InitVerificationTest {
                verification_account_index: 0,
                tree_indices: [0, 0],
                request: ProofRequest::Merge(
                    mutate(&public_inputs, |public_inputs| {
                        public_inputs.join_split.amount = 35493 + 1;
                    })
                ),
                success: false,
            },

            // Merge: amount < fee
            InitVerificationTest {
                verification_account_index: 0,
                tree_indices: [0, 0],
                request: ProofRequest::Merge(
                    mutate(&public_inputs, |public_inputs| {
                        public_inputs.join_split.amount = 35493 - 1;
                    })
                ),
                success: false,
            },

            // Send: commitment-count too low
            InitVerificationTest {
                verification_account_index: 0,
                tree_indices: [0, 0],
                request: ProofRequest::Send(
                    mutate(&public_inputs, |public_inputs| {
                        public_inputs.join_split.commitment_count = 0;
                    })
                ),
                success: false,
            },

            // Send: commitment-count too high
            InitVerificationTest {
                verification_account_index: 0,
                tree_indices: [0, 0],
                request: ProofRequest::Send(
                    mutate(&public_inputs, |public_inputs| {
                        public_inputs.join_split.commitment_count = 4 + 1;
                    })
                ),
                success: false,
            },

            // Invalid verification_account_index
            InitVerificationTest {
                verification_account_index: 1,
                tree_indices: [0, 0],
                request: ProofRequest::Send(public_inputs.clone()),
                success: false,
            },

            // Merge: fee == amount
            InitVerificationTest {
                verification_account_index: 0,
                tree_indices: [0, 0],
                request: ProofRequest::Merge(
                    mutate(&public_inputs, |public_inputs| {
                        public_inputs.join_split.amount = 35493;
                    })
                ),
                success: true,
            },

            InitVerificationTest {
                verification_account_index: 0,
                tree_indices: [0, 0],
                request: ProofRequest::Send(public_inputs.clone()),
                success: true,
            },

            // Migrate: failure 
            InitVerificationTest {
                verification_account_index: 0,
                tree_indices: [0, 0],
                request: ProofRequest::Migrate(
                    MigratePublicInputs {
                        join_split: public_inputs.join_split,
                        current_nsmt_root: [0; 32],
                        next_nsmt_root: [0; 32],
                    }
                ),
                success: false,
            },
        ];

        for test in tests {
            storage_account!(storage_account);
            nullifier_account!(nullifier_account);

            let result = init_verification(
                &fee_payer,
                &governor,
                &pool,
                &fee_collector,
                &verification_account,
                &storage_account,
                &nullifier_account,
                &nullifier_account,
                &system_program,
                test.verification_account_index,
                test.tree_indices,
                test.request,
            );

            if test.success {
                assert_matches!(result, Ok(()));
            } else {
                assert_matches!(result, Err(_));
            }
        }
    }

    macro_rules! pending_nulliier_map {
        ($id: ident) => {
            let data = vec![0; PendingNullifierHashesAccount::ACCOUNT_SIZE];
            pending_nulliier_map!(terminal $id, data);
        };
        ($id: ident, $map: expr) => {
            let mut data = vec![1];
            $map.serialize(&mut data).unwrap();
            pending_nulliier_map!(terminal $id, data);
        };

        (terminal $id: ident, $data: expr) => {
            let pk = Pubkey::new_unique();
            account!(pending_nullifier_map, pk, $data);
            let mut data = vec![0; PendingNullifierHashesAccount::SIZE];
            hash_map!(map, (0usize, &pending_nullifier_map));
            let mut $id = PendingNullifierHashesAccount::new(&mut data, map).unwrap();
        };
    }

    #[test]
    fn test_init_verification_validate_nullifier_hashes() {
        let mut data = vec![0; VerificationAccount::SIZE];
        let mut verification_account = VerificationAccount::new(&mut data).unwrap();
        verification_account.set_other_data(
            &VerificationAccountData {
                fee_payer: [0; 32],
                min_batching_rate: 0,
                remaining_amount: 0,
                unadjusted_fee: 0,
                tree_indices: [0, 1],
                request: ProofRequest::Send(
                    SendPublicInputs{
                        join_split: JoinSplitPublicInputs {
                            commitment_count: 3,
                            roots: vec![
                                Some(EMPTY_TREE[MT_HEIGHT as usize]),
                                None,
                                Some(EMPTY_TREE[MT_HEIGHT as usize]),
                            ],
                            nullifier_hashes: vec![
                                u256_from_str("1"),
                                u256_from_str("2"),
                                u256_from_str("3"),
                            ],
                            commitment: u256_from_str("1"),
                            fee_version: 0,
                            amount: LAMPORTS_PER_SOL,
                        },
                        recipient: u256_from_str("1"),
                        current_time: 0,
                        identifier: u256_from_str("1"),
                        salt: u256_from_str("1"),
                    }
                ),
            }
        );

        pending_nulliier_map!(pending0);
        pending_nulliier_map!(pending1);

        // Already setup
        verification_account.set_setup_state(&VerificationSetupState::NullifiersChecked);
        assert_matches!(init_verification_validate_nullifier_hashes(&mut verification_account, &mut pending0, &mut pending1, 0, [0, 1], false), Err(_));
        verification_account.set_setup_state(&VerificationSetupState::None);

        // Nullifier duplicates (ignore_duplicates will always result in success)
        // First nullifier is duplicate
        let mut nullifier_map = PendingNullifierHashesMap::new();
        nullifier_map.try_insert(U256Limbed2::from(u256_from_str("1")), 0).unwrap();
        pending_nulliier_map!(pending0, nullifier_map);
        pending_nulliier_map!(pending1);

        assert_matches!(init_verification_validate_nullifier_hashes(&mut verification_account, &mut pending0, &mut pending1, 0, [0, 1], false), Err(_));
        assert_matches!(init_verification_validate_nullifier_hashes(&mut verification_account, &mut pending0, &mut pending1, 0, [0, 1], true), Ok(()));

        verification_account.set_setup_state(&VerificationSetupState::None);

        // Second nullifier (same MT) is duplicate
        let mut nullifier_map = PendingNullifierHashesMap::new();
        nullifier_map.try_insert(U256Limbed2::from(u256_from_str("2")), 0).unwrap();
        pending_nulliier_map!(pending0, nullifier_map);
        pending_nulliier_map!(pending1);

        assert_matches!(init_verification_validate_nullifier_hashes(&mut verification_account, &mut pending0, &mut pending1, 0, [0, 1], false), Err(_));
        assert_matches!(init_verification_validate_nullifier_hashes(&mut verification_account, &mut pending0, &mut pending1, 0, [0, 1], true), Ok(()));

        verification_account.set_setup_state(&VerificationSetupState::None);

        // Third nullifier (second MT) is duplicate
        let mut nullifier_map = PendingNullifierHashesMap::new();
        nullifier_map.try_insert(U256Limbed2::from(u256_from_str("3")), 0).unwrap();
        pending_nulliier_map!(pending0);
        pending_nulliier_map!(pending1, nullifier_map);

        assert_matches!(init_verification_validate_nullifier_hashes(&mut verification_account, &mut pending0, &mut pending1, 0, [0, 1], false), Err(_));
        assert_matches!(init_verification_validate_nullifier_hashes(&mut verification_account, &mut pending0, &mut pending1, 0, [0, 1], true), Ok(()));

        verification_account.set_setup_state(&VerificationSetupState::None);
        pending_nulliier_map!(pending1);

        // Map is full
        let mut nullifier_map = PendingNullifierHashesMap::new();
        for i in 0..PendingNullifierHashesMap::MAX_ELEMENTS_COUNT {
            nullifier_map.try_insert(U256Limbed2([0, i as u128]), 0).unwrap();
        }
        pending_nulliier_map!(pending0, nullifier_map);
        assert_matches!(init_verification_validate_nullifier_hashes(&mut verification_account, &mut pending0, &mut pending1, 0, [0, 1], false), Err(_));

        // Success
        pending_nulliier_map!(pending0);
        assert_matches!(init_verification_validate_nullifier_hashes(&mut verification_account, &mut pending0, &mut pending1, 0, [0, 1], false), Ok(()));
    }

    #[test]
    fn test_init_verification_public_inputs() {
        let mut data = vec![0; VerificationAccount::SIZE];
        let mut verification_account = VerificationAccount::new(&mut data).unwrap();

        let public_inputs = SendPublicInputs{
            join_split: JoinSplitPublicInputs {
                commitment_count: 1,
                roots: vec![
                    Some(EMPTY_TREE[MT_HEIGHT as usize]),
                ],
                nullifier_hashes: vec![
                    u256_from_str("12345"),
                ],
                commitment: u256_from_str("1111"),
                fee_version: 12034,
                amount: LAMPORTS_PER_SOL,
            },
            recipient: u256_from_str("22222"),
            current_time: 123456,
            identifier: u256_from_str("3333333"),
            salt: u256_from_str("44444444"),
        };
        verification_account.save_raw_public_inputs(&public_inputs.public_signals());

        // Verification account is already setup
        verification_account.set_setup_state(&VerificationSetupState::PublicInputsSetup);
        assert_matches!(init_verification_public_inputs(&mut verification_account, 0), Err(_));
        verification_account.set_setup_state(&VerificationSetupState::NullifiersChecked);

        // Computation is finished
        verification_account.set_is_verified(&ElusivOption::Some(true));
        assert_matches!(init_verification_public_inputs(&mut verification_account, 0), Err(_));
        verification_account.set_is_verified(&ElusivOption::Some(false));
        assert_matches!(init_verification_public_inputs(&mut verification_account, 0), Err(_));
        verification_account.set_is_verified(&ElusivOption::None);

        // Success
        assert_matches!(init_verification_public_inputs(&mut verification_account, 0), Ok(()));

        assert_matches!(verification_account.get_setup_state(), VerificationSetupState::PublicInputsSetup);
        let expected = public_inputs.public_signals_big_integer();
        for (i, &expected) in expected.iter().enumerate() {
            assert_eq!(verification_account.get_public_input(i).0, expected);
        }

        // Second time will fail
        assert_matches!(init_verification_public_inputs(&mut verification_account, 0), Err(_));
    }

    #[test]
    fn test_init_verification_proof() {
        let mut data = vec![0; VerificationAccount::SIZE];
        let mut verification_account = VerificationAccount::new(&mut data).unwrap();

        let proof = test_proof();
        let raw_proof = proof.try_to_vec().unwrap().try_into().unwrap();
        let valid_pk = Pubkey::new(&[0; 32]);
        account!(fee_payer, valid_pk, vec![0; 0]);

        // Account not setup
        verification_account.set_setup_state(&VerificationSetupState::NullifiersChecked);
        assert_matches!(init_verification_proof(&fee_payer, &mut verification_account, 0, raw_proof), Err(_));
        verification_account.set_setup_state(&VerificationSetupState::PublicInputsSetup);

        // Computation already finished
        verification_account.set_is_verified(&ElusivOption::Some(true));
        assert_matches!(init_verification_proof(&fee_payer, &mut verification_account, 0, raw_proof), Err(_));
        verification_account.set_is_verified(&ElusivOption::Some(false));
        assert_matches!(init_verification_proof(&fee_payer, &mut verification_account, 0, raw_proof), Err(_));
        verification_account.set_is_verified(&ElusivOption::None);

        // Invalid fee_payer
        let invalid_pk = Pubkey::new_unique();
        account!(invalid_fee_payer, invalid_pk, vec![0; 0]);
        assert_matches!(init_verification_proof(&invalid_fee_payer, &mut verification_account, 0, raw_proof), Err(_));

        // Success
        assert_matches!(init_verification_proof(&fee_payer, &mut verification_account, 0, raw_proof), Ok(()));
        assert_matches!(verification_account.get_setup_state(), VerificationSetupState::ProofSetup);
        assert_eq!(verification_account.a.get(), proof.a);
        assert_eq!(verification_account.b.get(), proof.b);
        assert_eq!(verification_account.c.get(), proof.c);

        // Already setup proof
        assert_matches!(init_verification_proof(&fee_payer, &mut verification_account, 0, raw_proof), Err(_));
    }


    #[test]
    fn test_compute_verification() {
        let mut data = vec![0; VerificationAccount::SIZE];
        let mut verification_account = VerificationAccount::new(&mut data).unwrap();

        // Setup
        let proof = test_proof();
        verification_account.a.set(&proof.a);
        verification_account.b.set(&proof.b);
        verification_account.c.set(&proof.c);
        let public_inputs = test_public_inputs();
        for (i, &public_input) in public_inputs.iter().enumerate() {
            verification_account.set_public_input(i, &Wrap(public_input));
        }
        let instructions = prepare_public_inputs_instructions::<TestVKey>(&public_inputs);
        verification_account.set_prepare_inputs_instructions_count(&(instructions.len() as u32));
        for (i, &ix) in instructions.iter().enumerate() {
            verification_account.set_prepare_inputs_instructions(i, &(ix as u16));
        }

        // Public inputs not setup
        verification_account.set_setup_state(&VerificationSetupState::NullifiersChecked);
        assert_matches!(compute_verification(&mut verification_account, 0), Err(_));
        verification_account.set_setup_state(&VerificationSetupState::ProofSetup);

        // Computation is already finished (is_verified is Some)
        verification_account.set_is_verified(&ElusivOption::Some(true));
        assert_matches!(compute_verification(&mut verification_account, 0), Err(_));
        verification_account.set_is_verified(&ElusivOption::None);

        // Success for public input preparation
        for _ in 0..instructions.len() {
            assert_matches!(compute_verification(&mut verification_account, 0), Ok(()));
        }

        // Failure for miller loop (proof not setup)
        verification_account.set_setup_state(&VerificationSetupState::PublicInputsSetup);
        assert_matches!(compute_verification(&mut verification_account, 0), Err(_));
        verification_account.set_setup_state(&VerificationSetupState::ProofSetup);

        // Success
        for _ in 0..COMBINED_MILLER_LOOP_IXS + FINAL_EXPONENTIATION_IXS {
            assert_matches!(compute_verification(&mut verification_account, 0), Ok(()));
        }

        // Computation is finished
        assert_matches!(compute_verification(&mut verification_account, 0), Err(_));

        //assert_matches!(compute_verification(&mut verification_account, 0), Err(_));
        //assert_matches!(verification_account.get_is_verified().option(), Some(false));
    }

    #[test]
    fn test_is_timestamp_valid() {
        assert!(is_timestamp_valid(0, 1));
        assert!(is_timestamp_valid(two_pow!(5) as u64 - 1, 0));

        assert!(!is_timestamp_valid(two_pow!(5) as u64, 0));
    }

    #[test]
    fn test_is_vec_duplicate_free() {
        assert!(is_vec_duplicate_free(&<Vec<u8>>::new()));
        assert!(is_vec_duplicate_free(&vec![0]));
        assert!(is_vec_duplicate_free(&vec![0, 1, 2]));

        assert!(!is_vec_duplicate_free(&vec![0, 1, 2, 0]));
        assert!(!is_vec_duplicate_free(&vec![0, 1, 0, 2]));
        assert!(!is_vec_duplicate_free(&vec![0, 0]));
    }

    #[test]
    fn test_check_join_split_public_inputs() {
        storage_account!(storage);
        nullifier_account!(n_account);

        let valid_inputs = JoinSplitPublicInputs {
            commitment_count: 1,
            roots: vec![
                Some(EMPTY_TREE[MT_HEIGHT as usize]),
            ],
            nullifier_hashes: vec![
                u256_from_str("1"),
            ],
            commitment: u256_from_str("1"),
            fee_version: 0,
            amount: 0,
        };

        let invalid_public_inputs = [
            // Zero-commitment
            mutate(&valid_inputs, |inputs| {
                inputs.commitment = ZERO_COMMITMENT;
            }),

            // Invalid root for active MT
            mutate(&valid_inputs, |inputs| {
                inputs.roots[0] = Some([0; 32]);
            }),

            // First root is None
            mutate(&valid_inputs, |inputs| {
                inputs.roots[0] = None;
            }),

            // Mismatched nullifier_hashes amount
            mutate(&valid_inputs, |inputs| {
                inputs.commitment_count = 2;
            }),

            // Same nullifier_hash supplied twice for same MT
            mutate(&valid_inputs, |inputs| {
                inputs.commitment_count = 2;
                inputs.nullifier_hashes = vec![
                    u256_from_str("0"),
                    u256_from_str("0"),
                ];
                inputs.roots.push(None);
            }),

            // Invalid root in closed MT
            mutate(&valid_inputs, |inputs| {
                inputs.commitment_count = 2;
                inputs.nullifier_hashes = vec![
                    u256_from_str("0"),
                    u256_from_str("0"),
                ];
                inputs.roots.push(Some(EMPTY_TREE[MT_HEIGHT as usize]));
            }),
        ];

        for public_inputs in invalid_public_inputs {
            assert_matches!(
                check_join_split_public_inputs(&public_inputs, &storage, [&n_account, &n_account], &[0, 1]),
                Err(_)
            );
        }

        // Same MT supplied twice
        assert_matches!(
            check_join_split_public_inputs(
                &mutate(&valid_inputs, |inputs| {
                    inputs.commitment_count = 2;
                    inputs.nullifier_hashes = vec![
                        u256_from_str("0"),
                        u256_from_str("0"),
                    ];
                    inputs.roots.push(Some(u256_from_str("0")));
                }),
                &storage, [&n_account, &n_account], &[0, 0]
            ),
            Err(_)
        );

        // Success
        assert_matches!(
            check_join_split_public_inputs(&valid_inputs, &storage, [&n_account, &n_account], &[0, 1]),
            Ok(())
        );

        let valid_public_inputs = [
            // Same nullifier_hash supplied twice for different MT
            mutate(&valid_inputs, |inputs| {
                inputs.commitment_count = 2;
                inputs.nullifier_hashes = vec![
                    u256_from_str("0"),
                    u256_from_str("0"),
                ];
                inputs.roots.push(Some(u256_from_str("0")));
            }),
        ];

        for public_inputs in valid_public_inputs {
            assert_matches!(
                check_join_split_public_inputs(&public_inputs, &storage, [&n_account, &n_account], &[0, 1]),
                Ok(())
            );
        }

        // Duplicate nullifier_hash already exists
        let mut map = NullifierMap::new();
        map.try_insert(U256Limbed2::from(u256_from_str("1")), ()).unwrap();
        let mut data = vec![1];
        map.serialize(&mut data).unwrap();
        let pk = Pubkey::new_unique();
        account!(nullifier, pk, data);
        hash_map!(acc, (0usize, &nullifier));
        let mut data = vec![0; NullifierAccount::SIZE];
        let mut n_account = NullifierAccount::new(&mut data, acc).unwrap();
        n_account.set_nullifiers_count(&1);

        assert_matches!(
            check_join_split_public_inputs(
                &mutate(&valid_inputs, |inputs| {
                    inputs.nullifier_hashes = vec![
                        u256_from_str("1"),
                    ];
                }),
                &storage, [&n_account, &n_account], &[0, 1]
            ),
            Err(_)
        );
    }

    fn test_proof() -> Proof {
        proof_from_str(
            (
                "10026859857882131638516328056627849627085232677511724829502598764489185541935",
                "19685960310506634721912121951341598678325833230508240750559904196809564625591",
                false,
            ),
            (
                (
                    "857882131638516328056627849627085232677511724829502598764489185541935",
                    "685960310506634721912121951341598678325833230508240750559904196809564625591",
                ),
                (
                    "837064132573119120838379738103457054645361649757131991036638108422638197362",
                    "86803555845400161937398579081414146527572885637089779856221229551142844794",
                ),
                    false,
            ),
            (
                "21186803555845400161937398579081414146527572885637089779856221229551142844794",
                "85960310506634721912121951341598678325833230508240750559904196809564625591",
                false,
            ),
        )
    }

    fn test_public_inputs() -> Vec<BigInteger256> {
        vec![
            "7889586699914970744657798935358222218486353295005298675075639741334684257960",
            "9606705614694883961284553030253534686862979817135488577431113592919470999200",
            "7548080684044753634901903467536594261850721059805517798311616241293112471457",
            "7548080684044753634901903467536594261850721059805517798311616241293112471457",
            "7548080684044753634901903467536594261850721059805517798311616241293112471457",
            "17718047633435172913528840327177336488970255844461341542131787100983543190394",
            "17718047633435172913528840327177336488970255844461341542131787100983543190394",
            "0",
            "0",
            "340282366920938463463374607431768211455",
            "340282366920938463463374607431768211455",
            "120000",
            "1657140479",
            "1",
            "2",
            "2827970856290632118729271546490749634442294169342908710567180510922374163316",
        ].iter().map(|s| Fr::from_str(s).unwrap().into_repr()).collect()
    }
}