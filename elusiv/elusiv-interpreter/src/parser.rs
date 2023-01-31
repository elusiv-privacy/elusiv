use super::grammar::*;
use proc_macro2::{Delimiter, Group, TokenTree};
use std::convert::From;
use Token::*;

impl From<&[Group]> for Computation {
    fn from(groups: &[Group]) -> Self {
        Computation {
            scopes: groups.iter().map(|g| g.into()).collect(),
        }
    }
}

fn split_at(splitter: Token, trees: Vec<Token>) -> Vec<Vec<Token>> {
    let mut streams: Vec<Vec<Token>> = Vec::new();
    let mut stream = Vec::new();

    for t in trees {
        if t == splitter {
            streams.push(stream);
            stream = Vec::new();
            continue;
        }
        stream.push(t);
    }

    if !stream.is_empty() {
        streams.push(stream);
    }
    streams
}

impl From<&Group> for ComputationScope {
    fn from(group: &Group) -> Self {
        assert!(group.delimiter() == Delimiter::Brace, "Invalid delimiter");
        let trees: Vec<TokenTree> = group.stream().into_iter().collect();
        let tokens: Vec<Token> = trees.iter().map(|t| t.into()).collect();

        let token_slice;
        let compute_units;

        // Check for scope wide compute units
        if let (Some(cus), tail) = match_compute_units_head(&tokens[..]) {
            token_slice = tail;
            compute_units = Some(cus);
        } else {
            token_slice = &tokens[..];
            compute_units = None;
        }

        ComputationScope {
            stmt: token_slice.into(),
            read: vec![],
            free: vec![],
            write: vec![],
            scope_wide_compute_units: compute_units,
        }
    }
}

impl From<&[Token]> for Stmt {
    fn from(tree: &[Token]) -> Self {
        // Collection
        if matches!(
            tree.iter()
                .find(|t| matches!(t, Token::Punct(Punct::Semicolon))),
            Some(_)
        ) {
            let trees = split_at(SEMICOLON, tree.to_vec());
            let stmts: Vec<Stmt> = trees.iter().map(|t| t.into()).collect();
            if stmts.len() > 1 {
                return Stmt::Collection(stmts);
            } else {
                return stmts.first().unwrap().clone();
            }
        }

        match tree {
            [LET, Ident(id), COLON, Ident(ty), EQUALS, tail @ ..] => {
                Stmt::Let(SingleId(id.clone()), false, Type(ty.clone()), tail.into())
            }
            [LET, MUT, Ident(id), COLON, Ident(ty), EQUALS, tail @ ..] => {
                Stmt::Let(SingleId(id.clone()), true, Type(ty.clone()), tail.into())
            }
            [PARTIAL, Ident(id), EQUALS, Ident(fn_id), generics @ .., Group(args, Delimiter::Parenthesis), Group(g, Delimiter::Brace)] =>
            {
                let args = merge(
                    vec![Ident(fn_id.clone())],
                    merge(
                        generics.to_vec(),
                        vec![Group(args.clone(), Delimiter::Parenthesis)],
                    ),
                );

                // We wrap each partial stmt in a computation scope stmt
                Stmt::ComputeUnitStmt(
                    CUs::Multiple(fn_id.clone()),
                    Box::new(Stmt::Partial(
                        SingleId(id.clone()),
                        args.into(),
                        Box::new(g.into()),
                    )),
                )
            }
            [RETURN, tail @ ..] => Stmt::Return(tail.into()),

            // We just hard-code two cases for assignments for simplicity's sake: one ident and one field
            [Ident(id), EQUALS, tail @ ..] => {
                Stmt::Assign(Id::Single(SingleId(id.clone())), tail.into())
            }
            [Ident(a), DOT, Ident(b), EQUALS, tail @ ..] => Stmt::Assign(
                Id::Path(PathId(vec![a.clone() + ".", b.clone()])),
                tail.into(),
            ),

            // For loop
            [FOR, Ident(iter_id), COMMA, Ident(var_id), IN, arr, Group(g, Delimiter::Brace)] => {
                Stmt::For(
                    SingleId(iter_id.clone()),
                    SingleId(var_id.clone()),
                    vec![arr.clone()].into(),
                    Box::new(g.into()),
                )
            }

            // If-else and if stmts
            [IF, Group(c, Delimiter::Parenthesis), Group(t, Delimiter::Brace), ELSE, Group(f, Delimiter::Brace), tail @ ..] => {
                try_stmt_tail(
                    Stmt::IfElse(c.into(), Box::new(t.into()), Some(Box::new(f.into()))),
                    tail,
                )
            }
            [IF, Group(c, Delimiter::Parenthesis), Group(t, Delimiter::Brace), tail @ ..] => {
                try_stmt_tail(Stmt::IfElse(c.into(), Box::new(t.into()), None), tail)
            }

            // Grouping
            [Group(c, Delimiter::Brace), tail @ ..] => try_stmt_tail(
                if let (Some(compute_units), stmt) = match_compute_units_head(c) {
                    Stmt::ComputeUnitStmt(compute_units, Box::new(stmt.into()))
                } else {
                    (&c[..]).into()
                },
                tail,
            ),
            _ => Stmt::Invalid,
        }
    }
}

