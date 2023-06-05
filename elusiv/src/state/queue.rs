#![allow(dead_code)]

use crate::bytes::*;
use crate::error::ElusivError::{InvalidQueueAccess, QueueIsEmpty, QueueIsFull};
use crate::macros::guard;
use elusiv_types::ProgramAccount;
use solana_program::program_error::ProgramError;

/// Generates a [`QueueAccount`] and a [`Queue`] that implements the [`RingQueue`] trait
macro_rules! queue_account {
    ($id: ident, $id_account: ident, $size: expr, $ty_element: ty $(,)?) => {
        #[allow(dead_code)] // required for the pda_data field
        #[crate::macros::elusiv_account]
        pub struct $id_account {
            #[no_getter]
            #[no_setter]
            pda_data: PDAAccountData,

            head: u32,
            tail: u32,
            raw_data: [$ty_element; $size],
        }

        #[cfg(test)]
        const_assert_eq!(
            <$id_account as elusiv_types::SizedAccount>::SIZE,
            <elusiv_types::accounts::PDAAccountData as elusiv_types::bytes::BorshSerDeSized>::SIZE
                + (4 + 4)
                + <$ty_element as elusiv_types::bytes::BorshSerDeSized>::SIZE * ($size)
        );

        #[cfg(test)]
        const_assert_eq!(<$id as crate::state::queue::RingQueue>::SIZE, $size as u32);

        pub struct $id<'a, 'b> {
            account: &'b mut $id_account<'a>,
        }

        impl<'a, 'b> crate::state::queue::Queue<'a, 'b, $id_account<'a>> for $id<'a, 'b> {
            type T = $id<'a, 'b>;
            fn new(account: &'b mut $id_account<'a>) -> Self::T {
                $id { account }
            }
        }

        impl<'a, 'b> crate::state::queue::RingQueue for $id<'a, 'b> {
            type N = $ty_element;
            const CAPACITY: u32 = ($size as u32) - 1;

            fn get_head(&self) -> u32 {
                self.account.get_head()
            }

            fn set_head(&mut self, value: &u32) {
                self.account.set_head(value)
            }

            fn get_tail(&self) -> u32 {
                self.account.get_tail()
            }

            fn set_tail(&mut self, value: &u32) {
                self.account.set_tail(value)
            }

            fn get_data(&self, index: usize) -> Self::N {
                self.account.get_raw_data(index)
            }

            fn set_data(&mut self, index: usize, value: &Self::N) {
                self.account.set_raw_data(index, value)
            }
        }

        impl<'a, 'b> crate::state::queue::QueueAccount for $id<'a, 'b> {
            type T = $id_account<'a>;
        }
    };
}

pub trait QueueAccount {
    type T;
}

pub(crate) use queue_account;

pub trait Queue<'a, 'b, Account: ProgramAccount<'a>> {
    type T;
    fn new(account: &'b mut Account) -> Self::T;
}

/// Ring-queue with a capacity of [`RingQueue::CAPACITY`] elements
///
/// # Notes
///
/// - works by having two pointers, `head` and `tail` and a some data storage with getter, setter
/// - `head` points to the first element (first according to the FIFO definition)
/// - `tail` points to the location to insert the next element
/// - `head == (tail - 1) mod SIZE` => queue is full
/// - `head == tail` => queue is empty
pub trait RingQueue {
    type N: PartialEq + BorshSerDeSized + Clone;
    const CAPACITY: u32;
    const SIZE: u32 = Self::CAPACITY + 1;

    fn get_head(&self) -> u32;
    fn set_head(&mut self, value: &u32);

    fn get_tail(&self) -> u32;
    fn set_tail(&mut self, value: &u32);

    fn get_data(&self, index: usize) -> Self::N;
    fn set_data(&mut self, index: usize, value: &Self::N);

    /// Try to enqueue a new element in the queue
    fn enqueue(&mut self, value: Self::N) -> Result<(), ProgramError> {
        let head = self.get_head();
        let tail = self.get_tail();

        let next_tail = (tail + 1) % Self::SIZE;
        guard!(next_tail != head, QueueIsFull);

        self.set_data(tail as usize, &value);
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
        guard!(usize_as_u32_safe(offset) < self.len(), InvalidQueueAccess);

        Ok(self.get_data((head as usize + offset) % Self::SIZE as usize))
    }

    /// Try to remove the first element from the queue
    fn dequeue_first(&mut self) -> Result<Self::N, ProgramError> {
        let head = self.get_head();
        let tail = self.get_tail();
        guard!(head != tail, QueueIsEmpty);

        let value = self.get_data(head as usize);
        self.set_head(&((head + 1) % Self::SIZE));

        Ok(value)
    }

    fn remove(&mut self, count: u32) -> Result<(), ProgramError> {
        let head = self.get_head();
        guard!(self.len() >= count, InvalidQueueAccess);
        self.set_head(&((head + count) % Self::SIZE));
        Ok(())
    }

    fn contains(&self, value: &Self::N) -> bool {
        let mut ptr = self.get_head();
        let tail = self.get_tail();

        while ptr != tail {
            if self.get_data(ptr as usize) == *value {
                return true;
            }
            ptr = (ptr + 1) % Self::SIZE;
        }

        false
    }

