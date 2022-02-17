use solana_program::entrypoint::ProgramResult;
use ark_bn254::{ Fq, Fq2, Fq12, G2Affine, Parameters };
use ark_ec::models::bn::BnParameters;
use ark_ff::*;
use super::super::scalar::*;
use super::super::state::ProofVerificationAccount;

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

pub const MILLER_LOOP_ITERATIONS: usize = 337;
const ITERATION_ROUNDS: [usize; MILLER_LOOP_ITERATIONS] = [
    8, 2, 2, 7, 4, 2, 7, 4, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 14, 8, 2, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 14, 8, 2, 2, 14, 8, 2, 2, 14, 8, 2, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 3, 6, 2, 2, 7, 4, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 14, 8, 2, 2, 14, 8, 2, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 3, 6, 2, 2, 7, 4, 2, 14, 8, 2, 2, 14, 8, 2, 2, 14, 8, 2, 2, 6, 5, 2, 1
];
const MAIN_ROUNDS: usize = 1664;
const FULL_ROUNDS: usize = ADDITION_ROUNDS + DOUBLING_ROUNDS + 2 * ELL_ROUNDS + 1;

/// Computes the `miller_value` (12 q field elements)
/// - requires `MILLER_LOOP_ITERATIONS` calls to complete
/// - Question: is it correct/allowed to assume that b can/should never be = zero? -> we assume b always != infinity
pub fn partial_miller_loop(
    account: &mut ProofVerificationAccount,
    iteration: usize
) -> ProgramResult {

    let base_round = account.get_round();
    let rounds = ITERATION_ROUNDS[iteration];

    // - pops: r (3 * Fq2)
    let mut r = G2HomProjective {
        x: account.pop_fq2(),
        y: account.pop_fq2(),
        z: account.pop_fq2(),
    };

    let mut b = read_g2_affine(&account.proof_b);
    let neg_b = read_g2_affine(&account.b_neg);

    for round in 0..rounds {
        let round = base_round + round;
        let i = round / FULL_ROUNDS;

        if i < 64 { // Main loop
            let round = round % FULL_ROUNDS;

            match round {
                0 => {
                    if i > 0 {  // ~ 87084 CUs
                        let mut miller_value = account.pop_fq12();
                        miller_value.square_in_place();
                        account.push_fq12(miller_value);
                    }
                },

                1..=7 => {  // DOUBLING_ROUNDS
                    doubling_round(account, &mut r, round - 1);
                },
                8..=13 => { // DOUBLING_ROUNDS + ELL_ROUNDS
                    ell_round(account, round - 8);
                },

                14..=19 => {    // ADDITION_ROUNDS
                    let bit = Parameters::ATE_LOOP_COUNT[63 - i];
                    if bit == 1 {
                        addition_round(account, &mut r, &b, round - 14);
                    } else if bit == -1 {
                        addition_round(account, &mut r, &neg_b, round - 14);
                    }
                },
                20..=25 => {
                    if Parameters::ATE_LOOP_COUNT[63 - i] != 0 {
                        ell_round(account, round - 20);
                    }
                },
                _ => {}
            }
        } else {    // Final two coefficients
            let round = round - MAIN_ROUNDS;

            match round {
                0 => {
                    b = mul_by_char(b);
                    write_g2_affine(&mut account.proof_b, b);
                },

                1..=6 => {  // ADDITION_ROUNDS
                    addition_round(account, &mut r, &b, round - 1);
                },
                7..=12 => { // ADDITION_ROUNDS + ELL_ROUNDS
                    ell_round(account, round - 7);
                },

                13 => {
                    b = mul_by_char(b);
                    b.y = -b.y;
                    write_g2_affine(&mut account.proof_b, b);
                },

                14..=19 => {
                    addition_round(account, &mut r, &b, round - 14);
                },
                20..=25 => {
                    ell_round(account, round - 20);
                },
                _ => {}
            }
        }
    }

    // push r again
    account.push_fq2(r.z);
    account.push_fq2(r.y);
    account.push_fq2(r.x);

    account.set_round(base_round + rounds);

    Ok(())
}

const ADDITION_ROUNDS: usize = 6;

