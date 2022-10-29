use std::{fs, str::FromStr, env};
use proc_macro2::TokenStream;
use quote::quote;
use serde::{Serialize, Deserialize};
use solana_program::pubkey::Pubkey;

pub fn impl_program_id(ident: TokenStream) -> TokenStream {
    // Set 'PROGRAM_ID_<IDENT>' to override the program-id
    let custom_program_id_env_var = format!("PROGRAM_ID_{}", ident.to_string().to_uppercase());
    let id_str = match env::var(custom_program_id_env_var) {
        Ok(s) => s,
        Err(_) => read_program_id(),
    };

    let id_str = id_str.as_str();
    let id: TokenStream = format!("{:?}", Pubkey::from_str(id_str).unwrap().to_bytes()).parse().unwrap();

    quote! {
        pub const PROGRAM_ID: solana_program::pubkey::Pubkey = solana_program::pubkey::Pubkey::new_from_array(#id);
        solana_program::declare_id!(#id_str);
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Elusiv {
    program_id: String,
}

pub fn read_program_id() -> String {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let file_name = manifest_dir + "/Id.toml";
    let contents = fs::read_to_string(&file_name).unwrap();
    let id: Elusiv = toml::from_str(&contents).unwrap();
    id.program_id
}