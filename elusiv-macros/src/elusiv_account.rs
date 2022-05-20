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
                lifetimes.extend(quote! { , 'b });
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
            },
            "multi_account" => {    // Turns this PDA account into a Multi account
                let multi_account: String = named_sub_attribute("multi_account", attr).parse().unwrap();
                let count: TokenStream = multi_account.parse().unwrap();

                impls.extend(quote! {
                    impl<#lifetimes> crate::state::program_account::MultiAccountAccount<'b> for #name<#lifetimes> {
                        const COUNT: usize = #count;

                        fn get_account(&self, account_index: usize) -> &solana_program::account_info::AccountInfo<'b> {
                            &self.accounts[account_index]
                        }
                    }
                });

                // Add accounts field
                fields.extend(quote! { accounts, });
                definition.extend(quote! { accounts: Vec<&'b solana_program::account_info::AccountInfo<'b>>, });
                signature.extend(quote! { accounts: Vec<&'b solana_program::account_info::AccountInfo<'b>>, });
            },
            _ => { panic!("Invalid attribute {}", attr) }
        }
    }

    // Parse fields
    for field in input.fields {
        let field_name = &field.ident.expect("Field has no name");

        let getter_name = ident_with_prefix(field_name, "get_");
        let setter_name = ident_with_prefix(field_name, "set_");
        fields.extend(quote! { #field_name, });

        // Add mutable backing byte slice
        definition.extend(quote! { #field_name: &'a mut [u8], });

        match field.ty {
            Type::Path(type_path) => {  // Any field
                let ty = type_path.into_token_stream();

                // Init (using SerDeManager)
                init.extend(quote! {
                    let (#field_name, d) = d.split_at_mut(<#ty>::SIZE);
                });

                // Size increase
                total_size.extend(quote! {
                    + <#ty>::SIZE
                });

                // Getter and setter
                functions.extend(quote! {
                    pub fn #getter_name(&self) -> #ty {
                        <#ty>::deserialize(self.#field_name)
                    }

                    pub fn #setter_name(&mut self, value: #ty) {
                        <#ty>::serialize(value, &mut self.#field_name);
                    }
                });
            },
            Type::Array(type_array) => {    // Array field
                let ty = type_array.elem.clone().into_token_stream();
                let field_size = type_array.len;

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
                        <#ty>::deserialize(slice)
                    }

                    pub fn #setter_name(&mut self, index: usize, value: #ty) {
                        let offset = index * <#ty>::SIZE;
                        <#ty>::serialize(value, &mut self.#field_name[offset..offset + <#ty>::SIZE]);
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

        impl<#lifetimes> #name<#lifetimes> {
            const TOTAL_SIZE: usize = 0 #total_size;

            pub fn new(d: &'a mut [u8], #signature) -> Result<Self, solana_program::program_error::ProgramError> {
                // Check for correct size
                crate::macros::guard!(
                    d.len() == Self::TOTAL_SIZE,
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