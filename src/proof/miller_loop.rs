use solana_program::entrypoint::ProgramResult;
use ark_bn254::{ Fq, Fq2, G2Affine, Parameters };
use ark_ec::models::bn::BnParameters;
use ark_ff::*;
use super::{state::ProofAccount, VerificationKey};

// TODO: mul_by_034, square_in_place

/// Inverse of 2 (in q)
/// - Calculated using: Fq::one().double().inverse().unwrap()
pub const TWO_INV: Fq = Fq::new(BigInteger256::new([9781510331150239090, 15059239858463337189, 10331104244869713732, 2249375503248834476]));

pub type EllCoeff = (Fq2, Fq2, Fq2);

#[derive(Debug)]
struct G2HomProjective {
    pub x: Fq2,
    pub y: Fq2,
    pub z: Fq2,
}

pub const MILLER_LOOP_ITERATIONS: usize = 43;
const ITERATION_ROUNDS: [usize; MILLER_LOOP_ITERATIONS] = [18, 24, 23, 24, 23, 16, 23, 30, 17, 30, 17, 17, 30, 23, 30, 30, 16, 30, 17, 30, 17, 17, 23, 30, 23, 23, 17, 30, 17, 24, 16, 30, 16, 30, 17, 17, 30, 17, 23, 23, 23, 32, 13];
const MAIN_ROUNDS: usize = 960;
const FULL_ROUNDS: usize = 1 + 1 + 2 * ELL_ROUNDS + 1;

/// Computes the `miller_value` (12 q field elements)
/// - requires `MILLER_LOOP_ITERATIONS` calls to complete
/// - Question: is it correct/allowed to assume that b can/should never be = zero? -> we assume b always != infinity
pub fn partial_miller_loop<VKey: VerificationKey>(
    account: &mut ProofAccount,
    iteration: usize
) -> ProgramResult {

    let base_round = account.get_round() as usize;
    let rounds = ITERATION_ROUNDS[iteration];

    // - pops: r (3 * Fq2)
    let mut r = G2HomProjective {
        x: account.fq2.pop(),
        y: account.fq2.pop(),
        z: account.fq2.pop(),
    };

    let mut b: G2Affine = account.get_proof_b();
    let neg_b: G2Affine = account.get_proof_b_neg();

    for round in 0..rounds {
        let round = base_round + round;
        let i = round / FULL_ROUNDS;

        if i < 64 { // Main loop
            let round = round % FULL_ROUNDS;

            match round {
                0 => {
                    if i > 0 {  // (CUs: Max: 91923 Avg: 89720 Min: 86998 )
                        let mut miller_value = account.fq12.pop();
                        miller_value.square_in_place();
                        account.fq12.push(miller_value);
                    }
                },

                1 => {
                    doubling_round(account, &mut r);
                },
                2..=7 => { // 6
                    ell_round::<VKey>(account, round - 2);
                },

                8 => {
                    let bit = Parameters::ATE_LOOP_COUNT[63 - i];
                    if bit == 1 {
                        addition_round(account, &mut r, &b);
                    } else if bit == -1 {
                        addition_round(account, &mut r, &neg_b);
                    }
                },
                9..=14 => {
                    if Parameters::ATE_LOOP_COUNT[63 - i] != 0 {
                        ell_round::<VKey>(account, round - 9);
                    }
                },
                _ => {}
            }
        } else {    // Final two coefficients
            let round = round - MAIN_ROUNDS;

            match round {
                0 => {
                    b = mul_by_char(b);
                    account.set_proof_b(b);
                },

                1 => {
                    addition_round(account, &mut r, &b);
                },
                2..=7 => { // ADDITION_ROUNDS + ELL_ROUNDS
                    ell_round::<VKey>(account, round - 7);
                },

                8 => {
                    b = mul_by_char(b);
                    b.y = -b.y;
                    account.set_proof_b(b);
                },

                9 => {
                    addition_round(account, &mut r, &b);
                },
                10..=15 => {
                    ell_round::<VKey>(account, round - 10);
                },
                _ => {}
            }
        }
    }

    // push r again
    account.fq2.push(r.z);
    account.fq2.push(r.y);
    account.fq2.push(r.x);

    account.set_round((base_round + rounds) as u64);

    Ok(())
}

/// Formula for line function when working with homogeneous projective coordinates
fn addition_round(
    account: &mut ProofAccount,
    r: &mut G2HomProjective,
    q: &G2Affine,
) {
    let lambda = r.x - &(q.x * &r.z);
    let theta = r.y - &(q.y * &r.z);
    let d = lambda.square();
    let e = lambda * &d;
    let g = r.x * &d;
    let c = theta.square();
    let f = r.z * &c;
    let h = e + &f - &g.double();

    r.x = lambda * &h;
    r.y = theta * &(g - &h) - &(e * &r.y);
    r.z *= &e;

    let j = theta * &q.x - &(lambda * &q.y);

    // Push coefficients
    account.fq2.push(lambda);
    account.fq2.push(-theta);
    account.fq2.push(j);
}

