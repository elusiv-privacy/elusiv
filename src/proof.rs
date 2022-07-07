#[cfg(not(tarpaulin_include))]
pub mod vkey;
pub mod verifier;
#[cfg(test)] mod test_proofs;

use borsh::{BorshSerialize, BorshDeserialize};
use elusiv_computation::RAM;
use elusiv_derive::BorshSerDeSized;
use solana_program::entrypoint::ProgramResult;
pub use verifier::*;
use ark_bn254::{Fq, Fq2, Fq6, Fq12};
use ark_ff::BigInteger256;
use vkey::VerificationKey;
use crate::error::ElusivError;
use crate::processor::ProofRequest;
use crate::state::program_account::{SizedAccount, PDAAccountData};
use crate::types::{U256, MAX_PUBLIC_INPUTS_COUNT, Proof, Lazy, RawProof};
use crate::fields::{Wrap, G1A, G2A, G2HomProjective};
use crate::macros::{elusiv_account, guard};
use crate::bytes::{BorshSerDeSized, ElusivOption, usize_as_u32_safe};

pub type RAMFq<'a> = LazyRAM<'a, Fq, 6>;
pub type RAMFq2<'a> = LazyRAM<'a, Fq2, 10>;
pub type RAMFq6<'a> = LazyRAM<'a, Fq6, 3>;
pub type RAMFq12<'a> = LazyRAM<'a, Fq12, 7>;
pub type RAMG2A<'a> = LazyRAM<'a, G2A, 1>;

const MAX_PREPARE_INPUTS_INSTRUCTIONS: usize = MAX_PUBLIC_INPUTS_COUNT * 10;

/// Account used for verifying proofs over the span of multiple transactions
/// - exists only for verifying a single proof, closed afterwards
#[elusiv_account(pda_seed = b"proof", partial_computation)]
pub struct VerificationAccount {
    pda_data: PDAAccountData,

    instruction: u32,
    round: u32,

    prepare_inputs_instructions_count: u32,
    prepare_inputs_instructions: [u16; MAX_PREPARE_INPUTS_INSTRUCTIONS],

    vkey: u8,
    step: VerificationStep,
    is_setup: bool,

    // Public inputs
    public_input: [Wrap<BigInteger256>; MAX_PUBLIC_INPUTS_COUNT],

    // Proof
    #[pub_non_lazy] a: Lazy<'a, G1A>,
    #[pub_non_lazy] b: Lazy<'a, G2A>,
    #[pub_non_lazy] c: Lazy<'a, G1A>,

    // Computation values
    #[pub_non_lazy] prepared_inputs: Lazy<'a, G1A>,
    #[pub_non_lazy] r: Lazy<'a, G2HomProjective>,
    #[pub_non_lazy] f: Lazy<'a, Wrap<Fq12>>,
    #[pub_non_lazy] alt_b: Lazy<'a, G2A>,
    coeff_index: u8,

    // RAMs for storing computation values
    #[pub_non_lazy] ram_fq: RAMFq<'a>,
    #[pub_non_lazy] ram_fq2: RAMFq2<'a>,
    #[pub_non_lazy] ram_fq6: RAMFq6<'a>,
    #[pub_non_lazy] ram_fq12: RAMFq12<'a>,

    // if true, the proof request can be finalized
    is_verified: ElusivOption<bool>,

    other_data: VerificationAccountData,
}

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct VerificationAccountData {
    pub fee_payer: U256,
    pub min_batching_rate: u32,
    pub remaining_amount: u64,
    pub unadjusted_fee: u64,
    pub tree_indices: [u64; 2],
    pub request: ProofRequest,
}

impl<'a> VerificationAccount<'a> {
    pub fn setup(
        &mut self,
        public_inputs: &[U256],
        proof: RawProof,
        instructions: &Vec<u32>,
        vkey: u8,
        data: VerificationAccountData,
    ) -> ProgramResult {
        self.set_other_data(&data);
        self.set_vkey(&vkey);

        // Save proof
        let proof: Proof = proof.into();

        self.a.set_serialize(&proof.a);
        self.b.set_serialize(&proof.b);
        self.c.set_serialize(&proof.c);

        // Temporarily save raw prepared inputs
        for (i, &public_input) in public_inputs.iter().enumerate() {
            let offset = i * 32;
            self.public_input[offset..(32 + offset)].copy_from_slice(&public_input[..32]);
        }

        // Setup input preparation instructions
        self.setup_public_inputs_instructions(instructions)?;

        self.serialize_rams()?;

        Ok(())
    }

    pub fn setup_public_inputs_instructions(
        &mut self,
        instructions: &Vec<u32>,
    ) -> Result<(), std::io::Error> {
        assert!(instructions.len() <= MAX_PREPARE_INPUTS_INSTRUCTIONS);

        self.set_prepare_inputs_instructions_count(&usize_as_u32_safe(instructions.len()));

        // It's guaranteed that the cast to u16 here is safe (see super::proof::vkey)
        let mut instructions: Vec<u16> = instructions.iter().map(|&x| x as u16).collect();
        instructions.extend(vec![0; MAX_PREPARE_INPUTS_INSTRUCTIONS - instructions.len()]);

        let instructions: [u16; MAX_PREPARE_INPUTS_INSTRUCTIONS] = instructions.try_into().unwrap();
        let bytes = instructions.try_to_vec()?;
        self.set_all_prepare_inputs_instructions(&bytes[..]);

        Ok(())
    }

