use super::storage::*;
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashMap;
use std::string::ToString;

/// A computation consists of n scopes
/// - a scope is a single part of the computation
/// - after sequential execution of all n scopes, the computation is finished
#[derive(Debug, Clone)]
pub struct Computation {
    pub scopes: Vec<ComputationScope>,
}

/// In order to achieve more complex partial computations, we need to allow for repeated execution of the same code
/// This means depending on the statements/expressions contained in a scope, this scope can be repeated multiple finite times
#[derive(Debug, Clone)]
pub struct ComputationScope {
    pub stmt: Stmt,

    // Memory reading, freeing and writing happening in this scope
    pub read: Vec<MemoryRead>,
    pub free: Vec<MemoryId>,
    pub write: Vec<MemoryId>,

    // Compute units
    pub scope_wide_compute_units: Option<CUs>,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    // Non-terminal stmts
    Collection(Vec<Stmt>),
    IfElse(Expr, Box<Stmt>, Option<Box<Stmt>>),
    For(SingleId, SingleId, Expr, Box<Stmt>),

    // Terminal stmts
    Let(SingleId, bool, Type, Expr), // Let.1 is the mutability
    Assign(Id, Expr),
    // `partial v = fn<generics>(params) { <<Stmt+>> }`
    Partial(SingleId, Expr, Box<Stmt>),
    Return(Expr),

    // Stmt with explicitly known compute units
    ComputeUnitStmt(CUs, Box<Stmt>),

    // Invalid stmt
    Invalid,
}

#[derive(Debug, Clone)]
pub enum Id {
    Single(SingleId),
    Path(PathId),
}

#[derive(Debug, Clone)]
pub struct SingleId(pub String);

#[derive(Debug, Clone)]
pub struct PathId(pub Vec<String>);

/// Types are only allowed as Strings without punctuations
///
/// # Examples
///
/// - allowed: `let a: Type`
/// - not allowed: `let a: Option<Type>` (here you need to first define a type with: `type TypeOpt = Option<Type>` and use `TypeOpt`)
#[derive(Debug, Clone)]
pub struct Type(pub String);

