use solana_program::entrypoint::ProgramResult;
use ark_bn254::{ Fq, Fq2, Fq6, Fq12, G2Affine, Parameters };
use ark_ec::models::bn::BnParameters;
use ark_ff::*;
use super::super::scalar::*;
use super::super::state::ProofVerificationAccount;

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

pub const MILLER_LOOP_ITERATIONS: usize = 123;
const ITERATION_ROUNDS: [usize; MILLER_LOOP_ITERATIONS] = [18, 17, 12, 24, 9, 17, 14, 24, 25, 15, 8, 17, 25, 14, 8, 17, 10, 17, 12, 24, 25, 15, 24, 9, 17, 14, 24, 25, 15, 8, 17, 10, 17, 12, 24, 25, 15, 8, 17, 25, 14, 24, 25, 15, 24, 25, 15, 8, 17, 25, 14, 24, 9, 17, 14, 24, 25, 15, 8, 17, 10, 17, 12, 8, 17, 25, 14, 24, 25, 15, 24, 9, 17, 14, 24, 9, 17, 14, 24, 25, 15, 8, 17, 25, 14, 8, 17, 10, 17, 12, 24, 25, 15, 8, 17, 25, 14, 24, 9, 17, 14, 8, 17, 25, 14, 24, 9, 17, 14, 24, 9, 17, 14, 24, 9, 17, 14, 24, 25, 15, 23, 16, 10];
const MAIN_ROUNDS: usize = 2048;
const FULL_ROUNDS: usize = ADDITION_ROUNDS + DOUBLING_ROUNDS + 2 * ELL_ROUNDS + 2;

/// Computes the `miller_value` (12 q field elements)
/// - requires `MILLER_LOOP_ITERATIONS` calls to complete
/// - Question: is it correct/allowed to assume that b can/should never be = zero? -> we assume b always != infinity
pub fn partial_miller_loop(
    account: &mut ProofVerificationAccount,
    iteration: usize
) -> ProgramResult {

    let base_round = account.get_current_round();
    let rounds = ITERATION_ROUNDS[iteration];

    let mut r = G2HomProjective {
        x: read_fq2_le_montgomery(&account.b_homo_r[..64]),
        y: read_fq2_le_montgomery(&account.b_homo_r[64..128]),
        z: read_fq2_le_montgomery(&account.b_homo_r[128..]),
    };

    let mut b = read_g2_affine(&account.proof_b);
    let neg_b = read_g2_affine(&account.b_neg);

    for round in 0..rounds {
        let round = base_round + round;
        let i = round / FULL_ROUNDS;

        if i < 64 { // Main loop (64 + 25 coefficients)
            let round = round % FULL_ROUNDS;
            if round == 0 {
                if i > 0 {
                    let mut miller_value = read_miller_value(account);    
                    miller_value.square_in_place();
                    write_miller_value(account, miller_value);
                }
            } else
            if round < 7 {
                doubling_round(account, &mut r, round - 1);
            } else
            if round < 7 + 9 {
                ell_round(account, round - 7);
            } else
            if round < 7 + 9 + 6 {
                let bit = Parameters::ATE_LOOP_COUNT[63 - i];
                if bit == 1 {
                    addition_round(account, &mut r, &b, round - 16);
                } else if bit == -1 {
                    addition_round(account, &mut r, &neg_b, round - 16);
                }
            } else
            if round < 7 + 9 + 6 + 9 {
                if Parameters::ATE_LOOP_COUNT[63 - i] != 0 {
                    ell_round(account, round - 22);
                }
            }
        } else {    // Final two coefficients
            let round = round - MAIN_ROUNDS;
            if round == 0 {
                b = mul_by_char(b);
                write_g2_affine(&mut account.proof_b, b);
            } else
            if round < 7 {
                addition_round(account, &mut r, &b, round - 1);
            } else
            if round < 7 + 9 {
                ell_round(account, round - 7);
            } else
            if round == 7 + 9 {
                b = mul_by_char(b);
                b.y = -b.y;
                write_g2_affine(&mut account.proof_b, b);
            } else
            if round < 7 + 9 + 1 + 6 {
                addition_round(account, &mut r, &b, round - 17);
            } else
            if round < 7 + 9 + 1 + 6 + 9 {
                ell_round(account, round - 23);
            }
        }
    }

    // save r again
    write_fq2(&mut account.b_homo_r[..64], r.x);
    write_fq2(&mut account.b_homo_r[64..128], r.y);
    write_fq2(&mut account.b_homo_r[128..], r.z);

    account.set_current_round(base_round + rounds);

    Ok(())
}

const ADDITION_ROUNDS: usize = 6;

