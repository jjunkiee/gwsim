//! `gwsim template` — reading and writing build codes.

use std::io::{self, Write};
use std::path::Path;

use gwsim_data::core::Attribute;
use gwsim_data::dataset::DataSet;
use gwsim_data::source::DirSource;
use gwsim_data::template::{
    EquipmentTemplate, SkillTemplate, Template, TemplateError, decode as decode_any,
};

use crate::data::{FAILED, OK};
use crate::{DecodeArgs, EncodeArgs};

/// Runs `gwsim template decode`.
pub fn decode(args: &DecodeArgs, out: &mut impl Write) -> io::Result<i32> {
    let template = match decode_any(&args.code) {
        Ok(template) => template,
        Err(error) => {
            writeln!(out, "that is not a template code: {error}")?;
            return Ok(FAILED);
        }
    };

    // Skill names are a nicety, not a requirement: the codec works with no
    // data loaded at all, and an unreadable data directory must not stop a
    // code being decoded.
    let data = load_data(args.data_dir.as_deref());

    match template {
        Template::Skill(skill) => {
            if args.json {
                write_skill_json(&skill, data.as_ref(), out)?
            } else {
                write_skill_text(&skill, data.as_ref(), out)?
            }
        }
        Template::Equipment(equipment) => {
            if args.json {
                write_equipment_json(&equipment, out)?
            } else {
                write_equipment_text(&equipment, out)?
            }
        }
    }

    Ok(OK)
}

/// Runs `gwsim template encode`.
pub fn encode(args: &EncodeArgs, out: &mut impl Write) -> io::Result<i32> {
    let text = match std::fs::read_to_string(&args.file) {
        Ok(text) => text,
        Err(error) => {
            writeln!(out, "{}: {error}", args.file.display())?;
            return Ok(FAILED);
        }
    };

    // A file may hold either kind. Skill templates are far more common, so
    // that is tried first and its error is the one reported.
    match ron::from_str::<SkillTemplate>(&text) {
        Ok(template) => {
            writeln!(out, "{}", template.encode())?;
            Ok(OK)
        }
        Err(skill_error) => match ron::from_str::<EquipmentTemplate>(&text) {
            Ok(template) => {
                writeln!(out, "{}", template.encode())?;
                Ok(OK)
            }
            Err(_) => {
                writeln!(
                    out,
                    "{}: this is not a SkillTemplate: {skill_error}",
                    args.file.display()
                )?;
                Ok(FAILED)
            }
        },
    }
}

fn load_data(data_dir: Option<&Path>) -> Option<DataSet> {
    let dir = data_dir.unwrap_or(Path::new("data"));
    if !dir.is_dir() {
        return None;
    }
    DataSet::load(&DirSource::new(dir)).ok()
}

/// How a skill id is shown: the number always, the name when we know it.
fn skill_label(id: gwsim_data::ids::SkillId, data: Option<&DataSet>) -> String {
    match data.and_then(|data| data.skill_by_id(id)) {
        Some(skill) => format!("{id} {}", skill.name),
        None => format!("{id} (unknown)"),
    }
}

fn write_skill_text(
    template: &SkillTemplate,
    data: Option<&DataSet>,
    out: &mut impl Write,
) -> io::Result<()> {
    let professions = match template.secondary {
        Some(secondary) => format!(
            "{:?}/{:?} ({}/{})",
            template.primary,
            secondary,
            template.primary.abbrev(),
            secondary.abbrev()
        ),
        None => format!("{:?} ({}/--)", template.primary, template.primary.abbrev()),
    };

    writeln!(out, "Skill template")?;
    writeln!(out, "  professions: {professions}")?;

    if template.attributes.is_empty() {
        writeln!(out, "  attributes:  none")?;
    } else {
        writeln!(out, "  attributes:")?;
        for (attribute, rank) in &template.attributes {
            writeln!(out, "    {:<20} {rank}", format!("{attribute:?}"))?;
        }
        // Worth saying out loud, because the difference between a stored rank
        // and an effective one is a classic misreading.
        writeln!(
            out,
            "    (ranks are from attribute points only; runes and headgear are not \
             part of a skill template)"
        )?;
    }

    writeln!(out, "  skills:")?;
    for (index, slot) in template.skills.iter().enumerate() {
        let shown = match slot {
            Some(id) => skill_label(*id, data),
            None => "(empty)".to_owned(),
        };
        writeln!(out, "    {}. {shown}", index + 1)?;
    }

    Ok(())
}