/// https://docs.rs/ark-bn254/0.3.0/src/ark_bn254/curves/g2.rs.html#19
/// COEFF_B = 3/(u+9) = (19485874751759354771024239261021720505790618469301721065564631296452457478373, 266929791119991161246907387137283842545076965332900288569378510910307636690)
const COEFF_B: Fq2 = field_new!(Fq2,
    field_new!(Fq, "19485874751759354771024239261021720505790618469301721065564631296452457478373"),
    field_new!(Fq, "266929791119991161246907387137283842545076965332900288569378510910307636690"),
);

/// Formula for line function when working with homogeneous projective coordinates
/// 68000
fn doubling_round(
    account: &mut ProofAccount,
    r: &mut G2HomProjective,
) {
    let c = r.z.square();
    let e = COEFF_B * &(c.double() + &c);

    let f = e.double() + &e;
    let b = r.y.square();
    let i = e - &b;
    
    let mut a = r.x * &r.y;
    a.mul_assign_by_fp(&TWO_INV);

    let mut j = r.x.square();
    j = j.double() + &j;
    r.x = a * &(b - &f);

    let h = (r.y + &r.z).square() - &(b + &c);
    r.z = b * &h;

    let mut g = b + &f;
    g.mul_assign_by_fp(&TWO_INV);
    g = g.square();

    let e_square = e.square();

    r.y = g - &(e_square.double() + &e_square);

    // Push coefficients
    account.fq2.push(-h);
    account.fq2.push(j);
    account.fq2.push(i);
}

const ELL_ROUNDS: usize = 6;

/// Evaluates the line function at point p
/// - CUs: [11677, 92056, 10550, 92091, 10147, 91988]
fn ell_round<VKey: VerificationKey>(
    account: &mut ProofAccount,
    round: usize,
) {
    let mut miller_value = account.fq12.pop();
    let coeff_ic = account.get_current_coeff() as usize;

    match round {
        // - swaps: coeff1 and coeff3
        // - pops: coeff1, coeff2
        // - pushes: coeff2, coeff1
        0 => {  // Multiply `a` by first coeff values (CUs: Max: 11677 Avg: 11234 Min: 9514)
            // Swap coeff1 and coeff3
            account.fq2.swap(0, 2);

            let a = account.get_proof_a();
            let mut c0 = account.fq2.pop();
            c0.mul_assign_by_fp(&a.y);
            let mut c1 = account.fq2.pop();
            c1.mul_assign_by_fp(&a.x);

            account.fq2.push(c1);
            account.fq2.push(c0);
        },

        // - pops: coeff1, coeff2, coeff3
        1 => {  // (CUs: Max: 92056 Avg: 90300 Min: 88569)
            miller_value.mul_by_034(
                &account.fq2.pop(),
                &account.fq2.pop(),
                &account.fq2.pop(),
            );
        },

        // - pushes: c1, c0
        2 => {  // Multiply `p_inputs` by second coeff values(CUs: Max: 10550 Avg: 10482 Min: 10462)
            let p_inputs = account.get_prepared_inputs();
            let mut c0 = VKey::gamma_g2_neg_pc(coeff_ic).0;
            c0.mul_assign_by_fp(&p_inputs.y);

            let mut c1 = VKey::gamma_g2_neg_pc(coeff_ic).1;
            c1.mul_assign_by_fp(&p_inputs.x);

            account.fq2.push(c1);
            account.fq2.push(c0);
        },

        // - pops: c0, c1
        3 => {  // (CUs: Max: 92091 Avg: 90528 Min: 89564)
            miller_value.mul_by_034(
                &account.fq2.pop(),
                &account.fq2.pop(),
                &VKey::gamma_g2_neg_pc(coeff_ic).2,
            );
        },

        // - pushes: c1, c0
        4 => {  // Multiply `c` by third coeff values (CUs: Max: 10147 Avg: 10117 Min: 10102)
            let c = account.get_proof_c();
            let mut c0 = VKey::delta_g2_neg_pc(coeff_ic).0;
            c0.mul_assign_by_fp(&c.y);

            let c = account.get_proof_c();
            let mut c1 = VKey::delta_g2_neg_pc(coeff_ic).1;
            c1.mul_assign_by_fp(&c.x);

            account.fq2.push(c1);
            account.fq2.push(c0);
        },

        // - pops: c0, c1
        5 => {  // (CUs: Max: 91988 Avg: 90576 Min: 89615 )
            miller_value.mul_by_034(
                &account.fq2.pop(),
                &account.fq2.pop(),
                &VKey::delta_g2_neg_pc(coeff_ic).2,
            );

            account.set_current_coeff((coeff_ic + 1) as u64);
        },
        _ => {}
    }

    account.fq12.push(miller_value);
}

