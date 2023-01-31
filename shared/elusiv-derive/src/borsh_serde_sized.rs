use proc_macro2::TokenStream;
use quote::quote;
use syn::Fields;

pub fn impl_borsh_serde_sized(ast: &syn::DeriveInput) -> TokenStream {
    let ident = &ast.ident.clone();
    let (impl_generics, ty_generics, where_clause) = &ast.generics.split_for_impl();
    let mut sizes = Vec::new();

    fn size_of_fields(fields: &Fields) -> TokenStream {
        let mut var_size = quote! {};
        for field in fields {
            let field_ty = &field.ty;
            if var_size.is_empty() {
                var_size.extend(quote! { <#field_ty>::SIZE });
            } else {
                var_size.extend(quote! { + <#field_ty>::SIZE });
            }
        }
        var_size
    }

    match &ast.data {
        syn::Data::Enum(e) => {
            let mut len = quote! {};

            for (i, var) in e.variants.iter().enumerate() {
                let i = i as u8;
                let size = size_of_fields(&var.fields);

                if size.is_empty() {
                    len.extend(quote! { #i => { 0 }, });
                } else {
                    len.extend(quote! { #i => { #size }, });
                }
                sizes.push(size);
            }
            sizes.retain(|x| !x.is_empty());

            len = if sizes.is_empty() {
                quote! { 0 }
            } else {
                quote! {
                    match variant_index {
                        #len
                        _ => panic!()
                    }
                }
            };

            let mut size = quote! {};
            if !sizes.is_empty() {
                size = sizes[0].clone();
                for s in sizes {
                    size = quote! { elusiv_types::bytes::max(#s, #size) }
                }
                size = quote! { + #size };
            }

            quote! {
                impl #impl_generics elusiv_types::bytes::BorshSerDeSized for #ident #ty_generics #where_clause {
                    const SIZE: usize = 1 #size;
                }

                impl #impl_generics elusiv_types::bytes::BorshSerDeSizedEnum for #ident #ty_generics #where_clause {
                    fn len(variant_index: u8) -> usize {
                        #len
                    }
                }
            }
        }
        syn::Data::Struct(s) => {
            sizes.push(size_of_fields(&s.fields));
            let size: TokenStream = sizes.iter().fold(quote! {}, |acc, x| quote! { #acc #x });

            quote! {
                impl #impl_generics elusiv_types::bytes::BorshSerDeSized for #ident #ty_generics #where_clause {
                    const SIZE: usize = #size;
                }
            }
        }
        _ => {
            panic!()
        }
    }
}

pub fn impl_borsh_serde_placeholder(ast: &syn::DeriveInput) -> TokenStream {
    let ident = &ast.ident.clone();
    let (impl_generics, ty_generics, where_clause) = &ast.generics.split_for_impl();

    quote! {
        impl #impl_generics borsh::BorshDeserialize for #ident #ty_generics #where_clause {
            fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> { panic!() }
        }

        impl #impl_generics borsh::BorshSerialize for #ident #ty_generics #where_clause {
            fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> { panic!() }
        }
    }
}
