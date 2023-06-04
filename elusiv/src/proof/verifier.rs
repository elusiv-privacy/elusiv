//! Groth16 proof verification (https://eprint.iacr.org/2016/260.pdf)
//! Since these computations are computationally expensive, we use `elusiv_computations` macros to generate partial-computation-functions.
//! Calling those functions `n` times (over the span of multiple transactions) results in a finished computation.

#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::assign_op_pattern)]

use super::vkey::VerifyingKey;
use crate::bytes::{usize_as_u32_safe, usize_as_u8_safe};
use crate::error::ElusivError::{
    self, ComputationIsAlreadyFinished, CouldNotProcessProof, InvalidAccountState,
    PartialComputationError,
};
use crate::error::ElusivResult;
use crate::fields::{G2HomProjective, Wrap, G1A, G2A};
use crate::processor::COMPUTE_VERIFICATION_IX_COUNT;
use crate::state::proof::{RAMFq, VerificationAccount, VerificationState};
use crate::types::U256;
use ark_bn254::{
    Fq, Fq12, Fq12Parameters, Fq2, Fq6, Fq6Parameters, G1Affine, G1Projective, G2Affine, Parameters,
};
use ark_ec::models::bn::BnParameters;
use ark_ec::ProjectiveCurve;
use ark_ff::fields::models::{
    fp12_2over3over2::Fp12ParamsWrapper, fp6_3over2::Fp6ParamsWrapper, QuadExtParameters,
};
use ark_ff::{biginteger::BigInteger256, field_new, CubicExtParameters, Field, One, Zero};
use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_computation::{PartialComputation, RAM};
use elusiv_derive::BorshSerDeSized;
use elusiv_interpreter::elusiv_computations;
use elusiv_utils::guard;
use std::ops::{AddAssign, Neg};

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized, Clone)]
#[cfg_attr(any(test, feature = "elusiv-client"), derive(Debug, PartialEq))]
pub enum VerificationStep {
    PublicInputPreparation,
    CombinedMillerLoop,
    FinalExponentiation,
}

/// Requires `verification_account.prepare_inputs_instructions_count + COMBINED_MILLER_LOOP_IXS + FINAL_EXPONENTIATION_IXS` calls to verify a valid proof
pub fn verify_partial(
    verification_account: &mut VerificationAccount,
    vkey: &VerifyingKey,
    instruction_index: u16,
) -> Result<Option<bool>, ElusivError> {
    let instruction = verification_account.get_instruction() as usize;
    let round = verification_account.get_round() as usize;
    let step = verification_account.get_step();

    match step {
        VerificationStep::PublicInputPreparation => {
            // This enables us to use a uniform number of ixs per tx (by only allowing the last ix to perform the computation)
            if instruction_index != COMPUTE_VERIFICATION_IX_COUNT - 1 {
                return Ok(None);
            }

            prepare_public_inputs(verification_account, vkey, instruction, round)?;
            verification_account.serialize_rams().unwrap();
        }
        VerificationStep::CombinedMillerLoop => {
            // Proof first has to be setup
            guard!(
                verification_account.get_state() == VerificationState::ProofSetup,
                InvalidAccountState
            );

            combined_miller_loop(verification_account, vkey, instruction, round)?;
            verification_account.serialize_rams().unwrap();
        }
        VerificationStep::FinalExponentiation => {
            // This enables us to use a uniform number of ixs per tx (by only allowing the last ix to perform the computation)
            if instruction_index != COMPUTE_VERIFICATION_IX_COUNT - 1 {
                return Ok(None);
            }

            let v = final_exponentiation(verification_account, vkey, instruction, round);
            verification_account.serialize_rams().unwrap();
            return v;
        }
    }

    Ok(None)
}

pub fn prepare_public_inputs(
    verification_account: &mut VerificationAccount,
    vkey: &VerifyingKey,
    instruction: usize,
    round: usize,
) -> ElusivResult {
    let rounds = verification_account.get_prepare_inputs_instructions(instruction);

    let result = prepare_public_inputs_partial(round, rounds as usize, verification_account, vkey);

    if round + rounds as usize == prepare_public_inputs_rounds(vkey.public_inputs_count) {
        let prepared_inputs = result.ok_or(CouldNotProcessProof)?;

        verification_account
            .prepared_inputs
            .set(G1A(prepared_inputs));

        verification_account.set_step(&VerificationStep::CombinedMillerLoop);
        verification_account.set_round(&0);
        verification_account.set_instruction(&0);
    } else {
        verification_account.set_round(&(round as u32 + rounds as u32));
        verification_account.set_instruction(&(instruction as u32 + 1));
    }

    Ok(())
}

pub fn combined_miller_loop(
    verification_account: &mut VerificationAccount,
    vkey: &VerifyingKey,
    instruction: usize,
    round: usize,
) -> ElusivResult {
    let rounds = CombinedMillerLoop::INSTRUCTION_ROUNDS[instruction] as usize;

    let mut r = verification_account.r.get();
    let mut alt_b = verification_account.alt_b.get();
    let mut coeff_index = verification_account.get_coeff_index() as usize;

    let a = verification_account.a.get().0;
    let b = verification_account.b.get().0;
    let c = verification_account.c.get().0;
    let prepared_inputs = verification_account.prepared_inputs.get().0;

    let mut result = None;
    for round in round..round + rounds {
        result = combined_miller_loop_partial(
            round,
            verification_account,
            vkey,
            &a,
            &b,
            &c,
            &prepared_inputs,
            &mut r,
            &mut coeff_index,
            &mut alt_b,
        )?;
    }

    verification_account.set_coeff_index(&usize_as_u8_safe(coeff_index));

    if round + rounds == CombinedMillerLoop::TOTAL_ROUNDS as usize {
        let f = result.ok_or(CouldNotProcessProof)?;

        // Add `f` for the final exponentiation
        verification_account.f.set(Wrap(f));

        verification_account.set_step(&VerificationStep::FinalExponentiation);
        verification_account.set_round(&0);
        verification_account.set_instruction(&0);
    } else {
        verification_account.r.set(r);
        verification_account.alt_b.set(alt_b);

        verification_account.set_round(&usize_as_u32_safe(round + rounds));
        verification_account.set_instruction(&(instruction as u32 + 1));
    }

    Ok(())
}

