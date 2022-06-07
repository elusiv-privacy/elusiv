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

pub const MAX_CUS: u32 = MAX_COMPUTE_UNIT_LIMIT - COMPUTE_UNIT_PADDING;

#[cfg(feature = "compute-unit-optimization")]
pub struct PartialComputationResult {
    pub instructions: Vec<PartialComputationInstruction>,
    pub total_rounds: u32,
    pub total_compute_units: u32,
}

#[cfg(feature = "compute-unit-optimization")]
/// Generates instructions (batching of multiple computation rounds) to fit a partial computation in the MAX_COMPUTE_UNIT_LIMIT
pub fn compute_unit_optimization(round_costs: Vec<u32>) -> PartialComputationResult {
    let mut instructions = Vec::new();

    let mut rounds = 0;
    let mut start_round = 0;
    let mut compute_units = 0;
    let mut total_compute_units = 0;

    for r in round_costs {
        if compute_units + r > MAX_CUS {
            instructions.push(
                PartialComputationInstruction {
                    start_round,
                    rounds,
                    compute_units: compute_units + COMPUTE_UNIT_PADDING
                }
            );

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
        instructions.push(
            PartialComputationInstruction {
                start_round,
                rounds,
                compute_units: compute_units + COMPUTE_UNIT_PADDING
            }
        );
    }

    PartialComputationResult {
        instructions,
        total_compute_units,
        total_rounds: start_round + rounds
    }
}