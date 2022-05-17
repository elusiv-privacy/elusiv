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

                for field in var.fields {
                    let field_name = field.ident.clone().unwrap();
                    fields.extend(quote! { #field_name, });
                }

                let mut signature = quote! { #fields };

                // Account attributes
                for attr in var.attrs {
                    let attr_name = attr.path.get_ident().unwrap().to_string();
                    let sub_attrs = sub_attrs_prepare(attr.tokens.to_string());
                    let sub_attrs: Vec<&str> = (&sub_attrs).split(",").collect();
                    let account: TokenStream = sub_attrs[0].parse().unwrap();

                    accounts.extend(quote! {
                        let #account = solana_program::account_info::next_account_info(account_info_iter)?;    
                    });

                    match attr_name.as_str() {
                        "prg" => {  // Program owned account with static pubkey
                            let ty = program_account_type(sub_attrs[1]);

                            // Static pubkey check
                            accounts.extend(quote!{
                                guard!(#ty::KEY == #account.key, InvalidAccount);

                                let acc_data = account_data_mut(#account);
                                let mut #account = #ty::from_data(acc_data)?;
                            });
                        },
                        "usr" => {  // User account
                        },
                        "sig" => {  // User account as signer
                            // Check signer
                            accounts.extend(quote!{
                                guard!(#account.is_signer(), InvalidAccount);
                            });
                        },
                        "sys" => {  // System program account
                            // Check that system progam fits the key expression's pubkey
                            let key: TokenStream = named_sub_attribute("key", sub_attrs[1]).parse().unwrap();
                            accounts.extend(quote!{
                                guard!(#key == #account.key, InvalidAccount);
                            });
                        },
                        v => {  // PDA based accounts
                            let ty = program_account_type(sub_attrs[1]);
                            let pda_offset: TokenStream = if let Some(offset) = sub_attrs.get(2) {
                                named_sub_attribute("pda_offset", offset).parse().unwrap()
                            } else { quote!{} };

                            // PDA check
                            accounts.extend(quote!{
                                guard_pda_account!(#account, #ty::pda_seed(&[#pda_offset]));
                                let acc_data = account_data_mut(#account);
                            });

                            match v {
                                "pda" => {  // Single PDA account
                                    accounts.extend(quote!{
                                        let mut #account = #ty::from_data(acc_data)?;
                                    });
                                },
                                "pdi" => {  // Single PDA AccountInfo (!)
                                },
                                "arr" => {  // Base account and n additional PDA array-accounts
                                    let ty = program_account_type(sub_attrs[1]);
        
                                    // Base PDA offset (u64)
                                    let pda_offset: TokenStream = named_sub_attribute("pda_offset", sub_attrs[2]).parse().unwrap();
        
                                    accounts.extend(quote!{
                                        // Array accounts plus PDA check for each
                                        let mut array_accounts = Vec::new();
                                        for i in 0..#ty::ACCOUNTS_COUNT {
                                            let array_account = solana_program::account_info::next_account_info(account_info_iter)?;    
                                            array_accounts.push(array_account);
        
                                            guard_pda_account!(array_account, #ty::pda_seed(&[#pda_offset, i as u64]));
                                        }
                                        guard!(#ty::ACCOUNTS_COUNT == sub, InvalidAccount);

                                        let #account = #ty::new(acc_data, array_accounts)?;
                                    });
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
                        #fn_name(#signature)
                    },
                })
            }
        },
        _ => {}
    }

    quote! {
        // Program entrypoint and instruction matching
        solana_program::entrypoint!(process_instruction);
        pub fn process_instruction(
            program_id: &solana_program::pubkey::Pubkey,
            accounts: &[solana_program::account_info::AccountInfo],
            instruction_data: &[u8]
        ) -> solana_program::entrypoint::ProgramResult {
            use solana_program::program_error::ProgramError::InvalidInstructionData;

            let mut data = &mut &instruction_data;
            let instruction = borsh::BorshDeserialize::deserialize(data).or(Err(InvalidInstructionData.into()));
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