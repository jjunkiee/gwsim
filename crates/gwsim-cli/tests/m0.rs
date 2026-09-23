//! T3.10.7: the M0 acceptance tests.
//!
//! The player's eight skills against the dummy spec (level 26, 40 energy that
//! never regenerates, armor 60, 480 health), with the numbers from
//! `docs/findings/T3.10.1-m0-hand-calcs.md`. Effective ranks: Fast Casting
//! 11, Domination Magic 16, Inspiration Magic 9.
//!
//! Units: 0 is the player; 1 is the first dummy; 2 stands adjacent to it
//! (100 away); 3 is nearby but not adjacent (224 away).

use std::path::PathBuf;

use assert_cmd::Command;
use gwsim_data::core::CoreData;
use gwsim_data::pack::DataPack;
use gwsim_engine::ai::Controller;
use gwsim_engine::sim::{Fired, Sim};
use gwsim_engine::time::SimTime;
use gwsim_engine::unit::{Action, HEALTH_SCALE, SlotState, Target, UnitId, UnitKind};
use gwsim_engine::{FightSetup, RunSeed, SeedList};

const PLAYER: UnitId = UnitId(0);
const FIRST: UnitId = UnitId(1);
const ADJACENT: UnitId = UnitId(2);
const NEARBY: UnitId = UnitId(3);

// Bar slots, in the party file's order.
const ARCANE_ECHO: u8 = 0;
const ENERGY_SURGE: u8 = 1;
const MISTRUST: u8 = 2;
const UNNATURAL_SIGNET: u8 = 3;
const CRY_OF_FRUSTRATION: u8 = 4;
const SPIRITUAL_PAIN: u8 = 5;
const POWER_DRAIN: u8 = 6;
const AIR_OF_SUPERIORITY: u8 = 7;

const DUMMY_HEALTH: i32 = 480;
const DUMMY_ENERGY: i32 = 40;

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn setup() -> FightSetup {
    let pack = DataPack::from_dir(data_dir()).expect("data/ loads");
    let core = CoreData::load(data_dir().join("core")).expect("core data loads");
    let party = pack
        .data
        .party(&"m0-player".parse().unwrap())
        .expect("the M0 party")
        .clone();
    let situation = pack.data.situations[&"dummies-hm".parse().unwrap()]
        .value
        .clone();
    FightSetup::new(&pack.data, &core, &party, &situation).expect("the M0 fight prepares")
}

fn seed() -> RunSeed {
    SeedList::new(1, 1).get(0)
}

/// A fight in which nobody decides anything: the test gives every order.
fn scripted() -> Sim {
    let mut sim = setup().sim(seed()).with_log();
    sim.controllers.fill(Controller::Idle);
    sim
}

fn health(sim: &Sim, unit: UnitId) -> i32 {
    sim.units[unit.index()].health_points()
}

fn damage_taken(sim: &Sim, unit: UnitId) -> i32 {
    DUMMY_HEALTH - health(sim, unit)
}

fn skill_in(sim: &Sim, slot: u8) -> u16 {
    sim.units[PLAYER.index()].bar[usize::from(slot)]
        .as_ref()
        .expect("a skill in the slot")
        .skill
}

/// Uses a skill now, runs until just after its activation ends, then on
/// through the aftercast so the player is free again.
fn use_and_finish(sim: &mut Sim, slot: u8, target: Target) {
    let skill = skill_in(sim, slot);
    let activation = sim.activation_ms(PLAYER, skill);
    sim.use_skill(PLAYER, slot, target)
        .expect("the skill is usable");
    let until = sim.now.plus(activation + 1);
    sim.step_until(until);
    while !matches!(sim.units[PLAYER.index()].action, Action::Idle) {
        let next = sim.now.plus(50);
        sim.step_until(next);
    }
}

/// Makes a dummy start casting the player's Energy Surge, so interrupts have
/// a spell to stop.
fn make_casting(sim: &mut Sim, unit: UnitId) {
    let spell = skill_in(sim, ENERGY_SURGE);
    let dummy = &mut sim.units[unit.index()];
    dummy.bar[0] = Some(SlotState {
        skill: spell,
        original: spell,
        ready_at: SimTime(0),
        disabled_until: SimTime(0),
        adrenaline: 0,
        revert_generation: 0,
    });
    dummy.action = Action::Activating {
        slot: 0,
        target: Target::Unit(PLAYER),
        started: sim.now,
        ends_at: SimTime(60_000),
        failed: false,
    };
}

