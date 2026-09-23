//! One fight: the clock, the loop and the event bus (T3.1.5, T3.6.6).
//!
//! [`Sim::run`] alternates two things until the fight ends:
//!
//! 1. it pops every event due before the next tick, in `(time, class,
//!    sequence)` order, setting the clock to each event's time;
//! 2. it advances the clock to the tick and runs the continuous processes:
//!    movement, regeneration, attacks and AI decisions.
//!
//! The fight ends in a **win** when every hostile unit is dead, a **wipe**
//! when every party member is dead, and a **timeout** (a loss, §12.3) at the
//! situation's limit, A-030 by default.

use std::sync::Arc;

use gwsim_data::AssumptionId;
use gwsim_data::dsl::{Control, Event};

use crate::effects::{ActiveEffect, EffectSource, EndReason};
use crate::exec::ExecCtx;
use crate::log::{LogEvent, LogKind};
use crate::result::{Outcome, RunStats};
use crate::rng::{RunSeed, Streams};
use crate::setup::FightData;
use crate::time::{EventKind, EventQueue, SimTime, TICK_MS};
use crate::unit::{Action, Team, Unit, UnitId};

/// How deep triggers may nest before the chain is cut (T3.6.6).
pub const MAX_TRIGGER_DEPTH: u8 = 8;

/// Something that happened, which triggers and handlers can react to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fired {
    pub event: Event,
    /// Whose effects are asked: the caster of a spell, the unit that was hit,
    /// the unit that died.
    pub subject: UnitId,
    /// The other party: the spell's target, the attacker, the killer.
    pub other: Option<UnitId>,
    /// The skill involved, as a fight skill index.
    pub skill: Option<u16>,
    pub amount: f64,
}

impl Fired {
    /// An event with only a subject.
    pub fn new(event: Event, subject: UnitId) -> Self {
        Fired {
            event,
            subject,
            other: None,
            skill: None,
            amount: 0.0,
        }
    }
}

/// One fight in progress.
#[derive(Debug, Clone)]
pub struct Sim {
    pub fight: Arc<FightData>,
    pub seed: RunSeed,
    pub now: SimTime,
    pub queue: EventQueue,
    pub units: Vec<Unit>,
    pub controllers: Vec<crate::ai::Controller>,
    pub streams: Streams,
    pub next_effect_id: u32,
    pub projectiles: Vec<crate::attack::Projectile>,
    /// The combat log, when one was asked for (WP3.9).
    pub log: Option<Vec<LogEvent>>,
    pub stats: RunStats,
    /// Assumptions this fight relied on, as a bit per id.
    pub assumptions: u64,
    pub outcome: Option<(Outcome, SimTime)>,
    /// When the fight proper began (aggro). Clear time counts from here.
    pub engaged_at: Option<SimTime>,
    /// Dhuum's Covenant was on and a party member died.
    pub covenant_broken: bool,
    /// Which foe groups have noticed the party (AI-F1), by group index.
    pub aggroed: Vec<bool>,
    /// The party's called target (§11.6), which heroes attack first.
    pub called_target: Option<UnitId>,
    /// When a foe last started a skill, so heroes can interrupt at once.
    pub foe_cast_started: Option<SimTime>,
    /// Spirits carrying an aura, so stat queries need not scan every unit.
    pub aura_spirits: Vec<UnitId>,
    /// The next pre-fight cast (T4.7.3), and when it started waiting.
    pub prefight_step: usize,
    pub prefight_since: SimTime,
    /// The next chain fight to spawn (T4.8.4), and the rest before it.
    pub chain_step: usize,
    pub resting_until: Option<SimTime>,
    /// When the current fight began, for its timeout (A-030).
    pub fight_started_at: SimTime,
    /// When the current fight's foes first engaged.
    pub segment_engaged: Option<SimTime>,
    /// Fights finished so far, and party deaths at the start of this one.
    pub segments: Vec<crate::result::Segment>,
    pub deaths_before: u32,
    /// Effects whose definition gives its caster a modifier (Life Siphon's
    /// regeneration), as (bearer, effect id), so stat queries need not scan
    /// every effect in the fight.
    pub caster_effects: Vec<(UnitId, u32)>,
    trigger_depth: u8,
}

