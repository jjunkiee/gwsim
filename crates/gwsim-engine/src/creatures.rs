//! Resurrection, and the hooks for creatures and items skills create.
//!
//! Resurrection is complete here. Summoning minions, creating spirits and
//! holding bundles arrive with the M1 mechanics (WP4.3); until then these
//! hooks log that they were asked for, so an encoding that uses one is
//! visible in a combat log rather than silently doing nothing.

use crate::exec::ExecCtx;
use crate::log::{LogEvent, LogKind};
use crate::sim::Sim;
use crate::unit::{Action, ENERGY_SCALE, HEALTH_SCALE, UnitId};

impl Sim {
    /// Raises a dead unit with a share of its health and energy (§10.11).
    pub fn resurrect(&mut self, unit: UnitId, health_percent: f64, energy_percent: f64) {
        if self.units[unit.index()].alive() {
            return;
        }
        let max_health = self.max_health(unit);
        let max_energy = self.max_energy(unit);
        let now = self.now;
        let u = &mut self.units[unit.index()];
        u.action = Action::Idle;
        u.action_generation = u.action_generation.wrapping_add(1);
        u.health =
            ((f64::from(max_health) * health_percent / 100.0).round() as i32).max(1) * HEALTH_SCALE;
        u.energy = (f64::from(max_energy) * energy_percent / 100.0).round() as i32 * ENERGY_SCALE;
        u.dead_at = None;
        u.corpse_available = false;
        u.next_decision_at = now;
        self.log_event(
            LogEvent::new(now, LogKind::Heal)
                .target(unit)
                .detail("resurrected"),
        );
    }

    /// Summons a creature. Implemented by WP4.3.
    pub fn summon(&mut self, caster: UnitId, creature: &str, level: u8, _ctx: &ExecCtx) {
        self.unimplemented_hook(caster, &format!("summon {creature} at level {level}"));
    }

    /// Creates a spirit. Implemented by WP4.3.
    pub fn create_spirit(
        &mut self,
        caster: UnitId,
        spirit: &str,
        level: u8,
        seconds: f64,
        _ctx: &ExecCtx,
    ) {
        self.unimplemented_hook(
            caster,
            &format!("create spirit {spirit} at level {level} for {seconds}s"),
        );
    }

    /// Picks up a bundle. Implemented by WP4.3.
    pub fn hold_bundle(&mut self, caster: UnitId, bundle: &str, seconds: f64, _ctx: &ExecCtx) {
        self.unimplemented_hook(caster, &format!("hold {bundle} for {seconds}s"));
    }

    /// Drops the held bundle. Implemented by WP4.3.
    pub fn drop_bundle(&mut self, caster: UnitId) {
        self.unimplemented_hook(caster, "drop bundle");
    }

    /// A skill's energy cost after effects that change it (Soul Twisting),
    /// given the cost after inherent attributes. WP4.1 adds them.
    pub fn adjust_energy_cost(&self, _unit: UnitId, _skill: u16, cost: f64) -> f64 {
        cost
    }

    /// Whether an effect prevents interrupting this unit. WP4.3 adds them.
    pub fn prevents_interrupt(&self, _unit: UnitId) -> bool {
        false
    }

    fn unimplemented_hook(&mut self, caster: UnitId, what: &str) {
        self.log_event(
            LogEvent::new(self.now, LogKind::Warning)
                .source(caster)
                .detail(&format!("not yet simulated: {what}")),
        );
    }
}
