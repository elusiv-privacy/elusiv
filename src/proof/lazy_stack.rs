use super::super::error::ElusivError;
use super::super::bytes::bytes_to_u32;
use solana_program::program_error::ProgramError;

#[derive(Copy, Clone)]
enum ValueState {
    None,
    Used,
    Modified,
}

/// Stack that serializes values only if needed and stores them on the heap
/// - storage layout:
///     - 4 bytes stack pointer
///     - stack values
pub struct LazyHeapStack<'a, F: Copy> {
    data: &'a mut [u8],
    bytecount: usize,
    stack: Vec<Option<F>>,
    state: Vec<ValueState>,
    pub stack_pointer: usize,
    serialize: Box<dyn Fn(F, &mut [u8]) + 'a>,
    deserialize: Box<dyn Fn(&[u8]) -> F + 'a>,
}

pub const fn stack_size(size: usize, bytecount: usize) -> usize {
    size * bytecount + 4
}

impl<'a, F: Copy> LazyHeapStack<'a, F> {
    pub fn new<S, D>(
        data: &'a mut [u8],
        size: usize,
        bytecount: usize,
        serialize: S,
        deserialize: D,
    ) -> Result<LazyHeapStack<'a, F>, ProgramError>
    where
        S:  Fn(F, &mut [u8]) + 'a,
        D:  Fn(&[u8]) -> F + 'a,
    {
        if stack_size(size, bytecount) != data.len() {
            return Err(ElusivError::InvalidAccountSize.into());
        }

        let stack_pointer = bytes_to_u32(&data[..4]) as usize;
        Ok(
            LazyHeapStack {
                data,
                bytecount,
                stack: vec![None; size],
                state: vec![ValueState::None; size],
                stack_pointer,
                serialize: Box::new(serialize),
                deserialize: Box::new(deserialize),
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
                let slice = &self.data[4 + index * self.bytecount..4 + (index + 1) * self.bytecount];
                let v = (self.deserialize)(slice);
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
                let slice = &mut slice[i * self.bytecount..(i + 1) * self.bytecount]; 
                (self.serialize)(self.stack[i].unwrap(), slice);
            }
        }
    }
}