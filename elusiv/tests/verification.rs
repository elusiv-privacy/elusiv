//! Tests the proof verification

#[cfg(not(tarpaulin_include))]
mod common;
use borsh::BorshDeserialize;
use common::*;
use elusiv::token::{LAMPORTS_TOKEN_ID, Lamports, USDC_TOKEN_ID, TokenPrice, TokenAuthorityAccount, Token, TOKENS, USDT_TOKEN_ID, spl_token_account_data};
use pyth_sdk_solana::Price;
use solana_program::program_pack::Pack;
use solana_program::system_program;
use elusiv::bytes::{ElusivOption, BorshSerDeSized};
use elusiv::fields::u256_from_str_skip_mr;
use elusiv::instruction::{ElusivInstruction, WritableUserAccount, SignerAccount, WritableSignerAccount, UserAccount};
use elusiv::proof::vkey::SendQuadraVKey;
use elusiv::proof::{VerificationAccount, prepare_public_inputs_instructions, VerificationState};
use elusiv::state::governor::{FeeCollectorAccount, PoolAccount};
use elusiv::state::empty_root_raw;
use elusiv::state::program_account::{PDAAccount, ProgramAccount, SizedAccount, PDAAccountData};
use elusiv::types::{RawU256, Proof, SendPublicInputs, JoinSplitPublicInputs, PublicInputs, compute_fee_rec_lamports, compute_fee_rec, RawProof, RecipientAccount};
use elusiv::proof::verifier::proof_from_str;
use elusiv::processor::{ProofRequest, FinalizeSendData};
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program::pubkey::Pubkey;
use solana_program_test::*;
use spl_associated_token_account::get_associated_token_address;

async fn start_verification_test() -> ElusivProgramTest {
    let mut test = ElusivProgramTest::start_with_setup().await;

    test.setup_storage_account().await;
    test.create_merkle_tree(0).await;
    test.create_merkle_tree(1).await;

    test
}

#[derive(Clone)]
struct FullSendRequest {
    proof: Proof,
    public_inputs: SendPublicInputs,
}

fn send_request(index: usize) -> FullSendRequest {
    let proof = proof_from_str(
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
    );

    let requests = vec![
        FullSendRequest {
            proof,
            public_inputs: SendPublicInputs {
                join_split: JoinSplitPublicInputs {
                    commitment_count: 1,
                    roots: vec![
                        Some(empty_root_raw()),
                    ],
                    nullifier_hashes: vec![
                        RawU256::new(u256_from_str_skip_mr("10026859857882131638516328056627849627085232677511724829502598764489185541935")),
                    ],
                    commitment: RawU256::new(u256_from_str_skip_mr("685960310506634721912121951341598678325833230508240750559904196809564625591")),
                    fee_version: 0,
                    amount: LAMPORTS_PER_SOL * 123,
                    fee: 0,
                    token_id: 0,
                },
                recipient: RecipientAccount::new(u256_from_str_skip_mr("19685960310506634721912121951341598678325833230508240750559904196809564625591"), true),
                current_time: 0,
                identifier: RawU256::new(u256_from_str_skip_mr("139214303935475888711984321184227760578793579443975701453971046059378311483")),
                salt: RawU256::new(u256_from_str_skip_mr("230508240750559904196809564625")),
            }
        },
        FullSendRequest {
            proof,
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
                    token_id: 0,
                },
                recipient: RecipientAccount::new(u256_from_str_skip_mr("19685960310506634721912121951341598678325833230508240750559904196809564625591"), true),
                current_time: 0,
                identifier: RawU256::new(u256_from_str_skip_mr("139214303935475888711984321184227760578793579443975701453971046059378311483")),
                salt: RawU256::new(u256_from_str_skip_mr("230508240750559904196809564625")),
            }
        },
        FullSendRequest {
            proof,
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
                    token_id: 0,
                },
                recipient: RecipientAccount::new(u256_from_str_skip_mr("19685960310506634721912121951341598678325833230508240750559904196809564625591"), true),
                current_time: 0,
                identifier: RawU256::new(u256_from_str_skip_mr("139214303935475888711984321184227760578793579443975701453971046059378311483")),
                salt: RawU256::new(u256_from_str_skip_mr("230508240750559904196809564625")),
            }
        },
        FullSendRequest {
            proof,
            public_inputs: SendPublicInputs {
                join_split: JoinSplitPublicInputs {
                    commitment_count: 3,
                    roots: vec![
                        Some(empty_root_raw()),
                        None,
                        None,
                    ],
                    nullifier_hashes: vec![
                        RawU256::new(u256_from_str_skip_mr("10026859857882131638516328056627849627085232677511724829502598764489185541935")),
                        RawU256::new(u256_from_str_skip_mr("19685960310506634721912121951341598678325833230508240750559904196809564625591")),
                        RawU256::new(u256_from_str_skip_mr("168596031050663472212195134159867832583323058240750559904196809564625591")),
                    ],
                    commitment: RawU256::new(u256_from_str_skip_mr("685960310506634721912121951341598678325833230508240750559904196809564625591")),
                    fee_version: 0,
                    amount: LAMPORTS_PER_SOL * 123,
                    fee: 0,
                    token_id: 0,
                },
                recipient: RecipientAccount::new(u256_from_str_skip_mr("19685960310506634721912121951341598678325833230508240750559904196809564625591"), true),
                current_time: 0,
                identifier: RawU256::new(u256_from_str_skip_mr("139214303935475888711984321184227760578793579443975701453971046059378311483")),
                salt: RawU256::new(u256_from_str_skip_mr("230508240750559904196809564625")),
            }
        },
        FullSendRequest {
            proof,
            public_inputs: SendPublicInputs {
                join_split: JoinSplitPublicInputs {
                    commitment_count: 4,
                    roots: vec![
                        Some(empty_root_raw()),
                        None,
                        None,
                        None,
                    ],
                    nullifier_hashes: vec![
                        RawU256::new(u256_from_str_skip_mr("10026859857882131638516328056627849627085232677511724829502598764489185541935")),
                        RawU256::new(u256_from_str_skip_mr("19685960310506634721912121951341598678325833230508240750559904196809564625591")),
                        RawU256::new(u256_from_str_skip_mr("168596031050663472212195134159867832583323058240750559904196809564625591")),
                        RawU256::new(u256_from_str_skip_mr("96859603105066347219121219513415986783258332305082407505599041968095646559")),
                    ],
                    commitment: RawU256::new(u256_from_str_skip_mr("685960310506634721912121951341598678325833230508240750559904196809564625591")),
                    fee_version: 0,
                    amount: LAMPORTS_PER_SOL * 123,
                    fee: 0,
                    token_id: 0,
                },
                recipient: RecipientAccount::new(u256_from_str_skip_mr("19685960310506634721912121951341598678325833230508240750559904196809564625591"), true),
                current_time: 0,
                identifier: RawU256::new(u256_from_str_skip_mr("139214303935475888711984321184227760578793579443975701453971046059378311483")),
                salt: RawU256::new(u256_from_str_skip_mr("230508240750559904196809564625")),
            }
        },
    ];
    requests[index].clone()
}

