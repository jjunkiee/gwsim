//! Scenario tests for the loop, pipeline, effects, damage, regeneration and
//! movement (T3.1.6, T3.2.7, T3.4.11, T3.5.9, T3.6.10).
//!
//! Each starts from the M0 fight with every controller idle, so the test
//! gives every order. Units: 0 is the player (Mesmer, Fast Casting 11); 1 to
//! 3 are training dummies 1,000 gwinches north.

use std::path::PathBuf;

use gwsim_data::core::{Condition, CoreData};
use gwsim_data::pack::DataPack;
use gwsim_engine::ai::Controller;
use gwsim_engine::damage::HealKind;
use gwsim_engine::geom::Vec2;
use gwsim_engine::log::LogKind;
use gwsim_engine::pipeline::{InterruptScope, Invalid, Order};
use gwsim_engine::sim::Sim;
use gwsim_engine::time::SimTime;
use gwsim_engine::unit::{Action, ENERGY_SCALE, HEALTH_SCALE, Target, UnitId};
use gwsim_engine::{FightSetup, Outcome, SeedList};

const PLAYER: UnitId = UnitId(0);
const FIRST: UnitId = UnitId(1);
const ENERGY_SURGE: u8 = 1;
const SPIRITUAL_PAIN: u8 = 5;

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn setup() -> FightSetup {
    let pack = DataPack::from_dir(data_dir()).expect("data/ loads");
    let core = CoreData::load(data_dir().join("core")).expect("core data loads");
    let party = pack
        .data
        .party(&"m0-player".parse().unwrap())
        .unwrap()
        .clone();
    let situation = pack.data.situations[&"dummies-hm".parse().unwrap()]
        .value
        .clone();
    FightSetup::new(&pack.data, &core, &party, &situation).expect("the M0 fight prepares")
}

fn scripted() -> Sim {
    let mut sim = setup().sim(SeedList::new(1, 1).get(0)).with_log();
    sim.controllers.fill(Controller::Idle);
    sim
}

fn skill_in(sim: &Sim, slot: u8) -> u16 {
    sim.units[PLAYER.index()].bar[usize::from(slot)]
        .as_ref()
        .unwrap()
        .skill
}

fn advance(sim: &mut Sim, ms: u32) {
    let until = sim.now.plus(ms);
    sim.step_until(until);
}

fn free(sim: &Sim) -> bool {
    matches!(sim.units[PLAYER.index()].action, Action::Idle)
}

// -------------------------------------------------------------------- loop

#[test]
fn a_fight_nobody_can_win_times_out_at_180_seconds() {
    let setup = setup();
    let mut sim = setup.sim(SeedList::new(1, 1).get(0));
    for dummy in &mut sim.units[1..] {
        dummy.base_max_health = 1_000_000;
        dummy.health = 1_000_000 * HEALTH_SCALE;
    }
    let result = sim.run();
    assert_eq!(result.outcome, Outcome::Timeout);
    assert_eq!(result.ended_ms, 180_000);
    assert_eq!(result.clear_time_ms, None);
}

#[test]
fn the_fight_ends_when_the_party_dies() {
    let mut sim = scripted();
    sim.kill(PLAYER, None);
    assert_eq!(sim.outcome.map(|(o, _)| o), Some(Outcome::Wipe));
}

#[test]
fn the_fight_ends_when_the_last_foe_dies() {
    let mut sim = scripted();
    for index in 1..4 {
        sim.kill(UnitId(index), Some(PLAYER));
    }
    assert_eq!(sim.outcome.map(|(o, _)| o), Some(Outcome::Win));
}

// ---------------------------------------------------------------- pipeline

#[test]
fn a_skill_needs_its_energy() {
    let mut sim = scripted();
    sim.units[PLAYER.index()].energy = 2 * ENERGY_SCALE;
    assert_eq!(
        sim.can_use(PLAYER, ENERGY_SURGE, Target::Unit(FIRST)),
        Err(Invalid::NoEnergy)
    );
}

