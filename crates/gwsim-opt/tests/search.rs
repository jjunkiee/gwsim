//! T5.2.7 and T5.5.3: the search on a synthetic problem with a known front,
//! determinism, and exhaustive mode against brute force.
//!
//! The evaluator is fake and cheap: a bar's clear time and deaths are sums
//! of fixed per-skill numbers, and one skill makes a bar fail half its runs.
//! The space is small (three open positions from the player's pool), so the
//! true front can be found by brute force and compared.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use gwsim_data::dataset::DataSet;
use gwsim_data::foe::SkillRef;
use gwsim_data::ids::SkillId;
use gwsim_data::party::PartyFile;
use gwsim_data::source::DirSource;
use gwsim_engine::RunSeed;
use gwsim_engine::rng::splitmix64;
use gwsim_opt::evaluate::{Evaluator, RunOutcome, Stages};
use gwsim_opt::exhaustive::{self, PoolFile, Refusal};
use gwsim_opt::genome::LockMask;
use gwsim_opt::objectives::{Goal, Objective, Scoring, ThresholdRule, dominates};
use gwsim_opt::operators::SlotContext;
use gwsim_opt::pools::{PoolOptions, SlotPools};
use gwsim_opt::roles::SlotRole;
use gwsim_opt::search::{self, CancelToken, Config, Controls, Outcome, Problem};

fn data() -> &'static DataSet {
    static DATA: OnceLock<DataSet> = OnceLock::new();
    DATA.get_or_init(|| {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        DataSet::load(&DirSource::new(dir)).unwrap()
    })
}

fn party() -> PartyFile {
    data()
        .party(&"m1-mesmerway".parse().unwrap())
        .unwrap()
        .clone()
}

/// Per-skill numbers from the id, fixed and arbitrary.
fn numbers(id: SkillId) -> (f64, f64) {
    let mut state = u64::from(id.get());
    let a = (splitmix64(&mut state) % 20) as f64;
    let b = (splitmix64(&mut state) % 20) as f64;
    (a, b)
}

/// The skill that makes a bar fail half its runs.
const POISON: u16 = 67; // Shatter Hex

struct Synthetic;

impl Synthetic {
    fn objectives(party: &PartyFile) -> (f64, f64, bool) {
        let bar = &party.slots[0].build.skills;
        let (mut clear, mut deaths) = (0.0, 0.0);
        for id in bar.iter().skip(5).flatten() {
            let (a, b) = numbers(*id);
            clear += a;
            deaths += b;
        }
        let poisoned = bar.iter().flatten().any(|id| id.get() == POISON);
        (clear, deaths, poisoned)
    }
}

impl Evaluator for Synthetic {
    fn situations(&self) -> usize {
        1
    }

    fn run(
        &self,
        party: &PartyFile,
        _: usize,
        seeds: &[RunSeed],
    ) -> Result<Vec<RunOutcome>, String> {
        let (clear, deaths, poisoned) = Synthetic::objectives(party);
        Ok(seeds
            .iter()
            .map(|seed| {
                let won = !poisoned || seed.0 % 2 == 0;
                RunOutcome {
                    won,
                    clear_ms: won.then_some((clear * 1000.0) as u32 + 1000),
                    deaths: deaths as u32,
                    damage_taken: 0,
                    energy_left: 0,
                    dp_end: 0,
                }
            })
            .collect())
    }
}

/// The player free, with five positions and everything else locked, so
/// only positions 6–8 are searched.
fn problem(base: &PartyFile) -> Problem<'static> {
    let locks = LockMask {
        skills: 0b0001_1111,
        gear: true,
        attributes: true,
        secondary: true,
    };
    let mut pools = SlotPools::for_slot(&base.slots[0], data(), &PoolOptions::default());
    pools.primaries = vec![base.slots[0].build.primary];
    let contexts = BTreeMap::from([(
        0,
        SlotContext {
            pools,
            role: SlotRole::Interrupt,
        },
    )]);
    Problem {
        data: data(),
        base: base.clone(),
        free: vec![(0, locks)],
        contexts,
        seeds: Vec::new(),
    }
}

