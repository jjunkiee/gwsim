//! Rust handlers for the skills the DSL cannot express (T3.6.8, §8.5).
//!
//! A handler is registered by name and referenced from a skill file as
//! `handler: Some((name: "arcane_echo"))`. It can run when the skill is used
//! ([`SkillHandler::on_use`]), react to events on the effects it created
//! ([`SkillHandler::on_event`]), and tidy up when one ends. Every method has
//! a no-op default, so a handler implements only what it needs.
//!
//! The registry also implements the data crate's
//! [`gwsim_data::dsl::HandlerRegistry`], so `gwsim data validate` and
//! `describe` see exactly the handlers the engine has.

use std::collections::BTreeMap;

use gwsim_data::dsl::{HandlerDescribe, Value};

use crate::effects::{ActiveEffect, EndReason};
use crate::exec::ExecCtx;
use crate::sim::{Fired, Sim};
use crate::unit::UnitId;

mod air_of_superiority;
mod arcane_echo;
pub mod m1;

pub use air_of_superiority::AirOfSuperiority;
pub use arcane_echo::ArcaneEcho;

/// A one-of-a-kind skill implemented in Rust.
pub trait SkillHandler: HandlerDescribe + Send + Sync {
    /// The name skill files refer to it by.
    fn name(&self) -> &'static str;

    /// Runs when the skill completes, after its DSL effects (if any).
    fn on_use(&self, _sim: &mut Sim, _ctx: &mut ExecCtx) {}

    /// Runs when an event reaches an effect this handler's skill created.
    fn on_event(&self, _sim: &mut Sim, _fired: &Fired, _bearer: UnitId, _effect: u32) {}

    /// Runs when an effect this handler's skill created ends.
    fn on_effect_end(
        &self,
        _sim: &mut Sim,
        _bearer: UnitId,
        _effect: &ActiveEffect,
        _reason: EndReason,
    ) {
    }

    /// The energy cost of a skill the bearer is about to use, while an
    /// effect this handler created is on it (Soul Twisting).
    fn adjust_cost(
        &self,
        _sim: &Sim,
        _bearer: UnitId,
        _effect: &ActiveEffect,
        _skill: u16,
        cost: f64,
    ) -> f64 {
        cost
    }

    /// The recharge of a skill the bearer has just used, while an effect this
    /// handler created is on it (Soul Twisting).
    fn adjust_recharge(
        &self,
        _sim: &Sim,
        _bearer: UnitId,
        _effect: &ActiveEffect,
        _skill: u16,
        recharge_ms: u32,
    ) -> u32 {
        recharge_ms
    }

    /// A rank this handler's effect sets outright, replacing the bearer's
    /// own (Master of Magic).
    fn set_rank(
        &self,
        _sim: &Sim,
        _effect: &ActiveEffect,
        _attribute: gwsim_data::core::Attribute,
    ) -> Option<u8> {
        None
    }
}

/// Every handler the engine has, by name.
pub struct HandlerRegistry {
    handlers: Vec<Box<dyn SkillHandler>>,
    by_name: BTreeMap<&'static str, usize>,
}

impl std::fmt::Debug for HandlerRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.by_name.keys()).finish()
    }
}

impl HandlerRegistry {
    /// An empty registry.
    pub fn empty() -> Self {
        HandlerRegistry {
            handlers: Vec::new(),
            by_name: BTreeMap::new(),
        }
    }

    /// The registry with every built-in handler.
    pub fn standard() -> Self {
        let mut registry = HandlerRegistry::empty();
        registry.register(Box::new(ArcaneEcho));
        registry.register(Box::new(AirOfSuperiority));
        m1::register(&mut registry);
        registry
    }

    /// Adds a handler. A second handler with the same name replaces the
    /// first, which lets tests register a stand-in.
    pub fn register(&mut self, handler: Box<dyn SkillHandler>) {
        let name = handler.name();
        match self.by_name.get(name) {
            Some(index) => self.handlers[*index] = handler,
            None => {
                self.by_name.insert(name, self.handlers.len());
                self.handlers.push(handler);
            }
        }
    }

    /// A handler's index, by name.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.by_name.get(name).copied()
    }

    /// A handler by index.
    pub fn get(&self, index: usize) -> &dyn SkillHandler {
        self.handlers[index].as_ref()
    }

    /// Every registered name, sorted.
    pub fn names(&self) -> Vec<&'static str> {
        self.by_name.keys().copied().collect()
    }
}

impl gwsim_data::dsl::HandlerRegistry for HandlerRegistry {
    fn get(&self, name: &str) -> Option<&dyn HandlerDescribe> {
        let index = self.index_of(name)?;
        let handler: &dyn SkillHandler = self.handlers[index].as_ref();
        Some(handler as &dyn HandlerDescribe)
    }
}

/// A parameter's number, for handlers reading `params`.
pub fn param_number(params: &BTreeMap<String, Value>, name: &str) -> Option<f64> {
    match params.get(name)? {
        Value::Fixed(value) => Some(f64::from(*value)),
        Value::Percent(value) => Some(f64::from(*value)),
        _ => None,
    }
}
