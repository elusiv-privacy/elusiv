//! Tests the account setup process

#[cfg(not(tarpaulin_include))]
mod common;
use common::program_setup::{start_program_solana_program_test_with_accounts_setup, setup_pda_accounts, setup_queue_accounts};

use solana_program_test::*;

#[tokio::test]
async fn test_verify_full_proof() {
    let (mut banks_client, payer, recent_blockhash, _, _) = start_program_solana_program_test_with_accounts_setup(
        |_| {},
        |_| {},
        |_| {},
        |_| {},
        |_| {},
        |_| {},
    ).await;
}