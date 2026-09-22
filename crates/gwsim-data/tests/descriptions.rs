//! T1.5.7: golden tests for the description renderer.
//!
//! These snapshots are the **review standard**. A contributor encoding a
//! skill reads the generated sentence next to the wiki page and looks for a
//! disagreement, so a sentence that is grammatical but wrong is worse than
//! one that is obviously broken.
//!
//! None of the text below is taken from the game or the wiki (C2).

use gwsim_data::describe::{DescribeContext, describe, describe_action};
use gwsim_data::dsl::{Action, EffectDef, NoHandlers, Value};
use gwsim_data::skill::Skill;

fn action(text: &str) -> Action {
    ron::from_str(text).unwrap_or_else(|error| panic!("{text}\n{error}"))
}

fn render(text: &str) -> String {
    let context = DescribeContext::ranges(&NoHandlers);
    describe_action(&action(text), &[], &context).expect("should describe")
}

fn render_at(text: &str, rank: u8) -> String {
    let context = DescribeContext::at_rank(rank, &NoHandlers);
    describe_action(&action(text), &[], &context).expect("should describe")
}

// ------------------------------------------------------------ the criterion

/// T1.5.5's done criterion, from DESIGN §8.6.
#[test]
fn the_illustrative_fireball_renders_as_the_design_says() {
    let sentence =
        render("Damage(to: Adjacent(TargetFoe), kind: Some(Fire), amount: Scaled(7, 112))");
    assert_eq!(
        sentence,
        "Target foe and adjacent foes take 7…112 fire damage."
    );
}

#[test]
fn a_rank_turns_the_range_into_a_number() {
    // round(7 + 12 * 105 / 15) = 91.
    let sentence = render_at(
        "Damage(to: Adjacent(TargetFoe), kind: Some(Fire), amount: Scaled(7, 112))",
        12,
    );
    assert_eq!(
        sentence,
        "Target foe and adjacent foes take 91 fire damage."
    );
}

// ------------------------------------------------------------------ actions

#[test]
fn damage_variants() {
    insta::assert_snapshot!(
        [
            render("Damage(to: TargetFoe, amount: Fixed(20))"),
            render("Damage(to: TargetFoe, amount: Fixed(20), armor_ignoring: true)"),
            render("Damage(to: Nearby(TargetFoe), kind: Some(Lightning), amount: Scaled(5, 110))"),
            render("LifeSteal(to: TargetFoe, amount: Scaled(5, 65))"),
        ]
        .join("\n")
    );
}

#[test]
fn healing_and_resources() {
    insta::assert_snapshot!(
        [
            render("Heal(to: TargetAlly, amount: Scaled(30, 180))"),
            render("GainEnergy(to: Self, amount: Scaled(1, 31))"),
            render("LoseEnergy(to: TargetFoe, amount: Scaled(1, 10))"),
            render("SacrificeHealth(percent: Fixed(17))"),
            render("Resurrect(to: TargetAlly, health_percent: Fixed(50), energy_percent: Scaled(5, 35))"),
        ]
        .join("\n")
    );
}

#[test]
fn conditions_and_effects() {
    insta::assert_snapshot!(
        [
            render(
                "ApplyCondition(to: Adjacent(Self), condition: Weakness, duration: Scaled(5, 15))"
            ),
            render("RemoveConditions(from: Party, count: Fixed(99))"),
            render("RemoveEffects(from: TargetAlly, kind: Hex, count: Fixed(1))"),
            render("Interrupt(to: TargetFoe)"),
            render("FailSkill(to: Target)"),
            render("KnockDown(to: Adjacent(TargetFoe), duration: Fixed(2))"),
        ]
        .join("\n")
    );
}

