//! Tests the proof verification

mod common;

use borsh::{BorshSerialize, BorshDeserialize};
use common::*;
use elusiv::token::{LAMPORTS_TOKEN_ID, Lamports, USDC_TOKEN_ID, TokenPrice, Token, TOKENS, USDT_TOKEN_ID, spl_token_account_data};
use elusiv_computation::PartialComputation;
use elusiv_types::MultiAccountAccountData;
use pyth_sdk_solana::Price;
use solana_program::program_pack::Pack;
use solana_program::system_program;
use elusiv::bytes::{ElusivOption, BorshSerDeSized};
use elusiv::instruction::{ElusivInstruction, WritableUserAccount, SignerAccount, WritableSignerAccount, UserAccount};
use elusiv::proof::vkey::{SendQuadraVKey, VerifyingKeyInfo, VKeyAccount, VKeyAccountEager};
use elusiv::proof::{VerificationAccount, prepare_public_inputs_instructions, VerificationState, CombinedMillerLoop};
use elusiv::state::governor::{FeeCollectorAccount, PoolAccount};
use elusiv::state::{empty_root_raw, NullifierMap, NULLIFIERS_PER_ACCOUNT};
use elusiv::state::program_account::{PDAAccount, ProgramAccount, SizedAccount, PDAAccountData};
use elusiv::types::{RawU256, Proof, SendPublicInputs, JoinSplitPublicInputs, PublicInputs, compute_fee_rec_lamports, compute_fee_rec, RawProof, OrdU256, U256, compute_extra_data_hash};
use elusiv::proof::verifier::proof_from_str;
use elusiv::processor::{ProofRequest, FinalizeSendData, program_token_account_address};
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
                recipient_is_associated_token_account: false,
                current_time: 0,
                extra_data_hash: u256_from_str_skip_mr(DEFAULT_EXTRA_DATA_HASH),
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
                recipient_is_associated_token_account: false,
                current_time: 0,
                extra_data_hash: u256_from_str_skip_mr(DEFAULT_EXTRA_DATA_HASH),
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
                recipient_is_associated_token_account: false,
                current_time: 0,
                extra_data_hash: u256_from_str_skip_mr(DEFAULT_EXTRA_DATA_HASH),
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
                recipient_is_associated_token_account: false,
                current_time: 0,
                extra_data_hash: u256_from_str_skip_mr(DEFAULT_EXTRA_DATA_HASH),
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
                recipient_is_associated_token_account: false,
                current_time: 0,
                extra_data_hash: u256_from_str_skip_mr(DEFAULT_EXTRA_DATA_HASH),
            }
        },
    ];
    requests[index].clone()
}

struct ExtraData {
    recipient: U256,
    identifier: U256,
    iv: U256,
    encrypted_owner: U256,
}

const DEFAULT_EXTRA_DATA_HASH: &str = "241513166508321350627618709707967777063380694253583200648944705250489865558";

impl Default for ExtraData {
    fn default() -> Self {
        ExtraData {
            recipient: u256_from_str_skip_mr("115792089237316195423570985008687907853269984665640564039457584007913129639935"),
            identifier: u256_from_str_skip_mr("1"),
            iv: u256_from_str_skip_mr("5683487854789"),
            encrypted_owner: u256_from_str_skip_mr("5789489458548458945478235642378"),
        }
    }
}

