use ark_ff::Zero;
use ark_bn254::Fr;
use super::poseidon_constants::*;

type Scalar = Fr;

macro_rules! matrix_mix {
    ($new_state: ident, $i: literal, $state: ident) => {
        $new_state[$i] += matrices[$i * 3] * $state[0];
        $new_state[$i] += matrices[$i * 3 + 1] * $state[1];
        $new_state[$i] += matrices[$i * 3 + 2] * $state[2];
    };
}

/// Computes the Poseidon Hash for two input values over multiple calls
/// - our Poseidon parameters: Sbox: 0, Cells: 2, RF: 8, RP: 56 (https://eprint.iacr.org/2019/458.pdf (table 2, table 8))
/// - so for arity 2 we have 8 full rounds and 57 partial rounds
/// - in our implementation we use two types of rounds: computation rounds and Poseidon rounds
/// - circom javascript reference implementation: https://github.com/iden3/circomlibjs/blob/9300d3f820b40a16d2f342ab5127a0cb9090bd15/src/poseidon_reference.js#L27
pub fn binary_poseidon_hash_partial(round: usize, state: &mut [Scalar; 3]) {
    let c = generate_constants(iteration);
    let iteration = get_iteration_start_and_length(iteration);

    // Poseidon round
    let pr = round / 65;

    // Ark
    state[0] += c[pr][0];
    state[1] += c[pr][1];
    state[2] += c[pr][2];

    // Sbox
    if round < 4 || round >= 61 { // First and last full rounds
        for j in 0..3 {
            let aux = state[j];
            state[j] = state[j].square();
            state[j] = state[j].square();
            state[j] *= &aux;
        }
    } else {
        // Middle partial rounds
        let aux = state[0];
        state[0] = state[0].square();
        state[0] = state[0].square();
        state[0] *= &aux;
    }

    // Mix 
    let mut new_state = [Scalar::zero(); 3];
    matrix_mix!(new_state, 0, state);
    matrix_mix!(new_state, 1, state);
    matrix_mix!(new_state, 2, state);
    state = new_state;
}