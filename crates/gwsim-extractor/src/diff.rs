//! `gwsim-extract diff`: what changed on the wiki since seeding (WP2.7).
//!
//! It re-derives every value from the cache as it stands and compares it
//! with the numbers part of the committed files. The hand-written encoding
//! is ignored, and so are the foe fields WP4.2 fills by hand (weapon, AI
//! tags, provenance).
//!
//! **It never writes to `data/`.** The data is read through a
//! [`DataSource`], which has no write method, and the report goes wherever
//! `--out` says.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use gwsim_data::core::{CoreData, Profession};
use gwsim_data::dataset::DataSet;
use gwsim_data::foe::Foe;
use gwsim_data::provenance::ReviewStatus;
use gwsim_data::skill::Skill;
use gwsim_data::{DataSource, WikiTitle};
use serde::Serialize;

use crate::cache::PageCache;
use crate::seed::{extract_foe, extract_skill};
use crate::update::UpdateLine;

/// One difference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "class")]
pub enum Change {
    /// A value differs.
    FieldChanged {
        path: String,
        old: String,
        new: String,
    },
    /// In the index, but no file yet.
    NewPage,
    /// A file exists, but the page is gone or now redirects elsewhere.
    RemovedPage { reason: String },
    /// The description hash differs: the skill needs re-translation.
    DescriptionChanged,
    /// The page could not be read or parsed.
    ParseProblem { reason: String },
}

/// What kind of entity an entry is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Kind {
    Skill,
    Foe,
}

/// Everything found for one skill or foe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub kind: Kind,
    pub title: String,
    /// The committed file, where there is one.
    pub file: Option<String>,
    pub profession: Option<Profession>,
    /// The committed file's review status. A `Reviewed` skill whose numbers
    /// changed needs reviewing again.
    pub review: Option<ReviewStatus>,
    pub changes: Vec<Change>,
    /// Game update lines naming this skill (T2.7.4).
    pub context: Vec<String>,
}

/// The whole report.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DiffReport {
    pub entries: Vec<Entry>,
    /// How many index skills have no file. Counted, not listed one by one,
    /// because for most of the project that is most of the game.
    pub new_pages: usize,
}

impl DiffReport {
    /// Entries with at least one change to a committed file.
    pub fn changed(&self) -> impl Iterator<Item = &Entry> {
        self.entries
            .iter()
            .filter(|entry| !entry.changes.is_empty())
    }

    /// Whether nothing committed has changed.
    pub fn is_clean(&self) -> bool {
        self.changed().next().is_none()
    }

    /// Counts per change class.
    pub fn counts(&self) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        for entry in &self.entries {
            for change in &entry.changes {
                *counts.entry(class_name(change)).or_insert(0) += 1;
            }
        }
        counts
    }
}

fn class_name(change: &Change) -> &'static str {
    match change {
        Change::FieldChanged { .. } => "FieldChanged",
        Change::NewPage => "NewPage",
        Change::RemovedPage { .. } => "RemovedPage",
        Change::DescriptionChanged => "DescriptionChanged",
        Change::ParseProblem { .. } => "ParseProblem",
    }
}

fn field<T: std::fmt::Debug + PartialEq>(changes: &mut Vec<Change>, path: &str, old: &T, new: &T) {
    if old != new {
        changes.push(Change::FieldChanged {
            path: path.to_owned(),
            old: format!("{old:?}"),
            new: format!("{new:?}"),
        });
    }
}

