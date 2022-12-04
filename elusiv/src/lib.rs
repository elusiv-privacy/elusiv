#![allow(clippy::derive_partial_eq_without_eq)]

pub mod bytes;
pub mod commitment;
mod error;
pub mod entrypoint;
pub mod fields;
pub mod instruction;
mod macros;
pub mod map;
pub mod processor;
pub mod proof;
pub mod state;
pub mod token;
pub mod types;

pub use entrypoint::*;
pub use elusiv_computation;

#[macro_use]
extern crate static_assertions;