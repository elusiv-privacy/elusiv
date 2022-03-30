use std::string::ToString;
use quote::quote;

pub fn impl_account(ast: &proc_macro::TokenStream) -> proc_macro2::TokenStream {
    let args = ast.to_string();
    let args: Vec<&str> = args.split(",").collect();
    let ident = args[0];

    if args.len() == 1 {   // Program account objects
        let (name, ty) = get_account(ident);

        // Unpack account and check for correct account id
        quote! {
            let #name = solana_program::account_info::next_account_info(account_info_iter)?;
            if *#name.key != #ty::ID {
                return Err(crate::error::ElusivError::InvalidAccount.into());
            }
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

                    if *#name.key != crate::pool::ID {
                        return Err(crate::error::ElusivError::InvalidAccount.into());
                    }
                }
            },
            "nullifier" => {
                quote! {
                    let nullifier_acc_info = solana_program::account_info::next_account_info(account_info_iter)?;
                    if *nullifier_acc_info.owner != crate::id() {
                        return Err(crate::error::ElusivError::InvalidAccount.into());
                    }

                    // Check if nullifier account is active or archived
                    archive_account.is_nullifier_account_valid(&storage_account, nullifier_acc_info.key.to_bytes())?; 

                    let acc_data = &mut nullifier_acc_info.data.borrow_mut()[..];
                    let mut #name = NullifierAccount::new(&nullifier_acc_info, acc_data)?;

                    // Check that key saved in nullifier account matches too
                    if nullifier_acc_info.key.to_bytes() != #name.get_key() {
                        return Err(crate::error::ElusivError::InvalidAccount.into());
                    }
                }
            }
            _ => { panic!("Invalid role {}", role); }
        }
    } else {
        panic!("Invalid arguments");
    }
}

const ACCOUNTS: [&'static str; 5] = [
    "Storage",
    "Archive",
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