/// Formula for line function when working with homogeneous projective coordinates
/// - CUs: [12673, 23173, 15199, 27102, 12907, 12661]
fn addition_round(
    account: &mut ProofVerificationAccount,
    r: &mut G2HomProjective,
    q: &G2Affine,
    round: usize,
) {
    match round {
        // - pushes: coeff1, lambda (Fq2)
        0 => { // compute lambda and store as 1st coeff element (~ 12673 CUs)
            let lambda = r.x - &(q.x * &r.z);

            // Push 1. coefficient and 1 placeholder
            account.push_fq2(lambda);

            // Push lambda
            account.push_fq2(lambda);
        },

        // - pops: lambda
        // - pushes: coeff2, lambda, theta (Fq2)
        1 => { // compute theta and store as 2nd coeff element (~ 12746 CUs)
            let theta = r.y - &(q.y * &r.z);
            let lambda = account.pop_fq2();

            account.push_fq2(-theta);
            account.push_fq2(lambda);
            account.push_fq2(theta);
        },

        // - pushes: e, g (Fq2)
        2 => { // e, g (~ 23173 CUs)
            let lambda = account.peek_fq2(1);

            let d = lambda.square();
            let e = lambda * &d;
            let g = r.x * &d;

            account.push_fq2(e);
            account.push_fq2(g);
        },

        // - pushes: h (Fq2)
        3 => { // c, h (~ 15199 CUs)
            let g = account.peek_fq2(0);
            let e = account.peek_fq2(1);
            let theta = account.peek_fq2(2);

            let c = theta.square();
            let f = r.z * &c;
            let h = e + &f - &g.double();

            account.push_fq2(h);
        },

        // - pops: h, g, e
        4 => { // Assign to r (~ 27102 CUs)
            let h = account.pop_fq2();
            let g = account.pop_fq2();
            let e = account.pop_fq2();
            let theta = account.peek_fq2(0);
            let lambda = account.peek_fq2(1);

            r.x = lambda * &h;
            r.y = theta * &(g - &h) - &(e * &r.y);
            r.z *= &e;
        },

        // - pops: theta, lambda
        // - pushes: coeff3
        5 => { // compute the last coeff element (~ 16506 CUs)
            let theta = account.pop_fq2();
            let lambda = account.pop_fq2();

            let j = theta * &q.x - &(lambda * &q.y);

            account.push_fq2(j);
        },
        _ => {}
    }
}

/// https://docs.rs/ark-bn254/0.3.0/src/ark_bn254/curves/g2.rs.html#19
/// COEFF_B = 3/(u+9) = (19485874751759354771024239261021720505790618469301721065564631296452457478373, 266929791119991161246907387137283842545076965332900288569378510910307636690)
const COEFF_B: Fq2 = field_new!(Fq2,
    field_new!(Fq, "19485874751759354771024239261021720505790618469301721065564631296452457478373"),
    field_new!(Fq, "266929791119991161246907387137283842545076965332900288569378510910307636690"),
);

const DOUBLING_ROUNDS: usize = 7;

/// Formula for line function when working with homogeneous projective coordinates
/// - CUs: [16767, 25817, 13078, 15379, 15070, 5567, 10000]
fn doubling_round(
    account: &mut ProofVerificationAccount,
    r: &mut G2HomProjective,
    round: usize,
) {
    match round {
        // - pushes: coeff1, coeff2, coeff3, e, c
        0 => { // c, e (~ 16767 CUs)
            let c = r.z.square();
            let e = COEFF_B * &(c.double() + &c);

            // Push coeff placeholders
            account.stack_fq2.push_empty();
            account.stack_fq2.push_empty();
            account.stack_fq2.push_empty();

            account.push_fq2(e);
            account.push_fq2(c);
        },

        // - pushes: b, f
        1 => { // b, f, d set 3. coeff element (~ 25817 CUs)
            let e = account.peek_fq2(1);

            let f = e.double() + &e;
            let b = r.y.square();
            let i = e - &b;
            
            // Set the 3. coeff
            account.stack_fq2.replace(2, i);

            account.push_fq2(b);
            account.push_fq2(f);
        },

        // - pushes: a (Fq2)
        2 => {  // a (~ 13078 CUs)
            let mut a = r.x * &r.y;
            a.mul_assign_by_fp(&TWO_INV);

            account.push_fq2(a);
        },

        // - pops: a
        3 => {  // set 2. coeff element & set r.x (~ 15379 CUs)
            let a = account.pop_fq2();
            let f = account.peek_fq2(0);
            let b = account.peek_fq2(1);

            let mut j = r.x.square();
            j = j.double() + &j;
            r.x = a * &(b - &f);

            // Set the 2. coeff
            account.stack_fq2.replace(5, j);
        },

        4 => {  // set 1. coeff element & assign r.z (~ 15070 CUs)
            let b = account.peek_fq2(1);
            let c = account.peek_fq2(2);

            let h = (r.y + &r.z).square() - &(b + &c);
            r.z = b * &h;

            // Set the 1. coeff
            account.stack_fq2.replace(6, -h);
        },

        // - pops: f, b
        // - pushes: g
        5 => {
            let f = account.pop_fq2();
            let b = account.pop_fq2();

            let mut g = b + &f;
            g.mul_assign_by_fp(&TWO_INV);
            g = g.square();

            account.push_fq2(g);
        },

        // - pops: g, c, e
        6 => {  // set r.y (~ 5567 CUs)
            let g = account.pop_fq2();
            account.stack_fq2.pop_empty();
            let e = account.pop_fq2();

            let e_square = e.square();

            r.y = g - &(e_square.double() + &e_square);
        },
        _ => {}
    }
}