macro_rules! pda_token_account {
    ($ty: ty, $token_id: expr, $test: expr) => {
        {
            pda_account!(acc, $ty, None, $test);
            Pubkey::new(&acc.get_token_account($token_id).unwrap())
        }
    };
}

async fn skip_computation(verification_account_index: u32, success: bool, test: &mut ElusivProgramTest) {
    test.set_pda_account::<VerificationAccount, _>(Some(verification_account_index), |data| {
        let mut verification_account = VerificationAccount::new(data).unwrap();
        verification_account.set_is_verified(&ElusivOption::Some(success));
    }).await;
}

async fn set_verification_state(verification_account_index: u32, state: VerificationState, test: &mut ElusivProgramTest) {
    test.set_pda_account::<VerificationAccount, _>(Some(verification_account_index), |data| {
        let mut verification_account = VerificationAccount::new(data).unwrap();
        verification_account.set_state(&state);
    }).await;
}

#[tokio::test]
async fn test_init_proof_signers() {
    let mut test = start_verification_test().await;
    let warden = test.new_actor().await;
    let warden2 = test.new_actor().await;
    let nullifier_accounts = test.nullifier_accounts(0).await;

    let fee = test.genesis_fee().await;
    let mut request = send_request(0);
    compute_fee_rec_lamports::<SendQuadraVKey, _>(&mut request.public_inputs, &fee);

    let pool = PoolAccount::find(None).0;
    let fee_collector = FeeCollectorAccount::find(None).0;
    let recipient = request.public_inputs.recipient.pubkey();
    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;
    let subvention = fee.proof_subvention;
    let commitment_hash_fee = fee.commitment_hash_computation_fee(0);

    let verification_account_rent = test.rent(VerificationAccount::SIZE).await;
    let nullifier_duplicate_account_rent = test.rent(PDAAccountData::SIZE).await;
    warden.airdrop(
        LAMPORTS_TOKEN_ID,
        verification_account_rent.0 + nullifier_duplicate_account_rent.0,
        &mut test,
    ).await;
    warden.airdrop(LAMPORTS_TOKEN_ID, commitment_hash_fee.0, &mut test).await;
    warden2.airdrop(LAMPORTS_TOKEN_ID, commitment_hash_fee.0, &mut test).await;
    test.airdrop_lamports(&fee_collector, subvention.0).await;

    test.ix_should_succeed(
        ElusivInstruction::init_verification_instruction(
            0,
            [0, 1],
            ProofRequest::Send(request.public_inputs.clone()),
            false,
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(nullifier_duplicate_account),
            UserAccount(recipient),
            &user_accounts(&[nullifier_accounts[0]]),
            &[],
        ),
        &[&warden.keypair],
    ).await;

    // Invalid signer calls `init_verification_transfer_fee`
    test.ix_should_fail(
        ElusivInstruction::init_verification_transfer_fee_instruction(
            0,
            WritableSignerAccount(warden2.pubkey),
            WritableUserAccount(warden2.pubkey),
            WritableUserAccount(pool),
            WritableUserAccount(fee_collector),
            UserAccount(system_program::id()),
            UserAccount(system_program::id()),
            UserAccount(system_program::id()),
        ),
        &[&warden2.keypair],
    ).await;

    test.ix_should_succeed(
        ElusivInstruction::init_verification_transfer_fee_instruction(
            0,
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(warden.pubkey),
            WritableUserAccount(pool),
            WritableUserAccount(fee_collector),
            UserAccount(system_program::id()),
            UserAccount(system_program::id()),
            UserAccount(system_program::id()),
        ),
        &[&warden.keypair],
    ).await;

    // Invalid signer calls `init_verification_proof`
    test.ix_should_fail(
        ElusivInstruction::init_verification_proof_instruction(
            0,
            RawProof::try_from_slice(&vec![0; RawProof::SIZE]).unwrap().try_into().unwrap(),
            SignerAccount(warden2.pubkey),
        ),
        &[&warden2.keypair],
    ).await;

    test.ix_should_succeed(
        ElusivInstruction::init_verification_proof_instruction(
            0,
            RawProof::try_from_slice(&vec![0; RawProof::SIZE]).unwrap().try_into().unwrap(),
            SignerAccount(warden.pubkey),
        ),
        &[&warden.keypair],
    ).await;
}

