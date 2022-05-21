use solana_program::hash::Hash;
use solana_program_test::*;
use solana_sdk::signature::Keypair;
use elusiv::entrypoint::process_instruction;

pub async fn start_program_solana_program_test() -> (solana_program_test::BanksClient, Keypair, Hash) {
    let mut test = ProgramTest::default();
    let program_id = elusiv::id();
    test.add_program("elusiv", program_id, processor!(process_instruction));
    test.start().await
}