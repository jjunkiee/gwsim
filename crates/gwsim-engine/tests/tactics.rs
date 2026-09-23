//! T4.7.6: the tactics plan — generation against the PvX notes, the
//! pre-fight phase, formation positions and overrides.

use std::path::PathBuf;

use gwsim_data::core::CoreData;
use gwsim_data::dataset::DataSet;
use gwsim_data::pack::DataPack;
use gwsim_data::party::PartyFile;
use gwsim_data::scenario::Situation;
use gwsim_data::skill::RoleTag;
use gwsim_data::tactics::{HeroModeName, SlotMode, TacticsOverrides, TargetRule};
use gwsim_engine::log::LogKind;
use gwsim_engine::unit::HeroMode;
use gwsim_engine::{FightSetup, SeedList};

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn load() -> (DataSet, CoreData, PartyFile, Situation) {
    let pack = DataPack::from_dir(data_dir()).unwrap();
    let core = CoreData::load(data_dir().join("core")).unwrap();
    let party = pack
        .data
        .party(&"m1-mesmerway".parse().unwrap())
        .unwrap()
        .clone();
    let situation = pack.data.situations[&"kournan-patrol-hm".parse().unwrap()]
        .value
        .clone();
    (pack.data, core, party, situation)
}

#[test]
fn the_m1_tactics_plan_is_as_recorded() {
    let (data, core, party, situation) = load();
    let setup = FightSetup::new(&data, &core, &party, &situation).unwrap();
    let text = ron::ser::to_string_pretty(&setup.fight.tactics, ron::ser::PrettyConfig::default())
        .unwrap();
    insta::assert_snapshot!(text);
}

#[test]
fn the_plan_follows_the_pvx_tactics_notes() {
    // §20.1: backline on Guard, midline on Fight; pre-cast Shelter → Union →
    // Displacement → Armor of Unfeeling; flag heroes apart against AoE.
    let (data, core, party, situation) = load();
    let setup = FightSetup::new(&data, &core, &party, &situation).unwrap();
    let plan = &setup.fight.tactics;
    assert_eq!(
        plan.mode_of("hero 7"),
        HeroModeName::Guard,
        "the spirit-caster is backline"
    );
    assert_eq!(
        plan.mode_of("hero 5"),
        HeroModeName::Guard,
        "the healer is backline"
    );
    assert_eq!(
        plan.mode_of("hero 1"),
        HeroModeName::Fight,
        "a Mesmer is midline"
    );
    let order: Vec<String> = plan
        .pre_fight
        .iter()
        .map(|p| format!("{:?}", p.skill))
        .collect();
    let position = |slug: &str| order.iter().position(|s| s.contains(slug)).unwrap();
    assert!(position("soul-twisting") < position("\"shelter"));
    assert!(position("\"shelter") < position("\"union"));
    assert!(position("\"union") < position("displacement"));
    assert!(position("displacement") < position("armor-of-unfeeling"));
    assert!(
        plan.spread_against_aoe,
        "Kournan Scribes cast Fireball and Meteor"
    );
    assert_eq!(
        plan.called_targets,
        vec![TargetRule::FoeWithRole(RoleTag::Healing)]
    );
}

#[test]
fn the_spirits_are_up_before_aggro_and_cost_energy() {
    let (data, core, party, situation) = load();
    let setup = FightSetup::new(&data, &core, &party, &situation).unwrap();
    let mut sim = setup.sim(SeedList::new(1, 1).get(0)).with_log();
    let hero7 = sim
        .units
        .iter()
        .position(|u| u.slot_index == Some(7))
        .unwrap();
    let full = sim.units[hero7].energy;
    sim.step_until(gwsim_engine::time::SimTime(8_000));
    assert!(
        sim.aggroed.iter().all(|a| !*a),
        "no foe has noticed the party yet"
    );
    let spirits = sim
        .units
        .iter()
        .filter(|u| u.kind == gwsim_engine::unit::UnitKind::Spirit && u.alive())
        .count();
    assert_eq!(spirits, 3, "Shelter, Union and Displacement stand");
    assert!(sim.units[hero7].energy < full, "the rituals were paid for");
    let casts = sim
        .log
        .as_ref()
        .unwrap()
        .iter()
        .filter(|e| e.kind == LogKind::Decision && e.detail == "pre-fight cast")
        .count();
    assert_eq!(casts, 5);
}

#[test]
fn heroes_start_at_their_formation_points() {
    let (data, core, party, situation) = load();
    let setup = FightSetup::new(&data, &core, &party, &situation).unwrap();
    let units = setup.units();
    let leader = units.iter().find(|u| u.slot_index == Some(0)).unwrap().pos;
    for slot in 1..8 {
        let hero = units.iter().find(|u| u.slot_index == Some(slot)).unwrap();
        assert!(
            hero.pos.y < leader.y,
            "hero {slot} stands behind the player"
        );
        let expected = if matches!(hero.hero_mode, HeroMode::Guard) {
            -350.0
        } else {
            -100.0
        };
        assert!(
            (hero.pos.y - leader.y - expected).abs() < 1e-3,
            "hero {slot} at {}",
            hero.pos.y
        );
    }
    // Spread against area damage: nobody within the nearby band of another
    // on the same line.
    for a in units.iter().filter(|u| u.slot_index.is_some()) {
        for b in units
            .iter()
            .filter(|u| u.slot_index.is_some() && u.id != a.id)
        {
            if (a.pos.y - b.pos.y).abs() < 1.0 {
                assert!(
                    a.pos.distance(b.pos) > 252.0,
                    "{} and {} are too close",
                    a.name,
                    b.name
                );
            }
        }
    }
}

#[test]
fn an_override_survives_regeneration() {
    let (data, core, mut party, situation) = load();
    party.tactics = Some(TacticsOverrides {
        hero_modes: Some(vec![SlotMode {
            slot: "hero 7".into(),
            mode: HeroModeName::Fight,
        }]),
        ..TacticsOverrides::default()
    });
    let setup = FightSetup::new(&data, &core, &party, &situation).unwrap();
    let plan = &setup.fight.tactics;
    assert_eq!(
        plan.mode_of("hero 7"),
        HeroModeName::Fight,
        "the user's choice stands"
    );
    assert_eq!(
        plan.mode_of("hero 5"),
        HeroModeName::Guard,
        "the rest is still generated"
    );
    assert!(!plan.pre_fight.is_empty());
}
