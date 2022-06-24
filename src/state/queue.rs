//! Queues are used to store hash, proof verification and payment requests

use borsh::{BorshSerialize, BorshDeserialize};
use solana_program::program_error::ProgramError;
use crate::commitment::commitments_per_batch;
use crate::error::ElusivError::{QueueIsFull, QueueIsEmpty, InvalidFeeVersion, InvalidQueueAccess};
use crate::macros::guard;
use crate::bytes::*;
use crate::macros::*;
use crate::processor::{BaseCommitmentHashRequest, CommitmentHashRequest};
use super::program_account::{SizedAccount, ProgramAccount, PDAAccountFields, MultiInstancePDAAccount};

/// Generates a `QueueAccount` and a `Queue` that implements the `RingQueue` trait
macro_rules! queue_account {
    ($id: ident, $id_account: ident, $seed: literal, $size: literal, $ty_element: ty, $max_instances: literal) => {
        #[elusiv_account(pda_seed = $seed)]
        pub struct $id_account {
            bump_seed: u8,
            version: u8,
            initialized: bool,

            head: u64,
            tail: u64,
            data: [$ty_element; $size],
        }

        const_assert_eq!(
            <$id_account>::SIZE,
            PDAAccountFields::SIZE + (8 + 8) + <$ty_element>::SIZE * ($size)
        );

        const_assert_eq!(
            <$id>::SIZE,
            $size
        );

        impl<'a> MultiInstancePDAAccount for $id_account<'a> {
            const MAX_INSTANCES: u64 = $max_instances;
        }

        const_assert!(1 <= $id_account::MAX_INSTANCES);

        pub struct $id<'a, 'b> {
            account: &'b mut $id_account<'a>,
        }

        impl<'a, 'b> Queue<'a, 'b, $id_account<'a>> for $id<'a, 'b> {
            type T = $id<'a, 'b>;
            fn new(account: &'b mut $id_account<'a>) -> Self::T { $id { account } }
        }
        
        impl<'a, 'b> RingQueue for $id<'a, 'b> {
            type N = $ty_element;
            const CAPACITY: u64 = $size - 1;
        
            fn get_head(&self) -> u64 { self.account.get_head() }
            fn set_head(&mut self, value: &u64) { self.account.set_head(value) }
            fn get_tail(&self) -> u64 { self.account.get_tail() }
            fn set_tail(&mut self, value: &u64) { self.account.set_tail(value) }
            fn get_data(&self, index: usize) -> Self::N { self.account.get_data(index) }
            fn set_data(&mut self, index: usize, value: &Self::N) { self.account.set_data(index, value) }
        }
    };
}

pub trait Queue<'a, 'b, Account: ProgramAccount<'a>> {
    type T;
    fn new(account: &'b mut Account) -> Self::T;
}

// Base commitment queue
queue_account!(BaseCommitmentQueue, BaseCommitmentQueueAccount, b"base_commitment_queue", 101, BaseCommitmentHashRequest, 10);

// Queue used for storing commitments that should sequentially inserted into the active MT
queue_account!(CommitmentQueue, CommitmentQueueAccount, b"commitment_queue", 240, CommitmentHashRequest, 1);

impl<'a, 'b> CommitmentQueue<'a, 'b> {
    /// Returns the next batch of commitments to be hashed together
    pub fn next_batch(&self) -> Result<(Vec<CommitmentHashRequest>, u32), ProgramError> {
        let mut requests = Vec::new();
        let mut highest_batching_rate = 0;
        let mut commitment_count: usize = u32::MAX as usize;
        let mut fee_version = None;

        while requests.len() < commitment_count {
            let request = self.view(requests.len())?;

            highest_batching_rate = std::cmp::max(highest_batching_rate, request.min_batching_rate);
            commitment_count = commitments_per_batch(highest_batching_rate);

            // Just a (hopefully always) redundant fee-check (depends on the fee upgrade logic)
            match fee_version {
                Some(f) => guard!(f == request.fee_version, InvalidFeeVersion),
                None => {}
            };
            fee_version = Some(request.fee_version);

            requests.push(request);
        }

        Ok((requests, highest_batching_rate))
    }
}

