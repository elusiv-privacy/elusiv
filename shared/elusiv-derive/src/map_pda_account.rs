use quote::{quote, ToTokens};
use proc_macro2::TokenStream;

pub fn impl_map_pda_account(ast: &syn::DeriveInput) -> TokenStream {
    let ident = &ast.ident.clone();
    let (impl_generics, ty_generics, where_clause) = &ast.generics.split_for_impl();

    assert!(ast.attrs.len() == 1);
    let ty = ast.attrs[0].clone().into_token_stream();

    let ident_str = ident.to_string();
    let pda_seed_string = if ident_str.ends_with("Account") {
        String::from(&ident_str[..ident_str.len() - "Account".len()])
    } else {
        ident_str
    };
    let pda_seed = pda_seed_string.as_bytes();
    let pda_seed_tokens: TokenStream = format!("{:?}", pda_seed).parse().unwrap();
    if pda_seed.len() > 32 {
        panic!("PDA-Seeds are only allowed to be <= 32 bytes in length (found {})", pda_seed.len());
    }

    quote! {
        impl #impl_generics elusiv_types::accounts::MapPDAAccount for #ident #ty_generics #where_clause {
            type T = #ty;
            const PROGRAM_ID: Pubkey = crate::PROGRAM_ID;
            const SEED: &'static [u8] = &#pda_seed_tokens;
        }
    }
}