fn write_skill_json(
    template: &SkillTemplate,
    data: Option<&DataSet>,
    out: &mut impl Write,
) -> io::Result<()> {
    let attributes: Vec<serde_json::Value> = template
        .attributes
        .iter()
        .map(|(attribute, rank)| {
            serde_json::json!({
                "attribute": format!("{attribute:?}"),
                "template_id": attribute.template_id(),
                "rank": rank,
            })
        })
        .collect();

    let skills: Vec<serde_json::Value> = template
        .skills
        .iter()
        .map(|slot| match slot {
            Some(id) => serde_json::json!({
                "id": id.get(),
                "name": data
                    .and_then(|data| data.skill_by_id(*id))
                    .map(|skill| skill.name.clone()),
            }),
            None => serde_json::Value::Null,
        })
        .collect();

    let value = serde_json::json!({
        "kind": "skill",
        "primary": format!("{:?}", template.primary),
        "secondary": template.secondary.map(|p| format!("{p:?}")),
        "attributes": attributes,
        "skills": skills,
    });

    writeln!(out, "{}", serde_json::to_string_pretty(&value)?)
}

fn write_equipment_text(template: &EquipmentTemplate, out: &mut impl Write) -> io::Result<()> {
    writeln!(out, "Equipment template")?;
    writeln!(
        out,
        "  note: equipment templates describe PvP equipment, and only PvP characters \
         can load one."
    )?;
    if template.items.is_empty() {
        writeln!(out, "  items: none")?;
        return Ok(());
    }
    writeln!(out, "  items:")?;
    for item in &template.items {
        let dye = item
            .dye
            .name()
            .map(|name| name.to_owned())
            .unwrap_or_else(|| format!("#{}", item.dye.0));
        let preview = if item.dye.is_preview_only() {
            " (preview only)"
        } else {
            ""
        };
        writeln!(
            out,
            "    {:<8} item #{:<5} dye {dye}{preview}",
            format!("{:?}", item.slot),
            item.item_id
        )?;
        for modifier in &item.modifiers {
            writeln!(out, "               modifier #{modifier}")?;
        }
    }
    Ok(())
}

fn write_equipment_json(template: &EquipmentTemplate, out: &mut impl Write) -> io::Result<()> {
    let items: Vec<serde_json::Value> = template
        .items
        .iter()
        .map(|item| {
            serde_json::json!({
                "slot": format!("{:?}", item.slot),
                "item_id": item.item_id,
                "dye": item.dye.0,
                "dye_name": item.dye.name(),
                "modifiers": item.modifiers,
            })
        })
        .collect();

    let value = serde_json::json!({ "kind": "equipment", "items": items });
    writeln!(out, "{}", serde_json::to_string_pretty(&value)?)
}

/// Reports a decode failure in the same shape the command would.
pub fn describe_failure(error: &TemplateError) -> String {
    format!("that is not a template code: {error}")
}

