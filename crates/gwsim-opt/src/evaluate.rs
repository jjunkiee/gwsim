//! Adaptive evaluation: common random numbers, successive halving, a cache
//! and parallel runs (WP5.3, §13.4, ENG-42).
//!
//! - **CRN.** Every candidate runs every situation on the same seed list, so
//!   run `i` of any two candidates faces the same random numbers and their
//!   comparison is paired.
//! - **Stages** [Proposed]: every candidate gets 16 runs per situation; the
//!   better half of a generation 64; the frontier 256. A candidate is held
//!   back from promotion only when it is *significantly* short of the
//!   threshold: its Wilson interval lies wholly below it in some situation.
//! - **Cache.** Runs are kept per (canonical candidate hash, situation);
//!   extending 16 runs to 64 reuses the first 16. The seed list and data
//!   pack are fixed for a search, so they are part of the cache's identity
//!   rather than its key. The cache is bounded and evicts the least recently
//!   used entry.
//! - **Parallelism.** The (candidate, situation) jobs run on all cores with
//!   `rayon`; results are stored in job order, so nothing depends on
//!   scheduling.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use gwsim_data::core::CoreData;
use gwsim_data::core::TitleTrack;
use gwsim_data::dataset::DataSet;
use gwsim_data::party::PartyFile;
use gwsim_data::scenario::Situation;
use gwsim_engine::harness::wilson;
use gwsim_engine::{FightSetup, RunSeed, SeedList};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::objectives::SituationStats;

/// What one run contributes to the objectives.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RunOutcome {
    pub won: bool,
    pub clear_ms: Option<u32>,
    pub deaths: u32,
    pub damage_taken: i64,
    pub energy_left: i32,
    pub dp_end: u8,
}

/// Runs parties in situations. The engine is one implementation; tests use
/// cheap synthetic ones.
pub trait Evaluator: Sync {
    /// How many situations there are.
    fn situations(&self) -> usize;

    /// Runs a party in one situation on each seed, in order.
    fn run(
        &self,
        party: &PartyFile,
        situation: usize,
        seeds: &[RunSeed],
    ) -> Result<Vec<RunOutcome>, String>;
}

/// The engine as an evaluator.
pub struct EngineEvaluator<'a> {
    pub data: &'a DataSet,
    pub core: &'a CoreData,
    pub situations: Vec<Situation>,
    /// Title ranks from the account profile; empty means every track at its
    /// maximum.
    pub title_ranks: BTreeMap<TitleTrack, u8>,
}

impl Evaluator for EngineEvaluator<'_> {
    fn situations(&self) -> usize {
        self.situations.len()
    }

    fn run(
        &self,
        party: &PartyFile,
        situation: usize,
        seeds: &[RunSeed],
    ) -> Result<Vec<RunOutcome>, String> {
        let situation = &self.situations[situation];
        let mut setup =
            FightSetup::new(self.data, self.core, party, situation).map_err(|e| e.to_string())?;
        let ranks = if situation.mode.melandrus_accord {
            TitleTrack::ALL.into_iter().map(|t| (t, 0)).collect()
        } else {
            self.title_ranks.clone()
        };
        if !ranks.is_empty() {
            setup = setup.with_title_ranks(ranks);
        }
        Ok(seeds
            .iter()
            .map(|seed| {
                let r = setup.run(*seed);
                RunOutcome {
                    won: r.won(),
                    clear_ms: r.clear_time_ms,
                    deaths: r.deaths,
                    damage_taken: r.damage_taken,
                    energy_left: r.energy_left,
                    dp_end: r.dp_end,
                }
            })
            .collect())
    }
}

/// Summarises runs for the objectives.
pub fn stats_of(runs: &[RunOutcome]) -> SituationStats {
    let n = runs.len();
    let wins = runs.iter().filter(|r| r.won).count();
    let mean = |f: &dyn Fn(&RunOutcome) -> f64| {
        if n == 0 {
            0.0
        } else {
            runs.iter().map(f).sum::<f64>() / n as f64
        }
    };
    let clears: Vec<f64> = runs
        .iter()
        .filter_map(|r| r.clear_ms)
        .map(|ms| f64::from(ms) / 1000.0)
        .collect();
    let interval = wilson(wins, n);
    SituationStats {
        runs: n,
        wins,
        win_rate: if n == 0 { 0.0 } else { wins as f64 / n as f64 },
        win_low: interval.low,
        win_high: interval.high,
        clear_time_s: (!clears.is_empty())
            .then(|| clears.iter().sum::<f64>() / clears.len() as f64),
        deaths: mean(&|r| f64::from(r.deaths)),
        damage_taken: mean(&|r| r.damage_taken as f64),
        energy_left: mean(&|r| f64::from(r.energy_left)),
        dp_end: mean(&|r| f64::from(r.dp_end)),
    }
}

/// Run counts per stage (T5.3.1) [Proposed].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stages {
    pub first: usize,
    pub second: usize,
    pub frontier: usize,
}

