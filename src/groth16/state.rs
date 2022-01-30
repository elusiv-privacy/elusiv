use super::super::storage_account::*;

use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use super::super::error::ElusivError::{ InvalidStorageAccount, InvalidStorageAccountSize };
use super::super::scalar::*;
use super::Proof;
use byteorder::{ ByteOrder, LittleEndian };
use ark_bn254::G1Affine;

solana_program::declare_id!("746Em3pvd2Rd2L3BRZ31RJ5qukorCiAw4kpudFkxgyBy");

pub struct WithdrawVerificationAccount<'a> {
    /// Amount (8 bytes)
    amount: &'a mut [u8],

    /// Nullifier hash (32 bytes)
    nullifier_hash: &'a mut [u8],

    /// Proof (324 bytes)
    proof: &'a mut [u8],

    /// Proof iteraction of current 
    /// - (u16 represented as 2 bytes)
    current_iteration: &'a mut [u8],

    /// Prepared inputs
    /// - x: 32 bytes
    /// - y: 32 bytes
    /// - infinity: boolean byte
    pub prepared_inputs: &'a mut [u8],
}

impl<'a> WithdrawVerificationAccount<'a> {
    pub const TOTAL_SIZE: usize = 8 + 32 + 324 + 2 + 65;

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

        let (amount, data) = data.split_at_mut(8);
        let (nullifier_hash, data) = data.split_at_mut(32);
        let (proof, data) = data.split_at_mut(324);
        let (current_iteration, data) = data.split_at_mut(2);
        let (prepared_inputs, _) = data.split_at_mut(65);

        Ok(
            WithdrawVerificationAccount {
                amount,
                nullifier_hash,
                proof,
                current_iteration,
                prepared_inputs,
            }
        )
    }
}

impl<'a> WithdrawVerificationAccount<'a> {
    pub fn get_amount(&self) -> u64 {
        LittleEndian::read_u64(&self.amount)
    }
    pub fn set_amount(&mut self, amount: u64) {
        let bytes = u64::to_le_bytes(amount);
        for i in 0..8 {
            self.amount[i] = bytes[i];
        }
    }

    pub fn get_nullifier_hash(&self) -> ScalarLimbs {
        bytes_to_limbs(&self.nullifier_hash)
    }
    pub fn set_nullifier_hash(&mut self, bytes: &[u8]) -> ProgramResult {
        set(&mut self.nullifier_hash, 0, 32, bytes)
    }

    pub fn get_proof(&self) -> Result<Proof, ProgramError> {
        Proof::from_bytes(&self.proof)
    }
    pub fn set_proof(&mut self, bytes: &[u8]) -> ProgramResult {
        set(&mut self.proof, 0, 324, bytes)
    }

    pub fn get_current_iteration(&self) -> u16 { bytes_to_u16(self.current_iteration) }
    pub fn set_current_iteration(&mut self, round: u16) {
        let bytes = round.to_le_bytes();
        self.current_iteration[0] = bytes[0];
        self.current_iteration[1] = bytes[1];
    }

    pub fn set_prepared_inputs(&mut self, pis: G1Affine) -> ProgramResult {
        let bytes = write_g1_affine(pis);
        set(&mut self.prepared_inputs, 0, 65, &bytes)
    }
    pub fn get_prepared_inputs(&self) -> G1Affine {
        read_g1_affine(&self.prepared_inputs)
    }
}

#[cfg(test)]
mod tests {
    type StorageAccount<'a> = super::WithdrawVerificationAccount<'a>;

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
}