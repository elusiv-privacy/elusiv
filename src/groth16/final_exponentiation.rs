use ark_bn254::{ Fq2, Fq6, Fq12, Parameters, Fq12Parameters, Fq2Parameters, Fq6Parameters };
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
use ark_ec::bn::BnParameters;
use super::super::scalar::*;
use super::super::state::ProofVerificationAccount;

pub const FINAL_EXPONENTIATION_ITERATIONS: usize = 1;
pub const FINAL_EXPONENTIATION_ROUNDS: [usize; 1] = [1];

pub fn final_exponentiation(
    account: &mut ProofVerificationAccount,
) -> Fq12 {
    let mut f = read_fq12(account.get_ram(F_OFFSET, 12));

    // ~ 285923 CUs
    // Inverse f
    if f.is_zero() { panic!() }
    for round in 0..F12_INVERSE_ROUND_COUNT {
        f12_inverse(&f, account, round);  // -> fail if inverse fails
    }
    let mut f2 = read_fq12(account.get_ram(F2_OFFSET, 12));

    // ~ 629 CUs
    let mut f1 = f;
    f1.conjugate();
    let mut r = f1 * &f2; // ~ 131991
    f2 = r;

    // ~ 53325
    for round in 0..F12_FROBENIUS_MAP_ROUND_COUNT {
        f12_frobenius_map(&mut r, 2, round);
    }

    r *= &f2;   // ~ 131961

    let mut y0 = r;
    for round in 0..EXP_BY_NEG_X_ROUND_COUNT {
        y0 = exp_by_neg_x(y0, account, F2_OFFSET + 12, round);
    }
    
    let y1 = cyclotomic_square(y0);    // ~ 45634
    let y2 = cyclotomic_square(y1);    // ~ 45569
    let mut y3 = y2 * &y1;  // ~ 132119

    let mut y4 = y3;
    for round in 0..EXP_BY_NEG_X_ROUND_COUNT {
        y4 = exp_by_neg_x(y4, account, F2_OFFSET + 12, round);
    }
    //let y4 = exp_by_neg_x(y3, account, F2_OFFSET + 12);  // ~ 6_009_534

    let y5 = cyclotomic_square(y4);

    let mut y6 = y5;
    for round in 0..EXP_BY_NEG_X_ROUND_COUNT {
        y6 = exp_by_neg_x(y6, account, F2_OFFSET + 12, round);
    }
    //let mut y6 = exp_by_neg_x(y5, account, F2_OFFSET + 12);

    y3.conjugate();
    y6.conjugate();
    let y7 = y6 * &y4;
    let mut y8 = y7 * &y3;
    let y9 = y8 * &y1;
    let y10 = y8 * &y4;
    let y11 = y10 * &r;
    let mut y12 = y9;
    for round in 0..F12_FROBENIUS_MAP_ROUND_COUNT {
        f12_frobenius_map(&mut y12, 1, round);
    }
    let y13 = y12 * &y11;
    for round in 0..F12_FROBENIUS_MAP_ROUND_COUNT {
        f12_frobenius_map(&mut y8, 2, round);
    }
    let y14 = y8 * &y13;
    r.conjugate();
    let mut y15 = r * &y9;
    for round in 0..F12_FROBENIUS_MAP_ROUND_COUNT {
        f12_frobenius_map(&mut y15, 3, round);
    }
    let y16 = y15 * &y14;
    y16
}

const F_OFFSET: usize = 0;

const V1_OFFSET: usize = 12;
const S0_OFFSET: usize = 18;
const S1_OFFSET: usize = 20;
const S2_OFFSET: usize = 22;
const T6_OFFSET: usize = 24;
const V0A_OFFSET: usize = 26;

const F2_OFFSET: usize = 12;

const RES_OFFSET: usize = 24;
const FE_OFFSET: usize = 36;
const FE_INV_OFFSET: usize = 48;
const FOUND_NONZ_OFFSET: usize = 60;

// ### RAM usage:
// Base:
// - f (12 32 bytes)
// - f2
// - r
//
// f12_inverse:
// - v1, v0 (6 32 bytes)
// - 
// f6_inverse:
// - s0 (2)
// - s1 (2)
// - s2 (2)
// - t6 (2)
// - v0a (1)
//
// exp_by_neg_x
// - res
// - fe
// - fe_inv
// - found_non_zero

const F12_INVERSE_ROUND_COUNT: usize = 3 + F6_INVERSE_ROUND_COUNT;