#[tokio::test]
async fn test_init_proof_lamports() {
    let mut test = start_verification_test().await;
    let warden = test.new_actor().await;
    let nullifier_accounts = test.nullifier_accounts(0).await;

    let fee = test.genesis_fee().await;
    let mut request = send_request(0);
    compute_fee_rec_lamports::<SendQuadraVKey, _>(&mut request.public_inputs, &fee);

    let pool = PoolAccount::find(None).0;
    let fee_collector = FeeCollectorAccount::find(None).0;
    let recipient = request.public_inputs.recipient.pubkey();
    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;

    let verification_account_rent = test.rent(VerificationAccount::SIZE).await;
    let nullifier_duplicate_account_rent = test.rent(PDAAccountData::SIZE).await;
    warden.airdrop(
        LAMPORTS_TOKEN_ID,
        verification_account_rent.0 + nullifier_duplicate_account_rent.0,
        &mut test,
    ).await;

    let init_verification_instruction = |v_index: u32, skip_nullifier_pda: bool| {
        ElusivInstruction::init_verification_instruction(
            v_index,
            [0, 1],
            ProofRequest::Send(request.public_inputs.clone()),
            skip_nullifier_pda,
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(nullifier_duplicate_account),
            UserAccount(recipient),
            &user_accounts(&[nullifier_accounts[0]]),
            &[],
        )
    };

    // TODO: Failure: recipient is spl-token-account

    // Failure if skip_nullifier_pda := true (and nullifier_pda does not exist)
    test.ix_should_fail(
        init_verification_instruction(0, true),
        &[&warden.keypair],
    ).await;

    test.ix_should_succeed(
        init_verification_instruction(0, false),
        &[&warden.keypair],
    ).await;

    assert_eq!(0, warden.lamports(&mut test).await);

    warden.airdrop(LAMPORTS_TOKEN_ID, verification_account_rent.0, &mut test).await;

    // Testing duplicate verifications (allowed when flag is set)
    // Failure if skip_nullifier_pda := false (and nullifier_pda exists)
    test.ix_should_fail(
        init_verification_instruction(1, false),
        &[&warden.keypair],
    ).await;
    // Success if skip_nullifier_pda := true (and nullifier_pda exists)
    test.ix_should_succeed(
        init_verification_instruction(1, true),
        &[&warden.keypair],
    ).await;

    assert_eq!(0, warden.lamports(&mut test).await);

    let subvention = fee.proof_subvention;
    let commitment_hash_fee = fee.commitment_hash_computation_fee(0);

    warden.airdrop(LAMPORTS_TOKEN_ID, commitment_hash_fee.0, &mut test).await;
    test.airdrop_lamports(&fee_collector, subvention.0).await;

    test.ix_should_succeed(
        ElusivInstruction::init_verification_transfer_fee_instruction(
            0,
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(warden.pubkey),
            WritableUserAccount(pool),
            WritableUserAccount(fee_collector),
            UserAccount(system_program::id()),
            UserAccount(system_program::id()),
            UserAccount(system_program::id()),
        ),
        &[&warden.keypair],
    ).await;

    assert_eq!(0, warden.lamports(&mut test).await);
    assert_eq!(0, test.pda_lamports(&fee_collector, FeeCollectorAccount::SIZE).await.0);
    assert_eq!(commitment_hash_fee.0 + subvention.0, test.pda_lamports(&pool, PoolAccount::SIZE).await.0);
}

