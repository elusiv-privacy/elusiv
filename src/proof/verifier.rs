//! Groth16 proof verification
//! Since these computations are computationally very expensive, we use `elusiv_computations` macros to generate partial-computation-functions.
//! Calling those functions `n` times (over the span of multiple transactions) results in a finished computation.

use elusiv_interpreter::elusiv_computations;
use elusiv_computation::{PartialComputation, compute_unit_instructions};
use std::ops::Neg;
use std::cmp::min;
use ark_ec::ProjectiveCurve;
use ark_bn254::{Fq, Fq2, Fq6, Fq12, Fq12Parameters, G1Affine, G2Affine, Fq6Parameters, Parameters, G1Projective};
use ark_ff::fields::models::{ QuadExtParameters, fp12_2over3over2::Fp12ParamsWrapper, fp6_3over2::Fp6ParamsWrapper};
use ark_ff::{Field, CubicExtParameters, One, Zero, biginteger::BigInteger256, field_new};
use ark_ec::models::bn::BnParameters;
use crate::error::ElusivError::{ComputationIsAlreadyFinished, PartialComputationError};
use crate::fields::G2HomProjective;
use super::*;

pub fn verify_partial<VKey: VerificationKey>(
    round: usize,
    rounds: usize,
    verifier_account: &mut VerificationAccount,
) -> Result<Option<bool>, ElusivError> {
    // Public input preparation
    if round < VKey::PREPARE_PUBLIC_INPUTS_ROUNDS {
        let max_rounds = min(rounds, VKey::PREPARE_PUBLIC_INPUTS_ROUNDS);

        match prepare_public_inputs_partial::<VKey>(round, max_rounds, verifier_account) {
            None => {}//guard!(round != VKey::PREPARE_PUBLIC_INPUTS_ROUNDS - 1, CouldNotProcessProof),
            Some(prepared_inputs) => {
                verifier_account.prepared_inputs.set(&G1A(prepared_inputs));
                let b = verifier_account.b.get().0;
                verifier_account.r.set(&G2HomProjective { x: b.x, y: b.y, z: Fq2::one() });
            }
        }
    }

    // Combined miller loop
    else if round < VKey::COMBINED_MILLER_LOOP_ROUNDS {
        let mut r = verifier_account.r.get();
        let a = verifier_account.a.get().0;
        let b = verifier_account.b.get().0;
        let c = verifier_account.c.get().0;
        let prepared_inputs = verifier_account.prepared_inputs.get().0;

        let upper_bound = min(round + rounds, VKey::COMBINED_MILLER_LOOP_ROUNDS) - VKey::PREPARE_PUBLIC_INPUTS_ROUNDS;
        let round = round - VKey::PREPARE_PUBLIC_INPUTS_ROUNDS;
        for round in round..upper_bound {
            match combined_miller_loop_partial::<VKey>(round, verifier_account, &a, &b, &c, &prepared_inputs, &mut r)? {
                None => {}//guard!(round != VKey::COMBINED_MILLER_LOOP_ROUNDS - 1, CouldNotProcessProof),
                Some(f) => {
                    // Add `f` for the final exponentiation
                    verifier_account.f.set(&Wrap(f));
                }
            }
        }

        verifier_account.r.set(&r);
    }
    
    // Final exponentiation
    else if round < VKey::FINAL_EXPONENTIATION_ROUNDS {
        let f = verifier_account.f.get().0;

        let upper_bound = min(round + rounds, VKey::FINAL_EXPONENTIATION_ROUNDS) - VKey::COMBINED_MILLER_LOOP_ROUNDS;
        let round = round - VKey::COMBINED_MILLER_LOOP_ROUNDS;
        for round in round..upper_bound {
            match final_exponentiation_partial(round, verifier_account, &f)? {
                None => {}//guard!(round != VKey::FINAL_EXPONENTIATION_ROUNDS - 1, CouldNotProcessProof),
                Some(v) => {
                    // Final verification, we check:
                    // https://github.com/zkcrypto/bellman/blob/9bb30a7bd261f2aa62840b80ed6750c622bebec3/src/groth16/verifier.rs#L43
                    // https://github.com/arkworks-rs/groth16/blob/765817f77a6e14964c6f264d565b18676b11bd59/src/verifier.rs#L60
                    return Ok(Some(VKey::alpha_g1_beta_g2() == v))
                }
            }
        }
    }

    // Too many rounds
    else if round >= VKey::FINAL_EXPONENTIATION_ROUNDS {
        return Err(ComputationIsAlreadyFinished)
    }

    Ok(None)
}

macro_rules! read_g1_p{
    ($ram: expr, $o: literal) => { G1Projective::new($ram.read($o), $ram.read($o + 1), $ram.read($o + 2)) };
}

pub const PREPARE_PUBLIC_INPUTS_ROUNDS: usize = 257;