/// Matches a leading compute units documentation comment
/// - returns the compute units (if existent) and the remaining tail
/// - allowed syntax:
///     - Single Compute units: <usize>
///     - Compute units mapping: <ident> in { (<pattern> : <usize> , )+ }
///         - dependent on the variable <ident> (which has to be known at compile time => any of the for-loop variables at the moment)
///         - patterns are mapped to values
///         - '_' matches the remaining patterns
fn match_compute_units_head(tokens: &[Token]) -> (Option<CUs>, &[Token]) {
    if let [HASH, Group(g, Delimiter::Bracket), c_tail @ ..] = tokens {
        if let [DOC, EQUALS, Literal(compute_units)] = &g[..] {
            let cutoff = if compute_units.starts_with('r') { 3 } else { 2 }; // raw string literal
            let cus = String::from(&compute_units[cutoff..&compute_units.len() - 1]);

            if let Some(compute_units) = try_parse_usize(&cus) {
                // Single static compute units
                return (Some(CUs::Single(compute_units)), c_tail);
            } else {
                // Mappings of (at compile time known) values of variables to compute units
                let token: Vec<&str> = cus.split(' ').collect();

                if let [ident, "in", "{", mapping @ .., "}"] = &token[..] {
                    /// Recursively finds mappings
                    fn get_mapping(m: &[&str]) -> Vec<ComputeUnitMapping> {
                        match m {
                            [] => vec![],
                            [pattern, ":", value, ",", tail @ ..]
                            | [pattern, ":", value, tail @ ..] => {
                                let mut tail = get_mapping(tail);
                                tail.insert(
                                    0,
                                    ComputeUnitMapping {
                                        pattern: if *pattern == "_" {
                                            None
                                        } else {
                                            Some(try_parse_usize(pattern).unwrap())
                                        },
                                        value: if let Some(compute_units) = try_parse_usize(value) {
                                            CUs::Single(compute_units)
                                        } else {
                                            CUs::Multiple(String::from(*value))
                                        },
                                    },
                                );
                                tail
                            }
                            _ => panic!("Invalid compute unit mapping syntax"),
                        }
                    }

                    return (
                        Some(CUs::Mapping {
                            ident: String::from(*ident),
                            mapping: get_mapping(mapping),
                        }),
                        c_tail,
                    );
                } else {
                    return (Some(CUs::Multiple(cus)), c_tail);
                }
            }
        }
    }
    (None, tokens)
}