fn config(seed: u64) -> Config {
    let scoring = Scoring {
        objectives: vec![Objective::ClearTime, Objective::Deaths],
        goal: Goal::default(),
        threshold: 0.95,
        rule: ThresholdRule::EverySituation,
        weights: vec![1.0],
    };
    let mut config = Config::new(scoring, seed);
    config.population = 24;
    config.max_generations = Some(25);
    config.stagnation = 25;
    config.stages = Stages {
        first: 4,
        second: 8,
        frontier: 16,
    };
    config
}

fn run(seed: u64) -> Outcome {
    let base = party();
    let problem = problem(&base);
    let mut snapshots = 0;
    let mut on_snapshot = |_: &search::Snapshot| snapshots += 1;
    let outcome = search::optimise(
        &problem,
        &Synthetic,
        config(seed),
        Controls {
            should_stop: &|| false,
            cancel: CancelToken::new(),
            on_snapshot: &mut on_snapshot,
            on_progress: &|_| {},
        },
    )
    .unwrap();
    assert!(snapshots >= 2, "a snapshot after every generation");
    outcome
}

/// The true front over positions 6–8, by brute force: (clear, deaths).
fn true_front() -> BTreeSet<(i64, i64)> {
    let base = party();
    let slot = &base.slots[0];
    let pools = SlotPools::for_slot(slot, data(), &PoolOptions::default());
    let fixed: Vec<SkillId> = slot
        .build
        .skills
        .iter()
        .take(5)
        .flatten()
        .copied()
        .collect();
    let has_elite = fixed
        .iter()
        .any(|id| data().skill_by_id(*id).unwrap().elite);
    let pve_fixed = fixed
        .iter()
        .filter(|id| data().skill_by_id(**id).unwrap().pve_only)
        .count();
    let candidates: Vec<SkillId> = pools
        .skills_for(data(), slot.build.primary, slot.build.secondary)
        .into_iter()
        .filter(|id| !fixed.contains(id) && id.get() != POISON)
        .collect();
    let mut points = Vec::new();
    for i in 0..candidates.len() {
        for j in i + 1..candidates.len() {
            for k in j + 1..candidates.len() {
                let trio = [candidates[i], candidates[j], candidates[k]];
                let skills: Vec<_> = trio
                    .iter()
                    .map(|id| data().skill_by_id(*id).unwrap())
                    .collect();
                let elites = skills.iter().filter(|s| s.elite).count() + usize::from(has_elite);
                let pve = skills.iter().filter(|s| s.pve_only).count() + pve_fixed;
                if elites > 1 || pve > 3 {
                    continue;
                }
                let (a, b) = trio
                    .iter()
                    .map(|id| numbers(*id))
                    .fold((0.0, 0.0), |s, v| (s.0 + v.0, s.1 + v.1));
                points.push((a as i64, b as i64));
            }
        }
    }
    points
        .iter()
        .copied()
        .filter(|p| !points.iter().any(|q| q.0 <= p.0 && q.1 <= p.1 && q != p))
        .collect()
}

#[test]
fn the_search_finds_most_of_a_known_front() {
    let outcome = run(1);
    let truth = true_front();
    let found: BTreeSet<(i64, i64)> = outcome
        .frontier
        .iter()
        .map(|c| {
            (
                (c.score.objectives[0] - 1.0).round() as i64,
                c.score.objectives[1].round() as i64,
            )
        })
        .collect();
    let hit = truth.intersection(&found).count();
    assert!(
        hit * 10 >= truth.len() * 7,
        "found {hit} of {} front points: {found:?} vs {truth:?}",
        truth.len()
    );
    // Nothing on the reported frontier is dominated by anything else found.
    for a in &outcome.frontier {
        assert!(
            a.score.feasible,
            "the poisoned skill must not reach the frontier"
        );
        for b in &outcome.frontier {
            assert!(!dominates(&b.score, &a.score));
        }
    }
}

#[test]
fn the_same_seed_gives_the_same_frontier() {
    let a = run(7);
    let b = run(7);
    assert_eq!(a, b);
}

