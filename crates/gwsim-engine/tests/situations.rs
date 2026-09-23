//! T4.8.6: the M1 situations, chains and situation switches.

use std::path::PathBuf;

use gwsim_data::core::CoreData;
use gwsim_data::dataset::DataSet;
use gwsim_data::pack::DataPack;
use gwsim_data::party::PartyFile;
use gwsim_data::scenario::{ChainStep, Situation, SituationEncounters};
use gwsim_engine::combat;
use gwsim_engine::time::{SimTime, TICK_MS};
use gwsim_engine::unit::HEALTH_SCALE;
use gwsim_engine::{FightSetup, Outcome, SeedList};

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn load() -> (DataSet, CoreData) {
    let pack = DataPack::from_dir(data_dir()).unwrap();
    let core = CoreData::load(data_dir().join("core")).unwrap();
    (pack.data, core)
}

fn party(data: &DataSet, slug: &str) -> PartyFile {
    data.party(&slug.parse().unwrap()).unwrap().clone()
}

fn situation(data: &DataSet, slug: &str) -> Situation {
    data.situations[&slug.parse().unwrap()].value.clone()
}

#[test]
fn every_m1_situation_runs_32_seeds_to_an_outcome() {
    let (data, core) = load();
    let m1 = party(&data, "m1-mesmerway");
    for slug in [
        "dummies-hm",
        "kournan-melee-heavy-hm",
        "kournan-caster-heavy-hm",
        "kournan-healer-heavy-hm",
        "kournan-patrol-hm",
        "kournan-patrol-hm-chain2",
    ] {
        let setup = FightSetup::new(&data, &core, &m1, &situation(&data, slug))
            .unwrap_or_else(|e| panic!("{slug}: {e}"));
        for seed in SeedList::new(4, 32).seeds() {
            let result = setup.run(*seed);
            assert!(
                matches!(
                    result.outcome,
                    Outcome::Win | Outcome::Wipe | Outcome::Timeout
                ),
                "{slug}"
            );
            assert!(!result.segments.is_empty(), "{slug}: no segment recorded");
        }
    }
}

#[test]
fn a_won_chain_records_both_fights() {
    let (data, core) = load();
    let setup = FightSetup::new(
        &data,
        &core,
        &party(&data, "m1-mesmerway"),
        &situation(&data, "kournan-patrol-hm-chain2"),
    )
    .unwrap();
    let wins: Vec<_> = SeedList::new(2, 16)
        .seeds()
        .iter()
        .map(|s| setup.run(*s))
        .filter(|r| r.won())
        .collect();
    assert!(!wins.is_empty());
    for result in wins {
        assert_eq!(result.segments.len(), 2);
        assert!(result.segments.iter().all(|s| s.outcome == Outcome::Win));
        assert_eq!(result.first_failed, None);
        let total: u32 = result.segments.iter().filter_map(|s| s.clear_time_ms).sum();
        assert!(
            result.clear_time_ms.unwrap() > total,
            "the chain's time includes the rest"
        );
    }
}

/// The player alone against dummies, twice, with a 20 second rest.
fn dummy_chain(data: &DataSet) -> Situation {
    let mut chain = situation(data, "dummies-hm");
    chain.encounters = SituationEncounters::Chain(vec![
        ChainStep {
            encounter: "training-dummies".parse().unwrap(),
            rest_after: None,
        },
        ChainStep {
            encounter: "training-dummies".parse().unwrap(),
            rest_after: None,
        },
    ]);
    chain
}

#[test]
fn health_carries_into_the_next_fight_and_regenerates_during_the_rest() {
    let (data, core) = load();
    let setup = FightSetup::new(
        &data,
        &core,
        &party(&data, "m0-player"),
        &dummy_chain(&data),
    )
    .unwrap();
    let mut sim = setup.sim(SeedList::new(1, 1).get(0));
    // Run fight 1 to its end.
    while sim.resting_until.is_none() && sim.outcome.is_none() {
        let next = sim.now.plus(TICK_MS);
        sim.step_until(next);
    }
    let until = sim
        .resting_until
        .expect("fight 1 is won and the rest begins");
    // Wound the player, then let the rest pass.
    sim.units[0].health = 100 * HEALTH_SCALE;
    let last_combat = sim.units[0].last_combat.unwrap_or(SimTime::ZERO);
    let from = sim.now;
    sim.step_until(until);
    // Natural regeneration only: +1 pip after 5 s out of combat, +1 every
    // 2 s, 2 health per pip per second, one tick at a time.
    let mut expected = 100 * HEALTH_SCALE;
    let mut t = (from.ms() / TICK_MS + 1) * TICK_MS;
    while t <= until.ms() {
        let pips = combat::natural_regeneration_pips(t - last_combat.ms());
        expected += pips * 2 * HEALTH_SCALE / (1000 / TICK_MS as i32);
        t += TICK_MS;
    }
    assert_eq!(sim.units[0].health, expected);
    // Fight 2 begins with that health, and its foes are fresh.
    let next = sim.now.plus(TICK_MS);
    sim.step_until(next);
    assert!(sim.units.iter().filter(|u| u.foe_index.is_some()).count() >= 6);
    assert!(sim.units[0].health <= expected + 1000);
}

