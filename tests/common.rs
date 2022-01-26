#![allow(dead_code)]

use {
    assert_matches::*,
    solana_program::{
        instruction::Instruction,
        pubkey::Pubkey,
        instruction::AccountMeta,
        hash::Hash,
        system_program,
        native_token::LAMPORTS_PER_SOL,
    },
    solana_program_test::*,
    solana_sdk::{
        signature::Signer,
        transaction::Transaction,
        signature::Keypair,
    },
    rand::Rng,
    std::str::FromStr,
    elusiv::poseidon::*,

    elusiv::merkle::*,
    elusiv::entrypoint::process_instruction,
    elusiv::state::{
        TOTAL_SIZE,
        TREE_SIZE,
        TREE_LEAF_START,
        TREE_LEAF_COUNT,
    },
    elusiv::state::StorageAccount,
    elusiv::state::TREE_HEIGHT,

    elusiv::poseidon::ITERATIONS,
    ark_ff::*,
    num_bigint::BigUint,
    ark_groth16::Proof,
    ark_bn254::*,
};

// String number conversions
pub fn str_to_bytes(str: &str) -> Vec<u8> {
    let mut writer: Vec<u8> = vec![];
    str_to_bigint(str).write(&mut writer).unwrap();
    writer
}

pub fn str_to_bigint(str: &str) -> BigInteger256 {
    BigInteger256::try_from(BigUint::from_str(str).unwrap()).unwrap()
}

// Storage account
pub fn storage_id() -> Pubkey {
    Pubkey::from_str("SRMuApVNdxXokk5GT7XD5cUUgXMBCoAz2LHeuAoKWRt").unwrap()
}

pub async fn get_storage_data(banks_client: &mut BanksClient) -> Vec<u8> {
    banks_client.get_account(storage_id()).await.unwrap().unwrap().data
}

pub fn new_storage_account_data() -> String {
    let mut data: Vec<u8> = vec![0; TOTAL_SIZE];

    // Setup Merkle tree and initialize values
    {
        let storage = StorageAccount::from(&mut data).unwrap();
        let poseidon = Poseidon2::new();
        let hash = |left: Scalar, right: Scalar| { poseidon.full_hash(left, right) };
        initialize_store(storage.merkle_tree, Scalar::zero(), hash);
    }

    base64::encode(&data)
}

// Account balance
pub async fn get_balance(banks_client: &mut BanksClient, pubkey: Pubkey) -> u64 {
    banks_client.get_account(pubkey).await.unwrap().unwrap().lamports
}

// Commitment generation
fn random_scalar() -> Scalar {
    let mut random = rand::thread_rng().gen::<[u8; 32]>();
    random[31] = 0;
    from_bytes_le(&random)
}

pub fn valid_commitment() -> Scalar {
    //let nullifier = random_scalar();
    let random = random_scalar();
    //Poseidon::new().hash_two(nullifier, random)
    random
}

pub async fn start_program<F>(setup: F) -> (solana_program_test::BanksClient, Keypair, Hash)
where F: Fn(&mut ProgramTest) -> ()
{
    let mut test = ProgramTest::default();
    let program_id = elusiv::id();
    test.add_program("elusiv", program_id, processor!(process_instruction));
    setup(&mut test);
    test.set_compute_max_units(180000 * (TREE_HEIGHT as u64) * (ITERATIONS as u64));
    test.start().await
}

pub async fn start_program_with_storage(storage_id: Pubkey) -> (solana_program_test::BanksClient, Keypair, Hash) {
    let setup = |test: &mut ProgramTest| {
        let data = new_storage_account_data();
        test.add_account_with_base64_data(storage_id, 100000000, elusiv::id(), &data);
    };
    start_program(setup).await
}

pub fn get_commitments(account_data: &mut [u8]) -> Vec<Scalar> {
    let tree_leaves = &account_data[TREE_LEAF_START * 32..TREE_SIZE];
    let mut leaves = Vec::new();
    for l in 0..TREE_LEAF_COUNT {
        leaves.push(from_bytes_le(&tree_leaves[l * 32..(l + 1) * 32]))
    }
    leaves
}

