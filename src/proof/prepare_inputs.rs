use solana_program::entrypoint::ProgramResult;
use ark_bn254::{ Fq, G1Affine, G1Projective };
use ark_ec::{
    ProjectiveCurve,
};
use ark_ff::*;
use core::ops::{ AddAssign };
use super::{state::*, VerificationKey};

//pub const PREPARE_INPUTS_ITERATIONS: usize = 104;
pub const PREPARE_INPUTS_BASE_ITERATIONS: usize = 52;
//pub const PREPARE_INPUTS_ROUNDS: [usize; PREPARE_INPUTS_ITERATIONS] = [3, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 1];

// Solution: at the moment we can just take 52 * PUBLIC_INPUTS_COUNt
// -> we can set the number of iterations per verification key

/// Prepares `INPUTS_COUNT` public inputs (into one `G1Affine`)
/// - requires `PREPARATION_ITERATIONS` calls to complete
pub fn partial_prepare_inputs<VKey: VerificationKey>(
    account: &mut ProofAccount,
    iteration: usize
) -> ProgramResult {

    let base_round = account.get_round() as usize;
    let rounds = VKey::prepapre_inputs_rounds()[iteration];

    for round in base_round..(base_round + rounds) {
        let input = round / (MUL_G1A_SCALAR_ROUNDS + 1);
        let round = round % (MUL_G1A_SCALAR_ROUNDS + 1);

        match round {
            // - pops: product (G1Projective, 3 Fqs)
            // - pushes: product (G1Projective)
            // Multiplication of gamma_abc_g1[input + 1] and input[input] (~ 34000 CUs)
            0..=MUL_G1A_SCALAR_ROUNDS_MINUS_ONE => {
                let mut product = pop_g1_projective(account);   // (~ 169 CUs)

                partial_mul_g1a_scalar(
                    &VKey::gamma_abc_g1()[input + 1],
                    &mut product,
                    &account.get_inputs_be(input),
                    round,
                );
    
                push_g1_projective(account, product);
            },

            // - pops: product, g_ic
            // - pushes: g_ic, empty product
            // Add the product to g_ic after mul is finished (~ 36300 CUs)
            MUL_G1A_SCALAR_ROUNDS => {
                // Pop product
                let product = pop_g1_projective(account);
                let mut g_ic = pop_g1_projective(account);

                g_ic.add_assign(product);

                // Convert value from projective to affine form after last iteration
                if iteration == VKey::PREPARE_INPUTS_ITERATIONS - 1 {
                    push_g1_affine(account, g_ic.into());
                } else {
                    push_g1_projective(account, g_ic);
                    push_g1_projective(account, G1Projective::zero());
                }
            }

            _ => {}
        }
    }

    account.set_round((base_round + rounds) as u64);

    Ok(())
}

pub const MUL_G1A_SCALAR_ROUNDS: usize = 256;
pub const MUL_G1A_SCALAR_ROUNDS_MINUS_ONE: usize = MUL_G1A_SCALAR_ROUNDS - 1;

/// Multiplies a `G1Affine` with a `Scalar`
/// - requires MUL_G1A_SCALAR_ITERATIONS calls to complete
/// - 1 round: ~ 34000 CUs
pub fn partial_mul_g1a_scalar(
    g1a: &G1Affine,
    acc: &mut G1Projective,
    bytes_be: &[u8],
    round: usize,
) {
    let first_non_zero = find_first_non_zero(bytes_be);

    if round < first_non_zero { return; }

    // Multiplication core
    double_in_place(acc); // ~ 13014 CUs
    if get_bit(bytes_be, round / 8, 7 - (round % 8)) {
        add_assign_mixed(acc, &g1a); // ~ 21000 CUs
    }
}

fn find_first_non_zero(bytes_be: &[u8]) -> usize {
    for byte in 0..32 {
        for bit in 0..8 {
            if get_bit(bytes_be, byte, bit) {
                return byte * 8 + bit;
            }
        }
    }
    return 256
}

fn double_in_place(g1p: &mut G1Projective) {
    if g1p.is_zero() { return; }

    let mut a = g1p.x.square();
    let b = g1p.y.square();
    let mut c = b.square();
    let d = ((g1p.x + &b).square() - &a - &c).double();
    let e = a + &*a.double_in_place();
    let f = e.square();
    g1p.z *= &g1p.y;
    g1p.z.double_in_place();
    g1p.x = f - &d - &d;
    g1p.y = (d - &g1p.x) * &e - &*c.double_in_place().double_in_place().double_in_place();
}

