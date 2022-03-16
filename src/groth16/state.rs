use super::super::storage_account::*;

use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use byteorder::{ ByteOrder, LittleEndian };
use ark_bn254::{ Fq, Fq2, Fq6, Fq12, G1Affine, G1Projective };
use ark_ff::*;
use solana_program::pubkey::Pubkey;
use super::lazy_stack::LazyHeapStack;
use super::super::error::ElusivError::{ InvalidStorageAccount, InvalidStorageAccountSize };
use super::super::fields::base::*;
use super::super::instruction::PUBLIC_INPUTS_COUNT;

const ZERO_1: Fq = field_new!(Fq, "0");
const ONE_1: Fq = field_new!(Fq, "1");

pub const STACK_FQ_SIZE: usize = 6;
pub const STACK_FQ2_SIZE: usize = 10;
pub const STACK_FQ6_SIZE: usize = 2;
pub const STACK_FQ12_SIZE: usize = 7;
pub const STACK_FQ_BYTES: usize = STACK_FQ_SIZE * 32 + 4;
pub const STACK_FQ2_BYTES: usize = STACK_FQ2_SIZE * 2 * 32 + 4;
pub const STACK_FQ6_BYTES: usize = STACK_FQ6_SIZE * 6 * 32 + 4;
pub const STACK_FQ12_BYTES: usize = STACK_FQ12_SIZE * 12 * 32 + 4;

solana_program::declare_id!("9s36U3mfkxCf4kqsa5FLLoYWoG93xCFvVEiExsZCjufd");

pub struct ProofVerificationAccount<'a> {
    pub amount: &'a mut [u8],
    pub recipient: &'a mut [u8],

    pub stack_fq: LazyHeapStack<'a, Fq, >,
    pub stack_fq2: LazyHeapStack<'a, Fq2>,
    pub stack_fq6: LazyHeapStack<'a, Fq6>,
    pub stack_fq12: LazyHeapStack<'a, Fq12>,

    /// Original inputs
    /// - `[u8; PUBLIC_INPUTS_COUNT * 32]`
    /// - big endian
    pub inputs_be: &'a mut [u8],
    prepared_inputs: Option<G1Affine>,
    coeff_ic: &'a mut [u8],

    pub proof_a: &'a mut [u8],
    pub proof_b: &'a mut [u8],
    pub proof_c: &'a mut [u8],
    pub b_neg: &'a mut [u8],

    iteration: &'a mut [u8],
    round: &'a mut [u8],
}

impl<'a> ProofVerificationAccount<'a> {
    pub const TOTAL_SIZE: usize = 8 + 32 + STACK_FQ_BYTES + STACK_FQ2_BYTES + STACK_FQ6_BYTES + STACK_FQ12_BYTES + PUBLIC_INPUTS_COUNT * 32 + 4 + G1AFFINE_SIZE + G2AFFINE_SIZE + G1AFFINE_SIZE + G2AFFINE_SIZE + 4 + 4;

    pub fn new(
        account_info: &solana_program::account_info::AccountInfo,
        data: &'a mut [u8],
        program_id: &solana_program::pubkey::Pubkey,
    ) -> Result<Self, ProgramError> {
        if account_info.owner != program_id { return Err(InvalidStorageAccount.into()); }
        if !account_info.is_writable { return Err(InvalidStorageAccount.into()); }
        // TODO: check id

        Self::from_data(data) 
    }

