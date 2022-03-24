use solana_program::program_error::ProgramError;
use super::*;
use super::super::types::U256;

pub fn verify_proof<VKey: VerificationKey>(
    account: &mut ProofAccount,
    _iteration: usize
) -> Result<bool, ProgramError> {
    // Final verification check
    let result = account.fq12.pop();
    Ok(result == VKey::alpha_g1_beta_g2())
}

pub fn full_verification<VKey: VerificationKey>(
    proof: super::Proof,
    inputs: &[U256]
) -> bool {
    let mut data = vec![0; ProofAccount::TOTAL_SIZE];
    let mut account = ProofAccount::from_data(&mut data).unwrap();
    account.reset::<VKey>(proof, inputs).unwrap();

    // Prepare inputs
    for i in 0..VKey::PREPARE_INPUTS_ITERATIONS {
        partial_prepare_inputs::<VKey>(&mut account, i).unwrap();
    }
    account.set_round(0);

    // Miller loop
    for i in 0..MILLER_LOOP_ITERATIONS {
        partial_miller_loop::<VKey>(&mut account, i).unwrap();
    }
    account.set_round(0);

    // Final exponentiation
    for i in 0..FINAL_EXPONENTIATION_ITERATIONS {
        partial_final_exponentiation(&mut account, i);
    }

    verify_proof::<VKey>(&mut account, VKey::FULL_ITERATIONS).unwrap()
}