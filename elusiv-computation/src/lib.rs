//! Structs and types for partial computations

/// A single partial-computation program call consists of `rounds` and require `computs_units`
/// - a collection of `PartialComputationInstruction` form a full computation
/// - calling a single `PartialComputationInstruction` with `compute_units` guarantees successfull execution
#[derive(Debug)]
pub struct PartialComputationInstruction {
    pub start_round: u32,
    pub rounds: u32,
    pub compute_units: u32,
}

/// Representation of a partial computation
pub trait PartialComputation<const INSTRUCTION_COUNT: usize> {
    const INSTRUCTIONS: [PartialComputationInstruction; INSTRUCTION_COUNT];
    const TOTAL_ROUNDS: u32;
    const TOTAL_COMPUTE_UNITS: u32;
}

/// https://github.com/solana-labs/solana/blob/a1522d00242c2888a057c3d4238d902f063af9be/program-runtime/src/compute_budget.rs#L14
pub const MAX_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

/// Security padding to never exceed the computation budget
pub const COMPUTE_UNIT_PADDING: u32 = 100_000;