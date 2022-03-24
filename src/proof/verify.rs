use solana_program::program_error::ProgramError;
use super::*;
use crate::types::U256;
use crate::error::ElusivError;

pub fn verify_proof<VKey: VerificationKey>(
    account: &mut ProofAccount,
    iteration: usize
) -> Result<bool, ProgramError> {
    // Check that computation is complete
    if iteration != VKey::FULL_ITERATIONS {
        return Err(ElusivError::ProofComputationIsNotYetFinished.into());
    }

    // Final verification check
    let result = account.fq12.pop();
    Ok(result == VKey::alpha_g1_beta_g2())
}

pub fn full_verification<VKey: VerificationKey>(
    proof: super::Proof,
    inputs: &[U256]
) -> Result<bool, ProgramError> {
    let mut data = vec![0; ProofAccount::TOTAL_SIZE];
    let mut account = ProofAccount::from_data(&mut data)?;
    account.reset::<VKey>(proof, inputs)?;

    // Prepare inputs
    for i in 0..VKey::PREPARE_INPUTS_ITERATIONS {
        partial_prepare_inputs::<VKey>(&mut account, i)?;
    }
    account.set_round(0);

    // Miller loop
    for i in 0..MILLER_LOOP_ITERATIONS {
        partial_miller_loop::<VKey>(&mut account, i)?;
    }
    account.set_round(0);

    // Final exponentiation
    for i in 0..FINAL_EXPONENTIATION_ITERATIONS {
        partial_final_exponentiation(&mut account, i)?;
    }

    verify_proof::<VKey>(&mut account, VKey::FULL_ITERATIONS)
}