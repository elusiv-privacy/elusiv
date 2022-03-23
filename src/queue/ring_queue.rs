use super::super::error::ElusivError;
use super::super::bytes::bytes_to_u64;
use solana_program::program_error::ProgramError;

/// Ring queue with SIZE - 1 elements that can be stored at a given time
/// - storage layout:
///     - 8 bytes head
///     - 8 bytes tail
///     - queue
pub struct RingQueue<'a, N: Copy> {
    size: usize,
    bytes: usize,
    data: &'a mut [u8],
    serialize: Box<dyn Fn(&[u8]) -> N + 'a>,
    deserialize: Box<dyn Fn(N) -> Vec<u8> + 'a>,
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
        S: Fn(&[u8]) -> N + 'a,
        D: Fn(N) -> Vec<u8> + 'a,
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

    fn get_head(&self) -> usize {
        bytes_to_u64(&self.data[..8]) as usize
    }

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
        (self.serialize)(bytes)
    }

    fn set(&mut self, i: usize, value: N) {
        let offset = 16 + (i * self.bytes);
        let bytes = (self.deserialize)(value);
        for (i, &byte) in bytes.iter().enumerate() {
            self.data[offset + i] = byte;
        }
    }
}