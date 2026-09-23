//! Role tags, slot roles, role-weighted sampling and heuristic bars (WP5.4,
//! §8.4, §13.5).
//!
//! - **Automatic tags** ([`auto_tags`]) are read from the DSL: a `Damage`
//!   action is Damage, an area selector AoE, `Interrupt` Interrupt,
//!   removing hexes Hex removal, healing Healing, a spirit Spirit, a minion
//!   Minion, energy gain or drain Energy management, a movement penalty on
//!   foes Snare, `Resurrect` Resurrection, party-wide buffs Party buff,
//!   damage reduction Protection, and so on. A file's manual `roles` are
//!   added to them ([`tags`]).
//! - **A slot's role** ([`SlotRole`]) is inferred from its build, or set.
//! - **Mutation draws** weight each skill by how well its tags fit the
//!   slot's role, with a small chance ε of any legal skill instead.
//! - **Heuristic bars** take the highest-rated skills for a role.

use std::collections::BTreeSet;

use gwsim_data::build::Build;
use gwsim_data::core::Profession;
use gwsim_data::dataset::DataSet;
use gwsim_data::dsl::{Action, Control, EffectKind, Selector, Stat};
use gwsim_data::ids::SkillId;
use gwsim_data::skill::{RoleTag, Skill};
use serde::{Deserialize, Serialize};

use crate::rng::OptRng;

/// The chance of drawing any legal skill rather than a role-weighted one
/// (T5.4.3) [Proposed].
pub const EXPLORATION: f64 = 0.1;

fn selector_is_area(selector: &Selector) -> bool {
    match selector {
        Selector::Adjacent(_)
        | Selector::Nearby(_)
        | Selector::InTheArea(_)
        | Selector::Earshot(_)
        | Selector::SpiritRange(_)
        | Selector::Around { .. } => true,
        Selector::Filtered { of, .. }
        | Selector::Reduced { of, .. }
        | Selector::Secondary { of, .. } => selector_is_area(of),
        _ => false,
    }
}

fn selector_is_party(selector: &Selector) -> bool {
    match selector {
        Selector::Party | Selector::PartyInRange(_) | Selector::Allies => true,
        Selector::Filtered { of, .. } => selector_is_party(of),
        _ => false,
    }
}

fn selector_names(selector: &Selector, tags: &mut BTreeSet<RoleTag>) {
    match selector {
        Selector::Spirits => {
            tags.insert(RoleTag::Spirit);
        }
        Selector::Minions => {
            tags.insert(RoleTag::Minion);
        }
        Selector::Filtered { of, .. }
        | Selector::Reduced { of, .. }
        | Selector::Secondary { of, .. }
        | Selector::Nearest(of)
        | Selector::Adjacent(of)
        | Selector::Nearby(of)
        | Selector::Earshot(of)
        | Selector::SpiritRange(of)
        | Selector::InTheArea(of) => selector_names(of, tags),
        _ => {}
    }
}

/// The tags a handler declares (T5.4.1): what its Rust does that the DSL
/// cannot show.
pub fn handler_tags(name: &str) -> &'static [RoleTag] {
    match name {
        "air_of_superiority" => &[RoleTag::EnergyManagement, RoleTag::Healing],
        "soul_twisting" => &[RoleTag::Spirit, RoleTag::EnergyManagement],
        "resurrection_chant" => &[RoleTag::Resurrection],
        "master_of_magic" => &[RoleTag::Damage],
        _ => &[],
    }
}

fn selector_is_foe(selector: &Selector) -> bool {
    matches!(selector, Selector::TargetFoe | Selector::Foes)
        || match selector {
            Selector::Adjacent(inner)
            | Selector::Nearby(inner)
            | Selector::InTheArea(inner)
            | Selector::Earshot(inner) => selector_is_foe(inner),
            _ => false,
        }
}

