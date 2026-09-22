//! `gwsim data describe` — what a skill does, in our own words.

use std::io::{self, Write};

use gwsim_data::core::{CoreData, Profession};
use gwsim_data::dataset::DataSet;
use gwsim_data::describe::{DescribeContext, describe};
use gwsim_data::dsl::NoHandlers;
use gwsim_data::ids::{SkillId, slugify};
use gwsim_data::provenance::ReviewStatus;
use gwsim_data::skill::Skill;

use crate::DescribeSkillArgs;
use crate::data::{FAILED, OK};

/// Runs `gwsim data describe`.
pub fn run(args: &DescribeSkillArgs, out: &mut impl Write) -> io::Result<i32> {
    let loaded = match crate::loading::load(args.data_dir.as_deref()) {
        Ok(loaded) => loaded,
        Err((origin, problems)) => {
            writeln!(out, "could not read {origin}:")?;
            writeln!(out, "{problems}")?;
            return Ok(FAILED);
        }
    };
    let data = loaded.data;
    let core = args
        .data_dir
        .as_deref()
        .map(|dir| dir.join("core"))
        .or_else(|| Some(std::path::PathBuf::from("data/core")))
        .and_then(|dir| CoreData::load(dir).ok());

    let profession = match parse_profession(args.profession.as_deref()) {
        Ok(profession) => profession,
        Err(message) => {
            writeln!(out, "{message}")?;
            return Ok(FAILED);
        }
    };
    let status = match parse_status(args.status.as_deref()) {
        Ok(status) => status,
        Err(message) => {
            writeln!(out, "{message}")?;
            return Ok(FAILED);
        }
    };

    // Printing every skill because no arguments were given is a surprise,
    // so the whole-tree dump has to be asked for. clap already rejects
    // --all alongside a filter, which leaves exactly these two cases.
    if !args.all && args.skill.is_none() && profession.is_none() && status.is_none() {
        writeln!(
            out,
            "name a skill, or filter with --profession or --status, or pass --all for every skill"
        )?;
        return Ok(FAILED);
    }

    let matched = select(&data, args, profession, status);
    if matched.is_empty() {
        match (&args.skill, args.all) {
            (Some(name), _) => writeln!(out, "no skill matches {name:?}")?,
            (None, true) => writeln!(out, "there are no skills in the data yet")?,
            (None, false) => writeln!(out, "no skills match those filters")?,
        }
        return Ok(FAILED);
    }

    if args.json {
        let values: Vec<serde_json::Value> = matched
            .iter()
            .map(|skill| as_json(skill, args, core.as_ref()))
            .collect();
        writeln!(out, "{}", serde_json::to_string_pretty(&values)?)?;
    } else {
        for (index, skill) in matched.iter().enumerate() {
            if index > 0 {
                writeln!(out)?;
            }
            write_text(skill, args, core.as_ref(), out)?;
        }
    }

    Ok(OK)
}

/// Parses a profession name, listing the valid ones when it is not one.
fn parse_profession(name: Option<&str>) -> Result<Option<Profession>, String> {
    let Some(name) = name else { return Ok(None) };
    Profession::ALL
        .into_iter()
        .find(|profession| format!("{profession:?}").eq_ignore_ascii_case(name))
        .map(Some)
        .ok_or_else(|| {
            let names: Vec<String> = Profession::ALL
                .iter()
                .map(|profession| format!("{profession:?}"))
                .collect();
            format!(
                "{name:?} is not a profession; try one of {}",
                names.join(", ")
            )
        })
}

/// Parses a review status, listing the valid ones when it is not one.
fn parse_status(name: Option<&str>) -> Result<Option<ReviewStatus>, String> {
    let Some(name) = name else { return Ok(None) };
    for status in [
        ReviewStatus::NumbersOnly,
        ReviewStatus::Draft,
        ReviewStatus::Reviewed,
    ] {
        if format!("{status:?}").eq_ignore_ascii_case(name) {
            return Ok(Some(status));
        }
    }
    Err(format!(
        "{name:?} is not a review status; try NumbersOnly, Draft or Reviewed"
    ))
}

