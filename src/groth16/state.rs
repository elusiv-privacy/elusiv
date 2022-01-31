use super::super::storage_account::*;

use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use byteorder::{ ByteOrder, LittleEndian };
use ark_bn254::{ Fq, G1Projective };
use ark_ff::{ Zero, One };
use byteorder::{ BigEndian };
use super::super::error::ElusivError::{ InvalidStorageAccount, InvalidStorageAccountSize };
use super::super::scalar::*;

pub const INPUTS_COUNT: usize = 2;

solana_program::declare_id!("746Em3pvd2Rd2L3BRZ31RJ5qukorCiAw4kpudFkxgyBy");

pub struct ProofVerificationAccount<'a> {
    //////////////////////////////////////////////////////////////////////////////
    // Cached values

    /// Computation iteration 
    /// - `u32`
    current_iteration: &'a mut [u8],

    /// Computation round (multiple rounds in each iteration)
    /// - `u32`
    current_round: &'a mut [u8],

    /// Original inputs
    /// - `[u8; INPUTS_COUNT * 256]`
    /// - big endian
    /// - stored as bits (1 true, 0 false)
    /// - leading bits are replaced by the value 2
    input_bits: &'a mut [u8],

    /// Prepared inputs
    /// - `G1Projective`
    /// - x: 32 bytes
    /// - y: 32 bytes
    /// - z: 32 bytes
    pub p_inputs: &'a mut [u8],

    /// Product used for prepared inputs construction
    /// - `G1Projective`
    /// - x: 32 bytes
    /// - y: 32 bytes
    /// - z: 32 bytes
    pub p_product: &'a mut [u8],

    //////////////////////////////////////////////////////////////////////////////
    // Withdraw data

    /// Amount
    /// - `u64`
    /// - 8 bytes
    amount: &'a mut [u8],

    /// Nullifier hash
    /// - `Scalar`
    /// - 32 bytes
    nullifier_hash: &'a mut [u8],
}

impl<'a> ProofVerificationAccount<'a> {
    pub const TOTAL_SIZE: usize = 4 + 4 + INPUTS_COUNT * 256 + G1PROJECTIVE_SIZE + G1PROJECTIVE_SIZE + 8 + 32;

    pub fn new(
        account_info: &solana_program::account_info::AccountInfo,
        data: &'a mut [u8],
        program_id: &solana_program::pubkey::Pubkey,
    ) -> Result<Self, ProgramError> {
        if account_info.owner != program_id { return Err(InvalidStorageAccount.into()); }
        if !account_info.is_writable { return Err(InvalidStorageAccount.into()); }
        //if *account_info.key != id() { return Err(InvalidStorageAccount.into()); }

        Self::from_data(data) 
    }

    pub fn from_data(data: &'a mut [u8]) -> Result<Self, ProgramError> {
        if data.len() != Self::TOTAL_SIZE { return Err(InvalidStorageAccountSize.into()); }

        let (current_iteration, data) = data.split_at_mut(4);
        let (current_round, data) = data.split_at_mut(4);
        let (input_bits, data) = data.split_at_mut(INPUTS_COUNT * 256);
        let (p_inputs, data) = data.split_at_mut(G1PROJECTIVE_SIZE);
        let (p_product, data) = data.split_at_mut(G1PROJECTIVE_SIZE);
        let (amount, data) = data.split_at_mut(8);
        let (nullifier_hash, _) = data.split_at_mut(32);

        Ok(
            ProofVerificationAccount {
                current_iteration,
                current_round,
                input_bits,
                p_inputs,
                p_product,
                amount,
                nullifier_hash,
            }
        )
    }

    pub fn init(
        &mut self,
        inputs: Vec<[u8; 32]>,
        amount: u64,
        nullifier_hash: ScalarLimbs,
    ) -> ProgramResult {
        // Parse inputs
        // - leading zeros are padded as the value 2
        // - big endian
        for i in 0..inputs.len() {
            let bits = bit_encode(inputs[i]);
            for j in 0..256 {
                self.input_bits[i * 256 + j] = bits[j];
            }
        }

        // Store amount

        // Store nullifier_hash

        // Reset counters
        self.set_current_iteration(0);
        self.set_current_round(0);

        Ok(())
    }
}

/// Encodes the scalar bytes as bits required for the scalar multiplicaiton
/// 
/// ### Arguments
/// 
/// * `scalar` - `[u8; 32]` le bytes in repr form (!)
pub fn bit_encode(scalar: [u8; 32]) -> Vec<u8> {
    let bytes_be: Vec<u8> = scalar.iter().copied().rev().collect();
    let mut bits = Vec::new();
    let mut is_leading = true;
    for byte in bytes_be {
        for i in (0..8).rev() {
            let bit = (byte >> i) & 1;
            if bit == 1 { is_leading = false; }

            bits.push(if bit == 0 && is_leading { 2 } else { bit });
        }
    }
    bits
}

// Iterations
impl<'a> ProofVerificationAccount<'a> {
    pub fn get_current_iteration(&self) -> usize {
        bytes_to_u32(self.current_iteration) as usize
    }
    pub fn get_current_round(&self) -> usize {
        bytes_to_u32(self.current_round) as usize
    }

    fn set_current_iteration(&mut self, iteration: u32) {
        let bytes = iteration.to_le_bytes();
        self.current_iteration[0] = bytes[0];
        self.current_iteration[1] = bytes[1];
        self.current_iteration[2] = bytes[2];
        self.current_iteration[3] = bytes[3];
    }

    pub fn inc_current_iteration(&mut self) {
        self.set_current_iteration(bytes_to_u32(self.current_iteration) + 1);
    }

    pub fn set_current_round(&mut self, round: usize) {
        let bytes = (round as u32).to_le_bytes();
        self.current_round[0] = bytes[0];
        self.current_round[1] = bytes[1];
        self.current_round[2] = bytes[2];
        self.current_round[3] = bytes[3];
    }

    pub fn get_input_bits(&self, input: usize) -> [u8; 256] {
        let mut bits = [0 as u8; 256];
        let ib = &self.input_bits[input * 256..(input + 1) * 256];
        for i in 0..256 {
            bits[i] = ib[i];
        }
        bits 
    }
}

// Withdrawal data
impl<'a> ProofVerificationAccount<'a> {
    pub fn get_amount(&self) -> u64 {
        LittleEndian::read_u64(&self.amount)
    }

    pub fn get_nullifier_hash(&self) -> ScalarLimbs {
        bytes_to_limbs(&self.nullifier_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type StorageAccount<'a> = super::ProofVerificationAccount<'a>;

    #[test]
    fn test_correct_size() {
        let mut data = [0; StorageAccount::TOTAL_SIZE];
        StorageAccount::from_data(&mut data).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_invalid_size() {
        let mut data = [0; StorageAccount::TOTAL_SIZE - 1];
        StorageAccount::from_data(&mut data).unwrap();
    }

    #[test]
    fn test_bit_encode_input() {
        let bytes_le = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0b11111111, 0];
        let bits = bit_encode(bytes_le);

        let mut expect = vec![0 as u8; 256];
        for i in 0..8 {
            expect[i] = 2;
        }
        for i in 8..16 {
            expect[i] = 1;
        }

        assert_eq!(expect, bits);
    }
}