use solana_program::{
    program_error::{
        ProgramError,
        ProgramError::InvalidArgument,
    },
};
use std::convert::TryInto;
use super::scalar::*;

pub enum ElusivInstruction {
    /// Initialize deposit, store amount and start hashing
    /// 
    /// Accounts expected:
    /// 0. [signer, writable] Depositor account
    /// 1. [owned, writable] Program account
    /// 2. [owned, writable] Deposit account
    InitDeposit {
        /// Deposit amount in Lamports
        amount: u64,

        /// Poseidon Commitment
        /// - in Montgomery form
        commitment: ScalarLimbs,
    },

    /// Compute the Merkle tree hashes
    /// 
    /// Accounts expected:
    /// 0. [owned, writable] Deposit account
    ComputeDeposit,

    /// Finish the hash computation and deposit SOL
    /// 
    /// Accounts expected:
    /// 0. [signer, writable] Depositor account
    /// 1. [owned, writable] Program account
    /// 2. [owned, writable] Deposit account
    /// 2. [static] System program
    FinishDeposit,

    /// Withdraw SOL
    /// 
    /// Accounts expected:
    /// 0. [owned, writable] Program account
    /// 1. [owned, writable] Withdraw account
    InitWithdraw {
        /// Withdrawal amount in Lamports
        amount: u64,

        /// Nullifier Hash
        /// - in Montgomery form
        nullifier_hash: ScalarLimbs,

        /// Merkle root
        /// - in Montgomery form
        root: ScalarLimbs,

        /// Groth16 proof
        /// 
        /// - in Montgomery form
        /// Consists of:
        /// - A: 2 [u64; 4] + 1 u8
        /// - B: 2 * (2 [u64; 4]) + 2 u8
        /// - C: 2 [u64; 4] + 1 u8
        proof: [u8; 324],
    },

    /// Groth16 verification computation
    /// 
    /// Accounts expected:
    /// 0. [owned, writable] Withdraw account
    VerifyWithdraw,

    /// Transfers the funds to the recipient
    /// 
    /// Accounts expected:
    /// 0. [owned, writable] Program account
    /// 1. [owned, writable] Withdraw account
    /// 2. [writable] Recipient account
    FinishWithdraw,

    /// Transfers the funds to the recipient
    /// 
    /// Accounts expected:
    /// 0. [owned] Program account
    /// 1. [owned] Deposit account
    /// 2. [owned] Withdraw account
    Log {
        index: u8
    }
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

            3 => Self::unpack_init_withdraw(&rest),
            4 => Ok(Self::VerifyWithdraw),
            5 => Ok(Self::FinishWithdraw),

            6 => Self::unpack_log(&rest),
            _ => Err(InvalidArgument)
        }
    }

    fn unpack_deposit(data: &[u8]) -> Result<Self, ProgramError> {
        // Unpack deposit amount
        let (amount, data) = unpack_u64(&data)?;
        
        // Unpack commitment
        let (bytes, _) = unpack_32_bytes(data)?;
        let commitment = bytes_to_limbs(bytes);

        Ok(ElusivInstruction::InitDeposit{ amount, commitment })
    }

    fn unpack_init_withdraw(data: &[u8]) -> Result<Self, ProgramError> {
        // Unpack withdrawal amount
        let (amount, data) = unpack_u64(&data)?;

        // Unpack nullifier hash
        let (nullifier_hash, data) = unpack_limbs(&data)?;

        // Unpack merkle root
        let (root, _) = unpack_limbs(&data)?;

        // Raw zkSNARK proof
        if data.len() != 324 { return Err(ProgramError::InvalidInstructionData); }
        let proof: [u8; 324] = data.try_into().unwrap();

        Ok(ElusivInstruction::InitWithdraw{ amount, proof, nullifier_hash, root })
    }

    fn unpack_log(data: &[u8]) -> Result<Self, ProgramError> {
        let (&index, _) = data.split_first().ok_or(ProgramError::InvalidInstructionData)?;

        Ok(ElusivInstruction::Log { index })
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

pub fn unpack_limbs(data: &[u8]) -> Result<(ScalarLimbs, &[u8]), ProgramError> {
    let (bytes, data) = unpack_32_bytes(data)?;

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

pub fn unpack_single_byte_as_limbs(data: &[u8]) -> Result<(ScalarLimbs, &[u8]), ProgramError> {
    let (bytes, data) = unpack_single_byte_as_32_bytes(data)?;

    Ok((bytes_to_limbs(&bytes), data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use ark_ff::{ BigInteger256, bytes::ToBytes };
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
        let mut data = vec![4];
        data.extend([0; 8]);
        data.extend([0; 32]);
        data.extend([0; 32]);
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

    #[test]
    fn test_unpack_proof_field_element() {
        let expect = str_to_bytes("21885181906247530850009302772481187419681297849437516181180948289250346671858");
        let bytes = vec![242, 30, 172, 108, 189, 207, 207, 247, 213, 213, 57, 218, 109, 91, 54, 61, 252, 37, 48, 12, 80, 212, 48, 193, 233, 204, 12, 91, 241, 146, 98, 48];
        assert_eq!(expect, bytes);
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