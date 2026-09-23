//! `gwsim check` — the relative checks of DESIGN §17.4 (WP4.10).
//!
//! Absolute numbers are uncalibrated (D22), so correctness is checked
//! relatively: the benchmark party must beat weakened copies of itself (RC1)
//! and a naive baseline team (RC2) under identical conditions, and some
//! properties must always hold whatever the numbers are (RC6).
//!
//! **Every comparison is paired on shared seeds** (ENG-42) and judged by the
//! rule in [`crate::compare::verdict`]: A beats B when the paired 95%
//! interval of the win-rate difference lies above zero, or when the win rates
//! are equal within it and the clear-time difference's interval lies below
//! zero. The bootstrap uses 1,000 resamples with a fixed seed.

use std::io::{self, Write};
use std::path::PathBuf;

use gwsim_data::build::{Build, SlotKind};
use gwsim_data::core::{ArmorSlot, Attribute};
use gwsim_data::dataset::DataSet;
use gwsim_data::party::PartyFile;
use gwsim_data::provenance::ReviewStatus;
use gwsim_data::skill::Skill;
use gwsim_engine::harness::{self, Evaluation};
use gwsim_engine::unit::{HEALTH_SCALE, Target, UnitId};
use gwsim_engine::{FightSetup, SeedList};
use serde::Serialize;

use crate::compare::{self, SituationComparison, Verdict};
use crate::data::{FAILED, OK};
use crate::evaluate::{self, Context};
use crate::results::{PackInfo, SituationInput, UNCALIBRATED};
use crate::{CheckArgs, CheckLevel, CheckSuite};

/// The benchmark party the checks defend.
pub const BENCHMARK: &str = "m1-mesmerway";

/// RC2's baseline team (T4.10.1).
pub const BASELINE: &str = "baseline-naive";

/// The situation RC1 is judged in.
pub const RC1_SITUATION: &str = "kournan-patrol-hm";

/// The situation set RC2 is judged on.
pub const RC2_SET: &str = "m1";

/// RC1 passes when the benchmark beats at least this share of the variants.
pub const RC1_SHARE: f64 = 0.9;

/// How many random off-role swaps RC1 tries.
pub const RC1_SWAPS: usize = 20;

/// RC2's margins: a win rate this much higher, or an equal one with a clear
/// time this much shorter.
pub const RC2_WIN_MARGIN: f64 = 0.30;
pub const RC2_CLEAR_MARGIN: f64 = 0.30;

/// The seed the variant choices are drawn from, so RC1 always tries the
/// same variants.
pub const VARIANT_SEED: u64 = 0x5243_315f_7661_7273;

/// The master seed of every paired comparison.
pub const CHECK_SEED: u64 = 1;

/// Runs per comparison at each level. Reduced keeps CI under its budget.
pub fn runs_for(level: CheckLevel) -> usize {
    match level {
        CheckLevel::Reduced => 48,
        CheckLevel::Full => 256,
    }
}

/// One check's outcome.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CheckResult {
    pub id: String,
    pub description: String,
    pub pass: bool,
    /// One line per thing checked, for a person.
    pub details: Vec<String>,
    /// The numbers behind the verdict, for a tool.
    pub evidence: serde_json::Value,
}

/// A check: an id, what it checks, which suite it is in, and how to run it.
pub struct Check {
    pub id: &'static str,
    pub description: &'static str,
    pub suite: CheckSuite,
    pub run: fn(&Context, CheckLevel) -> Result<CheckResult, String>,
}

