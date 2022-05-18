//! Queues are used to store hash, proof verification and payment requests

use crate::error::ElusivError;
use crate::error::ElusivError::{ QueueIsFull, QueueIsEmpty };
use crate::macros::guard;
use crate::bytes::*;
use crate::macros::*;
use crate::types::{ U256, JoinSplitProofData, JoinSplitPublicInputs, RawProof };

/// Generates a `QueueAccount` that implements the `RingQueue` trait
macro_rules! queue_account {
    ($name: ident, $size: expr, $type: ident) => {
        {
            //#[crate::macros::elusiv_account]
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

// Queue used for storing the base_commitments and amounts that should be hashed into commitments
queue_account!(BaseCommitmentQueueAccount, 1024, BaseCommitmentHashRequest);

// Queue used for storing commitments that should sequentially inserted into the active Merkle tree
queue_account!(CommitmentQueueAccount, 1024, U256);

// Queue storing any kind of proof that needs to be verified
queue_account!(ProofQueueAccount, 256, ProofRequest);

// Queue storing the money transfer requests derived from verified Send proofs
queue_account!(SendQueueAccount, 256, SendFinalizationRequest);

#[derive(SerDe)]
/// Request for computing `commitment = h(base_commitment, amount)`
pub struct BaseCommitmentHashRequest {
    pub base_commitment: U256,
    pub amount: u64,
    pub commitment: U256,
}

#[derive(SerDe)]
pub struct ProofRequest<const N: usize> {
    pub proof_data: JoinSplitProofData<N>,
    pub public_inputs: JoinSplitProofData<N>,
}

#[derive(SerDe)]
/// Request for transferring `amount` funds to a `recipient`
pub struct SendFinalizationRequest {
    pub amount: u64,
    pub recipient: U256,
}

/// Ring queue with a capacity of `SIZE - 1` elements
/// - works by having two pointers, `head` and `tail` and a some data storage with getter, setter
/// - `head` points to the first element (first according to the FIFO definition)
/// - `tail` points to the location to insert the next element
/// - `head == tail - 1` => queue is full
/// - `head == tail` => queue is empty
pub trait RingQueue {
    type N: PartialEq;
    const SIZE: u64;

    fn get_head(&self) -> u64;
    fn set_head(&mut self, value: u64);

    fn get_tail(&self) -> u64;
    fn set_tail(&mut self, value: u64);

    fn get_data(&self, index: usize) -> Self::N;
    fn set_data(&mut self, index: usize, value: Self::N);

    /// Try to enqueue a new element in the queue
    fn enqueue(&mut self, value: Self::N) -> Result<(), ElusivError> {
        let head = self.get_head();
        let tail = self.get_tail();

        let next_tail = (tail + 1) % Self::SIZE;
        guard!(next_tail != head, QueueIsFull);

        self.set_data(tail as usize, value);
        self.set_tail(next_tail);

        Ok(())
    }

    /// Try to read the first element in the queue without removing it
    fn view_first(&self) -> Result<Self::N, ElusivError> {
        let head = self.get_head();
        let tail = self.get_tail();

        guard!(head != tail, QueueIsEmpty);

        Ok(self.get_data(head as usize))
    }

    /// Try to remove the first element from the queue
    fn dequeue_first(&mut self) -> Result<Self::N, ElusivError> {
        let head = self.get_head();
        let tail = self.get_tail();

        guard!(head != tail, QueueIsEmpty);

        let value = self.get_data(head as usize);
        self.set_head((head + 1) % Self::SIZE);

        Ok(value)
    }

    fn contains(&self, value: Self::N) -> bool {
        let mut ptr = self.get_head();
        let tail = self.get_tail();

        while ptr != tail {
            if self.get_data(ptr as usize) == value { return true; }
            ptr = (ptr + 1) % Self::SIZE;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: usize = 7;

    struct TestQueue {
        head: usize,
        tail: usize,
        data: [usize; SIZE],
    }

    impl RingQueue for TestQueue {
        const SIZE: u64 = SIZE as u64;

        fn get_head(&self) -> usize { self.head }
        fn set_head(&mut self, value: usize) { self.head = value; }

        fn get_tail(&self) -> usize { self.tail }
        fn set_tail(&mut self, value: usize) { self.tail = value; }

        fn get_data(&self, index: usize) -> usize { self.data[index] }
        fn set_data(&mut self, index: usize, value: usize) { self.data[index] = value; }
    }

    #[test]
    fn test_persistent_fifo() {
        let mut queue = TestQueue { head: 0, tail: 0, data: [0; SIZE] };
        for i in 1..SIZE {
            queue.enqueue(i as u32).unwrap();
            assert_eq!(1, queue.view_first().unwrap()); // first element does not change
        }
    }

    #[test]
    fn test_max_size() {
        let mut full_queue = TestQueue { head: 1, tail: 0, data: [0; SIZE] };
        assert!(matches!(full_queue.enqueue(1), Err(_)));
    }

    #[test]
    fn test_ordering() {
        let mut queue = TestQueue { head: 0, tail: 0, data: [0; SIZE] };
        for i in 1..SIZE {
            assert_eq!(i, queue.view_first().unwrap());
            queue.dequeue_first().unwrap();
        }
        assert!(matches!(queue.dequeue_first(), Err(_)));
    }
}