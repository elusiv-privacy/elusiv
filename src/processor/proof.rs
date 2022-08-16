use std::collections::HashSet;
use solana_program::{
    entrypoint::ProgramResult,
    account_info::AccountInfo,
    clock::Clock,
    sysvar::Sysvar,
};
use crate::macros::{guard, BorshSerDeSized, EnumVariantIndex, pda_account};
use crate::processor::ZERO_COMMITMENT_RAW;
use crate::processor::utils::{open_pda_account_with_offset, close_account, open_pda_account, transfer_token};
use crate::proof::precompute::PrecomputesAccount;
use crate::proof::{prepare_public_inputs_instructions, verify_partial, VerificationAccountData, VerificationState};
use crate::state::MT_COMMITMENT_COUNT;
use crate::state::program_account::PDAAccountData;
use crate::state::queue::{CommitmentQueue, CommitmentQueueAccount, Queue, RingQueue};
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
    InvalidMerkleRoot,
    InvalidPublicInputs,
    InvalidInstructionData,
    ComputationIsAlreadyFinished,
    ComputationIsNotYetFinished,
    CouldNotInsertNullifier,
    InvalidFeeVersion,
    FeatureNotAvailable,
};
use crate::proof::{
    VerificationAccount,
    vkey::{SendQuadraVKey, MigrateUnaryVKey},
};
use crate::token::{Token, verify_token_account, TokenPrice};
use crate::types::{RawProof, SendPublicInputs, MigratePublicInputs, PublicInputs, JoinSplitPublicInputs, U256, Proof, RawU256};
use crate::bytes::{BorshSerDeSized, BorshSerDeSizedEnum, ElusivOption, usize_as_u32_safe};
use borsh::{BorshSerialize, BorshDeserialize};

use super::CommitmentHashRequest;

#[derive(BorshSerialize, BorshDeserialize, BorshSerDeSized, EnumVariantIndex, PartialEq, Debug, Clone)]
pub enum ProofRequest {
    Send(SendPublicInputs),
    Merge(SendPublicInputs),
    Migrate(MigratePublicInputs),
}

macro_rules! execute_with_vkey {
    ($kind: expr, $vk: ident, $e: expr) => {
        match $kind {
            0 => { type $vk = SendQuadraVKey; $e }
            1 => { type $vk = SendQuadraVKey; $e }
            2 => { type $vk = MigrateUnaryVKey; $e }
            _ => panic!()
        }
    };
}

macro_rules! proof_request {
    ($request: expr, $public_inputs: ident, $e: expr) => {
        match $request {
            ProofRequest::Send($public_inputs) => { $e },
            ProofRequest::Merge($public_inputs) => { $e },
            ProofRequest::Migrate($public_inputs) => { $e },
        }
    };
}

impl ProofRequest {
    pub fn fee_version(&self) -> u32 {
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
/// Initializes a new proof verification
pub fn init_verification<'a, 'b, 'c, 'd>(
    fee_payer: &AccountInfo<'a>,
    fee_payer_token_account: &AccountInfo<'a>,
    pool: &AccountInfo<'a>,
    pool_account: &AccountInfo<'a>,
    fee_collector: &AccountInfo<'a>,
    fee_collector_account: &AccountInfo<'a>,

    sol_usd_price_account: &AccountInfo,
    token_usd_price_account: &AccountInfo,

    governor: &GovernorAccount,
    verification_account: &AccountInfo<'a>,
    nullifier_duplicate_account: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,

    recipient: &AccountInfo,
    storage_account: &StorageAccount,
    n_acc_0: &NullifierAccount<'b, 'c, 'd>,
    n_acc_1: &NullifierAccount<'b, 'c, 'd>,

    verification_account_index: u32,
    tree_indices: [u32; MAX_MT_COUNT],
    request: ProofRequest,
) -> ProgramResult {
    let vkey = request.variant_index();
    let raw_public_inputs = proof_request!(
        &request,
        public_inputs,
        public_inputs.public_signals()
    );
    let instructions = execute_with_vkey!(
        vkey,
        VKey,
        prepare_public_inputs_instructions::<VKey>(
            &proof_request!(
                &request,
                public_inputs,
                public_inputs.public_signals_skip_mr()
            )
        )
    );

    // Verify public inputs
    let join_split = match &request {
        ProofRequest::Send(public_inputs) => {
            guard!(public_inputs.verify_additional_constraints(), InvalidPublicInputs);

            guard!(recipient.key.to_bytes() == public_inputs.recipient.skip_mr(), InvalidAccount);
            guard!(verify_token_account(recipient, public_inputs.join_split.token_id)?, InvalidAccount);

            if cfg!(not(test)) {
                let clock = Clock::get()?;
                let current_timestamp: u64 = clock.unix_timestamp.try_into().unwrap();
                guard!(is_timestamp_valid(public_inputs.current_time, current_timestamp), InvalidInstructionData);
            }
            &public_inputs.join_split
        }
        ProofRequest::Merge(public_inputs) => {
            guard!(public_inputs.join_split.amount == 0, InvalidAmount);
            guard!(public_inputs.verify_additional_constraints(), InvalidPublicInputs);
            &public_inputs.join_split
        }
        ProofRequest::Migrate(_) => {
            // Migrate from archived MTs not implemented yet
            return Err(FeatureNotAvailable.into())
        }
    };
    check_join_split_public_inputs(
        join_split,
        storage_account,
        [n_acc_0, n_acc_1],
        &tree_indices,
    )?;

    guard!(request.fee_version() == governor.get_fee_version(), InvalidFeeVersion);
    let token_id = join_split.token_id;
    let price = TokenPrice::new(sol_usd_price_account, token_usd_price_account, token_id)?;
    let min_batching_rate = governor.get_commitment_batching_rate();
    let fee = governor.get_program_fee();
    let subvention = fee.proof_subvention.into_token(&price, token_id)?;
    let proof_verification_fee = fee.proof_verification_computation_fee(instructions.len()).into_token(&price, token_id)?;
    let commitment_hash_fee = fee.commitment_hash_computation_fee(min_batching_rate);
    let commitment_hash_fee_token = commitment_hash_fee.into_token(&price, token_id)?;
    let network_fee = Token::new_checked(token_id, fee.proof_network_fee.calc(join_split.amount))?;
    //verify_program_token_accounts(fee_collector, fee_collector_account, pool, pool_account, token_id)?;

    // `fee_collector` transfers `subvention` to `pool`
    transfer_token(
        fee_collector,
        fee_collector_account,
        pool_account,
        token_program,
        subvention,
    )?;

    // `fee_payer` transfers `commitment_hash_fee` to `pool`
    transfer_token(
        fee_payer,
        fee_payer,
        pool,
        system_program,
        commitment_hash_fee.into_token_strict(),
    )?;

    let fee = (((commitment_hash_fee_token + proof_verification_fee)? + network_fee)? - subvention)?;
    guard!(fee.amount() == join_split.fee, InvalidPublicInputs);

    // Open `nullifier_duplicate_account`
    let nullifier_hashes: Vec<U256> = join_split.nullifier_hashes.iter().map(|n| n.skip_mr()).collect();
    let nullifier_hashes: Vec<&[u8]> = nullifier_hashes.iter().map(|n| &n[..]).collect();
    open_pda_account(
        fee_payer,
        nullifier_duplicate_account,
        PDAAccountData::SIZE,
        &nullifier_hashes,
    )?;

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
            fee_payer: RawU256::new(fee_payer_token_account.key.to_bytes()),
            nullifier_duplicate_pda: RawU256::new(nullifier_duplicate_account.key.to_bytes()),
            min_batching_rate,
            subvention: subvention.amount(),
            commitment_hash_fee,
            commitment_hash_fee_token: commitment_hash_fee_token.amount(),
            proof_verification_fee: proof_verification_fee.amount(),
            network_fee: network_fee.amount(),
        },
        request,
        tree_indices,
    )
}

