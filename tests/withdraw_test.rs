mod common;
use assert_matches::*;
//use solana_program_test::*;
use solana_sdk::signature::Signer;
use common::*;

//#[tokio::test]
//#[ignore]
async fn _test_withdraw() {
    //capture_compute_units();
    //check_compute_units();

    // Setup program and storage account
    let (mut banks_client, payer, recent_blockhash) = start_program_with_program_accounts(elusiv::groth16::ITERATIONS as u64).await;

    // Withdrawal data
    let recipient = payer.pubkey();

    // Send transaction
    let t = withdraw_transaction(&payer, recipient, recent_blockhash, withdraw_data(&test_proof(), &test_inputs())).await;
    assert_matches!(banks_client.process_transaction(t).await, Ok(()));
}