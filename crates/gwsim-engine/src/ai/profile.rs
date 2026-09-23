//! What a skill is for, read from its encoding once per fight (T4.4.5,
//! T4.5.4).
//!
//! The AI controllers ask the same questions of every skill: does it need
//! something of its target (a foe casting, an ally hexed)? Does it heal,
//! remove, protect, summon, raise? Would casting it again be wasted? Rather
//! than hand-write AI hints for every skill, those answers are read from the
//! DSL, which already says all of it. A skill's own `ai` hints, where a file
//! gives them, are applied on top (priority, `use_when`, `never_when`).

use gwsim_data::dsl::{Action, Control, EffectKind, Filter, Selector, Stat};
use gwsim_data::skill::Skill;

/// The AI-relevant shape of one skill.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SkillProfile {
    /// The skill does nothing unless this holds of its target: its effects
    /// all sit inside one `If` on the target (Cry of Frustration's "if target
    /// foe is using a skill").
    pub requirement: Option<Filter>,
    /// Heals or grants health to its target ally.
    pub heals_target: bool,
    /// Heals the party or allies in an area.
    pub heals_party: bool,
    /// Removes effects of this kind from its target.
    pub removes: Option<EffectKind>,
    /// Removes conditions.
    pub removes_conditions: bool,
    /// Puts an effect definition on its target: not reapplied while it lasts.
    pub target_effect: Option<u16>,
    /// Puts an effect definition on its user: kept up, not reapplied.
    pub self_effect: Option<u16>,
    /// Puts an effect on allies around it (a shout's buff).
    pub party_effect: Option<u16>,
    /// Creates a spirit of this type.
    pub spirit: Option<String>,
    pub summons: bool,
    pub resurrects: bool,
    pub interrupts: bool,
    pub damages: bool,
    /// Gains its user energy.
    pub gains_energy: bool,
    /// Raises its target's energy regeneration (a battery: Blood is Power).
    pub battery: bool,
    /// Speeds its targets up ("Incoming!", "Fall Back!").
    pub speed_boost: bool,
    /// Only worth using near the user's own spirits or creatures (Armor of
    /// Unfeeling, Signet of Creation).
    pub needs_creatures: bool,
    /// Knocks down.
    pub knocks_down: bool,
    /// Holds a bundle (an item spell).
    pub bundle: bool,
    /// A handler does the work; its name.
    pub handler: Option<String>,
}

impl SkillProfile {
    /// Reads a skill's profile from its encoding.
    pub fn of(skill: &Skill) -> SkillProfile {
        let mut profile = SkillProfile::default();
        let Some(encoding) = &skill.encoding else {
            return profile;
        };
        profile.handler = encoding.handler.as_ref().map(|h| h.name.clone());
        if let Some(name) = &profile.handler {
            match name.as_str() {
                "resurrection_chant" => profile.resurrects = true,
                "arcane_echo" | "air_of_superiority" | "master_of_magic" | "soul_twisting" => {
                    // Each applies a lasting effect to its user.
                    profile.self_effect = Some(u16::MAX);
                }
                _ => {}
            }
        }

        // A requirement: every effect inside one `If` with no `otherwise`,
        // judged on the target.
        if let [Action::Control(control)] = encoding.effects.as_slice()
            && let Control::If {
                condition,
                of,
                otherwise,
                ..
            } = control.as_ref()
            && otherwise.is_empty()
            && of.as_ref().is_none_or(is_target)
        {
            profile.requirement = Some(condition.clone());
        }

        let defs = &encoding.effect_defs;
        let visit = |action: &Action, profile: &mut SkillProfile| match action {
            Action::Heal { to, .. } | Action::HealthGain { to, .. } => {
                if is_target(to) {
                    profile.heals_target = true;
                } else if matches!(
                    to,
                    Selector::Party | Selector::PartyInRange(_) | Selector::InRangeOf(_)
                ) {
                    profile.heals_party = true;
                }
            }
            Action::RemoveEffects { from, kind, .. } if is_target(from) => {
                profile.removes = Some(*kind);
            }
            Action::RemoveConditions { .. } => profile.removes_conditions = true,
            Action::ApplyEffect { to, effect, .. } => {
                let index = defs.iter().position(|d| d.id == *effect).map(|i| i as u16);
                if matches!(to, Selector::SelfUnit) {
                    profile.self_effect = index;
                } else if matches!(
                    to,
                    Selector::Party
                        | Selector::PartyInRange(_)
                        | Selector::Filtered { .. }
                        | Selector::Around { .. }
                ) {
                    profile.party_effect = index;
                } else {
                    profile.target_effect = index;
                }
                if let Some(def) = index.and_then(|i| defs.get(usize::from(i))) {
                    for inner in &def.while_active {
                        if let Action::ModifyStat { stat, .. } = inner {
                            match stat {
                                Stat::MovementSpeed => profile.speed_boost = true,
                                Stat::EnergyRegeneration => profile.battery = true,
                                _ => {}
                            }
                        }
                    }
                }
            }
            Action::CreateSpirit { spirit, .. } => profile.spirit = Some(spirit.to_string()),
            Action::Summon { .. } => profile.summons = true,
            Action::Resurrect { .. } => profile.resurrects = true,
            Action::Interrupt { .. } => profile.interrupts = true,
            Action::Damage { .. } | Action::LifeSteal { .. } => profile.damages = true,
            Action::GainEnergy {
                to: Selector::SelfUnit,
                ..
            } => profile.gains_energy = true,
            Action::KnockDown { .. } => profile.knocks_down = true,
            Action::HoldBundle { .. } => profile.bundle = true,
            _ => {}
        };
        for action in walk(&encoding.effects) {
            visit(action, &mut profile);
        }
        // A skill whose worth depends on the user's creatures.
        let needs = |action: &Action| {
            let text = format!("{action:?}");
            text.contains("CreaturesControlled")
                || text.contains("IsSpirit, Owned")
                || text.contains("Owned")
        };
        if walk(&encoding.effects).any(needs)
            && !profile.damages
            && profile.spirit.is_none()
            && !profile.summons
        {
            profile.needs_creatures = true;
        }
        profile
    }
}

/// Whether a selector names the skill's target.
fn is_target(selector: &Selector) -> bool {
    matches!(
        selector,
        Selector::Target | Selector::TargetFoe | Selector::TargetAlly | Selector::TargetOtherAlly
    )
}

/// Every action in a list, following `If`, `Chance`, `Sequence` and
/// `ForEach` into their branches.
fn walk(actions: &[Action]) -> impl Iterator<Item = &Action> {
    let mut found = Vec::new();
    fn visit<'a>(actions: &'a [Action], found: &mut Vec<&'a Action>) {
        for action in actions {
            found.push(action);
            if let Action::Control(control) = action {
                match control.as_ref() {
                    Control::If {
                        then, otherwise, ..
                    } => {
                        visit(then, found);
                        visit(otherwise, found);
                    }
                    Control::ForEach { actions, .. }
                    | Control::Chance { actions, .. }
                    | Control::Triggered { actions, .. } => visit(actions, found),
                    Control::Sequence(inner) => visit(inner, found),
                }
            }
        }
    }
    visit(actions, &mut found);
    found.into_iter()
}
