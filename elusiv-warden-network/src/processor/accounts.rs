/// Closes a program owned account in devnet and localhost
/// 
/// # Note
/// 
/// - `signer` needs to be the program's keypair
#[cfg(not(feature = "mainnet"))]
pub fn close_program_account<'a>(
    signer: &solana_program::account_info::AccountInfo<'a>,
    account: &solana_program::account_info::AccountInfo<'a>,
) -> solana_program::entrypoint::ProgramResult {
    assert!(!cfg!(feature = "mainnet"));
    assert_eq!(*signer.key, crate::ID);

    elusiv_utils::close_account(signer, account)
}