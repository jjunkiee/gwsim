//! Air of Superiority (T3.10.4).
//!
//! A PvE-only Asura skill: for a title-scaled duration, every time the user
//! earns experience from a kill it gains one random Asura benefit. The
//! outcomes and their chances are documented on the wiki page (T3.10.1):
//!
//! | Benefit | Chance |
//! | --- | --- |
//! | all conditions removed | 30% |
//! | double experience (no effect in a simulation) | 20% |
//! | healed for 50 | 20% |
//! | gain 5 energy | 20% |
//! | recharge all skills | 10% |
//!
//! Who "earns experience from a kill" is A-036: any party member alive and
//! within party range of an experience-giving kill. The draw comes from the
//! user's skill-chance stream.

use std::collections::BTreeMap;

use gwsim_data::core::TitleTrack;
use gwsim_data::dsl::{EffectKind, Event, HandlerDescribe, StackingBehaviour, Value};

use super::SkillHandler;
use crate::effects::{ApplyRequest, EffectSource, StackKey};
use crate::exec::ExecCtx;
use crate::log::{LogEvent, LogKind};
use crate::rng::Purpose;
use crate::sim::{Fired, Sim};
use crate::unit::{ENERGY_SCALE, UnitId};

/// The handler.
pub struct AirOfSuperiority;

/// The five outcomes' weights, in the order the module comment lists them.
pub const OUTCOME_WEIGHTS: [f64; 5] = [0.30, 0.20, 0.20, 0.20, 0.10];

/// The wiki's figures for the heal and energy benefits.
const HEAL: f64 = 50.0;
const ENERGY: i32 = 5;

/// The default duration, when the skill file gives no `duration` param.
fn default_duration() -> Value {
    Value::TitleScaled(TitleTrack::Asura, 20, 30)
}

impl HandlerDescribe for AirOfSuperiority {
    fn describe(&self, _params: &BTreeMap<String, Value>) -> String {
        "For 20...30 seconds (Asura rank), each time you earn experience from a kill you \
         gain one random benefit: all conditions removed (30%), double experience (20%), \
         healed for 50 (20%), 5 energy (20%), or all skills recharged (10%)."
            .to_owned()
    }
}

impl SkillHandler for AirOfSuperiority {
    fn name(&self) -> &'static str {
        "air_of_superiority"
    }

    fn on_use(&self, sim: &mut Sim, ctx: &mut ExecCtx) {
        let Some(skill) = ctx.skill else { return };
        let params = sim.handler_params(skill);
        let duration = params
            .get("duration")
            .cloned()
            .unwrap_or_else(default_duration);
        let seconds = sim.eval_value(&duration, ctx);
        sim.apply_effect(ApplyRequest {
            target: ctx.caster,
            source: EffectSource::Handler { skill },
            kind: EffectKind::Title,
            caster: ctx.caster,
            rank: ctx.rank,
            duration_ms: Some((seconds * 1000.0).round() as u32),
            upkeep: 0,
            trigger_count: 0,
            trigger_charges: Vec::new(),
            key: StackKey::Handler(skill),
            rule: StackingBehaviour::Replace,
            slot: ctx.slot,
        });
    }

    fn on_event(&self, sim: &mut Sim, fired: &Fired, bearer: UnitId, _effect: u32) {
        if fired.event != Event::OnExperienceKill {
            return;
        }
        let outcome = sim
            .streams
            .unit(bearer, Purpose::SkillChance)
            .weighted(&OUTCOME_WEIGHTS);
        let detail = match outcome {
            0 => {
                sim.remove_conditions(bearer, usize::MAX, None);
                "Air of Superiority: conditions removed"
            }
            1 => "Air of Superiority: double experience",
            2 => {
                sim.heal(bearer, bearer, HEAL, None, crate::damage::HealKind::Heal);
                "Air of Superiority: healed for 50"
            }
            3 => {
                sim.gain_energy(bearer, ENERGY * ENERGY_SCALE);
                "Air of Superiority: 5 energy"
            }
            _ => {
                let now = sim.now;
                for slot in sim.units[bearer.index()].bar.iter_mut().flatten() {
                    slot.ready_at = now;
                }
                "Air of Superiority: all skills recharged"
            }
        };
        sim.log_event(
            LogEvent::new(sim.now, LogKind::Decision)
                .source(bearer)
                .detail(detail),
        );
    }
}