/// Compares the numbers part of two skills (T2.7.1).
pub fn compare_skill(committed: &Skill, extracted: &Skill) -> Vec<Change> {
    let mut changes = Vec::new();
    field(&mut changes, "id", &committed.id, &extracted.id);
    field(&mut changes, "name", &committed.name, &extracted.name);
    field(
        &mut changes,
        "profession",
        &committed.profession,
        &extracted.profession,
    );
    field(
        &mut changes,
        "attribute",
        &committed.attribute,
        &extracted.attribute,
    );
    field(&mut changes, "kind", &committed.kind, &extracted.kind);
    field(&mut changes, "elite", &committed.elite, &extracted.elite);
    field(
        &mut changes,
        "pve_only",
        &committed.pve_only,
        &extracted.pve_only,
    );
    field(
        &mut changes,
        "title_track",
        &committed.title_track,
        &extracted.title_track,
    );
    field(
        &mut changes,
        "campaign",
        &committed.campaign,
        &extracted.campaign,
    );
    field(
        &mut changes,
        "cost.energy",
        &committed.cost.energy,
        &extracted.cost.energy,
    );
    field(
        &mut changes,
        "cost.adrenaline",
        &committed.cost.adrenaline,
        &extracted.cost.adrenaline,
    );
    field(
        &mut changes,
        "cost.sacrifice_pct",
        &committed.cost.sacrifice_pct,
        &extracted.cost.sacrifice_pct,
    );
    field(
        &mut changes,
        "cost.upkeep",
        &committed.cost.upkeep,
        &extracted.cost.upkeep,
    );
    field(
        &mut changes,
        "cost.overcast",
        &committed.cost.overcast,
        &extracted.cost.overcast,
    );
    field(
        &mut changes,
        "activation",
        &committed.activation,
        &extracted.activation,
    );
    field(
        &mut changes,
        "recharge",
        &committed.recharge,
        &extracted.recharge,
    );
    field(&mut changes, "target", &committed.target, &extracted.target);
    field(&mut changes, "range", &committed.range, &extracted.range);
    field(&mut changes, "aoe", &committed.aoe, &extracted.aoe);
    field(
        &mut changes,
        "projectile",
        &committed.projectile,
        &extracted.projectile,
    );
    field(&mut changes, "flags", &committed.flags, &extracted.flags);

    // Scaled numbers by label, so a row added or removed is one change,
    // not a cascade.
    let old: BTreeMap<&str, _> = committed
        .extracted
        .scaled
        .iter()
        .map(|n| (n.label.as_str(), n))
        .collect();
    let new: BTreeMap<&str, _> = extracted
        .extracted
        .scaled
        .iter()
        .map(|n| (n.label.as_str(), n))
        .collect();
    for (label, number) in &old {
        match new.get(label) {
            Some(other) => {
                let triple = |n: &gwsim_data::skill::ScaledNumber| (n.r0, n.r12, n.r15);
                if triple(number) != triple(other) {
                    changes.push(Change::FieldChanged {
                        path: format!("scaled[{label}]"),
                        old: format!("{}...{}...{}", number.r0, number.r12, number.r15),
                        new: format!("{}...{}...{}", other.r0, other.r12, other.r15),
                    });
                }
            }
            // A row the page no longer shows is reported, not treated as a
            // removal of the skill (§22).
            None => changes.push(Change::FieldChanged {
                path: format!("scaled[{label}]"),
                old: format!("{}...{}...{}", number.r0, number.r12, number.r15),
                new: "(missing on the page)".to_owned(),
            }),
        }
    }
    for (label, number) in &new {
        if !old.contains_key(label) {
            changes.push(Change::FieldChanged {
                path: format!("scaled[{label}]"),
                old: "(not in the file)".to_owned(),
                new: format!("{}...{}...{}", number.r0, number.r12, number.r15),
            });
        }
    }

    if committed.extracted.description_hash != extracted.extracted.description_hash {
        changes.push(Change::DescriptionChanged);
    }
    changes
}

/// Compares the extracted parts of two foes.
pub fn compare_foe(committed: &Foe, extracted: &Foe) -> Vec<Change> {
    let mut changes = Vec::new();
    field(&mut changes, "name", &committed.name, &extracted.name);
    field(
        &mut changes,
        "affiliation",
        &committed.affiliation,
        &extracted.affiliation,
    );
    field(
        &mut changes,
        "species",
        &committed.species,
        &extracted.species,
    );
    field(
        &mut changes,
        "professions",
        &committed.professions,
        &extracted.professions,
    );
    field(&mut changes, "level", &committed.level, &extracted.level);
    field(
        &mut changes,
        "attributes.nm",
        &committed.attributes.nm,
        &extracted.attributes.nm,
    );
    // A hard-mode rank the page does not give is filled from A-005 by hand
    // later, so only a page value is compared.
    if extracted.attributes.hm.is_some() {
        field(
            &mut changes,
            "attributes.hm",
            &committed.attributes.hm,
            &extracted.attributes.hm,
        );
    }
    field(&mut changes, "skills", &committed.skills, &extracted.skills);
    let variant_bars = |foe: &Foe| -> Vec<(String, Option<Vec<gwsim_data::foe::FoeSkill>>)> {
        foe.variants
            .iter()
            .map(|v| (v.name.clone(), v.skills.clone()))
            .collect()
    };
    field(
        &mut changes,
        "variants",
        &variant_bars(committed),
        &variant_bars(extracted),
    );
    field(
        &mut changes,
        "armor.default",
        &committed.armor.default,
        &extracted.armor.default,
    );
    field(
        &mut changes,
        "armor.per_type",
        &committed.armor.per_type,
        &extracted.armor.per_type,
    );
    field(
        &mut changes,
        "armor.level_context",
        &committed.armor.level_context,
        &extracted.armor.level_context,
    );
    field(&mut changes, "boss", &committed.boss, &extracted.boss);
    changes
}

