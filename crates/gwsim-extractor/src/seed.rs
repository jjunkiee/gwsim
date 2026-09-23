//! `gwsim-extract seed`: write initial data files, never overwriting (WP2.6).
//!
//! Skills are written `NumbersOnly` with no encoding; foes are written
//! `Draft`, with assumption references wherever the wiki is silent. **An
//! existing file is never touched** (Q10): every output is opened with
//! `create_new`, and a file that exists is listed as "exists, not touched".
//!
//! The writers are hand-rolled rather than `ron`'s pretty printer so the
//! files read like the hand-written ones — defaults omitted, one scaled
//! number per line — and so re-seeding produces identical bytes. Every leaf
//! value still goes through `ron`'s own serialiser, so what is written is
//! exactly what the loader reads back.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use gwsim_data::core::{CoreData, DamageType, HmLevelKind};
use gwsim_data::foe::{ArmorTable, Foe, FoeSkill, FoeVariant, ModeValue, SkillRef};
use gwsim_data::provenance::{Provenance, ReviewStatus};
use gwsim_data::skill::Skill;
use gwsim_data::skill_index::SkillIndexFile;
use gwsim_data::{AssumptionId, IsoDate, SkillId, WikiTitle, slugify};
use serde::Serialize;

use crate::cache::PageCache;
use crate::clock::iso_date;
use crate::foe::{RawFoe, parse_foe, traits_for_species};
use crate::normalise::{NormaliseWarning, Normalised, normalise};
use crate::skill::parse_skill;

/// A value as `ron` writes it on one line.
fn ron<T: Serialize + ?Sized>(value: &T) -> String {
    ron::to_string(value)
        .unwrap_or_else(|error| panic!("a data value failed to serialise: {error}"))
}

// ------------------------------------------------------------------ paths

/// Where a skill's file goes, relative to the data root (§8.1).
///
/// Monster skills go to `skills/monster/`; PvE-only and profession-less
/// skills to `skills/common/`; the rest to their profession's folder.
pub fn skill_path(skill: &Skill, monster: bool) -> PathBuf {
    let folder = if monster {
        "monster".to_owned()
    } else if skill.pve_only {
        "common".to_owned()
    } else {
        match skill.profession {
            Some(profession) => format!("{profession:?}").to_lowercase(),
            None => "common".to_owned(),
        }
    };
    PathBuf::from("skills")
        .join(folder)
        .join(format!("{}.ron", slugify(&skill.name)))
}

/// Where a foe's file goes: `creatures/foes/<affiliation>/<slug>.ron`.
pub fn foe_path(foe: &Foe) -> PathBuf {
    PathBuf::from("creatures")
        .join("foes")
        .join(slugify(&foe.affiliation).as_str())
        .join(format!("{}.ron", slugify(&foe.name)))
}

/// What happened to one output file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Written {
    Created,
    /// The file was already there and was not touched (Q10).
    Exists,
}

/// Writes a file only if it does not exist.
pub fn create_new(path: &Path, text: &str) -> io::Result<Written> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            file.write_all(text.as_bytes())?;
            Ok(Written::Created)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(Written::Exists),
        Err(error) => Err(error),
    }
}

// ---------------------------------------------------------------- writers

/// A provenance block, with defaults omitted.
fn write_provenance(out: &mut String, provenance: &Provenance, indent: &str) {
    out.push_str(&format!("{indent}provenance: (\n"));
    out.push_str(&format!(
        "{indent}    sources: {},\n",
        ron(&provenance.sources)
    ));
    out.push_str(&format!(
        "{indent}    crawled: {},\n",
        ron(&provenance.crawled)
    ));
    out.push_str(&format!(
        "{indent}    review: {},\n",
        ron(&provenance.review)
    ));
    if let Some(reviewer) = &provenance.reviewed_by {
        out.push_str(&format!(
            "{indent}    reviewed_by: Some({}),\n",
            ron(reviewer)
        ));
    }
    if !provenance.assumptions.is_empty() {
        out.push_str(&format!(
            "{indent}    assumptions: {},\n",
            ron(&provenance.assumptions)
        ));
    }
    if !provenance.notes.is_empty() {
        out.push_str(&format!("{indent}    notes: {},\n", ron(&provenance.notes)));
    }
    out.push_str(&format!("{indent}),\n"));
}

