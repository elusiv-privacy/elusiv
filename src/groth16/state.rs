use super::super::storage_account::*;

use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use byteorder::{ ByteOrder, LittleEndian };
use ark_bn254::{ Fq, Fq2, Fq6, Fq12, G2Projective };
use ark_ff::*;
use super::lazy_stack::LazyHeapStack;
use super::super::error::ElusivError::{ InvalidStorageAccount, InvalidStorageAccountSize };
use super::super::scalar::*;

const ZERO_1: Fq = field_new!(Fq, "0");
const ZERO_2: Fq2 = field_new!(Fq2, ZERO_1, ZERO_1);
const ZERO_6: Fq6 = field_new!(Fq6, ZERO_2, ZERO_2, ZERO_2);
const ZERO_12: Fq12 = field_new!(Fq12, ZERO_6, ZERO_6);

pub const STACK_FQ_SIZE: usize = 20;
pub const STACK_FQ6_SIZE: usize = 7;
pub const STACK_FQ12_SIZE: usize = 7;

solana_program::declare_id!("746Em3pvd2Rd2L3BRZ31RJ5qukorCiAw4kpudFkxgyBy");

pub struct ProofVerificationAccount<'a> {
    pub stack_fq: LazyHeapStack<Fq>,
    pub stack_fq6: LazyHeapStack<Fq6>,
    pub stack_fq12: LazyHeapStack<Fq12>,

    iteration: &'a mut [u8],
    round: &'a mut [u8],
}

impl<'a> ProofVerificationAccount<'a> {
    pub const TOTAL_SIZE: usize = (STACK_FQ_SIZE + STACK_FQ6_SIZE * 6 + STACK_FQ12_SIZE * 12) * 32 + 4 + 4;

    pub fn new(
        account_info: &solana_program::account_info::AccountInfo,
        data: &'a mut [u8],
        program_id: &solana_program::pubkey::Pubkey,
    ) -> Result<Self, ProgramError> {
        if account_info.owner != program_id { return Err(InvalidStorageAccount.into()); }
        if !account_info.is_writable { return Err(InvalidStorageAccount.into()); }

        Self::from_data(data) 
    }

