//! The parsers over the synthetic fixtures: discovery (T2.3.5), skills
//! (T2.4.6) and foes and areas (T2.5.5).

mod common;

use std::collections::BTreeSet;

use common::fixture;
use gwsim_data::core::{
    Attribute, Campaign, DamageType, Profession, RangeBand, SkillType, TitleTrack,
};
use gwsim_data::skill::{Aoe, TargetKind};
use gwsim_data::{SkillId, WikiTitle};
use gwsim_extractor::discovery::{
    cross_check_category, merge, parse_category_members, parse_game_integration, parse_named_rows,
    parse_profession_list, parse_skill_list,
};
use gwsim_extractor::foe::{parse_area, parse_foe};
use gwsim_extractor::normalise::normalise;
use gwsim_extractor::skill::parse_skill;

fn title(text: &str) -> WikiTitle {
    WikiTitle(text.to_owned())
}

fn normalised(file: &str, name: &str) -> gwsim_extractor::normalise::Normalised {
    let raw = parse_skill(&fixture(file)).expect("fixture should have an infobox");
    normalise(&raw, &title(name), None, "2026-09-23".parse().unwrap()).expect("should normalise")
}

// -------------------------------------------------------------- discovery

#[test]
fn the_skill_list_gives_ids_and_titles_without_the_placeholder() {
    let listed = parse_skill_list(&fixture("skill_list.html"));
    assert_eq!(listed.len(), 8, "No Skill (id 0) is not a skill");
    assert_eq!(listed[0].id, SkillId(9001));
    assert_eq!(listed[0].title, title("Lorem Surge"));
    assert!(listed.iter().any(|s| s.title.as_str() == "Ipsum Hex (PvP)"));
}

#[test]
fn a_game_integration_page_gives_rows_and_notices_a_further_page() {
    let page = parse_game_integration(&fixture("game_integration.html"));
    assert_eq!(page.rows.len(), 5);
    assert_eq!(page.rows[2].title, Some(title("Adipiscing Laughter")));
    assert_eq!(page.rows[4].title, None, "an unused id has no page");
    // 501-1000 is a known range, so no further page is reported.
    assert_eq!(page.next_page, None);
}

#[test]
fn a_profession_list_gives_attribute_campaign_and_elite() {
    let rows = parse_profession_list(&fixture("profession_list.html"), Some(Profession::Mesmer));
    assert_eq!(rows.len(), 4);
    let surge = &rows[0];
    assert_eq!(surge.title, title("Lorem Surge"));
    assert!(surge.elite);
    assert_eq!(surge.attribute.as_deref(), Some("Domination Magic"));
    assert_eq!(surge.campaign, Some(Campaign::Core));
    assert!(!rows[1].elite);
    assert_eq!(rows[3].attribute, None, "No Attribute is None");
    assert_eq!(rows[3].campaign, Some(Campaign::Factions));
}

#[test]
fn the_merge_joins_by_title_and_reports_the_planted_gaps() {
    let listed = parse_skill_list(&fixture("skill_list.html"));
    let professions =
        parse_profession_list(&fixture("profession_list.html"), Some(Profession::Mesmer));
    let pve_only = parse_named_rows(&fixture("pve_only_list.html"));
    let integration = parse_game_integration(&fixture("game_integration.html")).rows;

    let merged = merge(&listed, &professions, &pve_only, &integration);

    let titles: Vec<&str> = merged.skills.iter().map(|s| s.title.as_str()).collect();
    assert!(
        !titles.contains(&"Ipsum Hex (PvP)"),
        "PvP titles are excluded (D3)"
    );
    assert_eq!(merged.report.pvp_excluded.len(), 1);

    let surge = merged
        .skills
        .iter()
        .find(|s| s.id == SkillId(9001))
        .unwrap();
    assert_eq!(surge.profession, Some(Profession::Mesmer));
    assert!(surge.elite);
    assert_eq!(surge.slug.as_str(), "lorem-surge");

    let title_skill = merged
        .skills
        .iter()
        .find(|s| s.id == SkillId(9003))
        .unwrap();
    assert!(title_skill.pve_only);
    assert_eq!(title_skill.profession, None);

    // Sit Chop and Amet is Power are on no list in the fixtures.
    let unlisted: BTreeSet<&str> = merged
        .report
        .ids_without_list_entry
        .iter()
        .map(|s| s.title.as_str())
        .collect();
    assert_eq!(unlisted, BTreeSet::from(["Sit Chop", "Amet is Power"]));

    // The integration page's ids 1-4 are not the skill list's 9001-9009.
    assert_eq!(merged.report.monster_or_unlisted.len(), 4);
    assert!(merged.skills.windows(2).all(|pair| pair[0].id < pair[1].id));
}

