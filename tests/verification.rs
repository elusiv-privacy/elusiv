//! Tests the proof verification

#[cfg(not(tarpaulin_include))]
mod common;

use ark_ec::{ProjectiveCurve, PairingEngine};
use assert_matches::assert_matches;
use common::*;
use common::program_setup::*;
use elusiv::bytes::{ElusivOption, BorshSerDeSized};
use elusiv::fields::u256_to_fr_skip_mr;
use elusiv::instruction::{ElusivInstruction, WritableUserAccount, SignerAccount, WritableSignerAccount, UserAccount};
use elusiv::proof::vkey::{VerificationKey, SendQuadraVKey};
use elusiv::proof::{VerificationAccount, VerificationState, prepare_public_inputs_instructions, COMBINED_MILLER_LOOP_IXS, FINAL_EXPONENTIATION_IXS};
use elusiv::state::fee::ProgramFee;
use elusiv::state::governor::{FeeCollectorAccount, GovernorAccount, FEE_COLLECTOR_MINIMUM_BALANCE, PoolAccount};
use elusiv::state::queue::{CommitmentQueueAccount, CommitmentQueue, Queue, RingQueue};
use elusiv::state::{empty_root_raw, NullifierAccount};
use elusiv::state::program_account::{PDAAccount, ProgramAccount, SizedAccount, PDAAccountData};
use elusiv::types::{RawU256, Proof, SendPublicInputs, JoinSplitPublicInputs, PublicInputs, compute_fee_rec};
use elusiv::proof::verifier::proof_from_str;
use elusiv::processor::ProofRequest;
use elusiv_utils::batch_instructions;
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program::pubkey::Pubkey;
use solana_program_test::*;

async fn setup_verification_tests() -> (ProgramTestContext, Actor) {
    let mut context = start_program_solana_program_test().await;

    setup_initial_accounts(&mut context).await;
    setup_storage_account(&mut context).await;
    create_merkle_tree(&mut context, 0).await;
    create_merkle_tree(&mut context, 1).await;

    let fee_collector = FeeCollectorAccount::find(None).0;
    airdrop(&fee_collector, FEE_COLLECTOR_MINIMUM_BALANCE, &mut context).await;

    let client = Actor::new(&mut context).await;
    (context, client)
}

#[derive(Clone)]
struct FullSendRequest {
    proof: Proof,
    public_inputs: SendPublicInputs,
}

fn send_requests(program_fee: &ProgramFee) -> Vec<FullSendRequest> {
    let mut requests = vec![
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
                    commitment_count: 2,
                    roots: vec![
                        Some(empty_root_raw()),
                        None,
                    ],
                    nullifier_hashes: vec![
                        RawU256::new(u256_from_str_skip_mr("10026859857882131638516328056627849627085232677511724829502598764489185541935")),
                        RawU256::new(u256_from_str_skip_mr("13921430393547588871192356721184227660578793579443975701453971046059378311483")),
                    ],
                    commitment: RawU256::new(u256_from_str_skip_mr("685960310506634721912121951341598678325833230508240750559904196809564625591")),
                    fee_version: 0,
                    amount: LAMPORTS_PER_SOL * 123,
                    fee: 0,
                },
                recipient: RawU256::new(u256_from_str_skip_mr("19685960310506634721912121951341598678325833230508240750559904196809564625591")),
                current_time: 0,
                identifier: RawU256::new(u256_from_str_skip_mr("139214303935475888711984321184227760578793579443975701453971046059378311483")),
                salt: RawU256::new(u256_from_str_skip_mr("230508240750559904196809564625")),
            }
        },
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
                    commitment_count: 2,
                    roots: vec![
                        Some(empty_root_raw()),
                        Some(empty_root_raw()),
                    ],
                    nullifier_hashes: vec![
                        RawU256::new(u256_from_str_skip_mr("10026859857882131638516328056627849627085232677511724829502598764489185541935")),
                        RawU256::new(u256_from_str_skip_mr("19685960310506634721912121951341598678325833230508240750559904196809564625591")),
                    ],
                    commitment: RawU256::new(u256_from_str_skip_mr("685960310506634721912121951341598678325833230508240750559904196809564625591")),
                    fee_version: 0,
                    amount: LAMPORTS_PER_SOL * 123,
                    fee: 0,
                },
                recipient: RawU256::new(u256_from_str_skip_mr("19685960310506634721912121951341598678325833230508240750559904196809564625591")),
                current_time: 0,
                identifier: RawU256::new(u256_from_str_skip_mr("139214303935475888711984321184227760578793579443975701453971046059378311483")),
                salt: RawU256::new(u256_from_str_skip_mr("230508240750559904196809564625")),
            }
        },
    ];

    for request in requests.iter_mut() {
        compute_fee_rec::<SendQuadraVKey, _>(&mut request.public_inputs, program_fee);
    }

    requests
}