    /// Only valid before public inputs have been setup
    pub fn load_raw_public_input(&self, index: usize) -> U256 {
        let offset = index * 32;
        self.public_input[offset..offset + 32].try_into().unwrap()
    }

    pub fn serialize_rams(&mut self) -> Result<(), std::io::Error> {
        self.ram_fq.serialize()?;
        self.ram_fq2.serialize()?;
        self.ram_fq6.serialize()?;
        self.ram_fq12.serialize()?;

        Ok(())
    }
}

/// Stores data lazily on the heap, read requests will trigger deserialization
/// 
/// Note: heap allocation happens jit
pub struct LazyRAM<'a, N: Clone + Copy, const SIZE: usize> {
    /// Stores all serialized values
    /// - if an element has value None, it has not been initialized yet
    data: Vec<Option<N>>,
    source: &'a mut [u8],
    changes: Vec<bool>,

    /// Base-pointer for sub-function-calls
    frame: usize,
}

impl<'a, N: Clone + Copy, const SIZE: usize> RAM<N> for LazyRAM<'a, N, SIZE>
where Wrap<N>: BorshSerDeSized
{
    fn write(&mut self, value: N, index: usize) {
        self.check_vector_size(self.frame + index);
        self.data[self.frame + index] = Some(value);
        self.changes[self.frame + index] = true;
    }

    fn read(&mut self, index: usize) -> N {
        let i = self.frame + index;
        self.check_vector_size(i);

        match &self.data[i] {
            Some(v) => *v,
            None => {
                let data = &self.source[i * <Wrap<N>>::SIZE..(i + 1) * <Wrap<N>>::SIZE];
                let v = <Wrap<N>>::try_from_slice(data).unwrap();
                self.data[i] = Some(v.0);
                (&self.data[i]).unwrap()
            }
        }
    }

    fn set_frame(&mut self, frame: usize) { self.frame = frame }
    fn get_frame(&mut self) -> usize { self.frame }
}

impl<'a, N: Clone + Copy, const SIZE: usize> LazyRAM<'a, N, SIZE>
where Wrap<N>: BorshSerDeSized
{
    const SIZE: usize = <Wrap<N>>::SIZE * SIZE;

    pub fn new(source: &'a mut [u8]) -> Self {
        assert!(source.len() == Self::SIZE);
        LazyRAM { data: vec![], frame: 0, source, changes: vec![] }
    }

    /// `check_vector_size` has to be called before every `data` access
    /// - this allows us to do jit heap allocation
    fn check_vector_size(&mut self, index: usize) {
        assert!(index < SIZE);
        if self.data.len() <= index {
            let extension = index + 1 - self.data.len();
            self.data.extend(vec![None; extension]);
            self.changes.extend(vec![false; extension]);
        }
    }

    pub fn serialize(&mut self) -> Result<(), std::io::Error> {
        for (i, &change) in self.changes.iter().enumerate() {
            if change {
                if let Some(value) = self.data[i] {
                    <Wrap<N>>::override_slice(
                        &Wrap(value),
                        &mut self.source[i * <Wrap<N>>::SIZE..(i + 1) * <Wrap<N>>::SIZE]
                    )?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::program_account::ProgramAccount;

    #[test]
    fn test_verification_account_setup() {
        let mut data = vec![0; VerificationAccount::SIZE];
        VerificationAccount::new(&mut data).unwrap();
    }

    #[test]
    #[ignore]
    fn test_setup_verification_account() {
        panic!()
    }

    #[test]
    #[ignore]
    fn test_setup_public_inputs_instructions() {
        panic!()
    }

    impl BorshDeserialize for Wrap<u64> {
        fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> { Ok(Wrap(u64::deserialize(buf)?)) }
    }
    impl BorshSerialize for Wrap<u64> {
        fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> { self.0.serialize(writer) }
    }
    impl BorshSerDeSized for Wrap<u64> {
        const SIZE: usize = u64::SIZE;
    }

    #[test]
    fn test_lazy_ram() {
        let mut data = vec![0; u64::SIZE * 2];
        let mut ram = LazyRAM::<'_, _, 2>::new(&mut data);

        ram.write(123456789u64, 0);
        assert_eq!(ram.read(0), 123456789);

        ram.inc_frame(1);
        ram.write(u64::MAX, 0);
        ram.dec_frame(1);

        assert_eq!(ram.read(0), 123456789);
        assert_eq!(ram.read(1), u64::MAX);

        ram.serialize().unwrap();

        assert_eq!(&data[..8], &u64::to_le_bytes(123456789)[..]);
        assert_eq!(&data[8..], &u64::to_le_bytes(u64::MAX)[..]);
    }

    #[test]
    fn test_check_vector_size() {
        let mut data = vec![0; VerificationAccount::SIZE];
        let account = VerificationAccount::new(&mut data).unwrap();
        let mut ram = account.ram_fq12; 

        assert_eq!(ram.data.len(), 0);
        assert_eq!(ram.changes.len(), 0);

        ram.check_vector_size(0);
        assert_eq!(ram.data.len(), 1);
        assert_eq!(ram.changes.len(), 1);

        ram.check_vector_size(0);
        assert_eq!(ram.data.len(), 1);
        assert_eq!(ram.changes.len(), 1);

        ram.check_vector_size(2);
        assert_eq!(ram.data.len(), 3);
        assert_eq!(ram.changes.len(), 3);
    }
}