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
    let f = read_fq12(account.get_ram(F_OFFSET, 12));

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
    f12_frobenius_map(&mut r, 2);

    r *= &f2;   // ~ 131961

    let y0 = exp_by_neg_x(r);   // ~ 6_006_136
    
    let y1 = cyclotomic_square(y0);    // ~ 45634
    let y2 = cyclotomic_square(y1);    // ~ 45569
    let mut y3 = y2 * &y1;  // ~ 132119
    let y4 = exp_by_neg_x(y3);  // ~ 6_009_534
    let y5 = cyclotomic_square(y4);
    let mut y6 = exp_by_neg_x(y5);
    y3.conjugate();
    y6.conjugate();
    let y7 = y6 * &y4;
    let mut y8 = y7 * &y3;
    let y9 = y8 * &y1;
    let y10 = y8 * &y4;
    let y11 = y10 * &r;
    let mut y12 = y9;
    f12_frobenius_map(&mut y12, 1);
    let y13 = y12 * &y11;
    f12_frobenius_map(&mut y8, 2);
    let y14 = y8 * &y13;
    r.conjugate();
    let mut y15 = r * &y9;
    f12_frobenius_map(&mut y15, 3);
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
    let mut result = f;
    result.cyclotomic_square_in_place();
    result
}

fn f12_frobenius_map(f: &mut Fq12, power: usize) {
    f6_frobenius_map(&mut f.c0, power);
    f6_frobenius_map(&mut f.c1, power);
    f.c1.mul_assign_by_fp2(Fq12Parameters::FROBENIUS_COEFF_FP12_C1[power % 12]);
}

fn f6_frobenius_map(f: &mut Fq6, power: usize) {
    f2_frobenius_map(&mut f.c0, power);
    f2_frobenius_map(&mut f.c1, power);
    f2_frobenius_map(&mut f.c2, power);
    f.c1 *= &Fq6Parameters::FROBENIUS_COEFF_FP6_C1[power % 6];
    f.c2 *= &Fq6Parameters::FROBENIUS_COEFF_FP6_C2[power % 6];
}

fn f2_frobenius_map(f: &mut Fq2, power: usize) {
    f.c1 *= &Fq2Parameters::FROBENIUS_COEFF_FP2_C1[power % 2];
}

fn exp_by_neg_x(mut f: Fq12) -> Fq12 {
    f = cyclotomic_exp(&f, Parameters::X);
    f.conjugate();
    f
}

fn cyclotomic_exp(fe: &Fq12, exponent: impl AsRef<[u64]>) -> Fq12 {
    let mut res = Fq12::one();
    let mut fe_inverse = *fe;
    fe_inverse.conjugate();

    let mut found_nonzero = false;
    let naf = find_wnaf(exponent.as_ref()); // ~ 17213

    // 130504
    // 45286
    // 45885
    //
    // 177466
    //let mut i = 0;
    //for &value in naf.iter().rev() {
    for i in 0..WNAF_SIZE {
        let value = naf[WNAF_SIZE - i - 1];

        // ~ 45281
        if found_nonzero { 
            res.cyclotomic_square_in_place();
        }

        if value != 0 {
            found_nonzero = true;

            if value > 0 {  // 132364
                res *= fe;
            } else {    // 132044
                res *= &fe_inverse;
            }
        }
    }
    res
}

const WNAF_SIZE: usize = 63;

// What is the max WNAF length? (guess: 64 or 63)
pub fn find_wnaf(num: &[u64]) -> Vec<i64> {
    let is_zero = |num: &[u64]| num.iter().all(|x| *x == 0u64);
    let is_odd = |num: &[u64]| num[0] & 1 == 1;
    let sub_noborrow = |num: &mut [u64], z: u64| {
        let mut other = vec![0u64; num.len()];
        other[0] = z;
        let mut borrow = 0;

        for (a, b) in num.iter_mut().zip(other) {
            *a = sbb(*a, b, &mut borrow);
        }
    };
    let add_nocarry = |num: &mut [u64], z: u64| {
        let mut other = vec![0u64; num.len()];
        other[0] = z;
        let mut carry = 0;

        for (a, b) in num.iter_mut().zip(other) {
            *a = adc(*a, b, &mut carry);
        }
    };
    let div2 = |num: &mut [u64]| {
        let mut t = 0;
        for i in num.iter_mut().rev() {
            let t2 = *i << 63;
            *i >>= 1;
            *i |= t;
            t = t2;
        }
    };

    let mut num = num.to_vec();
    let mut res = vec![];

    while !is_zero(&num) {
        let z: i64;
        if is_odd(&num) {
            z = 2 - (num[0] % 4) as i64;
            if z >= 0 {
                sub_noborrow(&mut num, z as u64)
            } else {
                add_nocarry(&mut num, (-z) as u64)
            }
        } else {
            z = 0;
        }
        res.push(z);
        div2(&mut num);
    }

    res
}

/// Calculate a + (b * c) + carry
fn adc(a: u64, b: u64, carry: &mut u64) -> u64 {
    let tmp = (a as u128) + (b as u128) + (*carry as u128);
    *carry = (tmp >> 64) as u64;
    tmp as u64
}

/// Calculate a - b - borrow
fn sbb(a: u64, b: u64, borrow: &mut u64) -> u64 {
    let tmp = (1u128 << 64) + (a as u128) - (b as u128) - (*borrow as u128);
    *borrow = if tmp >> 64 == 0 { 1 } else { 0 };
    tmp as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use ark_ec::PairingEngine;
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
}