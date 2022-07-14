//! Tests the proof verification

#[cfg(not(tarpaulin_include))]
mod common;

use assert_matches::assert_matches;
use borsh::BorshSerialize;
use common::*;
use common::program_setup::*;
use elusiv::instruction::{ElusivInstruction, WritableUserAccount, SignerAccount, WritableSignerAccount};
use elusiv::proof::{VerificationAccount, VerificationSetupState};
use elusiv::state::{EMPTY_TREE, MT_HEIGHT};
use elusiv::state::program_account::{PDAAccount, ProgramAccount};
use elusiv::types::{Proof, SendPublicInputs, JoinSplitPublicInputs, U256Limbed2};
use elusiv::proof::verifier::proof_from_str;
use elusiv::processor::ProofRequest;
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program_test::*;

async fn setup_verification_tests() -> (ProgramTestContext, Actor) {
    let mut context = start_program_solana_program_test().await;

    setup_initial_accounts(&mut context).await;
    setup_storage_account(&mut context).await;
    create_merkle_tree(&mut context, 0).await;
    create_merkle_tree(&mut context, 1).await;

    let client = Actor::new(&mut context).await;
    (context, client)
}

struct FullSendRequest {
    proof: Proof,
    public_inputs: SendPublicInputs,
}

fn send_requests() -> Vec<FullSendRequest> {
    vec![
        FullSendRequest {
            proof: proof_from_str(
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
            ),
            public_inputs: SendPublicInputs {
                join_split: JoinSplitPublicInputs {
                    commitment_count: 1,
                    roots: vec![
                        Some(EMPTY_TREE[MT_HEIGHT as usize]),
                    ],
                    nullifier_hashes: vec![
                        u256_from_str("10026859857882131638516328056627849627085232677511724829502598764489185541935"),
                    ],
                    commitment: u256_from_str("685960310506634721912121951341598678325833230508240750559904196809564625591"),
                    fee_version: 0,
                    amount: LAMPORTS_PER_SOL * 123,
                },
                recipient: u256_from_str("19685960310506634721912121951341598678325833230508240750559904196809564625591"),
                current_time: 0,
                identifier: u256_from_str("139214303935475888711984321184227760578793579443975701453971046059378311483"),
                salt: u256_from_str("230508240750559904196809564625"),
            }
        }
    ]
}

