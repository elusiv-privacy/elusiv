use ark_ff::{Field, Zero};
use ark_bn254::Fr;
use super::poseidon_constants::*;

pub const TOTAL_POSEIDON_ROUNDS: u32 = 65;

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
pub fn binary_poseidon_hash_partial(round: u32, state: &mut [Fr; 3]) {
    // Load constants (~ 260 CUs)
    let constants = constants(round as usize);

    // Ark (~ 277 CUs)
    state[0] += constants[0];
    state[1] += constants[1];
    state[2] += constants[2];

    // Sbox
    if !(4..61).contains(&round) { // First and last full rounds (~ 15411 CUs)
        round!(0, state);
        round!(1, state);
        round!(2, state);
    } else { // Middle partial rounds (~ 5200 CUs)
        round!(0, state);
    }

    // Mix (~ 17740)
    let mut new_state = [Fr::zero(); 3];
    matrix_mix!(new_state, 0, 0, state);
    matrix_mix!(new_state, 1, 3, state);
    matrix_mix!(new_state, 2, 6, state);

    *state = new_state;
}

#[cfg(test)]
pub fn full_poseidon2_hash(a: Fr, b: Fr) -> Fr {
    let mut state = [Fr::zero(), a, b];
    for round in 0..TOTAL_POSEIDON_ROUNDS {
        binary_poseidon_hash_partial(round, &mut state);
    }
    state[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::EMPTY_TREE;
    use std::str::FromStr;

    #[test]
    fn test_binary_poseidon_hash() {
        assert_eq!(
            full_poseidon2_hash(Fr::zero(), Fr::zero()),
            Fr::from_str("14744269619966411208579211824598458697587494354926760081771325075741142829156").unwrap(),
        );

        assert_eq!(
            full_poseidon2_hash(Fr::from_str("1").unwrap(), Fr::from_str("2").unwrap()),
            Fr::from_str("7853200120776062878684798364095072458815029376092732009249414926327459813530").unwrap(),
        );

        assert_eq!(
            full_poseidon2_hash(Fr::from_str("4631032765893457899344").unwrap(), Fr::from_str("3453623782378239237823937").unwrap()),
            Fr::from_str("15798376151120407607995325383260410478881539926269713789760505676493608861934").unwrap(),
        );

        assert_eq!(
            full_poseidon2_hash(Fr::from_str("78758278433947439").unwrap(), Fr::from_str("2727127217219281927655748957").unwrap()),
            Fr::from_str("10053855256797203809243706937712819679696785488432523709871608122822392032095").unwrap(),
        );

        assert_eq!(
            full_poseidon2_hash(Fr::from_str("74758992786068504743996048").unwrap(), Fr::from_str("8434739230482761332454").unwrap()),
            Fr::from_str("17221088121480185305804562315627270623879289277074607312826677888427107195721").unwrap(),
        );

        // Inverted last two hashes
        assert_eq!(
            full_poseidon2_hash(Fr::from_str("2727127217219281927655748957").unwrap(), Fr::from_str("78758278433947439").unwrap()),
            Fr::from_str("12873223109498890755823667267246854666756739205168367165343839421529315277098").unwrap(),
        );

        assert_eq!(
            full_poseidon2_hash(Fr::from_str("8434739230482761332454").unwrap(), Fr::from_str("74758992786068504743996048").unwrap()),
            Fr::from_str("19385810945896973295264096509875610220438906021083240188787615240974188410069").unwrap(),
        );
    }

    #[test]
    fn test_mt_default_values() {
        let mut a = full_poseidon2_hash(Fr::zero(), Fr::zero());
        for empty_value in EMPTY_TREE {
            assert_eq!(a, empty_value);
            a = full_poseidon2_hash(a, a);
        }
    }
}