/// The numbers part of a skill file, in canonical form (T2.6.1).
pub fn write_skill(skill: &Skill) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// {} — seeded from {}\n",
        skill.name,
        skill.wiki.url()
    ));
    out.push_str(
        "// The numbers were written by `gwsim-extract seed`; the encoding is written by hand (§8.3).\n",
    );
    out.push_str("(\n");
    out.push_str(&format!("    id: {},\n", skill.id.get()));
    out.push_str(&format!("    name: {},\n", ron(&skill.name)));
    out.push_str(&format!("    wiki: {},\n", ron(skill.wiki.as_str())));
    out.push_str(&format!("    profession: {},\n", ron(&skill.profession)));
    out.push_str(&format!("    attribute: {},\n", ron(&skill.attribute)));
    out.push_str(&format!("    kind: {},\n", ron(&skill.kind)));
    out.push_str(&format!("    elite: {},\n", skill.elite));
    out.push_str(&format!("    pve_only: {},\n", skill.pve_only));
    if skill.title_track.is_some() {
        out.push_str(&format!("    title_track: {},\n", ron(&skill.title_track)));
    }
    out.push_str(&format!("    campaign: {},\n", ron(&skill.campaign)));

    let cost = &skill.cost;
    let mut parts = Vec::new();
    if cost.energy != 0 {
        parts.push(format!("energy: {}", cost.energy));
    }
    if cost.adrenaline != 0 {
        parts.push(format!("adrenaline: {}", cost.adrenaline));
    }
    if cost.sacrifice_pct != 0 {
        parts.push(format!("sacrifice_pct: {}", cost.sacrifice_pct));
    }
    if cost.upkeep != 0 {
        parts.push(format!("upkeep: {}", cost.upkeep));
    }
    if cost.overcast != 0 {
        parts.push(format!("overcast: {}", cost.overcast));
    }
    out.push_str(&format!("    cost: ({}),\n", parts.join(", ")));
    out.push_str(&format!("    activation: {},\n", ron(&skill.activation)));
    out.push_str(&format!("    recharge: {},\n", ron(&skill.recharge)));
    out.push_str(&format!("    target: {},\n", ron(&skill.target)));
    out.push_str(&format!("    range: {},\n", ron(&skill.range)));
    out.push_str(&format!("    aoe: {},\n", ron(&skill.aoe)));
    if skill.projectile.is_some() {
        out.push_str(&format!("    projectile: {},\n", ron(&skill.projectile)));
    }

    let flags = &skill.flags;
    let mut set = Vec::new();
    for (name, on) in [
        ("easily_interrupted", flags.easily_interrupted),
        ("touch", flags.touch),
        ("half_range", flags.half_range),
        ("needs_corpse", flags.needs_corpse),
        ("unblockable", flags.unblockable),
    ] {
        if on {
            set.push(format!("{name}: true"));
        }
    }
    if !set.is_empty() {
        out.push_str(&format!("    flags: ({}),\n", set.join(", ")));
    }

    out.push_str("    extracted: (\n");
    if !skill.extracted.scaled.is_empty() {
        out.push_str("        scaled: [\n");
        for number in &skill.extracted.scaled {
            out.push_str(&format!(
                "            (label: {}, r0: {}, r12: {}, r15: {}{}),\n",
                ron(&number.label),
                number.r0,
                number.r12,
                number.r15,
                if number.special_rounding {
                    ", special_rounding: true"
                } else {
                    ""
                }
            ));
        }
        out.push_str("        ],\n");
    }
    if !skill.extracted.fixed.is_empty() {
        out.push_str(&format!(
            "        fixed: {},\n",
            ron(&skill.extracted.fixed)
        ));
    }
    if let Some(hash) = &skill.extracted.description_hash {
        out.push_str(&format!("        description_hash: Some({}),\n", ron(hash)));
    }
    out.push_str("    ),\n");

    match &skill.encoding {
        None => out.push_str("    encoding: None,\n"),
        Some(encoding) => out.push_str(&format!("    encoding: Some({}),\n", ron(encoding))),
    }
    write_provenance(&mut out, &skill.provenance, "    ");
    out.push_str(")\n");
    out
}

fn write_mode_ranks(ranks: &ModeValue<Vec<(gwsim_data::core::Attribute, u8)>>) -> String {
    match &ranks.hm {
        Some(hm) => format!("(nm: {}, hm: Some({}))", ron(&ranks.nm), ron(hm)),
        None => format!("(nm: {}, hm: None)", ron(&ranks.nm)),
    }
}

