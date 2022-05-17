use super::super::error::ElusivError::InvalidAccount;
use crate::bytes::{ SerDe, SerDeManager };
use crate::macros::guard;
use solana_program::program_error::ProgramError;

/// Stores data lazily on the heap, read requests will trigger serialization
pub struct LazyRAM<'a, N: Clone + SerDe<T=N>, const SIZE: usize> {
    /// Stores all serialized values
    /// - if an element has value None, it has not been initialized yet
    data: Vec<Option<N>>,
    source: &'a mut [u8],
    changes: Vec<bool>,

    /// Base-pointer for function-calls
    frame: usize,
}

impl<'a, N: Clone + SerDe<T=N>, const SIZE: usize> SerDeManager<Self> for LazyRAM<'a, N, SIZE> {
    const SIZE_BYTES: usize = SIZE * N::SIZE;
    fn mut_backing_store(data: &'a mut [u8]) -> Result<Self, ProgramError> {
        guard!(data.len() == SIZE, InvalidAccount);
        Ok(Self::new(data))
    }
}

impl<'a, N: Clone + SerDe<T=N>, const SIZE: usize> LazyRAM<'a, N, SIZE> {
    pub fn new(source: &'a mut [u8]) -> Self {
        let mut data = vec![];
        for _ in 0..SIZE { data.push(None); }
        let changes = vec![false, SIZE];

        LazyRAM { data, frame: 0, source, changes }
    }

    pub fn write(&mut self, value: N, index: usize) {
        self.data[self.frame + index] = Some(value);
    }

    pub fn read(&mut self, index: usize) -> N {
        let i = self.frame + index;
        match self.data[i] {
            Some(v) => v,
            None => {
                let data = &self.source[i * N::SIZE..(i + 1) * N::SIZE];
                let v = N::deserialize(data);
                self.data[i] = Some(v);
                v
            }
        }
    }

    pub fn free(&mut self, index: usize) {
        // we don't need to give free any functionality, since it's the caller responsibility, to only read correct values
    }

    /// Call this before calling a function
    /// - we don't do any checked arithmethic here since we in any case require the calls and parameters to be correct (data is never dependent on user input)
    pub fn inc_frame(&mut self, frame: usize) {
        self.frame += frame;
    }

    /// Call this when returning a function
    pub fn dec_frame(&mut self, frame: usize) {
        self.frame -= frame;
    }

    /// Saves all changes
    pub fn serialize(&mut self) {
        for (i, change) in self.changes.iter().enumerate() {
            if change {
                if let Some(value) = self.data[i] {
                    let data = &mut self.source[i * N::SIZE..(i + 1) * N::SIZE];
                    N::write(value, data);
                }
            }
        }
        self.changes = vec![false, SIZE];
    }
}