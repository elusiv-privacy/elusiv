use std::{fs, str::FromStr};
use proc_macro2::TokenStream;
use quote::quote;
use serde::{Serialize, Deserialize};
use solana_program::pubkey::Pubkey;

#[derive(Serialize, Deserialize, Debug)]
struct Tokens {
    token: Vec<Token>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Token {
    symbol: String,
    mint: String,
    mint_devnet: String,
    active: bool,
    decimals: Option<u8>,
    price_base_exp: Option<u8>,
    min: u64,
    max: u64,
    pyth_usd_price_mainnet: String,
    pyth_usd_price_devnet: String,
}

pub fn impl_parse_tokens() -> TokenStream {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let file_name = manifest_dir + "/Token.toml";
    let contents = fs::read_to_string(&file_name).unwrap();
    let tokens: Tokens = toml::from_str(&contents).unwrap();
    let count = tokens.token.len();

    let mut content = quote!{};
    let mut symbols = quote!{};

    fn parse_pubkey(str: &str) -> TokenStream {
        format!("{:?}", Pubkey::from_str(str).unwrap().to_bytes()).parse().unwrap()
    }

    for (i, token) in tokens.token.iter().enumerate() {
        let sym: TokenStream = format!("{}_TOKEN_ID", token.symbol).parse().unwrap();
        let sym_fn: TokenStream = format!("{}_token", token.symbol.to_lowercase()).parse().unwrap();
        let id = i as u16;
        symbols.extend(quote!{
            #[cfg(feature = "instruction-abi")]
            pub const #sym: u16 = #id;

            #[cfg(feature = "instruction-abi")]
            pub const fn #sym_fn() -> ElusivToken {
                TOKENS[#i]
            }
        });

        let decimals = token.decimals.unwrap_or_default();
        let price_base_exp = token.price_base_exp.unwrap_or_default();
        let min = token.min;
        let max = token.max;
        let mint = if cfg!(feature = "devnet") {
            parse_pubkey(&token.mint_devnet)
        } else {
            parse_pubkey(&token.mint)
        };
        let pyth_usd_price_key = if cfg!(feature = "devnet") {
            parse_pubkey(&token.pyth_usd_price_devnet)
        } else {
            parse_pubkey(&token.pyth_usd_price_mainnet)
        };

        content.extend(quote!{
            ElusivToken {
                mint: Pubkey::new_from_array(#mint),
                decimals: #decimals,
                price_base_exp: #price_base_exp,
                pyth_usd_price_key: Pubkey::new_from_array(#pyth_usd_price_key),
                min: #min,
                max: #max,
            },
        });
    }

    quote! {
        #symbols

        pub const TOKENS: [ElusivToken; #count] = [
            #content
        ];
    }
}