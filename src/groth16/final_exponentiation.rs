use ark_bn254::{ Fq2, Fq6, Fq12, Fq12Parameters, Fq2Parameters, Fq6Parameters };
use ark_ff::fields::{
    Field,
    models::{
        QuadExtParameters,
        CubicExtParameters,
        Fp12Parameters,
        Fp6Parameters,
        Fp2Parameters,
        fp12_2over3over2::Fp12ParamsWrapper,
        fp6_3over2::Fp6ParamsWrapper,
        fp2::Fp2ParamsWrapper,
    }
};
use ark_ff::{ One, Zero };
use super::super::state::ProofVerificationAccount;

// TODO: Handle unwrap/zero cases
// TODO: Add method to clear element from stack

pub const FINAL_EXPONENTIATION_ITERATIONS: usize = 1;
pub const FINAL_EXPONENTIATION_ROUNDS: [usize; 1] = [1];

const RANGE_COUNT: usize = 38;
const LAST_ROUND: usize = 113;

pub fn final_exponentiation(
    account: &mut ProofVerificationAccount,
    iteration: usize,
) {
    for round in 0..=LAST_ROUND {
        match round {
            0 => {   // Check whether f is zero (if true, it cannot be inverted)
                let f = account.peek_fq12(0);

                if f.is_zero() { panic!() }
            },

            // - pushes: f2
            1..=9 => {   // f2 <- f^{-1} (~ 285923 CUs)
                let f = account.peek_fq12(0);

                // - pushes: f2 after last round
                f12_inverse(&f, account, round - 1);  // -> fail if inverse fails
            },

            // - pops: f2, f
            // - pushes: f2, r (Fq12)
            10 => {
                let f2 = account.pop_fq12();
                let f = account.pop_fq12();

                let mut f1 = f;
                f1.conjugate();
                let r = f1;

                account.push_fq12(f2);
                account.push_fq12(r);
            },

            // - pops: r, f2
            // - pushes: mul stack vars, f2, r
            11..=15 => {   // r <- f1 * f2
                mul(account, round - 11);
            },

            // - pops: r, f2
            // - pushes: f2, r
            16 => {   // f2 <- r
                let r = account.pop_fq12();
                account.stack_fq12.pop_empty();

                account.push_fq12(r);
                account.push_fq12(r);
            },

            // - pops: r
            // - pushes: r
            17..=19 => { // ~ 53325
                frobenius_map(account, 2, round - 17);
            },

            // - pops: r, f2
            // - pushes: f2 (unchanged), r
            20..=24 => { //r *= &f2;   // ~ 131961 // -> r
                mul(account, round - 20);
            },

            // - pops: r, f2
            // - pushes: r (unchanged), y0
            25 => {
                let r = account.pop_fq12();
                account.stack_fq12.pop_empty();

                let y0 = r;

                account.push_fq12(r);
                account.push_fq12(y0);
            },

            // - pops: y0
            // - pushes: y0
            26..=30 => {
                exp_neg_x(account, round - 26);
            },
            
            // - pops: y0
            // - pushes: y1 (-> r, y1)
            31 => { // -> y1
                let y0 = account.pop_fq12();
                let y1 = cyclotomic_square(y0);    // ~ 45634

                account.push_fq12(y1);
            },

            // - pushes y2 (-> r, y1, y2)
            32 => {
                let y1 = account.peek_fq12(0);
                let y2 = cyclotomic_square(y1);    // ~ 45569

                account.push_fq12(y2);
            },

            // - pops: y2, y1
            // - pushes: mul stack vars, y1, y3
            33..=37 => { //y3 = y2 * y1;  (~ 132119 CUs)
                mul(account, round - 33);
            },

            // - pops: y3
            // - pushes: y3, y4
            38 => {
                let y3 = account.pop_fq12();

                account.push_fq12(y3);
                account.push_fq12(y3);
            },

            // - pops: y4
            // - pushes: local stack vars, y4 (-> r, y1, y3, y4)
            39..=43 => {   // y4 = exp_by_neg_x(y3) (~ 6_009_534 CUs)
                exp_neg_x(account, round - 39);
            },

            // - pushes: y5
            44 => { // y5 <- cyclotomic_square(y4) (~ 45634 CUs)
                let y4 = account.peek_fq12(0);

                let y5 = cyclotomic_square(y4);

                account.push_fq12(y5);
            },

            // - pops: y5
            // - pushes: y6
            45..=49 => {   // y6 = exp_by_neg_x(y5) (~ 6_009_534 CUs)
                exp_neg_x(account, round - 45);
            },

            // - pops: y6,
            // - pushes: y7
            50 => {   // y7 <- y6.conjugate()
                let mut y7 = account.pop_fq12();

                y7.conjugate();

                account.push_fq12(y7);
            },

            51..=55 => { // y7 *= y4;  (~ 132119 CUs)
                mul(account, round - 51);
            },

            // - pops: y7, y4, y3
            // - pushes: y4, y3, y8
            56 => {
                let y8 = account.pop_fq12();
                let y4 = account.pop_fq12();
                let mut y3 = account.pop_fq12();

                y3.conjugate();

                account.push_fq12(y4);
                account.push_fq12(y3);
                account.push_fq12(y8);
            },

            57..=61 => {   // y8 *= y3
                mul(account, round - 57);
            },

            // - pops: y8, y3, y4, y1
            // - pushes: y8, y4, y10, y1, y9
            62 => {
                let y8 = account.pop_fq12();
                account.stack_fq12.pop_empty();
                let y4 = account.pop_fq12();
                let y1 = account.pop_fq12();

                account.push_fq12(y8);
                account.push_fq12(y4);
                account.push_fq12(y8);  // y10
                account.push_fq12(y1);
                account.push_fq12(y8);  // y9
            },

            63..=67 => {   // y9 *= y1
                mul(account, round - 63);
            },

            // - pops: y9, y1, y10, y4
            // - pushes: y9, y4, y10 (-> r, y8, y9, y4, y10) 
            68 => {
                account.stack_fq12.swap(0, 3);  // swap y9 and y4
                let y4 = account.pop_fq12();
                account.stack_fq12.pop_empty(); // drain y1
                account.push_fq12(y4);
                account.stack_fq12.swap(0, 1); // swap y4 and y10
            },

            69..=73 => {   // y10 *= y4
                mul(account, round - 69);
            },

            // - -> stack: (-> y9, y8, r, y10)
            74 => {
                account.stack_fq12.swap(0, 1);  // swap y10 and y4
                account.stack_fq12.pop_empty(); // drain y4
                account.stack_fq12.swap(1, 3);  // swap y9 and r
            },

            75..=79 => {   // y11 = y10 * r
                mul(account, round - 75);
            },

            // - pushes: y12 (-> y9, y8, r, y11, y12)
            80 => {
                let y9 = account.peek_fq12(3);
                account.push_fq12(y9);
            },

            81..=83 => {   // y12 = frobenius_map(y9, power: 1)
                frobenius_map(account, 1, round - 81);
            },

            84..=88 => {   // y13 = y12 * y11
                mul(account, round - 84);
            },

            // - -> stack: (-> y9, y11, r, y13, y8)
            89 => {   //bring y8 to the top of the stack
                account.stack_fq12.swap(0, 3);  // swap y8 and y13
                account.stack_fq12.swap(1, 3);  // swap y13 and y11
            },

            90..=92 => {   // y8 = frobenius_map(y8, power: 2)
                frobenius_map(account, 2, round - 90);
            },

            93..=97 => {   // y8 *= y13
                mul(account, round - 93);
            },

            // - -> stack: (-> y8, y9, r)
            98 => {
                // (-> y9, y11, r, y13, y8)
                let y8 = account.pop_fq12();
                account.stack_fq12.pop_empty();
                let mut r = account.pop_fq12();
                account.stack_fq12.pop_empty();
                let y9 = account.pop_fq12();

                r.conjugate();

                account.push_fq12(y8);
                account.push_fq12(y9);
                account.push_fq12(r);
            },

            99..=103 => {   // r *= y9
                mul(account, round - 99);
            },

            104..=106 => {   // r = frobenius_map(r, power: 3)
                frobenius_map(account, 3, round - 104);
            },

            // - -> stack: (-> y8, r)
            107 => {
                account.stack_fq12.swap(0, 1);  // swap r and y9
                account.stack_fq12.pop_empty(); // drain y9
            },

            108..=112 => {   // r *= y8
                mul(account, round - 108);
            },

            // - -> stack: (-> r)
            113 => {
                account.stack_fq12.swap(0, 1);
                account.stack_fq12.pop_empty();
            },
            _ => {} 
        }
    }
}

