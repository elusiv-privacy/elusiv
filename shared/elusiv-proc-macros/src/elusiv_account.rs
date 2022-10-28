use std::str::FromStr;
use solana_program::pubkey::Pubkey;
use syn::{Type, Data, Field, Fields};
use quote::{quote, ToTokens};
use proc_macro2::{TokenStream, TokenTree};
use crate::program_id::read_program_id;

struct ElusivAccountAttr {
    ident: String,
    value: TokenStream,
}

struct Lifetimes {
    lifetimes: Vec<TokenStream>,
    all_lifetimes: TokenStream,
}

impl Lifetimes {
    fn new() -> Self {
        Self { lifetimes: Vec::new(), all_lifetimes: quote!() }
    }

    fn push(&mut self, lifetime: TokenStream) {
        self.lifetimes.push(lifetime.clone());
        self.all_lifetimes.extend(quote!{ #lifetime , });
    }
}

impl ToTokens for Lifetimes {
    fn to_token_stream(&self) -> TokenStream {
        self.all_lifetimes.clone()
    }

    fn into_token_stream(self) -> TokenStream where Self: Sized, {
       self.all_lifetimes 
    }

    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(self.all_lifetimes.clone())
    }
}

fn vis_token(vis: &syn::Visibility) -> TokenStream {
    if let syn::Visibility::Public(_) = vis {
        quote!(pub)
    } else {
        quote!()
    }
}

pub fn impl_elusiv_account(ast: &syn::DeriveInput, attrs: TokenStream) -> TokenStream {
    let ident = ast.ident.clone();
    let vis = vis_token(&ast.vis);
    let s = if let Data::Struct(s) = &ast.data { s } else { panic!("Only structs can be used with `elusiv_account`") };

    let attrs: Vec<TokenTree> = attrs.into_iter().collect();
    let attrs = match_attrs(&attrs);

    // The struct name is used as PDASeed
    // TODO: It would be ideal to use the whole module path to automatically prevent duplicates

    let ident_str = ident.to_string();
    let pda_seed = ident_str.as_bytes();
    let pda = Pubkey::find_program_address(
        &[pda_seed],
        &Pubkey::from_str(&read_program_id()).unwrap(),
    ).0;
    let single_instance_pda: TokenStream = format!("{:?}", pda.to_bytes()).parse().unwrap();

    let mut lifetimes = Lifetimes::new();
    let mut field_idents = quote!();
    let mut field_defs = quote!();
    let mut fields_split = quote!();
    let mut fns = quote!();
    let mut sizes = Vec::new();
    let mut impls = quote!();

    // Signature for the `new`
    let mut account_ty = quote!{ elusiv_types::accounts::ProgramAccount };
    let mut new_signature = quote!{ d: &'a mut [u8], };

    // 'a lifetime for the `ProgramAccount` impl
    lifetimes.push(quote!('a));

    for attr in attrs {
        match attr.ident.as_str() {
            // Turns the account into an `MultiAccountAccount`
            "multi_account" => {
                let sub_account_count = inner_attr_value("sub_account_count", &attr.value);
                let sub_account_size = inner_attr_value("sub_account_size", &attr.value);

                enforce_field(
                    quote!{
                        multi_account_data : MultiAccountAccountData < #sub_account_count >
                    },
                    1,
                    &s.fields,
                );
            
                account_ty = quote!{ elusiv_types::accounts::MultiAccountProgramAccount };

                // 'a, 'b, 't lifetimes for the `MultiAccountProgramAccount` impl
                lifetimes.push(quote!('b));
                lifetimes.push(quote!('t));
                let b_lifetime = lifetimes.lifetimes[1].clone();
                let t_lifetime = lifetimes.lifetimes[2].clone();

                new_signature = quote!{
                    d: &'a mut [u8], accounts: std::collections::HashMap<usize, &'b solana_program::account_info::AccountInfo<'t>>,
                };

                impls.extend(quote!{
                    impl < #lifetimes > elusiv_types::accounts::MultiAccountAccount < #t_lifetime > for #ident < #lifetimes > {
                        const COUNT: usize = #sub_account_count;
                        const ACCOUNT_SIZE: usize = #sub_account_size;

                        unsafe fn get_account_unsafe(&self, account_index: usize) -> Result<&solana_program::account_info::AccountInfo< #t_lifetime >, solana_program::program_error::ProgramError> {
                            match self.accounts.get(&account_index) {
                                Some(&m) => Ok(m),
                                None => Err(solana_program::program_error::ProgramError::InvalidArgument)
                            }
                        }
                    }
                });

                field_idents.extend(quote!{
                    accounts,
                });

                field_defs.extend(quote!{
                    #[doc = "The sub accounts, mapped by their index"]
                    accounts: std::collections::HashMap<usize, &#b_lifetime solana_program::account_info::AccountInfo< #t_lifetime >>,
                });
            }

            // Turns the account into a `ComputationAccount`
            "partial_computation" => {
                enforce_field(quote!{ instruction : u32 }, 1, &s.fields);
                enforce_field(quote!{ round : u32 }, 2, &s.fields);

                impls.extend(quote! {
                    #[cfg(feature = "instruction-abi")]
                    impl < #lifetimes > elusiv_types::accounts::ComputationAccount for #ident < #lifetimes > {
                        fn instruction(&self) -> u32 {
                            self.get_instruction()
                        }

                        fn round(&self) -> u32 {
                            self.get_round()
                        }
                    }
                });
            }
            
            // Turns the account into a `SingleInstancePDAAccount`
            "single_instance" => {
                impls.extend(quote!{
                    impl < #lifetimes > elusiv_types::accounts::SingleInstancePDAAccount for #ident < #lifetimes > {
                        const SINGLE_INSTANCE_ADDRESS: solana_program::pubkey::Pubkey = solana_program::pubkey::Pubkey::new_from_array(#single_instance_pda);
                    }
                });
            }

            // Opts-out of using the account as a PDAAccount
            "no_pda" => {
                todo!("no_pda")
            }

            any => panic!("Invalid attribute '{}'", any)
        }
    }

    // Since all ElusivAccounts are PDAAccounts, they require leading PDAAccountData
    enforce_field(quote!{ pda_data : PDAAccountData }, 0, &s.fields);

    for Field {
        attrs,
        vis,
        ident,
        ty,
        ..
    } in &s.fields {
        let field_ident = ident.clone().unwrap();
        let vis = vis_token(vis);
        let getter_ident: TokenStream = format!("get_{}", field_ident).parse().unwrap();
        let setter_ident: TokenStream = format!("set_{}", field_ident).parse().unwrap();
        let mut custom_field = false;
        let mut use_getter = true;
        let mut use_setter = true;

        let mut doc = quote!();
        for attr in attrs {
            let attr_ident = attr.path.get_ident().unwrap().to_string();
            match attr_ident.as_str() {
                // Documentation
                "doc" => {
                    doc.extend(attr.to_token_stream());
                }

                // Type accpets the mutable slice and handles serialization/deserialization autonomously
                // - in consequence, skips creation of getter and setter functions
                // - note: the type needs to impl `elusiv_types::bytes::SizedType`
                "pub_non_lazy" => {
                    use_getter = false;
                    use_setter = false;
                    custom_field = true;
        
                    field_defs.extend(quote!{
                        #doc
                        #vis #field_ident: #ty,
                    });

                    fields_split.extend(quote!{
                        let (#field_ident, d) = d.split_at_mut(<#ty as elusiv_types::bytes::SizedType>::SIZE);
                        let #field_ident = <#ty>::new(#field_ident);
                    });
                }

                "lazy" => {
                    todo!("lazy")
                }

                // Deserializes the value by default
                "deserialize_by_default" => {
                    todo!("deserialize_by_default")
                }

                // Skips creation of a getter function
                "no_getter" => {
                    use_getter = false;
                }

                // Skips creation of a setter function
                "no_setter" => {
                    use_setter = false;
                }

                any => panic!("Unknown attribute '{}' for field '{}'", any, field_ident)
            }
        }

        field_idents.extend(quote!{
            #field_ident,
        });

        if !custom_field {
            field_defs.extend(quote!{
                #doc
                #field_ident: &'a mut [u8],
            });
        }

        match ty {
            Type::Path(_) => {
                if custom_field {
                    sizes.push(quote!{ <#ty as elusiv_types::bytes::SizedType>::SIZE });
                } else {
                    sizes.push(quote!{ <#ty as elusiv_types::bytes::BorshSerDeSized>::SIZE });

                    fields_split.extend(quote!{
                        let (#field_ident, d) = d.split_at_mut(<#ty as elusiv_types::bytes::BorshSerDeSized>::SIZE);
                    });

                    if use_getter {
                        fns.extend(quote!{
                            #doc
                            #vis fn #getter_ident(&self) -> #ty {
                                <#ty as borsh::BorshDeserialize>::try_from_slice(self.#field_ident).unwrap()
                            }
                        });
                    }
                    
                    if use_setter {
                        fns.extend(quote!{
                            #doc
                            #vis fn #setter_ident(&mut self, value: &#ty) {
                                let v = <#ty as borsh::BorshSerialize>::try_to_vec(value).unwrap();
                                self.#field_ident[..v.len()].copy_from_slice(&v[..]);
                            }
                        });
                    }
                }
            }
            Type::Array(array) => {
                if custom_field {
                    panic!("Custom fields are not allowed with Array-types");
                }

                let ty = array.elem.clone().into_token_stream();
                let len = array.len.clone();
                let size = quote!{ <#ty as elusiv_types::bytes::BorshSerDeSized>::SIZE * #len };
                sizes.push(size.clone());

                fields_split.extend(quote!{
                    let (#field_ident, d) = d.split_at_mut(#size);
                });

                if use_getter {
                    fns.extend(quote!{
                        #doc
                        #vis fn #getter_ident(&self, index: usize) -> #ty {
                            let slice = &self.#field_ident[index * <#ty as elusiv_types::bytes::BorshSerDeSized>::SIZE..(index + 1) * <#ty as elusiv_types::bytes::BorshSerDeSized>::SIZE];
                            <#ty as borsh::BorshDeserialize>::try_from_slice(slice).unwrap()
                        }
                    });
                }

                if use_setter {
                    fns.extend(quote!{
                        #doc
                        #vis fn #setter_ident(&mut self, index: usize, value: &#ty) {
                            let offset = index * <#ty as elusiv_types::bytes::BorshSerDeSized>::SIZE;
                            let v = <#ty as borsh::BorshSerialize>::try_to_vec(value).unwrap();
                            self.#field_ident[offset..][..v.len()].copy_from_slice(&v[..]);
                        }
                    });
                }
            }
            _ => panic!("Invalid field type '{:?}' for '{:?}'", ty, field_ident)
        }
    }

    let pda_seed_tokens: TokenStream = format!("{:?}", pda_seed).parse().unwrap();
    let ident_str = ident_str.as_str();
    let account_size = sizes.iter()
        .fold(quote!(), |acc, x| {
            if acc.is_empty() {
                x.clone()
            } else {
                quote!{ #acc + #x }
            }
        });

    quote! {
        #vis struct #ident < #lifetimes > {
            #field_defs
        }

        impl < #lifetimes > #ident < #lifetimes >{
            #fns
        }

        #impls

        impl < #lifetimes > #account_ty < #lifetimes > for #ident < #lifetimes > {
            fn new(#new_signature) -> Result<Self, solana_program::program_error::ProgramError> {
                if d.len() != <Self as elusiv_types::accounts::SizedAccount>::SIZE {
                    return Err(solana_program::program_error::ProgramError::InvalidArgument)
                }

                #fields_split

                Ok(Self { #field_idents })
            }
        }

        impl < #lifetimes > elusiv_types::accounts::SizedAccount for #ident < #lifetimes > {
            const SIZE: usize = #account_size;
        }

        impl < #lifetimes > elusiv_types::accounts::PDAAccount for #ident < #lifetimes > {
            const PROGRAM_ID: solana_program::pubkey::Pubkey = crate::PROGRAM_ID;            
            const SEED: &'static [u8] = &#pda_seed_tokens;

            #[cfg(feature = "instruction-abi")]
            const IDENT: &'static str = #ident_str;
        }
    }
}

/// Enforces that a field definition at a specific index matches the stream (visibility is ignored)
fn enforce_field(stream: TokenStream, index: usize, fields: &Fields) {
    let field = fields.iter().collect::<Vec<&Field>>()[index].clone();
    let ident = field.ident;
    let ty = field.ty;
    let expected = quote!{ #ident : #ty }.to_string();

    assert_eq!(
        expected,
        stream.to_string(),
        "Invalid field at {}. Exptected '{}', got '{}'",
        index,
        expected,
        stream
    );
}

/// Matches attributes with the syntac `ident: value, ..` with value being a `TokenStream`
fn match_attrs(tree: &[TokenTree]) -> Vec<ElusivAccountAttr> {
    match tree {
        [
            TokenTree::Ident(attr_ident),
            TokenTree::Punct(colon),
            tail @ ..
        ] => {
            let ident = attr_ident.to_string();
            assert_eq!(colon.to_string(), ":", "Invalid character '{}'", colon);

            let (value, tail) = match tail {
                [
                    any,
                    TokenTree::Punct(comma),
                    tail @ ..
                ] => {
                    assert_eq!(comma.to_string(), ",", "Invalid character '{}' in attribute '{}'", comma, ident);
                    (any.to_token_stream(), match_attrs(tail))
                }
                [ any ] => (any.to_token_stream(), vec![]),
                _ => panic!("Invalid value for argument '{}'", ident),
            };

            let mut v = vec![ElusivAccountAttr { ident, value }];
            v.extend(tail);
            v
        }
        [] => Vec::new(),
        _ => panic!("Invalid arguments"),
    }
}

/// Matches an inner `TokenStream` (either as attributes with `match_attrs` or as `Literal`/`Ident`)
fn match_inner(inner: TokenStream) -> Vec<ElusivAccountAttr> {
    let tree: Vec<TokenTree> = inner.clone().into_iter().collect();

    match &tree[..] {
        [ TokenTree::Ident(id) ] => {
            vec![ElusivAccountAttr { ident: id.to_string(), value: quote!() }]
        }
        [ TokenTree::Literal(lit) ] => {
            vec![ElusivAccountAttr { ident: lit.to_string(), value: quote!() }]
        }
        [ TokenTree::Group(g) ] => {
            match_attrs(&g.stream().into_iter().collect::<Vec<TokenTree>>())
        }
        _ => panic!("Invalid inner attributes '{}'", inner)
    }
}

/// Returns the value for an inner attribute (syntax: `attr_ident: value`)
fn inner_attr_value(attr_ident: &str, inner: &TokenStream) -> TokenStream {
    let inner_attrs = match_inner(inner.clone());
    for ElusivAccountAttr { ident, value } in inner_attrs {
        if ident == attr_ident {
            return value
        }
    }
    panic!("Inner attribute '{}' not found in '{}'", attr_ident, inner);
}