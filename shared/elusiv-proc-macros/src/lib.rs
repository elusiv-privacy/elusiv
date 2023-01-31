extern crate proc_macro;

mod elusiv_account;
mod elusiv_hash_compute_units;
mod parse_tokens;
mod program_id;
mod repeat;
mod utils;

use elusiv_account::impl_elusiv_account;
use elusiv_hash_compute_units::impl_elusiv_hash_compute_units;
use parse_tokens::impl_parse_tokens;
use program_id::{impl_declare_program_id, impl_program_id};
use repeat::impl_repeat;
use syn::{parse_macro_input, DeriveInput};

/// Just-in-time mutable-byte-slice-backed serialization account
///
/// # Notes
///
/// Automatically also derives [`elusiv_types::PDAAccount`]
#[proc_macro_attribute]
pub fn elusiv_account(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_elusiv_account(&ast, args.into()).into()
}

/// Creates a struct `Name` that implements `elusiv_computation::PartialComputation`
///
/// # Usage
/// - `elusiv_hash_compute_units!(<name>, <NUMBER_OF_HASHES>)`
#[proc_macro]
pub fn elusiv_hash_compute_units(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    impl_elusiv_hash_compute_units(input.into()).into()
}

/// Repeates an expression count times
///
/// # Usage
///
/// - `repeat!({<<expr>>}, <<count>>)`
/// - use `_index` inside of `<<expr>>` to get the current index of the loop
#[proc_macro]
pub fn repeat(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    impl_repeat(input.into()).into()
}

/// Parses `Token.toml`
#[proc_macro]
pub fn elusiv_tokens(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    impl_parse_tokens().into()
}

/// Parses `Id.toml` and returns a const [`solana_program::pubkey::Pubkey`]
///
/// # Usage
///
/// Provide the name of the program as argument.
/// If no name is supplied, the runtime value of `CARGO_PKG_NAME` will be used as fallback.
///
/// # Example
///
/// ```
/// const ELUSIV_PROGRAM_ID: solana_program::pubkey::Pubkey = program_id!(elusiv);
/// const ELUSIV_2_PROGRAM_ID: solana_program::pubkey::Pubkey = program_id!(elusiv-2);
/// ```
#[proc_macro]
pub fn program_id(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    impl_program_id(input.to_string()).into()
}

/// Parses `Id.toml` and implements [`solana_program::declare_id`]
///
/// # Usage
///
/// Provide the name of the program as argument.
/// If no name is supplied, the runtime value of `CARGO_PKG_NAME` will be used as fallback.
///
/// # Example
///
/// ```
/// declare_program_id!(elusiv);
/// declare_program_id!(elusiv-2);
/// ```
#[proc_macro]
pub fn declare_program_id(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    impl_declare_program_id(input.to_string()).into()
}
