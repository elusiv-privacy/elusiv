use quote::quote;
use syn::Fields;
use proc_macro2::TokenStream;

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
            for var in e.variants.iter() {
                sizes.push(size_of_fields(&var.fields));
            }
            sizes.retain(|x| !x.is_empty());
            let mut size = quote!{};
            if !sizes.is_empty() {
                size = sizes[0].clone();
                for s in sizes {
                    size = quote!{ crate::bytes::max(#s, #size) }
                }
                size = quote!{ + #size };
            }

            quote! {
                impl #impl_generics BorshSerDeSized for #ident #ty_generics #where_clause {
                    const SIZE: usize = 1 #size;
                }
            }
        },
        syn::Data::Struct(s) => {
            sizes.push(size_of_fields(&s.fields));
            let size: TokenStream = sizes.iter().fold(quote!{}, |acc, x| quote!{ #acc #x });

            quote! {
                impl #impl_generics BorshSerDeSized for #ident #ty_generics #where_clause {
                    const SIZE: usize = #size;
                }
            }
        },
        _ => { panic!() }
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