use {
    solana_program::hash::Hash,
    solana_program_test::*,
    solana_sdk::signature::Keypair,

    elusiv::entrypoint::process_instruction,

    super::accounts::*,
};

pub async fn start_program<F>(setup: F, iterations: u64) -> (solana_program_test::BanksClient, Keypair, Hash)
where F: Fn(&mut ProgramTest) -> ()
{
    let mut test = ProgramTest::default();
    let program_id = elusiv::id();
    test.add_program("elusiv", program_id, processor!(process_instruction));

    setup(&mut test);

    //let cus = 2000000 * iterations;
    let cus = 30_000_000;

    test.set_compute_max_units(cus);
    test.start().await
}

pub async fn start_program_with_program_accounts(iterations: u64) -> (solana_program_test::BanksClient, Keypair, Hash) {
    let setup = |test: &mut ProgramTest| {
        let data = new_program_accounts_data();
        test.add_account_with_base64_data(program_account_id(), 100000000, elusiv::id(), &data.0);
        test.add_account_with_base64_data(deposit_account_id(), 100000000, elusiv::id(), &data.1);
        test.add_account_with_base64_data(withdraw_account_id(), 100000000, elusiv::id(), &data.2);
    };
    start_program(setup, iterations).await
}