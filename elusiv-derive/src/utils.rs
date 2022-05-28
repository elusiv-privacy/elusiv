use std::string::ToString;

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

/// Named sub attribute example: #[macro(named_sub_attr = value)] (returns value)
pub fn named_sub_attribute<'a>(name: &str, attr: &'a str) -> &'a str {
    let ident = String::from(name) + "=";
    assert!(attr.starts_with(&ident), "Parameter ({}) does not start with: '{}' (whitespace sensitive!)", attr, ident);
    attr.strip_prefix(&ident).unwrap()
}