//! The shape of a `data/skills/**/*.ron` file (§8.3).

use serde::{Deserialize, Serialize};

use crate::core::{Attribute, Campaign, Profession, RangeBand, SkillType, TitleTrack};
use crate::dsl;
use crate::ids::{SkillId, WikiTitle};
use crate::provenance::Provenance;
use crate::units::Seconds;

/// One skill.
///
/// A skill file has three parts, and the split matters:
///
/// - **header fields**, which the extractor seeds and a person maintains;
/// - **`extracted`**, the numbers the extractor read off the wiki verbatim;
/// - **`encoding`**, written by hand, saying what the skill actually does.
///
/// Keeping `extracted` separate from `encoding` is what makes §17.3's check
/// possible: the encoding's scaled values must agree with the numbers the
/// wiki published, and a disagreement is a bug in one of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Skill {
    /// The id this skill has in template codes.
    pub id: SkillId,
    /// The skill's name, as the wiki spells it.
    pub name: String,
    /// The wiki page the facts came from.
    pub wiki: WikiTitle,

    /// The profession that owns it. [`None`] for common and monster skills.
    #[serde(default)]
    pub profession: Option<Profession>,
    /// The attribute it scales on. [`None`] for skills that do not scale.
    #[serde(default)]
    pub attribute: Option<Attribute>,

    /// The skill's type, which decides aftercast and queue behaviour.
    pub kind: SkillType,
    /// Whether it is elite. At most one elite fits on a bar.
    #[serde(default)]
    pub elite: bool,
    /// Whether it is PvE-only. At most three fit on a bar, and none on a hero.
    #[serde(default)]
    pub pve_only: bool,
    /// The title track it scales on. Only PvE-only skills have one.
    #[serde(default)]
    pub title_track: Option<TitleTrack>,
    /// The campaign it comes from.
    pub campaign: Campaign,

    pub cost: Cost,
    /// How long it takes to activate.
    pub activation: Seconds,
    /// How long before it can be used again.
    pub recharge: Seconds,

    pub target: TargetKind,
    /// How far it reaches. [`None`] for untargeted skills.
    #[serde(default)]
    pub range: Option<RangeBand>,
    /// The area it covers, for skills that hit more than their target.
    #[serde(default)]
    pub aoe: Option<Aoe>,
    /// Whether it fires a projectile, and what kind.
    #[serde(default)]
    pub projectile: Option<ProjectileKind>,

    #[serde(default)]
    pub flags: SkillFlags,

    /// The numbers the extractor read from the wiki.
    #[serde(default)]
    pub extracted: Extracted,

    /// What the skill does. [`None`] while the file is `NumbersOnly`.
    #[serde(default)]
    pub encoding: Option<Encoding>,

    pub provenance: Provenance,
}

/// What using a skill costs.
///
/// Every field defaults to zero, so a file only states the costs a skill
/// actually has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cost {
    /// Energy paid at the start of activation.
    #[serde(default)]
    pub energy: u16,
    /// Adrenaline strikes needed.
    #[serde(default)]
    pub adrenaline: u16,
    /// Health sacrificed, as a percentage of maximum. Taken *after* a
    /// successful activation, not at the start.
    #[serde(default)]
    pub sacrifice_pct: u8,
    /// Energy regeneration pips consumed while maintained.
    #[serde(default)]
    pub upkeep: i8,
    /// Overcast incurred, formerly called exhaustion.
    #[serde(default)]
    pub overcast: u16,
}

impl Cost {
    /// Whether the skill costs nothing at all. True of most signets.
    pub fn is_free(&self) -> bool {
        *self == Cost::default()
    }
}

/// What a skill can be pointed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetKind {
    /// The user, and only the user.
    #[serde(rename = "Self")]
    SelfOnly,
    Foe,
    Ally,
    /// An ally other than the user.
    OtherAlly,
    AllyOrSelf,
    Spirit,
    Corpse,
    /// A point on the ground.
    Location,
    Minion,
    /// Nothing is targeted: wards, most shouts, flash enchantments.
    None,
}

/// The area a skill covers beyond its target.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Aoe {
    /// A named band from `core/ranges.ron`.
    Band(RangeBand),
    /// An explicit radius in gwinches, for skills that match no band.
    ///
    /// Exists because of A-025: about fifteen skills use 240 where "nearby" is
    /// 252, and the difference is real rather than a rounding of the wiki's.
    Radius(f32),
}

/// How a skill's projectile travels, if it has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectileKind {
    /// Ordinary travel time, and it can be blocked or miss.
    Standard,
    /// Arrives instantly. Most spells that show a projectile.
    Instant,
    /// Arcs, and is affected by terrain height.
    Arcing,
}

/// Properties that do not fit anywhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillFlags {
    /// Any damage interrupts it, as though the user were Dazed.
    #[serde(default)]
    pub easily_interrupted: bool,
    /// It needs touch range rather than its type's usual range.
    #[serde(default)]
    pub touch: bool,
    /// Its range is half of what its band would give.
    ///
    /// A flag rather than a `RangeBand` variant, because "half range" means
    /// half of *this skill's* range and so has no fixed value.
    #[serde(default)]
    pub half_range: bool,
    /// It cannot be used unless a corpse is in range.
    #[serde(default)]
    pub needs_corpse: bool,
    /// It cannot be blocked.
    #[serde(default)]
    pub unblockable: bool,
}