#[test]
fn a_used_skill_recharges_and_the_energy_is_spent() {
    let mut sim = scripted();
    let before = sim.units[PLAYER.index()].energy;
    sim.use_skill(PLAYER, ENERGY_SURGE, Target::Unit(FIRST))
        .unwrap();
    assert_eq!(
        before - sim.units[PLAYER.index()].energy,
        5 * ENERGY_SCALE,
        "paid at the start"
    );
    advance(&mut sim, 1203 + 750 + 50);
    assert!(free(&sim));
    assert_eq!(
        sim.can_use(PLAYER, ENERGY_SURGE, Target::Unit(FIRST)),
        Err(Invalid::Recharging)
    );
}

#[test]
fn a_busy_caster_cannot_start_another_skill() {
    let mut sim = scripted();
    sim.use_skill(PLAYER, ENERGY_SURGE, Target::Unit(FIRST))
        .unwrap();
    assert_eq!(
        sim.can_use(PLAYER, SPIRITUAL_PAIN, Target::Unit(FIRST)),
        Err(Invalid::Busy)
    );
}

#[test]
fn the_aftercast_holds_the_caster_for_three_quarters_of_a_second() {
    let mut sim = scripted();
    sim.use_skill(PLAYER, SPIRITUAL_PAIN, Target::Unit(FIRST))
        .unwrap();
    advance(&mut sim, 602 + 700);
    assert!(!free(&sim), "still in aftercast");
    advance(&mut sim, 100);
    assert!(free(&sim));
}

#[test]
fn a_target_out_of_range_is_approached_then_hit() {
    let mut sim = scripted();
    sim.units[FIRST.index()].pos = Vec2::new(0.0, 3000.0);
    sim.order(
        PLAYER,
        Order::UseSkill {
            slot: ENERGY_SURGE,
            target: Target::Unit(FIRST),
        },
    );
    advance(&mut sim, 12_000);
    assert_eq!(sim.units[FIRST.index()].energy_points(), 40 - 11);
    let walked = sim.units[PLAYER.index()].pos.y;
    assert!(
        walked >= 3000.0 - 1248.0 - 1.0,
        "stopped at casting range, at {walked}"
    );
    assert!(
        walked < 3000.0 - 1000.0,
        "but no closer than needed, at {walked}"
    );
}

#[test]
fn an_interrupted_spell_does_nothing_and_recharges() {
    let mut sim = scripted();
    sim.use_skill(PLAYER, ENERGY_SURGE, Target::Unit(FIRST))
        .unwrap();
    advance(&mut sim, 500);
    assert!(sim.interrupt(PLAYER, FIRST, InterruptScope::Spell));
    let now = sim.now;
    advance(&mut sim, 2000);
    assert_eq!(
        sim.units[FIRST.index()].energy_points(),
        40,
        "the spell never landed"
    );
    let ready = sim.units[PLAYER.index()].bar[usize::from(ENERGY_SURGE)]
        .as_ref()
        .unwrap()
        .ready_at;
    assert_eq!(ready, now.plus(10_000));
}

#[test]
fn dazed_doubles_a_spells_activation() {
    let mut sim = scripted();
    let surge = skill_in(&sim, ENERGY_SURGE);
    assert!(sim.apply_condition(FIRST, PLAYER, Condition::Dazed, 10_000));
    assert_eq!(sim.activation_ms(PLAYER, surge), 2406);
}

#[test]
fn a_knockdown_interrupts_and_holds_the_unit() {
    let mut sim = scripted();
    sim.use_skill(PLAYER, ENERGY_SURGE, Target::Unit(FIRST))
        .unwrap();
    advance(&mut sim, 300);
    sim.knock_down(PLAYER, 2000);
    assert_eq!(
        sim.can_use(PLAYER, SPIRITUAL_PAIN, Target::Unit(FIRST)),
        Err(Invalid::KnockedDown)
    );
    advance(&mut sim, 2050);
    assert!(free(&sim));
    assert_eq!(
        sim.units[FIRST.index()].energy_points(),
        40,
        "the spell was interrupted"
    );
}

