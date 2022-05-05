use super::super::error::ElusivError;
use super::super::bytes::bytes_to_u32;
use solana_program::program_error::ProgramError;
use crate::bytes::{ SerDe, SerDeManager };
use crate::macros::guard;

#[derive(Copy, Clone)]
enum ValueState {
    None,
    Used,
    Modified,
}

/// Stack that serializes values only if needed and stores them on the heap
/// - serialization only happens when `serialize_stack` is called
/// - storage layout: (use `stack_size` to compute the size)
///     - 4 bytes stack pointer
///     - stack values
pub struct LazyHeapStack<'a, F: Copy + SerDe<F>, const SIZE: usize> {
    pub stack_pointer: usize,
    stack: Vec<Option<F>>,
    state: Vec<ValueState>,
    data: &'a mut [u8], // explicit mut backing store
}

pub const fn stack_size(size: usize, bytecount: usize) -> usize {
    size * bytecount + 4
}

impl<'a, F: Copy + SerDe<F>, const SIZE: usize> SerDeManager<LazyHeapStack<'a, F, SIZE>> for LazyHeapStack<'a, F, SIZE> {
    const SIZE_BYTES: usize = 4 + SIZE * F::SIZE;

    fn mut_backing_store(data: &'a mut [u8]) -> Result<LazyHeapStack<'a, F, SIZE>, ProgramError> {
        LazyHeapStack::<'a, F, SIZE>::new(data)
    }
}

impl<'a, F: Copy + SerDe<F>, const SIZE: usize> LazyHeapStack<'a, F, SIZE> {
    pub fn new(data: &'a mut [u8]) -> Result<LazyHeapStack<'a, F, SIZE>, ProgramError> {
        guard!(
            stack_size(SIZE, F::SIZE) == data.len(),
            ElusivError::InvalidAccountSize
        );

        let stack_pointer = bytes_to_u32(&data[..4]) as usize;
        Ok(
            LazyHeapStack {
                data,
                stack: vec![None; SIZE],
                state: vec![ValueState::None; SIZE],
                stack_pointer,
            }
        )
    }

    pub fn push(&mut self, v: F) {
        self.set(self.stack_pointer, v);
        self.stack_pointer += 1;
    }

    pub fn peek(&mut self, offset: usize) -> F {
        self.get(self.stack_pointer - offset - 1)
    }

    pub fn pop(&mut self) -> F {
        self.stack_pointer -= 1;
        self.get(self.stack_pointer)
    }

    pub fn push_empty(&mut self) {
        self.stack_pointer += 1;
    }

    pub fn pop_empty(&mut self) {
        self.stack_pointer -= 1;
    }

    pub fn swap(&mut self, offset_a: usize, offset_b: usize) {
        // TODO: more efficient version which checks if values have been loaded
        let ia = self.stack_pointer - offset_a - 1;
        let ib = self.stack_pointer - offset_b - 1;
        let a = self.get(ia);
        let b = self.get(ib);
        self.set(ia, b);
        self.set(ib, a);
    }

    pub fn replace(&mut self, offset: usize, v: F) {
        self.set(self.stack_pointer - offset - 1, v)
    }

    pub fn clear(&mut self) {
        self.stack_pointer = 0;
        self.serialize_stack();
    }

    fn get(&mut self, index: usize) -> F {
        match self.stack[index] {
            Some(v) => v,
            None => {
                let slice = &self.data[4 + index * F::SIZE..4 + (index + 1) * F::SIZE];
                let v = F::deserialize(slice);
                self.stack[index] = Some(v);
                self.state[index] = ValueState::Used;
                v
            }
        }
    }

    fn set(&mut self, index: usize, v: F) {
        self.stack[index] = Some(v);
        self.state[index] = ValueState::Modified;
    }

    /// Serializes all modified values and the stack pointer
    pub fn serialize_stack(&mut self) {
        // Serialize stack pointer
        let bytes = (self.stack_pointer as u32).to_le_bytes();
        self.data[0] = bytes[0];
        self.data[1] = bytes[1];
        self.data[2] = bytes[2];
        self.data[3] = bytes[3];

        // Serialize stack values
        for i in 0..self.stack_pointer {
            if let ValueState::Modified = self.state[i] {
                let slice = &mut self.data[4..];
                let slice = &mut slice[i * F::SIZE..(i + 1) * F::SIZE]; 
                for (i, &byte) in F::serialize(self.stack[i].unwrap()).iter().enumerate() {
                    slice[i] = byte;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: usize = 12;

    fn get_stack<'a>(data: &'a mut [u8]) -> LazyHeapStack<'a, u32, SIZE> {
        LazyHeapStack::new(data).unwrap()
    }

    #[test]
    fn test_stack() {
        let mut data = vec![0; stack_size(SIZE, u32::SIZE)];
        let mut stack = get_stack(&mut data);

        // Test LIFO
        for i in 0..SIZE {
            stack.push(i as u32);
        }
        for i in (0..SIZE).rev() {
            assert_eq!(stack.pop(), i as u32);
        }
    }
}