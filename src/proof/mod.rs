pub mod vkey;
mod verifier;
mod ram;

pub use verifier::*;
use ark_bn254::{ Fq2, Fq12, G1Affine, G2Affine, G1Projective };
use verifier::{ COMBINED_MILLER_LOOP_ROUNDS_COUNT, FINAL_EXPONENTIATION_ROUNDS_COUNT };

/// A Groth16 verification key
pub trait VerificationKey {
    const PUBLIC_INPUTS_COUNT: usize;

    const PREPARE_PUBLIC_INPUTS_ROUNDS: usize = Self::PUBLIC_INPUTS_COUNT * 254;
    const COMBINED_MILLER_LOOP_ROUNDS: usize = Self::PREPARE_PUBLIC_INPUTS_ROUNDS + COMBINED_MILLER_LOOP_ROUNDS_COUNT;
    const FINAL_EXPONENTIATION_ROUNDS: usize = Self::COMBINED_MILLER_LOOP_ROUNDS + FINAL_EXPONENTIATION_ROUNDS_COUNT;

    fn gamma_abc_g1_0() -> G1Projective;
    fn gamma_abc_g1(index: usize) -> Vec<G1Affine>;
    fn alpha_g1_beta_g2() -> Fq12;
    fn gamma_g2_neg_pc(coeff_index: usize, i: usize) -> &'static Fq2;
    fn delta_g2_neg_pc(coeff_index: usize, i: usize) -> &'static Fq2;
    fn alpha_g1() -> G1Affine;
    fn beta_g2() -> G2Affine;
    fn gamma_g2() -> G2Affine;
    fn delta_g2() -> G2Affine;
}