pub fn final_exponentiation(
    verification_account: &mut VerificationAccount,
    vkey: &VerifyingKey,
    instruction: usize,
    round: usize,
) -> Result<Option<bool>, ElusivError> {
    guard!(
        instruction < FinalExponentiation::IX_COUNT,
        ComputationIsAlreadyFinished
    );

    let rounds = FinalExponentiation::INSTRUCTION_ROUNDS[instruction] as usize;

    let f = verification_account.f.get().0;

    let mut result = None;
    for round in round..round + rounds {
        result = final_exponentiation_partial(round, verification_account, &f)?;
    }

    verification_account.set_round(&usize_as_u32_safe(round + rounds));
    verification_account.set_instruction(&(instruction as u32 + 1));

    if round + rounds == FinalExponentiation::TOTAL_ROUNDS as usize {
        let v = result.ok_or(CouldNotProcessProof)?;
        verification_account.f.set(Wrap(v));

        // Final verification, we check:
        // https://github.com/zkcrypto/bellman/blob/9bb30a7bd261f2aa62840b80ed6750c622bebec3/src/groth16/verifier.rs#L43
        // https://github.com/arkworks-rs/groth16/blob/765817f77a6e14964c6f264d565b18676b11bd59/src/verifier.rs#L60
        return Ok(Some(vkey.alpha_beta() == v));
    }

    Ok(None)
}

macro_rules! read_g1_p {
    ($ram: expr, $o: literal) => {
        G1Projective::new($ram.read($o), $ram.read($o + 1), $ram.read($o + 2))
    };
}

const PREPARE_PUBLIC_INPUTS_ROUNDS: usize = 33;
const fn prepare_public_inputs_rounds(public_inputs_count: usize) -> usize {
    PREPARE_PUBLIC_INPUTS_ROUNDS * public_inputs_count
}

/// Public input preparation
///
/// # Notes
///
/// - `prepared_inputs = \sum_{i = 0}ˆ{N} input_{i} gamma_abc_g1_{i}`
/// - reference implementation: https://github.com/arkworks-rs/groth16/blob/765817f77a6e14964c6f264d565b18676b11bd59/src/verifier.rs#L22
/// - N public inputs (elements of the scalar field) in non-reduced form
/// - the total rounds required for preparation of all inputs is `PREPARE_PUBLIC_INPUTS_ROUNDS` * N
/// - this partial computation is different from the rest, in that it's cost is dependent on the public inputs count and bits
/// - for `prepare_public_inputs` we use 1 instruction with 1.4m compute units
fn prepare_public_inputs_partial(
    round: usize,
    rounds: usize,
    storage: &mut VerificationAccount,
    vkey: &VerifyingKey,
) -> Option<G1Affine> {
    let mut acc: G1Projective = read_g1_p!(storage.ram_fq, 3);
    let mut input_index = round / PREPARE_PUBLIC_INPUTS_ROUNDS;
    let mut public_input = storage.get_public_input(input_index).skip_mr();

    for round in round..round + rounds {
        let round = round % PREPARE_PUBLIC_INPUTS_ROUNDS;
        if round == 0 {
            acc = G1Projective::zero();
        }

        if round < PREPARE_PUBLIC_INPUTS_ROUNDS - 1 {
            let gamma_abc = vkey.gamma_abc(input_index, round, public_input[round]);
            acc.add_assign_mixed(&gamma_abc);
        } else {
            // Adding
            let mut g_ic = if input_index == 0 {
                vkey.gamma_abc_base()
            } else {
                read_g1_p!(storage.ram_fq, 0)
            };

            if public_input != [0; 32] {
                g_ic += acc;
            }

            if input_index < vkey.public_inputs_count - 1 {
                write_g1_projective(&mut storage.ram_fq, &g_ic, 0);

                input_index += 1;
                public_input = storage.get_public_input(input_index).skip_mr();
            } else {
                return Some(g_ic.into_affine());
            }
        }
    }

    write_g1_projective(&mut storage.ram_fq, &acc, 3);

    None
}

#[cfg(feature = "elusiv-client")]
pub fn precomputed_input_preparation(
    vkey: &VerifyingKey,
    public_inputs: &[U256],
) -> Option<G1Affine> {
    if public_inputs.len() != vkey.public_inputs_count {
        return None;
    }

    let mut g_ic = vkey.gamma_abc_base();
    for (i, public_input) in public_inputs.iter().enumerate() {
        if *public_input == [0; 32] {
            continue;
        }

        let mut acc = G1Projective::zero();

        for (j, window) in public_input.iter().enumerate() {
            let gamma_abc = vkey.gamma_abc(i, j, *window);
            acc.add_assign_mixed(&gamma_abc);
        }

        g_ic += acc;
    }
    Some(g_ic.into_affine())
}

const ADD_MIXED_COST: u16 = 22;
const ADD_COST: u16 = 30;
const MAX_CUS: u16 = 1_330; // 1_400_000 / 1000 minus padding

/// Returns the instructions (and their rounds) required for a specific public-input-bound input preparation
pub fn prepare_public_inputs_instructions(
    public_inputs: &[U256],
    public_inputs_count: usize,
) -> Vec<u32> {
    assert!(public_inputs.len() == public_inputs_count);

    let mut instructions = Vec::new();

    let mut total_rounds = 0;
    let mut rounds = 0;
    let mut compute_units = 0;

    for public_input in public_inputs.iter() {
        for b in 0..33 {
            let cus = if b == 32 {
                if *public_input == [0; 32] {
                    0
                } else {
                    ADD_COST
                }
            } else if public_input[b] == 0 {
                0
            } else {
                ADD_MIXED_COST
            };

            if compute_units + cus > MAX_CUS {
                instructions.push(rounds);

                rounds = 1;
                compute_units = cus;
            } else {
                rounds += 1;
                compute_units += cus;
            }

            total_rounds += 1;
        }
    }

    if rounds > 0 {
        instructions.push(rounds);
    }

    // Redundant check
    assert_eq!(
        total_rounds,
        prepare_public_inputs_rounds(public_inputs_count)
    );

    instructions
}

#[cfg(test)]
const_assert_eq!(ADDITION_STEP_ROUNDS_COUNT, 2);
#[cfg(test)]
const_assert_eq!(DOUBLING_STEP_ROUNDS_COUNT, 2);
#[cfg(test)]
const_assert_eq!(MUL_BY_CHARACTERISTICS_ROUNDS_COUNT, 2);
#[cfg(test)]
const_assert_eq!(MUL_BY_034_ROUNDS_COUNT, 3);
#[cfg(test)]
const_assert_eq!(COMBINED_ELL_ROUNDS_COUNT, 13);

pub const COMBINED_MILLER_LOOP_IXS: usize = 215;
pub const FINAL_EXPONENTIATION_IXS: usize = 17;

