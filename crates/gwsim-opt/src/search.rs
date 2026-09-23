//! The NSGA-II search: generations, staged evaluation, seeding, stopping
//! and the anytime frontier (T5.2.5, T5.2.6, T5.6.4).
//!
//! Each generation: evaluate the offspring (16 runs, then 64 for the better
//! half, then 256 for the frontier), rank parents and offspring together
//! under constrained domination, and keep the best by the crowded
//! comparison. Offspring come from binary tournaments, crossover, mutation
//! and repair, and duplicates (by canonical hash) are dropped.
//!
//! **The crate reads no clock.** The caller stops a search through
//! [`Controls::should_stop`] (a time budget, say) or [`CancelToken`]; a
//! search capped by generations instead is exactly reproducible from its
//! seed.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gwsim_data::build::Build;
use gwsim_data::dataset::DataSet;
use gwsim_data::party::PartyFile;
use gwsim_engine::SeedList;
use serde::{Deserialize, Serialize};

use crate::evaluate::{self, Cache, Evaluator, Progress, Stages};
use crate::genome::{LockMask, PartyGenome, SlotGenome};
use crate::nsga;
use crate::objectives::{Score, Scoring};
use crate::operators::{self, Rates, SlotContext};
use crate::repair::repair;
use crate::rng::OptRng;

/// Where a candidate came from, for the reports (T5.2.6).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Origin {
    /// The party as given.
    Current,
    /// A benchmark build that fits the free slots.
    Benchmark(String),
    /// A heuristic bar for a role.
    Heuristic(String),
    /// Random legal fill.
    Random,
    /// Bred in this generation.
    Generation(usize),
    /// Enumerated by exhaustive mode.
    Exhaustive(usize),
}

/// A starting candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct Seed {
    pub genome: PartyGenome,
    pub origin: Origin,
}

/// What is being optimised.
pub struct Problem<'a> {
    pub data: &'a DataSet,
    /// The party the free slots live in.
    pub base: PartyFile,
    /// The free slots and what is locked inside them.
    pub free: Vec<(usize, LockMask)>,
    /// Per free slot: pools and role.
    pub contexts: BTreeMap<usize, SlotContext>,
    /// Starting candidates, besides the current party (always included).
    pub seeds: Vec<Seed>,
}

/// Search settings (§13.3 defaults, [Proposed]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub population: usize,
    pub stages: Stages,
    pub scoring: Scoring,
    pub seed: u64,
    /// Stop after this many generations (for reproducible runs and tests).
    pub max_generations: Option<usize>,
    /// Stop after this many generations without a new frontier member.
    pub stagnation: usize,
    pub rates: Rates,
    pub cache_entries: usize,
    /// How many ranked candidates to report.
    pub report: usize,
}

impl Config {
    /// The defaults for a scoring.
    pub fn new(scoring: Scoring, seed: u64) -> Config {
        Config {
            population: 64,
            stages: Stages::default(),
            scoring,
            seed,
            max_generations: None,
            stagnation: 8,
            rates: Rates::default(),
            cache_entries: evaluate::CACHE_ENTRIES,
            report: 20,
        }
    }
}

/// Stops a search cleanly from another thread (Ctrl-C).
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> CancelToken {
        CancelToken::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// How the caller steers a search.
pub struct Controls<'a> {
    /// Asked between generations: stop now (a time budget, say).
    pub should_stop: &'a (dyn Fn() -> bool + Sync),
    pub cancel: CancelToken,
    /// Called after every generation with the current frontier.
    pub on_snapshot: &'a mut (dyn FnMut(&Snapshot) + Send),
    /// Called as runs complete, from worker threads.
    pub on_progress: &'a (dyn Fn(Progress) + Sync),
}

/// Why a search stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    Budget,
    Stagnation,
    Generations,
    Cancelled,
    /// Exhaustive mode ran every combination.
    Exhausted,
}

/// A free slot's build in a report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlotBuild {
    pub slot: String,
    pub build: Build,
}

/// One candidate as reported (T5.6.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    /// The canonical hash, in hex.
    pub id: String,
    pub origin: Origin,
    pub builds: Vec<SlotBuild>,
    pub score: Score,
    /// Runs per situation behind the score (the evaluation depth).
    pub runs: usize,
}

/// The frontier as it stands after a generation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Snapshot {
    pub generation: usize,
    pub frontier: Vec<Candidate>,
    pub best: Option<Candidate>,
    pub candidates: usize,
    pub runs_simulated: usize,
}

