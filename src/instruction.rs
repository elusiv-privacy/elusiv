use super::types::*;
use super::processor::*;

#[derive(crate::macros::ElusivInstruction)]
pub enum ElusivInstruction {
    // Client sends base commitment and amount to be stored in the Elusiv program
    #[sig(sender)]
    #[prg(storage_account, Storage)]
    #[prg(queue, BaseCommitmentQueue)]
    #[pdi(pool, Pool)]
    #[arr(tree, ActiveTree, pda_offset = storage_account.get_active_tree())]
    #[sys(system_program, key = solana_program::system_program::id())]
    Store {
        base_commitment: U256,  // h(nullifier, timestamp)
        amount: u64,
        timestamp: u64,
        commitment: U256,
    },

    // Binary merge proof
    Merge {
        proof_data: JoinSplitProofData<2>,
    },
    
    // Unary migrate proof
    Migrate {
        proof_data: JoinSplitProofData<1>,
        current_nsmt_root: U256,
        next_nsmt_root: U256,
    },

    // Binary send proof
    Send {
        proof_data: JoinSplitProofData<2>,
        amount: u64,
        recipient: U256,
    },

    // Funds are transferred to the recipient
    FinalizeSend,

    InitProof,
    ComputeProof,
    FinalizeProof,

    InitCommitment,
    ComputeCommitment,
    FinalizeCommitment,

    // Resets the main MT and creates a new active TreeAccount PDA
    ActivateTree,
    ArchiveTree,
}