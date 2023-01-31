use proc_macro2::TokenStream;
use quote::quote;

pub fn impl_byte_backed_jit(ast: &syn::DeriveInput) -> TokenStream {
    let ident = &ast.ident.clone();
    let (impl_generics, ty_generics, where_clause) = &ast.generics.split_for_impl();
    let mut content = quote! {};
    let mut fields = quote! {};

    match &ast.data {
        syn::Data::Struct(s) => {
            for field in &s.fields {
                let field_ty = &field.ty;
                let field_ident = field.clone().ident.unwrap();
                fields.extend(quote! { #field_ident, });
                content.extend(quote! {
                    let (#field_ident, data) = data.split_at_mut(<#field_ty>::SIZE);
                    let #field_ident = <#field_ty>::new(#field_ident);
                });
            }

            quote! {
                impl #impl_generics #ident #ty_generics #where_clause {
                    pub fn new(data: &'a mut [u8]) -> Self {
                        #content
                        Self { #fields }
                    }
                }
            }
        }
        _ => {
            panic!()
        }
    }
}