// ------------------------------------------------------------- activation

#[test]
fn activation_and_recharge_match_the_hand_calculations() {
    let mut sim = scripted();
    // (slot, activation ms, recharge ms): Fast Casting 11 multiplies
    // activation by 0.5^(11/15) = 0.6015; PvE Mesmer spells recharge 33%
    // faster, rounded to the second.
    let expected = [
        (ARCANE_ECHO, 1203, 13_000),
        (ENERGY_SURGE, 1203, 10_000),
        (MISTRUST, 1203, 8_000),
        (UNNATURAL_SIGNET, 602, 10_000),
        (CRY_OF_FRUSTRATION, 150, 13_000),
        (SPIRITUAL_PAIN, 602, 5_000),
        (POWER_DRAIN, 150, 13_000),
        (AIR_OF_SUPERIORITY, 0, 20_000),
    ];
    for (slot, activation, recharge) in expected {
        let skill = skill_in(&sim, slot);
        assert_eq!(
            sim.activation_ms(PLAYER, skill),
            activation,
            "slot {slot} activation"
        );
        assert_eq!(
            sim.recharge_ms(PLAYER, skill),
            recharge,
            "slot {slot} recharge"
        );
    }
}

#[test]
fn the_player_has_the_derived_stats() {
    let sim = scripted();
    assert_eq!(sim.max_energy(PLAYER), 42);
    assert_eq!(sim.max_health(PLAYER), 465);
}

// ----------------------------------------------------------------- values

#[test]
fn energy_surge_drains_11_and_deals_77_then_58_nearby() {
    let mut sim = scripted();
    use_and_finish(&mut sim, ENERGY_SURGE, Target::Unit(FIRST));
    assert_eq!(sim.units[FIRST.index()].energy_points(), DUMMY_ENERGY - 11);
    assert_eq!(damage_taken(&sim, FIRST), 77);
    assert_eq!(damage_taken(&sim, ADJACENT), 58);
    assert_eq!(damage_taken(&sim, NEARBY), 58);
}

#[test]
fn energy_surge_damage_follows_the_energy_actually_lost() {
    let mut sim = scripted();
    sim.units[FIRST.index()].energy = 4 * gwsim_engine::unit::ENERGY_SCALE;
    use_and_finish(&mut sim, ENERGY_SURGE, Target::Unit(FIRST));
    assert_eq!(sim.units[FIRST.index()].energy_points(), 0);
    assert_eq!(damage_taken(&sim, FIRST), 28);
    assert_eq!(damage_taken(&sim, ADJACENT), 21);
}

#[test]
fn mistrust_fails_the_next_spell_and_deals_64_to_every_foe_it_reaches() {
    let mut sim = scripted();
    use_and_finish(&mut sim, MISTRUST, Target::Unit(FIRST));
    assert_eq!(
        damage_taken(&sim, FIRST),
        0,
        "nothing happens until the foe casts"
    );

    // The dummy casts a spell on the player, one of the caster's allies.
    make_casting(&mut sim, FIRST);
    let mut fired = Fired::new(gwsim_data::dsl::Event::OnSpellCast, FIRST);
    fired.other = Some(PLAYER);
    sim.fire(fired);
    // Since 2026-06-24 the target takes the area's 75% too: 0.75 × 85.
    assert_eq!(damage_taken(&sim, FIRST), 64);
    assert_eq!(damage_taken(&sim, ADJACENT), 64);
    assert_eq!(damage_taken(&sim, NEARBY), 64);

    // One charge: a second spell does nothing.
    sim.fire(fired);
    assert_eq!(damage_taken(&sim, FIRST), 64);
}

#[test]
fn mistrust_ignores_a_spell_cast_on_a_foe() {
    let mut sim = scripted();
    use_and_finish(&mut sim, MISTRUST, Target::Unit(FIRST));
    let mut fired = Fired::new(gwsim_data::dsl::Event::OnSpellCast, FIRST);
    fired.other = Some(ADJACENT);
    sim.fire(fired);
    assert_eq!(damage_taken(&sim, FIRST), 0);
}