/// Ring queue with a capacity of `CAPACITY` elements
/// - works by having two pointers, `head` and `tail` and a some data storage with getter, setter
/// - `head` points to the first element (first according to the FIFO definition)
/// - `tail` points to the location to insert the next element
/// - `head == tail - 1` => queue is full
/// - `head == tail` => queue is empty
pub trait RingQueue {
    type N: PartialEq + BorshSerDeSized + Clone;
    const CAPACITY: u64;
    const SIZE: u64 = Self::CAPACITY + 1;

    fn get_head(&self) -> u64;
    fn set_head(&mut self, value: &u64);

    fn get_tail(&self) -> u64;
    fn set_tail(&mut self, value: &u64);

    fn get_data(&self, index: usize) -> Self::N;
    fn set_data(&mut self, index: usize, value: &Self::N);

    /// Try to enqueue a new element in the queue
    fn enqueue(&mut self, value: Self::N) -> Result<(), ProgramError> {
        let head = self.get_head();
        let tail = self.get_tail();

        let next_tail = (tail + 1) % Self::SIZE;
        guard!(next_tail != head, QueueIsFull);

        self.set_data(u64_as_usize_safe(tail), &value);
        self.set_tail(&next_tail);

        Ok(())
    }

    /// Try to read the first element in the queue without removing it
    fn view_first(&self) -> Result<Self::N, ProgramError> {
        self.view(0)
    }

    fn view(&self, offset: usize) -> Result<Self::N, ProgramError> {
        let head = self.get_head();
        let tail = self.get_tail();
        guard!(head != tail, QueueIsEmpty);
        guard!((offset as u64) < self.len(), InvalidQueueAccess);

        Ok(self.get_data((u64_as_usize_safe(head) + offset) % (u64_as_usize_safe(Self::SIZE))))
    }

    /// Try to remove the first element from the queue
    fn dequeue_first(&mut self) -> Result<Self::N, ProgramError> {
        let head = self.get_head();
        let tail = self.get_tail();
        guard!(head != tail, QueueIsEmpty);

        let value = self.get_data(u64_as_usize_safe(head));
        self.set_head(&((head + 1) % Self::SIZE));

        Ok(value)
    }

    fn contains(&self, value: &Self::N) -> bool {
        let mut ptr = self.get_head();
        let tail = self.get_tail();

        while ptr != tail {
            if self.get_data(u64_as_usize_safe(ptr)) == *value { return true; }
            ptr = (ptr + 1) % Self::SIZE;
        }

        false
    }

    fn len(&self) -> u64 {
        let head = self.get_head();
        let tail = self.get_tail();

        if tail < head {
            head + tail
        } else {
            tail - head
        }
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn empty_slots(&self) -> u64 {
        Self::CAPACITY - self.len()
    }

    #[cfg(test)]
    fn clear(&mut self) {
        self.set_head(&0);
        self.set_tail(&0);
    }
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;

    use crate::{commitment::MAX_COMMITMENT_BATCHING_RATE, fields::{u64_to_scalar, fr_to_u256_le}};

    use super::*;

    struct TestQueue<const S: usize> {
        head: u64,
        tail: u64,
        data: [u32; S],
    }

    impl<const S: usize> RingQueue for TestQueue<S> {
        type N = u32;
        const CAPACITY: u64 = S as u64 - 1;

        fn get_head(&self) -> u64 { self.head }
        fn set_head(&mut self, value: &u64) { self.head = *value; }

        fn get_tail(&self) -> u64 { self.tail }
        fn set_tail(&mut self, value: &u64) { self.tail = *value; }

        fn get_data(&self, index: usize) -> u32 { self.data[index] }
        fn set_data(&mut self, index: usize, value: &u32) { self.data[index] = *value; }
    }

    impl<const S: usize> TestQueue<S> {
        pub fn capacity(&self) -> u64 { Self::CAPACITY }
    }

    macro_rules! test_queue {
        ($id: ident, $size: literal, $head: literal, $tail: literal) => {
            let mut $id = TestQueue { head: $head, tail: $tail, data: [0; $size] };
        };
    }

    #[test]
    fn test_persistent_fifo() {
        test_queue!(queue, 7, 0, 0);

        for i in 0..queue.capacity() {
            queue.enqueue(i as u32).unwrap();
            assert_eq!(0, queue.view_first().unwrap()); // first element does not change
            assert_eq!(queue.len(), i as u64 + 1);
        }
    }

    #[test]
    fn test_full_cycle() {
        test_queue!(queue, 7, 0, 0);

        for i in 0..queue.capacity() {
            queue.enqueue(i as u32).unwrap();
            assert_eq!(0, queue.view_first().unwrap()); // first element does not change
            assert_eq!(queue.len(), i as u64 + 1);
        }

        assert_matches!(queue.enqueue(2), Err(_));

        // Remove and insert one
        for i in 0..queue.capacity() {
            queue.dequeue_first().unwrap();
            queue.enqueue(i as u32).unwrap();
        }
    }

    #[test]
    fn test_max_size() {
        test_queue!(full_queue, 3, 1, 0);
        assert_matches!(full_queue.enqueue(1), Err(_));

        full_queue.dequeue_first().unwrap();
        assert_matches!(full_queue.enqueue(1), Ok(()));
        assert_matches!(full_queue.enqueue(2), Err(_));

        full_queue.dequeue_first().unwrap();
        assert_matches!(full_queue.enqueue(2), Ok(()));
    }

    #[test]
    fn test_len() {
        test_queue!(queue, 4, 0, 0);
        assert_eq!(queue.len(), 0);
        assert_eq!(queue.empty_slots(), 3);

        queue.enqueue(0).unwrap();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.empty_slots(), 2);

        queue.dequeue_first().unwrap();
        assert_eq!(queue.len(), 0);
        assert_eq!(queue.empty_slots(), 3);
    }

