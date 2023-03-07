use elusiv_proc_macro_utils::enforce_field;
use proc_macro2::{TokenStream, TokenTree};
use quote::{quote, ToTokens};
use syn::{Data, Field, Type};

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
        Self {
            lifetimes: Vec::new(),
            all_lifetimes: quote!(),
        }
    }

    fn push(&mut self, lifetime: TokenStream) {
        self.lifetimes.push(lifetime.clone());
        self.all_lifetimes.extend(quote! { #lifetime , });
    }

    fn as_anonymous_lifetimes(&self) -> TokenStream {
        let mut s = quote!();
        for i in 0..self.lifetimes.len() {
            s.extend(quote!('_));

            if i < self.lifetimes.len() - 1 {
                s.extend(quote!(,));
            }
        }
        s
    }
}

impl ToTokens for Lifetimes {
    fn to_token_stream(&self) -> TokenStream {
        self.all_lifetimes.clone()
    }

    fn into_token_stream(self) -> TokenStream
    where
        Self: Sized,
    {
        self.all_lifetimes
    }

    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(self.all_lifetimes.clone())
    }
}

/// Returns the value for an inner attribute (syntax: `attr_ident: value`)
fn inner_attr_value(attr_ident: &str, inner: &TokenStream) -> TokenStream {
    let inner_attrs = match_inner(inner.clone());
    for ElusivAccountAttr { ident, value } in inner_attrs {
        if ident == attr_ident {
            return value;
        }
    }
    panic!("Inner attribute '{}' not found in '{}'", attr_ident, inner);
}

/// Checks whether a type is bound by lifetimes
fn is_type_lifetime_bound(ty: &Type) -> bool {
    ty.to_token_stream().to_string().contains('\'')
}

/// Anonymizes all lifetimes of a type
fn anonymize_type_lifetimes(ty: &mut Type) {
    if let Type::Path(syn::TypePath {
        path: syn::Path { segments, .. },
        ..
    }) = ty
    {
        for segment in segments.iter_mut() {
            if let syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                args,
                ..
            }) = &mut segment.arguments
            {
                for arg in args.iter_mut() {
                    if let syn::GenericArgument::Lifetime(lt) = arg {
                        lt.ident = syn::Ident::new(&String::from("_"), lt.ident.span());
                    }
                }
            }
        }
    }
}

