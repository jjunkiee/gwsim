//! The build optimiser.
//!
//! Genome and constraints, repair, NSGA-II with constrained domination,
//! adaptive evaluation and successive halving, and the exhaustive mode.
//!
//! The crate does no I/O and reads no clock (ENG-1): the caller decides when
//! a search should stop, through [`search::Controls`], and receives progress
//! through callbacks.
//!
//! See `docs/DESIGN.md` section 13.

pub mod attributes;
pub mod evaluate;
pub mod exhaustive;
pub mod genome;
pub mod nsga;
pub mod objectives;
pub mod operators;
pub mod pools;
pub mod repair;
pub mod rng;
pub mod roles;
pub mod runes;
pub mod sampler;
pub mod search;
