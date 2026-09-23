//! Many seeded runs, aggregated with confidence intervals (WP3.8, D5).
//!
//! [`evaluate`] runs a setup on every seed of a [`SeedList`] in parallel with
//! `rayon`, one run per task, and collects the results **in seed order**, so
//! the aggregates cannot depend on how the threads were scheduled.
//! [`evaluate_until_stable`] adds batches until the win rate and clear time
//! are known well enough (T3.8.4).

use rayon::prelude::*;
use serde::Serialize;

use crate::result::{Outcome, RunResult};
use crate::rng::{RunSeed, SeedList};
use crate::setup::FightSetup;
use crate::sim::Sim;
use crate::unit::{ENERGY_SCALE, Team};

impl Sim {
    /// Turns a finished fight into its result.
    pub fn finish(mut self) -> RunResult {
        let (outcome, ended) = self.outcome.unwrap_or((Outcome::Timeout, self.now));
        for unit in &self.units {
            if let (Some(slot), Team::Party) = (unit.slot_index, unit.team) {
                self.stats.slots[slot].energy_left = unit.energy / ENERGY_SCALE;
            }
        }
        let engaged = self.engaged_at.unwrap_or(crate::time::SimTime::ZERO);
        let clear_time_ms =
            (outcome == Outcome::Win).then(|| ended.ms().saturating_sub(engaged.ms()));
        let deaths = self.stats.slots.iter().map(|s| s.deaths).sum();
        let damage_taken = self.stats.slots.iter().map(|s| s.damage_taken).sum();
        let energy_left = self.stats.slots.iter().map(|s| s.energy_left).sum();
        let mut draft: Vec<gwsim_data::SkillId> = self
            .stats
            .skills
            .iter()
            .enumerate()
            .filter(|(i, s)| s.uses > 0 && self.fight.skills[*i].draft)
            .map(|(i, _)| self.fight.skills[i].skill.id)
            .collect();
        draft.sort();
        let mut stats = self.stats;
        for (index, skill) in stats.skills.iter_mut().enumerate() {
            skill.id = self.fight.skills[index].skill.id.get();
        }
        RunResult {
            seed: self.seed.0,
            outcome,
            clear_time_ms,
            ended_ms: ended.ms(),
            deaths,
            dp_end: (deaths.min(4) * 15) as u8,
            damage_taken,
            energy_left,
            assumptions_touched: {
                let bits = self.assumptions;
                (0..64u16)
                    .filter(|id| bits & (1 << id) != 0)
                    .filter_map(gwsim_data::AssumptionId::from_number)
                    .collect()
            },
            draft_skills_used: draft,
            first_failed: self.segments.iter().position(|s| s.outcome != Outcome::Win),
            segments: std::mem::take(&mut self.segments),
            covenant_broken: self.covenant_broken,
            unit_names: if self.log.is_some() {
                self.units.iter().map(|u| u.name.clone()).collect()
            } else {
                Vec::new()
            },
            log: self.log,
            stats,
        }
    }
}

/// How to run an evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvalOptions {
    /// Runs per batch for the stop-when-stable rule.
    pub batch: usize,
    /// Stop once the win-rate interval's half-width is at most this.
    pub win_half_width: f64,
    /// Stop once the clear-time interval's half-width is at most this share
    /// of the mean.
    pub clear_time_relative: f64,
    /// Never run more than this.
    pub max_runs: usize,
    /// Never stop before this, when the rate is 0 or 1.
    pub min_runs_certain: usize,
}

impl Default for EvalOptions {
    /// The T3.8.4 defaults [Proposed]: batches of 32, a ±2.5-point win-rate
    /// interval, a ±2% clear-time interval, at most 1,024 runs, and at least
    /// 64 when every run agrees.
    fn default() -> Self {
        EvalOptions {
            batch: 32,
            win_half_width: 0.025,
            clear_time_relative: 0.02,
            max_runs: 1024,
            min_runs_certain: 64,
        }
    }
}

/// A 95% interval.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Interval {
    pub low: f64,
    pub high: f64,
}

impl Interval {
    pub fn half_width(&self) -> f64 {
        (self.high - self.low) / 2.0
    }
}

/// Summary statistics of one metric.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Summary {
    pub n: usize,
    pub mean: f64,
    pub median: f64,
    pub min: f64,
    pub max: f64,
    /// A 95% interval for the mean (normal approximation).
    pub ci: Interval,
}

impl Summary {
    /// Summarises a sample. An empty sample gives zeros.
    pub fn of(values: &[f64]) -> Summary {
        let n = values.len();
        if n == 0 {
            return Summary {
                n,
                mean: 0.0,
                median: 0.0,
                min: 0.0,
                max: 0.0,
                ci: Interval {
                    low: 0.0,
                    high: 0.0,
                },
            };
        }
        let mean = values.iter().sum::<f64>() / n as f64;
        let mut sorted = values.to_vec();
        sorted.sort_by(f64::total_cmp);
        let median = if n % 2 == 1 {
            sorted[n / 2]
        } else {
            (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
        };
        let variance = if n > 1 {
            values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64
        } else {
            0.0
        };
        let half = 1.96 * (variance / n as f64).sqrt();
        Summary {
            n,
            mean,
            median,
            min: sorted[0],
            max: sorted[n - 1],
            ci: Interval {
                low: mean - half,
                high: mean + half,
            },
        }
    }
}

/// The Wilson score interval for `wins` in `n` at 95%.
pub fn wilson(wins: usize, n: usize) -> Interval {
    if n == 0 {
        return Interval {
            low: 0.0,
            high: 1.0,
        };
    }
    let z = 1.96f64;
    let n = n as f64;
    let p = wins as f64 / n;
    let denominator = 1.0 + z * z / n;
    let centre = (p + z * z / (2.0 * n)) / denominator;
    let half = z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt() / denominator;
    Interval {
        low: (centre - half).max(0.0),
        high: (centre + half).min(1.0),
    }
}

/// Why an evaluation stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum StopReason {
    /// The seed list asked for exactly this many.
    Fixed,
    /// Both intervals were narrow enough.
    Stable,
    /// The run limit was reached first.
    MaxRuns,
}

