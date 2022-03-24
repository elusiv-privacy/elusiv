use elusiv_account::*;
use solana_program::entrypoint::ProgramResult;
use ark_bn254::{ Fq, Fq2, Fq6, Fq12, G1Affine, G2Affine, G1Projective };
use ark_ff::*;
use crate::error::ElusivError;
use crate::queue::proof_request::ProofRequest;
use super::VerificationKey;
use super::lazy_stack::{ LazyHeapStack, stack_size };
use super::super::fields::base::*;
use super::super::types::U256;

const ZERO_1: Fq = field_new!(Fq, "0");
const ONE_1: Fq = field_new!(Fq, "1");

const MAX_PUBLIC_INPUTS_COUNT: usize = 6;

solana_program::declare_id!("9KxywMSGSvk7yoVd3QV8bWbQd5EY4CPxxZZRtmAZaW2T");

#[derive(ElusivAccount)]
#[remove_original_implementation]
pub struct ProofAccount {
    // Is finished (if true, the account can be reset)
    is_finished: bool,

    // Stacks
    #[lazy_stack(6, 32, serialize_fq, deserialize_fq)]
    pub fq: LazyHeapStack<'a, Fq>,

    #[lazy_stack(10, 64, serialize_fq2, deserialize_fq2)]
    pub fq2: LazyHeapStack<'a, Fq2>,

    #[lazy_stack(2, 192, serialize_fq6, deserialize_fq6)]
    pub fq6: LazyHeapStack<'a, Fq6>,

    #[lazy_stack(7, 384, serialize_fq12, deserialize_fq12)]
    pub fq12: LazyHeapStack<'a, Fq12>,

    // Proof
    pub proof_a: G1Affine,
    pub proof_b: G2Affine,
    pub proof_c: G1Affine,
    pub proof_b_neg: G2Affine,

    // Public inputs
    inputs_be: [U256; MAX_PUBLIC_INPUTS_COUNT],
    #[lazy_option]
    prepared_inputs: G1Affine,
    current_coeff: u64,

    // Progress
    iteration: u64,
    round: u64,
}

impl<'a> ProofAccount<'a> {
    pub fn reset_with_request<VKey: VerificationKey>(
        &mut self,
        request: ProofRequest,
    ) -> ProgramResult {
        // Check if account can be reset
        if !self.get_is_finished() {
            return Err(ElusivError::ProofAccountCannotBeReset.into());
        }
        self.set_is_finished(false);

        // Parse proof
        let proof = super::Proof::from_bytes(&request.get_proof_data().proof)?;

        // Public inputs
        let public_inputs = request.get_public_inputs();

        self.reset::<VKey>(proof, &public_inputs)
    }

    pub fn reset<VKey: VerificationKey>(
        &mut self,
        proof: super::Proof,
        public_inputs: &[U256],
    ) -> ProgramResult {
        // Check public inputs count
        if public_inputs.len() != VKey::PUBLIC_INPUTS_COUNT {
            return Err(ElusivError::InvalidPublicInputs.into());
        }

        // Parse inputs
        // - big endian
        for (i, input) in public_inputs.iter().enumerate() {
            let bytes_be: Vec<u8> = input.iter().copied().rev().collect();
            self.set_inputs_be(i, &bytes_be);
        }

        // Reset stack
        self.fq.clear();
        self.fq2.clear();
        self.fq6.clear();
        self.fq12.clear();

        // Save proof
        self.set_proof_a(proof.a);
        self.set_proof_b(proof.b);
        self.set_proof_c(proof.c);
        self.set_proof_b_neg(-proof.b);

        // Store proof computation values
        self.fq2.push(Fq2::one());
        self.fq2.push(proof.b.y);
        self.fq2.push(proof.b.x);

        // Push super::gamma_abc_g1_0() (aka the starting value for g_ic)
        self.fq.push(VKey::gamma_abc_g1_0().z);
        self.fq.push(VKey::gamma_abc_g1_0().y);
        self.fq.push(VKey::gamma_abc_g1_0().x);

        // Push the empy product acc
        push_g1_projective(self, G1Projective::zero());

        // Push the miller value
        self.fq12.push(Fq12::one());

        // Reset counters
        self.set_iteration(0);
        self.set_round(0);
        self.set_current_coeff(0);

        // Save stack changes
        self.serialize();

        Ok(())
    }

