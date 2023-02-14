extern crate proc_macro;

mod borsh_serde_sized;
mod elusiv_instruction;
mod enum_variant;
mod jit;
mod pda_account;
mod utils;

use syn::{parse_macro_input, DeriveInput};

use borsh_serde_sized::*;
use elusiv_instruction::*;
use enum_variant::*;
use jit::*;
use pda_account::*;

/// Instructions parsing
///
/// # Account attributes
/// - Each enum variant (instruction) can require accounts
/// - Specify accounts using attributes with the following syntax:
/// `#[type(name, Type, pda_offset = .., key = .., [ signer, writable, include_child_accounts, account_info ])]`
/// - with:
///     - type:
///         - `acc`: user accounts or any `AccountInfo` that has no basic checks
///         - `prg`
///         - `sys`: a different program (most likely system program) (requires the key = .. field)
///         - `pda`
///     - name: name of the variable
///     - Type: if the account isa `PDAAccount`, specify it's type (even when using the account as an `AccountInfo`, since we need to check the PDA seed)
///     - fields:
///         - `pda_offset`: you can specify fields contained in the data of previous account or the instruction itself
///         - `key`: address of the program (`sys`)
///     - extra_attributes:
///         - `signer`
///         - `writable`
///         - `find_pda`: does a PDA verification with a pda_offset but with unknown runtime, since no bump is supplied (used for renting new PDAs)
///         - `skip_pda_verification`: skips the PDA verification, resulting in an [`UnverifiedAccountInfo`] (requires the `account_info` attribute)
///         - `account_info`: returns an `AccountInfo` object (only relevant for PDAs)
///         - `include_child_accounts`: the `Type` has to implement the `crate::state::program_account::ParentAccount` trait and up to `Type::COUNT + 1` accounts can be matched (but at least 1)
///         - `skip_abi`: can be used to add manual pda_offsets in the abi
///
/// # Other attributes
/// - Each variant can also be equipped with any other kind of attributes (cfg or do documentation).
/// - Documentation can either be added using the `doc` attribute or with the normal syntax.
/// - The only restriction is that docs need to be first, followed by any kind of attr and then the account attrs.
///
/// # Usage
/// ```
/// #[derive(ElusivInstruction)]
/// pub enum ElusivInstruction {
///     #[pda(account_name, AccountType, pda_offset = field_one, [ writable ])]
///     InstructionOne {
///         field_one: u64,
///     }
/// }
/// ```
#[proc_macro_derive(ElusivInstruction, attributes(acc, sys, pda, map))]
pub fn elusiv_instruction(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_elusiv_instruction(&ast).into()
}

#[proc_macro_derive(BorshSerDeSized)]
pub fn borsh_serde_sized(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_borsh_serde_sized(&ast).into()
}

#[proc_macro_derive(BorshSerDePlaceholder)]
pub fn borsh_placeholder(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_borsh_serde_placeholder(&ast).into()
}

#[proc_macro_derive(EnumVariantIndex)]
pub fn enum_variant_index(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_enum_variant_index(&ast).into()
}

/// Derives the [`PDAAccount`] trait
#[proc_macro_derive(PDAAccount)]
pub fn pda_account(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_pda_account(&ast).into()
}

#[proc_macro_derive(ByteBackedJIT)]
pub fn jit(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_byte_backed_jit(&ast).into()
}
