use super::state::*;

pub fn verify_proof(
    account: &mut ProofVerificationAccount,
    _iteration: usize
) -> bool {
    let result = account.pop_fq12();
    result == super::alpha_g1_beta_g2()
}