/// Every check, in the order they run.
pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "SELF",
            description: "A team compared with itself is not better (T4.10.2).",
            suite: CheckSuite::Unit,
            run: check_self,
        },
        Check {
            id: "DET",
            description: "The same seed gives the same run, twice over.",
            suite: CheckSuite::Unit,
            run: check_determinism,
        },
        Check {
            id: "RC1",
            description: "Mesmerway beats its weakened variants (§17.4).",
            suite: CheckSuite::Relative,
            run: check_rc1,
        },
        Check {
            id: "RC2",
            description: "Mesmerway beats the naive baseline team (§17.4).",
            suite: CheckSuite::Relative,
            run: check_rc2,
        },
        Check {
            id: "RC6",
            description: "Monotonicity properties hold (§17.4).",
            suite: CheckSuite::Relative,
            run: check_rc6,
        },
        Check {
            id: "RC4",
            description: "The PvX bar ranks highly among random legal player builds (§17.4).",
            suite: CheckSuite::Relative,
            run: crate::check_opt::check_rc4,
        },
        Check {
            id: "RC5",
            description: "The optimiser does at least as well as the PvX bar (§17.4).",
            suite: CheckSuite::Relative,
            run: crate::check_opt::check_rc5,
        },
    ]
}

/// The whole report `gwsim check` writes.
#[derive(Debug, Clone, Serialize)]
pub struct CheckReport {
    pub schema_version: u32,
    pub level: String,
    pub uncalibrated: String,
    pub pack: PackInfo,
    pub results: Vec<CheckResult>,
}

/// Runs `gwsim check`.
pub fn run(args: &CheckArgs, out: &mut impl Write) -> io::Result<i32> {
    let context = match evaluate::load_context(args.data_dir.as_deref()) {
        Ok(context) => context,
        Err(message) => {
            writeln!(out, "{message}")?;
            return Ok(FAILED);
        }
    };
    let selected: Vec<Check> = checks()
        .into_iter()
        .filter(|c| match args.suite {
            CheckSuite::All => true,
            suite => c.suite == suite,
        })
        .filter(|c| args.only.is_empty() || args.only.iter().any(|o| o.eq_ignore_ascii_case(c.id)))
        .collect();
    if selected.is_empty() {
        writeln!(out, "no check matches those options")?;
        return Ok(FAILED);
    }

    writeln!(out, "gwsim check ({:?}, {:?})", args.suite, args.level)?;
    writeln!(out, "  {UNCALIBRATED}")?;
    let pack = PackInfo::of(&context.loaded);
    writeln!(out, "  data: {}, pack {}", pack.origin, pack.short_hash())?;
    writeln!(out)?;

    let mut results = Vec::new();
    for check in &selected {
        let outcome = evaluate::with_threads(args.threads, || (check.run)(&context, args.level))
            .and_then(|r| r);
        let result = outcome.unwrap_or_else(|message| CheckResult {
            id: check.id.to_owned(),
            description: check.description.to_owned(),
            pass: false,
            details: vec![format!("could not run: {message}")],
            evidence: serde_json::Value::Null,
        });
        writeln!(
            out,
            "{} {}: {}",
            if result.pass { "PASS" } else { "FAIL" },
            result.id,
            result.description
        )?;
        for line in &result.details {
            writeln!(out, "    {line}")?;
        }
        results.push(result);
    }

    let failed = results.iter().filter(|r| !r.pass).count();
    writeln!(out)?;
    writeln!(
        out,
        "{} of {} checks passed",
        results.len() - failed,
        results.len()
    )?;
    let report = CheckReport {
        schema_version: crate::results::SCHEMA_VERSION,
        level: format!("{:?}", args.level),
        uncalibrated: UNCALIBRATED.to_owned(),
        pack,
        results,
    };
    let path = args
        .report
        .clone()
        .unwrap_or_else(|| PathBuf::from("gwsim-check.json"));
    match serde_json::to_string_pretty(&report) {
        Ok(text) => match std::fs::write(&path, text) {
            Ok(()) => writeln!(out, "report written to {}", path.display())?,
            Err(error) => writeln!(out, "could not write {}: {error}", path.display())?,
        },
        Err(error) => writeln!(out, "could not serialise the report: {error}")?,
    }
    Ok(if failed == 0 { OK } else { FAILED })
}

// ------------------------------------------------------------ comparisons

fn party(context: &Context, slug: &str) -> Result<PartyFile, String> {
    evaluate::find_party(&context.loaded.data, slug)
}

fn single(context: &Context, slug: &str) -> Result<SituationInput, String> {
    let (mut inputs, _) = evaluate::situation_inputs(&context.loaded.data, Some(slug), None)?;
    Ok(inputs.remove(0))
}

