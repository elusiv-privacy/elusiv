use {
    solana_program::pubkey::Pubkey,
    solana_program_test::*,
    std::str::FromStr,
    ark_ff::*,
    elusiv::state::*,
};

// Storage accounts
pub fn program_account_id() -> Pubkey { Pubkey::from_str("SRMuApVNdxXokk5GT7XD5cUUgXMBCoAz2LHeuAoKWRt").unwrap() }
pub fn deposit_account_id() -> Pubkey { Pubkey::from_str("22EpmWFRE2LueXfghXSbryxd5CWLXLS5gxjHG5hrt4eb").unwrap() }
pub fn withdraw_account_id() -> Pubkey { Pubkey::from_str("EMkFvuRAB1iWEDsY1kdgCrrokExdWNh3dUqbLebms4FY").unwrap() }

pub fn new_program_accounts_data() -> (String, String, String) {
    let data0: Vec<u8> = vec![0; ProgramAccount::TOTAL_SIZE];
    let data1: Vec<u8> = vec![0; DepositHashingAccount::TOTAL_SIZE];
    let data2: Vec<u8> = vec![0; ProofVerificationAccount::TOTAL_SIZE];

    (
        base64::encode(&data0),
        base64::encode(&data1),
        base64::encode(&data2),
    )
}

// Fetch account balance and data
pub async fn get_balance(banks_client: &mut BanksClient, pubkey: Pubkey) -> u64 {
    banks_client.get_account(pubkey).await.unwrap().unwrap().lamports
}

pub async fn get_account_data(banks_client: &mut BanksClient, id: Pubkey) -> Vec<u8> {
    banks_client.get_account(id).await.unwrap().unwrap().data
}