//! Tests the proof verification

mod common;

use borsh::{BorshDeserialize, BorshSerialize};
use common::*;
use elusiv::bytes::{BorshSerDeSized, ElusivOption};
use elusiv::fields::{u64_to_u256, u64_to_u256_skip_mr};
use elusiv::instruction::{
    ElusivInstruction, SignerAccount, UserAccount, WritableSignerAccount, WritableUserAccount,
};
use elusiv::processor::{program_token_account_address, FinalizeSendData, ProofRequest};
use elusiv::proof::verifier::{
    prepare_public_inputs_instructions, proof_from_str, CombinedMillerLoop, FinalExponentiation,
    VerificationStep,
};
use elusiv::proof::vkey::{SendQuadraVKey, VerifyingKeyInfo};
use elusiv::state::commitment::CommitmentQueue;
use elusiv::state::fee::ProgramFee;
use elusiv::state::governor::{FeeCollectorAccount, PoolAccount};
use elusiv::state::metadata::{CommitmentMetadata, MetadataQueue};
use elusiv::state::nullifier::{NullifierAccount, NullifierMap, NULLIFIERS_PER_ACCOUNT};
use elusiv::state::program_account::{PDAAccount, PDAAccountData, ProgramAccount, SizedAccount};
use elusiv::state::proof::{VerificationAccount, VerificationState};
use elusiv::state::queue::RingQueue;
use elusiv::state::storage::{empty_root_raw, StorageAccount, MT_HEIGHT};
use elusiv::state::vkey::{VKeyAccount, VKeyAccountEager};
use elusiv::token::{
    spl_token_account_data, Lamports, Token, TokenPrice, LAMPORTS_TOKEN_ID, TOKENS, USDC_TOKEN_ID,
    USDT_TOKEN_ID,
};
use elusiv::types::{
    compute_fee_rec, compute_fee_rec_lamports, generate_hashed_inputs, InputCommitment,
    JoinSplitPublicInputs, OptionalFee, OrdU256, Proof, PublicInputs, RawProof, RawU256,
    SendPublicInputs, JOIN_SPLIT_MAX_N_ARITY, U256,
};
use elusiv_computation::PartialComputation;
use elusiv_types::tokens::Price;
use elusiv_types::ParentAccount;
use elusiv_utils::two_pow;
use solana_program::instruction::Instruction;
use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_program::{system_instruction, system_program};
use solana_program_test::*;
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use spl_associated_token_account::get_associated_token_address;

async fn start_verification_test() -> ElusivProgramTest {
    let mut test = start_test_with_setup().await;

    setup_storage_account(&mut test).await;
    create_merkle_tree(&mut test, 0).await;
    create_merkle_tree(&mut test, 1).await;

    test
}

#[derive(Clone)]
struct FullSendRequest {
    proof: Proof,
    public_inputs: SendPublicInputs,
}

impl FullSendRequest {
    fn update_fee_lamports(&mut self, fee: &ProgramFee) {
        compute_fee_rec_lamports::<SendQuadraVKey, _>(&mut self.public_inputs, fee);
    }

    fn update_fee_token(&mut self, fee: &ProgramFee, price: &TokenPrice) {
        compute_fee_rec::<SendQuadraVKey, _>(&mut self.public_inputs, fee, price)
    }
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

    let default_hashed_inputs = ExtraData::default().hash();

    let requests = vec![
        FullSendRequest {
            proof,
            public_inputs: SendPublicInputs {
                join_split: JoinSplitPublicInputs {
                    input_commitments: vec![
                        InputCommitment {
                            root: Some(empty_root_raw()),
                            nullifier_hash: RawU256::new(u256_from_str_skip_mr("10026859857882131638516328056627849627085232677511724829502598764489185541935")),
                        }
                    ],
                    output_commitment: RawU256::new(u256_from_str_skip_mr("685960310506634721912121951341598678325833230508240750559904196809564625591")),
                    recent_commitment_index: 0,
                    fee_version: 0,
                    amount: LAMPORTS_PER_SOL * 123,
                    fee: 0,
                    optional_fee: OptionalFee::default(),
                    token_id: 0,
                    metadata: CommitmentMetadata::default(),
                },
                recipient_is_associated_token_account: false,
                hashed_inputs: default_hashed_inputs,
                solana_pay_transfer: false,
            }
        },
        FullSendRequest {
            proof,
            public_inputs: SendPublicInputs {
                join_split: JoinSplitPublicInputs {
                    input_commitments: vec![
                        InputCommitment {
                            root: Some(empty_root_raw()),
                            nullifier_hash: RawU256::new(u256_from_str_skip_mr("10026859857882131638516328056627849627085232677511724829502598764489185541935")),
                        },
                        InputCommitment {
                            root: None,
                            nullifier_hash: RawU256::new(u256_from_str_skip_mr("13921430393547588871192356721184227660578793579443975701453971046059378311483")),
                        },
                    ],
                    output_commitment: RawU256::new(u256_from_str_skip_mr("685960310506634721912121951341598678325833230508240750559904196809564625591")),
                    recent_commitment_index: 0,
                    fee_version: 0,
                    amount: LAMPORTS_PER_SOL * 123,
                    fee: 0,
                    optional_fee: OptionalFee::default(),
                    token_id: 0,
                    metadata: CommitmentMetadata::default(),
                },
                recipient_is_associated_token_account: false,
                hashed_inputs: default_hashed_inputs,
                solana_pay_transfer: false,
            }
        },
        FullSendRequest {
            proof,
            public_inputs: SendPublicInputs {
                join_split: JoinSplitPublicInputs {
                    input_commitments: vec![
                        InputCommitment {
                            root: Some(empty_root_raw()),
                            nullifier_hash: RawU256::new(u256_from_str_skip_mr("10026859857882131638516328056627849627085232677511724829502598764489185541935")),
                        },
                        InputCommitment {
                            root: Some(empty_root_raw()),
                            nullifier_hash: RawU256::new(u256_from_str_skip_mr("19685960310506634721912121951341598678325833230508240750559904196809564625591")),
                        },
                    ],
                    output_commitment: RawU256::new(u256_from_str_skip_mr("685960310506634721912121951341598678325833230508240750559904196809564625591")),
                    recent_commitment_index: 0,
                    fee_version: 0,
                    amount: LAMPORTS_PER_SOL * 123,
                    fee: 0,
                    optional_fee: OptionalFee::default(),
                    token_id: 0,
                    metadata: CommitmentMetadata::default(),
                },
                recipient_is_associated_token_account: false,
                hashed_inputs: default_hashed_inputs,
                solana_pay_transfer: false,
            }
        },
        FullSendRequest {
            proof,
            public_inputs: SendPublicInputs {
                join_split: JoinSplitPublicInputs {
                    input_commitments: vec![
                        InputCommitment {
                            root: Some(empty_root_raw()),
                            nullifier_hash: RawU256::new(u256_from_str_skip_mr("10026859857882131638516328056627849627085232677511724829502598764489185541935")),
                        },
                        InputCommitment {
                            root: None,
                            nullifier_hash: RawU256::new(u256_from_str_skip_mr("19685960310506634721912121951341598678325833230508240750559904196809564625591")),
                        },
                        InputCommitment {
                            root: None,
                            nullifier_hash: RawU256::new(u256_from_str_skip_mr("168596031050663472212195134159867832583323058240750559904196809564625591")),
                        },
                    ],
                    output_commitment: RawU256::new(u256_from_str_skip_mr("685960310506634721912121951341598678325833230508240750559904196809564625591")),
                    recent_commitment_index: 0,
                    fee_version: 0,
                    amount: LAMPORTS_PER_SOL * 123,
                    fee: 0,
                    optional_fee: OptionalFee::default(),
                    token_id: 0,
                    metadata: CommitmentMetadata::default(),
                },
                recipient_is_associated_token_account: false,
                hashed_inputs: default_hashed_inputs,
                solana_pay_transfer: false,
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
    reference: U256,
    is_associated_token_account: bool,
    metadata: CommitmentMetadata,
    optional_fee: OptionalFee,
    memo: Option<Vec<u8>>,
}

impl Default for ExtraData {
    fn default() -> Self {
        ExtraData {
            recipient: u256_from_str_skip_mr(
                "115792089237316195423570985008687907853269984665640564039457584007913129639935",
            ),
            identifier: u256_from_str_skip_mr("1"),
            iv: u256_from_str_skip_mr("5683487854789"),
            encrypted_owner: u256_from_str_skip_mr("5789489458548458945478235642378"),
            reference: [0; 32],
            is_associated_token_account: false,
            metadata: CommitmentMetadata::default(),
            optional_fee: OptionalFee::default(),
            memo: None,
        }
    }
}

impl ExtraData {
    fn hash(&self) -> U256 {
        generate_hashed_inputs(
            &self.recipient,
            &self.identifier,
            &self.iv,
            &self.encrypted_owner,
            &self.reference,
            self.is_associated_token_account,
            &self.metadata,
            &self.optional_fee,
            &self.memo,
        )
    }

    fn recipient(&self) -> Pubkey {
        Pubkey::new_from_array(self.recipient)
    }

    fn identifier(&self) -> Pubkey {
        Pubkey::new_from_array(self.identifier)
    }

    fn reference(&self) -> Pubkey {
        Pubkey::new_from_array(self.reference)
    }
}

async fn init_verification_simple(
    proof: &Proof,
    public_inputs: &SendPublicInputs,
    identifier: U256,
    test: &mut ElusivProgramTest,
) {
    let nullifier_accounts = nullifier_accounts(test, 0).await;

    test.tx_should_succeed_simple(&[
        ElusivInstruction::init_verification_instruction(
            0,
            SendQuadraVKey::VKEY_ID,
            [0, 1],
            ProofRequest::Send(public_inputs.clone()),
            false,
            WritableSignerAccount(test.payer()),
            WritableUserAccount(public_inputs.join_split.nullifier_duplicate_pda().0),
            UserAccount(Pubkey::new_from_array(identifier)),
            &user_accounts(&[nullifier_accounts[0]]),
            &[],
        ),
        ElusivInstruction::init_verification_transfer_fee_sol_instruction(0, test.payer()),
        ElusivInstruction::init_verification_proof_instruction(
            0,
            *proof,
            SignerAccount(test.payer()),
        ),
    ])
    .await
}

async fn skip_computation(
    warden_pubkey: Pubkey,
    verification_account_index: u32,
    success: bool,
    test: &mut ElusivProgramTest,
) {
    test.set_pda_account::<VerificationAccount, _>(
        &elusiv::id(),
        Some(warden_pubkey),
        Some(verification_account_index),
        |data| {
            let mut verification_account = VerificationAccount::new(data).unwrap();
            verification_account.set_is_verified(&ElusivOption::Some(success));
        },
    )
    .await;
}

async fn set_verification_state(
    warden_pubkey: Pubkey,
    verification_account_index: u32,
    state: VerificationState,
    test: &mut ElusivProgramTest,
) {
    test.set_pda_account::<VerificationAccount, _>(
        &elusiv::id(),
        Some(warden_pubkey),
        Some(verification_account_index),
        |data| {
            let mut verification_account = VerificationAccount::new(data).unwrap();
            verification_account.set_state(&state);
        },
    )
    .await;
}

async fn setup_vkey_account<VKey: VerifyingKeyInfo>(
    test: &mut ElusivProgramTest,
) -> (Pubkey, Pubkey) {
    let sub_account_pubkey = Pubkey::new_unique();
    let mut data = VKey::verifying_key_source();
    data.insert(0, 1);
    test.set_account_rent_exempt(&sub_account_pubkey, &data, &elusiv::id())
        .await;

    let (pda, bump) = VKeyAccount::find(Some(VKey::VKEY_ID));
    let data = VKeyAccountEager {
        pda_data: PDAAccountData {
            bump_seed: bump,
            version: 0,
        },
        pubkeys: [Some(sub_account_pubkey).into(), None.into()],
        public_inputs_count: VKey::PUBLIC_INPUTS_COUNT,
        is_frozen: true,
        authority: ElusivOption::None,
        version: 1,
    }
    .try_to_vec()
    .unwrap();
    test.set_program_account_rent_exempt(&elusiv::id(), &pda, &data)
        .await;

    (pda, sub_account_pubkey)
}

async fn insert_nullifier_hashes(
    test: &mut ElusivProgramTest,
    mt_index: u32,
    nullifier_hashes: &[U256],
) {
    let count = nullifier_hashes.len();
    let mut nullifier_hashes: Vec<_> = nullifier_hashes.iter().map(|n| OrdU256(*n)).collect();
    nullifier_hashes.sort();

    // Get all NullifierAccount child account data
    let nullifier_accounts = nullifier_accounts(test, mt_index).await;
    let mut nullifier_accounts_data = Vec::with_capacity(NullifierAccount::COUNT);
    for nullifier_account in nullifier_accounts.iter() {
        nullifier_accounts_data.push(test.data(nullifier_account).await);
    }

    {
        // Modify the child accounts locally
        let mut maps: Vec<_> = nullifier_accounts_data
            .iter_mut()
            .map(|data| NullifierMap::new(&mut data[1..]))
            .collect();

        while let Some(nullifier_hash) = nullifier_hashes.pop() {
            for map in maps.iter_mut() {
                if map.is_full() {
                    continue;
                }

                assert!(map.try_insert_default(nullifier_hash).unwrap().is_none());
                break;
            }
        }

        // Update the NullifierAccount parent account data
        let mut data = test.data(&NullifierAccount::find(Some(mt_index)).0).await;
        let mut nullifier_account = NullifierAccount::new(&mut data).unwrap();
        for (i, map) in maps.iter_mut().enumerate() {
            if !map.is_empty() {
                nullifier_account.set_max_values(i, &Some(map.max().0).into());
            }
        }
        let nullifier_count = nullifier_account.get_nullifier_hash_count();
        nullifier_account.set_nullifier_hash_count(&(nullifier_count + count as u32));

        test.set_program_account_rent_exempt(
            &elusiv::id(),
            &NullifierAccount::find(Some(mt_index)).0,
            &data,
        )
        .await;
    }

    // Update all of the NullifierAccount child accounts
    for (i, data) in nullifier_accounts_data.iter().enumerate() {
        test.set_account_rent_exempt(&nullifier_accounts[i], data, &elusiv::id())
            .await;
    }
}

fn merge(a: &[Instruction], b: &[&Instruction]) -> Vec<Instruction> {
    let mut a = a.to_vec();
    a.extend(b.iter().map(|&ix| ix.clone()).collect::<Vec<Instruction>>());
    a
}

#[tokio::test]
async fn test_init_proof_signers() {
    let mut test = start_verification_test().await;
    let warden = test.new_actor().await;
    let warden2 = test.new_actor().await;
    let nullifier_accounts = nullifier_accounts(&mut test, 0).await;
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;

    let fee = genesis_fee(&mut test).await;
    let mut request = send_request(0);
    request.update_fee_lamports(&fee);

    let pool = PoolAccount::find(None).0;
    let fee_collector = FeeCollectorAccount::find(None).0;
    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;
    let subvention = fee.proof_subvention;
    let commitment_hash_fee = fee.commitment_hash_computation_fee(0);

    let verification_account_rent = test.rent(VerificationAccount::SIZE).await;
    let nullifier_duplicate_account_rent = test.rent(PDAAccountData::SIZE).await;
    warden
        .airdrop(
            LAMPORTS_TOKEN_ID,
            verification_account_rent.0 + nullifier_duplicate_account_rent.0,
            &mut test,
        )
        .await;
    warden
        .airdrop(LAMPORTS_TOKEN_ID, commitment_hash_fee.0, &mut test)
        .await;
    warden2
        .airdrop(LAMPORTS_TOKEN_ID, commitment_hash_fee.0, &mut test)
        .await;
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
            UserAccount(Pubkey::new_unique()),
            &user_accounts(&[nullifier_accounts[0]]),
            &[],
        ),
        &[&warden.keypair],
    )
    .await;

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
    )
    .await;

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
    )
    .await;

    // Invalid signer calls `init_verification_proof`
    test.ix_should_fail(
        ElusivInstruction::init_verification_proof_instruction(
            0,
            RawProof::try_from_slice(&vec![0; RawProof::SIZE])
                .unwrap()
                .try_into()
                .unwrap(),
            SignerAccount(warden2.pubkey),
        ),
        &[&warden2.keypair],
    )
    .await;

    test.ix_should_succeed(
        ElusivInstruction::init_verification_proof_instruction(
            0,
            RawProof::try_from_slice(&vec![0; RawProof::SIZE])
                .unwrap()
                .try_into()
                .unwrap(),
            SignerAccount(warden.pubkey),
        ),
        &[&warden.keypair],
    )
    .await;
}

