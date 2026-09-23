//! T1.5.4: static checks on a skill's encoding.
//!
//! These are the rules the schema cannot express — a `Scaled` value with
//! nothing to scale on, an `ApplyEffect` naming an effect that does not
//! exist, a trigger that can never fire. Each one would otherwise produce a
//! skill that loads, runs, and quietly does the wrong thing.

use crate::dataset::{DataSet, Entry};
use crate::dsl::{Action, Control, EffectDef, Selector, Value};
use crate::error::{DataError, DataErrors};
use crate::skill::{Skill, TargetKind};

/// Checks every encoded skill in a data set.
pub fn check_encodings(data: &DataSet, problems: &mut DataErrors) {
    for entry in data.skills.values() {
        let Some(encoding) = &entry.value.encoding else {
            continue;
        };

        let defined: Vec<&str> = encoding
            .effect_defs
            .iter()
            .map(|definition| definition.id.as_str())
            .collect();

        for action in &encoding.effects {
            check_action(entry, action, &defined, problems);
        }
        for definition in &encoding.effect_defs {
            check_effect_def(entry, definition, &defined, problems);
        }

        // Two definitions sharing an id makes one of them unreachable.
        let mut ids = defined.clone();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        if ids.len() != before {
            problems.0.push(DataError::consistency(
                &entry.path,
                "two effect definitions share an id; an ApplyEffect can only reach one \
                 of them",
            ));
        }
    }
}

fn check_effect_def(
    entry: &Entry<Skill>,
    definition: &EffectDef,
    defined: &[&str],
    problems: &mut DataErrors,
) {
    for action in definition
        .while_active
        .iter()
        .chain(definition.on_end.iter())
    {
        check_action(entry, action, defined, problems);
    }
    for trigger in &definition.triggers {
        check_control(entry, trigger, defined, problems);
    }
    if let Some(duration) = &definition.duration {
        check_positive(entry, duration, "a duration", problems);
    }
}

fn check_action(
    entry: &Entry<Skill>,
    action: &Action,
    defined: &[&str],
    problems: &mut DataErrors,
) {
    let skill = &entry.value;

    // A Scaled value with no attribute silently sits at rank 0 forever.
    for value in action_values(action) {
        for inner in value.walk() {
            match inner {
                Value::Scaled(..) if skill.attribute.is_none() => {
                    problems.0.push(DataError::consistency(
                        &entry.path,
                        "this encoding uses Scaled, but the skill has no attribute to \
                         scale on; give it one, or use ScaledBy to name another",
                    ));
                }
                Value::TitleScaled(..) if skill.title_track.is_none() => {
                    problems.0.push(DataError::consistency(
                        &entry.path,
                        "this encoding uses TitleScaled, but the skill has no title_track",
                    ));
                }
                _ => {}
            }
        }
    }

    if let Action::ApplyEffect {
        effect, duration, ..
    } = action
    {
        // A colon marks a global slug, which lives outside this file.
        if !defined.contains(&effect.as_str()) && !effect.contains(':') {
            problems.0.push(DataError::reference(
                &entry.path,
                format!(
                    "this skill applies the effect `{effect}`, which it does not define; \
                     add it to effect_defs, or use a global slug"
                ),
            ));
        }
        check_positive(entry, duration, "an effect duration", problems);
    }

    if let Some(selector) = action_selector(action) {
        check_selector(entry, selector, problems);
    }

    if let Action::Control(control) = action {
        check_control(entry, control, defined, problems);
    }
}

fn check_control(
    entry: &Entry<Skill>,
    control: &Control,
    defined: &[&str],
    problems: &mut DataErrors,
) {
    match control {
        Control::If {
            then, otherwise, ..
        } => {
            for action in then.iter().chain(otherwise.iter()) {
                check_action(entry, action, defined, problems);
            }
        }
        Control::ForEach { actions, .. } | Control::Sequence(actions) => {
            for action in actions {
                check_action(entry, action, defined, problems);
            }
        }
        Control::Chance { percent, actions } => {
            if !(*percent > 0.0 && *percent <= 100.0) {
                problems.0.push(DataError::consistency(
                    &entry.path,
                    format!(
                        "a Chance is {percent}%, which means nothing; it must be above 0 \
                         and at most 100"
                    ),
                ));
            }
            for action in actions {
                check_action(entry, action, defined, problems);
            }
        }
        Control::Triggered {
            actions, charges, ..
        } => {
            if *charges == Some(0) {
                problems.0.push(DataError::consistency(
                    &entry.path,
                    "a trigger has 0 charges, so it can never fire; leave charges unset \
                     for an unlimited trigger",
                ));
            }
            for action in actions {
                check_action(entry, action, defined, problems);
            }
        }
    }
}

