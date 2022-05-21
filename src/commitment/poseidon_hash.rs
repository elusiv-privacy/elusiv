use ark_ff::{Field, Zero};
use ark_bn254::Fr;
use super::poseidon_constants::*;

pub const TOTAL_POSEIDON_ROUNDS: usize = 65;

macro_rules! matrix_mix {
    ($new_state: ident, $s: literal, $i: literal, $state: ident) => {
        $new_state[$s] += MATRIX[$i] * $state[0];
        $new_state[$s] += MATRIX[$i + 1] * $state[1];
        $new_state[$s] += MATRIX[$i + 2] * $state[2];
    };
}

macro_rules! round {
    ($i: literal, $state: ident) => {
        {
            let aux = $state[$i];
            $state[$i] = $state[$i].square();
            $state[$i] = $state[$i].square();
            $state[$i] *= &aux;
        }
    };
}

/// Computes the Poseidon Hash for two input values over multiple calls
/// - for input arity 2 we have 8 full rounds and 57 partial rounds (recommended in: https://eprint.iacr.org/2019/458.pdf (table 2, table 8))
/// - in our implementation we use two types of rounds: computation rounds and Poseidon rounds
/// - circom javascript reference implementation: https://github.com/iden3/circomlibjs/blob/9300d3f820b40a16d2f342ab5127a0cb9090bd15/src/poseidon_reference.js#L27
pub fn binary_poseidon_hash_partial(round: usize, state: &mut [Fr; 3]) {
    // Load constants
    let constants = constants(round);

    // Ark
    state[0] += constants[0];
    state[1] += constants[1];
    state[2] += constants[2];

    // Sbox
    if round < 4 || round >= 61 { // First and last full rounds
        round!(0, state);
        round!(1, state);
        round!(2, state);
    } else { // Middle partial rounds
        round!(0, state);
    }

    // Mix 
    let mut new_state = [Fr::zero(); 3];
    matrix_mix!(new_state, 0, 0, state);
    matrix_mix!(new_state, 1, 3, state);
    matrix_mix!(new_state, 2, 6, state);

    *state = new_state;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn full_poseidon2_hash(a: Fr, b: Fr) -> Fr {
        let mut state = [Fr::zero(), a, b];
        for round in 0..TOTAL_POSEIDON_ROUNDS {
            binary_poseidon_hash_partial(round, &mut state);
        }
        state[0]
    }

    #[test]
    fn test_binary_poseidon_hash() {
        assert_eq!(
            full_poseidon2_hash(Fr::zero(), Fr::zero()),
            Fr::from_str("14744269619966411208579211824598458697587494354926760081771325075741142829156").unwrap(),
        );
    }
}