use quote::quote;

pub fn impl_serde(ast: &syn::DeriveInput) -> proc_macro2::TokenStream {
    let ident = &ast.ident.clone();
    let (impl_generics, ty_generics, where_clause) = &ast.generics.split_for_impl();
    let mut size = quote! { 0 };
    let mut ser = quote! {};
    let mut de = quote! {};

    match &ast.data {
        syn::Data::Enum(e) => {
            let mut sizes = quote!{};
            for (i, var) in e.variants.iter().enumerate() {
                let var_ident = var.ident.clone();
                let mut var_size = quote! { 0 };
                let mut var_ser = quote! {};
                let mut var_de = quote! {};
                let mut fields = quote! {};
                let i = i as u8;

                for field in var.clone().fields {
                    let field_name = field.ident.clone().unwrap();
                    let field_ty = field.ty;

                    // Serialization
                    var_ser.extend(quote! {
                        <#field_ty>::serialize(#field_name, &mut data[(#var_size)..(#var_size + <#field_ty>::SIZE)]);
                    });

                    // Deserialization
                    var_de.extend(quote! {
                        let (#field_name, data) = data.split_at(<#field_ty>::SIZE);
                        let #field_name = <#field_ty>::deserialize(#field_name);
                    });

                    fields.extend(quote! { #field_name, });
                    var_size.extend(quote! { + <#field_ty>::SIZE });
                }

                size.extend(quote! { ,(1 + #var_size) });

                // Serialization
                ser.extend(quote! {
                    #ident::#var_ident { #fields } => {
                        data[0] = #i;
                        #var_ser
                    },
                });

                // Deserialization
                de.extend(quote! {
                    #i => {
                        #var_de
                        #ident::#var_ident { #fields }
                    },
                });

                sizes.extend(quote!{
                    #ident::#var_ident { .. } => #var_size + 1,
                });
            }

            quote! {
                impl #impl_generics SerDe for #ident #ty_generics #where_clause {
                    type T = Self;
                    const SIZE: usize = 1 + crate::macros::max!(#size);
                
                    fn deserialize(data: &[u8]) -> Self::T {
                        let (tag, data) = data.split_first().unwrap();
                        match tag {
                            #de
                            _ => { panic!() }
                        }
                    }
                
                    fn serialize(value: Self::T, data: &mut [u8]) {
                        match value {
                            #ser
                            _ => { panic!() }
                        }
                    }

                    fn serialize_vec(value: Self::T) -> Vec<u8> {
                        let mut v = vec![0; value.size()];
                        Self::serialize(value, &mut v[..]);
                        v
                    }
                }

                impl #ty_generics #ident #ty_generics #where_clause {
                    pub fn size(&self) -> usize {
                        match self {
                            #sizes
                        }
                    }
                }
            }
        },
        syn::Data::Struct(s) => {
            let mut fields = quote!{};

            for field in s.clone().fields {
                let field_name = field.ident.clone().unwrap();
                let field_ty = field.ty;

                // Serialization
                ser.extend(quote! {
                    <#field_ty>::serialize(value.#field_name, &mut data[#size..#size + <#field_ty>::SIZE]);
                });

                // Deserialization
                de.extend(quote! {
                    let (#field_name, data) = data.split_at(<#field_ty>::SIZE);
                    let #field_name = <#field_ty>::deserialize(#field_name);
                });

                fields.extend(quote! { #field_name, });
                size.extend(quote! { + <#field_ty>::SIZE });
            }

            quote! {
                impl #impl_generics SerDe for #ident #ty_generics #where_clause {
                    type T = Self;
                    const SIZE: usize = #size;
                
                    fn deserialize(data: &[u8]) -> Self::T {
                        assert!(data.len() >= Self::SIZE);
                        #de
                        #ident { #fields }
                    }
                
                    fn serialize(value: Self::T, data: &mut [u8]) {
                        assert!(data.len() >= Self::SIZE);
                        #ser
                    }
                }
            }
        },
        _ => { panic!("") }
    }
}