/// Multiply by field characteristic
fn mul_by_char(r: G2Affine) -> G2Affine {
    let mut s = r;
    s.x.frobenius_map(1);
    s.x *= &Parameters::TWIST_MUL_BY_Q_X;
    s.y.frobenius_map(1);
    s.y *= &Parameters::TWIST_MUL_BY_Q_Y;

    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::{ Fq12, Bn254, G1Affine, G2Affine };
    use ark_ec::models::bn::{ TwistType };
    use std::str::FromStr;
    use ark_ec::PairingEngine;
    use core::ops::Neg;
    use super::super::{ partial_prepare_inputs };
    use crate::fields::scalar::*;
    use crate::fields::utils::*;

    type VKey = crate::proof::vkey::SendVerificationKey;

    #[test]
    fn test_addition_rounds() {
        let b = get_b();
        let mut data = vec![0; ProofAccount::TOTAL_SIZE];
        let mut account = ProofAccount::from_data(&mut data).unwrap();

        let mut r1 = G2HomProjective { x: b.x, y: b.y, z: Fq2::one() };
        let mut r2 = G2HomProjective { x: b.x, y: b.y, z: Fq2::one() };
        doubling_step_original(&mut r1, &TWO_INV);
        doubling_step_original(&mut r2, &TWO_INV);

        // Computation
        addition_round(&mut account, &mut r1, &b);
        let c3 = account.fq2.pop();
        let c2 = account.fq2.pop();
        let c1 = account.fq2.pop();
        let result = (c1, c2, c3);

        // Original
        let expected = addition_step_original(&mut r2, &b);

        assert_eq!(result, expected);
        assert_eq!(r1.x, r2.x);
        assert_eq!(r1.y, r2.y);
        assert_eq!(r1.z, r2.z);
        assert_stack_is_cleared(&account);
    }

    #[test]
    fn test_doubling_rounds() {
        let b = get_b();
        let mut data = vec![0; ProofAccount::TOTAL_SIZE];
        let mut account = ProofAccount::from_data(&mut data).unwrap();

        let mut r1 = G2HomProjective { x: b.x, y: b.y, z: Fq2::one() };
        let mut r2 = G2HomProjective { x: b.x, y: b.y, z: Fq2::one() };

        // Computation
        doubling_round(&mut account, &mut r1);
        let c3 = account.fq2.pop();
        let c2 = account.fq2.pop();
        let c1 = account.fq2.pop();
        let result = (c1, c2, c3);

        // Original
        let expected = doubling_step_original(&mut r2, &TWO_INV);

        assert_eq!(result, expected);
        assert_eq!(r1.x, r2.x);
        assert_eq!(r1.y, r2.y);
        assert_eq!(r1.z, r2.z);
        assert_stack_is_cleared(&account);
    }

    #[test]
    fn test_ell_rounds() {
        let mut data = vec![0; ProofAccount::TOTAL_SIZE];
        let mut account = init_account(&mut data);

        let b = get_b();
        let mut r = G2HomProjective { x: b.x, y: b.y, z: Fq2::one() };
        let b_coeffs = doubling_step_original(&mut r, &TWO_INV);
        for i in 0..VKey::PREPARE_INPUTS_ITERATIONS { partial_prepare_inputs::<VKey>(&mut account, i).unwrap(); }

        // Add coefficients
        account.fq2.push(b_coeffs.0);
        account.fq2.push(b_coeffs.1);
        account.fq2.push(b_coeffs.2);

        // ell computation
        for round in 0..ELL_ROUNDS { ell_round::<VKey>(&mut account, round); }
        let result = account.fq12.pop();

        // Original
        let mut miller = Fq12::one();
        ell_original(&mut miller, b_coeffs, &get_a());
        let p_inputs = account.get_prepared_inputs();
        ell_original(&mut miller, VKey::gamma_g2_neg_pc(0), &p_inputs);
        ell_original(&mut miller, VKey::delta_g2_neg_pc(0), &get_c());

        assert_eq!(result, miller);
    }

    #[test]
    fn test_miller() {
        let mut data = vec![0; ProofAccount::TOTAL_SIZE];
        let mut account = init_account(&mut data);

        // Computation
        for i in 0..VKey::PREPARE_INPUTS_ITERATIONS { partial_prepare_inputs::<VKey>(&mut account, i).unwrap(); }
        let p_inputs = account.get_prepared_inputs();
        account.set_round(0);
        for iteration in 0..MILLER_LOOP_ITERATIONS {
            partial_miller_loop::<VKey>(&mut account, iteration).unwrap();
        }
        let result = account.fq12.pop();

        // Original
        let miller = Bn254::miller_loop(
            [
                ( get_a().into(), get_b().into() ),
                ( p_inputs.into(), VKey::gamma_g2().neg().into() ),
                ( get_c().into(), VKey::delta_g2().neg().into() ),
            ]
            .iter(),
        );

        assert_eq!(result, miller);
    }

    /// Stack convention:
    /// - every private function has to clear the local stack
    /// - public functions are allowed to return values on the stack
    fn assert_stack_is_cleared(account: &ProofAccount) {
        assert_eq!(account.fq.stack_pointer, 0);
        assert_eq!(account.fq2.stack_pointer, 0);
        assert_eq!(account.fq6.stack_pointer, 0);
        assert_eq!(account.fq12.stack_pointer, 0);
    }

    fn addition_step_original(r: &mut G2HomProjective, q: &G2Affine) -> EllCoeff {
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

        match Parameters::TWIST_TYPE {
            TwistType::M => (j, -theta, lambda),
            TwistType::D => (lambda, -theta, j),
        }
    }

    fn doubling_step_original(r: &mut G2HomProjective, two_inv: &Fq) -> EllCoeff {
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
        match Parameters::TWIST_TYPE {
            TwistType::M => (i, j.double() + &j, -h),
            TwistType::D => (-h, j.double() + &j, i),
        }
    }

    // https://github.com/arkworks-rs/algebra/blob/6ea310ef09f8b7510ce947490919ea6229bbecd6/ec/src/models/bn/mod.rs#L59
    fn ell_original(f: &mut Fq12, coeffs: (Fq2, Fq2, Fq2), p: &G1Affine) {
        let mut c0: Fq2 = coeffs.0;
        let mut c1: Fq2 = coeffs.1;
        let c2: Fq2 = coeffs.2;
    
        c0.mul_assign_by_fp(&p.y);
        c1.mul_assign_by_fp(&p.x);
        f.mul_by_034(&c0, &c1, &c2);
    }

    fn get_a() -> G1Affine {
        G1Affine::new(
            Fq::from_str("20925091368075991963132407952916453596237117852799702412141988931506241672722").unwrap(),
            Fq::from_str("6039012589018526855429190661364232506642511499289558287989232491174672020857").unwrap(),
            false
        )
    }

    fn get_b() -> G2Affine {
        G2Affine::new(
            Fq2::new(
                Fq::from_str("20925091368075991963132407952916453596237117852799702412141988931506241672722").unwrap(),
                Fq::from_str("6039012589018526855429190661364232506642511499289558287989232491174672020857").unwrap(),
            ),
            Fq2::new(
                Fq::from_str("18684276579894497974780190092329868933855710870485375969907530111657029892231").unwrap(),
                Fq::from_str("5932690455294482368858352783906317764044134926538780366070347507990829997699").unwrap(),
            ),
            false
        )
    }

    fn get_c() -> G1Affine {
        G1Affine::new(
            Fq::from_str("20925091368075991963132407952916453596237117852799702412141988931506241672722").unwrap(),
            Fq::from_str("6039012589018526855429190661364232506642511499289558287989232491174672020857").unwrap(),
            false
        )
    }

    fn init_account<'a>(data: &'a mut [u8]) -> ProofAccount<'a> {
        let mut account = ProofAccount::from_data(data).unwrap();
        let inputs = [
            from_str_10("20643720223837027367320733428836459266646763523911772324593310161284187566894"),
            from_str_10("19526707366532583397322534596786476145393586591811230548888354920504818678603"),
        ];
        account.reset::<VKey>(
            super::super::Proof{ a: get_a(), b: get_b(), c: get_c() },
            &[
                vec_to_array_32(to_bytes_le_repr(inputs[0])),
                vec_to_array_32(to_bytes_le_repr(inputs[1])),
            ],
        ).unwrap();

        /*assert_eq!(read_g2_affine(&account.proof_b), get_b());
        assert_eq!(read_g2_affine(&account.b_neg), -get_b());
        assert_eq!(read_fq2_le_montgomery(&account.b_homo_r[..64]), get_b().x);
        assert_eq!(read_fq2_le_montgomery(&account.b_homo_r[64..128]), get_b().y);
        assert_eq!(read_fq2_le_montgomery(&account.b_homo_r[128..]), Fq2::one());
        assert_eq!(read_g1_affine(&account.proof_a), get_a());
        assert_eq!(read_g1_affine(&account.proof_c), get_c());*/

        account
    }
}