    #[test]
    fn test_ordering() {
        test_queue!(queue, 13, 0, 0);

        for i in 1..13 {
            queue.enqueue(i as u32).unwrap();
        }

        for i in 1..13 {
            assert_eq!(i as u32, queue.view_first().unwrap());
            queue.dequeue_first().unwrap();
        }
        assert_matches!(queue.dequeue_first(), Err(_));
    }

    #[test]
    fn test_view() {
        test_queue!(queue, 13, 0, 0); 

        assert_matches!(queue.view(0), Err(_));

        queue.enqueue(0).unwrap();

        queue.view(0).unwrap();
        assert_matches!(queue.view(1), Err(_));
    }

    #[test]
    fn test_clear_queue() {
        test_queue!(queue, 13, 0, 0);
        queue.enqueue(0).unwrap();
        queue.clear();
        assert!(queue.is_empty());
    }

    #[test]
    fn test_next_batch() {
        let mut data = vec![0; CommitmentQueueAccount::SIZE];
        let mut q = CommitmentQueueAccount::new(&mut data).unwrap();
        let mut q = CommitmentQueue::new(&mut q);

        // Incomplete batch
        for _ in 0..3 {
            q.enqueue(CommitmentHashRequest { commitment: [0; 32], fee_version: 0, min_batching_rate: 2 }).unwrap();
        }
        assert_matches!(q.next_batch(), Err(_));

        // Complete batches (with variing batching rates)
        q.clear();
        for b in 0..=MAX_COMMITMENT_BATCHING_RATE {
            let c = commitments_per_batch(b as u32);
            for i in 0..c {
                q.enqueue(
                    CommitmentHashRequest {
                        commitment: fr_to_u256_le(&u64_to_scalar(i as u64)),
                        fee_version: 0,
                        min_batching_rate: if i == 0 { b as u32 } else { 0 },
                    }
                ).unwrap();
            }

        }

        for b in 0..=MAX_COMMITMENT_BATCHING_RATE {
            let (batch, batching_rate) = q.next_batch().unwrap();
            for _ in 0..commitments_per_batch(batching_rate) {
                q.dequeue_first().unwrap();
            }

            assert_eq!(batching_rate as usize, b);
            for (i, c) in batch.iter().enumerate() {
                assert_eq!(c.commitment, fr_to_u256_le(&u64_to_scalar(i as u64)));
            }
        }

        // Mismatching fee
        q.clear();
        q.enqueue(CommitmentHashRequest { commitment: [0; 32], fee_version: 0, min_batching_rate: 1 }).unwrap();
        q.enqueue(CommitmentHashRequest { commitment: [0; 32], fee_version: 1, min_batching_rate: 1 }).unwrap();
        assert_matches!(q.next_batch(), Err(_));
    }
}