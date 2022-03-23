use ark_bn254::{ Fq2, Fq12, G1Affine, G2Affine, G1Projective };
pub trait VerificationKey {
    const PUBLIC_INPUTS_COUNT: usize;
    const ID: usize;

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