impl Sim {
    /// A fight ready to start. Normally built by [`crate::setup::FightSetup`].
    pub fn new(
        fight: Arc<FightData>,
        units: Vec<Unit>,
        controllers: Vec<crate::ai::Controller>,
        seed: RunSeed,
    ) -> Sim {
        let stats = RunStats::new(&units, fight.skills.len());
        let streams = Streams::new(seed, units.len());
        Sim {
            fight,
            seed,
            now: SimTime::ZERO,
            queue: EventQueue::with_capacity(256),
            units,
            controllers,
            streams,
            next_effect_id: 0,
            projectiles: Vec::new(),
            log: None,
            stats,
            assumptions: 0,
            outcome: None,
            engaged_at: None,
            covenant_broken: false,
            aggroed: Vec::new(),
            called_target: None,
            foe_cast_started: None,
            aura_spirits: Vec::new(),
            prefight_step: 0,
            prefight_since: SimTime::ZERO,
            chain_step: 0,
            resting_until: None,
            fight_started_at: SimTime::ZERO,
            segment_engaged: None,
            segments: Vec::new(),
            deaths_before: 0,
            caster_effects: Vec::new(),
            trigger_depth: 0,
        }
    }

    /// Turns on the combat log for this fight.
    pub fn with_log(mut self) -> Sim {
        self.log = Some(Vec::new());
        self
    }

    /// Records that the fight relied on an assumption.
    pub fn touch(&mut self, id: u16) {
        if id < 64 {
            self.assumptions |= 1 << id;
        }
    }

    /// The assumptions touched, as ids.
    pub fn assumptions_touched(&self) -> Vec<AssumptionId> {
        (0..64u16)
            .filter(|id| self.assumptions & (1 << id) != 0)
            .filter_map(AssumptionId::from_number)
            .collect()
    }

    /// Runs the fight to its end.
    pub fn run(mut self) -> crate::result::RunResult {
        self.check_outcome();
        while self.outcome.is_none() {
            let next_tick = SimTime((self.now.ms() / TICK_MS + 1) * TICK_MS);
            self.step_until(next_tick);
        }
        self.finish()
    }

    /// Advances the fight to a time: events first, then any ticks passed.
    pub fn step_until(&mut self, until: SimTime) {
        while self.outcome.is_none() {
            let next_tick = SimTime((self.now.ms() / TICK_MS + 1) * TICK_MS);
            let horizon = next_tick.min(until);
            while let Some(event) = self.queue.pop_due(horizon) {
                self.now = event.at;
                self.dispatch(event.kind);
                if self.outcome.is_some() {
                    return;
                }
            }
            if next_tick > until {
                self.now = until;
                return;
            }
            self.now = next_tick;
            self.tick();
            self.check_outcome();
            if self.now >= until {
                return;
            }
        }
    }

    fn dispatch(&mut self, kind: EventKind) {
        match kind {
            EventKind::ActivationEnd { unit, generation } => {
                if self.units[unit.index()].action_generation == generation {
                    self.finish_activation(unit);
                }
            }
            // The end of an aftercast, or of a knockdown.
            EventKind::AftercastEnd { unit, generation } => {
                let u = &mut self.units[unit.index()];
                if u.action_generation == generation
                    && matches!(
                        u.action,
                        Action::Aftercast { .. } | Action::KnockedDown { .. }
                    )
                {
                    u.action = Action::Idle;
                    u.action_generation = u.action_generation.wrapping_add(1);
                    self.after_action(unit);
                }
            }
            EventKind::EffectExpiry { unit, effect } => self.expire_effect(unit, effect),
            EventKind::ProjectileImpact { projectile } => self.projectile_impact(projectile),
            EventKind::AttackHit { unit, generation } => self.attack_hit(unit, generation),
            EventKind::SlotRevert {
                unit,
                slot,
                generation,
            } => self.revert_slot(unit, slot, generation),
            EventKind::CreatureExpiry { unit } => self.kill(unit, None),
            EventKind::Marker(_) => {}
        }
        self.check_outcome();
    }

