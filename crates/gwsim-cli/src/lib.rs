//! The gwsim command-line interface.
//!
//! The argument types live in the library rather than the binary so that
//! integration tests, and later the relative checks of section 17.4, can
//! use the same code the `gwsim` binary runs.
//!
//! See `docs/DESIGN.md` sections 14 and 15.

use clap::{Parser, Subcommand};

/// Guild Wars Reforged PvE build simulator.
#[derive(Debug, Parser)]
#[command(
    name = "gwsim",
    version,
    about = "Guild Wars Reforged PvE build simulator"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// The subcommands of `gwsim`.
///
/// Empty for now. `evaluate` arrives with M0 (T3.10.6), `data validate`
/// with T1.2.12, `check` and `compare` with M1, and `optimise` with M2.
#[derive(Debug, Subcommand)]
pub enum Command {}