/// What a search returns (T5.6.4).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Outcome {
    /// Non-dominated feasible candidates at full depth, best goal first.
    pub frontier: Vec<Candidate>,
    /// The best candidates by the goal: feasible first, then by depth.
    pub ranked: Vec<Candidate>,
    pub generations: usize,
    pub candidates: usize,
    pub runs_simulated: usize,
    pub stop: StopReason,
}

impl Outcome {
    /// Whether the search was cut short by a cancel.
    pub fn interrupted(&self) -> bool {
        self.stop == StopReason::Cancelled
    }
}

/// Everything evaluated so far.
struct Archive {
    genomes: BTreeMap<u128, (PartyGenome, Origin)>,
}

pub(crate) struct Engine<'p, 'e> {
    pub problem: &'p Problem<'p>,
    pub evaluator: &'e dyn Evaluator,
    pub config: Config,
    pub cache: Cache,
    pub seeds: SeedList,
    archive: Archive,
}

impl<'p, 'e> Engine<'p, 'e> {
    pub fn new(problem: &'p Problem<'p>, evaluator: &'e dyn Evaluator, config: Config) -> Self {
        let seeds = SeedList::new(config.seed, config.stages.frontier);
        Engine {
            problem,
            evaluator,
            cache: Cache::new(config.cache_entries),
            seeds,
            config,
            archive: Archive {
                genomes: BTreeMap::new(),
            },
        }
    }

    pub(crate) fn remember(&mut self, genome: &PartyGenome, origin: &Origin) -> u128 {
        let hash = genome.hash();
        self.archive
            .genomes
            .entry(hash)
            .or_insert_with(|| (genome.clone(), origin.clone()));
        hash
    }

    fn score(&self, hash: u128) -> Score {
        self.config
            .scoring
            .score(evaluate::stats(self.evaluator, &self.cache, hash))
    }

    fn depth(&self, hash: u128) -> usize {
        (0..self.evaluator.situations())
            .map(|s| self.cache.runs(hash, s).len())
            .min()
            .unwrap_or(0)
    }

    /// Brings hashes to `runs` runs.
    fn ensure(
        &mut self,
        hashes: &[u128],
        runs: usize,
        progress: &(dyn Fn(Progress) + Sync),
        generation: usize,
    ) -> Result<(), String> {
        let parties: Vec<(u128, PartyFile)> = hashes
            .iter()
            .map(|h| (*h, self.archive.genomes[h].0.to_party(&self.problem.base)))
            .collect();
        let refs: Vec<(u128, &PartyFile)> = parties.iter().map(|(h, p)| (*h, p)).collect();
        let candidates = self.archive.genomes.len();
        evaluate::ensure(
            self.evaluator,
            &mut self.cache,
            &self.seeds,
            &refs,
            runs,
            &|done, planned| {
                progress(Progress {
                    generation,
                    runs_done: done,
                    runs_planned: planned,
                    candidates,
                })
            },
        )
    }

    /// Staged evaluation of a set of candidates (T5.3.1): everyone to the
    /// first stage, the better half (unless significantly short) to the
    /// second, the frontier to the last.
    pub fn evaluate(
        &mut self,
        hashes: &[u128],
        progress: &(dyn Fn(Progress) + Sync),
        generation: usize,
    ) -> Result<(), String> {
        let stages = self.config.stages;
        self.ensure(hashes, stages.first, progress, generation)?;
        let (front, crowd) = self.rank(hashes);
        let order = nsga::select(&front, &crowd, hashes, hashes.len().div_ceil(2));
        let weights = self.config.scoring.weights.clone();
        let threshold = self.config.scoring.threshold;
        let promoted: Vec<u128> = order
            .into_iter()
            .map(|i| hashes[i])
            .filter(|h| {
                !evaluate::significantly_short(
                    &evaluate::stats(self.evaluator, &self.cache, *h),
                    &weights,
                    threshold,
                )
            })
            .collect();
        self.ensure(&promoted, stages.second, progress, generation)?;
        let (front, _) = self.rank(hashes);
        let frontier: Vec<u128> = hashes
            .iter()
            .zip(&front)
            .filter(|(_, f)| **f == 0)
            .map(|(h, _)| *h)
            .collect();
        self.ensure(&frontier, stages.frontier, progress, generation)
    }

    pub fn rank(&self, hashes: &[u128]) -> (Vec<usize>, Vec<f64>) {
        let scores: Vec<Score> = hashes.iter().map(|h| self.score(*h)).collect();
        let refs: Vec<&Score> = scores.iter().collect();
        nsga::rank(&refs, hashes)
    }

