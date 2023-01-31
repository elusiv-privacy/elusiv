use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::quote;

pub fn impl_repeat(input: TokenStream) -> TokenStream {
    let input: Vec<TokenTree> = input.into_iter().collect();
    let mut output = quote! {};

    match &input[..] {
        [TokenTree::Group(g), TokenTree::Punct(p), TokenTree::Literal(l)] => {
            assert_eq!(g.delimiter(), Delimiter::Brace);
            assert_eq!(p.to_string(), ",");
            let rounds: usize = l.to_string().parse().unwrap();

            let expr = g.stream().to_string();
            for i in 0..rounds {
                let i = i.to_string();
                let e: TokenStream = expr.clone().replace("_index", &i).parse().unwrap();
                output.extend(e);
            }
        }
        _ => panic!("Invalid syntax"),
    }

    output
}
