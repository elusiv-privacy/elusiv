mod grammar;
mod interpreter;
mod parser;
mod storage;

use elusiv_computation::compute_unit_optimization;
use elusiv_utils::batched_instructions_tx_count;
use parser::try_parse_usize;
use proc_macro::TokenStream;
use proc_macro2::{Delimiter, TokenTree, TokenTree::*};
use quote::quote;
use std::collections::HashMap;
use std::iter::IntoIterator;

/// For computations that are so costly, that they cannot be performed in a single step
/// - This macro splits the computation you describe into `n` separate steps, all within a specified compute-unit budget.
/// - After `n` calls the computation is finished and the result is returned.
/// - The interpreter takes care of storage management between the computation steps.
/// - Each function requires an argument: `storage: T` with the following specification:
///     - for each type `Type` that needs to be used in multiple steps,
///     - a field `ram_type: RAMType<Type>` needs to exist on `storage`,
///     - where `RAMType` implements `elusiv_computation::RAM`.
///
/// # Macro output
/// - a function `name_partial(round: usize, param_0, .., param_k) -> Result<Option<ReturnType>, &'static str>`
/// - the count of rounds `NAME_ROUNDS_COUNT: usize` (function calls) required to complete the computation
/// - this means after `NAME_ROUNDS_COUNT` calls of `name_partial` it will return `Ok(Some(v))` if all went well
/// - **IMPORTANT**: it's the callers responsibility to make sure that if a single step of the computation return `Err(_)` no further computations are performed, otherwise undefined behavior would result
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
///         - **IMPORTANT**: the conditional expression is not allowed to be changed in any branch stmt (or have side effects), otherwise this leads to undefined behavior
///     - partial computations:
///         - for more powerful computations it's possible to call other elusiv_computations with `partial <<Id>> = <<Expr::Fn>>(..) { <<Stmt>> }`
///         - this results in `k - 1` rounds doing the computation and in the last round `k` the stmt is performed with the specified var
/// - `Expr`:
///     - unary-operators: deref and ref
///     - binary-operators: add, mul, sub, less, larger, equals (single Equal sign for both assignment and comparison)
///     - ids, literals, function calls, arrays,
///     - a safe unwrap expr: `unwrap <<Expr>>` will cause the function to return `Err(_)` if the expr matches `None`
/// - `Id`s can either be single idents or idents intersected by dots (:: accessors not allowed atm)
///
/// - **TODO**: add compute-budget documentation
///
/// # Usage
/// ```
/// elusiv_computations!(
///     main_fn, MainFnPartialComputationType, 1_400_000,
///     
///     double(&mut storage: Storage, v: u32) -> u32 {
///         {   /// 100
///             let mut a: u32 = v;
///         }
///         {   /// 1_000
///             return 2 * a;
///         }
///     }
///
///     main_fn(&mut storage: Storage, v: u32) -> u32 {
///         {   /// 100
///             let a: u32 = v;
///         }
///         {
///             partial v = double(storage, a) {
///                 a = v;
///             }
///         }
///         {
///             partial v = double(storage, a) {
///                 a = v;
///             }
///         }
///         {
///             return a;
///         }
///     }
/// );
/// ```
#[proc_macro]
pub fn elusiv_computations(attrs: TokenStream) -> TokenStream {
    impl_mult_step_computations(attrs.into()).into()
}

