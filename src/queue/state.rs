#![allow(unused_imports)]

use super::ring_queue::RingQueue;
use crate::commitment::commitment::*;
use crate::bytes::*;
use crate::macros::*;
use crate::types::{ U256, JoinSplitProofData };

macro_rules! queue_account {
    ($name: ident, $size: expr, $type: ident) => {
        quote::quote! {
            #[crate::macros::elusiv_account]
            struct $name {
                head: u64,
                tail: u64,
                data: [$type: $size],
            }

            impl RingQueue for $name {
                type N = $type;
                const SIZE: u64 = $size;
            }
        }
    };
}

queue_account!(BaseCommitmentQueueAccount, 1024, BaseCommitmentHashRequest);
queue_account!(CommitmentQueueAccount, 1024, U256);
queue_account!(ProofQueueAccount, 256, ProofRequest);
queue_account!(SendQueueAccount, 256, SendFinalizationRequest);

#[derive(SerDe)]
pub enum ProofRequest {
    Send {
        proof_data: JoinSplitProofData<2>,
        recipient: U256,
        amount: u64,
    },
    Merge {
        proof_data: JoinSplitProofData<2>,
    },
    Migrate {
        proof_data: JoinSplitProofData<1>,
        current_nsmt_root: U256,
        next_nsmt_root: U256,
    },
}

#[derive(SerDe)]
pub struct SendFinalizationRequest {
    pub amount: u64,
    pub recipient: U256,
}