#[test]
fn a_category_cross_check_counts_matches_and_gaps() {
    let listed = parse_skill_list(&fixture("skill_list.html"));
    let professions =
        parse_profession_list(&fixture("profession_list.html"), Some(Profession::Mesmer));
    let merged = merge(&listed, &professions, &BTreeSet::new(), &[]);
    let members = parse_category_members(&fixture("category.html"));
    assert_eq!(members.len(), 3);
    let check = cross_check_category(&members, &merged.skills);
    assert_eq!(check.matched, 3);
    assert!(check.missing_from_index.is_empty());

    let with_gap = [members, vec![title("Unheard Of")]].concat();
    let check = cross_check_category(&with_gap, &merged.skills);
    assert_eq!(check.missing_from_index, vec![title("Unheard Of")]);
}

// ------------------------------------------------------------------ skills

#[test]
fn a_caster_skill_parses_to_its_raw_fields() {
    let raw = parse_skill(&fixture("skill_basic.html")).unwrap();
    assert_eq!(raw.name, "Lorem Surge");
    assert_eq!(raw.id, Some(9001));
    assert_eq!(raw.box_class, "Mesmer");
    assert_eq!(raw.stat("Energy"), Some("10"));
    assert_eq!(
        raw.stat("Activation"),
        Some("0.75"),
        "the hidden sort key wins over ¾"
    );
    assert_eq!(raw.stat("Recharge"), Some("12"));
    assert_eq!(raw.profession.as_deref(), Some("Mesmer"));
    assert_eq!(raw.attribute.as_deref(), Some("Domination Magic"));
    assert_eq!(raw.type_text.as_deref(), Some("Elite spell"));
    assert_eq!(raw.campaigns, vec!["Core"]);
    assert!(raw.in_category("Skills with nearby AoE"));
    let progression = raw.progression.as_ref().unwrap();
    assert_eq!(progression.attribute_label, "Domination Magic");
    assert_eq!(progression.rows.len(), 2);
    assert_eq!(progression.rows[0].values.len(), 22);
    assert_eq!(raw.description_values, vec![vec![3, 9, 11]]);
}

#[test]
fn a_caster_skill_normalises_to_typed_numbers() {
    let result = normalised("skill_basic.html", "Lorem Surge");
    let skill = &result.skill;
    assert_eq!(skill.id, SkillId(9001));
    assert_eq!(skill.profession, Some(Profession::Mesmer));
    assert_eq!(skill.attribute, Some(Attribute::DominationMagic));
    assert_eq!(skill.kind, SkillType::Spell);
    assert!(skill.elite);
    assert_eq!(skill.campaign, Campaign::Core);
    assert_eq!(skill.cost.energy, 10);
    assert_eq!(skill.activation.ms(), 750);
    assert_eq!(skill.recharge.ms(), 12_000);
    assert_eq!(skill.target, TargetKind::Foe);
    assert_eq!(skill.range, Some(RangeBand::Casting));
    assert_eq!(skill.aoe, Some(Aoe::Band(RangeBand::Nearby)));

    let scaled = &skill.extracted.scaled;
    assert_eq!(
        (
            scaled[0].label.as_str(),
            scaled[0].r0,
            scaled[0].r12,
            scaled[0].r15
        ),
        ("energy loss", 3, 9, 11)
    );
    assert_eq!((scaled[1].r0, scaled[1].r12, scaled[1].r15), (6, 73, 90));
    assert!(scaled.iter().all(|n| !n.special_rounding));
    assert!(
        skill
            .extracted
            .description_hash
            .as_deref()
            .unwrap()
            .starts_with("blake3:")
    );
    assert!(skill.encoding.is_none());
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
}