fn setup_for(
    context: &Context,
    party: &PartyFile,
    input: &SituationInput,
) -> Result<FightSetup, String> {
    Ok(evaluate::prepare(context, party, std::slice::from_ref(input))?.remove(0))
}

/// Compares two evaluations already run on the same seeds.
pub fn judge(input: &SituationInput, a: &Evaluation, b: &Evaluation) -> SituationComparison {
    let metrics = compare::differences(a, b);
    SituationComparison {
        name: input.situation.name.clone(),
        slug: input.slug.clone(),
        runs: a.runs.min(b.runs),
        verdict: compare::verdict(&metrics),
        metrics,
    }
}

fn summary_line(label: &str, c: &SituationComparison) -> String {
    let win = &c.metrics[0];
    let clear = &c.metrics[1];
    format!(
        "{label}: win {:.1}% vs {:.1}% (diff {:+.1}, {:+.1} to {:+.1}); clear {:.1} s vs {:.1} s; {}",
        win.a * 100.0,
        win.b * 100.0,
        win.difference.mean * 100.0,
        win.difference.ci.low * 100.0,
        win.difference.ci.high * 100.0,
        clear.a,
        clear.b,
        match c.verdict {
            Verdict::ABetter => "benchmark better",
            Verdict::BBetter => "variant better",
            Verdict::NotDifferent => "not different",
        }
    )
}

fn check_self(context: &Context, level: CheckLevel) -> Result<CheckResult, String> {
    let input = single(context, RC1_SITUATION)?;
    let team = party(context, BENCHMARK)?;
    let setup = setup_for(context, &team, &input)?;
    let seeds = SeedList::new(CHECK_SEED, runs_for(level) / 2);
    let a = harness::evaluate(&setup, &seeds);
    let b = harness::evaluate(&setup, &seeds);
    let comparison = judge(&input, &a, &b);
    Ok(CheckResult {
        id: "SELF".to_owned(),
        description: "A team compared with itself is not better (T4.10.2).".to_owned(),
        pass: comparison.verdict == Verdict::NotDifferent,
        details: vec![summary_line("Mesmerway vs itself", &comparison)],
        evidence: serde_json::to_value(&comparison).unwrap_or_default(),
    })
}

fn check_determinism(context: &Context, _level: CheckLevel) -> Result<CheckResult, String> {
    let input = single(context, RC1_SITUATION)?;
    let team = party(context, BENCHMARK)?;
    let setup = setup_for(context, &team, &input)?;
    let seeds = SeedList::new(CHECK_SEED, 8);
    let first: Vec<u64> = harness::run_seeds(&setup, seeds.seeds())
        .iter()
        .map(|r| r.digest())
        .collect();
    let second: Vec<u64> = seeds
        .seeds()
        .iter()
        .map(|s| setup.run(*s).digest())
        .collect();
    Ok(CheckResult {
        id: "DET".to_owned(),
        description: "The same seed gives the same run, twice over.".to_owned(),
        pass: first == second,
        details: vec![format!(
            "{} runs, parallel and serial digests {}",
            first.len(),
            if first == second { "agree" } else { "differ" }
        )],
        evidence: serde_json::json!({"parallel": first, "serial": second}),
    })
}

// -------------------------------------------------------------------- RC1

/// A weakened copy of the benchmark and what was done to it.
#[derive(Debug, Clone)]
pub struct Variant {
    pub kind: VariantKind,
    pub label: String,
    pub party: PartyFile,
}

/// The four kinds of weakening (T4.10.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum VariantKind {
    EliteRemoved,
    OffRoleSwap,
    AttributesMisallocated,
    NoRunesOrInsignias,
}

/// A deterministic stream of choices.
struct Choices(u64);

impl Choices {
    fn below(&mut self, n: usize) -> usize {
        (gwsim_engine::rng::splitmix64(&mut self.0) % n.max(1) as u64) as usize
    }
}

fn encoded(skill: &Skill) -> bool {
    skill.encoding.is_some() && skill.provenance.review != ReviewStatus::NumbersOnly
}

