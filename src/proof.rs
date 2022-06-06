#[cfg(not(tarpaulin_include))]
pub mod vkey;

pub mod verifier;

use borsh::{BorshSerialize, BorshDeserialize};
pub use verifier::*;
use ark_bn254::{Fq, Fq2, Fq6, Fq12, G1Affine};
use ark_ff::{Zero, BigInteger256, PrimeField};
use vkey::VerificationKey;
use crate::error::ElusivError;
use crate::macros::elusiv_account;
use crate::state::queue::ProofRequest;
use crate::types::{U256, MAX_PUBLIC_INPUTS_COUNT, Proof, Lazy};
use crate::fields::{Wrap, u256_to_fr, G1A, G2A, G2HomProjective};
use crate::macros::{guard, multi_instance_account};
use crate::error::ElusivError::AccountCannotBeReset;
use crate::state::program_account::SizedAccount;
use crate::bytes::BorshSerDeSized;

pub type RAMFq<'a> = LazyRAM<'a, Fq, 6>;
pub type RAMFq2<'a> = LazyRAM<'a, Fq2, 10>;
pub type RAMFq6<'a> = LazyRAM<'a, Fq6, 3>;
pub type RAMFq12<'a> = LazyRAM<'a, Fq12, 7>;
pub type RAMG2A<'a> = LazyRAM<'a, G2A, 1>;

/// Account used for verifying all kinds of Groth16 proofs over the span of multiple transactions
#[elusiv_account(pda_seed = b"proof", partial_computation)]
pub struct VerificationAccount {
    bump_seed: u8,
    initialized: bool,

    is_active: bool,
    instruction: u32,
    fee_payer: U256,

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

// We can allow multiple parallel proof verifications
multi_instance_account!(VerificationAccount<'a>, 1);

impl<'a> VerificationAccount<'a> {
    /// A VerificationAccount can be reset after a computation has been succesfully finished or has failed
    pub fn reset<VKey: VerificationKey>(
        &mut self,
        proof_request: ProofRequest,
    ) -> Result<(), ElusivError> {
        guard!(!self.get_is_active(), AccountCannotBeReset);

        self.set_is_verified(&false);
        self.set_is_active(&true);
        self.set_instruction(&0);

        let proof: Proof = proof_request.raw_proof().into();
        self.a.set(&proof.a);
        self.b.set(&proof.b);
        self.c.set(&proof.c);

        let public_inputs = proof_request.public_inputs();
        for i in 0..VKey::PUBLIC_INPUTS_COUNT {
            self.set_public_input(i, &Wrap(u256_to_fr(&public_inputs[i]).into_repr()));
        }

        self.prepared_inputs.set(&G1A(G1Affine::zero()));

        self.set_request(&proof_request);

        Ok(())
    }

    pub fn serialize_lazy_fields(&mut self) {
        self.ram_fq.serialize();
        self.ram_fq2.serialize();
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
        let mut data = Vec::new();
        for _ in 0..SIZE { data.push(None); }

        LazyRAM { data, frame: 0, source, changes: vec![false; SIZE] }
    }

    pub fn write(&mut self, value: N, index: usize) {
        self.data[self.frame + index] = Some(value);
    }

    pub fn read(&mut self, index: usize) -> N {
        let i = self.frame + index;
        match &self.data[i] {
            Some(v) => v.clone(),
            None => {
                let data = &self.source[i * <Wrap<N>>::SIZE..(i + 1) * <Wrap<N>>::SIZE];
                let v = <Wrap<N>>::try_from_slice(&data).unwrap();
                self.data[i] = Some(v.0);
                (&self.data[i]).unwrap().clone()
            }
        }
    }

    /// Call this before calling a function
    /// - we don't do any checked arithmethic here since we in any case require the calls and parameters to be correct (data is never dependent on user input)
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
}