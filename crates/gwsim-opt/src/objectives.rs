//! Objectives, the success threshold and aggregation over a situation set
//! (WP5.6, §12.5, §13.6, Q16, Q17).
//!
//! - **Objectives** are computed from per-situation run statistics and are
//!   all *minimised* internally; "most energy left" is negated.
//! - **The threshold rule** (T5.6.2) [Proposed, owner confirms]: the success
//!   threshold (default 95%) must be met in *every* situation with non-zero
//!   weight. The alternative, a weighted aggregate, is a flag.
//! - **Aggregation**: objective values are weighted means over situations.
//! - **Violation** (T5.2.1): how far a build is from feasible, which orders
//!   infeasible builds under constrained domination. Under the per-situation
//!   rule it is the sum of the shortfalls; under the aggregate rule the one
//!   shortfall.

use serde::{Deserialize, Serialize};

/// The default success threshold (§13.6).
pub const DEFAULT_THRESHOLD: f64 = 0.95;

/// The clear time given to a situation with no wins, in seconds, so the
/// objective stays finite for crowding distances. Far above any real clear.
pub const NO_WIN_CLEAR_S: f64 = 10_000.0;

/// What a search can optimise (T5.6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Objective {
    /// Fastest clear: mean clear time over wins (the default goal).
    ClearTime,
    /// Fewest party deaths per run.
    Deaths,
    /// Least damage taken per run.
    DamageTaken,
    /// Most energy left at the end.
    EnergyLeft,
    /// Lowest death penalty at the end of a chain.
    DpEnd,
}

impl Objective {
    pub const ALL: [Objective; 5] = [
        Objective::ClearTime,
        Objective::Deaths,
        Objective::DamageTaken,
        Objective::EnergyLeft,
        Objective::DpEnd,
    ];

    /// The value to minimise for one situation's statistics.
    pub fn minimised(self, stats: &SituationStats) -> f64 {
        match self {
            Objective::ClearTime => stats.clear_time_s.unwrap_or(NO_WIN_CLEAR_S),
            Objective::Deaths => stats.deaths,
            Objective::DamageTaken => stats.damage_taken,
            Objective::EnergyLeft => -stats.energy_left,
            Objective::DpEnd => stats.dp_end,
        }
    }

    /// The value as a person reads it (energy left positive again).
    pub fn display(self, minimised: f64) -> f64 {
        match self {
            Objective::EnergyLeft => -minimised,
            _ => minimised,
        }
    }

    /// Parses `clear-time`, `deaths`, `damage-taken`, `energy-left`, `dp`.
    pub fn parse(text: &str) -> Option<Objective> {
        match text.to_ascii_lowercase().replace('_', "-").as_str() {
            "clear-time" | "clear" | "fastest-clear" => Some(Objective::ClearTime),
            "deaths" | "fewest-deaths" => Some(Objective::Deaths),
            "damage-taken" | "damage" => Some(Objective::DamageTaken),
            "energy-left" | "energy" => Some(Objective::EnergyLeft),
            "dp" | "dp-end" => Some(Objective::DpEnd),
            _ => None,
        }
    }
}

/// The goal a ranked list is sorted by: one objective, or a weighted sum of
/// several (each minimised).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Goal {
    pub terms: Vec<(Objective, f64)>,
}

impl Default for Goal {
    fn default() -> Self {
        Goal {
            terms: vec![(Objective::ClearTime, 1.0)],
        }
    }
}

impl Goal {
    /// The goal's value from objective values in `objectives` order.
    pub fn value(&self, objectives: &[Objective], values: &[f64]) -> f64 {
        self.terms
            .iter()
            .map(|(objective, weight)| {
                objectives
                    .iter()
                    .position(|o| o == objective)
                    .map(|i| values[i] * weight)
                    .unwrap_or(0.0)
            })
            .sum()
    }

