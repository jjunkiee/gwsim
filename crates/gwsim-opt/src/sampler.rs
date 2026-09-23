//! Random legal builds (T5.9.1): the random fill of generation 0, and RC4's
//! sample of player builds.
//!
//! A random build draws a secondary profession (or none), a bar of pool
//! skills with at most one elite and at most three PvE-only skills, random
//! runes and insignias, and then goes through repair, which allocates the
//! attributes and makes the gear legal. All from one seed.

use gwsim_data::core::Profession;
use gwsim_data::dataset::DataSet;

use crate::genome::FreeGenome;
use crate::operators::SlotContext;
use crate::repair::repair;
use crate::rng::OptRng;

/// The chance a random build has no secondary profession.
const NO_SECONDARY: f64 = 0.1;

/// Replaces a free slot's unlocked parts with a random legal build.
pub fn randomise(free: &mut FreeGenome, ctx: &SlotContext, data: &DataSet, rng: &mut OptRng) {
    let locks = free.locks;
    let build = &mut free.build;
    if !locks.gear && !locks.secondary && ctx.pools.primaries.len() > 1 {
        build.primary = *rng.pick(&ctx.pools.primaries).unwrap_or(&build.primary);
    }
    if !locks.secondary {
        let primary = build.primary;
        let others: Vec<Profession> = Profession::ALL
            .into_iter()
            .filter(|p| *p != primary)
            .collect();
        build.secondary = if rng.chance(NO_SECONDARY) {
            None
        } else {
            rng.pick(&others).copied()
        };
    }
    let mut pool = ctx.pools.skills_for(data, build.primary, build.secondary);
    let mut elite = false;
    let mut pve_only = 0;
    for position in 0..8 {
        if locks.skill(position) {
            if let Some(skill) = build.skills[position].and_then(|id| data.skill_by_id(id)) {
                elite |= skill.elite;
                pve_only += usize::from(skill.pve_only);
            }
            continue;
        }
        pool.retain(|id| {
            !build.skills.contains(&Some(*id))
                && data
                    .skill_by_id(*id)
                    .is_some_and(|s| (!s.elite || !elite) && (!s.pve_only || pve_only < 3))
        });
        let choice = rng.pick(&pool).copied();
        if let Some(skill) = choice.and_then(|id| data.skill_by_id(id)) {
            elite |= skill.elite;
            pve_only += usize::from(skill.pve_only);
        }
        build.skills[position] = choice;
    }
    if !locks.gear {
        let runes = ctx.pools.runes_for(data, build.primary);
        let insignias = ctx.pools.insignias_for(data, build.primary);
        for piece in build.armor.iter_mut() {
            piece.rune = rng.pick(&runes).map(|r| r.slug.clone());
            piece.insignia = rng.pick(&insignias).cloned();
        }
        build.headgear_attribute = rng.pick(build.primary.attributes()).copied();
    }
    repair(free, &ctx.pools, ctx.role, data, rng, true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genome::{LockMask, PartyGenome, SlotGenome};
    use crate::pools::{PoolOptions, SlotPools};
    use crate::roles::SlotRole;
    use gwsim_data::build::SlotKind;
    use gwsim_data::source::DirSource;
    use std::collections::BTreeSet;
    use std::path::Path;

    #[test]
    fn a_thousand_random_player_builds_are_legal_and_varied() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let data = DataSet::load(&DirSource::new(dir)).unwrap();
        let party = data.party(&"m1-mesmerway".parse().unwrap()).unwrap();
        let genome = PartyGenome::from_party(party, &[(0, LockMask::default())]);
        let SlotGenome::Free(template) = genome.slots[0].clone() else {
            unreachable!()
        };
        let ctx = SlotContext {
            pools: SlotPools::for_slot(&party.slots[0], &data, &PoolOptions::default()),
            role: SlotRole::Interrupt,
        };
        let mut rng = OptRng::new(2026);
        let mut professions = BTreeSet::new();
        let mut elites = BTreeSet::new();
        for _ in 0..1000 {
            let mut free = template.clone();
            randomise(&mut free, &ctx, &data, &mut rng);
            let problems = free.build.check(&data, SlotKind::Human);
            assert!(problems.is_empty(), "{problems:?}");
            professions.insert((free.build.primary, free.build.secondary));
            for id in free.build.skills.iter().flatten() {
                if data.skill_by_id(*id).unwrap().elite {
                    elites.insert(*id);
                }
            }
        }
        assert!(
            professions.len() > 30,
            "{} profession pairs",
            professions.len()
        );
        assert!(elites.len() > 10, "{} elites", elites.len());
    }
}
