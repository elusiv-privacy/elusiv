mod common;
use common::program_setup::*;
use common::{
    get_balance,
    get_data,
};
use solana_program_test::*;
/*
#[tokio::test]
async fn test_store() {
    let (mut banks_client, payer, recent_blockhash) = start_program_solana_program_test().await;

    let base_commitment = [0; 32];
    let amount: u64 = LAMPORTS_PER_SOL;
    let commitment = [0; 32];

    let base_commitment_request = BaseCommitmentHashRequest {
        base_commitment,
        amount,
        commitment,
        is_active: false
    };

    let mut transaction = Transaction::new_with_payer(
        &[
            ElusivInstruction::store(base_commitment_request, SignerAccount(payer.pubkey())),
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
}

*/

macro_rules! assert_account {
    ($ty: ty, $banks_client: ident) => {
        assert!(get_balance(&mut $banks_client, <$ty>::pubkey(&vec![]).0).await > 0);
        assert!(get_data(&mut $banks_client, <$ty>::pubkey(&vec![]).0).await.len() == <$ty>::SIZE);
    };
}

#[tokio::test]
async fn test_setup_pda_accounts() {
    let (mut banks_client, payer, recent_blockhash) = start_program_solana_program_test().await;

    setup_all_accounts(&mut banks_client, &payer, recent_blockhash).await;
}

/*#[tokio::test]
#[ignore]
async fn test_fail() {
    let (mut banks_client, payer, recent_blockhash) = start_program_solana_program_test().await;
    setup_pda_accounts(&mut banks_client, &payer, recent_blockhash).await;

    let mut transaction = Transaction::new_with_payer(
        &[
            request_compute_units(1_400_000),
            ElusivInstruction::test_fail(),
        ],
        Some(&payer.pubkey()),
    );
    transaction.sign(&[&payer], recent_blockhash);
    assert_matches!(banks_client.process_transaction(transaction).await, Ok(()));
}*/