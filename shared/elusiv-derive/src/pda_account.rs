use elusiv_proc_macro_utils::pda;
use proc_macro2::TokenStream;
use quote::quote;

pub fn impl_pda_account(ast: &syn::DeriveInput) -> TokenStream {
    let ident = &ast.ident.clone();
    let (impl_generics, ty_generics, where_clause) = &ast.generics.split_for_impl();

    let ident_str = ident.to_string();
    let pda_seed_string = if ident_str.ends_with("Account") {
        String::from(&ident_str[..ident_str.len() - "Account".len()])
    } else {
        ident_str.clone()
    };
    let pda_seed = pda_seed_string.as_bytes();
    let pda_seed_tokens: TokenStream = format!("{:?}", pda_seed).parse().unwrap();
    if pda_seed.len() > 32 {
        panic!(
            "PDA-Seeds are only allowed to be <= 32 bytes in length (found {})",
            pda_seed.len()
        );
    }
    let (first_pubkey, first_bump) = pda(pda_seed);
    let first_pubkey: TokenStream = format!("{:?}", first_pubkey.to_bytes()).parse().unwrap();
    let ident_str = ident_str.as_str();

    if let syn::Data::Struct(_s) = &ast.data {
        // TODO: The first field always has to be [`PDAAccountData`] (serialization also needs to ensure this order)

        quote! {
            impl #impl_generics elusiv_types::accounts::PDAAccount for #ident #ty_generics #where_clause {
                const PROGRAM_ID: solana_program::pubkey::Pubkey = crate::PROGRAM_ID;
                const SEED: &'static [u8] = &#pda_seed_tokens;
                const FIRST_PDA: (solana_program::pubkey::Pubkey, u8) = (solana_program::pubkey::Pubkey::new_from_array(#first_pubkey), #first_bump);

                #[cfg(feature = "elusiv-client")]
                const IDENT: &'static str = #ident_str;
            }
        }
    } else {
        panic!("Only structs allowed")
    }
}
