use solana_program::program_error::ProgramError;
use crate::bytes::*;
use crate::types::{ ProofData, U256 };

#[derive(Clone, Copy, PartialEq)]
pub struct ProofRequest {
    pub proof_data: ProofData,
    pub nullifier_account: U256,
    pub fee: u64,
    pub kind: ProofRequestKind,
}

#[derive(Clone, Copy, PartialEq)]
#[derive(elusiv_account::ElusivInstruction)]
pub enum ProofRequestKind {
    Store {
        commitment: U256,
    },
    Bind {
        unbound_commitment: U256,
        bound_commitment: U256,
    },
    Send {
        recipient: U256,
    },
}

impl ProofRequestKind {
    pub const SIZE: usize = 32 + 32;

    pub fn deserialize(data: &[u8]) -> ProofRequestKind {
        Self::unpack(data).unwrap()
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        
        match *self {
            Self::Store { commitment } => {
                buffer.push(0);

                buffer.extend(serialize_u256(commitment));
            },
            Self::Bind { unbound_commitment, bound_commitment } => {
                buffer.push(1);

                buffer.extend(serialize_u256(unbound_commitment));
                buffer.extend(serialize_u256(bound_commitment));
            },
            Self::Send { recipient } => {
                buffer.push(2);

                buffer.extend(serialize_u256(recipient));
            },
        }

        buffer
    }
}

impl ProofRequest {
    pub const SIZE: usize = ProofData::SIZE + 32 + 32 + 8 + ProofRequestKind::SIZE;

    pub fn deserialize(data: &[u8]) -> ProofRequest {
        let (proof_data, data) = unpack_proof_data(data).unwrap();
        let (nullifier_account, data) = unpack_u256(data).unwrap();
        let (fee, data) = unpack_u64(data).unwrap();
        let kind = ProofRequestKind::deserialize(data);

        ProofRequest { proof_data, nullifier_account, fee, kind }
    }

    pub fn serialize(value: ProofRequest) -> Vec<u8> {
        let mut buffer = Vec::new();

        buffer.extend(write_proof_data(value.proof_data));
        buffer.extend(serialize_u256(value.nullifier_account));
        buffer.extend(value.fee.to_le_bytes());
        buffer.extend(value.kind.serialize());
        
        buffer
    }

    pub fn get_commitments(&self) -> Vec<U256> {
        match self.kind {
            ProofRequestKind::Store { commitment } => {
                vec![commitment]
            },
            ProofRequestKind::Bind { unbound_commitment, bound_commitment } => {
                vec![
                    unbound_commitment,
                    bound_commitment,
                ]
             },
             ProofRequestKind::Send { .. } => {
                vec![]
            },
        }
    }

    pub fn get_public_inputs(&self) -> Vec<U256> {
        let mut public_inputs = vec![
            self.proof_data.nullifier,
            self.proof_data.root,
            u64_to_u256(
                self.proof_data.amount
            ),
        ];

        match self.kind {
            ProofRequestKind::Store { commitment, .. } => {
                public_inputs.push(commitment);
            },
            ProofRequestKind::Bind { unbound_commitment, bound_commitment, .. } => {
                public_inputs.push(unbound_commitment);
                public_inputs.push(bound_commitment);
            },
            ProofRequestKind::Send { recipient, .. } => {
                public_inputs.push(recipient);
            },
        }

        public_inputs
    }
}
