//! Mutation and crossover for builds (T5.2.3, T5.2.4, §13.3).
//!
//! Each operator changes only its own part of a slot; repair makes the
//! result legal afterwards. An operator reports whether it changed the bar
//! or the professions, because only then should repair re-derive the
//! attributes — otherwise it would undo a "shift attribute points" mutation.

use gwsim_data::core::{Attribute, Profession};
use gwsim_data::dataset::DataSet;
use gwsim_data::ids::SkillId;
use serde::{Deserialize, Serialize};

use crate::genome::{FreeGenome, PartyGenome, SlotGenome};
use crate::pools::SlotPools;
use crate::rng::OptRng;
use crate::roles::{self, SlotRole};

/// The mutation operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mutation {
    ReplaceSkill,
    SwapElite,
    ShiftAttributes,
    ChangeSecondary,
    ChangePrimary,
    ChangeRune,
    ChangeInsignia,
    ChangeWeapon,
}

impl Mutation {
    pub const ALL: [Mutation; 8] = [
        Mutation::ReplaceSkill,
        Mutation::SwapElite,
        Mutation::ShiftAttributes,
        Mutation::ChangeSecondary,
        Mutation::ChangePrimary,
        Mutation::ChangeRune,
        Mutation::ChangeInsignia,
        Mutation::ChangeWeapon,
    ];
}

/// Operator probabilities (T5.2.1) [Proposed].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rates {
    /// Relative weights of the mutations, in [`Mutation::ALL`] order.
    pub mutation_weights: [f64; 8],
    /// Chance a child is made by crossover rather than copied.
    pub crossover: f64,
    /// Chance of a second mutation on the same child.
    pub second_mutation: f64,
}

impl Default for Rates {
    fn default() -> Self {
        Rates {
            mutation_weights: [0.45, 0.10, 0.15, 0.08, 0.02, 0.10, 0.05, 0.05],
            crossover: 0.9,
            second_mutation: 0.3,
        }
    }
}

/// What a slot's operators need to know.
#[derive(Debug, Clone)]
pub struct SlotContext {
    pub pools: SlotPools,
    pub role: SlotRole,
}

fn open_positions(free: &FreeGenome) -> Vec<usize> {
    (0..8).filter(|p| !free.locks.skill(*p)).collect()
}

fn candidates(free: &FreeGenome, ctx: &SlotContext, data: &DataSet) -> Vec<SkillId> {
    ctx.pools
        .skills_for(data, free.build.primary, free.build.secondary)
        .into_iter()
        .filter(|id| !free.build.skills.contains(&Some(*id)))
        .collect()
}

