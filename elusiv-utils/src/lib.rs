pub mod error;

use elusiv_computation::MAX_COMPUTE_UNIT_LIMIT;
#[cfg(feature = "sdk")] use error::UtilsError;
#[cfg(feature = "sdk")]
use solana_program::{
    instruction::Instruction,
    pubkey::Pubkey,
    system_instruction,
};
#[cfg(feature = "sdk")]
use solana_sdk::{
    signature::Signer,
    signer::keypair::Keypair, compute_budget::ComputeBudgetInstruction,
};

#[cfg(feature = "sdk")]
/// Batches multiple identical instructions together
pub fn batch_instructions(
    total_ix_count: usize,
    compute_units_per_ix: u32,
    ix: Instruction,
) -> Vec<Vec<Instruction>> {
    let mut v = Vec::new();

    let batch_size = 1_400_000 / compute_units_per_ix as usize;
    let mut ixs = vec![
        ComputeBudgetInstruction::set_compute_unit_limit(batch_size as u32 * compute_units_per_ix),
    ];
    for _ in 0..batch_size {
        ixs.push(ix.clone());
    }

    for _ in 0..total_ix_count / batch_size {
        v.push(ixs.clone());
    }
    
    let remaining_ix_count = total_ix_count % batch_size;
    if remaining_ix_count > 0 {
        let mut ixs = vec![
            ComputeBudgetInstruction::set_compute_unit_limit(batch_size as u32 * compute_units_per_ix),
        ];
        for _ in 0..remaining_ix_count {
            ixs.push(ix.clone());
        }
        v.push(ixs);
    }

    v
}

pub fn batched_instructions_tx_count(
    total_ix_count: usize,
    compute_units_per_ix: u32,
) -> usize {
    let batch_size = MAX_COMPUTE_UNIT_LIMIT as usize / compute_units_per_ix as usize;
    total_ix_count / batch_size + if total_ix_count % batch_size == 0 { 0 } else { 1 }
}

#[cfg(feature = "sdk")]
/// Creates a new data account with `account_size` data
/// - `amount` needs to be at least the amount required for rent-exemption
pub fn create_account(
    payer: &Keypair,
    program_id: &Pubkey,
    account_size: usize,
    amount: u64,
) -> Result<(Instruction, Keypair), UtilsError> {
    let new_account_keypair = Keypair::new();

    let create_account_ix = system_instruction::create_account(
        &payer.pubkey(),
        &new_account_keypair.pubkey(),
        amount,
        account_size.try_into().unwrap(),
        program_id,
    );

    Ok((create_account_ix, new_account_keypair))
}