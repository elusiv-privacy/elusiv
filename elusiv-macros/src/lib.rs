extern crate proc_macro;

mod elusiv_account;
mod elusiv_instruction;
mod borsh_serde_sized;
mod utils;

use syn::{ parse_macro_input, DeriveInput };
use elusiv_account::*;
use elusiv_instruction::*;
use borsh_serde_sized::*;

/// Just-in-time mutable-byte-slice-backed serialization account
/// - every field is represented by a `&mut [u8]`
/// - every field has a setter (serialization) and getter (deserialization) function
#[proc_macro_attribute]
pub fn elusiv_account(args: proc_macro::TokenStream, input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_elusiv_account(&ast, args.into()).into()
}

/// Instructions account parsing
/// 
/// # Account attributes
/// - Each enum variant (instruction) can require accounts
/// - Specify accounts using attributes with the following syntax:
/// `#[type(name, Type, pda_offset = .., key = .., [ signer, writable, multi_accounts, account_info, no_subaccount_check ])]`
/// - with:
///     - type:
///         - `usr`: user accounts or any `AccountInfo` that has no basic checks
///         - `prg`
///         - `sys`: a different program (most likely system program) (requires the key = .. field)
///         - `pda`
///     - name: name of the variable
///     - Type: if the account isa `PDAAccount`, specify it's type (even when using the account as an `AccountInfo`, since we need to check the PDA seed)
///     - fields:
///         - `pda_offset`: you can specify fields contained in the data of previous account or the instruction itself
///         - `key`: address of the program (`sys`)
///     - extra_attributes (always in the following order)
///         - `signer`
///         - `writable`
///         - `multi_accounts`: the `Type` has to implement the `crate::state::program_account::MultiAccountAccount` trait and `Type::COUNT + 1` accounts will be required
///         - `account_info`: returns an `AccountInfo` object (only relevant for PDAs)
///         - `no_subaccount_check`: **SKIPS THE PUBKEY VERIFICATION of the subaccounts (ONLY TO BE USED WHEN CREATING A NEW ACCOUNT!)**
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
#[proc_macro_derive(ElusivInstruction, attributes(usr, sys, pda, prg))]
pub fn elusiv_instruction(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_elusiv_instruction(&ast).into()
}

#[proc_macro_derive(BorshSerDeSized)]
pub fn borsh_serde_sized(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    impl_borsh_serde_sized(&ast).into()
}