//! Program account storing funds
 
pub struct ReserveAccount {}
impl crate::state::program_account::PDAAccount for ReserveAccount {
    const SEED: &'static [u8] = b"sol_reserve";
}