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
//! See `docs/DESIGN.md` sections 10 and 11.