/// The skills a set of filters picks out.
fn select<'a>(
    data: &'a DataSet,
    args: &DescribeSkillArgs,
    profession: Option<Profession>,
    status: Option<ReviewStatus>,
) -> Vec<&'a Skill> {
    let mut matched: Vec<&Skill> = Vec::new();

    if args.all {
        // clap guarantees no other filter is set alongside --all.
        return data.skills.values().map(|entry| &entry.value).collect();
    }

    if let Some(name) = &args.skill {
        // A name may arrive as a slug, a template id, or the skill's own
        // name. All three are things a person reasonably has to hand.
        if let Ok(id) = name.parse::<u16>()
            && let Some(skill) = data.skill_by_id(SkillId(id))
        {
            return vec![skill];
        }
        let slug = name.parse().unwrap_or_else(|_| slugify(name));
        if let Some(skill) = data.skill(&slug) {
            return vec![skill];
        }
        return Vec::new();
    }

    for entry in data.skills.values() {
        let skill = &entry.value;
        if let Some(profession) = profession
            && skill.profession != Some(profession)
        {
            continue;
        }
        if let Some(status) = status
            && skill.provenance.review != status
        {
            continue;
        }
        matched.push(skill);
    }
    matched
}

fn write_text(
    skill: &Skill,
    args: &DescribeSkillArgs,
    core: Option<&CoreData>,
    out: &mut impl Write,
) -> io::Result<()> {
    let context = context_for(args, core);

    writeln!(out, "{} ({})", skill.name, skill.id)?;
    writeln!(out, "  {}", skill.wiki.url())?;

    let profession = skill
        .profession
        .map(|profession| format!("{profession:?}"))
        .unwrap_or_else(|| "common".to_owned());
    let elite = if skill.elite { ", elite" } else { "" };
    let pve = if skill.pve_only { ", PvE only" } else { "" };
    writeln!(out, "  {profession} {:?}{elite}{pve}", skill.kind)?;

    writeln!(out, "  {}", costs(skill))?;
    writeln!(out, "  {}", describe(skill, &context))?;

    if let Some(encoding) = &skill.encoding {
        if !encoding.roles.is_empty() {
            let roles: Vec<String> = encoding
                .roles
                .iter()
                .map(|role| format!("{role:?}"))
                .collect();
            writeln!(out, "  roles: {}", roles.join(", "))?;
        }
        if let Some(hints) = &encoding.ai {
            writeln!(
                out,
                "  ai: priority {}, target {:?}",
                hints.priority, hints.target
            )?;
        }
    }

    writeln!(out, "  review: {:?}", skill.provenance.review)?;
    if skill.provenance.review != ReviewStatus::Reviewed {
        writeln!(
            out,
            "  (not reviewed: compare this against the wiki page above)"
        )?;
    }
    Ok(())
}

fn context_for<'a>(args: &DescribeSkillArgs, core: Option<&'a CoreData>) -> DescribeContext<'a> {
    DescribeContext {
        rank: args.rank,
        title_rank: args.title_rank,
        core,
        handlers: &NoHandlers,
    }
}

fn costs(skill: &Skill) -> String {
    let mut parts = Vec::new();
    if skill.cost.energy > 0 {
        parts.push(format!("{} energy", skill.cost.energy));
    }
    if skill.cost.adrenaline > 0 {
        parts.push(format!("{} adrenaline", skill.cost.adrenaline));
    }
    if skill.cost.sacrifice_pct > 0 {
        parts.push(format!("{}% sacrifice", skill.cost.sacrifice_pct));
    }
    if skill.cost.upkeep != 0 {
        parts.push(format!("{} upkeep", skill.cost.upkeep));
    }
    if skill.cost.overcast > 0 {
        parts.push(format!("{} overcast", skill.cost.overcast));
    }
    if parts.is_empty() {
        parts.push("free".to_owned());
    }
    if skill.activation.ms() > 0 {
        parts.push(format!("{} activation", skill.activation));
    }
    if skill.recharge.ms() > 0 {
        parts.push(format!("{} recharge", skill.recharge));
    }
    parts.join(", ")
}

fn as_json(skill: &Skill, args: &DescribeSkillArgs, core: Option<&CoreData>) -> serde_json::Value {
    let context = context_for(args, core);
    serde_json::json!({
        "id": skill.id.get(),
        "name": skill.name,
        "wiki": skill.wiki.url(),
        "profession": skill.profession.map(|p| format!("{p:?}")),
        "kind": format!("{:?}", skill.kind),
        "elite": skill.elite,
        "pve_only": skill.pve_only,
        "description": describe(skill, &context),
        "roles": skill
            .encoding
            .as_ref()
            .map(|encoding| {
                encoding
                    .roles
                    .iter()
                    .map(|role| format!("{role:?}"))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        "review": format!("{:?}", skill.provenance.review),
    })
}