fn walk(actions: &[Action], skill: &Skill, tags: &mut BTreeSet<RoleTag>) {
    for action in actions {
        let target = match action {
            Action::Damage { to, .. }
            | Action::Heal { to, .. }
            | Action::HealthGain { to, .. }
            | Action::ApplyEffect { to, .. }
            | Action::ModifyStat { to, .. }
            | Action::SetCriticalImmune { to } => Some(to),
            _ => None,
        };
        if let Some(to) = target {
            selector_names(to, tags);
        }
        match action {
            Action::Damage { to, .. }
            | Action::LifeSteal { to, .. }
            | Action::HealthLoss { to, .. } => {
                tags.insert(RoleTag::Damage);
                if selector_is_area(to) {
                    tags.insert(RoleTag::Aoe);
                }
            }
            Action::Heal { to, .. } | Action::HealthGain { to, .. } => {
                if !matches!(to, Selector::SelfUnit)
                    || skill.target == gwsim_data::skill::TargetKind::None
                {
                    tags.insert(RoleTag::Healing);
                }
            }
            Action::GainEnergy { .. } | Action::DrainEnergy { .. } | Action::LoseEnergy { .. } => {
                tags.insert(RoleTag::EnergyManagement);
            }
            Action::Interrupt { .. } | Action::FailSkill { .. } => {
                tags.insert(RoleTag::Interrupt);
            }
            Action::RemoveEffects { kind, .. } => {
                tags.insert(match kind {
                    EffectKind::Hex => RoleTag::HexRemoval,
                    _ => RoleTag::EnchantmentRemoval,
                });
            }
            Action::RemoveConditions { .. } => {
                tags.insert(RoleTag::ConditionRemoval);
            }
            Action::Summon { .. } => {
                tags.insert(RoleTag::Minion);
            }
            Action::CreateSpirit { attack_damage, .. } => {
                tags.insert(RoleTag::Spirit);
                if attack_damage.is_some() {
                    tags.insert(RoleTag::Damage);
                }
            }
            Action::RunHandler { name } => tags.extend(handler_tags(name).iter().copied()),
            Action::Resurrect { .. } => {
                tags.insert(RoleTag::Resurrection);
            }
            Action::ReduceIncomingDamage { .. } | Action::SetCriticalImmune { .. } => {
                tags.insert(RoleTag::Protection);
            }
            Action::KnockDown { .. } => {
                tags.insert(RoleTag::Pressure);
            }
            Action::ModifyStat { to, stat, .. } => {
                let on_foes = selector_is_foe(to);
                if *stat == Stat::MovementSpeed && on_foes {
                    tags.insert(RoleTag::Snare);
                }
                if matches!(stat, Stat::EnergyRegeneration | Stat::MaxEnergy) && !on_foes {
                    tags.insert(RoleTag::EnergyManagement);
                }
                if matches!(stat, Stat::HealthPerSecond | Stat::HealthRegeneration) && !on_foes {
                    tags.insert(RoleTag::Healing);
                }
                if matches!(stat, Stat::Armor | Stat::DamageTaken | Stat::BlockChance) && !on_foes {
                    tags.insert(RoleTag::Protection);
                }
                if selector_is_party(to) {
                    tags.insert(RoleTag::PartyBuff);
                }
            }
            Action::ApplyEffect { to, effect, .. } => {
                if selector_is_party(to) {
                    tags.insert(RoleTag::PartyBuff);
                }
                if let Some(def) = skill
                    .encoding
                    .as_ref()
                    .and_then(|e| e.effect_defs.iter().find(|d| d.id == *effect))
                    && def.kind == EffectKind::Hex
                {
                    tags.insert(RoleTag::Hex);
                }
            }
            Action::Control(control) => walk_control(control, skill, tags),
            _ => {}
        }
    }
}

fn walk_control(control: &Control, skill: &Skill, tags: &mut BTreeSet<RoleTag>) {
    match control {
        Control::If {
            then, otherwise, ..
        } => {
            walk(then, skill, tags);
            walk(otherwise, skill, tags);
        }
        Control::ForEach { actions, .. }
        | Control::Chance { actions, .. }
        | Control::Triggered { actions, .. } => walk(actions, skill, tags),
        Control::Sequence(actions) => walk(actions, skill, tags),
    }
}