#[tokio::test]
async fn test_verify_invalid_proof() {
    let (mut context, mut client) = setup_verification_tests().await;
    let (_, nullifier_0, writable_nullifier_0) = nullifier_accounts(0, &mut context).await;
    let precomputes_accounts = setup_precomputes(&mut context).await;

    pda_account!(governor, GovernorAccount, None, context);
    let fee = governor.get_program_fee();
    let request = &send_requests(&fee)[0];

    let fee_collector = FeeCollectorAccount::find(None).0;
    airdrop(&fee_collector, fee.base_commitment_subvention, &mut context).await;
    let fee_collector_balance = get_balance(&fee_collector, &mut context).await;

    let pool = PoolAccount::find(None).0;
    let pool_balance = get_balance(&pool, &mut context).await;

    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;
    let rent = verification_rent(&mut context).await;

    let init_ix = ElusivInstruction::init_verification_instruction(
        0,
        [0, 1],
        ProofRequest::Send(request.public_inputs.clone()),
        WritableSignerAccount(client.pubkey),
        WritableUserAccount(nullifier_duplicate_account),
        &nullifier_0,
        &[],
    );

    // Init twice will fail
    tx_should_fail(&[init_ix.clone(), init_ix.clone()], &mut client, &mut context).await;

    let init_proof_ix = ElusivInstruction::init_verification_proof_instruction(
        0,
        request.proof.try_into().unwrap(),
        SignerAccount(client.pubkey),
    );

    // Init proof before init will fail
    ix_should_fail(init_proof_ix.clone(), &mut client, &mut context).await;

    client.airdrop(LAMPORTS_PER_SOL, &mut context).await;
    let client_balance = LAMPORTS_PER_SOL;
    assert_eq!(client_balance, client.balance(&mut context).await);

    // Init success
    ix_should_succeed(init_ix, &mut client, &mut context).await;

    // Subvention paid by fee_collector into pool
    assert_eq!(fee_collector_balance - fee.proof_subvention, get_balance(&fee_collector, &mut context).await);
    assert_eq!(pool_balance + fee.proof_subvention, get_balance(&pool, &mut context).await);

    // Rent paid by client
    assert_eq!(client_balance - rent - fee.lamports_per_tx, client.balance(&mut context).await);

    pda_account!(verification_account, VerificationAccount, Some(0), context);
    assert_matches!(verification_account.get_state(), VerificationState::None);
    let prepare_inputs_ix_count = verification_account.get_prepare_inputs_instructions_count();
    let public_inputs = request.public_inputs.public_signals_skip_mr();
    let expected_instructions = prepare_public_inputs_instructions::<SendQuadraVKey>(&public_inputs);
    assert_eq!(expected_instructions.len() as u32, prepare_inputs_ix_count);
    for (i, &public_input) in public_inputs.iter().enumerate() {
        assert_eq!(verification_account.get_public_input(i).skip_mr(), public_input);
    }

    // Init proof success
    ix_should_succeed(init_proof_ix, &mut client, &mut context).await;

    pda_account!(mut verification_account, VerificationAccount, Some(0), context);
    assert_matches!(verification_account.get_state(), VerificationState::ProofSetup);
    assert_eq!(verification_account.a.get().0, request.proof.a.0);
    assert_eq!(verification_account.b.get().0, request.proof.b.0);
    assert_eq!(verification_account.c.get().0, request.proof.c.0);
    assert_eq!(verification_account.get_vkey(), 0);

    // Input preparation
    for _ in 0..prepare_inputs_ix_count as u64 {
        tx_should_succeed(&[
            request_compute_units(1_400_000),
            ElusivInstruction::compute_verification_instruction(0, &precomputes_accounts)
        ], &mut client, &mut context).await;
    }

    // Check prepared inputs
    pda_account!(mut verification_account, VerificationAccount, Some(0), context);
    let public_inputs: Vec<ark_bn254::Fr> = request.public_inputs.public_signals().iter().map(|x| u256_to_fr_skip_mr(&x.reduce())).collect();
    let pvk = ark_pvk::<SendQuadraVKey>();
    let prepared_inputs = ark_groth16::prepare_inputs(&pvk, &public_inputs).unwrap().into_affine();
    assert_eq!(verification_account.prepared_inputs.get().0, prepared_inputs);

    // Combined miller loop
    let ix = ElusivInstruction::compute_verification_instruction(0, &[]);
    for ixs in batch_instructions(COMBINED_MILLER_LOOP_IXS, 350_000, ix.clone()) {
        tx_should_succeed(&ixs, &mut client, &mut context).await;
    }

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
    for ixs in batch_instructions(FINAL_EXPONENTIATION_IXS, 1_400_000, ix.clone()) {
        tx_should_succeed(&ixs, &mut client, &mut context).await;
    }

    pda_account!(mut verification_account, VerificationAccount, Some(0), context);
    let final_exponentiation_result = ark_bn254::Bn254::final_exponentiation(&combined_miller_loop_result);
    assert_eq!(verification_account.f.get().0, final_exponentiation_result.unwrap());
    assert_matches!(verification_account.get_is_verified().option(), Some(false));

    let recipient = Pubkey::new(&request.public_inputs.recipient.skip_mr());
    let identifier = Pubkey::new(&request.public_inputs.identifier.skip_mr());
    let salt = Pubkey::new(&request.public_inputs.salt.skip_mr());

    let finalize_ix = ElusivInstruction::finalize_verification_send_instruction(
        0,
        UserAccount(identifier),
        UserAccount(salt),
        Some(0),
        &[WritableUserAccount(writable_nullifier_0[0].0)],
        Some(1),
        &[],
    );

    // Two finalizes will fail
    tx_should_fail(&[finalize_ix.clone(), finalize_ix.clone()], &mut client, &mut context).await;

    // Finalize
    ix_should_succeed(finalize_ix.clone(), &mut client, &mut context).await;

    let finalize_transfer_ix = ElusivInstruction::finalize_verification_transfer_instruction(
        0,
        0,
        WritableUserAccount(recipient),
        WritableUserAccount(client.pubkey),
        WritableUserAccount(nullifier_duplicate_account),
    );

    // Two finalize transfers will fail
    tx_should_fail(&[finalize_transfer_ix.clone(), finalize_transfer_ix.clone()], &mut client, &mut context).await;

    ix_should_succeed(finalize_transfer_ix.clone(), &mut client, &mut context).await;

    // Second finalize transfer will fail
    ix_should_fail(finalize_ix, &mut client, &mut context).await;

    // Subvention and rent transferred to fee_collector
    assert_eq!(pool_balance, get_balance(&pool, &mut context).await);
    assert_eq!(
        fee_collector_balance + rent + (fee.proof_subvention - fee.proof_subvention),
        get_balance(&fee_collector, &mut context).await
    );

    // verification_account and nullifier_duplicate_account closed
    assert!(account_does_not_exist(VerificationAccount::find(Some(0)).0, &mut context).await);
    assert!(account_does_not_exist(nullifier_duplicate_account, &mut context).await);
}