#[test]
fn a_derived_row_is_flagged_special_rounding() {
    let skill = normalised("skill_special_rounding.html", "Consectetur Spike").skill;
    let scaled = &skill.extracted.scaled;
    assert!(!scaled[0].special_rounding);
    assert!(
        scaled[1].special_rounding,
        "5 × energy loss does not follow Gr"
    );
    assert_eq!((scaled[1].r0, scaled[1].r12, scaled[1].r15), (5, 40, 50));
}

#[test]
fn a_split_page_normalises_as_its_pve_version() {
    let skill = normalised("skill_split_pve.html", "Ipsum Hex").skill;
    assert_eq!(skill.kind, SkillType::HexSpell);
    assert_eq!(skill.campaign, Campaign::Nightfall);
    assert_eq!(
        skill.extracted.scaled.len(),
        2,
        "a 21-column table still gives triples"
    );
    assert!(skill.provenance.notes.contains("PvE version"));
}

#[test]
fn a_title_skill_records_its_track_and_title_rank_values() {
    let result = normalised("skill_title.html", "Dolor of Superiority");
    let skill = &result.skill;
    assert!(skill.pve_only);
    assert_eq!(skill.profession, None);
    assert_eq!(skill.attribute, None);
    assert_eq!(skill.title_track, Some(TitleTrack::Asura));
    assert_eq!(skill.kind, SkillType::Skill);
    assert_eq!(skill.target, TargetKind::None);
    let duration = &skill.extracted.scaled[0];
    // Title ranks 0, 4 and 5 are effective ranks 0, 12 and 15.
    assert_eq!((duration.r0, duration.r12, duration.r15), (12, 20, 22));
    assert!(!duration.special_rounding);
}

#[test]
fn adrenaline_sacrifice_upkeep_and_overcast_are_costs() {
    let chop = normalised("skill_adrenaline.html", "Sit Chop").skill;
    assert_eq!(chop.cost.adrenaline, 6);
    assert_eq!(chop.kind, SkillType::AxeAttack);
    assert_eq!(chop.activation.ms(), 0);
    assert_eq!(chop.range, None, "attack skills reach as far as the weapon");
    assert!(chop.provenance.notes.contains("no progression table"));

    let power = normalised("skill_sacrifice.html", "Amet is Power").skill;
    assert_eq!(power.cost.sacrifice_pct, 21);
    assert_eq!(power.cost.upkeep, 1);
    assert_eq!(power.cost.overcast, 4);
    assert_eq!(power.cost.energy, 3);
    assert_eq!(power.activation.ms(), 1500);
    assert_eq!(power.attribute, None);
    assert_eq!(power.target, TargetKind::OtherAlly);
    assert!(power.elite);
}

#[test]
fn a_monster_skill_has_no_profession_and_is_marked_monster() {
    let result = normalised("skill_monster.html", "Adipiscing Laughter");
    assert!(result.monster);
    assert_eq!(result.skill.profession, None);
    assert_eq!(
        result.skill.campaign,
        Campaign::Prophecies,
        "the first known campaign"
    );
    assert_eq!(result.skill.aoe, Some(Aoe::Band(RangeBand::Earshot)));
}

#[test]
fn the_page_id_is_the_fallback_and_a_disagreement_is_warned() {
    let raw = parse_skill(&fixture("skill_basic.html")).unwrap();
    let date: gwsim_data::IsoDate = "2026-09-23".parse().unwrap();
    let from_index = normalise(
        &raw,
        &title("Lorem Surge"),
        Some(SkillId(1234)),
        date.clone(),
    )
    .unwrap();
    assert_eq!(from_index.skill.id, SkillId(1234));
    assert!(
        from_index
            .warnings
            .iter()
            .any(|w| w.message.contains("1234"))
    );

    let mut no_id = raw.clone();
    no_id.id = None;
    assert!(normalise(&no_id, &title("Lorem Surge"), None, date).is_err());
}

#[test]
fn the_same_page_always_hashes_the_same() {
    let first = normalised("skill_basic.html", "Lorem Surge").skill;
    let second = normalised("skill_redirected.html", "Lorem Surge").skill;
    assert_eq!(
        first.extracted.description_hash,
        second.extracted.description_hash
    );
}