/// Every RC1 variant of a party: each hero's elite removed, 20 seeded
/// off-role swaps, attributes misallocated per profession group, and no
/// runes or insignias. The player slot is never changed (it is locked).
pub fn variants(party: &PartyFile, data: &DataSet) -> Vec<Variant> {
    let mut out = Vec::new();
    let heroes: Vec<usize> = party
        .slots
        .iter()
        .enumerate()
        .filter(|(_, s)| s.kind != SlotKind::Human && !s.locked)
        .map(|(i, _)| i)
        .collect();

    // Each hero's elite removed, the slot left empty.
    for &index in &heroes {
        let slot = &party.slots[index];
        if let Some(position) = slot.build.skills.iter().position(|id| {
            id.and_then(|id| data.skill_by_id(id))
                .is_some_and(|s| s.elite)
        }) {
            let mut copy = party.clone();
            copy.slots[index].build.skills[position] = None;
            copy.name = format!("{}: {} without its elite", party.name, slot.name);
            out.push(Variant {
                kind: VariantKind::EliteRemoved,
                label: format!("{} elite removed", slot.name),
                party: copy,
            });
        }
    }

    // Off-role swaps: a non-elite skill replaced by a legal, encoded skill of
    // the hero's professions that shares none of its roles.
    let mut choices = Choices(VARIANT_SEED);
    let mut attempts = 0;
    let mut swaps = 0;
    while swaps < RC1_SWAPS && attempts < RC1_SWAPS * 50 && !heroes.is_empty() {
        attempts += 1;
        let index = heroes[choices.below(heroes.len())];
        let slot = &party.slots[index];
        let filled: Vec<usize> = (0..8)
            .filter(|&p| {
                slot.build.skills[p]
                    .and_then(|id| data.skill_by_id(id))
                    .is_some_and(|s| !s.elite)
            })
            .collect();
        if filled.is_empty() {
            continue;
        }
        let position = filled[choices.below(filled.len())];
        let Some(original) = slot.build.skills[position].and_then(|id| data.skill_by_id(id)) else {
            continue;
        };
        let roles = original
            .encoding
            .as_ref()
            .map(|e| e.roles.clone())
            .unwrap_or_default();
        let professions = [Some(slot.build.primary), slot.build.secondary];
        let pool: Vec<&Skill> = data
            .skills
            .values()
            .map(|e| &e.value)
            .filter(|s| {
                encoded(s)
                    && !s.elite
                    && !s.pve_only
                    && s.profession.is_some()
                    && professions.contains(&s.profession)
                    && !slot.build.skills.contains(&Some(s.id))
                    && s.encoding
                        .as_ref()
                        .is_some_and(|e| e.roles.iter().all(|r| !roles.contains(r)))
            })
            .collect();
        if pool.is_empty() {
            continue;
        }
        let replacement = pool[choices.below(pool.len())];
        let mut copy = party.clone();
        copy.slots[index].build.skills[position] = Some(replacement.id);
        if !copy.slots[index].build.is_legal(data, SlotKind::Hero) {
            continue;
        }
        swaps += 1;
        copy.name = format!(
            "{}: {} swaps {} for {}",
            party.name, slot.name, original.name, replacement.name
        );
        out.push(Variant {
            kind: VariantKind::OffRoleSwap,
            label: format!("{}: {} -> {}", slot.name, original.name, replacement.name),
            party: copy,
        });
    }

    // Attributes misallocated: per primary profession, every such hero moves
    // all points from its highest attribute into an unused one.
    let mut professions: Vec<_> = heroes
        .iter()
        .map(|&i| party.slots[i].build.primary)
        .collect();
    professions.sort();
    professions.dedup();
    for profession in professions {
        let mut copy = party.clone();
        let mut moved = Vec::new();
        for &index in &heroes {
            let build = &mut copy.slots[index].build;
            if build.primary != profession {
                continue;
            }
            if let Some(changed) = misallocate(build) {
                moved.push(format!("{} {changed}", party.slots[index].name));
            }
        }
        if !moved.is_empty()
            && heroes
                .iter()
                .all(|&i| copy.slots[i].build.is_legal(data, SlotKind::Hero))
        {
            copy.name = format!("{}: {profession:?} attributes misallocated", party.name);
            out.push(Variant {
                kind: VariantKind::AttributesMisallocated,
                label: format!("{profession:?} heroes misallocated ({})", moved.join("; ")),
                party: copy,
            });
        }
    }

    // No runes or insignias on any hero.
    let mut copy = party.clone();
    for &index in &heroes {
        for piece in copy.slots[index].build.armor.iter_mut() {
            piece.rune = None;
            piece.insignia = None;
        }
    }
    copy.name = format!("{}: no runes or insignias", party.name);
    out.push(Variant {
        kind: VariantKind::NoRunesOrInsignias,
        label: "heroes without runes or insignias".to_owned(),
        party: copy,
    });
    out
}