#[test]
fn unnatural_signet_deals_79_and_53_to_adjacent_only_when_hexed() {
    let mut sim = scripted();
    use_and_finish(&mut sim, UNNATURAL_SIGNET, Target::Unit(FIRST));
    assert_eq!(damage_taken(&sim, FIRST), 79);
    assert_eq!(damage_taken(&sim, ADJACENT), 0, "not hexed or enchanted");

    let mut sim = scripted();
    use_and_finish(&mut sim, MISTRUST, Target::Unit(FIRST));
    use_and_finish(&mut sim, UNNATURAL_SIGNET, Target::Unit(FIRST));
    assert_eq!(damage_taken(&sim, FIRST), 79);
    assert_eq!(damage_taken(&sim, ADJACENT), 53);
    assert_eq!(damage_taken(&sim, NEARBY), 0, "224 away is not adjacent");
}

#[test]
fn cry_of_frustration_needs_a_skill_in_use_then_deals_79_and_59() {
    let mut sim = scripted();
    use_and_finish(&mut sim, CRY_OF_FRUSTRATION, Target::Unit(FIRST));
    assert_eq!(
        damage_taken(&sim, FIRST),
        0,
        "the dummy is not using a skill"
    );

    let mut sim = scripted();
    make_casting(&mut sim, FIRST);
    use_and_finish(&mut sim, CRY_OF_FRUSTRATION, Target::Unit(FIRST));
    assert_eq!(damage_taken(&sim, FIRST), 79);
    assert_eq!(damage_taken(&sim, ADJACENT), 59);
    assert_eq!(damage_taken(&sim, NEARBY), 59);
    assert!(
        sim.units[FIRST.index()].activating().is_none(),
        "the dummy was interrupted"
    );
}

#[test]
fn spiritual_pain_deals_79_and_132_to_summoned_creatures() {
    let mut sim = scripted();
    sim.units[ADJACENT.index()].kind = UnitKind::Minion;
    use_and_finish(&mut sim, SPIRITUAL_PAIN, Target::Unit(FIRST));
    assert_eq!(damage_taken(&sim, FIRST), 79);
    // A minion also decays while the spell is cast, so read the hit itself.
    let hit: i32 = sim
        .log
        .as_ref()
        .unwrap()
        .iter()
        .filter(|e| e.kind == gwsim_engine::log::LogKind::Damage && e.target == Some(ADJACENT.0))
        .filter_map(|e| e.amount)
        .sum();
    assert_eq!(hit, 132);
    assert_eq!(damage_taken(&sim, NEARBY), 0, "not summoned");
}

#[test]
fn power_drain_interrupts_a_spell_and_returns_19_energy() {
    let energy_after = |casting: bool| {
        let mut sim = scripted();
        // Room below the cap for the whole gain.
        sim.units[PLAYER.index()].energy = 10 * gwsim_engine::unit::ENERGY_SCALE;
        if casting {
            make_casting(&mut sim, FIRST);
        }
        use_and_finish(&mut sim, POWER_DRAIN, Target::Unit(FIRST));
        let interrupted = sim.units[FIRST.index()].activating().is_none();
        (sim.units[PLAYER.index()].energy, interrupted)
    };
    let (without, _) = energy_after(false);
    let (with, interrupted) = energy_after(true);
    assert!(interrupted);
    assert_eq!(with - without, 19 * gwsim_engine::unit::ENERGY_SCALE);
}

// -------------------------------------------------------------- behaviour

#[test]
fn arcane_echo_becomes_the_next_spell_for_20_seconds() {
    let mut sim = scripted();
    let echo = skill_in(&sim, ARCANE_ECHO);
    let surge = skill_in(&sim, ENERGY_SURGE);
    use_and_finish(&mut sim, ARCANE_ECHO, Target::Unit(PLAYER));
    sim.step_until(sim.now.plus(1000));
    use_and_finish(&mut sim, ENERGY_SURGE, Target::Unit(FIRST));
    assert_eq!(
        skill_in(&sim, ARCANE_ECHO),
        surge,
        "the slot holds the copy"
    );
    let copied_at = sim.now;

    // The copy is usable at once, and is a real Energy Surge.
    sim.step_until(sim.now.plus(1000));
    use_and_finish(&mut sim, ARCANE_ECHO, Target::Unit(ADJACENT));
    assert_eq!(
        sim.units[ADJACENT.index()].energy_points(),
        DUMMY_ENERGY - 11
    );

    // After 20 seconds the slot is Arcane Echo again.
    sim.step_until(copied_at.plus(20_000 + 100));
    assert_eq!(skill_in(&sim, ARCANE_ECHO), echo);
}

