mod common;
use ark_bn254::{ Bn254 };
use ark_ec::{ AffineCurve, PairingEngine, ProjectiveCurve };
use elusiv::state::ProofVerificationAccount;
use elusiv::scalar::*;
use elusiv::groth16::*;
use ark_groth16::{ verify_proof };
use common::*;

#[test]
fn test_full_proof() {
    // Check that gamma_abc_g1 match in the verifying keys
    assert_eq!(
        elusiv::groth16::gamma_abc_g1_0(),
        elusiv::groth16::gamma_abc_g1()[0].into_projective()
    );

    // Setup proof account
    let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
    let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();
    let proof = test_proof();
    let inputs = test_inputs_fe();
    account.init(
        vec![
            vec_to_array_32(to_bytes_le_repr(inputs[0])),
            vec_to_array_32(to_bytes_le_repr(inputs[1]))
        ],
        0, [0,0,0,0],
        proof.generate_proof()
    ).unwrap();

    // Expected setup
    let pvk = ark_pvk();
    let prepared_inputs = ark_groth16::prepare_inputs(&pvk, &inputs).unwrap();

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

    // Expected miller value
    let miller = Bn254::miller_loop(
        [
            ( proof.a().into(), proof.b().into() ),
            ( prepared_inputs.into_affine().into(), pvk.gamma_g2_neg_pc ),
            ( proof.c().into(), pvk.delta_g2_neg_pc ),
        ]
        .iter()
    );
    assert_eq!(account.peek_fq12(0), miller);

    // Final exponentiation
    for i in 0..FINAL_EXPONENTIATION_ITERATIONS {
        partial_final_exponentiation(&mut account, i);
    }

    let expected = Bn254::final_exponentiation(&miller).unwrap();
    assert_eq!(account.peek_fq12(0), expected);

    // Verify
    let result = elusiv::groth16::verify_proof(&mut account, 0);
    let expected = verify_proof(&ark_pvk(), &proof.generate_test_proof(), &inputs).unwrap();

    assert_eq!(result, expected);
    //assert_stack_is_cleared(&account); //TODO: pop the prepared inputs from the stack
}

/*fn assert_stack_is_cleared(account: &ProofVerificationAccount) {
    assert_eq!(account.stack_fq.stack_pointer, 0);
    assert_eq!(account.stack_fq2.stack_pointer, 0);
    assert_eq!(account.stack_fq6.stack_pointer, 0);
    assert_eq!(account.stack_fq12.stack_pointer, 0);
}*/