#[tokio::test]
async fn test_init_proof_token() {
    let mut test = start_verification_test().await;
    test.create_spl_token(USDC_TOKEN_ID, true).await;

    let mut recipient = test.new_actor().await;
    recipient.open_token_account(USDC_TOKEN_ID, 0, &mut test).await;

    let mut warden = test.new_actor().await;
    warden.open_token_account(USDC_TOKEN_ID, 0, &mut test).await;

    let sol_usd_price = Price { price: 41, conf: 0, expo: 0};
    let usdc_usd_price = Price { price: 1, conf: 0, expo: 0 };
    let price = TokenPrice::new_from_sol_price(sol_usd_price, usdc_usd_price, USDC_TOKEN_ID).unwrap();
    let sol_price_account = test.token_to_usd_price_pyth_account(0);
    let token_price_account = test.token_to_usd_price_pyth_account(USDC_TOKEN_ID);
    test.set_token_to_usd_price_pyth(0, sol_usd_price).await;
    test.set_token_to_usd_price_pyth(USDC_TOKEN_ID, usdc_usd_price).await;

    let mut request = send_request(0);
    request.public_inputs.join_split.token_id = USDC_TOKEN_ID;
    request.public_inputs.join_split.amount = 1_000_000;

    let fee = test.genesis_fee().await;
    compute_fee_rec::<SendQuadraVKey, _>(&mut request.public_inputs, &fee, &price);

    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;
    let nullifier_accounts = test.nullifier_accounts(0).await;

    let verification_account_rent = test.rent(VerificationAccount::SIZE).await;
    let nullifier_duplicate_account_rent = test.rent(PDAAccountData::SIZE).await;
    warden.airdrop(
        LAMPORTS_TOKEN_ID,
        verification_account_rent.0 + nullifier_duplicate_account_rent.0,
        &mut test,
    ).await;

    let init_instruction = |public_inputs: SendPublicInputs, recipient: Pubkey| {
        ElusivInstruction::init_verification_instruction(
            0,
            [0, 1],
            ProofRequest::Send(public_inputs),
            false,
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(nullifier_duplicate_account),
            UserAccount(recipient),
            &user_accounts(&[nullifier_accounts[0]]),
            &[],
        )
    };

    // Failure: recipient is not token account of correct mint
    {
        let recipient = request.public_inputs.recipient.pubkey();
        test.ix_should_fail(
            init_instruction(request.public_inputs.clone(), recipient),
            &[&warden.keypair],
        ).await;
    }

    let recipient_token_account = recipient.get_token_account(USDC_TOKEN_ID);
    request.public_inputs.recipient.address = recipient_token_account.to_bytes();

    test.ix_should_succeed(
        init_instruction(request.public_inputs.clone(), recipient_token_account),
        &[&warden.keypair],
    ).await;

    assert_eq!(0, warden.lamports(&mut test).await);

    let subvention = fee.proof_subvention.into_token(&price, USDC_TOKEN_ID).unwrap();
    let commitment_hash_fee = fee.commitment_hash_computation_fee(0);

    pda_account!(pool, PoolAccount, None, test);
    let pool_account = Pubkey::new(&pool.get_token_account(USDC_TOKEN_ID).unwrap());

    pda_account!(fee_collector, FeeCollectorAccount, None, test);
    let fee_collector_account = Pubkey::new(&fee_collector.get_token_account(USDC_TOKEN_ID).unwrap());

    warden.airdrop(LAMPORTS_TOKEN_ID, commitment_hash_fee.0, &mut test).await;
    test.airdrop(&fee_collector_account, subvention).await;

    test.ix_should_succeed(
        ElusivInstruction::init_verification_transfer_fee_instruction(
            0,
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(warden.get_token_account(USDC_TOKEN_ID)),
            WritableUserAccount(pool_account),
            WritableUserAccount(fee_collector_account),
            UserAccount(sol_price_account),
            UserAccount(token_price_account),
            UserAccount(spl_token::id()),
        ),
        &[&warden.keypair],
    ).await;

    assert_eq!(0, warden.lamports(&mut test).await);
    assert_eq!(0, warden.balance(USDC_TOKEN_ID, &mut test).await);
    assert_eq!(commitment_hash_fee.0, test.pda_lamports(&PoolAccount::find(None).0, PoolAccount::SIZE).await.0);
    assert_eq!(subvention.amount(), test.spl_balance(&pool_account).await);
}

#[tokio::test]
#[ignore]
async fn test_finalize_proof() {
    // TODO: test invalid timestamp
    // TODO: Failure: invalid recipient
    // TODO: Failure: invalid fee_payer
    panic!()
}

#[tokio::test]
async fn test_finalize_proof_lamports() {
    let mut test = start_verification_test().await;
    let warden = test.new_actor().await;
    let recipient = test.new_actor().await;
    let nullifier_accounts = test.nullifier_accounts(0).await;
    let fee = test.genesis_fee().await;

    let mut request = send_request(0);
    request.public_inputs.recipient.address = recipient.pubkey.to_bytes();
    compute_fee_rec_lamports::<SendQuadraVKey, _>(&mut request.public_inputs, &fee);

    let pool = PoolAccount::find(None).0;
    let fee_collector = FeeCollectorAccount::find(None).0;
    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;

    let public_inputs = request.public_inputs.public_signals_skip_mr();
    let input_preparation_tx_count = prepare_public_inputs_instructions::<SendQuadraVKey>(&public_inputs).len();
    let subvention = fee.proof_subvention;
    let proof_verification_fee = fee.proof_verification_computation_fee(input_preparation_tx_count);
    let commitment_hash_fee = fee.commitment_hash_computation_fee(0);
    let network_fee = Lamports(fee.proof_network_fee.calc(request.public_inputs.join_split.amount));
    let verification_account_rent = test.rent(VerificationAccount::SIZE).await;
    let nullifier_duplicate_account_rent = test.rent(PDAAccountData::SIZE).await;

    warden.airdrop(
        LAMPORTS_TOKEN_ID,
        verification_account_rent.0 + nullifier_duplicate_account_rent.0 + commitment_hash_fee.0,
        &mut test,
    ).await;
    test.airdrop_lamports(&fee_collector, subvention.0).await;

    // Init
    test.tx_should_succeed(
        &[
            ElusivInstruction::init_verification_instruction(
                0,
                [0, 1],
                ProofRequest::Send(request.public_inputs.clone()),
                false,
                WritableSignerAccount(warden.pubkey),
                WritableUserAccount(nullifier_duplicate_account),
                UserAccount(recipient.pubkey),
                &user_accounts(&[nullifier_accounts[0]]),
                &[],
            ),
            ElusivInstruction::init_verification_transfer_fee_sol_instruction(
                0,
                warden.pubkey,
            ),
            ElusivInstruction::init_verification_proof_instruction(
                0,
                request.proof,
                SignerAccount(warden.pubkey),
            ),
        ],
        &[&warden.keypair],
    ).await;

    assert_eq!(0, warden.lamports(&mut test).await);
    assert_eq!(0, test.pda_lamports(&fee_collector, FeeCollectorAccount::SIZE).await.0);

    // Skip computation
    skip_computation(0, true, &mut test).await;

    let identifier = Pubkey::new(&request.public_inputs.identifier.skip_mr());
    let salt = Pubkey::new(&request.public_inputs.salt.skip_mr());

    // Finalize
    test.ix_should_succeed_simple(
        ElusivInstruction::finalize_verification_send_instruction(
            FinalizeSendData {
                timestamp: 0,
                total_amount: request.public_inputs.join_split.total_amount(),
                token_id: 0,
                mt_index: 0,
                commitment_index: 0,
            },
            0,
            UserAccount(identifier),
            UserAccount(salt),
        )
    ).await;

    test.ix_should_succeed_simple(
        ElusivInstruction::finalize_verification_send_nullifiers_instruction(
            0,
            Some(0),
            &writable_user_accounts(&[nullifier_accounts[0]]),
            Some(1),
            &[],
        )
    ).await;

    // IMPORTANT: Pool already contains subvention (so we airdrop commitment_hash_fee - subvention)
    test.airdrop_lamports(
        &pool,
        request.public_inputs.join_split.amount + commitment_hash_fee.0 - subvention.0 + proof_verification_fee.0 + network_fee.0
    ).await;

    test.ix_should_succeed_simple(
        ElusivInstruction::finalize_verification_transfer_lamports_instruction(
            0,
            WritableUserAccount(recipient.pubkey),
            WritableUserAccount(warden.pubkey),
            WritableUserAccount(nullifier_duplicate_account),
        )
    ).await;

    assert!(test.account_does_not_exist(&VerificationAccount::find(Some(0)).0).await);
    assert!(test.account_does_not_exist(&nullifier_duplicate_account).await);

    assert_eq!(
        commitment_hash_fee.0 + proof_verification_fee.0 + verification_account_rent.0 + nullifier_duplicate_account_rent.0,
        warden.lamports(&mut test).await
    );
    assert_eq!(
        request.public_inputs.join_split.amount,
        recipient.lamports(&mut test).await
    );

    // fee_collector has network_fee (lamports)
    assert_eq!(network_fee.0, test.pda_lamports(&fee_collector, FeeCollectorAccount::SIZE).await.0);

    // pool has computation_fee (lamports)
    assert_eq!(commitment_hash_fee.0, test.pda_lamports(&pool, PoolAccount::SIZE).await.0);
}

