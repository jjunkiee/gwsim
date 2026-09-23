//! From a [`RawSkill`] to the typed numbers part of a skill file (T2.4.2 to
//! T2.4.5).
//!
//! Every rule here comes from T2.1 §4. A string that matches no rule becomes
//! a [`NormaliseWarning`] naming the page; nothing panics, because one odd
//! page must not stop a 1,400-page seed.

use gwsim_data::core::{Campaign, RangeBand, SkillType, TitleTrack};
use gwsim_data::provenance::{Provenance, ReviewStatus};
use gwsim_data::skill::{
    Aoe, Cost, Extracted, ProjectileKind, ScaledNumber, Skill, SkillFlags, TargetKind,
};
use gwsim_data::{IsoDate, Seconds, SkillId, WikiTitle};

use crate::discovery::{parse_attribute, parse_campaign, parse_profession, squash};
use crate::html::collapse;
use crate::skill::{Progression, RawSkill};

/// A value the normaliser could not place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormaliseWarning {
    pub page: String,
    pub message: String,
}

/// Why a page could not become a skill at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormaliseError {
    /// Neither the index nor the page gives an id; guessing is not allowed.
    NoId(String),
    /// The Type row is missing or names no known type.
    NoType(String),
}

impl std::fmt::Display for NormaliseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NormaliseError::NoId(page) => {
                write!(f, "{page}: no skill id on the page or in the index")
            }
            NormaliseError::NoType(page) => write!(f, "{page}: no recognisable skill type"),
        }
    }
}

impl std::error::Error for NormaliseError {}

/// A normalised skill and everything noticed on the way.
#[derive(Debug, Clone, PartialEq)]
pub struct Normalised {
    pub skill: Skill,
    pub warnings: Vec<NormaliseWarning>,
    /// Whether it is a monster skill, which decides its folder.
    pub monster: bool,
}

/// Seconds from a stat value: `0.75`, `01.5`, `¾`, `1½`, `2`.
pub fn parse_seconds(text: &str) -> Option<Seconds> {
    let fraction = parse_fraction(text)?;
    Seconds::from_secs_f64(fraction).ok()
}

/// A number with an optional vulgar fraction, as the wiki writes durations.
pub fn parse_fraction(text: &str) -> Option<f64> {
    let text = collapse(text);
    if text.is_empty() {
        return None;
    }
    if let Ok(value) = text.parse::<f64>() {
        return Some(value);
    }
    let (whole, fraction) = match text.char_indices().find(|(_, c)| "¼½¾".contains(*c)) {
        Some((index, c)) => (&text[..index], Some(c)),
        None => (text.as_str(), None),
    };
    let whole = if whole.trim().is_empty() {
        0.0
    } else {
        whole.trim().parse::<f64>().ok()?
    };
    let part = match fraction {
        Some('¼') => 0.25,
        Some('½') => 0.5,
        Some('¾') => 0.75,
        _ => 0.0,
    };
    Some(whole + part)
}

/// A whole number from a stat, ignoring a trailing `%` or `+`.
fn parse_integer(text: &str) -> Option<i32> {
    let trimmed = text.trim().trim_end_matches(['%', '+']).trim();
    trimmed.parse().ok()
}

/// The skill type and elite flag from the Type row.
///
/// "Elite  skill" has two spaces on the Soul Twisting page (T2.1 §3), so
/// whitespace is collapsed before matching.
pub fn parse_skill_type(text: &str) -> Option<(SkillType, bool)> {
    let text = collapse(text);
    let (elite, rest) = match text.strip_prefix("Elite ") {
        Some(rest) => (true, rest.trim()),
        None => (false, text.as_str()),
    };
    let wanted = squash(rest);
    SkillType::ALL
        .into_iter()
        .find(|kind| squash(&format!("{kind:?}")) == wanted)
        .map(|kind| (kind, elite))
}