    // Stack serialization
    pub fn serialize(&mut self) {
        self.fq.serialize_stack();
        self.fq2.serialize_stack();
        self.fq6.serialize_stack();
        self.fq12.serialize_stack();
    }

    // Proof preparation
    pub fn get_prepared_inputs(&mut self) -> G1Affine {
        match self.prepared_inputs {
            Some(v) => v,
            None => {
                let v = G1Affine::new(
                    self.fq.peek(0),
                    self.fq.peek(1),
                    self.fq.peek(2) == ONE_1,
                );
        
                self.prepared_inputs = Some(v);
                v
            }
        }
    }
}

pub fn pop_g1_projective(account: &mut ProofAccount) -> G1Projective {
    G1Projective::new(
        account.fq.pop(),
        account.fq.pop(),
        account.fq.pop(),
    )
}

pub fn push_g1_projective(account: &mut ProofAccount, p: G1Projective) {
    account.fq.push(p.z);
    account.fq.push(p.y);
    account.fq.push(p.x);
}

pub fn peek_g1_affine(account: &mut ProofAccount) -> G1Affine {
    G1Affine::new(
        account.fq.peek(0),
        account.fq.peek(1),
        account.fq.peek(2) == ONE_1,
    )
}

pub fn pop_g1_affine(account: &mut ProofAccount) -> G1Affine {
    G1Affine::new(
        account.fq.pop(),
        account.fq.pop(),
        account.fq.pop() == ONE_1,
    )
}

pub fn push_g1_affine(account: &mut ProofAccount, p: G1Affine) {
    account.fq.push(if p.infinity { ONE_1 } else { ZERO_1 });
    account.fq.push(p.y);
    account.fq.push(p.x);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bn254::{ Fq, Fq6 };
    use std::str::FromStr;

    #[test]
    fn test_correct_size() {
        let mut data = [0; ProofAccount::TOTAL_SIZE];
        ProofAccount::from_data(&mut data).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_invalid_size() {
        let mut data = [0; ProofAccount::TOTAL_SIZE - 1];
        ProofAccount::from_data(&mut data).unwrap();
    }

    #[test]
    fn test_stack_fq() {
        let mut data = [0; ProofAccount::TOTAL_SIZE];
        let mut account = ProofAccount::from_data(&mut data).unwrap();

        let f = Fq::from_str("20925091368075991963132407952916453596237117852799702412141988931506241672722").unwrap();

        account.fq.push(f);
        let peek = account.fq.peek(0);
        let pop = account.fq.pop();

        assert_eq!(peek, f);
        assert_eq!(pop, f);
    }

    #[test]
    fn test_stack_fq2() {
        let mut data = [0; ProofAccount::TOTAL_SIZE];
        let mut account = ProofAccount::from_data(&mut data).unwrap();

        let f = Fq2::new(
            Fq::from_str("20925091368075991963132407952916453596237117852799702412141988931506241672722").unwrap(),
            Fq::from_str("18684276579894497974780190092329868933855710870485375969907530111657029892231").unwrap(),
        );

        account.fq2.push(f);
        let peek = account.fq2.peek(0);
        let pop = account.fq2.pop();

        assert_eq!(peek, f);
        assert_eq!(pop, f);
    }

    #[test]
    fn test_stack_fq6() {
        let mut data = [0; ProofAccount::TOTAL_SIZE];
        let mut account = ProofAccount::from_data(&mut data).unwrap();

        let f = get_fq6();

        account.fq6.push(f);
        let peek = account.fq6.peek(0);
        let pop = account.fq6.pop();

        assert_eq!(peek, f);
        assert_eq!(pop, f);
    }

    #[test]
    fn test_stack_fq12() {
        let mut data = [0; ProofAccount::TOTAL_SIZE];
        let mut account = ProofAccount::from_data(&mut data).unwrap();

        let f = Fq12::new(get_fq6(), get_fq6());

        account.fq12.push(f);
        let peek = account.fq12.peek(0);
        let pop = account.fq12.pop();

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