#[test]
fn damage_reduction_has_four_distinct_readings() {
    // Six M1 skills reduce damage in four different ways, and each must read
    // as the thing it actually is.
    insta::assert_snapshot!(
        [
            render("ReduceIncomingDamage(to: TargetAlly, flat: Some(Scaled(3, 18)))"),
            render("ReduceIncomingDamage(to: Self, percent: Some(Fixed(50)))"),
            render("ReduceIncomingDamage(to: Self, percent: Some(Scaled(5, 35)), only_from: Some(Spells))"),
            render("ReduceIncomingDamage(to: InRangeOf(Spirits), cap_percent_of_max_health: Some(Fixed(10)))"),
        ]
        .join("\n")
    );
}

#[test]
fn creatures_and_bundles() {
    insta::assert_snapshot!(
        [
            render(r#"Summon(creature: "bone-fiend", level: Scaled(1, 17))"#),
            render(r#"CreateSpirit(spirit: "recuperation", level: Scaled(1, 14), duration: Scaled(15, 45))"#),
            render(r#"HoldBundle(bundle: "kaolais-ashes", duration: Scaled(15, 60))"#),
            render("DropBundle"),
        ]
        .join("\n")
    );
}

#[test]
fn stat_changes_read_as_gains_and_losses() {
    insta::assert_snapshot!(
        [
            render("ModifyStat(to: Party, stat: Armor, amount: Fixed(24))"),
            render("ModifyStat(to: Self, stat: HealthRegeneration, amount: Scaled(3, 10))"),
            render("ModifyStat(to: Self, stat: AttributeRank(DeathMagic), amount: Fixed(2))"),
            render("SetStat(to: Self, stat: ElementalAttributes, value: Scaled(8, 14))"),
        ]
        .join("\n")
    );
}

// ------------------------------------------------------------------ control

#[test]
fn a_conditional_skill_reads_as_a_condition() {
    insta::assert_snapshot!(render(
        r#"Control(If(
            condition: Enchanted,
            of: Some(TargetFoe),
            then: [Damage(to: TargetFoe, amount: Scaled(10, 100))],
        ))"#
    ));
}

#[test]
fn a_conditional_with_an_alternative_reads_as_both() {
    insta::assert_snapshot!(render(
        r#"Control(If(
            condition: BelowHealth(percent: 50.0),
            of: Some(TargetFoe),
            then: [GainEnergy(to: Self, amount: Fixed(7))],
            otherwise: [Damage(to: TargetFoe, amount: Fixed(10))],
        ))"#
    ));
}

#[test]
fn a_triggered_effect_with_charges_says_how_many() {
    insta::assert_snapshot!(
        [
            render(
                r#"Control(Triggered(
                    event: OnSpellCast,
                    charges: Some(1),
                    actions: [FailSkill(to: Target)],
                ))"#
            ),
            render(
                r#"Control(Triggered(
                    event: OnHit,
                    charges: Some(5),
                    actions: [Damage(to: Adjacent(TargetFoe), amount: Scaled(5, 50))],
                ))"#
            ),
            render(
                r#"Control(Triggered(
                    event: OnSkillUsed,
                    actions: [Interrupt(to: Nearby(TargetFoe))],
                ))"#
            ),
        ]
        .join("\n")
    );
}

#[test]
fn a_chance_reads_as_a_percentage() {
    insta::assert_snapshot!(render(
        "Control(Chance(percent: 20.0, actions: [Interrupt(to: TargetFoe)]))"
    ));
}

// ------------------------------------------------------------------- values

#[test]
fn a_secondary_target_share_is_spelled_out() {
    // The construct seven M1 skills use. A reader has to be able to tell that
    // nearby foes take less.
    insta::assert_snapshot!(render(
        r#"Damage(
            to: Secondary(of: Nearby(TargetFoe), factor: 0.75),
            amount: Scaled(10, 80),
        )"#
    ));
}

#[test]
fn per_unit_values_name_their_unit() {
    insta::assert_snapshot!(
        [
            render("Damage(to: TargetFoe, amount: PerUnit(value: Fixed(7), of: EnergyLost))"),
            render("Heal(to: TargetAlly, amount: PerUnit(value: Fixed(5), of: HealthLost))"),
            render(
                "GainEnergy(to: Self, amount: PerUnit(value: Fixed(4), of: CreaturesControlled))"
            ),
        ]
        .join("\n")
    );
}

