//! The result file `gwsim evaluate --json` writes (T4.9.2, §14.3).
//!
//! **The file is versioned** ([`SCHEMA_VERSION`]) and documented in
//! `docs/result-schema.md`. It carries everything needed to reproduce a run
//! — the party and situations inline, the master seed, and each run's seed —
//! so `gwsim log` can re-simulate any run from the file alone (T4.9.6), and
//! the data pack version, so it can refuse when the data has changed
//! (ENG-43).
//!
//! The numbers are per-run means, so two evaluations with different run
//! counts can be read side by side.

use std::collections::BTreeSet;

use gwsim_data::build::Build;
use gwsim_data::core::Profession;
use gwsim_data::coverage::Coverage;
use gwsim_data::dataset::DataSet;
use gwsim_data::party::{PartyFile, PartySlot};
use gwsim_data::provenance::ReviewStatus;
use gwsim_data::scenario::Situation;
use gwsim_data::{AssumptionId, SkillId};
use gwsim_engine::FightSetup;
use gwsim_engine::harness::{Evaluation, Summary};
use gwsim_engine::result::RunResult;
use serde::{Deserialize, Serialize};

use crate::loading::Loaded;

/// The result file's format version. Raise it on any change a reader could
/// trip over, and describe the change in `docs/result-schema.md`.
pub const SCHEMA_VERSION: u32 = 1;

/// The label every report carries (D22).
pub const UNCALIBRATED: &str = "Absolute numbers are uncalibrated: compare builds with each \
    other under the same conditions, not with the game (D22).";

/// A whole result file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultFile {
    pub schema_version: u32,
    pub inputs: Inputs,
    /// Report 1: each slot's build and codes.
    pub builds: Vec<BuildReport>,
    /// Reports 2 and 4, per situation.
    pub situations: Vec<SituationReport>,
    /// The weighted aggregate, when a situation set was evaluated.
    #[serde(default)]
    pub set: Option<SetReport>,
    /// The notes every report carries (T4.9.3).
    pub notes: Notes,
}

/// What was evaluated, completely enough to evaluate it again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Inputs {
    pub party: PartyFile,
    pub situations: Vec<SituationInput>,
    /// The situation set's slug, when one was given.
    #[serde(default)]
    pub set: Option<String>,
    /// The master seed; run `i` uses the `i`th seed of its list.
    pub seed: u64,
    pub runs: RunsRequest,
    #[serde(default)]
    pub reviewed_only: bool,
    pub pack: PackInfo,
}

/// How many runs were asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunsRequest {
    /// Exactly this many.
    Fixed(usize),
    /// Until the result is stable (T3.8.4).
    Auto,
}

/// One situation as evaluated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SituationInput {
    /// Its slug in the data, when it came from there.
    #[serde(default)]
    pub slug: Option<String>,
    /// Its weight in the set (1 alone).
    pub weight: f64,
    pub situation: Situation,
}

/// The data pack a result was computed against (ENG-43).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackInfo {
    pub content_hash: String,
    pub baseline: String,
    pub gwsim_version: String,
    /// Where the data was read from.
    pub origin: String,
}

impl PackInfo {
    pub fn of(loaded: &Loaded) -> PackInfo {
        PackInfo {
            content_hash: loaded.version.content_hash.clone(),
            baseline: loaded.version.baseline.clone(),
            gwsim_version: loaded.version.gwsim_version.clone(),
            origin: loaded.origin.to_string(),
        }
    }

    /// The first 12 hex digits of the hash, for text.
    pub fn short_hash(&self) -> &str {
        &self.content_hash[..self.content_hash.len().min(12)]
    }
}

/// Report 1: one slot's build (§14 #1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildReport {
    pub slot: String,
    pub kind: String,
    pub professions: String,
    /// The bar in order; `None` for an empty slot.
    pub skills: Vec<Option<String>>,
    /// Skill template code (type 14).
    pub skill_code: String,
    /// Armor, one line per piece, then the weapon set.
    pub equipment: Vec<String>,
    /// Equipment template code (type 15), when any rune or insignia has an
    /// id. PvE characters cannot load these, and a hero's carries weapons
    /// only in game.
    #[serde(default)]
    pub equipment_code: Option<String>,
}

/// A metric's mean, median and 95% interval.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Metric {
    pub n: usize,
    pub mean: f64,
    pub median: f64,
    pub low: f64,
    pub high: f64,
}