fn write_skill_list(out: &mut String, skills: &[FoeSkill], indent: &str) {
    out.push_str("[\n");
    for skill in skills {
        let name = match &skill.skill {
            SkillRef::Slug(slug) => format!("Slug({})", ron(slug)),
            SkillRef::Id(id) => format!("Id({})", id.get()),
        };
        out.push_str(&format!(
            "{indent}    (skill: {name}{}),\n",
            if skill.hm_only { ", hm_only: true" } else { "" }
        ));
    }
    out.push_str(&format!("{indent}]"));
}

/// A foe file in canonical form (T2.6.2).
pub fn write_foe(foe: &Foe) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// {} — seeded from {}\n",
        foe.name,
        foe.wiki.url()
    ));
    out.push_str(
        "// Draft: `gwsim-extract seed` wrote the numbers; gaps carry assumption references.\n",
    );
    out.push_str("(\n");
    out.push_str(&format!("    name: {},\n", ron(&foe.name)));
    out.push_str(&format!("    wiki: {},\n", ron(foe.wiki.as_str())));
    out.push_str(&format!("    affiliation: {},\n", ron(&foe.affiliation)));
    out.push_str(&format!("    species: {},\n", ron(&foe.species)));
    out.push_str(&format!("    traits: {},\n", ron(&foe.traits)));
    out.push_str(&format!("    professions: {},\n", ron(&foe.professions)));
    let level = match foe.level.hm {
        Some(hm) => format!("(nm: {}, hm: Some({hm}))", foe.level.nm),
        None => format!("(nm: {}, hm: None)", foe.level.nm),
    };
    out.push_str(&format!("    level: {level},\n"));
    out.push_str(&format!(
        "    attributes: {},\n",
        write_mode_ranks(&foe.attributes)
    ));
    out.push_str("    skills: ");
    write_skill_list(&mut out, &foe.skills, "    ");
    out.push_str(",\n");
    if !foe.variants.is_empty() {
        out.push_str("    variants: [\n");
        for variant in &foe.variants {
            out.push_str(&format!(
                "        (\n            name: {},\n",
                ron(&variant.name)
            ));
            if let Some(weapon) = &variant.weapon {
                out.push_str(&format!("            weapon: Some({}),\n", ron(weapon)));
            }
            if let Some(skills) = &variant.skills {
                out.push_str("            skills: Some(");
                write_skill_list(&mut out, skills, "            ");
                out.push_str("),\n");
            }
            out.push_str("        ),\n");
        }
        out.push_str("    ],\n");
    }

    let armor = &foe.armor;
    let mut parts = Vec::new();
    if let Some(default) = armor.default {
        parts.push(format!("default: Some({default})"));
    }
    if !armor.per_type.is_empty() {
        parts.push(format!("per_type: {}", ron(&armor.per_type)));
    }
    if let Some(level) = armor.level_context {
        parts.push(format!("level_context: Some({level})"));
    }
    if !armor.note.is_empty() {
        parts.push(format!("note: {}", ron(&armor.note)));
    }
    out.push_str(&format!("    armor: ({}),\n", parts.join(", ")));
    if let Some(health) = foe.health_override {
        out.push_str(&format!("    health_override: Some({health}),\n"));
    }
    if let Some(energy) = foe.energy {
        out.push_str(&format!("    energy: Some({energy}),\n"));
    }
    if let Some(weapon) = &foe.weapon {
        out.push_str(&format!("    weapon: Some({}),\n", ron(weapon)));
    }
    if foe.boss {
        out.push_str("    boss: true,\n");
    }
    if !foe.ai_tags.is_empty() {
        out.push_str(&format!("    ai_tags: {},\n", ron(&foe.ai_tags)));
    }
    write_provenance(&mut out, &foe.provenance, "    ");
    out.push_str(")\n");
    out
}

/// `data/skills/index.ron`, one skill per line.
pub fn write_index(index: &SkillIndexFile) -> String {
    let mut out = String::new();
    out.push_str("// Every player skill in the game: the coverage denominator (T2.3.6).\n");
    out.push_str(
        "// Generated by `gwsim-extract index` from the wiki's skill lists; facts only.\n",
    );
    out.push_str("(\n");
    write_provenance(&mut out, &index.provenance, "    ");
    out.push_str("    skills: [\n");
    for skill in &index.skills {
        let mut fields = vec![
            format!("id: {}", skill.id.get()),
            format!("title: {}", ron(skill.title.as_str())),
            format!("slug: {}", ron(&skill.slug)),
        ];
        if let Some(profession) = skill.profession {
            fields.push(format!("profession: Some({profession:?})"));
        }
        if skill.elite {
            fields.push("elite: true".to_owned());
        }
        if skill.pve_only {
            fields.push("pve_only: true".to_owned());
        }
        if let Some(campaign) = skill.campaign {
            fields.push(format!("campaign: Some({campaign:?})"));
        }
        out.push_str(&format!("        ({}),\n", fields.join(", ")));
    }
    out.push_str("    ],\n)\n");
    out
}

