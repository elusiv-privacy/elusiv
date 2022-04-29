use proc_macro2::Span;
use std::string::ToString;
use syn::{ Ident, Expr, Lit, ExprLit, LitInt };

pub fn ident_with_prefix(ident: &Ident, prefix: &str) -> Ident {
    let mut name = String::from(prefix);
    name.push_str(&ident.to_string());
    Ident::new(&name, Span::call_site())
}

pub fn int_expr(num: usize) -> Expr {
    Expr::Lit(ExprLit{
        attrs: vec![],
        lit: Lit::Int(LitInt::new(&num.to_string(), Span::call_site()))
    })
}