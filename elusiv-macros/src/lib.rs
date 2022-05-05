extern crate proc_macro;

mod elusiv_account;
mod elusiv_instruction;
mod utils;

use proc_macro2::TokenStream;
use syn::{ parse_macro_input, DeriveInput };
use elusiv_account::*;
use elusiv_instruction::*;

/// Just-in-time mutable-byte-slice-backed serialization account
/// - every field is represented by a `&mut [u8]`
/// - every field has a setter (serialization) and getter (deserialization) function
#[proc_macro_attribute]
pub fn elusiv_account(args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_elusiv_account(&ast, args.into()).into()
    //panic!("{}", impl_elusiv_account(&ast, args.into()))
}

/// Removes the complete TokenStream
#[proc_macro_attribute]
pub fn remove_token_stream(_args: proc_macro::TokenStream, _input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::new()
}

/// Instructions account parsing, entrypoint creation
#[proc_macro_derive(ElusivInstruction, attributes(usr, sig, prg, pda, arr, sys))]
pub fn elusiv_instruction(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_elusiv_instruction(&ast).into()
}

/// Creates a constant `ID: Pubkey`
#[proc_macro]
pub fn pubkey(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let id: TokenStream = input.into();
    quote::quote! {
        pub const ID: solana_program::pubkey::Pubkey = solana_program::pubkey!(#id);
    }.into()
}