impl Default for Stages {
    fn default() -> Self {
        Stages {
            first: 16,
            second: 64,
            frontier: 256,
        }
    }
}

/// Progress, for the command line's status line and the desktop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Progress {
    pub generation: usize,
    pub runs_done: usize,
    pub runs_planned: usize,
    pub candidates: usize,
}

/// The default cache bound, in (candidate, situation) entries.
pub const CACHE_ENTRIES: usize = 50_000;

/// Cached runs, keyed by (candidate hash, situation).
#[derive(Debug, Default)]
pub struct Cache {
    entries: BTreeMap<(u128, usize), (Vec<RunOutcome>, u64)>,
    clock: u64,
    capacity: usize,
    /// Runs actually simulated, as opposed to served from the cache.
    pub runs_simulated: usize,
}

impl Cache {
    pub fn new(capacity: usize) -> Cache {
        Cache {
            capacity: capacity.max(1),
            ..Cache::default()
        }
    }

    /// The runs held for a candidate in a situation.
    pub fn runs(&self, hash: u128, situation: usize) -> &[RunOutcome] {
        self.entries
            .get(&(hash, situation))
            .map(|(runs, _)| runs.as_slice())
            .unwrap_or(&[])
    }

    fn touch(&mut self, key: (u128, usize)) {
        self.clock += 1;
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.1 = self.clock;
        }
    }

    fn extend(&mut self, key: (u128, usize), runs: Vec<RunOutcome>) {
        self.clock += 1;
        let clock = self.clock;
        let entry = self
            .entries
            .entry(key)
            .or_insert_with(|| (Vec::new(), clock));
        entry.0.extend(runs);
        entry.1 = clock;
        while self.entries.len() > self.capacity {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(k, (_, used))| (*used, **k))
                .map(|(k, _)| *k);
            match oldest {
                Some(k) => {
                    self.entries.remove(&k);
                }
                None => break,
            }
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Brings candidates to at least `runs` runs in every situation, simulating
/// only what the cache lacks, in parallel.
pub fn ensure(
    evaluator: &dyn Evaluator,
    cache: &mut Cache,
    seeds: &SeedList,
    candidates: &[(u128, &PartyFile)],
    runs: usize,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<(), String> {
    let mut jobs: Vec<(u128, &PartyFile, usize, usize)> = Vec::new();
    let mut seen: Vec<u128> = Vec::new();
    for (hash, party) in candidates {
        if seen.contains(hash) {
            continue;
        }
        seen.push(*hash);
        for situation in 0..evaluator.situations() {
            let have = cache.runs(*hash, situation).len();
            if have < runs {
                jobs.push((*hash, party, situation, have));
            } else {
                cache.touch((*hash, situation));
            }
        }
    }
    let planned: usize = jobs.iter().map(|(_, _, _, have)| runs - have).sum();
    let done = AtomicUsize::new(0);
    let all = SeedList::new(seeds.master, runs.max(seeds.len()));
    let results: Vec<Result<Vec<RunOutcome>, String>> = jobs
        .par_iter()
        .map(|(_, party, situation, have)| {
            let out = evaluator.run(party, *situation, &all.seeds()[*have..runs]);
            let now = done.fetch_add(runs - have, Ordering::Relaxed) + (runs - have);
            progress(now, planned);
            out
        })
        .collect();
    for ((hash, _, situation, _), result) in jobs.into_iter().zip(results) {
        let outcomes = result?;
        cache.runs_simulated += outcomes.len();
        cache.extend((hash, situation), outcomes);
    }
    Ok(())
}

/// Per-situation statistics from whatever runs the cache holds.
pub fn stats(evaluator: &dyn Evaluator, cache: &Cache, hash: u128) -> Vec<SituationStats> {
    (0..evaluator.situations())
        .map(|s| stats_of(cache.runs(hash, s)))
        .collect()
}

/// Whether a candidate is significantly short of the threshold somewhere:
/// its Wilson interval lies wholly below it (T5.3.1).
pub fn significantly_short(stats: &[SituationStats], weights: &[f64], threshold: f64) -> bool {
    stats
        .iter()
        .zip(weights)
        .any(|(s, w)| *w > 0.0 && s.runs > 0 && s.win_high < threshold)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use gwsim_engine::rng::splitmix64;

    /// A synthetic evaluator: each "party" wins with the probability written
    /// in its name, deterministically per seed.
    pub struct Toy;

    pub fn win_rate(party: &PartyFile) -> f64 {
        party.name.parse().unwrap_or(0.0)
    }

    impl Evaluator for Toy {
        fn situations(&self) -> usize {
            2
        }

        fn run(
            &self,
            party: &PartyFile,
            situation: usize,
            seeds: &[RunSeed],
        ) -> Result<Vec<RunOutcome>, String> {
            let p = win_rate(party);
            Ok(seeds
                .iter()
                .map(|seed| {
                    let mut state = seed.0 ^ situation as u64;
                    let roll = (splitmix64(&mut state) >> 11) as f64 / (1u64 << 53) as f64;
                    RunOutcome {
                        won: roll < p,
                        clear_ms: (roll < p).then_some(40_000),
                        deaths: 0,
                        damage_taken: 0,
                        energy_left: 0,
                        dp_end: 0,
                    }
                })
                .collect())
        }
    }

    pub fn party(win_rate: f64) -> PartyFile {
        PartyFile {
            name: win_rate.to_string(),
            notes: String::new(),
            sources: Vec::new(),
            benchmarks: Vec::new(),
            slots: Vec::new(),
            tactics: None,
        }
    }

    #[test]
    fn extending_reuses_the_cached_prefix() {
        let seeds = SeedList::new(1, 64);
        let mut cache = Cache::new(100);
        let p = party(0.5);
        ensure(&Toy, &mut cache, &seeds, &[(1, &p)], 16, &|_, _| {}).unwrap();
        assert_eq!(cache.runs_simulated, 32);
        ensure(&Toy, &mut cache, &seeds, &[(1, &p)], 16, &|_, _| {}).unwrap();
        assert_eq!(cache.runs_simulated, 32, "a second look costs nothing");
        let first16: Vec<_> = cache.runs(1, 0).to_vec();
        ensure(&Toy, &mut cache, &seeds, &[(1, &p)], 64, &|_, _| {}).unwrap();
        assert_eq!(cache.runs_simulated, 32 + 96);
        assert_eq!(&cache.runs(1, 0)[..16], first16.as_slice());
    }

    #[test]
    fn results_do_not_depend_on_the_thread_count() {
        let seeds = SeedList::new(9, 64);
        let parties: Vec<PartyFile> = (1..=8).map(|i| party(f64::from(i) / 10.0)).collect();
        let candidates: Vec<(u128, &PartyFile)> = parties
            .iter()
            .enumerate()
            .map(|(i, p)| (i as u128, p))
            .collect();
        let run = |threads: usize| {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            pool.install(|| {
                let mut cache = Cache::new(1000);
                ensure(&Toy, &mut cache, &seeds, &candidates, 64, &|_, _| {}).unwrap();
                (0..8u128)
                    .map(|h| stats(&Toy, &cache, h))
                    .collect::<Vec<_>>()
            })
        };
        assert_eq!(run(1), run(6));
    }

    #[test]
    fn the_cache_evicts_the_least_recently_used() {
        let seeds = SeedList::new(1, 16);
        let mut cache = Cache::new(4);
        let a = party(0.5);
        ensure(
            &Toy,
            &mut cache,
            &seeds,
            &[(1, &a), (2, &a)],
            16,
            &|_, _| {},
        )
        .unwrap();
        ensure(&Toy, &mut cache, &seeds, &[(1, &a)], 16, &|_, _| {}).unwrap();
        ensure(&Toy, &mut cache, &seeds, &[(3, &a)], 16, &|_, _| {}).unwrap();
        assert!(cache.runs(2, 0).is_empty(), "2 was used least recently");
        assert_eq!(cache.runs(1, 0).len(), 16);
        assert_eq!(cache.len(), 4);
    }

    #[test]
    fn paired_differences_vary_less_than_unpaired_ones() {
        // T5.3.4: two near-identical candidates on shared seeds differ far
        // less, run for run, than on independent seeds.
        let shared = SeedList::new(3, 400);
        let other = SeedList::new(4, 400);
        let a = Toy.run(&party(0.60), 0, shared.seeds()).unwrap();
        let b_paired = Toy.run(&party(0.62), 0, shared.seeds()).unwrap();
        let b_unpaired = Toy.run(&party(0.62), 0, other.seeds()).unwrap();
        let variance = |x: &[RunOutcome], y: &[RunOutcome]| {
            let d: Vec<f64> = x
                .iter()
                .zip(y)
                .map(|(p, q)| f64::from(u8::from(p.won)) - f64::from(u8::from(q.won)))
                .collect();
            let m = d.iter().sum::<f64>() / d.len() as f64;
            d.iter().map(|v| (v - m).powi(2)).sum::<f64>() / d.len() as f64
        };
        assert!(variance(&a, &b_paired) * 5.0 < variance(&a, &b_unpaired));
    }

    #[test]
    fn a_candidate_is_short_only_when_significantly_so() {
        let seeds = SeedList::new(1, 64);
        let mut cache = Cache::new(100);
        let strong = party(0.99);
        let weak = party(0.3);
        ensure(
            &Toy,
            &mut cache,
            &seeds,
            &[(1, &strong), (2, &weak)],
            16,
            &|_, _| {},
        )
        .unwrap();
        let weights = [1.0, 1.0];
        assert!(!significantly_short(
            &stats(&Toy, &cache, 1),
            &weights,
            0.95
        ));
        assert!(significantly_short(&stats(&Toy, &cache, 2), &weights, 0.95));
    }
}
