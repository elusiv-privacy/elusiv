use syn::{ Type, DataStruct, Data };
use quote::{ quote, ToTokens };
use proc_macro2::TokenStream;
use super::utils::*;

macro_rules! assert_field {
    ($id: ident, $iter: ident, $e: expr) => {
        let $id = $iter.next().expect(&format!("First field has to be `{}`", $e));
        if $id.to_token_stream().to_string() != $e {
            panic!("Could not find field has to be `{}`", $e);
        }
    };
}

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
    //let mut init_after = quote! {};
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

    // Iter used for fields: `bump_seed`, `initialized`, `pubkeys` that traits "require"
    let fields_iter = input.clone();
    let mut fields_iter = fields_iter.fields.iter();
    let mut is_pda = false;

    // Special implementations
    for attr in attrs {
        let attr_ident = attr.split("=").next().unwrap();
        match attr_ident {
            "pda_seed" => { // PDA based account
                assert_field!(first_field, fields_iter, "bump_seed : u8");
                assert_field!(second_field, fields_iter, "initialized : bool");

                is_pda = true;
                let seed: TokenStream = named_sub_attribute("pda_seed", attr).parse().unwrap();
                impls.extend(quote! {
                    impl<#lifetimes> crate::state::program_account::PDAAccount for #name<#lifetimes> {
                        const SEED: &'static [u8] = #seed;
                    }
                });
            },
            "multi_account" => {    // Turns this PDA account into a Multi account
                assert!(is_pda);

                let multi_account: String = named_sub_attribute("multi_account", attr).parse().unwrap();
                let multi_account = (&multi_account[1..multi_account.len() - 1]).split(";").collect::<Vec<&str>>();
                let count: TokenStream = multi_account[0].parse().unwrap();
                let max_account_size: TokenStream = multi_account[1].parse().unwrap();

                assert_field!(first_field, fields_iter, format!("pubkeys : [U256 ; {}]", multi_account[0]));
                assert_field!(first_field, fields_iter, "finished_setup : bool");

                impls.extend(quote! {
                    impl<#lifetimes> crate::state::program_account::MultiAccountAccount<'t> for #name<#lifetimes> {
                        const COUNT: usize = #count;
                        const INTERMEDIARY_ACCOUNT_SIZE: usize = #max_account_size;

                        fn get_all_pubkeys(&self) -> Vec<U256> {
                            let mut res = Vec::new();
                            for i in 0..Self::COUNT {
                                res.push(self.get_pubkeys(i));
                            }
                            res
                        }

                        fn set_all_pubkeys(&mut self, pubkeys: &[U256]) {
                            assert!(pubkeys.len() == Self::COUNT);
                            for i in 0..Self::COUNT {
                                self.set_pubkeys(i, &pubkeys[i]);
                            }
                        }

                        fn get_account(&self, account_index: usize) -> &solana_program::account_info::AccountInfo<'t> {
                            &self.accounts[account_index]
                        }
                    }
                });

                // Add accounts field (IMPORTANT: no verification happens here, caller needs to make sure that the accounts match the pubkeys)
                fields.extend(quote! { accounts, });
                definition.extend(quote! {
                    accounts: &'b [solana_program::account_info::AccountInfo<'t>],
                });
                signature.extend(quote! {
                    accounts: &'b [solana_program::account_info::AccountInfo<'t>],
                });

                // Adds check that the supplied accounts are the correct ones (failed when the account has not been setup yet)
                // - INFO: not needed since we do this with ElusivInstruction
                /*init_after.extend(quote!{
                    // Check that account has been setup
                    crate::macros::guard!(r.get_finished_setup(), crate::error::ElusivError::InvalidAccount);

                    // Check for pubkey match
                    assert_eq!(accounts.len(), Self::COUNT);
                    for i in 0..Self::COUNT {
                        assert_eq!(r.get_pubkeys(i), accounts[i].key.to_bytes());
                    }
                });*/
            },
            "partial_computation" => {
                assert_field!(first_field, fields_iter, "is_active : bool");
                assert_field!(second_field, fields_iter, "instruction : u32");
                assert_field!(third_field, fields_iter, "fee_payer : U256");
            },
            _ => { }
        }
    }

    // Parse fields
    for field in input.fields {
        let field_name = &field.ident.expect("Field has no name");

        let getter_name = ident_with_prefix(field_name, "get_");
        let setter_name = ident_with_prefix(field_name, "set_");
        let all_setter_name = ident_with_prefix(field_name, "set_all_");
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
                            self.#field_name[offset..][i] = v[i];
                        }
                    }

                    pub fn #all_setter_name(&mut self, v: &[u8]) {
                        assert!(v.len() == self.#field_name.len());
                        for i in 0..v.len() {
                            self.#field_name[i] = v[i];
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
                //let r = #name { #fields };
                // Additional checks
                //#init_after
                //Ok(r)
            }

            // Access functions
            #functions
        }

        #impls
    }
}