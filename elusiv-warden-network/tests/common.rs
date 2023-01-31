#![allow(unused_macros)]
#![allow(dead_code)]

pub use elusiv_test::*;
use elusiv_types::{WritableSignerAccount, TOKENS};
use elusiv_warden_network::{
    instruction::ElusivWardenNetworkInstruction,
    warden::{
        BasicWardenFeatures, ElusivBasicWardenConfig, Timezone, WardenFeatures, WardenRegion,
        WardensAccount,
    },
};
use solana_program::pubkey::Pubkey;
use std::net::Ipv4Addr;

pub const ELUSIV_PROGRAM_ID: Pubkey = elusiv_proc_macros::program_id!(elusiv);
pub const OTHER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32,
]);

pub async fn start_test() -> ElusivProgramTest {
    compile_mock_program();

    ElusivProgramTest::start(&[
        (
            String::from("elusiv_warden_network"),
            elusiv_warden_network::id(),
            solana_program_test::processor!(elusiv_warden_network::process_instruction),
        ),
        (
            String::from("mock_program"),
            ELUSIV_PROGRAM_ID,
            solana_program_test::processor!(mock_program::process_instruction),
        ),
        (
            String::from("mock_program"),
            OTHER_PROGRAM_ID,
            solana_program_test::processor!(mock_program::process_instruction),
        ),
    ])
    .await
}

pub async fn start_test_with_setup() -> ElusivProgramTest {
    let mut test = start_test().await;

    test.ix_should_succeed_simple(ElusivWardenNetworkInstruction::init_instruction(
        WritableSignerAccount(test.payer()),
    ))
    .await;

    test
}

pub async fn register_warden(test: &mut ElusivProgramTest, warden: &mut Actor) {
    let warden_id = test
        .eager_account::<WardensAccount, _>(None)
        .await
        .next_warden_id;

    test.ix_should_succeed(
        ElusivWardenNetworkInstruction::register_basic_warden_instruction(
            warden_id,
            ElusivBasicWardenConfig {
                ident: String::new().try_into().unwrap(),
                key: warden.pubkey,
                operator: None.into(),
                addr: Ipv4Addr::new(0, 0, 0, 0),
                rpc_port: 0,
                uses_proxy: false,
                tls_mode: elusiv_warden_network::warden::TlsMode::NoTls,
                jurisdiction: 0,
                timezone: Timezone {
                    area: 0,
                    location: String::new().try_into().unwrap(),
                },
                region: WardenRegion::America,
                version: [0, 0, 0],
                platform: String::new().try_into().unwrap(),
                warden_features: WardenFeatures::default(),
                basic_warden_features: BasicWardenFeatures::default(),
                tokens: [false; TOKENS.len()],
            },
            WritableSignerAccount(warden.pubkey),
        ),
        &[&warden.keypair],
    )
    .await;
}
