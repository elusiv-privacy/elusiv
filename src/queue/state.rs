use super::ring_queue::{ RingQueue, queue_size };
use crate::types::U256;
use crate::commitment::commitment::*;
use super::proof_request::ProofRequest;
use super::send_finalization_request::SendFinalizationRequest;
use crate::bytes::{ serialize_u256, deserialize_u256 };
use crate::macros::{ ElusivAccount, remove_original_implementation };

const U_COMMITMENT_QUEUE_SIZE: usize = 20;
const V_COMMITMENT_QUEUE_SIZE: usize = 20;
const PROOF_QUEUE_SIZE: usize = 20;
pub const SEND_QUEUE_SIZE: usize = 20;

#[derive(ElusivAccount)]
#[remove_original_implementation]
struct QueueAccount {
    // Commitments which are missing a hash of core and amount
    #[queue(U_COMMITMENT_QUEUE_SIZE, UnverifiedCommitment::SIZE, serialize_unverified_commitment, deserialize_unverified_commitment)]
    pub unverified_commitment_queue: RingQueue<'a, U256>,

    // Commitments that have either been publicly hashed or verified using zk
    #[queue(V_COMMITMENT_QUEUE_SIZE, 32, serialize_u256, deserialize_u256)]
    pub verified_commitment_queue: RingQueue<'a, U256>,

    #[queue(PROOF_QUEUE_SIZE, ProofRequest::SIZE, ProofRequest::serialize, ProofRequest::deserialize)]
    pub proof_queue: RingQueue<'a, ProofRequest>,

    #[queue(SEND_QUEUE_SIZE, SendFinalizationRequest::SIZE, SendFinalizationRequest::serialize, SendFinalizationRequest::deserialize)]
    pub send_queue: RingQueue<'a, SendFinalizationRequest>,
}

impl<'a> QueueAccount<'a> {
    crate::macros::pubkey!("CYFkyPAmHjayCwhRS6LpQjY2E7atNeLS3b8FE1HTYQY4");
}
