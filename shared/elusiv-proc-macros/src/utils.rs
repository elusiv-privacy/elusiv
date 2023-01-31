/// Removes whitespaces and the first and last brackets
pub fn sub_attrs_prepare(sub_attrs: String) -> String {
    let mut sub_attrs = sub_attrs;
    sub_attrs.retain(|c| !c.is_whitespace());
    sub_attrs
}