fn mul(account: &mut ProofVerificationAccount, round: usize) {
    let mut a = account.pop_fq12();
    let b = account.peek_fq12(0);

    f12_mul_assign(&mut a, &b, account, round);

    account.push_fq12(a);
}

fn exp_neg_x(account: &mut ProofVerificationAccount, round: usize) {
    let mut v = account.pop_fq12();

    exp_by_neg_x(&mut v, account, round);

    account.push_fq12(v);
}

fn frobenius_map(account: &mut ProofVerificationAccount, power: usize, round: usize) {
    let mut v = account.pop_fq12();

    f12_frobenius_map(&mut v, power, round);

    account.push_fq12(v);
}

const F12_INVERSE_ROUND_COUNT: usize = 3 + F6_INVERSE_ROUND_COUNT;

fn f12_inverse(
    f: &Fq12,
    account: &mut ProofVerificationAccount,
    round: usize,
) {
    match round {
        // - pushes: v1 (Fq6)
        0 => {  // ~ 28000
            let v1 = f.c1.square();
            account.push_fq6(v1);
        },

        // - pops: v1
        // - pushes: v0 (Fq6)
        1 => {  // ~ 30000
            let v1 = account.pop_fq6();
    
            let v2 = f.c0.square();
            let v0 = Fp12ParamsWrapper::<Fq12Parameters>::sub_and_mul_base_field_by_nonresidue(&v2, &v1);   // ~ 1621
    
            account.push_fq6(v0);
        },

        // - pops: v0
        // - pushes: f6_inverse stack variables, v0 (unchanged)
        (2..=F6_INVERSE_ROUND_COUNT_PLUS_ONE) => {    // ~ 231693
            let v0 = account.pop_fq6();

            if v0.is_zero() { panic!() }
            f6_inverse(&v0, account, round - 2);

            account.push_fq6(v0);
        },

        // - pops: v0
        // - pushes: f2
        F6_INVERSE_ROUND_COUNT_PLUS_TWO => {
            let _ = account.pop_fq6();
            let v0 = account.pop_fq6();

            let c0 = f.c0 * &v0;
            let c1 = -(f.c1 * &v0);
            let res = Fq12::new(c0, c1);

            account.push_fq12(res);
        }
        _ => {}
    }
}