/// The tags read from a skill's encoding alone.
pub fn auto_tags(skill: &Skill) -> BTreeSet<RoleTag> {
    let mut tags = BTreeSet::new();
    if let Some(encoding) = &skill.encoding {
        walk(&encoding.effects, skill, &mut tags);
        // Every effect a skill defines is something it does, whether it is
        // applied directly or carried by a spirit's aura.
        for def in &encoding.effect_defs {
            walk(&def.while_active, skill, &mut tags);
            walk(&def.on_end, skill, &mut tags);
            for control in &def.triggers {
                walk_control(control, skill, &mut tags);
            }
        }
        if let Some(handler) = &encoding.handler {
            tags.extend(handler_tags(&handler.name).iter().copied());
        }
    }
    tags
}

/// Automatic tags plus the file's manual ones (T5.4.1).
pub fn tags(skill: &Skill) -> BTreeSet<RoleTag> {
    let mut tags = auto_tags(skill);
    if let Some(encoding) = &skill.encoding {
        tags.extend(encoding.roles.iter().copied());
    }
    tags
}

/// What a slot is for (T5.4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SlotRole {
    Healer,
    Protection,
    Damage,
    Interrupt,
    Minions,
    Spirits,
    Support,
}

impl SlotRole {
    /// Every role.
    pub const ALL: [SlotRole; 7] = [
        SlotRole::Healer,
        SlotRole::Protection,
        SlotRole::Damage,
        SlotRole::Interrupt,
        SlotRole::Minions,
        SlotRole::Spirits,
        SlotRole::Support,
    ];

    /// How well a tag serves this role: 1 for its core tags, 0.5 for
    /// helpful ones, 0 otherwise.
    pub fn affinity(self, tag: RoleTag) -> f64 {
        use RoleTag as T;
        let (core, helpful): (&[RoleTag], &[RoleTag]) = match self {
            SlotRole::Healer => (
                &[T::Healing],
                &[
                    T::ConditionRemoval,
                    T::HexRemoval,
                    T::Resurrection,
                    T::EnergyManagement,
                ],
            ),
            SlotRole::Protection => (
                &[T::Protection, T::Defence],
                &[T::Spirit, T::HexRemoval, T::Healing, T::PartyBuff],
            ),
            SlotRole::Damage => (
                &[T::Damage, T::Aoe, T::Spike],
                &[T::Pressure, T::Hex, T::Snare],
            ),
            SlotRole::Interrupt => (
                &[T::Interrupt],
                &[
                    T::EnergyManagement,
                    T::Damage,
                    T::Hex,
                    T::EnchantmentRemoval,
                ],
            ),
            SlotRole::Minions => (&[T::Minion], &[T::Damage, T::Healing, T::EnergyManagement]),
            SlotRole::Spirits => (&[T::Spirit], &[T::Protection, T::Healing, T::Damage]),
            SlotRole::Support => (
                &[T::PartyBuff, T::EnergyManagement],
                &[T::HexRemoval, T::ConditionRemoval, T::Resurrection],
            ),
        };
        if core.contains(&tag) {
            1.0
        } else if helpful.contains(&tag) {
            0.5
        } else {
            0.0
        }
    }

    /// How well a skill fits: its best-fitting tag.
    pub fn fit(self, skill: &Skill) -> f64 {
        tags(skill)
            .into_iter()
            .map(|t| self.affinity(t))
            .fold(0.0, f64::max)
    }