fn impl_mult_step_computations(stream: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let input: Vec<TokenTree> = stream.into_iter().collect();

    match &input[..] {
        [TokenTree::Ident(fn_name), TokenTree::Punct(_), TokenTree::Ident(computation_name), TokenTree::Punct(_), TokenTree::Literal(compute_budget_per_ix), TokenTree::Punct(_), tail @ ..] =>
        {
            let mut rounds_map = HashMap::new();
            let mut compute_units_map = HashMap::new();
            let stream = multi_step_computation(tail, &mut rounds_map, &mut compute_units_map);

            // Create compute unit stream for last partial computation
            let cus = compute_units_map[&fn_name.to_string()].clone();
            let compute_budget: u32 =
                try_parse_usize(&compute_budget_per_ix.to_string()).unwrap() as u32;
            let optimization =
                compute_unit_optimization(cus.iter().map(|&x| x as u32).collect(), compute_budget);
            let size = optimization.instructions.len();
            let total_rounds = optimization.total_rounds;
            let total_compute_units = optimization.total_compute_units;
            let computation_name: proc_macro2::TokenStream =
                computation_name.to_string().parse().unwrap();
            let instructions = optimization
                .instructions
                .iter()
                .fold(quote! {}, |acc, &rounds| {
                    assert!(rounds <= u8::MAX as u32);
                    let rounds: proc_macro2::TokenStream = rounds.to_string().parse().unwrap();
                    quote! { #acc #rounds, }
                });
            let tx_count =
                batched_instructions_tx_count(optimization.instructions.len(), compute_budget);

            quote! {
                pub struct #computation_name { }

                impl elusiv_computation::PartialComputation<#size> for #computation_name {
                    const TX_COUNT: usize = #tx_count;
                    const INSTRUCTION_ROUNDS: [u8; #size] = [ #instructions ];
                    const TOTAL_ROUNDS: u32 = #total_rounds;
                    const TOTAL_COMPUTE_UNITS: u32 = #total_compute_units;
                    const COMPUTE_BUDGET_PER_IX: u32 = #compute_budget;
                }

                #stream
            }
        }
        _ => panic!("Invalid syntax"),
    }
}

fn multi_step_computation(
    input: &[TokenTree],
    previous_computation_rounds: &mut HashMap<String, usize>,
    previous_compute_units: &mut HashMap<String, Vec<usize>>,
) -> proc_macro2::TokenStream {
    match input {
        // matches: `name{<generics>}(params) -> ty, {computation}`
        [Ident(id), Group(generics), Group(p), Punct(arrow0), Punct(arrow1), Ident(ty), Group(c), tail @ ..] =>
        {
            assert_eq!(p.delimiter(), Delimiter::Parenthesis);
            assert_eq!(c.delimiter(), Delimiter::Brace);
            assert_eq!(arrow0.to_string(), "-");
            assert_eq!(arrow1.to_string(), ">");

            let computation = c.stream().into_iter().collect();
            let id = &id.to_string();
            let params = p.stream();
            let ty = ty.to_string().parse().unwrap();

            // Optional generics
            let generics: proc_macro2::TokenStream =
                match &generics.stream().into_iter().collect::<Vec<TokenTree>>()[..] {
                    gen @ [TokenTree::Punct(open), .., TokenTree::Punct(close)] => {
                        assert_eq!(open.to_string(), "<");
                        assert_eq!(close.to_string(), ">");

                        let mut g = quote::quote! {};
                        for t in gen {
                            g.extend(proc_macro2::TokenStream::from(t.clone()));
                        }
                        g
                    }
                    _ => quote::quote! {},
                };

            let (rounds, compute_units, stream) = interpreter::interpret(
                computation,
                id,
                generics,
                params,
                ty,
                previous_computation_rounds,
                previous_compute_units,
            );
            previous_computation_rounds.insert(id.clone(), rounds);
            previous_compute_units
                .insert(format!("{}_zero", id.clone()), vec![0; compute_units.len()]);
            previous_compute_units.insert(id.clone(), compute_units);
            let tail =
                multi_step_computation(tail, previous_computation_rounds, previous_compute_units);

            quote! {
                #stream
                #tail
            }
        }

        // matches: `name(params) -> ty, {computation}`
        [Ident(id), Group(p), Punct(arrow0), Punct(arrow1), Ident(ty), Group(c), tail @ ..] => {
            assert_eq!(p.delimiter(), Delimiter::Parenthesis);
            assert_eq!(c.delimiter(), Delimiter::Brace);
            assert_eq!(arrow0.to_string(), "-");
            assert_eq!(arrow1.to_string(), ">");

            let computation = c.stream().into_iter().collect();
            let id = &id.to_string();
            let params = p.stream();
            let ty = ty.to_string().parse().unwrap();

            let (rounds, compute_units, stream) = interpreter::interpret(
                computation,
                id,
                quote! {},
                params,
                ty,
                previous_computation_rounds,
                previous_compute_units,
            );
            previous_computation_rounds.insert(id.clone(), rounds);
            previous_compute_units
                .insert(format!("{}_zero", id.clone()), vec![0; compute_units.len()]);
            previous_compute_units.insert(id.clone(), compute_units);
            let tail =
                multi_step_computation(tail, previous_computation_rounds, previous_compute_units);

            quote! {
                #stream
                #tail
            }
        }

        [] => {
            quote! {}
        }
        [Punct(comma), tail @ ..] => {
            assert_eq!(comma.to_string(), ",");

            multi_step_computation(tail, previous_computation_rounds, previous_compute_units)
        }

        tree => panic!("Invalid macro input {:?}", tree),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_eq_stream {
        ($a: expr, $b: expr) => {
            assert_eq!($a.to_string(), $b.to_string())
        };
    }

    #[test]
    fn test_simple_computation() {
        // This is the macro input
        let input = quote! {
            fn_two, FnTwoComputation,

            fn_name() -> isize {
                {   /// 10000
                    let a: isize = 8;
                    let b: isize = 10;
                }
                {   /// 10000
                    b = a + b * 3;
                }
                {   /// 10000
                    return alpha_beta.child.call(b);
                }
            },

            fn_two() -> isize {
                {   /// 10000
                    let c: isize = 12 + 2;
                }
                {   /// 10000
                    return c;
                }
            }
        };

        // And it should "compile" to this code:
        let expected = quote! {
            pub const FN_NAME_ROUNDS_COUNT: usize = 3usize;

            fn fn_name_partial(round: usize, ) -> Result<Option<isize>, ElusivError> {
                match round {
                    round if round == 0usize => {
                        let a: isize = 8;
                        let b: isize = 10;

                        storage.ram_isize.write(a, 0usize);
                        storage.ram_isize.write(b, 1usize);
                    },
                    round if round == 1usize => {
                        let a = storage.ram_isize.read(0usize);
                        let mut b = storage.ram_isize.read(1usize);

                        b = (a + (b * 3));

                        storage.ram_isize.write(b, 0usize);
                    },
                    round if round == 2usize => {
                        let b = storage.ram_isize.read(0usize);

                        return Ok(Some(alpha_beta.child.call(b,)));
                    },
                    _ => {}
                }
                Ok(None)
            }

            pub const FN_TWO_ROUNDS_COUNT: usize = 2usize;

            fn fn_two_partial(round: usize, ) -> Result<Option<isize>, ElusivError> {
                match round {
                    round if round == 0usize => {
                        let c: isize = (12 + 2);

                        storage.ram_isize.write(c, 0usize);
                    },
                    round if round == 1usize => {
                        let c = storage.ram_isize.read(0usize);

                        return Ok(Some(c));
                    },
                    _ => {}
                }
                Ok(None)
            }
        };

        let res = impl_mult_step_computations(input);
        assert_eq_stream!(res, expected);
    }
}
