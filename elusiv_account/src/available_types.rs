use proc_macro2::TokenStream;
use syn::{ Ident, Expr };
use quote::quote;

pub struct FieldType {
    pub ident: &'static str,
    pub byte_size: usize,

    pub def: Option<Box<dyn Fn(&Ident) -> TokenStream>>,
    pub split: Option<Box<dyn Fn(&Ident, Expr) -> TokenStream>>,

    pub getter: Option<Box<dyn Fn(&Ident, &Ident) -> TokenStream>>,
    pub setter: Option<Box<dyn Fn(&Ident, &Ident) -> TokenStream>>,
}

// All types that fields are allowed to have
pub fn available_types() -> Vec<FieldType> {
    vec![
        FieldType {
            ident: "u64",
            byte_size: 8,
            def: None, split: None,
            getter: Some(Box::new(|getter_name: &Ident, field_name: &Ident| { quote! {
                pub fn #getter_name(&self) -> u64 {
                    let bytes = [ self.#field_name[0], self.#field_name[1], self.#field_name[2], self.#field_name[3], self.#field_name[4], self.#field_name[5], self.#field_name[6], self.#field_name[7] ];
                    u64::from_le_bytes(bytes)
                }
            }})),
            setter: Some(Box::new(|setter_name: &Ident, field_name: &Ident| { quote! {
                pub fn #setter_name(&mut self, value: u64) {
                    let bytes = value.to_le_bytes();
                    for (i, &byte) in bytes.iter().enumerate() {
                        self.#field_name[i] = byte;
                    }
                }
            }})),
        },

        FieldType {
            ident: "bool",
            byte_size: 1,
            def: None, split: None,
            getter: Some(Box::new(|getter_name: &Ident, field_name: &Ident| { quote! {
                pub fn #getter_name(&self) -> bool {
                    self.#field_name[0] == 1
                }
            }})),
            setter: Some(Box::new(|setter_name: &Ident, field_name: &Ident| { quote! {
                pub fn #setter_name(&mut self, value: bool) {
                    self.#field_name[0] = if value { 1 } else { 0 };
                }
            }})),
        },

        FieldType {
            ident: "U256",
            byte_size: 32,
            def: None, split: None,
            getter: Some(Box::new(|getter_name: &Ident, field_name: &Ident| { quote! {
                pub fn #getter_name(&self) -> U256 {
                    let mut a = [0; 32];
                    for i in 0..32 { a[i] = self.#field_name[i]; }
                    a
                }
            }})),
            setter: Some(Box::new(|setter_name: &Ident, field_name: &Ident| { quote! {
                pub fn #setter_name(&mut self, i: usize, bytes: &[u8]) {
                    for (i, &byte) in bytes.iter().enumerate() {
                        self.#field_name[i] = byte;
                    }
                }
            }})),
        },
        
        FieldType {
            ident: "Option < U256 >",
            byte_size: 32 + 1,
            def: None, split: None,
            getter: Some(Box::new(|getter_name: &Ident, field_name: &Ident| { quote! {
                pub fn #getter_name(&self) -> Option<U256> {
                    let mut option = self.#field_name[0];
                    let mut a = [0; 32];
                    for i in 0..32 { a[i] = self.#field_name[i + 1]; }

                    if option == 0 {
                        None
                    } else {
                        Some(a)
                    }
                }
            }})),
            setter: Some(Box::new(|setter_name: &Ident, field_name: &Ident| { quote! {
                pub fn #setter_name(&mut self, i: usize, value: Option<U256>) {
                    match value {
                        None => {
                            self.#field_name[0] = 0;
                        },
                        Some(v) => {
                            self.#field_name[0] = 1;
                            for (i, &byte) in v.iter().enumerate() {
                                self.#field_name[i + 1] = byte;
                            }
                        }
                    }
                }
            }})),
        },

        FieldType {
            ident: "[U256]",
            byte_size: 32,
            def: None, split: None,
            getter: Some(Box::new(|getter_name: &Ident, field_name: &Ident| { quote! {
                pub fn #getter_name(&self, i: usize) -> U256 {
                    let offset = i * 32;
                    let mut a = [0; 32];
                    for i in 0..32 { a[i] = self.#field_name[offset + i]; }
                    a
                }
            }})),
            setter: Some(Box::new(|setter_name: &Ident, field_name: &Ident| { quote! {
                pub fn #setter_name(&mut self, i: usize, bytes: &[u8]) {
                    let offset = i * 32;
                    for (i, &byte) in bytes.iter().enumerate() {
                        self.#field_name[offset + i] = byte;
                    }
                }
            }})),
        },

        FieldType {
            ident: "G1Affine",
            byte_size: 65,
            def: None, split: None,
            getter: Some(Box::new(|getter_name: &Ident, field_name: &Ident| { quote! {
                pub fn #getter_name(&self) -> G1Affine {
                    read_g1_affine(self.#field_name)
                }
            }})),
            setter: Some(Box::new(|setter_name: &Ident, field_name: &Ident| { quote! {
                pub fn #setter_name(&mut self, value: G1Affine) {
                    write_g1_affine(self.#field_name, value);
                }
            }})),
        },

        FieldType {
            ident: "G2Affine",
            byte_size: 129,
            def: None, split: None,
            getter: Some(Box::new(|getter_name: &Ident, field_name: &Ident| { quote! {
                pub fn #getter_name(&self) -> G2Affine {
                    read_g2_affine(self.#field_name)
                }
            }})),
            setter: Some(Box::new(|setter_name: &Ident, field_name: &Ident| { quote! {
                pub fn #setter_name(&mut self, value: G2Affine) {
                    write_g2_affine(self.#field_name, value);
                }
            }})),
        },
    ]
}