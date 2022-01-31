use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use ark_bn254::{ G1Affine, G1Projective };
use ark_ec::{
    ProjectiveCurve,
};
use ark_ff::*;
use core::ops::{ AddAssign };
use super::gamma_abc_g1;
use super::super::scalar::*;
use super::super::state::ProofVerificationAccount;

pub const PREPARATION_ITERATIONS: usize = 66;
pub const PREPARATION_ITERATION_ROUNDS: [usize; PREPARATION_ITERATIONS] = [
    4, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 7, 5,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 7, 1,
];

/// Prepares `INPUTS_COUNT` public inputs (into one `G1Affine`)
/// - requires `PREPARATION_ITERATIONS` calls to complete
pub fn partial_prepare_inputs(account: &mut ProofVerificationAccount, iteration: usize) -> ProgramResult {
    let round = account.get_current_round();
    let rounds = PREPARATION_ITERATION_ROUNDS[iteration];
    let iteration = iteration / 33;

    // Multiplication of gamma_abc_g1[i + 1] and input[i]
    let product = partial_mul_g1a_scalar(
        &gamma_abc_g1()[iteration + 1],
        read_g1_projective(&account.p_product),
        account.get_input_bits(iteration),
        round,
        rounds,
    )?;
    write_g1_projective(&mut account.p_product, product);

    // Add the product to g_ic after mul is finished
    // ~ 36300 CUs
    if round + rounds == 256 {
        let mut g_ic = if iteration == 0 { super::gamma_abc_g1_0() } else { read_g1_projective(&account.p_inputs) };
        g_ic.add_assign(product);
        write_g1_projective(&mut account.p_inputs, g_ic);

        account.set_current_round(0);
    } else {
        account.set_current_round(round + rounds);
    }

    Ok(())
}

pub const MUL_G1A_SCALAR_ROUNDS: usize = 256;

/// Multiplies a `G1Affine` with a `Scalar`
/// - requires MUL_G1A_SCALAR_ITERATIONS calls to complete
/// - `scalar_bits` needs to be supplied in the state's `encode_bits` format
/// - 1 round: ~ 24608 CUs
pub fn partial_mul_g1a_scalar(
    g1a: &G1Affine,
    acc: G1Projective,
    scalar_bits: [u8; 256],
    base_round: usize,
    rounds: usize,
) -> Result<G1Projective, ProgramError> {
    let mut acc = if base_round == 0 { G1Projective::zero() } else { acc };

    for r in base_round..base_round + rounds {
        // Leading zeros (encoded with `2`) are ignored
        if scalar_bits[r] == 2 { continue; }

        // Multiplication core
        acc.double_in_place();
        if scalar_bits[r] == 1 {
            acc.add_assign_mixed(g1a);
        }
    }

    Ok(acc)
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
        let scalar_bits = vec_to_array_256(super::super::state::bit_encode(vec_to_array_32(to_bytes_le_repr(scalar))));

        let mut res = G1Projective::zero();
        let mut round = 0;
        for i in 0..PREPARATION_ITERATIONS {
            let rounds = PREPARATION_ITERATION_ROUNDS[i];
            res = partial_mul_g1a_scalar(&g1a, res, scalar_bits, round, rounds).unwrap();

            round += rounds;
            if round == 256 { round = 0; }
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
        account.init(vec!
            [
                vec_to_array_32(to_bytes_le_repr(inputs[0])),
                vec_to_array_32(to_bytes_le_repr(inputs[1]))
            ],
            0, [0,0,0,0]
        ).unwrap();

        // Result
        for i in 0..PREPARATION_ITERATIONS {
            partial_prepare_inputs(&mut account, i).unwrap();
        }

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
            gamma_g2_neg_pc: gamma_g2_neg_pc(),
            delta_g2_neg_pc: delta_g2_neg_pc(),
        };
        let expect = ark_groth16::prepare_inputs(&pvk, &inputs).unwrap();

        assert_eq!(
            read_g1_projective(&account.p_inputs),
            expect
        );
    }
}