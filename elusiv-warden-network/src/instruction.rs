#![allow(clippy::large_enum_variant)]

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::system_program;
use crate::network::ElusivBasicWardenNetworkAccount;
use crate::warden::{
    ElusivWardenID,
    ElusivBasicWardenConfig,
    ElusivWardensAccount,
    ElusivBasicWardenAccount,
};
use crate::macros::ElusivInstruction;
use crate::processor;

#[derive(BorshDeserialize, BorshSerialize, ElusivInstruction)]
pub enum ElusivWardenNetworkInstruction {
    // -------- Program initialization --------

    #[acc(payer, { signer, writable })]
    #[pda(wardens, ElusivWardensAccount, { writable, find_pda, account_info })]
    #[pda(basic_network, ElusivBasicWardenNetworkAccount, { writable, find_pda, account_info })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    Init,

    // -------- Basic Warden --------

    #[acc(warden, { signer, writable })]
    #[pda(warden_account, ElusivBasicWardenAccount, pda_offset = Some(warden_id), { writable, find_pda, account_info })]
    #[pda(wardens, ElusivWardensAccount, { writable })]
    #[pda(basic_network, ElusivBasicWardenNetworkAccount, { writable })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    RegisterBasicWarden {
        warden_id: ElusivWardenID,
        config: ElusivBasicWardenConfig,
    },

    #[acc(warden, { signer })]
    #[pda(warden_account, ElusivBasicWardenAccount, pda_offset = Some(warden_id), { writable })]
    UpdateBasicWardenState {
        warden_id: ElusivWardenID,
        is_active: bool,
    },

    #[acc(warden, { signer })]
    #[pda(warden_account, ElusivBasicWardenAccount, pda_offset = Some(warden_id), { writable })]
    #[acc(lut_account)]
    UpdateBasicWardenLut {
        warden_id: ElusivWardenID,
    },

    #[acc(warden, { signer, writable })]
    #[pda(warden_account, ElusivBasicWardenAccount, pda_offset = Some(warden_id), { writable, account_info })]
    #[sys(system_program, key = system_program::ID, { ignore })]
    CloseBasicWarden {
        warden_id: ElusivWardenID,
    },
}