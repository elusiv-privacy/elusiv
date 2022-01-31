use solana_program::program_error::ProgramError;
use super::super::instruction::{
    unpack_single_byte_as_limbs,
    unpack_limbs,
};
use ark_ff::biginteger::BigInteger256;
use ark_bn254::{
    G1Affine, G2Affine,
    G1Projective, G2Projective,
    Fq2,
    Fq,
};

pub const PROOF_BYTES_SIZE: usize = 260;

pub struct Proof {
    pub a: G1Affine,
    pub b: G2Affine,
    pub c: G1Affine,
}

impl Proof {
    pub fn from_bytes(data: &[u8]) -> Result<Self, ProgramError> {
        let (ax, data) = unpack_limbs(&data)?;
        let (ay, data) = unpack_limbs(&data)?;
        let (az, data) = unpack_single_byte_as_limbs(&data)?;
        let (b00, data) = unpack_limbs(&data)?;
        let (b01, data) = unpack_limbs(&data)?;
        let (b10, data) = unpack_limbs(&data)?;
        let (b11, data) = unpack_limbs(&data)?;
        let (b20, data) = unpack_single_byte_as_limbs(&data)?;
        let (b21, data) = unpack_single_byte_as_limbs(&data)?;
        let (cx, data) = unpack_limbs(&data)?;
        let (cy, data) = unpack_limbs(&data)?;
        let (cz, _) = unpack_single_byte_as_limbs(&data)?;

        let proof: Proof = Proof {
            a: G1Affine::from(
                G1Projective::new(
                    Fq::new(BigInteger256(ax)),
                    Fq::new(BigInteger256(ay)),
                    Fq::new(BigInteger256(az)),
                )
            ),
            b: G2Affine::from(
                G2Projective::new(
                    Fq2::new(
                        Fq::new(BigInteger256(b00)),
                        Fq::new(BigInteger256(b01)),
                    ),
                    Fq2::new(
                        Fq::new(BigInteger256(b10)),
                        Fq::new(BigInteger256(b11)),
                    ),
                    Fq2::new(
                        Fq::new(BigInteger256(b20)),
                        Fq::new(BigInteger256(b21)),
                    )
                )
            ),
            c: G1Affine::from(
                G1Projective::new(
                    Fq::new(BigInteger256(cx)),
                    Fq::new(BigInteger256(cy)),
                    Fq::new(BigInteger256(cz)),
                )
            ),
        };

        Ok(proof)
    }
}