    /// Parses `clear-time` or `clear-time:1,deaths:20`.
    pub fn parse(text: &str) -> Option<Goal> {
        let terms: Option<Vec<(Objective, f64)>> = text
            .split(',')
            .map(|part| {
                let mut pieces = part.split(':');
                let objective = Objective::parse(pieces.next()?.trim())?;
                let weight = match pieces.next() {
                    Some(w) => w.trim().parse().ok()?,
                    None => 1.0,
                };
                Some((objective, weight))
            })
            .collect();
        terms.filter(|t| !t.is_empty()).map(|terms| Goal { terms })
    }
}

/// How the success threshold is judged over a set (T5.6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThresholdRule {
    /// Met in every situation with non-zero weight [Proposed default].
    #[default]
    EverySituation,
    /// Met by the weighted mean win rate.
    WeightedAggregate,
}

/// One candidate's statistics in one situation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SituationStats {
    pub runs: usize,
    pub wins: usize,
    pub win_rate: f64,
    /// Wilson 95% interval of the win rate.
    pub win_low: f64,
    pub win_high: f64,
    /// Mean clear time over wins; `None` without any.
    pub clear_time_s: Option<f64>,
    pub deaths: f64,
    pub damage_taken: f64,
    pub energy_left: f64,
    pub dp_end: f64,
}

/// A candidate's standing over the whole set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Score {
    pub situations: Vec<SituationStats>,
    /// Weighted means, minimised, in the search's objective order.
    pub objectives: Vec<f64>,
    /// The ranking goal's value (minimised).
    pub goal: f64,
    pub feasible: bool,
    /// How far from feasible; 0 when feasible.
    pub violation: f64,
}

/// The rules a score is computed under.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scoring {
    pub objectives: Vec<Objective>,
    pub goal: Goal,
    pub threshold: f64,
    pub rule: ThresholdRule,
    /// Situation weights, in situation order.
    pub weights: Vec<f64>,
}

impl Scoring {
    /// Scores a candidate from its per-situation statistics (T5.6.3).
    pub fn score(&self, situations: Vec<SituationStats>) -> Score {
        let total: f64 = self
            .weights
            .iter()
            .filter(|w| **w > 0.0)
            .sum::<f64>()
            .max(f64::MIN_POSITIVE);
        let weighted = |f: &dyn Fn(&SituationStats) -> f64| -> f64 {
            situations
                .iter()
                .zip(&self.weights)
                .filter(|(_, w)| **w > 0.0)
                .map(|(s, w)| f(s) * w)
                .sum::<f64>()
                / total
        };
        let objectives: Vec<f64> = self
            .objectives
            .iter()
            .map(|o| weighted(&|s| o.minimised(s)))
            .collect();
        let violation = match self.rule {
            ThresholdRule::EverySituation => situations
                .iter()
                .zip(&self.weights)
                .filter(|(_, w)| **w > 0.0)
                .map(|(s, _)| (self.threshold - s.win_rate).max(0.0))
                .sum(),
            ThresholdRule::WeightedAggregate => {
                (self.threshold - weighted(&|s| s.win_rate)).max(0.0)
            }
        };
        let goal = self.goal.value(&self.objectives, &objectives);
        Score {
            situations,
            objectives,
            goal,
            feasible: violation <= 0.0,
            violation,
        }
    }
}