#[tokio::test]
async fn test_verify_valid_proof() {
    // TODO: proof is not actually valid, we just fake it later. Use actual valid proof and storage account instead

    let (mut context, mut client) = setup_verification_tests().await;
    let (_, nullifier_0, writable_nullifier_0) = nullifier_accounts(0, &mut context).await;
    let precomputes_accounts = setup_precomputes(&mut context).await;

    pda_account!(governor, GovernorAccount, None, context);
    let fee = governor.get_program_fee();
    let request = &send_requests(&fee)[0];

    let fee_collector = FeeCollectorAccount::find(None).0;
    airdrop(&fee_collector, fee.base_commitment_subvention, &mut context).await;
    let fee_collector_balance = get_balance(&fee_collector, &mut context).await;

    let pool = PoolAccount::find(None).0;
    let pool_balance = get_balance(&pool, &mut context).await;

    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;
    let rent = verification_rent(&mut context).await;

    let init_ix = ElusivInstruction::init_verification_instruction(
        0,
        [0, 1],
        ProofRequest::Send(request.public_inputs.clone()),
        WritableSignerAccount(client.pubkey),
        WritableUserAccount(nullifier_duplicate_account),
        &nullifier_0,
        &[],
    );

    // Init twice will fail
    tx_should_fail(&[init_ix.clone(), init_ix.clone()], &mut client, &mut context).await;

    let init_proof_ix = ElusivInstruction::init_verification_proof_instruction(
        0,
        request.proof.try_into().unwrap(),
        SignerAccount(client.pubkey),
    );

    // Init proof before init will fail
    ix_should_fail(init_proof_ix.clone(), &mut client, &mut context).await;

    client.airdrop(LAMPORTS_PER_SOL, &mut context).await;
    let client_balance = LAMPORTS_PER_SOL;
    assert_eq!(client_balance, client.balance(&mut context).await);

    // Init success
    tx_should_succeed(&[init_ix, init_proof_ix], &mut client, &mut context).await;

    // Subvention paid by fee_collector into pool
    assert_eq!(fee_collector_balance - fee.proof_subvention, get_balance(&fee_collector, &mut context).await);
    assert_eq!(pool_balance + fee.proof_subvention, get_balance(&pool, &mut context).await);

    // Rent paid by client
    assert_eq!(client_balance - rent - fee.lamports_per_tx, client.balance(&mut context).await);

    // Input preparation
    pda_account!(verification_account, VerificationAccount, Some(0), context);
    let prepare_inputs_ix_count = verification_account.get_prepare_inputs_instructions_count();
    for _ in 0..prepare_inputs_ix_count {
        tx_should_succeed(&[
            request_compute_units(1_400_000),
            ElusivInstruction::compute_verification_instruction(0, &precomputes_accounts)
        ], &mut client, &mut context).await;
    }

    // Combined miller loop
    let ix = ElusivInstruction::compute_verification_instruction(0, &[]);
    for ixs in batch_instructions(COMBINED_MILLER_LOOP_IXS, 350_000, ix.clone()) {
        tx_should_succeed(&ixs, &mut client, &mut context).await;
    }

    // Final exponentiation
    for ixs in batch_instructions(FINAL_EXPONENTIATION_IXS, 1_400_000, ix.clone()) {
        tx_should_succeed(&ixs, &mut client, &mut context).await;
    }

    // Fake valid proof (TODO: remove this once circuits are fixed)
    pda_account!(verification_account, VerificationAccount, Some(0), context);
    assert_matches!(verification_account.get_is_verified().option(), Some(false));
    set_pda_account::<VerificationAccount, _>(&mut context, Some(0), |data| {
        let mut verification_account = VerificationAccount::new(data).unwrap();
        verification_account.set_is_verified(&ElusivOption::Some(true));
    }).await;

    let recipient = Pubkey::new(&request.public_inputs.recipient.skip_mr());
    let identifier = Pubkey::new(&request.public_inputs.identifier.skip_mr());
    let salt = Pubkey::new(&request.public_inputs.salt.skip_mr());

    let pool = PoolAccount::find(None).0;
    let amount = request.public_inputs.join_split.amount;
    let unadjusted_fee = fee.proof_verification_fee(prepare_inputs_ix_count as usize, 0, amount);
    let subvention = fee.proof_subvention;
    airdrop(&pool, amount + unadjusted_fee - subvention, &mut context).await;
    assert_eq!(pool_balance + amount + unadjusted_fee, get_balance(&pool, &mut context).await);

    // Finalize with missing sub-account will fail
    ix_should_fail(
        ElusivInstruction::finalize_verification_send_instruction(
            0,
            UserAccount(identifier),
            UserAccount(salt),
            Some(0),
            &[],
            Some(1),
            &[],
        ),
        &mut client, &mut context
    ).await;

    let finalize_ix = ElusivInstruction::finalize_verification_send_instruction(
        0,
        UserAccount(identifier),
        UserAccount(salt),
        Some(0),
        &[WritableUserAccount(writable_nullifier_0[0].0)],
        Some(1),
        &[],
    );

    // Two finalizes will fail
    tx_should_fail(&[finalize_ix.clone(), finalize_ix.clone()], &mut client, &mut context).await;

    // Finalize (without transfer ix -> one extra tx sent)
    ix_should_succeed(finalize_ix.clone(), &mut client, &mut context).await;

    let nullifier_hashes = request.public_inputs.join_split.nullifier_hashes.clone();
    nullifier_account(&mut context, Some(0), |n: &NullifierAccount| {
        assert_eq!(n.get_nullifier_hash_count(), 2);
        assert!(!n.can_insert_nullifier_hash(nullifier_hashes[0].reduce()).unwrap());
        assert!(!n.can_insert_nullifier_hash(nullifier_hashes[1].reduce()).unwrap());
    }).await;

    nullifier_account(&mut context, Some(1), |n: &NullifierAccount| {
        assert_eq!(n.get_nullifier_hash_count(), 0);
    }).await;

    // Invalid nullifier_duplicate_account
    ix_should_fail(
        ElusivInstruction::finalize_verification_transfer_instruction(
            0,
            0,
            WritableUserAccount(recipient),
            WritableUserAccount(client.pubkey),
            WritableUserAccount(CommitmentQueueAccount::find(None).0),
        ),
        &mut client, &mut context
    ).await;

    let finalize_transfer_ix = ElusivInstruction::finalize_verification_transfer_instruction(
        0,
        0,
        WritableUserAccount(recipient),
        WritableUserAccount(client.pubkey),
        WritableUserAccount(nullifier_duplicate_account),
    );

    // Update fee version in the mean time (will not affect the fee)
    set_single_pda_account!(GovernorAccount, &mut context, None, |acc: &mut GovernorAccount| {
        acc.set_fee_version(&1);
    });

    // Two finalize transfers will fail
    tx_should_fail(&[finalize_transfer_ix.clone(), finalize_transfer_ix.clone()], &mut client, &mut context).await;

    ix_should_succeed(finalize_transfer_ix.clone(), &mut client, &mut context).await;

    // Second finalize transfer will fail
    ix_should_fail(finalize_ix, &mut client, &mut context).await;

    let network_fee = fee.proof_verification_network_fee(amount);
    let reward = fee.relayer_proof_reward;

    // Tx cost compensation, reward and rent paid to fee_payer
    assert_eq!(
        client_balance + reward - fee.lamports_per_tx,
        client.balance(&mut context).await
    );

    // network_fee sent to fee_collector
    assert_eq!(
        fee_collector_balance - subvention + network_fee,
        get_balance(&fee_collector, &mut context).await
    );

    // commitment_hash_fee remains in pool
    let commitment_hash_fee = fee.commitment_hash_fee(governor.get_commitment_batching_rate());
    assert_eq!(
        pool_balance + commitment_hash_fee,
        get_balance(&pool, &mut context).await
    );

    // amount sent to recipient
    assert_eq!(amount, get_balance(&recipient, &mut context).await);

    // verification_account and nullifier_duplicate_account closed
    assert!(account_does_not_exist(VerificationAccount::find(Some(0)).0, &mut context).await);
    assert!(account_does_not_exist(nullifier_duplicate_account, &mut context).await);

    // Check commitment queue
    queue!(queue, CommitmentQueue, CommitmentQueueAccount, None, context);
    let commitment = queue.view_first().unwrap();
    assert_eq!(queue.len(), 1);
    assert_eq!(commitment.commitment, request.public_inputs.join_split.commitment.reduce());
    assert_eq!(commitment.fee_version, 0);
}

fn ark_pvk<VKey: VerificationKey>() -> ark_groth16::PreparedVerifyingKey<ark_bn254::Bn254> {
    let mut gamma_abc_g1 = Vec::new();
    for i in 0..=VKey::PUBLIC_INPUTS_COUNT {
        gamma_abc_g1.push(VKey::gamma_abc_g1(i));
    }

    let vk = ark_groth16::VerifyingKey {
        alpha_g1: VKey::alpha_g1(),
        beta_g2: VKey::beta_g2(),
        gamma_g2: VKey::gamma_g2(),
        delta_g2: VKey::delta_g2(),
        gamma_abc_g1,
    };
    ark_groth16::prepare_verifying_key(&vk)
}

/// Returns the rent required for renting a nullifier_duplicate_account and verification_account
async fn verification_rent(context: &mut ProgramTestContext) -> u64 {
    get_account_cost(context, PDAAccountData::SIZE).await + get_account_cost(context, VerificationAccount::SIZE).await
}