#[tokio::test]
async fn test_finalize_proof_token() {
    let mut test = start_verification_test().await;
    test.create_spl_token(USDC_TOKEN_ID, true).await;

    let mut recipient = test.new_actor().await;
    recipient.open_token_account(USDC_TOKEN_ID, 0, &mut test).await;

    let mut warden = test.new_actor().await;
    warden.open_token_account(USDC_TOKEN_ID, 0, &mut test).await;

    let sol_usd_price = Price { price: 41, conf: 0, expo: 0};
    let usdc_usd_price = Price { price: 1, conf: 0, expo: 0 };
    let price = TokenPrice::new_from_sol_price(sol_usd_price, usdc_usd_price, USDC_TOKEN_ID).unwrap();
    let sol_price_account = test.token_to_usd_price_pyth_account(0);
    let token_price_account = test.token_to_usd_price_pyth_account(USDC_TOKEN_ID);
    test.set_token_to_usd_price_pyth(0, sol_usd_price).await;
    test.set_token_to_usd_price_pyth(USDC_TOKEN_ID, usdc_usd_price).await;

    let mut request = send_request(0);
    request.public_inputs.join_split.token_id = USDC_TOKEN_ID;
    request.public_inputs.join_split.amount = 1_000_000;

    let nullifier_accounts = test.nullifier_accounts(0).await;
    let fee = test.genesis_fee().await;
    compute_fee_rec::<SendQuadraVKey, _>(&mut request.public_inputs, &fee, &price);

    let recipient_token_account = recipient.get_token_account(USDC_TOKEN_ID);
    request.public_inputs.recipient.address = recipient_token_account.to_bytes();

    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;

    let public_inputs = request.public_inputs.public_signals_skip_mr();
    let input_preparation_tx_count = prepare_public_inputs_instructions::<SendQuadraVKey>(&public_inputs).len();
    let subvention = fee.proof_subvention.into_token(&price, USDC_TOKEN_ID).unwrap();
    let proof_verification_fee = fee.proof_verification_computation_fee(input_preparation_tx_count).into_token(&price, USDC_TOKEN_ID).unwrap();
    let commitment_hash_fee = fee.commitment_hash_computation_fee(0);
    let commitment_hash_fee_token = commitment_hash_fee.into_token(&price, USDC_TOKEN_ID).unwrap();
    let network_fee = Token::new(USDC_TOKEN_ID, fee.proof_network_fee.calc(request.public_inputs.join_split.amount));
    let verification_account_rent = test.rent(VerificationAccount::SIZE).await;
    let nullifier_duplicate_account_rent = test.rent(PDAAccountData::SIZE).await;

    pda_account!(pool, PoolAccount, None, test);
    let pool_account = Pubkey::new(&pool.get_token_account(USDC_TOKEN_ID).unwrap());

    pda_account!(fee_collector, FeeCollectorAccount, None, test);
    let fee_collector_account = Pubkey::new(&fee_collector.get_token_account(USDC_TOKEN_ID).unwrap());

    warden.airdrop(
        LAMPORTS_TOKEN_ID,
        verification_account_rent.0 + nullifier_duplicate_account_rent.0 + commitment_hash_fee.0,
        &mut test,
    ).await;
    test.airdrop(&fee_collector_account, subvention).await;

    // Init
    test.tx_should_succeed(
        &[
            ElusivInstruction::init_verification_instruction(
                0,
                [0, 1],
                ProofRequest::Send(request.public_inputs.clone()),
                false,
                WritableSignerAccount(warden.pubkey),
                WritableUserAccount(nullifier_duplicate_account),
                UserAccount(recipient_token_account),
                &user_accounts(&[nullifier_accounts[0]]),
                &[],
            ),
            ElusivInstruction::init_verification_transfer_fee_instruction(
                0,
                WritableSignerAccount(warden.pubkey),
                WritableUserAccount(warden.get_token_account(USDC_TOKEN_ID)),
                WritableUserAccount(pool_account),
                WritableUserAccount(fee_collector_account),
                UserAccount(sol_price_account),
                UserAccount(token_price_account),
                UserAccount(spl_token::id()),
            ),
            ElusivInstruction::init_verification_proof_instruction(
                0,
                request.proof,
                SignerAccount(warden.pubkey),
            ),
        ],
        &[&warden.keypair],
    ).await;

    assert_eq!(0, warden.lamports(&mut test).await);
    assert_eq!(0, warden.balance(USDC_TOKEN_ID, &mut test).await);
    assert_eq!(commitment_hash_fee.0, test.pda_lamports(&PoolAccount::find(None).0, PoolAccount::SIZE).await.0);
    assert_eq!(subvention.amount(), test.spl_balance(&pool_account).await);

    skip_computation(0, true, &mut test).await;

    let identifier = Pubkey::new(&request.public_inputs.identifier.skip_mr());
    let salt = Pubkey::new(&request.public_inputs.salt.skip_mr());

    // Finalize
    test.ix_should_succeed_simple(
        ElusivInstruction::finalize_verification_send_instruction(
            FinalizeSendData {
                timestamp: 0,
                total_amount: request.public_inputs.join_split.total_amount(),
                token_id: USDC_TOKEN_ID,
                mt_index: 0,
                commitment_index: 0,
            },
            0,
            UserAccount(identifier),
            UserAccount(salt),
        )
    ).await;

    test.ix_should_succeed_simple(
        ElusivInstruction::finalize_verification_send_nullifiers_instruction(
            0,
            Some(0),
            &writable_user_accounts(&[nullifier_accounts[0]]),
            Some(1),
            &[],
        )
    ).await;

    // IMPORTANT: Pool already contains subvention (so we airdrop commitment_hash_fee - subvention)
    test.airdrop(
        &pool_account,
        Token::new(
            USDC_TOKEN_ID,
            request.public_inputs.join_split.amount + commitment_hash_fee_token.amount() - subvention.amount() + proof_verification_fee.amount() + network_fee.amount()
        )
    ).await;

    test.ix_should_succeed(
        ElusivInstruction::finalize_verification_transfer_token_instruction(
            0,
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(recipient_token_account),
            UserAccount(recipient_token_account),
            WritableUserAccount(warden.pubkey),
            WritableUserAccount(warden.get_token_account(USDC_TOKEN_ID)),
            WritableUserAccount(pool_account),
            WritableUserAccount(fee_collector_account),
            WritableUserAccount(nullifier_duplicate_account),
            UserAccount(spl_token::id()),
        ),
        &[&warden.keypair],
    ).await;

    assert!(test.account_does_not_exist(&VerificationAccount::find(Some(0)).0).await);
    assert!(test.account_does_not_exist(&nullifier_duplicate_account).await);

    assert_eq!(
        verification_account_rent.0 + nullifier_duplicate_account_rent.0,
        warden.lamports(&mut test).await
    );

    // warden has proof_verification_fee and commitment_hash_fee (token)
    assert_eq!(
        proof_verification_fee.amount() + commitment_hash_fee_token.amount(),
        warden.balance(USDC_TOKEN_ID, &mut test).await
    );

    // recipient has amount (token)
    assert_eq!(
        request.public_inputs.join_split.amount,
        recipient.balance(USDC_TOKEN_ID, &mut test).await
    );

    // fee_collector has network_fee (token)
    assert_eq!(network_fee.amount(), test.spl_balance(&fee_collector_account).await);

    // Pool contains computation_fee (lamports)
    assert_eq!(commitment_hash_fee.0, test.pda_lamports(&PoolAccount::find(None).0, PoolAccount::SIZE).await.0);
}