#[tokio::test]
async fn test_init_proof_lamports() {
    let mut test = start_verification_test().await;
    let warden = test.new_actor().await;
    let nullifier_accounts = nullifier_accounts(&mut test, 0).await;
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;

    let fee = genesis_fee(&mut test).await;
    let mut request = send_request(0);
    request.update_fee_lamports(&fee);

    let pool = PoolAccount::find(None).0;
    let fee_collector = FeeCollectorAccount::find(None).0;
    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;

    let verification_account_rent = test.rent(VerificationAccount::SIZE).await;
    let nullifier_duplicate_account_rent = test.rent(PDAAccountData::SIZE).await;
    warden
        .airdrop(
            LAMPORTS_TOKEN_ID,
            verification_account_rent.0 + nullifier_duplicate_account_rent.0,
            &mut test,
        )
        .await;

    let init_verification_instruction =
        |v_index: u8, commitment: Option<U256>, skip_nullifier_pda: bool| {
            let mut request = request.clone();
            if let Some(commitment) = commitment {
                request.public_inputs.join_split.output_commitment = RawU256::new(commitment);
                request.update_fee_lamports(&fee);
            }

            ElusivInstruction::init_verification_instruction(
                v_index,
                SendQuadraVKey::VKEY_ID,
                [0, 1],
                ProofRequest::Send(request.public_inputs),
                skip_nullifier_pda,
                WritableSignerAccount(warden.pubkey),
                WritableUserAccount(nullifier_duplicate_account),
                UserAccount(Pubkey::new_unique()),
                &user_accounts(&[nullifier_accounts[0]]),
                &[],
            )
        };

    // Failure if skip_nullifier_pda := true (and nullifier_pda does not exist)
    test.ix_should_fail(
        init_verification_instruction(0, None, true),
        &[&warden.keypair],
    )
    .await;

    test.ix_should_succeed(
        init_verification_instruction(0, None, false),
        &[&warden.keypair],
    )
    .await;

    assert_eq!(0, warden.lamports(&mut test).await);

    warden
        .airdrop(LAMPORTS_TOKEN_ID, verification_account_rent.0, &mut test)
        .await;

    // Testing duplicate verifications (allowed when flag is set)
    // Failure if skip_nullifier_pda := false (and nullifier_pda exists)
    test.ix_should_fail(
        init_verification_instruction(1, None, false),
        &[&warden.keypair],
    )
    .await;

    // If skip_nullifier_pda := true (and nullifier_pda exists) will fail due to duplicate commitment
    test.ix_should_fail(
        init_verification_instruction(1, None, true),
        &[&warden.keypair],
    )
    .await;

    // Success for a different commitment with skip_nullifier_pda := true
    test.ix_should_succeed(
        init_verification_instruction(1, Some(u256_from_str("1234")), true),
        &[&warden.keypair],
    )
    .await;

    assert_eq!(0, warden.lamports(&mut test).await);

    let subvention = fee.proof_subvention;
    let commitment_hash_fee = fee.commitment_hash_computation_fee(0);

    warden
        .airdrop(LAMPORTS_TOKEN_ID, commitment_hash_fee.0, &mut test)
        .await;
    test.airdrop_lamports(&fee_collector, subvention.0).await;

    // Failure because of invalid amount (recipient always has to exist)
    let rent_exemption = test.rent(0).await;
    test.set_pda_account::<VerificationAccount, _>(
        &elusiv::id(),
        Some(warden.pubkey),
        Some(0),
        |data| {
            let mut verification_account = VerificationAccount::new(data).unwrap();
            let mut request = verification_account.get_request();
            if let ProofRequest::Send(public_inputs) = &mut request {
                public_inputs.join_split.amount = rent_exemption.0 - 1;
            }
            verification_account.set_request(&request);
        },
    )
    .await;

    let transfer_fee_instruction = ElusivInstruction::init_verification_transfer_fee_instruction(
        0,
        WritableSignerAccount(warden.pubkey),
        WritableUserAccount(warden.pubkey),
        WritableUserAccount(pool),
        WritableUserAccount(fee_collector),
        UserAccount(system_program::id()),
        UserAccount(system_program::id()),
        UserAccount(system_program::id()),
    );

    test.ix_should_fail(transfer_fee_instruction.clone(), &[&warden.keypair])
        .await;

    // Reset request
    test.set_pda_account::<VerificationAccount, _>(
        &elusiv::id(),
        Some(warden.pubkey),
        Some(0),
        |data| {
            let mut verification_account = VerificationAccount::new(data).unwrap();
            verification_account.set_request(&ProofRequest::Send(request.public_inputs));
        },
    )
    .await;

    test.ix_should_succeed(transfer_fee_instruction, &[&warden.keypair])
        .await;

    assert_eq!(0, warden.lamports(&mut test).await);
    assert_eq!(
        0,
        test.pda_lamports(&fee_collector, FeeCollectorAccount::SIZE)
            .await
            .0
    );
    assert_eq!(
        commitment_hash_fee.0 + subvention.0,
        test.pda_lamports(&pool, PoolAccount::SIZE).await.0
    );
}