const F6_INVERSE_ROUND_COUNT: usize = 6;
const F6_INVERSE_ROUND_COUNT_PLUS_ONE: usize = F6_INVERSE_ROUND_COUNT + 1;
const F6_INVERSE_ROUND_COUNT_PLUS_TWO: usize = F6_INVERSE_ROUND_COUNT + 2;

fn f6_inverse(
    f: &Fq6,
    account: &mut ProofVerificationAccount,
    round: usize,
) {
    match round {
        // - pushes: s2 (Fq2)
        0 => {  // ~ 11000
            let t1 = f.c1.square();
            let t4 = f.c0 * &f.c2;
            let s2 = t1 - &t4;
    
            account.push_fq2(s2);
        },

        // - pushes: s1 (Fq2), s0 (Fq2)
        1 => {  // ~ 22000
            let t0 = f.c0.square();
            let t2 = f.c2.square();
            let t3 = f.c0 * &f.c1;
            let t5 = f.c1 * &f.c2;
            let n5 = Fp6ParamsWrapper::<Fq6Parameters>::mul_base_field_by_nonresidue(&t5);
            let s0 = t0 - &n5;
            let s1 = Fp6ParamsWrapper::<Fq6Parameters>::mul_base_field_by_nonresidue(&t2) - &t3;

            account.push_fq2(s1);
            account.push_fq2(s0);
        },

        // - pushes: t6 (Fq2)
        2 => {  // ~ 21000
            let s0 = account.peek_fq2(0);
            let s1 = account.peek_fq2(1);
            let s2 = account.peek_fq2(2);

            let a1 = f.c2 * &s1;
            let a2 = f.c1 * &s2;
            let mut a3 = a1 + &a2;
            a3 = Fp6ParamsWrapper::<Fq6Parameters>::mul_base_field_by_nonresidue(&a3);
            let t6 = f.c0 * &s0 + &a3;  // ~ 6467
            if t6.is_zero() { panic!() }

            account.push_fq2(t6);
        },

        // - pushes: v0a (Fq)
        3 => {  // ~ 3346
            let t6 = account.peek_fq2(0);
    
            let v1a = t6.c1.square();
            let v2a = t6.c0.square();
            let v0a = Fp2ParamsWrapper::<Fq2Parameters>::sub_and_mul_base_field_by_nonresidue(&v2a, &v1a); // ~ 125
    
            account.push_fq(v0a);
        },

        // - pops: v0a
        // - pushes: v0a (Fq)
        4 => {  // ~ 64678 - 100.000
            let mut v0a = account.pop_fq();

            v0a = v0a.inverse().unwrap();

            account.push_fq(v0a);
        },

        // - pops: v0a, t6, s0, s1, s2
        // - pushes: v1 (Fq6)
        5 => {   // ~ 23000
            let v0a = account.pop_fq();
            let mut t6 = account.pop_fq2();
            let s0 = account.pop_fq2();
            let s1 = account.pop_fq2();
            let s2 = account.pop_fq2();
    
            let c0 = t6.c0 * &v0a;    // ~ 1904
            let c1 = -(t6.c1 * &v0a); // ~ 1949
            t6 = Fq2::new(c0, c1);
            let c0 = t6 * &s0;  // ~ 6000
            let c1 = t6 * &s1;  // ~ 6000
            let c2 = t6 * &s2;  // ~ 6000
            let v1 = Fq6::new(c0, c1, c2);
    
            account.push_fq6(v1);
        },
        _ => {}
    }
}

