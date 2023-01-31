use proc_macro2::TokenStream;
use quote::quote;

pub fn impl_enum_variant_index(ast: &syn::DeriveInput) -> TokenStream {
    let ident = &ast.ident.clone();
    let (impl_generics, ty_generics, where_clause) = &ast.generics.split_for_impl();
    let mut output = quote! {};

    match &ast.data {
        syn::Data::Enum(e) => {
            assert!(e.variants.len() <= u8::MAX as usize);

            for (i, var) in e.variants.iter().enumerate() {
                let id = var.ident.clone();
                let i = i as u8;

                output.extend(quote! {
                    #ident::#id { .. } => #i,
                })
            }
        }
        _ => {
            panic!()
        }
    }

    quote! {
        impl #impl_generics #ident #ty_generics #where_clause {
            pub fn variant_index(&self) -> u8 {
                match self {
                    #output
                    _ => panic!()
                }
            }
        }
    }
}
