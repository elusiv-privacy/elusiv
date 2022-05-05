use crate::error::ElusivError;
use crate::error::ElusivError::{ QueueIsFull, QueueIsEmpty };
use crate::macros::guard;

/// Ring queue with `size - 1` elements that can be stored at a given time
/// - serialization happens after each modification
/// - `data`: [head: u64, tail: u64, queue elements]
pub trait RingQueue<N: PartialEq> {
    fn get_size(&self) -> u64;

    fn get_head(&self) -> u64;
    fn set_head(&mut self, value: u64);

    fn get_tail(&self) -> u64;
    fn set_tail(&mut self, value: u64);

    fn get_data(&self, index: usize) -> N;
    fn set_data(&mut self, index: usize, value: N);

    fn enqueue(&mut self, value: N) -> Result<(), ElusivError> {
        let head = self.get_head();
        let tail = self.get_tail();

        let next_tail = (tail + 1) % self.get_size();
        guard!(next_tail != head, QueueIsFull);

        self.set_data(tail as usize, value);
        self.set_tail(next_tail);

        Ok(())
    }

    fn view_first(&self) -> Result<N, ElusivError> {
        let head = self.get_head();
        let tail = self.get_tail();

        guard!(head != tail, QueueIsEmpty);

        Ok(self.get_data(head as usize))
    }

    fn dequeue_first(&mut self) -> Result<N, ElusivError> {
        let head = self.get_head();
        let tail = self.get_tail();

        guard!(head != tail, QueueIsEmpty);

        let value = self.get_data(head as usize);
        self.set_head((head + 1) % self.get_size());

        Ok(value)
    }

    fn contains(&self, value: N) -> bool {
        let mut ptr = self.get_head();
        let tail = self.get_tail();

        while ptr != tail {
            if self.get_data(ptr as usize) == value { return true; }
            ptr = (ptr + 1) % self.get_size();
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: usize = 3;

    struct TestQueue {
        size: u64,
        head: u64,
        tail: u64,
        data: [u32; SIZE],
    }

    impl RingQueue<u32> for TestQueue {
        fn get_size(&self) -> u64 { self.size }

        fn get_head(&self) -> u64 { self.head }
        fn set_head(&mut self, value: u64) { self.head = value; }

        fn get_tail(&self) -> u64 { self.tail }
        fn set_tail(&mut self, value: u64) { self.tail = value; }

        fn get_data(&self, index: usize) -> u32 { self.data[index] }
        fn set_data(&mut self, index: usize, value: u32) { self.data[index] = value; }
    }

    #[test]
    fn test_queue() {
        let mut queue = TestQueue { size: SIZE as u64, head: 0, tail: 0, data: [0; SIZE] };

        // Test that first element does not change (FIFO)
        for i in 1..SIZE {
            queue.enqueue(i as u32).unwrap();
            assert_eq!(
                queue.view_first().unwrap(),
                1
            );
        }

        // Test max size
        assert!(matches!(queue.enqueue(1), Err(_)));

        // Test the queue ordering
        for i in 1..SIZE {
            assert_eq!(
                queue.view_first().unwrap(),
                i as u32
            );
            queue.dequeue_first().unwrap();
        }

        // Test queue is empty
        assert!(matches!(queue.dequeue_first(), Err(_)));

        // Test multiple fillings
        for i in 1..SIZE * 3 {
            queue.enqueue(i as u32).unwrap();
            assert_eq!(
                queue.view_first().unwrap(),
                i as u32
            );
            queue.dequeue_first().unwrap();
        }
    }
}