/// Many runs of one party in one situation (§7.3).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Evaluation {
    pub runs: usize,
    pub wins: usize,
    pub win_rate: f64,
    pub win_ci: Interval,
    /// Over wins only.
    pub clear_time_s: Summary,
    pub deaths: Summary,
    pub damage_taken: Summary,
    pub energy_left: Summary,
    pub dp_end: Summary,
    pub stop: StopReason,
    pub results: Vec<RunResult>,
}

impl Evaluation {
    /// Aggregates results already in seed order.
    pub fn from_results(results: Vec<RunResult>, stop: StopReason) -> Evaluation {
        let runs = results.len();
        let wins = results.iter().filter(|r| r.won()).count();
        let clear: Vec<f64> = results
            .iter()
            .filter_map(|r| r.clear_time_ms)
            .map(|ms| f64::from(ms) / 1000.0)
            .collect();
        let metric = |f: &dyn Fn(&RunResult) -> f64| -> Summary {
            Summary::of(&results.iter().map(f).collect::<Vec<_>>())
        };
        Evaluation {
            runs,
            wins,
            win_rate: if runs == 0 {
                0.0
            } else {
                wins as f64 / runs as f64
            },
            win_ci: wilson(wins, runs),
            clear_time_s: Summary::of(&clear),
            deaths: metric(&|r| f64::from(r.deaths)),
            damage_taken: metric(&|r| r.damage_taken as f64),
            energy_left: metric(&|r| f64::from(r.energy_left)),
            dp_end: metric(&|r| f64::from(r.dp_end)),
            stop,
            results,
        }
    }
}

/// Runs every seed in a list, in parallel, keeping seed order (T3.8.2).
pub fn run_seeds(setup: &FightSetup, seeds: &[RunSeed]) -> Vec<RunResult> {
    seeds.par_iter().map(|seed| setup.run(*seed)).collect()
}

/// Evaluates a setup on a fixed seed list.
pub fn evaluate(setup: &FightSetup, seeds: &SeedList) -> Evaluation {
    Evaluation::from_results(run_seeds(setup, seeds.seeds()), StopReason::Fixed)
}

/// Adds batches of runs until the result is stable (T3.8.4).
pub fn evaluate_until_stable(setup: &FightSetup, master: u64, options: EvalOptions) -> Evaluation {
    let mut results: Vec<RunResult> = Vec::new();
    let seeds = SeedList::new(master, options.max_runs);
    loop {
        let start = results.len();
        let end = (start + options.batch).min(options.max_runs);
        results.extend(run_seeds(setup, &seeds.seeds()[start..end]));
        let n = results.len();
        let wins = results.iter().filter(|r| r.won()).count();
        let certain = wins == 0 || wins == n;
        let win_ok = if certain {
            n >= options.min_runs_certain
        } else {
            wilson(wins, n).half_width() <= options.win_half_width
        };
        let clear: Vec<f64> = results
            .iter()
            .filter_map(|r| r.clear_time_ms)
            .map(f64::from)
            .collect();
        let clear_ok = clear.is_empty() || {
            let summary = Summary::of(&clear);
            summary.mean == 0.0
                || summary.ci.half_width() <= options.clear_time_relative * summary.mean
        };
        if win_ok && clear_ok {
            return Evaluation::from_results(results, StopReason::Stable);
        }
        if n >= options.max_runs {
            return Evaluation::from_results(results, StopReason::MaxRuns);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wilson_matches_the_worked_figure() {
        // T3.8.3: 95 wins in 100 gives about [0.888, 0.978].
        let interval = wilson(95, 100);
        assert!((interval.low - 0.888).abs() < 0.002, "{interval:?}");
        assert!((interval.high - 0.978).abs() < 0.002, "{interval:?}");
    }

    #[test]
    fn wilson_behaves_at_the_edges() {
        let none = wilson(0, 50);
        assert_eq!(none.low, 0.0);
        assert!(none.high > 0.0 && none.high < 0.1);
        let all = wilson(50, 50);
        assert_eq!(all.high, 1.0);
        assert!(all.low > 0.9);
    }

    #[test]
    fn a_mean_interval_matches_a_hand_calculation() {
        // Mean 3, sample sd sqrt(2.5), n 5: half-width 1.96 × sqrt(0.5).
        let summary = Summary::of(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(summary.mean, 3.0);
        assert_eq!(summary.median, 3.0);
        let expected = 1.96 * (2.5f64 / 5.0).sqrt();
        assert!((summary.ci.half_width() - expected).abs() < 1e-12);
    }
}
