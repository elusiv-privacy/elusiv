use std::str::FromStr;

use super::utils::upper_camel_to_upper_snake;
use proc_macro2::{TokenStream, TokenTree};
use quote::{quote, ToTokens};

const ACC_ATTR: &str = "acc";
const SYS_ATTR: &str = "sys";
const PDA_ATTR: &str = "pda";
const MAP_ATTR: &str = "map";

const RESERVED_ATTR_IDENTS: [&str; 4] = [ACC_ATTR, SYS_ATTR, PDA_ATTR, MAP_ATTR];

enum AttrType {
    Docs,
    Any,
    Account,
}

pub fn impl_elusiv_instruction(ast: &syn::DeriveInput) -> proc_macro2::TokenStream {
    let ast_ident = &ast.ident;

    let mut matches = quote!();
    let mut functions = quote!();
    let mut abi_functions = quote!();
    let mut variant_indices = quote!();

    if let syn::Data::Enum(e) = &ast.data {
        for (var_index, var) in e.variants.clone().iter().enumerate() {
            let ident = &var.ident;
            let name = upper_camel_to_upper_snake(&ident.to_string()).to_lowercase();
            let fn_name_abi: TokenStream = format!("{name}_instruction").parse().unwrap();
            let fn_name: TokenStream = name.parse().unwrap();

            let var_index_name: TokenStream =
                format!("{}_INDEX", name.to_uppercase()).parse().unwrap();
            let var_index = var_index as u8;
            variant_indices.extend(quote! {
                pub const #var_index_name: u8 = #var_index;
            });

            // Processor calls
            let mut accounts = quote!();
            let mut fields = quote!();
            let mut signature = quote!();

            // Instruction creation
            let mut fields_with_type = quote!();
            let mut user_accounts = quote!();
            let mut instruction_accounts = quote!();

            let mut docs = quote!();
            let mut other_attrs = quote!();
            let mut current_attr_type = AttrType::Docs;

            for field in &var.fields {
                let field_name = field.ident.clone().unwrap();
                let ty = field.ty.clone();

                fields.extend(quote! { #field_name, });
                fields_with_type.extend(quote! { #field_name: #ty, });
            }

            // Account attributes
            for (_, attr) in var.attrs.iter().enumerate() {
                let attr_name = attr.path.get_ident().unwrap().to_string();

                // No `ElusivInstruction` specific attribute
                if !RESERVED_ATTR_IDENTS.contains(&attr_name.as_str()) {
                    if attr_name == "doc" {
                        assert!(
                            matches!(current_attr_type, AttrType::Docs),
                            "Invalid attribute order"
                        );

                        docs.extend(docs.to_token_stream());
                    } else {
                        assert!(
                            matches!(current_attr_type, AttrType::Docs | AttrType::Any),
                            "Invalid attribute order"
                        );

                        current_attr_type = AttrType::Any;
                        other_attrs.extend(attr.to_token_stream());
                    }

                    continue;
                }

                current_attr_type = AttrType::Account;

                // Sub-attrs are the fields as in #[usr(sub_attr_0 = .., sub_attr_1, .., { sub_attr_n, .. })] (braces are ignored)
                let mut fields = attr.tokens.to_string();
                fields.retain(|x| x != '{' && x != '}' && !x.is_whitespace());
                let mut sub_attrs = Vec::new();
                let mut sub_attr = String::new();
                for token in fields[1..fields.len() - 1]
                    .parse::<TokenStream>()
                    .unwrap()
                    .into_iter()
                {
                    if let TokenTree::Punct(punct) = &token {
                        if punct.to_string() == "," {
                            sub_attrs.push(sub_attr);
                            sub_attr = String::new();
                            continue;
                        }
                    }

                    sub_attr.push_str(&token.to_string());
                }
                sub_attrs.push(sub_attr);
                let sub_attrs: Vec<(&str, Option<&str>)> = sub_attrs
                    .iter()
                    .map(|s| {
                        let split: Vec<&str> = s.split('=').collect();
                        match split.len() {
                            1 => (split[0], None),
                            2 => (split[0], Some(split[1])),
                            _ => panic!("Invalid attribute found ('{}')", s),
                        }
                    })
                    .collect();

                let mut account: TokenStream = sub_attrs[0].0.parse().unwrap();
                let mut account_init = Vec::new(); // used for creating the instruction objects with the abi-feature

                accounts.extend(quote! {
                    let #account = &solana_program::account_info::next_account_info(account_info_iter)?;
                });

                // Signer check
                let is_signer = contains_key(&sub_attrs, "signer");
                if is_signer {
                    accounts.extend(quote!{
                        if !#account.is_signer { return Err(solana_program::program_error::ProgramError::MissingRequiredSignature) }
                    });
                }

                // Writable check
                let is_writable = contains_key(&sub_attrs, "writable");
                /*if is_writable {
                    accounts.extend(quote!{
                        if !#account.is_writable { return Err(solana_program::program_error::ProgramError::InvalidArgument) }
                    });
                }*/

                // Ownership check
                let is_owned = contains_key(&sub_attrs, "owned");
                if is_owned {
                    accounts.extend(quote!{
                        if *#account.owner != crate::ID { return Err(solana_program::program_error::ProgramError::IllegalOwner) }
                    });
                }

                // Ignore means not passing the account to the processor function
                let ignore = contains_key(&sub_attrs, "ignore");

                // `AccountInfo`?
                let as_account_info = contains_key(&sub_attrs, "account_info");

                let mut_token = if is_writable {
                    quote! { mut }
                } else {
                    quote! {}
                };
                let account_init_fn = if is_writable {
                    quote! { new }
                } else {
                    quote! { new_readonly }
                };

                let user_account_type = if is_signer {
                    if is_writable {
                        quote! { WritableSignerAccount }
                    } else {
                        quote! { SignerAccount }
                    }
                } else if is_writable {
                    quote! { WritableUserAccount }
                } else {
                    quote! { UserAccount }
                };

                match attr_name.as_str() {
                    // `AccountInfo` (usage: <name>)
                    ACC_ATTR => {
                        user_accounts.extend(quote! { #account: #user_account_type, });
                        account_init.push(quote!{
                            accounts.push(solana_program::instruction::AccountMeta::#account_init_fn(#account.0, #is_signer));
                        });
                    }

                    // System program `AccountInfo` (usage: <name> <key = ..>)
                    SYS_ATTR => {
                        // Check that system program pubkey is correct (for this we have a field `key` that the pubkey gets compared to)
                        let key: TokenStream =
                            value::<String>(&sub_attrs, "key").unwrap().parse().unwrap();

                        accounts.extend(quote!{
                            if #key != *#account.key { return Err(solana_program::program_error::ProgramError::InvalidArgument) };
                        });

                        account_init.push(quote!{
                            accounts.push(solana_program::instruction::AccountMeta::#account_init_fn(#key, #is_signer));
                        });
                    }

                    // PDA accounts (usage: <name> <AccountType> <pda_offset: u32 = ..>? <account_info>? <include_child_accounts>? <ownership>)
                    PDA_ATTR => {
                        // Every PDA account needs to implement the trait `elusiv::state::program_account::PDAAccount`
                        // - this trait allows us to verify PDAs
                        // - this allows us to define `ParentAccount`s, which are a single main PDA account with `COUNT` child-accounts
                        // - the seed of the main account plus the index of each child-account is used to generate their PDAs

                        // The PDA account type
                        let ty: TokenStream = String::from(sub_attrs[1].0).parse().unwrap();

                        // The PDA offset is an optional field, used to add an offset to the seed
                        let pda_offset: TokenStream =
                            value(&sub_attrs, "pda_offset").unwrap_or(quote!(None));

                        // The PDA pubkey is an optional field, used to add an associated pubkey to the seed
                        let pda_pubkey: Option<TokenStream> = value(&sub_attrs, "pda_pubkey");

                        // ParentAccount?
                        let include_child_accounts =
                            contains_key(&sub_attrs, "include_child_accounts");

                        let skip_abi = contains_key(&sub_attrs, "skip_abi");
                        if skip_abi {
                            let offset_ident: TokenStream =
                                format!("{}_pda_offset", sub_attrs[0].0).parse().unwrap();

                            if pda_pubkey.is_some() {
                                let pubkey_ident: TokenStream =
                                    format!("{}_pda_pubkey", sub_attrs[0].0).parse().unwrap();

                                user_accounts.extend(quote!{ #pubkey_ident: solana_program::pubkey::Pubkey, #offset_ident: Option<u32>, });
                                account_init.push(quote!{
                                    accounts.push(
                                        solana_program::instruction::AccountMeta::#account_init_fn(
                                            <#ty as elusiv_types::accounts::PDAAccount>::find_with_pubkey(#pubkey_ident, #offset_ident).0,
                                            #is_signer
                                        )
                                    );
                                });
                            } else {
                                user_accounts.extend(quote! { #offset_ident: Option<u32>, });
                                account_init.push(quote!{
                                    accounts.push(
                                        solana_program::instruction::AccountMeta::#account_init_fn(
                                            <#ty as elusiv_types::accounts::PDAAccount>::find(#offset_ident).0,
                                            #is_signer
                                        )
                                    );
                                });
                            }
                        } else if let Some(pda_pubkey) = pda_pubkey.clone() {
                            account_init.push(quote!{
                                accounts.push(
                                    solana_program::instruction::AccountMeta::#account_init_fn(
                                        <#ty as elusiv_types::accounts::PDAAccount>::find_with_pubkey(#pda_pubkey, #pda_offset).0,
                                        #is_signer
                                    )
                                );
                            });
                        } else {
                            account_init.push(quote!{
                                accounts.push(
                                    solana_program::instruction::AccountMeta::#account_init_fn(
                                        <#ty as elusiv_types::accounts::PDAAccount>::find(#pda_offset).0,
                                        #is_signer
                                    )
                                );
                            });
                        }

                        // PDA verification
                        let find_pda = contains_key(&sub_attrs, "find_pda"); // does not read the bump byte from the account data

                        // Skips the PDA verification
                        let skip_pda_verification =
                            contains_key(&sub_attrs, "skip_pda_verification");

                        #[allow(clippy::collapsible_else_if)]
                        if !skip_pda_verification {
                            let check_pda = if find_pda {
                                if let Some(pda_pubkey) = pda_pubkey {
                                    quote! {
                                        if <#ty as elusiv_types::accounts::PDAAccount>::find_with_pubkey(#pda_pubkey, #pda_offset).0 != *#account.key {
                                            return Err(solana_program::program_error::ProgramError::InvalidArgument)
                                        }
                                    }
                                } else {
                                    quote! {
                                        if <#ty as elusiv_types::accounts::PDAAccount>::find(#pda_offset).0 != *#account.key {
                                            return Err(solana_program::program_error::ProgramError::InvalidArgument)
                                        }
                                    }
                                }
                            } else {
                                if let Some(pda_pubkey) = pda_pubkey {
                                    quote! {
                                        <#ty as elusiv_types::accounts::PDAAccount>::verify_account_with_pubkey(&#account, #pda_pubkey, #pda_offset)?;
                                    }
                                } else {
                                    quote! {
                                        <#ty as elusiv_types::accounts::PDAAccount>::verify_account(&#account, #pda_offset)?;
                                    }
                                }
                            };
                            accounts.extend(check_pda);
                        }

                        if include_child_accounts {
                            // ParentAccount with arbitrary number of child-accounts
                            accounts.extend(quote!{
                                let acc_data = &mut #account.data.borrow_mut()[..];
                                let mut #account = <#ty as elusiv_types::accounts::ProgramAccount>::new(acc_data)?;

                                let child_accounts = <#ty as elusiv_types::accounts::ParentAccount>::find_child_accounts(
                                    &#account,
                                    &crate::ID,
                                    #is_writable,
                                    account_info_iter,
                                )?;
                            });

                            user_accounts.extend(quote! { #account: &[#user_account_type], });
                            account_init.push(quote!{
                                for account in #account {
                                    accounts.push(solana_program::instruction::AccountMeta::#account_init_fn(account.0, #is_signer));
                                }
                            });

                            if as_account_info {
                                accounts.extend(quote! {
                                    accounts.insert(0, #account);
                                    let #account = accounts;
                                });
                                account = quote! { #account };
                            } else if is_writable {
                                accounts.extend(quote!{ <#ty as elusiv_types::accounts::ParentAccount>::set_child_accounts(&mut #account, child_accounts); });
                                account = quote! { &mut #account };
                            } else {
                                accounts.extend(quote!{ <#ty as elusiv_types::accounts::ParentAccount>::set_child_accounts(&mut #account, child_accounts); });
                                account = quote! { &#account };
                            }
                        } else if as_account_info {
                            account = quote! { &#account };
                        } else if is_writable {
                            accounts.extend(quote!{
                                let acc_data = &mut #account.data.borrow_mut()[..];
                                let #mut_token #account = <#ty as elusiv_types::accounts::ProgramAccount>::new(acc_data)?;
                            });
                            account = quote! { &mut #account };
                        } else {
                            accounts.extend(quote!{
                                let acc_data = &mut #account.data.borrow_mut()[..];
                                let #mut_token #account = <#ty as elusiv_types::accounts::ProgramAccount>::new(acc_data)?;
                            });
                            account = quote! { &#account };
                        }

                        if skip_pda_verification {
                            assert!(
                                as_account_info,
                                "'skip_pda_verification' needs to be used with 'account_info'"
                            );

                            account = quote! {
                                elusiv_types::accounts::UnverifiedAccountInfo::new(&#account)
                            }
                        }
                    }

                    // MapPDAAccount's
                    MAP_ATTR => {
                        todo!()
                    }

                    v => panic!("Invalid attribute name {}", v),
                }

                // Add account to processor call signature
                if !ignore {
                    signature.extend(quote! { #account, });
                }

                // Add account init
                instruction_accounts.extend(
                    account_init
                        .iter()
                        .fold(quote! {}, |acc, x| quote! { #acc #x }),
                );
            }

            matches.extend(quote! {
                #other_attrs
                #ast_ident::#ident { #fields } => {
                    Self::#fn_name(accounts, #fields)
                },
            });

            functions.extend(quote!{
                #docs
                #other_attrs
                fn #fn_name(accounts: &[solana_program::account_info::AccountInfo], #fields_with_type) -> solana_program::entrypoint::ProgramResult {
                    let mut account_info_iter = &mut accounts.iter();
                    #accounts
                    processor::#fn_name(#signature #fields)
                }
            });

            abi_functions.extend(quote!{
                #docs
                #other_attrs
                pub fn #fn_name_abi(#fields_with_type #user_accounts) -> solana_program::instruction::Instruction {
                    let mut accounts = Vec::new();

                    #instruction_accounts
                    let data = #ast_ident::#ident { #fields };
                    let data = #ast_ident::try_to_vec(&data).unwrap();

                    solana_program::instruction::Instruction::new_with_bytes(
                        crate::id(),
                        &data,
                        accounts,
                    )
                }
            });
        }

        quote! {
            impl #ast_ident {
                pub fn process(program_id: &solana_program::pubkey::Pubkey, accounts: &[solana_program::account_info::AccountInfo], instruction: #ast_ident) -> solana_program::entrypoint::ProgramResult {
                    match instruction {
                        #matches
                        _ => { Err(solana_program::program_error::ProgramError::InvalidInstructionData) }
                    }
                }

                #functions

                #variant_indices
            }

            #[cfg(feature = "elusiv-client")]
            impl #ast_ident {
                #abi_functions
            }

        }
    } else {
        panic!("Only enums can be instructions")
    }
}

fn contains_key(attrs: &[(&str, Option<&str>)], key: &str) -> bool {
    attrs.iter().any(|(k, _)| *k == key)
}

fn value<T: FromStr>(attrs: &[(&str, Option<&str>)], key: &str) -> Option<T> {
    let v = attrs.iter().find(|(k, _)| *k == key)?.1?;
    match T::from_str(v) {
        Ok(v) => Some(v),
        Err(_) => None,
    }
}