impl ExtraData {
    fn hash(&self) -> U256 {
        compute_extra_data_hash(self.recipient, self.identifier, self.iv, self.encrypted_owner)
    }
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

async fn skip_finalize_verification_send(verification_account_index: u32, recipient: &Pubkey, test: &mut ElusivProgramTest) {
    test.set_pda_account::<VerificationAccount, _>(Some(verification_account_index), |data| {
        let mut verification_account = VerificationAccount::new(data).unwrap();
        let mut other_data = verification_account.get_other_data();
        other_data.recipient_wallet = ElusivOption::Some(RawU256::new(recipient.to_bytes()));
        verification_account.set_other_data(&other_data);
    }).await;
}

async fn setup_vkey_account<VKey: VerifyingKeyInfo>(test: &mut ElusivProgramTest) -> (Pubkey, Pubkey) {
    let sub_account_pubkey = Pubkey::new_unique();
    let mut data = VKey::verifying_key_source();
    data.insert(0, 1);
    test.set_account_rent_exempt(&sub_account_pubkey, &data, &elusiv::id()).await;

    let (pda, bump) = VKeyAccount::find(Some(VKey::VKEY_ID));
    let data = VKeyAccountEager {
        pda_data: PDAAccountData {
            bump_seed: bump,
            version: 0,
            initialized: true,
        },
        multi_account_data: MultiAccountAccountData {
            pubkeys: [ElusivOption::Some(sub_account_pubkey)]
        },
        vkey_id: VKey::VKEY_ID,
        hash: VKey::HASH,
        public_inputs_count: VKey::PUBLIC_INPUTS_COUNT,
        is_checked: true,
        deploy_authority: ElusivOption::None,
        instruction: 0,
    }.try_to_vec().unwrap();
    test.set_program_account_rent_exempt(&pda, &data).await;

    (pda, sub_account_pubkey)
}

#[tokio::test]
async fn test_init_proof_signers() {
    let mut test = start_verification_test().await;
    let warden = test.new_actor().await;
    let warden2 = test.new_actor().await;
    let nullifier_accounts = test.nullifier_accounts(0).await;
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;

    let fee = test.genesis_fee().await;
    let mut request = send_request(0);
    compute_fee_rec_lamports::<SendQuadraVKey, _>(&mut request.public_inputs, &fee);

    let pool = PoolAccount::find(None).0;
    let fee_collector = FeeCollectorAccount::find(None).0;
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
            SendQuadraVKey::VKEY_ID,
            [0, 1],
            ProofRequest::Send(request.public_inputs.clone()),
            false,
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(nullifier_duplicate_account),
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
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;

    let fee = test.genesis_fee().await;
    let mut request = send_request(0);
    compute_fee_rec_lamports::<SendQuadraVKey, _>(&mut request.public_inputs, &fee);

    let pool = PoolAccount::find(None).0;
    let fee_collector = FeeCollectorAccount::find(None).0;
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
            SendQuadraVKey::VKEY_ID,
            [0, 1],
            ProofRequest::Send(request.public_inputs.clone()),
            skip_nullifier_pda,
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(nullifier_duplicate_account),
            &user_accounts(&[nullifier_accounts[0]]),
            &[],
        )
    };

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
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;

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

    // Failure: recipient is not token account of correct mint
    /*{
        // TODO: Move this test to finalize
        let recipient = request.public_inputs.recipient.pubkey();
        test.ix_should_fail(
            init_instruction(request.public_inputs.clone(), recipient),
            &[&warden.keypair],
        ).await;
    }*/

    test.ix_should_succeed(
        ElusivInstruction::init_verification_instruction(
            0,
            SendQuadraVKey::VKEY_ID,
            [0, 1],
            ProofRequest::Send(request.public_inputs.clone()),
            false,
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(nullifier_duplicate_account),
            &user_accounts(&[nullifier_accounts[0]]),
            &[],
        ),
        &[&warden.keypair],
    ).await;

    assert_eq!(0, warden.lamports(&mut test).await);

    let subvention = fee.proof_subvention.into_token(&price, USDC_TOKEN_ID).unwrap();
    let commitment_hash_fee = fee.commitment_hash_computation_fee(0);

    let pool_account = program_token_account_address::<PoolAccount>(USDC_TOKEN_ID, None).unwrap();
    let fee_collector_account = program_token_account_address::<FeeCollectorAccount>(USDC_TOKEN_ID, None).unwrap();

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
    let nullifier_accounts = test.nullifier_accounts(0).await;
    let fee = test.genesis_fee().await;
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;

    let mut request = send_request(0);
    compute_fee_rec_lamports::<SendQuadraVKey, _>(&mut request.public_inputs, &fee);

    let pool = PoolAccount::find(None).0;
    let fee_collector = FeeCollectorAccount::find(None).0;
    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;

    let public_inputs = request.public_inputs.public_signals_skip_mr();
    let input_preparation_tx_count = prepare_public_inputs_instructions(&public_inputs, SendQuadraVKey::public_inputs_count()).len();
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
                SendQuadraVKey::VKEY_ID,
                [0, 1],
                ProofRequest::Send(request.public_inputs.clone()),
                false,
                WritableSignerAccount(warden.pubkey),
                WritableUserAccount(nullifier_duplicate_account),
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

    let extra_data = ExtraData::default();
    let recipient = Pubkey::new_from_array(extra_data.recipient);
    let identifier = Pubkey::new_from_array(extra_data.identifier);
    let iv = Pubkey::new_from_array(extra_data.iv);

    // Fill in nullifiers to test heap/compute unit limits
    {   
        let count = NULLIFIERS_PER_ACCOUNT - 1;
        let mut data = test.data(&nullifier_accounts[0]).await;
        {
            let mut map = NullifierMap::new(&mut data[1..]);
            for _ in 0..count {
                let x: [u8; 32] = rand::random();
                map.try_insert_default(OrdU256(x)).unwrap();
            }
        }
        test.set_account_rent_exempt(&nullifier_accounts[0], &data, &elusiv::id()).await;
    }

    // Finalize
    test.tx_should_succeed_simple(
        &[
            ElusivInstruction::finalize_verification_send_instruction(
                FinalizeSendData {
                    timestamp: 0,
                    total_amount: request.public_inputs.join_split.total_amount(),
                    token_id: 0,
                    mt_index: 0,
                    commitment_index: 0,
                    encrypted_owner: extra_data.encrypted_owner,
                },
                0,
                UserAccount(recipient),
                UserAccount(identifier),
                UserAccount(iv),
            )
        ]
    ).await;

    test.tx_should_succeed_simple(
        &[
            request_compute_units(1_400_000),
            ElusivInstruction::finalize_verification_send_nullifiers_instruction(
                0,
                Some(0),
                &writable_user_accounts(&[nullifier_accounts[0]]),
                Some(1),
                &[],
            )
        ]
    ).await;

    // IMPORTANT: Pool already contains subvention (so we airdrop commitment_hash_fee - subvention)
    test.airdrop_lamports(
        &pool,
        request.public_inputs.join_split.amount + commitment_hash_fee.0 - subvention.0 + proof_verification_fee.0 + network_fee.0
    ).await;

    test.ix_should_succeed_simple(
        ElusivInstruction::finalize_verification_transfer_lamports_instruction(
            0,
            WritableUserAccount(recipient),
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
        test.lamports(&recipient).await.0
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
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;

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
    let extra_data = ExtraData {
        recipient: recipient_token_account.to_bytes(),
        ..Default::default()
    };
    request.public_inputs.extra_data_hash = extra_data.hash();

    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;

    let public_inputs = request.public_inputs.public_signals_skip_mr();
    let input_preparation_tx_count = prepare_public_inputs_instructions(&public_inputs, SendQuadraVKey::public_inputs_count()).len();
    let subvention = fee.proof_subvention.into_token(&price, USDC_TOKEN_ID).unwrap();
    let proof_verification_fee = fee.proof_verification_computation_fee(input_preparation_tx_count).into_token(&price, USDC_TOKEN_ID).unwrap();
    let commitment_hash_fee = fee.commitment_hash_computation_fee(0);
    let commitment_hash_fee_token = commitment_hash_fee.into_token(&price, USDC_TOKEN_ID).unwrap();
    let network_fee = Token::new(USDC_TOKEN_ID, fee.proof_network_fee.calc(request.public_inputs.join_split.amount));
    let verification_account_rent = test.rent(VerificationAccount::SIZE).await;
    let nullifier_duplicate_account_rent = test.rent(PDAAccountData::SIZE).await;

    let pool_account = program_token_account_address::<PoolAccount>(USDC_TOKEN_ID, None).unwrap();
    let fee_collector_account = program_token_account_address::<FeeCollectorAccount>(USDC_TOKEN_ID, None).unwrap();

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
                SendQuadraVKey::VKEY_ID,
                [0, 1],
                ProofRequest::Send(request.public_inputs.clone()),
                false,
                WritableSignerAccount(warden.pubkey),
                WritableUserAccount(nullifier_duplicate_account),
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

    let identifier = Pubkey::new_from_array(extra_data.identifier);
    let iv = Pubkey::new_from_array(extra_data.iv);

    // Finalize
    test.ix_should_succeed_simple(
        ElusivInstruction::finalize_verification_send_instruction(
            FinalizeSendData {
                timestamp: 0,
                total_amount: request.public_inputs.join_split.total_amount(),
                token_id: USDC_TOKEN_ID,
                mt_index: 0,
                commitment_index: 0,
                encrypted_owner: extra_data.encrypted_owner,
            },
            0,
            UserAccount(recipient_token_account),
            UserAccount(identifier),
            UserAccount(iv),
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
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;
    let warden = test.new_actor().await;
    let recipient = test.new_actor().await;
    let nullifier_accounts = test.nullifier_accounts(0).await;
    let pool = PoolAccount::find(None).0;
    let fee_collector = FeeCollectorAccount::find(None).0;
    let mut request = send_request(0);
    let extra_data = ExtraData {
        recipient: recipient.pubkey.to_bytes(),
        ..Default::default()
    };
    request.public_inputs.extra_data_hash = extra_data.hash();
    compute_fee_rec_lamports::<SendQuadraVKey, _>(&mut request.public_inputs, &test.genesis_fee().await);
    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;
    let identifier = Pubkey::new_from_array(extra_data.identifier);
    let iv = Pubkey::new_from_array(extra_data.iv);

    warden.airdrop(LAMPORTS_TOKEN_ID, LAMPORTS_PER_SOL, &mut test).await;
    test.airdrop_lamports(&fee_collector, LAMPORTS_PER_SOL).await;
    test.airdrop_lamports(&pool, LAMPORTS_PER_SOL * 1000).await;

    let init_instructions = |v_index: u32, skip_nullifier_pda: bool| {
        [
            ElusivInstruction::init_verification_instruction(
                v_index,
                SendQuadraVKey::VKEY_ID,
                [0, 1],
                ProofRequest::Send(request.public_inputs.clone()),
                skip_nullifier_pda,
                WritableSignerAccount(warden.pubkey),
                WritableUserAccount(nullifier_duplicate_account),
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
                    encrypted_owner: extra_data.encrypted_owner,
                },
                v_index,
                UserAccount(recipient.pubkey),
                UserAccount(identifier),
                UserAccount(iv),
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
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;

    let mut warden = test.new_actor().await;
    warden.open_token_account(USDC_TOKEN_ID, 0, &mut test).await;

    let sol_usd_price = Price { price: 41, conf: 0, expo: 0};
    let usdc_usd_price = Price { price: 1, conf: 0, expo: 0 };
    let price = TokenPrice::new_from_sol_price(sol_usd_price, usdc_usd_price, USDC_TOKEN_ID).unwrap();
    test.set_token_to_usd_price_pyth(0, sol_usd_price).await;
    test.set_token_to_usd_price_pyth(USDC_TOKEN_ID, usdc_usd_price).await;

    let mut request = send_request(0);
    let recipient = test.new_actor().await;
    let extra_data = ExtraData {
        recipient: recipient.pubkey.to_bytes(),
        ..Default::default()
    };
    request.public_inputs.recipient_is_associated_token_account = true;
    request.public_inputs.extra_data_hash = extra_data.hash();
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

    let pool_account = program_token_account_address::<PoolAccount>(USDC_TOKEN_ID, None).unwrap();
    let fee_collector_account = program_token_account_address::<FeeCollectorAccount>(USDC_TOKEN_ID, None).unwrap();
    test.airdrop(&fee_collector_account, subvention).await;

    test.ix_should_succeed(
        ElusivInstruction::init_verification_instruction(
            0,
            SendQuadraVKey::VKEY_ID,
            [0, 1],
            ProofRequest::Send(request.clone().public_inputs),
            false,
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(nullifier_duplicate_account),
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
    skip_finalize_verification_send(0, &recipient.pubkey, &mut test).await;
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

#[tokio::test]
async fn test_compute_proof_verifcation_instruction_uniformity() {
    let mut test = start_verification_test().await;
    let (_, vkey_sub_account) = setup_vkey_account::<SendQuadraVKey>(&mut test).await;
    let warden = test.new_actor().await;
    let nullifier_accounts = test.nullifier_accounts(0).await;
    let fee = test.genesis_fee().await;
    let mut request = send_request(0);
    compute_fee_rec::<SendQuadraVKey, _>(&mut request.public_inputs, &fee, &TokenPrice::new_lamports());

    let fee_collector = FeeCollectorAccount::find(None).0;
    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;

    let public_inputs = request.public_inputs.public_signals_skip_mr();
    let input_preparation_tx_count = prepare_public_inputs_instructions(&public_inputs, SendQuadraVKey::public_inputs_count()).len();
    let subvention = fee.proof_subvention;
    let commitment_hash_fee = fee.commitment_hash_computation_fee(0);
    let verification_account_rent = test.rent(VerificationAccount::SIZE).await;
    let nullifier_duplicate_account_rent = test.rent(PDAAccountData::SIZE).await;

    warden.airdrop(
        LAMPORTS_TOKEN_ID,
        verification_account_rent.0 + nullifier_duplicate_account_rent.0 + commitment_hash_fee.0,
        &mut test,
    ).await;
    test.airdrop_lamports(&fee_collector, subvention.0).await;

    test.tx_should_succeed(
        &[
            ElusivInstruction::init_verification_instruction(
                0,
                SendQuadraVKey::VKEY_ID,
                [0, 1],
                ProofRequest::Send(request.public_inputs.clone()),
                false,
                WritableSignerAccount(warden.pubkey),
                WritableUserAccount(nullifier_duplicate_account),
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

    // Both input preparation and combined miller loop work with 4 instructions (but input preparation only uses one)
    for _ in 0..input_preparation_tx_count + CombinedMillerLoop::TX_COUNT {
        test.tx_should_succeed_simple(
            &[
                request_compute_units(1_400_000),
                ElusivInstruction::compute_verification_instruction(0, SendQuadraVKey::VKEY_ID, &[UserAccount(vkey_sub_account)]),
                ElusivInstruction::compute_verification_instruction(0, SendQuadraVKey::VKEY_ID, &[UserAccount(vkey_sub_account)]),
                ElusivInstruction::compute_verification_instruction(0, SendQuadraVKey::VKEY_ID, &[UserAccount(vkey_sub_account)]),
                ElusivInstruction::compute_verification_instruction(0, SendQuadraVKey::VKEY_ID, &[UserAccount(vkey_sub_account)]),
            ]
        ).await;
    }
}