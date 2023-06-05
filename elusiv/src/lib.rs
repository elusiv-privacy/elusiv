#![allow(clippy::derive_partial_eq_without_eq)]

pub mod buffer;
pub mod bytes;
pub mod commitment;
pub mod entrypoint;
mod error;
pub mod fields;
pub mod instruction;
mod macros;
pub mod map;
pub mod processor;
pub mod proof;
pub mod state;
pub mod token;
pub mod types;

pub use elusiv_computation;
pub use entrypoint::*;

#[macro_use]
#[cfg(test)]
extern crate static_assertions;

#[cfg(all(feature = "devnet", feature = "mainnet"))]
compile_error!(
    "The 'devnet' and 'mainnet' features are mutually exclusive and cannot be used together."
);