#[tokio::test]
async fn test_finalize_proof_skip_nullifier_pda() {
    let mut test = start_verification_test().await;
    let warden = test.new_actor().await;
    let recipient = test.new_actor().await;
    let nullifier_accounts = test.nullifier_accounts(0).await;
    let pool = PoolAccount::find(None).0;
    let fee_collector = FeeCollectorAccount::find(None).0;
    let mut request = send_request(0);
    request.public_inputs.recipient.address = recipient.pubkey.to_bytes();
    compute_fee_rec_lamports::<SendQuadraVKey, _>(&mut request.public_inputs, &test.genesis_fee().await);
    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;
    let identifier = Pubkey::new(&request.public_inputs.identifier.skip_mr());
    let salt = Pubkey::new(&request.public_inputs.salt.skip_mr());

    warden.airdrop(LAMPORTS_TOKEN_ID, LAMPORTS_PER_SOL, &mut test).await;
    test.airdrop_lamports(&fee_collector, LAMPORTS_PER_SOL).await;
    test.airdrop_lamports(&pool, LAMPORTS_PER_SOL * 1000).await;

    let init_instructions = |v_index: u32, skip_nullifier_pda: bool| {
        [
            ElusivInstruction::init_verification_instruction(
                v_index,
                [0, 1],
                ProofRequest::Send(request.public_inputs.clone()),
                skip_nullifier_pda,
                WritableSignerAccount(warden.pubkey),
                WritableUserAccount(nullifier_duplicate_account),
                UserAccount(recipient.pubkey),
                &user_accounts(&[nullifier_accounts[0]]),
                &[],
            ),
            ElusivInstruction::init_verification_transfer_fee_sol_instruction(
                v_index,
                warden.pubkey,
            ),
            ElusivInstruction::init_verification_proof_instruction(
                v_index,
                request.proof,
                SignerAccount(warden.pubkey),
            ),
        ]
    };

    // Three verifications of the same proof (the last one is simulated as an invalid proof)
    test.tx_should_succeed(&init_instructions(0, false), &[&warden.keypair]).await;
    test.tx_should_succeed(&init_instructions(1, true), &[&warden.keypair]).await;
    test.tx_should_succeed(&init_instructions(2, true), &[&warden.keypair]).await;

    // Skip computations
    for (i, is_valid) in (0..3).zip([true, true, false]) {
        skip_computation(i, is_valid, &mut test).await;
    }

    let finalize = |v_index: u32, is_valid: bool| {
        let ixs = [
            ElusivInstruction::finalize_verification_send_instruction(
                FinalizeSendData {
                    timestamp: 0,
                    total_amount: request.public_inputs.join_split.total_amount(),
                    token_id: 0,
                    mt_index: 0,
                    commitment_index: 0,
                },
                v_index,
                UserAccount(identifier),
                UserAccount(salt),
            ),
            ElusivInstruction::finalize_verification_send_nullifiers_instruction(
                v_index,
                Some(0),
                &writable_user_accounts(&[nullifier_accounts[0]]),
                Some(1),
                &[],
            ),
            ElusivInstruction::finalize_verification_transfer_lamports_instruction(
                v_index,
                WritableUserAccount(recipient.pubkey),
                WritableUserAccount(warden.pubkey),
                WritableUserAccount(nullifier_duplicate_account),
            ),
        ];

        if is_valid {
            ixs.to_vec()
        } else {
            vec![ixs[0].clone(), ixs[2].clone()]
        }
    };

    // Invalid verification (will not close nullifier_duplicate_pda)
    test.tx_should_succeed_simple(&finalize(2, false)).await;

    // 2. verification is faster than 1. (will not close nullifier_duplicate_pda)
    test.tx_should_succeed_simple(&finalize(1, true)).await;

    // 1. verification is unable to complete
    test.tx_should_fail_simple(&finalize(0, true)).await;

    assert!(test.account_does_not_exist(&VerificationAccount::find(Some(1)).0).await);
    assert!(test.account_does_not_exist(&VerificationAccount::find(Some(2)).0).await);
    assert!(test.account_does_exist(&VerificationAccount::find(Some(0)).0).await);
    assert!(test.account_does_exist(&nullifier_duplicate_account).await);
}

