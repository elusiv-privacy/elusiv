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

    elusiv::scalar::*,
    elusiv::entrypoint::process_instruction,
    elusiv::state::*,
    elusiv::poseidon,
    elusiv::groth16,

    ark_ff::*,
    num_bigint::BigUint,
    ark_bn254::*,
};

// String number conversions
pub fn str_to_bytes(str: &str) -> Vec<u8> {
    //to_bytes_le_mont(from_str_10(str)
    let mut writer: Vec<u8> = vec![];
    str_to_bigint(str).write(&mut writer).unwrap();
    writer
}

pub fn str_to_bigint(str: &str) -> BigInteger256 {
    BigInteger256::try_from(BigUint::from_str(str).unwrap()).unwrap()
}

// Storage accounts
pub fn program_account_id() -> Pubkey { Pubkey::from_str("SRMuApVNdxXokk5GT7XD5cUUgXMBCoAz2LHeuAoKWRt").unwrap() }
pub fn deposit_account_id() -> Pubkey { Pubkey::from_str("22EpmWFRE2LueXfghXSbryxd5CWLXLS5gxjHG5hrt4eb").unwrap() }
pub fn withdraw_account_id() -> Pubkey { Pubkey::from_str("EMkFvuRAB1iWEDsY1kdgCrrokExdWNh3dUqbLebms4FY").unwrap() }

pub fn new_program_accounts_data() -> (String, String, String) {
    let data0: Vec<u8> = vec![0; ProgramAccount::TOTAL_SIZE];
    let data1: Vec<u8> = vec![0; DepositHashingAccount::TOTAL_SIZE];
    let data2: Vec<u8> = vec![0; WithdrawVerificationAccount::TOTAL_SIZE];

    (
        base64::encode(&data0),
        base64::encode(&data1),
        base64::encode(&data2),
    )
}

// Fetch account balance and data
pub async fn get_balance(banks_client: &mut BanksClient, pubkey: Pubkey) -> u64 {
    banks_client.get_account(pubkey).await.unwrap().unwrap().lamports
}

pub async fn get_account_data(banks_client: &mut BanksClient, id: Pubkey) -> Vec<u8> {
    banks_client.get_account(id).await.unwrap().unwrap().data
}

// Commitment generation and fetching
fn random_scalar() -> Scalar {
    let mut random = rand::thread_rng().gen::<[u8; 32]>();
    random[31] = 0;
    from_bytes_le_repr(&random).unwrap()
}

pub fn valid_commitment() -> Scalar {
    let nullifier = random_scalar();
    let random = random_scalar();
    poseidon::Poseidon2::new().full_hash(nullifier, random)
}

pub fn get_commitments(account_data: &mut [u8]) -> Vec<Scalar> {
    let tree_leaves = &account_data[TREE_LEAF_START * 32..TREE_SIZE];
    let mut leaves = Vec::new();
    for l in 0..TREE_LEAF_COUNT {
        leaves.push(from_bytes_le_mont(&tree_leaves[l * 32..(l + 1) * 32]))
    }
    leaves
}

// Program setups
pub async fn start_program<F>(setup: F) -> (solana_program_test::BanksClient, Keypair, Hash)
where F: Fn(&mut ProgramTest) -> ()
{
    let mut test = ProgramTest::default();
    let program_id = elusiv::id();
    test.add_program("elusiv", program_id, processor!(process_instruction));

    setup(&mut test);

    // Deposit
    let mut cus = 180000 * (TREE_HEIGHT as u64) * (poseidon::ITERATIONS as u64);
    // Withdraw
    cus += 200000 * (groth16::ITERATIONS as u64 + 1);

    test.set_compute_max_units(cus);
    test.start().await
}

pub async fn start_program_with_program_accounts() -> (solana_program_test::BanksClient, Keypair, Hash) {
    let setup = |test: &mut ProgramTest| {
        let data = new_program_accounts_data();
        test.add_account_with_base64_data(program_account_id(), 100000000, elusiv::id(), &data.0);
        test.add_account_with_base64_data(deposit_account_id(), 100000000, elusiv::id(), &data.1);
        test.add_account_with_base64_data(withdraw_account_id(), 100000000, elusiv::id(), &data.2);
    };
    start_program(setup).await
}

// Deposit
pub async fn send_deposit_transaction(payer: &Keypair, recent_blockhash: Hash, data: Vec<u8>) -> Transaction {
    // Start deposit
    let mut instructions = Vec::new();
    instructions.push(Instruction {
        program_id: elusiv::id(),
        accounts: vec!
        [
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(program_account_id(), false),
            AccountMeta::new(deposit_account_id(), false),
        ],
        data: data,
    });

    // Calculate deposit
    for _ in 0..poseidon::ITERATIONS * TREE_HEIGHT - 1 {
        instructions.push(Instruction {
            program_id: elusiv::id(),
            accounts: vec![AccountMeta::new(deposit_account_id(), false)],
            data: vec![1],
        });
    }

    // Finalize deposit
    instructions.push(Instruction {
        program_id: elusiv::id(),
        accounts: vec!
        [
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(program_account_id(), false),
            AccountMeta::new(deposit_account_id(), false),
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

    let t = send_deposit_transaction(&payer, recent_blockhash, data).await;
    assert_matches!(banks_client.process_transaction(t).await, Ok(()));

    commitment
}

pub fn deposit_data(commitment: Scalar) -> Vec<u8> {
    let mut data = vec![0];
    let amount: u64 = LAMPORTS_PER_SOL;
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&to_bytes_le_mont(commitment));
    data
}

// Withdraw
pub async fn send_withdraw_transaction(payer: &Keypair, recipient: Pubkey, recent_blockhash: Hash, data: Vec<u8>) -> Transaction {
    // Start withdrawal
    let mut instructions = Vec::new();
    instructions.push(Instruction {
        program_id: elusiv::id(),
        accounts: vec!
        [
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(program_account_id(), false),
            AccountMeta::new(withdraw_account_id(), false),
        ],
        data: data,
    });

    // Compute verification
    for _ in 0..groth16::ITERATIONS {
        instructions.push(Instruction {
            program_id: elusiv::id(),
            accounts: vec![ AccountMeta::new(withdraw_account_id(), false) ],
            data: vec![4],
        });
    }

    // Finalize deposit
    instructions.push(Instruction {
        program_id: elusiv::id(),
        accounts: vec!
        [
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(program_account_id(), false),
            AccountMeta::new(withdraw_account_id(), false),
            AccountMeta::new(recipient, true),
        ],
        data: vec![5],
    });

    // Sign and send transaction
    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));
    transaction.sign(&[payer], recent_blockhash);
    transaction
}

pub fn withdraw_data(proof: ProofString, inputs: &[&str]) -> Vec<u8> {
    let mut data = vec![3];

    //let amount: u64 = LAMPORTS_PER_SOL;
    //data.extend_from_slice(&amount.to_le_bytes());

    proof.push_to_vec(&mut data);

    for input in inputs {
        data.extend(to_bytes_le_mont(from_str_10(input)));
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
    pub fn generate_proof(&self) -> groth16::Proof {
        groth16::Proof {
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