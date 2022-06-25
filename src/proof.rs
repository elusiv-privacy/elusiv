#[cfg(not(tarpaulin_include))]
pub mod vkey;

pub mod verifier;

use borsh::{BorshSerialize, BorshDeserialize};
use solana_program::program_error::ProgramError;
pub use verifier::*;
use ark_bn254::{Fq, Fq2, Fq6, Fq12, G1Affine};
use ark_ff::{Zero, BigInteger256};
use vkey::VerificationKey;
use crate::error::ElusivError;
use crate::processor::ProofRequest;
use crate::state::program_account::{SizedAccount, PDAAccountData};
use crate::types::{U256, MAX_PUBLIC_INPUTS_COUNT, Proof, Lazy};
use crate::fields::{Wrap, G1A, G2A, G2HomProjective};
use crate::macros::{elusiv_account, guard};
use crate::bytes::BorshSerDeSized;

pub type RAMFq<'a> = LazyRAM<'a, Fq, 6>;
pub type RAMFq2<'a> = LazyRAM<'a, Fq2, 10>;
pub type RAMFq6<'a> = LazyRAM<'a, Fq6, 3>;
pub type RAMFq12<'a> = LazyRAM<'a, Fq12, 7>;
pub type RAMG2A<'a> = LazyRAM<'a, G2A, 1>;

/// Account used for verifying all kinds of Groth16 proofs over the span of multiple transactions
/// - exists only for verifying a single proof, closed afterwards
#[elusiv_account(pda_seed = b"proof", partial_computation)]
pub struct VerificationAccount {
    pda_data: PDAAccountData,

    is_active: bool,
    instruction: u32,
    fee_payer: U256,
    fee_version: u64,

    // if true, the proof request can be finalized
    is_verified: bool,

    // RAMs for storing computation values (they manage serialization on their own -> ram.serialize needs to be explicitly called)
    #[pub_non_lazy] ram_fq: RAMFq<'a>,
    #[pub_non_lazy] ram_fq2: RAMFq2<'a>,
    #[pub_non_lazy] ram_fq6: RAMFq6<'a>,
    #[pub_non_lazy] ram_fq12: RAMFq12<'a>,
    #[pub_non_lazy] ram_g2a: RAMG2A<'a>,

    // Proof
    #[pub_non_lazy] a: Lazy<'a, G1A>,
    #[pub_non_lazy] b: Lazy<'a, G2A>,
    #[pub_non_lazy] c: Lazy<'a, G1A>,

    // Public inputs
    public_input: [Wrap<BigInteger256>; MAX_PUBLIC_INPUTS_COUNT],

    // Computation values
    #[pub_non_lazy] prepared_inputs: Lazy<'a, G1A>,
    #[pub_non_lazy] r: Lazy<'a, G2HomProjective>,
    #[pub_non_lazy] f: Lazy<'a, Wrap<Fq12>>,

    // Request
    request: ProofRequest,
}

impl<'a> VerificationAccount<'a> {
    /// A VerificationAccount can be reset after a computation has been successfully finished or has failed
    pub fn reset(
        &mut self,
        public_inputs: &[BigInteger256],
        proof_request: ProofRequest,
        fee_payer: U256,
    ) -> Result<(), ProgramError> {
        guard!(!self.get_is_active(), ElusivError::AccountCannotBeReset);

        self.set_is_verified(&false);
        self.set_is_active(&true);
        self.set_instruction(&0);
        self.set_fee_payer(&fee_payer);
        self.set_fee_version(&proof_request.fee_version());

        let proof: Proof = proof_request.raw_proof().into();
        self.a.set(&proof.a);
        self.b.set(&proof.b);
        self.c.set(&proof.c);

        for (i, &public_input) in public_inputs.iter().enumerate() {
            self.set_public_input(i, &Wrap(public_input));
        }

        self.prepared_inputs.set(&G1A(G1Affine::zero()));

        self.set_request(&proof_request);

        Ok(())
    }

    pub fn serialize_lazy_fields(&mut self) {
        self.ram_fq.serialize();
        self.ram_fq2.serialize();
        self.ram_fq6.serialize();
        self.ram_fq12.serialize();
        self.ram_g2a.serialize();

        self.a.serialize();
        self.b.serialize();
        self.c.serialize();

        self.prepared_inputs.serialize();
        self.r.serialize();
        self.f.serialize();
    }
}

/// Stores data lazily on the heap, read requests will trigger deserialization
/// 
/// Note: heap allocation happens jit
pub struct LazyRAM<'a, N: Clone + Copy, const SIZE: usize> where Wrap<N>: BorshSerDeSized {
    /// Stores all serialized values
    /// - if an element has value None, it has not been initialized yet
    data: Vec<Option<N>>,
    source: &'a mut [u8],
    changes: Vec<bool>,

    /// Base-pointer for function-calls
    frame: usize,
}

impl<'a, N: Clone + Copy, const SIZE: usize> LazyRAM<'a, N, SIZE> where Wrap<N>: BorshSerDeSized {
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

    pub fn write(&mut self, value: N, index: usize) {
        self.check_vector_size(self.frame + index);
        self.data[self.frame + index] = Some(value);
        self.changes[self.frame + index] = true;
    }

    pub fn read(&mut self, index: usize) -> N {
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

    /// Call this before calling a function
    /// - we don't do any checked arithmetic here since we in any case require the calls and parameters to be correct (data is never dependent on user input)
    pub fn inc_frame(&mut self, frame: usize) {
        self.frame += frame;
    }

    /// Call this when returning a function
    pub fn dec_frame(&mut self, frame: usize) {
        self.frame -= frame;
    }

    pub fn serialize(&mut self) {
        for (i, &change) in self.changes.iter().enumerate() {
            if change {
                if let Some(value) = self.data[i] {
                    <Wrap<N>>::override_slice(
                        &Wrap(value),
                        &mut self.source[i * <Wrap<N>>::SIZE..(i + 1) * <Wrap<N>>::SIZE]
                    );
                }
            }
        }
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
    fn test_reset_verification_account() {

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

        ram.serialize();

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