#[tokio::test]
async fn test_associated_token_account() {
    let mut test = start_verification_test().await;
    test.create_spl_token(USDC_TOKEN_ID, true).await;

    let mut warden = test.new_actor().await;
    warden.open_token_account(USDC_TOKEN_ID, 0, &mut test).await;

    let sol_usd_price = Price { price: 41, conf: 0, expo: 0};
    let usdc_usd_price = Price { price: 1, conf: 0, expo: 0 };
    let price = TokenPrice::new_from_sol_price(sol_usd_price, usdc_usd_price, USDC_TOKEN_ID).unwrap();
    test.set_token_to_usd_price_pyth(0, sol_usd_price).await;
    test.set_token_to_usd_price_pyth(USDC_TOKEN_ID, usdc_usd_price).await;

    let mut request = send_request(0);
    let recipient = test.new_actor().await;
    request.public_inputs.recipient = RecipientAccount::new(recipient.pubkey.to_bytes(), false);
    request.public_inputs.join_split.token_id = USDC_TOKEN_ID;
    request.public_inputs.join_split.amount = 1_000_000;

    let fee = test.genesis_fee().await;
    compute_fee_rec::<SendQuadraVKey, _>(&mut request.public_inputs, &fee, &price);
    let subvention = fee.proof_subvention.into_token(&price, USDC_TOKEN_ID).unwrap();
    let commitment_hash_fee = fee.commitment_hash_computation_fee(0);

    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;
    let nullifier_accounts = test.nullifier_accounts(0).await;

    let verification_account_rent = test.rent(VerificationAccount::SIZE).await;
    let nullifier_duplicate_account_rent = test.rent(PDAAccountData::SIZE).await;
    let token_account_rent = test.rent(spl_token::state::Account::LEN).await;
    let token_account_rent_token = token_account_rent.into_token(&price, USDC_TOKEN_ID).unwrap();
    warden.airdrop(
        LAMPORTS_TOKEN_ID,
        commitment_hash_fee.0 + verification_account_rent.0 + nullifier_duplicate_account_rent.0 + token_account_rent.0,
        &mut test,
    ).await;

    let pool_account = pda_token_account!(PoolAccount, USDC_TOKEN_ID, test);
    let fee_collector_account = pda_token_account!(FeeCollectorAccount, USDC_TOKEN_ID, test);
    test.airdrop(&fee_collector_account, subvention).await;

    test.ix_should_succeed(
        ElusivInstruction::init_verification_instruction(
            0,
            [0, 1],
            ProofRequest::Send(request.clone().public_inputs),
            false,
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(nullifier_duplicate_account),
            UserAccount(recipient.pubkey),
            &user_accounts(&[nullifier_accounts[0]]),
            &[],
        ),
        &[&warden.keypair],
    ).await;

    let transfer_ix = ElusivInstruction::init_verification_transfer_fee_token_instruction(
        0,
        USDC_TOKEN_ID,
        warden.pubkey,
        warden.get_token_account(USDC_TOKEN_ID),
        pool_account,
        fee_collector_account,
    );
    test.ix_should_succeed(transfer_ix.clone(), &[&warden.keypair]).await;

    assert_eq!(0, warden.lamports(&mut test).await);
    assert_eq!(
        token_account_rent.0 + commitment_hash_fee.0,
        test.pda_lamports(&PoolAccount::find(None).0, PoolAccount::SIZE).await.0
    );

    skip_computation(0, true, &mut test).await;
    set_verification_state(0, VerificationState::Finalized, &mut test).await;

    test.airdrop(&pool_account, Token::new(USDC_TOKEN_ID, 100_000_000)).await;
    test.airdrop_lamports(
        &PoolAccount::find(None).0,
        1_000_000_000
    ).await;

    let mint = TOKENS[USDC_TOKEN_ID as usize].mint;
    let associated_token_account = get_associated_token_address(&recipient.pubkey, &mint);
    let associated_token_account_invalid = get_associated_token_address(&recipient.pubkey, &TOKENS[USDT_TOKEN_ID as usize].mint);

    let signer = test.new_actor().await;
    let transfer_ix = |recipient: Pubkey, recipient_wallet: Pubkey| {
        ElusivInstruction::finalize_verification_transfer_token_instruction(
            0,
            WritableSignerAccount(signer.pubkey),
            WritableUserAccount(recipient),
            UserAccount(recipient_wallet),
            WritableUserAccount(warden.pubkey),
            WritableUserAccount(warden.get_token_account(USDC_TOKEN_ID)),
            WritableUserAccount(pool_account),
            WritableUserAccount(fee_collector_account),
            WritableUserAccount(nullifier_duplicate_account),
            UserAccount(mint),
        )
    };

    let valid_ix = transfer_ix(associated_token_account, recipient.pubkey);
    let test_fork = test.fork_for_instructions(&[valid_ix.clone()]).await;
    let test_fork2 = test.fork_for_instructions(&[valid_ix.clone()]).await;

    // Failure: missing signature
    test.ix_should_fail_simple(valid_ix.clone()).await;

    // Failure: Invalid recipient wallet
    test.ix_should_fail(
        transfer_ix(associated_token_account, warden.pubkey),
        &[&signer.keypair],
    ).await;

    // Failure: Invalid recipient associated token account
    test.ix_should_fail(
        transfer_ix(associated_token_account_invalid, recipient.pubkey),
        &[&signer.keypair],
    ).await;

    test.ix_should_succeed(valid_ix, &[&signer.keypair]).await;

    // Check funds
    assert_eq!(
        request.public_inputs.join_split.amount - token_account_rent_token.amount(),
        test.spl_balance(&associated_token_account).await
    );
    assert_eq!(0, signer.lamports(&mut test).await);
    assert_eq!(
        verification_account_rent.0 + nullifier_duplicate_account_rent.0,
        warden.lamports(&mut test).await
    );

    // Test failure case
    {
        let mut test = test_fork;
        skip_computation(0, false, &mut test).await;

        test.ix_should_succeed(
            transfer_ix(associated_token_account, recipient.pubkey),
            &[&signer.keypair],
        ).await;

        // All funds should flow to fee_collector
        assert_eq!(
            subvention.amount(),
            test.spl_balance(&fee_collector_account).await
        );
        assert_eq!(
            token_account_rent.0 + commitment_hash_fee.0 + verification_account_rent.0 + nullifier_duplicate_account_rent.0,
            test.pda_lamports(&FeeCollectorAccount::find(None).0, FeeCollectorAccount::SIZE).await.0
        );
        assert_eq!(0, signer.lamports(&mut test).await);
    }

    // Associated token account already exists
    {
        let mut test = test_fork2;

        test.set_account_rent_exempt(&associated_token_account, &spl_token_account_data(USDC_TOKEN_ID), &spl_token::ID).await;
        /*test.ix_should_succeed_simple(
            create_associated_token_account(
                &test.payer(),
                &recipient.pubkey,
                &mint,
                &spl_token::ID,
            )
        ).await;*/

        test.ix_should_succeed(
            transfer_ix(associated_token_account, recipient.pubkey),
            &[&signer.keypair],
        ).await;

        assert_eq!(0, signer.lamports(&mut test).await);
        assert_eq!(
            request.public_inputs.join_split.amount,
            test.spl_balance(&associated_token_account).await
        );
        assert_eq!(
            token_account_rent.0 + verification_account_rent.0 + nullifier_duplicate_account_rent.0,
            warden.lamports(&mut test).await
        );
    }
}

#[tokio::test]
#[ignore]
async fn test_finalize_proof_failure_lamports() {
    panic!()
}

#[tokio::test]
#[ignore]
async fn test_finalize_proof_failure_token() {
    panic!()
}