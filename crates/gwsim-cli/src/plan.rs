//! `gwsim plan` — the priority plan a human slot follows (T4.6.2).
//!
//! A slot with a plan in its party file uses it as written; otherwise one is
//! generated from the build. Either way this prints it in full as RON, ready
//! to paste into the party file and edit.

use std::io::{self, Write};

use gwsim_data::build::SlotKind;

use crate::PlanArgs;
use crate::data::{FAILED, OK};

/// Runs `gwsim plan`.
pub fn run(args: &PlanArgs, out: &mut impl Write) -> io::Result<i32> {
    let loaded = match crate::loading::load(args.data_dir.as_deref()) {
        Ok(loaded) => loaded,
        Err((origin, problems)) => {
            writeln!(out, "could not read {origin}:")?;
            writeln!(out, "{problems}")?;
            return Ok(FAILED);
        }
    };
    let party = match crate::evaluate::find_party(&loaded.data, &args.party) {
        Ok(party) => party,
        Err(problem) => {
            writeln!(out, "{problem}")?;
            return Ok(FAILED);
        }
    };
    let slot = match &args.slot {
        Some(name) => party.slots.iter().find(|s| s.name == *name),
        None => party.slots.iter().find(|s| s.kind == SlotKind::Human),
    };
    let Some(slot) = slot else {
        writeln!(out, "no such slot in {}", party.name)?;
        return Ok(FAILED);
    };
    let (plan, origin) = match &slot.plan {
        Some(plan) => (plan.clone(), "as written in the party file"),
        None => (
            gwsim_engine::ai::plan_gen::generate(&slot.build, &loaded.data),
            "generated from the build",
        ),
    };
    let text = ron::ser::to_string_pretty(&plan, ron::ser::PrettyConfig::default())
        .map_err(io::Error::other)?;
    writeln!(
        out,
        "// {}'s plan, {origin}. Skills are template ids.",
        slot.name
    )?;
    writeln!(out, "{text}")?;
    Ok(OK)
}
