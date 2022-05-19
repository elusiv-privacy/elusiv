//! Currently the single SOL pool used to store funds

pub struct PoolAccount {}
impl crate::state::program_account::PDAAccount for PoolAccount {
    const SEED: &'static [u8] = b"sol_pool";
}