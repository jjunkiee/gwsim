//! Exhaustive mode: every combination from a small pool (WP5.5, §13.3,
//! UC8).
//!
//! A pool file names a slot, the bar positions to fill (or how many), and
//! the candidate skills. Every combination is enumerated in lexicographic
//! order with everything else fixed, illegal ones (a second elite, too many
//! PvE-only skills, a skill the slot's professions cannot carry, a duplicate
//! of a skill already on the bar) are dropped, and the rest go through the
//! same staged evaluation as the evolutionary search. **A run over the limit
//! is refused**, with the count and a suggestion to use evolutionary mode.

use gwsim_data::dataset::DataSet;
use gwsim_data::foe::SkillRef;
use gwsim_data::ids::SkillId;
use serde::{Deserialize, Serialize};

use crate::evaluate::Evaluator;
use crate::genome::{PartyGenome, SlotGenome};
use crate::search::{Config, Controls, Engine, Origin, Outcome, Problem, StopReason};

/// The most combinations exhaustive mode will run (§13.3) [Proposed].
pub const DEFAULT_LIMIT: usize = 50_000;

/// A pool file (T5.5.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolFile {
    /// The slot, by name.
    pub slot: String,
    /// Bar positions to fill, counted from 1. Without them, the last `k`.
    #[serde(default)]
    pub positions: Vec<usize>,
    #[serde(default)]
    pub k: Option<usize>,
    /// The candidate skills, by slug, name or id.
    pub skills: Vec<SkillRef>,
}

/// Why exhaustive mode would not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    TooMany { count: u128, limit: usize },
    Invalid(String),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::TooMany { count, limit } => write!(
                f,
                "{count} legal combinations is over the limit of {limit}; narrow the pool, or use \
                 --mode evolutionary, which samples the space instead of enumerating it"
            ),
            Refusal::Invalid(message) => write!(f, "{message}"),
        }
    }
}

/// n choose k, saturating.
pub fn choose(n: usize, k: usize) -> u128 {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    let mut result: u128 = 1;
    for i in 0..k {
        result = result.saturating_mul((n - i) as u128) / (i as u128 + 1);
    }
    result
}

/// Resolves a pool's skills and positions against a slot's build.
pub fn resolve(
    pool: &PoolFile,
    data: &DataSet,
    bar: &[Option<SkillId>; 8],
) -> Result<(Vec<usize>, Vec<SkillId>), Refusal> {
    let mut skills = Vec::new();
    for reference in &pool.skills {
        let skill = match reference {
            SkillRef::Id(id) => data.skill_by_id(*id),
            SkillRef::Slug(slug) => data.skill(slug),
        }
        .ok_or_else(|| Refusal::Invalid(format!("no skill {reference:?} in the data")))?;
        if !skills.contains(&skill.id) {
            skills.push(skill.id);
        }
    }
    skills.sort();
    let positions: Vec<usize> = if pool.positions.is_empty() {
        let k = pool
            .k
            .ok_or_else(|| Refusal::Invalid("give positions or k".to_owned()))?;
        if k == 0 || k > 8 {
            return Err(Refusal::Invalid(format!("k must be 1 to 8, not {k}")));
        }
        (8 - k..8).collect()
    } else {
        let mut positions = Vec::new();
        for p in &pool.positions {
            if *p == 0 || *p > 8 {
                return Err(Refusal::Invalid(format!("position {p} is not 1 to 8")));
            }
            positions.push(p - 1);
        }
        positions.sort();
        positions.dedup();
        positions
    };
    // A candidate already on the bar outside the filled positions would be
    // a duplicate.
    skills.retain(|id| {
        !bar.iter()
            .enumerate()
            .any(|(p, s)| !positions.contains(&p) && *s == Some(*id))
    });
    Ok((positions, skills))
}