    pub fn from_data(data: &'a mut [u8]) -> Result<Self, ProgramError> {
        if data.len() != Self::TOTAL_SIZE { return Err(InvalidStorageAccountSize.into()); }

        let (amount, data) = data.split_at_mut(8);

        let (recipient, data) = data.split_at_mut(32);

        let (stack_fq, data) = data.split_at_mut(STACK_FQ_BYTES);
        let stack_fq = LazyHeapStack::new(stack_fq, STACK_FQ_SIZE, 32, serialize_fq, deserialize_fq);

        let (stack_fq2, data) = data.split_at_mut(STACK_FQ2_BYTES);
        let stack_fq2 = LazyHeapStack::new(stack_fq2, STACK_FQ2_SIZE, 64, serialize_fq2, deserialize_fq2);

        let (stack_fq6, data) = data.split_at_mut(STACK_FQ6_BYTES);
        let stack_fq6 = LazyHeapStack::new(stack_fq6, STACK_FQ6_SIZE, 192, serialize_fq6, deserialize_fq6);

        let (stack_fq12, data) = data.split_at_mut(STACK_FQ12_BYTES);
        let stack_fq12 = LazyHeapStack::new(stack_fq12, STACK_FQ12_SIZE, 384, serialize_fq12, deserialize_fq12);

        let (inputs_be, data) = data.split_at_mut(PUBLIC_INPUTS_COUNT * 32);
        let (coeff_ic, data) = data.split_at_mut(4);
        let (proof_a, data) = data.split_at_mut(G1AFFINE_SIZE);
        let (proof_b, data) = data.split_at_mut(G2AFFINE_SIZE);
        let (proof_c, data) = data.split_at_mut(G1AFFINE_SIZE);
        let (b_neg, data) = data.split_at_mut(G2AFFINE_SIZE);
        let (iteration, data) = data.split_at_mut(4);
        let (round, _) = data.split_at_mut(4);

        Ok(
            ProofVerificationAccount {
                amount,
                recipient,
                stack_fq,
                stack_fq2,
                stack_fq6,
                stack_fq12,
                inputs_be,
                prepared_inputs: None,
                coeff_ic,
                proof_a,
                proof_b,
                proof_c,
                b_neg,
                iteration,
                round,
            }
        )
    }

    pub fn init(
        &mut self,
        amount: u64,
        recipient: Pubkey,
        proof: super::Proof,
        public_inputs: [[u8; 32]; PUBLIC_INPUTS_COUNT],
    ) -> ProgramResult {
        // Amount
        for (i, byte) in amount.to_le_bytes().iter().enumerate() {
            self.amount[i] = *byte;
        }

        // Recipient
        for (i, byte) in recipient.to_bytes().iter().enumerate() {
            self.recipient[i] = *byte;
        }

        // Parse inputs
        // - big endian
        for (i, input) in public_inputs.iter().enumerate() {
            let bytes_be: Vec<u8> = input.iter().copied().rev().collect();
            for j in 0..32 {
                self.inputs_be[i * 32 + j] = bytes_be[j];
            }
        }

        // Reset stack
        self.stack_fq.clear();
        self.stack_fq2.clear();
        self.stack_fq6.clear();
        self.stack_fq12.clear();

        // Save proof
        write_g1_affine(&mut self.proof_a, proof.a);
        write_g1_affine(&mut self.proof_c, proof.c);
        write_g2_affine(&mut self.proof_b, proof.b);
        write_g2_affine(&mut self.b_neg, -proof.b);

        // Store proof computation values
        self.push_fq2(Fq2::one());
        self.push_fq2(proof.b.y);
        self.push_fq2(proof.b.x);

        // Push super::gamma_abc_g1_0() (aka the starting value for g_ic)
        self.push_fq(super::gamma_abc_g1_0().z);
        self.push_fq(super::gamma_abc_g1_0().y);
        self.push_fq(super::gamma_abc_g1_0().x);

        // Push the empy product acc
        push_g1_projective(self, G1Projective::zero());

        // Push the miller value
        self.push_fq12(Fq12::one());

        // Reset counters
        self.set_iteration(0);
        self.set_round(0);
        self.set_coeff_ic(0);

        // Save stack changes
        self.serialize();

        Ok(())
    }
}

// Stack pushing
impl<'a> ProofVerificationAccount<'a> {
    #[inline(always)]
    pub fn push_fq(&mut self, v: Fq) {
        self.stack_fq.push(v)
    }