/// Public input preparation
/// - reference implementation: https://github.com/arkworks-rs/groth16/blob/765817f77a6e14964c6f264d565b18676b11bd59/src/verifier.rs#L22
/// - N public inputs (elements of the scalar field)
/// - the total rounds required for preparation of all inputs is `PREPARE_PUBLIC_INPUTS_ROUNDS` * N
/// - this partial computation is different from the rest, in that it's cost is dependent on the public inputs count and bits
fn prepare_public_inputs_partial<VKey: VerificationKey>(
    round: usize,
    rounds: usize,
    storage: &mut VerificationAccount,
) -> Option<G1Affine> {
    let mut acc: G1Projective = read_g1_p!(storage.ram_fq, 3); // (CUs: max: 813, min: 181, avg: 193) -> sum: 345856

    let mut input_index = round / PREPARE_PUBLIC_INPUTS_ROUNDS;
    let mut public_input = storage.get_public_input(input_index).0;
    let mut first_non_zero = find_first_non_zero(&public_input);
    let mut gamma_abc_g1 = VKey::gamma_abc_g1(input_index + 1); // mixed addition is faster than pure projective

    for round in round..round + rounds {
        let round = round % PREPARE_PUBLIC_INPUTS_ROUNDS;
        if round == 0 { acc = G1Projective::zero(); }

        if round < PREPARE_PUBLIC_INPUTS_ROUNDS - 1 { // Standard ec scalar multiplication
            if round < first_non_zero { continue }
    
            // Multiplication core
            acc.double_in_place(); // (CUs: max: 12642, min: 123, avg: 12281)
            if get_bit(&public_input, round) {
                acc.add_assign_mixed(&gamma_abc_g1); // (CUs: max: 20836, min: 211, avg: 19912)
            }
        } else { // Adding
            let g_ic = acc + if input_index == 0 { VKey::gamma_abc_g1_0() } else { read_g1_p!(storage.ram_fq, 0) };

            if input_index < VKey::PUBLIC_INPUTS_COUNT - 1 {
                write_g1_projective(&mut storage.ram_fq, &g_ic, 0);

                input_index += 1;
                public_input = storage.get_public_input(input_index).0;
                first_non_zero = find_first_non_zero(&public_input);
                gamma_abc_g1 = VKey::gamma_abc_g1(input_index + 1);
            } else {
                return Some(g_ic.into_affine())
            }
        }
    }

    write_g1_projective(&mut storage.ram_fq, &acc, 3);  // (CUs: max: 150, min: 150, avg: 150) -> in sum: 268800

    None
}

const DOUBLE_IN_PLACE_COST: u32 = 12_000;
const ADD_ASSIGN_MIXED_COST: u32 = 20_000;

/// Returns the instructions (and their rounds) required for a specific public-input bound input preparation
pub fn prepare_public_inputs_instructions<VKey: VerificationKey>(public_inputs: &[BigInteger256]) -> Vec<u32> {
    let mut rounds = Vec::new();

    for i in 0..VKey::PUBLIC_INPUTS_COUNT {
        let skip = find_first_non_zero(&public_inputs[i]);
        for b in skip..256 {
            if get_bit(&public_inputs[i], b) {
                rounds.push(DOUBLE_IN_PLACE_COST + ADD_ASSIGN_MIXED_COST);
            } else {
                rounds.push(DOUBLE_IN_PLACE_COST);
            }
        }
    }

    compute_unit_instructions(rounds)
}

fn write_g1_projective(ram: &mut RAMFq, g1p: &G1Projective, offset: usize) {
    ram.write(g1p.x, offset);
    ram.write(g1p.y, offset + 1);
    ram.write(g1p.z, offset + 2);
}

/// Returns the bit, indexed in bit-endian from `bytes_le` in little-endian format
fn get_bit(repr_num: &BigInteger256, bit: usize) -> bool {
    let limb = bit / 64;
    let local_bit = bit % 64;
    let bytes = u64::to_be_bytes(repr_num.0[3 - limb]);
    (bytes[local_bit / 8] >> (7 - (local_bit % 8))) & 1 == 1
}

/// Returns the first non-zero bit in big-endian for a value `bytes_le` in little-endian
fn find_first_non_zero(repr_num: &BigInteger256) -> usize {
    for limb in 0..4 {
        let bytes = u64::to_be_bytes(repr_num.0[3 - limb]);
        for byte in 0..8 {
            for bit in 0..8 {
                if (bytes[byte] >> (7 - bit)) & 1 == 1 {
                    return limb * 64 + byte * 8 + bit;
                }
            }
        }
    }
    256
}

/// Inverse of 2 (in q)
/// - Calculated using: Fq::one().double().inverse().unwrap()
const TWO_INV: Fq = Fq::new(BigInteger256::new([9781510331150239090, 15059239858463337189, 10331104244869713732, 2249375503248834476]));

const_assert_eq!(ADDITION_STEP_ROUNDS_COUNT, 2);
const_assert_eq!(DOUBLING_STEP_ROUNDS_COUNT, 3);
const_assert_eq!(MUL_BY_CHARACTERISTICS_ROUNDS_COUNT, 2);
const_assert_eq!(MUL_BY_034_ROUNDS_COUNT, 4);
const_assert_eq!(COMBINED_ELL_ROUNDS_COUNT, 16);

// TX count assertions
const_assert_eq!(CombinedMillerLoop::INSTRUCTIONS.len(), 33);
const_assert_eq!(FinalExponentiation::INSTRUCTIONS.len(), 16);

