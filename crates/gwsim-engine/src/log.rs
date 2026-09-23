//! The combat log (WP3.9, §14.3).
//!
//! Recorded only when asked for: a fight without a log keeps `None` and every
//! emission point is one branch. A logged fight and an unlogged one produce
//! the same result, which the determinism tests check (T3.9.5).

use serde::Serialize;

use crate::time::SimTime;
use crate::unit::UnitId;

/// What happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum LogKind {
    SkillStarted,
    SkillCompleted,
    SkillInterrupted,
    SkillCancelled,
    SkillFailed,
    SkillFizzled,
    AttackStarted,
    AttackHit,
    AttackMissed,
    AttackBlocked,
    CriticalHit,
    Damage,
    Heal,
    EnergyChanged,
    EffectApplied,
    EffectRemoved,
    EffectEnded,
    KnockedDown,
    Death,
    Kill,
    MovementStarted,
    MovementStopped,
    Decision,
    Aggro,
    FightEnded,
    Warning,
}

/// One log line.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LogEvent {
    pub t_ms: u32,
    pub kind: LogKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<u16>,
    /// The skill's template id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<i32>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

impl LogEvent {
    pub fn new(at: SimTime, kind: LogKind) -> Self {
        LogEvent {
            t_ms: at.ms(),
            kind,
            source: None,
            target: None,
            skill: None,
            amount: None,
            detail: String::new(),
        }
    }

    pub fn source(mut self, unit: UnitId) -> Self {
        self.source = Some(unit.0);
        self
    }

    pub fn target(mut self, unit: UnitId) -> Self {
        self.target = Some(unit.0);
        self
    }

    pub fn skill(mut self, id: u16) -> Self {
        self.skill = Some(id);
        self
    }

    pub fn amount(mut self, amount: i32) -> Self {
        self.amount = Some(amount);
        self
    }

    pub fn detail(mut self, text: &str) -> Self {
        self.detail = text.to_owned();
        self
    }
}

/// Writes a log as JSON Lines, one object per line (T3.9.3).
pub fn to_json_lines(events: &[LogEvent]) -> String {
    let mut out = String::new();
    for event in events {
        out.push_str(&serde_json::to_string(event).unwrap_or_default());
        out.push('\n');
    }
    out
}

/// Writes a log as text lines such as
/// `  12.350s  Player → Dummy 1  Energy Surge  Damage 84`.
///
/// `unit_name` and `skill_name` resolve ids, so the engine stays free of
/// data lookups here.
pub fn to_text(
    events: &[LogEvent],
    unit_name: impl Fn(u16) -> String,
    skill_name: impl Fn(u16) -> String,
) -> String {
    let mut out = String::new();
    for event in events {
        let who = match (event.source, event.target) {
            (Some(source), Some(target)) if source != target => {
                format!("{} → {}", unit_name(source), unit_name(target))
            }
            (Some(unit), _) | (None, Some(unit)) => unit_name(unit),
            (None, None) => String::new(),
        };
        let skill = event.skill.map(&skill_name).unwrap_or_default();
        let amount = event.amount.map(|a| format!(" {a}")).unwrap_or_default();
        let detail = if event.detail.is_empty() {
            String::new()
        } else {
            format!(" ({})", event.detail)
        };
        out.push_str(&format!(
            "{}  {:<28} {:<24} {:?}{amount}{detail}\n",
            SimTime(event.t_ms),
            who,
            skill,
            event.kind
        ));
    }
    out
}