/// Called once after `init_verification` to initialize the proof's public inputs
/// - Note: has to be called by the original `fee_payer`, that called `init_verification`
/// - depending on the MT-count this has to be called in a different tx than the init-tx
pub fn init_verification_proof(
    fee_payer: &AccountInfo,
    verification_account: &mut VerificationAccount,

    _verification_account_index: u32,
    proof: RawProof,
) -> ProgramResult {
    guard!(matches!(verification_account.get_state(), VerificationState::None), InvalidAccountState);
    guard!(verification_account.get_is_verified().option().is_none(), ComputationIsAlreadyFinished);
    guard!(verification_account.get_other_data().fee_payer.skip_mr() == fee_payer.key.to_bytes(), InvalidAccount);

    let proof: Proof = proof.try_into()?;
    verification_account.a.set(&proof.a);
    verification_account.b.set(&proof.b);
    verification_account.c.set(&proof.c);

    verification_account.set_state(&VerificationState::ProofSetup);

    Ok(())
}

/// Partial proof verification computation
pub fn compute_verification(
    verification_account: &mut VerificationAccount,
    precomputes_account: &PrecomputesAccount,

    _verification_account_index: u32,
) -> ProgramResult {
    guard!(precomputes_account.get_is_setup(), InvalidAccountState);
    guard!(
        matches!(verification_account.get_state(), VerificationState::None) ||
        matches!(verification_account.get_state(), VerificationState::ProofSetup),
        InvalidAccountState
    );
    guard!(verification_account.get_is_verified().option().is_none(), ComputationIsAlreadyFinished);

    match execute_with_vkey!(
        verification_account.get_kind(),
        VKey,
        verify_partial::<_, VKey>(verification_account, precomputes_account)
    ) {
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

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Clone, Copy)]
pub struct FinalizeSendData {
    pub timestamp: u64,
    pub total_amount: u64,
    pub token_id: u16,

    /// Estimated index of the MT in which the next-commitment will be inserted
    pub mt_index: u32,

    /// Estimated index of the next-commitment in the MT
    pub commitment_index: u32,
}

/// First finalize instruction
/// - for valid proof finalization: `finalize_verification_send, `finalize_verification_send_nullifiers`, `finalize_verification_transfer`
/// - for invalid proof: `finalize_verification_send`, `finalize_verification_transfer`
pub fn finalize_verification_send(
    identifier_account: &AccountInfo,
    salt_account: &AccountInfo,
    commitment_hash_queue: &mut CommitmentQueueAccount,
    verification_account: &mut VerificationAccount,
    storage_account: &StorageAccount,

    data: FinalizeSendData,
    _verification_account_index: u32,
) -> ProgramResult {
    guard!(matches!(verification_account.get_state(), VerificationState::ProofSetup), InvalidAccountState);

    match verification_account.get_is_verified() {
        ElusivOption::None => return Err(ComputationIsNotYetFinished.into()),
        ElusivOption::Some(false) => {
            verification_account.set_state(&VerificationState::Finalized);
            return Ok(())
        }
        _ => {}
    }

    let request = verification_account.get_request();
    let public_inputs = match request {
        ProofRequest::Send(public_inputs) => public_inputs,
        ProofRequest::Merge(public_inputs) => public_inputs,
        _ => return Err(FeatureNotAvailable.into())
    };

    guard!(identifier_account.key.to_bytes() == public_inputs.identifier.skip_mr(), InvalidAccount);
    guard!(salt_account.key.to_bytes() == public_inputs.salt.skip_mr(), InvalidAccount);

    let (commitment_index, mt_index) = minimum_commitment_mt_index(
        storage_account.get_trees_count(),
        storage_account.get_next_commitment_ptr(),
        CommitmentQueue::new(commitment_hash_queue).len()
    );
    guard!(data.timestamp == public_inputs.current_time, InvalidInstructionData);
    guard!(data.total_amount == public_inputs.join_split.total_amount(), InvalidInstructionData);
    guard!(data.token_id == public_inputs.join_split.token_id, InvalidInstructionData);
    guard!(data.commitment_index == commitment_index, InvalidInstructionData);
    guard!(data.mt_index == mt_index, InvalidInstructionData);

    verification_account.set_state(&VerificationState::InsertNullifiers);

    Ok(())
}

