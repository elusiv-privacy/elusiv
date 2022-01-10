use {
    solana_program::pubkey::Pubkey,
    poseidon::poseidon::Poseidon,
    poseidon::scalar,
    std::str::FromStr,
    rand::Rng,
    solana_program_test::*,
    elusiv::state::TOTAL_SIZE,
    elusiv::entrypoint::process_instruction,
};

// Storage account
pub fn storage_account_id() -> Pubkey {
    Pubkey::from_str("SRMuApVNdxXokk5GT7XD5cUUgXMBCoAz2LHeuAoKWRt").unwrap()
}

pub fn new_storage_account_data(size: usize) -> String {
    let data: Vec<u8> = vec![0; size];
    base64::encode(&data)
}

// Commitment generation
fn random_scalar() -> scalar::Scalar {
    let mut random = rand::thread_rng().gen::<[u8; 32]>();
    random[31] = 0;
    scalar::from_bytes_le(&random)
}

pub fn valid_commitment() -> scalar::Scalar {
    let random = random_scalar();
    let nullifier = random_scalar();
    Poseidon::new().hash(vec![random, nullifier]).unwrap()
}

pub async fn start_program<F>(setup: F) -> (solana_program_test::BanksClient, solana_sdk::signature::Keypair, solana_program::hash::Hash)
where F: Fn(&mut ProgramTest) -> ()
{
    let mut test = ProgramTest::default();
    let program_id = elusiv::id();
    test.add_program("elusiv", program_id, processor!(process_instruction));
    setup(&mut test);
    test.start().await
}

pub async fn start_program_with_storage(storage_id: Pubkey) -> (solana_program_test::BanksClient, solana_sdk::signature::Keypair, solana_program::hash::Hash) {
    let setup = |test: &mut ProgramTest| {
        let data = new_storage_account_data(TOTAL_SIZE);
        test.add_account_with_base64_data(storage_id, 100000000, elusiv::id(), &data);
    };
    start_program(setup).await
}