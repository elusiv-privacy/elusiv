use ark_bn254::{ Fq2, Fq12, G1Affine, G2Affine, G1Projective };
use crate::proof::{
    prepare_inputs_tx_count,
    MILLER_LOOP_ITERATIONS,
    FINAL_EXPONENTIATION_ITERATIONS,
};

pub trait VerificationKey {
    const PUBLIC_INPUTS_COUNT: usize;

    const PREPARE_INPUTS_ITERATIONS: usize = prepare_inputs_tx_count(Self::PUBLIC_INPUTS_COUNT);
    const FULL_ITERATIONS: usize = Self::PREPARE_INPUTS_ITERATIONS + MILLER_LOOP_ITERATIONS + FINAL_EXPONENTIATION_ITERATIONS;

    // Ranges
    const PREPARE_INPUTS: (usize, usize) = (
        0,
        Self::PREPARE_INPUTS_ITERATIONS
    );
    const MILLER_LOOP: (usize, usize) = (
        Self::PREPARE_INPUTS.1,
        Self::PREPARE_INPUTS.1 + MILLER_LOOP_ITERATIONS
    );
    const FINAL_EXPONENTIATION: (usize, usize) = (
        Self::MILLER_LOOP.1,
        Self::MILLER_LOOP.1 + FINAL_EXPONENTIATION_ITERATIONS
    );

    fn gamma_abc_g1_0() -> G1Projective;
    fn gamma_abc_g1() -> Vec<G1Affine>;
    fn alpha_g1_beta_g2() -> Fq12;
    fn gamma_g2_neg_pc(i: usize) -> (Fq2, Fq2, Fq2);
    fn delta_g2_neg_pc(i: usize) -> (Fq2, Fq2, Fq2);
    fn alpha_g1() -> G1Affine;
    fn beta_g2() -> G2Affine;
    fn gamma_g2() -> G2Affine;
    fn delta_g2() -> G2Affine;
}
