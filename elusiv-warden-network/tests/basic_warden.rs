mod common;

use common::*;
use solana_program_test::*;
use std::net::Ipv4Addr;
use elusiv_types::{WritableSignerAccount, SignerAccount, UserAccount, ProgramAccount, TOKENS};
use elusiv_warden_network::{
    instruction::ElusivWardenNetworkInstruction,
    warden::{ElusivBasicWardenConfig, BasicWardenAccount, BasicWardenStatsAccount, BasicWardenMapAccount, ElusivBasicWardenFeatures},
    processor::{unix_timestamp_to_day_and_year, TRACKABLE_ELUSIV_INSTRUCTIONS},
};
use solana_program::{pubkey::Pubkey, instruction::{Instruction, AccountMeta}};

#[tokio::test]
async fn test_register() {
    let mut test = start_test_with_setup().await;

    let ident = String::from("Test Warden 1");
    let platform = String::from("Linux, Ubuntu Server 22.0");

    let mut config = ElusivBasicWardenConfig {
        ident: ident.try_into().unwrap(),
        key: test.payer(),
        operator: Some(Pubkey::new_unique()).into(),
        addr: Ipv4Addr::new(0, 0, 0, 0),
        rpc_port: 0,
        tls_mode: elusiv_warden_network::warden::TlsMode::NoTls,
        jurisdiction: 0,
        location: 0,
        timezone: 0,
        version: [0, 0, 0],
        platform: platform.try_into().unwrap(),
        features: ElusivBasicWardenFeatures::default(),
        tokens: [false; TOKENS.len()],
    };

    // Invalid warden_id
    test.ix_should_fail_simple(
        ElusivWardenNetworkInstruction::register_basic_warden_instruction(
            1,
            config.clone(),
            WritableSignerAccount(test.payer()),
        )
    ).await;

    // Invalid config.key
    config.key = Pubkey::new_unique();
    test.ix_should_fail_simple(
        ElusivWardenNetworkInstruction::register_basic_warden_instruction(
            0,
            config.clone(),
            WritableSignerAccount(test.payer()),
        )
    ).await;

    config.key = test.payer();
    test.ix_should_succeed_simple(
        ElusivWardenNetworkInstruction::register_basic_warden_instruction(
            0,
            config.clone(),
            WritableSignerAccount(test.payer()),
        )
    ).await;

    // Duplicate registration fails
    test.ix_should_fail_simple(
        ElusivWardenNetworkInstruction::register_basic_warden_instruction(
            0,
            config.clone(),
            WritableSignerAccount(test.payer()),
        )
    ).await;

    let map_account_data = test.eager_account2::<BasicWardenMapAccount, _>(test.payer(), None).await;
    assert_eq!(0, map_account_data.warden_id);

    let basic_warden_account = test.eager_account::<BasicWardenAccount, _>(Some(0)).await;
    let basic_warden = basic_warden_account.warden;
    assert_eq!(basic_warden.config, config);
    assert_eq!(basic_warden.lut, Pubkey::new_from_array([0; 32]));
    assert!(!basic_warden.is_active);

    // TODO: Check join_timestamp and activation_timestamp
}

#[tokio::test]
async fn test_register_warden_id() {
    let mut test = start_test_with_setup().await;
    let number_of_wardens = 100;

    for n in 0..number_of_wardens {
        let mut warden = Actor::new(&mut test).await;
        register_warden(&mut test, &mut warden).await;

        let map_account_data = test.eager_account2::<BasicWardenMapAccount, _>(warden.pubkey, None).await;
        assert_eq!(n, map_account_data.warden_id);

        let basic_warden_account = test.eager_account::<BasicWardenAccount, _>(Some(n)).await;
        assert_eq!(basic_warden_account.warden.config.key, warden.pubkey);
    }
}

