use borsh::{BorshDeserialize, BorshSerialize};
use elusiv_derive::BorshSerDeSized;
use crate::bytes::BorshSerDeSized;

#[derive(BorshDeserialize, BorshSerialize, BorshSerDeSized)]
pub struct ProofRequest {
    empty: bool,
}