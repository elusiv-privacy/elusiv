use solana_program::program_error::ProgramError;
use crate::proof::{PROOF_BYTES_SIZE };
use crate::bytes::*;
use crate::types::{ ProofData, U256 };

#[derive(Clone, Copy, PartialEq)]
#[derive(elusiv_account::ElusivInstruction)]
pub enum ProofRequest {
    Store {
        proof_data: ProofData,
        fee: u64,
        commitment: U256,
    },
    Bind {
        proof_data: ProofData,
        fee: u64,
        unbound_commitment: U256,
        bound_commitment: U256,
    },
    Send {
        proof_data: ProofData,
        fee: u64,
        recipient: U256,
    },
}

impl ProofRequest {
    pub const SIZE: usize = 8 + 32 + 32 + PROOF_BYTES_SIZE + 8 + 32 + 32;

    pub fn deserialize(data: &[u8]) -> ProofRequest {
        Self::unpack(data).unwrap()
    }

    pub fn serialize(value: ProofRequest) -> Vec<u8> {
        let mut buffer = Vec::new();
        
        match value {
            ProofRequest::Store { proof_data, fee, commitment } => {
                buffer.push(0);

                buffer.extend(write_proof_data(proof_data));
                buffer.extend(fee.to_le_bytes());
                buffer.extend(serialize_u256(commitment));
            },
            ProofRequest::Bind { proof_data, fee, unbound_commitment, bound_commitment } => {
                buffer.push(1);

                buffer.extend(write_proof_data(proof_data));
                buffer.extend(fee.to_le_bytes());
                buffer.extend(serialize_u256(unbound_commitment));
                buffer.extend(serialize_u256(bound_commitment));
            },
            ProofRequest::Send { proof_data, fee, recipient } => {
                buffer.push(2);

                buffer.extend(write_proof_data(proof_data));
                buffer.extend(fee.to_le_bytes());
                buffer.extend(serialize_u256(recipient));
            },
        }

        buffer
    }

    pub fn get_proof_data(&self) -> ProofData {
        match *self {
            ProofRequest::Store { proof_data, .. } => { proof_data },
            ProofRequest::Bind { proof_data, .. } => { proof_data },
            ProofRequest::Send { proof_data, .. } => { proof_data },
        }
    }

    pub fn get_fee(&self) -> u64 {
        match *self {
            ProofRequest::Store { fee, .. } => { fee },
            ProofRequest::Bind { fee, .. } => { fee },
            ProofRequest::Send { fee, .. } => { fee },
        }
    }

    pub fn get_commitments(&self) -> Vec<U256> {
        match *self {
            ProofRequest::Store { commitment, .. } => {
                vec![commitment]
            },
            ProofRequest::Bind { unbound_commitment, bound_commitment, .. } => {
                vec![
                    unbound_commitment,
                    bound_commitment,
                ]
             },
            ProofRequest::Send { .. } => {
                vec![]
            },
        }
    }

    pub fn get_public_inputs(&self) -> Vec<U256> {
        let proof_data = self.get_proof_data();
        let mut public_inputs = vec![
            proof_data.nullifier_hash,
            proof_data.root,
            u64_to_u256(
                proof_data.amount
            ),
        ];

        match *self {
            ProofRequest::Store { commitment, .. } => {
                public_inputs.push(commitment);
            },
            ProofRequest::Bind { unbound_commitment, bound_commitment, .. } => {
                public_inputs.push(unbound_commitment);
                public_inputs.push(bound_commitment);
            },
            ProofRequest::Send { recipient, .. } => {
                public_inputs.push(recipient);
            },
        }

        public_inputs
    }
}