elusiv_computations!(
    combined_miller_loop, CombinedMillerLoop,

    // Doubling step
    // https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/g2.rs#L139
    doubling_step(storage: &mut VerificationAccount, r: &mut G2HomProjective) -> Coefficients {
        {   /// 40_000
            let mut a: Fq2 = r.x * r.y;
            a = mul_by_fp(&a, TWO_INV);
            let b: Fq2 = r.y.square();
            let c: Fq2 = r.z.square();
            let e: Fq2 = COEFF_B * (c.double() + c);
            let f: Fq2 = e.double() + e;
            let mut g: Fq2 = b + f;
            g = mul_by_fp(&g, TWO_INV);
            let h0: Fq2 = r.y + r.z;
            let h: Fq2 = h0.square() - (b + c);
            let e_square: Fq2 = e.square();
        }
        {   /// 18_000
            r.x = a * (b - f);
            r.y = g.square() - (e_square.double() + e_square);
            r.z = b * h;
        }
        {   /// 5_000
            let i: Fq2 = e - b;
            let j: Fq2 = r.x.square();
            return new_coeffs(h.neg(), j.double() + j, i);
        }
    },

    // Addition step
    // https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/g2.rs#L168
    addition_step(storage: &mut VerificationAccount, r: &mut G2HomProjective, q: &G2Affine) -> Coefficients {
        {   /// 40_000
            let theta: Fq2 = r.y - (q.y * r.z);
            let lambda: Fq2 = r.x - (q.x * r.z);
            let c: Fq2 = theta.square();
            let d: Fq2 = lambda.square();
            let e: Fq2 = lambda * d;
            let f: Fq2 = r.z * c;
            let g: Fq2 = r.x * d;
        }
        {   /// 40_000
            let h: Fq2 = e + f - g.double();
            let rx: Fq2 = lambda * h;
            let ry: Fq2 = theta * (g - h) - (e * r.y);
            let rz: Fq2 = r.z * e;
            let j: Fq2 = theta * q.x - (lambda * q.y);

            r.x = rx;
            r.y = ry;
            r.z = rz;

            return new_coeffs(lambda, theta.neg(), j);
        }
    },

    // Mul by characteristics
    // https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/g2.rs#L127
    mul_by_characteristics(storage: &mut VerificationAccount, r: &G2Affine) -> G2Affine {
        {   /// 9_000
            let mut x: Fq2 = frobenius_map_fq2_one(r.x);
            x = x * TWIST_MUL_BY_Q_X;
        }
        {   /// 9_000
            let mut y: Fq2 = frobenius_map_fq2_one(r.y);
            y = y * TWIST_MUL_BY_Q_Y;
            return G2Affine::new(x, y, r.infinity);
        }
    },

    // f.mul_by_034(c0, c1, coeffs.2); (with: self -> f; c0 -> c0; d0 -> c1; d1 -> coeffs.2)
    // https://github.com/arkworks-rs/r1cs-std/blob/b7874406ec614748608b1739b1578092a8c97fb8/src/fields/fp12.rs#L43
    mul_by_034(
        storage: &mut VerificationAccount,
        c0: &Fq2, d0: &Fq2, d1: &Fq2, f: Fq12
    ) -> Fq12 {
        {   /// 19_000
            let a: Fq6 = Fq6::new(f.c0.c0 * c0, f.c0.c1 * c0, f.c0.c2 * c0);
        }

        {   /// 40_000
            let b: Fq6 = mul_fq6_by_c0_c1_0(f.c1, d0, d1);
        }

        {   /// 40_000
            let e: Fq6 = mul_fq6_by_c0_c1_0(f.c0 + f.c1, &(*c0 + d0), d1);
        }

        {   /// 1_200
            return Fq12::new(mul_base_field_by_nonresidue(b) + a, e - (a + b));
        }
    },

    // We evaluate the line function for A, the prepared inputs and C
    // - inside the miller loop we do evaluations on three elements
    // - multi_ell combines those three calls in one function
    // - normal ell implementation: https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/mod.rs#L59
    combined_ell{<VKey: VerificationKey>}(
        storage: &mut VerificationAccount,
        a: &G1Affine, prepared_inputs: &G1Affine, c: &G1Affine, c0: &Fq2, c1: &Fq2, c2: &Fq2, coeff_index: usize, f: Fq12,
    ) -> Fq12 {
        // ell on A with c0, c1, c2
        {   /// 7_900
            let r: Fq12 = f;

            let a0: Fq2 = mul_by_fp(c0, a.y);
            let a1: Fq2 = mul_by_fp(c1, a.x);
        }
        {   /// mul_by_034
            if (!(a.is_zero())) {
                partial v = mul_by_034(storage, &a0, &a1, c2, r) { r = v }
            }
        }

        // ell on prepared_inputs with gamma_g2_neg_pc
        {   /// 7_900
            let b0: Fq2 = mul_by_fp(&(VKey::gamma_g2_neg_pc_0(coeff_index)), prepared_inputs.y);
            let b1: Fq2 = mul_by_fp(&(VKey::gamma_g2_neg_pc_1(coeff_index)), prepared_inputs.x);
        }
        {   /// mul_by_034
            if (!(prepared_inputs.is_zero())) {
                partial v = mul_by_034(storage, &b0, &b1, &(VKey::gamma_g2_neg_pc_2(coeff_index)), r) { r = v }
            }
        }

        // ell on C with delta_g2_neg_pc
        {   /// 7_900
            let d0: Fq2 = mul_by_fp(&(VKey::delta_g2_neg_pc_0(coeff_index)), c.y);
            let d1: Fq2 = mul_by_fp(&(VKey::delta_g2_neg_pc_1(coeff_index)), c.x);
        }
        {   /// mul_by_034
            if (!(c.is_zero())) {
                partial v = mul_by_034(storage, &d0, &d1, &(VKey::delta_g2_neg_pc_2(coeff_index)), r) { r = v }
            }
        }

        {   /// 0
            return r;
        }
    },

    // We combine the miller loop and the coefficient generation for B
    // - miller loop ref: https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/mod.rs#L99
    // - coefficient generation ref: https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/g2.rs#L68
    // - implementation:
    // - the miller loop receives an iterator over 3 elements (https://github.com/arkworks-rs/groth16/blob/765817f77a6e14964c6f264d565b18676b11bd59/src/verifier.rs#L41)
    // - for B we need to generate the coeffs (all other coeffs already are generated befor compilation)
    // - so we have a var r = (x: rbx, y: rby, z: rbz)
    combined_miller_loop{<VKey: VerificationKey>}(
        storage: &mut VerificationAccount,
        a: &G1Affine, b: &G2Affine, c: &G1Affine, prepared_inputs: &G1Affine, r: &mut G2HomProjective,
    ) -> Fq12 {
        {   /// 500
            r.x = b.x;
            r.x = b.y;
            r.x = Fq2::one();

            let f: Fq12 = Fq12::one();

            // values for B coeffs generation (https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/g2.rs#L79)
            let alt_b: G2A = G2A(b.neg());
            let c0: Fq2 = Fq2::zero();
            let c1: Fq2 = Fq2::zero();
            let c2: Fq2 = Fq2::zero();
        }

        // Reversed ATE_LOOP_COUNT with the the last element removed (so the first in the reversed order)
        // https://github.com/arkworks-rs/curves/blob/1551d6d76ce5abf6e7925e53b0ea1af7dbc421c3/bn254/src/curves/mod.rs#L21
        {
            for i, ate_loop_count in [1,1,0,1,0,0,2,0,1,1,0,0,0,2,0,0,1,1,0,0,2,0,0,0,0,0,1,0,0,2,0,0,1,1,1,0,0,0,0,2,0,1,0,0,2,0,1,1,0,0,1,0,0,2,1,0,0,2,0,1,0,1,0,0,0] {
                {   /// i in { 0 : 0 , _ : 84_673 }
                    if (i > 0) {
                        f = f.square();
                    }
                }

                partial v = doubling_step(storage, r) { c0=v.0; c1=v.1; c2=v.2; };
                partial v = combined_ell::<VKey>(storage, a, prepared_inputs, c, &c0, &c1, &c2, i, f) { f = v; };

                {   /// ate_loop_count in { 0 : addition_step_zero , _ : addition_step }
                    if (ate_loop_count > 0) {
                        if (ate_loop_count = 1) {
                            partial v = addition_step(storage, r, b) { c0=v.0; c1=v.1; c2=v.2; };
                        } else {
                            partial v = addition_step(storage, r, &(alt_b.0)) { c0=v.0; c1=v.1; c2=v.2; };
                        }
                    }
                }
                {   /// ate_loop_count in { 0 : combined_ell_zero , _ : combined_ell }
                    if (ate_loop_count > 0) {
                        partial v = combined_ell::<VKey>(storage, a, prepared_inputs, c, &c0, &c1, &c2, i, f) { f = v; };
                    }    
                }
            }
        }
        // The final two coefficient triples
        {
            partial v = mul_by_characteristics(storage, b) { alt_b = G2A(v); };
            partial v = addition_step(storage, r, &(alt_b.0)) { c0=v.0; c1=v.1; c2=v.2; };
            partial v = combined_ell::<VKey>(storage, a, prepared_inputs, c, &c0, &c1, &c2, 0, f) {
                if (!(prepared_inputs.is_zero())) { f = v; }
            };
            partial v = mul_by_characteristics(storage, &(alt_b.0)) { alt_b = G2A(v); };
        }
        {   /// 0
            alt_b = G2A(G2Affine::new(alt_b.0.x, alt_b.0.y.neg(), alt_b.0.infinity));
        }
        {
            partial v = addition_step(storage, r, &(alt_b.0)) { c0=v.0; c1=v.1; c2=v.2; };
            partial v = combined_ell::<VKey>(storage, a, prepared_inputs, c, &c0, &c1, &c2, 0, f) {
                if (!(prepared_inputs.is_zero())) { f = v; }
            };
        }
        {   /// 0
            return f;
        }
    }
);