const ELL_ROUNDS: usize = 6;

/// Evaluates the line function at point p
/// - CUs: [15000, 90000, 15000, 90000, 15000, 90000]
fn ell_round(
    account: &mut ProofVerificationAccount,
    round: usize,
) {
    let mut miller_value = account.pop_fq12();
    let coeff_ic = account.get_coeff_ic();

    match round {
        // - swaps: coeff1 and coeff3
        // - pops: coeff1, coeff2
        // - pushes: coeff2, coeff1
        0 => {  // Multiply `a` by first coeff values (~ 15000 CUs CUs)
            // Swap coeff1 and coeff3
            account.stack_fq2.swap(0, 2);

            let a = read_g1_affine(account.proof_a);
            let mut c0 = account.pop_fq2();
            c0.mul_assign_by_fp(&a.y);
            let mut c1 = account.pop_fq2();
            c1.mul_assign_by_fp(&a.x);

            account.push_fq2(c1);
            account.push_fq2(c0);
        },

        // - pops: coeff1, coeff2, coeff3
        1 => {  // (~ 89310 CUs)
            miller_value.mul_by_034(
                &account.pop_fq2(),
                &account.pop_fq2(),
                &account.pop_fq2(),
            );
        },

        // - pushes: c1, c0
        2 => {  // Multiply `p_inputs` by second coeff values (~ 15000 CUs)
            let p_inputs = account.get_prepared_inputs();
            let mut c0 = super::gamma_g2_neg_pc(coeff_ic).0;
            c0.mul_assign_by_fp(&p_inputs.y);

            let mut c1 = super::gamma_g2_neg_pc(coeff_ic).1;
            c1.mul_assign_by_fp(&p_inputs.x);

            account.push_fq2(c1);
            account.push_fq2(c0);
        },

        // - pops: c0, c1
        3 => {  // (~ 89985 CUs)
            miller_value.mul_by_034(
                &account.pop_fq2(),
                &account.pop_fq2(),
                &super::gamma_g2_neg_pc(coeff_ic).2,
            );
        },

        // - pushes: c1, c0
        4 => {  // Multiply `c` by third coeff values (~ 15000 CUs)
            let c = read_g1_affine(account.proof_c);
            let mut c0 = super::delta_g2_neg_pc(coeff_ic).0;
            c0.mul_assign_by_fp(&c.y);

            let c = read_g1_affine(account.proof_c);
            let mut c1 = super::delta_g2_neg_pc(coeff_ic).1;
            c1.mul_assign_by_fp(&c.x);

            account.push_fq2(c1);
            account.push_fq2(c0);
        },

        // - pops: c0, c1
        5 => {  // (~ 90117 CUs)
            miller_value.mul_by_034(
                &account.pop_fq2(),
                &account.pop_fq2(),
                &super::delta_g2_neg_pc(coeff_ic).2,
            );

            account.inc_coeff_ic();
        },
        _ => {}
    }

    account.push_fq12(miller_value);
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
    use ark_bn254::{ Bn254, G1Affine, G2Affine };
    use ark_ec::models::bn::{ TwistType };
    use std::str::FromStr;
    use ark_ec::PairingEngine;
    use core::ops::Neg;
    use super::super::vkey::*;
    use super::super::{ PREPARE_INPUTS_ITERATIONS, partial_prepare_inputs };

    #[test]
    fn test_addition_rounds() {
        let b = get_b();
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();

        let mut r1 = G2HomProjective { x: b.x, y: b.y, z: Fq2::one() };
        let mut r2 = G2HomProjective { x: b.x, y: b.y, z: Fq2::one() };
        doubling_step_original(&mut r1, &TWO_INV);
        doubling_step_original(&mut r2, &TWO_INV);

        // Computation
        for round in 0..ADDITION_ROUNDS {
            addition_round(
                &mut account,
                &mut r1,
                &b,
                round,
            );
        }
        let c3 = account.pop_fq2();
        let c2 = account.pop_fq2();
        let c1 = account.pop_fq2();
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
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();

        let mut r1 = G2HomProjective { x: b.x, y: b.y, z: Fq2::one() };
        let mut r2 = G2HomProjective { x: b.x, y: b.y, z: Fq2::one() };

        // Computation
        for round in 0..DOUBLING_ROUNDS {
            doubling_round(
                &mut account,
                &mut r1,
                round,
            );
        }
        let c3 = account.pop_fq2();
        let c2 = account.pop_fq2();
        let c1 = account.pop_fq2();
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
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = init_account(&mut data);

        let b = get_b();
        let mut r = G2HomProjective { x: b.x, y: b.y, z: Fq2::one() };
        let b_coeffs = doubling_step_original(&mut r, &TWO_INV);
        for i in 0..PREPARE_INPUTS_ITERATIONS { partial_prepare_inputs(&mut account, i).unwrap(); }

        // Add coefficients
        account.push_fq2(b_coeffs.0);
        account.push_fq2(b_coeffs.1);
        account.push_fq2(b_coeffs.2);

        // ell computation
        for round in 0..ELL_ROUNDS { ell_round(&mut account, round); }
        let result = account.pop_fq12();

        // Original
        let mut miller = Fq12::one();
        ell_original(&mut miller, b_coeffs, &get_a());
        let p_inputs = account.get_prepared_inputs();
        ell_original(&mut miller, gamma_g2_neg_pc(0), &p_inputs);
        ell_original(&mut miller, delta_g2_neg_pc(0), &get_c());

        assert_eq!(result, miller);
    }

    #[test]
    fn test_miller() {
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = init_account(&mut data);

        // Computation
        for i in 0..PREPARE_INPUTS_ITERATIONS { partial_prepare_inputs(&mut account, i).unwrap(); }
        let p_inputs = account.get_prepared_inputs();
        account.set_round(0);
        for iteration in 0..MILLER_LOOP_ITERATIONS {
            partial_miller_loop(&mut account, iteration).unwrap();
        }
        let result = account.pop_fq12();

        // Original
        let miller = Bn254::miller_loop(
            [
                ( get_a().into(), get_b().into() ),
                ( p_inputs.into(), gamma_g2().neg().into() ),
                ( get_c().into(), delta_g2().neg().into() ),
            ]
            .iter(),
        );

        assert_eq!(result, miller);
    }

    /// Stack convention:
    /// - every private function has to clear the local stack
    /// - public functions are allowed to return values on the stack
    fn assert_stack_is_cleared(account: &ProofVerificationAccount) {
        assert_eq!(account.stack_fq.stack_pointer, 0);
        assert_eq!(account.stack_fq2.stack_pointer, 0);
        assert_eq!(account.stack_fq6.stack_pointer, 0);
        assert_eq!(account.stack_fq12.stack_pointer, 0);
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

    fn ell_original(f: &mut Fq12, coeffs: (Fq2, Fq2, Fq2), p: &G1Affine) {
        let mut c0 = coeffs.0;
        let mut c1 = coeffs.1;
        let c2 = coeffs.2;
    
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

    fn init_account<'a>(data: &'a mut [u8]) -> ProofVerificationAccount<'a> {
        let mut account = ProofVerificationAccount::from_data(data).unwrap();
        let inputs = [
            from_str_10("20643720223837027367320733428836459266646763523911772324593310161284187566894"),
            from_str_10("19526707366532583397322534596786476145393586591811230548888354920504818678603"),
        ];
        account.init(vec!
            [
                vec_to_array_32(to_bytes_le_repr(inputs[0])),
                vec_to_array_32(to_bytes_le_repr(inputs[1]))
            ],
            0, [0,0,0,0],
            super::super::Proof{ a: get_a(), b: get_b(), c: get_c() }
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