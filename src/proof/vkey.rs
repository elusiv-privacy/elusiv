mod vkey_migrate;
mod vkey_merge;
mod vkey_send;

pub use vkey_migrate::*;
pub use vkey_merge::*;
pub use vkey_send::*;

use ark_bn254::{ Fq2, Fq12, G1Affine, G2Affine, G1Projective };
use super::verifier::{ COMBINED_MILLER_LOOP_ROUNDS_COUNT, FINAL_EXPONENTIATION_ROUNDS_COUNT };

/// Groth16 verification key
/// https://github.com/arkworks-rs/groth16/blob/765817f77a6e14964c6f264d565b18676b11bd59/src/data_structures.rs#L44
pub trait VerificationKey {
    const PUBLIC_INPUTS_COUNT: usize;

    const PREPARE_PUBLIC_INPUTS_ROUNDS: usize = Self::PUBLIC_INPUTS_COUNT * super::PREPARE_PUBLIC_INPUTS_ROUNDS;
    const COMBINED_MILLER_LOOP_ROUNDS: usize = Self::PREPARE_PUBLIC_INPUTS_ROUNDS + COMBINED_MILLER_LOOP_ROUNDS_COUNT;
    const FINAL_EXPONENTIATION_ROUNDS: usize = Self::COMBINED_MILLER_LOOP_ROUNDS + FINAL_EXPONENTIATION_ROUNDS_COUNT;
    const ROUNDS: usize = Self::FINAL_EXPONENTIATION_ROUNDS;

    fn gamma_abc_g1_0() -> G1Projective;
    fn gamma_abc_g1(index: usize) -> G1Affine;
    fn alpha_g1_beta_g2() -> Fq12;

    fn gamma_g2_neg_pc_0(index: usize) -> Fq2;
    fn gamma_g2_neg_pc_1(index: usize) -> Fq2;
    fn gamma_g2_neg_pc_2(index: usize) -> Fq2;

    fn delta_g2_neg_pc_0(index: usize) -> Fq2;
    fn delta_g2_neg_pc_1(index: usize) -> Fq2;
    fn delta_g2_neg_pc_2(index: usize) -> Fq2;

    fn alpha_g1() -> G1Affine;
    fn beta_g2() -> G2Affine;
    fn gamma_g2() -> G2Affine;
    fn delta_g2() -> G2Affine;
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::Bn254;
    use ark_groth16::{prepare_verifying_key, VerifyingKey};

    #[test]
    fn test_vkeys() {
        test_vkey::<SendBinaryVKey>();
        test_vkey::<MergeBinaryVKey>();
        test_vkey::<MigrateUnaryVKey>();
    }

    fn test_vkey<VKey: VerificationKey>() {
        let mut gamma_abc_g1 = Vec::new();
        for i in 0..=VKey::PUBLIC_INPUTS_COUNT { gamma_abc_g1.push(VKey::gamma_abc_g1(i)); }
        let vk = VerifyingKey::<Bn254> {
            alpha_g1: VKey::alpha_g1(),
            beta_g2: VKey::beta_g2(),
            gamma_g2: VKey::gamma_g2(),
            delta_g2: VKey::delta_g2(),
            gamma_abc_g1,
        };
        let pvk = prepare_verifying_key(&vk);

        assert_eq!(VKey::alpha_g1_beta_g2(), pvk.alpha_g1_beta_g2);

        for (i, c) in pvk.delta_g2_neg_pc.ell_coeffs.iter().enumerate() {
            assert_eq!(VKey::delta_g2_neg_pc_0(i), c.0);
            assert_eq!(VKey::delta_g2_neg_pc_1(i), c.1);
            assert_eq!(VKey::delta_g2_neg_pc_2(i), c.2);
        }

        for (i, c) in pvk.gamma_g2_neg_pc.ell_coeffs.iter().enumerate() {
            assert_eq!(VKey::gamma_g2_neg_pc_0(i), c.0);
            assert_eq!(VKey::gamma_g2_neg_pc_1(i), c.1);
            assert_eq!(VKey::gamma_g2_neg_pc_2(i), c.2);
        }
    }
}