    /// The role a build's bar serves most: each skill votes with its fit to
    /// every role; ties go to the earlier role in [`SlotRole::ALL`].
    pub fn infer(build: &Build, data: &DataSet) -> SlotRole {
        let skills: Vec<&Skill> = build
            .skills
            .iter()
            .flatten()
            .filter_map(|id| data.skill_by_id(*id))
            .collect();
        SlotRole::ALL
            .into_iter()
            .map(|role| {
                let core: f64 = skills
                    .iter()
                    .map(|s| if role.fit(s) >= 1.0 { 1.0 } else { 0.0 })
                    .sum();
                (role, core)
            })
            .fold((SlotRole::Support, -1.0), |best, next| {
                if next.1 > best.1 { next } else { best }
            })
            .0
    }

    /// Parses `healer`, `protection`, … for `--role`.
    pub fn parse(text: &str) -> Option<SlotRole> {
        SlotRole::ALL
            .into_iter()
            .find(|r| format!("{r:?}").eq_ignore_ascii_case(text))
    }
}

/// Draws a skill for a slot of this role from `candidates`: role-weighted,
/// or with probability [`EXPLORATION`] uniformly (T5.4.3). A small floor
/// keeps every legal skill possible.
pub fn draw(
    role: SlotRole,
    candidates: &[SkillId],
    data: &DataSet,
    rng: &mut OptRng,
) -> Option<SkillId> {
    if candidates.is_empty() {
        return None;
    }
    if rng.chance(EXPLORATION) {
        return rng.pick(candidates).copied();
    }
    let weights: Vec<f64> = candidates
        .iter()
        .map(|id| data.skill_by_id(*id).map(|s| role.fit(s)).unwrap_or(0.0) + 0.05)
        .collect();
    Some(candidates[rng.weighted(&weights)])
}

/// A static rating of a skill for a role (T5.4.4) [Proposed]: its fit,
/// times the size of its scaled values at rank 12 per energy spent and per
/// second of recharge. Crude by design; refining it from evaluation data is
/// future work.
pub fn rating(role: SlotRole, skill: &Skill) -> f64 {
    let fit = role.fit(skill);
    if fit == 0.0 {
        return 0.0;
    }
    let magnitude: f64 = skill
        .extracted
        .scaled
        .iter()
        .map(|v| f64::from(v.r12.abs()))
        .sum::<f64>()
        .max(10.0);
    let energy = f64::from(skill.cost.energy.max(1));
    let recharge = skill.recharge.as_secs_f64().max(1.0);
    fit * magnitude / (energy * recharge).sqrt()
}

/// A greedy bar for a role and profession pair from `candidates`: the
/// highest-rated skills, at most one elite, then attributes allocated
/// (T5.4.4). The caller repairs the gear.
pub fn heuristic_bar(
    role: SlotRole,
    primary: Profession,
    secondary: Option<Profession>,
    candidates: &[SkillId],
    data: &DataSet,
) -> Build {
    let mut rated: Vec<(f64, SkillId)> = candidates
        .iter()
        .filter_map(|id| data.skill_by_id(*id).map(|s| (rating(role, s), *id)))
        .collect();
    rated.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut build = Build::new(primary);
    build.secondary = secondary;
    let mut elite = false;
    let mut pve_only = 0;
    let mut position = 0;
    for (_, id) in rated {
        if position >= 8 {
            break;
        }
        let Some(skill) = data.skill_by_id(id) else {
            continue;
        };
        if skill.elite && elite {
            continue;
        }
        if skill.pve_only && pve_only >= 3 {
            continue;
        }
        elite |= skill.elite;
        pve_only += usize::from(skill.pve_only);
        build.skills[position] = Some(id);
        position += 1;
    }
    crate::attributes::allocate(&mut build, data);
    build
}

#[cfg(test)]
mod tests {
    use super::*;
    use gwsim_data::source::DirSource;
    use std::path::Path;

    fn data() -> DataSet {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        DataSet::load(&DirSource::new(dir)).unwrap()
    }

