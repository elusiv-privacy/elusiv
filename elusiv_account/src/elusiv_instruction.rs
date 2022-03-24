use std::string::ToString;
use syn::{ Type, Field, Ident };
use quote::{ quote, ToTokens };

pub fn impl_elusiv_instruction(ast: &syn::DeriveInput) -> proc_macro2::TokenStream {
    let name = &ast.ident;
    let mut imp= quote! {};

    match &ast.data {
        syn::Data::Enum(e) => {
            for (i, var) in e.variants.iter().enumerate() {
                let i = i as u8;
                let var_name = &var.ident;
                let mut variant = quote! {};
                let mut var_imp = quote! {};
                let mut init = quote! {};

                // Add variant fields
                for field in var.fields.iter() {
                    let field_name = field.ident.clone().unwrap();

                    variant.extend(quote! { #field, });
                    var_imp.extend(get_field_unpack(field.clone(), &field_name));
                    init.extend(quote! { #field_name, });
                }

                // Add variant implementation
                imp.extend(quote! {
                    #i => {
                        #var_imp

                        Ok(Self::#var_name { #init })
                    },
                });
            }
        },
        _ => {}
    }
    quote! {
        impl #name {
            pub fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
                let (&tag, data) = data
                    .split_first()
                    .ok_or(solana_program::program_error::ProgramError::InvalidInstructionData)?;
                
                match tag {
                    #imp
                    _ => Err(solana_program::program_error::ProgramError::InvalidArgument)
                }
            }
        }
     }
}

pub fn get_field_unpack(field: Field, field_name: &Ident) -> proc_macro2::TokenStream {
    match field.ty {
        Type::Path(type_path) => {
            let type_name = type_path.into_token_stream().to_string();

            match type_name.as_str() {
                "u64" => { quote! {
                    let (#field_name, data) = unpack_u64(&data)?;
                } },
                "U256" => { quote! {
                    let (#field_name, data) = unpack_u256(&data)?;
                } },
                "ProofData" => { quote! {
                    let (#field_name, data) = unpack_proof_data(&data)?;
                } },
                _ => { panic!("Invalid field {} type {}", field_name, type_name); }
            }
        },
        _ => { panic!("Invalid field {}", field_name); }
    }
}