use elusiv_proc_macro_utils::{pubkey_bytes, read_program_id};
use proc_macro2::TokenStream;
use quote::quote;

pub fn impl_program_id(program_name: String) -> TokenStream {
    let id = pubkey_bytes(&read_program_id(&program_name));

    quote! {
        solana_program::pubkey::Pubkey::new_from_array(#id)
    }
}

pub fn impl_declare_program_id(program_name: String) -> TokenStream {
    let program_id = read_program_id(&program_name);
    let id_str = program_id.as_str();
    let id = pubkey_bytes(id_str);

    quote! {
        pub const PROGRAM_ID: solana_program::pubkey::Pubkey = solana_program::pubkey::Pubkey::new_from_array(#id);
        solana_program::declare_id!(#id_str);
    }
}