#[ignore]
#[tokio::test]
async fn test_register_warden_account_fuzzing() {
    let mut test = start_test_with_setup().await;
    let warden = Actor::new(&mut test).await;

    let config = ElusivBasicWardenConfig {
        ident: String::new().try_into().unwrap(),
        key: test.payer(),
        operator: Some(Pubkey::new_unique()).into(),
        addr: Ipv4Addr::new(0, 0, 0, 0),
        rpc_port: 0,
        tls_mode: elusiv_warden_network::warden::TlsMode::NoTls,
        jurisdiction: 0,
        location: 0,
        timezone: 0,
        version: [0, 0, 0],
        platform: String::new().try_into().unwrap(),
        features: ElusivBasicWardenFeatures::default(),
        tokens: [false; TOKENS.len()],
    };

    test.invalid_accounts_fuzzing(
        &ElusivWardenNetworkInstruction::register_basic_warden_instruction(
            0,
            config.clone(),
            WritableSignerAccount(test.payer()),
        ),
        &warden
    ).await;
}

#[tokio::test]
async fn test_update_state() {
    let mut test = start_test_with_setup().await;

    let mut warden = Actor::new(&mut test).await;
    register_warden(&mut test, &mut warden).await;

    let basic_warden_account = test.eager_account::<BasicWardenAccount, _>(Some(0)).await;
    let timestamp = basic_warden_account.warden.activation_timestamp;

    async fn set_timestamp(test: &mut ElusivProgramTest, timestamp: u64) {
        test.set_pda_account::<BasicWardenAccount, _>(&elusiv_warden_network::id(), None, Some(0), |data| {
            let mut account = BasicWardenAccount::new(data).unwrap();
            let mut warden = account.get_warden();
            warden.activation_timestamp = timestamp;
            account.set_warden(&warden);
        }).await;
    }

    // Invalid signer
    test.ix_should_fail_simple(
        ElusivWardenNetworkInstruction::update_basic_warden_state_instruction(
            0,
            true,
            SignerAccount(warden.pubkey),
        )
    ).await;
    test.ix_should_fail_simple(
        ElusivWardenNetworkInstruction::update_basic_warden_state_instruction(
            0,
            true,
            SignerAccount(test.payer()),
        )
    ).await;

    async fn set_state(test: &mut ElusivProgramTest, is_active: bool, warden: &Actor) {
        test.ix_should_succeed(
            ElusivWardenNetworkInstruction::update_basic_warden_state_instruction(
                0,
                is_active,
                SignerAccount(warden.pubkey),
            ),
            &[&warden.keypair],
        ).await;
    }

    set_timestamp(&mut test, 0).await;
    set_state(&mut test, true, &warden).await;

    let basic_warden_account = test.eager_account::<BasicWardenAccount, _>(Some(0)).await;
    assert!(basic_warden_account.warden.is_active);
    assert_eq!(basic_warden_account.warden.activation_timestamp, timestamp);

    set_timestamp(&mut test, 0).await;
    set_state(&mut test, false, &warden).await;

    let basic_warden_account = test.eager_account::<BasicWardenAccount, _>(Some(0)).await;
    assert!(!basic_warden_account.warden.is_active);
    assert_eq!(basic_warden_account.warden.activation_timestamp, timestamp);
    let timestamp = basic_warden_account.warden.activation_timestamp;

    // Same state can be set multiple times (but timestamp is unchanged)
    set_state(&mut test, false, &warden).await;
    set_state(&mut test, true, &warden).await;
    set_state(&mut test, true, &warden).await;

    let basic_warden_account = test.eager_account::<BasicWardenAccount, _>(Some(0)).await;
    assert_eq!(basic_warden_account.warden.activation_timestamp, timestamp);
}

#[tokio::test]
async fn test_update_lut() {
    let mut test = start_test_with_setup().await;

    let mut warden = Actor::new(&mut test).await;
    register_warden(&mut test, &mut warden).await;

    // Invalid signer
    test.ix_should_fail_simple(
        ElusivWardenNetworkInstruction::update_basic_warden_lut_instruction(
            0,
            SignerAccount(warden.pubkey),
            UserAccount(Pubkey::new_unique()),
        )
    ).await;
    test.ix_should_fail_simple(
        ElusivWardenNetworkInstruction::update_basic_warden_lut_instruction(
            0,
            SignerAccount(test.payer()),
            UserAccount(Pubkey::new_unique()),
        )
    ).await;

    async fn set_lut(test: &mut ElusivProgramTest, lut: Pubkey, warden: &Actor) {
        test.ix_should_succeed(
            ElusivWardenNetworkInstruction::update_basic_warden_lut_instruction(
                0,
                SignerAccount(warden.pubkey),
                UserAccount(lut),
            ),
            &[&warden.keypair],
        ).await;
    }

    // LUT is updated correctly
    let lut = Pubkey::new_unique();
    set_lut(&mut test, lut, &warden).await;

    let basic_warden_account = test.eager_account::<BasicWardenAccount, _>(Some(0)).await;
    assert_eq!(basic_warden_account.warden.lut, lut);

    // Multiple updates possible
    let lut = Pubkey::new_unique();
    set_lut(&mut test, lut, &warden).await;

    let basic_warden_account = test.eager_account::<BasicWardenAccount, _>(Some(0)).await;
    assert_eq!(basic_warden_account.warden.lut, lut);
}

