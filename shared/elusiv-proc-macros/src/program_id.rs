use std::{fs, str::FromStr};
use proc_macro2::TokenStream;
use quote::quote;
use serde::{Serialize, Deserialize};
use solana_program::pubkey::Pubkey;

#[derive(Serialize, Deserialize, Debug)]
struct Elusiv {
    program_id: ProgramId,
}

#[derive(Serialize, Deserialize, Debug)]
struct ProgramId {
    mainnet: String,
    devnet: String,
    local: String,
}

pub fn impl_program_id() -> TokenStream {
    let id_str = read_program_id();

    let id_str = id_str.as_str();
    let id: TokenStream = format!("{:?}", Pubkey::from_str(id_str).unwrap().to_bytes()).parse().unwrap();

    quote! {
        pub const PROGRAM_ID: solana_program::pubkey::Pubkey = solana_program::pubkey::Pubkey::new_from_array(#id);
        solana_program::declare_id!(#id_str);
    }
}

fn read_program_id() -> String {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let file_name = manifest_dir + "/Id.toml";
    let contents = fs::read_to_string(&file_name).unwrap();
    let id: Elusiv = toml::from_str(&contents).unwrap();

    if cfg!(feature = "mainnet") {
        id.program_id.mainnet
    } else if cfg!(feature = "devnet") {
        id.program_id.devnet
    } else {
        id.program_id.local
    }
}