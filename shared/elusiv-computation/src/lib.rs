/// Representation of a partial computation
pub trait PartialComputation<const INSTRUCTION_COUNT: usize> {
    const IX_COUNT: usize = INSTRUCTION_COUNT;
    const TX_COUNT: usize;

    /// Rounds performed in a specific instruction
    const INSTRUCTION_ROUNDS: [u8; INSTRUCTION_COUNT];

    /// Rounds performed across all instructions
    const TOTAL_ROUNDS: u32;

    /// All required compute units
    const TOTAL_COMPUTE_UNITS: u32;
    const COMPUTE_BUDGET_PER_IX: u32;
}

/// Interface required by a `elusiv_computations` partial computation
/// - functional requirements of implementations:
///     - `write(a, 0)`, `write(b, 1)` => `read(0) == a && read(1) == b`
///     - `write(c, 1`), `set_frame(1)` => `read(0) == c`
pub trait RAM<N> {
    fn write(&mut self, value: N, index: usize);
    fn read(&mut self, index: usize) -> N;

    fn set_frame(&mut self, frame: usize);
    fn get_frame(&mut self) -> usize;

    /// Called before moving to a new function-frame (nested/recursive partial computations)
    /// - we don't do any checked arithmetic here since we in any case require the calls and parameters to be correct
    fn inc_frame(&mut self, frame: usize) {
        let f = self.get_frame();
        self.set_frame(f + frame);
    }

    /// Call this when returning a function
    fn dec_frame(&mut self, frame: usize) {
        let f = self.get_frame();
        self.set_frame(f - frame);
    }
}

/// https://github.com/solana-labs/solana/blob/master/program-runtime/src/compute_budget.rs#L14
pub const MAX_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

/// Security padding to never exceed the computation budget
pub const COMPUTE_UNIT_PADDING: u32 = 10_000;

#[cfg(feature = "compute-unit-optimization")]
pub struct PartialComputationResult {
    pub instructions: Vec<u32>,
    pub total_rounds: u32,
    pub total_compute_units: u32,
}

#[cfg(feature = "compute-unit-optimization")]
/// Generates instructions (batching of multiple computation rounds) to fit a partial computation in the MAX_COMPUTE_UNIT_LIMIT
pub fn compute_unit_optimization(round_costs: Vec<u32>, max_cus: u32) -> PartialComputationResult {
    let max_cus = max_cus - COMPUTE_UNIT_PADDING;
    let mut instructions = Vec::new();

    let mut rounds = 0;
    let mut start_round = 0;
    let mut compute_units = 0;
    let mut total_compute_units = 0;

    for r in round_costs {
        if compute_units + r > max_cus {
            instructions.push(rounds);

            start_round += rounds;
            rounds = 1;
            compute_units = r;
        } else {
            rounds += 1;
            compute_units += r;
        }

        total_compute_units += r;
    }

    if rounds > 0 {
        instructions.push(rounds);
    }

    let total_rounds = start_round + rounds;
    assert!(total_rounds <= u16::MAX as u32); // assert this since `VerificationAccount` saves rounds as u16

    PartialComputationResult {
        instructions,
        total_compute_units,
        total_rounds,
    }
}

pub fn compute_unit_instructions(round_costs: Vec<u32>, max_cus: u32) -> Vec<u32> {
    let max_cus = max_cus - COMPUTE_UNIT_PADDING;
    let mut instructions = Vec::new();

    let mut rounds = 0;
    let mut compute_units = 0;

    for r in round_costs {
        if compute_units + r > max_cus {
            instructions.push(rounds);

            rounds = 1;
            compute_units = r;
        } else {
            rounds += 1;
            compute_units += r;
        }
    }

    if rounds > 0 {
        instructions.push(rounds);
    }

    instructions
}
