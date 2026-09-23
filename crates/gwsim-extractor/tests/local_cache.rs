//! Local-only tests over the real pages in `.cache/wiki/` (T2.4.6, T2.5.5).
//!
//! The cache holds raw wiki HTML and is never committed (§9.5), so these are
//! `#[ignore]`d and skip cleanly when it is empty. Run them after a crawl:
//!
//! ```text
//! cargo test -p gwsim-extractor --test local_cache -- --ignored --nocapture
//! ```

use std::path::PathBuf;

use gwsim_data::WikiTitle;
use gwsim_extractor::cache::PageCache;
use gwsim_extractor::seed::{extract_foe, extract_skill};

fn cache() -> Option<PageCache> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.cache/wiki");
    let cache = PageCache::new(root);
    if cache.titles().is_empty() {
        eprintln!("the cache is empty; nothing to check");
        return None;
    }
    Some(cache)
}

/// The M1 skills (§20.3), plus the variant skills the Kournan pages name.
pub const M1_SKILLS: [&str; 71] = [
    "Panic",
    "Energy Surge",
    "Cry of Frustration",
    "Mistrust",
    "Unnatural Signet",
    "Shatter Hex",
    "Spiritual Pain",
    "Power Drain",
    "Drain Enchantment",
    "Arcane Echo",
    "Spirit Transfer",
    "Mend Body and Soul",
    "Spirit Light",
    "Protective Was Kaolai",
    "Recuperation",
    "Signet of Spirits",
    "Ancestors' Rage",
    "Spirit Siphon",
    "Lamentation",
    "Life",
    "Soul Twisting",
    "Shelter",
    "Union",
    "Displacement",
    "Armor of Unfeeling",
    "Boon of Creation",
    "Signet of Creation",
    "Blood is Power",
    "Blood Bond",
    "Signet of Lost Souls",
    "Animate Bone Fiend",
    "Putrid Bile",
    "Masochism",
    "Blood of the Master",
    "\"Incoming!\"",
    "\"Fall Back!\"",
    "\"Stand Your Ground!\"",
    "Resurrection Chant",
    "Remove Hex",
    "Air of Superiority",
    "Disrupting Chop",
    "Executioner's Strike",
    "Magehunter Strike",
    "Sprint",
    "Armor of Sanctity",
    "Eremite's Attack",
    "Pious Renewal",
    "Veil of Thorns",
    "\"Never Surrender!\"",
    "Cautery Signet",
    "Mighty Throw",
    "Wild Throw",
    "Crossfire",
    "Infuriating Heat",
    "Precision Shot",
    "Troll Unguent",
    "Whirling Defense",
    "Aftershock",
    "Aura of Restoration",
    "Fireball",
    "Master of Magic",
    "Meteor",
    "Enchanter's Conundrum",
    "Power Spike",
    "Shatter Enchantment",
    "Life Siphon",
    "Strip Enchantment",
    "Convert Hexes",
    "Reversal of Fortune",
    "Shielding Hands",
    "Zealous Benediction",
];

pub const KOURNANS: [&str; 8] = [
    "Kournan Guard",
    "Kournan Zealot",
    "Kournan Phalanx",
    "Kournan Bowman",
    "Kournan Scribe",
    "Kournan Seer",
    "Kournan Oppressor",
    "Kournan Priest",
];

#[test]
#[ignore = "reads the local .cache/, which is never committed"]
fn every_cached_m1_skill_parses_without_errors() {
    let Some(cache) = cache() else { return };
    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    for title in M1_SKILLS {
        match extract_skill(&cache, &WikiTitle(title.to_owned()), None) {
            Ok(normalised) => {
                for warning in normalised.warnings {
                    warnings.push(format!("{title}: {}", warning.message));
                }
            }
            Err(reason) => failures.push(format!("{title}: {reason}")),
        }
    }
    for warning in &warnings {
        eprintln!("warning: {warning}");
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
#[ignore = "reads the local .cache/, which is never committed"]
fn every_cached_kournan_parses_and_names_its_skills() {
    let Some(cache) = cache() else { return };
    let mut failures = Vec::new();
    for title in KOURNANS {
        match extract_foe(&cache, &WikiTitle(title.to_owned()), None) {
            Ok((foe, warnings)) => {
                eprintln!(
                    "{title}: {:?} level {:?} attrs {:?} hm {:?} armor {:?}/{:?}",
                    foe.professions,
                    foe.level,
                    foe.attributes.nm,
                    foe.attributes.hm,
                    foe.armor.default,
                    foe.armor.per_type
                );
                let bar: Vec<String> = foe
                    .skills
                    .iter()
                    .map(|s| format!("{:?}{}", s.skill, if s.hm_only { " (HM)" } else { "" }))
                    .collect();
                eprintln!("  bar: {bar:?}");
                for variant in &foe.variants {
                    eprintln!("  variant {}: {:?}", variant.name, variant.skills);
                }
                for warning in warnings {
                    eprintln!("  warning: {warning}");
                }
            }
            Err(reason) => failures.push(format!("{title}: {reason}")),
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
