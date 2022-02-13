pub struct LazyHeapStack<F: Copy> {
    pub stack: Vec<F>,
    pub stack_pointer: usize,
}

impl<F: Copy> LazyHeapStack<F> {
    pub fn push(&mut self, v: F) {
        self.stack[self.stack_pointer] = v;
        self.stack_pointer += 1;
    }

    pub fn peek(&self, offset: usize) -> F {
        self.stack[self.stack_pointer - offset - 1]
    }

    pub fn pop(&mut self) -> F {
        self.stack_pointer -= 1;
        self.stack[self.stack_pointer]
    }

    pub fn push_empty(&mut self) {
        self.stack_pointer += 1;
    }

    pub fn pop_empty(&mut self) {
        self.stack_pointer -= 1;
    }

    pub fn swap(&mut self, offset_a: usize, offset_b: usize) {
        self.stack.swap(
            self.stack_pointer - offset_a - 1,
            self.stack_pointer - offset_b - 1,
        );
    }

    pub fn replace(&mut self, offset: usize, v: F) {
        self.stack[self.stack_pointer - offset - 1] = v;
    }
}