/// Moves every point from a build's highest attribute into an unused
/// attribute of its primary profession. Returns what moved.
fn misallocate(build: &mut Build) -> Option<String> {
    let (&from, &rank) = build
        .attribute_points
        .iter()
        .max_by_key(|(a, r)| (**r, **a))?;
    let to = Attribute::ALL.into_iter().find(|a| {
        a.profession() == build.primary
            && !build.attribute_points.contains_key(a)
            && *a != build.primary.primary_attribute()
    })?;
    build.attribute_points.remove(&from);
    build.attribute_points.insert(to, rank);
    Some(format!("{from:?} {rank} -> {to:?}"))
}

fn check_rc1(context: &Context, level: CheckLevel) -> Result<CheckResult, String> {
    let data = &context.loaded.data;
    let input = single(context, RC1_SITUATION)?;
    let benchmark = party(context, BENCHMARK)?;
    let seeds = SeedList::new(CHECK_SEED, runs_for(level));
    let reference = harness::evaluate(&setup_for(context, &benchmark, &input)?, &seeds);
    let variants = variants(&benchmark, data);

    let mut details = Vec::new();
    let mut evidence = Vec::new();
    let mut beaten = 0;
    let mut elites_ok = true;
    for variant in &variants {
        let setup = setup_for(context, &variant.party, &input)?;
        let evaluation = harness::evaluate(&setup, &seeds);
        let comparison = judge(&input, &reference, &evaluation);
        let beats = comparison.verdict == Verdict::ABetter;
        if beats {
            beaten += 1;
        } else if variant.kind == VariantKind::EliteRemoved {
            elites_ok = false;
        }
        details.push(summary_line(&variant.label, &comparison));
        evidence.push(serde_json::json!({
            "kind": variant.kind,
            "label": variant.label,
            "benchmark_beats": beats,
            "comparison": comparison,
        }));
    }
    let share = beaten as f64 / variants.len().max(1) as f64;
    let pass = share >= RC1_SHARE && elites_ok;
    details.insert(
        0,
        format!(
            "beats {beaten} of {} variants ({:.0}%, needs {:.0}%); every elite-removed variant beaten: {}",
            variants.len(),
            share * 100.0,
            RC1_SHARE * 100.0,
            if elites_ok { "yes" } else { "no" }
        ),
    );
    Ok(CheckResult {
        id: "RC1".to_owned(),
        description: "Mesmerway beats its weakened variants (§17.4).".to_owned(),
        pass,
        details,
        evidence: serde_json::json!({
            "situation": RC1_SITUATION,
            "runs": seeds.len(),
            "variants": evidence,
        }),
    })
}

// -------------------------------------------------------------------- RC2

/// Whether a situation is a fight: some foe carries a skill. Training
/// dummies carry none and never fight back.
fn is_combat(setup: &FightSetup) -> bool {
    setup
        .units()
        .iter()
        .any(|u| u.foe_index.is_some() && u.bar.iter().any(Option::is_some))
}

