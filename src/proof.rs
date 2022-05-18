pub mod vkey;
mod verifier;
mod ram;

pub use verifier::*;
use ark_bn254::{ Fq, Fq2, Fq12, G1Affine, G2Affine };
use ram::LazyRAM;
use vkey::VerificationKey;
use crate::error::ElusivError;
use crate::error::ElusivError::{ AccountCannotBeReset };
use crate::macros::{elusiv_account, guard};
use crate::state::program_account::PartialComputationAccount;
use crate::state::queue::ProofRequest;
use crate::types::{ U256, MAX_PUBLIC_INPUTS_COUNT, Proof };

pub type RAMFq<'a> = LazyRAM<'a, Fq, 6>;
pub type RAMFq2<'a> = LazyRAM<'a, Fq2, 10>;
pub type RAMFq6<'a> = LazyRAM<'a, Fq2, 10>;
pub type RAMFq12<'a> = LazyRAM<'a, Fq2, 10>;
pub type RAMG2Affine<'a> = LazyRAM<'a, Fq2, 10>;

/// Account used for verifying all kinds of Groth16 proofs over the span of multiple transactions
#[elusiv_account(pda_seed = b"proof")]
pub struct VerificationAccount {
    // `PartialComputationAccount` trait fields
    is_active: bool,
    round: u64,
    total_rounds: u64,
    fee_payer: U256,

    // RAMs for storing computation values
    ram_fq: RAMFq<'a>,
    ram_fq2: RAMFq2<'a>,
    ram_fq6: RAMFq6<'a>,
    ram_fq12: RAMFq12<'a>,
    ram_g2affine: RAMG2Affine<'a>,

    // Proof
    a: G1Affine,
    b: G2Affine,
    c: G1Affine,

    // Public inputs
    public_input: [U256; MAX_PUBLIC_INPUTS_COUNT],
    prepared_inputs: G1Affine,
}

impl<'a> PartialComputationAccount for VerificationAccount<'a> { }

impl<'a> VerificationAccount<'a> {
    pub fn reset(
        &mut self,
        proof_request: ProofRequest,
        fee_payer: U256,
    ) -> Result<(), ElusivError> {
        guard!(!self.get_is_active(), AccountCannotBeReset);

        let vkey: dyn VerificationKey = proof_request.request.verification_key();

        self.set_is_active(true);
        self.set_round(0);
        self.set_total_rounds(vkey::VerificationKey::ROUNDS as u64);
        self.set_fee_payer(fee_payer);

        // TODO: reset rams ?

        let proof: Proof = proof_request.raw_proof().into();
        self.set_a(proof.a);
        self.set_b(proof.b);
        self.set_c(proof.c);

        let public_inputs = proof_request.public_inputs();
        for i in 0..vkey::VerificationKey::PUBLIC_INPUTS_COUNT {
            self.set_public_input(i, public_inputs[i]);
        }

        self.set_prepared_inputs(G1Affine::zero());

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

    a: Option<G1Affine>,
    b: Option<G2Affine>,
    c: Option<G1Affine>,

    prepared_inputs: Option<G1Affine>,

    // used for preparing b in the combined miller loop
    // we store this value in the ram_fq2 and add a getter/setter
    pub r: Option<G2HomProjective>,

    // used for the final exponentiation
    // we store this value in the ram_fq12 and add a getter/setter
    pub f: Option<Fq12>,
}

/// Creates a function that allows for lazily storing RAM objects in the `VerificationAccountWrapper`
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
    getter!(get_ram_fq, ram_fq, RAMFq<'a>);
    getter!(get_ram_fq2, ram_fq2, RAMFq2<'a>);
    getter!(get_ram_fq6, ram_fq6, RAMFq6<'a>);
    getter!(get_ram_fq12, ram_fq12, RAMFq12<'a>);
    getter!(get_ram_g2affine, ram_g2affine, RAMG2Affine<'a>);

    getter!(get_a, a, G1Affine);
    getter!(get_b, b, G2Affine);
    getter!(get_c, c, G1Affine);

    getter!(get_prepared_inputs, prepared_inputs, G1Affine);

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