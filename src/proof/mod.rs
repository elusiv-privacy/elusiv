pub mod vkey;
mod verifier;
mod ram;

pub use verifier::*;
use ark_bn254::{ Fq, Fq2, Fq12, G1Affine, G2Affine, G1Projective };
use verifier::{ COMBINED_MILLER_LOOP_ROUNDS_COUNT, FINAL_EXPONENTIATION_ROUNDS_COUNT };
use ram::LazyRAM;
use crate::macros::elusiv_account;
use crate::state::program_account::PartialComputationAccount;
use crate::types::{ U256, MAX_PUBLIC_INPUTS_COUNT };

/// Groth16 verification key
pub trait VerificationKey {
    const PUBLIC_INPUTS_COUNT: usize;

    const PREPARE_PUBLIC_INPUTS_ROUNDS: usize = Self::PUBLIC_INPUTS_COUNT * 254;
    const COMBINED_MILLER_LOOP_ROUNDS: usize = Self::PREPARE_PUBLIC_INPUTS_ROUNDS + COMBINED_MILLER_LOOP_ROUNDS_COUNT;
    const FINAL_EXPONENTIATION_ROUNDS: usize = Self::COMBINED_MILLER_LOOP_ROUNDS + FINAL_EXPONENTIATION_ROUNDS_COUNT;

    fn gamma_abc_g1_0() -> G1Projective;
    fn gamma_abc_g1(index: usize) -> Vec<G1Affine>;
    fn alpha_g1_beta_g2() -> Fq12;
    fn gamma_g2_neg_pc(coeff_index: usize, i: usize) -> &'static Fq2;
    fn delta_g2_neg_pc(coeff_index: usize, i: usize) -> &'static Fq2;
    fn alpha_g1() -> G1Affine;
    fn beta_g2() -> G2Affine;
    fn gamma_g2() -> G2Affine;
    fn delta_g2() -> G2Affine;
}

pub type RAMFq<'a> = LazyRAM<'a, Fq, 6>;
pub type RAMFq2<'a> = LazyRAM<'a, Fq2, 10>;
pub type RAMFq6<'a> = LazyRAM<'a, Fq2, 10>;
pub type RAMFq12<'a> = LazyRAM<'a, Fq2, 10>;
pub type RAMG2Affine<'a> = LazyRAM<'a, Fq2, 10>;

/// Account used for verifying all kinds of Groth16 proofs over the span of multiple transactions
#[elusiv_account(pda_seed = b"proof")]
pub struct VerificationAccount {
    // PartialComputationAccount fields
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

/// Creates a function that allows for lazily storing RAM objects in the VerificationAccountWrapper
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