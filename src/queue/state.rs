#![allow(unused_imports)]

use super::ring_queue::RingQueue;
use crate::types::U256;
use crate::commitment::commitment::*;
use super::proof_request::ProofRequest;
use super::send_finalization_request::SendFinalizationRequest;

macro_rules! queue_account {
    ($name: ident, $size: expr, $type: ident) => {
        quote::quote! {
            #[derive(crate::macros::ElusivAccount)]
            #[crate::macros::remove_token_stream]
            struct $name {
                size: u64,
                head: u64,
                tail: u64,
                data: [$type: $size],
            }

            impl RingQueue<$type> for $name { }
        }
    };
}

queue_account!(BaseCommitmentQueueAccount, 1024, BaseCommitmentHashRequest);
queue_account!(CommitmentQueueAccount, 1024, U256);
queue_account!(ProofQueueAccount, 256, ProofRequest);
queue_account!(SendQueueAccount, 256, SendFinalizationRequest);