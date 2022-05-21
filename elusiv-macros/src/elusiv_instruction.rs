use quote::quote;
use super::utils::{ upper_camel_to_upper_snake, sub_attrs_prepare, named_sub_attribute };
use proc_macro2::TokenStream;

pub fn impl_elusiv_instruction(ast: &syn::DeriveInput) -> proc_macro2::TokenStream {
    let mut matches = quote! {};

    match &ast.data {
        syn::Data::Enum(e) => {
            for var in e.variants.clone() {
                let ident = var.ident;
                let fn_name: TokenStream = upper_camel_to_upper_snake(&ident.to_string()).to_lowercase().parse().unwrap();
                let mut accounts = quote! {};
                let mut fields = quote! {};
                let mut signature = quote! {};

                for field in var.fields {
                    let field_name = field.ident.clone().unwrap();
                    fields.extend(quote! { #field_name, });
                }

                // Account attributes
                for attr in var.attrs {
                    let attr_name = attr.path.get_ident().unwrap().to_string();
                    let sub_attrs = sub_attrs_prepare(attr.tokens.to_string());
                    let sub_attrs: Vec<&str> = (&sub_attrs[1..sub_attrs.len() - 1]).split(",").collect();
                    let mut account: TokenStream = sub_attrs[0].parse().unwrap();

                    accounts.extend(quote! {
                        let #account = solana_program::account_info::next_account_info(account_info_iter)?;    
                    });

                    match attr_name.as_str() {
                        "usr_inf" => {  // User `AccountInfo`
                        },
                        "sig_inf" => {  // User `AccountInfo` as signer
                            // Check signer
                            accounts.extend(quote!{
                                guard!(#account.is_signer, InvalidAccount);
                            });
                        },
                        "sys_inf" => {  // System program `AccountInfo`
                            // Check that system progam fits the key expression's pubkey
                            let key: TokenStream = named_sub_attribute("key", sub_attrs[1]).parse().unwrap();
                            accounts.extend(quote!{
                                guard!(#key == *#account.key, InvalidAccount);
                            });
                        },
                        v => {  // PDA based accounts
                            let ty = program_account_type(sub_attrs[1]);
                            let pda_offset: TokenStream = if let Some(offset) = sub_attrs.get(2) {
                                named_sub_attribute("pda_offset", offset).parse().unwrap()
                            } else { quote!{} };

                            // PDA check
                            accounts.extend(quote!{
                                guard!(<#ty>::is_valid_pubkey(&[#pda_offset], #account.key), InvalidAccount);
                            });

                            match v {
                                "pda_acc" => {  // Single PDA account
                                    accounts.extend(quote!{
                                        let acc_data = &mut #account.data.borrow_mut()[..];
                                        let #account = #ty::new(acc_data)?;
                                    });
                                    account = quote!{ &#account };
                                },
                                "pda_mut" => {  // Single mutable PDA account
                                    accounts.extend(quote!{
                                        let acc_data = &mut #account.data.borrow_mut()[..];
                                        let mut #account = #ty::new(acc_data)?;
                                    });
                                    account = quote!{ &mut #account };
                                },
                                "pda_own" => {  // Single mutable PDA account
                                    accounts.extend(quote!{
                                        let acc_data = &mut #account.data.borrow_mut()[..];
                                        let mut #account = #ty::new(acc_data)?;
                                    });
                                    account = quote!{ #account };
                                },
                                "pda_inf" => {  // Single PDA `AccountInfo` (!)
                                },
                                "pda_arr" => {  // Base account and n additional PDA array-accounts
                                    let ty = program_account_type(sub_attrs[1]);

                                    // Base PDA offset (u64)
                                    let pda_offset: TokenStream = named_sub_attribute("pda_offset", sub_attrs[2]).parse().unwrap();

                                    // Mutability
                                    let mutability = if sub_attrs.len() > 3 { named_sub_attribute("mut", sub_attrs[3]) == "true" } else { false };
        
                                    accounts.extend(quote!{
                                        let acc_data = &mut #account.data.borrow_mut()[..];

                                        // Array accounts plus PDA check for each
                                        let mut accounts = Vec::new();
                                        for i in 0..#ty::COUNT {
                                            let array_account = solana_program::account_info::next_account_info(account_info_iter)?;    
                                            guard!(<#ty>::is_valid_pubkey(&[#pda_offset, i as u64], array_account.key), InvalidAccount);
                                            accounts.push(array_account);
                                        }
                                        guard!(#ty::COUNT == accounts.len(), InvalidAccount);
                                    });

                                    if mutability {
                                        accounts.extend(quote!{ let mut #account = #ty::new(acc_data, accounts)?; });
                                        account = quote!{ &mut #account };
                                    } else {
                                        accounts.extend(quote!{ let #account = #ty::new(acc_data, accounts)?; });
                                        account = quote!{ &#account };
                                    }
                                },
                                _ => { panic!("Invalid attribute name") }
                            }
                        }
                    }

                    // Add account to processor call signature
                    signature.extend(quote!{ #account, });
                }

                matches.extend(quote! {
                    ElusivInstruction::#ident { #fields } => {
                        #accounts
                        #fn_name(#signature #fields)
                    },
                })
            }
        },
        _ => {}
    }

    quote! {
        // Program entrypoint and instruction matching
        solana_program::entrypoint!(process_instruction);
        pub fn process_instruction<'a>(
            program_id: &solana_program::pubkey::Pubkey,
            accounts: &'a [solana_program::account_info::AccountInfo<'a>],
            instruction_data: &[u8]
        ) -> solana_program::entrypoint::ProgramResult {
            use solana_program::program_error::ProgramError::InvalidInstructionData;

            let mut data = &mut &instruction_data;
            let instruction = ElusivInstruction::deserialize(data); // panics for wrong instruction data
            let account_info_iter = &mut accounts.iter();
            
            match instruction {
                #matches
                _ => { Err(InvalidInstructionData.into()) }
            }
        }
    }
}

fn program_account_type(name: &str) -> TokenStream {
    (String::from(name) + "Account").parse().unwrap()
}