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

    const PREPARE_PUBLIC_INPUTS_ROUNDS: usize = Self::PUBLIC_INPUTS_COUNT * 254;
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