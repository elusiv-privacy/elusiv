use super::grammar::*;
use std::convert::From;
use proc_macro2::{ Group, Delimiter, TokenTree };
use Token::*;

impl From<&[Group]> for Computation {
    fn from(groups: &[Group]) -> Self {
        Computation { scopes: groups.iter().map(|g| g.into()).collect() }
    }
}

fn split_at<'a>(splitter: Token, trees: Vec<Token>) -> Vec<Vec<Token>> {
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

    if stream.len() > 0 {
        streams.push(stream);
    }

    streams
}

impl From<&Group> for ComputationScope {
    fn from(group: &Group) -> Self {
        assert!(group.delimiter() == Delimiter::Brace, "Invalid delimiter");
        let trees: Vec<TokenTree> = group.stream().into_iter().collect();
        let tokens: Vec<Token> = trees.iter().map(|t| t.into()).collect();

        ComputationScope {
            stmt: (&tokens[..]).into(),
            read: vec![],
            write: vec![],
        }
    }
}

impl From<&[Token]> for Stmt {
    fn from(tree: &[Token]) -> Self {
        // Collection
        if matches!(tree.iter().find(|t| matches!(t, Token::Punct(Punct::Semicolon))), Some(_)) {
            let trees = split_at(SEMICOLON, tree.to_vec());
            let stmts: Vec<Stmt> = trees.iter().map(|t| t.into()).collect();
            return Stmt::Collection(stmts)
        }

        // Non-terminal
        match tree {
            [ LET, Ident(id), COLON, Ident(ty), EQUALS, .. ] => {
                Stmt::Let(SingleId(id.clone()), Type(ty.clone()), (&tree[5..]).into())
            },
            [ LET, MUT, Ident(id), COLON, Ident(ty), EQUALS, .. ] => {
                Stmt::LetMut(SingleId(id.clone()), Type(ty.clone()), (&tree[6..]).into())
            },
            [ PARTIAL, Ident(id), EQUALS, Ident(fn_id), Group(args, Delimiter::Parenthesis), Group(g, Delimiter::Brace) ] => {
                let args = vec![Ident(fn_id.clone()), Group(args.clone(), Delimiter::Parenthesis)];
                Stmt::Partial(SingleId(id.clone()), args.into(), Box::new(g.into()))
            },
            [ RETURN, .. ] => {
                Stmt::Return((&tree[3..]).into())
            },
            [ Ident(id), EQUALS, .. ] => {
                Stmt::Assign(SingleId(id.clone()), (&tree[2..]).into())
            },

            // For loop
            [ FOR, Ident(iter_id), COMMA, Ident(var_id), IN, arr, Group(g, Delimiter::Brace) ] => {
                Stmt::For(SingleId(iter_id.clone()), SingleId(var_id.clone()), vec![arr.clone()].into(), Box::new(g.into()))
            },

            // If-else and if stmts
            [ IF, Group(c, Delimiter::Parenthesis), Group(t, Delimiter::Brace), ELSE, Group(f, Delimiter::Brace) ] => {
                Stmt::IfElse(c.into(), Box::new(t.into()), Box::new(f.into()))
            },
            [ IF, Group(c, Delimiter::Parenthesis), Group(t, Delimiter::Brace) ] => {
                Stmt::IfElse(c.into(), Box::new(t.into()), Box::new(Stmt::Collection(vec![])))
            },
            _ => { panic!("Invalid stmt stream {:#?}", tree) }
        }
    }
}

impl From<Vec<Token>> for Stmt {
    fn from(tree: Vec<Token>) -> Self { (&tree[..]).into() }
}
impl From<&Vec<Token>> for Stmt {
    fn from(tree: &Vec<Token>) -> Self { (&tree[..]).into() }
}

macro_rules! cast_enum {
    ($v: expr, $ty: ident) => {
        if let $ty(v) = $v { v } else { panic!("Invalid enum cast") }
    };
}

const BIN_OP_BINDING: [BinOp; 3] = [BinOp::Add, BinOp::Sub, BinOp::Mul];

