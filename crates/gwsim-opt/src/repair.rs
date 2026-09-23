//! Repair: any genome back into a legal build (T5.1.4, OPT-1, OPT-5).
//!
//! Mutation and crossover produce whatever they produce; repair makes it
//! legal again, in the order OPT-5 gives:
//!
//! 1. after a profession change, a skill the build can no longer carry is
//!    replaced by a pool skill that shares a role with it;
//! 2. duplicate skills go;
//! 3. extra elites go, keeping the one that best fits the slot's role;
//! 4. extra PvE-only skills go (all of them on a hero);
//! 5. empty positions are filled from the pool by role;
//! 6. attributes are re-derived for the bar;
//! 7. runes, insignias, headgear and the weapon set's attribute are fixed
//!    for the primary profession;
//! 8. redundant runes are removed.
//!
//! **Locked positions and locked gear are never touched** (OPT-3).

use gwsim_data::build::{Build, SlotKind};
use gwsim_data::core::Attribute;
use gwsim_data::dataset::DataSet;
use gwsim_data::ids::SkillId;
use gwsim_data::items::RuneKind;

use crate::genome::{FreeGenome, LockMask};
use crate::pools::SlotPools;
use crate::rng::OptRng;
use crate::roles::{self, SlotRole};

/// The most PvE-only skills a human bar may carry.
pub const MAX_PVE_ONLY: usize = 3;

fn open(locks: &LockMask, position: usize) -> bool {
    !locks.skill(position)
}

/// Repairs one free slot in place. `reallocate` re-derives the attribute
/// points for the bar (step 6); without it, points are only made legal, so a
/// "shift attribute points" mutation survives repair.
pub fn repair(
    free: &mut FreeGenome,
    pools: &SlotPools,
    role: SlotRole,
    data: &DataSet,
    rng: &mut OptRng,
    reallocate: bool,
) {
    let locks = free.locks;
    let build = &mut free.build;

    // Professions.
    if !pools.primaries.contains(&build.primary) {
        build.primary = pools.primaries[0];
    }
    if build.secondary == Some(build.primary) {
        build.secondary = None;
    }

    let allowed = pools.skills_for(data, build.primary, build.secondary);

    // 1. Skills the build can no longer carry, or that are not candidates.
    for position in 0..8 {
        let Some(id) = build.skills[position] else {
            continue;
        };
        if !open(&locks, position) || allowed.contains(&id) {
            continue;
        }
        let old_tags = data.skill_by_id(id).map(roles::tags).unwrap_or_default();
        let sharing: Vec<SkillId> = allowed
            .iter()
            .copied()
            .filter(|c| !build.skills.contains(&Some(*c)))
            .filter(|c| {
                data.skill_by_id(*c)
                    .is_some_and(|s| roles::tags(s).iter().any(|t| old_tags.contains(t)))
            })
            .collect();
        build.skills[position] = if sharing.is_empty() {
            None
        } else {
            roles::draw(role, &sharing, data, rng)
        };
    }

    // 2. Duplicates: the first copy stays (a locked copy wins).
    for position in 0..8 {
        let Some(id) = build.skills[position] else {
            continue;
        };
        let first = (0..8)
            .find(|p| build.skills[*p] == Some(id) && !open(&locks, *p))
            .or_else(|| (0..8).find(|p| build.skills[*p] == Some(id)));
        if first != Some(position) && open(&locks, position) {
            build.skills[position] = None;
        }
    }

    // 3 and 4. Elites and PvE-only skills over the limits.
    let fit = |id: SkillId| data.skill_by_id(id).map(|s| role.fit(s)).unwrap_or(0.0);
    let limit_pve = if free.kind == SlotKind::Human {
        MAX_PVE_ONLY
    } else {
        0
    };
    for (is_kind, limit) in [
        (
            (&|id: SkillId| data.skill_by_id(id).is_some_and(|s| s.elite))
                as &dyn Fn(SkillId) -> bool,
            1usize,
        ),
        (
            &|id: SkillId| data.skill_by_id(id).is_some_and(|s| s.pve_only),
            limit_pve,
        ),
    ] {
        let mut holders: Vec<usize> = (0..8)
            .filter(|p| build.skills[*p].is_some_and(is_kind))
            .collect();
        // Locked holders first, then by fit, best first.
        holders.sort_by(|a, b| {
            let (sa, sb) = (
                build.skills[*a].unwrap_or(SkillId(0)),
                build.skills[*b].unwrap_or(SkillId(0)),
            );
            open(&locks, *a)
                .cmp(&open(&locks, *b))
                .then(fit(sb).total_cmp(&fit(sa)))
                .then(a.cmp(b))
        });
        for position in holders.into_iter().skip(limit) {
            if open(&locks, position) {
                build.skills[position] = None;
            }
        }
    }

    // 5. Fill empty positions by role, within the limits.
    for position in 0..8 {
        if build.skills[position].is_some() || !open(&locks, position) {
            continue;
        }
        let elites = build
            .skills
            .iter()
            .flatten()
            .filter(|id| data.skill_by_id(**id).is_some_and(|s| s.elite))
            .count();
        let pve = build
            .skills
            .iter()
            .flatten()
            .filter(|id| data.skill_by_id(**id).is_some_and(|s| s.pve_only))
            .count();
        let candidates: Vec<SkillId> = allowed
            .iter()
            .copied()
            .filter(|c| !build.skills.contains(&Some(*c)))
            .filter(|c| {
                data.skill_by_id(*c)
                    .is_some_and(|s| (!s.elite || elites == 0) && (!s.pve_only || pve < limit_pve))
            })
            .collect();
        build.skills[position] = roles::draw(role, &candidates, data, rng);
    }

    // 6. Attributes for the bar, or at least legal ones. Locked points
    // still have to suit the professions.
    let open_attributes = attributes_of(build);
    build
        .attribute_points
        .retain(|a, r| open_attributes.contains(a) && *r > 0);
    for rank in build.attribute_points.values_mut() {
        *rank = (*rank).min(crate::attributes::MAX_POINT_RANK);
    }
    let over_budget = gwsim_data::derived::points_spent(build) > gwsim_data::derived::MAX_POINTS;
    if !locks.attributes && (reallocate || over_budget) {
        crate::attributes::allocate(build, data);
    }

    // 7 and 8. Gear.
    if !locks.gear {
        fix_gear(build, pools, data, rng);
    }
}