    fn len(&self) -> u32 {
        let head = self.get_head();
        let tail = self.get_tail();

        if tail >= head {
            tail - head
        } else {
            Self::SIZE - head + tail
        }
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn empty_slots(&self) -> u32 {
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
    use super::*;
    use crate::error::ElusivError;

    struct TestQueue<const S: usize> {
        head: u32,
        tail: u32,
        data: [u32; S],
    }

    impl<const S: usize> RingQueue for TestQueue<S> {
        type N = u32;
        const CAPACITY: u32 = S as u32 - 1;

        fn get_head(&self) -> u32 {
            self.head
        }
        fn set_head(&mut self, value: &u32) {
            self.head = *value;
        }

        fn get_tail(&self) -> u32 {
            self.tail
        }
        fn set_tail(&mut self, value: &u32) {
            self.tail = *value;
        }

        fn get_data(&self, index: usize) -> u32 {
            self.data[index]
        }
        fn set_data(&mut self, index: usize, value: &u32) {
            self.data[index] = *value;
        }
    }

    impl<const S: usize> TestQueue<S> {
        pub fn capacity(&self) -> u32 {
            Self::CAPACITY
        }
    }

    macro_rules! test_queue {
        ($id: ident, $size: literal, $head: literal, $tail: literal) => {
            let mut $id = TestQueue {
                head: $head,
                tail: $tail,
                data: [0; $size],
            };
        };
    }

    #[test]
    fn test_persistent_fifo() {
        test_queue!(queue, 7, 0, 0);

        for i in 0..queue.capacity() {
            queue.enqueue(i).unwrap();
            assert_eq!(0, queue.view_first().unwrap()); // first element does not change
            assert_eq!(queue.len(), i + 1);
        }
    }

    #[test]
    fn test_full_cycle() {
        test_queue!(queue, 7, 0, 0);

        for i in 0..queue.capacity() {
            queue.enqueue(i).unwrap();
            assert_eq!(0, queue.view_first().unwrap()); // first element does not change
            assert_eq!(queue.len(), i + 1);
        }

        assert_eq!(queue.enqueue(2), Err(ElusivError::QueueIsFull.into()));

        // Remove and insert one
        for i in 0..queue.capacity() {
            queue.dequeue_first().unwrap();
            queue.enqueue(i).unwrap();
        }
    }

    #[test]
    fn test_max_size() {
        test_queue!(full_queue, 3, 1, 0);
        assert_eq!(full_queue.enqueue(1), Err(ElusivError::QueueIsFull.into()));

        full_queue.dequeue_first().unwrap();
        assert_eq!(full_queue.enqueue(1), Ok(()));
        assert_eq!(full_queue.enqueue(2), Err(ElusivError::QueueIsFull.into()));

        full_queue.dequeue_first().unwrap();
        assert_eq!(full_queue.enqueue(2), Ok(()));
    }

    #[test]
    fn test_len() {
        test_queue!(queue, 10, 0, 0);
        assert_eq!(queue.len(), 0);

        for start in 0..9 {
            queue.set_head(&start);
            queue.set_tail(&start);

            assert_eq!(queue.len(), 0);

            for i in 1..10 {
                queue.enqueue(1).unwrap();
                assert_eq!(queue.len(), i);
            }

            for i in (0..9).rev() {
                queue.dequeue_first().unwrap();
                assert_eq!(queue.len(), i);
            }
        }

        test_queue!(queue, 3, 0, 0);
        queue.set_head(&2);
        queue.set_tail(&2);

        assert_eq!(queue.len(), 0);

        queue.enqueue(1).unwrap();
        assert_eq!(queue.len(), 1);
        queue.dequeue_first().unwrap();

        queue.enqueue(1).unwrap();
        assert_eq!(queue.len(), 1);
        queue.dequeue_first().unwrap();

        queue.enqueue(1).unwrap();
        assert_eq!(queue.len(), 1);
        queue.dequeue_first().unwrap();

        assert_eq!(queue.len(), 0);
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
        assert_eq!(queue.dequeue_first(), Err(ElusivError::QueueIsEmpty.into()));
    }

    #[test]
    fn test_view() {
        test_queue!(queue, 13, 0, 0);

        assert_eq!(queue.view(0), Err(ElusivError::QueueIsEmpty.into()));

        queue.enqueue(0).unwrap();

        queue.view(0).unwrap();
        assert_eq!(queue.view(1), Err(ElusivError::InvalidQueueAccess.into()));
    }

    #[test]
    fn test_view_invalid() {
        test_queue!(queue, 10, 0, 0);
        queue.head = 9;
        queue.tail = 9;
        queue.enqueue(1).unwrap();

        assert_eq!(queue.view(2), Err(ElusivError::InvalidQueueAccess.into()));
    }

    #[test]
    fn test_remove() {
        test_queue!(queue, 13, 0, 0);

        queue.enqueue(0).unwrap();
        queue.enqueue(1).unwrap();
        queue.enqueue(2).unwrap();
        queue.remove(2).unwrap();

        assert_eq!(queue.view_first().unwrap(), 2);
    }

    #[test]
    fn test_remove_invalid() {
        test_queue!(queue, 10, 0, 0);
        assert_eq!(queue.remove(1), Err(ElusivError::InvalidQueueAccess.into()));

        queue.enqueue(1).unwrap();
        assert_eq!(queue.remove(2), Err(ElusivError::InvalidQueueAccess.into()));
        queue.remove(1).unwrap();

        test_queue!(queue, 10, 0, 0);
        queue.head = 9;
        queue.tail = 9;

        queue.enqueue(1).unwrap();

        assert_eq!(queue.remove(2), Err(ElusivError::InvalidQueueAccess.into()));
        queue.remove(1).unwrap();
    }

    #[test]
    fn test_clear_queue() {
        test_queue!(queue, 13, 0, 0);
        queue.enqueue(0).unwrap();
        queue.clear();
        assert!(queue.is_empty());
    }
}