// Deposit
pub async fn send_deposit_transaction(storage_id: Pubkey, payer: &Keypair, recent_blockhash: Hash, data: Vec<u8>) -> Transaction {
    // Start deposit
    let mut instructions = Vec::new();
    instructions.push(Instruction {
        program_id: elusiv::id(),
        accounts: vec!
        [
            AccountMeta::new(storage_id, false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(system_program::id(), false),
        ],
        data: data,
    });

    // Calculate deposit
    for _ in 0..ITERATIONS * TREE_HEIGHT - 1 {
        instructions.push(Instruction {
            program_id: elusiv::id(),
            accounts: vec![AccountMeta::new(storage_id, false)],
            data: vec![1],
        });
    }

    // Finalize deposit
    instructions.push(Instruction {
        program_id: elusiv::id(),
        accounts: vec!
        [
            AccountMeta::new(storage_id, false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(system_program::id(), false),
        ],
        data: vec![2],
    });

    // Sign and send transaction
    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));
    transaction.sign(&[payer], recent_blockhash);
    transaction
}

pub async fn send_valid_deposit(payer: &Keypair, banks_client: &mut BanksClient, recent_blockhash: Hash) -> Scalar {
    let commitment = valid_commitment();
    let data = deposit_data(commitment);

    let t = send_deposit_transaction(storage_id(), &payer, recent_blockhash, data).await;
    assert_matches!(banks_client.process_transaction(t).await, Ok(()));

    commitment
}

pub fn deposit_data(commitment: Scalar) -> Vec<u8> {
    let mut data = vec![0];
    let amount: u64 = LAMPORTS_PER_SOL;
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&to_bytes_le(commitment));
    data
}

// Withdraw
pub async fn send_withdraw_transaction(program_id: Pubkey, storage_id: Pubkey, payer: Keypair, recipient: Pubkey, recent_blockhash: Hash, data: Vec<u8>) -> Transaction {
    let mut transaction = Transaction::new_with_payer(
        &[Instruction {
            program_id,
            accounts: vec!
            [
                AccountMeta::new(storage_id, false) ,
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(recipient, false),
            ],
            data,
        }],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    transaction
}

pub fn withdraw_data(proof: ProofString, inputs: &[&str]) -> Vec<u8> {
    let mut data = vec![3];

    let amount: u64 = LAMPORTS_PER_SOL;
    data.extend_from_slice(&amount.to_le_bytes());

    proof.push_to_vec(&mut data);

    for input in inputs {
        data.extend(str_to_bytes(input));
    }

    data
}

// Proof
pub struct ProofString {
    pub ax: &'static str,
    pub ay: &'static str,
    pub az: &'static str,

    pub bx0: &'static str,
    pub bx1: &'static str,
    pub by0: &'static str,
    pub by1: &'static str,
    pub bz0: &'static str,
    pub bz1: &'static str,

    pub cx: &'static str,
    pub cy: &'static str,
    pub cz: &'static str,
}

impl ProofString {
    pub fn generate_proof(&self) -> Proof<Bn254> {
        Proof {
            a: G1Affine::from(G1Projective::new(str_to_bigint(self.ax).into(), str_to_bigint(self.ay).into(), str_to_bigint(self.az).into())),
            b: G2Affine::from(
                G2Projective::new(
                    Fq2::new(str_to_bigint(self.bx0).into(), str_to_bigint(self.bx1).into()),
                    Fq2::new(str_to_bigint(self.by0).into(), str_to_bigint(self.by1).into()),
                    Fq2::new(str_to_bigint(self.bz0).into(), str_to_bigint(self.bz1).into()),
                )
            ),
            c: G1Affine::from(G1Projective::new(str_to_bigint(self.cx).into(), str_to_bigint(self.cy).into(), str_to_bigint(self.cz).into()))
        }
    }

    pub fn push_to_vec(&self, v: &mut Vec<u8>) {
        v.extend(str_to_bytes(self.ax));
        v.extend(str_to_bytes(self.ay));
        v.push(if self.az == "0" { 0 } else { 1 });

        v.extend(str_to_bytes(self.bx0));
        v.extend(str_to_bytes(self.bx1));
        v.extend(str_to_bytes(self.by0));
        v.extend(str_to_bytes(self.by1));
        v.push(if self.bz0 == "0" { 0 } else { 1 });
        v.push(if self.bz1 == "0" { 0 } else { 1 });

        v.extend(str_to_bytes(self.cx));
        v.extend(str_to_bytes(self.cy));
        v.push(if self.cz == "0" { 0 } else { 1 });
    }
}