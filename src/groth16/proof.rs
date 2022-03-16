use solana_program::program_error::ProgramError;
use super::super::instruction::{
    unpack_bool,
    unpack_limbs,
};
use super::super::fields::base::{ write_g1_affine, write_g2_affine };
use ark_ff::biginteger::BigInteger256;
use ark_bn254::{
    G1Affine, G2Affine,
    Fq2,
    Fq,
};

pub const PROOF_BYTES_SIZE: usize = 259;

#[derive(Copy, Clone)]
pub struct Proof {
    pub a: G1Affine,
    pub b: G2Affine,
    pub c: G1Affine,
}

impl Proof {
    pub fn from_bytes(data: &[u8]) -> Result<Self, ProgramError> {
        let (ax, data) = unpack_limbs(&data)?;
        let (ay, data) = unpack_limbs(&data)?;
        let (a_infinity, data) = unpack_bool(&data)?;

        let (bx0, data) = unpack_limbs(&data)?;
        let (bx1, data) = unpack_limbs(&data)?;
        let (by0, data) = unpack_limbs(&data)?;
        let (by1, data) = unpack_limbs(&data)?;
        let (b_infinity, data) = unpack_bool(&data)?;

        let (cx, data) = unpack_limbs(&data)?;
        let (cy, data) = unpack_limbs(&data)?;
        let (c_infinity, _) = unpack_bool(&data)?;

        let proof: Proof = Proof {
            a: G1Affine::new(
                Fq::new(BigInteger256(ax)),
                Fq::new(BigInteger256(ay)),
                a_infinity
            ),
            b: G2Affine::new(
                Fq2::new(
                    Fq::new(BigInteger256(bx0)),
                    Fq::new(BigInteger256(bx1)),
                ),
                Fq2::new(
                    Fq::new(BigInteger256(by0)),
                    Fq::new(BigInteger256(by1)),
                ),
                b_infinity
            ),
            c: G1Affine::new(
                Fq::new(BigInteger256(cx)),
                Fq::new(BigInteger256(cy)),
                c_infinity
            ),
        };

        Ok(proof)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![0; PROOF_BYTES_SIZE];
        write_g1_affine(&mut bytes[..65], self.a);
        write_g2_affine(&mut bytes[65..194], self.b);
        write_g1_affine(&mut bytes[194..259], self.c);
        bytes
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::str::FromStr;

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