const COEFF_OFFSET: usize = 0;
const MILLER_OFFSET: usize = 6;

const A_OFFSET: usize = 18;
const LAMBDA_OFFSET: usize = A_OFFSET;

const B_OFFSET: usize = 20;
const THETHA_OFFSET: usize = B_OFFSET;

const C_OFFSET: usize = 22;
const E_OFFSET: usize = 24;
const F_OFFSET: usize = 26;
const G_OFFSET: usize = 28;
const H_OFFSET: usize = 30;

// ### RAM usage:
// - coeff (6 words)
//
// - miller_value (12 words)
//
// - a / lambda (2 words)
// - b / thetha (2 words)
// - c (2 words)
// - e (2 words)
// - f (2 words)
// - g (2 words)
// - h (2 words)

/// Formula for line function when working with homogeneous projective coordinates
/// - CUs: [6500, 6500, 15000, 11000, 24000, 13000]
fn addition_round(
    account: &mut ProofVerificationAccount,
    r: &mut G2HomProjective,
    q: &G2Affine,
    round: usize,
) {
    match round {
        0 => { // compute lambda and store as 1st coeff element (~ 6500 CUs)
            let lambda = r.x - &(q.x * &r.z);   // 6531

            write_fq2(account.get_ram_mut(LAMBDA_OFFSET, 2), lambda);
            set_coeff_element(account, 0, lambda);
        },
        1 => { // compute theta and store as 2nd coeff element (~ 6500 CUs)
            let theta = r.y - &(q.y * &r.z);    // 6541

            write_fq2(account.get_ram_mut(THETHA_OFFSET, 2), theta);
            set_coeff_element(account, 1, -theta);
        },
        2 => { // e, g (~ 15000 CUs)
            let lambda = read_fq2(account.get_ram(LAMBDA_OFFSET, 2));

            let d = lambda.square();    // 4113
            let e = lambda * &d;    // 6230
            let g = r.x * &d;   // 6229

            write_fq2(account.get_ram_mut(E_OFFSET, 2), e);
            write_fq2(account.get_ram_mut(G_OFFSET, 2), g);
        },
        3 => { // c, h (~ 11000 CUs)
            let theta = read_fq2(account.get_ram(THETHA_OFFSET, 2));
            let e = read_fq2(account.get_ram(E_OFFSET, 2));
            let g = read_fq2(account.get_ram(G_OFFSET, 2));

            let c = theta.square(); // 4132
            let f = r.z * &c;   // 6241
            let h = e + &f - &g.double();   // 654

            write_fq2(account.get_ram_mut(H_OFFSET, 2), h);
        },
        4 => { // Assign to r (~ 24000 CUs)
            let theta = read_fq2(account.get_ram(THETHA_OFFSET, 2));
            let lambda = read_fq2(account.get_ram(LAMBDA_OFFSET, 2));
            let e = read_fq2(account.get_ram(E_OFFSET, 2));
            let g = read_fq2(account.get_ram(G_OFFSET, 2));
            let h = read_fq2(account.get_ram(H_OFFSET, 2));

            r.x = lambda * &h;  // 6238
            r.y = theta * &(g - &h) - &(e * &r.y);  // 12907
            r.z *= &e;  // 6055
        },
        5 => { // compute the last coeff element (~ 13000 CUs)
            let theta = read_fq2(account.get_ram(THETHA_OFFSET, 2));
            let lambda = read_fq2(account.get_ram(LAMBDA_OFFSET, 2));

            let j = theta * &q.x - &(lambda * &q.y);    // 12661

            set_coeff_element(account, 2, j);
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

const DOUBLING_ROUNDS: usize = 6;

/// Formula for line function when working with homogeneous projective coordinates
/// - CUs: [10000, 11000, 8000, 18000, 11000, 15000]
fn doubling_round(
    account: &mut ProofVerificationAccount,
    r: &mut G2HomProjective,
    round: usize,
) {
    match round {
        0 => {  // a (~ 10000 CUs)
            let mut a = r.x * &r.y; // 6272 + 3780
            a.mul_assign_by_fp(&TWO_INV);

            write_fq2(account.get_ram_mut(A_OFFSET, 2), a);
        },
        1 => { // c, e (~ 11000 CUs)
            let c = r.z.square();   // 4159
            let e = COEFF_B * &(c.double() + &c);   // 6541

            write_fq2(account.get_ram_mut(C_OFFSET, 2), c);
            write_fq2(account.get_ram_mut(E_OFFSET, 2), e);
        },
        2 => { // b, f, d set 3. coeff element (~ 8000 CUs)
            let e = read_fq2(account.get_ram(E_OFFSET, 2));

            let f = e.double() + &e; // 325 + 4325 + 364
            let b = r.y.square();
            let i = e - &b;

            let mut g = b + &f; // 278 + 3773 + k
            g.mul_assign_by_fp(&TWO_INV);
            g = g.square();

            write_fq2(account.get_ram_mut(B_OFFSET, 2), b);
            write_fq2(account.get_ram_mut(F_OFFSET, 2), f);
            write_fq2(account.get_ram_mut(G_OFFSET, 2), g);

            set_coeff_element(account, 2, i);
        },
        3 => {  // set 2. coeff element & set r.x (~ 18000 CUs)
            let a = read_fq2(account.get_ram(A_OFFSET, 2));
            let b = read_fq2(account.get_ram(B_OFFSET, 2));
            let f = read_fq2(account.get_ram(F_OFFSET, 2));

            let mut j = r.x.square();
            j = j.double() + &j;
            r.x = a * &(b - &f);    // 6499

            set_coeff_element(account, 1, j);
        },
        4 => {  // set 1. coeff element & assign r.z (~ 11000 CUs)
            let b = read_fq2(account.get_ram(B_OFFSET, 2));
            let c = read_fq2(account.get_ram(C_OFFSET, 2));

            let h = (r.y + &r.z).square() - &(b + &c);  // 4945 + 6215 + x
            r.z = b * &h;

            set_coeff_element(account, 0, -h);
        },
        5 => {  // set r.y (~ 15000 CUs)
            let e = read_fq2(account.get_ram(E_OFFSET, 2));
            let g = read_fq2(account.get_ram(G_OFFSET, 2));

            let e_square = e.square();  // 4157

            r.y = g - &(e_square.double() + &e_square);    // 4769 - k
        },
        _ => {}
    }
}

const ELL_ROUNDS: usize = 9;

/// Evaluates the line function at point p
/// - CUs: [11000, 11000, 11000, 11000, 11000, 11000, 11000, 11000, 11000]
fn ell_round(
    account: &mut ProofVerificationAccount,
    round: usize,
) {
    let mut miller_value = read_miller_value(account);
    let coeff_ic = account.get_coeff_ic();

    match round {
        // Multiply `a` by first coeff values
        0 => {
            let a = read_g1_affine(account.proof_a);
            let mut c0 = get_coeff_element(account, 0);
            c0.mul_assign_by_fp(&a.y);

            set_coeff_element(account, 0, c0);
        },
        1 => {
            let a = read_g1_affine(account.proof_a);
            let mut c1 = get_coeff_element(account, 1);
            c1.mul_assign_by_fp(&a.x);

            set_coeff_element(account, 1, c1);
        },
        2 => {
            miller_value.mul_by_034(
                &get_coeff_element(account, 0),
                &get_coeff_element(account, 1),
                &get_coeff_element(account, 2),
            );
        },

        // Multiply `p_inputs` by second coeff values
        3 => {
            let p_inputs = read_g1_affine(account.p_inputs);
            let mut c0 = super::gamma_g2_neg_pc(coeff_ic).0;
            c0.mul_assign_by_fp(&p_inputs.y);

            set_coeff_element(account, 0, c0);
        },
        4 => {
            let p_inputs = read_g1_affine(account.p_inputs);
            let mut c1 = super::gamma_g2_neg_pc(coeff_ic).1;
            c1.mul_assign_by_fp(&p_inputs.x);

            set_coeff_element(account, 1, c1);
        },
        5 => {
            miller_value.mul_by_034(
                &get_coeff_element(account, 0),
                &get_coeff_element(account, 1),
                &super::gamma_g2_neg_pc(coeff_ic).2,
            );
        },

        // Multiply `c` by third coeff values
        6 => {
            let c = read_g1_affine(account.proof_c);
            let mut c0 = super::delta_g2_neg_pc(coeff_ic).0;
            c0.mul_assign_by_fp(&c.y);

            set_coeff_element(account, 0, c0);
        },
        7 => {
            let c = read_g1_affine(account.proof_c);
            let mut c1 = super::delta_g2_neg_pc(coeff_ic).1;
            c1.mul_assign_by_fp(&c.x);

            set_coeff_element(account, 1, c1);
        },
        8 => {
            miller_value.mul_by_034(
                &get_coeff_element(account, 0),
                &get_coeff_element(account, 1),
                &super::delta_g2_neg_pc(coeff_ic).2,
            );

            account.inc_coeff_ic();
        },
        _ => {}
    }

    write_miller_value(account, miller_value);
}

fn set_coeff_element(account: &mut ProofVerificationAccount, element: usize, value: Fq2) {
    write_fq2(&mut account.get_ram_mut(COEFF_OFFSET + 2 * element, 2), value);
}

fn get_coeff_element(account: &ProofVerificationAccount, element: usize) -> Fq2 {
    read_fq2(&account.get_ram(COEFF_OFFSET + 2 * element, 2))
}

pub fn read_miller_value(account: &ProofVerificationAccount) -> Fq12 {
    let a = read_fq2(&account.get_ram(MILLER_OFFSET, 2));
    let b = read_fq2(&account.get_ram(MILLER_OFFSET + 2, 2));
    let c = read_fq2(&account.get_ram(MILLER_OFFSET + 4, 2));

    let d = read_fq2(&account.get_ram(MILLER_OFFSET + 6, 2));
    let e = read_fq2(&account.get_ram(MILLER_OFFSET + 8, 2));
    let f = read_fq2(&account.get_ram(MILLER_OFFSET + 10, 2));

    Fq12::new(
        Fq6::new(a, b, c),
        Fq6::new(d, e, f),
    )
}

pub fn write_miller_value(account: &mut ProofVerificationAccount, value: Fq12) {
    write_fq2(&mut account.get_ram_mut(MILLER_OFFSET, 2), value.c0.c0);
    write_fq2(&mut account.get_ram_mut(MILLER_OFFSET + 2, 2), value.c0.c1);
    write_fq2(&mut account.get_ram_mut(MILLER_OFFSET + 4, 2), value.c0.c2);

    write_fq2(&mut account.get_ram_mut(MILLER_OFFSET + 6, 2), value.c1.c0);
    write_fq2(&mut account.get_ram_mut(MILLER_OFFSET + 8, 2), value.c1.c1);
    write_fq2(&mut account.get_ram_mut(MILLER_OFFSET + 10, 2), value.c1.c2);
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
        let mut account = init_account(&mut data);

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
        let result = (
            get_coeff_element(&account, 0),
            get_coeff_element(&account, 1),
            get_coeff_element(&account, 2),
        );

        // Original
        let expected = addition_step_original(&mut r2, &b);

        assert_eq!(result, expected);
        assert_eq!(r1.x, r2.x);
        assert_eq!(r1.y, r2.y);
        assert_eq!(r1.z, r2.z);
    }

    #[test]
    fn test_doubling_rounds() {
        let b = get_b();
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = init_account(&mut data);

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
        let result = (
            get_coeff_element(&account, 0),
            get_coeff_element(&account, 1),
            get_coeff_element(&account, 2),
        );

        // Original
        let expected = doubling_step_original(&mut r2, &TWO_INV);

        assert_eq!(result, expected);
        assert_eq!(r1.x, r2.x);
        assert_eq!(r1.y, r2.y);
        assert_eq!(r1.z, r2.z);
    }

    #[test]
    fn test_ell_rounds() {
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = init_account(&mut data);

        let b = get_b();
        let mut r = G2HomProjective { x: b.x, y: b.y, z: Fq2::one() };
        let b_coeffs = doubling_step_original(&mut r, &TWO_INV);
        for i in 0..PREPARE_INPUTS_ITERATIONS { partial_prepare_inputs(&mut account, i).unwrap(); }
        write_miller_value(&mut account, Fq12::one());
        set_coeff_element(&mut account, 0, b_coeffs.0);
        set_coeff_element(&mut account, 1, b_coeffs.1);
        set_coeff_element(&mut account, 2, b_coeffs.2);

        // ell computation
        for round in 0..ELL_ROUNDS { ell_round(&mut account, round); }
        let result = read_miller_value(&account);

        // Original
        let mut miller = Fq12::one();
        ell_original(&mut miller, b_coeffs, &get_a());
        let p_inputs = read_g1_affine(account.p_inputs);
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
        let p_inputs = read_g1_affine(account.p_inputs);
        for iteration in 0..MILLER_LOOP_ITERATIONS {
            partial_miller_loop(&mut account, iteration).unwrap();
        }
        let result = read_miller_value(&account);

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

        assert_eq!(read_g2_affine(&account.proof_b), get_b());
        assert_eq!(read_g2_affine(&account.b_neg), -get_b());
        assert_eq!(read_fq2_le_montgomery(&account.b_homo_r[..64]), get_b().x);
        assert_eq!(read_fq2_le_montgomery(&account.b_homo_r[64..128]), get_b().y);
        assert_eq!(read_fq2_le_montgomery(&account.b_homo_r[128..]), Fq2::one());
        assert_eq!(read_g1_affine(&account.proof_a), get_a());
        assert_eq!(read_g1_affine(&account.proof_c), get_c());

        account
    }
}