/// Every legal combination, in lexicographic order, or a refusal.
pub fn enumerate(
    free: &crate::genome::FreeGenome,
    positions: &[usize],
    skills: &[SkillId],
    data: &DataSet,
    limit: usize,
) -> Result<Vec<Vec<SkillId>>, Refusal> {
    let k = positions.len();
    let raw = choose(skills.len(), k);
    // Enumerating to count the legal ones is cheap only up to a point.
    if raw > (limit as u128).saturating_mul(10) {
        return Err(Refusal::TooMany { count: raw, limit });
    }
    let mut out = Vec::new();
    let mut index: Vec<usize> = (0..k).collect();
    if k == 0 || k > skills.len() {
        return Ok(out);
    }
    loop {
        let combination: Vec<SkillId> = index.iter().map(|i| skills[*i]).collect();
        let mut build = free.build.clone();
        for (position, id) in positions.iter().zip(&combination) {
            build.skills[*position] = Some(*id);
        }
        let legal = build.check(data, free.kind).into_iter().all(|p| {
            // Attribute points and gear are fixed; only the bar is judged.
            !matches!(
                p,
                gwsim_data::build::LegalityError::DuplicateSkill(_)
                    | gwsim_data::build::LegalityError::TooManyElites(_)
                    | gwsim_data::build::LegalityError::TooManyPveOnly(_)
                    | gwsim_data::build::LegalityError::PveOnlyOnHero(_)
                    | gwsim_data::build::LegalityError::SkillNotAvailable { .. }
                    | gwsim_data::build::LegalityError::UnknownSkill(_)
            )
        });
        if legal {
            out.push(combination);
            if out.len() > limit {
                return Err(Refusal::TooMany { count: raw, limit });
            }
        }
        // Next combination: bump the rightmost index that can move.
        let n = skills.len();
        let Some(i) = (0..k).rev().find(|i| index[*i] < n - k + i) else {
            return Ok(out);
        };
        index[i] += 1;
        for j in i + 1..k {
            index[j] = index[j - 1] + 1;
        }
    }
}

/// Runs exhaustive mode on one free slot: enumerate, then evaluate every
/// combination through the stages.
pub fn run(
    problem: &Problem,
    evaluator: &dyn Evaluator,
    config: Config,
    pool: &PoolFile,
    limit: usize,
    controls: Controls,
) -> Result<Outcome, Refusal> {
    let slot = problem
        .base
        .slots
        .iter()
        .position(|s| s.name == pool.slot)
        .ok_or_else(|| Refusal::Invalid(format!("no slot {:?} in the party", pool.slot)))?;
    let genome = PartyGenome::from_party(&problem.base, &problem.free);
    let SlotGenome::Free(free) = &genome.slots[slot] else {
        return Err(Refusal::Invalid(format!(
            "slot {:?} is not free",
            pool.slot
        )));
    };
    let (positions, skills) = resolve(pool, problem.data, &free.build.skills)?;
    let combinations = enumerate(free, &positions, &skills, problem.data, limit)?;

    let mut engine = Engine::new(problem, evaluator, config);
    let mut hashes = Vec::new();
    for (n, combination) in combinations.iter().enumerate() {
        let mut candidate = genome.clone();
        if let SlotGenome::Free(f) = &mut candidate.slots[slot] {
            for (position, id) in positions.iter().zip(combination) {
                f.build.skills[*position] = Some(*id);
            }
        }
        hashes.push(engine.remember(&candidate, &Origin::Exhaustive(n)));
    }
    hashes.dedup();
    engine
        .evaluate(&hashes, controls.on_progress, 0)
        .map_err(Refusal::Invalid)?;
    let stop = if controls.cancel.is_cancelled() {
        StopReason::Cancelled
    } else {
        StopReason::Exhausted
    };
    Ok(engine.outcome(0, stop))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choose_counts_combinations() {
        assert_eq!(choose(8, 3), 56);
        assert_eq!(choose(20, 3), 1140);
        assert_eq!(choose(3, 5), 0);
        assert_eq!(choose(72, 8), 11_969_016_345);
    }
}
