use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use ark_bn254::{ Fq, G1Affine, G1Projective };
use ark_ec::{
    ProjectiveCurve,
};
use ark_ff::*;
use core::ops::{ AddAssign };
use super::gamma_abc_g1;
use super::state::*;

pub const PREPARE_INPUTS_ITERATIONS: usize = 66;
pub const PREPARE_INPUTS_ROUNDS: [usize; PREPARE_INPUTS_ITERATIONS] = [
    3, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 7, 6,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 7, 1,
];
const ZERO: Fq = field_new!(Fq, "0");

/// Prepares `INPUTS_COUNT` public inputs (into one `G1Affine`)
/// - requires `PREPARATION_ITERATIONS` calls to complete
pub fn partial_prepare_inputs(
    account: &mut ProofVerificationAccount,
    iteration: usize
) -> ProgramResult {

    let round = account.get_round();
    let rounds = PREPARE_INPUTS_ROUNDS[iteration];
    let i = iteration / 33;

    let mut product = pop_g1_projective(account);

    // Multiplication of gamma_abc_g1[i + 1] and input[i]
    // ~ rounds * 24608 CUs
    partial_mul_g1a_scalar(
        &gamma_abc_g1()[i + 1],
        &mut product,
        &account.inputs_be[i * 32..(i + 1) * 32],
        round,
        rounds,
    )?;
    //write_g1_projective(&mut account.get_ram_mut(0, 3), product);
    push_g1_projective(account, product);

    // Add the product to g_ic after mul is finished
    // ~ 36300 CUs
    if round + rounds == 256 {
        // Move g_ic to the top of the stack
        let mut g_ic = get_gic(account);

        g_ic.add_assign(product);
        push_g1_projective(account, g_ic);

        account.set_round(0);

        // Push null product acc
        account.push_fq(ZERO);
        account.push_fq(ZERO);
        account.push_fq(ZERO);
    } else {
        account.set_round(round + rounds);
    }

    // Convert value from projective to affine form after last iteration
    if iteration == PREPARE_INPUTS_ITERATIONS - 1 {
        let v = get_gic(account);
        push_g1_affine(account, v.into());
    }

    Ok(())
}

pub const MUL_G1A_SCALAR_ROUNDS: usize = 256;

/// Multiplies a `G1Affine` with a `Scalar`
/// - requires MUL_G1A_SCALAR_ITERATIONS calls to complete
/// - 1 round: ~ 24608 CUs
pub fn partial_mul_g1a_scalar(
    g1a: &G1Affine,
    acc: &mut G1Projective,
    bytes_be: &[u8],
    base_round: usize,
    rounds: usize,
) -> Result<(), ProgramError> {
    let first_non_zero = find_first_non_zero(bytes_be);

    for r in base_round..base_round + rounds {
        if r < first_non_zero { continue; }

        // Multiplication core
        acc.double_in_place();
        if get_bit(bytes_be, r / 8, 7 - (r % 8)) {
            acc.add_assign_mixed(g1a);
        }
    }

    Ok(())
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

#[inline(always)]
/// Returns true if the bit is 1
fn get_bit(bytes_be: &[u8], byte: usize, bit: usize) -> bool {
    (bytes_be[byte] >> bit) & 1 == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::vkey::*;
    use ark_groth16::{
        VerifyingKey,
        PreparedVerifyingKey,
    };
    use ark_ec::AffineCurve;
    use ark_bn254::{ G2Affine, G1Affine };
    use core::ops::Neg;
    use super::super::super::scalar::*;

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
            partial_mul_g1a_scalar(&g1a, &mut res, &scalar_bits, round, 1).unwrap();
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
        let mut data = vec![0; ProofVerificationAccount::TOTAL_SIZE];
        let mut account = ProofVerificationAccount::from_data(&mut data).unwrap();
        account.init(
            vec![
                vec_to_array_32(to_bytes_le_repr(inputs[0])),
                vec_to_array_32(to_bytes_le_repr(inputs[1]))
            ],
            0, [0,0,0,0],
            super::super::Proof{ a: G1Affine::zero(), b: G2Affine::zero(), c: G1Affine::zero() }
        ).unwrap();

        // Result
        for i in 0..PREPARE_INPUTS_ITERATIONS {
            partial_prepare_inputs(&mut account, i).unwrap();
        }
        let result = account.get_prepared_inputs();
        account.stack_fq.pop_empty();
        account.stack_fq.pop_empty();
        account.stack_fq.pop_empty();

        // ark_groth16 result
        let vk: VerifyingKey<ark_bn254::Bn254> = VerifyingKey {
            alpha_g1: alpha_g1(),
            beta_g2: beta_g2(),
            gamma_g2: gamma_g2(),
            delta_g2: delta_g2(),
            gamma_abc_g1: gamma_abc_g1(),
        };
        let pvk = PreparedVerifyingKey {
            vk,
            alpha_g1_beta_g2: alpha_g1_beta_g2(),
            gamma_g2_neg_pc: gamma_g2().neg().into(),
            delta_g2_neg_pc: gamma_g2().neg().into(),
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
    fn assert_stack_is_cleared(account: &ProofVerificationAccount) {
        assert_eq!(account.stack_fq.stack_pointer, 0);
    }
}