#[test]
fn a_timeout_ends_the_fight_as_a_loss() {
    let (data, core) = load();
    let mut short = situation(&data, "dummies-hm");
    short.timeout = Some(gwsim_data::units::Seconds::from_secs_f64(10.0).unwrap());
    let setup = FightSetup::new(&data, &core, &party(&data, "m0-player"), &short).unwrap();
    let mut sim = setup.sim(SeedList::new(1, 1).get(0));
    for dummy in &mut sim.units[1..] {
        dummy.base_max_health = 100_000;
        dummy.health = 100_000 * HEALTH_SCALE;
    }
    let result = sim.run();
    assert_eq!(result.outcome, Outcome::Timeout);
    assert_eq!(result.ended_ms, 10_000);
}

#[test]
fn starting_death_penalty_lowers_maximum_health() {
    let (data, core) = load();
    let mut penalised = situation(&data, "dummies-hm");
    penalised.starting_dp = 15;
    let setup = FightSetup::new(&data, &core, &party(&data, "m0-player"), &penalised).unwrap();
    let sim = setup.sim(SeedList::new(1, 1).get(0));
    // 465 × 0.85 = 395.25.
    assert_eq!(sim.max_health(gwsim_engine::unit::UnitId(0)), 395);
}

#[test]
fn a_party_death_breaks_dhuums_covenant() {
    let (data, core) = load();
    let mut covenant = situation(&data, "dummies-hm");
    covenant.mode.dhuums_covenant = true;
    let setup = FightSetup::new(&data, &core, &party(&data, "m0-player"), &covenant).unwrap();
    let mut sim = setup.sim(SeedList::new(1, 1).get(0));
    sim.kill(gwsim_engine::unit::UnitId(0), None);
    let result = sim.finish();
    assert!(result.covenant_broken);
    assert_eq!(result.outcome, Outcome::Wipe);
    assert_eq!(result.dp_end, 15);
}

#[test]
fn reforged_mode_weakens_pre_searing_foes() {
    let (mut data, core) = load();
    let guard: gwsim_data::Slug = "kournan-guard".parse().unwrap();
    let normal = {
        let setup = FightSetup::new(
            &data,
            &core,
            &party(&data, "m0-player"),
            &situation(&data, "kournan-patrol-hm"),
        )
        .unwrap();
        setup
            .units()
            .iter()
            .find(|u| u.name == "Kournan Guard")
            .unwrap()
            .clone()
    };
    data.foes.get_mut(&guard).unwrap().value.pre_searing = true;
    let mut reforged = situation(&data, "kournan-patrol-hm");
    reforged.mode.reforged_mode = true;
    let setup = FightSetup::new(&data, &core, &party(&data, "m0-player"), &reforged).unwrap();
    let weakened = setup
        .units()
        .iter()
        .find(|u| u.name == "Kournan Guard")
        .unwrap();
    assert_eq!(
        weakened.base_max_health,
        (f64::from(normal.base_max_health) * 0.8).round() as i32
    );
    assert_eq!(
        weakened.armor[0].by_type[0],
        (f64::from(normal.armor[0].by_type[0]) * 0.8).round() as i16
    );
}

#[test]
fn melandrus_accord_refuses_a_mercenary_hero() {
    let (mut data, core) = load();
    data.heroes = Some(gwsim_data::dataset::Entry {
        path: "creatures/heroes.ron".into(),
        value: gwsim_data::foe::HeroesFile {
            provenance: data.dummies.as_ref().unwrap().value.provenance.clone(),
            heroes: vec![gwsim_data::foe::Hero {
                slug: "my-mercenary".parse().unwrap(),
                name: "My mercenary".into(),
                profession: gwsim_data::core::Profession::Mesmer,
                availability: gwsim_data::foe::HeroAvailability::Mercenary,
                mercenary: true,
            }],
        },
    });
    let mut m1 = party(&data, "m1-mesmerway");
    m1.slots[1].hero = Some("my-mercenary".parse().unwrap());
    let mut accord = situation(&data, "kournan-patrol-hm");
    accord.mode.melandrus_accord = true;
    let refused = FightSetup::new(&data, &core, &m1, &accord);
    assert!(refused.is_err());
    accord.mode.melandrus_accord = false;
    assert!(FightSetup::new(&data, &core, &m1, &accord).is_ok());
}
