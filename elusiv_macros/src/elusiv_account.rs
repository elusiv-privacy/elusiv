use std::string::ToString;
use syn::{ Type, DataStruct, Data };
use quote::{ quote, ToTokens };
use syn::Expr;
use super::utils::*;
use super::available_types::*;

pub fn impl_elusiv_account(ast: &syn::DeriveInput) -> proc_macro2::TokenStream {
    let name = &ast.ident.clone();

    fn get_struct(ast: syn::DeriveInput) -> DataStruct {
        if let Data::Struct(input) = ast.data { return input; } else { panic!("Struct not found"); }
    }
    let input = get_struct(ast.clone());

    let mut definition = quote! { };
    let mut init = quote! {};
    let mut fields = quote! {};
    let mut functions = quote! {};
    let mut total_size = quote! { 0 };

    for field in input.fields {
        // Add field to init
        let field_name = &field.ident.expect("Field has no name");
        let getter_name = ident_with_prefix(field_name, "get_");
        let setter_name = ident_with_prefix(field_name, "set_");
        fields.extend(quote! { #field_name, });

        // Attributed field
        if let Some(attr) = field.attrs.first() {
            let attr_name = attr.path.get_ident().unwrap().to_string();
            let attr_name = attr_name.as_str();

            match attr_name {
                "lazy_option" => {
                    let ty = field.ty;
                    definition.extend(quote! {
                        #field_name: Option<#ty>,
                    });
                    init.extend(quote! {
                        let #field_name = None;
                    });
                },
                _ => {  // Sub attributed accounts
                    let sub_attrs = attr.tokens.to_string();
                    let sub_attrs: Vec<&str> = (&sub_attrs[1..&sub_attrs.len() - 1]).split(",").collect();
                    let sub_attrs: Vec<proc_macro2::TokenStream> = sub_attrs.iter().map(|&s| {
                        let x: proc_macro2::TokenStream = s.parse().unwrap();
                        x
                    }).collect();

                    let ty = field.ty;

                    let size = sub_attrs[0].clone();

                    match attr_name {
                        "buffer" => {
                            definition.extend(quote! { pub #field_name: &'a mut [u8], });
                            init.extend(quote! {
                                let (#field_name, data) = data.split_at_mut(#size);
                            });
                            total_size.extend(quote! { + #size });
                        },
                        _ => {
                            definition.extend(quote! { pub #field_name: #ty, });

                            let byte_count = sub_attrs[1].clone();
                            let serialize = sub_attrs[2].clone();
                            let deserialize = sub_attrs[3].clone();

                            match attr_name {
                                "lazy_stack" => {
                                    init.extend(quote! {
                                        let (#field_name, data) = data.split_at_mut(stack_size(#size, #byte_count));
                                        let #field_name = LazyHeapStack::new(#field_name, #size, #byte_count, #serialize, #deserialize)?;
                                    });
                                    total_size.extend(quote! { + stack_size(#size, #byte_count) });
                                },
                                "queue" => {
                                    init.extend(quote! {
                                        let (#field_name, data) = data.split_at_mut(queue_size(#size, #byte_count));
                                        let #field_name = RingQueue::new(#field_name, #size, #byte_count, #serialize, #deserialize)?;
                                    });
                                    total_size.extend(quote! { + queue_size(#size, #byte_count) });
                                },
                                _ => { panic!("Unknown attribute {}", attr_name); }
                            }
                        }
                    }
                }
            }

            continue;
        }

        // Normal field
        let mut type_name = String::new();
        let mut field_size: Expr = int_expr(0);
        let mut is_array = false;
        match field.ty {
            Type::Path(type_path) => {
                type_name = type_path.into_token_stream().to_string();
            },
            Type::Array(type_array) => {
                let ty = *type_array.elem;
                type_name = format!("[{}]", ty.clone().into_token_stream().to_string());
                field_size = type_array.len;
                is_array = true;
            },
            _ => { }
        }

        // Find matching type
        let types = available_types();
        let t = types.iter().find(|&ty| ty.ident == type_name).unwrap();

        let size = t.byte_size;

        // Increase total_size
        if is_array {
            total_size.extend(quote! { + #size * #field_size });
        } else {
            field_size = int_expr(size);
            total_size.extend(quote! { + #size });
        }

        // Add mutable byte array field
        if let Some(def) = &t.def {
            definition.extend((*def)(field_name));
        } else {
            definition.extend(quote! {
                #field_name: &'a mut [u8],
            });
        }

        // Add mutable splitting
        if let Some(sp) = &t.split {
            init.extend((*sp)(field_name, field_size));
        } else {
            if is_array {
                init.extend(quote! {
                    let (#field_name, data) = data.split_at_mut(#size * #field_size);
                });
            } else {
                init.extend(quote! {
                    let (#field_name, data) = data.split_at_mut(#size);
                });
            }
        }

        // Add getter
        if let Some(getter) = &t.getter {
            functions.extend((*getter)(&getter_name, field_name));
        }

        // Add setter
        if let Some(setter) = &t.setter {
            functions.extend((*setter)(&setter_name, field_name));
        }
    }

    quote! {
        pub struct #name<'a> {
            #definition
        }

        impl<'a> #name<'a> {
            pub const TOTAL_SIZE: usize = #total_size;

            pub fn new(
                account_info: &solana_program::account_info::AccountInfo,
                data: &'a mut [u8],
            ) -> Result<Self, solana_program::program_error::ProgramError> {
                // Check for correct owner
                if *account_info.owner != crate::id() {
                    return Err(crate::error::ElusivError::InvalidAccount.into());
                }

                // Check for writability
                if !account_info.is_writable {
                    return Err(crate::error::ElusivError::InvalidAccount.into());
                }

                Self::from_data(data)
            }

            pub fn from_data(data: &'a mut [u8]) -> Result<Self, solana_program::program_error::ProgramError> {
                // Check for correct size
                if data.len() != Self::TOTAL_SIZE {
                    return Err(crate::error::ElusivError::InvalidAccountSize.into());
                }

                // All value initializations 
                #init

                Ok(
                    #name {
                        #fields
                    }
                )
            }

            // Access functions
            #functions
        }
    }
}