use std::string::ToString;
use quote::quote;

pub fn impl_account(ast: &proc_macro::TokenStream) -> proc_macro2::TokenStream {
    let args = ast.to_string();
    let args: Vec<&str> = args.split(",").collect();
    let ident = args[0];

    if args.len() == 1 {   // Program account objects
        let (name, ty) = get_account(ident);

        quote! {
            let #name = solana_program::account_info::next_account_info(account_info_iter)?;
            let acc_data = &mut #name.data.borrow_mut()[..];
            let mut #name = #ty::new(&#name, acc_data)?;
        }
    } else if args.len() == 2 {    // Account info objects
        let role = String::from(args[1]).replace(" ", "");
        let role = role.as_str();
        let name: proc_macro2::TokenStream = ident.to_lowercase().parse().unwrap();

        match role {
            "signer" => {
                quote! {
                    let #name = solana_program::account_info::next_account_info(account_info_iter)?;
                    if !#name.is_signer {
                        return Err(crate::error::ElusivError::SenderIsNotSigner.into());
                    }
                }
            },
            "no_check" => {
                quote! {
                    let #name = solana_program::account_info::next_account_info(account_info_iter)?;
                }
            },
            "pool" => {
                quote! {
                    let #name = solana_program::account_info::next_account_info(account_info_iter)?;

                    if *#name.owner != crate::id() {
                        return Err(crate::error::ElusivError::InvalidAccount.into());
                    }

                    if *#name.key != crate::pool::id() {
                        return Err(crate::error::ElusivError::InvalidAccount.into());
                    }
                }
            },
            _ => { panic!("Invalid role {}", role); }
        }
    } else {
        panic!("Invalid arguments");
    }
}

const ACCOUNTS: [&'static str; 4] = [
    "Storage",
    "Queue",
    "Commitment",
    "Proof",
];

pub fn get_account(acc: &str) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    if let Some(_) = ACCOUNTS.iter().find(|&a| *a == acc) {
        let name = acc.to_lowercase() + "_account";
        let name: proc_macro2::TokenStream = name.parse().unwrap();

        let ty = String::from(acc) + "Account";
        let ty: proc_macro2::TokenStream = ty.parse().unwrap();

        (name, ty)
    } else {
        panic!("Invalid account {}", acc);
    }
}