#[test]
fn capped_and_floored_values_read_as_limits() {
    insta::assert_snapshot!(
        [
            render("Heal(to: TargetAlly, amount: Min(PerUnit(value: Fixed(1), of: HealthLost), Scaled(15, 80)))"),
            render("GainEnergy(to: Self, amount: Max(Fixed(1), Scaled(3, 12)))"),
        ]
        .join("\n")
    );
}

#[test]
fn a_title_scaled_value_shows_its_range_without_a_rank() {
    insta::assert_snapshot!(render(
        "Damage(to: TargetFoe, amount: TitleScaled(Asura, 10, 50))"
    ));
}

#[test]
fn a_percent_of_something_names_what_it_is_of() {
    insta::assert_snapshot!(render(
        "Heal(to: Self, amount: PercentOf(percent: 200.0, of: EnergyCost))"
    ));
}

// ---------------------------------------------------------- effect defs

#[test]
fn a_hex_with_a_duration_and_a_modifier() {
    let effect: EffectDef = ron::from_str(
        r#"(
            id: "life-siphon",
            kind: Hex,
            while_active: [
                ModifyStat(to: Target, stat: HealthRegeneration, amount: Scaled(-1, -3)),
            ],
        )"#,
    )
    .unwrap();

    let context = DescribeContext::ranges(&NoHandlers);
    let sentence = describe_action(
        &action(r#"ApplyEffect(to: TargetFoe, effect: "life-siphon", duration: Scaled(12, 24))"#),
        std::slice::from_ref(&effect),
        &context,
    )
    .unwrap();

    insta::assert_snapshot!(sentence);
}

#[test]
fn a_delayed_effect_reads_as_a_delay() {
    // Ancestors' Rage: an empty effect with an on_end. If this did not read
    // as a delay, the "no Delay action" decision would be wrong.
    let effect: EffectDef = ron::from_str(
        r#"(
            id: "ancestors-rage",
            kind: Enchantment,
            on_end: [
                Damage(to: Adjacent(TargetAlly), kind: Some(Lightning), amount: Scaled(5, 110)),
            ],
        )"#,
    )
    .unwrap();

    let context = DescribeContext::ranges(&NoHandlers);
    let sentence = describe_action(
        &action(r#"ApplyEffect(to: Self, effect: "ancestors-rage", duration: Fixed(1))"#),
        std::slice::from_ref(&effect),
        &context,
    )
    .unwrap();

    insta::assert_snapshot!(sentence);
}

#[test]
fn a_triggered_effect_inside_a_hex() {
    let effect: EffectDef = ron::from_str(
        r#"(
            id: "mistrust",
            kind: Hex,
            triggers: [
                Triggered(
                    event: OnSpellCast,
                    charges: Some(1),
                    actions: [
                        FailSkill(to: Target),
                        Damage(to: TargetFoe, amount: Scaled(10, 80)),
                    ],
                ),
            ],
        )"#,
    )
    .unwrap();

    let context = DescribeContext::ranges(&NoHandlers);
    let sentence = describe_action(
        &action(r#"ApplyEffect(to: TargetFoe, effect: "mistrust", duration: Fixed(6))"#),
        std::slice::from_ref(&effect),
        &context,
    )
    .unwrap();

    insta::assert_snapshot!(sentence);
}

// ----------------------------------------------------------------- handlers

struct StubHandler;

impl gwsim_data::dsl::HandlerDescribe for StubHandler {
    fn describe(&self, _params: &std::collections::BTreeMap<String, Value>) -> String {
        "Replaces itself with the next spell you cast.".to_owned()
    }
}

struct StubRegistry(StubHandler);

impl gwsim_data::dsl::HandlerRegistry for StubRegistry {
    fn get(&self, name: &str) -> Option<&dyn gwsim_data::dsl::HandlerDescribe> {
        (name == "arcane_echo").then_some(&self.0 as &dyn gwsim_data::dsl::HandlerDescribe)
    }
}

