mod instruction;
mod error;
mod processor;
pub mod merkle;
pub mod verifier;
pub mod state;
pub mod entrypoint;

mod poseidon;
mod poseidon_constants;
mod scalar;

solana_program::declare_id!("HXhc8wZvczWycdJ9sGXUE3PDS2wrjdonxVAmvzfNpMou");