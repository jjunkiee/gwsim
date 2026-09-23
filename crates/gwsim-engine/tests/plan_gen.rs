//! T4.6.3: plan generation, editing and the human reaction delay.

use std::path::PathBuf;

use gwsim_data::core::CoreData;
use gwsim_data::dataset::DataSet;
use gwsim_data::pack::DataPack;
use gwsim_data::plan::{Pick, PlanTarget, PriorityPlan};
use gwsim_engine::ai::plan_gen;
use gwsim_engine::log::LogKind;
use gwsim_engine::{FightSetup, SeedList};

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn data() -> DataSet {
    DataPack::from_dir(data_dir()).expect("data/ loads").data
}

fn player_plan(data: &DataSet) -> PriorityPlan {
    let party = data.party(&"m1-mesmerway".parse().unwrap()).unwrap();
    plan_gen::generate(&party.slots[0].build, data)
}

#[test]
fn the_generated_m1_player_plan_is_as_recorded() {
    let plan = player_plan(&data());
    let text = ron::ser::to_string_pretty(&plan, ron::ser::PrettyConfig::default()).unwrap();
    insta::assert_snapshot!(text);
}

#[test]
fn the_generated_plan_follows_the_design_example() {
    // DESIGN §11.5: keep Air of Superiority up; Arcane Echo, then Energy
    // Surge on the foe with the most energy; Mistrust on a caster; Unnatural
    // Signet on hexed or enchanted targets; Cry of Frustration and Power
    // Drain as interrupts; Spiritual Pain where foes are gathered.
    let data = data();
    let plan = player_plan(&data);
    let id = |slug: &str| data.skill(&slug.parse().unwrap()).unwrap().id;
    let target_of = |slug: &str| {
        plan.rules
            .iter()
            .find(|r| r.skill == gwsim_data::foe::SkillRef::Id(id(slug)))
            .map(|r| r.target.clone())
            .unwrap_or_else(|| panic!("no rule for {slug}"))
    };
    assert_eq!(
        plan.maintain,
        vec![gwsim_data::foe::SkillRef::Id(id("air-of-superiority"))]
    );
    assert_eq!(target_of("arcane-echo"), PlanTarget::SelfUnit);
    assert!(matches!(
        target_of("energy-surge"),
        PlanTarget::Foe {
            pick: Pick::MostEnergy,
            ..
        }
    ));
    assert!(matches!(
        target_of("mistrust"),
        PlanTarget::Foe {
            filter: Some(gwsim_data::dsl::Filter::HoldingCasterWeapon),
            ..
        }
    ));
    assert!(matches!(
        target_of("unnatural-signet"),
        PlanTarget::Foe {
            filter: Some(_),
            ..
        }
    ));
    assert!(matches!(
        target_of("cry-of-frustration"),
        PlanTarget::Foe {
            filter: Some(gwsim_data::dsl::Filter::Casting),
            ..
        }
    ));
    assert!(matches!(
        target_of("power-drain"),
        PlanTarget::Foe {
            filter: Some(gwsim_data::dsl::Filter::CastingSpell),
            ..
        }
    ));
    assert!(matches!(
        target_of("spiritual-pain"),
        PlanTarget::Foe {
            pick: Pick::MostFoesNearby,
            ..
        }
    ));
    // Interrupts come before damage (T4.6.1).
    let position = |slug: &str| {
        plan.rules
            .iter()
            .position(|r| r.skill == gwsim_data::foe::SkillRef::Id(id(slug)))
            .unwrap()
    };
    assert!(position("cry-of-frustration") < position("energy-surge"));
}

fn setup(plan: Option<PriorityPlan>) -> FightSetup {
    let pack = DataPack::from_dir(data_dir()).unwrap();
    let core = CoreData::load(data_dir().join("core")).unwrap();
    let mut party = pack
        .data
        .party(&"m0-player".parse().unwrap())
        .unwrap()
        .clone();
    party.slots[0].plan = plan;
    let situation = pack.data.situations[&"dummies-hm".parse().unwrap()]
        .value
        .clone();
    FightSetup::new(&pack.data, &core, &party, &situation).unwrap()
}

#[test]
fn an_edited_plan_is_followed_as_written() {
    // A plan that only keeps Air of Superiority up never casts anything else.
    let data = data();
    let only = PriorityPlan {
        maintain: vec![gwsim_data::foe::SkillRef::Slug(
            "air-of-superiority".parse().unwrap(),
        )],
        rules: Vec::new(),
        default: gwsim_data::plan::DefaultAction::Idle,
    };
    let result = setup(Some(only)).run_logged(SeedList::new(1, 1).get(0));
    let started: Vec<u16> = result
        .log
        .unwrap()
        .iter()
        .filter(|e| e.kind == LogKind::SkillStarted)
        .filter_map(|e| e.skill)
        .collect();
    let aos = data
        .skill(&"air-of-superiority".parse().unwrap())
        .unwrap()
        .id
        .get();
    assert!(!started.is_empty());
    assert!(started.iter().all(|s| *s == aos), "{started:?}");
}

#[test]
fn a_generated_plan_drives_the_m0_fight() {
    let result = setup(None).run(SeedList::new(1, 1).get(0));
    assert!(result.won(), "{:?}", result.outcome);
}

#[test]
fn decisions_wait_the_human_reaction_delay() {
    // A-012: each decision comes at least 250 ms after the last action ends.
    let result = setup(None).run_logged(SeedList::new(1, 1).get(0));
    let log = result.log.unwrap();
    let mut free_at: Option<u32> = None;
    for event in &log {
        match event.kind {
            LogKind::SkillCompleted if event.source == Some(0) => {
                // Free after the aftercast, at most 750 ms later.
                free_at = Some(event.t_ms);
            }
            LogKind::Decision if event.source == Some(0) => {
                if let Some(at) = free_at.take() {
                    assert!(
                        event.t_ms >= at + 250,
                        "decided at {} after completing at {at}",
                        event.t_ms
                    );
                }
            }
            _ => {}
        }
    }
}