fn cyclotomic_square(f: Fq12) -> Fq12 {
    // TODO: Convert cyclotomic Square into rounds system
    let mut result = f;
    result.cyclotomic_square_in_place();
    result
}

const F12_FROBENIUS_MAP_ROUND_COUNT: usize = 3;

fn f12_frobenius_map(
    f: &mut Fq12,
    power: usize,
    round: usize,
) {
    match round {
        0 => {
            f6_frobenius_map(&mut f.c0, power); // ~ 17625
        },
        1 => {
            f6_frobenius_map(&mut f.c1, power); // ~ 17637
        },
        2 => {
            f.c1.mul_assign_by_fp2(Fq12Parameters::FROBENIUS_COEFF_FP12_C1[power % 12]);    // ~ 18065
        }
        _ => {}
    }
}

fn f6_frobenius_map(
    f: &mut Fq6,
    power: usize
) {
    f2_frobenius_map(&mut f.c0, power);
    f2_frobenius_map(&mut f.c1, power);
    f2_frobenius_map(&mut f.c2, power);
    f.c1 *= &Fq6Parameters::FROBENIUS_COEFF_FP6_C1[power % 6];
    f.c2 *= &Fq6Parameters::FROBENIUS_COEFF_FP6_C2[power % 6];
}

#[inline(always)]
fn f2_frobenius_map(f: &mut Fq2, power: usize) {
    f.c1 *= &Fq2Parameters::FROBENIUS_COEFF_FP2_C1[power % 2];
}

const EXP_BY_NEG_X_ROUND_COUNT: usize = 2 + CYCLOTOMIC_ROUNDS_LEN;

const CYCLOTOMIC_EXPRESSION_SUB_ROUND_COUNT: usize = F12_MUL_ROUND_COUNT + 1;
const CYCLOTOMIC_EXPRESSION_ROUND_COUNT: usize = X_WNAF_L * CYCLOTOMIC_EXPRESSION_SUB_ROUND_COUNT;

