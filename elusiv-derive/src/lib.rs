extern crate proc_macro;

mod elusiv_instruction;
mod borsh_serde_sized;
mod enum_variant;
mod utils;

use syn::{ parse_macro_input, DeriveInput };
use elusiv_instruction::*;
use borsh_serde_sized::*;
use enum_variant::*;

/// Instructions account parsing
/// 
/// # Account attributes
/// - Each enum variant (instruction) can require accounts
/// - Specify accounts using attributes with the following syntax:
/// `#[type(name, Type, pda_offset = .., key = .., [ signer, writable, multi_accounts, account_info, no_sub_account_check ])]`
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
///         - `find_pda`: does a PDA verification with a pda_offset but with unknown runtime, since no bump is supplied (used for reting new PDAs)
///         - `multi_accounts`: the `Type` has to implement the `crate::state::program_account::MultiAccountAccount` trait and `Type::COUNT + 1` accounts will be required
///         - `account_info`: returns an `AccountInfo` object (only relevant for PDAs)
///         - `no_sub_account_check`: **SKIPS THE PUBKEY VERIFICATION of the sub-accounts (ONLY TO BE USED WHEN CREATING A NEW ACCOUNT!)**
///         - `ignore_sub_accounts`: ignores all sub-accounts of a multi-account
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
#[proc_macro_derive(ElusivInstruction, attributes(acc, sys, pda, prg))]
pub fn elusiv_instruction(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_elusiv_instruction(&ast).into()
}

#[proc_macro_derive(BorshSerDeSized)]
pub fn borsh_serde_sized(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_borsh_serde_sized(&ast).into()
}

#[proc_macro_derive(EnumVariantIndex)]
pub fn enum_variant_index(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_enum_variant_index(&ast).into()
}