impl From<&[Token]> for Expr {
    fn from(tree: &[Token]) -> Self {
        // Binops
        for op in BIN_OP_BINDING {
            if let Some(bop_pos) = tree.iter().position(|t| *t == BOp(op.clone()) ) {
                let bop = cast_enum!(tree[bop_pos].clone(), BOp);
                let l: Expr = (&tree[..bop_pos]).into();
                let r: Expr = (&tree[bop_pos + 1..]).into();

                return Expr::BinOp(Box::new(l), bop, Box::new(r))
            }
        }

        match tree {
            // Function call
            [ Ident(id), Group(group, Delimiter::Parenthesis) ] => {
                let trees = split_at(COMMA, group.clone());
                let exprs: Vec<Expr> = trees.iter().map(|t| t.into()).collect();
                Expr::Fn(Id::Single(SingleId(id.clone())), exprs)
            },

            // Array
            [ Group(group, Delimiter::Bracket) ] => {
                let trees = split_at(COMMA, group.clone());
                let exprs: Vec<Expr> = trees.iter().map(|t| t.into()).collect();
                Expr::Array(exprs)
            },

            [ Literal(lit) ] => { Expr::Literal(lit.clone()) },
            [ Ident(id) ] => { Expr::Id(Id::Single(SingleId(id.clone()))) },

            // Dot separated idents
            [ Ident(id), Token::Punct(Punct::Dot), .. ] => {
                let tail = &tree[2..];
                let mut path = vec![id.clone()];
                let mut accessed = true;
                for token in tail {
                    if accessed {
                        match token {
                            Ident(id) => path.push(id.clone()),
                            Literal(lit) => path.push(lit.clone()), // Literal accessors are used for tuples
                            _ => panic!("Invalid var accessor in ident")
                        }
                        accessed = false;
                    } else {
                        match token {
                            Token::Punct(Punct::Dot) => accessed = true,
                            _ => panic!("Invalid punct in ident")
                        }
                    }
                }
                Expr::Id(Id::Path(PathId(path)))
            },

            // Parenthesized group
            [ Group(group, Delimiter::Parenthesis) ] => group.into(),
            _ => { panic!("Invalid expression stream {:#?}", tree) }
        }
    }
}

impl From<Vec<Token>> for Expr {
    fn from(tree: Vec<Token>) -> Self { (&tree[..]).into() }
}
impl From<&Vec<Token>> for Expr {
    fn from(tree: &Vec<Token>) -> Self { (&tree[..]).into() }
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

const EQUALS: Token = Token::Punct(Punct::Equals);
const SEMICOLON: Token = Token::Punct(Punct::Semicolon);
const COLON: Token = Token::Punct(Punct::Colon);
const COMMA: Token = Token::Punct(Punct::Comma);
const HASH: Token = Token::Punct(Punct::Hash);

#[derive(Eq, PartialEq, Clone, Debug)]
enum Token {
    Ident(String),
    Literal(String),
    Group(Vec<Token>, Delimiter),

    Keyword(Keyword),
    Punct(Punct),
    BOp(BinOp),
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
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum Punct {
    Equals,
    Comma,
    Semicolon,
    Colon,
    Hash,
    Dot,
}

impl From<&TokenTree> for Token {
    fn from(tree: &TokenTree) -> Self {
        match tree {
            TokenTree::Ident(ident) => {
                let s = ident.to_string();

                match s.as_str() {
                    "let" => { LET },
                    "mut" => { MUT },
                    "unwrap" => { UNWRAP },
                    "partial" => { PARTIAL },
                    "return" => { RETURN },
                    "if" => { IF },
                    "else" => { ELSE },
                    "for" => { FOR },
                    "in" => { IN },
                    "round" => { panic!("Reserved ident `round` used") },
                    _ => { Token::Ident(s) }
                }
            },
            TokenTree::Punct(punct) => {
                let s = punct.to_string();

                match s.as_str() {
                    "=" => { EQUALS },
                    ";" => { SEMICOLON },
                    ":" => { COLON },
                    "," => { COMMA },
                    "." => { Token::Punct(Punct::Dot) },
                    "#" => { HASH },

                    "+" => { BOp(BinOp::Add) },
                    "-" => { BOp(BinOp::Sub) },
                    "*" => { BOp(BinOp::Mul) },
                    _ => { panic!("Unknown punctation: {}", s) }
                }
            },
            TokenTree::Literal(lit) => { Token::Literal(lit.to_string()) },
            TokenTree::Group(group) => {
                let trees: Vec<TokenTree> = group.stream().into_iter().collect();
                let tokens: Vec<Token> = trees.iter().map(|g| g.into()).collect();
                Token::Group(tokens, group.delimiter())
            }
        }
    }
}