use std::string::ToString;
use quote::quote;

pub fn impl_account(ast: &proc_macro::TokenStream) -> proc_macro2::TokenStream {
    let args = ast.to_string();
    let args: Vec<&str> = args.split(", ").collect();
    let ident = args[0];

    //panic!("{:?}", args);

    if args.len() == 1 {   // Program account objects
        let (name, ty) = get_account(ident);

        quote! {
            let #name = solana_program::account_info::next_account_info(account_info_iter)?;
            let acc_data = &mut #name.data.borrow_mut()[..];
            let mut #name = #ty::new(&#name, acc_data)?;
        }
    } else if args.len() == 2 {    // Account info objects
        let role = args[1];

        match role {
            "signer" => {
                let name = get_signer(ident);

                quote! {
                    let #name = solana_program::account_info::next_account_info(account_info_iter)?;
                    if !#name.is_signer {
                        return Err(crate::error::ElusivError::SenderIsNotSigner.into());
                    }
                }
            },
            "user" => {
                let name: proc_macro2::TokenStream = ident.to_lowercase().parse().unwrap();

                quote! {
                    let #name = solana_program::account_info::next_account_info(account_info_iter)?;
                }
            },
            "pool" => {
                let name: proc_macro2::TokenStream = ident.to_lowercase().parse().unwrap();

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

const SIGNERS: [&'static str; 3] = [
    "Sender",
    "Relayer",
    "Cranker",
];

pub fn get_signer(acc: &str) -> proc_macro2::TokenStream {
    if let Some(_) = SIGNERS.iter().find(|&a| *a == acc) {
        let name = acc.to_lowercase();
        let name: proc_macro2::TokenStream = name.parse().unwrap();

        name
    } else {
        panic!("Invalid account {}", acc);
    }
}