const_assert_eq!(INVERSE_FQ12_ROUNDS_COUNT, 5);
const_assert_eq!(EXP_BY_NEG_X_ROUNDS_COUNT, 128);

elusiv_computations!(
    final_exponentiation, FinalExponentiation,

    // https://github.com/arkworks-rs/algebra/blob/4dd6c3446e8ab22a2ba13505a645ea7b3a69f493/ff/src/fields/models/quadratic_extension.rs#L366
    // Guide to Pairing-based Cryptography, Algorithm 5.19.
    inverse_fq12(storage: &mut VerificationAccount, f: Fq12) -> Fq12 {
        {   /// 28_000
            let v1: Fq6 = f.c1.square();
        }
        {   /// 28_000
            let v2: Fq6 = f.c0.square();
        }
        {   /// 800
            let mut v0: Fq6 = sub_and_mul_base_field_by_nonresidue(v2, v1);
        }
        {   /// 147_000
            let v3: Fq6 = unwrap v0.inverse();
        }
        {   /// 85_000
            let v: Fq6 = f.c1 * v3;
            return Fq12::new(f.c0 * v3, v.neg());
        }
    },

    // Using exp_by_neg_x and cyclotomic_exp
    // https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/mod.rs#L78
    // https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ff/src/fields/models/fp12_2over3over2.rs#L56
    exp_by_neg_x(storage: &mut VerificationAccount, fe: Fq12) -> Fq12 {
        {   /// 1_500
            let fe_inverse: Fq12 = conjugate(fe);
            let res: Fq12 = Fq12::one();
        }

        // Non-adjacent window form of exponent Parameters::X (u64: 4965661367192848881)
        // NAF computed using: https://citeseerx.ist.psu.edu/viewdoc/download?doi=10.1.1.394.3037&rep=rep1&type=pdf Page 98
        // - but removed the last zero value, since it has no effect
        // - and then inverted the array
        {
            for i, value in [1,0,0,0,1,0,1,0,0,2,0,1,0,1,0,2,0,0,1,0,1,0,2,0,2,0,2,0,1,0,0,0,1,0,0,1,0,1,0,1,0,2,0,1,0,0,1,0,0,0,0,1,0,1,0,0,0,0,2,0,0,0,1] {
                {   /// i in { 0 : 0 , _ : 47_000 }
                    if (i > 0) {
                        res = res.cyclotomic_square();
                    }
                }

                {   /// value in { 0 : 0 , _ : 129_000 }
                    if (value > 0) {
                        if (value = 1) {
                            res = res * fe;
                        } else { // value == 2
                            res = res * fe_inverse;
                        }
                    }
                }
            }
        }
        {   /// 1_500
            return conjugate(res);
        }
    },

    // Final exponentiation
    // - reference implementation: https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/mod.rs#L153
    final_exponentiation(storage: &mut VerificationAccount, f: &Fq12) -> Fq12 {
        {   /// 1_500
            let r: Fq12 = conjugate(*f);
            let f2: Fq12 = *f;
        }
        {
            partial v = inverse_fq12(storage, f2) {
                r = r * v;
                f2 = r;
            }
        }
        {   /// 181_000
            r = frobenius_map(r, 2);
            r = r * f2;
            let y0: Fq12 = r;
        }
        {
            partial v = exp_by_neg_x(storage, y0) { y0 = v; }
        }
        {   /// 220_000
            let y1: Fq12 = y0.cyclotomic_square();
            let y2: Fq12 = y1.cyclotomic_square();
            let y3: Fq12 = y2 * y1;
            let y4: Fq12 = y3;
        }
        {
            partial v = exp_by_neg_x(storage, y4) { y4 = v; }
        }
        {   /// 45_000
            let y5: Fq12 = y4.cyclotomic_square();
            let y6: Fq12 = y5;
        }
        {
            partial v = exp_by_neg_x(storage, y6) { y6 = v; }
        }
        {   /// 2_000
            y3 = conjugate(y3);
            y6 = conjugate(y6);
        }
        {   /// 126_000
            let y7: Fq12 = y6 * y4;
        }
        {   /// 126_000
            let y8: Fq12 = y7 * y3;
        }
        {   /// 126_000
            let y9: Fq12 = y8 * y1;
        }
        {   /// 126_000
            let y10: Fq12 = y8 * y4;
        }
        {   /// 126_000
            let y11: Fq12 = y10 * r;
        }
        {   /// 55_000
            let mut y12: Fq12 = y9;
            y12 = frobenius_map(y12, 1);
        }
        {   /// 126_000
            let y13: Fq12 = y12 * y11;
        }
        {   /// 55_000
            y8 = frobenius_map(y8, 2);
        }
        {   /// 127_000
            let y14: Fq12 = y8 * y13;
            r = conjugate(r);
        }
        {   /// 126_000
            let y15: Fq12 = r * y9;
        }
        {   /// 55_000
            y15 = frobenius_map(y15, 3);
        }
        {   /// 126_000
            return y15 * y14;
        }
    }
);

