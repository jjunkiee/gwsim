//! Encounters, situations, situation sets and benchmarks (§12, D16).

use serde::{Deserialize, Serialize};

use crate::core::Campaign;
use crate::foe::{AiTag, ModeValue};
use crate::ids::{AssumptionId, Slug};
use crate::provenance::Provenance;
use crate::units::Seconds;

// ---------------------------------------------------------------- encounters

/// A fight: which foes, where they stand, and where the party starts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Encounter {
    pub name: String,
    /// The area it happens in, as the wiki names it.
    pub area: String,
    pub campaign: Campaign,
    pub groups: Vec<Group>,
    /// Where the party starts, in gwinches.
    pub party_start: Position,
    pub provenance: Provenance,
}

/// A cluster of foes that reacts as one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub foes: Vec<GroupFoe>,
    pub formation: Formation,
    /// Where the group stands, relative to the party's start.
    pub position: Position,
    /// Levels that override each foe's own. Useful for reusing a roster at a
    /// different difficulty.
    #[serde(default)]
    pub level_override: Option<ModeValue<u8>>,
    #[serde(default)]
    pub ai_tags: Vec<AiTag>,
}

/// One entry in a group's roster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupFoe {
    /// The foe file, by slug.
    pub foe: Slug,
    /// Which of the foe's variants to use.
    #[serde(default)]
    pub variant: Option<String>,
    #[serde(default = "one")]
    pub count: u8,
}

fn one() -> u8 {
    1
}

/// How a group is laid out on the ground.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Formation {
    /// Packed into a circle of this radius.
    Cluster { radius: f32 },
    /// Strung out in a line, this far apart.
    Line { spacing: f32 },
    /// Placed individually, in roster order.
    Explicit(Vec<Position>),
}

/// A point on the ground, in gwinches.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

// ---------------------------------------------------------------- situations

/// An encounter plus the conditions it is fought under.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Situation {
    pub name: String,
    pub encounters: SituationEncounters,
    #[serde(default)]
    pub mode: ModeSwitches,
    pub party_size: u8,
    /// Consumables active for the whole run.
    #[serde(default)]
    pub consumables: Vec<Slug>,
    /// Death penalty the party starts with, as a percentage.
    #[serde(default)]
    pub starting_dp: u8,
    /// Morale boost the party starts with, as a percentage.
    #[serde(default)]
    pub starting_morale: u8,
    /// How long before the fight counts as lost. A-030 supplies 180 seconds
    /// when this is absent.
    #[serde(default)]
    pub timeout: Option<Seconds>,
    /// Overrides for the tactics plan (§11.6, T4.7.5): each field given is
    /// kept, and everything else is generated from the party's builds.
    #[serde(default)]
    pub tactics_overrides: Option<crate::tactics::TacticsOverrides>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub assumption_refs: Vec<AssumptionId>,
}

/// One fight, or several in a row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SituationEncounters {
    Single(Slug),
    /// Fights in order, with a rest between each. A-028 supplies 20 seconds
    /// where a chain does not say.
    Chain(Vec<ChainStep>),
}

/// One fight in a chain, and the rest that follows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChainStep {
    pub encounter: Slug,
    #[serde(default)]
    pub rest_after: Option<Seconds>,
}

/// Which game modes are on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModeSwitches {
    #[serde(default)]
    pub hard_mode: bool,
    #[serde(default)]
    pub reforged_mode: bool,
    #[serde(default)]
    pub dhuums_covenant: bool,
    #[serde(default)]
    pub melandrus_accord: bool,
}

// ------------------------------------------------------------ situation sets

/// A weighted collection of situations, evaluated together.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SituationSet {
    pub name: String,
    pub entries: Vec<SituationSetEntry>,
    pub provenance: Provenance,
}

/// One situation's share of a set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SituationSetEntry {
    pub situation: Slug,
    /// How much this situation counts for. Relative to the other entries.
    pub weight: f64,
}

impl SituationSet {
    /// The total weight, which the entries are scored relative to.
    pub fn total_weight(&self) -> f64 {
        self.entries.iter().map(|entry| entry.weight).sum()
    }
}

// ------------------------------------------------------------- benchmarks