pub fn impl_elusiv_account(ast: &syn::DeriveInput, attrs: TokenStream) -> TokenStream {
    let ident = ast.ident.clone();
    let eager_ident: TokenStream = format!("{}Eager", ident).parse().unwrap();
    let vis = &ast.vis.to_token_stream();
    let s = if let Data::Struct(s) = &ast.data {
        s
    } else {
        panic!("Only structs can be used with `elusiv_account`")
    };

    let struct_attrs = ast.attrs.iter().fold(TokenStream::new(), |acc, x| {
        let mut acc = acc;
        acc.extend(x.to_token_stream());
        acc
    });

    let attrs: Vec<TokenTree> = attrs.into_iter().collect();
    let attrs = match_attrs(&attrs);

    // The struct name is used as PDASeed
    // TODO: It would be ideal to use the whole module path to automatically prevent duplicates

    let mut lifetimes = Lifetimes::new();
    let mut field_idents = quote!();
    let mut field_defs = quote!();
    let mut fields_split = quote!();
    let mut fns = quote!();
    let mut sizes = Vec::new();
    let mut impls = quote!();
    let mut eager_idents = quote!();
    let mut eager_defs = quote!();
    let mut eager_init = quote!();
    let mut use_eager_type = false;

    // 'a lifetime for the `ProgramAccount` impl
    let program_account_lifetime = quote!('a);
    lifetimes.push(program_account_lifetime.clone());

    for attr in attrs {
        match attr.ident.as_str() {
            // Turns the account into an `ParentAccount` with `child_account_count` childs
            "parent_account" => {
                let child_account_count = inner_attr_value("child_account_count", &attr.value);
                let child_account_type = inner_attr_value("child_account", &attr.value);

                // TODO: field no longer does not need to be enforced at a specific index
                enforce_field(
                    quote! {
                        pubkeys : [ElusivOption < Pubkey >; #child_account_count]
                    },
                    1,
                    &s.fields,
                );

                // 'a, 'b, 't lifetimes for the `ParentAccount` impl
                lifetimes.push(quote!('b));
                lifetimes.push(quote!('t));
                let b_lifetime = lifetimes.lifetimes[1].clone();
                let t_lifetime = lifetimes.lifetimes[2].clone();

                impls.extend(quote!{
                    impl < #lifetimes > elusiv_types::accounts::ParentAccount < #program_account_lifetime, #b_lifetime, #t_lifetime > for #ident < #lifetimes > {
                        const COUNT: usize = #child_account_count;
                        type Child = #child_account_type;

                        fn set_child_accounts(parent: &mut Self, child_accounts: Vec<Option<&'b solana_program::account_info::AccountInfo<'t>>>) {
                            parent.child_accounts = child_accounts
                        }

                        fn set_child_pubkey(&mut self, index: usize, pubkey: ElusivOption<solana_program::pubkey::Pubkey>) {
                            self.set_pubkeys(index, &pubkey)
                        }

                        fn get_child_pubkey(&self, index: usize) -> Option<solana_program::pubkey::Pubkey> {
                            self.get_pubkeys(index).option()
                        }

                        unsafe fn get_child_account_unsafe(&self, child_index: usize) -> Result<& #b_lifetime solana_program::account_info::AccountInfo< #t_lifetime >, solana_program::program_error::ProgramError> {
                            match self.child_accounts[child_index] {
                                Some(child) => Ok(child),
                                None => Err(solana_program::program_error::ProgramError::NotEnoughAccountKeys)
                            }
                        }
                    }
                });

                impls.extend(quote! {
                    #[cfg(feature = "elusiv-client")]
                    impl elusiv_types::accounts::EagerParentAccountRepr for #eager_ident {
                        fn child_pubkeys(&self) -> Vec<Option<solana_program::pubkey::Pubkey>> {
                            self.pubkeys.iter()
                                .map(|p| p.option())
                                .collect()
                        }
                    }
                });

                field_idents.extend(quote! {
                    child_accounts,
                });

                fields_split.extend(quote!{
                    let child_accounts = vec![None; <Self as elusiv_types::accounts::ParentAccount>::COUNT];
                });

                field_defs.extend(quote!{
                    child_accounts: Vec<Option<&#b_lifetime solana_program::account_info::AccountInfo< #t_lifetime >>>,
                });
            }

            // Turns the account into a `ComputationAccount`
            "partial_computation" => {
                enforce_field(quote! { instruction : u32 }, 1, &s.fields);
                enforce_field(quote! { round : u32 }, 2, &s.fields);

                impls.extend(quote! {
                    #[cfg(feature = "elusiv-client")]
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

            // Opts-out of using the account as a PDAAccount
            "no_pda" => {
                todo!("no_pda")
            }

            "deserialized_type" => {
                todo!("deserialized_type")
            }

            // Adds the eager type variant (IFF the 'elusiv-client' feature is active)
            "eager_type" => {
                use_eager_type = true;
            }

            any => panic!("Invalid attribute '{}'", any),
        }
    }

    // Since all ElusivAccounts are PDAAccounts, they require leading PDAAccountData
    enforce_field(quote! { pda_data : PDAAccountData }, 0, &s.fields);

    for Field {
        attrs,
        vis,
        ident,
        ty,
        ..
    } in &s.fields
    {
        let field_ident = ident.clone().unwrap();
        let vis = vis.to_token_stream();
        let getter_ident: TokenStream = format!("get_{}", field_ident).parse().unwrap();
        let setter_ident: TokenStream = format!("set_{}", field_ident).parse().unwrap();
        let mut custom_field = false;
        let mut use_getter = true;
        let mut use_setter = true;

        if field_ident == "data" {
            panic!("'data' is a reserved keyword, please pick a different field identifier")
        }

        eager_idents.extend(quote! { #field_ident, });

        let mut doc = quote!();
        for attr in attrs {
            let attr_ident = attr.path.get_ident().unwrap().to_string();
            match attr_ident.as_str() {
                // Documentation
                "doc" => {
                    doc.extend(attr.to_token_stream());
                }

                // Type accepts the mutable slice and handles serialization/deserialization autonomously
                // - in consequence, skips creation of getter and setter functions
                // - note: the type needs to impl `elusiv_types::bytes::SizedType`
                "lazy" => {
                    use_getter = false;
                    use_setter = false;
                    custom_field = true;

                    field_defs.extend(quote! {
                        #doc
                        #vis #field_ident: #ty,
                    });

                    fields_split.extend(quote!{
                        let (#field_ident, data) = data.split_at_mut(<#ty as elusiv_types::bytes::SizedType>::SIZE);
                        let #field_ident = <#ty>::new(#field_ident);
                    });

                    // Because of the lifetime dependency of some custom fields, we only represent the types that don't use lifetimes
                    if is_type_lifetime_bound(ty) {
                        eager_defs.extend(quote! {
                            #doc
                            pub #field_ident: Vec<u8>,
                        });
                    } else {
                        eager_defs.extend(quote! {
                            #doc
                            pub #field_ident: #ty,
                        });
                    }
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

                any => panic!("Unknown attribute '{}' for field '{}'", any, field_ident),
            }
        }

        field_idents.extend(quote! {
            #field_ident,
        });

        if !custom_field {
            field_defs.extend(quote! {
                #doc
                #field_ident: &'a mut [u8],
            });

            eager_defs.extend(quote! {
                #doc
                pub #field_ident: #ty,
            });
        }

        match ty {
            Type::Path(_) => {
                if custom_field {
                    sizes.push(quote! { <#ty as elusiv_types::bytes::SizedType>::SIZE });

                    if is_type_lifetime_bound(ty) {
                        let mut ty2 = ty.clone();
                        anonymize_type_lifetimes(&mut ty2);
                        eager_init.extend(quote!{
                            let (#field_ident, data) = data.split_at(<#ty2 as elusiv_types::bytes::SizedType>::SIZE);
                            let #field_ident = #field_ident.to_vec();
                        });
                    } else {
                        eager_init.extend(quote!{
                            let (#field_ident, data) = data.split_at(<#ty as elusiv_types::bytes::SizedType>::SIZE);
                            let #field_ident = <#ty>::new(#field_ident)?;
                        });
                    }
                } else {
                    sizes.push(quote! { <#ty as elusiv_types::bytes::BorshSerDeSized>::SIZE });

                    fields_split.extend(quote!{
                        let (#field_ident, data) = data.split_at_mut(<#ty as elusiv_types::bytes::BorshSerDeSized>::SIZE);
                    });

                    eager_init.extend(quote!{
                        let (#field_ident, data) = data.split_at(<#ty as elusiv_types::bytes::BorshSerDeSized>::SIZE);
                        let #field_ident = <#ty as borsh::BorshDeserialize>::try_from_slice(#field_ident)?;
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
                        fns.extend(quote! {
                            #doc
                            #vis fn #setter_ident(&mut self, value: &#ty) {
                                let mut slice = &mut self.#field_ident[..<#ty as elusiv_types::bytes::BorshSerDeSized>::SIZE];
                                borsh::BorshSerialize::serialize(value, &mut slice).unwrap();
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
                let size = quote! { <#ty as elusiv_types::bytes::BorshSerDeSized>::SIZE * #len };
                sizes.push(size.clone());

                fields_split.extend(quote! {
                    let (#field_ident, data) = data.split_at_mut(#size);
                });

                eager_init.extend(quote!{
                    let (#field_ident, data) = data.split_at(#size);
                    let #field_ident = <[#ty; #len] as borsh::BorshDeserialize>::try_from_slice(#field_ident)?;
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
                    fns.extend(quote! {
                        #doc
                        #vis fn #setter_ident(&mut self, index: usize, value: &#ty) {
                            let offset = index * <#ty as elusiv_types::bytes::BorshSerDeSized>::SIZE;
                            let mut slice = &mut self.#field_ident[offset..offset + <#ty as elusiv_types::bytes::BorshSerDeSized>::SIZE];
                            borsh::BorshSerialize::serialize(value, &mut slice).unwrap();
                        }
                    });
                }
            }
            _ => panic!("Invalid field type '{:?}' for '{:?}'", ty, field_ident),
        }
    }

    let account_size_test: TokenStream =
        format!("test_{}_account_size", ident.to_string().to_lowercase())
            .parse()
            .unwrap();
    let account_size = sizes.iter().fold(quote!(), |acc, x| {
        if acc.is_empty() {
            x.clone()
        } else {
            quote! { #acc + #x }
        }
    });
    let anonymous_lifetimes = lifetimes.as_anonymous_lifetimes();

    let eager_type = if use_eager_type {
        quote! {
            #[cfg(feature = "elusiv-client")]
            #[derive(Debug, Clone)]
            #[derive(borsh::BorshSerialize)]
            #vis struct #eager_ident {
                #eager_defs
            }

            #[cfg(feature = "elusiv-client")]
            impl < #lifetimes > elusiv_types::accounts::EagerAccount < #program_account_lifetime > for #ident < #lifetimes > {
                type Repr = #eager_ident;
            }

            #[cfg(feature = "elusiv-client")]
            impl elusiv_types::accounts::EagerAccountRepr for #eager_ident {
                fn new(data: Vec<u8>) -> Result<Self, std::io::Error> {
                    if data.len() != < #ident < #anonymous_lifetimes > as elusiv_types::accounts::SizedAccount>::SIZE {
                        return Err(std::io::Error::new(std::io::ErrorKind::Other, "Invalid account data len"))
                    }

                    #eager_init

                    Ok(Self { #eager_idents })
                }
            }
        }
    } else {
        quote!()
    };

    quote! {
        #struct_attrs
        #[derive(elusiv_derive::PDAAccount)]
        #vis struct #ident < #lifetimes > {
            #field_defs
        }

        impl < #lifetimes > #ident < #lifetimes > {
            #fns
        }

        #impls

        impl < #lifetimes > elusiv_types::accounts::ProgramAccount < #program_account_lifetime > for #ident < #lifetimes > {
            fn new(data: &'a mut [u8]) -> Result<Self, solana_program::program_error::ProgramError> {
                if data.len() != <Self as elusiv_types::accounts::SizedAccount>::SIZE {
                    return Err(solana_program::program_error::ProgramError::InvalidAccountData)
                }

                #fields_split

                Ok(Self { #field_idents })
            }
        }

        impl < #lifetimes > elusiv_types::accounts::SizedAccount for #ident < #lifetimes > {
            const SIZE: usize = #account_size;
        }

        // Test to verify the account to be of valid PDA-size (10 KiB)
        #[cfg(test)]
        mod #account_size_test {
            use super::*;

            #[test]
            fn #account_size_test() {
                assert!(<#ident as elusiv_types::accounts::SizedAccount>::SIZE <= 10240);
            }
        }

        #eager_type
    }
}

/// Matches attributes with the syntac `ident: value, ..` with value being a `TokenStream`
fn match_attrs(tree: &[TokenTree]) -> Vec<ElusivAccountAttr> {
    match tree {
        [TokenTree::Ident(attr_ident), TokenTree::Punct(colon), tail @ ..] => {
            let ident = attr_ident.to_string();
            assert_eq!(colon.to_string(), ":", "Invalid character '{}'", colon);

            let (value, tail) = match tail {
                [any, TokenTree::Punct(comma), tail @ ..] => {
                    assert_eq!(
                        comma.to_string(),
                        ",",
                        "Invalid character '{}' in attribute '{}'",
                        comma,
                        ident
                    );
                    (any.to_token_stream(), match_attrs(tail))
                }
                [any] => (any.to_token_stream(), vec![]),
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
        [TokenTree::Ident(id)] => {
            vec![ElusivAccountAttr {
                ident: id.to_string(),
                value: quote!(),
            }]
        }
        [TokenTree::Literal(lit)] => {
            vec![ElusivAccountAttr {
                ident: lit.to_string(),
                value: quote!(),
            }]
        }
        [TokenTree::Group(g)] => match_attrs(&g.stream().into_iter().collect::<Vec<TokenTree>>()),
        _ => panic!("Invalid inner attributes '{}'", inner),
    }
}