const HANDLER_SKILL: &str = r#"(
    id: 75,
    name: "Handler Skill",
    wiki: "Handler Skill",
    profession: Some(Mesmer),
    kind: EnchantmentSpell,
    campaign: Prophecies,
    cost: (energy: 15),
    activation: 2.0,
    recharge: 20.0,
    target: Self,
    encoding: Some((
        handler: Some((name: "arcane_echo")),
    )),
    provenance: (
        sources: ["https://wiki.guildwars.com/wiki/Arcane_Echo"],
        crawled: "2026-09-22",
        review: Draft,
    ),
)"#;

#[test]
fn a_handler_describes_itself_through_the_registry() {
    let skill: Skill = ron::from_str(HANDLER_SKILL).expect("should parse");
    let registry = StubRegistry(StubHandler);
    let context = DescribeContext::ranges(&registry);
    insta::assert_snapshot!(describe(&skill, &context));
}

#[test]
fn an_unregistered_handler_says_so_rather_than_pretending() {
    let skill: Skill = ron::from_str(HANDLER_SKILL).expect("should parse");
    let context = DescribeContext::ranges(&NoHandlers);
    let text = describe(&skill, &context);
    assert!(text.contains("arcane_echo"), "{text}");
    assert!(text.contains("no description"), "{text}");
}

// -------------------------------------------------------------- whole skills

#[test]
fn an_unencoded_skill_says_so() {
    let text = ron::from_str::<Skill>(&HANDLER_SKILL.replace(
        r#"encoding: Some((
        handler: Some((name: "arcane_echo")),
    )),"#,
        "encoding: None,",
    ))
    .map(|skill| describe(&skill, &DescribeContext::ranges(&NoHandlers)))
    .expect("should parse");

    assert_eq!(text, "This skill has not been encoded yet.");
}

#[test]
fn a_multi_action_skill_reads_as_several_sentences() {
    let skill: Skill = ron::from_str(
        r#"(
    id: 68,
    name: "Multi Action",
    wiki: "Multi Action",
    profession: Some(Mesmer),
    kind: Spell,
    campaign: Prophecies,
    cost: (energy: 5),
    activation: 2.0,
    recharge: 20.0,
    target: Foe,
    encoding: Some((
        effects: [
            RemoveEffects(from: TargetFoe, kind: Enchantment, count: Fixed(1)),
            Control(Triggered(
                event: OnEnchantmentRemoved,
                charges: Some(1),
                actions: [
                    GainEnergy(to: Self, amount: Scaled(8, 17)),
                    Heal(to: Self, amount: Scaled(40, 120)),
                ],
            )),
        ],
    )),
    provenance: (
        sources: ["https://wiki.guildwars.com/wiki/Drain_Enchantment"],
        crawled: "2026-09-22",
        review: Draft,
    ),
)"#,
    )
    .expect("should parse");

    insta::assert_snapshot!(describe(&skill, &DescribeContext::ranges(&NoHandlers)));
}

#[test]
fn every_sentence_starts_with_a_capital_and_ends_with_a_stop() {
    // A cheap guard that keeps the generated text readable as prose rather
    // than as a debug dump.
    let samples = [
        "Damage(to: TargetFoe, amount: Fixed(20))",
        "Heal(to: TargetAlly, amount: Fixed(20))",
        "GainEnergy(to: Self, amount: Fixed(5))",
        "Interrupt(to: TargetFoe)",
        "ApplyCondition(to: TargetFoe, condition: Bleeding, duration: Fixed(5))",
        "KnockDown(to: TargetFoe, duration: Fixed(2))",
        "ModifyStat(to: Self, stat: Armor, amount: Fixed(20))",
        "SetUnblockable",
        "DropBundle",
    ];
    for text in samples {
        let sentence = render(text);
        let first = sentence.chars().next().expect("not empty");
        assert!(
            first.is_uppercase(),
            "{text} rendered as {sentence:?}, which does not start with a capital"
        );
        assert!(
            sentence.ends_with('.'),
            "{text} rendered as {sentence:?}, which does not end with a full stop"
        );
    }
}
