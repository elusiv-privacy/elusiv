use syn::{ Type, DataStruct, Data };
use quote::{ quote, ToTokens };
use proc_macro2::TokenStream;
use super::utils::*;

macro_rules! assert_field {
    // $e is whitespace-sensitive
    ($id: ident, $iter: ident, $e: expr) => {
        let $id = $iter.next().expect(&format!("Field has to be `{}`", $e));
        if $id.to_token_stream().to_string() != $e {
            panic!("Could not find: `{}` in {:?}", $e, $id.to_token_stream().to_string());
        }
    };
}

pub fn impl_elusiv_account(ast: &syn::DeriveInput, attrs: TokenStream) -> TokenStream {
    let name = &ast.ident.clone();

    fn get_struct(ast: syn::DeriveInput) -> DataStruct {
        if let Data::Struct(input) = ast.data { input } else { panic!("Struct not found"); }
    }
    let input = get_struct(ast.clone());

    let mut definition = quote! {};
    let mut total_size = quote! {};
    let mut impls = quote! {};
    let mut init = quote! {};
    let mut account_trait = quote! { crate::state::program_account::ProgramAccount<'a> }; // either ProgramAccount or MultiAccountAccount
    let mut fields = quote! {};
    let mut signature = quote! {};
    let mut lifetimes = quote!{ 'a };
    let mut functions = quote! {};

    // Attributes
    let attrs = sub_attrs_prepare(attrs.to_string());
    let attrs: Vec<&str> = (&attrs).split(',').collect();

    // Lifetimes
    for attr in &attrs {
        let attr_ident = attr.split('=').next().unwrap();
        if attr_ident == "multi_account" {
            lifetimes.extend(quote! { , 'b, 't });
            account_trait = quote! { crate::state::program_account::MultiAccountProgramAccount<'a, 'b, 't> };
        }
    }

    // Iter used for fields: `bump_seed`, `initialized`, `pubkeys` that traits "require"
    let fields_iter = input.clone();
    let mut fields_iter = fields_iter.fields.iter();
    let mut is_pda = false;

    // Special implementations
    for attr in attrs {
        let attr_ident = attr.split('=').next().unwrap();
        match attr_ident {
            "pda_seed" => { // PDA based account
                assert_field!(field0, fields_iter, "pda_data : PDAAccountData");

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
                let multi_account = (&multi_account[1..multi_account.len() - 1]).split(';').collect::<Vec<&str>>();

                let ty: TokenStream = multi_account[0].parse().unwrap();
                let count: TokenStream = multi_account[1].parse().unwrap();
                let account_size: TokenStream = multi_account[2].parse().unwrap();

                assert_field!(field0, fields_iter, format!("multi_account_data : MultiAccountAccountData < {} >", multi_account[1]));

                impls.extend(quote! {
                    impl<#lifetimes> crate::state::program_account::MultiAccountAccount<'t> for #name<#lifetimes> {
                        type T = #ty;
                        const COUNT: usize = #count;
                        const ACCOUNT_SIZE: usize = #account_size;

                        fn get_account(&self, account_index: usize) -> Result<&solana_program::account_info::AccountInfo<'t>, solana_program::program_error::ProgramError> {
                            match self.accounts.get(&account_index) {
                                Some(&m) => Ok(m),
                                None => panic!()
                            }
                        }

                        fn modify(&mut self, index: usize, value: Self::T) {
                            self.modifications.insert(index, value);
                        }
                    }
                });

                // Add accounts field (IMPORTANT: no verification happens here, caller needs to make sure that the accounts match the pubkeys)
                fields.extend(quote! { accounts, modifications, });
                definition.extend(quote! {
                    accounts: std::collections::HashMap<usize, &'b solana_program::account_info::AccountInfo<'t>>,
                    pub modifications: std::collections::HashMap<usize, #ty>,
                });
                signature.extend(quote! {
                    accounts: std::collections::HashMap<usize, &'b solana_program::account_info::AccountInfo<'t>>,
                });
                init.extend(quote! {
                    let modifications = std::collections::HashMap::new();
                });
            },
            "partial_computation" => {
                assert_field!(field1, fields_iter, "instruction : u32");
                assert_field!(field1, fields_iter, "round : u32");
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
        if field.attrs.iter().any(|x| *x.path.get_ident().unwrap() == "pub_non_lazy") {
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
                            self.#field_name[..v.len()].copy_from_slice(&v[..]);
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
                        self.#field_name[offset..][..v.len()].copy_from_slice(&v[..]);
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

        impl<#lifetimes> #account_trait for #name<#lifetimes> {
            type T = #name<#lifetimes>;

            fn new(d: &'a mut [u8], #signature) -> Result<Self, solana_program::program_error::ProgramError> {
                crate::macros::guard!(d.len() == Self::SIZE, crate::error::ElusivError::InvalidAccountSize);
                #init
                Ok(#name { #fields })
            }
        }

        impl<#lifetimes> #name<#lifetimes> {
            // Access functions
            #functions
        }

        #impls
    }
}