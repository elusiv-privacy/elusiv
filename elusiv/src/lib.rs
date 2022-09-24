#![allow(clippy::derive_partial_eq_without_eq)]

mod error;
mod macros;
pub mod types;
pub mod bytes;
pub mod map;
pub mod instruction;
pub mod processor;
pub mod state;
pub mod fields;
pub mod proof;
pub mod token;
pub mod commitment;
pub mod entrypoint;

pub use entrypoint::*;
pub use elusiv_computation;

#[macro_use]
extern crate static_assertions;