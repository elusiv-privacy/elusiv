use std::str::FromStr;
use proc_macro2::TokenStream;
use quote::quote;
use solana_program::pubkey::Pubkey;

pub fn impl_program_id(input: TokenStream) -> TokenStream {
    let pk_str = input.to_string();
    let pk_str = &pk_str.as_str()[1..pk_str.len() - 1];
    let id: TokenStream = format!("{:?}", Pubkey::from_str(pk_str).unwrap().to_bytes()).parse().unwrap();

    quote! {
        pub const PROGRAM_ID: solana_program::pubkey::Pubkey = solana_program::pubkey::Pubkey::new_from_array(#id);
        solana_program::declare_id!(#pk_str);
    }
}