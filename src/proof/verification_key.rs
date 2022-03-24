use ark_bn254::{ Fq2, Fq12, G1Affine, G2Affine, G1Projective };
use crate::proof::{
    PREPARE_INPUTS_BASE_ITERATIONS,
    MILLER_LOOP_ITERATIONS,
    FINAL_EXPONENTIATION_ITERATIONS,
};

pub trait VerificationKey {
    const PUBLIC_INPUTS_COUNT: usize;

    const PREPARE_INPUTS_ITERATIONS: usize = Self::PUBLIC_INPUTS_COUNT * PREPARE_INPUTS_BASE_ITERATIONS;
    const FULL_ITERATIONS: usize = Self::PREPARE_INPUTS_ITERATIONS + MILLER_LOOP_ITERATIONS + FINAL_EXPONENTIATION_ITERATIONS;

    fn gamma_abc_g1_0() -> G1Projective;
    fn gamma_abc_g1() -> Vec<G1Affine>;
    fn alpha_g1_beta_g2() -> Fq12;
    fn gamma_g2_neg_pc(i: usize) -> (Fq2, Fq2, Fq2);
    fn delta_g2_neg_pc(i: usize) -> (Fq2, Fq2, Fq2);
    fn alpha_g1() -> G1Affine;
    fn beta_g2() -> G2Affine;
    fn gamma_g2() -> G2Affine;
    fn delta_g2() -> G2Affine;

    fn prepapre_inputs_rounds() -> Vec<usize> {
        let mut rounds = vec![3];
        for i in 0..Self::PREPARE_INPUTS_ITERATIONS {
            rounds.push(5);
        }
        rounds.push(1);
        rounds
    }
}
