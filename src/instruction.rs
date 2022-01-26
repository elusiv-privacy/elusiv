use solana_program::{
    program_error::{
        ProgramError,
        ProgramError::InvalidArgument,
    },
};
use std::convert::TryInto;
use ark_bn254::{
    Bn254,
    G1Affine, G2Affine,
    G1Projective, G2Projective,
    Fq2,
};
use ark_groth16::{ Proof };
use super::poseidon::*;
use ark_ff::*;

pub enum ElusivInstruction {
    /// Initialize deposit, store amount and start hashing
    /// 
    /// Accounts expected:
    /// 0. [signer, writable] Depositor account
    /// 1. [owned, writable] Program account
    InitDeposit {
        /// Deposit amount in Lamports
        amount: u64,

        /// Poseidon Commitment
        commitment: ScalarLimbs,
    },

    /// Compute the Merkle tree hashes
    /// 
    /// Accounts expected:
    /// 0. [signer, writable] Depositor account
    /// 1. [owned, writable] Program account
    ComputeDeposit,

    /// Finish the hash computation and deposit SOL
    /// 
    /// Accounts expected:
    /// 0. [signer, writable] Depositor account
    /// 1. [owned, writable] Program account
    /// 2. [static] System program
    FinishDeposit,

    /// Withdraw SOL
    /// 
    /// Accounts expected:
    /// 0. [signer] Initiator of the withdrawal
    /// 1. [owned, writable] Program account
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
    },
}

impl ElusivInstruction {
    pub fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = data
            .split_first()
            .ok_or(ProgramError::InvalidInstructionData)?;

        match tag {
            0 => Self::unpack_deposit(&rest),
            1 => Ok(Self::ComputeDeposit),
            2 => Ok(Self::FinishDeposit),
            3 => Self::unpack_withdraw(&rest),
            _ => Err(InvalidArgument)
        }
    }

    fn unpack_deposit(data: &[u8]) -> Result<Self, ProgramError> {
        // Unpack deposit amount
        let (amount, data) = unpack_u64(&data)?;

        // Unpack commitment
        let (commitment, _) = unpack_limbs(&data)?;

        Ok(ElusivInstruction::InitDeposit{ amount, commitment })
    }

    fn unpack_withdraw(data: &[u8]) -> Result<Self, ProgramError> {
        // Unpack withdrawal amount
        let (amount, data) = unpack_u64(&data)?;

        // Unpack zkSNARK proof
        let (ax, data) = unpack_limbs(&data)?;
        let (ay, data) = unpack_limbs(&data)?;
        let (az, data) = upnack_single_byte_as_limbs(&data)?;
        let (b00, data) = unpack_limbs(&data)?;
        let (b01, data) = unpack_limbs(&data)?;
        let (b10, data) = unpack_limbs(&data)?;
        let (b11, data) = unpack_limbs(&data)?;
        let (b20, data) = upnack_single_byte_as_limbs(&data)?;
        let (b21, data) = upnack_single_byte_as_limbs(&data)?;
        let (cx, data) = unpack_limbs(&data)?;
        let (cy, data) = unpack_limbs(&data)?;
        let (cz, data) = upnack_single_byte_as_limbs(&data)?;

        let proof: Proof<Bn254> = Proof {
            a: G1Affine::from(
                G1Projective::new(
                    BigInteger256::new(ax).into(),
                    BigInteger256::new(ay).into(),
                    BigInteger256::new(az).into()
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
                    )
                )
            ),
            c: G1Affine::from(
                G1Projective::new(
                    BigInteger256::new(cx).into(),
                    BigInteger256::new(cy).into(),
                    BigInteger256::new(cz).into()
                )
            ),
        };

        // Unpack nullifier hash
        let (nullifier_hash, data) = unpack_limbs(&data)?;

        // Unpack merkle root
        let (root, _) = unpack_limbs(&data)?;

        Ok(ElusivInstruction::Withdraw{ amount, proof, nullifier_hash, root })
    }
}

fn unpack_u64(data: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
    let value = data
        .get(..8)
        .and_then(|slice| slice.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(InvalidArgument)?;

    Ok((value, &data[8..]))
}

fn unpack_32_bytes(data: &[u8]) -> Result<(&[u8], &[u8]), ProgramError> {
    let bytes = data.get(..32).ok_or(InvalidArgument)?;

    Ok((bytes, &data[32..]))
}

// TODO: Check if every value is < r/p
fn unpack_limbs(data: &[u8]) -> Result<(ScalarLimbs, &[u8]), ProgramError> {
    let (bytes, data) = unpack_32_bytes(data)?;
    //msg!(&format!("{:?}", bytes));

    Ok((bytes_to_limbs(bytes), data))
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

    Ok((bytes_to_limbs(&bytes), data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use ark_ff::{ BigInteger256 };
    use num_bigint::BigUint;
    use std::convert::TryFrom;

    // Test subsidiary unpacking functions
    #[test]
    fn test_unpack_u64() {
        let d: [u8; 8] = [0b00000001, 0, 0, 0, 0, 0, 0, 0b00000000];

        // Test little endian interpretation
        let (v, _) = unpack_u64(&d).unwrap();
        assert_eq!(v, 1);
    }

    #[test]
    fn test_unpack_withdraw() {
        // Withdrawal data
        let mut data = vec![3];
        data.extend([0,0,0,0,0,0,0,0]);
        data.extend(str_to_bytes("15200472642106544087859624808573647436446459686589177220422407004547835364093"));
        data.extend(str_to_bytes("18563249006229852218279298661872929163955035535605917747249479039354347737308"));
        data.push(1);
        data.extend(str_to_bytes("20636553466803549451478361961314475483171634413642350348046906733449463808895"));
        data.extend(str_to_bytes("3955337224043097728615186066317353350659966424133589619785214107405965410236"));
        data.extend(str_to_bytes("16669477906162214549333998971085624527095786690622350917799822973577201769757"));
        data.extend(str_to_bytes("10686129702127228201109048634021146893529704437134012687698468995076983569763"));
        data.push(1);
        data.push(0);
        data.extend(str_to_bytes("7825488021728597353611301562108479035418173715138578342437621330551207000521"));
        data.extend(str_to_bytes("17385834695111423269684287513728144523333186942287839669241715541894829818572"));
        data.push(1);
        data.extend(str_to_bytes("17385834695111423269684287513728144523333186942287839669241715541894829818572"));
        data.extend(str_to_bytes("17385834695111423269684287513728144523333186942287839669241715541894829818572"));

        ElusivInstruction::unpack(&data).unwrap();
    }

    pub fn str_to_bytes(str: &str) -> Vec<u8> {
        let mut writer: Vec<u8> = vec![];
        str_to_bigint(str).write(&mut writer).unwrap();
        writer
    }

    pub fn str_to_bigint(str: &str) -> BigInteger256 {
        BigInteger256::try_from(BigUint::from_str(str).unwrap()).unwrap()
    }
}