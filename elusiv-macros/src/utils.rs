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

/// Very simple converter from upper camel case to upper snake case
/// - so simple that it does not even handle multiple consecutive caps letters, so don't use them
pub fn upper_camel_to_upper_snake(camel: &str) -> String {
    let mut snake = String::new();

    for (i, char) in camel.chars().enumerate() {
        if char.is_uppercase() && i > 0 {
            snake.push_str("_");
        }
        snake.push_str(&char.to_uppercase().to_string());
    }

    snake
}

/// Removes whitespaces and the first and last brackets
pub fn sub_attrs_prepare<'a>(sub_attrs: String) -> String {
    let mut sub_attrs = String::from(sub_attrs);
    sub_attrs.retain(|c| !c.is_whitespace());
    //sub_attrs.pop();
    //sub_attrs.remove(0);
    sub_attrs
}

/// Named sub attribute example: #[macro(named_sub_attr = value)] (returns value)
pub fn named_sub_attribute<'a>(name: &str, attr: &'a str) -> &'a str {
    let ident = String::from(name) + "=";
    assert!(attr.starts_with(&ident), "Parameter ({}) does not start with: '{}' (whitespace sensitive!)", attr, ident);
    attr.strip_prefix(&ident).unwrap()
}