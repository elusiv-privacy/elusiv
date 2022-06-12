//! Tests the account setup process

#[cfg(not(tarpaulin_include))]
mod common;
//use common::*;
//use common::program_setup::*;

//use solana_program::native_token::LAMPORTS_PER_SOL;
use solana_program_test::*;

#[tokio::test]
async fn test_send_proof() {
    // Client has stored these three commitments in the MT
    /*let commitments = [
        base_commitment_request(
            "",
            "",
            LAMPORTS_PER_SOL / 2,
            0,
        ),
        base_commitment_request(
            "",
            "",
            LAMPORTS_PER_SOL / 2,
            0,
        ),
        base_commitment_request(
            "",
            "",
            LAMPORTS_PER_SOL / 4,
            0,
        ),
    ];
    let private_balance = commitments.iter().fold(0, |acc, x| acc + x.amount);*/

    // Client sends 1 SOL - fees to R1
    //let _remaining_balance = private_balance - LAMPORTS_PER_SOL;

    // Client sends 1/4 SOL - fees to R2

    // Client now has a private balance of zero
}