#[tokio::test]
async fn test_open_stats_account() {
    let mut test = start_test_with_setup().await;

    let mut warden = Actor::new(&mut test).await;
    register_warden(&mut test, &mut warden).await;

    async fn open_stats_account(test: &mut ElusivProgramTest, warden: Pubkey, year: u16) {
        test.ix_should_succeed_simple(
            ElusivWardenNetworkInstruction::open_basic_warden_stats_account_instruction(
                year,
                UserAccount(warden),
                WritableSignerAccount(test.payer()),
            )
        ).await;
    }

    for year in 2022..2072 {
        open_stats_account(&mut test, warden.pubkey, year).await;

        let account = test.eager_account2::<BasicWardenStatsAccount, _>(warden.pubkey, Some(year as u32)).await;
        assert_eq!(account.year, year);
    }
}

#[tokio::test]
async fn test_track_stats() {
    let mut test = start_test_with_setup().await;

    let mut warden = Actor::new(&mut test).await;
    register_warden(&mut test, &mut warden).await;

    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let year = unix_timestamp_to_day_and_year(timestamp).unwrap().1;

    test.ix_should_succeed_simple(
        ElusivWardenNetworkInstruction::open_basic_warden_stats_account_instruction(
            year,
            UserAccount(warden.pubkey),
            WritableSignerAccount(test.payer()),
        )
    ).await;

    for ix in TRACKABLE_ELUSIV_INSTRUCTIONS {
        let mut accounts = Vec::new();
        for _ in 0..ix.warden_index {
            accounts.push(AccountMeta::new(Pubkey::new_unique(), false));
        }
        accounts.push(AccountMeta::new(warden.pubkey, true));

        // Invalid warden index
        let mut accounts_1 = accounts.clone();
        accounts_1.insert(0, AccountMeta::new(Pubkey::new_unique(), false));
        test.tx_should_fail(
            &[
                Instruction::new_with_bytes(ELUSIV_PROGRAM_ID, &[ix.instruction_id], accounts_1),
                ElusivWardenNetworkInstruction::track_basic_warden_stats_instruction(year, UserAccount(warden.pubkey)),
            ],
            &[&warden.keypair]
        ).await;

        // Invalid instruction id
        test.tx_should_fail(
            &[
                Instruction::new_with_bytes(ELUSIV_PROGRAM_ID, &[ix.instruction_id + 1], accounts.clone()),
                ElusivWardenNetworkInstruction::track_basic_warden_stats_instruction(year, UserAccount(warden.pubkey)),
            ],
            &[&warden.keypair]
        ).await;

        // Invalid program_id
        test.tx_should_fail(
            &[
                Instruction::new_with_bytes(OTHER_PROGRAM_ID, &[ix.instruction_id], accounts.clone()),
                ElusivWardenNetworkInstruction::track_basic_warden_stats_instruction(year, UserAccount(warden.pubkey)),
            ],
            &[&warden.keypair]
        ).await;

        // Invalid signer
        test.tx_should_fail_simple(
            &[
                Instruction::new_with_bytes(ELUSIV_PROGRAM_ID, &[ix.instruction_id + 1], accounts.clone()),
                ElusivWardenNetworkInstruction::track_basic_warden_stats_instruction(year, UserAccount(warden.pubkey)),
            ]
        ).await;

        test.tx_should_succeed(
            &[
                Instruction::new_with_bytes(ELUSIV_PROGRAM_ID, &[ix.instruction_id], accounts),
                ElusivWardenNetworkInstruction::track_basic_warden_stats_instruction(year, UserAccount(warden.pubkey)),
            ],
            &[&warden.keypair]
        ).await;
    }
}