    #[inline(always)]
    pub fn push_fq2(&mut self, v: Fq2) {
        self.stack_fq2.push(v);
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

// Stack popping
impl<'a> ProofVerificationAccount<'a> {
    pub fn pop_fq(&mut self) -> Fq {
        self.stack_fq.pop()
    }

    pub fn pop_fq2(&mut self) -> Fq2 {
        self.stack_fq2.pop()
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
        self.stack_fq2.peek(offset)
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
        bytes_to_u32(&self.iteration) as usize
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
    pub fn serialize(&mut self) {
        self.stack_fq.serialize_stack();
        self.stack_fq2.serialize_stack();
        self.stack_fq6.serialize_stack();
        self.stack_fq12.serialize_stack();
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

    pub fn get_prepared_inputs(&mut self) -> G1Affine {
        if let Some(pi) = self.prepared_inputs { return pi; }
        G1Affine::new(
            self.peek_fq(0),
            self.peek_fq(1),
            self.peek_fq(2) == ONE_1,
        )
    }
}

pub fn pop_g1_projective(account: &mut ProofVerificationAccount) -> G1Projective {
    G1Projective::new(
        account.pop_fq(),
        account.pop_fq(),
        account.pop_fq(),
    )
}

pub fn push_g1_projective(account: &mut ProofVerificationAccount, p: G1Projective) {
    account.push_fq(p.z);
    account.push_fq(p.y);
    account.push_fq(p.x);
}

pub fn peek_g1_affine(account: &mut ProofVerificationAccount) -> G1Affine {
    G1Affine::new(
        account.peek_fq(0),
        account.peek_fq(1),
        account.peek_fq(2) == ONE_1,
    )
}

pub fn pop_g1_affine(account: &mut ProofVerificationAccount) -> G1Affine {
    G1Affine::new(
        account.pop_fq(),
        account.pop_fq(),
        account.pop_fq() == ONE_1,
    )
}

pub fn push_g1_affine(account: &mut ProofVerificationAccount, p: G1Affine) {
    account.push_fq(if p.infinity { ONE_1 } else { ZERO_1 });
    account.push_fq(p.y);
    account.push_fq(p.x);
}

#[inline(always)]
fn save_fq(v: Fq, buffer: &mut [u8], offset: usize) {
    save_limb(v.0.0[0], buffer, 0 + offset);
    save_limb(v.0.0[1], buffer, 8 + offset);
    save_limb(v.0.0[2], buffer, 16 + offset);
    save_limb(v.0.0[3], buffer, 24 + offset);
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

fn serialize_fq(f: Fq, data: &mut [u8]) {
    save_fq(f, data, 0);
}

fn deserialize_fq(data: &[u8]) -> Fq {
    Fq::new(BigInteger256::read(data).unwrap())
}

fn serialize_fq2(f: Fq2, data: &mut [u8]) {
    save_fq(f.c0, data, 0);
    save_fq(f.c1, data, 32);
}

fn deserialize_fq2(data: &[u8]) -> Fq2 {
    Fq2::new(
        Fq::new(BigInteger256::read(&data[0..32]).unwrap()),
        Fq::new(BigInteger256::read(&data[32..64]).unwrap()),
    )
}

fn serialize_fq6(f: Fq6, data: &mut [u8]) {
    save_fq(f.c0.c0, data, 0);
    save_fq(f.c0.c1, data, 32);
    save_fq(f.c1.c0, data, 64);
    save_fq(f.c1.c1, data, 96);
    save_fq(f.c2.c0, data, 128);
    save_fq(f.c2.c1, data, 160);
}

fn deserialize_fq6(data: &[u8]) -> Fq6 {
    Fq6::new(
        Fq2::new(
            Fq::new(BigInteger256::read(&data[0..32]).unwrap()),
            Fq::new(BigInteger256::read(&data[32..64]).unwrap()),
        ),
        Fq2::new(
            Fq::new(BigInteger256::read(&data[64..96]).unwrap()),
            Fq::new(BigInteger256::read(&data[96..128]).unwrap()),
        ),
        Fq2::new(
            Fq::new(BigInteger256::read(&data[128..160]).unwrap()),
            Fq::new(BigInteger256::read(&data[160..192]).unwrap()),
        ),
    )
}

fn serialize_fq12(f: Fq12, data: &mut [u8]) {
    save_fq(f.c0.c0.c0, data, 0);
    save_fq(f.c0.c0.c1, data, 32);
    save_fq(f.c0.c1.c0, data, 64);
    save_fq(f.c0.c1.c1, data, 96);
    save_fq(f.c0.c2.c0, data, 128);
    save_fq(f.c0.c2.c1, data, 160);
    save_fq(f.c1.c0.c0, data, 192);
    save_fq(f.c1.c0.c1, data, 224);
    save_fq(f.c1.c1.c0, data, 256);
    save_fq(f.c1.c1.c1, data, 288);
    save_fq(f.c1.c2.c0, data, 320);
    save_fq(f.c1.c2.c1, data, 352);
}

fn deserialize_fq12(data: &[u8]) -> Fq12 {
    Fq12::new(
        deserialize_fq6(&data[0..192]),
        deserialize_fq6(&data[192..384]),
    )
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

    #[test]
    #[should_panic]
    fn test_invalid_size() {
        let mut data = [0; StorageAccount::TOTAL_SIZE - 1];
        StorageAccount::from_data(&mut data).unwrap();
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