fn add_assign_mixed(g1p: &mut G1Projective, other: &G1Affine) {
    if other.is_zero() { return; }

    if g1p.is_zero() {
        g1p.x = other.x;
        g1p.y = other.y;
        g1p.z = Fq::one();
        return;
    }

    // (~ 7417 CUs)

    let z1z1 = g1p.z.square();
    let u2 = other.x * &z1z1;
    let s2 = (other.y * &g1p.z) * &z1z1;

    if g1p.x == u2 && g1p.y == s2 { // ~ 1314 CUs
        g1p.double_in_place();
    } else {    // ~ 13528 CUs
        // If we're adding -a and a together, self.z becomes zero as H becomes zero.

        let h = u2 - &g1p.x;
        let hh = h.square();
        let mut i = hh;
        i.double_in_place().double_in_place();
        let mut j = h * &i;
        let r = (s2 - &g1p.y).double();
        let v = g1p.x * &i;
        g1p.x = r.square();
        g1p.x -= &j;
        g1p.x -= &v;
        g1p.x -= &v;
        j *= &g1p.y; // J = 2*Y1*J
        j.double_in_place();
        g1p.y = v - &g1p.x;
        g1p.y *= &r;
        g1p.y -= &j;
        g1p.z += &h;
        g1p.z.square_in_place();
        g1p.z -= &z1z1;
        g1p.z -= &hh;
    }
}

#[inline(always)]
/// Returns true if the bit is 1
fn get_bit(bytes_be: &[u8], byte: usize, bit: usize) -> bool {
    (bytes_be[byte] >> bit) & 1 == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_groth16::{
        VerifyingKey,
        PreparedVerifyingKey,
    };
    use ark_ec::AffineCurve;
    use ark_bn254::{ G2Affine, G1Affine };
    use core::ops::Neg;
    use super::super::super::fields::scalar::*;
    use super::super::super::fields::utils::*;

    type VKey = crate::proof::vkey::SendVerificationKey;

    #[test]
    fn test_mul_g1a_scalar() {
        let g1a = G1Affine::from(
            G1Projective::new(
                BigInteger256([4442864439166756984, 4574045506909349437, 10701839041301083415, 1612794170237378160]).into(),
                BigInteger256([2454593247855632740, 17197849827163444358, 3273120395094234488, 3314060189894239153]).into(),
                BigInteger256([1, 0, 0, 0]).into(),
            )
        );
        let scalar = from_str_10("19526707366532583397322534596786476145393586591811230548888354920504818678603");
        let scalar_bits: Vec<u8> = to_bytes_le_repr(scalar).iter().copied().rev().collect();

        let mut res = G1Projective::zero();
        for round in 0..MUL_G1A_SCALAR_ROUNDS {
            partial_mul_g1a_scalar(&g1a, &mut res, &scalar_bits, round);
        }

        let expect = g1a.mul(scalar);

        assert_eq!(expect, res);
    }

    #[test]
    fn test_partial_prepare_inputs() {
        let inputs = [
            from_str_10("20643720223837027367320733428836459266646763523911772324593310161284187566894"),
            from_str_10("19526707366532583397322534596786476145393586591811230548888354920504818678603"),
        ];
        let mut data = vec![0; ProofAccount::TOTAL_SIZE];
        let mut account = ProofAccount::from_data(&mut data).unwrap();
        account.reset::<VKey>(
            super::super::Proof{ a: G1Affine::zero(), b: G2Affine::zero(), c: G1Affine::zero() },
            &[
                vec_to_array_32(to_bytes_le_repr(inputs[0])),
                vec_to_array_32(to_bytes_le_repr(inputs[1])),
            ],
        ).unwrap();

        // Result
        for i in 0..VKey::PREPARE_INPUTS_ITERATIONS {
            partial_prepare_inputs::<VKey>(&mut account, i).unwrap();
        }
        let result = pop_g1_affine(&mut account);

        // ark_groth16 result
        let vk: VerifyingKey<ark_bn254::Bn254> = VerifyingKey {
            alpha_g1: VKey::alpha_g1(),
            beta_g2: VKey::beta_g2(),
            gamma_g2: VKey::gamma_g2(),
            delta_g2: VKey::delta_g2(),
            gamma_abc_g1: VKey::gamma_abc_g1(),
        };
        let pvk = PreparedVerifyingKey {
            vk,
            alpha_g1_beta_g2: VKey::alpha_g1_beta_g2(),
            gamma_g2_neg_pc: VKey::gamma_g2().neg().into(),
            delta_g2_neg_pc: VKey::delta_g2().neg().into(),
        };
        let expect: G1Projective = ark_groth16::prepare_inputs(&pvk, &inputs).unwrap();

        assert_eq!(
            result,
            G1Affine::from(expect),
        );
        assert_stack_is_cleared(&account);
    }

    /// Stack convention:
    /// - every private function has to clear the local stack
    /// - public functions are allowed to return values on the stack
    fn assert_stack_is_cleared(account: &ProofAccount) {
        assert_eq!(account.fq.stack_pointer, 0);
    }
}