// -------------------------------------------------------------------- foes

#[test]
fn a_foe_page_gives_levels_attributes_skills_and_armor() {
    let foe = parse_foe(&fixture("foe_basic.html")).unwrap();
    assert_eq!(foe.name, "Lorem Seer");
    assert_eq!(foe.affiliation.as_deref(), Some("Lorem military"));
    assert_eq!(foe.species.as_deref(), Some("Human"));
    assert_eq!(foe.professions, vec![Profession::Mesmer]);
    assert_eq!(foe.levels.len(), 2);
    let top = foe.top_level().unwrap();
    assert_eq!((top.nm, top.hm), (19, Some(27)));

    let variant = &foe.variants[0];
    assert_eq!(
        variant.attributes.nm,
        vec![
            (Attribute::DominationMagic, 13),
            (Attribute::InspirationMagic, 11)
        ]
    );
    assert_eq!(
        variant.attributes.hm,
        Some(vec![
            (Attribute::DominationMagic, 18),
            (Attribute::InspirationMagic, 11)
        ])
    );
    assert_eq!(variant.skills.len(), 4);
    assert!(variant.skills[1].elite);
    assert_eq!(variant.skills[1].min_level, Some(20));
    assert!(variant.skills[3].hm_only);
    assert!(!variant.skills[0].hm_only);

    let armor = &foe.armor[0];
    assert_eq!(armor.level, Some(19));
    assert_eq!(armor.values.len(), 7);
    assert!(armor.values.iter().all(|(_, value)| *value == 57));
    assert!(foe.warnings.is_empty(), "{:?}", foe.warnings);
}

#[test]
fn a_foe_with_variants_keeps_each_bar_and_both_armor_figures() {
    let foe = parse_foe(&fixture("foe_variants.html")).unwrap();
    assert_eq!(foe.variants.len(), 2);
    assert_eq!(foe.variants[0].name.as_deref(), Some("Axe-wielder"));
    assert_eq!(foe.variants[1].name.as_deref(), Some("Hammer-wielder"));
    assert_eq!(
        foe.variants[0].attributes.nm,
        vec![(Attribute::Strength, 12)]
    );
    assert_eq!(
        foe.variants[1].attributes.nm,
        vec![(Attribute::Strength, 12)],
        "a variant with no attribute line shares the first's"
    );
    assert_eq!(foe.variants[1].skills.len(), 3);

    assert_eq!(foe.armor.len(), 2);
    let axe = &foe.armor[0];
    assert_eq!(axe.label.as_deref(), Some("Axe wielder"));
    let value = |damage| {
        axe.values
            .iter()
            .find(|(d, _)| *d == damage)
            .map(|(_, v)| *v)
    };
    assert_eq!(value(DamageType::Slashing), Some(111));
    assert_eq!(value(DamageType::Fire), Some(91));
    assert_eq!(foe.armor[1].label.as_deref(), Some("Hammer wielder"));
}

#[test]
fn an_area_roster_lists_its_groups_with_levels_and_its_bosses() {
    let roster = parse_area(&fixture("area.html"));
    assert_eq!(roster.area, "Ipsum Valley");
    assert_eq!(roster.groups.len(), 2);
    assert_eq!(roster.groups[1].name, "Humans (Lorem military)");
    let seer = &roster.groups[1].foes[1];
    assert_eq!(seer.title, "Lorem Seer");
    assert_eq!(seer.profession, Some(Profession::Mesmer));
    let level = seer.level.as_ref().unwrap();
    assert_eq!((level.nm, level.hm), (19, Some(27)));
    assert_eq!(roster.bosses.len(), 1);
    assert_eq!(roster.bosses[0].foes[0].title, "Sit the Proud");
    assert_eq!(
        roster.foe_titles().len(),
        4,
        "NPCs before the Foes heading are not foes"
    );
}

#[test]
fn game_update_lines_attach_to_skills_and_skip_pvp() {
    let lines = gwsim_extractor::update::parse_update(&fixture("game_update.html"), "20990101");
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].skill, "Lorem Surge");
    assert_eq!(lines[1].skill, "Ipsum Hex");
    assert!(lines[0].change.contains("9 to 7"));
}
