mod interpreter;
mod grammar;
mod parser;

use proc_macro::TokenStream;
use proc_macro2::{ TokenTree, Delimiter };

/// # Arguments
/// 
/// # Macro output
/// - a function `name_partial(round: usize, ram_ty_0: RAM<Ty0>, .., ram_ty_n: Ram<Tyn>, param_0, .., param_k)`
/// - the count of rounds `NAME_ROUNDS_COUNT: usize` (function calls) required to complete the computation 
/// - the max compute units used per round `NAME_ROUND: [usize; NAME_ROUNDS_COUNT]`
/// 
/// # Examples
/// 
/// ```
/// elusiv_computation!(
///     fn_name (param0: ty0, param1: ty1, param2: ty2),
///     {
///         { // COMPUTE_UNITS_0
///             ..
///         }
///         { // COMPUTE_UNITS_1
///             ..
///         }
///         { // [PARTIAL_CUS0, PARTIAL_CUS1, ..]
///             .. 
///         }
///     }
/// )
/// ```
/// # Syntax
/// - Unary operators:
///     - There are currently no unary operators
///     - So no referencing or dereferencing
/// - Conditional statements:
///     - If and If/else statement (important, different to Rust these are not expressions)
///     - they also require parenthesis around the conditional expression
///     - `if (<<Expr>>) { <<Stmt>> } else { <<Stmt>> }`
/// - Loops:
///     - There are currently no loops inside of partial computations allowed
///     - Of course you can use loops in functions called by a partial computation
///     - But for creating multiple scopes with an iterator variable you can use the following syntax:
///     - ` for <<Id>> in <<Array>>:`
/// - If/else are statements and not expressions (other than in Rust)
/// - scopes (aka partial computations)
/// - variable definitions
/// - binary operators: +, *, - can be used, if the var type implements the op-traits
/// - no unary operators, use function calls instead
/// - variables that are used in a later scope again, need to have an explicit type annotation
/// - return: `return: <<Type>> <<Expr>>;`
#[proc_macro]
pub fn elusiv_computation(attrs: TokenStream) -> TokenStream {
    impl_multi_step_computation(attrs.into()).into()
}

fn impl_multi_step_computation(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let tree: Vec<TokenTree> = input.into_iter().collect();
    match &tree[..] {
        // matches: `ident (params) -> ty, {computation}`
        [
            TokenTree::Ident(name),
            TokenTree::Group(p),
            TokenTree::Punct(arrow0),
            TokenTree::Punct(arrow1),
            TokenTree::Ident(ty),
            TokenTree::Punct(comma),
            TokenTree::Group(c)
        ] => {
            assert_eq!(p.delimiter(), Delimiter::Parenthesis);
            assert_eq!(c.delimiter(), Delimiter::Brace);
            assert_eq!(arrow0.to_string(), "-");
            assert_eq!(arrow1.to_string(), ">");
            assert_eq!(comma.to_string(), ",");

            let computation = c.stream().into_iter().collect();
            let name = &name.to_string();
            let params = p.stream();
            let ty = (&ty.to_string()).parse().unwrap();

            interpreter::interpret(computation, name, params, ty).into()
        },
        tree => panic!("Invalid macro input {:?}", tree) 
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    macro_rules! assert_eq_stream {
        ($a: expr, $b: expr) => {
            assert_eq!($a.to_string(), $b.to_string())
        };
    }

    #[test]
    fn test_complex_computation() {
        // This is the macro input
        let input = quote!{
            fn_name () -> isize,
            {
                {
                    let a: isize = 8;
                }
                {
                    for i, value in [1,2,3] {
                        a = (a + b) * value;
                        partial r = compute() {
                            b = a * r;
                        };
                        //a = a * 2;
                    }
                }
                {
                    return a;
                }
            }
        };

        // And it should "compile" to this code:
        let expected = quote!{
            pub fn fn_name_partial(round: usize) -> Result<Option<()>, &'static str> {
                match round {
                    round if round >= 0usize && round < 1usize => {
                        let a: isize = 8;
                    },
                    round if round >= 1usize && round < 1usize + (3usize * (1 + COMPUTE_ROUNDS_COUNT + 1)) => {
                        let i = (round - 1usize) / (1 + COMPUTE_ROUNDS_COUNT + 1);
                        let round = (round - 1usize) % (1 + COMPUTE_ROUNDS_COUNT + 1);
                        let v = vec![1,2,3,];
                        let value = v[i];

                        match round {
                            round if round >= 0 && round < 1 {
                                a = ((a + b) * value);
                            },
                            round if round >= 1 && round < 1 + COMPUTE_ROUNDS_COUNT {
                                let round = round - 1;
                                if round < COMPUTE_ROUNDS_COUNT - 1 {
                                    match compute_partial(round,) {
                                        Ok(_) => {},
                                        Err(_) => { return Err("Partial computation error") }
                                    }
                                } else if round == COMPUTE_ROUNDS_COUNT - 1 {
                                    let r = match compute_partial(round,) {
                                        Ok(v) => v,
                                        Err(_) => { return Err("Partial computation error") }
                                    };

                                    b = a * r;
                                }
                            },
                            /*round if round >= 1 + COMPUTE_ROUNDS_COUNT && round < 1 + COMPUTE_ROUNDS_COUNT + 1 {
                                a = a * 2;
                            },*/
                            _ => {}
                        }
                    },
                    round if round >= 1usize + (3usize * (1)) && round < 2usize + (3usize * (1)) => {
                        return Some(a);
                    },
                    _ => {}
                }
                Ok(None)
            }
        };

        let res = impl_multi_step_computation(input);

        /*let file: syn::File = syn::parse2(res.clone()).unwrap();
        let pretty = prettyplease::unparse(&file);
        println!("{}", pretty);*/

        assert_eq_stream!(res, expected);
    }
}

/*pub fn fn_name_partial (round : usize) -> Result < Option < () > , & 'static str > {
    match round {
        round if round >= 0usize && round < 1usize => {
            let a : isize = 8 ;
        } ,
        round if round >= 1usize && round < 1usize + (3usize * (COMPUTE_ROUNDS_COUNT + 1)) => {
            let i = (round - 1usize) / COMPUTE_ROUNDS_COUNT + 1 ;
            let v = vec ! [1 , 2 , 3 ,] ;
            let value = v [i] ;
            match round {
                round if round >= 0 && round < 0 + 1 => {
                    let round = round - 0;
                    a = ((a + b) * value) ;
                } ,
                round if round >= 0 + 1 && round < 0 + 1 + COMPUTE_ROUNDS_COUNT + 1 => {
                    let round = round - (0 + 1);
                    if round < COMPUTE_ROUNDS_COUNT - 1 {
                        match compute_partial (round ,) {
                            Ok (_) => { } , Err (_) => { return Err ("Partial computation error") }
                        }
                    } else if round == COMPUTE_ROUNDS_COUNT - 1 {
                        let r = match compute_partial (round ,) {
                            Ok (v) => v ,
                            Err (_) => { return Err ("Partial computation error") }
                        } ;
                        b = (a * r) ;
                    }
                },
                _ => { }
            }
        } ,
        round if round >= 1usize + (3usize * (COMPUTE_ROUNDS_COUNT + 1)) && round < 2usize + (3usize * (COMPUTE_ROUNDS_COUNT + 1)) => {
            return Some (a) ;
        },
        _ => { }
    }
    Ok (None)
}*/