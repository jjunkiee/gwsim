//! Arcane Echo (T3.10.4).
//!
//! The only M0 skill that changes the skill bar itself, which is why it is a
//! handler (T1.5.1). From the wiki's page and notes (T3.10.1):
//!
//! - using it places an enchantment on the user for `window` seconds (20);
//! - if the user completes a spell in that time, Arcane Echo's slot becomes
//!   that spell, ready to use, for `copy` seconds (20). The copy happens
//!   before the triggering spell's own effects;
//! - if the user uses a non-spell skill first, the enchantment ends with no
//!   effect;
//! - when the copy expires, the slot reverts and Arcane Echo is disabled for
//!   its own recharge (20 s) — the page's "disabled for 20 seconds";
//! - it cannot copy itself.

use std::collections::BTreeMap;

use gwsim_data::dsl::{EffectKind, Event, HandlerDescribe, StackingBehaviour, Value};

use super::{SkillHandler, param_number};
use crate::effects::{ApplyRequest, EffectSource, EndReason, StackKey};
use crate::exec::ExecCtx;
use crate::sim::{Fired, Sim};
use crate::time::EventKind;
use crate::unit::UnitId;

/// The handler.
pub struct ArcaneEcho;

const DEFAULT_WINDOW_S: f64 = 20.0;
const DEFAULT_COPY_S: f64 = 20.0;

impl HandlerDescribe for ArcaneEcho {
    fn describe(&self, params: &BTreeMap<String, Value>) -> String {
        let window = param_number(params, "window").unwrap_or(DEFAULT_WINDOW_S);
        let copy = param_number(params, "copy").unwrap_or(DEFAULT_COPY_S);
        format!(
            "For {window} seconds, the next spell you cast replaces this skill for {copy} \
             seconds; using a non-spell skill first ends it. It cannot copy itself."
        )
    }
}

impl SkillHandler for ArcaneEcho {
    fn name(&self) -> &'static str {
        "arcane_echo"
    }

    fn on_use(&self, sim: &mut Sim, ctx: &mut ExecCtx) {
        let Some(skill) = ctx.skill else { return };
        let params = sim.handler_params(skill);
        let window = param_number(&params, "window").unwrap_or(DEFAULT_WINDOW_S);
        sim.apply_effect(ApplyRequest {
            target: ctx.caster,
            source: EffectSource::Handler { skill },
            kind: EffectKind::Enchantment,
            caster: ctx.caster,
            rank: ctx.rank,
            duration_ms: Some((window * 1000.0) as u32),
            upkeep: 0,
            trigger_count: 0,
            trigger_charges: Vec::new(),
            key: StackKey::Handler(skill),
            rule: StackingBehaviour::Replace,
            slot: ctx.slot,
        });
    }

    fn on_event(&self, sim: &mut Sim, fired: &Fired, bearer: UnitId, effect: u32) {
        let Some(used) = fired.skill else { return };
        let Some(active) = sim.units[bearer.index()]
            .effects
            .iter()
            .find(|e| e.id == effect)
        else {
            return;
        };
        let EffectSource::Handler { skill: echo } = active.source else {
            return;
        };
        let Some(slot) = active.slot else { return };
        let is_spell = sim.fight.skills[usize::from(used)].is_spell;

        match fired.event {
            // Starting a non-spell ends the echo with no effect.
            Event::OnSkillActivationStart if !is_spell && used != echo => {
                sim.end_effect(bearer, effect, EndReason::Replaced);
            }
            // Completing a spell copies it, before its own effects resolve.
            Event::OnSkillActivationEnd if is_spell && used != echo => {
                let params = sim.handler_params(echo);
                let copy = param_number(&params, "copy").unwrap_or(DEFAULT_COPY_S);
                let now = sim.now;
                let revert_at = now.plus((copy * 1000.0) as u32);
                let unit = &mut sim.units[bearer.index()];
                if let Some(state) = unit.bar[usize::from(slot)].as_mut() {
                    state.skill = used;
                    state.ready_at = now;
                    state.revert_generation = state.revert_generation.wrapping_add(1);
                    let generation = state.revert_generation;
                    sim.queue.schedule(
                        revert_at,
                        EventKind::SlotRevert {
                            unit: bearer,
                            slot,
                            generation,
                        },
                    );
                }
                sim.end_effect(bearer, effect, EndReason::Replaced);
            }
            _ => {}
        }
    }
}