/// Target, range, area and flags, from the page's categories (T2.1 §3).
pub fn parse_targeting(
    raw: &RawSkill,
    kind: SkillType,
) -> (TargetKind, Option<RangeBand>, Option<Aoe>, SkillFlags) {
    let target = if raw.in_category("Skills that target foes") {
        TargetKind::Foe
    } else if raw.in_category("Skills that target other allies") {
        TargetKind::OtherAlly
    } else if raw.in_category("Skills that target allies") {
        TargetKind::Ally
    } else if raw.in_category("Skills that target dead party members")
        || raw.in_category("Skills that target corpses")
    {
        TargetKind::Corpse
    } else if raw.in_category("Skills that target spirits") {
        TargetKind::Spirit
    } else if raw.in_category("Untargeted skills") {
        TargetKind::None
    } else {
        TargetKind::SelfOnly
    };

    let mut flags = SkillFlags {
        easily_interrupted: raw.in_category("Easily interrupted skills"),
        touch: raw.in_category("Touch skills"),
        half_range: raw.in_category("Half ranged skills"),
        needs_corpse: raw.in_category("Skills that cause Exploits Corpse"),
        unblockable: false,
    };

    let is_attack = kind.is_a(SkillType::AttackSkill);
    let targeted = matches!(
        target,
        TargetKind::Foe
            | TargetKind::Ally
            | TargetKind::OtherAlly
            | TargetKind::Corpse
            | TargetKind::Spirit
    );
    let range = if flags.touch {
        Some(RangeBand::Touch)
    } else if raw.in_category("Casting ranged skills") || flags.half_range {
        Some(RangeBand::Casting)
    } else if is_attack || raw.in_category("Point blank skills") {
        // Attack skills reach as far as the weapon does.
        None
    } else if targeted {
        // Most targeted spells carry no range category but are casting
        // ranged, which is the wiki's default for targeted non-attacks.
        Some(RangeBand::Casting)
    } else {
        None
    };
    if flags.touch {
        flags.half_range = false;
    }

    let aoe = [
        ("Skills with adjacent AoE", RangeBand::Adjacent),
        ("Skills with nearby AoE", RangeBand::Nearby),
        ("Skills with in the area AoE", RangeBand::InTheArea),
        ("Skills with earshot AoE", RangeBand::Earshot),
        ("Skills with spirit AoE", RangeBand::SpiritRange),
        // Large spirit range is the nature-ritual band since 2026-08-26 (A-018).
        ("Skills with large spirit AoE", RangeBand::NatureRitual),
        ("Skills with party AoE", RangeBand::Party),
    ]
    .into_iter()
    .find(|(category, _)| raw.in_category(category))
    .map(|(_, band)| Aoe::Band(band));

    (target, range, aoe, flags)
}

/// The title track a progression's attribute label names, if any.
pub fn parse_title_track(label: &str) -> Option<TitleTrack> {
    let name = label.strip_suffix(" rank")?;
    TitleTrack::ALL
        .into_iter()
        .find(|track| squash(&format!("{track:?}")) == squash(name))
}

/// The title-rank column that gives each effective rank, for the tracks that
/// share the Asura table (T2.1 §3, `titles.ron`).
const TITLE_COLUMN_FOR_RANK_12: u8 = 4;
const TITLE_COLUMN_FOR_RANK_15: u8 = 5;

/// A row label in our form: lower case, footnote digits and stray
/// whitespace removed. "Energy gain1" becomes "energy gain".
pub fn normalise_label(label: &str) -> String {
    let collapsed = collapse(label).to_lowercase();
    let trimmed = collapsed.trim_end_matches(|c: char| c.is_ascii_digit());
    // Only strip digits that were stuck to a word, not a label that is a
    // number in its own right.
    if trimmed.ends_with(|c: char| c.is_alphabetic()) {
        trimmed.to_owned()
    } else {
        collapsed
    }
}

/// Scaled numbers from a progression table (T2.4.3).
///
/// Every rank is recomputed with the Gr formula; a row whose page values
/// differ keeps the page's values and is flagged `special_rounding`.
pub fn scaled_numbers(
    progression: &Progression,
    title_track: Option<TitleTrack>,
    page: &str,
    warnings: &mut Vec<NormaliseWarning>,
) -> Vec<ScaledNumber> {
    let mut numbers = Vec::new();
    for row in &progression.rows {
        let label = normalise_label(&row.label);
        let (r0, r12, r15) = if title_track.is_some() {
            (
                row.at(0),
                row.at(TITLE_COLUMN_FOR_RANK_12),
                row.at(TITLE_COLUMN_FOR_RANK_15),
            )
        } else {
            (row.at(0), row.at(12), row.at(15))
        };
        let (Some(r0), Some(r12), Some(r15)) = (r0, r12, r15) else {
            warnings.push(NormaliseWarning {
                page: page.to_owned(),
                message: format!("progression row {label:?} lacks a rank-0, 12 or 15 value"),
            });
            continue;
        };

        let special_rounding = if title_track.is_some() {
            gwsim_data::derived::scaled(r0, r15, 12) != r12
        } else {
            row.values
                .iter()
                .any(|(rank, value)| gwsim_data::derived::scaled(r0, r15, *rank) != *value)
        };
        numbers.push(ScaledNumber {
            label,
            r0,
            r12,
            r15,
            special_rounding,
        });
    }
    numbers
}

