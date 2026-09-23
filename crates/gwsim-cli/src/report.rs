//! Text rendering of the reports (§14, T4.9.3 to T4.9.7).
//!
//! **Every report opens with [`write_header`] and closes with
//! [`write_footer`]**, which between them print the §14.1 notes: the
//! "uncalibrated" label, the assumptions touched with their statements, the
//! draft skills used, the coverage limits, the data pack version and the
//! seeds. A report that skipped them would present numbers without the
//! caveats that make them honest.

use std::io::{self, Write};

use crate::results::{BuildReport, Metric, Notes, SetReport, SituationReport};

/// The opening lines of every report.
pub fn write_header(out: &mut impl Write, title: &str, notes: &Notes) -> io::Result<()> {
    writeln!(out, "{title}")?;
    writeln!(out, "  {}", notes.uncalibrated)?;
    writeln!(
        out,
        "  data: {}, pack {} (baseline {}, gwsim {})",
        notes.pack.origin,
        notes.pack.short_hash(),
        notes.pack.baseline,
        notes.pack.gwsim_version
    )?;
    if !notes.drafts.is_empty() {
        writeln!(
            out,
            "  warning: {} draft skill(s) used; see the notes at the end",
            notes.drafts.len()
        )?;
    }
    writeln!(out)
}

/// The closing notes of every report.
pub fn write_footer(out: &mut impl Write, notes: &Notes) -> io::Result<()> {
    writeln!(out, "Notes")?;
    writeln!(out, "  {}", notes.uncalibrated)?;
    if notes.assumptions.is_empty() {
        writeln!(out, "  assumptions touched: none")?;
    } else {
        writeln!(out, "  assumptions touched:")?;
        for a in &notes.assumptions {
            writeln!(out, "    {} [{}] {}", a.id, a.status, a.statement)?;
        }
    }
    if notes.drafts.is_empty() {
        writeln!(out, "  draft skills used: none")?;
    } else {
        writeln!(
            out,
            "  warning: draft skills used ({}); their encodings are not yet reviewed \
             against the wiki, so results that depend on them may change:",
            notes.drafts.len()
        )?;
        for chunk in notes.drafts.chunks(4) {
            writeln!(out, "    {}", chunk.join(", "))?;
        }
    }
    if notes.coverage.is_empty() {
        writeln!(out, "  coverage limits: none that apply")?;
    } else {
        writeln!(out, "  coverage limits:")?;
        for line in &notes.coverage {
            writeln!(out, "    {line}")?;
        }
    }
    writeln!(
        out,
        "  data pack: {} (baseline {}, gwsim {}, from {})",
        notes.pack.content_hash, notes.pack.baseline, notes.pack.gwsim_version, notes.pack.origin
    )?;
    let runs: Vec<String> = notes.runs.iter().map(|n| n.to_string()).collect();
    writeln!(
        out,
        "  seeds: master {}, runs {}",
        notes.seed,
        if runs.is_empty() {
            "none".to_owned()
        } else {
            runs.join(" + ")
        }
    )
}

/// Report 1: each slot's bar and codes.
pub fn write_builds(out: &mut impl Write, builds: &[BuildReport]) -> io::Result<()> {
    writeln!(out, "Builds")?;
    for build in builds {
        writeln!(
            out,
            "  {} ({}, {})",
            build.slot, build.kind, build.professions
        )?;
        let skills: Vec<&str> = build
            .skills
            .iter()
            .map(|s| s.as_deref().unwrap_or("(empty)"))
            .collect();
        writeln!(out, "    skills:    {}", skills.join(", "))?;
        writeln!(out, "    skill code:     {}", build.skill_code)?;
        if let Some(code) = &build.equipment_code {
            writeln!(out, "    equipment code: {code}")?;
        }
        for line in &build.equipment {
            writeln!(out, "    {line}")?;
        }
    }
    writeln!(
        out,
        "  (equipment codes carry runes and insignias only; PvE characters cannot load \
         them, and in game a hero's carries weapons only)"
    )?;
    writeln!(out)
}

fn interval(metric: &Metric, scale: f64, digits: usize) -> String {
    format!(
        "{:.*}  ({:.*} to {:.*})",
        digits,
        metric.mean * scale,
        digits,
        metric.low * scale,
        digits,
        metric.high * scale
    )
}

/// Report 2: one situation's metrics with their 95% intervals.
pub fn write_metrics(out: &mut impl Write, situation: &SituationReport) -> io::Result<()> {
    let m = &situation.metrics;
    let label = situation
        .slug
        .as_deref()
        .map(|slug| format!("{} [{slug}]", situation.name))
        .unwrap_or_else(|| situation.name.clone());
    writeln!(out, "{label}")?;
    writeln!(out, "  runs:          {} ({})", m.runs, m.stop)?;
    writeln!(
        out,
        "  win rate:      {} %   {} of {} won",
        interval(&m.win_rate, 100.0, 1),
        m.wins,
        m.runs
    )?;
    if m.wins > 0 {
        writeln!(
            out,
            "  clear time:    {} s   median {:.1} s",
            interval(&m.clear_time_s, 1.0, 1),
            m.clear_time_s.median
        )?;
    } else {
        writeln!(out, "  clear time:    no wins")?;
    }
    writeln!(out, "  deaths:        {}", interval(&m.deaths, 1.0, 2))?;
    writeln!(
        out,
        "  damage taken:  {}",
        interval(&m.damage_taken, 1.0, 0)
    )?;
    writeln!(out, "  energy left:   {}", interval(&m.energy_left, 1.0, 0))?;
    if m.dp_end.mean > 0.0 {
        writeln!(out, "  DP at end:     {} %", interval(&m.dp_end, 1.0, 1))?;
    }
    writeln!(out)
}

