use super::{
    Proof,
    alpha_g1_beta_g2,
    gamma_abc_g1,
    gamma_g2_neg_pc,
    delta_g2_neg_pc,
};
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
use ark_ff::{
    One,
    fields::Field,
    fields::PrimeField,
    BigInteger256,
    bytes::ToBytes,
};
use core::ops::{ AddAssign };

pub fn full_verification(proof: Proof, public_inputs: &[Fr]) -> bool {
    let pis = prepare_inputs(gamma_abc_g1(), public_inputs);
    for i in 0..super::ITERATIONS {
        partial_verification(i);
    }
    final_verification()
}

pub fn prepare_inputs(gamma_abc_g1: Vec<G1Affine>, public_inputs: &[Fr]) -> G1Affine {
    let mut g_ic = gamma_abc_g1[0].into_projective();
    for (i, b) in public_inputs.iter().zip(gamma_abc_g1.iter().skip(1)) {
        g_ic.add_assign(&b.mul(i.into_repr()));
    }
    g_ic.into_affine()
}

pub fn prepare_proof(proof: Proof) -> (G1Prepared<Parameters>, G2Prepared<Parameters>, G1Prepared<Parameters>) {
    (proof.a.into(), proof.b.into(), proof.c.into())    
}

pub fn partial_verification(
    iteration: usize,
) {

}

pub fn final_verification() -> bool {
    let qap = miller_loop(
        [
            ( proof.a.into(), proof.b.into() ),
            ( prepared_inputs.into(), gamma_g2_neg_pc() ),
            ( proof.c.into(), delta_g2_neg_pc() ),
        ]
        .iter(),
    );

    let test = final_exponentiation(&qap).unwrap();

    test == alpha_g1_beta_g2()
}

const X: &'static [u64] = &[4965661367192848881];
const ATE_LOOP_COUNT: &'static [i8] = &[
    0, 0, 0, 1, 0, 1, 0, -1, 0, 0, 1, -1, 0, 0, 1, 0, 0, 1, 1, 0, -1, 0, 0, 1, 0, -1, 0, 0, 0,
    0, 1, 1, 1, 0, 0, -1, 0, 0, 1, 0, 0, 0, 0, 0, -1, 0, 0, 1, 1, 0, 0, -1, 0, 0, 0, 1, 1, 0,
    -1, 0, 0, 1, 0, 1, 1,
];

fn miller_loop<'a, I>(i: I) -> Fq12
where
    I: IntoIterator<Item = &'a (G1Prepared<Parameters>, G2Prepared<Parameters>)>,
{
    // p in G1P and q in G2P
    // pushes (p, (c0, c1, c2)) 
    // p and three coefficients of the line evaluations as calculated in
    // -> No real computation
    let mut pairs = vec![];
    // 3 iterations
    for (p, q) in i {
        if !p.is_zero() && !q.is_zero() {
            pairs.push((p, q.ell_coeffs.iter()));
        }
    }

    // Start f of with value 1 (in Fq12 (2 Fq6 (3 Fq2 (2 Fq))))
    let mut f = Fq12::one();

    // i in 65..1 -> 64 iterations
    for i in (1..ATE_LOOP_COUNT.len()).rev() {
        // Square f in every but the first iteration
        if i != ATE_LOOP_COUNT.len() - 1 {
            f.square_in_place();
        }

        // 3 ell calls
        for (p, ref mut coeffs) in &mut pairs {
            ell(&mut f, coeffs.next().unwrap(), &p.0);
        }

        let bit = ATE_LOOP_COUNT[i - 1];
        match bit {
            1 => {
                for &mut (p, ref mut coeffs) in &mut pairs {
                    ell(&mut f, coeffs.next().unwrap(), &p.0);
                }
            }
            -1 => {
                for &mut (p, ref mut coeffs) in &mut pairs {
                    ell(&mut f, coeffs.next().unwrap(), &p.0);
                }
            }
            _ => continue,
        }
    }

    for &mut (p, ref mut coeffs) in &mut pairs {
        ell(&mut f, coeffs.next().unwrap(), &p.0);
    }

    for &mut (p, ref mut coeffs) in &mut pairs {
        ell(&mut f, coeffs.next().unwrap(), &p.0);
    }

    f
}

fn final_exponentiation(f: &Fq12) -> Option<Fq12> {
    // Easy part: result = elt^((q^6-1)*(q^2+1)).
    // Follows, e.g., Beuchat et al page 9, by computing result as follows:
    //   elt^((q^6-1)*(q^2+1)) = (conj(elt) * elt^(-1))^(q^2+1)

    // f1 = r.conjugate() = f^(p^6)
    let mut f1 = *f;
    f1.conjugate();

    f.inverse().map(|mut f2| {
        // f2 = f^(-1);
        // r = f^(p^6 - 1)
        let mut r = f1 * &f2;

        // f2 = f^(p^6 - 1)
        f2 = r;
        // r = f^((p^6 - 1)(p^2))
        r.frobenius_map(2);

        // r = f^((p^6 - 1)(p^2) + (p^6 - 1))
        // r = f^((p^6 - 1)(p^2 + 1))
        r *= &f2;

        // Hard part follows Laura Fuentes-Castaneda et al. "Faster hashing to G2"
        // by computing:
        //
        // result = elt^(q^3 * (12*z^3 + 6z^2 + 4z - 1) +
        //               q^2 * (12*z^3 + 6z^2 + 6z) +
        //               q   * (12*z^3 + 6z^2 + 4z) +
        //               1   * (12*z^3 + 12z^2 + 6z + 1))
        // which equals
        //
        // result = elt^( 2z * ( 6z^2 + 3z + 1 ) * (q^4 - q^2 + 1)/r ).

        let y0 = exp_by_neg_x(r);
        let y1 = y0.cyclotomic_square();
        let y2 = y1.cyclotomic_square();
        let mut y3 = y2 * &y1;
        let y4 = exp_by_neg_x(y3);
        let y5 = y4.cyclotomic_square();
        let mut y6 = exp_by_neg_x(y5);
        y3.conjugate();
        y6.conjugate();
        let y7 = y6 * &y4;
        let mut y8 = y7 * &y3;
        let y9 = y8 * &y1;
        let y10 = y8 * &y4;
        let y11 = y10 * &r;
        let mut y12 = y9;
        y12.frobenius_map(1);
        let y13 = y12 * &y11;
        y8.frobenius_map(2);
        let y14 = y8 * &y13;
        r.conjugate();
        let mut y15 = r * &y9;
        y15.frobenius_map(3);
        let y16 = y15 * &y14;

        y16
    })
}

type EllCoeff<F> = (F, F, F);

/// Evaluates the line function at point p.
fn ell(f: &mut Fq12, coeffs: &EllCoeff<Fq2>, p: &G1Affine) {
    let mut c0 = coeffs.0;
    let mut c1 = coeffs.1;
    let c2 = coeffs.2;

    c0.mul_assign_by_fp(&p.y);
    c1.mul_assign_by_fp(&p.x);
    f.mul_by_034(&c0, &c1, &c2);
}

fn exp_by_neg_x(mut f: Fq12) -> Fq12 {
    f = f.cyclotomic_exp(&X);
    f.conjugate();
    f
}