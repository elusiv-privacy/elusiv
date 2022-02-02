use solana_program::entrypoint::ProgramResult;
use ark_bn254::{ Fq, Fq2, G2Affine, Parameters };
use ark_ec::models::bn::BnParameters;
use ark_ff::*;
use super::super::scalar::*;
use super::super::state::ProofVerificationAccount;

pub const B_COEFFS_COUNT: usize = 91;
pub const B_COEFF_LENGTH: usize = 3 * 2 * 32;
pub const B_COEFFS_TOTAL_BYTES: usize = B_COEFFS_COUNT * B_COEFF_LENGTH;

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

pub const PREPARE_PROOF_ITERATIONS: usize = 37;
const ITERATION_ROUNDS: [usize; 37] = [15, 21, 27, 21, 15, 21, 27, 21, 21, 16, 26, 21, 27, 26, 22, 21, 27, 14, 15, 20, 34, 14, 22, 26, 22, 14, 22, 20, 27, 14, 21, 21, 21, 21, 21, 27, 11];
const MAIN_ROUNDS: usize = 768;
const FULL_ROUNDS: usize = ADDITION_STEP_ROUNDS + DOUBLING_STEP_ROUNDS;

const ADDITION_STEP_ROUNDS_PLUS_ONE: usize = ADDITION_STEP_ROUNDS + 1;
const ADDITION_STEP_ROUNDS_PLUS_TWO: usize = ADDITION_STEP_ROUNDS + 2;
const ADDITION_STEP_ROUNDS_TWICE_PLUS_TWO: usize = ADDITION_STEP_ROUNDS * 2 + 2;

