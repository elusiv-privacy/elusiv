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

pub type RAMFq<'a> = LazyRAM<'a, Fq, 6>;
pub type RAMFq2<'a> = LazyRAM<'a, Fq2, 10>;
pub type RAMFq6<'a> = LazyRAM<'a, Fq6, 10>;
pub type RAMFq12<'a> = LazyRAM<'a, Fq12, 10>;
pub type RAMG2Affine<'a> = LazyRAM<'a, G2A, 4>;

pub const MAX_VERIFICATION_ACCOUNTS_COUNT: u64 = 1;

/// Account used for verifying all kinds of Groth16 proofs over the span of multiple transactions
#[elusiv_account(pda_seed = b"proof")]
pub struct VerificationAccount {
    // if true, the proof request can be finalized
    is_verified: bool,

    // `PartialComputationAccount` fields
    // if false: the account can be reset and a new computation can start, if true: clients can participate in the current computation by sending tx
    is_active: bool,
    // the index of the last round
    round: u64,
    // the count of all rounds
    total_rounds: u64,
    // account that payed the fees for the whole computation up-front (will be reimbursed after a successfull computation)
    fee_payer: U256,

    // RAMs for storing computation values
    ram_fq: RAMFq,
    ram_fq2: RAMFq2,
    ram_fq6: RAMFq6,
    ram_fq12: RAMFq12,
    ram_g2affine: RAMG2Affine,

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
        fee_payer: U256,
    ) -> Result<(), ElusivError> {
        guard!(!self.get_is_active(), AccountCannotBeReset);

        self.set_is_verified(false);
        self.set_is_active(true);
        self.set_round(0);
        self.set_total_rounds(VKey::ROUNDS as u64);
        self.set_fee_payer(fee_payer);

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
}

/// Used to store the different rams lazily
pub struct VerificationAccountWrapper<'a> {
    pub account: &'a mut VerificationAccount<'a>,

    ram_fq: Option<RAMFq<'a>>,
    ram_fq2: Option<RAMFq2<'a>>,
    ram_fq6: Option<RAMFq6<'a>>,
    ram_fq12: Option<RAMFq12<'a>>,
    ram_g2affine: Option<RAMG2Affine<'a>>,

    a: Option<G1A>,
    b: Option<G2A>,
    c: Option<G1A>,

    prepared_inputs: Option<G1A>,

    // used for preparing b in the combined miller loop
    // we store this value in the ram_fq2 and add a getter/setter
    pub r: Option<G2HomProjective>,

    // used for the final exponentiation
    // we store this value in the ram_fq12 and add a getter/setter
    pub f: Option<Fq12>,
}

/// Creates a function that allows for lazily storing RAM objects in the `VerificationAccountWrapper`
macro_rules! ram {
    ($fn_name: ident, $name: ident, $ty: ty) => {
        pub fn $fn_name(&mut self) -> &mut $ty {
            match self.$name {
                Some(v) => &mut v,
                None => {
                    let f = self.account.$name;
                    self.$name = Some(<$ty>::new(f));
                    &mut self.$name.unwrap()
                }
            }
        }
    };
}

macro_rules! getter {
    ($fn_name: ident, $name: ident, $ty: ty) => {
        pub fn $fn_name(&mut self) -> &mut $ty {
            match self.$name {
                Some(v) => &mut v,
                None => {
                    self.$name = Some(self.account.$fn_name());
                    &mut self.$name.unwrap()
                }
            }
        }
    };
}

impl<'a> VerificationAccountWrapper<'a> {
    pub fn new(account: &'a mut VerificationAccount<'a>) -> Self {
        Self {
            account,
            ram_fq: None, ram_fq2: None, ram_fq6: None, ram_fq12: None, ram_g2affine: None,
            a: None, b: None, c: None, prepared_inputs: None, r: None, f: None
        }
    }

    ram!(get_ram_fq, ram_fq, RAMFq);
    ram!(get_ram_fq2, ram_fq2, RAMFq2);
    ram!(get_ram_fq6, ram_fq6, RAMFq6);
    ram!(get_ram_fq12, ram_fq12, RAMFq12);
    ram!(get_ram_g2ffine, ram_g2affine, RAMG2Affine);

    getter!(get_a, a, G1A);
    getter!(get_b, b, G2A);
    getter!(get_c, c, G1A);

    getter!(get_prepared_inputs, prepared_inputs, G1A);

    pub fn get_r(&mut self) -> &mut G2HomProjective {
        match self.r {
            Some(v) => &mut v,
            None => {
                let ram = self.get_ram_fq2();
                self.r = Some(G2HomProjective { x: ram.read(0), y: ram.read(1), z: ram.read(2) });
                ram.inc_frame(3);
                &mut self.r.unwrap()
            }
        }
    }

    pub fn save_r(&mut self) {
        match self.r {
            Some(r) => {
                let ram = self.get_ram_fq2();
                ram.dec_frame(3);
                ram.write(r.x, 0);
                ram.write(r.y, 1);
                ram.write(r.z, 2);
            },
            None => {}
        }
    }

    pub fn get_f(&mut self) -> &mut Fq12 {
        match self.f {
            Some(v) => &mut v,
            None => {
                let ram = self.get_ram_fq12();
                ram.dec_frame(3);
                self.f = Some(ram.read(0));
                &mut self.f.unwrap()
            }
        }
    }

    pub fn save_f(&mut self) {
        match self.f {
            Some(f) => {
                let ram = self.get_ram_fq12();
                ram.dec_frame(1);
                ram.write(f, 0);
            },
            None => {}
        }
    }
}

/// Stores data lazily on the heap, read requests will trigger serialization
pub struct LazyRAM<'a, N: Clone + SerDe<T=N>, const SIZE: usize> {
    /// Stores all serialized values
    /// - if an element has value None, it has not been initialized yet
    data: Vec<Option<N>>,
    source: &'a [u8],
    changes: Vec<bool>,

    /// Base-pointer for function-calls
    frame: usize,
}

impl<'a, N: Clone + SerDe<T=N>, const SIZE: usize> SerDe for LazyRAM<'a, N, SIZE> {
    type T = LazyRAM<'a, N, SIZE>;
    const SIZE: usize = N::SIZE * SIZE;

    fn deserialize(data: &[u8]) -> Self::T {
        panic!()
        
    }

    fn serialize(value: Self::T, data: &mut [u8]) {
        for (i, &change) in value.changes.iter().enumerate() {
            if change {
                if let Some(value) = value.data[i] {
                    let data = &mut data[i * N::SIZE..(i + 1) * N::SIZE];
                    N::serialize(value, data);
                }
            }
        }
    }
}

impl<'a, N: Clone + SerDe<T=N>, const SIZE: usize> LazyRAM<'a, N, SIZE> {
    pub fn new(source: &'a mut [u8]) -> Self {
        assert!(source.len() == Self::SIZE);
        let data = Vec::new();
        for _ in 0..SIZE { data.push(None); }

        LazyRAM { data, frame: 0, source, changes: vec![false; SIZE] }
    }

    pub fn write(&mut self, value: N, index: usize) {
        self.data[self.frame + index] = Some(value);
    }

    pub fn read(&mut self, index: usize) -> N {
        let i = self.frame + index;
        match self.data[i] {
            Some(v) => v,
            None => {
                let data = &self.source[i * N::SIZE..(i + 1) * N::SIZE];
                let v = N::deserialize(data);
                self.data[i] = Some(v);
                v
            }
        }
    }

    pub fn free(&mut self, index: usize) {
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
}