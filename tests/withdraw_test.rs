mod common;
use {
    assert_matches::*,
    solana_program_test::*,
    solana_sdk::signature::Signer,
    //elusiv::scalar::*,
    //elusiv::poseidon::*,
    //solana_program::native_token::LAMPORTS_PER_SOL,
    common::*,
};

#[tokio::test]
async fn test_withdraw() {
    // Setup program and storage account
    let (mut banks_client, payer, recent_blockhash) = start_program_with_program_accounts(1).await;

    // Withdrawal data
    let recipient = payer.pubkey();

    // Send transaction
    let t = withdraw_transaction(&payer, recipient, recent_blockhash, withdraw_data(&test_proof(), &test_inputs())).await;
    assert_matches!(banks_client.process_transaction(t).await, Ok(()));
}