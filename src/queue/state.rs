use super::ring_queue::{ RingQueue, queue_size };
use super::super::types::U256;
use super::super::bytes::{ serialize_u256, deserialize_u256 };
use elusiv_account::*;

const COMMITMENT_QUEUE_SIZE: usize = 20;

solana_program::declare_id!("CYFkyPAmHjayCwhRS6LpQjY2E7atNeLS3b8FE1HTYQY4");

#[derive(ElusivAccount)]
#[remove_original_implementation]
struct QueueAccount {
    #[queue(COMMITMENT_QUEUE_SIZE, 32, serialize_u256, deserialize_u256)]
    pub commitment_queue: RingQueue<'a , U256>,
}