mod interpreter;
mod grammar;
mod parser;

use proc_macro::TokenStream;
use proc_macro2::{ TokenTree, Delimiter };

/// For computations that are so costly, that they cannot be performed in a single step
/// - this macro splits the computation you describe into `n` separate steps
/// - after `n` program calls the computation is finished and the result returned
/// 
/// # Macro output
/// - a function `name_partial(round: usize, param_0, .., param_k) -> Result<Option<ReturnType>, &'static str>`
/// - the count of rounds `NAME_ROUNDS_COUNT: usize` (function calls) required to complete the computation 
/// - this means after `NAME_ROUNDS_COUNT` calls of `name_partial` it will return `Ok(Some(v))` if all went well
/// - IMPORTANT: it's the callers responsibility to make sure that if a single step of the computation return `Err(_)` no further computations are performed, otherwise undefinied behaviour would result
/// 
/// # Syntax
/// - A `Computation` consists of multiple `ComputationScope`s
/// - `ComputationScope`:
///     - contains a `Stmt` and manages reading/writing to the RAM
///     - `{ <<Stmt>> }`
/// - `Stmt`:
///     - variable declaration: `let mut <<Id>>: <<Type>> = <<Expr>>;` with `Type` being String idents
///     - assignment and returning: `<<Id>> = <<Expr>>;`, `return <<Expr>>;`
///     - collections: multiple statements
///     - for-loops:
///         - `for <<Id>>, <<Id>> in [e0, .., en] { <<Stmt>> }`
///         - with an iterator and value ident
///     - conditionals:
///         - `if (<<Expr>>) { <<Stmt>> }` or `if (<<Expr>>) { <<Stmt>> } else { <<Stmt>> }`
///         - IMPORTANT: the conditional expression is not allowed to be changed in any branch stmt, otherwise this leads to undefined behaviour
///     - partial computations:
///         - for more powerful computations it's possible to call other elusiv_computations with `partial <<Id>> = <<Expr::Fn>>(..) { <<Stmt>> }`
///         - this results in `k - 1` rounds doing the computation and in the last round `k` the stmt is performed with the specified var
/// - `Expr`:
///     - ids, literals, binary-operators, function calls, arrays, 
///     - a safe unwrap expr: `unwrap <<Expr>>` will cause the function to return `Err(_)` if the expr matches `None`
/// - `Id`s can either be single idents or idents intersected by dots
/// - at the moment no unary operators, so use function calls instead
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
        ($a: expr, $b: expr) => { assert_eq!($a.to_string(), $b.to_string()) };
    }

    #[test]
    fn test_complex_computation() {
        // This is the macro input
        let input = quote!{
            fn_name () -> isize,
            {
                {
                    let a: isize = 8;
                    let b: isize = 10;
                }
                {
                    for i, value in [1,2,3] {
                        a = (a + b) * value;
                        partial r = compute() {
                            b = a * r;
                        };
                        a = a * 2;
                    }
                }
                {
                    if (condition) {
                        partial r = compute() {
                            b = a * r;
                        };
                    } else {
                        b = b + a;
                    }
                }
                {
                    return a;
                }
            }
        };

        // And it should "compile" to this code:
        let expected = quote!{
            pub fn fn_name_partial(round: usize,) -> Result<Option<isize>, &'static str> {
                match round {
                    round if round >= 0usize && round < 1usize => {
                        let a: isize = 8;
                        let b: isize = 10;
                    },
                    round if round >= 1usize && round < 1usize + (3usize * (0 + 1 + (COMPUTE_ROUNDS_COUNT + 0) + 1)) => {
                        let i = (round - 1usize) / (0 + 1 + (COMPUTE_ROUNDS_COUNT + 0) + 1);
                        let round = (round - 1usize) % (0 + 1 + (COMPUTE_ROUNDS_COUNT + 0) + 1);
                        let v = vec![1,2,3,];
                        let value = v[i];

                        match round {
                            round if round >= 0 && round < 0 + 1 => {
                                let round = round - (0);

                                a = ((a + b) * value);
                            },
                            round if round >= 0 + 1 && round < 0 + 1 + (COMPUTE_ROUNDS_COUNT + 0) => {
                                let round = round - (0 + 1);

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

                                    b = (a * r);
                                }
                            },
                            round if round >= 0 + 1 + (COMPUTE_ROUNDS_COUNT + 0) && round < 0 + 1 + (COMPUTE_ROUNDS_COUNT + 0) + 1 => {
                                let round = round - (0 + 1 + (COMPUTE_ROUNDS_COUNT + 0));

                                a = (a * 2);
                            },
                            _ => {}
                        }
                    },
                    round if round >= 1usize + (3usize * (0 + 1 + (COMPUTE_ROUNDS_COUNT + 0) + 1)) &&
                        round < 1usize + (3usize * (0 + 1 + (COMPUTE_ROUNDS_COUNT + 0) + 1)) + (COMPUTE_ROUNDS_COUNT + 0) =>
                    {
                        let round = round - (1usize + (3usize * (0 + 1 + (COMPUTE_ROUNDS_COUNT + 0) + 1)));
                        if (condition) {
                            if round < (COMPUTE_ROUNDS_COUNT + 0) {
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

                                    b = (a * r);
                                }
                            }
                        } else {
                            if round < 1 {
                                b = (b + a);
                            }
                        }
                    },
                    round if round >= 1usize + (3usize * (0 + 1 + (COMPUTE_ROUNDS_COUNT + 0) + 1)) + (COMPUTE_ROUNDS_COUNT + 0) &&
                    round < 2usize + (3usize * (0 + 1 + (COMPUTE_ROUNDS_COUNT + 0) + 1)) + (COMPUTE_ROUNDS_COUNT + 0) =>
                    {
                        return Some(a);
                    },
                    _ => {}
                }
                Ok(None)
            }
        };

        let res = impl_multi_step_computation(input);
        assert_eq_stream!(res, expected);
    }
}