use quote::quote;
use super::utils::{ upper_camel_to_upper_snake, named_sub_attribute };
use proc_macro2::TokenStream;

pub fn impl_elusiv_instruction(ast: &syn::DeriveInput) -> proc_macro2::TokenStream {
    let mut matches = quote!{};
    let mut functions = quote!{};

    match &ast.data {
        syn::Data::Enum(e) => {
            for var in e.variants.clone() {
                let ident = var.ident;
                let fn_name: TokenStream = upper_camel_to_upper_snake(&ident.to_string()).to_lowercase().parse().unwrap();

                // Processor calls
                let mut accounts = quote!{};
                let mut fields = quote!{};
                let mut signature = quote!{};

                // Instruction creation
                let mut fields_with_type = quote!{};
                let mut user_accounts = quote!{};
                let mut instruction_accounts = quote!{};

                for field in var.fields {
                    let field_name = field.ident.clone().unwrap();
                    let ty = field.ty;

                    fields.extend(quote! { #field_name, });
                    fields_with_type.extend(quote! { #field_name: #ty, });
                }

                // Account attributes
                for (i, attr) in var.attrs.iter().enumerate() {
                    let attr_name = attr.path.get_ident().unwrap().to_string();

                    // Sub-attrs are the fields as in #[usr(sub_attr0 = .., sub_attr1, ..)]
                    let mut fields = attr.tokens.to_string();
                    fields.retain(|x| x != '{' && x != '}' && !x.is_whitespace());
                    let sub_attrs: Vec<&str> = (&fields[1..fields.len() - 1]).split(",").collect();

                    let mut account: TokenStream = sub_attrs[0].parse().unwrap();
                    let mut account_init = Vec::new(); // used for creating the instruction objects with the abi-feature

                    accounts.extend(quote! {
                        let #account = next_account_info(account_info_iter)?;    
                    });

                    // Signer check
                    let is_signer = sub_attrs.contains(&"signer");
                    if  is_signer {
                        accounts.extend(quote!{
                            if !#account.is_signer { return Err(InvalidArgument) }
                        });
                    }

                    // Writable check
                    let is_writable= sub_attrs.contains(&"writable");
                    if is_writable {
                        accounts.extend(quote!{
                            if !#account.is_writable { return Err(InvalidArgument) }
                        });
                    }

                    // Ownership check
                    let is_owned= sub_attrs.contains(&"owned");
                    if is_owned {
                        accounts.extend(quote!{
                            if #account.owner != program_id { return Err(InvalidArgument) }
                        });
                    }

                    // Ignore means not passing the account to the processor function
                    let ignore = sub_attrs.contains(&"ignore");

                    // `AccountInfo`?
                    let as_account_info = sub_attrs.contains(&"account_info");

                    let mut_token = if is_writable { quote!{ mut } } else { quote!{} };
                    let account_init_fn = if is_writable { quote!{ new } } else { quote!{ new_readonly } };

                    let user_account_type = if is_signer {
                        if is_writable { quote!{ SignerAccount } } else { quote!{ WritableSignerAccount } }
                    } else {
                        if is_writable { quote!{ WritableUserAccount } } else { quote!{ UserAccount } }
                    };

                    match attr_name.as_str() {
                        // User `AccountInfo` (usage: <name>)
                        "usr" => {
                            user_accounts.extend(quote!{ #account: #user_account_type, });
                            account_init.push(quote!{
                                accounts.push(AccountMeta::#account_init_fn(#account.0, #is_signer));
                            });
                        },

                        // Program owned accounts that satisfy a pubkey constraint
                        "prg" => {
                            let ty = program_account_type(sub_attrs[1]);
                            let key: TokenStream = named_sub_attribute("key", sub_attrs[2]).parse().unwrap();

                            user_accounts.extend(quote!{ #account: #user_account_type, });
                            account_init.push(quote!{
                                accounts.push(AccountMeta::#account_init_fn(#account.0, #is_signer));
                            });

                            if !is_owned {
                                accounts.extend(quote!{
                                    if #account.owner != program_id { return Err(InvalidArgument) }
                                });
                            }

                            accounts.extend(quote!{
                                if #account.key.to_bytes() != #key { return Err(InvalidArgument) }
                            });

                            if as_account_info {
                                account = quote!{ &#account };
                            } else {
                                accounts.extend(quote!{
                                    let acc_data = &mut #account.data.borrow_mut()[..];
                                    let #mut_token #account = <#ty>::new(acc_data)?;
                                });

                                if is_writable {
                                    account = quote!{ &mut #account };
                                } else {
                                    account = quote!{ &#account };
                                }
                            }

                        },

                        // System program `AccountInfo` (usage: <name> <key = ..>)
                        "sys" => {
                            // Check that system progam pubkey is correct (for this we have a field `key` that the pubkey gets compared to)
                            let key: TokenStream = named_sub_attribute("key", sub_attrs[1]).parse().unwrap();

                            accounts.extend(quote!{
                                if #key != *#account.key { return Err(InvalidArgument) };
                            });

                            account_init.push(quote!{
                                accounts.push(AccountMeta::#account_init_fn(#key, #is_signer));
                            });
                        },

                        // PDA accounts (usage: <name> <AccountType> <pda_offset: u64 = ..>? <account_info>? <multi_account>? <ownership>)
                        "pda" => {
                            // Every PDA account needs to implement the trait `elusiv::state::program_account::PDAAccount`
                            // - this trait allows us to verify PDAs
                            // - this allows us to define `MultiAccountAccount`s, which are a single main PDA account with `COUNT` sub-accounts
                            // - the seed of the main account plus the index of each sub-account is used to generate their PDAs

                            // The PDA account type
                            let ty = program_account_type(sub_attrs[1]);

                            // The PDA offset is an optional field, used to add an offset to the seed (e.g. to index of tree)
                            // - note: you can reference a field from an account added before this one as an offset as well
                            let pda_offset: TokenStream = if let Some(offset) = sub_attrs.get(2) {
                                if offset.starts_with("pda_offset") {
                                    named_sub_attribute("pda_offset", offset).parse().unwrap()
                                } else { quote!{ None } }
                            } else { quote!{ None } };

                            // Multi account account
                            let multi_account = sub_attrs.contains(&"multi_accounts");

                            // (For multi accountx account): SKIPS THE PUBKEY VERIFICATION of the subaccounts (ONLY TO BE USED WHEN CREATING A NEW ACCOUNT!)
                            let no_subaccount_check = sub_attrs.contains(&"no_subaccount_check");

                            // PDA verification
                            accounts.extend(quote!{
                                if !<#ty>::is_valid_pubkey(&#account, #pda_offset, #account.key)? { return Err(InvalidArgument) }
                            });

                            account_init.push(quote!{
                                accounts.push(AccountMeta::#account_init_fn(<#ty>::find(#pda_offset).0, #is_signer));
                            });

                            if multi_account {
                                let write_check = if !is_writable { quote!{} } else {
                                    quote!{ if !accounts[i].is_writable { return Err(InvalidArgument) } }
                                };
                                let sub_account_check = if no_subaccount_check { quote!{} } else {
                                    quote!{ if accounts[i].key.to_bytes() != fields_check.pubkeys[i] { return Err(InvalidArgument) } }
                                };

                                // Sub-accounts with PDA and ownership check for each
                                accounts.extend(quote!{
                                    let acc_data = &mut #account.data.borrow_mut()[..];
                                    let fields_check = match MultiAccountAccountFields::<{<#ty>::COUNT}>::try_from_slice(&acc_data[..MultiAccountAccountFields::<{<#ty>::COUNT}>::SIZE]) {
                                        Ok(a) => a,
                                        Err(_) => return Err(InvalidArgument)
                                    };
                                    let mut accounts = next_account_infos(account_info_iter, <#ty>::COUNT)?;
                                    for i in 0..<#ty>::COUNT {
                                        #write_check
                                        #sub_account_check
                                    }
                                });

                                let arr_name: TokenStream = format!("multi_accounts_{}", i).parse().unwrap();
                                user_accounts.extend(quote!{ #arr_name: [#user_account_type; <#ty>::COUNT], });
                                account_init.push(quote!{
                                    for i in 0..<#ty>::COUNT {
                                        accounts.push(AccountMeta::#account_init_fn(#arr_name[i].0, #is_signer));
                                    }
                                });

                                if as_account_info {
                                    accounts.extend(quote!{
                                        accounts.insert(0, #account);
                                        let #account = accounts;
                                    });
                                    account = quote!{ #account };
                                } else {
                                    if is_writable {
                                        accounts.extend(quote!{ let mut #account = #ty::new(acc_data, accounts)?; });
                                        account = quote!{ &mut #account };
                                    } else {
                                        accounts.extend(quote!{ let #account = #ty::new(acc_data, accounts)?; });
                                        account = quote!{ &#account };
                                    }
                                }
                            } else {
                                if as_account_info {
                                    account = quote!{ &#account };
                                } else {
                                    accounts.extend(quote!{
                                        let acc_data = &mut #account.data.borrow_mut()[..];
                                        let #mut_token #account = <#ty>::new(acc_data)?;
                                    });

                                    if is_writable {
                                        account = quote!{ &mut #account };
                                    } else {
                                        account = quote!{ &#account };
                                    }
                                }
                            }
                        },
                        v => panic!("Invalid attribute name {}", v)
                    }

                    // Add account to processor call signature
                    if !ignore {
                        signature.extend(quote!{ #account, });
                    }

                    // Add account init
                    instruction_accounts.extend(account_init.iter().fold(quote!{}, |acc, x| quote!{ #acc #x }));
                }

                matches.extend(quote! {
                    ElusivInstruction::#ident { #fields } => {
                        #accounts
                        #fn_name(#signature #fields)
                    },
                });

                functions.extend(quote!{
                    pub fn #fn_name(#fields_with_type #user_accounts) -> solana_program::instruction::Instruction {
                        let mut accounts = Vec::new();

                        #instruction_accounts
                        let data = ElusivInstruction::#ident { #fields };
                        let data = ElusivInstruction::try_to_vec(&data).unwrap();

                        solana_program::instruction::Instruction::new_with_bytes(
                            crate::id(),
                            &data,
                            accounts,
                        )
                    }
                });
            }
        },
        _ => {}
    }

    quote! {
        pub fn process_instruction(program_id: &Pubkey, accounts: &[AccountInfo], instruction: ElusivInstruction) -> ProgramResult {
            let account_info_iter = &mut accounts.iter();
            
            match instruction {
                #matches
                _ => { Err(InvalidInstructionData) }
            }
        }        

        #[cfg(feature = "instruction-abi")]
        impl ElusivInstruction {
            #functions
        }

        #[cfg(feature = "instruction-abi")]
        #[derive(Debug)]
        pub struct UserAccount(pub solana_program::pubkey::Pubkey);

        #[cfg(feature = "instruction-abi")]
        #[derive(Debug)]
        pub struct WritableUserAccount(pub solana_program::pubkey::Pubkey);

        #[cfg(feature = "instruction-abi")]
        #[derive(Debug)]
        pub struct SignerAccount(pub solana_program::pubkey::Pubkey);

        #[cfg(feature = "instruction-abi")]
        #[derive(Debug)]
        pub struct WritableSignerAccount(pub solana_program::pubkey::Pubkey);
    }
}

fn program_account_type(name: &str) -> TokenStream {
    (String::from(name) + "Account").parse().unwrap()
}