/// The weighted aggregate of a set.
pub fn write_set(out: &mut impl Write, set: &SetReport) -> io::Result<()> {
    writeln!(out, "Set {}", set.name)?;
    writeln!(
        out,
        "  weighted win rate:   {:.1} %",
        set.weighted_win_rate * 100.0
    )?;
    if let Some(clear) = set.weighted_clear_time_s {
        writeln!(out, "  weighted clear time: {clear:.1} s")?;
    }
    writeln!(out)
}

/// Report 4: contributions per slot and skill, what interrupts stopped, and
/// the energy timeline.
pub fn write_contributions(out: &mut impl Write, situation: &SituationReport) -> io::Result<()> {
    writeln!(out, "  Contributions per run")?;
    writeln!(
        out,
        "    {:<10} {:<24} {:>6} {:>8} {:>8} {:>8} {:>8} {:>6}",
        "slot", "skill", "uses", "damage", "healing", "overheal", "prevent", "intr"
    )?;
    for row in &situation.contributions {
        writeln!(
            out,
            "    {:<10} {:<24} {:>6.1} {:>8.0} {:>8.0} {:>8.0} {:>8.0} {:>6.2}",
            row.slot,
            truncate(&row.skill, 24),
            row.uses,
            row.damage,
            row.healing,
            row.overhealing,
            row.mitigation,
            row.interrupts
        )?;
    }
    if !situation.stopped.is_empty() {
        writeln!(out, "  Interrupts landed")?;
        for row in &situation.stopped {
            writeln!(
                out,
                "    {} stopped {}: {:.2} per run",
                row.by, row.stopped, row.per_run
            )?;
        }
    }
    writeln!(out, "  Energy over time (mean, every 5 s)")?;
    for timeline in &situation.energy {
        let samples: Vec<String> = timeline
            .per_second
            .iter()
            .step_by(5)
            .map(|e| format!("{e:>3.0}"))
            .collect();
        writeln!(out, "    {:<10} {}", timeline.slot, samples.join(" "))?;
    }
    writeln!(out)
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_owned()
    } else {
        let mut cut: String = text.chars().take(width - 1).collect();
        cut.push('…');
        cut
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::{
        AssumptionNote, ContributionRow, EnergyTimeline, Metrics, PackInfo, StoppedRow,
    };

    fn metric(mean: f64, low: f64, high: f64) -> Metric {
        Metric {
            n: 64,
            mean,
            median: mean,
            low,
            high,
        }
    }

    fn situation() -> SituationReport {
        SituationReport {
            slug: Some("kournan-patrol-hm".to_owned()),
            name: "Kournan patrol HM".to_owned(),
            weight: 1.0,
            metrics: Metrics {
                runs: 64,
                wins: 60,
                win_rate: metric(0.9375, 0.85, 0.975),
                clear_time_s: metric(43.4, 42.3, 44.6),
                deaths: metric(0.08, 0.01, 0.14),
                damage_taken: metric(2042.0, 1944.0, 2139.0),
                energy_left: metric(147.0, 142.0, 153.0),
                dp_end: metric(1.2, 0.2, 2.2),
                stop: "Fixed".to_owned(),
            },
            contributions: vec![ContributionRow {
                slot: "hero 7".to_owned(),
                skill: "Shelter".to_owned(),
                uses: 2.0,
                damage: 0.0,
                healing: 0.0,
                overhealing: 0.0,
                mitigation: 161.0,
                interrupts: 0.0,
            }],
            stopped: vec![StoppedRow {
                by: "Cry of Frustration".to_owned(),
                stopped: "Fireball".to_owned(),
                per_run: 0.67,
            }],
            energy: vec![EnergyTimeline {
                slot: "player".to_owned(),
                per_second: (0..12).map(|t| 42.0 - f64::from(t)).collect(),
            }],
            runs: Vec::new(),
        }
    }

    fn notes() -> Notes {
        Notes {
            uncalibrated: crate::results::UNCALIBRATED.to_owned(),
            assumptions: vec![AssumptionNote {
                id: "A-015".to_owned(),
                statement: "The health and armor of a created spirit.".to_owned(),
                status: "Assumed".to_owned(),
            }],
            drafts: vec!["Shelter".to_owned()],
            coverage: vec!["128 of 141 Mesmer skills unavailable".to_owned()],
            pack: PackInfo {
                content_hash: "0123456789abcdef".to_owned(),
                baseline: "2026-09-22".to_owned(),
                gwsim_version: "0.1.0".to_owned(),
                origin: "data".to_owned(),
            },
            seed: 1,
            runs: vec![64],
        }
    }

    #[test]
    fn report_2_and_4_text() {
        let mut out = Vec::new();
        write_header(&mut out, "gwsim evaluate: M1 Mesmerway", &notes()).unwrap();
        write_metrics(&mut out, &situation()).unwrap();
        write_contributions(&mut out, &situation()).unwrap();
        write_footer(&mut out, &notes()).unwrap();
        insta::assert_snapshot!(String::from_utf8(out).unwrap());
    }
}
