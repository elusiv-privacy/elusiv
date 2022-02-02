
    /*let qap = miller_loop(
        [
            ( proof.a.into(), proof.b.into() ),
            ( prepared_inputs.into(), gamma_g2_neg_pc() ),
            ( proof.c.into(), delta_g2_neg_pc() ),
        ]
        .iter(),
    );

    let test = final_exponentiation(&qap).unwrap();

    test == alpha_g1_beta_g2()*/

const X: &'static [u64] = &[4965661367192848881];
const ATE_LOOP_COUNT: &'static [i8] = &[
    0, 0, 0, 1, 0, 1, 0, -1, 0, 0, 1, -1, 0, 0, 1, 0, 0, 1, 1, 0, -1, 0, 0, 1, 0, -1, 0, 0, 0,
    0, 1, 1, 1, 0, 0, -1, 0, 0, 1, 0, 0, 0, 0, 0, -1, 0, 0, 1, 1, 0, 0, -1, 0, 0, 0, 1, 1, 0,
    -1, 0, 0, 1, 0, 1, 1,
];
pub const G2_ELL_COEFF_COUNT: usize = 92;
// 64 + 2 + 26 = 92

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
        if bit == 1 {
            for &mut (p, ref mut coeffs) in &mut pairs {
                ell(&mut f, coeffs.next().unwrap(), &p.0);
            }
        } else if bit == -1 {
            for &mut (p, ref mut coeffs) in &mut pairs {
                ell(&mut f, coeffs.next().unwrap(), &p.0);
            }
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


use super::super::storage_account::*;

use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use super::super::error::ElusivError::{ InvalidStorageAccount, InvalidStorageAccountSize };
use super::super::scalar::*;
use byteorder::{ ByteOrder, LittleEndian };
use ark_bn254::G1Affine;
use ark_bn254::{ Parameters, Fq };
use ark_ec::models::bn::{ g1::G1Prepared, g2::G2Prepared };

const G1_SIZE: usize = 65;
const PREPARED_G2_SIZE: usize = super::verify::G2_ELL_COEFF_COUNT * 6 * 32 + 1;

solana_program::declare_id!("746Em3pvd2Rd2L3BRZ31RJ5qukorCiAw4kpudFkxgyBy");

pub struct ProofVerificationAccount<'a> {
    /// Public inputs
    pub input_bits_be: &'a mut [u8],

    /// Amount (8 bytes)
    amount: &'a mut [u8],

    /// Nullifier hash (32 bytes)
    nullifier_hash: &'a mut [u8],

    /// Prepared A (G1Affine)
    prepared_a: &'a mut [u8],

    /// Prepared B (Prepared G2Affine)
    /// - G2_ELL_COEFF_COUNT * 3 * 32 elements
    /// - inifinity byte
    prepared_b: &'a mut [u8],

    /// Prepared C (G1Affine)
    prepared_c: &'a mut [u8],

    /// Proof iteraction of current 
    /// - (u16 represented as 2 bytes)
    current_iteration: &'a mut [u8],

    /// Prepared inputs
    /// - x: 32 bytes
    /// - y: 32 bytes
    /// - infinity: boolean byte
    pub prepared_inputs: &'a mut [u8],
    pub prepared_product: &'a mut [u8],
}

impl<'a> ProofVerificationAccount<'a> {
    pub const TOTAL_SIZE: usize = 8 + 32 + G1_SIZE + PREPARED_G2_SIZE + G1_SIZE + 2 + G1_SIZE;

    pub fn new(
        account_info: &solana_program::account_info::AccountInfo,
        data: &'a mut [u8],
        program_id: &solana_program::pubkey::Pubkey,
    ) -> Result<Self, ProgramError> {
        if account_info.owner != program_id { return Err(InvalidStorageAccount.into()); }
        if !account_info.is_writable { return Err(InvalidStorageAccount.into()); }
        //if *account_info.key != id() { return Err(InvalidStorageAccount.into()); }

        Self::from_data(data) 
    }

