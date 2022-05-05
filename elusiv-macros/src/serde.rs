use quote::quote;

pub fn impl_serde(ast: &syn::DeriveInput) -> proc_macro2::TokenStream {
    let ident = &ast.ident.clone();
    let (impl_generics, ty_generics, where_clause) = &ast.generics.split_for_impl();
    let mut size = quote! { 0 };
    let mut ser = quote! {};
    let mut de = quote! {};

    match &ast.data {
        syn::Data::Enum(e) => {
            for (i, var) in e.variants.iter().enumerate() {
                let var_ident = var.ident.clone();
                let mut var_size = quote! {};
                let mut var_ser = quote! {};
                let mut var_de = quote! {};
                let mut fields = quote! {};
                let i = i as u8;

                for field in var.clone().fields {
                    let field_name = field.ident.clone().unwrap();
                    let field_ty = field.ty;

                    fields.extend(quote! { #field_name, });
                    var_size.extend(quote! { + <#field_ty>::SIZE });

                    var_ser.extend(quote! {
                        buffer.extend(&<#field_ty>::serialize(#field_name));
                    });
                    var_de.extend(quote! {
                        let (#field_name, data) = data.split_at(<#field_ty>::SIZE);
                        let #field_name = <#field_ty>::deserialize(#field_name);
                    });
                }

                size.extend(quote! { ,(1 #var_size) });
                de.extend(quote! {
                    #i => {
                        #var_de
                        #ident::#var_ident { #fields }
                    },
                });
                ser.extend(quote! {
                    #ident::#var_ident { #fields } => {
                        buffer.push(#i);
                        #var_ser
                    },
                });
            }

            quote! {
                impl #impl_generics SerDe for #ident #ty_generics #where_clause {
                    type T = Self;
                    const SIZE: usize = crate::macros::max!(#size);
                
                    fn deserialize(data: &[u8]) -> Self::T {
                        let (tag, data) = data.split_first().unwrap();
                        match tag {
                            #de
                            _ => { panic!("") }
                        }
                    }
                
                    fn serialize(value: Self::T) -> Vec<u8> {
                        let mut buffer = Vec::new();
                        match value {
                            #ser
                        }
                        buffer
                    }
                }
            }
        },
        syn::Data::Struct(s) => {
            let mut fields = quote! {};

            for field in s.clone().fields {
                let field_name = field.ident.clone().unwrap();
                let field_ty = field.ty;

                fields.extend(quote! { #field_name, });
                size.extend(quote! { + <#field_ty>::SIZE });

                ser.extend(quote! {
                    buffer.extend(&<#field_ty>::serialize(value.#field_name));
                });
                de.extend(quote! {
                    let (#field_name, data) = data.split_at(<#field_ty>::SIZE);
                    let #field_name = <#field_ty>::deserialize(#field_name);
                });
            }

            quote! {
                impl #impl_generics SerDe for #ident #ty_generics #where_clause {
                    type T = Self;
                    const SIZE: usize = #size;
                
                    fn deserialize(data: &[u8]) -> Self::T {
                        #de
                        #ident { #fields }
                    }
                
                    fn serialize(value: Self::T) -> Vec<u8> {
                        let buffer = Vec::new();
                        #ser
                        buffer
                    }
                }
            }
        },
        _ => { panic!("") }
    }
}