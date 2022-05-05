use ark_ff::*;
use super::poseidon_constants::*;
use super::super::fields::scalar::Scalar;

/// Binary Poseidon hasher
pub struct Poseidon2 {
    matrices: [Scalar; 9]
}

impl Poseidon2 {
    pub fn new() -> Poseidon2 { Poseidon2 { matrices: generate_m() } }

    pub fn full_hash(&self, b: Scalar, c: Scalar) -> Scalar {
        let mut state = [Scalar::zero(), b, c];
        for i in 0..ITERATIONS {
            state = self.partial_hash(i, state[0], state[1], state[2]);
        }
        state[0]
    }

    /// Hash of a single iteration
    /// - iterations group multiple rounds together
    /// - our binary Poseidon consists of 65 rounds
    pub fn partial_hash(&self, iteration: usize, a: Scalar, b: Scalar, c: Scalar) -> [Scalar; 3] {
        let mut state = [a, b, c];
        let c = generate_constants(iteration);
        let iteration = get_iteration_start_and_length(iteration);

        for i in 0..iteration.1 {
            // Ark
            state[0] += c[i][0];
            state[1] += c[i][1];
            state[2] += c[i][2];

            let round = iteration.0 + i;

            // Sbox
            if round < 4 || round >= 61 {
                for j in 0..3 {
                    let aux = state[j];
                    state[j] = state[j].square();
                    state[j] = state[j].square();
                    state[j] *= &aux;
                }
            } else {
                let aux = state[0];
                state[0] = state[0].square();
                state[0] = state[0].square();
                state[0] *= &aux;
            }

            // Mix 
            let mut new_state = [Scalar::zero(); 3];
            new_state[0] += self.matrices[0] * state[0];
            new_state[0] += self.matrices[1] * state[1];
            new_state[0] += self.matrices[2] * state[2];

            new_state[1] += self.matrices[3] * state[0];
            new_state[1] += self.matrices[4] * state[1];
            new_state[1] += self.matrices[5] * state[2];

            new_state[2] += self.matrices[6] * state[0];
            new_state[2] += self.matrices[7] * state[1];
            new_state[2] += self.matrices[8] * state[2];

            state = new_state;
        }

        state
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use super::super::super::fields::scalar::*;

    #[test]
    fn test_null_hash() {
        let p = Poseidon2::new();
        let hash = p.full_hash(Scalar::zero(), Scalar::zero());

        assert_eq!("0x2098F5FB9E239EAB3CEAC3F27B81E481DC3124D55FFED523A839EE8446B64864", to_hex_string(hash));
        assert_eq!(from_str_10("14744269619966411208579211824598458697587494354926760081771325075741142829156"), hash);
    }

    #[test]
    fn test_hash() {
        let p = Poseidon2::new();
        let hash = p.full_hash(from_str_10("8144211214817430829349003215074481182100404296535680119964943950269151541972"), Scalar::zero());
        assert_eq!(hash, from_str_10("3521277125107847192640759927250026508659373094488056016877049883968245990497"));

        let p = Poseidon2::new();
        let hash = p.full_hash(from_str_10("13552763967912093594457579779110052252941986640568606066796890732453878304904"), Scalar::zero());
        assert_eq!(hash, from_str_10("2788832706231923317949979783323167016733265655607476807262415957398223972822"));
    }

    #[test]
    fn test_det() {
        let p = Poseidon2::new();
        let hash0 = p.full_hash(Scalar::zero(), Scalar::zero());
        let hash1 = p.full_hash(Scalar::zero(), Scalar::zero());
        assert_eq!(hash0, hash1);
    }

    #[test]
    fn test_partial_hash() {
        let p = Poseidon2::new();
        let mut state = [Scalar::zero(), Scalar::zero(), Scalar::zero()];
        for i in 0..ITERATIONS {
            state = p.partial_hash(i, state[0], state[1], state[2]);
        }
        let hash = state[0];

        assert_eq!("0x2098F5FB9E239EAB3CEAC3F27B81E481DC3124D55FFED523A839EE8446B64864", to_hex_string(hash));
    }
}