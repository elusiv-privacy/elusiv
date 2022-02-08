use super::super::storage_account::*;

use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use byteorder::{ ByteOrder, LittleEndian };
use ark_bn254::{ Fq2, G2Projective };
use ark_ff::{ One };
use super::super::error::ElusivError::{ InvalidStorageAccount, InvalidStorageAccountSize };
use super::super::scalar::*;

pub const INPUTS_COUNT: usize = 2;
pub const RAM_WORD_SIZE: usize = 64;

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

    /// RAM used for caching values for next instruction/round
    /// - `RAM_WORD_SIZE` 32 byte words
    computation_ram: &'a mut [u8],

    //////////////////////////////////////////////////////////////////////////////
    // Inputs preparation

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

    //////////////////////////////////////////////////////////////////////////////
    // Proof

    //TODO: Save A and C as G1Projective
    /// A value of the proof
    /// - `G1Affine` -> 65 bytes
    pub proof_a: &'a mut [u8],

    /// C value of the proof
    /// - `G1Affine` -> 65 bytes
    pub proof_c: &'a mut [u8],

    /// B value of the proof
    /// - `G2Affine` -> 130 bytes
    pub proof_b: &'a mut [u8],

    /// B value negated
    pub b_neg: &'a mut [u8],

    /// r value for b computation
    /// - `G2HomProjective` (basically as G2Affine)
    /// - 130 bytes
    pub b_homo_r: &'a mut [u8],

    // Insertion pointer pointing to next free coeffient field
    // - u32
    coeff_ic: &'a mut [u8],

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
    pub const TOTAL_SIZE: usize = 4 + 4 + INPUTS_COUNT * 256 + G1PROJECTIVE_SIZE + RAM_WORD_SIZE * 32 + G1AFFINE_SIZE + G1AFFINE_SIZE + G2AFFINE_SIZE + G2AFFINE_SIZE + G2PROJECTIVE_SIZE + 4 + 8 + 32;

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

        let (computation_ram, data) = data.split_at_mut(RAM_WORD_SIZE * 32);

        let (proof_a, data) = data.split_at_mut(G1AFFINE_SIZE);
        let (proof_c, data) = data.split_at_mut(G1AFFINE_SIZE);
        let (proof_b, data) = data.split_at_mut(G2AFFINE_SIZE);
        let (b_neg, data) = data.split_at_mut(G2AFFINE_SIZE);
        let (b_homo_r, data) = data.split_at_mut(G2PROJECTIVE_SIZE);
        let (coeff_ic, data) = data.split_at_mut(4);

        let (amount, data) = data.split_at_mut(8);
        let (nullifier_hash, _) = data.split_at_mut(32);

        Ok(
            ProofVerificationAccount {
                current_iteration,
                current_round,
                input_bits,
                p_inputs,
                computation_ram,
                proof_a,
                proof_c,
                proof_b,
                b_neg,
                b_homo_r,
                coeff_ic,
                amount,
                nullifier_hash,
            }
        )
    }

    pub fn init(
        &mut self,
        inputs: Vec<[u8; 32]>,
        _amount: u64,
        _nullifier_hash: ScalarLimbs,
        proof: super::Proof,
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

        // Store raw proof data
        write_g1_affine(&mut self.proof_a, proof.a);
        write_g1_affine(&mut self.proof_c, proof.c);
        write_g2_affine(&mut self.proof_b, proof.b);

        // Store proof preparation values
        write_g2_projective(&mut self.b_homo_r, G2Projective::new(proof.b.x, proof.b.y, Fq2::one()));
        write_g2_affine(&mut self.b_neg, -proof.b);

        // Reset counters
        self.set_current_iteration(0);
        self.set_current_round(0);
        self.set_coeff_ic(0);

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

    pub fn inc_current_iteration(&mut self, count: u32) {
        self.set_current_iteration(bytes_to_u32(self.current_iteration) + count);
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

// RAM usage
impl<'a> ProofVerificationAccount<'a> {
    pub fn get_ram_mut(&mut self, offset: usize, length: usize) -> &mut [u8] {
        &mut self.computation_ram[offset * 32..(offset + length) * 32]
    }

    pub fn get_ram(&self, offset: usize, length: usize) -> &[u8] {
        &self.computation_ram[offset * 32..(offset + length) * 32]
    }
}

// Proof preparation
impl<'a> ProofVerificationAccount<'a> {
    fn set_coeff_ic(&mut self, index: u32) {
        LittleEndian::write_u32(&mut self.coeff_ic, index);
    }

    pub fn get_coeff_ic(&self) -> usize {
        bytes_to_u32(self.coeff_ic) as usize
    }

    pub fn inc_coeff_ic(&mut self) {
        let ic = self.get_coeff_ic();
        LittleEndian::write_u32(&mut self.coeff_ic, ic as u32 + 1);
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