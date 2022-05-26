pub mod program_setup;

use solana_program_test::BanksClient;
use solana_program::pubkey::Pubkey;

// Fetch account balance and data
pub async fn get_balance(banks_client: &mut BanksClient, pubkey: Pubkey) -> u64 {
    banks_client.get_account(pubkey).await.unwrap().unwrap().lamports
}

pub async fn get_data(banks_client: &mut BanksClient, id: Pubkey) -> Vec<u8> {
    banks_client.get_account(id).await.unwrap().unwrap().data
}