const CYCLOTOMIC_ROUNDS_LEN: usize = 3;
const CYCLOTOMIC_ROUNDS: [(usize, usize); CYCLOTOMIC_ROUNDS_LEN] = [
    (0, 2),
    (2, 10),
    (10, CYCLOTOMIC_EXPRESSION_ROUND_COUNT)
];
const CYCLOTOMIC_ROUNDS_LEN_PLUS_ONE: usize = CYCLOTOMIC_ROUNDS_LEN + 1;

const X_WNAF_L: usize = 63;

/// Non-adjacent window form of exponent Parameters::X (u64: 4965661367192848881)
/// NAF computed using: https://citeseerx.ist.psu.edu/viewdoc/download?doi=10.1.1.394.3037&rep=rep1&type=pdf Page 98
const X_WNAF: [i64; X_WNAF_L] = [1, 0, 0, 0, -1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1, 0, 0, 1, 0, -1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, -1, 0, -1, 0, -1, 0, 1, 0, 1, 0, 0, -1, 0, 1, 0, 1, 0, -1, 0, 0, 1, 0, 1, 0, 0, 0, 1];

/// A
/// - in the WNAF loop, we have `F12_MUL_ROUND_COUNT` * `X_WNAF_L` iterations (since we use `F12_MUL_ROUND_COUNT` per multiplication)
/// - for the iterations in which we don't have any multiplication, we skip using a cost of 0 CUs
/// - Question: more expensive to conjugate or to store and read?
fn exp_by_neg_x(
    f: &mut Fq12,
    account: &mut ProofVerificationAccount,
    round: usize,
) {
    match round {
        // - pushes: fe, fe_inverse
        0 => {  // (~ 1000 CUs)
            let mut fe_inverse = *f;
            fe_inverse.conjugate();

            account.push_fq12(*f);
            account.push_fq12(fe_inverse);

            *f = Fq12::one();
        },

        // - pops: fe_inverse, fe
        // - pushes: f12_mul_assign stack vars, fe, fe_inverse
        1..=CYCLOTOMIC_ROUNDS_LEN => { // Cyclotomic expression
            let fe_inverse = account.pop_fq12();
            let fe = account.pop_fq12();

            let (lower_round, upper_round) = CYCLOTOMIC_ROUNDS[round - 1];
            
            for i in lower_round..upper_round {
                let sub_round = i % CYCLOTOMIC_EXPRESSION_SUB_ROUND_COUNT;
                let i = i / CYCLOTOMIC_EXPRESSION_SUB_ROUND_COUNT;
                let value = X_WNAF[X_WNAF_L - 1 - i];

                if sub_round == 0 {
                    if i > 0 {
                        f.cyclotomic_square_in_place();
                    }
                } else {
                    if value > 0 {
                        f12_mul_assign(f, &fe, account, sub_round - 1);
                    } else if value < 0 {
                        f12_mul_assign(f, &fe_inverse, account, sub_round - 1);
                    }
                }
            }

            account.push_fq12(fe);
            account.push_fq12(fe_inverse);
        },

        // - pops: fe_inverse, fe
        CYCLOTOMIC_ROUNDS_LEN_PLUS_ONE => {
            let _ = account.pop_fq12();
            let _ = account.pop_fq12();
            
            f.conjugate();
        },
        _ => { }
    }
}

const F12_MUL_ROUND_COUNT: usize = 5;

// Karatsuba multiplication;
// Guide to Pairing-based cryprography, Algorithm 5.16.
/// [20000, 20000, 20000, 20000, 46000]
fn f12_mul_assign(
    a: &mut Fq12,
    b: &Fq12,
    account: &mut ProofVerificationAccount,
    round: usize,
) {
    // ~ 42000
    match round {
        // - pushes: v0
        0 => {  
            let v0 = f6_mul(a.c0, b.c0, Fq6::zero(), 0);
            account.push_fq6(v0);
        },
        1 => { 
            let mut v0 = account.pop_fq6();
            v0 = f6_mul(a.c0, b.c0, v0, 1);
            account.push_fq6(v0);
        },

        // - pushes: v1
        2 => {
            let v1 = f6_mul(a.c1, b.c1, Fq6::zero(), 0);
            account.push_fq6(v1);
        },
        3 => {  
            let mut v1 = account.pop_fq6();
            v1 = f6_mul(a.c1, b.c1, v1, 1);
            account.push_fq6(v1);
        },

        // - pops: v1, v0
        4 => {  
            let v1 = account.pop_fq6();
            let v0 = account.pop_fq6();

            a.c1 += &a.c0;  // ~ 400
            a.c1 *= &(b.c0 + &b.c1);    // ~ 43000

            a.c1 -= &v0;    // ~ 400
            a.c1 -= &v1;    // ~ 400
            a.c0 = Fp12ParamsWrapper::<Fq12Parameters>::add_and_mul_base_field_by_nonresidue(&v0, &v1); // ~ 1831
        },
        _ => {}
    }
}

