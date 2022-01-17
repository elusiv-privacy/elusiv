/*use {
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
    poseidon::*,

    elusiv::state::{
        TOTAL_SIZE,
        TREE_SIZE,
        //TREE_HEIGHT,
        TREE_LEAF_START,
        TREE_LEAF_COUNT,
    },
    elusiv::entrypoint::process_instruction,
    ark_ff::*,
    num_bigint::BigUint,
};
use ark_groth16::Proof;
use ark_bn254::*;

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

#[allow(dead_code)]
pub async fn get_storage_data(banks_client: &mut BanksClient) -> Vec<u8> {
    banks_client.get_account(storage_id()).await.unwrap().unwrap().data
}

pub fn new_storage_account_data() -> String {
    let data: Vec<u8> = vec![0; TOTAL_SIZE];
    //let (mut merkle_store, _) = data.split_at_mut(TREE_SIZE);

    // Setup Merkle tree
    /*let poseidon = Poseidon::new();
    let limbs_to_value = |limbs: &[u8]| { from_bytes_le(limbs) };
    let value_to_limbs = |value: Scalar| { to_bytes_le(value) };
    let hash = |left: Scalar, right: Scalar| { poseidon.hash(vec![left, right]).unwrap() };*/
    //let mut tree = elusiv::limbed_merkle::LimbedMerkleTree::new(TREE_HEIGHT, &mut merkle_store, 5, hash, limbs_to_value, value_to_limbs).unwrap();
    //tree.initialize_store(Scalar::zero());

    //println!("{:?}", &merkle_store[..32]);

    base64::encode(&data)
}

// Accounts
#[allow(dead_code)]
pub async fn get_balance(banks_client: &mut BanksClient, pubkey: Pubkey) -> u64 {
    banks_client.get_account(pubkey).await.unwrap().unwrap().lamports
}

// Commitment generation
#[allow(dead_code)]
fn random_scalar() -> Scalar {
    let mut random = rand::thread_rng().gen::<[u8; 32]>();
    random[31] = 0;
    from_bytes_le(&random)
}

#[allow(dead_code)]
pub fn valid_commitment() -> Scalar {
    //let nullifier = random_scalar();
    let random = random_scalar();
    //Poseidon::new().hash_two(nullifier, random)
    random
}

#[allow(dead_code)]
pub async fn start_program<F>(setup: F) -> (solana_program_test::BanksClient, Keypair, Hash)
where F: Fn(&mut ProgramTest) -> ()
{
    let mut test = ProgramTest::default();
    let program_id = elusiv::id();
    test.add_program("elusiv", program_id, processor!(process_instruction));
    setup(&mut test);
    test.start().await
}

#[allow(dead_code)]
pub async fn start_program_with_storage(storage_id: Pubkey) -> (solana_program_test::BanksClient, Keypair, Hash) {
    let setup = |test: &mut ProgramTest| {
        let data = new_storage_account_data();
        test.add_account_with_base64_data(storage_id, 100000000, elusiv::id(), &data);
    };
    start_program(setup).await
}

#[allow(dead_code)]
pub fn get_commitments(account_data: &mut [u8]) -> Vec<Scalar> {
    let tree_leaves = &account_data[TREE_LEAF_START * 32..TREE_SIZE];
    let mut leaves = Vec::new();
    for l in 0..TREE_LEAF_COUNT {
        leaves.push(from_bytes_le(&tree_leaves[l * 32..(l + 1) * 32]))
    }
    leaves
}

// Deposit
pub async fn send_deposit_transaction(program_id: Pubkey, storage_id: Pubkey, payer: &Keypair, recent_blockhash: Hash, data: Vec<u8>) -> Transaction {
    let mut transaction = Transaction::new_with_payer(
        &[Instruction {
            program_id,
            accounts: vec!
            [
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(storage_id, false),
                AccountMeta::new(system_program::id(), false),
            ],
            data,
        }],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[payer], recent_blockhash);
    transaction
}

#[allow(dead_code)]
pub async fn send_valid_deposit(payer: &Keypair, banks_client: &mut BanksClient, recent_blockhash: Hash) -> Scalar {
    let commitment = valid_commitment();
    let data = deposit_data(commitment);

    let t = send_deposit_transaction(elusiv::id(), storage_id(), &payer, recent_blockhash, data).await;
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
#[allow(dead_code)]
pub async fn send_withdraw_transaction(program_id: Pubkey, storage_id: Pubkey, payer: Keypair, recipient: Pubkey, recent_blockhash: Hash, data: Vec<u8>) -> Transaction {
    let mut transaction = Transaction::new_with_payer(
        &[Instruction {
            program_id,
            accounts: vec!
            [
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(storage_id, false) ,
                AccountMeta::new(recipient, false),
            ],
            data,
        }],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    transaction
}

// Proof
#[allow(dead_code)]
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

#[allow(dead_code)]
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
}*/