// ------------------------------------------------------------- extraction

/// The crawl date of a cached page, as the provenance date.
fn crawled_date(cache: &PageCache, title: &WikiTitle) -> IsoDate {
    cache
        .meta(title)
        .and_then(|meta| meta.fetched_at.get(..10).map(str::to_owned))
        .and_then(|date| date.parse().ok())
        .unwrap_or_else(|| {
            iso_date(0)
                .parse()
                .unwrap_or_else(|_| unreachable!("a formatted date parses"))
        })
}

/// Re-derives a skill's numbers from the cache.
pub fn extract_skill(
    cache: &PageCache,
    title: &WikiTitle,
    index_id: Option<SkillId>,
) -> Result<Normalised, String> {
    let meta = cache
        .meta(title)
        .ok_or_else(|| format!("{title} is not cached"))?;
    if meta.status == 404 {
        return Err(format!("{title} does not exist on the wiki (404)"));
    }
    let body = cache
        .html(title)
        .ok_or_else(|| format!("{title} has no cached body"))?;
    let raw = parse_skill(&body).ok_or_else(|| format!("{title} has no skill infobox"))?;
    normalise(&raw, title, index_id, crawled_date(cache, title)).map_err(|e| e.to_string())
}

fn assumption(id: &str) -> AssumptionId {
    id.parse()
        .unwrap_or_else(|_| unreachable!("{id} is a well-formed assumption id"))
}