/*#[tokio::test]
#[ignore]
async fn test_send_proof() {
    // Note: since the circuits are being audited atm we will have a valid proof for the real circuits only in a few commits
    let (mut context, mut client) = setup_verification_tests().await;
    pda_account!(active_verifications_pda, ActiveVerificationsAccount, None, context);
    let active_verifications_map = WritableUserAccount(Pubkey::new(&active_verifications_pda.get_pubkey()));
    create_merkle_tree(&mut context, 1).await;
    let (_, nullifier_0, writable_nullifier_0) = nullifier_accounts(0, &mut context).await;

    let request = &send_requests()[0];

    // Init proof
    tx_should_succeed(
        &[
            request_compute_units(1_400_000),
            ElusivInstruction::init_verification_instruction(
                0,
                [0, 1],
                request.proof.try_to_vec().unwrap().try_into().unwrap(),
                ProofRequest::Send(request.public_inputs.clone()),
                false,
                SignerAccount(client.pubkey),
                active_verifications_map,
                &nullifier_0,
                &[],
            ),
            ElusivInstruction::init_verification_public_inputs_instruction(0),
        ],
        &mut client, &mut context,
    ).await;

    pda_account!(mut verification_account, VerificationAccount, Some(0), context);
    let prepare_inputs_ix_count = verification_account.get_prepare_inputs_instructions_count();

    // Check proof values
    assert_eq!(verification_account.a.get().0, request.proof.a.0);
    assert_eq!(verification_account.b.get().0, request.proof.b.0);
    assert_eq!(verification_account.c.get().0, request.proof.c.0);
    assert_eq!(verification_account.get_vkey(), 0);

    // Input preparation
    for _ in 0..prepare_inputs_ix_count as u64 {
        tx_should_succeed(&[
            request_compute_units(1_400_000),
            ElusivInstruction::compute_verification_instruction(0)
        ], &mut client, &mut context).await;
    }

    // Check prepared inputs
    pda_account!(mut verification_account, VerificationAccount, Some(0), context);
    let public_inputs: Vec<ark_bn254::Fr> = request.public_inputs.public_signals().iter().map(u256_to_fr).collect();
    let pvk = ark_pvk::<SendDecaVKey>();
    let prepared_inputs = ark_groth16::prepare_inputs(&pvk, &public_inputs).unwrap().into_affine();
    assert_eq!(verification_account.prepared_inputs.get().0, prepared_inputs);

    let ix = ElusivInstruction::compute_verification_instruction(0);

    // Combined miller loop
    for ixs in batch_instructions(COMBINED_MILLER_LOOP_IXS, 4, 350_000, ix.clone()) {
        tx_should_succeed(&ixs, &mut client, &mut context).await;
    }

    // Check combined miller loop result
    pda_account!(mut verification_account, VerificationAccount, Some(0), context);
    let combined_miller_loop_result = ark_bn254::Bn254::miller_loop(
        [
            (request.proof.a.0.into(), request.proof.b.0.into()),
            (prepared_inputs.into(), pvk.gamma_g2_neg_pc),
            (request.proof.c.0.into(), pvk.delta_g2_neg_pc),
        ]
        .iter(),
    );
    assert_eq!(verification_account.get_coeff_index(), 91);
    assert_eq!(verification_account.f.get().0, combined_miller_loop_result);

    // Final exponentiation
    for ixs in batch_instructions(FINAL_EXPONENTIATION_IXS, 1, 1400000, ix.clone()) {
        tx_should_succeed(&ixs, &mut client, &mut context).await;
    }

    // Check final exponentiation result
    pda_account!(mut verification_account, VerificationAccount, Some(0), context);
    let final_exponentiation_result = ark_bn254::Bn254::final_exponentiation(&combined_miller_loop_result);
    assert_eq!(verification_account.f.get().0, final_exponentiation_result.unwrap());

    // Check verification result
    assert!(!verification_account.get_is_verified().option().unwrap());

    // Finalize
    let identifier_account = Pubkey::new(&request.public_inputs.identifier);
    let recipient = Pubkey::new(&request.public_inputs.recipient);

    tx_should_succeed(
        &[
            request_compute_units(1_400_000),
            ElusivInstruction::finalize_verification_instruction(
                FinalizeFields {
                    identifier: request.public_inputs.identifier,
                    salt: request.public_inputs.salt,
                    commitment: request.public_inputs.join_split.commitment,
                },
                0,
                0,
                [0, 1],
                UserAccount(identifier_account),
                WritableUserAccount(recipient),
                WritableUserAccount(client.pubkey),
                &writable_nullifier_0,
                &[],
            )
        ],
        &mut client, &mut context
    ).await;

    // Check that verification account is closed and rent is payed to the fee_collector

    /*let fetching_account = Pubkey::new(&request.public_inputs.identifier);
    let recipient = Pubkey::new(&request.public_inputs.recipient);
    pda_account!(verification_account, VerificationAccount, Some(0), context);*/

    //assert_matches!(verification_account.get_is_verified().option(), Some(false));

    // Finalize
    /*tx_should_succeed(&[
            request_compute_units(1_400_000),
            ElusivInstruction::finalize_proof_instruction(
                0,
                0,
                [0, 1],
                WritableUserAccount(client.pubkey),
                UserAccount(fetching_account),
                WritableUserAccount(recipient),
                &writable_nullifier_0,
                &[],
            )
        ],
        &mut client, &mut context
    ).await;*/
}*/