/// Attempts to parse a String into a usize, ignoring any '_' character
pub fn try_parse_usize(source: &str) -> Option<usize> {
    let mut source = String::from(source);
    source.retain(|x| x != '_');
    match source.parse::<usize>() {
        Ok(u) => Some(u),
        Err(_) => None,
    }
}

fn try_stmt_tail(head: Stmt, tail: &[Token]) -> Stmt {
    match tail.into() {
        Stmt::Invalid => head,
        Stmt::Collection(c) => {
            let mut c = c;
            c.insert(0, head);
            Stmt::Collection(c)
        }
        t => Stmt::Collection(vec![head, t]),
    }
}

impl From<Vec<Token>> for Stmt {
    fn from(tree: Vec<Token>) -> Self {
        (&tree[..]).into()
    }
}
impl From<&Vec<Token>> for Stmt {
    fn from(tree: &Vec<Token>) -> Self {
        (&tree[..]).into()
    }
}

const UN_OP_BINDING: [UnOp; 3] = [UnOp::Ref, UnOp::Deref, UnOp::Not];
const BIN_OP_BINDING: [BinOp; 6] = [
    BinOp::Add,
    BinOp::Sub,
    BinOp::Mul,
    BinOp::LargerThan,
    BinOp::LessThan,
    BinOp::Equals,
];