impl From<&Summary> for Metric {
    fn from(summary: &Summary) -> Metric {
        Metric {
            n: summary.n,
            mean: summary.mean,
            median: summary.median,
            low: summary.ci.low,
            high: summary.ci.high,
        }
    }
}

/// Report 2: the metrics of one situation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    pub runs: usize,
    pub wins: usize,
    /// The win rate, with its Wilson 95% interval.
    pub win_rate: Metric,
    /// Over wins only.
    pub clear_time_s: Metric,
    pub deaths: Metric,
    pub damage_taken: Metric,
    pub energy_left: Metric,
    /// Death penalty at the end, in percent (chains).
    pub dp_end: Metric,
    /// Why the runs stopped: Fixed, Stable or MaxRuns.
    pub stop: String,
}

impl Metrics {
    pub fn of(evaluation: &Evaluation) -> Metrics {
        Metrics {
            runs: evaluation.runs,
            wins: evaluation.wins,
            win_rate: Metric {
                n: evaluation.runs,
                mean: evaluation.win_rate,
                median: evaluation.win_rate,
                low: evaluation.win_ci.low,
                high: evaluation.win_ci.high,
            },
            clear_time_s: (&evaluation.clear_time_s).into(),
            deaths: (&evaluation.deaths).into(),
            damage_taken: (&evaluation.damage_taken).into(),
            energy_left: (&evaluation.energy_left).into(),
            dp_end: (&evaluation.dp_end).into(),
            stop: format!("{:?}", evaluation.stop),
        }
    }
}

/// One run, enough to find and check it again (T4.9.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSummary {
    pub index: usize,
    pub seed: u64,
    pub outcome: String,
    #[serde(default)]
    pub clear_time_ms: Option<u32>,
    pub ended_ms: u32,
    pub deaths: u32,
    pub dp_end: u8,
    pub damage_taken: i64,
    pub energy_left: i32,
    #[serde(default)]
    pub first_failed: Option<usize>,
    /// The engine's digest of the whole run, in hex; a re-simulation that
    /// matches it reproduced the run exactly.
    pub digest: String,
}

impl RunSummary {
    pub fn of(index: usize, result: &RunResult) -> RunSummary {
        RunSummary {
            index,
            seed: result.seed,
            outcome: format!("{:?}", result.outcome),
            clear_time_ms: result.clear_time_ms,
            ended_ms: result.ended_ms,
            deaths: result.deaths,
            dp_end: result.dp_end,
            damage_taken: result.damage_taken,
            energy_left: result.energy_left,
            first_failed: result.first_failed,
            digest: format!("{:016x}", result.digest()),
        }
    }
}

/// Report 4: what one slot achieved with one skill, per run (§14.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContributionRow {
    pub slot: String,
    /// The skill, or "attacks and other" for what no skill caused.
    pub skill: String,
    pub uses: f64,
    pub damage: f64,
    pub healing: f64,
    pub overhealing: f64,
    /// Damage prevented by effects this slot put up.
    pub mitigation: f64,
    pub interrupts: f64,
}

/// How often one skill stopped another, per run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoppedRow {
    pub by: String,
    pub stopped: String,
    pub per_run: f64,
}

/// A slot's energy each second, averaged over the runs still going.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnergyTimeline {
    pub slot: String,
    pub per_second: Vec<f64>,
}

/// Everything about one situation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SituationReport {
    #[serde(default)]
    pub slug: Option<String>,
    pub name: String,
    pub weight: f64,
    pub metrics: Metrics,
    pub contributions: Vec<ContributionRow>,
    pub stopped: Vec<StoppedRow>,
    pub energy: Vec<EnergyTimeline>,
    pub runs: Vec<RunSummary>,
}

/// A situation set's weighted aggregate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetReport {
    pub name: String,
    pub total_weight: f64,
    /// Σ weight × win rate / Σ weight.
    pub weighted_win_rate: f64,
    /// The same over the situations with any win.
    #[serde(default)]
    pub weighted_clear_time_s: Option<f64>,
}

/// An assumption a result relied on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssumptionNote {
    pub id: String,
    pub statement: String,
    pub status: String,
}

