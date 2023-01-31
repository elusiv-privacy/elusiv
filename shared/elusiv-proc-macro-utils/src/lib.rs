use proc_macro2::TokenStream;
use serde::{Deserialize, Serialize};
use solana_program::pubkey::Pubkey;
use std::{collections::HashMap, fs, str::FromStr};
use syn::{Field, Fields};

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
    testnet: String,
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
            program_id.testnet
        };

        map.insert(program_id.name, pubkey);
    }
    map
}

pub fn pda(pda_seed: &[u8]) -> (Pubkey, u8) {
    let program_id = Pubkey::from_str(&read_program_id("")).unwrap();
    Pubkey::find_program_address(&[pda_seed], &program_id)
}

pub fn pubkey_bytes(pubkey: &str) -> TokenStream {
    format!("{:?}", Pubkey::from_str(pubkey).unwrap().to_bytes())
        .parse()
        .unwrap()
}

/// Enforces that a field definition at a specific index matches the stream (visibility is ignored)
pub fn enforce_field(stream: TokenStream, index: usize, fields: &Fields) {
    let field = fields.iter().collect::<Vec<&Field>>()[index].clone();
    let ident = field.ident;
    let ty = field.ty;
    let expected = quote::quote! { #ident : #ty }.to_string();

    assert_eq!(
        expected,
        stream.to_string(),
        "Invalid field at {}. Exptected '{}', got '{}'",
        index,
        expected,
        stream
    );
}

/// Attempts to parse a String into a usize, ignoring any '_' character
pub fn try_parse_usize(source: &str) -> Option<usize> {
    let mut source = String::from(source);
    source.retain(|x| x != '_');
    match source.parse::<usize>() {
        Ok(u) => Some(u),
        Err(_) => None,
    }
}