/// https://docs.rs/ark-bn254/0.3.0/src/ark_bn254/curves/g2.rs.html#19
/// COEFF_B = 3/(u+9) = (19485874751759354771024239261021720505790618469301721065564631296452457478373, 266929791119991161246907387137283842545076965332900288569378510910307636690)
const COEFF_B: Fq2 = field_new!(Fq2,
    field_new!(Fq, "19485874751759354771024239261021720505790618469301721065564631296452457478373"),
    field_new!(Fq, "266929791119991161246907387137283842545076965332900288569378510910307636690"),
);

type Coefficients = (Fq2, Fq2, Fq2);
fn new_coeffs(c0: Fq2, c1: Fq2, c2: Fq2) -> Coefficients { (c0, c1, c2) }

const TWIST_MUL_BY_Q_X: Fq2 = Parameters::TWIST_MUL_BY_Q_X;
const TWIST_MUL_BY_Q_Y: Fq2 = Parameters::TWIST_MUL_BY_Q_Y;

fn frobenius_map_fq2_one(f: Fq2) -> Fq2 {
    let mut k = f.clone();
    k.frobenius_map(1);
    k
}

// https://github.com/arkworks-rs/algebra/blob/4dd6c3446e8ab22a2ba13505a645ea7b3a69f493/ff/src/fields/models/quadratic_extension.rs#L87
// https://github.com/arkworks-rs/algebra/blob/4dd6c3446e8ab22a2ba13505a645ea7b3a69f493/ff/src/fields/models/quadratic_extension.rs#L56
fn sub_and_mul_base_field_by_nonresidue(x: Fq6, y: Fq6) -> Fq6 {
    x - mul_base_field_by_nonresidue(y)
}