    pub fn candidate(&self, hash: u128) -> Candidate {
        let (genome, origin) = &self.archive.genomes[&hash];
        Candidate {
            id: format!("{hash:032x}"),
            origin: origin.clone(),
            builds: genome
                .free_slots()
                .into_iter()
                .map(|i| SlotBuild {
                    slot: self.problem.base.slots[i].name.clone(),
                    build: genome.slots[i].build().clone(),
                })
                .collect(),
            score: self.score(hash),
            runs: self.depth(hash),
        }
    }

    /// The frontier of everything evaluated at full depth, and the ranked
    /// list.
    pub fn outcome(&self, generations: usize, stop: StopReason) -> Outcome {
        let all: Vec<u128> = self.archive.genomes.keys().copied().collect();
        let deep: Vec<u128> = all
            .iter()
            .copied()
            .filter(|h| self.depth(*h) >= self.config.stages.frontier)
            .collect();
        let pool = if deep.is_empty() { all.clone() } else { deep };
        let (front, _) = self.rank(&pool);
        let mut frontier: Vec<Candidate> = pool
            .iter()
            .zip(&front)
            .filter(|(_, f)| **f == 0)
            .map(|(h, _)| self.candidate(*h))
            .filter(|c| c.score.feasible)
            .collect();
        frontier.sort_by(|a, b| a.score.goal.total_cmp(&b.score.goal).then(a.id.cmp(&b.id)));
        let mut ranked: Vec<Candidate> = all.iter().map(|h| self.candidate(*h)).collect();
        let full = self.config.stages.frontier;
        ranked.sort_by(|a, b| {
            b.score
                .feasible
                .cmp(&a.score.feasible)
                .then((b.runs >= full).cmp(&(a.runs >= full)))
                .then(a.score.violation.total_cmp(&b.score.violation))
                .then(a.score.goal.total_cmp(&b.score.goal))
                .then(a.id.cmp(&b.id))
        });
        ranked.truncate(self.config.report);
        Outcome {
            frontier,
            ranked,
            generations,
            candidates: self.archive.genomes.len(),
            runs_simulated: self.cache.runs_simulated,
            stop,
        }
    }

    fn snapshot(&self, generation: usize, population: &[u128]) -> Snapshot {
        let (front, _) = self.rank(population);
        let frontier: Vec<Candidate> = population
            .iter()
            .zip(&front)
            .filter(|(_, f)| **f == 0)
            .map(|(h, _)| self.candidate(*h))
            .collect();
        let best = frontier
            .iter()
            .filter(|c| c.score.feasible)
            .min_by(|a, b| a.score.goal.total_cmp(&b.score.goal))
            .cloned();
        Snapshot {
            generation,
            frontier,
            best,
            candidates: self.archive.genomes.len(),
            runs_simulated: self.cache.runs_simulated,
        }
    }

    /// Repairs every free slot of a genome.
    fn repair_all(&self, genome: &mut PartyGenome, reallocate: &[usize], rng: &mut OptRng) {
        for (index, slot) in genome.slots.iter_mut().enumerate() {
            if let SlotGenome::Free(free) = slot
                && let Some(ctx) = self.problem.contexts.get(&index)
            {
                repair(
                    free,
                    &ctx.pools,
                    ctx.role,
                    self.problem.data,
                    rng,
                    reallocate.contains(&index),
                );
            }
        }
    }

    /// One child from two parents.
    fn breed(&self, a: &PartyGenome, b: &PartyGenome, rng: &mut OptRng) -> PartyGenome {
        let free = a.free_slots();
        let mut reallocate = Vec::new();
        let mut child = if rng.chance(self.config.rates.crossover) {
            if free.len() > 1 && rng.chance(0.5) {
                operators::slot_crossover(a, b, rng)
            } else {
                let slot = *rng.pick(&free).unwrap_or(&0);
                reallocate.push(slot);
                operators::bar_crossover(a, b, slot, rng)
            }
        } else {
            a.clone()
        };
        let mutations = 1 + usize::from(rng.chance(self.config.rates.second_mutation));
        for _ in 0..mutations {
            let Some(&slot) = rng.pick(&free) else {
                break;
            };
            let Some(ctx) = self.problem.contexts.get(&slot) else {
                continue;
            };
            if let SlotGenome::Free(genome) = &mut child.slots[slot] {
                let mutation = operators::pick_mutation(&self.config.rates, rng);
                if operators::mutate(genome, mutation, ctx, self.problem.data, rng) == Some(true) {
                    reallocate.push(slot);
                }
            }
        }
        self.repair_all(&mut child, &reallocate, rng);
        child
    }

