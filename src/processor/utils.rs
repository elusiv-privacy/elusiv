use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use crate::error::ElusivError::{ InvalidAccount, InvalidMerkleRoot };
use crate::macros::guard;
use crate::state::{ TREE_SIZE, TREE_COMMITMENT_COUNT };
use crate::types::{ ProofData };
use crate::state::{ NullifierAccount, StorageAccount };

pub fn tx_count_to_lamports(tx_count: u64) -> u64 {
    let fee_calculator = solana_program::fee_calculator::FeeCalculator::default();
    tx_count * fee_calculator.lamports_per_signature
}

const BYTES_PER_COMMITMENT: usize = (TREE_SIZE * 32) / TREE_COMMITMENT_COUNT;

/// Computes how much the program needs to charge the user for infinite storage time (no tree pruning)
/// - we charge this per tx
/// - for every arity tx's, the user can store a nullifier
/// - higher arity => lower rent fees 
/// - "best case": arity of commitment count (aka only commitments) => 32 bytes storage per commitment
/// - example: arity of infinity => 0.00089784 SOL per byte for two years => about 3 EUR in total
/// - => this is too much, so we need to think about an automatic pruning system and a system in which a user can
/// - additionally we need to think about the fees we take in total
pub fn compute_rent_fee(arity: u64, byte_rent_excemption: u64) -> u64 {
    (BYTES_PER_COMMITMENT + 32 / arity) * byte_rent_excemption
}

fn send_with_system_program<'a>(
    sender: &AccountInfo<'a>,
    recipient: &AccountInfo<'a>,
    system_program: & AccountInfo<'a>,
    lamports: u64,
) -> ProgramResult {
    // Check that system_program is correct
    guard!(
        *system_program.key == solana_program::system_program::ID,
        InvalidAccount
    );

    // Transfer funds from sender into the pool
    let instruction = solana_program::system_instruction::transfer(
        &sender.key,
        recipient.key,
        amount + fee
    );
    let (_, bump_seed) = Pubkey::find_program_address(&[b"elusiv"], &super::super::id());
    solana_program::program::invoke_signed(
        &instruction,
        &[
            sender.clone(),
            recipient.clone(),
            system_program.clone(),
        ],
        &[&[&b"elusiv"[..], &[bump_seed]]],
    )    
}

// TODO: change tree computation so that root = hash(root, pubkey)

pub fn check_shared_public_inputs(
    proof_data: ProofData,
    storage_account: &StorageAccount,
    nullifier_accounts: [&NullifierAccount; 2],
) -> ProgramResult {
    let uses_multiple_trees = nullifier_accounts[0].get_key() != nullifier_accounts[1].get_key();

    // Check that commitment is new
    storage_account.can_insert_commitment(commitment)?;

    // Check that one nullifier_account does not have multiple different roots (proof will also verify that)
    guard!(
        uses_multiple_trees || proof_data.roots[0] == proof_data.roots[1],
        InvalidMerkleRoot
    );

    // Check that roots are valid by matching the nullifier account
    storage_account.is_root_valid(nullifier_accounts[0], proof_data.roots[0])?;
    storage_account.is_root_valid(nullifier_accounts[1], proof_data.roots[1])?;

    // Check that nullifiers are different for the same tree (the proof will also verify that)
    guard!(
        uses_multiple_trees || proof_data.nullifiers[0] != proof_data.nullifiers[1],
        NullifierAccountDuplicate
    );

    // Check if nullifiers are new
    // Note:
    // - This check itself does not prevent double spending, just a check to prevent spam/mistaken double
    // - The important check happens at the end of the proof verification
    nullifier_account[0].can_insert_nullifier(proof_data.nullifiers[0])?;
    nullifier_account[1].can_insert_nullifier(proof_data.nullifiers[1])?;

    // Check that commitment is new
    storage_account.can_insert_commitment(proof_data.commitment)
}