/// Builds a Draft foe file from a parsed page (T2.6.2).
///
/// Assumption references are added automatically: no weapon adds A-004,
/// missing hard-mode ranks add A-005, and variants add A-007.
pub fn build_foe(
    raw: &RawFoe,
    title: &WikiTitle,
    crawled: IsoDate,
    core: Option<&CoreData>,
) -> Result<(Foe, Vec<String>), String> {
    let mut warnings = raw.warnings.clone();
    let mut notes = Vec::new();
    let mut assumptions = vec![assumption("A-004")];

    let Some(&primary) = raw.professions.first() else {
        return Err(format!("{title}: no profession in the infobox"));
    };
    let secondary = raw.professions.get(1).copied();

    let top = raw
        .top_level()
        .ok_or_else(|| format!("{title}: no level in the infobox"))?;
    let mut level = top.clone();
    if raw.levels.len() > 1 {
        notes.push(format!(
            "the page lists {} level pairs; the highest, {} ({}), is used",
            raw.levels.len(),
            top.nm,
            top.hm
                .map(|hm| hm.to_string())
                .unwrap_or_else(|| "none".to_owned())
        ));
    }
    if level.hm.is_none()
        && let Some(core) = core
    {
        level.hm = core.levels.hm_levels.hm_level(
            if raw.boss {
                HmLevelKind::Boss
            } else {
                HmLevelKind::NonBoss
            },
            level.nm,
        );
        if level.hm.is_some() {
            notes.push("hard-mode level from the levels.ron mapping".to_owned());
        }
    }

    let species = raw.species.clone().unwrap_or_else(|| "Unknown".to_owned());
    let traits = traits_for_species(&species).unwrap_or_else(|| {
        warnings.push(format!("species {species:?} has no trait mapping"));
        Vec::new()
    });

    // A heading such as "Level 20 and above" makes a level variant rather
    // than a loadout: the one covering the foe's level is its bar, and the
    // others describe other levels of the same foe, so they are dropped.
    let level_variant = raw
        .variants
        .iter()
        .position(|variant| variant.covers(level.nm));
    let bar_index = level_variant.unwrap_or(0);
    let mut first = raw.variants.get(bar_index).cloned().unwrap_or_default();
    if first.attributes.nm.is_empty()
        && let Some(with_ranks) = raw.variants.iter().find(|v| !v.attributes.nm.is_empty())
    {
        first.attributes = with_ranks.attributes.clone();
    }
    let dropped = raw
        .variants
        .iter()
        .enumerate()
        .filter(|(index, variant)| *index != bar_index && variant.levels.is_some())
        .count();
    if level_variant.is_some() {
        notes.push(format!(
            "the bar is the page's {:?} list, which covers level {}",
            first.name.as_deref().unwrap_or(""),
            level.nm
        ));
    }
    if dropped > 0 {
        notes.push(format!(
            "{dropped} level variant(s) for other levels were left out"
        ));
    }
    if first.attributes.nm.is_empty() {
        warnings.push("the page gives no attribute ranks".to_owned());
        notes.push("the page gives no attribute ranks; WP4.2 supplies them".to_owned());
    }
    let attributes = ModeValue {
        nm: first.attributes.nm.clone(),
        hm: first.attributes.hm.clone(),
    };
    if attributes.hm.is_none() {
        assumptions.push(assumption("A-005"));
    }

    let to_skills = |variant: &crate::foe::RawVariant| -> Vec<FoeSkill> {
        variant
            .skills
            .iter()
            .map(|skill| FoeSkill {
                skill: SkillRef::Slug(slugify(&skill.title)),
                hm_only: skill.hm_only,
            })
            .collect()
    };
    let skills = to_skills(&first);
    let variants: Vec<FoeVariant> = raw
        .variants
        .iter()
        .enumerate()
        .filter(|(index, variant)| *index != bar_index && variant.levels.is_none())
        .map(|(_, variant)| FoeVariant {
            name: variant
                .name
                .as_deref()
                .map(|name| slugify(name).to_string())
                .unwrap_or_else(|| "variant".to_owned()),
            weapon: None,
            skills: Some(to_skills(variant)),
            armor: None,
        })
        .collect();
    if !variants.is_empty() {
        assumptions.push(assumption("A-007"));
        notes.push(format!(
            "the bar is the page's first variant ({}); the others are listed as variants",
            first.name.as_deref().unwrap_or("unnamed")
        ));
    }
    if first.skills.iter().any(|skill| skill.min_level.is_some()) {
        notes.push(
            "some skills are carried only at higher levels; the bar is the full one".to_owned(),
        );
    }

    let armor = match raw.armor.first() {
        Some(table) => armor_table(table, raw.armor.len() > 1),
        None => {
            warnings.push("no armor table".to_owned());
            ArmorTable::default()
        }
    };

    assumptions.sort();
    assumptions.dedup();
    let foe = Foe {
        name: raw.name.clone(),
        wiki: title.clone(),
        affiliation: raw
            .affiliation
            .clone()
            .unwrap_or_else(|| "Unaffiliated".to_owned()),
        species,
        traits,
        professions: (primary, secondary),
        level,
        attributes,
        skills,
        variants,
        armor,
        health_override: None,
        energy: None,
        weapon: None,
        boss: raw.boss,
        pre_searing: false,
        ai_tags: Vec::new(),
        provenance: Provenance {
            sources: vec![title.url()],
            crawled,
            review: ReviewStatus::Draft,
            reviewed_by: None,
            assumptions,
            notes: notes.join("; "),
        },
    };
    Ok((foe, warnings))
}

/// An armor table in the data's shape: the elemental figure is the default,
/// and physical types that differ are listed (T2.1 §3: "a / b" is physical
/// against elemental).
fn armor_table(table: &crate::foe::RawArmorTable, more_tables: bool) -> ArmorTable {
    let elemental = table
        .values
        .iter()
        .find(|(damage, _)| damage.is_elemental())
        .map(|(_, value)| *value);
    let default = elemental.or_else(|| table.values.first().map(|(_, v)| *v));
    let per_type: Vec<(DamageType, i16)> = table
        .values
        .iter()
        .filter(|(_, value)| Some(*value) != default)
        .copied()
        .collect();
    let mut note = String::new();
    if let Some(label) = &table.label {
        note = format!("the page's {label:?} table");
    }
    if more_tables {
        note.push_str(if note.is_empty() { "" } else { "; " });
        note.push_str("the page has further tables for other variants");
    }
    ArmorTable {
        default,
        per_type,
        level_context: table.level,
        note,
    }
}

/// Re-derives a foe from the cache.
pub fn extract_foe(
    cache: &PageCache,
    title: &WikiTitle,
    core: Option<&CoreData>,
) -> Result<(Foe, Vec<String>), String> {
    let meta = cache
        .meta(title)
        .ok_or_else(|| format!("{title} is not cached"))?;
    if meta.status == 404 {
        return Err(format!("{title} does not exist on the wiki (404)"));
    }
    let body = cache
        .html(title)
        .ok_or_else(|| format!("{title} has no cached body"))?;
    let raw = parse_foe(&body).ok_or_else(|| format!("{title} has no NPC infobox"))?;
    build_foe(&raw, title, crawled_date(cache, title), core)
}

