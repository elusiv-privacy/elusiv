extern crate proc_macro;

mod elusiv_account;
mod available_types;
mod account;
mod elusiv_instruction;
mod utils;

use proc_macro2::TokenStream;
use syn::{ parse_macro_input, DeriveInput };
use elusiv_account::*;
use elusiv_instruction::*;
use account::*;

/// Just-in-time mutable-byte-slice-backed serialization account
/// ### Attributes
/// * `lazy_option` - Option<T> with default value to None
/// * `lazy_stack(size, bytes, serialize, deserialize)`
/// * `queue(size, bytes, serialize, deserialize)`
/// ### Basic types
/// * `u64` - 
/// * `[U256]` - 
/// * `G1Affine` - 
/// * `G2Affine` - 
#[proc_macro_derive(ElusivAccount, attributes(lazy_option, proof_request, lazy_stack, queue))]
pub fn elusiv_account(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_elusiv_account(&ast).into()
}

/// Removes the complete TokenStream
#[proc_macro_attribute]
pub fn remove_original_implementation(_args: proc_macro::TokenStream, _input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    proc_macro::TokenStream::new()
}

/// Adds unwrap implementations to Instruction enums
#[proc_macro_derive(ElusivInstruction)]
pub fn elusiv_instruction(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_elusiv_instruction(&ast).into()
}

/// Brings program accounts or Account Info's into scope
/// ### Usage
/// - account!(ident, role)
/// - no role (program accounts):
///     - Storage
///     - Queue
///     - Commitment
///     - Proof
/// - signer:
///     - Sender
///     - Relayer
///     - Cranker
/// - pool
/// - no_check
#[proc_macro]
pub fn account(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    impl_account(&input).into()
}

#[proc_macro]
pub fn pubkey(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let id: TokenStream = input.into();
    quote::quote! {
        pub const ID: solana_program::pubkey::Pubkey = solana_program::pubkey!(#id);
    }.into()
}