fn check_rc2(context: &Context, level: CheckLevel) -> Result<CheckResult, String> {
    let data = &context.loaded.data;
    let (inputs, _) = evaluate::situation_inputs(data, None, Some(RC2_SET))?;
    let benchmark = party(context, BENCHMARK)?;
    let baseline = party(context, BASELINE)?;
    let seeds = SeedList::new(CHECK_SEED, runs_for(level));
    let mut details = Vec::new();
    let mut evidence = Vec::new();
    let mut pass = true;
    for input in &inputs {
        let a = setup_for(context, &benchmark, input)?;
        if !is_combat(&a) {
            details.push(format!("{}: skipped, not a fight", input.situation.name));
            continue;
        }
        let b = setup_for(context, &baseline, input)?;
        let ea = harness::evaluate(&a, &seeds);
        let eb = harness::evaluate(&b, &seeds);
        let comparison = judge(input, &ea, &eb);
        let win = &comparison.metrics[0];
        let win_margin = win.a - win.b >= RC2_WIN_MARGIN;
        let equal = !win.a_better() && !win.b_better();
        let clear_margin = equal && eb.wins > 0 && ea.wins > 0 && {
            let base = eb.clear_time_s.mean;
            base > 0.0 && ea.clear_time_s.mean <= base * (1.0 - RC2_CLEAR_MARGIN)
        };
        let ok = win_margin || clear_margin;
        pass &= ok;
        details.push(format!(
            "{}: {} (win {:.1}% vs {:.1}%, mean clear over wins {:.1} s vs {:.1} s)",
            input.situation.name,
            if ok { "ok" } else { "not by the margin" },
            win.a * 100.0,
            win.b * 100.0,
            ea.clear_time_s.mean,
            eb.clear_time_s.mean
        ));
        evidence.push(serde_json::json!({
            "situation": input.slug,
            "passes": ok,
            "comparison": comparison,
        }));
    }
    Ok(CheckResult {
        id: "RC2".to_owned(),
        description: "Mesmerway beats the naive baseline team (§17.4).".to_owned(),
        pass,
        details,
        evidence: serde_json::json!({
            "set": RC2_SET,
            "runs": seeds.len(),
            "win_margin": RC2_WIN_MARGIN,
            "clear_margin": RC2_CLEAR_MARGIN,
            "situations": evidence,
        }),
    })
}

// -------------------------------------------------------------------- RC6

/// One property's outcome.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Property {
    pub name: String,
    pub holds: bool,
    pub detail: String,
}

fn check_rc6(context: &Context, level: CheckLevel) -> Result<CheckResult, String> {
    let properties = vec![
        energy_surge_rises_with_domination(context)?,
        a_healer_never_lowers_the_win_rate(context, level)?,
        a_vigor_rune_never_lowers_health(context)?,
        fast_casting_never_lengthens_activation(context)?,
    ];
    Ok(CheckResult {
        id: "RC6".to_owned(),
        description: "Monotonicity properties hold (§17.4).".to_owned(),
        pass: properties.iter().all(|p| p.holds),
        details: properties
            .iter()
            .map(|p| format!("{}: {}", p.name, p.detail))
            .collect(),
        evidence: serde_json::to_value(&properties).unwrap_or_default(),
    })
}

/// Raising Domination Magic never lowers Energy Surge's damage (single use,
/// fixed seed, every rank from 0 to 21).
pub fn energy_surge_rises_with_domination(context: &Context) -> Result<Property, String> {
    let input = single(context, "dummies-hm")?;
    let team = party(context, "m0-player")?;
    let setup = setup_for(context, &team, &input)?;
    let mut damages = Vec::new();
    for rank in 0..=21u8 {
        let mut sim = setup.sim(SeedList::new(CHECK_SEED, 1).get(0));
        let player = UnitId(0);
        sim.units[0].base_ranks[Attribute::DominationMagic.index()] = rank;
        let slot = (0..8)
            .find(|s| {
                sim.slot_skill(player, *s).is_some_and(|k| {
                    sim.fight.skills[usize::from(k)].slug.as_str() == "energy-surge"
                })
            })
            .ok_or("m0-player does not carry Energy Surge")?;
        let target = sim
            .units
            .iter()
            .find(|u| u.foe_index.is_some())
            .map(|u| u.id)
            .ok_or("no dummy to hit")?;
        let total = |sim: &gwsim_engine::sim::Sim| -> i64 {
            sim.units
                .iter()
                .filter(|u| u.foe_index.is_some())
                .map(|u| i64::from(u.health))
                .sum()
        };
        let before = total(&sim);
        sim.use_skill(player, slot, Target::Unit(target))
            .map_err(|e| format!("Energy Surge could not be used: {e:?}"))?;
        let until = sim.now.plus(5_000);
        sim.step_until(until);
        damages.push((before - total(&sim)) / i64::from(HEALTH_SCALE));
    }
    let holds = damages.windows(2).all(|w| w[1] >= w[0]);
    Ok(Property {
        name: "Domination vs Energy Surge".to_owned(),
        holds,
        detail: format!("damage at ranks 0 to 21: {damages:?}"),
    })
}

