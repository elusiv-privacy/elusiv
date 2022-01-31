use super::{ Proof, gamma_abc_g1 };
use ark_bn254::{
    Parameters,
    G1Affine,
    Fr, Fq, Fq2, Fq12,
};
use ark_ec::{
    models::bn::g1::G1Prepared,
    models::bn::g2::G2Prepared,
    AffineCurve,
    ProjectiveCurve,
};
use super::super::scalar::*;
use ark_ff::*;
use core::ops::{ AddAssign };
use super::super::state::ProofVerificationAccount;
use super::super::storage_account::set;

pub fn prepare_proof(proof: Proof) -> (G1Affine, G2Prepared<Parameters>, G1Affine) {
    (proof.a, proof.b.into(), proof.c)
}

pub fn partial_verification(iteration: usize) {

}

pub fn final_verification() -> bool {
    false
}