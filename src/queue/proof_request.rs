use solana_program::program_error::ProgramError;
use crate::bytes::*;
use crate::proof::PROOF_BYTES_SIZE;
use crate::types::{ RawProof, U256 };

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, PartialEq)]
pub struct ProofRequest {
    pub proof: RawProof,
    pub nullifier_accounts: [U256; 2],
    pub nullifiers: [U256; 2],
    pub commitment: U256,
    pub kind: ProofRequestKind,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, PartialEq)]
pub enum ProofRequestKind {
    Send {
        recipient: U256,
        amount: u64,
    },
    Merge,
}

impl ProofRequestKind {
    pub const SIZE: usize = 32 + 8;

    pub fn deserialize(data: &[u8]) -> ProofRequestKind {
        Self::unpack(data).unwrap()
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        
        match *self {
            Self::Send { recipient, amount } => {
                buffer.push(0);
                buffer.extend(serialize_u256(recipient));
                buffer.extend(amount.to_le_bytes());
            },
            Self::Merge => {
                buffer.push(1);
            }
        }

        buffer
    }
}

impl ProofRequest {
    pub const SIZE: usize = PROOF_BYTES_SIZE + 64 + 64 + 32 + ProofRequestKind::SIZE;

    pub fn deserialize(data: &[u8]) -> ProofRequest {
        let (proof, data) = unpack_raw_proof(data).unwrap();

        let (nullifier_account_a, data) = unpack_u256(data).unwrap();
        let (nullifier_account_b, data) = unpack_u256(data).unwrap();
        let nullifier_accounts = [nullifier_account_a, nullifier_account_b];

        let (nullifier_a, data) = unpack_u256(data).unwrap();
        let (nullifier_b, data) = unpack_u256(data).unwrap();
        let nullifiers = [nullifier_a, nullifier_b];

        let (commitment, data) = unpack_u256(data).unwrap();
        let kind = ProofRequestKind::deserialize(data);

        ProofRequest { proof, nullifier_accounts, nullifiers, commitment, kind }
    }

    pub fn serialize(value: ProofRequest) -> Vec<u8> {
        let mut buffer = Vec::new();

        buffer.extend(write_raw_proof(value.proof));

        buffer.extend(serialize_u256(value.nullifier_accounts[0]));
        buffer.extend(serialize_u256(value.nullifier_accounts[1]));

        buffer.extend(serialize_u256(value.nullifier_accounts[1]));

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