/// The notes every report carries (T4.9.3, §14.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notes {
    pub uncalibrated: String,
    /// The assumptions any run touched.
    pub assumptions: Vec<AssumptionNote>,
    /// Skills used while still `Draft`.
    pub drafts: Vec<String>,
    /// What the data does not cover that matters here.
    pub coverage: Vec<String>,
    pub pack: PackInfo,
    pub seed: u64,
    /// Runs per situation, in order.
    pub runs: Vec<usize>,
}

impl Notes {
    /// Collects the notes for a set of evaluations of `parties`.
    pub fn collect<'a>(
        data: &DataSet,
        pack: PackInfo,
        seed: u64,
        parties: &[&PartyFile],
        evaluations: impl IntoIterator<Item = &'a Evaluation>,
        setups: &[&FightSetup],
    ) -> Notes {
        let mut assumptions: BTreeSet<AssumptionId> = BTreeSet::new();
        let mut drafts: BTreeSet<SkillId> = BTreeSet::new();
        let mut runs = Vec::new();
        for evaluation in evaluations {
            runs.push(evaluation.runs);
            for result in &evaluation.results {
                assumptions.extend(result.assumptions_touched.iter().copied());
                drafts.extend(result.draft_skills_used.iter().copied());
            }
        }
        Notes {
            uncalibrated: UNCALIBRATED.to_owned(),
            assumptions: assumptions
                .into_iter()
                .map(|id| match data.assumptions.get(id) {
                    Some(a) => AssumptionNote {
                        id: id.to_string(),
                        statement: a.statement.clone(),
                        status: format!("{:?}", a.status),
                    },
                    None => AssumptionNote {
                        id: id.to_string(),
                        statement: "(not in assumptions.ron)".to_owned(),
                        status: "Unknown".to_owned(),
                    },
                })
                .collect(),
            drafts: drafts
                .into_iter()
                .map(|id| {
                    data.skill_by_id(id)
                        .map(|s| s.name.clone())
                        .unwrap_or_else(|| format!("skill {id}"))
                })
                .collect(),
            coverage: coverage_notes(data, parties, setups),
            pack,
            seed,
            runs,
        }
    }
}

/// Coverage limits that bear on these parties and fights: per profession in
/// the parties, how many skills cannot be used yet, and which foes carry
/// skills with no encoding.
pub fn coverage_notes(
    data: &DataSet,
    parties: &[&PartyFile],
    setups: &[&FightSetup],
) -> Vec<String> {
    let coverage = Coverage::compute(data);
    let mut professions: BTreeSet<Profession> = BTreeSet::new();
    for party in parties {
        for slot in &party.slots {
            professions.insert(slot.build.primary);
            if let Some(secondary) = slot.build.secondary {
                professions.insert(secondary);
            }
        }
    }
    let mut notes = Vec::new();
    for profession in professions {
        if let Some(counts) = coverage.by_profession.get(&Some(profession)) {
            let unavailable = counts.numbers_only + counts.not_started;
            if unavailable > 0 {
                notes.push(format!(
                    "{unavailable} of {} {profession:?} skills unavailable ({} NumbersOnly, {} not \
                     started); the optimiser cannot pick them",
                    counts.total(),
                    counts.numbers_only,
                    counts.not_started
                ));
            }
        }
    }
    let mut foes: BTreeSet<String> = BTreeSet::new();
    for setup in setups {
        for unit in setup.units().iter().filter(|u| u.foe_index.is_some()) {
            let slug = gwsim_data::ids::slugify(&unit.name);
            if let Some(missing) = coverage
                .foes_with_unencoded_skills
                .iter()
                .find(|(foe, _)| foe.as_str() == slug.as_str())
            {
                foes.insert(format!(
                    "{} uses {} skill(s) with no encoding, which it never casts",
                    unit.name,
                    missing.1.len()
                ));
            }
        }
    }
    notes.extend(foes);
    notes
}

/// Report 1 for one slot.
pub fn build_report(slot: &PartySlot, data: &DataSet) -> BuildReport {
    let build = &slot.build;
    let (skill, equipment) = build.to_templates(data);
    let professions = match build.secondary {
        Some(secondary) => format!("{}/{}", build.primary.abbrev(), secondary.abbrev()),
        None => format!("{}/--", build.primary.abbrev()),
    };
    BuildReport {
        slot: slot.name.clone(),
        kind: format!("{:?}", slot.kind),
        professions,
        skills: build
            .skills
            .iter()
            .map(|id| {
                id.map(|id| {
                    data.skill_by_id(id)
                        .map(|s| s.name.clone())
                        .unwrap_or_else(|| format!("skill {id}"))
                })
            })
            .collect(),
        skill_code: skill.encode(),
        equipment: equipment_lines(build),
        equipment_code: (!equipment.items.is_empty()).then(|| equipment.encode()),
    }
}