fn mul_base_field_by_nonresidue(v: Fq6) -> Fq6 {
    Fp12ParamsWrapper::<Fq12Parameters>::mul_base_field_by_nonresidue(&v)
}

// https://github.com/arkworks-rs/r1cs-std/blob/b7874406ec614748608b1739b1578092a8c97fb8/src/fields/fp6_3over2.rs#L53
fn mul_fq6_by_c0_c1_0(f: Fq6, c0: &Fq2, c1: &Fq2) -> Fq6 {
    let v0: Fq2 = f.c0 * c0;
    let v1: Fq2 = f.c1 * c1;

    let a1_plus_a2: Fq2 = f.c1 + f.c2;
    let a0_plus_a1: Fq2 = f.c0 + f.c1;
    let a0_plus_a2: Fq2 = f.c0 + f.c2;

    let b1_plus_b2: Fq2 = c1.clone();
    let b0_plus_b1: Fq2 = *c0 + c1;
    let b0_plus_b2: Fq2 = c0.clone();

    Fq6::new(
        (a1_plus_a2 * b1_plus_b2 - &v1) * Fp6ParamsWrapper::<Fq6Parameters>::NONRESIDUE + &v0,
        a0_plus_a1 * &b0_plus_b1 - &v0 - &v1,
        a0_plus_a2 * &b0_plus_b2 - &v0 + &v1,
    )
}

fn mul_by_fp(v: &Fq2, fp: Fq) -> Fq2 {
    let mut v: Fq2 = *v;
    v.mul_assign_by_fp(&fp);
    v
}

fn conjugate(f: Fq12) -> Fq12 {
    let mut k = f.clone();
    k.conjugate();
    k
}
fn frobenius_map(f: Fq12, u: usize) -> Fq12 {
    let mut k = f.clone();
    k.frobenius_map(u);
    k
}

#[cfg(test)]
mod tests {
    use crate::fields::fr_to_u256_le;
    use crate::{state::queue::SendProofRequest, types::SendPublicInputs};
    use crate::state::program_account::ProgramAccount;

    use super::*;
    use std::str::FromStr;
    use ark_bn254::{Fr, Bn254};
    use ark_ec::PairingEngine;
    use ark_ec::models::bn::BnParameters;
    use ark_ff::PrimeField;
    use ark_groth16::{VerifyingKey, prepare_inputs, prepare_verifying_key};

    type VK = super::super::vkey::SendBinaryVKey;

    fn f() -> Fq12 {
        let f = Fq6::new(
            Fq2::new(
                Fq::from_str("20925091368075991963132407952916453596237117852799702412141988931506241672722").unwrap(),
                Fq::from_str("18684276579894497974780190092329868933855710870485375969907530111657029892231").unwrap(),
            ),
            Fq2::new(
                Fq::from_str("5932690455294482368858352783906317764044134926538780366070347507990829997699").unwrap(),
                Fq::from_str("18684276579894497974780190092329868933855710870485375969907530111657029892231").unwrap(),
            ),
            Fq2::new(
                Fq::from_str("18684276579894497974780190092329868933855710870485375969907530111657029892231").unwrap(),
                Fq::from_str("19526707366532583397322534596786476145393586591811230548888354920504818678603").unwrap(),
            ),
        );
        Fq12::new(f, f)
    }

    fn g2_affine() -> G2Affine { G2Affine::new(f().c0.c0, f().c0.c1, false) }

    fn proof(ax: &str, ay: &str, ainf: bool, b0x: &str, b0y: &str, b1x: &str, b1y: &str, binf: bool, cx: &str, cy: &str, cinf: bool) -> Proof {
        Proof {
            a: G1A(G1Affine::new(Fq::from_str(ax).unwrap(), Fq::from_str(ay).unwrap(), ainf)),
            b: G2A(G2Affine::new(
                Fq2::new(Fq::from_str(b0x).unwrap(), Fq::from_str(b0y).unwrap()),
                Fq2::new(Fq::from_str(b1x).unwrap(), Fq::from_str(b1y).unwrap()),
                binf
            )),
            c: G1A(G1Affine::new(Fq::from_str(cx).unwrap(), Fq::from_str(cy).unwrap(), cinf)),
        }
    }