#[test]
fn arcane_echo_ends_when_a_non_spell_is_used() {
    let mut sim = scripted();
    let echo = skill_in(&sim, ARCANE_ECHO);
    use_and_finish(&mut sim, ARCANE_ECHO, Target::Unit(PLAYER));
    assert!(has_effect_from(&sim, echo));
    sim.step_until(sim.now.plus(1000));
    use_and_finish(&mut sim, AIR_OF_SUPERIORITY, Target::Unit(PLAYER));
    assert!(!has_effect_from(&sim, echo));
    assert_eq!(skill_in(&sim, ARCANE_ECHO), echo, "nothing was copied");
}

fn has_effect_from(sim: &Sim, skill: u16) -> bool {
    use gwsim_engine::effects::EffectSource;
    sim.units[PLAYER.index()].effects.iter().any(|e| {
        matches!(e.source, EffectSource::Skill { skill: s, .. } | EffectSource::Handler { skill: s } if s == skill)
    })
}

fn air_of_superiority_outcomes(sim: &Sim) -> usize {
    sim.log
        .as_ref()
        .unwrap()
        .iter()
        .filter(|event| event.detail.starts_with("Air of Superiority:"))
        .count()
}

#[test]
fn air_of_superiority_fires_once_on_an_experience_kill() {
    let mut sim = scripted();
    use_and_finish(&mut sim, AIR_OF_SUPERIORITY, Target::Unit(PLAYER));
    sim.step_until(sim.now.plus(1000));
    sim.units[FIRST.index()].health = HEALTH_SCALE;
    use_and_finish(&mut sim, SPIRITUAL_PAIN, Target::Unit(FIRST));
    assert!(!sim.units[FIRST.index()].alive());
    assert_eq!(air_of_superiority_outcomes(&sim), 1);
}

#[test]
fn air_of_superiority_ignores_a_kill_without_experience() {
    let mut sim = scripted();
    sim.units[FIRST.index()].gives_experience = false;
    use_and_finish(&mut sim, AIR_OF_SUPERIORITY, Target::Unit(PLAYER));
    sim.step_until(sim.now.plus(1000));
    sim.units[FIRST.index()].health = HEALTH_SCALE;
    use_and_finish(&mut sim, SPIRITUAL_PAIN, Target::Unit(FIRST));
    assert!(!sim.units[FIRST.index()].alive());
    assert_eq!(air_of_superiority_outcomes(&sim), 0);
}

#[test]
fn air_of_superiority_does_nothing_when_not_active() {
    let mut sim = scripted();
    sim.units[FIRST.index()].health = HEALTH_SCALE;
    use_and_finish(&mut sim, SPIRITUAL_PAIN, Target::Unit(FIRST));
    assert_eq!(air_of_superiority_outcomes(&sim), 0);
}

// ------------------------------------------------------------ determinism

#[test]
fn the_same_seed_gives_the_same_result() {
    let setup = setup();
    let seeds = SeedList::new(7, 4);
    for seed in seeds.seeds() {
        assert_eq!(setup.run(*seed).digest(), setup.run(*seed).digest());
    }
}

#[test]
fn the_player_wins_against_the_dummies() {
    let setup = setup();
    let result = setup.run(seed());
    assert!(result.won(), "{:?}", result.outcome);
    assert_eq!(result.deaths, 0);
}

fn digests(threads: &str) -> String {
    let output = Command::cargo_bin("gwsim")
        .unwrap()
        .current_dir(data_dir().join(".."))
        .args([
            "evaluate",
            "--party",
            "m0-player",
            "--situation",
            "dummies-hm",
            "--runs",
            "24",
            "--seed",
            "3",
            "--threads",
            threads,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let digests: Vec<String> = json["situations"][0]["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|run| run["digest"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(digests.len(), 24);
    digests.join(",")
}

#[test]
fn evaluate_is_identical_at_1_and_n_threads() {
    assert_eq!(digests("1"), digests("6"));
}