/// A build's gear as readable lines.
fn equipment_lines(build: &Build) -> Vec<String> {
    let mut lines: Vec<String> = build
        .armor
        .iter()
        .map(|piece| {
            let insignia = piece
                .insignia
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "no insignia".to_owned());
            let rune = piece
                .rune
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "no rune".to_owned());
            format!("{:?}: {insignia}, {rune}", piece.slot)
        })
        .collect();
    let set = &build.weapon_set;
    let mut weapon: Vec<String> = Vec::new();
    for part in [
        &set.main,
        &set.offhand,
        &set.prefix,
        &set.suffix,
        &set.inscription,
    ]
    .into_iter()
    .flatten()
    {
        weapon.push(part.to_string());
    }
    weapon.extend(set.offhand_upgrades.iter().map(|u| u.to_string()));
    if !weapon.is_empty() {
        let scope = set
            .attribute
            .map(|a| format!(" ({a:?})"))
            .unwrap_or_default();
        lines.push(format!("Weapons: {}{scope}", weapon.join(", ")));
    }
    lines
}

/// Report 4 for one evaluation: contributions per slot and skill.
pub fn contributions(evaluation: &Evaluation, setup: &FightSetup) -> Vec<ContributionRow> {
    let runs = evaluation.runs.max(1) as f64;
    let slot_names = slot_names(setup);
    // Rows keyed by (slot, skill), in slot order then first appearance.
    let mut keys: Vec<(u8, Option<u16>)> = Vec::new();
    for result in &evaluation.results {
        for c in &result.stats.contributions {
            if !keys.contains(&(c.slot, c.skill)) {
                keys.push((c.slot, c.skill));
            }
        }
    }
    keys.sort_by_key(|(slot, skill)| {
        (
            *slot,
            skill.is_none(),
            skill_bar_position(setup, *slot, *skill),
        )
    });
    keys.into_iter()
        .map(|(slot, skill)| {
            let mut row = ContributionRow {
                slot: slot_names
                    .get(usize::from(slot))
                    .cloned()
                    .unwrap_or_else(|| format!("slot {slot}")),
                skill: skill
                    .map(|k| setup.fight.skills[usize::from(k)].skill.name.clone())
                    .unwrap_or_else(|| "attacks and other".to_owned()),
                uses: 0.0,
                damage: 0.0,
                healing: 0.0,
                overhealing: 0.0,
                mitigation: 0.0,
                interrupts: 0.0,
            };
            for result in &evaluation.results {
                if let Some(c) = result
                    .stats
                    .contributions
                    .iter()
                    .find(|c| c.slot == slot && c.skill == skill)
                {
                    row.uses += f64::from(c.uses);
                    row.damage += c.damage as f64;
                    row.healing += c.healing as f64;
                    row.overhealing += c.overhealing as f64;
                    row.mitigation += c.mitigation as f64;
                    row.interrupts += f64::from(c.interrupts);
                }
            }
            row.uses /= runs;
            row.damage /= runs;
            row.healing /= runs;
            row.overhealing /= runs;
            row.mitigation /= runs;
            row.interrupts /= runs;
            row
        })
        .collect()
}

/// Where a skill sits on a slot's bar, so rows read in bar order.
fn skill_bar_position(setup: &FightSetup, slot: u8, skill: Option<u16>) -> usize {
    let Some(skill) = skill else {
        return usize::MAX;
    };
    setup
        .units()
        .iter()
        .find(|u| u.slot_index == Some(usize::from(slot)))
        .and_then(|u| {
            u.bar.iter().position(|s| {
                s.as_ref()
                    .is_some_and(|s| s.original == skill || s.skill == skill)
            })
        })
        .unwrap_or(usize::MAX - 1)
}