    fn send2_public_inputs(
        proof: Proof,
        nullifier_hashes: [&str; 2],
        roots: [&str; 2],
        commitment: &str,
        recipient: U256,
        amount: u64,
        timestamp: u64,
    ) -> SendProofRequest {
        SendProofRequest {
            proof_data: crate::types::JoinSplitProofData {
                proof: proof.try_to_vec().unwrap().try_into().unwrap(),
                tree_indices: [0, 0],
            },
            public_inputs: SendPublicInputs {
                join_split: crate::types::JoinSplitPublicInputs {
                    nullifier_hashes: [
                        fr_to_u256_le(&Fr::from_str(nullifier_hashes[0]).unwrap()),
                        fr_to_u256_le(&Fr::from_str(nullifier_hashes[1]).unwrap()),
                    ],
                    roots: [
                        fr_to_u256_le(&Fr::from_str(roots[0]).unwrap()),
                        fr_to_u256_le(&Fr::from_str(roots[1]).unwrap()),
                    ],
                    commitment: fr_to_u256_le(&Fr::from_str(commitment).unwrap()),
                },
                recipient,
                amount,
                timestamp,
            },
            fee_payer: [0; 32],
        }
    }

    macro_rules! storage {
        ($id: ident) => {
            let mut data = vec![0; VerificationAccount::SIZE];
            let mut $id = VerificationAccount::new(&mut data).unwrap();
        };
    }

    fn verify_full<VKey: VerificationKey>(verifier_account: &mut VerificationAccount) -> Option<bool> {
        verify_partial::<VK>(
            0,
            VK::PREPARE_PUBLIC_INPUTS_ROUNDS,
            verifier_account
        ).unwrap();

        verify_partial::<VK>(
            VK::PREPARE_PUBLIC_INPUTS_ROUNDS,
            VK::COMBINED_MILLER_LOOP_ROUNDS - VK::PREPARE_PUBLIC_INPUTS_ROUNDS,
            verifier_account
        ).unwrap();

        verify_partial::<VK>(
            VK::COMBINED_MILLER_LOOP_ROUNDS,
            VK::FINAL_EXPONENTIATION_ROUNDS - VK::COMBINED_MILLER_LOOP_ROUNDS,
            verifier_account
        ).unwrap()
    }

    #[test]
    fn test_verify_partial() {
        storage!(verifier_account);

        // Invalid proof
        let proof = proof(
            "10026859857882131638516328056627849627085232677511724829502598764489185541935",
            "19685960310506634721912121951341598678325833230508240750559904196809564625591",
            false,
            "857882131638516328056627849627085232677511724829502598764489185541935",
            "685960310506634721912121951341598678325833230508240750559904196809564625591",
            "837064132573119120838379738103457054645361649757131991036638108422638197362",
            "86803555845400161937398579081414146527572885637089779856221229551142844794",
            false,
            "21186803555845400161937398579081414146527572885637089779856221229551142844794",
            "85960310506634721912121951341598678325833230508240750559904196809564625591",
            false,
        );
        let request = ProofRequest::Send {
            request: send2_public_inputs(
                proof,
                [
                    "1937398579081414146527572885637089779856221229551142844794",
                    "16193739857908141146527572885637089779856221229551142844794"
                ],
                [
                    "937398579081414146527572885637089779856221229551142844794",
                    "3985791414146527572885637089779856221229551142844794"
                ],
                "4001619373985790814141465275728856370897",
                [0; 32],
                100000,
                12345678,
            )
        };
        verifier_account.reset::<VK>(request).unwrap();

        let result = verify_full::<VK>(&mut verifier_account);
        assert_eq!(result.unwrap(), false);
    }

    #[test]
    fn test_prepare_public_inputs() {
        let public_inputs = vec![
            Fr::from_str("5932690455294482368858352783906317764044134926538780366070347507990829997699").unwrap(),
            Fr::from_str("18684276579894497974780190092329868933855710870485375969907530111657029892231").unwrap(),
            Fr::from_str("19526707366532583397322534596786476145393586591811230548888354920504818678603").unwrap(),
            Fr::from_str("20925091368075991963132407952916453596237117852799702412141988931506241672722").unwrap(),
            Fr::from_str("3932690455294482368858352783906317764044134926538780366070347507990829997699").unwrap(),
            Fr::from_str("932690455294482368858352783906317764044134926538780366070347507990829997699").unwrap(),
            Fr::from_str("455294482368858352783906317764044134926538780366070347507990829997699").unwrap(),
        ];
        storage!(storage);
        for (i, public_input) in public_inputs.iter().enumerate() {
            storage.set_public_input(i, &Wrap(public_input.into_repr()));
        }

        let value = prepare_public_inputs_partial::<VK>(0, VK::PREPARE_PUBLIC_INPUTS_ROUNDS, &mut storage).unwrap();

        let mut gamma_abc_g1 = Vec::new();
        for i in 0..=VK::PUBLIC_INPUTS_COUNT {
            gamma_abc_g1.push(VK::gamma_abc_g1(i));
        }

        let vk = VerifyingKey::<Bn254> {
            alpha_g1: VK::alpha_g1(),
            beta_g2: VK::beta_g2(),
            gamma_g2: VK::gamma_g2(),
            delta_g2: VK::delta_g2(),
            gamma_abc_g1,
        };
        let pvk = prepare_verifying_key(&vk);
        let expected = prepare_inputs(&pvk, &public_inputs).unwrap().into_affine();

        assert_eq!(value, expected);
    }