#[tokio::test]
async fn test_init_proof_token() {
    let mut test = start_verification_test().await;
    test.create_spl_token(USDC_TOKEN_ID).await;
    enable_program_token_account::<PoolAccount>(&mut test, USDC_TOKEN_ID, None).await;
    enable_program_token_account::<FeeCollectorAccount>(&mut test, USDC_TOKEN_ID, None).await;
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;

    let mut warden = test.new_actor().await;
    warden.open_token_account(USDC_TOKEN_ID, 0, &mut test).await;

    let fee = genesis_fee(&mut test).await;
    let sol_usd_price = Price {
        price: 41,
        conf: 0,
        expo: 0,
    };
    let usdc_usd_price = Price {
        price: 1,
        conf: 0,
        expo: 0,
    };
    let price =
        TokenPrice::new_from_sol_price(sol_usd_price, usdc_usd_price, USDC_TOKEN_ID).unwrap();
    let sol_price_account = test.token_to_usd_price_pyth_account(0);
    let token_price_account = test.token_to_usd_price_pyth_account(USDC_TOKEN_ID);
    test.set_token_to_usd_price_pyth(0, sol_usd_price).await;
    test.set_token_to_usd_price_pyth(USDC_TOKEN_ID, usdc_usd_price)
        .await;

    let mut request = send_request(0);
    request.public_inputs.join_split.token_id = USDC_TOKEN_ID;
    request.public_inputs.join_split.amount = 1_000_000;
    request.update_fee_token(&fee, &price);

    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;
    let nullifier_accounts = nullifier_accounts(&mut test, 0).await;

    let verification_account_rent = test.rent(VerificationAccount::SIZE).await;
    let nullifier_duplicate_account_rent = test.rent(PDAAccountData::SIZE).await;
    warden
        .airdrop(
            LAMPORTS_TOKEN_ID,
            verification_account_rent.0 + nullifier_duplicate_account_rent.0,
            &mut test,
        )
        .await;

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
            UserAccount(Pubkey::new_unique()),
            &user_accounts(&[nullifier_accounts[0]]),
            &[],
        ),
        &[&warden.keypair],
    )
    .await;

    assert_eq!(0, warden.lamports(&mut test).await);

    let subvention = fee
        .proof_subvention
        .into_token(&price, USDC_TOKEN_ID)
        .unwrap();
    let commitment_hash_fee = fee.commitment_hash_computation_fee(0);

    let pool_account = program_token_account_address::<PoolAccount>(USDC_TOKEN_ID, None).unwrap();
    let fee_collector_account =
        program_token_account_address::<FeeCollectorAccount>(USDC_TOKEN_ID, None).unwrap();

    warden
        .airdrop(LAMPORTS_TOKEN_ID, commitment_hash_fee.0, &mut test)
        .await;
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
    )
    .await;

    assert_eq!(0, warden.lamports(&mut test).await);
    assert_eq!(0, warden.balance(USDC_TOKEN_ID, &mut test).await);
    assert_eq!(
        commitment_hash_fee.0,
        test.pda_lamports(&PoolAccount::find(None).0, PoolAccount::SIZE)
            .await
            .0
    );
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
    let optional_fee_collector = test.new_actor().await;
    let nullifier_accounts = nullifier_accounts(&mut test, 0).await;
    let fee = genesis_fee(&mut test).await;
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;

    let mut request = send_request(0);
    request.public_inputs.join_split.optional_fee = OptionalFee {
        collector: optional_fee_collector.pubkey,
        amount: 1234,
    };
    let extra_data = ExtraData {
        optional_fee: request.public_inputs.join_split.optional_fee.clone(),
        ..Default::default()
    };
    request.public_inputs.hashed_inputs = extra_data.hash();
    request.update_fee_lamports(&fee);

    let pool = PoolAccount::find(None).0;
    let fee_collector = FeeCollectorAccount::find(None).0;
    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;

    let public_inputs = request.public_inputs.public_signals_skip_mr();
    let input_preparation_tx_count =
        prepare_public_inputs_instructions(&public_inputs, SendQuadraVKey::public_inputs_count())
            .len();
    let subvention = fee.proof_subvention;
    let proof_verification_fee = fee.proof_verification_computation_fee(input_preparation_tx_count);
    let commitment_hash_fee = fee.commitment_hash_computation_fee(0);
    let network_fee = Lamports(
        fee.proof_network_fee
            .calc(request.public_inputs.join_split.amount),
    );
    let verification_account_rent = test.rent(VerificationAccount::SIZE).await;
    let nullifier_duplicate_account_rent = test.rent(PDAAccountData::SIZE).await;

    warden
        .airdrop(
            LAMPORTS_TOKEN_ID,
            verification_account_rent.0
                + nullifier_duplicate_account_rent.0
                + commitment_hash_fee.0,
            &mut test,
        )
        .await;
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
                UserAccount(Pubkey::new_from_array(extra_data.identifier)),
                &user_accounts(&[nullifier_accounts[0]]),
                &[],
            ),
            ElusivInstruction::init_verification_transfer_fee_sol_instruction(0, warden.pubkey),
            ElusivInstruction::init_verification_proof_instruction(
                0,
                request.proof,
                SignerAccount(warden.pubkey),
            ),
        ],
        &[&warden.keypair],
    )
    .await;

    assert_eq!(0, warden.lamports(&mut test).await);
    assert_eq!(
        0,
        test.pda_lamports(&fee_collector, FeeCollectorAccount::SIZE)
            .await
            .0
    );

    // Skip computation
    skip_computation(warden.pubkey, 0, true, &mut test).await;

    let recipient = Pubkey::new_from_array(extra_data.recipient);
    let identifier = Pubkey::new_from_array(extra_data.identifier);
    let reference = Pubkey::new_from_array(extra_data.reference);

    // Fill in nullifiers to test heap/compute unit limits
    insert_nullifier_hashes(
        &mut test,
        0,
        &(0..NULLIFIERS_PER_ACCOUNT as u64 - 1)
            .map(u64_to_u256)
            .collect::<Vec<_>>(),
    )
    .await;

    // Finalize
    let finalize_verification_send_instruction =
        ElusivInstruction::finalize_verification_send_instruction(
            0,
            FinalizeSendData {
                total_amount: request.public_inputs.join_split.total_amount(),
                encrypted_owner: extra_data.encrypted_owner,
                iv: extra_data.iv,
                ..Default::default()
            },
            false,
            UserAccount(recipient),
            UserAccount(identifier),
            UserAccount(reference),
            UserAccount(warden.pubkey),
        );
    let finalize_verification_send_nullifier_instruction =
        ElusivInstruction::finalize_verification_insert_nullifier_instruction(
            0,
            UserAccount(warden.pubkey),
            Some(0),
            &writable_user_accounts(&[nullifier_accounts[0]]),
        );
    let finalize_verification_transfer_lamports_instruction =
        ElusivInstruction::finalize_verification_transfer_lamports_instruction(
            0,
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(recipient),
            WritableUserAccount(optional_fee_collector.pubkey),
            WritableUserAccount(nullifier_duplicate_account),
        );

    // IMPORTANT: Pool already contains subvention (so we airdrop commitment_hash_fee - subvention)
    test.airdrop_lamports(
        &pool,
        request.public_inputs.join_split.amount + commitment_hash_fee.0 - subvention.0
            + proof_verification_fee.0
            + network_fee.0,
    )
    .await;

    // Individual instruction should fail
    test.tx_should_fail(
        &[
            request_compute_units(1_400_000),
            finalize_verification_send_instruction.clone(),
        ],
        &[&warden.keypair],
    )
    .await;

    test.tx_should_succeed(
        &[
            request_compute_units(1_400_000),
            finalize_verification_send_instruction,
            finalize_verification_send_nullifier_instruction,
            finalize_verification_transfer_lamports_instruction,
        ],
        &[&warden.keypair],
    )
    .await;

    assert!(
        test.account_does_not_exist(
            &VerificationAccount::find_with_pubkey(warden.pubkey, Some(0)).0
        )
        .await
    );
    assert!(
        test.account_does_not_exist(&nullifier_duplicate_account)
            .await
    );

    assert_eq!(
        commitment_hash_fee.0
            + proof_verification_fee.0
            + verification_account_rent.0
            + nullifier_duplicate_account_rent.0,
        warden.lamports(&mut test).await
    );

    // recipient hat amount - optional_fee.amount
    assert_eq!(
        request.public_inputs.join_split.amount
            - request.public_inputs.join_split.optional_fee.amount,
        test.lamports(&recipient).await.0
    );

    // optional_fee_collector has optional_fee.amount
    assert_eq!(
        request.public_inputs.join_split.optional_fee.amount,
        optional_fee_collector.lamports(&mut test).await
    );

    // fee_collector has network_fee (lamports)
    assert_eq!(
        network_fee.0,
        test.pda_lamports(&fee_collector, FeeCollectorAccount::SIZE)
            .await
            .0
    );

    // pool has computation_fee (lamports)
    assert_eq!(
        commitment_hash_fee.0,
        test.pda_lamports(&pool, PoolAccount::SIZE).await.0
    );

    queue!(commitment_queue, CommitmentQueue, test);
    assert_eq!(commitment_queue.len(), 1);
    assert_eq!(
        commitment_queue.view_first().unwrap().commitment,
        request.public_inputs.join_split.output_commitment.reduce()
    );

    queue!(metadata_queue, MetadataQueue, test);
    assert_eq!(metadata_queue.len(), 1);
    assert_eq!(
        metadata_queue.view_first().unwrap(),
        request.public_inputs.join_split.metadata
    );
}

