use super::super::storage_account::bytes_to_u32;

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

impl<'a, F: Copy> LazyHeapStack<'a, F> {
    pub fn new<S, D>(
        data: &'a mut [u8],
        size: usize,
        bytecount: usize,
        serialize: S,
        deserialize: D,
    ) -> LazyHeapStack<'a, F>
    where
        S:  Fn(F, &mut [u8]) + 'a,
        D:  Fn(&[u8]) -> F + 'a,
    {

        if bytecount * size + 4 != data.len() { println!("WRONG SIZE {}", data.len()); panic!() }

        let stack_pointer = bytes_to_u32(&data[..4]) as usize;
        LazyHeapStack {
            data,
            bytecount,
            stack: vec![None; size],
            state: vec![ValueState::None; size],
            stack_pointer,
            serialize: Box::new(serialize),
            deserialize: Box::new(deserialize),
        }
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
        for i in 0..self.stack_pointer {
            if let ValueState::Modified = self.state[i] {
                let slice = &mut self.data[4 + i * self.bytecount..4 + (i + 1) * self.bytecount]; 
                (self.serialize)(self.stack[i].unwrap(), slice);
            }
        }
    }
}