/// Computes `B_COEFFS_COUNT` coefficients – 3-tuples of `Fq2`
/// - requires `PROOF_PREPARATION_ITERATIONS` calls to complete
/// - Question: is it correct/allowed to assume that b can/should never be = zero? -> we assume b always != infinity
/// 
/// - we require `DOUBLING_STEP_ROUNDS` rounds to compute the doubling_step
/// - we require `ADDITION_STEP_ROUNDS` rounds to compute the addition_step
/// - in some main loop iterations we only compute the doubling_step
/// - to have the same amount of rounds per iteration, we say each iteration takes FULL_ROUNDS rounds
///   but if addition_step is not computed, we just assign them a computation cost of 0 CUs
///   (and therefore just skip them)
/// 
/// - `ITERATION_ROUNDS` computed using:
///     - reverse ATE_LOOP_COUNT[..64]
///     - build a cost vector
///     - for every 0 in ATE_LOOP_COUNT, extend it by the doubling_step CU array and an empty (addition_step) CU array
///     - for every != 0 in ATE_LOOP_COUNT, extend it by the doubling_step and addition_step CU array
///     - add the mul_by_char cost + addition_step CU array twice
///     - then just fit the rounds into 200_000
pub fn partial_prepare_proof(
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

        if i <= 63 { // Main loop
            let round = round % FULL_ROUNDS;
            if round < ADDITION_STEP_ROUNDS {
                doubling_step(account, &mut r, round);
            } else {
                let round = round - ADDITION_STEP_ROUNDS;

                let bit = Parameters::ATE_LOOP_COUNT[63 - i];
                if bit == 1 {
                    addition_step(account, &mut r, &b, round);
                } else if bit == -1 {
                    addition_step(account, &mut r, &neg_b, round);
                }
            }
        } else {    // Final two coefficients
            let round = round - MAIN_ROUNDS;

            match round {
                0 => {  // Compute b1 (and override b itself) (~ 18000 CUs)
                    b = mul_by_char(b);
                    write_g2_affine(&mut account.proof_b, b);
                },
                1..=ADDITION_STEP_ROUNDS => {
                    addition_step(account, &mut r, &b, round - 1);
                },
                ADDITION_STEP_ROUNDS_PLUS_ONE => {  // Compute b2 (and override b itself) (~ 18000 CUs)
                    b = mul_by_char(b);
                    b.y = -b.y;
                    write_g2_affine(&mut account.proof_b, b);
                },
                ADDITION_STEP_ROUNDS_PLUS_TWO..=ADDITION_STEP_ROUNDS_TWICE_PLUS_TWO => {
                    addition_step(account, &mut r, &b, round - ADDITION_STEP_ROUNDS_PLUS_TWO);
                }
                _ => {}
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

const ADDITION_STEP_ROUNDS: usize = 6;

const THETHA_OFFSET: usize = 0;
const LAMBDA_OFFSET: usize = 2;
const E_OFFSET: usize = 4;
const G_OFFSET: usize = 6;
const H_OFFSET: usize = 8;

/// Formula for line function when working with homogeneous projective coordinates
/// - CUs: [6500, 6500, 15000, 11000, 24000, 13000]
/// 
/// ### RAM usage:
/// - theta (2 words)
/// - lamdba (2 words)
/// - e (2 words)
/// - g (2 words)
/// - h (2 words)
fn addition_step(
    account: &mut ProofVerificationAccount,
    r: &mut G2HomProjective,
    q: &G2Affine,
    round: usize,
) {
    match round {
        0 => { // compute lambda and store as 1st coeff element (~ 6500 CUs)
            let lambda = r.x - &(q.x * &r.z);   // 6531

            write_fq2(account.get_ram_mut(LAMBDA_OFFSET, 2), lambda);
            account.set_b_coeff_element(0, lambda);
        },
        1 => { // compute theta and store as 2nd coeff element (~ 6500 CUs)
            let theta = r.y - &(q.y * &r.z);    // 6541

            write_fq2(account.get_ram_mut(THETHA_OFFSET, 2), theta);
            account.set_b_coeff_element(1, -theta);
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

            account.set_b_coeff_element(2, j);
            account.inc_b_ell_coeffs_ic();
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

const DOUBLING_STEP_ROUNDS: usize = 6;

const A_OFFSET: usize = 0;
const B_OFFSET: usize = 2;
const C_OFFSET: usize = 8;
const F_OFFSET: usize = 10;

/// Formula for line function when working with homogeneous projective coordinates
/// - CUs: [10000, 11000, 8000, 18000, 11000, 15000]
/// 
/// ### RAM usage:
/// - a (2 words)
/// - b (2 words)
/// - e (2 words)
/// - g (2 words)
/// - c (2 words)
/// - f (2 words)
fn doubling_step(
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

            account.set_b_coeff_element(2, i);

            let mut g = b + &f; // 278 + 3773 + k
            g.mul_assign_by_fp(&TWO_INV);
            g = g.square();

            write_fq2(account.get_ram_mut(B_OFFSET, 2), b);
            write_fq2(account.get_ram_mut(F_OFFSET, 2), f);
            write_fq2(account.get_ram_mut(G_OFFSET, 2), g);
        },
        3 => {  // set 2. coeff element & set r.x (~ 18000 CUs)
            let a = read_fq2(account.get_ram(A_OFFSET, 2));
            let b = read_fq2(account.get_ram(B_OFFSET, 2));
            let f = read_fq2(account.get_ram(F_OFFSET, 2));

            let mut j = r.x.square();
            j = j.double() + &j;
            r.x = a * &(b - &f);    // 6499

            account.set_b_coeff_element(1, j);
        },
        4 => {  // set 1. coeff element & assign r.z (~ 11000 CUs)
            let b = read_fq2(account.get_ram(B_OFFSET, 2));
            let c = read_fq2(account.get_ram(C_OFFSET, 2));

            let h = (r.y + &r.z).square() - &(b + &c);  // 4945 + 6215 + x
            r.z = b * &h;

            account.set_b_coeff_element(0, -h);
        },
        5 => {  // set r.y (~ 15000 CUs)
            let e = read_fq2(account.get_ram(E_OFFSET, 2));
            let g = read_fq2(account.get_ram(G_OFFSET, 2));

            let e_square = e.square();  // 4157

            r.y = g - &(e_square.double() + &e_square);    // 4769 - k
            account.inc_b_ell_coeffs_ic();
        },
        _ => {}
    }
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
    use ark_bn254::{ G1Affine, G2Affine };
    use ark_ec::models::bn::{ TwistType, g2::G2Prepared };
    use std::str::FromStr;

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

    fn init_account(account: &mut ProofVerificationAccount) {
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
            super::super::Proof{ a: G1Affine::zero(), b: get_b(), c: G1Affine::zero() }
        ).unwrap();

        assert_eq!(read_g2_affine(&account.proof_b), get_b());
        assert_eq!(read_g2_affine(&account.b_neg), -get_b());
        assert_eq!(read_fq2_le_montgomery(&account.b_homo_r[..64]), get_b().x);
        assert_eq!(read_fq2_le_montgomery(&account.b_homo_r[64..128]), get_b().y);
        assert_eq!(read_fq2_le_montgomery(&account.b_homo_r[128..]), Fq2::one());
    }

    #[test]
    fn test_addition_step() {
        let b = get_b();
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();
        init_account(&mut account);

        let mut r1 = G2HomProjective { x: b.x, y: b.y, z: Fq2::one() };
        let mut r2 = G2HomProjective { x: b.x, y: b.y, z: Fq2::one() };
        doubling_step_original(&mut r1, &TWO_INV);
        doubling_step_original(&mut r2, &TWO_INV);

        // Computation
        for round in 0..ADDITION_STEP_ROUNDS {
            addition_step(
                &mut account,
                &mut r1,
                &b,
                round,
            );
        }
        let result = account.get_b_coeff(0);

        // Original
        let expected = addition_step_original(&mut r2, &b);

        assert_eq!(result, expected);
        assert_eq!(r1.x, r2.x);
        assert_eq!(r1.y, r2.y);
        assert_eq!(r1.z, r2.z);
    }

    #[test]
    fn test_doubling_step() {
        let b = get_b();
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();
        init_account(&mut account);

        let mut r1 = G2HomProjective { x: b.x, y: b.y, z: Fq2::one() };
        let mut r2 = G2HomProjective { x: b.x, y: b.y, z: Fq2::one() };

        // Computation
        for round in 0..DOUBLING_STEP_ROUNDS {
            doubling_step(
                &mut account,
                &mut r1,
                round,
            );
        }
        let result = account.get_b_coeff(0);

        // Original
        let expected = doubling_step_original(&mut r2, &TWO_INV);

        assert_eq!(result, expected);
        assert_eq!(r1.x, r2.x);
        assert_eq!(r1.y, r2.y);
        assert_eq!(r1.z, r2.z);
    }

    #[test]
    fn test_prepare_proof() {
        let b = get_b();
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();
        init_account(&mut account);

        // Computation
        for iteration in 0..PREPARE_PROOF_ITERATIONS {
            partial_prepare_proof(&mut account, iteration).unwrap();
        }
        let mut el_coeffs = Vec::new();
        for i in 0..B_COEFFS_COUNT {
            el_coeffs.push(account.get_b_coeff(i));
        }

        // Original
        let expected: G2Prepared<Parameters> = b.into();

        for i in 0..B_COEFFS_COUNT {
            assert_eq!(el_coeffs[i], expected.ell_coeffs[i]);
        }
    }
}