#[cfg(test)]
const_assert_eq!(CombinedMillerLoop::IX_COUNT, COMBINED_MILLER_LOOP_IXS);

#[cfg(test)]
const_assert_eq!(CombinedMillerLoop::TX_COUNT, 43);

#[cfg(test)]
const_assert_eq!(FinalExponentiation::IX_COUNT, FINAL_EXPONENTIATION_IXS);

#[cfg(test)]
const_assert_eq!(FinalExponentiation::TX_COUNT, 17);

elusiv_computations!(
    combined_miller_loop, CombinedMillerLoop, 250_000,

    // Doubling step
    // https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/g2.rs#L139
    doubling_step(storage: &mut VerificationAccount, r: &mut G2HomProjective) -> Coefficients {
        {   /// 43_000
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
        {   /// 25_000
            let i: Fq2 = e - b;
            let j: Fq2 = r.x.square();

            r.x = a * (b - f);
            r.y = g.square() - (e_square.double() + e_square);
            r.z = b * h;

            return new_coeffs(h.neg(), j.double() + j, i);
        }
    },

    // Addition step
    // https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/g2.rs#L168
    addition_step(storage: &mut VerificationAccount, r: &mut G2HomProjective, q: &G2Affine) -> Coefficients {
        {   /// 43_000
            let theta: Fq2 = r.y - (q.y * r.z);
            let lambda: Fq2 = r.x - (q.x * r.z);
            let c: Fq2 = theta.square();
            let d: Fq2 = lambda.square();
            let e: Fq2 = lambda * d;
            let f: Fq2 = r.z * c;
            let g: Fq2 = r.x * d;
        }
        {   /// 42_000
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
        {   /// 12_000
            let mut x: Fq2 = frobenius_map_fq2_one(r.x);
            x = x * TWIST_MUL_BY_Q_X;
        }
        {   /// 12_000
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
        {   /// 20_500
            let a: Fq6 = Fq6::new(f.c0.c0 * c0, f.c0.c1 * c0, f.c0.c2 * c0);
        }

        {   /// 55_500
            let b: Fq6 = mul_fq6_by_c0_c1_0(f.c1, d0, d1);
        }

        {   /// 44_500
            let e: Fq6 = mul_fq6_by_c0_c1_0(f.c0 + f.c1, &(*c0 + d0), d1);
            return Fq12::new(mul_base_field_by_nonresidue(b) + a, e - (a + b));
        }
    },

    // We evaluate the line function for A, the prepared inputs and C
    // - inside the miller loop we do evaluations on three elements
    // - multi_ell combines those three calls in one function
    // - normal ell implementation: https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/mod.rs#L59
    combined_ell(
        storage: &mut VerificationAccount,
        vkey: &VerifyingKey,
        a: &G1Affine, prepared_inputs: &G1Affine, c: &G1Affine,
        c0: &Fq2, c1: &Fq2, c2: &Fq2, coeff_index: usize, f: Fq12,
    ) -> Fq12 {
        // ell on A with c0, c1, c2
        {   /// 9_500
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
        {   /// 9_200
            let b0: Fq2 = mul_by_fp(&(vkey.gamma_g2_neg_pc(coeff_index, 0)), prepared_inputs.y);
            let b1: Fq2 = mul_by_fp(&(vkey.gamma_g2_neg_pc(coeff_index, 1)), prepared_inputs.x);
        }
        {   /// mul_by_034
            if (!(prepared_inputs.is_zero())) {
                partial v = mul_by_034(storage, &b0, &b1, &(vkey.gamma_g2_neg_pc(coeff_index, 2)), r) { r = v }
            }
        }

        // ell on C with delta_g2_neg_pc
        {   /// 9_200
            let d0: Fq2 = mul_by_fp(&(vkey.delta_g2_neg_pc(coeff_index, 0)), c.y);
            let d1: Fq2 = mul_by_fp(&(vkey.delta_g2_neg_pc(coeff_index, 1)), c.x);
        }
        {   /// mul_by_034
            if (!(c.is_zero())) {
                partial v = mul_by_034(storage, &d0, &d1, &(vkey.delta_g2_neg_pc(coeff_index, 2)), r) { r = v }
            }
        }

        {   /// 1000
            return r;
        }
    },

    // 0,0,0,1,0,1,0,2,0,0,1,2,0,0,1,0,0,1,1,0,2,0,0,1,0,2,0,0,0,0,1,1,1,0,0,2,0,0,1,0,0,0,0,0,2,0,0,1,1,0,0,2,0,0,0,1,1,0,2,0,0,1,0,1,1,
    // 1,1,0,1,0,0,2,0,1,1,0,0,0,2,0,0,1,1,0,0,2,0,0,0,0,0,1,0,0,2,0,0,1,1,1,0,0,0,0,2,0,1,0,0,2,0,1,1,0,0,1,0,0,2,1,0,0,2,0,1,0,1,0,0,0

    // We combine the miller loop and the coefficient generation for B
    // - miller loop ref: https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/mod.rs#L99
    // - coefficient generation ref: https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/g2.rs#L68
    // - implementation:
    // - the miller loop receives an iterator over 3 elements (https://github.com/arkworks-rs/groth16/blob/765817f77a6e14964c6f264d565b18676b11bd59/src/verifier.rs#L41)
    // - for B we need to generate the coefficients (all other coefficients are already generated before compilation)
    combined_miller_loop(
        storage: &mut VerificationAccount,
        vkey: &VerifyingKey,
        a: &G1Affine, b: &G2Affine, c: &G1Affine, prepared_inputs: &G1Affine,
        r: &mut G2HomProjective, j: &mut usize, alt_b: &mut G2A,
    ) -> Fq12 {
        {   /// 3000
            r.x = b.x;
            r.y = b.y;
            r.z = Fq2::one();

            let f: Fq12 = Fq12::one();

            // values for B coefficient generation (https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/g2.rs#L79)
            _ = alt_b.set(b.neg());
            let c0: Fq2 = Fq2::zero();
            let c1: Fq2 = Fq2::zero();
            let c2: Fq2 = Fq2::zero();
        }

        // Reversed ATE_LOOP_COUNT with the the last element removed (so the first in the reversed order)
        // https://github.com/arkworks-rs/curves/blob/1551d6d76ce5abf6e7925e53b0ea1af7dbc421c3/bn254/src/curves/mod.rs#L21
        // https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/mod.rs#L121 (last element is ignored)
        {
            for i, ate_loop_count in [1,0,1,0,0,2,0,1,1,0,0,0,2,0,0,1,1,0,0,2,0,0,0,0,0,1,0,0,2,0,0,1,1,1,0,0,0,0,2,0,1,0,0,2,0,1,1,0,0,1,0,0,2,1,0,0,2,0,1,0,1,0,0,0] {
                {   /// i in { 0 : 0 , _ : 88_000 }
                    if (i > 0) {
                        _ = f.square_in_place();
                    }
                }

                partial v = doubling_step(storage, r) { c0=v.0; c1=v.1; c2=v.2; };
                partial v = combined_ell(storage, vkey, a, prepared_inputs, c, &c0, &c1, &c2, *j, f) {
                    f = v;
                    _ = j.add_assign(1);
                };

                {   /// ate_loop_count in { 0 : addition_step_zero , _ : addition_step }
                    if (ate_loop_count > 0) {
                        if (ate_loop_count = 1) {
                            partial v = addition_step(storage, r, b) { c0=v.0; c1=v.1; c2=v.2; };
                        } else {
                            partial v = addition_step(storage, r, alt_b.get()) { c0=v.0; c1=v.1; c2=v.2; };
                        }
                    }
                }
                {   /// ate_loop_count in { 0 : combined_ell_zero , _ : combined_ell }
                    if (ate_loop_count > 0) {
                        partial v = combined_ell(storage, vkey, a, prepared_inputs, c, &c0, &c1, &c2, *j, f) {
                            f = v;
                            _ = j.add_assign(1);
                        };
                    }
                }
            }
        }
        // The final two coefficient triples
        {
            partial v = mul_by_characteristics(storage, b) {
                _ = alt_b.set(v);
            };
            partial v = addition_step(storage, r, alt_b.get()) { c0=v.0; c1=v.1; c2=v.2; };

            partial v = combined_ell(storage, vkey, a, prepared_inputs, c, &c0, &c1, &c2, *j, f) {
                if (!(prepared_inputs.is_zero())) {
                    f = v;
                    _ = j.add_assign(1);
                }
            };
            partial v = mul_by_characteristics(storage, alt_b.get()) {
                _ = alt_b.set(G2Affine::new(v.x, v.y.neg(), v.infinity));
            };
        }
        {
            partial v = addition_step(storage, r, alt_b.get()) { c0=v.0; c1=v.1; c2=v.2; };

            partial v = combined_ell(storage, vkey, a, prepared_inputs, c, &c0, &c1, &c2, *j, f) {
                if (!(prepared_inputs.is_zero())) {
                    f = v;
                    _ = j.add_assign(1);
                }
            };
        }
        {   /// 500
            return f;
        }
    }
);

#[cfg(test)]
const_assert_eq!(INVERSE_FQ12_ROUNDS_COUNT, 4);
#[cfg(test)]
const_assert_eq!(EXP_BY_NEG_X_ROUNDS_COUNT, 128);

elusiv_computations!(
    final_exponentiation, FinalExponentiation, 1_300_000,

    // https://github.com/arkworks-rs/algebra/blob/80857c9714c5a59068f8c20f1298e2138440a1d0/ff/src/fields/models/quadratic_extension.rs#L688
    // Guide to Pairing-based cryprography, Algorithm 5.16.
    /*mul_fq12(storage: &mut VerificationAccount, a: Fq12, b: Fq12) -> Fq12 {
        {   /// 63_000
            let v0: Fq6 = a.c0 * b.c0;
            let v1: Fq6 = a.c1 * b.c1;
        }
        {   /// 63_000
            let mut c1: Fq6 = a.c1 + a.c0;
            c1 = c1 * (b.c0 + b.c1);
            c1 = c1 - v0;
            c1 = c1 - v1;
        }
        {   /// 1_000
            let c0: Fq6 = add_and_mul_base_field_by_nonresidue(v0, v1);
            return Fq12::new(c0, c1);
        }
    }*/

    // https://github.com/arkworks-rs/algebra/blob/4dd6c3446e8ab22a2ba13505a645ea7b3a69f493/ff/src/fields/models/quadratic_extension.rs#L366
    // Guide to Pairing-based Cryptography, Algorithm 5.19.
    inverse_fq12(storage: &mut VerificationAccount, f: Fq12) -> Fq12 {
        {   /// 28_500
            let v1: Fq6 = f.c1.square();
        }
        {   /// 28_500
            let v2: Fq6 = f.c0.square();
        }
        {   /// 150_000
            let v0: Fq6 = sub_and_mul_base_field_by_nonresidue(v2, v1);
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
            let q: Fq12 = r;
            let f2: Fq12 = *f;
        }
        {
            partial v = inverse_fq12(storage, f2) {
                f2 = v;
            }
        }
        {   /// 126_000
            r = r * f2;
            f2 = r;
        }
        {   /// 55_000
            r = frobenius_map(r, 2);
        }
        {   /// 126_000
            r = r * f2;
            let y0: Fq12 = r;
        }
        {
            partial v = exp_by_neg_x(storage, y0) { y0 = v; }
        }
        {   /// 90_000
            let y1: Fq12 = y0.cyclotomic_square();
            let y2: Fq12 = y1.cyclotomic_square();
        }
        {   /// 126_000
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

fn write_g1_projective(ram: &mut RAMFq, g1p: &G1Projective, offset: usize) {
    ram.write(g1p.x, offset);
    ram.write(g1p.y, offset + 1);
    ram.write(g1p.z, offset + 2);
}

/// Inverse of 2 (in q)
/// - Calculated using: Fq::one().double().inverse().unwrap()
const TWO_INV: Fq = Fq::new(BigInteger256::new([
    9781510331150239090,
    15059239858463337189,
    10331104244869713732,
    2249375503248834476,
]));

/// https://docs.rs/ark-bn254/0.3.0/src/ark_bn254/curves/g2.rs.html#19
/// COEFF_B = 3/(u+9) = (19485874751759354771024239261021720505790618469301721065564631296452457478373, 266929791119991161246907387137283842545076965332900288569378510910307636690)
const COEFF_B: Fq2 = field_new!(
    Fq2,
    field_new!(
        Fq,
        "19485874751759354771024239261021720505790618469301721065564631296452457478373"
    ),
    field_new!(
        Fq,
        "266929791119991161246907387137283842545076965332900288569378510910307636690"
    ),
);

type Coefficients = (Fq2, Fq2, Fq2);
fn new_coeffs(c0: Fq2, c1: Fq2, c2: Fq2) -> Coefficients {
    (c0, c1, c2)
}

const TWIST_MUL_BY_Q_X: Fq2 = Parameters::TWIST_MUL_BY_Q_X;
const TWIST_MUL_BY_Q_Y: Fq2 = Parameters::TWIST_MUL_BY_Q_Y;

fn frobenius_map_fq2_one(f: Fq2) -> Fq2 {
    let mut k = f;
    k.frobenius_map(1);
    k
}

// https://github.com/arkworks-rs/algebra/blob/4dd6c3446e8ab22a2ba13505a645ea7b3a69f493/ff/src/fields/models/quadratic_extension.rs#L87
// https://github.com/arkworks-rs/algebra/blob/4dd6c3446e8ab22a2ba13505a645ea7b3a69f493/ff/src/fields/models/quadratic_extension.rs#L56
fn sub_and_mul_base_field_by_nonresidue(x: Fq6, y: Fq6) -> Fq6 {
    x - mul_base_field_by_nonresidue(y)
}

/*fn add_and_mul_base_field_by_nonresidue(x: Fq6, y: Fq6) -> Fq6 {
    x + mul_base_field_by_nonresidue(y)
}*/

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

    let b1_plus_b2: Fq2 = *c1;
    let b0_plus_b1: Fq2 = *c0 + c1;
    let b0_plus_b2: Fq2 = *c0;

    Fq6::new(
        (a1_plus_a2 * b1_plus_b2 - v1) * Fp6ParamsWrapper::<Fq6Parameters>::NONRESIDUE + v0,
        a0_plus_a1 * b0_plus_b1 - v0 - v1,
        a0_plus_a2 * b0_plus_b2 - v0 + v1,
    )
}

fn mul_by_fp(v: &Fq2, fp: Fq) -> Fq2 {
    let mut v: Fq2 = *v;
    v.mul_assign_by_fp(&fp);
    v
}

fn conjugate(f: Fq12) -> Fq12 {
    let mut k = f;
    k.conjugate();
    k
}
fn frobenius_map(f: Fq12, u: usize) -> Fq12 {
    let mut k = f;
    k.frobenius_map(u);
    k
}

#[cfg(feature = "test-elusiv")]
use crate::types::Proof;
#[cfg(feature = "test-elusiv")]
use std::str::FromStr;

#[cfg(feature = "test-elusiv")]
pub fn proof_from_str(
    a: (&str, &str, bool),
    b: ((&str, &str), (&str, &str), bool),
    c: (&str, &str, bool),
) -> Proof {
    Proof {
        a: G1A(G1Affine::new(
            Fq::from_str(a.0).unwrap(),
            Fq::from_str(a.1).unwrap(),
            a.2,
        )),
        b: G2A(G2Affine::new(
            Fq2::new(Fq::from_str(b.0 .0).unwrap(), Fq::from_str(b.0 .1).unwrap()),
            Fq2::new(Fq::from_str(b.1 .0).unwrap(), Fq::from_str(b.1 .1).unwrap()),
            b.2,
        )),
        c: G1A(G1Affine::new(
            Fq::from_str(c.0).unwrap(),
            Fq::from_str(c.1).unwrap(),
            c.2,
        )),
    }
}

#[cfg(feature = "test-elusiv")]
#[allow(clippy::type_complexity)]
pub fn proof_from_str_projective(
    a: (&str, &str, &str),
    b: ((&str, &str), (&str, &str), (&str, &str)),
    c: (&str, &str, &str),
) -> Proof {
    use ark_bn254::G2Projective;

    Proof {
        a: G1A(G1Projective::new(
            Fq::from_str(a.0).unwrap(),
            Fq::from_str(a.1).unwrap(),
            Fq::from_str(a.2).unwrap(),
        )
        .into()),
        b: G2A(G2Projective::new(
            Fq2::new(Fq::from_str(b.0 .0).unwrap(), Fq::from_str(b.0 .1).unwrap()),
            Fq2::new(Fq::from_str(b.1 .0).unwrap(), Fq::from_str(b.1 .1).unwrap()),
            Fq2::new(Fq::from_str(b.2 .0).unwrap(), Fq::from_str(b.2 .1).unwrap()),
        )
        .into()),
        c: G1A(G1Projective::new(
            Fq::from_str(c.0).unwrap(),
            Fq::from_str(c.1).unwrap(),
            Fq::from_str(c.2).unwrap(),
        )
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::{u256_from_str_skip_mr, u256_to_fr_skip_mr};
    use crate::macros::zero_program_account;
    use crate::proof::test_proofs::{invalid_proofs, valid_proofs};
    use crate::proof::vkey::{TestVKey, VerifyingKeyInfo};
    use crate::state::metadata::CommitmentMetadata;
    use crate::state::storage::empty_root_raw;
    use crate::types::{
        InputCommitment, JoinSplitPublicInputs, OptionalFee, PublicInputs, RawU256,
        SendPublicInputs,
    };
    use ark_bn254::{Bn254, Fr};
    use ark_ec::bn::G2Prepared;
    use ark_ec::models::bn::BnParameters;
    use ark_ec::PairingEngine;
    use ark_groth16::prepare_inputs;
    use solana_program::native_token::LAMPORTS_PER_SOL;
    use std::str::FromStr;

    fn setup_storage_account<VKey: VerifyingKeyInfo>(
        storage: &mut VerificationAccount,
        proof: Proof,
        public_inputs: &[U256],
    ) {
        storage.a.set(proof.a);
        storage.b.set(proof.b);
        storage.c.set(proof.c);
        storage.set_state(&VerificationState::ProofSetup);

        for (i, &public_input) in public_inputs.iter().enumerate() {
            storage.set_public_input(i, &RawU256::new(public_input));
        }

        let instructions =
            prepare_public_inputs_instructions(public_inputs, VKey::public_inputs_count());
        storage
            .setup_public_inputs_instructions(&instructions)
            .unwrap();
    }

    fn f() -> Fq12 {
        let f = Fq6::new(
            Fq2::new(
                Fq::from_str(
                    "20925091368075991963132407952916453596237117852799702412141988931506241672722",
                )
                .unwrap(),
                Fq::from_str(
                    "18684276579894497974780190092329868933855710870485375969907530111657029892231",
                )
                .unwrap(),
            ),
            Fq2::new(
                Fq::from_str(
                    "5932690455294482368858352783906317764044134926538780366070347507990829997699",
                )
                .unwrap(),
                Fq::from_str(
                    "18684276579894497974780190092329868933855710870485375969907530111657029892231",
                )
                .unwrap(),
            ),
            Fq2::new(
                Fq::from_str(
                    "18684276579894497974780190092329868933855710870485375969907530111657029892231",
                )
                .unwrap(),
                Fq::from_str(
                    "19526707366532583397322534596786476145393586591811230548888354920504818678603",
                )
                .unwrap(),
            ),
        );
        Fq12::new(f, f)
    }

    fn g2_affine() -> G2Affine {
        G2Affine::new(f().c0.c0, f().c0.c1, false)
    }

    macro_rules! vkey {
        ($id: ident, $vkey: ident) => {
            let source = $vkey::verifying_key_source();
            let $id = VerifyingKey::new(&source, $vkey::public_inputs_count()).unwrap();
        };
    }

    #[test]
    fn test_prepare_public_inputs() {
        vkey!(vkey, TestVKey);
        let pvk = TestVKey::arkworks_pvk();
        let public_inputs = vec![
            "5932690455294482368858352783906317764044134926538780366070347507990829997699",
            "18684276579894497974780190092329868933855710870485375969907530111657029892231",
            "19526707366532583397322534596786476145393586591811230548888354920504818678603",
            "20925091368075991963132407952916453596237117852799702412141988931506241672722",
            "3932690455294482368858352783906317764044134926538780366070347507990829997699",
            "932690455294482368858352783906317764044134926538780366070347507990829997699",
            "455294482368858352783906317764044134926538780366070347507990829997699",
            "5932690455294482368858352783906317764044134926538780366070347507990829997699",
            "18684276579894497974780190092329868933855710870485375969907530111657029892231",
            "19526707366532583397322534596786476145393586591811230548888354920504818678603",
            "20925091368075991963132407952916453596237117852799702412141988931506241672722",
            "3932690455294482368858352783906317764044134926538780366070347507990829997699",
            "932690455294482368858352783906317764044134926538780366070347507990829997699",
            "455294482368858352783906317764044134926538780366070347507990829997699",
        ];

        // First version
        zero_program_account!(mut storage, VerificationAccount);
        for (i, public_input) in public_inputs.iter().enumerate() {
            storage.set_public_input(i, &RawU256::new(u256_from_str_skip_mr(public_input)));
        }

        // precomputed_input_preparation version
        let p_result = precomputed_input_preparation(
            &vkey,
            &public_inputs
                .iter()
                .map(|&p| u256_from_str_skip_mr(p))
                .collect::<Vec<U256>>()[..],
        )
        .unwrap();

        let result = prepare_public_inputs_partial(
            0,
            prepare_public_inputs_rounds(TestVKey::public_inputs_count()),
            &mut storage,
            &vkey,
        )
        .unwrap();
        let public_inputs: Vec<Fr> = public_inputs
            .iter()
            .map(|s| Fr::from_str(s).unwrap())
            .collect();
        let expected = prepare_inputs(&pvk, &public_inputs).unwrap().into_affine();
        assert_eq!(result, expected);
        assert_eq!(result, p_result);

        // Second version
        zero_program_account!(mut storage, VerificationAccount);
        let public_inputs = valid_proofs()[0].public_inputs.clone();
        setup_storage_account::<TestVKey>(&mut storage, valid_proofs()[0].proof, &public_inputs);

        for i in 0..storage.get_prepare_inputs_instructions_count() {
            let round = storage.get_round();
            prepare_public_inputs(&mut storage, &vkey, i as usize, round as usize).unwrap();
        }
        let expected = prepare_inputs(
            &pvk,
            &public_inputs
                .iter()
                .map(|&x| u256_to_fr_skip_mr(&RawU256::new(x).reduce()))
                .collect::<Vec<Fr>>(),
        )
        .unwrap()
        .into_affine();
        assert_eq!(storage.prepared_inputs.get().0, expected);
    }

    #[test]
    fn test_mul_by_characteristics() {
        zero_program_account!(mut storage, VerificationAccount);
        let mut value: Option<G2Affine> = None;
        for round in 0..MUL_BY_CHARACTERISTICS_ROUNDS_COUNT {
            value = mul_by_characteristics_partial(round, &mut storage, &g2_affine()).unwrap();
        }

        assert_eq!(value.unwrap(), reference_mul_by_char(g2_affine()));
    }

    #[test]
    fn test_combined_ell() {
        vkey!(vkey, TestVKey);
        zero_program_account!(mut storage, VerificationAccount);
        let mut value: Option<Fq12> = None;
        let a = G1Affine::new(
            Fq::from_str(
                "10026859857882131638516328056627849627085232677511724829502598764489185541935",
            )
            .unwrap(),
            Fq::from_str(
                "19685960310506634721912121951341598678325833230508240750559904196809564625591",
            )
            .unwrap(),
            false,
        );
        let prepared_inputs = G1Affine::new(
            Fq::from_str(
                "6859857882131638516328056627849627085232677511724829502598764489185541935",
            )
            .unwrap(),
            Fq::from_str("310506634721912121951341598678325833230508240750559904196809564625591")
                .unwrap(),
            false,
        );
        let c = G1Affine::new(
            Fq::from_str(
                "21186803555845400161937398579081414146527572885637089779856221229551142844794",
            )
            .unwrap(),
            Fq::from_str(
                "85960310506634721912121951341598678325833230508240750559904196809564625591",
            )
            .unwrap(),
            false,
        );
        let c0 = f().c0.c0;
        let c1 = f().c0.c1;
        let c2 = f().c0.c2;
        for round in 0..COMBINED_ELL_ROUNDS_COUNT {
            value = combined_ell_partial(
                round,
                &mut storage,
                &vkey,
                &a,
                &prepared_inputs,
                &c,
                &c0,
                &c1,
                &c2,
                0,
                f(),
            )
            .unwrap();
        }

        let pvk = TestVKey::arkworks_pvk();
        let mut expected = f();
        expected = reference_ell(expected, (c0, c1, c2), a);
        expected = reference_ell(expected, pvk.gamma_g2_neg_pc.ell_coeffs[0], prepared_inputs);
        expected = reference_ell(expected, pvk.delta_g2_neg_pc.ell_coeffs[0], c);

        assert_eq!(expected, value.unwrap());
    }

    #[test]
    fn test_combined_miller_loop() {
        vkey!(vkey, TestVKey);
        zero_program_account!(mut storage, VerificationAccount);
        let prepared_inputs = G1Affine::new(
            Fq::new(BigInteger256([
                8166105574990738357,
                14893958969660524502,
                13741065838606745905,
                2671370669009161592,
            ])),
            Fq::new(BigInteger256([
                1732807305541484699,
                1852698713330294736,
                13051725764221510649,
                2467965794402811157,
            ])),
            false,
        );

        let proof = proof_from_str(
            (
                "10026859857882131638516328056627849627085232677511724829502598764489185541935",
                "19685960310506634721912121951341598678325833230508240750559904196809564625591",
                false,
            ),
            (
                (
                    "857882131638516328056627849627085232677511724829502598764489185541935",
                    "685960310506634721912121951341598678325833230508240750559904196809564625591",
                ),
                (
                    "837064132573119120838379738103457054645361649757131991036638108422638197362",
                    "86803555845400161937398579081414146527572885637089779856221229551142844794",
                ),
                false,
            ),
            (
                "21186803555845400161937398579081414146527572885637089779856221229551142844794",
                "85960310506634721912121951341598678325833230508240750559904196809564625591",
                false,
            ),
        );

        // First version
        let mut r = G2HomProjective {
            x: Fq2::zero(),
            y: Fq2::zero(),
            z: Fq2::zero(),
        };
        let mut j = 0;
        let mut alt_b = G2A(g2_affine());
        let mut result = None;
        for round in 0..COMBINED_MILLER_LOOP_ROUNDS_COUNT {
            result = combined_miller_loop_partial(
                round,
                &mut storage,
                &vkey,
                &proof.a.0,
                &proof.b.0,
                &proof.c.0,
                &prepared_inputs,
                &mut r,
                &mut j,
                &mut alt_b,
            )
            .unwrap();
        }
        assert_eq!(j, 91);

        let pvk = TestVKey::arkworks_pvk();
        let b: G2Prepared<Parameters> = proof.b.0.into();

        let expected = Bn254::miller_loop(
            [
                (proof.a.0.into(), b),
                (prepared_inputs.into(), pvk.gamma_g2_neg_pc),
                (proof.c.0.into(), pvk.delta_g2_neg_pc),
            ]
            .iter(),
        );

        assert_eq!(result.unwrap(), expected);

        // Second version
        zero_program_account!(mut storage, VerificationAccount);
        storage.a.set(proof.a);
        storage.b.set(proof.b);
        storage.c.set(proof.c);
        storage.set_step(&VerificationStep::CombinedMillerLoop);
        storage.prepared_inputs.set(G1A(prepared_inputs));

        for i in 0..COMBINED_MILLER_LOOP_IXS {
            let round = storage.get_round();
            combined_miller_loop(&mut storage, &vkey, i, round as usize).unwrap();
        }
        assert_eq!(storage.f.get().0, expected);
    }

    #[test]
    fn test_addition_step() {
        zero_program_account!(mut storage, VerificationAccount);
        let q = g2_affine();
        let mut r = G2HomProjective {
            x: Fq2::new(
                Fq::from_str(
                    "20925091368075991963132407952916453596237117852799702412141988931506241672722",
                )
                .unwrap(),
                Fq::from_str(
                    "18684276579894497974780190092329868933855710870485375969907530111657029892231",
                )
                .unwrap(),
            ),
            y: Fq2::new(
                Fq::from_str(
                    "5932690455294482368858352783906317764044134926538780366070347507990829997699",
                )
                .unwrap(),
                Fq::from_str(
                    "18684276579894497974780190092329868933855710870485375969907530111657029892231",
                )
                .unwrap(),
            ),
            z: Fq2::new(
                Fq::from_str(
                    "18684276579894497974780190092329868933855710870485375969907530111657029892231",
                )
                .unwrap(),
                Fq::from_str(
                    "19526707366532583397322534596786476145393586591811230548888354920504818678603",
                )
                .unwrap(),
            ),
        };
        let mut r2 = r;

        let mut result = None;
        for round in 0..ADDITION_STEP_ROUNDS_COUNT {
            result = addition_step_partial(round, &mut storage, &mut r, &q).unwrap();
        }

        let expected = reference_addition_step(&mut r2, &q);

        assert_eq!(result.unwrap(), expected);
        assert_eq!(r.x, r2.x);
        assert_eq!(r.y, r2.y);
        assert_eq!(r.z, r2.z);
    }

    #[test]
    fn test_doubling_step() {
        zero_program_account!(mut storage, VerificationAccount);
        let mut r = G2HomProjective {
            x: Fq2::new(
                Fq::from_str(
                    "20925091368075991963132407952916453596237117852799702412141988931506241672722",
                )
                .unwrap(),
                Fq::from_str(
                    "18684276579894497974780190092329868933855710870485375969907530111657029892231",
                )
                .unwrap(),
            ),
            y: Fq2::new(
                Fq::from_str(
                    "5932690455294482368858352783906317764044134926538780366070347507990829997699",
                )
                .unwrap(),
                Fq::from_str(
                    "18684276579894497974780190092329868933855710870485375969907530111657029892231",
                )
                .unwrap(),
            ),
            z: Fq2::new(
                Fq::from_str(
                    "18684276579894497974780190092329868933855710870485375969907530111657029892231",
                )
                .unwrap(),
                Fq::from_str(
                    "19526707366532583397322534596786476145393586591811230548888354920504818678603",
                )
                .unwrap(),
            ),
        };
        let mut r2 = r;

        let mut result = None;
        for round in 0..DOUBLING_STEP_ROUNDS_COUNT {
            result = doubling_step_partial(round, &mut storage, &mut r).unwrap();
        }

        let expected = reference_doubling_step(&mut r2, &TWO_INV);

        assert_eq!(result.unwrap(), expected);
        assert_eq!(r.x, r2.x);
        assert_eq!(r.y, r2.y);
        assert_eq!(r.z, r2.z);
    }

    /*#[test]
    fn test_mul_fq12() {
        storage!(storage);
        let mut value: Option<Fq12> = None;
        for round in 0..MUL_FQ12_ROUNDS_COUNT {
            value = mul_fq12_partial(round, &mut storage, f(), f()).unwrap();
        }

        assert_eq!(value.unwrap(), f() * f());
    }*/

    #[test]
    fn test_inverse_fq12() {
        zero_program_account!(mut storage, VerificationAccount);
        let mut value: Option<Fq12> = None;
        for round in 0..INVERSE_FQ12_ROUNDS_COUNT {
            value = inverse_fq12_partial(round, &mut storage, f()).unwrap();
        }

        assert_eq!(value.unwrap(), f().inverse().unwrap());
    }

    #[test]
    fn test_exp_by_neg_x() {
        zero_program_account!(mut storage, VerificationAccount);
        let mut value: Option<Fq12> = None;
        for round in 0..EXP_BY_NEG_X_ROUNDS_COUNT {
            value = exp_by_neg_x_partial(round, &mut storage, f()).unwrap();
        }

        assert_eq!(value.unwrap(), reference_exp_by_neg_x(f()));
    }

    #[test]
    fn test_final_exponentiation() {
        vkey!(vkey, TestVKey);

        // First version
        zero_program_account!(mut storage, VerificationAccount);
        let mut value = None;
        for round in 0..FINAL_EXPONENTIATION_ROUNDS_COUNT {
            value = final_exponentiation_partial(round, &mut storage, &f()).unwrap();
        }

        let expected = Bn254::final_exponentiation(&f()).unwrap();
        assert_eq!(value.unwrap(), expected);

        // Second version
        zero_program_account!(mut storage, VerificationAccount);
        storage.set_step(&VerificationStep::FinalExponentiation);
        storage.f.set(Wrap(f()));

        for i in 0..FINAL_EXPONENTIATION_IXS {
            let round = storage.get_round();
            final_exponentiation(&mut storage, &vkey, i, round as usize).unwrap();
        }
        assert_eq!(storage.f.get().0, expected);
    }

    #[test]
    fn test_public_inputs_preparation_costs() {
        let public_inputs = SendPublicInputs {
            join_split: JoinSplitPublicInputs {
                input_commitments: vec![
                    InputCommitment {
                        root: Some(empty_root_raw()),
                        nullifier_hash: RawU256::new(u256_from_str_skip_mr("10026859857882131638516328056627849627085232677511724829502598764489185541935")),
                    },
                    InputCommitment {
                        root: None,
                        nullifier_hash: RawU256::new(u256_from_str_skip_mr("13921430393547588871192356721184227660578793579443975701453971046059378311483")),
                    },
                ],
                output_commitment: RawU256::new(u256_from_str_skip_mr("685960310506634721912121951341598678325833230508240750559904196809564625591")),
                recent_commitment_index: 456,
                fee_version: 0,
                amount: LAMPORTS_PER_SOL * 123,
                fee: 0,
                optional_fee: OptionalFee::default(),
                token_id: 0,
                metadata: CommitmentMetadata::default(),
            },
            hashed_inputs: u256_from_str_skip_mr("230508240750559904196809564625"),
            recipient_is_associated_token_account: true,
            solana_pay_transfer: false,
        };
        let p = public_inputs.public_signals_skip_mr();
        let v = prepare_public_inputs_instructions(&p, TestVKey::public_inputs_count());
        assert_eq!(v.len(), 3);
    }

    #[test]
    fn test_prepare_public_inputs_instructions() {
        let expected = prepare_public_inputs_rounds(TestVKey::public_inputs_count()) as u32;

        assert_eq!(
            prepare_public_inputs_instructions(
                &vec![[0; 32]; TestVKey::public_inputs_count()],
                TestVKey::public_inputs_count()
            ),
            vec![expected]
        );
    }

    fn full_verification<VKey: VerifyingKeyInfo>(
        proof: Proof,
        public_inputs: &[U256],
        vkey: &VerifyingKey,
    ) -> bool {
        zero_program_account!(mut storage, VerificationAccount);
        setup_storage_account::<VKey>(&mut storage, proof, public_inputs);
        let instruction_count = storage.get_prepare_inputs_instructions_count() as usize
            + COMBINED_MILLER_LOOP_IXS
            + FINAL_EXPONENTIATION_IXS;

        let mut result = None;
        for _ in 0..instruction_count {
            result = verify_partial(&mut storage, vkey, COMPUTE_VERIFICATION_IX_COUNT - 1).unwrap();
        }

        result.unwrap()
    }

    #[test]
    fn test_verify_proofs() {
        vkey!(vkey, TestVKey);

        for p in valid_proofs() {
            assert!(full_verification::<TestVKey>(
                p.proof,
                &p.public_inputs,
                &vkey
            ));
        }

        for p in invalid_proofs() {
            assert!(!full_verification::<TestVKey>(
                p.proof,
                &p.public_inputs,
                &vkey
            ));
        }
    }

    #[test]
    fn test_verify_partial_too_many_calls() {
        let proof = valid_proofs()[0].proof;
        let public_inputs = valid_proofs()[0].public_inputs.clone();
        zero_program_account!(mut storage, VerificationAccount);
        setup_storage_account::<TestVKey>(&mut storage, proof, &public_inputs);
        let instruction_count = storage.get_prepare_inputs_instructions_count() as usize
            + COMBINED_MILLER_LOOP_IXS
            + FINAL_EXPONENTIATION_IXS;

        vkey!(vkey, TestVKey);

        for _ in 0..instruction_count {
            verify_partial(&mut storage, &vkey, COMPUTE_VERIFICATION_IX_COUNT - 1).unwrap();
        }

        // Additional ix will result in error
        assert_eq!(
            verify_partial(&mut storage, &vkey, COMPUTE_VERIFICATION_IX_COUNT - 1),
            Err(ElusivError::ComputationIsAlreadyFinished)
        );
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
        let mut f = f.cyclotomic_exp(Parameters::X);
        if !Parameters::X_IS_NEGATIVE {
            f.conjugate();
        }
        f
    }

    // https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/g2.rs#L139
    #[allow(clippy::op_ref)]
    fn reference_doubling_step(r: &mut G2HomProjective, two_inv: &Fq) -> (Fq2, Fq2, Fq2) {
        let mut a = r.x * &r.y;
        a.mul_assign_by_fp(two_inv);
        let b = r.y.square();
        let c = r.z.square();
        let e = COEFF_B * &(c.double() + &c);
        let f = e.double() + &e;
        let mut g = b + &f;
        g.mul_assign_by_fp(two_inv);
        let h = (r.y + &r.z).square() - &(b + &c);
        let i = e - &b;
        let j = r.x.square();
        let e_square = e.square();

        r.x = a * &(b - &f);
        r.y = g.square() - &(e_square.double() + &e_square);
        r.z = b * &h;
        (-h, j.double() + &j, i)
    }

    // https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/g2.rs#L168
    #[allow(clippy::op_ref)]
    fn reference_addition_step(r: &mut G2HomProjective, q: &G2Affine) -> (Fq2, Fq2, Fq2) {
        // Formula for line function when working with
        // homogeneous projective coordinates.
        let theta = r.y - &(q.y * &r.z);
        let lambda = r.x - &(q.x * &r.z);
        let c = theta.square();
        let d = lambda.square();
        let e = lambda * &d;
        let f = r.z * &c;
        let g = r.x * &d;
        let h = e + &f - &g.double();
        r.x = lambda * &h;
        r.y = theta * &(g - &h) - &(e * &r.y);
        r.z *= &e;
        let j = theta * &q.x - &(lambda * &q.y);

        (lambda, -theta, j)
    }
}
