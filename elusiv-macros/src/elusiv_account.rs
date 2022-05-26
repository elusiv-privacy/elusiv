use syn::{ Type, DataStruct, Data };
use quote::{ quote, ToTokens };
use proc_macro2::TokenStream;
use super::utils::*;

pub fn impl_elusiv_account(ast: &syn::DeriveInput, attrs: TokenStream) -> TokenStream {
    let name = &ast.ident.clone();

    fn get_struct(ast: syn::DeriveInput) -> DataStruct {
        if let Data::Struct(input) = ast.data { return input; } else { panic!("Struct not found"); }
    }
    let input = get_struct(ast.clone());

    let mut definition = quote! {};
    let mut total_size = quote! {};
    let mut impls = quote! {};
    let mut init = quote! {};
    let mut fields = quote! {};
    let mut signature = quote! {};
    let mut lifetimes = quote!{ 'a };
    let mut functions = quote! {};

    // Attributes
    let attrs = sub_attrs_prepare(attrs.to_string());
    let attrs: Vec<&str> = (&attrs).split(",").collect();

    // Lifetimes
    for attr in &attrs {
        let attr_ident = attr.split("=").next().unwrap();
        match attr_ident {
            "multi_account" => {
                lifetimes.extend(quote! { , 'b, 't });
            },
            _ => {}
        }
    }

    // Special implementations
    for attr in attrs {
        let attr_ident = attr.split("=").next().unwrap();
        match attr_ident {
            "pda_seed" => { // PDA based account
                let seed: TokenStream = named_sub_attribute("pda_seed", attr).parse().unwrap();
                impls.extend(quote! {
                    impl<#lifetimes> crate::state::program_account::PDAAccount for #name<#lifetimes> {
                        const SEED: &'static [u8] = #seed;
                    }
                });

                // Add `bump_seed` field
                fields.extend(quote! { bump_seed, });
                definition.extend(quote! { bump_seed: &'a mut [u8], });
                init.extend(quote! { let (bump_seed, d) = d.split_at_mut(1); }); 
                total_size.extend(quote! { + 1 });
                functions.extend(quote! {
                    pub fn get_bump_seed(&self, index: usize) -> u8 { self.bump_seed[0] }
                    pub fn set_bump_seed(&mut self, index: usize, value: u8) { self.bump_seed[0] = value; }
                });
            },
            "multi_account" => {    // Turns this PDA account into a Multi account
                let multi_account: String = named_sub_attribute("multi_account", attr).parse().unwrap();
                let multi_account = (&multi_account[1..multi_account.len() - 1]).split(";").collect::<Vec<&str>>();
                let count: TokenStream = multi_account[0].parse().unwrap();
                let max_account_size: TokenStream = multi_account[1].parse().unwrap();

                // Add `pubkeys` field
                fields.extend(quote! { pubkeys, });
                definition.extend(quote! { pubkeys: &'a mut [u8], });
                init.extend(quote! { let (pubkeys, d) = d.split_at_mut(U256::SIZE * #count); }); 
                total_size.extend(quote! { + U256::SIZE * #count });
                impls.extend(quote! {
                    impl<#lifetimes> crate::state::program_account::MultiAccountAccount<'t> for #name<#lifetimes> {
                        const COUNT: usize = #count;
                        const INTERMEDIARY_ACCOUNT_SIZE: usize = #max_account_size;

                        fn get_all_pubkeys(&self) -> Vec<U256> {
                            let mut res = Vec::new();
                            for i in 0..Self::COUNT {
                                res.push(
                                    U256::try_from_slice(&self.pubkeys[i * U256::SIZE..]).unwrap()
                                );
                            }
                            res
                        }

                        fn set_all_pubkeys(&mut self, pubkeys: &[U256]) {
                            assert!(pubkeys.len() == Self::COUNT);
                            for i in 0..Self::COUNT {
                                let offset = i * U256::SIZE;
                                let v = U256::try_to_vec(&pubkeys[i]).unwrap();
                                for j in 0..v.len() {
                                    self.pubkeys[offset..][j] = v[j];
                                }
                            }
                        }

                        fn get_account(&self, account_index: usize) -> &solana_program::account_info::AccountInfo<'t> {
                            &self.accounts[account_index]
                        }
                    }
                });

                // Add accounts field
                fields.extend(quote! { accounts, });
                definition.extend(quote! {
                    accounts: &'b [solana_program::account_info::AccountInfo<'t>],
                });
                signature.extend(quote! {
                    accounts: &'b [solana_program::account_info::AccountInfo<'t>],
                });
            },
            _ => { }
        }
    }

    // Parse fields
    for field in input.fields {
        let field_name = &field.ident.expect("Field has no name");

        let getter_name = ident_with_prefix(field_name, "get_");
        let setter_name = ident_with_prefix(field_name, "set_");
        fields.extend(quote! { #field_name, });

        let mut use_getter_setter = true;

        // Attribute that prevents the usage of a getter and setter
        if let Some(_) = field.attrs.iter().find(|x| x.path.get_ident().unwrap().to_string() == "pub_non_lazy") {
            use_getter_setter = false;
        }

        match field.ty {
            Type::Path(type_path) => {  // Any field
                let ty = type_path.into_token_stream();

                init.extend(quote! {
                    let (#field_name, d) = d.split_at_mut(<#ty>::SIZE);
                });

                // Size increase
                total_size.extend(quote! {
                    + <#ty>::SIZE
                });

                if use_getter_setter {
                    // Add mutable backing byte slice
                    definition.extend(quote! { #field_name: &'a mut [u8], });

                    // Getter and setter
                    functions.extend(quote! {
                        pub fn #getter_name(&self) -> #ty {
                            <#ty>::try_from_slice(self.#field_name).unwrap()
                        }

                        pub fn #setter_name(&mut self, value: &#ty) {
                            let v = <#ty>::try_to_vec(value).unwrap();
                            for i in 0..v.len() {
                                self.#field_name[i] = v[i];
                            }
                        }
                    });
                } else {
                    definition.extend(quote! { pub #field_name: #ty, });

                    init.extend(quote! {
                        let #field_name = <#ty>::new(#field_name);
                    });
                }
                
            },
            Type::Array(type_array) => {    // Array field
                let ty = type_array.elem.clone().into_token_stream();
                let field_size = type_array.len;

                // Add mutable backing byte slice
                definition.extend(quote! { #field_name: &'a mut [u8], });

                // Array init
                init.extend(quote! {
                    let (#field_name, d) = d.split_at_mut(<#ty>::SIZE * #field_size);
                }); 

                // Size increase
                total_size.extend(quote! {
                    + (<#ty>::SIZE * #field_size)
                });

                // Array getter and setter
                functions.extend(quote! {
                    pub fn #getter_name(&self, index: usize) -> #ty {
                        let slice = &self.#field_name[index * <#ty>::SIZE..(index + 1) * <#ty>::SIZE];
                        <#ty>::try_from_slice(slice).unwrap()
                    }

                    pub fn #setter_name(&mut self, index: usize, value: &#ty) {
                        let offset = index * <#ty>::SIZE;
                        let v = <#ty>::try_to_vec(value).unwrap();
                        for i in 0..v.len() {
                            self.#field_name[offset..offset + <#ty>::SIZE][i] = v[i];
                        }
                    }
                });
            },
            _ => { panic!("Invalid field in struct") }
        }
    }

    quote! {
        pub struct #name<#lifetimes> {
            #definition
        }

        impl<#lifetimes> crate::state::program_account::SizedAccount for #name<#lifetimes> {
            const SIZE: usize =  0 #total_size;
        }

        impl<#lifetimes> #name<#lifetimes> {
            pub fn new(d: &'a mut [u8], #signature) -> Result<Self, solana_program::program_error::ProgramError> {
                // Check for correct size
                crate::macros::guard!(
                    d.len() == Self::SIZE,
                    crate::error::ElusivError::InvalidAccountSize
                );

                // All value initializations 
                #init

                Ok(#name { #fields })
            }

            // Access functions
            #functions
        }

        #impls
    }
}

/*fn push_field(fields: &mut Fields, ident: &str, ty: Type) {
    match fields {
        syn::Fields::Named(n) => {
            n.named.push(Field {
                attrs: vec![],
                vis: syn::Visibility::Inherited,
                ident: Some(syn::Ident::new(ident, proc_macro2::Span::call_site())),
                colon_token: Some(syn::token::Colon([proc_macro2::Span::call_site()])),
                ty,
            });
        },
        _ => panic!()
    }
}*/