/// A frozen build from an outside source, kept to compare against (D16).
///
/// The source URL and snapshot are recorded because PvXwiki blocks scripted
/// access (C3) and its pages change: without a dated snapshot, a benchmark
/// that stops matching its source cannot be told from one that was mistyped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Benchmark {
    pub name: String,
    /// Where the build was published.
    pub source_url: String,
    /// The archived copy the values were read from.
    #[serde(default)]
    pub snapshot_url: Option<String>,
    /// When that copy was taken.
    #[serde(default)]
    pub snapshot_date: Option<crate::ids::IsoDate>,
    pub slots: Vec<BenchmarkSlot>,
    pub provenance: Provenance,
}

/// One party member of a benchmark team.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkSlot {
    pub kind: SlotKind,
    /// The skill template code, exactly as published.
    pub skill_code: String,
    #[serde(default)]
    pub equipment_code: Option<String>,
    #[serde(default)]
    pub notes: String,
}

/// Who fills a party slot.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SlotKind {
    /// The player.
    Human,
    /// A hero. The slug is recorded where the identity matters.
    Hero(Option<Slug>),
    Henchman(Slug),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The §12.1 example, in the shape the design gives.
    const KOURNAN_PATROL: &str = r#"(
    name: "Kournan patrol",
    area: "Vehtendi Valley",
    campaign: Nightfall,
    groups: [
        (
            foes: [
                (foe: "kournan-guard", variant: Some("axe"), count: 1),
                (foe: "kournan-zealot", count: 1),
                (foe: "kournan-phalanx", count: 1),
                (foe: "kournan-bowman", count: 1),
                (foe: "kournan-scribe", count: 1),
                (foe: "kournan-seer", count: 1),
                (foe: "kournan-oppressor", count: 1),
                (foe: "kournan-priest", count: 1),
            ],
            formation: Cluster(radius: 150.0),
            position: (x: 0.0, y: 1800.0),
            ai_tags: [],
        ),
    ],
    party_start: (x: 0.0, y: 0.0),
    provenance: (
        sources: ["https://wiki.guildwars.com/wiki/Vehtendi_Valley"],
        crawled: "2026-09-22",
        review: Draft,
        assumptions: ["A-006", "A-008"],
    ),
)"#;

    #[test]
    fn the_design_example_encounter_parses() {
        let encounter: Encounter = ron::from_str(KOURNAN_PATROL).expect("should parse");
        assert_eq!(encounter.name, "Kournan patrol");
        assert_eq!(encounter.campaign, Campaign::Nightfall);
        assert_eq!(encounter.groups.len(), 1);
        assert_eq!(encounter.groups[0].foes.len(), 8);
        assert_eq!(
            encounter.groups[0].formation,
            Formation::Cluster { radius: 150.0 }
        );
        // A-006 (composition) and A-008 (start geometry) are both guesses, and
        // the file has to say so.
        assert_eq!(encounter.provenance.assumptions.len(), 2);
    }

    #[test]
    fn the_design_example_encounter_round_trips() {
        // T1.2.6's done criterion.
        let original: Encounter = ron::from_str(KOURNAN_PATROL).expect("should parse");
        let text = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default())
            .expect("should serialise");
        let back: Encounter = ron::from_str(&text).expect("should parse again");
        assert_eq!(back, original);
    }

    #[test]
    fn a_foe_entry_defaults_to_one_and_no_variant() {
        let entry: GroupFoe = ron::from_str(r#"(foe: "kournan-seer")"#).expect("should parse");
        assert_eq!(entry.count, 1);
        assert_eq!(entry.variant, None);
    }

    #[test]
    fn all_three_formations_round_trip() {
        let cases = [
            (
                "Cluster(radius: 150.0)",
                Formation::Cluster { radius: 150.0 },
            ),
            ("Line(spacing: 80.0)", Formation::Line { spacing: 80.0 }),
            (
                "Explicit([(x: 0.0, y: 0.0), (x: 100.0, y: 50.0)])",
                Formation::Explicit(vec![
                    Position { x: 0.0, y: 0.0 },
                    Position { x: 100.0, y: 50.0 },
                ]),
            ),
        ];
        for (text, expected) in cases {
            let parsed: Formation = ron::from_str(text).expect("should parse");
            assert_eq!(parsed, expected);
        }
    }

    #[test]
    fn a_single_encounter_situation_parses() {
        let situation: Situation = ron::from_str(
            r#"(
                name: "Kournan patrol, hard mode",
                encounters: Single("kournan-patrol"),
                mode: (hard_mode: true),
                party_size: 8,
            )"#,
        )
        .expect("should parse");

        assert_eq!(
            situation.encounters,
            SituationEncounters::Single("kournan-patrol".parse().unwrap())
        );
        assert!(situation.mode.hard_mode);
        assert!(!situation.mode.reforged_mode);
        // A-030 supplies the timeout when the file does not.
        assert_eq!(situation.timeout, None);
        assert_eq!(situation.starting_dp, 0);
    }

    #[test]
    fn a_chain_situation_records_the_rest_between_fights() {
        let situation: Situation = ron::from_str(
            r#"(
                name: "Two patrols",
                encounters: Chain([
                    (encounter: "kournan-patrol", rest_after: Some(20.0)),
                    (encounter: "kournan-patrol"),
                ]),
                party_size: 8,
            )"#,
        )
        .expect("should parse");

        let SituationEncounters::Chain(steps) = &situation.encounters else {
            panic!("expected a chain");
        };
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].rest_after.unwrap().ms(), 20_000);
        // The last step's rest is absent, which A-028 fills in.
        assert_eq!(steps[1].rest_after, None);
    }

    #[test]
    fn mode_switches_are_all_off_by_default() {
        let switches = ModeSwitches::default();
        assert!(!switches.hard_mode);
        assert!(!switches.reforged_mode);
        assert!(!switches.dhuums_covenant);
        assert!(!switches.melandrus_accord);
    }

    #[test]
    fn a_situation_set_weights_its_entries() {
        let set: SituationSet = ron::from_str(
            r#"(
                name: "Kournan mix",
                entries: [
                    (situation: "kournan-patrol-nm", weight: 1.0),
                    (situation: "kournan-patrol-hm", weight: 3.0),
                ],
                provenance: (
                    sources: ["https://wiki.guildwars.com/wiki/Vehtendi_Valley"],
                    crawled: "2026-09-22",
                    review: Draft,
                ),
            )"#,
        )
        .expect("should parse");

        assert_eq!(set.entries.len(), 2);
        assert_eq!(set.total_weight(), 4.0);
    }

    #[test]
    fn a_benchmark_keeps_its_template_codes_verbatim() {
        // The codes are the benchmark. Re-encoding them from a decoded build
        // would hide a decoder bug, so they are stored as published strings.
        let benchmark: Benchmark = ron::from_str(
            r#"(
                name: "7 Hero Mesmerway",
                source_url: "https://gwpvx.fandom.com/wiki/Build:Team_-_7_Hero_Mesmerway",
                snapshot_url: Some("https://web.archive.org/web/20260727151256/https://gwpvx.fandom.com/wiki/Build:Team_-_7_Hero_Mesmerway"),
                snapshot_date: Some("2026-07-27"),
                slots: [
                    (kind: Human, skill_code: "OQBTAUBPQaJ4EY6x0BAAAAAAuE"),
                    (kind: Hero(None), skill_code: "OQBTAWBPsBAkDmemuhAONDAAA"),
                ],
                provenance: (
                    sources: ["https://gwpvx.fandom.com/wiki/Build:Team_-_7_Hero_Mesmerway"],
                    crawled: "2026-09-22",
                    review: Draft,
                    notes: "Read from the Wayback snapshot; PvXwiki blocks scripted access (C3).",
                ),
            )"#,
        )
        .expect("should parse");

        assert_eq!(benchmark.slots.len(), 2);
        assert_eq!(benchmark.slots[0].skill_code, "OQBTAUBPQaJ4EY6x0BAAAAAAuE");
        assert_eq!(benchmark.snapshot_date.unwrap().to_string(), "2026-07-27");
    }

    #[test]
    fn a_misspelled_situation_field_is_rejected() {
        let text = r#"(
            name: "x",
            encounters: Single("y"),
            party_size: 8,
            hard_mode: true,
        )"#;
        // hard_mode lives inside `mode`, not at the top level. Without
        // deny_unknown_fields this would silently run in normal mode.
        let error = ron::from_str::<Situation>(text).expect_err("should be rejected");
        assert!(error.to_string().contains("hard_mode"));
    }
}