    /// The continuous processes, once per tick.
    fn tick(&mut self) {
        if let Some(until) = self.resting_until
            && self.now >= until
        {
            self.spawn_next_fight();
        }
        self.move_units();
        self.regenerate();
        self.drive_attacks();
        self.decide();
        if self.now.ms().is_multiple_of(1000) {
            self.stats.sample_energy(&self.units);
        }
    }

    /// Ends the fight if a stop condition holds.
    pub fn check_outcome(&mut self) {
        if self.outcome.is_some() {
            return;
        }
        if self.resting_until.is_some() {
            // Between fights of a chain; only a wipe could end it, and
            // nothing fights during a rest.
            return;
        }
        let alive = |team: Team| {
            self.units
                .iter()
                .filter(|u| u.team == team && u.alive() && !u.kind.is_summoned())
                .count()
        };
        let foes_alive = alive(Team::Foes);
        let party_alive = alive(Team::Party);
        let has_foes = self
            .units
            .iter()
            .any(|u| u.team == Team::Foes && !u.kind.is_summoned());
        let outcome = if party_alive == 0 {
            Some(Outcome::Wipe)
        } else if has_foes && foes_alive == 0 {
            if self.chain_step < self.fight.chain.len() {
                // A chain goes on: this fight is won; rest, then the next.
                self.end_segment(Outcome::Win);
                let rest = self.fight.chain[self.chain_step].rest_ms;
                self.resting_until = Some(self.now.plus(rest));
                self.log_event(
                    LogEvent::new(self.now, LogKind::FightEnded)
                        .detail(&format!("fight {} won; resting", self.segments.len())),
                );
                return;
            }
            Some(Outcome::Win)
        } else if self.now.ms().saturating_sub(self.fight_started_at.ms()) >= self.fight.timeout_ms
        {
            self.touch(30);
            Some(Outcome::Timeout)
        } else {
            None
        };
        if let Some(outcome) = outcome {
            self.end_segment(outcome);
            self.outcome = Some((outcome, self.now));
            self.log_event(
                LogEvent::new(self.now, LogKind::FightEnded).detail(&format!("{outcome:?}")),
            );
        }
    }

    /// Records the fight just ended as a segment of the run.
    fn end_segment(&mut self, outcome: Outcome) {
        let deaths: u32 = self.stats.slots.iter().map(|s| s.deaths).sum();
        let clear_time_ms = (outcome == Outcome::Win).then(|| {
            let engaged = self
                .segment_engaged
                .or(self.engaged_at)
                .unwrap_or(self.fight_started_at);
            self.now.ms().saturating_sub(engaged.ms())
        });
        self.segments.push(crate::result::Segment {
            outcome,
            clear_time_ms,
            deaths: deaths - self.deaths_before,
        });
        self.deaths_before = deaths;
    }

    /// Spawns a chain's next fight ahead of the party's leader, after the
    /// rest (T4.8.4). Health, energy, recharges, effects, minions, spirits
    /// and death penalty all carry over, since nothing resets them.
    fn spawn_next_fight(&mut self) {
        self.resting_until = None;
        let Some(step) = self.fight.chain.get(self.chain_step).cloned() else {
            return;
        };
        self.chain_step += 1;
        let anchor = self
            .party_leader()
            .map(|l| self.units[l.index()].pos)
            .unwrap_or(crate::geom::Vec2::ZERO);
        for (mut foe, controller) in step.foes.into_iter().zip(step.controllers) {
            foe.id = UnitId(self.units.len() as u16);
            foe.pos = anchor + foe.pos;
            foe.home = foe.pos;
            foe.foe_index = Some(self.stats.foe_ttk_ms.len());
            self.stats.foe_ttk_ms.push(None);
            self.units.push(foe);
            self.controllers.push(controller);
        }
        self.fight_started_at = self.now;
        self.segment_engaged = None;
        self.log_event(
            LogEvent::new(self.now, LogKind::FightEnded)
                .detail(&format!("fight {} begins", self.segments.len() + 1)),
        );
    }