    pub fn from_data(data: &'a mut [u8]) -> Result<Self, ProgramError> {
        if data.len() != Self::TOTAL_SIZE { return Err(InvalidStorageAccountSize.into()); }

        let (amount, data) = data.split_at_mut(8);
        let (nullifier_hash, data) = data.split_at_mut(32);
        let (prepared_a, data) = data.split_at_mut(G1_SIZE);
        let (prepared_b, data) = data.split_at_mut(PREPARED_G2_SIZE);
        let (prepared_c, data) = data.split_at_mut(G1_SIZE);
        let (current_iteration, data) = data.split_at_mut(2);
        let (prepared_inputs, _) = data.split_at_mut(G1_SIZE);

        Ok(
            ProofVerificationAccount {
                amount,
                nullifier_hash,
                prepared_a,
                prepared_b,
                prepared_c,
                current_iteration,
                prepared_inputs,
            }
        )
    }
}

impl<'a> ProofVerificationAccount<'a> {

}

impl<'a> ProofVerificationAccount<'a> {
    pub fn get_amount(&self) -> u64 {
        LittleEndian::read_u64(&self.amount)
    }
    pub fn set_amount(&mut self, amount: u64) {
        let bytes = u64::to_le_bytes(amount);
        for i in 0..8 {
            self.amount[i] = bytes[i];
        }
    }

    pub fn get_nullifier_hash(&self) -> ScalarLimbs {
        bytes_to_limbs(&self.nullifier_hash)
    }
    pub fn set_nullifier_hash(&mut self, bytes: &[u8]) -> ProgramResult {
        set(&mut self.nullifier_hash, 0, 32, bytes)
    }

    pub fn get_current_iteration(&self) -> usize { bytes_to_u16(self.current_iteration) as usize }
    pub fn set_current_iteration(&mut self, round: usize) {
        let round = round as u16;
        let bytes = round.to_le_bytes();
        self.current_iteration[0] = bytes[0];
        self.current_iteration[1] = bytes[1];
    }

    pub fn set_prepared_inputs(&mut self, pis: G1Affine) -> ProgramResult {
        let bytes = write_g1_affine(pis);
        set(&mut self.prepared_inputs, 0, G1_SIZE, &bytes)
    }
    pub fn get_prepared_inputs(&self) -> G1Affine {
        read_g1_affine(&self.prepared_inputs)
    }
}

impl<'a> ProofVerificationAccount<'a> {
    pub fn get_prepared_a(&self) -> G1Prepared<Parameters> {
        read_g1_affine(&self.prepared_a).into()
    }
    pub fn set_prepared_a(&mut self, a: G1Affine) -> ProgramResult {
        set(&mut self.prepared_a, 0, G1_SIZE, &write_g1_affine(a))
    }

    pub fn get_prepared_b_infinity(&self) -> bool {
        self.prepared_b[PREPARED_G2_SIZE - 1] == 1
    }
    pub fn get_prepared_b_coeff(&self, index: usize) -> (Fq, Fq, Fq) {
        let base = index * 32;
        (
            read_le_montgomery(&self.prepared_b[base..base + 32]),
            read_le_montgomery(&self.prepared_b[base + 32..base + 64]),
            read_le_montgomery(&self.prepared_b[base + 64..base + 96]),
        )
    }
    pub fn set_prepared_b(&mut self, b: G2Prepared<Parameters>) -> ProgramResult {
        let mut bytes = Vec::new();
        for coeff in b.ell_coeffs {
            bytes.extend(write_fq2_le_montgomery(coeff.0));
            bytes.extend(write_fq2_le_montgomery(coeff.1));
            bytes.extend(write_fq2_le_montgomery(coeff.2));
        }
        bytes.push(if b.infinity { 1 } else { 0 });
        set(&mut self.prepared_b, 0, PREPARED_G2_SIZE, &bytes)
    }

    pub fn get_prepared_c(&self) -> G1Prepared<Parameters> {
        read_g1_affine(&self.prepared_c).into()
    }
    pub fn set_prepared_c(&mut self, c: G1Affine) -> ProgramResult {
        set(&mut self.prepared_a, 0, G1_SIZE, &write_g1_affine(c))
    }
}

#[cfg(test)]
mod tests {
    type StorageAccount<'a> = super::ProofVerificationAccount<'a>;

    #[test]
    fn test_correct_size() {
        let mut data = [0; StorageAccount::TOTAL_SIZE];
        StorageAccount::from_data(&mut data).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_invalid_size() {
        let mut data = [0; StorageAccount::TOTAL_SIZE - 1];
        StorageAccount::from_data(&mut data).unwrap();
    }
}