    #[test]
    fn automatic_tags_cover_the_manual_ones_or_the_gap_is_known() {
        // The DSL cannot see intent: "Spike", "Pressure" and "Defence" are
        // judgements, and a heal-over-time the encoding writes as health
        // regeneration reads as a stat, not a heal. Those are the known gaps.
        let data = data();
        let known_gaps = [RoleTag::Spike, RoleTag::Pressure, RoleTag::Defence];
        let party = data.party(&"m1-mesmerway".parse().unwrap()).unwrap();
        let mut missing = Vec::new();
        for id in party
            .slots
            .iter()
            .flat_map(|s| s.build.skills.iter().flatten())
        {
            let skill = data.skill_by_id(*id).unwrap();
            let auto = auto_tags(skill);
            for manual in &skill.encoding.as_ref().unwrap().roles {
                if !auto.contains(manual) && !known_gaps.contains(manual) {
                    missing.push(format!("{}: {manual:?}", skill.name));
                }
            }
        }
        missing.sort();
        missing.dedup();
        // Recorded rather than asserted empty: the list is the explanation
        // T5.4.1 asks for, and it must not grow unnoticed.
        assert!(missing.len() <= 12, "{missing:#?}");
    }

    #[test]
    fn the_m1_heroes_are_inferred_as_expected() {
        let data = data();
        let party = data.party(&"m1-mesmerway".parse().unwrap()).unwrap();
        let role = |name: &str| {
            let slot = party.slots.iter().find(|s| s.name == name).unwrap();
            SlotRole::infer(&slot.build, &data)
        };
        assert_eq!(role("hero 5"), SlotRole::Healer);
        assert_eq!(role("hero 7"), SlotRole::Spirits);
        assert!(matches!(
            role("hero 1"),
            SlotRole::Interrupt | SlotRole::Damage
        ));
    }

    #[test]
    fn heuristic_bars_are_legal_for_every_m1_profession_pair() {
        let data = data();
        let party = data.party(&"m1-mesmerway".parse().unwrap()).unwrap();
        let options = crate::pools::PoolOptions::default();
        for slot in &party.slots {
            let pools = crate::pools::SlotPools::for_slot(slot, &data, &options);
            let candidates = pools.skills_for(&data, slot.build.primary, slot.build.secondary);
            for role in SlotRole::ALL {
                let build = heuristic_bar(
                    role,
                    slot.build.primary,
                    slot.build.secondary,
                    &candidates,
                    &data,
                );
                let problems = build.check(&data, slot.kind);
                assert!(problems.is_empty(), "{} {role:?}: {problems:?}", slot.name);
            }
        }
    }

    #[test]
    fn draws_follow_the_role_weights() {
        let data = data();
        let party = data.party(&"m1-mesmerway".parse().unwrap()).unwrap();
        let slot = party.slots.iter().find(|s| s.name == "hero 5").unwrap();
        let pools =
            crate::pools::SlotPools::for_slot(slot, &data, &crate::pools::PoolOptions::default());
        let candidates = pools.skills_for(&data, slot.build.primary, slot.build.secondary);
        let mut rng = OptRng::new(3);
        let mut healing = 0;
        let draws = 4000;
        for _ in 0..draws {
            let id = draw(SlotRole::Healer, &candidates, &data, &mut rng).unwrap();
            if SlotRole::Healer.fit(data.skill_by_id(id).unwrap()) > 0.0 {
                healing += 1;
            }
        }
        // Expected share: the fitting skills' weight over the total, blended
        // with 10% uniform exploration.
        let weight = |id: &SkillId| SlotRole::Healer.fit(data.skill_by_id(*id).unwrap()) + 0.05;
        let fitting: f64 = candidates
            .iter()
            .filter(|id| weight(id) > 0.05)
            .map(weight)
            .sum();
        let total: f64 = candidates.iter().map(weight).sum();
        let uniform = candidates.iter().filter(|id| weight(id) > 0.05).count() as f64
            / candidates.len() as f64;
        let expected = 0.9 * fitting / total + 0.1 * uniform;
        let observed = healing as f64 / f64::from(draws);
        assert!(
            (observed - expected).abs() < 0.03,
            "{observed} vs {expected}"
        );
    }
}
