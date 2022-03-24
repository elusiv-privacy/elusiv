use solana_program::{
    entrypoint::ProgramResult,
};

/// Store first commitment from queue in commitment account
pub fn init_commitment() -> ProgramResult {
    // Check if commitment account is in reset state
    // Check if commitments are new
    // Put commitment in account

    Ok(())
}

/// Compute hashes for commitment
pub fn compute_commitment() -> ProgramResult {
    Ok(())
}

/// Store commitment and hashes in storage account
pub fn finalize_commitment() -> ProgramResult {
    // Add commitments and hashes into storage account
    // Reset commitment account
    // Trye init commitment

    Ok(())
}