// ----------------------------------------------------------------- effects

#[test]
fn bleeding_drains_three_pips_of_health() {
    let mut sim = scripted();
    assert!(sim.apply_condition(PLAYER, FIRST, Condition::Bleeding, 10_000));
    advance(&mut sim, 10_000);
    // −3 pips is 6 health a second.
    let lost = 480 - sim.units[FIRST.index()].health_points();
    assert!((58..=60).contains(&lost), "{lost}");
    assert!(
        !sim.has_condition(FIRST, Condition::Bleeding),
        "it has expired"
    );
}

#[test]
fn a_reapplied_condition_keeps_the_longer_duration() {
    let mut sim = scripted();
    sim.apply_condition(PLAYER, FIRST, Condition::Bleeding, 10_000);
    sim.apply_condition(PLAYER, FIRST, Condition::Bleeding, 3_000);
    let bleeding: Vec<_> = sim.units[FIRST.index()]
        .effects
        .iter()
        .filter(|e| e.kind == gwsim_data::dsl::EffectKind::Condition)
        .collect();
    assert_eq!(bleeding.len(), 1);
    assert_eq!(bleeding[0].ends_at, Some(SimTime(10_000)));
}

#[test]
fn death_clears_effects() {
    let mut sim = scripted();
    sim.apply_condition(PLAYER, FIRST, Condition::Bleeding, 10_000);
    sim.kill(FIRST, Some(PLAYER));
    assert!(sim.units[FIRST.index()].effects.is_empty());
}

// ----------------------------------------------------- damage and healing

#[test]
fn healing_restores_health_up_to_the_maximum() {
    let mut sim = scripted();
    sim.health_loss(PLAYER, 100.0, Some(FIRST));
    assert_eq!(sim.units[PLAYER.index()].health_points(), 465 - 100);
    sim.heal(PLAYER, PLAYER, 60.0, None, HealKind::Heal);
    assert_eq!(sim.units[PLAYER.index()].health_points(), 465 - 40);
    sim.heal(PLAYER, PLAYER, 500.0, None, HealKind::Heal);
    assert_eq!(sim.units[PLAYER.index()].health_points(), 465);
}

// ------------------------------------------------------------ regeneration

#[test]
fn four_energy_pips_return_four_energy_in_three_seconds() {
    let mut sim = scripted();
    sim.units[PLAYER.index()].energy = 10 * ENERGY_SCALE;
    advance(&mut sim, 3000);
    assert_eq!(sim.units[PLAYER.index()].energy_points(), 14);
}

// ---------------------------------------------------------------- movement

#[test]
fn a_walk_covers_288_gwinches_a_second() {
    let mut sim = scripted();
    sim.order(PLAYER, Order::MoveTo(Vec2::new(0.0, -288.0)));
    advance(&mut sim, 500);
    let halfway = sim.units[PLAYER.index()].pos.y;
    assert!((halfway + 144.0).abs() < 15.0, "{halfway}");
    advance(&mut sim, 600);
    let arrived = sim.units[PLAYER.index()].pos.y;
    assert!((arrived + 288.0).abs() < 1.0, "{arrived}");
}

#[test]
fn auto_attacks_repeat_at_the_weapon_interval() {
    let mut sim = scripted();
    sim.order(PLAYER, Order::Attack(FIRST));
    advance(&mut sim, 12_000);
    let hits: Vec<u32> = sim
        .log
        .as_ref()
        .unwrap()
        .iter()
        .filter(|e| e.kind == LogKind::Damage && e.source == Some(PLAYER.0) && e.skill.is_none())
        .map(|e| e.t_ms)
        .collect();
    assert!(hits.len() >= 5, "{hits:?}");
    for pair in hits.windows(2) {
        assert_eq!(pair[1] - pair[0], 1750, "{hits:?}");
    }
}
