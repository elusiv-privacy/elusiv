use crate::bytes::{unpack_raw_proof, unpack_u256, unpack_u64};
use crate::proof::PROOF_BYTES_SIZE;

use super::ring_queue::{ RingQueue, queue_size };
use super::super::types::{ U256, RawProof };
use super::super::bytes::{ serialize_u256, deserialize_u256 };
use elusiv_account::*;

const COMMITMENT_QUEUE_SIZE: usize = 20;
const STORE_QUEUE_SIZE: usize = 20;
const BIND_QUEUE_SIZE: usize = 20;
const SEND_QUEUE_SIZE: usize = 20;

solana_program::declare_id!("CYFkyPAmHjayCwhRS6LpQjY2E7atNeLS3b8FE1HTYQY4");

#[derive(ElusivAccount)]
#[remove_original_implementation]
struct QueueAccount {
    #[queue(COMMITMENT_QUEUE_SIZE, 32, serialize_u256, deserialize_u256)]
    pub commitment_queue: RingQueue<'a, U256>,

    #[queue(STORE_QUEUE_SIZE, StoreRequest::SIZE, StoreRequest::serialize, StoreRequest::deserialize)]
    pub store_queue: RingQueue<'a, StoreRequest>,

    #[queue(BIND_QUEUE_SIZE, BindRequest::SIZE, BindRequest::serialize, BindRequest::deserialize)]
    pub bind_queue: RingQueue<'a , BindRequest>,

    /*#[queue(SEND_QUEUE_SIZE, 32, serialize_u256, deserialize_u256)]
    pub store_queue: RingQueue<'a , SendRequest>,*/
}

#[derive(Clone, Copy)]
pub struct StoreRequest {
    pub proof: RawProof,
    pub commitment: U256,
    pub nullifier_hash: U256,
    pub fee: u64,
}

impl StoreRequest {
    const SIZE: usize = PROOF_BYTES_SIZE + 32 + 32 + 8;

    pub fn deserialize(data: &[u8]) -> StoreRequest {
        let (proof, data) = unpack_raw_proof(data).unwrap();
        let (commitment, data) = unpack_u256(data).unwrap();
        let (nullifier_hash, data) = unpack_u256(data).unwrap();
        let (fee, _) = unpack_u64(data).unwrap();

        StoreRequest { proof, commitment, nullifier_hash, fee }
    }

    pub fn serialize(request: StoreRequest) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.extend(request.proof);
        buffer.extend(request.commitment);
        buffer.extend(request.nullifier_hash);
        buffer.extend(request.fee.to_le_bytes());
        buffer
    }
}

#[derive(Clone, Copy)]
pub struct BindRequest {
    pub proof: RawProof,
    pub commitments: [U256; 2],
    pub nullifier_hash: U256,
    pub fee: u64,
}

impl BindRequest {
    const SIZE: usize = PROOF_BYTES_SIZE + 32 * 2 + 32 + 8;

    pub fn deserialize(data: &[u8]) -> BindRequest {
        let (proof, data) = unpack_raw_proof(data).unwrap();
        let (commitment_a, data) = unpack_u256(data).unwrap();
        let (commitment_b, data) = unpack_u256(data).unwrap();
        let (nullifier_hash, data) = unpack_u256(data).unwrap();
        let (fee, _) = unpack_u64(data).unwrap();

        BindRequest { proof, commitments: [commitment_a, commitment_b], nullifier_hash, fee }
    }

    pub fn serialize(request: BindRequest) -> Vec<u8> {
        let mut buffer = Vec::new();
        buffer.extend(request.proof);
        buffer.extend(request.commitments[0]);
        buffer.extend(request.commitments[1]);
        buffer.extend(request.nullifier_hash);
        buffer.extend(request.fee.to_le_bytes());
        buffer
    }
}

#[derive(Clone, Copy)]
pub struct SendRequest {
    pub proof: RawProof,
    pub amount: u64,
    pub recipient: U256,
    pub nullifier_hash: U256,
    pub fee: u64,
}