use quote::quote;
use super::utils::{ upper_camel_to_upper_snake, sub_attrs_prepare, named_sub_attribute };
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
                for attr in var.attrs {
                    let attr_name = attr.path.get_ident().unwrap().to_string();

                    // Sub-attrs are the fields as in #[usr(sub_attr0 = .., sub_attr1, ..)]
                    let fields = sub_attrs_prepare(attr.tokens.to_string());
                    let mut fields_alt = fields.clone();
                    fields_alt.retain(|x| x != ']' && x != '[');

                    let sub_attrs: Vec<&str> = (&fields[1..fields.len() - 1]).split(",").collect();
                    let sub_attrs_ignore_braces: Vec<&str> = (&fields_alt[1..fields_alt.len() - 1]).split(",").collect();

                    let mut account: TokenStream = sub_attrs[0].parse().unwrap();
                    let mut account_init = Vec::new(); // used for creating the instruction objects with the abi-feature

                    accounts.extend(quote! {
                        let #account = solana_program::account_info::next_account_info(account_info_iter)?;    
                    });

                    // Signer check
                    let is_signer = matches!(sub_attrs_ignore_braces.iter().find(|&x| *x == "signer"), Some(_));
                    if is_signer {
                        accounts.extend(quote!{
                            guard!(#account.is_signer, InvalidAccount);
                        });
                    }

                    // Writable check
                    let is_writable= matches!(sub_attrs_ignore_braces.iter().find(|&x| *x == "writable"), Some(_));
                    if is_writable {
                        accounts.extend(quote!{
                            guard!(#account.is_writable, InvalidAccount);
                        });
                    }

                    let mut_token = if is_writable { quote!{ mut } } else { quote!{} };
                    let account_init_fn = if is_writable { quote!{ new } } else { quote!{ new_readonly } };

                    match attr_name.as_str() {
                        // User `AccountInfo` (usage: <name>)
                        "usr" => {
                            if is_signer {
                                if is_writable {
                                    user_accounts.extend(quote!{ #account: SignerAccount, });
                                } else {
                                    user_accounts.extend(quote!{ #account: WritableSignerAccount, });
                                }
                            } else {
                                if is_writable {
                                    user_accounts.extend(quote!{ #account: WritableUserAccount, });
                                } else {
                                    user_accounts.extend(quote!{ #account: UserAccount, });
                                }
                            }

                            account_init.push(quote!{
                                accounts.push(AccountMeta::#account_init_fn(#account.0, #is_signer));
                            });
                        },

                        // System program `AccountInfo` (usage: <name> <key = ..>)
                        "sys" => {
                            // Check that system progam pubkey is correct (for this we have a field `key` that the pubkey gets compared to)
                            let key: TokenStream = named_sub_attribute("key", sub_attrs[1]).parse().unwrap();

                            accounts.extend(quote!{
                                guard!(#key == *#account.key, InvalidAccount);
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
                                } else { quote!{ 0 } }
                            } else { quote!{ 0 } };

                            // `AccountInfo`?
                            let as_account_info = matches!(sub_attrs_ignore_braces.iter().find(|&x| *x == "account_info"), Some(_));

                            // Multi accounts account
                            let multi_account = matches!(sub_attrs_ignore_braces.iter().find(|&x| *x == "multi_accounts"), Some(_));

                            // Transfer ownership to the processor function, don't just pass a reference
                            let transfer_ownership = matches!(sub_attrs_ignore_braces.iter().find(|&x| *x == "ownership"), Some(_));

                            // PDA and ownership verification
                            accounts.extend(quote!{
                                guard!(<#ty>::is_valid_pubkey(&[#pda_offset], #account.key), InvalidAccount);
                                guard!(#account.owner == program_id, InvalidAccount);
                            });

                            account_init.push(quote!{
                                accounts.push(AccountMeta::#account_init_fn(<#ty>::pubkey(&[#pda_offset]).0, #is_signer));
                            });

                            if multi_account {
                                let write_check = if is_writable { quote!{ guard!(sub_account.is_writable, InvalidAccount); } } else { quote!{} };

                                // Sub-accounts with PDA and ownership check for each
                                accounts.extend(quote!{
                                    let acc_data = &mut #account.data.borrow_mut()[..];
                                    let mut accounts = Vec::new();
                                    for i in 0..#ty::COUNT {
                                        let sub_account = solana_program::account_info::next_account_info(account_info_iter)?;    
                                        guard!(<#ty>::is_valid_pubkey(&[#pda_offset, i as u64], sub_account.key), InvalidAccount);
                                        guard!(sub_account.owner == program_id, InvalidAccount);
                                        #write_check
                                        accounts.push(sub_account);
                                    }
                                    guard!(#ty::COUNT == accounts.len(), InvalidAccount);
                                });

                                account_init.push(quote!{
                                    for i in 0..#ty::COUNT {
                                        accounts.push(AccountMeta::#account_init_fn(<#ty>::pubkey(&[#pda_offset, i as u64]).0, #is_signer));
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
                                if !as_account_info {
                                    accounts.extend(quote!{
                                        let acc_data = &mut #account.data.borrow_mut()[..];
                                        let #account = <#ty>::new(acc_data)?;
                                    });

                                    accounts.extend(quote!{ let #mut_token #account = <#ty>::new(acc_data)?; });

                                    if is_writable {
                                        if transfer_ownership {
                                            account = quote!{ #account };
                                        } else {
                                            account = quote!{ &mut #account };
                                        }
                                    } else {
                                        account = quote!{ &#account };
                                    }
                                } else {
                                    account = quote!{ &#account };
                                }
                            }
                        },
                        v => panic!("Invalid attribute name {}", v)
                    }

                    // Add account to processor call signature
                    signature.extend(quote!{ #account, });

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
                        let data = ElusivInstruction::serialize_vec(data);

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
        // Program entrypoint and instruction matching
        solana_program::entrypoint!(process_instruction);
        pub fn process_instruction(program_id: &solana_program::pubkey::Pubkey, accounts: &[solana_program::account_info::AccountInfo], instruction_data: &[u8]) -> solana_program::entrypoint::ProgramResult {
            use solana_program::program_error::ProgramError::InvalidInstructionData;

            let mut data = &mut &instruction_data;
            let instruction = ElusivInstruction::deserialize(data);
            let account_info_iter = &mut accounts.iter();
            
            match instruction {
                #matches
                _ => { Err(InvalidInstructionData.into()) }
            }
        }

        #[cfg(feature = "instruction-abi")]
        impl ElusivInstruction {
            #functions
        }

        #[cfg(feature = "instruction-abi")]
        pub struct UserAccount(pub solana_program::pubkey::Pubkey);

        #[cfg(feature = "instruction-abi")]
        pub struct WritableUserAccount(pub solana_program::pubkey::Pubkey);

        #[cfg(feature = "instruction-abi")]
        pub struct SignerAccount(pub solana_program::pubkey::Pubkey);

        #[cfg(feature = "instruction-abi")]
        pub struct WritableSignerAccount(pub solana_program::pubkey::Pubkey);
    }
}

fn program_account_type(name: &str) -> TokenStream {
    (String::from(name) + "Account").parse().unwrap()
}