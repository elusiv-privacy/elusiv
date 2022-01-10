use solana_program::{
    program_error::{
        ProgramError,
        ProgramError::InvalidArgument,
    },
    msg
};
use std::convert::TryInto;
use ark_bn254::{
    Bn254,
    G1Affine, G2Affine,
    G1Projective, G2Projective,
    Fq2
};
use ark_groth16::{ Proof };
use ark_ff::{ BigInteger256 };
use poseidon::scalar;
use poseidon::scalar::ScalarLimbs;

pub enum ElusivInstruction {
    /// Deposits SOL 
    /// 
    /// Accounts expected:
    /// 0. [signer, writable] Depositor account
    /// 1. [owned, writable] Bank and storage account
    /// 2. [static] System program
    Deposit {
        /// Deposit amount in Lamports
        amount: u64,

        /// Poseidon Commitment
        commitment: ScalarLimbs,
    },

    /// Withdraw SOL
    /// 
    /// Accounts expected:
    /// 0. [signer] Initiator of the withdrawal
    /// 1. [owned, writable] Bank and storage account
    /// 2. [writable] Recipient of the withdrawal
    Withdraw {
        /// Withdrawal amount in Lamports
        amount: u64,

        /// Groth16 proof
        /// 
        /// Consists of:
        /// - A: 2 [u64; 4] + 1 u8
        /// - B: 2 * (2 [u64; 4]) + 2 u8
        /// - C: 2 [u64; 4] + 1 u8
        proof: Proof<Bn254>,

        /// Nullifier Hash
        nullifier_hash: ScalarLimbs,

        /// Merkle root
        root: ScalarLimbs,
    }
}

impl ElusivInstruction {
    pub fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = data
            .split_first()
            .ok_or(ProgramError::InvalidInstructionData)?;

        match tag {
            0 => Self::unpack_deposit(&rest),
            1 => Self::unpack_withdraw(&rest),
            _ => Err(InvalidArgument)
        }
    }

    fn unpack_deposit(data: &[u8]) -> Result<Self, ProgramError> {
        msg!("Unpack deposit");

        // Unpack deposit amount
        let (amount, data) = unpack_u64(&data, true)?;

        // Unpack commitment
        let (commitment, _) = upnack_limbs(&data)?;

        msg!("Deposit unpacked");

        Ok(ElusivInstruction::Deposit{ amount, commitment })
    }

    fn unpack_withdraw(data: &[u8]) -> Result<Self, ProgramError> {
        msg!("Unpack withdraw");

        // Unpack withdrawal amount
        let (amount, data) = unpack_u64(&data, true)?;

        // Unpack zkSNARK proof
        let (ax, data) = upnack_limbs(&data)?;
        let (ay, data) = upnack_limbs(&data)?;
        let (az, data) = upnack_single_byte_as_limbs(&data)?;
        let (b00, data) = upnack_limbs(&data)?;
        let (b01, data) = upnack_limbs(&data)?;
        let (b10, data) = upnack_limbs(&data)?;
        let (b11, data) = upnack_limbs(&data)?;
        let (b20, data) = upnack_single_byte_as_limbs(&data)?;
        let (b21, data) = upnack_single_byte_as_limbs(&data)?;
        let (cx, data) = upnack_limbs(&data)?;
        let (cy, data) = upnack_limbs(&data)?;
        let (cz, data) = upnack_single_byte_as_limbs(&data)?;
        let proof: Proof<Bn254> = Proof {
            a: G1Affine::from(
                G1Projective::new(
                    BigInteger256(ax).into(),
                    BigInteger256(ay).into(),
                    BigInteger256(az).into(),
                )
            ),
            b: G2Affine::from(
                G2Projective::new(
                    Fq2::new(
                        BigInteger256(b00).into(),
                        BigInteger256(b01).into(),
                    ),
                    Fq2::new(
                        BigInteger256(b10).into(),
                        BigInteger256(b11).into(),
                    ),
                    Fq2::new(
                        BigInteger256(b20).into(),
                        BigInteger256(b21).into(),
                    ),
                )
            ),
            c: G1Affine::from(
                G1Projective::new(
                    BigInteger256(cx).into(),
                    BigInteger256(cy).into(),
                    BigInteger256(cz).into(),
                )
            )
        };

        // Unpack nullifier hash
        let (nullifier_hash, data) = upnack_limbs(&data)?;

        // Unpack merkle root
        let (root, _) = upnack_limbs(&data)?;

        msg!("Withdraw unpacked");

        Ok(ElusivInstruction::Withdraw{ amount, proof, nullifier_hash, root })
    }
}

fn unpack_u64(data: &[u8], little_endian: bool) -> Result<(u64, &[u8]), ProgramError> {
    let value = data
        .get(..8)
        .and_then(|slice| slice.try_into().ok())
        .map(if little_endian { u64::from_le_bytes } else { u64::from_be_bytes })
        .ok_or(InvalidArgument)?;

    Ok((value, &data[8..]))
}

fn unpack_32_bytes(data: &[u8]) -> Result<(&[u8], &[u8]), ProgramError> {
    let bytes = data.get(..32).ok_or(InvalidArgument)?;

    Ok((bytes, &data[32..]))
}

// TODO: Check if every value is < r/p
fn upnack_limbs(data: &[u8]) -> Result<(ScalarLimbs, &[u8]), ProgramError> {
    let (bytes, data) = unpack_32_bytes(data)?;

    Ok((scalar::bytes_to_limbs(bytes), data))
}

fn unpack_single_byte_as_32_bytes(data: &[u8]) -> Result<([u8; 32], &[u8]), ProgramError> {
    let (&data, rest) = data
        .split_first()
        .ok_or(ProgramError::InvalidInstructionData)?;
    let mut bytes = [0; 32];
    bytes[0] = data;

    Ok((bytes, rest))
}

fn upnack_single_byte_as_limbs(data: &[u8]) -> Result<(ScalarLimbs, &[u8]), ProgramError> {
    let (bytes, data) = unpack_single_byte_as_32_bytes(data)?;

    Ok((scalar::bytes_to_limbs(&bytes), data))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test subsidiary unpacking functions
    #[test]
    fn test_unpack_u64() {
        let d: [u8; 8] = [0b00000001, 0, 0, 0, 0, 0, 0, 0b00000000];

        // Test little endian interpretation
        let (v, _) = unpack_u64(&d, true).unwrap();
        assert_eq!(v, 1);

        // Test big endian interpretation
        let (v, _) = unpack_u64(&d, false).unwrap();
        assert_eq!(v, 1 << 56);
    }

    #[test]
    fn test_unpack_u64_too_small() {

    }

    #[test]
    fn test_unpack_u256() {
        
    }

    #[test]
    fn test_unpack_byte_as_u256() {

    }

    #[test]
    fn test_unpack_deposit() {
        
    }

    #[test]
    fn test_unpack_withdraw() {
        
    }

    // Test instruction unpacking
}