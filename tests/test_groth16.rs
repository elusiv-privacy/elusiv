mod common;
use assert_matches::*;
use solana_program_test::*;
use solana_sdk::signature::Signer;
use ark_bn254::{ Parameters };
use ark_ec::{
        AffineCurve,
        models::bn::g2::G2Prepared,
    };
use elusiv::state::ProofVerificationAccount;
use elusiv::scalar::*;
use common::*;

#[tokio::test]
async fn test_prepare_inputs_and_proof() {
    capture_compute_units();

    // Check that gamma_abc_g1 match in the verifying keys
    assert_eq!(
        elusiv::groth16::gamma_abc_g1_0(),
        elusiv::groth16::gamma_abc_g1()[0].into_projective()
    );

    // Setup program and storage account
    let (mut banks_client, payer, recent_blockhash) = start_program_with_program_accounts(WITHDRAW_INSTRUCTIONS_COUNT).await;

    // Withdrawal data
    let recipient = payer.pubkey();
    let proof = test_proof(); 
    let inputs = test_inputs();    

    // Send transaction
    let t = withdraw_transaction(&payer, recipient, recent_blockhash, withdraw_data(&proof, &inputs)).await;
    assert_matches!(banks_client.process_transaction(t).await, Ok(()));

    // Storage account
    let mut account = get_account_data(&mut banks_client, withdraw_account_id()).await;
    let account = ProofVerificationAccount::from_data(&mut account).unwrap();

    // Check if values are parsed correctly into account
    assert_eq!(read_g1_affine(account.proof_a), proof.a());
    //assert_eq!(read_g2_affine(account.proof_b), proof.b()); -> b is overwritten in preparation
    assert_eq!(read_g1_affine(account.proof_c), proof.c());

    // Check if prepared_inputs match
    let prepared_inputs = read_g1_projective(&account.p_inputs);
    
    let pvk = ark_pvk();
    let inputs = vec![ from_str_10(inputs[0]), from_str_10(inputs[1]), ];
    let expected_inputs = ark_groth16::prepare_inputs(&pvk, &inputs).unwrap();

    assert_eq!(prepared_inputs, expected_inputs);

    // Check if prepared proof matches
    let expected: G2Prepared<Parameters> = proof.b().into();
    for i in 0..expected.ell_coeffs.len() {
        assert_eq!(expected.ell_coeffs[i].0, account.get_b_coeff(i).0);
        assert_eq!(expected.ell_coeffs[i].1, account.get_b_coeff(i).1);
        assert_eq!(expected.ell_coeffs[i].2, account.get_b_coeff(i).2);
    }
}