impl From<&[Token]> for Expr {
    fn from(tree: &[Token]) -> Self {
        // Unops
        if let Some(Punct(first)) = tree.first() {
            for op in &UN_OP_BINDING {
                let op = Some(op.clone());
                if first.as_unop() == op {
                    for i in 2..=tree.len() {
                        let expr: Expr = (&tree[1..i]).into();
                        if !matches!(expr, Expr::Invalid) {
                            let un_expr = Expr::UnOp(op.unwrap(), Box::new(expr));
                            if i == tree.len() {
                                // full expr is just an unop expr
                                return un_expr;
                            } else {
                                // if tokens remain on the right, we know that we have to be part of a binop expr
                                if let Punct(p) = &tree[i] {
                                    let bop = p.as_binop().unwrap();
                                    return Expr::BinOp(
                                        Box::new(un_expr),
                                        bop,
                                        Box::new((&tree[i + 1..]).into()),
                                    );
                                }
                            }
                            println!("Invalid unary operation");
                            return Expr::Invalid;
                        }
                    }
                    println!("Invalid unary operation");
                    return Expr::Invalid;
                }
            }
        }

        // Binops
        for op in &BIN_OP_BINDING {
            let op = Some(op.clone());
            if let Some(bop_pos) = tree.iter().position(|t| {
                if let Punct(p) = t {
                    p.as_binop() == op
                } else {
                    false
                }
            }) {
                let l: Expr = (&tree[..bop_pos]).into();
                let r: Expr = (&tree[bop_pos + 1..]).into();

                if !matches!(l, Expr::Invalid) && !matches!(r, Expr::Invalid) {
                    return Expr::BinOp(Box::new(l), op.unwrap(), Box::new(r));
                }
            }
        }

        match tree {
            // Function call without generics
            [Ident(id), Group(group, Delimiter::Parenthesis)] => {
                let trees = split_at(COMMA, group.clone());
                let exprs: Vec<Expr> = trees.iter().map(|t| t.into()).collect();
                Expr::Fn(Id::Single(SingleId(id.clone())), vec![], exprs)
            }

            // Generics Function call
            [Ident(id), COLON, COLON, LESS, generics @ .., LARGER, Group(group, Delimiter::Parenthesis)] =>
            {
                fn m(s: &[Token]) -> Vec<Id> {
                    match s {
                        [Ident(id), COMMA, tail @ ..] => {
                            merge(vec![Id::Single(SingleId(id.clone()))], m(tail))
                        }
                        [Ident(id)] => vec![Id::Single(SingleId(id.clone()))],
                        [] => vec![],
                        [..] => panic!("Invalid generics in function call"),
                    }
                }
                let generics = m(generics);
                let trees = split_at(COMMA, group.clone());
                let exprs: Vec<Expr> = trees.iter().map(|t| t.into()).collect();
                Expr::Fn(Id::Single(SingleId(id.clone())), generics, exprs)
            }

            // Array
            [Group(group, Delimiter::Bracket)] => {
                let trees = split_at(COMMA, group.clone());
                let exprs: Vec<Expr> = trees.iter().map(|t| t.into()).collect();
                Expr::Array(exprs)
            }

            [Literal(lit)] => Expr::Literal(lit.clone()),
            [Ident(id)] => Expr::Id(Id::Single(SingleId(id.clone()))),

            // Dot-separated idents
            // - we recursively match the tail and merge with the tail in order to construct all valid exprs
            // - IMPORTANT: this is an first implementation, it would be better to have a recursive access structure of ident, literals and functions
            // - I will probably add this in the future, but there is not need for it at the moment
            [Ident(a) | Literal(a), DOT, tail @ ..] => {
                let tail: Expr = tail.into();
                let a = a.clone() + ".";

                match tail {
                    Expr::Fn(Id::Single(SingleId(id)), g, p) => {
                        Expr::Fn(Id::Path(PathId(vec![a, id])), g, p)
                    }
                    Expr::Fn(Id::Path(PathId(path)), g, p) => {
                        Expr::Fn(Id::Path(PathId(merge(vec![a], path))), g, p)
                    }
                    Expr::Id(Id::Single(SingleId(id))) => Expr::Id(Id::Path(PathId(vec![a, id]))),
                    Expr::Id(Id::Path(PathId(path))) => {
                        Expr::Id(Id::Path(PathId(merge(vec![a], path))))
                    }
                    Expr::Literal(lit) => Expr::Id(Id::Path(PathId(vec![a, lit]))),
                    _ => Expr::Invalid,
                }
            }
            // Double-colon-separated idents
            [Ident(a), COLON, COLON, tail @ ..] => {
                let tail: Expr = tail.into();
                let a = a.clone() + "::";

                match tail {
                    Expr::Fn(Id::Single(SingleId(id)), g, p) => {
                        Expr::Fn(Id::Path(PathId(vec![a, id])), g, p)
                    }
                    Expr::Fn(Id::Path(PathId(path)), g, p) => {
                        Expr::Fn(Id::Path(PathId(merge(vec![a], path))), g, p)
                    }
                    Expr::Id(Id::Single(SingleId(id))) => Expr::Id(Id::Path(PathId(vec![a, id]))),
                    Expr::Id(Id::Path(PathId(path))) => {
                        Expr::Id(Id::Path(PathId(merge(vec![a], path))))
                    }
                    Expr::Literal(lit) => Expr::Id(Id::Path(PathId(vec![a, lit]))),
                    _ => Expr::Invalid,
                }
            }

            // Parenthesized group
            [Group(group, Delimiter::Parenthesis)] => group.into(),

            // Unwrap
            [UNWRAP, tail @ ..] => Expr::Unwrap(Box::new(tail.into())),
            _ => Expr::Invalid,
        }
    }
}

impl From<Vec<Token>> for Expr {
    fn from(tree: Vec<Token>) -> Self {
        (&tree[..]).into()
    }
}
impl From<&Vec<Token>> for Expr {
    fn from(tree: &Vec<Token>) -> Self {
        (&tree[..]).into()
    }
}

const LET: Token = Token::Keyword(Keyword::Let);
const MUT: Token = Token::Keyword(Keyword::Mut);
const PARTIAL: Token = Token::Keyword(Keyword::Partial);
const UNWRAP: Token = Token::Keyword(Keyword::Unwrap);
const RETURN: Token = Token::Keyword(Keyword::Return);
const IF: Token = Token::Keyword(Keyword::If);
const ELSE: Token = Token::Keyword(Keyword::Else);
const FOR: Token = Token::Keyword(Keyword::For);
const IN: Token = Token::Keyword(Keyword::In);
const DOC: Token = Token::Keyword(Keyword::Doc);

