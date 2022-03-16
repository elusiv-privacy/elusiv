use solana_program::pubkey::Pubkey;

use super::*;
use super::super::instruction::PUBLIC_INPUTS_COUNT;

pub fn verify_proof(
    account: &mut ProofVerificationAccount,
    _iteration: usize
) -> bool {
    let result = account.pop_fq12();
    result == super::alpha_g1_beta_g2()
}

pub fn full_verification(
    proof: super::Proof,
    recipient: Pubkey,
    amount: u64,
    inputs: [[u8; 32]; PUBLIC_INPUTS_COUNT]
) -> bool {
    let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
    let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();
    account.init(amount, recipient, proof, inputs).unwrap();

    // Prepare inputs
    for i in 0..PREPARE_INPUTS_ITERATIONS {
        partial_prepare_inputs(&mut account, i).unwrap();
    }
    account.set_round(0);

    // Miller loop
    for i in 0..MILLER_LOOP_ITERATIONS {
        partial_miller_loop(&mut account, i).unwrap();
    }
    account.set_round(0);

    // Final exponentiation
    for i in 0..FINAL_EXPONENTIATION_ITERATIONS {
        partial_final_exponentiation(&mut account, i);
    }

    verify_proof(&mut account, ITERATIONS)
}