/// The numbers the extractor read off the wiki.
///
/// These are recorded separately from the encoding so the two can be checked
/// against each other (§17.3). They are the wiki's claims; the encoding is our
/// interpretation of them.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extracted {
    /// Values that change with attribute rank.
    #[serde(default)]
    pub scaled: Vec<ScaledNumber>,
    /// Values that do not.
    #[serde(default)]
    pub fixed: Vec<FixedNumber>,
    /// A hash of the wiki's description, never the description itself.
    ///
    /// Lets the extractor's diff report "the wording changed" without ever
    /// storing ArenaNet's text in the repository (C2, §8.10).
    #[serde(default)]
    pub description_hash: Option<String>,
}

/// A wiki value that changes with attribute rank.
///
/// All three rank points are stored because the wiki publishes all three, and
/// checking that `r12` agrees with `r0` and `r15` catches transcription errors
/// (T1.2.8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScaledNumber {
    /// What the number means, in our words, such as `damage` or `duration`.
    pub label: String,
    /// The value at rank 0.
    pub r0: i32,
    /// The value at rank 12, as the wiki states it.
    pub r12: i32,
    /// The value at rank 15.
    pub r15: i32,
    /// Set when the skill rounds differently from the usual rule, which makes
    /// the `r12` check a warning rather than an error.
    #[serde(default)]
    pub special_rounding: bool,
}

impl ScaledNumber {
    /// The value this number *should* have at rank 12, under the usual rule
    /// `round(r0 + rank * (r15 - r0) / 15)`.
    pub fn expected_r12(&self) -> i32 {
        let span = f64::from(self.r15 - self.r0);
        let value = f64::from(self.r0) + 12.0 * span / 15.0;
        value.round() as i32
    }

    /// Whether the stated `r12` matches the usual rounding rule.
    pub fn r12_is_consistent(&self) -> bool {
        self.r12 == self.expected_r12()
    }
}

/// A wiki value that does not change with attribute rank.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixedNumber {
    /// What the number means, in our words.
    pub label: String,
    pub value: i32,
}

/// What a skill does, written by hand.
///
/// The DSL types are placeholders until WP1.5; see [`crate::dsl`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Encoding {
    /// The steps the skill performs when used.
    #[serde(default)]
    pub effects: Vec<dsl::Action>,
    /// Named effects the steps refer to: hexes, enchantments, stances.
    #[serde(default)]
    pub effect_defs: Vec<dsl::EffectDef>,
    /// A Rust handler, for the skills the DSL cannot express (§8.5).
    #[serde(default)]
    pub handler: Option<dsl::HandlerRef>,
    /// When the AI should use it.
    #[serde(default)]
    pub ai: Option<dsl::AiHints>,
    /// What the skill is for. Used by the optimiser to narrow searches.
    #[serde(default)]
    pub roles: Vec<RoleTag>,
}

