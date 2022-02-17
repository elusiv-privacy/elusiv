use solana_program::program_error::ProgramError;
use super::super::instruction::{
    unpack_bool,
    unpack_limbs,
};
use ark_ff::biginteger::BigInteger256;
use ark_bn254::{
    G1Affine, G2Affine,
    Fq2,
    Fq,
};

pub const PROOF_BYTES_SIZE: usize = 259;

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
}