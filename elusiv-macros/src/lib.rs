extern crate proc_macro;

mod elusiv_account;
mod elusiv_instruction;
mod serde;
mod utils;

use syn::{ parse_macro_input, DeriveInput };
use elusiv_account::*;
use elusiv_instruction::*;
use serde::*;

/// Just-in-time mutable-byte-slice-backed serialization account
/// - every field is represented by a `&mut [u8]`
/// - every field has a setter (serialization) and getter (deserialization) function
#[proc_macro_attribute]
pub fn elusiv_account(args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_elusiv_account(&ast, args.into()).into()
}

/// Instructions account parsing, entrypoint creation
#[proc_macro_derive(ElusivInstruction, attributes(usr_inf, sig_inf, pda_acc, pda_mut, pda_inf, pda_arr, sys_inf))]
pub fn elusiv_instruction(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_elusiv_instruction(&ast).into()
}

#[proc_macro_derive(SerDe)]
pub fn serde(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_serde(&ast).into()
   // panic!("{}", impl_serde(&ast))
}