#[test]
fn a_cancelled_search_keeps_its_frontier() {
    let base = party();
    let problem = problem(&base);
    let cancel = CancelToken::new();
    let token = cancel.clone();
    let mut on_snapshot = |_: &search::Snapshot| token.cancel();
    let outcome = search::optimise(
        &problem,
        &Synthetic,
        config(3),
        Controls {
            should_stop: &|| false,
            cancel,
            on_snapshot: &mut on_snapshot,
            on_progress: &|_| {},
        },
    )
    .unwrap();
    assert!(outcome.interrupted());
    assert!(!outcome.ranked.is_empty());
}

#[test]
fn a_budget_stops_the_search() {
    let base = party();
    let problem = problem(&base);
    let outcome = search::optimise(
        &problem,
        &Synthetic,
        config(3),
        Controls {
            should_stop: &|| true,
            cancel: CancelToken::new(),
            on_snapshot: &mut |_| {},
            on_progress: &|_| {},
        },
    )
    .unwrap();
    assert_eq!(outcome.stop, search::StopReason::Budget);
    assert_eq!(outcome.generations, 0);
}

fn pool(k: usize, skills: &[u16]) -> PoolFile {
    PoolFile {
        slot: "player".into(),
        positions: Vec::new(),
        k: Some(k),
        skills: skills.iter().map(|id| SkillRef::Id(SkillId(*id))).collect(),
    }
}

#[test]
fn exhaustive_mode_equals_brute_force() {
    // Three from eight Mesmer and common skills not already on the bar's
    // first five positions.
    let base = party();
    let problem = problem(&base);
    let spec = pool(3, &[23, 25, 67, 68, 69, 1336, 1345, 2416]);
    let outcome = exhaustive::run(
        &problem,
        &Synthetic,
        config(1),
        &spec,
        exhaustive::DEFAULT_LIMIT,
        Controls {
            should_stop: &|| false,
            cancel: CancelToken::new(),
            on_snapshot: &mut |_| {},
            on_progress: &|_| {},
        },
    )
    .unwrap();
    // Brute force with the synthetic numbers.
    let fixed: Vec<SkillId> = base.slots[0]
        .build
        .skills
        .iter()
        .take(5)
        .flatten()
        .copied()
        .collect();
    let resolved: Vec<SkillId> = [23u16, 25, 67, 68, 69, 1336, 1345, 2416]
        .iter()
        .map(|i| SkillId(*i))
        .filter(|id| data().skill_by_id(*id).is_some() && !fixed.contains(id))
        .collect();
    let mut best = f64::INFINITY;
    for i in 0..resolved.len() {
        for j in i + 1..resolved.len() {
            for k in j + 1..resolved.len() {
                let mut bar = base.clone();
                bar.slots[0].build.skills[5] = Some(resolved[i]);
                bar.slots[0].build.skills[6] = Some(resolved[j]);
                bar.slots[0].build.skills[7] = Some(resolved[k]);
                if !bar.slots[0]
                    .build
                    .is_legal(data(), gwsim_data::build::SlotKind::Human)
                {
                    continue;
                }
                let (clear, _, poisoned) = Synthetic::objectives(&bar);
                if !poisoned {
                    best = best.min(clear + 1.0);
                }
            }
        }
    }
    let top = &outcome.ranked[0];
    assert!(
        (top.score.goal - best).abs() < 1e-6,
        "{} vs {best}",
        top.score.goal
    );
    assert_eq!(outcome.stop, search::StopReason::Exhausted);
}

#[test]
fn an_oversized_pool_is_refused_with_its_count() {
    let base = party();
    let problem = problem(&base);
    let everything: Vec<u16> = data().skills.values().map(|e| e.value.id.get()).collect();
    let spec = pool(3, &everything);
    let refusal = exhaustive::run(
        &problem,
        &Synthetic,
        config(1),
        &spec,
        100,
        Controls {
            should_stop: &|| false,
            cancel: CancelToken::new(),
            on_snapshot: &mut |_| {},
            on_progress: &|_| {},
        },
    )
    .unwrap_err();
    assert!(matches!(refusal, Refusal::TooMany { .. }));
    assert!(refusal.to_string().contains("evolutionary"));
}