// Devegili OhEig Scott Dahab --- Multiplication and Squaring on
// AbstractPairing-Friendly
// Fields.pdf; Section 4 (Karatsuba)
fn f6_mul(
    lhs: Fq6,
    rhs: Fq6,
    p: Fq6,
    round: usize,
) -> Fq6 {
    if round == 0 { // ~ 19000
        Fq6::new(
            lhs.c0 * rhs.c0,
            lhs.c1 * rhs.c1,
            lhs.c2 * rhs.c2,
        )
    } else if round == 1 { // ~ 24000
        let x = (lhs.c1 + lhs.c2) * &(rhs.c1 + rhs.c2) - &p.c1 - &p.c2;
        let y = (lhs.c0 + lhs.c1) * &(rhs.c0 + rhs.c1) - &p.c0 - &p.c1;
        let z = (lhs.c0 + lhs.c2) * &(rhs.c0 + rhs.c2) - &p.c0 + &p.c1 - &p.c2;

        Fq6::new(
            p.c0 + &Fp6ParamsWrapper::<Fq6Parameters>::mul_base_field_by_nonresidue(&x),
            y + &Fp6ParamsWrapper::<Fq6Parameters>::mul_base_field_by_nonresidue(&p.c2),
            z,
        )
    } else {
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use ark_ec::{ PairingEngine, models::bn::BnParameters };
    use ark_bn254::{ Fq, Bn254, Parameters };

    #[test]
    pub fn test_f12_inverse() {
        let f = get_f();
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();

        for round in 0..F12_INVERSE_ROUND_COUNT {
            f12_inverse(&f, &mut account, round);
        }

        let expected = f.inverse().unwrap();
        let result = account.pop_fq12();

        assert_eq!(result, expected);
        assert_stack_is_cleared(&account);
    }

    #[test]
    pub fn test_f12_mul_assign() {
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();

        let expected = get_f() * get_f();

        let mut result = get_f();
        for round in 0..F12_MUL_ROUND_COUNT {
            f12_mul_assign(&mut result, &get_f(), &mut account, round);
        }

        assert_eq!(result, expected);
        assert_stack_is_cleared(&account);
    }

    #[test]
    pub fn test_mul() {
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();
        account.push_fq12(get_f());
        account.push_fq12(get_f());

        /*for round in 0..F12_MUL_ROUND_COUNT {
            mul(&mut account, round)
        }*/

        let expected = get_f() * get_f();
        let result = account.pop_fq12();
        assert_eq!(result, expected);
    }

    #[test]
    pub fn test_frobenius_map() {
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();
        account.push_fq12(get_f());

        for round in 0..F12_FROBENIUS_MAP_ROUND_COUNT {
            //frobenius_map(&mut account, 3, round);
        }

        let mut expected = get_f();
        expected.frobenius_map(3);

        let result = account.pop_fq12();

        assert_eq!(result, expected);
    }

    #[test]
    pub fn test_exp_by_neg_x() {
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();
        account.push_fq12(get_f());

        for round in 0..EXP_BY_NEG_X_ROUND_COUNT {
            exp_neg_x(&mut account, round);
        }

        let expected = original_exp_by_neg_x(get_f());
        let result = account.pop_fq12();

        assert_eq!(result, expected);
        assert_stack_is_cleared(&account);
    }

    #[test]
    pub fn test_final_exponentiation() {
        let f = get_f();
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();
        account.push_fq12(f);

        let expected = Bn254::final_exponentiation(&f).unwrap();
            
        for iteration in 0..FINAL_EXPONENTIATION_ITERATIONS {
            final_exponentiation(&mut account, iteration);
        }
        let result = account.pop_fq12();

        assert_eq!(result, expected);
        assert_stack_is_cleared(&account);
    }

    /// Stack convention:
    /// - every private function has to clear the local stack
    /// - public functions are allowed to return values on the stack
    fn assert_stack_is_cleared(account: &ProofVerificationAccount) {
        assert_eq!(account.stack_fq.stack_pointer, 0);
        assert_eq!(account.stack_fq6.stack_pointer, 0);
        assert_eq!(account.stack_fq12.stack_pointer, 0);
    }

    fn get_f() -> Fq12 {
        Fq12::new(
            Fq6::new(
                Fq2::new(
                    Fq::from_str("20925091368075991963132407952916453596237117852799702412141988931506241672722").unwrap(),
                    Fq::from_str("18684276579894497974780190092329868933855710870485375969907530111657029892231").unwrap(),
                ),
                Fq2::new(
                    Fq::from_str("5932690455294482368858352783906317764044134926538780366070347507990829997699").unwrap(),
                    Fq::from_str("18684276579894497974780190092329868933855710870485375969907530111657029892231").unwrap(),
                ),
                Fq2::new(
                    Fq::from_str("19526707366532583397322534596786476145393586591811230548888354920504818678603").unwrap(),
                    Fq::from_str("19526707366532583397322534596786476145393586591811230548888354920504818678603").unwrap(),
                ),
            ),
            Fq6::new(
                Fq2::new(
                    Fq::from_str("18684276579894497974780190092329868933855710870485375969907530111657029892231").unwrap(),
                    Fq::from_str("20925091368075991963132407952916453596237117852799702412141988931506241672722").unwrap(),
                ),
                Fq2::new(
                    Fq::from_str("5932690455294482368858352783906317764044134926538780366070347507990829997699").unwrap(),
                    Fq::from_str("20925091368075991963132407952916453596237117852799702412141988931506241672722").unwrap(),
                ),
                Fq2::new(
                    Fq::from_str("18684276579894497974780190092329868933855710870485375969907530111657029892231").unwrap(),
                    Fq::from_str("5932690455294482368858352783906317764044134926538780366070347507990829997699").unwrap(),
                ),
            ),
        )
    }

    fn original_exp_by_neg_x(f: Fq12) -> Fq12 {
        let mut f = f.cyclotomic_exp(&Parameters::X);
        if !Parameters::X_IS_NEGATIVE {
            f.conjugate();
        }
        f
    }
}

//let x = generate_ranges(); println!("{:?}", x); panic!();
/*fn generate_ranges() -> Vec<std::ops::RangeInclusive<usize>> {
    enum ArmType {
        One,
        Inverse,
        Mul,
        Frobenius,
        CyclotomicSquare,
        ExpByNegX,
    }
    use ArmType::*;
    let arms: [ArmType; RANGE_COUNT] = [
        One,
        Inverse,
        One,
        Mul,
        One,
        Frobenius,
        Mul,
        One,
        ExpByNegX,
        CyclotomicSquare,
        CyclotomicSquare,
        Mul,
        One,
        ExpByNegX,
        CyclotomicSquare,
        ExpByNegX,
        One,
        Mul,
        One,
        Mul,
        One,
        Mul,
        One,
        Mul,
        One,
        Mul,
        One,
        Frobenius,
        Mul,
        One,
        Frobenius,
        Mul,
        One,
        Mul,
        Frobenius,
        One,
        Mul,
        One
    ];
    let mut res = Vec::new();
    let mut base_round = 0;
    for arm in arms.iter() {
        let rounds = match arm {
            One => 1,
            Inverse => F12_INVERSE_ROUND_COUNT,
            Mul => F12_MUL_ROUND_COUNT,
            Frobenius => F12_FROBENIUS_MAP_ROUND_COUNT,
            CyclotomicSquare => 1,
            ExpByNegX => EXP_BY_NEG_X_ROUND_COUNT
        };
        res.push(base_round..=(base_round + rounds - 1));
        base_round += rounds;
    }
    res
}*/