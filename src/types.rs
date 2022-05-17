use crate::bytes::SerDe;
use crate::macros::*;
use ark_bn254::{ G1Affine, G2Affine };

pub type U256 = [u8; 32];
pub const U256_ZERO: U256 = [0; 32];

pub type RawProof = [u8; Proof::SIZE];

/// Minimum data and public inputs required for a n-ary join-split based proof
#[derive(SerDe)]
pub struct JoinSplitProofData<const N: usize> {
    pub proof: RawProof,
    pub nullifier_hashes: [U256; N],
    pub root_hashes: [U256; N],
    pub tree_indices: [u64; N],
    pub commitment: U256,
}

#[derive(SerDe, Copy, Clone)]
/// A Groth16 proof
pub struct Proof {
    pub a: G1Affine,
    pub b: G2Affine,
    pub c: G1Affine,
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;
    use ark_bn254::{ Fq, Fq2 };

    #[test]
    fn test_proof_bytes() {
        let proof = Proof {
            a: G1Affine::new(
                Fq::from_str("10026859857882131638516328056627849627085232677511724829502598764489185541935").unwrap(),
                Fq::from_str("19685960310506634721912121951341598678325833230508240750559904196809564625591").unwrap(),
                true
            ),
            b: G2Affine::new(
                Fq2::new(
                    Fq::from_str("20925091368075991963132407952916453596237117852799702412141988931506241672722").unwrap(),
                    Fq::from_str("6039012589018526855429190661364232506642511499289558287989232491174672020857").unwrap(),
                ),
                Fq2::new(
                    Fq::from_str("18684276579894497974780190092329868933855710870485375969907530111657029892231").unwrap(),
                    Fq::from_str("5932690455294482368858352783906317764044134926538780366070347507990829997699").unwrap(),
                ),
                false
            ),
            c: G1Affine::new(
                Fq::from_str("10026859857882131638516328056627849627085232677511724829502598764489185541935").unwrap(),
                Fq::from_str("5810683806126530275877423137657928095712201856589324885003647168396414659782").unwrap(),
                true
            ),
        };

        let bytes = proof.to_bytes();
        let after = Proof::from_bytes(&bytes).unwrap();

        assert_eq!(proof.a, after.a);
        assert_eq!(proof.b, after.b);
        assert_eq!(proof.c, after.c);
    }
}