use std::{fs, str::FromStr, collections::HashMap};
use proc_macro2::TokenStream;
use quote::quote;
use serde::{Serialize, Deserialize};
use solana_program::pubkey::Pubkey;

const ID_TOML_PATH: &str = "/../Id.toml";

#[derive(Serialize, Deserialize, Debug)]
struct Id {
    program_id: Vec<ProgramId>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ProgramId {
    name: String,
    mainnet: String,
    devnet: String,
    local: String,
}

pub fn impl_program_id(program_name: String) -> TokenStream {
    let program_id = read_program_id(&program_name);
    let id: TokenStream = format!("{:?}", Pubkey::from_str(&program_id).unwrap().to_bytes()).parse().unwrap();

    quote! {
        solana_program::pubkey::Pubkey::new_from_array(#id)
    }
}

pub fn impl_declare_program_id(program_name: String) -> TokenStream {
    let program_id = read_program_id(&program_name);
    let id_str = program_id.as_str();
    let id: TokenStream = format!("{:?}", Pubkey::from_str(id_str).unwrap().to_bytes()).parse().unwrap();

    quote! {
        pub const PROGRAM_ID: solana_program::pubkey::Pubkey = solana_program::pubkey::Pubkey::new_from_array(#id);
        solana_program::declare_id!(#id_str);
    }
}

pub fn read_program_id(program_name: &str) -> String {
    let program_ids = read_program_ids();

    if program_name.is_empty() {
        read_program_id(&std::env::var("CARGO_PKG_NAME").unwrap())
    } else {
        let id = program_ids.get(program_name).unwrap();
        id.clone()
    }
}

pub fn read_program_ids() -> HashMap<String, String> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let file_name = manifest_dir + ID_TOML_PATH;
    let contents = fs::read_to_string(file_name).unwrap();
    let id: Id = toml::from_str(&contents).unwrap();

    let mut map = HashMap::with_capacity(id.program_id.len());
    for program_id in id.program_id {
        let pubkey = if cfg!(feature = "mainnet") {
            program_id.mainnet
        } else if cfg!(feature = "devnet") {
            program_id.devnet
        } else {
            program_id.local
        };

        map.insert(program_id.name, pubkey);
    }
    map
}