#[tokio::test]
async fn test_finalize_proof_token() {
    let mut test = start_verification_test().await;
    test.create_spl_token(USDC_TOKEN_ID).await;
    enable_program_token_account::<PoolAccount>(&mut test, USDC_TOKEN_ID, None).await;
    enable_program_token_account::<FeeCollectorAccount>(&mut test, USDC_TOKEN_ID, None).await;
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;
    let nullifier_accounts = nullifier_accounts(&mut test, 0).await;
    let fee = genesis_fee(&mut test).await;

    let mut recipient = test.new_actor().await;
    recipient
        .open_token_account(USDC_TOKEN_ID, 0, &mut test)
        .await;

    let mut warden = test.new_actor().await;
    warden.open_token_account(USDC_TOKEN_ID, 0, &mut test).await;

    let mut optional_fee_collector = test.new_actor().await;
    optional_fee_collector
        .open_token_account(USDC_TOKEN_ID, 0, &mut test)
        .await;

    let sol_usd_price = Price {
        price: 41,
        conf: 0,
        expo: 0,
    };
    let usdc_usd_price = Price {
        price: 1,
        conf: 0,
        expo: 0,
    };
    let price =
        TokenPrice::new_from_sol_price(sol_usd_price, usdc_usd_price, USDC_TOKEN_ID).unwrap();
    let sol_price_account = test.token_to_usd_price_pyth_account(0);
    let token_price_account = test.token_to_usd_price_pyth_account(USDC_TOKEN_ID);
    test.set_token_to_usd_price_pyth(0, sol_usd_price).await;
    test.set_token_to_usd_price_pyth(USDC_TOKEN_ID, usdc_usd_price)
        .await;

    let mut request = send_request(0);
    request.public_inputs.join_split.optional_fee = OptionalFee {
        collector: optional_fee_collector.get_token_account(USDC_TOKEN_ID),
        amount: 1234,
    };
    request.public_inputs.join_split.token_id = USDC_TOKEN_ID;
    request.public_inputs.join_split.amount = 1_000_000;

    let recipient_token_account = recipient.get_token_account(USDC_TOKEN_ID);
    let extra_data = ExtraData {
        optional_fee: request.public_inputs.join_split.optional_fee.clone(),
        recipient: recipient_token_account.to_bytes(),
        ..Default::default()
    };
    request.public_inputs.hashed_inputs = extra_data.hash();
    request.update_fee_token(&fee, &price);

    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;

    let public_inputs = request.public_inputs.public_signals_skip_mr();
    let input_preparation_tx_count =
        prepare_public_inputs_instructions(&public_inputs, SendQuadraVKey::public_inputs_count())
            .len();
    let subvention = fee
        .proof_subvention
        .into_token(&price, USDC_TOKEN_ID)
        .unwrap();
    let proof_verification_fee = fee
        .proof_verification_computation_fee(input_preparation_tx_count)
        .into_token(&price, USDC_TOKEN_ID)
        .unwrap();
    let commitment_hash_fee = fee.commitment_hash_computation_fee(0);
    let commitment_hash_fee_token = commitment_hash_fee
        .into_token(&price, USDC_TOKEN_ID)
        .unwrap();
    let network_fee = Token::new(
        USDC_TOKEN_ID,
        fee.proof_network_fee
            .calc(request.public_inputs.join_split.amount),
    );
    let verification_account_rent = test.rent(VerificationAccount::SIZE).await;
    let nullifier_duplicate_account_rent = test.rent(PDAAccountData::SIZE).await;

    let pool_account = program_token_account_address::<PoolAccount>(USDC_TOKEN_ID, None).unwrap();
    let fee_collector_account =
        program_token_account_address::<FeeCollectorAccount>(USDC_TOKEN_ID, None).unwrap();

    warden
        .airdrop(
            LAMPORTS_TOKEN_ID,
            verification_account_rent.0
                + nullifier_duplicate_account_rent.0
                + commitment_hash_fee.0,
            &mut test,
        )
        .await;
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
                UserAccount(Pubkey::new_from_array(extra_data.identifier)),
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
    )
    .await;

    assert_eq!(0, warden.lamports(&mut test).await);
    assert_eq!(0, warden.balance(USDC_TOKEN_ID, &mut test).await);
    assert_eq!(
        commitment_hash_fee.0,
        test.pda_lamports(&PoolAccount::find(None).0, PoolAccount::SIZE)
            .await
            .0
    );
    assert_eq!(subvention.amount(), test.spl_balance(&pool_account).await);

    skip_computation(warden.pubkey, 0, true, &mut test).await;

    let identifier = Pubkey::new_from_array(extra_data.identifier);
    let reference = Pubkey::new_from_array(extra_data.reference);

    // Finalize
    let finalize_verification_send_instruction =
        ElusivInstruction::finalize_verification_send_instruction(
            0,
            FinalizeSendData {
                total_amount: request.public_inputs.join_split.total_amount(),
                token_id: USDC_TOKEN_ID,
                encrypted_owner: extra_data.encrypted_owner,
                iv: extra_data.iv,
                ..Default::default()
            },
            false,
            UserAccount(recipient_token_account),
            UserAccount(identifier),
            UserAccount(reference),
            UserAccount(warden.pubkey),
        );
    let finalize_verification_send_nullifier_instruction =
        ElusivInstruction::finalize_verification_insert_nullifier_instruction(
            0,
            UserAccount(warden.pubkey),
            Some(0),
            &writable_user_accounts(&[nullifier_accounts[0]]),
        );
    let finalize_verification_transfer_token_instruction =
        ElusivInstruction::finalize_verification_transfer_token_instruction(
            0,
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(warden.get_token_account(USDC_TOKEN_ID)),
            WritableUserAccount(recipient_token_account),
            UserAccount(recipient_token_account),
            WritableUserAccount(pool_account),
            WritableUserAccount(fee_collector_account),
            WritableUserAccount(optional_fee_collector.get_token_account(USDC_TOKEN_ID)),
            WritableUserAccount(nullifier_duplicate_account),
            UserAccount(spl_token::id()),
        );

    // IMPORTANT: Pool already contains subvention (so we airdrop commitment_hash_fee - subvention)
    test.airdrop(
        &pool_account,
        Token::new(
            USDC_TOKEN_ID,
            request.public_inputs.join_split.amount + commitment_hash_fee_token.amount()
                - subvention.amount()
                + proof_verification_fee.amount()
                + network_fee.amount(),
        ),
    )
    .await;

    // Individual instruction should fail
    test.ix_should_fail(
        finalize_verification_send_instruction.clone(),
        &[&warden.keypair],
    )
    .await;

    // Invalid signer
    test.tx_should_fail_simple(&[
        finalize_verification_send_instruction.clone(),
        finalize_verification_send_nullifier_instruction.clone(),
        finalize_verification_transfer_token_instruction.clone(),
    ])
    .await;

    // TODO: Invalid optional-fee-collector

    test.tx_should_succeed(
        &[
            finalize_verification_send_instruction,
            finalize_verification_send_nullifier_instruction,
            finalize_verification_transfer_token_instruction,
        ],
        &[&warden.keypair],
    )
    .await;

    assert!(
        test.account_does_not_exist(
            &VerificationAccount::find_with_pubkey(warden.pubkey, Some(0)).0
        )
        .await
    );
    assert!(
        test.account_does_not_exist(&nullifier_duplicate_account)
            .await
    );

    assert_eq!(
        verification_account_rent.0 + nullifier_duplicate_account_rent.0,
        warden.lamports(&mut test).await
    );

    // warden has proof_verification_fee and commitment_hash_fee (token)
    assert_eq!(
        proof_verification_fee.amount() + commitment_hash_fee_token.amount(),
        warden.balance(USDC_TOKEN_ID, &mut test).await
    );

    // recipient has amount - optional_fee.amount (token)
    assert_eq!(
        request.public_inputs.join_split.amount
            - request.public_inputs.join_split.optional_fee.amount,
        recipient.balance(USDC_TOKEN_ID, &mut test).await
    );

    // optional_fee_collector has optional_fee.amount (token)
    assert_eq!(
        request.public_inputs.join_split.optional_fee.amount,
        optional_fee_collector
            .balance(USDC_TOKEN_ID, &mut test)
            .await
    );

    // fee_collector has network_fee (token)
    assert_eq!(
        network_fee.amount(),
        test.spl_balance(&fee_collector_account).await
    );

    // Pool contains computation_fee (lamports)
    assert_eq!(
        commitment_hash_fee.0,
        test.pda_lamports(&PoolAccount::find(None).0, PoolAccount::SIZE)
            .await
            .0
    );
}