fn f12_inverse(
    f: &Fq12,
    account: &mut ProofVerificationAccount,
    round: usize,
) {
    match round {
        0 => {  // ~ 28000
            let v1 = f.c1.square();
    
            write_fq6(account.get_ram_mut(V1_OFFSET, 6), v1);
        },
        1 => {  // ~ 30000
            let v1 = read_fq6(account.get_ram(V1_OFFSET, 6));
    
            let v2 = f.c0.square();
            let v0 = Fp12ParamsWrapper::<Fq12Parameters>::sub_and_mul_base_field_by_nonresidue(&v2, &v1);   // ~ 1621
    
            write_fq6(account.get_ram_mut(V1_OFFSET, 6), v0);
        },
        (2..=F6_INVERSE_ROUND_COUNT_PLUS_ONE) => {    // ~ 231693
            let v0 = read_fq6(account.get_ram(V1_OFFSET, 6));

            if v0.is_zero() { panic!() }
            f6_inverse(&v0, account, round - 2);
        },
        F6_INVERSE_ROUND_COUNT_PLUS_TWO => {
            let v1 = read_fq6(account.get_ram(V1_OFFSET, 6));

            let c0 = f.c0 * &v1;
            let c1 = -(f.c1 * &v1);
            let f2 = Fq12::new(c0, c1);
    
            write_fq12(account.get_ram_mut(F2_OFFSET, 12), f2);
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
        0 => {  // ~ 11000
            let t1 = f.c1.square();
            let t4 = f.c0 * &f.c2;
            let s2 = t1 - &t4;
    
            write_fq2(account.get_ram_mut(S2_OFFSET, 2), s2);
        },
        1 => {  // ~ 22000
            let t0 = f.c0.square();
            let t2 = f.c2.square();
            let t3 = f.c0 * &f.c1;
            let t5 = f.c1 * &f.c2;
            let n5 = Fp6ParamsWrapper::<Fq6Parameters>::mul_base_field_by_nonresidue(&t5);
            let s0 = t0 - &n5;
            let s1 = Fp6ParamsWrapper::<Fq6Parameters>::mul_base_field_by_nonresidue(&t2) - &t3;

            write_fq2(account.get_ram_mut(S0_OFFSET, 2), s0);
            write_fq2(account.get_ram_mut(S1_OFFSET, 2), s1);
        },
        2 => {  // ~ 21000
            let s0 = read_fq2(account.get_ram(S0_OFFSET, 2));
            let s1 = read_fq2(account.get_ram(S1_OFFSET, 2));
            let s2 = read_fq2(account.get_ram(S2_OFFSET, 2));

            let a1 = f.c2 * &s1;
            let a2 = f.c1 * &s2;
            let mut a3 = a1 + &a2;
            a3 = Fp6ParamsWrapper::<Fq6Parameters>::mul_base_field_by_nonresidue(&a3);
            let t6 = f.c0 * &s0 + &a3;  // ~ 6467
            if t6.is_zero() { panic!() }

            write_fq2(account.get_ram_mut(T6_OFFSET, 2), t6);
        },
        3 => {  // ~ 3346
            let t6 = read_fq2(account.get_ram(T6_OFFSET, 2));
    
            let v1a = t6.c1.square();
            let v2a = t6.c0.square();
            let v0a = Fp2ParamsWrapper::<Fq2Parameters>::sub_and_mul_base_field_by_nonresidue(&v2a, &v1a); // ~ 125
    
            write_fq(account.get_ram_mut(V0A_OFFSET, 1), v0a);
        },
        4 => {  // ~ 64678 - 100.000
            let mut v0a = read_fq(account.get_ram(V0A_OFFSET, 1));

            v0a = v0a.inverse().unwrap();

            write_fq(account.get_ram_mut(V0A_OFFSET, 1), v0a);
        },
        5 => {   // ~ 23000
            let mut t6 = read_fq2(account.get_ram(T6_OFFSET, 2));
            let v0a = read_fq(account.get_ram(V0A_OFFSET, 2));
            let s0 = read_fq2(account.get_ram(S0_OFFSET, 2));
            let s1 = read_fq2(account.get_ram(S1_OFFSET, 2));
            let s2 = read_fq2(account.get_ram(S2_OFFSET, 2));
    
            let c0 = t6.c0 * &v0a;    // ~ 1904
            let c1 = -(t6.c1 * &v0a); // ~ 1949
            t6 = Fq2::new(c0, c1);
            let c0 = t6 * &s0;  // ~ 6000
            let c1 = t6 * &s1;  // ~ 6000
            let c2 = t6 * &s2;  // ~ 6000
            let v1 = Fq6::new(c0, c1, c2);
    
            write_fq6(account.get_ram_mut(V1_OFFSET, 6), v1);
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
) -> Fq12 {
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
    *f
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
/// ### RAM usage:
/// - fe
/// - f_inverse
fn exp_by_neg_x(
    f: Fq12,
    account: &mut ProofVerificationAccount,
    offset: usize,
    round: usize,
) -> Fq12 {
    let mut res = f;
    match round {
        0 => {
            let mut fe_inverse = f;
            fe_inverse.conjugate();

            write_fq12(&mut account.get_ram_mut(offset, 12), f);
            write_fq12(&mut account.get_ram_mut(offset + 12, 12), fe_inverse);

            Fq12::one()
        },
        1..=CYCLOTOMIC_ROUNDS_LEN => { // Cyclotomic expression
            let fe = read_fq12(account.get_ram(offset, 12));
            let fe_inverse = read_fq12(account.get_ram(offset + 12, 12));
            let (lower_round, upper_round) = CYCLOTOMIC_ROUNDS[round - 1];
            
            let mut res = f;

            for i in lower_round..upper_round {
                let sub_round = i % CYCLOTOMIC_EXPRESSION_SUB_ROUND_COUNT;
                let i = i / CYCLOTOMIC_EXPRESSION_SUB_ROUND_COUNT;
                let value = X_WNAF[X_WNAF_L - 1 - i];

                if sub_round == 0 {
                    if i > 0 {
                        res.cyclotomic_square_in_place();
                    }
                } else {
                    if value > 0 {
                        f12_mul_assign(&mut res, &fe, account, offset + 24, sub_round - 1);
                    } else if value < 0 {
                        f12_mul_assign(&mut res, &fe_inverse, account, offset + 24, sub_round - 1);
                    }
                }
            }

            res
        },
        CYCLOTOMIC_ROUNDS_LEN_PLUS_ONE => {
            let mut res = f;
            res.conjugate();
            res
        },
        _ => { f }
    }
}

const F12_MUL_ROUND_COUNT: usize = 5;

const F12_MUL_V0_LOFFSET: usize = 0;
const F12_MUL_V1_LOFFSET: usize = 6;

// Karatsuba multiplication;
// Guide to Pairing-based cryprography, Algorithm 5.16.
/// [20000, 20000, 20000, 20000, 46000]
fn f12_mul_assign(
    a: &mut Fq12,
    b: &Fq12,
    account: &mut ProofVerificationAccount,
    ram_offset: usize,
    round: usize,
) {
    // ~ 42000
    match round {
        0 => {
            let v0 = f6_mul(a.c0, b.c0, Fq6::zero(), 0);
            write_fq6(account.get_ram_mut(F12_MUL_V0_LOFFSET + ram_offset, 6), v0);
        },
        1 => {
            let mut v0 = read_fq6(account.get_ram_mut(F12_MUL_V0_LOFFSET + ram_offset, 6));
            v0 = f6_mul(a.c0, b.c0, v0, 1);
            write_fq6(account.get_ram_mut(F12_MUL_V0_LOFFSET + ram_offset, 6), v0);
        },
        2 => {
            let v1 = f6_mul(a.c1, b.c1, Fq6::zero(), 0);
            write_fq6(account.get_ram_mut(F12_MUL_V1_LOFFSET + ram_offset, 6), v1);
        },
        3 => {
            let mut v1 = read_fq6(account.get_ram_mut(F12_MUL_V1_LOFFSET + ram_offset, 6));
            v1 = f6_mul(a.c1, b.c1, v1, 1);
            write_fq6(account.get_ram_mut(F12_MUL_V1_LOFFSET + ram_offset, 6), v1);
        },
        4 => {
            let v0 = read_fq6(account.get_ram_mut(F12_MUL_V0_LOFFSET + ram_offset, 6));
            let v1 = read_fq6(account.get_ram_mut(F12_MUL_V1_LOFFSET + ram_offset, 6));

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
    use ark_bn254::{ Fq, Bn254 };

    #[test]
    pub fn test_f12_inverse() {
        let f = get_f();
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();

        for round in 0..F12_INVERSE_ROUND_COUNT {
            f12_inverse(&f, &mut account, round);
        }

        let expected = f.inverse().unwrap();
        let result = read_fq12(account.get_ram(F2_OFFSET, 12));

        assert_eq!(result, expected);
    }

    #[test]
    pub fn test_f12_mul_assign() {
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();

        let expected = get_f() * get_f();

        let mut result = get_f();
        for round in 0..F12_MUL_ROUND_COUNT {
            f12_mul_assign(&mut result, &get_f(), &mut account, 0, round);
        }

        assert_eq!(result, expected);
    }

    #[test]
    pub fn test_f12_frobenius_map() {
        let mut result = get_f();
        for round in 0..F12_FROBENIUS_MAP_ROUND_COUNT {
            result = f12_frobenius_map(&mut result, 3, round);
        }

        let mut expected = get_f();
        expected.frobenius_map(3);

        assert_eq!(result, expected);
    }

    #[test]
    pub fn test_exp_by_neg_x() {
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();

        let mut result = get_f();
        for round in 0..EXP_BY_NEG_X_ROUND_COUNT {
            result = exp_by_neg_x(result, &mut account, 0, round);
        }

        let expected = original_exp_by_neg_x(get_f());

        assert_eq!(result, expected);
    }

    #[test]
    pub fn test_final_exponentiation() {
        let f = get_f();
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();
        write_fq12(account.get_ram_mut(F_OFFSET, 12), f);
        assert_eq!(f, read_fq12(account.get_ram_mut(F_OFFSET, 12)));

        let expected = Bn254::final_exponentiation(&f).unwrap();
            
        let result = final_exponentiation(&mut account);

        assert_eq!(result, expected);
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