/// Applies one mutation to a free slot. Returns whether it changed the bar
/// or the professions (so attributes must be re-derived), or `None` if the
/// operator had nothing it could change.
pub fn mutate(
    free: &mut FreeGenome,
    mutation: Mutation,
    ctx: &SlotContext,
    data: &DataSet,
    rng: &mut OptRng,
) -> Option<bool> {
    let locks = free.locks;
    match mutation {
        Mutation::ReplaceSkill => {
            let positions = open_positions(free);
            let position = *rng.pick(&positions)?;
            let pool = candidates(free, ctx, data);
            let new = roles::draw(ctx.role, &pool, data, rng)?;
            free.build.skills[position] = Some(new);
            Some(true)
        }
        Mutation::SwapElite => {
            let is_elite = |id: SkillId| data.skill_by_id(id).is_some_and(|s| s.elite);
            let elites: Vec<SkillId> = candidates(free, ctx, data)
                .into_iter()
                .filter(|id| is_elite(*id))
                .collect();
            let new = *rng.pick(&elites)?;
            let current = open_positions(free)
                .into_iter()
                .find(|p| free.build.skills[*p].is_some_and(is_elite));
            let position = match current {
                Some(p) => p,
                None => *rng.pick(&open_positions(free))?,
            };
            free.build.skills[position] = Some(new);
            Some(true)
        }
        Mutation::ShiftAttributes => {
            if locks.attributes {
                return None;
            }
            let from: Vec<Attribute> = free.build.attribute_points.keys().copied().collect();
            let mut to: Vec<Attribute> = free.build.primary.attributes().to_vec();
            if let Some(s) = free.build.secondary {
                to.extend(
                    s.attributes()
                        .iter()
                        .copied()
                        .filter(|a| *a != s.primary_attribute()),
                );
            }
            let a = *rng.pick(&from)?;
            let b = *rng.pick(&to)?;
            crate::attributes::shift(&mut free.build, a, b).then_some(false)
        }
        Mutation::ChangeSecondary => {
            if locks.secondary {
                return None;
            }
            let primary = free.build.primary;
            let options: Vec<Option<Profession>> = std::iter::once(None)
                .chain(
                    Profession::ALL
                        .into_iter()
                        .filter(|p| *p != primary)
                        .map(Some),
                )
                .filter(|p| *p != free.build.secondary)
                .collect();
            free.build.secondary = *rng.pick(&options)?;
            Some(true)
        }
        Mutation::ChangePrimary => {
            if locks.gear || locks.secondary || ctx.pools.primaries.len() < 2 {
                return None;
            }
            let options: Vec<Profession> = ctx
                .pools
                .primaries
                .iter()
                .copied()
                .filter(|p| *p != free.build.primary)
                .collect();
            free.build.primary = *rng.pick(&options)?;
            Some(true)
        }
        Mutation::ChangeRune => {
            if locks.gear {
                return None;
            }
            let piece = rng.below(free.build.armor.len());
            let runes = ctx.pools.runes_for(data, free.build.primary);
            free.build.armor[piece].rune = if rng.chance(0.1) {
                None
            } else {
                rng.pick(&runes).map(|r| r.slug.clone())
            };
            Some(false)
        }
        Mutation::ChangeInsignia => {
            if locks.gear {
                return None;
            }
            let piece = rng.below(free.build.armor.len());
            let insignias = ctx.pools.insignias_for(data, free.build.primary);
            free.build.armor[piece].insignia = Some(rng.pick(&insignias)?.clone());
            Some(false)
        }
        Mutation::ChangeWeapon => {
            if locks.gear {
                return None;
            }
            // The weapon set's attribute (which spells its halve-casting
            // and halve-recharge chances reach) and the headgear attribute.
            let with_points: Vec<Attribute> = free.build.attribute_points.keys().copied().collect();
            if rng.chance(0.5) {
                free.build.weapon_set.attribute = Some(*rng.pick(&with_points)?);
            } else {
                let primary: Vec<Attribute> = free.build.primary.attributes().to_vec();
                free.build.headgear_attribute = Some(*rng.pick(&primary)?);
            }
            Some(false)
        }
    }
}

/// Picks a mutation by the rates' weights.
pub fn pick_mutation(rates: &Rates, rng: &mut OptRng) -> Mutation {
    Mutation::ALL[rng.weighted(&rates.mutation_weights)]
}

/// Slot-level crossover: each free slot's build comes from one parent or
/// the other.
pub fn slot_crossover(a: &PartyGenome, b: &PartyGenome, rng: &mut OptRng) -> PartyGenome {
    let mut child = a.clone();
    for (slot, other) in child.slots.iter_mut().zip(&b.slots) {
        if matches!(slot, SlotGenome::Free(_)) && rng.chance(0.5) {
            *slot = other.clone();
        }
    }
    child
}