/// Party slot names in slot order.
fn slot_names(setup: &FightSetup) -> Vec<String> {
    let mut slots: Vec<(usize, String)> = setup
        .units()
        .iter()
        .filter_map(|u| u.slot_index.map(|s| (s, u.name.clone())))
        .collect();
    slots.sort();
    slots.into_iter().map(|(_, name)| name).collect()
}

/// What interrupts stopped, per run.
pub fn stopped(evaluation: &Evaluation, setup: &FightSetup) -> Vec<StoppedRow> {
    let runs = evaluation.runs.max(1) as f64;
    let mut rows: Vec<(u16, u16, u32)> = Vec::new();
    for result in &evaluation.results {
        for s in &result.stats.stopped {
            match rows.iter_mut().find(|r| r.0 == s.by && r.1 == s.stopped) {
                Some(row) => row.2 += s.count,
                None => rows.push((s.by, s.stopped, s.count)),
            }
        }
    }
    rows.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1)));
    let name = |k: u16| setup.fight.skills[usize::from(k)].skill.name.clone();
    rows.into_iter()
        .map(|(by, stopped, count)| StoppedRow {
            by: name(by),
            stopped: name(stopped),
            per_run: f64::from(count) / runs,
        })
        .collect()
}

/// Energy per slot per second, averaged over the runs that lasted that long.
pub fn energy_timeline(evaluation: &Evaluation, setup: &FightSetup) -> Vec<EnergyTimeline> {
    let names = slot_names(setup);
    names
        .iter()
        .enumerate()
        .map(|(slot, name)| {
            let longest = evaluation
                .results
                .iter()
                .filter_map(|r| r.stats.energy_samples.get(slot).map(Vec::len))
                .max()
                .unwrap_or(0);
            let per_second = (0..longest)
                .map(|t| {
                    let (sum, n) = evaluation
                        .results
                        .iter()
                        .filter_map(|r| r.stats.energy_samples.get(slot)?.get(t))
                        .fold((0.0, 0usize), |(s, n), v| (s + f64::from(*v), n + 1));
                    if n == 0 { 0.0 } else { sum / n as f64 }
                })
                .collect();
            EnergyTimeline {
                slot: name.clone(),
                per_second,
            }
        })
        .collect()
}

/// Everything about one evaluated situation.
pub fn situation_report(
    input: &SituationInput,
    evaluation: &Evaluation,
    setup: &FightSetup,
) -> SituationReport {
    SituationReport {
        slug: input.slug.clone(),
        name: input.situation.name.clone(),
        weight: input.weight,
        metrics: Metrics::of(evaluation),
        contributions: contributions(evaluation, setup),
        stopped: stopped(evaluation, setup),
        energy: energy_timeline(evaluation, setup),
        runs: evaluation
            .results
            .iter()
            .enumerate()
            .map(|(i, r)| RunSummary::of(i, r))
            .collect(),
    }
}

/// The weighted aggregate over a set's situations.
pub fn set_report(name: &str, situations: &[SituationReport]) -> SetReport {
    let total: f64 = situations.iter().map(|s| s.weight).sum();
    let weighted_win_rate = if total > 0.0 {
        situations
            .iter()
            .map(|s| s.weight * s.metrics.win_rate.mean)
            .sum::<f64>()
            / total
    } else {
        0.0
    };
    let with_wins: Vec<&SituationReport> =
        situations.iter().filter(|s| s.metrics.wins > 0).collect();
    let clear_weight: f64 = with_wins.iter().map(|s| s.weight).sum();
    SetReport {
        name: name.to_owned(),
        total_weight: total,
        weighted_win_rate,
        weighted_clear_time_s: (clear_weight > 0.0).then(|| {
            with_wins
                .iter()
                .map(|s| s.weight * s.metrics.clear_time_s.mean)
                .sum::<f64>()
                / clear_weight
        }),
    }
}

/// The skills on a party's bars that are not `Reviewed` (`--reviewed-only`).
pub fn unreviewed_skills(party: &PartyFile, data: &DataSet) -> Vec<String> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for slot in &party.slots {
        for id in slot.build.skills.iter().flatten() {
            match data.skill_by_id(*id) {
                Some(skill) if skill.provenance.review == ReviewStatus::Reviewed => {}
                Some(skill) => {
                    names.insert(format!("{} ({:?})", skill.name, skill.provenance.review));
                }
                None => {
                    names.insert(format!("skill {id} (no file)"));
                }
            }
        }
    }
    names.into_iter().collect()
}
