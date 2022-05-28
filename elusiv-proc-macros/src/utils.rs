use proc_macro2::Span;
use std::string::ToString;
use syn::Ident;

pub fn ident_with_prefix(ident: &Ident, prefix: &str) -> Ident {
    let mut name = String::from(prefix);
    name.push_str(&ident.to_string());
    Ident::new(&name, Span::call_site())
}

/// Removes whitespaces and the first and last brackets
pub fn sub_attrs_prepare<'a>(sub_attrs: String) -> String {
    let mut sub_attrs = String::from(sub_attrs);
    sub_attrs.retain(|c| !c.is_whitespace());
    sub_attrs
}

/// Named sub attribute example: #[macro(named_sub_attr = value)] (returns value)
pub fn named_sub_attribute<'a>(name: &str, attr: &'a str) -> &'a str {
    let ident = String::from(name) + "=";
    assert!(attr.starts_with(&ident), "Parameter ({}) does not start with: '{}' (whitespace sensitive!)", attr, ident);
    attr.strip_prefix(&ident).unwrap()
}