/// Adding a healer hero to a failing team never lowers its win rate
/// (paired, within confidence).
pub fn a_healer_never_lowers_the_win_rate(
    context: &Context,
    level: CheckLevel,
) -> Result<Property, String> {
    let input = single(context, RC1_SITUATION)?;
    let full = party(context, BENCHMARK)?;
    // The failing team: the benchmark without its two healers.
    let healers = ["hero 5", "hero 6"];
    let mut failing = full.clone();
    failing
        .slots
        .retain(|s| !healers.contains(&s.name.as_str()));
    failing.name = "Mesmerway without healers".to_owned();
    let mut helped = failing.clone();
    let healer = full
        .slots
        .iter()
        .find(|s| s.name == healers[0])
        .ok_or("the benchmark has no hero 5")?
        .clone();
    helped.slots.push(healer);
    helped.name = "Mesmerway without healers, plus hero 5".to_owned();
    let seeds = SeedList::new(CHECK_SEED, runs_for(level));
    let before = harness::evaluate(&setup_for(context, &failing, &input)?, &seeds);
    let after = harness::evaluate(&setup_for(context, &helped, &input)?, &seeds);
    let comparison = judge(&input, &after, &before);
    let holds = !comparison.metrics[0].b_better();
    Ok(Property {
        name: "a healer hero added".to_owned(),
        holds,
        detail: format!(
            "win {:.1}% without, {:.1}% with (diff {:+.1}, {:+.1} to {:+.1})",
            before.win_rate * 100.0,
            after.win_rate * 100.0,
            comparison.metrics[0].difference.mean * 100.0,
            comparison.metrics[0].difference.ci.low * 100.0,
            comparison.metrics[0].difference.ci.high * 100.0
        ),
    })
}

/// Adding a Vigor rune never lowers maximum health, on every slot of the
/// benchmark and the baseline, on every armor piece, at every grade.
pub fn a_vigor_rune_never_lowers_health(context: &Context) -> Result<Property, String> {
    let data = &context.loaded.data;
    let mut checked = 0;
    let mut failures = Vec::new();
    for slug in [BENCHMARK, BASELINE] {
        let team = party(context, slug)?;
        for slot in &team.slots {
            let mut bare = slot.build.clone();
            for piece in bare.armor.iter_mut() {
                piece.rune = None;
            }
            let without = gwsim_data::derived::max_health(&bare, 20, data, &context.core);
            for grade in ["minor-vigor", "major-vigor", "superior-vigor"] {
                for piece in ArmorSlot::ALL {
                    let mut with = bare.clone();
                    with.armor[piece.index()].rune = Some(grade.parse().map_err(|_| "bad slug")?);
                    let health = gwsim_data::derived::max_health(&with, 20, data, &context.core);
                    checked += 1;
                    if health < without {
                        failures.push(format!("{} {grade} on {piece:?}", slot.name));
                    }
                }
            }
        }
    }
    Ok(Property {
        name: "a Vigor rune added".to_owned(),
        holds: failures.is_empty(),
        detail: if failures.is_empty() {
            format!("{checked} builds, none lost health")
        } else {
            format!("lost health: {}", failures.join(", "))
        },
    })
}