#[tokio::test]
async fn test_finalize_proof_skip_nullifier_pda() {
    let mut test = start_verification_test().await;
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;
    let warden = test.new_actor().await;
    let recipient = test.new_actor().await;
    let nullifier_accounts = nullifier_accounts(&mut test, 0).await;

    let fee = genesis_fee(&mut test).await;
    let mut request = send_request(0);
    let extra_data = ExtraData {
        recipient: recipient.pubkey.to_bytes(),
        ..Default::default()
    };
    request.public_inputs.hashed_inputs = extra_data.hash();
    request.update_fee_lamports(&fee);

    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;
    let identifier = Pubkey::new_from_array(extra_data.identifier);
    let reference = Pubkey::new_from_array(extra_data.reference);

    warden
        .airdrop(LAMPORTS_TOKEN_ID, LAMPORTS_PER_SOL, &mut test)
        .await;
    test.airdrop_lamports(&FeeCollectorAccount::find(None).0, LAMPORTS_PER_SOL)
        .await;
    test.airdrop_lamports(&PoolAccount::find(None).0, LAMPORTS_PER_SOL * 1000)
        .await;

    let init_instructions = |v_index: u8, commitment: Option<U256>, skip_nullifier_pda: bool| {
        let mut request = request.clone();
        if let Some(commitment) = commitment {
            request.public_inputs.join_split.output_commitment = RawU256::new(commitment);
            request.update_fee_lamports(&fee);
        }

        [
            ElusivInstruction::init_verification_instruction(
                v_index,
                SendQuadraVKey::VKEY_ID,
                [0, 1],
                ProofRequest::Send(request.public_inputs.clone()),
                skip_nullifier_pda,
                WritableSignerAccount(warden.pubkey),
                WritableUserAccount(nullifier_duplicate_account),
                UserAccount(Pubkey::new_from_array(extra_data.identifier)),
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
    test.tx_should_succeed(
        &init_instructions(0, Some(u256_from_str("1")), false),
        &[&warden.keypair],
    )
    .await;
    test.tx_should_succeed(
        &init_instructions(1, Some(u256_from_str("2")), true),
        &[&warden.keypair],
    )
    .await;
    test.tx_should_succeed(
        &init_instructions(2, Some(u256_from_str("3")), true),
        &[&warden.keypair],
    )
    .await;

    // Skip computations
    for (i, is_valid) in (0..3).zip([true, true, false]) {
        skip_computation(warden.pubkey, i, is_valid, &mut test).await;
    }

    let finalize = |v_index: u8, is_valid: bool| {
        let ixs = [
            ElusivInstruction::finalize_verification_send_instruction(
                v_index,
                FinalizeSendData {
                    total_amount: request.public_inputs.join_split.total_amount(),
                    encrypted_owner: extra_data.encrypted_owner,
                    iv: extra_data.iv,
                    ..Default::default()
                },
                false,
                UserAccount(recipient.pubkey),
                UserAccount(identifier),
                UserAccount(reference),
                UserAccount(warden.pubkey),
            ),
            ElusivInstruction::finalize_verification_insert_nullifier_instruction(
                v_index,
                UserAccount(warden.pubkey),
                Some(0),
                &writable_user_accounts(&[nullifier_accounts[0]]),
            ),
            ElusivInstruction::finalize_verification_transfer_lamports_instruction(
                v_index,
                WritableSignerAccount(warden.pubkey),
                WritableUserAccount(recipient.pubkey),
                WritableUserAccount(Pubkey::new_unique()),
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
    test.tx_should_succeed(&finalize(2, false), &[&warden.keypair])
        .await;

    // 2. verification is faster than 1. (will not close nullifier_duplicate_pda)
    test.tx_should_succeed(&finalize(1, true), &[&warden.keypair])
        .await;

    // 1. verification is unable to complete
    test.tx_should_fail(&finalize(0, true), &[&warden.keypair])
        .await;

    assert!(
        test.account_does_not_exist(
            &VerificationAccount::find_with_pubkey(warden.pubkey, Some(1)).0
        )
        .await
    );
    assert!(
        test.account_does_not_exist(
            &VerificationAccount::find_with_pubkey(warden.pubkey, Some(2)).0
        )
        .await
    );
    assert!(
        test.account_does_exist(&VerificationAccount::find_with_pubkey(warden.pubkey, Some(0)).0)
            .await
    );
    assert!(test.account_does_exist(&nullifier_duplicate_account).await);
}

#[tokio::test]
async fn test_finalize_proof_commitment_index() {
    let mut test = start_verification_test().await;
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;
    let warden = test.new_actor().await;
    let recipient = test.new_actor().await;
    let nullifier_accounts = nullifier_accounts(&mut test, 0).await;
    let pool = PoolAccount::find(None).0;
    let fee_collector = FeeCollectorAccount::find(None).0;

    let mut request = send_request(0);
    let extra_data = ExtraData {
        recipient: recipient.pubkey.to_bytes(),
        ..Default::default()
    };
    request.public_inputs.hashed_inputs = extra_data.hash();
    request.update_fee_lamports(&genesis_fee(&mut test).await);

    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;
    let identifier = Pubkey::new_from_array(extra_data.identifier);
    let reference = Pubkey::new_from_array(extra_data.reference);

    warden
        .airdrop(LAMPORTS_TOKEN_ID, LAMPORTS_PER_SOL, &mut test)
        .await;
    test.airdrop_lamports(&fee_collector, LAMPORTS_PER_SOL)
        .await;
    test.airdrop_lamports(&pool, LAMPORTS_PER_SOL * 1000).await;

    let init_instructions = [
        ElusivInstruction::init_verification_instruction(
            0,
            SendQuadraVKey::VKEY_ID,
            [0, 1],
            ProofRequest::Send(request.public_inputs.clone()),
            false,
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(nullifier_duplicate_account),
            UserAccount(Pubkey::new_from_array(extra_data.identifier)),
            &user_accounts(&[nullifier_accounts[0]]),
            &[],
        ),
        ElusivInstruction::init_verification_transfer_fee_sol_instruction(0, warden.pubkey),
        ElusivInstruction::init_verification_proof_instruction(
            0,
            request.proof,
            SignerAccount(warden.pubkey),
        ),
    ];

    test.tx_should_succeed(&init_instructions, &[&warden.keypair])
        .await;
    skip_computation(warden.pubkey, 0, true, &mut test).await;

    let finalize = |commitment_index: u32| {
        [
            ElusivInstruction::finalize_verification_send_instruction(
                0,
                FinalizeSendData {
                    total_amount: request.public_inputs.join_split.total_amount(),
                    token_id: 0,
                    mt_index: 0,
                    commitment_index,
                    encrypted_owner: extra_data.encrypted_owner,
                    iv: extra_data.iv,
                },
                false,
                UserAccount(recipient.pubkey),
                UserAccount(identifier),
                UserAccount(reference),
                UserAccount(warden.pubkey),
            ),
            ElusivInstruction::finalize_verification_insert_nullifier_instruction(
                0,
                UserAccount(warden.pubkey),
                Some(0),
                &writable_user_accounts(&[nullifier_accounts[0]]),
            ),
            ElusivInstruction::finalize_verification_transfer_lamports_instruction(
                0,
                WritableSignerAccount(warden.pubkey),
                WritableUserAccount(recipient.pubkey),
                WritableUserAccount(Pubkey::new_unique()),
                WritableUserAccount(nullifier_duplicate_account),
            ),
        ]
    };

    test.set_pda_account::<StorageAccount, _>(&elusiv::id(), None, None, |data| {
        let mut account = StorageAccount::new(data).unwrap();
        account.set_next_commitment_ptr(&2);
    })
    .await;

    // commitment_index too large
    let ixs = finalize(3);
    test.tx_should_fail(&ixs, &[&warden.keypair]).await;

    let mut fork = test.fork_for_instructions(&ixs).await;
    let mut fork1 = test.fork_for_instructions(&ixs).await;

    // commitment_index less
    test.tx_should_succeed(&finalize(1), &[&warden.keypair])
        .await;
    fork.tx_should_succeed(&finalize(0), &[&warden.keypair])
        .await;

    // commitment_index equal
    fork1
        .tx_should_succeed(&finalize(2), &[&warden.keypair])
        .await;
}

#[tokio::test]
async fn test_associated_token_account() {
    let mut test = start_verification_test().await;
    test.create_spl_token(USDC_TOKEN_ID).await;
    enable_program_token_account::<PoolAccount>(&mut test, USDC_TOKEN_ID, None).await;
    enable_program_token_account::<FeeCollectorAccount>(&mut test, USDC_TOKEN_ID, None).await;
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;

    let mut warden = test.new_actor().await;
    warden.open_token_account(USDC_TOKEN_ID, 0, &mut test).await;

    let fee = genesis_fee(&mut test).await;
    let sol_usd_price = Price {
        price: 41,
        conf: 0,
        expo: 0,
    };
    let usdc_usd_price = Price {
        price: 1,
        conf: 0,
        expo: 0,
    };
    let price =
        TokenPrice::new_from_sol_price(sol_usd_price, usdc_usd_price, USDC_TOKEN_ID).unwrap();
    let subvention = fee
        .proof_subvention
        .into_token(&price, USDC_TOKEN_ID)
        .unwrap();
    let commitment_hash_fee = fee.commitment_hash_computation_fee(0);
    test.set_token_to_usd_price_pyth(0, sol_usd_price).await;
    test.set_token_to_usd_price_pyth(USDC_TOKEN_ID, usdc_usd_price)
        .await;

    let mut request = send_request(0);
    let recipient = test.new_actor().await;
    let extra_data = ExtraData {
        recipient: recipient.pubkey.to_bytes(),
        is_associated_token_account: true,
        ..Default::default()
    };
    request.public_inputs.recipient_is_associated_token_account = true;
    request.public_inputs.hashed_inputs = extra_data.hash();
    request.public_inputs.join_split.token_id = USDC_TOKEN_ID;
    request.public_inputs.join_split.amount = 1_000_000;
    request.update_fee_token(&fee, &price);

    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;
    let nullifier_accounts = nullifier_accounts(&mut test, 0).await;

    let verification_account_rent = test.rent(VerificationAccount::SIZE).await;
    let nullifier_duplicate_account_rent = test.rent(PDAAccountData::SIZE).await;
    let token_account_rent = test.rent(spl_token::state::Account::LEN).await;
    let token_account_rent_token = token_account_rent
        .into_token(&price, USDC_TOKEN_ID)
        .unwrap();
    warden
        .airdrop(
            LAMPORTS_TOKEN_ID,
            commitment_hash_fee.0
                + verification_account_rent.0
                + nullifier_duplicate_account_rent.0
                + token_account_rent.0,
            &mut test,
        )
        .await;

    let pool_account = program_token_account_address::<PoolAccount>(USDC_TOKEN_ID, None).unwrap();
    let fee_collector_account =
        program_token_account_address::<FeeCollectorAccount>(USDC_TOKEN_ID, None).unwrap();
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
            UserAccount(Pubkey::new_from_array(extra_data.identifier)),
            &user_accounts(&[nullifier_accounts[0]]),
            &[],
        ),
        &[&warden.keypair],
    )
    .await;

    let transfer_ix = ElusivInstruction::init_verification_transfer_fee_token_instruction(
        0,
        USDC_TOKEN_ID,
        warden.pubkey,
        warden.get_token_account(USDC_TOKEN_ID),
        pool_account,
        fee_collector_account,
    );
    test.ix_should_succeed(transfer_ix.clone(), &[&warden.keypair])
        .await;

    assert_eq!(0, warden.lamports(&mut test).await);
    assert_eq!(
        token_account_rent.0 + commitment_hash_fee.0,
        test.pda_lamports(&PoolAccount::find(None).0, PoolAccount::SIZE)
            .await
            .0
    );

    skip_computation(warden.pubkey, 0, true, &mut test).await;
    set_verification_state(warden.pubkey, 0, VerificationState::ProofSetup, &mut test).await;

    test.airdrop(&pool_account, Token::new(USDC_TOKEN_ID, 100_000_000))
        .await;
    test.airdrop_lamports(&PoolAccount::find(None).0, 1_000_000_000)
        .await;

    let mint = TOKENS[USDC_TOKEN_ID as usize].mint;
    let associated_token_account = get_associated_token_address(&recipient.pubkey, &mint);
    let associated_token_account_invalid =
        get_associated_token_address(&recipient.pubkey, &TOKENS[USDT_TOKEN_ID as usize].mint);

    let instructions = |recipient: Pubkey, recipient_wallet: Pubkey| {
        vec![
            ElusivInstruction::finalize_verification_send_instruction(
                0,
                FinalizeSendData {
                    total_amount: request.public_inputs.join_split.total_amount(),
                    token_id: USDC_TOKEN_ID,
                    encrypted_owner: extra_data.encrypted_owner,
                    iv: extra_data.iv,
                    ..Default::default()
                },
                false,
                UserAccount(recipient_wallet),
                UserAccount(Pubkey::new_from_array(extra_data.identifier)),
                UserAccount(Pubkey::new_from_array(extra_data.reference)),
                UserAccount(warden.pubkey),
            ),
            ElusivInstruction::finalize_verification_insert_nullifier_instruction(
                0,
                UserAccount(warden.pubkey),
                Some(0),
                &writable_user_accounts(&[nullifier_accounts[0]]),
            ),
            ElusivInstruction::finalize_verification_transfer_token_instruction(
                0,
                WritableSignerAccount(warden.pubkey),
                WritableUserAccount(warden.get_token_account(USDC_TOKEN_ID)),
                WritableUserAccount(recipient),
                UserAccount(recipient_wallet),
                WritableUserAccount(pool_account),
                WritableUserAccount(fee_collector_account),
                WritableUserAccount(Pubkey::new_unique()),
                WritableUserAccount(nullifier_duplicate_account),
                UserAccount(mint),
            ),
        ]
    };

    let valid_ixs = instructions(associated_token_account, recipient.pubkey);
    let test_fork = test.fork_for_instructions(&valid_ixs).await;
    let test_fork2 = test.fork_for_instructions(&valid_ixs).await;

    // Failure: invalid signature (only original-fee-payer can finalize the verification)
    let signer = test.new_actor().await;
    test.tx_should_fail_simple(&valid_ixs).await;
    test.tx_should_fail(&valid_ixs, &[&signer.keypair]).await;

    // Failure: Invalid recipient wallet
    test.tx_should_fail(
        &instructions(associated_token_account, warden.pubkey),
        &[&warden.keypair],
    )
    .await;

    // Failure: Invalid recipient associated token account
    test.tx_should_fail(
        &instructions(associated_token_account_invalid, recipient.pubkey),
        &[&warden.keypair],
    )
    .await;

    test.tx_should_succeed(&valid_ixs, &[&warden.keypair]).await;

    // Check funds
    assert_eq!(
        request.public_inputs.join_split.amount - token_account_rent_token.amount(),
        test.spl_balance(&associated_token_account).await
    );
    assert_eq!(
        verification_account_rent.0 + nullifier_duplicate_account_rent.0,
        warden.lamports(&mut test).await
    );

    // Test failure case
    {
        let mut test = test_fork;
        skip_computation(warden.pubkey, 0, false, &mut test).await;

        let instructions = instructions(associated_token_account, recipient.pubkey);
        test.tx_should_succeed(
            &[instructions[0].clone(), instructions[2].clone()],
            &[&warden.keypair],
        )
        .await;

        // All funds should flow to fee_collector
        assert_eq!(
            subvention.amount(),
            test.spl_balance(&fee_collector_account).await
        );
        assert_eq!(
            token_account_rent.0
                + commitment_hash_fee.0
                + verification_account_rent.0
                + nullifier_duplicate_account_rent.0,
            test.pda_lamports(
                &FeeCollectorAccount::find(None).0,
                FeeCollectorAccount::SIZE
            )
            .await
            .0
        );
    }

    // Associated token account already exists
    {
        let mut test = test_fork2;

        test.set_account_rent_exempt(
            &associated_token_account,
            &spl_token_account_data(USDC_TOKEN_ID),
            &spl_token::ID,
        )
        .await;

        test.tx_should_succeed(
            &instructions(associated_token_account, recipient.pubkey),
            &[&warden.keypair],
        )
        .await;

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
async fn test_compute_proof_verifcation_invalid_proof() {
    let mut test = start_verification_test().await;
    let (_, vkey_sub_account) = setup_vkey_account::<SendQuadraVKey>(&mut test).await;
    let warden = test.new_actor().await;
    let nullifier_accounts = nullifier_accounts(&mut test, 0).await;
    let fee = genesis_fee(&mut test).await;
    let mut request = send_request(0);
    request.update_fee_token(&fee, &TokenPrice::new_lamports());

    let fee_collector = FeeCollectorAccount::find(None).0;
    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;

    let public_inputs = request.public_inputs.public_signals_skip_mr();
    let input_preparation_tx_count =
        prepare_public_inputs_instructions(&public_inputs, SendQuadraVKey::public_inputs_count())
            .len();
    let subvention = fee.proof_subvention;
    let commitment_hash_fee = fee.commitment_hash_computation_fee(0);
    let verification_account_rent = test.rent(VerificationAccount::SIZE).await;
    let nullifier_duplicate_account_rent = test.rent(PDAAccountData::SIZE).await;

    warden
        .airdrop(
            LAMPORTS_TOKEN_ID,
            verification_account_rent.0
                + nullifier_duplicate_account_rent.0
                + commitment_hash_fee.0,
            &mut test,
        )
        .await;
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
                UserAccount(Pubkey::new_unique()),
                &user_accounts(&[nullifier_accounts[0]]),
                &[],
            ),
            ElusivInstruction::init_verification_transfer_fee_sol_instruction(0, warden.pubkey),
            ElusivInstruction::init_verification_proof_instruction(
                0,
                request.proof,
                SignerAccount(warden.pubkey),
            ),
        ],
        &[&warden.keypair],
    )
    .await;

    let instructions = [
        request_compute_units(1_400_000),
        ComputeBudgetInstruction::set_compute_unit_price(0),
        ElusivInstruction::compute_verification_instruction(
            0,
            SendQuadraVKey::VKEY_ID,
            UserAccount(warden.pubkey),
            &[UserAccount(vkey_sub_account)],
        ),
        ElusivInstruction::compute_verification_instruction(
            0,
            SendQuadraVKey::VKEY_ID,
            UserAccount(warden.pubkey),
            &[UserAccount(vkey_sub_account)],
        ),
        ElusivInstruction::compute_verification_instruction(
            0,
            SendQuadraVKey::VKEY_ID,
            UserAccount(warden.pubkey),
            &[UserAccount(vkey_sub_account)],
        ),
        ElusivInstruction::compute_verification_instruction(
            0,
            SendQuadraVKey::VKEY_ID,
            UserAccount(warden.pubkey),
            &[UserAccount(vkey_sub_account)],
        ),
        ElusivInstruction::compute_verification_instruction(
            0,
            SendQuadraVKey::VKEY_ID,
            UserAccount(warden.pubkey),
            &[UserAccount(vkey_sub_account)],
        ),
    ];

    // Input preparation
    for _ in 0..input_preparation_tx_count {
        test.tx_should_succeed_simple(&instructions).await;
    }

    pda_account!(
        v_acc,
        VerificationAccount,
        Some(warden.pubkey),
        Some(0),
        test
    );
    assert_eq!(v_acc.get_is_verified().option(), None);
    assert_eq!(v_acc.get_step(), VerificationStep::CombinedMillerLoop);

    // Combined miller loop
    for _ in 0..CombinedMillerLoop::TX_COUNT {
        test.tx_should_succeed_simple(&instructions).await;
    }

    pda_account!(
        v_acc,
        VerificationAccount,
        Some(warden.pubkey),
        Some(0),
        test
    );
    assert_eq!(v_acc.get_is_verified().option(), None);
    assert_eq!(v_acc.get_step(), VerificationStep::FinalExponentiation);

    // Final exponentiation
    for _ in 0..FinalExponentiation::TX_COUNT {
        test.tx_should_succeed_simple(&instructions).await;
    }

    pda_account!(
        v_acc,
        VerificationAccount,
        Some(warden.pubkey),
        Some(0),
        test
    );
    assert_eq!(v_acc.get_is_verified().option(), Some(false));
    assert_eq!(v_acc.get_step(), VerificationStep::FinalExponentiation);
}

#[tokio::test]
async fn test_enforced_finalization_order() {
    let mut test = start_verification_test().await;
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;
    let nullifier_accounts = nullifier_accounts(&mut test, 0).await;

    let mut request = send_request(0);
    let extra_data = ExtraData::default();
    request.public_inputs.hashed_inputs = extra_data.hash();
    request.update_fee_lamports(&genesis_fee(&mut test).await);

    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;

    test.airdrop_lamports(&FeeCollectorAccount::find(None).0, LAMPORTS_PER_SOL)
        .await;
    test.airdrop_lamports(&PoolAccount::find(None).0, LAMPORTS_PER_SOL * 1000)
        .await;

    init_verification_simple(
        &request.proof,
        &request.public_inputs,
        extra_data.identifier,
        &mut test,
    )
    .await;
    skip_computation(test.payer(), 0, true, &mut test).await;

    let finalize_verification_send_instruction =
        ElusivInstruction::finalize_verification_send_instruction(
            0,
            FinalizeSendData {
                total_amount: request.public_inputs.join_split.total_amount(),
                encrypted_owner: extra_data.encrypted_owner,
                iv: extra_data.iv,
                ..Default::default()
            },
            false,
            UserAccount(extra_data.recipient()),
            UserAccount(extra_data.identifier()),
            UserAccount(extra_data.reference()),
            UserAccount(test.payer()),
        );
    let finalize_verification_send_nullifier_instruction =
        ElusivInstruction::finalize_verification_insert_nullifier_instruction(
            0,
            UserAccount(test.payer()),
            Some(0),
            &writable_user_accounts(&[nullifier_accounts[0]]),
        );
    let finalize_verification_transfer_lamports_instruction =
        ElusivInstruction::finalize_verification_transfer_lamports_instruction(
            0,
            WritableSignerAccount(test.payer()),
            WritableUserAccount(extra_data.recipient()),
            WritableUserAccount(Pubkey::new_unique()),
            WritableUserAccount(nullifier_duplicate_account),
        );

    set_verification_state(test.payer(), 0, VerificationState::ProofSetup, &mut test).await;
    test.ix_should_fail_simple(finalize_verification_send_instruction.clone())
        .await;
    test.ix_should_fail_simple(finalize_verification_send_nullifier_instruction.clone())
        .await;
    test.ix_should_fail_simple(finalize_verification_transfer_lamports_instruction.clone())
        .await;

    // TODO: add same test for SPL tokens

    set_verification_state(test.payer(), 0, VerificationState::ProofSetup, &mut test).await;
    test.tx_should_succeed_simple(&[
        finalize_verification_send_instruction,
        finalize_verification_send_nullifier_instruction,
        finalize_verification_transfer_lamports_instruction,
    ])
    .await;
}

async fn nullifier_finalization_test(number_of_start_nullifiers: u64, input_commitments_count: u8) {
    let mut test = start_verification_test().await;
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;
    let nullifier_accounts = nullifier_accounts(&mut test, 0).await;
    let pool = PoolAccount::find(None).0;
    let fee_collector = FeeCollectorAccount::find(None).0;

    insert_nullifier_hashes(
        &mut test,
        0,
        &(0..number_of_start_nullifiers)
            .map(u64_to_u256)
            .collect::<Vec<_>>(),
    )
    .await;

    let mut input_commitments: Vec<_> = (0..input_commitments_count)
        .map(|i| InputCommitment {
            root: None,
            nullifier_hash: RawU256::new(u64_to_u256_skip_mr(u64::MAX - i as u64)),
        })
        .collect();
    input_commitments[0].root = Some(empty_root_raw());

    let extra_data = ExtraData::default();
    let proof = send_request(0).proof;
    let mut public_inputs = SendPublicInputs {
        join_split: JoinSplitPublicInputs {
            input_commitments,
            output_commitment: RawU256::new(u256_from_str_skip_mr(
                "685960310506634721912121951341598678325833230508240750559904196809564625591",
            )),
            recent_commitment_index: 0,
            fee_version: 0,
            amount: LAMPORTS_PER_SOL * 123,
            fee: 0,
            optional_fee: OptionalFee::default(),
            token_id: 0,
            metadata: CommitmentMetadata::default(),
        },
        recipient_is_associated_token_account: false,
        hashed_inputs: extra_data.hash(),
        solana_pay_transfer: false,
    };
    compute_fee_rec_lamports::<SendQuadraVKey, _>(
        &mut public_inputs,
        &genesis_fee(&mut test).await,
    );
    let nullifier_duplicate_account = public_inputs.join_split.nullifier_duplicate_pda().0;
    let identifier = Pubkey::new_from_array(extra_data.identifier);
    let reference = Pubkey::new_from_array(extra_data.reference);
    let recipient = Pubkey::new_from_array(extra_data.recipient);

    test.airdrop_lamports(&fee_collector, LAMPORTS_PER_SOL)
        .await;
    test.airdrop_lamports(&pool, LAMPORTS_PER_SOL * 1000).await;

    init_verification_simple(&proof, &public_inputs, extra_data.identifier, &mut test).await;
    skip_computation(test.payer(), 0, true, &mut test).await;
    set_verification_state(test.payer(), 0, VerificationState::ProofSetup, &mut test).await;

    let mut instructions = vec![
        request_compute_units(1_400_000),
        ElusivInstruction::finalize_verification_send_instruction(
            0,
            FinalizeSendData {
                total_amount: public_inputs.join_split.total_amount(),
                encrypted_owner: extra_data.encrypted_owner,
                iv: extra_data.iv,
                ..Default::default()
            },
            false,
            UserAccount(recipient),
            UserAccount(identifier),
            UserAccount(reference),
            UserAccount(test.payer()),
        ),
    ];

    pda_account!(nullifier_account, NullifierAccount, None, Some(0), test);

    let nullifier_hashes: Vec<U256> = public_inputs
        .join_split
        .nullifier_hashes()
        .iter()
        .map(|n| n.reduce())
        .collect();

    // Insertion instructions
    for nullifier_hash in &nullifier_hashes {
        let child_account_index = nullifier_account.find_child_account_index(nullifier_hash);

        instructions.push(
            ElusivInstruction::finalize_verification_insert_nullifier_instruction(
                0,
                UserAccount(test.payer()),
                Some(0),
                &writable_user_accounts(
                    &nullifier_accounts[child_account_index..child_account_index + 1],
                ),
            ),
        );
    }

    // Movement instructions
    let number_of_movement_instructions =
        nullifier_account.number_of_movement_instructions(&nullifier_hashes);
    for i in 0..number_of_movement_instructions {
        instructions.push(
            ElusivInstruction::finalize_verification_insert_nullifier_instruction(
                0,
                UserAccount(test.payer()),
                Some(0),
                &writable_user_accounts(&[nullifier_accounts[i + 1]]),
            ),
        );
    }

    instructions.push(
        ElusivInstruction::finalize_verification_transfer_lamports_instruction(
            0,
            WritableSignerAccount(test.payer()),
            WritableUserAccount(recipient),
            WritableUserAccount(Pubkey::new_unique()),
            WritableUserAccount(nullifier_duplicate_account),
        ),
    );

    test.tx_should_succeed_simple(&instructions).await;
}

#[tokio::test]
async fn test_finalization_nullifier_insertions() {
    let max_nullifiers_count = two_pow!(MT_HEIGHT) as u64;

    for n in 1..=JOIN_SPLIT_MAX_N_ARITY as u8 {
        nullifier_finalization_test(0, n).await;
        nullifier_finalization_test(NULLIFIERS_PER_ACCOUNT as u64, n).await;
        nullifier_finalization_test(max_nullifiers_count - n as u64, n).await;
    }
}

async fn finalize_instructions(
    test: &mut ElusivProgramTest,
    request: &FullSendRequest,
    extra_data: &ExtraData,
    reference: &Pubkey,
    signer: &Pubkey,
    memo: Option<Vec<u8>>,
) -> Vec<Instruction> {
    let nullifier_accounts = nullifier_accounts(test, 0).await;

    vec![
        ElusivInstruction::finalize_verification_send_instruction(
            0,
            FinalizeSendData {
                total_amount: request.public_inputs.join_split.total_amount(),
                encrypted_owner: extra_data.encrypted_owner,
                iv: extra_data.iv,
                ..Default::default()
            },
            memo.is_some(),
            UserAccount(extra_data.recipient()),
            UserAccount(extra_data.identifier()),
            UserAccount(*reference),
            UserAccount(*signer),
        ),
        ElusivInstruction::finalize_verification_insert_nullifier_instruction(
            0,
            UserAccount(*signer),
            Some(0),
            &writable_user_accounts(&[nullifier_accounts[0]]),
        ),
        ElusivInstruction::finalize_verification_transfer_lamports_instruction(
            0,
            WritableSignerAccount(*signer),
            WritableUserAccount(extra_data.recipient()),
            WritableUserAccount(Pubkey::new_unique()),
            WritableUserAccount(request.public_inputs.join_split.nullifier_duplicate_pda().0),
        ),
    ]
}

#[tokio::test]
async fn test_isolated_memo() {
    let mut test = start_verification_test().await;
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;

    let memo = String::from("Hello World:)");
    let invalid_memo = String::from("Hello World");
    let mut request = send_request(0);
    let extra_data = ExtraData {
        memo: Some(memo.as_bytes().to_vec()),
        ..Default::default()
    };
    request.public_inputs.hashed_inputs = extra_data.hash();
    request.update_fee_lamports(&genesis_fee(&mut test).await);

    test.airdrop_lamports(&FeeCollectorAccount::find(None).0, LAMPORTS_PER_SOL)
        .await;
    test.airdrop_lamports(&PoolAccount::find(None).0, LAMPORTS_PER_SOL * 1000)
        .await;

    init_verification_simple(
        &request.proof,
        &request.public_inputs,
        extra_data.identifier,
        &mut test,
    )
    .await;
    skip_computation(test.payer(), 0, true, &mut test).await;
    set_verification_state(test.payer(), 0, VerificationState::ProofSetup, &mut test).await;

    let payer = test.payer();
    let valid_finalize_ixs = finalize_instructions(
        &mut test,
        &request,
        &extra_data,
        &extra_data.reference(),
        &payer,
        extra_data.memo.clone(),
    )
    .await;
    let valid_memo_ix = spl_memo::build_memo(memo.as_bytes(), &[]);
    let invalid_memo_ix = spl_memo::build_memo(invalid_memo.as_bytes(), &[]);

    // Invalid memo
    test.tx_should_fail_simple(&merge(&valid_finalize_ixs, &[&invalid_memo_ix]))
        .await;

    // use_memo := false
    let invalid_ixs = finalize_instructions(
        &mut test,
        &request,
        &extra_data,
        &extra_data.reference(),
        &payer,
        None,
    )
    .await;
    test.tx_should_fail_simple(&invalid_ixs).await;

    // Memo instruction missing
    test.tx_should_fail_simple(&valid_finalize_ixs).await;

    // Memo at wrong location
    test.tx_should_fail_simple(&merge(
        &valid_finalize_ixs,
        &[&valid_memo_ix, &ElusivInstruction::nop_instruction()],
    ))
    .await;

    // Invalid memo instruction
    test.tx_should_fail_simple(&merge(
        &valid_finalize_ixs,
        &[&spl_memo::build_memo(invalid_memo.as_bytes(), &[])],
    ))
    .await;

    // Success (+ allows instructions before transfer)
    test.tx_should_succeed_simple(&merge(
        &valid_finalize_ixs,
        &[&ElusivInstruction::nop_instruction(), &valid_memo_ix],
    ))
    .await;
}

#[tokio::test]
async fn test_solana_pay_lamports() {
    let mut test = start_verification_test().await;
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;

    let mut request = send_request(0);
    let extra_data = ExtraData::default();
    request.public_inputs.solana_pay_transfer = true;
    request.public_inputs.hashed_inputs = extra_data.hash();
    request.update_fee_lamports(&genesis_fee(&mut test).await);

    test.airdrop_lamports(&FeeCollectorAccount::find(None).0, LAMPORTS_PER_SOL)
        .await;
    test.airdrop_lamports(&PoolAccount::find(None).0, LAMPORTS_PER_SOL * 1000)
        .await;

    init_verification_simple(
        &request.proof,
        &request.public_inputs,
        extra_data.identifier,
        &mut test,
    )
    .await;
    skip_computation(test.payer(), 0, true, &mut test).await;
    set_verification_state(test.payer(), 0, VerificationState::ProofSetup, &mut test).await;

    let payer = test.payer();
    let valid_finalize_ixs = finalize_instructions(
        &mut test,
        &request,
        &extra_data,
        &extra_data.reference(),
        &payer,
        None,
    )
    .await;
    let valid_transfer_ix = system_instruction::transfer(
        &test.payer(),
        &extra_data.recipient(),
        request.public_inputs.join_split.amount,
    );

    // Transfer instruction missing
    test.tx_should_fail_simple(&valid_finalize_ixs).await;

    // Transfer at wrong location
    test.tx_should_fail_simple(&merge(
        &valid_finalize_ixs,
        &[&valid_transfer_ix, &ElusivInstruction::nop_instruction()],
    ))
    .await;

    // Invalid transfer
    test.tx_should_fail_simple(&merge(
        &valid_finalize_ixs,
        &[&system_instruction::transfer(
            &payer,
            &payer,
            request.public_inputs.join_split.amount,
        )],
    ))
    .await;

    // Success (+ allows instructions before transfer)
    test.tx_should_succeed_simple(&merge(
        &valid_finalize_ixs,
        &[&ElusivInstruction::nop_instruction(), &valid_transfer_ix],
    ))
    .await;
}

#[tokio::test]
async fn test_solana_pay_lamports_with_memo() {
    let mut test = start_verification_test().await;
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;

    let memo = String::from("Hello World:)");
    let invalid_memo = String::from("Hello World");
    let mut request = send_request(0);
    let extra_data = ExtraData {
        memo: Some(memo.as_bytes().to_vec()),
        ..Default::default()
    };
    request.public_inputs.solana_pay_transfer = true;
    request.public_inputs.hashed_inputs = extra_data.hash();
    request.update_fee_lamports(&genesis_fee(&mut test).await);

    test.airdrop_lamports(&FeeCollectorAccount::find(None).0, LAMPORTS_PER_SOL)
        .await;
    test.airdrop_lamports(&PoolAccount::find(None).0, LAMPORTS_PER_SOL * 1000)
        .await;

    init_verification_simple(
        &request.proof,
        &request.public_inputs,
        extra_data.identifier,
        &mut test,
    )
    .await;
    skip_computation(test.payer(), 0, true, &mut test).await;
    set_verification_state(test.payer(), 0, VerificationState::ProofSetup, &mut test).await;

    let payer = test.payer();
    let valid_finalize_ixs = finalize_instructions(
        &mut test,
        &request,
        &extra_data,
        &extra_data.reference(),
        &payer,
        extra_data.memo.clone(),
    )
    .await;
    let valid_memo_ix = spl_memo::build_memo(memo.as_bytes(), &[]);
    let invalid_memo_ix = spl_memo::build_memo(invalid_memo.as_bytes(), &[]);
    let valid_transfer_ix = system_instruction::transfer(
        &test.payer(),
        &extra_data.recipient(),
        request.public_inputs.join_split.amount,
    );

    // Invalid memo
    test.tx_should_fail_simple(&merge(
        &valid_finalize_ixs,
        &[&invalid_memo_ix, &valid_transfer_ix],
    ))
    .await;

    // Missing memo instruction
    test.tx_should_fail_simple(&merge(&valid_finalize_ixs, &[&valid_transfer_ix]))
        .await;

    // Missing memo
    let invalid_ixs = finalize_instructions(
        &mut test,
        &request,
        &extra_data,
        &extra_data.reference(),
        &payer,
        None,
    )
    .await;
    test.tx_should_fail_simple(&merge(&invalid_ixs, &[&valid_memo_ix, &valid_transfer_ix]))
        .await;

    // Invalid reference account
    let invalid_ixs = finalize_instructions(
        &mut test,
        &request,
        &extra_data,
        &Pubkey::new_unique(),
        &payer,
        extra_data.memo.clone(),
    )
    .await;
    test.tx_should_fail_simple(&merge(&invalid_ixs, &[&valid_memo_ix, &valid_transfer_ix]))
        .await;

    // Missing transfer instruction
    test.tx_should_fail_simple(&merge(&valid_finalize_ixs, &[&valid_memo_ix]))
        .await;

    // Invalid sender
    let signer2 = Actor::new(&mut test).await;
    test.tx_should_fail(
        &merge(
            &valid_finalize_ixs,
            &[
                &valid_memo_ix,
                &system_instruction::transfer(
                    &signer2.pubkey,
                    &extra_data.recipient(),
                    request.public_inputs.join_split.amount,
                ),
            ],
        ),
        &[&signer2.keypair],
    )
    .await;

    // Invalid recipient
    test.tx_should_fail_simple(&merge(
        &valid_finalize_ixs,
        &[
            &valid_memo_ix,
            &system_instruction::transfer(
                &test.payer(),
                &Pubkey::new_unique(),
                request.public_inputs.join_split.amount,
            ),
        ],
    ))
    .await;

    // Invalid amount
    test.tx_should_fail_simple(&merge(
        &valid_finalize_ixs,
        &[
            &valid_memo_ix,
            &system_instruction::transfer(
                &test.payer(),
                &extra_data.recipient(),
                request.public_inputs.join_split.amount - 1,
            ),
        ],
    ))
    .await;

    // Missing memo instruction
    test.tx_should_fail_simple(&merge(&valid_finalize_ixs, &[&valid_transfer_ix]))
        .await;

    // Invalid memo
    test.tx_should_fail_simple(&merge(
        &valid_finalize_ixs,
        &[
            &spl_memo::build_memo(invalid_memo.as_bytes(), &[]),
            &valid_transfer_ix,
        ],
    ))
    .await;

    // Invalid memo/transfer order
    test.tx_should_fail_simple(&merge(
        &valid_finalize_ixs,
        &[&valid_transfer_ix, &valid_memo_ix],
    ))
    .await;

    // Invalid trailing instruction
    test.tx_should_fail_simple(&merge(
        &valid_finalize_ixs,
        &[&valid_memo_ix, &valid_transfer_ix, &valid_memo_ix],
    ))
    .await;

    // Success
    test.tx_should_succeed_simple(&merge(
        &valid_finalize_ixs,
        &[&valid_memo_ix, &valid_transfer_ix],
    ))
    .await;
}

#[tokio::test]
async fn test_solana_pay_tokens() {
    let mut test = start_verification_test().await;
    test.create_spl_token(USDC_TOKEN_ID).await;
    enable_program_token_account::<PoolAccount>(&mut test, USDC_TOKEN_ID, None).await;
    enable_program_token_account::<FeeCollectorAccount>(&mut test, USDC_TOKEN_ID, None).await;
    setup_vkey_account::<SendQuadraVKey>(&mut test).await;
    let nullifier_accounts = nullifier_accounts(&mut test, 0).await;
    let fee = genesis_fee(&mut test).await;

    let mut recipient = test.new_actor().await;
    recipient
        .open_token_account(USDC_TOKEN_ID, 0, &mut test)
        .await;
    let recipient_token_account = recipient.get_token_account(USDC_TOKEN_ID);

    let mut warden = test.new_actor().await;
    warden.open_token_account(USDC_TOKEN_ID, 0, &mut test).await;

    let sol_usd_price = Price {
        price: 41,
        conf: 0,
        expo: 0,
    };
    let usdc_usd_price = Price {
        price: 1,
        conf: 0,
        expo: 0,
    };
    let price =
        TokenPrice::new_from_sol_price(sol_usd_price, usdc_usd_price, USDC_TOKEN_ID).unwrap();
    let sol_price_account = test.token_to_usd_price_pyth_account(0);
    let token_price_account = test.token_to_usd_price_pyth_account(USDC_TOKEN_ID);
    test.set_token_to_usd_price_pyth(0, sol_usd_price).await;
    test.set_token_to_usd_price_pyth(USDC_TOKEN_ID, usdc_usd_price)
        .await;

    let mut request = send_request(0);
    let extra_data = ExtraData {
        recipient: recipient_token_account.to_bytes(),
        ..Default::default()
    };
    request.public_inputs.hashed_inputs = extra_data.hash();
    request.public_inputs.join_split.token_id = USDC_TOKEN_ID;
    request.public_inputs.join_split.amount = 1_000_000;
    request.public_inputs.solana_pay_transfer = true;
    request.update_fee_token(&fee, &price);

    let nullifier_duplicate_account = request.public_inputs.join_split.nullifier_duplicate_pda().0;
    let pool_account = program_token_account_address::<PoolAccount>(USDC_TOKEN_ID, None).unwrap();
    let fee_collector_account =
        program_token_account_address::<FeeCollectorAccount>(USDC_TOKEN_ID, None).unwrap();

    warden
        .airdrop(LAMPORTS_TOKEN_ID, LAMPORTS_PER_SOL * 100, &mut test)
        .await;
    test.airdrop(&pool_account, Token::new(USDC_TOKEN_ID, 1_000_000_000))
        .await;
    test.airdrop(&fee_collector_account, Token::new(USDC_TOKEN_ID, 1_000_000))
        .await;

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
                UserAccount(Pubkey::new_from_array(extra_data.identifier)),
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
    )
    .await;

    skip_computation(warden.pubkey, 0, true, &mut test).await;

    let valid_finalize_ixs = vec![
        ElusivInstruction::finalize_verification_send_instruction(
            0,
            FinalizeSendData {
                total_amount: request.public_inputs.join_split.total_amount(),
                token_id: USDC_TOKEN_ID,
                encrypted_owner: extra_data.encrypted_owner,
                iv: extra_data.iv,
                ..Default::default()
            },
            false,
            UserAccount(recipient_token_account),
            UserAccount(extra_data.identifier()),
            UserAccount(extra_data.reference()),
            UserAccount(warden.pubkey),
        ),
        ElusivInstruction::finalize_verification_insert_nullifier_instruction(
            0,
            UserAccount(warden.pubkey),
            Some(0),
            &writable_user_accounts(&[nullifier_accounts[0]]),
        ),
        ElusivInstruction::finalize_verification_transfer_token_instruction(
            0,
            WritableSignerAccount(warden.pubkey),
            WritableUserAccount(warden.get_token_account(USDC_TOKEN_ID)),
            WritableUserAccount(recipient_token_account),
            UserAccount(recipient_token_account),
            WritableUserAccount(pool_account),
            WritableUserAccount(fee_collector_account),
            WritableUserAccount(Pubkey::new_unique()),
            WritableUserAccount(nullifier_duplicate_account),
            UserAccount(spl_token::id()),
        ),
    ];

    let transfer_ix = spl_token::instruction::transfer(
        &spl_token::id(),
        &warden.get_token_account(USDC_TOKEN_ID),
        &recipient_token_account,
        &warden.pubkey,
        &[&warden.pubkey],
        request.public_inputs.join_split.amount,
    )
    .unwrap();

    // Transfer instruction missing
    test.tx_should_fail(&valid_finalize_ixs, &[&warden.keypair])
        .await;

    // Invalid amount
    test.tx_should_fail(
        &merge(
            &valid_finalize_ixs,
            &[&spl_token::instruction::transfer(
                &spl_token::id(),
                &warden.get_token_account(USDC_TOKEN_ID),
                &recipient_token_account,
                &warden.pubkey,
                &[&warden.pubkey],
                request.public_inputs.join_split.amount - 1,
            )
            .unwrap()],
        ),
        &[&warden.keypair],
    )
    .await;

    // Invalid recipient
    test.tx_should_fail(
        &merge(
            &valid_finalize_ixs,
            &[&spl_token::instruction::transfer(
                &spl_token::id(),
                &warden.get_token_account(USDC_TOKEN_ID),
                &warden.get_token_account(USDC_TOKEN_ID),
                &warden.pubkey,
                &[&warden.pubkey],
                request.public_inputs.join_split.amount,
            )
            .unwrap()],
        ),
        &[&warden.keypair],
    )
    .await;

    // Invalid token-program
    let mut ix = transfer_ix.clone();
    ix.program_id = system_program::id();
    test.tx_should_fail(&merge(&valid_finalize_ixs, &[&ix]), &[&warden.keypair])
        .await;

    // Success
    test.tx_should_succeed(
        &merge(&valid_finalize_ixs, &[&transfer_ix]),
        &[&warden.keypair],
    )
    .await;
}