/// What a skill is for.
///
/// Most tags are derived from the encoding; a file may add more by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RoleTag {
    Damage,
    Aoe,
    Pressure,
    Spike,
    Interrupt,
    Hex,
    EnchantmentRemoval,
    HexRemoval,
    ConditionRemoval,
    Healing,
    Protection,
    EnergyManagement,
    Snare,
    Resurrection,
    PartyBuff,
    Minion,
    Spirit,
    Defence,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provenance::ReviewStatus;

    /// The §8.3 example, with the design's own illustrative numbers.
    const FIREBALL: &str = r#"(
    id: 0,
    name: "Fireball",
    wiki: "Fireball",
    profession: Some(Elementalist),
    attribute: Some(FireMagic),
    kind: Spell,
    elite: false,
    pve_only: false,
    campaign: Prophecies,
    cost: (energy: 10),
    activation: 2.0,
    recharge: 5.0,
    target: Foe,
    range: Some(Casting),
    aoe: Some(Band(Adjacent)),
    projectile: Some(Standard),
    extracted: (
        scaled: [
            (label: "damage", r0: 7, r12: 91, r15: 112),
        ],
    ),
    encoding: Some((
        effects: [
            Damage(to: Adjacent(TargetFoe), kind: Some(Fire), amount: Scaled(7, 112)),
        ],
        roles: [Damage, Aoe],
    )),
    provenance: (
        sources: ["https://wiki.guildwars.com/wiki/Fireball"],
        crawled: "2026-09-22",
        review: Draft,
    ),
)"#;

    fn fireball() -> Skill {
        ron::from_str(FIREBALL).expect("the example skill should parse")
    }

    #[test]
    fn the_design_example_parses() {
        let skill = fireball();
        assert_eq!(skill.name, "Fireball");
        assert_eq!(skill.profession, Some(Profession::Elementalist));
        assert_eq!(skill.attribute, Some(Attribute::FireMagic));
        assert_eq!(skill.kind, SkillType::Spell);
        assert_eq!(skill.cost.energy, 10);
        assert_eq!(skill.activation.ms(), 2000);
        assert_eq!(skill.recharge.ms(), 5000);
        assert_eq!(skill.target, TargetKind::Foe);
        assert_eq!(skill.provenance.review, ReviewStatus::Draft);
    }

    #[test]
    fn the_design_example_round_trips() {
        // T1.2.3's done criterion. Compared as values, not text: RON's pretty
        // printer lays struct variants out differently from a hand-written
        // file, so byte equality is the wrong test (T1.2.1 §6).
        let original = fireball();
        let text = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default())
            .expect("should serialise");
        let back: Skill = ron::from_str(&text).expect("should parse again");
        assert_eq!(back, original);
    }

    #[test]
    fn omitted_optional_fields_take_their_defaults() {
        let skill = fireball();
        assert!(!skill.flags.easily_interrupted);
        assert!(!skill.flags.touch);
        assert_eq!(skill.title_track, None);
        assert_eq!(skill.cost.adrenaline, 0);
        assert_eq!(skill.cost.upkeep, 0);
        assert_eq!(skill.extracted.description_hash, None);
        assert!(skill.extracted.fixed.is_empty());
    }

    #[test]
    fn a_misspelled_field_is_rejected() {
        // Without deny_unknown_fields this would parse and the real recharge
        // would silently stay at its default.
        let broken = FIREBALL.replace("recharge: 5.0", "recharge_time: 5.0");
        let error = ron::from_str::<Skill>(&broken).expect_err("should be rejected");
        let message = error.to_string();
        assert!(
            message.contains("recharge_time"),
            "the error should name the offending field: {message}"
        );
    }

    #[test]
    fn a_numbers_only_skill_has_no_encoding() {
        let text = FIREBALL
            .replace("review: Draft", "review: NumbersOnly")
            .replace(
                r#"encoding: Some((
        effects: [
            Damage(to: Adjacent(TargetFoe), kind: Some(Fire), amount: Scaled(7, 112)),
        ],
        roles: [Damage, Aoe],
    )),"#,
                "encoding: None,",
            );
        let skill: Skill = ron::from_str(&text).expect("should parse");
        assert_eq!(skill.encoding, None);
        assert_eq!(skill.provenance.review, ReviewStatus::NumbersOnly);
    }

    #[test]
    fn self_targeting_is_spelled_self_in_a_data_file() {
        // `Self` is a Rust keyword, so the variant is SelfOnly. Authors should
        // not have to know that.
        #[derive(Deserialize)]
        struct Holder {
            target: TargetKind,
        }
        let holder: Holder = ron::from_str("(target: Self)").expect("should parse");
        assert_eq!(holder.target, TargetKind::SelfOnly);

        let text = ron::to_string(&TargetKind::SelfOnly).unwrap();
        assert_eq!(text, "Self");
    }

    #[test]
    fn scaled_numbers_check_their_own_middle_value() {
        // round(7 + 12 * (112 - 7) / 15) = round(7 + 84) = 91.
        let number = ScaledNumber {
            label: "damage".to_owned(),
            r0: 7,
            r12: 91,
            r15: 112,
            special_rounding: false,
        };
        assert_eq!(number.expected_r12(), 91);
        assert!(number.r12_is_consistent());

        let wrong = ScaledNumber { r12: 90, ..number };
        assert!(!wrong.r12_is_consistent());
    }

    #[test]
    fn mistrusts_published_values_are_consistent() {
        // DESIGN 20.4 gives Mistrust as 10 at rank 0, 66 at 12 and 80 at 15.
        // round(10 + 12 * 70 / 15) = round(66.0) = 66.
        let number = ScaledNumber {
            label: "damage".to_owned(),
            r0: 10,
            r12: 66,
            r15: 80,
            special_rounding: false,
        };
        assert_eq!(number.expected_r12(), 66);
        assert!(number.r12_is_consistent());
    }

    #[test]
    fn an_aoe_can_be_a_band_or_an_explicit_radius() {
        #[derive(Deserialize)]
        struct Holder {
            aoe: Aoe,
        }
        let band: Holder = ron::from_str("(aoe: Band(Nearby))").unwrap();
        assert_eq!(band.aoe, Aoe::Band(RangeBand::Nearby));

        // A-025: the skills that use 240 rather than nearby's 252.
        let radius: Holder = ron::from_str("(aoe: Radius(240.0))").unwrap();
        assert_eq!(radius.aoe, Aoe::Radius(240.0));
    }

    #[test]
    fn a_free_skill_is_recognised_as_free() {
        assert!(Cost::default().is_free());
        assert!(
            !Cost {
                energy: 5,
                ..Cost::default()
            }
            .is_free()
        );
        // Signets cost nothing at all, which is their defining property.
        let signet: Cost = ron::from_str("()").unwrap();
        assert!(signet.is_free());
    }

    #[test]
    fn an_activation_time_must_be_a_whole_millisecond() {
        let broken = FIREBALL.replace("activation: 2.0", "activation: 0.00075");
        assert!(ron::from_str::<Skill>(&broken).is_err());
    }
}