const EQUALS: Token = Token::Punct(Punct::Equals);
const SEMICOLON: Token = Token::Punct(Punct::Semicolon);
const COLON: Token = Token::Punct(Punct::Colon);
const COMMA: Token = Token::Punct(Punct::Comma);
const HASH: Token = Token::Punct(Punct::Hash);
const DOT: Token = Token::Punct(Punct::Dot);
const LESS: Token = Token::Punct(Punct::LessThan);
const LARGER: Token = Token::Punct(Punct::LargerThan);

#[derive(Eq, PartialEq, Clone, Debug)]
enum Token {
    Ident(String),
    Literal(String),
    Group(Vec<Token>, Delimiter),

    Keyword(Keyword),
    Punct(Punct),
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum Keyword {
    Let,
    Mut,
    Return,
    Partial,
    Unwrap,
    If,
    Else,
    For,
    In,
    Doc,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum Punct {
    Equals,
    Comma,
    Semicolon,
    Colon,
    Hash,
    Dot,
    Plus,
    Minus,
    Asterisk,
    And,
    LessThan,
    LargerThan,
    ExclamationMark,
}

impl Punct {
    fn as_binop(&self) -> Option<BinOp> {
        match self {
            Punct::Plus => Some(BinOp::Add),
            Punct::Minus => Some(BinOp::Sub),
            Punct::Asterisk => Some(BinOp::Mul),
            Punct::LargerThan => Some(BinOp::LargerThan),
            Punct::LessThan => Some(BinOp::LessThan),
            Punct::Equals => Some(BinOp::Equals),
            _ => None,
        }
    }

    fn as_unop(&self) -> Option<UnOp> {
        match self {
            Punct::Asterisk => Some(UnOp::Deref),
            Punct::And => Some(UnOp::Ref),
            Punct::ExclamationMark => Some(UnOp::Not),
            _ => None,
        }
    }
}

impl From<&TokenTree> for Token {
    fn from(tree: &TokenTree) -> Self {
        match tree {
            TokenTree::Ident(ident) => {
                let s = ident.to_string();

                match s.as_str() {
                    "let" => LET,
                    "mut" => MUT,
                    "unwrap" => UNWRAP,
                    "partial" => PARTIAL,
                    "return" => RETURN,
                    "if" => IF,
                    "else" => ELSE,
                    "for" => FOR,
                    "in" => IN,
                    "doc" => DOC,
                    "round" => {
                        panic!("Reserved ident `round` used")
                    }
                    _ => Token::Ident(s),
                }
            }
            TokenTree::Punct(punct) => {
                let s = punct.to_string();

                match s.as_str() {
                    "=" => EQUALS,
                    ";" => SEMICOLON,
                    ":" => COLON,
                    "," => COMMA,
                    "." => Token::Punct(Punct::Dot),
                    "#" => HASH,

                    "+" => Token::Punct(Punct::Plus),
                    "-" => Token::Punct(Punct::Minus),
                    "*" => Token::Punct(Punct::Asterisk),
                    "<" => Token::Punct(Punct::LessThan),
                    ">" => Token::Punct(Punct::LargerThan),
                    "&" => Token::Punct(Punct::And),
                    "!" => Token::Punct(Punct::ExclamationMark),

                    _ => {
                        panic!("Unknown punctation: {}", s)
                    }
                }
            }
            TokenTree::Literal(lit) => Token::Literal(lit.to_string()),
            TokenTree::Group(group) => {
                let trees: Vec<TokenTree> = group.stream().into_iter().collect();
                let tokens: Vec<Token> = trees.iter().map(|g| g.into()).collect();
                Token::Group(tokens, group.delimiter())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_parse_usize() {
        assert_eq!(try_parse_usize("10_000").unwrap(), 10_000);
    }
}