    #[test]
    fn test_find_first_non_zero() {
        assert_eq!(find_first_non_zero(&BigInteger256::from(1)), 255);
    }

    #[test]
    fn test_get_bit() {
        assert_eq!(get_bit(&BigInteger256::from(1), 255), true);
    }

    #[test]
    fn test_mul_by_characteristics() {
        storage!(storage);
        let mut value: Option<G2Affine> = None;
        for round in 0..MUL_BY_CHARACTERISTICS_ROUNDS_COUNT {
            value = mul_by_characteristics_partial(round, &mut storage, &g2_affine()).unwrap();
        }

        assert_eq!(value.unwrap(), reference_mul_by_char(g2_affine()));
    }

    #[test]
    fn test_combined_ell() {
        storage!(storage);
        let mut value: Option<Fq12> = None;
        let a = G1Affine::new(
            Fq::from_str("10026859857882131638516328056627849627085232677511724829502598764489185541935").unwrap(),
            Fq::from_str("19685960310506634721912121951341598678325833230508240750559904196809564625591").unwrap(),
            false
        );
        let prepared_inputs = G1Affine::new(
            Fq::from_str("6859857882131638516328056627849627085232677511724829502598764489185541935").unwrap(),
            Fq::from_str("310506634721912121951341598678325833230508240750559904196809564625591").unwrap(),
            false
        );
        let c = G1Affine::new(
            Fq::from_str("21186803555845400161937398579081414146527572885637089779856221229551142844794").unwrap(),
            Fq::from_str("85960310506634721912121951341598678325833230508240750559904196809564625591").unwrap(),
            false
        );
        let c0 = f().c0.c0.clone();
        let c1 = f().c0.c1.clone();
        let c2 = f().c0.c2.clone();
        for round in 0..COMBINED_ELL_ROUNDS_COUNT {
            value = combined_ell_partial::<VK>(round, &mut storage, &a, &prepared_inputs, &c, &c0, &c1, &c2, 0, f()).unwrap();
        }

        let mut expected = f();
        expected = reference_ell(expected, (c0, c1, c2), a);
        expected = reference_ell(expected, (VK::gamma_g2_neg_pc_0(0), VK::gamma_g2_neg_pc_1(0), VK::gamma_g2_neg_pc_2(0)), prepared_inputs);
        expected = reference_ell(expected, (VK::delta_g2_neg_pc_0(0), VK::delta_g2_neg_pc_1(0), VK::delta_g2_neg_pc_2(0)), c);

        assert_eq!(expected, value.unwrap());
    }

    #[test]
    fn test_inverse_fq12() {
        storage!(storage);
        let mut value: Option<Fq12> = None;
        for round in 0..INVERSE_FQ12_ROUNDS_COUNT {
            value = inverse_fq12_partial(round, &mut storage, f()).unwrap();
        }

        assert_eq!(value.unwrap(), f().inverse().unwrap());
    }

    #[test]
    fn test_exp_by_neg_x() {
        storage!(storage);
        let mut value: Option<Fq12> = None;
        for round in 0..EXP_BY_NEG_X_ROUNDS_COUNT {
            value = exp_by_neg_x_partial(round, &mut storage, f()).unwrap();
        }

        assert_eq!(value.unwrap(), reference_exp_by_neg_x(f()));
    }

    #[test]
    fn test_final_exponentiation() {
        storage!(storage);
        let mut value = None;
        for round in 0..FINAL_EXPONENTIATION_ROUNDS_COUNT {
            value = final_exponentiation_partial(round, &mut storage, &f()).unwrap();
        }

        assert_eq!(value.unwrap(), Bn254::final_exponentiation(&f()).unwrap());
    }

    // https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/mod.rs#L59
    fn reference_ell(f: Fq12, coeffs: (Fq2, Fq2, Fq2), p: G1Affine) -> Fq12 {
        let mut c0: Fq2 = coeffs.0;
        let mut c1: Fq2 = coeffs.1;
        let c2: Fq2 = coeffs.2;
    
        c0.mul_assign_by_fp(&p.y);
        c1.mul_assign_by_fp(&p.x);

        let mut f = f;
        f.mul_by_034(&c0, &c1, &c2);
        f
    }

    // https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/g2.rs#L127
    fn reference_mul_by_char(r: G2Affine) -> G2Affine {
        let mut s = r;
        s.x.frobenius_map(1);
        s.x *= TWIST_MUL_BY_Q_X;
        s.y.frobenius_map(1);
        s.y *= TWIST_MUL_BY_Q_Y;
        s
    }

    // https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/mod.rs#L78
    fn reference_exp_by_neg_x(f: Fq12) -> Fq12 {
        let mut f = f.cyclotomic_exp(&Parameters::X);
        if !Parameters::X_IS_NEGATIVE { f.conjugate(); }
        f
    }
}