/// Constrained domination (Deb et al. 2002, §13.3): a feasible score beats
/// an infeasible one; of two infeasible ones the smaller violation wins; of
/// two feasible ones, the usual Pareto domination on the objectives.
pub fn dominates(a: &Score, b: &Score) -> bool {
    match (a.feasible, b.feasible) {
        (true, false) => true,
        (false, true) => false,
        (false, false) => a.violation < b.violation,
        (true, true) => {
            let mut better = false;
            for (x, y) in a.objectives.iter().zip(&b.objectives) {
                if x > y {
                    return false;
                }
                if x < y {
                    better = true;
                }
            }
            better
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(win_rate: f64, clear: f64, deaths: f64) -> SituationStats {
        SituationStats {
            runs: 100,
            wins: (win_rate * 100.0) as usize,
            win_rate,
            win_low: win_rate,
            win_high: win_rate,
            clear_time_s: Some(clear),
            deaths,
            damage_taken: 1000.0,
            energy_left: 50.0,
            dp_end: 0.0,
        }
    }

    fn scoring(rule: ThresholdRule) -> Scoring {
        Scoring {
            objectives: vec![
                Objective::ClearTime,
                Objective::Deaths,
                Objective::EnergyLeft,
            ],
            goal: Goal::default(),
            threshold: 0.95,
            rule,
            weights: vec![1.0, 3.0],
        }
    }

    #[test]
    fn a_two_situation_example_matches_the_hand_calculation() {
        // Weights 1 and 3: clear = (40 + 3 × 60) / 4 = 55; deaths =
        // (0 + 3 × 0.4) / 4 = 0.3; energy left negated = −50.
        let score = scoring(ThresholdRule::EverySituation)
            .score(vec![stats(1.0, 40.0, 0.0), stats(0.97, 60.0, 0.4)]);
        assert!((score.objectives[0] - 55.0).abs() < 1e-9);
        assert!((score.objectives[1] - 0.3).abs() < 1e-9);
        assert!((score.objectives[2] + 50.0).abs() < 1e-9);
        assert_eq!(score.goal, score.objectives[0]);
        assert!(score.feasible);
    }

    #[test]
    fn the_threshold_is_judged_per_situation_by_default() {
        let data = vec![stats(1.0, 40.0, 0.0), stats(0.93, 60.0, 0.4)];
        let every = scoring(ThresholdRule::EverySituation).score(data.clone());
        assert!(!every.feasible);
        assert!((every.violation - 0.02).abs() < 1e-9);
        // Weighted: (1.0 + 3 × 0.93) / 4 = 0.9475, still short by 0.0025.
        let aggregate = scoring(ThresholdRule::WeightedAggregate).score(data);
        assert!((aggregate.violation - 0.0025).abs() < 1e-9);
        let easy = Scoring {
            threshold: 0.9,
            ..scoring(ThresholdRule::WeightedAggregate)
        }
        .score(vec![stats(1.0, 40.0, 0.0), stats(0.93, 60.0, 0.4)]);
        assert!(easy.feasible);
    }

    #[test]
    fn a_zero_weight_situation_is_ignored() {
        let mut rules = scoring(ThresholdRule::EverySituation);
        rules.weights = vec![1.0, 0.0];
        let score = rules.score(vec![stats(1.0, 40.0, 0.0), stats(0.1, 90.0, 5.0)]);
        assert!(score.feasible);
        assert_eq!(score.objectives[0], 40.0);
    }

    #[test]
    fn constrained_domination_puts_feasible_first() {
        let rules = scoring(ThresholdRule::EverySituation);
        let fast_but_failing = rules.score(vec![stats(0.5, 20.0, 0.0), stats(0.5, 20.0, 0.0)]);
        let slow_but_safe = rules.score(vec![stats(1.0, 90.0, 1.0), stats(1.0, 90.0, 1.0)]);
        let less_failing = rules.score(vec![stats(0.9, 20.0, 0.0), stats(0.9, 20.0, 0.0)]);
        assert!(dominates(&slow_but_safe, &fast_but_failing));
        assert!(dominates(&less_failing, &fast_but_failing));
        let faster = rules.score(vec![stats(1.0, 80.0, 1.0), stats(1.0, 80.0, 1.0)]);
        assert!(dominates(&faster, &slow_but_safe));
        let trade = rules.score(vec![stats(1.0, 70.0, 2.0), stats(1.0, 70.0, 2.0)]);
        assert!(!dominates(&trade, &faster) && !dominates(&faster, &trade));
    }

    #[test]
    fn goals_parse_and_combine() {
        let goal = Goal::parse("clear-time:1,deaths:20").unwrap();
        assert_eq!(goal.terms.len(), 2);
        let value = goal.value(&[Objective::ClearTime, Objective::Deaths], &[50.0, 0.5]);
        assert_eq!(value, 60.0);
        assert!(Goal::parse("nonsense").is_none());
    }
}