    /// Called whenever a unit's action finishes and it is free again.
    pub fn after_action(&mut self, unit: UnitId) {
        let delay = crate::ai::reaction_delay(self, unit);
        let u = &mut self.units[unit.index()];
        u.next_decision_at = self.now.plus(delay);
        // Resume the auto-attack if there was one.
        if let Some(target) = u.attack_target
            && u.alive()
        {
            u.action = Action::Attacking { target };
        }
    }

    // ------------------------------------------------------------ triggers

    /// Sends an event to the triggers and handlers that care (T3.6.6).
    ///
    /// Triggers are consumed deterministically, in effect order on the
    /// subject. Nesting is limited to [`MAX_TRIGGER_DEPTH`]; a chain that
    /// reaches it is cut and logged, never looped.
    pub fn fire(&mut self, fired: Fired) {
        if self.trigger_depth >= MAX_TRIGGER_DEPTH {
            self.log_event(
                LogEvent::new(self.now, LogKind::Warning)
                    .source(fired.subject)
                    .detail("trigger depth limit reached; chain cut"),
            );
            return;
        }
        self.trigger_depth += 1;

        let subject = fired.subject;
        if subject.index() < self.units.len() {
            // Collect first: the actions may change the effect list.
            let mut matched: Vec<(u32, usize)> = Vec::new();
            let mut handled: Vec<(u32, usize)> = Vec::new();
            for effect in &self.units[subject.index()].effects {
                match effect.source {
                    EffectSource::Skill { skill, def } => {
                        let fight_skill = &self.fight.skills[usize::from(skill)];
                        if let Some(handler) = fight_skill.handler {
                            handled.push((effect.id, handler));
                        }
                        let Some(encoding) = &fight_skill.skill.encoding else {
                            continue;
                        };
                        let Some(def) = encoding.effect_defs.get(usize::from(def)) else {
                            continue;
                        };
                        for (index, trigger) in def.triggers.iter().enumerate() {
                            if let Control::Triggered { event, filter, .. } = trigger
                                && *event == fired.event
                                && effect.charges.get(index).copied().flatten() != Some(0)
                                && filter.as_ref().is_none_or(|filter| {
                                    let judged = fired.other.unwrap_or(subject);
                                    self.filter_passes(filter, judged, effect.caster, None)
                                })
                            {
                                matched.push((effect.id, index));
                            }
                        }
                    }
                    EffectSource::Handler { skill } => {
                        if let Some(handler) = self.fight.skills[usize::from(skill)].handler {
                            handled.push((effect.id, handler));
                        }
                    }
                    EffectSource::Condition(_) => {}
                }
            }

            // Triggers of the auras the subject stands in (Displacement's
            // block, Infuriating Heat's adrenaline). Auras have no charges.
            let mut aura_matches: Vec<(crate::exec::ActiveDef, usize)> = Vec::new();
            for active in self.defs_flagged(subject, crate::setup::DEF_TRIGGERS) {
                if active.effect.is_some() {
                    continue;
                }
                let Some(def) = self.def_of(&active) else {
                    continue;
                };
                for (index, trigger) in def.triggers.iter().enumerate() {
                    if let Control::Triggered { event, filter, .. } = trigger
                        && *event == fired.event
                        && filter.as_ref().is_none_or(|filter| {
                            let judged = fired.other.unwrap_or(subject);
                            self.filter_passes(filter, judged, active.caster, None)
                        })
                    {
                        aura_matches.push((active, index));
                    }
                }
            }

            for (id, index) in matched {
                self.run_trigger(subject, id, index, &fired);
            }
            for (active, index) in aura_matches {
                self.run_aura_trigger(subject, &active, index, &fired);
            }
            let fight = Arc::clone(&self.fight);
            for (id, handler) in handled {
                if self.units[subject.index()]
                    .effects
                    .iter()
                    .any(|e| e.id == id)
                {
                    fight
                        .handlers
                        .get(handler)
                        .on_event(self, &fired, subject, id);
                }
            }
        }

        self.trigger_depth -= 1;
    }

