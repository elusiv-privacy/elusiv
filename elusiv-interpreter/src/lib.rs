mod interpreter;
mod grammar;
mod parser;
mod storage;

use proc_macro::TokenStream;
use proc_macro2::{ TokenTree, Delimiter };

/// For computations that are so costly, that they cannot be performed in a single step
/// - this macro splits the computation you describe into `n` separate steps
/// - after `n` program calls the computation is finished and the result returned
/// - the interpreter takes care of storage management
/// - for each type that needs to be used in multiple steps, a object `ram_type: RAM<Type>` is required with the following interface:
///     - `write(value: Type, index: usize)`
///     - `read(index: usize) -> Type`
///     - `free(index: usize)`
///     - `inc_frame(offset: usize)` and `inc_frame(offset: usize)` (required for function calls)
/// 
/// # Macro output
/// - a function `name_partial(round: usize, param_0, .., param_k) -> Result<Option<ReturnType>, &'static str>`
/// - the count of rounds `NAME_ROUNDS_COUNT: usize` (function calls) required to complete the computation 
/// - this means after `NAME_ROUNDS_COUNT` calls of `name_partial` it will return `Ok(Some(v))` if all went well
/// - **IMPORTANT**: it's the callers responsibility to make sure that if a single step of the computation return `Err(_)` no further computations are performed, otherwise undefinied behaviour would result
/// 
/// # Syntax
/// - A `Computation` consists of multiple `ComputationScope`s
/// - `ComputationScope`:
///     - contains a `Stmt` and manages reading/writing to the RAM
///     - `{ <<Stmt>> }`
/// - `Stmt`:
///     - variable declaration:
///         - `let mut <<Id>>: <<Type>> = <<Expr>>;` with `Type` being String idents
///         - no shadowing is allowed
///     - assignment and returning: `<<Id>> = <<Expr>>;`, `return <<Expr>>;` (no field accesses allowed for assignments)
///     - collections: multiple statements
///     - for-loops:
///         - `for <<Id>>, <<Id>> in [e0, .., en] { <<Stmt>> }`
///         - with an iterator and value ident
///         - **IMPORTANT**: declarations that require writing are not allowed in for-loops (only local vars or assignments)
///     - conditionals:
///         - `if (<<Expr>>) { <<Stmt>> }` or `if (<<Expr>>) { <<Stmt>> } else { <<Stmt>> }`
///         - **IMPORTANT**: the conditional expression is not allowed to be changed in any branch stmt (or have side effects), otherwise this leads to undefined behaviour
///     - partial computations:
///         - for more powerful computations it's possible to call other elusiv_computations with `partial <<Id>> = <<Expr::Fn>>(..) { <<Stmt>> }`
///         - this results in `k - 1` rounds doing the computation and in the last round `k` the stmt is performed with the specified var
/// - `Expr`:
///     - unary-operators: deref and ref
///     - binary-operators: add, mul, sub, less, larger, equals (single Equal sign for both assignment and comparison)
///     - ids, literals, function calls, arrays, 
///     - a safe unwrap expr: `unwrap <<Expr>>` will cause the function to return `Err(_)` if the expr matches `None`
/// - `Id`s can either be single idents or idents intersected by dots (:: accessors not allowed atm)
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
            TokenTree::Group(c)
        ] => {
            assert_eq!(p.delimiter(), Delimiter::Parenthesis);
            assert_eq!(c.delimiter(), Delimiter::Brace);
            assert_eq!(arrow0.to_string(), "-");
            assert_eq!(arrow1.to_string(), ">");

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
            fn_name (ram_isize: &mut RAM<isize>) -> isize {
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
                        a = &a * 2;
                    }
                }
                {
                    if (a = b) {
                        partial r = compute() {
                            b = *a * *r;
                        };
                    } else {
                        b = b.field.0.tuple_field + &(a.fun());
                    }
                }
                {
                    return alpha_beta.child.call(b);
                }
            }
        };

        // And it should "compile" to this code:
        let expected = quote!{
            const FN_NAME_ROUNDS_COUNT: usize = 2usize + (3usize * (1 + (COMPUTE_ROUNDS_COUNT) + 1)) + (COMPUTE_ROUNDS_COUNT); 

            pub fn fn_name_partial(round: usize, ram_isize: &mut RAM<isize>) -> Result<Option<isize>, &'static str> {
                match round {
                    round if round >= 0usize && round < 1usize => {
                        let a: isize = 8;
                        let b: isize = 10;

                        ram_isize.write(a, 0usize);
                        ram_isize.write(b, 1usize);
                    },
                    round if round >= 1usize && round < 1usize + (3usize * (1 + (COMPUTE_ROUNDS_COUNT) + 1)) => {
                        let round = round - (1usize);

                        let mut a = ram_isize.read(0usize);
                        let mut b = ram_isize.read(1usize);

                        ram_isize.inc_frame(2usize);

                        {
                            let i = round / (1 + (COMPUTE_ROUNDS_COUNT) + 1);
                            let value = vec![1,2,3,][i];
                            let round = round % (1 + (COMPUTE_ROUNDS_COUNT) + 1);

                            match round {
                                round if round >= 0 && round < 1 => {
                                    let round = round - (0);

                                    a = ((a + b) * value);
                                },
                                round if round >= 1 && round < 1 + (COMPUTE_ROUNDS_COUNT) => {
                                    let round = round - (1);

                                    if round < COMPUTE_ROUNDS_COUNT - 1 {
                                        match compute_partial(round,) {
                                            Ok(_) => {},
                                            Err(_) => { return Err("Partial computation error") }
                                        }
                                    } else if round == COMPUTE_ROUNDS_COUNT - 1 {
                                        let r = match compute_partial(round,) {
                                            Ok(Some(v)) => v,
                                            _ => { return Err("Partial computation error") }
                                        };

                                        b = (a * r);
                                    }
                                },
                                round if round >= 1 + (COMPUTE_ROUNDS_COUNT) && round < 1 + (COMPUTE_ROUNDS_COUNT) + 1 => {
                                    let round = round - (1 + (COMPUTE_ROUNDS_COUNT));

                                    a = ((&a) * 2);
                                },
                                _ => {}
                            }
                        }

                        ram_isize.dec_frame(2usize);

                        ram_isize.write(a, 0usize);
                        ram_isize.write(b, 1usize);
                    },
                    round if round >= 1usize + (3usize * (1 + (COMPUTE_ROUNDS_COUNT) + 1)) &&
                        round < 1usize + (3usize * (1 + (COMPUTE_ROUNDS_COUNT) + 1)) + (COMPUTE_ROUNDS_COUNT) =>
                    {
                        let round = round - (1usize + (3usize * (1 + (COMPUTE_ROUNDS_COUNT) + 1)));

                        let a = ram_isize.read(0usize);
                        let mut b = ram_isize.read(1usize);

                        if round == (COMPUTE_ROUNDS_COUNT) - 1 {
                            ram_isize.free(0usize);
                        }

                        ram_isize.inc_frame(2usize);

                        if ((a == b)) {
                            if round < (COMPUTE_ROUNDS_COUNT) {
                                if round < COMPUTE_ROUNDS_COUNT - 1 {
                                    match compute_partial(round,) {
                                        Ok(_) => {},
                                        Err(_) => { return Err("Partial computation error") }
                                    }
                                } else if round == COMPUTE_ROUNDS_COUNT - 1 {
                                    let r = match compute_partial(round,) {
                                        Ok(Some(v)) => v,
                                        _ => { return Err("Partial computation error") }
                                    };

                                    b = ((*a) * (*r));
                                }
                            }
                        } else {
                            if round < 1 {
                                b = (b.field.0.tuple_field + (&a.fun()));
                            }
                        }

                        ram_isize.dec_frame(2usize);

                        if round < (COMPUTE_ROUNDS_COUNT) - 1 {
                            ram_isize.write(b, 1usize);
                        } else {
                            ram_isize.free(1usize);
                            ram_isize.write(b, 0usize);
                        }
                    },
                    round if round >= 1usize + (3usize * (1 + (COMPUTE_ROUNDS_COUNT) + 1)) + (COMPUTE_ROUNDS_COUNT) &&
                    round < 2usize + (3usize * (1 + (COMPUTE_ROUNDS_COUNT) + 1)) + (COMPUTE_ROUNDS_COUNT) =>
                    {
                        let b = ram_isize.read(0usize);
                        ram_isize.free(0usize);

                        return Ok(Some(alpha_beta.child.call(b,)));
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