/// Runs the comparison over a data source (T2.7.3).
pub fn diff(
    source: &dyn DataSource,
    cache: &PageCache,
    core: Option<&CoreData>,
    skills: bool,
    foes: bool,
    updates: &[UpdateLine],
) -> Result<DiffReport, String> {
    let data = DataSet::load(source).map_err(|problems| problems.to_string())?;
    let index = crate::crawl::build_index(cache).ok();
    let mut report = DiffReport::default();

    if skills {
        for entry in data.skills.values() {
            let committed = &entry.value;
            let title = committed.wiki.clone();
            let mut changes = Vec::new();

            let meta = cache.meta(&title);
            match &meta {
                Some(meta) if meta.status == 404 => changes.push(Change::RemovedPage {
                    reason: "the page answers 404".to_owned(),
                }),
                Some(meta)
                    if meta
                        .canonical_title
                        .as_deref()
                        .is_some_and(|c| c != title.as_str()) =>
                {
                    changes.push(Change::RemovedPage {
                        reason: format!(
                            "the page now redirects to {}",
                            meta.canonical_title.as_deref().unwrap_or_default()
                        ),
                    })
                }
                _ => {}
            }
            if changes.is_empty() {
                let index_id = index.as_ref().and_then(|merged| {
                    merged
                        .skills
                        .iter()
                        .find(|s| s.title == title)
                        .map(|s| s.id)
                });
                match extract_skill(cache, &title, index_id.or(Some(committed.id))) {
                    Ok(normalised) => changes = compare_skill(committed, &normalised.skill),
                    Err(reason) => changes.push(Change::ParseProblem { reason }),
                }
            }

            report.entries.push(Entry {
                kind: Kind::Skill,
                title: title.0.clone(),
                file: Some(entry.path.clone()),
                profession: committed.profession,
                review: Some(committed.provenance.review),
                context: updates
                    .iter()
                    .filter(|line| line.skill == title.as_str())
                    .map(|line| format!("{}: {}", line.page, line.change))
                    .collect(),
                changes,
            });
        }

        if let Some(merged) = &index {
            report.new_pages = merged
                .skills
                .iter()
                .filter(|s| data.skill_by_id(s.id).is_none())
                .count();
        }
    }

    if foes {
        for entry in data.foes.values() {
            let committed = &entry.value;
            let title = committed.wiki.clone();
            let changes = match extract_foe(cache, &title, core) {
                Ok((foe, _)) => compare_foe(committed, &foe),
                Err(reason) if reason.contains("404") => vec![Change::RemovedPage { reason }],
                Err(reason) => vec![Change::ParseProblem { reason }],
            };
            report.entries.push(Entry {
                kind: Kind::Foe,
                title: title.0.clone(),
                file: Some(entry.path.clone()),
                profession: Some(committed.professions.0),
                review: Some(committed.provenance.review),
                context: Vec::new(),
                changes,
            });
        }
    }

    report.entries.sort_by(|a, b| {
        (a.kind, a.profession.map(|p| p.index()), &a.title).cmp(&(
            b.kind,
            b.profession.map(|p| p.index()),
            &b.title,
        ))
    });
    Ok(report)
}

/// The markdown report (T2.7.2).
pub fn render_markdown(report: &DiffReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Wiki change report\n");
    if report.is_clean() {
        let _ = writeln!(out, "**No changes** to committed files.\n");
    } else {
        let _ = writeln!(out, "## Summary\n");
        let _ = writeln!(out, "| Class | Count |\n| --- | --- |");
        for (class, count) in report.counts() {
            let _ = writeln!(out, "| {class} | {count} |");
        }
        let _ = writeln!(out);
    }
    let _ = writeln!(
        out,
        "{} skills in the index have no file yet (new pages; not listed).\n",
        report.new_pages
    );

    let mut current_section: Option<String> = None;
    for entry in report.changed() {
        let section = format!(
            "{:?} — {}",
            entry.kind,
            entry
                .profession
                .map(|p| format!("{p:?}"))
                .unwrap_or_else(|| "common".to_owned())
        );
        if current_section.as_ref() != Some(&section) {
            let _ = writeln!(out, "## {section}\n");
            current_section = Some(section);
        }
        let link = WikiTitle(entry.title.clone()).url();
        let review = entry
            .review
            .map(|r| format!("{r:?}"))
            .unwrap_or_else(|| "no file".to_owned());
        let warning = if entry.review == Some(ReviewStatus::Reviewed) {
            " — **needs review again**"
        } else {
            ""
        };
        let _ = writeln!(out, "### [{}]({link}) ({review}){warning}\n", entry.title);
        for change in &entry.changes {
            let line = match change {
                Change::FieldChanged { path, old, new } => format!("`{path}`: {old} → {new}"),
                Change::NewPage => "new page".to_owned(),
                Change::RemovedPage { reason } => format!("removed: {reason}"),
                Change::DescriptionChanged => "description changed: re-translate".to_owned(),
                Change::ParseProblem { reason } => format!("could not parse: {reason}"),
            };
            let _ = writeln!(out, "- {line}");
        }
        for context in &entry.context {
            let _ = writeln!(out, "- update note: {context}");
        }
        let _ = writeln!(out);
    }

    let retranslate: Vec<&Entry> = report
        .entries
        .iter()
        .filter(|entry| entry.changes.contains(&Change::DescriptionChanged))
        .collect();
    if !retranslate.is_empty() {
        let _ = writeln!(out, "## Needs re-translation\n");
        for entry in retranslate {
            let _ = writeln!(out, "- {}", entry.title);
        }
    }
    out
}

/// The same report as JSON (T2.7.2).
pub fn render_json(report: &DiffReport) -> serde_json::Value {
    serde_json::json!({
        "schema_version": 1,
        "clean": report.is_clean(),
        "counts": report.counts(),
        "new_pages": report.new_pages,
        "entries": report.changed().collect::<Vec<_>>(),
    })
}
