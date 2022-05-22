pub mod vkey;
pub mod verifier;

pub use verifier::*;
use ark_bn254::{Fq, Fq2, Fq6, Fq12, G1Affine};
use ark_ff::Zero;
use vkey::VerificationKey;
use crate::error::ElusivError;
use crate::macros::elusiv_account;
use crate::state::queue::ProofRequest;
use crate::types::{U256, MAX_PUBLIC_INPUTS_COUNT, Proof};
use crate::fields::{G1A, G2A};
use crate::macros::guard;
use crate::bytes::SerDe;
use crate::error::ElusivError::AccountCannotBeReset;
use crate::state::program_account::SizedAccount;

pub type RAMFq<'a> = LazyRAM<'a, Fq, 6>;
pub type RAMFq2<'a> = LazyRAM<'a, Fq2, 10>;
pub type RAMFq6<'a> = LazyRAM<'a, Fq6, 10>;
pub type RAMFq12<'a> = LazyRAM<'a, Fq12, 10>;
pub type RAMG2A<'a> = LazyRAM<'a, G2A, 4>;

pub const MAX_VERIFICATION_ACCOUNTS_COUNT: u64 = 1;

/// Account used for verifying all kinds of Groth16 proofs over the span of multiple transactions
#[elusiv_account(pda_seed = b"proof")]
pub struct VerificationAccount {
    // if true, the proof request can be finalized
    is_verified: bool,

    // if false: the account can be reset and a new computation can start, if true: clients can participate in the current computation by sending tx
    is_active: bool,

    // the index of the last round
    round: u64,

    // the count of all rounds
    total_rounds: u64,

    // RAMs for storing computation values (they manage serialization on their own -> ram.serialize needs to be explicitly called)
    #[pub_non_lazy]
    ram_fq: RAMFq<'a>,
    #[pub_non_lazy]
    ram_fq2: RAMFq2<'a>,
    #[pub_non_lazy]
    ram_fq6: RAMFq6<'a>,
    #[pub_non_lazy]
    ram_fq12: RAMFq12<'a>,
    #[pub_non_lazy]
    ram_g2a: RAMG2A<'a>,

    // Proof
    a: G1A,
    b: G2A,
    c: G1A,

    // Public inputs
    public_input: [U256; MAX_PUBLIC_INPUTS_COUNT],
    prepared_inputs: G1A,

    // Request
    request: ProofRequest,
}

impl<'a> VerificationAccount<'a> {
    /// A VerificationAccount can be reset after a computation has been succesfully finished or has failed
    pub fn reset<VKey: VerificationKey>(
        &mut self,
        proof_request: ProofRequest,
    ) -> Result<(), ElusivError> {
        guard!(!self.get_is_active(), AccountCannotBeReset);

        self.set_is_verified(false);
        self.set_is_active(true);
        self.set_round(0);
        self.set_total_rounds(VKey::ROUNDS as u64);

        let proof: Proof = proof_request.raw_proof().into();
        self.set_a(proof.a);
        self.set_b(proof.b);
        self.set_c(proof.c);

        let public_inputs = proof_request.public_inputs();
        for i in 0..VKey::PUBLIC_INPUTS_COUNT {
            self.set_public_input(i, public_inputs[i]);
        }

        self.set_prepared_inputs(G1A(G1Affine::zero()));

        self.set_request(proof_request);

        Ok(())
    }

    pub fn serialize_rams(&mut self) {
        self.ram_fq.serialize();
        self.ram_fq2.serialize();
        self.ram_fq12.serialize();
        self.ram_g2a.serialize();
    }
}

/// Stores data lazily on the heap, read requests will trigger deserialization
pub struct LazyRAM<'a, N: Clone + Copy + SerDe<T=N>, const SIZE: usize> {
    /// Stores all serialized values
    /// - if an element has value None, it has not been initialized yet
    data: Vec<Option<N>>,
    source: &'a mut [u8],
    changes: Vec<bool>,

    /// Base-pointer for function-calls
    frame: usize,
}

impl<'a, N: Clone + Copy + SerDe<T=N>, const SIZE: usize> LazyRAM<'a, N, SIZE> {
    const SIZE: usize = N::SIZE * SIZE;

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
                let data = &self.source[i * N::SIZE..(i + 1) * N::SIZE];
                let v = N::deserialize(data);
                self.data[i] = Some(v);
                (&self.data[i]).unwrap().clone()
            }
        }
    }

    pub fn free(&mut self, _index: usize) {
        // we don't need to give free any functionality, since it's the caller responsibility, to only read correct values
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
                    let data = &mut self.source[i * N::SIZE..(i + 1) * N::SIZE];
                    N::serialize(value, data);
                }
            }
        }
    }
}