#[derive(Debug, Clone)]
pub enum Expr {
    UnOp(UnOp, Box<Expr>),
    BinOp(Box<Expr>, BinOp, Box<Expr>),
    Literal(String),
    Id(Id),
    // fn_name<generics>(params)
    Fn(Id, Vec<Id>, Vec<Expr>),
    Array(Vec<Expr>),
    Unwrap(Box<Expr>),
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinOp {
    Mul,
    Add,
    Sub,
    LessThan,
    LargerThan,
    Equals,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnOp {
    Ref,
    Deref,
    Not,
}

#[derive(Debug, Clone)]
pub enum CUs {
    Single(usize),
    Multiple(String), // ident of the array containing the CUs

    Collection(Vec<CUs>),

    // Maps a certain value of a variable to a CUs (the value none is the any-case)
    Mapping {
        ident: String,
        mapping: Vec<ComputeUnitMapping>,
    },
}

impl CUs {
    pub fn apply_mapping(&self, iter_id: &str, var_id: &str, iter: usize, var: usize) -> CUs {
        match self {
            CUs::Mapping { ident, mapping } => {
                if iter_id == ident {
                    for m in mapping {
                        if let Some(p) = m.pattern {
                            if p == iter {
                                return m.value.clone();
                            }
                        } else {
                            return m.value.clone();
                        }
                    }
                } else if var_id == ident {
                    for m in mapping {
                        if let Some(p) = m.pattern {
                            if p == var {
                                return m.value.clone();
                            }
                        } else {
                            return m.value.clone();
                        }
                    }
                }
                panic!(
                    "Invalid compute units mapping ({}, {:?}) with ({}, {})",
                    ident, mapping, iter_id, var_id
                );
            }
            CUs::Collection(c) => {
                let mut cus = Vec::new();
                for c in c {
                    cus.push(c.apply_mapping(iter_id, var_id, iter, var))
                }
                CUs::Collection(cus)
            }
            c => c.clone(),
        }
    }

    pub fn reduce(&self) -> CUs {
        match self {
            CUs::Collection(c) => {
                let mut cus = Vec::new();
                for c in c {
                    match c.reduce() {
                        CUs::Collection(c) => cus.extend(c),
                        c => cus.push(c),
                    }
                }
                CUs::Collection(cus)
            }
            c => c.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ComputeUnitMapping {
    pub pattern: Option<usize>,
    pub value: CUs,
}

/// - `rounds` == None means that the Stmt uses the same round as the scope or other stmts surrounding it
/// - on the scope level, if all rounds are None, the rounds-count is incremented by one
pub struct StmtResult {
    pub stream: TokenStream,
    pub rounds: usize,
}

impl Stmt {
    pub fn to_stream(
        &self,
        start_round: usize,
        previous_computation_rounds: &HashMap<String, usize>,
    ) -> StmtResult {
        match self {
            Stmt::Collection(stmts) => {
                let mut start_round = start_round;

                // We check all stmts and group them in "sub-scopes"
                // - this means we group stmts that should to be computed in the same round
                // - e.g.: two adjacent stmts are computed in the same round but a partial stmt requires multiple rounds and is not grouped together with those two assignments
                let mut sub_scopes: Vec<StmtResult> = vec![];
                let mut last_stmt: Option<&Stmt> = None;
                for stmt in stmts {
                    let result = stmt.to_stream(start_round, previous_computation_rounds);

                    if result.rounds == 0 {
                        // If a child has `0` round, we can compute this stmt with adjacent `0` round stmts
                        match sub_scopes.last_mut() {
                            Some(last) => {
                                if let Some(last_stmt) = last_stmt {
                                    if last.rounds == 0
                                        && !matches!(last_stmt, Stmt::ComputeUnitStmt(_, _))
                                    {
                                        last.stream.extend(result.stream);
                                    } else {
                                        sub_scopes.push(result);
                                    }
                                } else {
                                    sub_scopes.push(result);
                                }
                            }
                            None => sub_scopes.push(result),
                        }
                    } else {
                        // If a child consumes multiple rounds on it's own, we need a new sub-scope
                        start_round += result.rounds;
                        sub_scopes.push(result);
                    }

                    last_stmt = Some(stmt);
                }

                // If there are multiple groups, we match each scope to the rounds
                let stream;
                let rounds;
                if sub_scopes.len() == 1 && sub_scopes.first().unwrap().rounds == 0 {
                    stream = sub_scopes.first().unwrap().stream.clone();
                    rounds = 0;
                } else {
                    let mut m = quote! {};
                    let mut lower = 0;
                    let mut upper = 0;

                    for scope in sub_scopes {
                        upper += if scope.rounds == 0 { 1 } else { scope.rounds };

                        let s = scope.stream;
                        m.extend(quote! {
                            round if (#lower..#upper).contains(&round) => {
                                let round = round - (#lower);
                                #s
                            },
                        });

                        lower = upper;
                    }

                    stream = quote! {
                        match round {
                            #m
                            _ => {}
                        }
                    };
                    rounds = upper;
                }

                StmtResult { stream, rounds }
            }

            // If/else is a bit more tricky since we need constant round counts with conditionals
            // - so we first use the maximum of both branches rounds count as total count
            // - then we need to add an additional check so that each branch only receives it's required rounds
            Stmt::IfElse(cond, t, f) => {
                let cond: TokenStream = cond.into();

                let result_true = t.to_stream(start_round, previous_computation_rounds);
                let mut body_true = result_true.stream;

                let result_false = match f {
                    Some(f) => f.to_stream(start_round, previous_computation_rounds),
                    None => StmtResult {
                        stream: quote! {},
                        rounds: 0,
                    },
                };
                let mut body_false = result_false.stream;

                let rounds = std::cmp::max(result_true.rounds, result_false.rounds);

                // We adapt the bodies so that having too many rounds supplied to a branch, will not affect any computation
                let true_rounds = if result_true.rounds == 0 {
                    1
                } else {
                    result_true.rounds
                };
                body_true = quote! { if round < #true_rounds { #body_true } };

                let false_rounds = if result_false.rounds == 0 {
                    1
                } else {
                    result_false.rounds
                };
                body_false = quote! { if round < #false_rounds { #body_false } };

                if f.is_some() {
                    StmtResult {
                        stream: quote! {
                            if (#cond) {
                                #body_true
                            } else {
                                #body_false
                            }
                        },
                        rounds,
                    }
                } else {
                    StmtResult {
                        stream: quote! {
                            if (#cond) {
                                #body_true
                            }
                        },
                        rounds,
                    }
                }
            }

            // - the `iterations` of the for-loop are multiplied by the rounds required by the child
            // - we can directly pass the `start_round` since the for-loop does not consume any rounds itself
            Stmt::For(SingleId(iter_id), SingleId(var_id), Expr::Array(arr), child) => {
                let iterations = arr.len();
                let iter_id: TokenStream = iter_id.parse().unwrap();
                let var_id: TokenStream = var_id.parse().unwrap();
                let arr: TokenStream = Expr::Array(arr.clone()).into();
                assert!(
                    iterations > 0,
                    "For loop arrays need to contain at least one element"
                );

                let child_result = child.to_stream(start_round, previous_computation_rounds);
                let child_body = child_result.stream;
                let child_rounds = if child_result.rounds == 0 {
                    1
                } else {
                    child_result.rounds
                };

                if child_result.rounds == 0 {
                    StmtResult {
                        stream: quote! {
                            {
                                let #iter_id = round;
                                let #var_id = vec!#arr[#iter_id];
                                let round = 0;

                                #child_body
                            }
                        },
                        rounds: iterations,
                    }
                } else {
                    StmtResult {
                        stream: quote! {
                            {
                                let #iter_id = round / (#child_rounds);
                                let #var_id = vec!#arr[#iter_id];
                                let round = round % (#child_rounds);

                                #child_body
                            }
                        },
                        rounds: iterations * child_rounds,
                    }
                }
            }

            // The partial assignment calls another method generated using the same partial computation macro
            Stmt::Partial(
                SingleId(id),
                Expr::Fn(Id::Single(SingleId(fn_id)), generics, fn_args),
                child,
            ) => {
                let ident: TokenStream = id.parse().unwrap();

                let mut args = fn_args.clone();
                args.insert(0, Expr::Id(Id::Single(SingleId(String::from("round")))));
                let fn_call: TokenStream = Expr::Fn(
                    Id::Single(SingleId(format!("{}_partial", fn_id))),
                    generics.clone(),
                    args.clone(),
                )
                .into();

                if !previous_computation_rounds.contains_key(fn_id) {
                    panic!("{} const value required", fn_id)
                }
                let size = previous_computation_rounds[fn_id];

                let child_result = child.to_stream(start_round, previous_computation_rounds);
                let mut child_body = child_result.stream;

                let bound = size - 1;

                if child_body.to_string().contains("round") {
                    child_body = quote! {
                        let round = round - #bound;
                        #child_body
                    };
                }

                StmtResult {
                    stream: quote! {
                        if round < #bound {
                            #fn_call.or(Err(PartialComputationError))?;
                        } else if round == #bound {
                            let #ident = match #fn_call {
                                Ok(Some(v)) => v,
                                _ => { return Err(PartialComputationError) }
                            };

                            #child_body
                        }
                    },
                    rounds: size + child_result.rounds,
                }
            }

            Stmt::Let(SingleId(id), mutable, Type(ty), expr) => {
                let ident: TokenStream = id.parse().unwrap();
                let ty: TokenStream = ty.parse().unwrap();
                let value: TokenStream = expr.into();

                if *mutable {
                    StmtResult {
                        stream: quote! { let mut #ident: #ty = #value; },
                        rounds: 0,
                    }
                } else {
                    StmtResult {
                        stream: quote! { let #ident: #ty = #value; },
                        rounds: 0,
                    }
                }
            }

            Stmt::Assign(id, expr) => {
                let ident: TokenStream = id.to_string().parse().unwrap();
                let value: TokenStream = expr.into();

                StmtResult {
                    stream: quote! { #ident = #value; },
                    rounds: 0,
                }
            }

            Stmt::Return(expr) => {
                let value: TokenStream = expr.into();

                StmtResult {
                    stream: quote! { return Ok(Some(#value)); },
                    rounds: 0,
                }
            }

            Stmt::ComputeUnitStmt(_cus, stmt) => {
                stmt.to_stream(start_round, previous_computation_rounds)
            }

            _ => {
                panic!("Invalid stmt: {:?}", self)
            }
        }
    }

    pub fn get_compute_units(&self) -> CUs {
        match self {
            Stmt::ComputeUnitStmt(compute_units, _) => compute_units.clone(),
            Stmt::For(SingleId(iter_id), SingleId(var_id), Expr::Array(arr), child) => {
                let compute_units = child.get_compute_units();
                let mut cus = Vec::new();

                for (i, value) in arr.iter().enumerate() {
                    if let Expr::Literal(u) = value {
                        cus.push(compute_units.apply_mapping(
                            iter_id,
                            var_id,
                            i,
                            u.parse().unwrap(),
                        ));
                    } else {
                        panic!("Array elements need to be literals")
                    }
                }

                CUs::Collection(cus)
            }
            Stmt::Collection(c) => {
                let mut cus = Vec::new();
                for c in c {
                    cus.push(c.get_compute_units())
                }
                CUs::Collection(cus)
            }
            Stmt::Partial(_, Expr::Fn(Id::Single(SingleId(id)), _, _), _stmt) => {
                // TODO: not required atm but in the future add costs of last-round-stmt as well
                CUs::Multiple(id.clone())
            }

            Stmt::IfElse(_, _, _) => panic!("Compute units not allowed for if statement"),
            Stmt::Let(_, _, _, _) => panic!("Compute units not allowed for let statement"),
            Stmt::Assign(_, _) => panic!("Compute units not allowed for assign statement"),
            Stmt::Return(_) => panic!("Compute units not allowed for return statement"),
            _ => panic!("Could not find compute units"),
        }
    }
}

impl From<Expr> for TokenStream {
    fn from(expr: Expr) -> TokenStream {
        match expr {
            Expr::Literal(lit) => lit.parse().unwrap(),
            Expr::BinOp(l, op, r) => {
                let l: TokenStream = (*l).into();
                let op: TokenStream = op.to_string().parse().unwrap();
                let r: TokenStream = (*r).into();
                quote! { (#l #op #r) }
            }
            Expr::UnOp(op, e) => {
                let op: TokenStream = op.to_string().parse().unwrap();
                let e: TokenStream = (*e).into();
                quote! { (#op #e) }
            }
            Expr::Id(id) => id.to_string().parse().unwrap(),
            Expr::Fn(id, generics, exprs) => {
                let id: TokenStream = id.to_string().parse().unwrap();
                let mut args = quote! {};
                for expr in exprs {
                    let expr: TokenStream = expr.into();
                    args.extend(quote! { #expr, });
                }

                let mut generics: TokenStream = generics
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(",")
                    .parse()
                    .unwrap();
                if !generics.is_empty() {
                    generics = quote! { :: < #generics > };
                }

                quote! { #id #generics (#args) }
            }
            Expr::Array(exprs) => {
                let mut args = quote! {};
                for expr in exprs {
                    let expr: TokenStream = expr.into();
                    args.extend(quote! { #expr, });
                }
                quote! { [#args] }
            }
            Expr::Unwrap(expr) => {
                let expr: TokenStream = (*expr).into();
                quote! {
                    match #expr {
                        Some(v) => v,
                        None => return Err(PartialComputationError)
                    }
                }
            }
            Expr::Invalid => panic!("Invalid expression"),
        }
    }
}

impl From<&Expr> for TokenStream {
    fn from(expr: &Expr) -> TokenStream {
        expr.clone().into()
    }
}

impl ToString for BinOp {
    fn to_string(&self) -> String {
        let c = match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::LargerThan => ">",
            BinOp::LessThan => "<",
            BinOp::Equals => "==",
        };
        String::from(c)
    }
}

impl ToString for UnOp {
    fn to_string(&self) -> String {
        let c = match self {
            UnOp::Ref => "&",
            UnOp::Deref => "*",
            UnOp::Not => "!",
        };
        String::from(c)
    }
}

impl ToString for Id {
    fn to_string(&self) -> String {
        match self {
            Id::Single(SingleId(id)) => id.clone(),
            Id::Path(PathId(path)) => path.clone().join(""),
        }
    }
}

impl Id {
    pub fn get_var(&self) -> String {
        match self {
            Id::Single(SingleId(id)) => id.clone(),

            // Since we store each non-terminal path ident as `ident.` or `IDENT::`, we have to remove the last char
            // - this of course does not work for constants like with `IDENT::` but that's not a problem since we only need `get_var` for local vars
            Id::Path(PathId(path)) => {
                let mut v = path.first().unwrap().clone();
                v.pop();
                v
            }
        }
    }
}

pub fn merge<N>(l: Vec<N>, r: Vec<N>) -> Vec<N> {
    let mut v = l;
    v.extend(r);
    v
}

impl Stmt {
    /// Returns all contained terminal stmts for non-terminal stmts
    pub fn all_terminal_stmts(&self) -> Vec<Stmt> {
        match self {
            Stmt::Collection(s) => s
                .iter()
                .map(|s| s.all_terminal_stmts())
                .fold(Vec::new(), merge),
            Stmt::IfElse(_, t, f) => merge(
                t.all_terminal_stmts(),
                match f {
                    Some(f) => f.all_terminal_stmts(),
                    _ => vec![],
                },
            ),
            Stmt::For(_, _, _, s) => s.all_terminal_stmts(),
            Stmt::Partial(_, _, s) => s.all_terminal_stmts(),
            Stmt::ComputeUnitStmt(_, s) => s.all_terminal_stmts(),
            _ => {
                vec![self.clone()]
            }
        }
    }

    /// Returns all expressions in a statement
    pub fn all_exprs(&self) -> Vec<Expr> {
        match self {
            Stmt::Collection(s) => s.iter().map(|s| s.all_exprs()).fold(Vec::new(), merge),
            Stmt::IfElse(e, t, f) => merge(
                vec![e.clone()],
                merge(
                    (*t).all_exprs(),
                    match f {
                        Some(f) => f.all_exprs(),
                        _ => vec![],
                    },
                ),
            ),
            Stmt::For(_, _, e, s) => merge(vec![e.clone()], (*s).all_exprs()),
            Stmt::Partial(_, e, s) => merge(vec![e.clone()], (*s).all_exprs()),
            Stmt::Let(_, _, _, e) => vec![e.clone()],
            Stmt::Assign(_, e) => vec![e.clone()],
            Stmt::Return(e) => vec![e.clone()],
            Stmt::ComputeUnitStmt(_, s) => s.all_exprs(),

            Stmt::Invalid => panic!("Invalid statement"),
        }
    }
}

impl Expr {
    /// Returns all variable names used in an expression
    pub fn all_vars(&self) -> Vec<String> {
        match self {
            Expr::BinOp(l, _, r) => merge((*l).all_vars(), (*r).all_vars()),
            Expr::UnOp(_, e) => (*e).all_vars(),
            Expr::Literal(_) => vec![],
            Expr::Fn(id, _, e) => merge(vec![id.get_var()], Expr::Array(e.clone()).all_vars()),
            Expr::Array(e) => e.iter().map(|e| e.all_vars()).fold(Vec::new(), merge),
            Expr::Unwrap(e) => (*e).all_vars(),
            Expr::Invalid => panic!("Invalid expression"),

            Expr::Id(id) => vec![id.get_var()],
        }
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
    fn test_parse_id() {
        assert_eq_stream!(
            TokenStream::from(Expr::Id(Id::Single(SingleId(String::from("var_name"))))),
            quote! { var_name }
        );

        assert_eq_stream!(
            TokenStream::from(Expr::Id(Id::Path(PathId(vec![
                String::from("ab_cd."),
                String::from("efg."),
                String::from("CONST_NAME")
            ])))),
            quote! { ab_cd.efg.CONST_NAME }
        );

        assert_eq!(
            Id::Path(PathId(vec![String::from("alpha."), String::from("beta")])).get_var(),
            "alpha"
        );
    }

    #[test]
    fn test_parse_expr() {
        assert_eq_stream!(
            TokenStream::from(Expr::Unwrap(Box::new(Expr::Fn(
                Id::Single(SingleId(String::from("fn_name"))),
                vec![],
                vec![]
            )))),
            quote! { match fn_name() { Some(v) => v, None => return Err("Unwrap error") } }
        );
    }
}