    fn run_trigger(&mut self, bearer: UnitId, id: u32, index: usize, fired: &Fired) {
        let Some(effect) = self.units[bearer.index()]
            .effects
            .iter_mut()
            .find(|e| e.id == id)
        else {
            return;
        };
        let exhausted = match effect.charges.get_mut(index) {
            Some(Some(charges)) => {
                *charges = charges.saturating_sub(1);
                effect.charges.iter().all(|c| *c == Some(0))
            }
            _ => false,
        };
        let effect = effect.clone();
        let EffectSource::Skill { skill, def } = effect.source else {
            return;
        };
        let fight = Arc::clone(&self.fight);
        let Some(encoding) = &fight.skills[usize::from(skill)].skill.encoding else {
            return;
        };
        let Some(Control::Triggered { actions, .. }) = encoding
            .effect_defs
            .get(usize::from(def))
            .and_then(|def| def.triggers.get(index))
        else {
            return;
        };
        let mut ctx = ExecCtx::for_effect(&effect, bearer);
        ctx.other = fired.other;
        ctx.event_skill = fired.skill;
        ctx.event_amount = fired.amount;
        self.execute(actions, &mut ctx);
        if exhausted {
            self.end_effect(bearer, id, EndReason::Replaced);
        }
    }

    /// Runs one trigger of an aura on a unit standing in it.
    fn run_aura_trigger(
        &mut self,
        bearer: UnitId,
        active: &crate::exec::ActiveDef,
        index: usize,
        fired: &Fired,
    ) {
        if !self.units[active.caster.index()].alive() {
            return;
        }
        let fight = Arc::clone(&self.fight);
        let Some(Control::Triggered { actions, .. }) = fight.skills[usize::from(active.skill)]
            .skill
            .encoding
            .as_ref()
            .and_then(|e| e.effect_defs.get(usize::from(active.def)))
            .and_then(|def| def.triggers.get(index))
        else {
            return;
        };
        let mut ctx = ExecCtx::for_def(active, bearer);
        ctx.other = fired.other;
        ctx.event_skill = fired.skill;
        ctx.event_amount = fired.amount;
        self.execute(actions, &mut ctx);
    }

    /// Runs an effect's `on_end` actions.
    pub fn run_on_end(&mut self, bearer: UnitId, effect: &ActiveEffect) {
        let EffectSource::Skill { skill, def } = effect.source else {
            return;
        };
        let fight = Arc::clone(&self.fight);
        let Some(encoding) = &fight.skills[usize::from(skill)].skill.encoding else {
            return;
        };
        let Some(def) = encoding.effect_defs.get(usize::from(def)) else {
            return;
        };
        if def.on_end.is_empty() {
            return;
        }
        let mut ctx = ExecCtx::for_effect(effect, bearer);
        self.execute(&def.on_end, &mut ctx);
    }

    // ---------------------------------------------------------------- log

    /// Records a log event, if logging is on. The closure-free signature is
    /// deliberate: callers build the event only when [`Self::logging`] says so
    /// for anything expensive.
    pub fn log_event(&mut self, event: LogEvent) {
        if let Some(log) = &mut self.log {
            log.push(event);
        }
    }

    /// Whether a log is being kept.
    pub fn logging(&self) -> bool {
        self.log.is_some()
    }

    pub(crate) fn log_effect(
        &mut self,
        kind: LogKind,
        caster: UnitId,
        target: UnitId,
        source: EffectSource,
    ) {
        if !self.logging() {
            return;
        }
        let name = self.effect_name(source);
        self.log_event(
            LogEvent::new(self.now, kind)
                .source(caster)
                .target(target)
                .detail(&name),
        );
    }

    /// A readable name for an effect.
    pub fn effect_name(&self, source: EffectSource) -> String {
        match source {
            EffectSource::Skill { skill, .. } | EffectSource::Handler { skill } => {
                self.fight.skills[usize::from(skill)].skill.name.clone()
            }
            EffectSource::Condition(condition) => format!("{condition:?}"),
        }
    }
}
