use std::string::ToString;
use quote::quote;
use proc_macro2::TokenStream;
use super::storage::*;

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
}

#[derive(Debug, Clone)]
pub enum Stmt {
    // Non-terminal stmts
    Collection(Vec<Stmt>),
    IfElse(Expr, Box<Stmt>, Box<Stmt>),
    For(SingleId, SingleId, Expr, Box<Stmt>),
    
    // Terminal stmts
    Let(SingleId, bool, Type, Expr),    // Let.1 is the mutability
    Assign(Id, Expr),
    // `partial v = fn<generics>(params) { <<Stmt>> }`
    Partial(SingleId, Expr, Box<Stmt>),
    Return(Expr),
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

/// Types are only allowed as Strings without punctations
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

/// - `rounds` == None means that the Stmt uses the same round as the scope or other stmts surrounding it
/// - on the scope level, if all rounds are None, the rounds-count is incremented by one
pub struct StmtResult {
    pub stream: TokenStream,
    pub rounds: Option<TokenStream>,
}

impl Stmt {
    pub fn to_stream(&self, start_round: TokenStream) -> StmtResult {
        match self {
            Stmt::Collection(stmts) => {
                let mut start_round = start_round;

                // We check all stmts and group them in "sub-scopes"
                // - this means we group stmts that should to be computed in the same round
                // - e.g.: two adjacent stmts are computed in the same round but a partial stmt requires multiple rounds and is not grouped together with those two assignments
                let mut sub_scopes: Vec<StmtResult> = vec![];
                for stmt in stmts {
                    let result = stmt.to_stream(start_round.clone());

                    match result.rounds.clone() {
                        // If a child has `None` rounds, we can compute this stmt with adjacent `None` rounds stmts
                        None => {
                            match sub_scopes.last_mut() {
                                Some(last) => {
                                    match last.rounds {
                                        None => last.stream.extend(result.stream),
                                        Some(_) => sub_scopes.push(result),
                                    };
                                },
                                None => { sub_scopes.push(result) },
                            }
                        },

                        // If a child consumes multiple rounds on it's own, we need a new sub-scope
                        Some(r) => {
                            sub_scopes.push(result);
                            start_round.extend(quote!{ + #r });
                        }
                    }
                }

                // If there are multiple groups, we match each scope to the rounds
                let stream;
                let mut rounds: Option<TokenStream> = None;
                if sub_scopes.len() == 1 && matches!(sub_scopes.first().unwrap().rounds, None) {
                    stream = sub_scopes.first().unwrap().stream.clone();
                } else {
                    let mut m = quote!{};
                    let mut lower = quote!{};
                    let mut upper = quote!{};

                    for scope in sub_scopes {
                        match scope.rounds {
                            None => upper.extend(if upper.is_empty() { quote!{ 1 } } else { quote!{ + 1 } }),
                            Some(r) => upper.extend(if upper.is_empty() { quote!{ #r } } else { quote!{ + #r } }),
                        }

                        let s = scope.stream;
                        let l = if lower.is_empty() { quote!{ 0 } } else { lower.clone() };
                        m.extend(quote!{
                            round if round >= #l && round < #upper => {
                                let round = round - (#l);
                                #s
                            },
                        });

                        lower = upper.clone();
                    }

                    stream = quote!{
                        match round {
                            #m
                            _ => {}
                        }
                    };
                    rounds = if upper.is_empty() { None } else { Some(upper) };
                }

                StmtResult { stream, rounds }
            },

            // If/else is a bit more tricky since we need constant round counts with conditionals
            // - so we first use the maximum of both branches rounds count as total count
            // - then we need to add an additional check so that each branch only receives it's required rounds
            Stmt::IfElse(cond, t, f) => {
                let cond: TokenStream = cond.into();

                let result_true = t.to_stream(start_round.clone());
                let mut body_true = result_true.stream;

                let result_false = f.to_stream(start_round.clone());
                let mut body_false = result_false.stream;

                let rounds = match result_true.rounds.clone() {
                    Some(t) => {
                        match result_false.rounds.clone() {
                            Some(f) => Some(quote!{ max(#t, #f) }),
                            None => Some(t)
                        }
                    },
                    None => result_false.rounds.clone()
                };

                // We adapt the bodies so that having too many rounds supplied to a branch, will not affect any computation
                let true_rounds = result_true.rounds.unwrap_or(quote!{ 1 });
                body_true = quote!{ if round < #true_rounds { #body_true } };

                let false_rounds = result_false.rounds.unwrap_or(quote!{ 1 });
                body_false = quote!{ if round < #false_rounds { #body_false } };

                StmtResult { stream: quote!{
                    if (#cond) {
                        #body_true
                    } else {
                        #body_false
                    }
                }, rounds }
            },

            // - the `iterations` of the for-loop are multiplied by the rounds required by the child
            // - we can directly pass the `start_round` since the for-loop does not consume any rounds itself
            Stmt::For(SingleId(iter_id), SingleId(var_id), Expr::Array(arr), child) => {
                let iterations = arr.len();
                let iter_id: TokenStream = iter_id.parse().unwrap();
                let var_id: TokenStream = var_id.parse().unwrap();
                let arr: TokenStream = Expr::Array(arr.clone()).into();
                assert!(iterations > 0, "For loop arrays need to contain at least one element");

                let child_result = child.to_stream(start_round.clone());
                let child_body = child_result.stream;
                let child_rounds = child_result.rounds.unwrap_or(quote!{ 1 });

                StmtResult { stream: quote!{
                    {
                        let #iter_id = round / (#child_rounds);
                        let #var_id = vec!#arr[#iter_id];
                        let round = round % (#child_rounds);

                        #child_body
                    }
                }, rounds: Some(quote!{ (#iterations * (#child_rounds)) }) }
            },

            // The partial assignment calls another method generated using the same partial computation macro
            Stmt::Partial(SingleId(id), Expr::Fn(Id::Single(SingleId(fn_id)), generics, fn_args), child) => {
                let ident: TokenStream = id.parse().unwrap();

                let mut args = fn_args.clone();
                args.insert(0, Expr::Id(Id::Single(SingleId(String::from("round")))));
                let fn_call: TokenStream = Expr::Fn(Id::Single(SingleId(format!("{}_partial", fn_id))), generics.clone(), args.clone()).into();
                let size: TokenStream = format!("{}_ROUNDS_COUNT", fn_id.to_uppercase()).parse().unwrap();

                let child_result = child.to_stream(start_round);
                let child_body = child_result.stream;
                let child_rounds = child_result.rounds.unwrap_or(quote!{ });                

                StmtResult { stream: quote!{
                    if round < #size - 1 {
                        match #fn_call {
                            Ok(_) => {},
                            Err(_) => { return Err("Partial computation error") }
                        }
                    } else if round == #size - 1 {
                        let #ident = match #fn_call {
                            Ok(Some(v)) => v,
                            _ => { return Err("Partial computation error") }
                        };
                        #child_body
                    }
                }, rounds: Some(quote!{ (#size #child_rounds) }) }
            },

            Stmt::Let(SingleId(id), mutable, Type(ty), expr) => {
                let ident: TokenStream = id.parse().unwrap();
                let ty: TokenStream = ty.parse().unwrap();
                let value: TokenStream = expr.into();

                if *mutable {
                    StmtResult { stream: quote!{ let mut #ident: #ty = #value; }, rounds: None }
                } else {
                    StmtResult { stream: quote!{ let #ident: #ty = #value; }, rounds: None }
                }
            },

            Stmt::Assign(id, expr) => {
                let ident: TokenStream = id.to_string().parse().unwrap();
                let value: TokenStream = expr.into();

                StmtResult { stream: quote!{ #ident = #value; }, rounds: None }
            },

            Stmt::Return(expr) => {
                let value: TokenStream = expr.into();

                StmtResult { stream: quote!{ return Ok(Some(#value)); }, rounds: None }
            },

            _ => { panic!("Invalid stmt: {:?}", self) }
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
                quote!{ (#l #op #r) }
            },
            Expr::UnOp(op, e) => {
                let op: TokenStream = op.to_string().parse().unwrap();
                let e: TokenStream = (*e).into();
                quote!{ (#op #e) }
            },
            Expr::Id(id) => id.to_string().parse().unwrap(),
            Expr::Fn(id, generics, exprs) => {
                let id: TokenStream = id.to_string().parse().unwrap();
                let mut args = quote!{};
                for expr in exprs {
                    let expr: TokenStream = expr.into();
                    args.extend(quote!{ #expr, });
                }

                let mut generics: TokenStream = generics.iter().map(|v| v.to_string()).collect::<Vec<String>>().join(",").parse().unwrap();
                if !generics.is_empty() {
                    generics = quote!{ :: < #generics > };
                }

                quote!{ #id #generics (#args) }
            },
            Expr::Array(exprs) => {
                let mut args = quote!{};
                for expr in exprs {
                    let expr: TokenStream = expr.into();
                    args.extend(quote!{ #expr, });
                }
                quote!{ [#args] }
            },
            Expr::Unwrap(expr) => {
                let expr: TokenStream = (*expr).into();
                quote!{
                    match #expr {
                        Some(v) => v,
                        None => return Err("Unwrap error")
                    }
                }
            },
            Expr::Invalid => panic!("Invalid expression")
        }
    }
}

impl From<&Expr> for TokenStream {
    fn from(expr: &Expr) -> TokenStream { expr.clone().into() }
}

impl ToString for BinOp {
    fn to_string(&self) -> String {
        let c = match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::LargerThan => ">",
            BinOp::LessThan => "<",
            BinOp::Equals => "=="
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
            Id::Path(PathId(path)) => { let mut v = path.first().unwrap().clone(); v.pop(); v },
        }
    }
}

pub fn merge<N>(l: Vec<N>, r: Vec<N>) -> Vec<N> {
    let mut v = l; v.extend(r); v
}

impl Stmt {
    /// Returns all contained terminal stmts for non-terminal stmts
    pub fn all_terminal_stmts(&self) -> Vec<Stmt> {
        match self {
            Stmt::Collection(s) => s.iter().map(|s| s.all_terminal_stmts()).fold(Vec::new(), merge),
            Stmt::IfElse(_, t, f) => merge(t.all_terminal_stmts(), f.all_terminal_stmts()),
            Stmt::For(_, _, _, s) => s.all_terminal_stmts(),
            Stmt::Partial(_, _, s) =>  s.all_terminal_stmts(),
            _ => { vec![self.clone()] }
        }
    }

    /// Returns all expressions in a statement
    pub fn all_exprs(&self) -> Vec<Expr> {
        match self {
            Stmt::Collection(s) => s.iter().map(|s| s.all_exprs()).fold(Vec::new(), merge),
            Stmt::IfElse(e, t, f) => merge(vec![e.clone()], merge((*t).all_exprs(), (*f).all_exprs())),
            Stmt::For(_, _, e, s) => merge(vec![e.clone()], (*s).all_exprs()),
            Stmt::Partial(_, e, s) => merge(vec![e.clone()], (*s).all_exprs()),
            Stmt::Let(_, _, _, e) => vec![e.clone()],
            Stmt::Assign(_, e) => vec![e.clone()],
            Stmt::Return(e) => vec![e.clone()],
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
            Expr::Fn(id, _, e) => merge(vec![id.get_var().clone()], Expr::Array(e.clone()).all_vars()),
            Expr::Array(e) => e.iter().map(|e| e.all_vars()).fold(Vec::new(), merge),
            Expr::Unwrap(e) => (*e).all_vars(),
            Expr::Invalid => panic!("Invalid expression"),

            Expr::Id(id) => vec![id.to_string()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_eq_stream {
        ($a: expr, $b: expr) => { assert_eq!($a.to_string(), $b.to_string()) };
    }

    #[test]
    fn test_parse_id() {
        assert_eq_stream!(
            TokenStream::from(
                Expr::Id(Id::Single(SingleId(String::from("var_name"))))
            ),
            quote!{ var_name }
        );

        assert_eq_stream!(
            TokenStream::from(Expr::Id(Id::Path(PathId(vec![
                String::from("ab_cd."), String::from("efg."), String::from("CONST_NAME")
            ])))),
            quote!{ ab_cd.efg.CONST_NAME }
        );

        assert_eq!(
            Id::Path(PathId(vec![String::from("alpha."), String::from("beta")])).get_var(),
            "alpha"
        );
    }

    #[test]
    fn test_parse_expr() {
        assert_eq_stream!(
            TokenStream::from(
                Expr::Unwrap(
                    Box::new(Expr::Fn(Id::Single(SingleId(String::from("fn_name"))), vec![], vec![]))
                )
            ),
            quote!{ match fn_name() { Some(v) => v, None => return Err("Unwrap error") } }
        );
    }
}