// ------------------------------------------------------------------ seed

/// What to seed.
#[derive(Debug, Clone)]
pub struct SeedOptions {
    pub skills: bool,
    pub foes: bool,
    /// Titles to seed. Required for foes; for skills, empty means every skill
    /// the cached index lists and the cache holds.
    pub only: Vec<String>,
    pub data_dir: PathBuf,
    pub dry_run: bool,
}

/// What a seed did.
#[derive(Debug, Clone, Default)]
pub struct SeedSummary {
    pub created: Vec<PathBuf>,
    pub existed: Vec<PathBuf>,
    pub failed: Vec<(String, String)>,
    pub warnings: Vec<NormaliseWarning>,
}

/// Runs a seed.
pub fn seed(
    options: &SeedOptions,
    cache: &PageCache,
    out: &mut dyn Write,
) -> io::Result<SeedSummary> {
    let mut summary = SeedSummary::default();
    let index = crate::crawl::build_index(cache).ok();
    let core = CoreData::load(options.data_dir.join("core")).ok();

    let mut emit = |path: PathBuf, text: String, summary: &mut SeedSummary| -> io::Result<()> {
        let full = options.data_dir.join(&path);
        if options.dry_run {
            let exists = full.exists();
            writeln!(
                out,
                "  would {} {}",
                if exists { "skip (exists)" } else { "create" },
                path.display()
            )?;
            if exists {
                summary.existed.push(path);
            } else {
                summary.created.push(path);
            }
            return Ok(());
        }
        match create_new(&full, &text)? {
            Written::Created => summary.created.push(path),
            Written::Exists => summary.existed.push(path),
        }
        Ok(())
    };

    if options.skills {
        let titles: Vec<WikiTitle> = if options.only.is_empty() {
            index
                .as_ref()
                .map(|merged| {
                    merged
                        .skills
                        .iter()
                        .map(|s| s.title.clone())
                        .filter(|t| cache.html(t).is_some())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            options.only.iter().map(|t| WikiTitle(t.clone())).collect()
        };
        for title in titles {
            if crate::discovery::is_pvp_title(title.as_str()) {
                continue;
            }
            let index_id = index.as_ref().and_then(|merged| {
                merged
                    .skills
                    .iter()
                    .find(|s| s.title == title)
                    .map(|s| s.id)
            });
            match extract_skill(cache, &title, index_id) {
                Ok(normalised) => {
                    summary.warnings.extend(normalised.warnings.iter().cloned());
                    let path = skill_path(&normalised.skill, normalised.monster);
                    emit(path, write_skill(&normalised.skill), &mut summary)?;
                }
                Err(reason) => summary.failed.push((title.0.clone(), reason)),
            }
        }
    }

    if options.foes {
        for title in options.only.iter().map(|t| WikiTitle(t.clone())) {
            match extract_foe(cache, &title, core.as_ref()) {
                Ok((foe, warnings)) => {
                    summary
                        .warnings
                        .extend(warnings.into_iter().map(|message| NormaliseWarning {
                            page: title.0.clone(),
                            message,
                        }));
                    emit(foe_path(&foe), write_foe(&foe), &mut summary)?;
                }
                Err(reason) => summary.failed.push((title.0.clone(), reason)),
            }
        }
    }

    Ok(summary)
}

/// Prints a seed summary grouped by outcome.
pub fn print_summary(summary: &SeedSummary, out: &mut dyn Write) -> io::Result<()> {
    writeln!(out, "created {} files:", summary.created.len())?;
    for path in &summary.created {
        writeln!(out, "  {}", path.display())?;
    }
    if !summary.existed.is_empty() {
        writeln!(out, "exists, not touched ({}):", summary.existed.len())?;
        for path in &summary.existed {
            writeln!(out, "  {}", path.display())?;
        }
    }
    if !summary.failed.is_empty() {
        writeln!(out, "failed ({}):", summary.failed.len())?;
        for (title, reason) in &summary.failed {
            writeln!(out, "  {title}: {reason}")?;
        }
    }
    if !summary.warnings.is_empty() {
        writeln!(out, "warnings ({}):", summary.warnings.len())?;
        for warning in &summary.warnings {
            writeln!(out, "  {}: {}", warning.page, warning.message)?;
        }
    }
    Ok(())
}