/// Re-exported so callers can name the attribute type without another import.
pub type TemplateAttribute = Attribute;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DecodeArgs, EncodeArgs};

    const PLAYER: &str = "OQBTAUBPQaJ4EY6x0BAAAAAAuE";

    fn decode_args(code: &str, json: bool) -> DecodeArgs {
        DecodeArgs {
            code: code.to_owned(),
            json,
            data_dir: None,
        }
    }

    fn run(code: &str, json: bool) -> (i32, String) {
        let mut out = Vec::new();
        let status = decode(&decode_args(code, json), &mut out).unwrap();
        (status, String::from_utf8(out).unwrap())
    }

    #[test]
    fn the_player_code_prints_its_professions_and_attributes() {
        // T1.3.6's done criterion.
        let (status, report) = run(PLAYER, false);
        assert_eq!(status, OK);
        assert!(report.contains("Mesmer"), "{report}");
        assert!(report.contains("FastCasting"), "{report}");
        assert!(report.contains("DominationMagic"), "{report}");
        assert!(report.contains("InspirationMagic"), "{report}");
        assert!(report.contains("12"), "{report}");
    }

    #[test]
    fn a_build_with_no_secondary_says_so() {
        let (_, report) = run(PLAYER, false);
        assert!(report.contains("Mesmer (Me/--)"), "{report}");
    }

    #[test]
    fn unknown_skills_are_shown_as_numbers_rather_than_hidden() {
        let (_, report) = run(PLAYER, false);
        assert!(report.contains("#39 (unknown)"), "{report}");
        assert!(report.contains("(empty)"), "{report}");
    }

    #[test]
    fn the_report_warns_that_ranks_exclude_runes() {
        let (_, report) = run(PLAYER, false);
        assert!(
            report.contains("runes and headgear are not"),
            "a reader comparing this against a published build needs to know: {report}"
        );
    }

    #[test]
    fn a_bad_code_fails_with_a_readable_reason() {
        let (status, report) = run("not-a-code", false);
        assert_eq!(status, FAILED);
        assert!(report.contains("not a template code"), "{report}");
    }

    #[test]
    fn json_output_is_machine_readable() {
        let (status, report) = run(PLAYER, true);
        assert_eq!(status, OK);

        let value: serde_json::Value = serde_json::from_str(&report).expect("valid JSON");
        assert_eq!(value["kind"], "skill");
        assert_eq!(value["primary"], "Mesmer");
        assert!(value["secondary"].is_null());
        assert_eq!(value["attributes"].as_array().unwrap().len(), 3);
        assert_eq!(value["skills"].as_array().unwrap().len(), 8);
        // Empty slots are null, not missing, so the eight slots stay aligned.
        assert!(value["skills"][4].is_null());
        assert_eq!(value["skills"][1]["id"], 39);
    }

    #[test]
    fn an_equipment_code_decodes_and_says_what_it_is() {
        use gwsim_data::template::{Dye, EquipmentItem, EquipmentSlot};

        let template = EquipmentTemplate {
            items: vec![EquipmentItem {
                slot: EquipmentSlot::Chest,
                item_id: 42,
                dye: Dye(5),
                modifiers: vec![359],
            }],
        };
        let (status, report) = run(&template.encode(), false);
        assert_eq!(status, OK);
        assert!(report.contains("Equipment template"), "{report}");
        assert!(report.contains("PvP"), "{report}");
        assert!(report.contains("dye red"), "{report}");
        assert!(report.contains("modifier #359"), "{report}");
    }

    #[test]
    fn encoding_a_template_file_reproduces_the_code() {
        // The loop that matters: decode a published code, write it as RON,
        // encode it back, and get the same string.
        let template = SkillTemplate::decode(PLAYER).unwrap();
        let ron_text = ron::ser::to_string_pretty(&template, Default::default()).unwrap();

        let path = std::env::temp_dir().join("gwsim-template-test.ron");
        std::fs::write(&path, ron_text).unwrap();

        let mut out = Vec::new();
        let status = super::encode(&EncodeArgs { file: path.clone() }, &mut out).unwrap();
        assert_eq!(status, OK);
        assert_eq!(String::from_utf8(out).unwrap().trim(), PLAYER);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn encoding_a_file_that_is_not_a_template_fails_clearly() {
        let path = std::env::temp_dir().join("gwsim-template-bad.ron");
        std::fs::write(&path, "(not: \"a template\")").unwrap();

        let mut out = Vec::new();
        let status = super::encode(&EncodeArgs { file: path.clone() }, &mut out).unwrap();
        assert_eq!(status, FAILED);
        assert!(
            String::from_utf8(out).unwrap().contains("SkillTemplate"),
            "the message should say what was expected"
        );

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn encoding_a_missing_file_fails_clearly() {
        let mut out = Vec::new();
        let status = super::encode(
            &EncodeArgs {
                file: "no/such/file.ron".into(),
            },
            &mut out,
        )
        .unwrap();
        assert_eq!(status, FAILED);
    }
}