/// Bar-level crossover on one free slot: each unlocked position comes from
/// one parent or the other (repair then removes duplicates and fills gaps).
pub fn bar_crossover(
    a: &PartyGenome,
    b: &PartyGenome,
    slot: usize,
    rng: &mut OptRng,
) -> PartyGenome {
    let mut child = a.clone();
    if let (SlotGenome::Free(mine), SlotGenome::Free(theirs)) =
        (&mut child.slots[slot], &b.slots[slot])
    {
        for position in 0..8 {
            if !mine.locks.skill(position) && rng.chance(0.5) {
                mine.build.skills[position] = theirs.build.skills[position];
            }
        }
    }
    child
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::LockMask;
    use crate::pools::PoolOptions;
    use gwsim_data::party::PartyFile;
    use gwsim_data::source::DirSource;
    use std::path::Path;
    use std::sync::OnceLock;

    fn data() -> &'static DataSet {
        static DATA: OnceLock<DataSet> = OnceLock::new();
        DATA.get_or_init(|| {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
            DataSet::load(&DirSource::new(dir)).unwrap()
        })
    }

    fn party() -> &'static PartyFile {
        data().party(&"m1-mesmerway".parse().unwrap()).unwrap()
    }

    fn free_player() -> (FreeGenome, SlotContext) {
        let genome = PartyGenome::from_party(party(), &[(0, LockMask::default())]);
        let SlotGenome::Free(free) = genome.slots[0].clone() else {
            unreachable!()
        };
        let ctx = SlotContext {
            pools: SlotPools::for_slot(&party().slots[0], data(), &PoolOptions::default()),
            role: SlotRole::Interrupt,
        };
        (free, ctx)
    }

    #[test]
    fn each_mutation_changes_only_its_own_part() {
        for (seed, mutation) in Mutation::ALL.into_iter().enumerate() {
            let (before, ctx) = free_player();
            let mut after = before.clone();
            let mut rng = OptRng::new(seed as u64 + 11);
            let result = mutate(&mut after, mutation, &ctx, data(), &mut rng);
            let (b, a) = (&before.build, &after.build);
            let skills = b.skills != a.skills;
            let points = b.attribute_points != a.attribute_points;
            let professions = (b.primary, b.secondary) != (a.primary, a.secondary);
            let gear = b.armor != a.armor
                || b.weapon_set != a.weapon_set
                || b.headgear_attribute != a.headgear_attribute;
            let changed = match mutation {
                Mutation::ReplaceSkill | Mutation::SwapElite => !points && !professions && !gear,
                Mutation::ShiftAttributes => !skills && !professions && !gear,
                Mutation::ChangeSecondary | Mutation::ChangePrimary => !skills && !points && !gear,
                Mutation::ChangeRune | Mutation::ChangeInsignia | Mutation::ChangeWeapon => {
                    !skills && !points && !professions
                }
            };
            assert!(result.is_some(), "{mutation:?} found nothing to change");
            assert!(changed, "{mutation:?} touched another part");
        }
    }

    #[test]
    fn locked_parts_refuse_their_mutations() {
        let (mut free, ctx) = free_player();
        free.locks = LockMask {
            skills: 0xFF,
            gear: true,
            attributes: true,
            secondary: true,
        };
        let before = free.clone();
        let mut rng = OptRng::new(5);
        for mutation in Mutation::ALL {
            let _ = mutate(&mut free, mutation, &ctx, data(), &mut rng);
        }
        assert_eq!(free, before);
    }

    #[test]
    fn crossover_children_carry_only_parent_genes() {
        let base = party();
        let free: Vec<(usize, LockMask)> = (0..8).map(|i| (i, LockMask::default())).collect();
        let a = PartyGenome::from_party(base, &free);
        let mut b = a.clone();
        let (_, ctx) = free_player();
        let mut rng = OptRng::new(8);
        for slot in 0..8 {
            if let SlotGenome::Free(f) = &mut b.slots[slot] {
                let _ = mutate(f, Mutation::ReplaceSkill, &ctx, data(), &mut rng);
            }
        }
        for seed in 0..20 {
            let mut rng = OptRng::new(seed);
            let child = slot_crossover(&a, &b, &mut rng);
            for (i, slot) in child.slots.iter().enumerate() {
                assert!(slot == &a.slots[i] || slot == &b.slots[i]);
            }
            let child = bar_crossover(&a, &b, 0, &mut rng);
            for p in 0..8 {
                let s = child.slots[0].build().skills[p];
                assert!(s == a.slots[0].build().skills[p] || s == b.slots[0].build().skills[p]);
            }
        }
    }
}