/// Turns a raw page into the numbers part of a skill file.
///
/// `index_id` is the id discovery found for the title; the page's own id is
/// the fallback, and a disagreement is a warning.
pub fn normalise(
    raw: &RawSkill,
    title: &WikiTitle,
    index_id: Option<SkillId>,
    crawled: IsoDate,
) -> Result<Normalised, NormaliseError> {
    let page = title.as_str().to_owned();
    let mut warnings = Vec::new();
    let mut warn = |message: String| {
        warnings.push(NormaliseWarning {
            page: page.clone(),
            message,
        })
    };
    let mut notes: Vec<String> = Vec::new();

    let page_id = raw.id.map(SkillId);
    let id = match (index_id, page_id) {
        (Some(index), Some(on_page)) if index != on_page => {
            warn(format!(
                "the index gives id {index} but the page shows {on_page}; using the index"
            ));
            index
        }
        (Some(index), _) => index,
        (None, Some(on_page)) => on_page,
        (None, None) => return Err(NormaliseError::NoId(page)),
    };

    let (kind, elite) = raw
        .type_text
        .as_deref()
        .and_then(parse_skill_type)
        .ok_or_else(|| NormaliseError::NoType(page.clone()))?;

    let monster = raw.is_monster();
    let profession = match raw.profession.as_deref() {
        None | Some("Monster") => None,
        Some(text) => {
            let parsed = parse_profession(text);
            if parsed.is_none() {
                warn(format!("unknown profession {text:?}"));
            }
            parsed
        }
    };

    let title_track = raw
        .progression
        .as_ref()
        .and_then(|p| parse_title_track(&p.attribute_label))
        .or_else(|| raw.attribute.as_deref().and_then(parse_title_track));
    let attribute = match raw.attribute.as_deref() {
        None => None,
        Some(text) if text.ends_with(" rank") => {
            if title_track.is_none() {
                warn(format!("title track {text:?} is not modelled yet"));
            }
            None
        }
        Some(text) => {
            let parsed = parse_attribute(text);
            if parsed.is_none() {
                warn(format!("unknown attribute {text:?}"));
            }
            parsed
        }
    };
    if title_track.is_some() {
        notes.push(
            "title-scaled: values are at title ranks 0, 4 and 5 (effective 0, 12, 15)".to_owned(),
        );
    }

    let campaign = raw
        .campaigns
        .iter()
        .find_map(|text| parse_campaign(text))
        .unwrap_or_else(|| {
            warn(format!(
                "no known campaign in {:?}; using Core",
                raw.campaigns
            ));
            Campaign::Core
        });

    let mut cost = Cost::default();
    let mut activation = Seconds::ZERO;
    let mut recharge = Seconds::ZERO;
    for (field, value) in &raw.stats {
        match field.as_str() {
            "Energy" => match parse_integer(value) {
                Some(energy) => cost.energy = energy as u16,
                None => warn(format!("energy {value:?} did not parse")),
            },
            "Adrenaline" => match parse_integer(value) {
                Some(strikes) => cost.adrenaline = strikes as u16,
                None => warn(format!("adrenaline {value:?} did not parse")),
            },
            "Sacrifice" => match parse_integer(value) {
                Some(percent) => {
                    cost.sacrifice_pct = percent as u8;
                    if value.trim_end().ends_with('+') {
                        notes.push(format!("sacrifice is variable ({value} on the page)"));
                    }
                }
                None => warn(format!("sacrifice {value:?} did not parse")),
            },
            "Upkeep" => match parse_integer(value) {
                // The wiki writes upkeep as -1; the file stores pips consumed.
                Some(pips) => cost.upkeep = (pips.abs()) as i8,
                None => warn(format!("upkeep {value:?} did not parse")),
            },
            "Overcast" => match parse_integer(value) {
                Some(overcast) => cost.overcast = overcast as u16,
                None => warn(format!("overcast {value:?} did not parse")),
            },
            "Activation" => match parse_seconds(value) {
                Some(seconds) => activation = seconds,
                None => warn(format!("activation {value:?} did not parse")),
            },
            "Recharge" => match parse_seconds(value) {
                Some(seconds) => recharge = seconds,
                None => warn(format!("recharge {value:?} did not parse")),
            },
            other => warn(format!("unknown stat {other:?} = {value:?}")),
        }
    }

    let (target, range, aoe, flags) = parse_targeting(raw, kind);
    let projectile = raw
        .in_category("Projectile spells")
        .then_some(ProjectileKind::Standard);

    let mut scaled = Vec::new();
    match &raw.progression {
        Some(progression) => {
            scaled = scaled_numbers(progression, title_track, &page, &mut warnings);
        }
        None => notes.push("no progression table".to_owned()),
    }

    // Description values are a cross-check only: every x...y...z quoted in
    // the description should match some progression row's endpoints.
    for quoted in &raw.description_values {
        let (Some(first), Some(last)) = (quoted.first(), quoted.last()) else {
            continue;
        };
        if !scaled.iter().any(|n| n.r0 == *first && n.r15 == *last) {
            warnings.push(NormaliseWarning {
                page: page.clone(),
                message: format!(
                    "the description quotes {} but no progression row runs {first} to {last}",
                    quoted
                        .iter()
                        .map(i32::to_string)
                        .collect::<Vec<_>>()
                        .join("...")
                ),
            });
        }
    }

    if raw.in_category("PvE versions of skills") {
        notes.push("PvE version of a split skill (D3)".to_owned());
    }
    if monster {
        notes.push("monster skill".to_owned());
    }

    let skill = Skill {
        id,
        name: if raw.name.is_empty() {
            page.clone()
        } else {
            raw.name.clone()
        },
        wiki: title.clone(),
        profession,
        attribute,
        kind,
        elite,
        pve_only: raw.pve_only,
        title_track,
        campaign,
        cost,
        activation,
        recharge,
        target,
        range,
        aoe,
        projectile,
        flags,
        extracted: Extracted {
            scaled,
            fixed: Vec::new(),
            description_hash: raw.description_hash.clone(),
        },
        encoding: None,
        provenance: Provenance {
            sources: vec![title.url()],
            crawled,
            review: ReviewStatus::NumbersOnly,
            reviewed_by: None,
            assumptions: Vec::new(),
            notes: notes.join("; "),
        },
    };

    Ok(Normalised {
        skill,
        warnings,
        monster,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fraction_form_on_the_wiki_parses() {
        for (text, ms) in [
            ("0.25", 250),
            ("0.75", 750),
            ("01.5", 1500),
            ("¼", 250),
            ("½", 500),
            ("¾", 750),
            ("1¼", 1250),
            ("1½", 1500),
            ("2", 2000),
            ("45", 45_000),
        ] {
            assert_eq!(parse_seconds(text).map(Seconds::ms), Some(ms), "{text}");
        }
        assert_eq!(parse_seconds(""), None);
        assert_eq!(parse_seconds("soon"), None);
    }

    #[test]
    fn skill_types_parse_with_and_without_elite() {
        assert_eq!(
            parse_skill_type("Elite spell"),
            Some((SkillType::Spell, true))
        );
        assert_eq!(
            parse_skill_type("Hex spell"),
            Some((SkillType::HexSpell, false))
        );
        assert_eq!(
            parse_skill_type("Elite  skill"),
            Some((SkillType::Skill, true))
        );
        assert_eq!(
            parse_skill_type("Elite flash enchantment spell"),
            Some((SkillType::FlashEnchantmentSpell, true))
        );
        assert_eq!(
            parse_skill_type("Binding ritual"),
            Some((SkillType::BindingRitual, false))
        );
        assert_eq!(
            parse_skill_type("Off-hand attack"),
            Some((SkillType::OffHandAttack, false))
        );
        assert_eq!(
            parse_skill_type("Elite melee attack"),
            Some((SkillType::MeleeAttack, true))
        );
        assert_eq!(
            parse_skill_type("Spear attack"),
            Some((SkillType::SpearAttack, false))
        );
        assert_eq!(parse_skill_type("Nonsense"), None);
    }

    #[test]
    fn integers_ignore_percent_and_plus() {
        assert_eq!(parse_integer("33%"), Some(33));
        assert_eq!(parse_integer("5%+"), Some(5));
        assert_eq!(parse_integer("-1"), Some(-1));
    }

    #[test]
    fn labels_lose_footnotes_and_case() {
        assert_eq!(normalise_label("Energy gain1"), "energy gain");
        assert_eq!(normalise_label("Sacrifice\u{a0}%"), "sacrifice %");
        assert_eq!(
            normalise_label("Total Damage (Target)"),
            "total damage (target)"
        );
    }

    #[test]
    fn title_tracks_are_recognised_from_the_rank_label() {
        assert_eq!(parse_title_track("Asura rank"), Some(TitleTrack::Asura));
        assert_eq!(parse_title_track("Norn rank"), Some(TitleTrack::Norn));
        assert_eq!(
            parse_title_track("Ebon Vanguard rank"),
            Some(TitleTrack::EbonVanguard)
        );
        assert_eq!(
            parse_title_track("Hero rank"),
            None,
            "not a PvE skill track"
        );
        assert_eq!(parse_title_track("Domination Magic"), None);
    }
}