    pub fn from_data(data: &'a mut [u8]) -> Result<Self, ProgramError> {
        if data.len() != Self::TOTAL_SIZE { return Err(InvalidStorageAccountSize.into()); }

        let stack_fq = LazyHeapStack {
            stack: vec![ZERO_1; STACK_FQ_SIZE],
            stack_pointer: 0,
        };
        let stack_fq6 = LazyHeapStack {
            stack: vec![ZERO_6; STACK_FQ6_SIZE],
            stack_pointer: 0,
        };
        let stack_fq12 = LazyHeapStack {
            stack: vec![ZERO_12; STACK_FQ12_SIZE],
            stack_pointer: 0,
        };

        let (iteration, data) = data.split_at_mut(4);
        let (round, _) = data.split_at_mut(4);

        Ok(
            ProofVerificationAccount {
                stack_fq,
                stack_fq6,
                stack_fq12,
                iteration,
                round,
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

// Stack pushing
impl<'a> ProofVerificationAccount<'a> {
    #[inline(always)]
    pub fn push_fq(&mut self, v: Fq) {
        self.stack_fq.push(v)
    }

    #[inline(always)]
    pub fn push_fq2(&mut self, v: Fq2) {
        self.stack_fq.push(v.c1);
        self.stack_fq.push(v.c0);
    }

    #[inline(always)]
    pub fn push_fq6(&mut self, v: Fq6) {
        self.stack_fq6.push(v);
    }

    #[inline(always)]
    pub fn push_fq12(&mut self, v: Fq12) {
        self.stack_fq12.push(v);
    }
}

// Stack poping
impl<'a> ProofVerificationAccount<'a> {
    pub fn pop_fq(&mut self) -> Fq {
        self.stack_fq.pop()
    }

    pub fn pop_fq2(&mut self) -> Fq2 {
        let a = self.stack_fq.pop();
        let b = self.stack_fq.pop();
        Fq2::new(a, b)
    }

    pub fn pop_fq6(&mut self) -> Fq6 {
        self.stack_fq6.pop()
    }

    pub fn pop_fq12(&mut self) -> Fq12 {
        self.stack_fq12.pop()
    }
}

// Stack peeking
impl<'a> ProofVerificationAccount<'a> {
    pub fn peek_fq(&mut self, offset: usize) -> Fq {
        self.stack_fq.peek(offset)
    }

    pub fn peek_fq2(&mut self, offset: usize) -> Fq2 {
        let a = self.stack_fq.peek(offset * 2);
        let b = self.stack_fq.peek(offset * 2 + 1);
        Fq2::new(a, b)
    }

    pub fn peek_fq6(&mut self, offset: usize) -> Fq6 {
        self.stack_fq6.peek(offset)
    }

    pub fn peek_fq12(&mut self, offset: usize) -> Fq12 {
        self.stack_fq12.peek(offset)
    }
}

// Iterations and rounds
impl<'a> ProofVerificationAccount<'a> {
    pub fn get_iteration(&self) -> usize {
        bytes_to_u32(&self.round) as usize
    }

    pub fn set_iteration(&mut self, iteration: usize) {
        let bytes = (iteration as u32).to_le_bytes();
        self.iteration[0] = bytes[0];
        self.iteration[1] = bytes[1];
        self.iteration[2] = bytes[2];
        self.iteration[3] = bytes[3];
    }

    pub fn get_round(&self) -> usize {
        bytes_to_u32(&self.round) as usize
    }

    pub fn set_round(&mut self, round: usize) {
        let bytes = (round as u32).to_le_bytes();
        self.round[0] = bytes[0];
        self.round[1] = bytes[1];
        self.round[2] = bytes[2];
        self.round[3] = bytes[3];
    }
}

// Stack serialization
impl<'a> ProofVerificationAccount<'a> {
    pub fn save_stack(&mut self) {
        //Self::save_fq(self.stack[i], self.data, i << 5);
    }

    #[inline(always)]
    fn save_fq(v: Fq, buffer: &mut [u8], offset: usize) {
        Self::save_limb(v.0.0[0], buffer, 0 + offset);
        Self::save_limb(v.0.0[1], buffer, 8 + offset);
        Self::save_limb(v.0.0[2], buffer, 16 + offset);
        Self::save_limb(v.0.0[3], buffer, 24 + offset);
    }

    #[inline(never)]
    fn save_limb(v: u64, buffer: &mut [u8], offset: usize) {
        let a = u64::to_le_bytes(v);
        buffer[offset + 0] = a[0];
        buffer[offset + 1] = a[1];
        buffer[offset + 2] = a[2];
        buffer[offset + 3] = a[3];
        buffer[offset + 4] = a[4];
        buffer[offset + 5] = a[5];
        buffer[offset + 6] = a[6];
        buffer[offset + 7] = a[7];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::{ Fq, Fq6 };
    use std::str::FromStr;

    type StorageAccount<'a> = super::ProofVerificationAccount<'a>;

    #[test]
    fn test_correct_size() {
        let mut data = [0; StorageAccount::TOTAL_SIZE];
        StorageAccount::from_data(&mut data).unwrap();
    }

    /*#[test]
    #[should_panic]
    fn test_invalid_size() {
        let mut data = [0; StorageAccount::TOTAL_SIZE - 1];
        StorageAccount::from_data(&mut data).unwrap();
    }*/

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

    #[test]
    fn test_stack_fq() {
        let mut data = [0; StorageAccount::TOTAL_SIZE];
        let mut account = StorageAccount::from_data(&mut data).unwrap();

        let f = Fq::from_str("20925091368075991963132407952916453596237117852799702412141988931506241672722").unwrap();

        account.push_fq(f);
        let peek = account.peek_fq(0);
        let pop = account.pop_fq();

        assert_eq!(peek, f);
        assert_eq!(pop, f);
    }

    #[test]
    fn test_stack_fq2() {
        let mut data = [0; StorageAccount::TOTAL_SIZE];
        let mut account = StorageAccount::from_data(&mut data).unwrap();

        let f = Fq2::new(
            Fq::from_str("20925091368075991963132407952916453596237117852799702412141988931506241672722").unwrap(),
            Fq::from_str("18684276579894497974780190092329868933855710870485375969907530111657029892231").unwrap(),
        );

        account.push_fq2(f);
        let peek = account.peek_fq2(0);
        let pop = account.pop_fq2();

        assert_eq!(peek, f);
        assert_eq!(pop, f);
    }

    #[test]
    fn test_stack_fq6() {
        let mut data = [0; StorageAccount::TOTAL_SIZE];
        let mut account = StorageAccount::from_data(&mut data).unwrap();

        let f = get_fq6();

        account.push_fq6(f);
        let peek = account.peek_fq6(0);
        let pop = account.pop_fq6();

        assert_eq!(peek, f);
        assert_eq!(pop, f);
    }

    #[test]
    fn test_stack_fq12() {
        let mut data = [0; StorageAccount::TOTAL_SIZE];
        let mut account = StorageAccount::from_data(&mut data).unwrap();

        let f = Fq12::new(get_fq6(), get_fq6());

        account.push_fq12(f);
        let peek = account.peek_fq12(0);
        let pop = account.pop_fq12();

        assert_eq!(peek, f);
        assert_eq!(pop, f);
    }

    fn get_fq6() -> Fq6 {
        Fq6::new(
            Fq2::new(
                Fq::from_str("20925091368075991963132407952916453596237117852799702412141988931506241672722").unwrap(),
                Fq::from_str("18684276579894497974780190092329868933855710870485375969907530111657029892231").unwrap(),
            ),
            Fq2::new(
                Fq::from_str("5932690455294482368858352783906317764044134926538780366070347507990829997699").unwrap(),
                Fq::from_str("18684276579894497974780190092329868933855710870485375969907530111657029892231").unwrap(),
            ),
            Fq2::new(
                Fq::from_str("19526707366532583397322534596786476145393586591811230548888354920504818678603").unwrap(),
                Fq::from_str("19526707366532583397322534596786476145393586591811230548888354920504818678603").unwrap(),
            ),
        )
    }
}