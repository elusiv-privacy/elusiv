mod vkey_send;
mod vkey_migrate;
#[cfg(test)] mod vkey_test;

pub use vkey_migrate::*;
pub use vkey_send::*;
#[cfg(test)] pub use vkey_test::*;

use ark_bn254::{ Fq2, Fq12, G1Affine, G2Affine, G1Projective };

/// Prepared Groth16 verification key
/// https://github.com/arkworks-rs/groth16/blob/765817f77a6e14964c6f264d565b18676b11bd59/src/data_structures.rs#L44
pub trait VerificationKey {
    const PUBLIC_INPUTS_COUNT: usize;

    const PREPARE_PUBLIC_INPUTS_ROUNDS: usize = Self::PUBLIC_INPUTS_COUNT * super::PREPARE_PUBLIC_INPUTS_ROUNDS;

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

    #[cfg(test)]
    fn ark_vk() -> ark_groth16::VerifyingKey<ark_bn254::Bn254> {
        let mut gamma_abc_g1 = Vec::new();
        for i in 0..=Self::PUBLIC_INPUTS_COUNT {
            gamma_abc_g1.push(Self::gamma_abc_g1(i));
        }

        ark_groth16::VerifyingKey {
            alpha_g1: Self::alpha_g1(),
            beta_g2: Self::beta_g2(),
            gamma_g2: Self::gamma_g2(),
            delta_g2: Self::delta_g2(),
            gamma_abc_g1,
        }
    }

    #[cfg(test)]
    fn ark_pvk() -> ark_groth16::PreparedVerifyingKey<ark_bn254::Bn254> {
        ark_groth16::prepare_verifying_key(&Self::ark_vk())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vkeys() {
        test_vkey::<SendQuadraVKey>();
        test_vkey::<MigrateUnaryVKey>();
    }

    fn test_vkey<VKey: VerificationKey>() {
        let pvk = VKey::ark_pvk();

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

        // Guarantees, that all permutations of input preparation rounds are < u16::MAX
        assert!(VKey::PREPARE_PUBLIC_INPUTS_ROUNDS < u16::MAX as usize);
    }
}