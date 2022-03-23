use super::super::error::ElusivError;
use super::super::bytes::bytes_to_u64;
use solana_program::program_error::ProgramError;

/// Ring queue with `size - 1` elements that can be stored at a given time
/// - serialization happens after each modification
/// - storage layout: (use `queue_size` to compute the size)
///     - 8 bytes head
///     - 8 bytes tail
///     - queue
pub struct RingQueue<'a, N: Copy> {
    size: usize,
    bytes: usize,
    data: &'a mut [u8],
    serialize: Box<dyn Fn(N) -> Vec<u8> + 'a>,
    deserialize: Box<dyn Fn(&[u8]) -> N + 'a>,
}

pub const fn queue_size(size: usize, bytecount: usize) -> usize {
    size * bytecount + 2 * 16
}

impl<'a, N: Copy> RingQueue<'a, N> {
    pub fn new<S, D>(
        data: &'a mut [u8],
        size: usize,
        bytes: usize,
        serialize: S,
        deserialize: D,
    ) -> Result<RingQueue<'a, N>, ProgramError>
    where
        S: Fn(N) -> Vec<u8> + 'a,
        D: Fn(&[u8]) -> N + 'a,
    {
        if queue_size(size, bytes) != data.len() {
            return Err(ElusivError::InvalidAccountSize.into());
        }

        Ok(
            RingQueue {
                size,
                bytes,
                data,
                serialize: Box::new(serialize),
                deserialize: Box::new(deserialize),
            }
        )
    }

    /// Head points to the first element
    fn get_head(&self) -> usize {
        bytes_to_u64(&self.data[..8]) as usize
    }

    /// Tail points to the place where the next element is to be inserted
    fn get_tail(&self) -> usize {
        bytes_to_u64(&self.data[8..16]) as usize
    }

    fn set_head(&mut self, head: usize) {
        let bytes = head.to_le_bytes();
        for (i, &byte) in bytes.iter().enumerate() {
            self.data[i] = byte;
        }
    }

    fn set_tail(&mut self, tail: usize) {
        let bytes = tail.to_le_bytes();
        for (i, &byte) in bytes.iter().enumerate() {
            self.data[i + 8] = byte;
        }
    }

    pub fn enqueue(&mut self, value: N) -> Result<(), ElusivError> {
        let head = self.get_head();
        let tail = self.get_tail();

        let next_tail = (tail + 1) % self.size;
        if next_tail == head {
            return Err(ElusivError::QueueIsFull);
        }

        self.set(tail, value);
        self.set_tail(next_tail);

        Ok(())
    }

    pub fn first(&self) -> Result<N, ElusivError> {
        let head = self.get_head();
        let tail = self.get_tail();

        if head == tail {
            return Err(ElusivError::QueueIsEmpty);
        }

        Ok(self.get(head))
    }

    pub fn dequeue(&mut self) -> Result<(), ElusivError> {
        let head = self.get_head();
        let tail = self.get_tail();

        if head == tail {
            return Err(ElusivError::QueueIsEmpty);
        }

        self.set_head((head + 1) % self.size);

        Ok(())
    }

    fn get(&self, i: usize) -> N {
        let offset = 16 + (i * self.bytes);
        let bytes = &self.data[offset..offset + self.bytes];
        (self.deserialize)(bytes)
    }

    fn set(&mut self, i: usize, value: N) {
        let offset = 16 + (i * self.bytes);
        let bytes = (self.serialize)(value);
        for (i, &byte) in bytes.iter().enumerate() {
            self.data[offset + i] = byte;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: usize = 3;
    const BYTES: usize = 4;

    fn get_queue<'a>(data: &'a mut [u8]) -> RingQueue<'a, u32> {
        RingQueue::new(data, SIZE, BYTES,
            |value: u32| value.to_le_bytes().to_vec(),
            |bytes: &[u8]| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        ).unwrap()
    }

    #[test]
    fn test_wrong_size() {
        let size = 3;
        let bytes = 4;
        let mut data = vec![0; size * bytes];
        let queue = RingQueue::new( &mut data[..], size, bytes,
            |value: u32| value.to_le_bytes().to_vec(),
            |bytes: &[u8]| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        );

        assert!(matches!(queue, Err(_)));
    }

    #[test]
    fn test_queue() {
        let mut data = vec![0; queue_size(SIZE, BYTES)];
        let mut queue = get_queue(&mut data);

        // Test that first element does not change (FIFO)
        for i in 1..SIZE {
            queue.enqueue(i as u32).unwrap();
            assert_eq!(
                queue.first().unwrap(),
                1
            );
        }

        // Test max size
        assert!(matches!(queue.enqueue(1), Err(_)));

        // Test the queue ordering
        for i in 1..SIZE {
            assert_eq!(
                queue.first().unwrap(),
                i as u32
            );
            queue.dequeue().unwrap();
        }

        // Test queue is empty
        assert!(matches!(queue.dequeue(), Err(_)));

        // Test multiple fillings
        for i in 1..SIZE * 3 {
            queue.enqueue(i as u32).unwrap();
            assert_eq!(
                queue.first().unwrap(),
                i as u32
            );
            queue.dequeue().unwrap();
        }
    }
}