/// A selector built on the skill's target must not name the other side's
/// target: `TargetAlly` in a skill that targets a foe is a slip.
///
/// Only target-rooted selectors are judged. A foe-targeted skill may still
/// reach the user or the party: Power Drain gives *you* energy (F3.1).
fn check_selector(entry: &Entry<Skill>, selector: &Selector, problems: &mut DataErrors) {
    let target = entry.value.target;
    let root = target_root(selector);
    let mismatch = match target {
        TargetKind::Foe => matches!(root, Selector::TargetAlly | Selector::TargetOtherAlly),
        TargetKind::Ally | TargetKind::OtherAlly | TargetKind::AllyOrSelf => {
            matches!(root, Selector::TargetFoe)
        }
        _ => false,
    };

    if mismatch {
        problems.0.push(DataError::consistency(
            &entry.path,
            format!(
                "this skill targets {target:?} but an effect selects the other side; \
                 check the selector"
            ),
        ));
    }
}

/// The innermost selector an area, filter or share is built on.
fn target_root(selector: &Selector) -> &Selector {
    match selector {
        Selector::Adjacent(inner)
        | Selector::Nearby(inner)
        | Selector::InTheArea(inner)
        | Selector::Earshot(inner)
        | Selector::SpiritRange(inner)
        | Selector::InRangeOf(inner)
        | Selector::Nearest(inner) => target_root(inner),
        Selector::Filtered { of, .. }
        | Selector::Secondary { of, .. }
        | Selector::Reduced { of, .. } => target_root(of),
        other => other,
    }
}

fn check_positive(entry: &Entry<Skill>, value: &Value, what: &str, problems: &mut DataErrors) {
    let useless = match value {
        Value::Fixed(number) => *number <= 0,
        // A scaled duration may start at 0 and grow, which is ordinary. Only
        // one that is zero at *both* ends can never hold.
        Value::Scaled(at0, at15) => *at0 <= 0 && *at15 <= 0,
        _ => false,
    };
    if useless {
        problems.0.push(DataError::consistency(
            &entry.path,
            format!("{what} is zero or negative, so the effect would never hold"),
        ));
    }
}

/// Every value an action carries.
fn action_values(action: &Action) -> Vec<&Value> {
    match action {
        Action::Damage { amount, .. }
        | Action::LifeSteal { amount, .. }
        | Action::HealthLoss { amount, .. }
        | Action::Heal { amount, .. }
        | Action::HealthGain { amount, .. }
        | Action::GainEnergy { amount, .. }
        | Action::LoseEnergy { amount, .. }
        | Action::DrainEnergy { amount, .. } => vec![amount],
        Action::SacrificeHealth { percent } => vec![percent],
        Action::GainAdrenaline { strikes, .. } | Action::LoseAdrenaline { strikes, .. } => {
            vec![strikes]
        }
        Action::ApplyCondition { duration, .. } => vec![duration],
        Action::RemoveConditions { count, .. } => vec![count],
        Action::ApplyEffect { duration, .. } => vec![duration],
        Action::RemoveEffects { count, .. } => vec![count],
        Action::KnockDown { duration, .. } => vec![duration],
        Action::DisableSkills { duration, .. } => vec![duration],
        Action::ModifyRecharge { percent, .. } => vec![percent],
        Action::Summon { level, .. } => vec![level],
        Action::CreateSpirit {
            level, duration, ..
        } => vec![level, duration],
        Action::CreateArea { duration, .. } => vec![duration],
        Action::Resurrect {
            health_percent,
            energy_percent,
            ..
        } => vec![health_percent, energy_percent],
        Action::ModifyStat { amount, .. } => vec![amount],
        Action::SetStat { value, .. } => vec![value],
        Action::HoldBundle { duration, .. } => vec![duration],
        _ => Vec::new(),
    }
}

/// The selector an action points at, where it has one.
fn action_selector(action: &Action) -> Option<&Selector> {
    match action {
        Action::Damage { to, .. }
        | Action::LifeSteal { to, .. }
        | Action::HealthLoss { to, .. }
        | Action::Heal { to, .. }
        | Action::HealthGain { to, .. }
        | Action::GainEnergy { to, .. }
        | Action::LoseEnergy { to, .. }
        | Action::GainAdrenaline { to, .. }
        | Action::ApplyCondition { to, .. }
        | Action::ApplyEffect { to, .. }
        | Action::Interrupt { to, .. }
        | Action::FailSkill { to }
        | Action::KnockDown { to, .. }
        | Action::DisableSkills { to, .. }
        | Action::ModifyRecharge { to, .. }
        | Action::Resurrect { to, .. }
        | Action::ModifyStat { to, .. }
        | Action::SetStat { to, .. }
        | Action::ReduceIncomingDamage { to, .. }
        | Action::SetCriticalImmune { to } => Some(to),
        Action::DrainEnergy { from, .. }
        | Action::LoseAdrenaline { from, .. }
        | Action::RemoveConditions { from, .. }
        | Action::RemoveEffects { from, .. } => Some(from),
        _ => None,
    }
}