/// More Fast Casting never lengthens an activation, for every spell and
/// signet on the benchmark player's bar, at every rank from 0 to 21.
pub fn fast_casting_never_lengthens_activation(context: &Context) -> Result<Property, String> {
    let input = single(context, "dummies-hm")?;
    let team = party(context, BENCHMARK)?;
    let setup = setup_for(context, &team, &input)?;
    let mut failures = Vec::new();
    let mut checked = 0;
    let player = UnitId(0);
    let skills: Vec<u16> = {
        let sim = setup.sim(SeedList::new(CHECK_SEED, 1).get(0));
        (0..8).filter_map(|s| sim.slot_skill(player, s)).collect()
    };
    for skill in skills {
        let mut previous = u32::MAX;
        for rank in 0..=21u8 {
            let mut sim = setup.sim(SeedList::new(CHECK_SEED, 1).get(0));
            sim.units[0].base_ranks[Attribute::FastCasting.index()] = rank;
            let ms = sim.activation_ms(player, skill);
            checked += 1;
            if ms > previous {
                failures.push(format!(
                    "{} at rank {rank}: {ms} ms after {previous} ms",
                    sim.fight.skills[usize::from(skill)].skill.name
                ));
            }
            previous = ms;
        }
    }
    Ok(Property {
        name: "Fast Casting raised".to_owned(),
        holds: failures.is_empty(),
        detail: if failures.is_empty() {
            format!("{checked} activations, none longer")
        } else {
            failures.join("; ")
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn context() -> Context {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        evaluate::load_context(Some(&dir)).unwrap()
    }

    #[test]
    fn rc1_generates_every_kind_of_variant() {
        let context = context();
        let party = party(&context, BENCHMARK).unwrap();
        let variants = variants(&party, &context.loaded.data);
        let count = |kind| variants.iter().filter(|v| v.kind == kind).count();
        assert_eq!(count(VariantKind::EliteRemoved), 7);
        assert_eq!(count(VariantKind::OffRoleSwap), RC1_SWAPS);
        assert_eq!(count(VariantKind::AttributesMisallocated), 3);
        assert_eq!(count(VariantKind::NoRunesOrInsignias), 1);
        for variant in &variants {
            assert_eq!(
                variant.party.slots[0], party.slots[0],
                "the player is locked"
            );
            for slot in &variant.party.slots[1..] {
                assert!(
                    slot.build.is_legal(&context.loaded.data, SlotKind::Hero),
                    "{}",
                    variant.label
                );
            }
        }
        // The same variants every time.
        let again = variants_labels(&party, &context.loaded.data);
        assert_eq!(
            again,
            variants.iter().map(|v| v.label.clone()).collect::<Vec<_>>()
        );
    }

    fn variants_labels(party: &PartyFile, data: &DataSet) -> Vec<String> {
        variants(party, data).into_iter().map(|v| v.label).collect()
    }

    // RC6 runs in `cargo test` as well as in `gwsim check` (T4.10.5).

    #[test]
    fn rc6_energy_surge_rises_with_domination() {
        let property = energy_surge_rises_with_domination(&context()).unwrap();
        assert!(property.holds, "{}", property.detail);
    }

    #[test]
    fn rc6_a_vigor_rune_never_lowers_health() {
        let property = a_vigor_rune_never_lowers_health(&context()).unwrap();
        assert!(property.holds, "{}", property.detail);
    }

    #[test]
    fn rc6_fast_casting_never_lengthens_activation() {
        let property = fast_casting_never_lengthens_activation(&context()).unwrap();
        assert!(property.holds, "{}", property.detail);
    }

    #[test]
    fn rc6_a_healer_never_lowers_the_win_rate() {
        let property = a_healer_never_lowers_the_win_rate(&context(), CheckLevel::Reduced).unwrap();
        assert!(property.holds, "{}", property.detail);
    }

    #[test]
    fn a_team_compared_with_itself_is_not_better() {
        let result = check_self(&context(), CheckLevel::Reduced).unwrap();
        assert!(result.pass, "{:?}", result.details);
    }
}