#[tokio::test]
async fn test_init_verification() {
    let (mut context, mut client) = setup_verification_tests().await;
    let (_, nullifier_0, _writable_nullifier_0) = nullifier_accounts(0, &mut context).await;
    let pending_nullifiers_map_account = pending_nullifiers_map_account(0, &mut context).await;
    let request = &send_requests()[0];

    // Init start
    ix_should_succeed(
        ElusivInstruction::init_verification_instruction(
            0,
            [0, 1],
            ProofRequest::Send(request.public_inputs.clone()),
            WritableSignerAccount(client.pubkey),
            &nullifier_0,
            &[],
        ),
        &mut client, &mut context,
    ).await;

    pda_account!(verification_account, VerificationAccount, Some(0), context);
    assert_matches!(verification_account.get_setup_state(), VerificationSetupState::None);

    // Init nullifiers
    ix_should_succeed(
        ElusivInstruction::init_verification_validate_nullifier_hashes_instruction(
            0,
            [0, 1],
            false,
            &[WritableUserAccount(pending_nullifiers_map_account)],
            &[],
        ),
        &mut client, &mut context,
    ).await;

    pda_account!(verification_account, VerificationAccount, Some(0), context);
    assert_matches!(verification_account.get_setup_state(), VerificationSetupState::NullifiersChecked);

    // Init public inputs
    ix_should_succeed(
        ElusivInstruction::init_verification_public_inputs_instruction(0),
        &mut client, &mut context,
    ).await;

    pda_account!(verification_account, VerificationAccount, Some(0), context);
    assert_matches!(verification_account.get_setup_state(), VerificationSetupState::PublicInputsSetup);

    // Init proof
    ix_should_succeed(
        ElusivInstruction::init_verification_proof_instruction(
            0,
            request.proof.try_to_vec().unwrap().try_into().unwrap(),
            SignerAccount(client.pubkey),
        ),
        &mut client, &mut context,
    ).await;

    pda_account!(verification_account, VerificationAccount, Some(0), context);
    assert_matches!(verification_account.get_setup_state(), VerificationSetupState::ProofSetup);
}

// init duplicated nullifiers
// init twice
// init nullifier
// init invalid public inputs
// proof init invalid signer

#[tokio::test]
#[ignore]
async fn test_init_verification_validate_nullifier_hashes_instruction() {
    let (mut context, mut client) = setup_verification_tests().await;
    let (_, nullifier_0, _writable_nullifier_0) = nullifier_accounts(0, &mut context).await;
    let pending_nullifiers_map_account = pending_nullifiers_map_account(0, &mut context).await;
    let request = &send_requests()[0];
    let nullifier_hash = U256Limbed2::from(request.public_inputs.join_split.nullifier_hashes[0]);

    // Init start
    ix_should_succeed(
        ElusivInstruction::init_verification_instruction(
            0,
            [0, 1],
            ProofRequest::Send(request.public_inputs.clone()),
            WritableSignerAccount(client.pubkey),
            &nullifier_0,
            &[],
        ),
        &mut client, &mut context,
    ).await;

    let ix = ElusivInstruction::init_verification_validate_nullifier_hashes_instruction(
        0,
        [0, 1],
        false,
        &[WritableUserAccount(pending_nullifiers_map_account)],
        &[],
    );

    // Fuzzing
    test_instruction_fuzzing(
        &[],
        ix.clone(),
        &mut client,
        &mut context
    ).await;

    // Duplicate nullifier
    let original_map = pending_nullifiers_map(0, &mut context).await;
    let mut map = original_map.clone();
    map.try_insert(nullifier_hash, 0).unwrap();
    let mut data = vec![1];
    map.serialize(&mut data).unwrap();
    let lamports = get_balance(pending_nullifiers_map_account, &mut context).await;
    set_account(&mut context, &pending_nullifiers_map_account, data, lamports).await;

    ix_should_fail(ix.clone(), &mut client, &mut context).await;

    // Ignore duplicate nullifier
    ix_should_succeed(
        ElusivInstruction::init_verification_validate_nullifier_hashes_instruction(
            0,
            [0, 1],
            true,
            &[WritableUserAccount(pending_nullifiers_map_account)],
            &[],
        ),
        &mut client, &mut context
    ).await;

    let map = pending_nullifiers_map(0, &mut context).await;
    assert_eq!(map.len(), 1);
    //assert_eq!(*map.get(&nullifier_hash).unwrap(), 1);    // tow pending verifications for the same nullifier_hash

    // TODO: insert

    // Map is full
    // Success

    pda_account!(verification_account, VerificationAccount, Some(0), context);
    assert_matches!(verification_account.get_setup_state(), VerificationSetupState::NullifiersChecked);

    let map = pending_nullifiers_map(0, &mut context).await;
    assert_eq!(map.len(), 1);
    assert!(map.contains_key(&nullifier_hash));

    // Failure second time
    ix_should_fail(ix.clone(), &mut client, &mut context).await;
}

#[tokio::test]
#[ignore]
async fn test_compute_verification() {
    panic!()
}

#[tokio::test]
#[ignore]
async fn test_finalize_verification() {
    panic!()
}

#[tokio::test]
#[ignore]
async fn test_full_verification() {
    panic!()
}

// compute without proof init

// finalize twice
// finalize nullifier already exists (all kinds this can be the case)
// finalize invalid accounts
// finalize invalid fee