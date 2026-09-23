//! The deterministic combat simulation and its AI controllers.
//!
//! This crate runs fights: the event queue, the skill pipeline, damage
//! and healing, effects, and the AI controllers that decide what each
//! unit does.
//!
//! **This crate performs no I/O** (ENG-1). It neither reads files nor
//! writes them, so a simulation is reproducible from its inputs alone.
//! Loading is the caller's job, via `gwsim-data`.
//!
//! The pieces, in the order a fight uses them:
//!
//! - [`setup`] prepares a party in a situation once: skills, units, plans;
//! - [`sim`] runs one fight: the clock, events, ticks and the event bus;
//! - [`pipeline`] uses skills; [`attack`] swings weapons; [`movement`] walks;
//! - [`exec`] interprets the effect DSL; [`handlers`] run the skills it
//!   cannot express; [`effects`] keeps effects and conditions;
//! - [`damage`], [`regen`] and [`combat`] apply the wiki's formulas;
//! - [`ai`] decides; [`log`] records; [`harness`] runs many seeds.
//!
//! See `docs/DESIGN.md` sections 10 and 11.

pub mod ai;
pub mod attack;
pub mod combat;
pub mod creatures;
pub mod damage;
pub mod effects;
pub mod exec;
pub mod geom;
pub mod handlers;
pub mod harness;
pub mod inherent;
pub mod log;
pub mod movement;
pub mod pipeline;
pub mod regen;
pub mod result;
pub mod rng;
pub mod setup;
pub mod sim;
pub mod stats;
pub mod time;
pub mod unit;

pub use harness::{EvalOptions, Evaluation, evaluate, evaluate_until_stable};
pub use result::{Outcome, RunResult};
pub use rng::{RunSeed, SeedList};
pub use setup::{FightSetup, SetupError};
pub use sim::Sim;
