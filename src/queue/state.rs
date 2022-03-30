use super::ring_queue::{ RingQueue, queue_size };
use crate::types::U256;
use super::proof_request::ProofRequest;
use super::send_finalization_request::SendFinalizationRequest;
use crate::bytes::{ serialize_u256, deserialize_u256 };

const COMMITMENT_QUEUE_SIZE: usize = 20;
const QUEUE_SIZE: usize = 20;
pub const SEND_QUEUE_SIZE: usize = 20;

#[derive(elusiv_account::ElusivAccount)]
#[elusiv_account::remove_original_implementation]
struct QueueAccount {
    #[queue(COMMITMENT_QUEUE_SIZE, 32, serialize_u256, deserialize_u256)]
    pub commitment_queue: RingQueue<'a, U256>,

    #[queue(QUEUE_SIZE, ProofRequest::SIZE, ProofRequest::serialize, ProofRequest::deserialize)]
    pub proof_queue: RingQueue<'a, ProofRequest>,

    #[queue(SEND_QUEUE_SIZE, SendFinalizationRequest::SIZE, SendFinalizationRequest::serialize, SendFinalizationRequest::deserialize)]
    pub send_queue: RingQueue<'a, SendFinalizationRequest>,
}

impl<'a> QueueAccount<'a> {
    elusiv_account::pubkey!("CYFkyPAmHjayCwhRS6LpQjY2E7atNeLS3b8FE1HTYQY4");
}