pub fn finalize_verification_send_nullifiers<'a, 'b, 'c>(
    verification_account: &mut VerificationAccount,
    n_acc_0: &mut NullifierAccount<'a, 'b, 'c>,
    n_acc_1: &mut NullifierAccount<'a, 'b, 'c>,

    _verification_account_index: u32,
) -> ProgramResult {
    guard!(matches!(verification_account.get_state(), VerificationState::InsertNullifiers), InvalidAccountState);

    let request = verification_account.get_request();
    let public_inputs = match request {
        ProofRequest::Send(public_inputs) => public_inputs,
        ProofRequest::Merge(public_inputs) => public_inputs,
        _ => return Err(FeatureNotAvailable.into())
    };

    let nullifier_accounts: [&mut NullifierAccount<'a, 'b, 'c>; MAX_MT_COUNT] = [n_acc_0, n_acc_1];
    let mut tree_index = 0;
    for (i, root) in public_inputs.join_split.roots.iter().enumerate() {
        let nullifier_hash = public_inputs.join_split.nullifier_hashes[i].reduce();
        let index = match root {
            Some(_) => {
                let t = tree_index;
                tree_index += 1;
                t
            }
            None => 0,
        };
        nullifier_accounts[index].try_insert_nullifier_hash(nullifier_hash)?;
    }

    verification_account.set_state(&VerificationState::Finalized);

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn finalize_verification_transfer<'a>(
    recipient: &AccountInfo<'a>, // can be any account for merge/migrate
    original_fee_payer: &AccountInfo<'a>,
    pool: &AccountInfo<'a>,
    pool_account: &AccountInfo<'a>,
    fee_collector: &AccountInfo<'a>,
    fee_collector_account: &AccountInfo<'a>,

    commitment_hash_queue: &mut CommitmentQueueAccount,
    verification_account_info: &AccountInfo<'a>,
    nullifier_duplicate_account: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,

    _verification_account_index: u32,
) -> ProgramResult {
    pda_account!(mut verification_account, VerificationAccount, verification_account_info);
    let data = verification_account.get_other_data();
    let request = verification_account.get_request();
    let join_split = proof_request!(&request, public_inputs, public_inputs.join_split_inputs());

    guard!(matches!(verification_account.get_state(), VerificationState::Finalized), InvalidAccountState);
    guard!(nullifier_duplicate_account.key.to_bytes() == data.nullifier_duplicate_pda.skip_mr(), InvalidAccount);

    guard!(original_fee_payer.key.to_bytes() == data.fee_payer.skip_mr(), InvalidAccount);

    let token_id = join_split.token_id;

    if let ElusivOption::Some(false) = verification_account.get_is_verified() {
        // `rent` and `commitment_hash_fee` flow to `fee_collector`
        close_account(fee_collector, verification_account_info)?;
        close_account(fee_collector, nullifier_duplicate_account)?;
        verification_account.set_state(&VerificationState::Closed);

        // `pool` transfers `subvention` to `fee_collector`
        let subvention = Token::new_checked(token_id, data.subvention)?;
        transfer_token(
            pool,
            pool_account,
            fee_collector_account,
            token_program,
            subvention,
        )?;

        // `pool` transfers `commitment_hash_fee` to `fee_collector`
        transfer_token(
            pool,
            pool_account,
            fee_collector,
            system_program,
            data.commitment_hash_fee.into_token_strict(),
        )?;

        return Ok(())
    }

    if let ProofRequest::Send(public_inputs) = &request {
        guard!(recipient.key.to_bytes() == public_inputs.recipient.skip_mr(), InvalidAccount);

        // `pool` transfers `amount` to `recipient`
        transfer_token(
            pool,
            pool_account,
            recipient,
            token_program,
            Token::new_checked(token_id, public_inputs.join_split.amount)?,
        )?;
    }

    // `pool` transfers `commitment_hash_fee_token + proof_verification_fee` to `fee_payer`
    transfer_token(
        pool,
        pool_account,
        original_fee_payer,
        token_program,
        (
            Token::new_checked(token_id, data.commitment_hash_fee_token)? +
            Token::new_checked(token_id, data.proof_verification_fee)?
        )?
    )?;

    // `pool` transfers `network_fee` to `fee_collector`
    transfer_token(
        pool,
        pool_account,
        fee_collector_account,
        token_program,
        Token::new_checked(token_id, data.network_fee)?,
    )?;

    // Close `verification_account` and `nullifier_duplicate_account`
    if cfg!(not(test)) {
        close_account(original_fee_payer, verification_account_info)?;
        close_account(original_fee_payer, nullifier_duplicate_account)?;
    }

    let mut commitment_queue = CommitmentQueue::new(commitment_hash_queue);
    commitment_queue.enqueue(
        CommitmentHashRequest {
            commitment: join_split.commitment.reduce(),
            fee_version: join_split.fee_version,
            min_batching_rate: data.min_batching_rate,
        }
    )?;

    verification_account.set_state(&VerificationState::Closed);

    Ok(())
}

const TIMESTAMP_BITS_PRUNING: usize = 5;
fn is_timestamp_valid(asserted_time: u64, timestamp: u64) -> bool {
    (asserted_time >> TIMESTAMP_BITS_PRUNING) <= (timestamp >> TIMESTAMP_BITS_PRUNING)
}

fn is_vec_duplicate_free<T: std::cmp::Eq + std::hash::Hash + std::clone::Clone>(v: &Vec<T>) -> bool {
    (*v).clone().drain(..).collect::<HashSet<T>>().len() == v.len()
}

/// Computes the minimum index of a commitment and it's corresponding MT-index
fn minimum_commitment_mt_index(
    mt_index: u32,
    commitment_count: u32,
    commitment_queue_len: u32,
) -> (u32, u32) {
    let count = usize_as_u32_safe(MT_COMMITMENT_COUNT);
    let index = (commitment_count + commitment_queue_len) % count;
    let mt_offset = (commitment_count + commitment_queue_len) / count;
    (index, mt_index + mt_offset)
}

fn check_join_split_public_inputs(
    public_inputs: &JoinSplitPublicInputs,
    storage_account: &StorageAccount,
    nullifier_accounts: [&NullifierAccount; 2],
    tree_indices: &[u32; MAX_MT_COUNT],
) -> ProgramResult {
    // Check that the resulting commitment is not the zero-commitment
    guard!(public_inputs.commitment.skip_mr() != ZERO_COMMITMENT_RAW, InvalidPublicInputs);

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
                // - Note: roots are stored in mr-form
                if tree_indices[index] == active_tree_index { // Active tree
                    guard!(storage_account.is_root_valid(root.reduce()), InvalidMerkleRoot);
                } else { // Closed tree
                    guard!(root.reduce() == nullifier_accounts[index].get_root(), InvalidMerkleRoot);
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
        // - Note: nullifier-hashes are stored in mr-form
        guard!(
            nullifier_accounts[tree_index[i]].can_insert_nullifier_hash(public_inputs.nullifier_hashes[i].reduce())?,
            CouldNotInsertNullifier
        );
    }

    Ok(())
}

/*#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use assert_matches::assert_matches;
    use solana_program::native_token::LAMPORTS_PER_SOL;
    use solana_program::pubkey::Pubkey;
    use crate::fields::{u256_from_str, u256_from_str_skip_mr};
    use crate::processor::ZERO_COMMITMENT_RAW;
    use crate::proof::precompute::{VirtualPrecomputes, precompute_account_size};
    use crate::proof::{COMBINED_MILLER_LOOP_IXS, FINAL_EXPONENTIATION_IXS, proof_from_str};
    use crate::state::fee::ProgramFee;
    use crate::state::governor::{PoolAccount, FeeCollectorAccount};
    use crate::state::empty_root_raw;
    use crate::state::program_account::{SizedAccount, PDAAccount, MultiAccountProgramAccount, MultiAccountAccount};
    use crate::macros::{two_pow, zero_account, account, test_account_info, storage_account, nullifier_account, hash_map};
    use crate::types::{RawU256, Proof, compute_fee_rec};

    fn mutate<T: Clone, F>(v: &T, f: F) -> T where F: Fn(&mut T) {
        let mut i = v.clone();
        f(&mut i);
        i
    }

    #[test]
    #[allow(unused_mut)]
    fn test_init_verification() {
        test_account_info!(fee_payer, 0);
        zero_account!(mut governor, GovernorAccount);
        test_account_info!(pool, 0);
        test_account_info!(fee_collector, 0);
        governor.set_program_fee(&ProgramFee::default());
        let pda = VerificationAccount::find(Some(0)).0;
        account!(verification_account, pda, vec![0; VerificationAccount::SIZE]);

        let mut send_public_inputs = SendPublicInputs {
            join_split: JoinSplitPublicInputs {
                commitment_count: 1,
                roots: vec![
                    Some(empty_root_raw()),
                ],
                nullifier_hashes: vec![
                    RawU256::new(u256_from_str_skip_mr("1")),
                ],
                commitment: RawU256::new(u256_from_str_skip_mr("1")),
                fee_version: 0,
                amount: LAMPORTS_PER_SOL,
                fee: 0,
                token_id: 0,
            },
            recipient: RawU256::new(u256_from_str_skip_mr("1")),
            current_time: 0,
            identifier: RawU256::new(u256_from_str_skip_mr("1")),
            salt: RawU256::new(u256_from_str_skip_mr("1")),
        };
        compute_fee_rec::<SendQuadraVKey, _>(&mut send_public_inputs, &ProgramFee::default());

        let mut merge_public_inputs = SendPublicInputs{
            join_split: JoinSplitPublicInputs {
                commitment_count: 1,
                roots: vec![
                    Some(empty_root_raw()),
                ],
                nullifier_hashes: vec![
                    RawU256::new(u256_from_str_skip_mr("1")),
                ],
                commitment: RawU256::new(u256_from_str_skip_mr("1")),
                fee_version: 0,
                amount: 0,
                fee: 0,
                token_id: 0,
            },
            recipient: RawU256::new(u256_from_str_skip_mr("1")),
            current_time: 0,
            identifier: RawU256::new(u256_from_str_skip_mr("1")),
            salt: RawU256::new(u256_from_str_skip_mr("1")),
        };
        compute_fee_rec::<SendQuadraVKey, _>(&mut merge_public_inputs, &ProgramFee::default());

        let nullifier_duplicate_pda = send_public_inputs.join_split.nullifier_duplicate_pda().0;
        account!(nullifier_duplicate_account, nullifier_duplicate_pda, vec![1]);

        struct InitVerificationTest {
            verification_account_index: u32,
            tree_indices: [u32; MAX_MT_COUNT],
            request: ProofRequest,
            success: bool,
        }

        let tests = [
            // Send: invalid fee
            InitVerificationTest {
                verification_account_index: 0,
                tree_indices: [0, 0],
                request: ProofRequest::Send(
                    mutate(&send_public_inputs, |public_inputs| {
                        public_inputs.join_split.fee = 0;
                    })
                ),
                success: false,
            },

            // Merge: Invalid amount
            InitVerificationTest {
                verification_account_index: 0,
                tree_indices: [0, 0],
                request: ProofRequest::Merge(
                    mutate(&merge_public_inputs, |public_inputs| {
                        public_inputs.join_split.amount = 1;
                    })
                ),
                success: false,
            },

            // Send: commitment-count too low
            InitVerificationTest {
                verification_account_index: 0,
                tree_indices: [0, 0],
                request: ProofRequest::Send(
                    mutate(&send_public_inputs, |public_inputs| {
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
                    mutate(&send_public_inputs, |public_inputs| {
                        public_inputs.join_split.commitment_count = 4 + 1;
                    })
                ),
                success: false,
            },

            // Merge: invalid fee
            InitVerificationTest {
                verification_account_index: 0,
                tree_indices: [0, 0],
                request: ProofRequest::Merge(
                    mutate(&merge_public_inputs, |public_inputs| {
                        public_inputs.join_split.fee = 0;
                    })
                ),
                success: false,
            },

            // Merge: valid fee
            InitVerificationTest {
                verification_account_index: 0,
                tree_indices: [0, 0],
                request: ProofRequest::Merge(merge_public_inputs.clone()),
                success: true,
            },

            InitVerificationTest {
                verification_account_index: 0,
                tree_indices: [0, 0],
                request: ProofRequest::Send(send_public_inputs.clone()),
                success: true,
            },

            // Migrate: failure 
            InitVerificationTest {
                verification_account_index: 0,
                tree_indices: [0, 0],
                request: ProofRequest::Migrate(
                    MigratePublicInputs {
                        join_split: send_public_inputs.join_split,
                        current_nsmt_root: RawU256::new([0; 32]),
                        next_nsmt_root: RawU256::new([0; 32]),
                    }
                ),
                success: false,
            },
        ];

        storage_account!(storage_account);
        nullifier_account!(nullifier_account);

        for test in tests {
            let result = init_verification(
                &fee_payer,
                &governor,
                &pool,
                &fee_collector,
                &verification_account,
                &nullifier_duplicate_account,
                &storage_account,
                &nullifier_account,
                &nullifier_account,
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

        // TODO: Invalid nullifier_duplicate_account
    }

    #[test]
    fn test_init_verification_proof() {
        let mut data = vec![0; VerificationAccount::SIZE];
        let mut verification_account = VerificationAccount::new(&mut data).unwrap();

        let proof = test_proof();
        let raw_proof = proof.try_into().unwrap();
        let valid_pk = Pubkey::new(&[0; 32]);
        account!(fee_payer, valid_pk, vec![0; 0]);

        // Account setup
        verification_account.set_state(&VerificationState::ProofSetup);
        assert_matches!(init_verification_proof(&fee_payer, &mut verification_account, 0, raw_proof), Err(_));
        verification_account.set_state(&VerificationState::None);

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
        assert_matches!(verification_account.get_state(), VerificationState::ProofSetup);
        assert_eq!(verification_account.a.get(), proof.a);
        assert_eq!(verification_account.b.get(), proof.b);
        assert_eq!(verification_account.c.get(), proof.c);

        // Already setup proof
        assert_matches!(init_verification_proof(&fee_payer, &mut verification_account, 0, raw_proof), Err(_));
    }

    macro_rules! precomputes_sub_account {
        ($id: ident, $vkey: ident) => {
            let mut data = vec![0; precompute_account_size::<$vkey>()];
            let precomputes = VirtualPrecomputes::<$vkey>::new(&mut data);

            let mut d = vec![1];
            d.extend(precomputes.0.data.to_vec());

            let pk = solana_program::pubkey::Pubkey::new_unique();
            account!($id, pk, d);
        };
    }

    macro_rules! precomputes_account {
        ($id: ident) => {
            let mut data = vec![0; PrecomputesAccount::SIZE];
            let mut map = HashMap::new();

            precomputes_sub_account!(acc0, SendQuadraVKey);
            map.insert(0, &acc0);

            let mut $id = PrecomputesAccount::new(&mut data, map).unwrap();
            $id.set_is_setup(&true);
        };
    }

    #[test]
    fn test_compute_verification() {
        let mut data = vec![0; VerificationAccount::SIZE];
        let mut verification_account = VerificationAccount::new(&mut data).unwrap();
        precomputes_account!(precomputes_account);

        // Setup
        let public_inputs = test_public_inputs();
        for (i, &public_input) in public_inputs.iter().enumerate() {
            verification_account.set_public_input(i, &RawU256::new(public_input));
        }
        let instructions = prepare_public_inputs_instructions::<SendQuadraVKey>(&public_inputs);
        verification_account.set_prepare_inputs_instructions_count(&(instructions.len() as u32));
        for (i, &ix) in instructions.iter().enumerate() {
            verification_account.set_prepare_inputs_instructions(i, &(ix as u16));
        }

        // Computation is already finished (is_verified is Some)
        verification_account.set_is_verified(&ElusivOption::Some(true));
        assert_matches!(compute_verification(&mut verification_account, &precomputes_account, 0), Err(_));
        verification_account.set_is_verified(&ElusivOption::None);

        // Success for public input preparation
        for _ in 0..instructions.len() {
            assert_matches!(compute_verification(&mut verification_account, &precomputes_account, 0), Ok(()));
        }

        // Failure for miller loop (proof not setup)
        assert_matches!(compute_verification(&mut verification_account, &precomputes_account, 0), Err(_));

        let proof = test_proof();
        verification_account.a.set(&proof.a);
        verification_account.b.set(&proof.b);
        verification_account.c.set(&proof.c);
        verification_account.set_state(&VerificationState::ProofSetup);

        // Success
        for _ in 0..COMBINED_MILLER_LOOP_IXS + FINAL_EXPONENTIATION_IXS {
            assert_matches!(compute_verification(&mut verification_account, &precomputes_account, 0), Ok(()));
        }

        // Computation is finished
        assert_matches!(compute_verification(&mut verification_account, &precomputes_account, 0), Err(_));
        assert_matches!(verification_account.get_is_verified().option(), Some(false));
    }

    macro_rules! finalize_send_test {
        ($send_public_inputs: ident, $v_account: ident, $v_data: ident, $queue: ident, $n_acc_0: ident, $n_acc_1: ident, $nullifier_duplicate_pda: ident, $finalize_data: ident) => {
            let $send_public_inputs = SendPublicInputs {
                join_split: JoinSplitPublicInputs {
                    commitment_count: 1,
                    roots: vec![
                        Some(empty_root_raw()),
                    ],
                    nullifier_hashes: vec![
                        RawU256::new(u256_from_str_skip_mr("1")),
                    ],
                    commitment: RawU256::new(u256_from_str_skip_mr("987654321")),
                    fee_version: 0,
                    amount: LAMPORTS_PER_SOL,
                    fee: 10000,
                    token_id: 0,
                },
                recipient: RawU256::new(u256_from_str_skip_mr("123")),
                current_time: 112233,
                identifier: RawU256::new(u256_from_str_skip_mr("12345")),
                salt: RawU256::new(u256_from_str_skip_mr("6789")),
            };
    
            let fee_payer = Pubkey::new_unique().to_bytes();
            let $nullifier_duplicate_pda = PoolAccount::find(None).0.to_bytes();
            let mut $v_data = vec![0; VerificationAccount::SIZE];
            let mut $v_account = VerificationAccount::new(&mut $v_data).unwrap();
            $v_account.setup(
                &[],
                &vec![0],
                0,
                VerificationAccountData {
                    fee_payer: RawU256::new(fee_payer),
                    nullifier_duplicate_pda: RawU256::new($nullifier_duplicate_pda),
                    min_batching_rate: 0,
                    unadjusted_fee: 0,
                },
                ProofRequest::Send($send_public_inputs.clone()),
                [0, 1],
            ).unwrap();
            $v_account.set_state(&VerificationState::ProofSetup);
            $v_account.set_is_verified(&ElusivOption::Some(true));
    
            let mut data = vec![0; CommitmentQueueAccount::SIZE];
            let mut $queue = CommitmentQueueAccount::new(&mut data).unwrap();

            let $finalize_data = FinalizeSendData {
                timestamp: $send_public_inputs.current_time,
                total_amount: $send_public_inputs.join_split.total_amount(),
                token_id: 0,
                mt_index: 0,
                commitment_index: 0,
            };
    
            nullifier_account!(mut $n_acc_0);
            nullifier_account!(mut $n_acc_1);
        };
    }

    macro_rules! storage_account {
        ($id: ident) => {
            let mut data = vec![0; StorageAccount::SIZE];
            let $id = StorageAccount::new(&mut data, HashMap::new()).unwrap();
        };
    }

    #[test]
    fn test_finalize_verification_send_valid() {
        finalize_send_test!(send_public_inputs, v_account, v_data, queue, _n, _n, nullifier_duplicate_pda, finalize_data);
        let identifier_pk = Pubkey::new(&send_public_inputs.identifier.skip_mr());
        let salt_pk = Pubkey::new(&send_public_inputs.salt.skip_mr());
        account!(identifier, identifier_pk, vec![]);
        account!(salt, salt_pk, vec![]);
        storage_account!(storage);

        // Verification is not finished
        v_account.set_is_verified(&ElusivOption::None);
        assert_matches!(
            finalize_verification_send(&identifier, &salt, &mut queue, &mut v_account, &storage, finalize_data, 0),
            Err(_)
        );

        v_account.set_is_verified(&ElusivOption::Some(true));

        { // Invalid identifier
            account!(identifier, salt_pk, vec![]); 
            assert_matches!(
                finalize_verification_send(&identifier, &salt, &mut queue, &mut v_account, &storage, finalize_data, 0),
                Err(_)
            );
        }

        { // Invalid salt
            account!(salt, identifier_pk, vec![]); 
            assert_matches!(
                finalize_verification_send(&identifier, &salt, &mut queue, &mut v_account, &storage, finalize_data, 0),
                Err(_)
            );
        }

        // Invalid finalize_data
        for invalid_data in [
            mutate(&finalize_data, |d| { d.timestamp = 0 }),
            mutate(&finalize_data, |d| { d.total_amount = send_public_inputs.join_split.amount }),
            mutate(&finalize_data, |d| { d.token_id = 1 }),
            mutate(&finalize_data, |d| { d.commitment_index = 1 }),
            mutate(&finalize_data, |d| { d.mt_index = 1 }),
        ] {
            assert_matches!(
                finalize_verification_send(&identifier, &salt, &mut queue, &mut v_account, &storage, invalid_data, 0),
                Err(_)
            );
        }

        // Success
        assert_matches!(
            finalize_verification_send(&identifier, &salt, &mut queue, &mut v_account, &storage, finalize_data, 0),
            Ok(())
        );

        assert_matches!(v_account.get_state(), VerificationState::InsertNullifiers);

        // Called twice
        assert_matches!(
            finalize_verification_send(&identifier, &salt, &mut queue, &mut v_account, &storage, finalize_data, 0),
            Err(_)
        );
    }

    #[test]
    #[allow(unused_mut)]
    fn test_finalize_verification_send_invalid() {
        finalize_send_test!(send_public_inputs, v_account, v_data, queue, _n, _n, nullifier_duplicate_pda, finalize_data);
        let identifier_pk = Pubkey::new(&send_public_inputs.identifier.skip_mr());
        let salt_pk = Pubkey::new(&send_public_inputs.salt.skip_mr());
        account!(identifier, identifier_pk, vec![]);
        account!(salt, salt_pk, vec![]);
        v_account.set_is_verified(&ElusivOption::Some(false));
        storage_account!(storage);

        assert_matches!(
            finalize_verification_send(&identifier, &salt, &mut queue, &mut v_account, &storage, finalize_data, 0),
            Ok(())
        );        
        assert_matches!(v_account.get_state(), VerificationState::Finalized);
    }

    #[test]
    fn test_finalize_verification_migrate() {
        let migrate_public_inputs = MigratePublicInputs {
            join_split: JoinSplitPublicInputs {
                commitment_count: 1,
                roots: vec![
                    Some(empty_root_raw()),
                ],
                nullifier_hashes: vec![
                    RawU256::new(u256_from_str_skip_mr("1")),
                ],
                commitment: RawU256::new(u256_from_str_skip_mr("1")),
                fee_version: 0,
                amount: LAMPORTS_PER_SOL,
                fee: 10000,
                token_id: 0,
            },
            current_nsmt_root: RawU256::new([0; 32]),
            next_nsmt_root: RawU256::new([0; 32]),
        };

        let pk = Pubkey::new_unique();
        account!(acc, pk, vec![]);

        let mut data = vec![0; VerificationAccount::SIZE];
        let mut v_account = VerificationAccount::new(&mut data).unwrap();
        v_account.set_request(&ProofRequest::Migrate(migrate_public_inputs));
        v_account.set_state(&VerificationState::ProofSetup);
        v_account.set_is_verified(&ElusivOption::Some(true));

        let mut data = vec![0; CommitmentQueueAccount::SIZE];
        let mut queue = CommitmentQueueAccount::new(&mut data).unwrap();

        let finalize_data = FinalizeSendData { timestamp: 0, total_amount: 0, token_id: 0, mt_index: 0, commitment_index: 0 };
        storage_account!(storage);

        assert_matches!(
            finalize_verification_send(&acc, &acc, &mut queue, &mut v_account, &storage, finalize_data, 0),
            Err(_)
        );
    }

    macro_rules! pda_account {
        ($id: ident, $pk: expr) => {
            let pk = $pk; 
            account!($id, pk, vec![1]);
        };
    }

    #[test]
    fn test_finalize_verification_send_nullifiers() {
        finalize_send_test!(send_public_inputs, v_account, v_data, _q, n_acc_0, n_acc_1, nullifier_duplicate_pda, _f);

        // inalize_verification_send not called
        v_account.set_state(&VerificationState::InsertNullifiers);

        // Nullifier duplicate
        n_acc_0.try_insert_nullifier_hash(send_public_inputs.join_split.nullifier_hashes[0].reduce()).unwrap();
        assert_matches!(
            finalize_verification_send_nullifiers(&mut v_account, &mut n_acc_0, &mut n_acc_1, 0),
            Err(_)
        );

        nullifier_account!(mut n_acc_0);

        // Success
        assert_matches!(
            finalize_verification_send_nullifiers(&mut v_account, &mut n_acc_0, &mut n_acc_1, 0),
            Ok(())
        );

        assert_matches!(v_account.get_state(), VerificationState::Finalized);

        // Called twice
        assert_matches!(
            finalize_verification_send_nullifiers(&mut v_account, &mut n_acc_0, &mut n_acc_1, 0),
            Err(_)
        );
    }

    #[test]
    #[allow(unused_mut)]
    fn test_finalize_verification_transfer() {
        finalize_send_test!(send_public_inputs, v_account, v_data, queue, _n, _n, nullifier_duplicate_pda, _f);

        let recipient_pk = Pubkey::new(&send_public_inputs.recipient.skip_mr());
        account!(recipient, recipient_pk, vec![]);

        let fee_payer_pk = Pubkey::new(&v_account.get_other_data().fee_payer.skip_mr());
        account!(fee_payer, fee_payer_pk, vec![]);

        let mut data = vec![0; FeeAccount::SIZE];
        let fee = FeeAccount::new(&mut data).unwrap();

        pda_account!(pool, PoolAccount::find(None).0);
        pda_account!(collector, FeeCollectorAccount::find(None).0);
        pda_account!(nullifier, Pubkey::new(&nullifier_duplicate_pda));

        account!(v_acc, fee_payer_pk, v_data.clone());

        // Finalize not called prior
        assert_matches!(
            finalize_verification_transfer(&recipient, &fee_payer, &fee, &pool, &collector, &mut queue, &v_acc, &nullifier, 0, 0),
            Err(_)
        );

        VerificationAccount::new(&mut v_data[..]).unwrap().set_state(&VerificationState::Finalized);
        account!(v_acc, fee_payer_pk, v_data);

        // Invalid original_fee_payer
        assert_matches!(
            finalize_verification_transfer(&recipient, &recipient, &fee, &pool, &collector, &mut queue, &v_acc, &nullifier, 0, 0),
            Err(_)
        );

        // Invalid nullifier_duplicate_account
        assert_matches!(
            finalize_verification_transfer(&recipient, &fee_payer, &fee, &pool, &collector, &mut queue, &v_acc, &recipient, 0, 0),
            Err(_)
        );

        // Invalid recipient
        assert_matches!(
            finalize_verification_transfer(&fee_payer, &fee_payer, &fee, &pool, &collector, &mut queue, &v_acc, &nullifier, 0, 0),
            Err(_)
        );

        // Invalid fee version
        assert_matches!(
            finalize_verification_transfer(&recipient, &fee_payer, &fee, &pool, &collector, &mut queue, &v_acc, &nullifier, 0, 1),
            Err(_)
        );

        { // Commitment queue is full
            let mut queue = CommitmentQueue::new(&mut queue);
            for _ in 0..CommitmentQueue::CAPACITY {
                queue.enqueue(
                    CommitmentHashRequest {
                        commitment: [0; 32],
                        fee_version: 0,
                        min_batching_rate: 0,
                    }
                ).unwrap();
            }
        }
        assert_matches!(
            finalize_verification_transfer(&recipient, &fee_payer, &fee, &pool, &collector, &mut queue, &v_acc, &nullifier, 0, 1),
            Err(_)
        );

        let mut data = vec![0; CommitmentQueueAccount::SIZE];
        let mut queue = CommitmentQueueAccount::new(&mut data).unwrap();

        assert_matches!(
            finalize_verification_transfer(&recipient, &fee_payer, &fee, &pool, &collector, &mut queue, &v_acc, &nullifier, 0, 0),
            Ok(())
        );

        let data = &mut v_acc.data.borrow_mut()[..];
        let verification_account = VerificationAccount::new(data).unwrap();
        assert_matches!(verification_account.get_state(), VerificationState::Closed);
        let mut queue = CommitmentQueue::new(&mut queue);
        assert_eq!(queue.len(), 1);
        let commitment = queue.view_first().unwrap();
        assert_eq!(commitment.commitment, send_public_inputs.join_split.commitment.reduce());
    }

    #[test]
    fn test_is_timestamp_valid() {
        assert!(is_timestamp_valid(0, 1));
        assert!(is_timestamp_valid(two_pow!(5) as u64 - 1, 0));

        assert!(!is_timestamp_valid(two_pow!(5) as u64, 0));
    }

    #[test]
    fn test_minimum_commitment_mt_index() {
        assert_eq!(minimum_commitment_mt_index(0, 0, 0), (0, 0));
        assert_eq!(minimum_commitment_mt_index(0, 1, 0), (1, 0));
        assert_eq!(minimum_commitment_mt_index(0, 1, 1), (2, 0));

        assert_eq!(minimum_commitment_mt_index(0, MT_COMMITMENT_COUNT as u32, 0), (0, 1));
        assert_eq!(
            minimum_commitment_mt_index(0, MT_COMMITMENT_COUNT as u32, MT_COMMITMENT_COUNT as u32 + 1),
            (1, 2)
        );
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
                Some(empty_root_raw()),
            ],
            nullifier_hashes: vec![
                RawU256::new(u256_from_str_skip_mr("1")),
            ],
            commitment: RawU256::new(u256_from_str_skip_mr("1")),
            fee_version: 0,
            amount: 0,
            fee: 123,
            token_id: 0,
        };

        let invalid_public_inputs = [
            // Zero-commitment
            mutate(&valid_inputs, |inputs| {
                inputs.commitment = RawU256::new(ZERO_COMMITMENT_RAW);
            }),

            // Invalid root for active MT
            mutate(&valid_inputs, |inputs| {
                inputs.roots[0] = Some(RawU256::new([0; 32]));
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
                    RawU256::new(u256_from_str_skip_mr("0")),
                    RawU256::new(u256_from_str_skip_mr("0")),
                ];
                inputs.roots.push(None);
            }),

            // Invalid root in closed MT
            mutate(&valid_inputs, |inputs| {
                inputs.commitment_count = 2;
                inputs.nullifier_hashes = vec![
                    RawU256::new(u256_from_str_skip_mr("0")),
                    RawU256::new(u256_from_str_skip_mr("0")),
                ];
                inputs.roots.push(Some(empty_root_raw()));
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
                        RawU256::new(u256_from_str_skip_mr("0")),
                        RawU256::new(u256_from_str_skip_mr("0")),
                    ];
                    inputs.roots.push(Some(RawU256::new(u256_from_str_skip_mr("0"))));
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
                    RawU256::new(u256_from_str_skip_mr("0")),
                    RawU256::new(u256_from_str_skip_mr("0")),
                ];
                inputs.roots.push(Some(RawU256::new(u256_from_str_skip_mr("0"))));
            }),
        ];

        for public_inputs in valid_public_inputs {
            assert_matches!(
                check_join_split_public_inputs(&public_inputs, &storage, [&n_account, &n_account], &[0, 1]),
                Ok(())
            );
        }

        // Duplicate nullifier_hash already exists
        let data = vec![0; NullifierAccount::ACCOUNT_SIZE];
        let pk = Pubkey::new_unique();
        account!(sub_account, pk, data);

        hash_map!(acc, (0usize, &sub_account));
        let mut data = vec![0; NullifierAccount::SIZE];
        let mut n_account = NullifierAccount::new(&mut data, acc).unwrap();

        n_account.try_insert_nullifier_hash(u256_from_str("1")).unwrap();

        assert_matches!(
            check_join_split_public_inputs(
                &mutate(&valid_inputs, |inputs| {
                    inputs.nullifier_hashes = vec![
                        RawU256::new(u256_from_str_skip_mr("1")),
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

    fn test_public_inputs() -> Vec<U256> {
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
            "0",
        ].iter().map(|s| u256_from_str_skip_mr(*s)).collect()
    }
}*/