/// The attributes a build may put points into.
fn attributes_of(build: &Build) -> Vec<Attribute> {
    let mut out = build.primary.attributes().to_vec();
    if let Some(secondary) = build.secondary {
        out.extend(secondary.attributes().iter().copied());
    }
    out
}

/// The primary attribute with the most points (ties: the primary
/// attribute, then template order).
fn best_primary_attribute(build: &Build) -> Attribute {
    let primary = build.primary;
    primary
        .attributes()
        .iter()
        .copied()
        .max_by_key(|a| {
            (
                build.attribute_points.get(a).copied().unwrap_or(0),
                *a == primary.primary_attribute(),
            )
        })
        .unwrap_or(primary.primary_attribute())
}

fn fix_gear(build: &mut Build, pools: &SlotPools, data: &DataSet, rng: &mut OptRng) {
    let primary = build.primary;
    let runes = pools.runes_for(data, primary);
    let insignias = pools.insignias_for(data, primary);
    let best = best_primary_attribute(build);

    for piece in 0..build.armor.len() {
        // Runes that no longer fit become the same grade for the best
        // primary attribute.
        if let Some(slug) = build.armor[piece].rune.clone()
            && !runes.iter().any(|r| r.slug == slug)
        {
            let tier = data
                .runes
                .as_ref()
                .and_then(|f| f.value.runes.iter().find(|r| r.slug == slug))
                .and_then(|r| match &r.kind {
                    RuneKind::Attribute { tier, .. } => Some(*tier),
                    _ => None,
                });
            build.armor[piece].rune = tier.and_then(|tier| {
                runes
                    .iter()
                    .find(|r| {
                        matches!(&r.kind, RuneKind::Attribute { attribute, tier: t }
                            if *attribute == best && *t == tier)
                    })
                    .map(|r| r.slug.clone())
            });
        }
        if let Some(slug) = &build.armor[piece].insignia
            && !insignias.contains(slug)
        {
            build.armor[piece].insignia = rng.pick(&insignias).cloned();
        }
    }
    // A second rune for the same attribute adds nothing but its penalty.
    for piece in crate::runes::redundant_runes(build, data) {
        build.armor[piece].rune = None;
    }
    if let Some(attribute) = build.headgear_attribute
        && attribute.profession() != primary
    {
        build.headgear_attribute = Some(best);
    }
    if let Some(attribute) = build.weapon_set.attribute
        && !attributes_of(build).contains(&attribute)
    {
        build.weapon_set.attribute = build
            .attribute_points
            .iter()
            .max_by_key(|(a, r)| (**r, **a))
            .map(|(a, _)| *a);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::PartyGenome;
    use crate::pools::PoolOptions;
    use gwsim_data::core::Profession;
    use gwsim_data::source::DirSource;
    use proptest::prelude::*;
    use std::path::Path;
    use std::sync::OnceLock;

    fn data() -> &'static DataSet {
        static DATA: OnceLock<DataSet> = OnceLock::new();
        DATA.get_or_init(|| {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
            DataSet::load(&DirSource::new(dir)).unwrap()
        })
    }

    /// Scrambles a slot as badly as mutation and crossover could: random
    /// professions, random skills from the whole data set (any profession,
    /// elites, PvE-only), random points and runes.
    fn scramble(free: &mut FreeGenome, rng: &mut OptRng, data: &DataSet) {
        let all: Vec<SkillId> = data.skills.values().map(|e| e.value.id).collect();
        let locks = free.locks;
        let build = &mut free.build;
        if free.kind == SlotKind::Human {
            build.primary = Profession::ALL[rng.below(10)];
        }
        build.secondary = if rng.chance(0.2) {
            None
        } else {
            Some(Profession::ALL[rng.below(10)])
        };
        for position in 0..8 {
            if !locks.skill(position) {
                build.skills[position] = if rng.chance(0.1) {
                    None
                } else {
                    rng.pick(&all).copied()
                };
            }
        }
        build.attribute_points.clear();
        for _ in 0..4 {
            build.attribute_points.insert(
                Attribute::ALL[rng.below(Attribute::ALL.len())],
                rng.below(13) as u8,
            );
        }
        let runes: Vec<_> = data
            .runes
            .as_ref()
            .unwrap()
            .value
            .runes
            .iter()
            .map(|r| r.slug.clone())
            .collect();
        for piece in 0..5 {
            build.armor[piece].rune = rng.pick(&runes).cloned();
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        #[test]
        fn repair_always_gives_a_legal_build_with_its_locks_intact(
            seed in any::<u64>(),
            slot in 0usize..8,
            lock_bits in any::<u8>(),
        ) {
            let data = data();
            let party = data.party(&"m1-mesmerway".parse().unwrap()).unwrap();
            let locks = LockMask { skills: lock_bits & 0b0000_0111, ..LockMask::default() };
            let genome = PartyGenome::from_party(party, &[(slot, locks)]);
            let crate::genome::SlotGenome::Free(mut free) = genome.slots[slot].clone() else {
                panic!("slot {slot} should be free");
            };
            let original = free.build.clone();
            let mut rng = OptRng::new(seed);
            scramble(&mut free, &mut rng, data);
            let locked_before: Vec<_> = (0..8).filter(|p| locks.skill(*p)).map(|p| free.build.skills[p]).collect();
            let pools = SlotPools::for_slot(&party.slots[slot], data, &PoolOptions::default());
            let role = SlotRole::infer(&original, data);
            repair(&mut free, &pools, role, data, &mut rng, true);
            let problems: Vec<_> = free
                .build
                .check(data, free.kind)
                .into_iter()
                // A scrambled locked position may hold a skill of a profession
                // the slot no longer has; that is the lock's price, not
                // repair's fault.
                .filter(|p| !matches!(p, gwsim_data::build::LegalityError::SkillNotAvailable { .. }
                    | gwsim_data::build::LegalityError::DuplicateSkill(_)
                    | gwsim_data::build::LegalityError::TooManyElites(_)
                    | gwsim_data::build::LegalityError::PveOnlyOnHero(_)
                    | gwsim_data::build::LegalityError::TooManyPveOnly(_)) || locks.skills == 0)
                .collect();
            prop_assert!(problems.is_empty(), "{problems:?}");
            let locked_after: Vec<_> = (0..8).filter(|p| locks.skill(*p)).map(|p| free.build.skills[p]).collect();
            prop_assert_eq!(locked_before, locked_after);
            if free.kind != SlotKind::Human {
                prop_assert!(free.build.skills.iter().flatten().all(|id| !data.skill_by_id(*id).unwrap().pve_only));
                prop_assert_eq!(free.build.primary, original.primary);
            }
        }
    }

    #[test]
    fn repair_leaves_a_legal_build_alone_apart_from_attributes() {
        let data = data();
        let party = data.party(&"m1-mesmerway".parse().unwrap()).unwrap();
        let genome = PartyGenome::from_party(party, &[(1, LockMask::default())]);
        let crate::genome::SlotGenome::Free(mut free) = genome.slots[1].clone() else {
            unreachable!()
        };
        let skills = free.build.skills;
        let pools = SlotPools::for_slot(&party.slots[1], data, &PoolOptions::default());
        repair(
            &mut free,
            &pools,
            SlotRole::Interrupt,
            data,
            &mut OptRng::new(1),
            false,
        );
        assert_eq!(
            free.build.attribute_points,
            party.slots[1].build.attribute_points
        );
        assert_eq!(free.build.skills, skills);
        assert!(free.build.is_legal(data, SlotKind::Hero));
    }
}