    /// Generation 0: the current party, the seeds, then random fill.
    fn initial(&mut self, rng: &mut OptRng) -> Vec<u128> {
        let mut population = Vec::new();
        let current = PartyGenome::from_party(&self.problem.base, &self.problem.free);
        let mut starts = vec![Seed {
            genome: current,
            origin: Origin::Current,
        }];
        starts.extend(self.problem.seeds.iter().cloned());
        for seed in starts {
            let mut genome = seed.genome;
            self.repair_all(&mut genome, &[], rng);
            let hash = self.remember(&genome, &seed.origin);
            if !population.contains(&hash) {
                population.push(hash);
            }
        }
        let mut attempts = 0;
        while population.len() < self.config.population && attempts < self.config.population * 20 {
            attempts += 1;
            let mut genome = PartyGenome::from_party(&self.problem.base, &self.problem.free);
            for slot in genome.free_slots() {
                if let (SlotGenome::Free(free), Some(ctx)) =
                    (&mut genome.slots[slot], self.problem.contexts.get(&slot))
                {
                    crate::sampler::randomise(free, ctx, self.problem.data, rng);
                }
            }
            let hash = self.remember(&genome, &Origin::Random);
            if !population.contains(&hash) {
                population.push(hash);
            }
        }
        population
    }
}

/// Runs a search.
pub fn optimise(
    problem: &Problem,
    evaluator: &dyn Evaluator,
    config: Config,
    controls: Controls,
) -> Result<Outcome, String> {
    let mut rng = OptRng::new(config.seed ^ 0x6f70_7469_6d69_7365);
    let mut engine = Engine::new(problem, evaluator, config);
    let progress = controls.on_progress;
    let mut population = engine.initial(&mut rng);
    engine.evaluate(&population, progress, 0)?;
    (controls.on_snapshot)(&engine.snapshot(0, &population));

    let mut generation = 0;
    let mut stagnant = 0;
    let mut best_front: Vec<u128> = front_of(&engine, &population);
    let stop = loop {
        if controls.cancel.is_cancelled() {
            break StopReason::Cancelled;
        }
        if (controls.should_stop)() {
            break StopReason::Budget;
        }
        if engine
            .config
            .max_generations
            .is_some_and(|max| generation >= max)
        {
            break StopReason::Generations;
        }
        if stagnant >= engine.config.stagnation {
            break StopReason::Stagnation;
        }
        generation += 1;

        // Offspring.
        let (front, crowd) = engine.rank(&population);
        let mut offspring = Vec::new();
        let mut attempts = 0;
        while offspring.len() < engine.config.population && attempts < engine.config.population * 10
        {
            attempts += 1;
            let a = nsga::tournament(&front, &crowd, &population, &mut rng);
            let b = nsga::tournament(&front, &crowd, &population, &mut rng);
            let pa = engine.archive.genomes[&population[a]].0.clone();
            let pb = engine.archive.genomes[&population[b]].0.clone();
            let child = engine.breed(&pa, &pb, &mut rng);
            let hash = child.hash();
            if population.contains(&hash)
                || offspring.contains(&hash)
                || engine.archive.genomes.contains_key(&hash)
            {
                continue;
            }
            engine.remember(&child, &Origin::Generation(generation));
            offspring.push(hash);
        }
        if offspring.is_empty() {
            stagnant = engine.config.stagnation;
            continue;
        }
        engine.evaluate(&offspring, progress, generation)?;

        // Environmental selection over parents and offspring.
        let mut combined = population.clone();
        combined.extend(&offspring);
        let (front, crowd) = engine.rank(&combined);
        let keep = nsga::select(&front, &crowd, &combined, engine.config.population);
        population = keep.into_iter().map(|i| combined[i]).collect();
        engine.evaluate(&population, progress, generation)?;

        let new_front = front_of(&engine, &population);
        if new_front.iter().any(|h| !best_front.contains(h)) {
            stagnant = 0;
        } else {
            stagnant += 1;
        }
        best_front = new_front;
        (controls.on_snapshot)(&engine.snapshot(generation, &population));
    };
    Ok(engine.outcome(generation, stop))
}

/// The feasible front of a population.
fn front_of(engine: &Engine, population: &[u128]) -> Vec<u128> {
    let (front, _) = engine.rank(population);
    let mut hashes: Vec<u128> = population
        .iter()
        .zip(&front)
        .filter(|(h, f)| **f == 0 && engine.score(**h).feasible)
        .map(|(h, _)| *h)
        .collect();
    hashes.sort();
    hashes
}
