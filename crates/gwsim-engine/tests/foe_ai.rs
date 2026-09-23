//! T4.4.9: foe AI scenarios, one per AI-F item that M1 exercises.

use std::path::PathBuf;

use gwsim_data::core::CoreData;
use gwsim_data::pack::DataPack;
use gwsim_engine::ai::Controller;
use gwsim_engine::geom::Vec2;
use gwsim_engine::pipeline::Order;
use gwsim_engine::sim::Sim;
use gwsim_engine::unit::{Action, HEALTH_SCALE, Target, UnitId};
use gwsim_engine::{FightSetup, SeedList};

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn sim_for(situation: &str, hard: bool) -> Sim {
    let pack = DataPack::from_dir(data_dir()).unwrap();
    let core = CoreData::load(data_dir().join("core")).unwrap();
    let party = pack
        .data
        .party(&"m1-mesmerway".parse().unwrap())
        .unwrap()
        .clone();
    let mut situation = pack.data.situations[&situation.parse().unwrap()]
        .value
        .clone();
    situation.mode.hard_mode = hard;
    let setup = FightSetup::new(&pack.data, &core, &party, &situation).unwrap();
    let mut sim = setup.sim(SeedList::new(1, 1).get(0)).with_log();
    sim.prefight_step = sim.fight.prefight.len();
    sim
}

fn foe(sim: &Sim, name: &str) -> UnitId {
    sim.units
        .iter()
        .find(|u| u.name == format!("Kournan {name}"))
        .unwrap()
        .id
}

fn slot(sim: &Sim, index: usize) -> UnitId {
    sim.units
        .iter()
        .find(|u| u.slot_index == Some(index))
        .unwrap()
        .id
}

#[test]
fn ai_f1_the_group_aggroes_together_at_aggro_range() {
    let mut sim = sim_for("kournan-patrol-hm", true);
    let aggro = sim.fight.tunables.aggro_range;
    let guard = foe(&sim, "Guard");
    let player = slot(&sim, 0);
    // Everyone else far away; the player just outside, then just inside.
    for index in 1..8 {
        let id = slot(&sim, index);
        sim.units[id.index()].pos = Vec2::new(-10_000.0, -10_000.0);
    }
    let nearest = sim
        .units
        .iter()
        .filter(|u| u.foe_index.is_some())
        .map(|u| u.pos)
        .min_by(|a, b| a.y.total_cmp(&b.y))
        .unwrap();
    sim.units[player.index()].pos = nearest - Vec2::new(0.0, aggro + 1.0);
    assert!(!sim.group_aggroed(guard));
    sim.units[player.index()].pos = nearest - Vec2::new(0.0, aggro - 1.0);
    assert!(sim.group_aggroed(guard));
    let priest = foe(&sim, "Priest");
    assert!(sim.group_aggroed(priest), "the whole group engages");
}

#[test]
fn ai_f2_a_foe_picks_the_weaker_target() {
    let mut sim = sim_for("kournan-patrol-hm", false);
    let guard = foe(&sim, "Guard");
    let player = slot(&sim, 0);
    let hero5 = slot(&sim, 5);
    // Both at the same distance and health; hero 5 wears Tormentor's (+10).
    let spot = sim.units[guard.index()].pos;
    for index in 0..8 {
        let id = slot(&sim, index);
        sim.units[id.index()].pos = Vec2::new(-10_000.0, 0.0);
    }
    sim.units[player.index()].pos = spot + Vec2::new(300.0, 0.0);
    sim.units[hero5.index()].pos = spot + Vec2::new(-300.0, 0.0);
    sim.units[player.index()].health = 400 * HEALTH_SCALE;
    sim.units[hero5.index()].health = 400 * HEALTH_SCALE;
    sim.units[player.index()].base_max_health = 465;
    sim.units[hero5.index()].base_max_health = 465;
    assert_eq!(sim.foe_target(guard), Some(player), "60 armor over 70");
}

#[test]
fn ai_f3_the_priest_heals_the_hurt_and_the_seer_interrupts() {
    let mut sim = sim_for("kournan-patrol-hm", true);
    sim.controllers.fill(Controller::Idle);
    let priest = foe(&sim, "Priest");
    let zealot = foe(&sim, "Zealot");
    sim.aggroed = vec![true];
    sim.units[zealot.index()].health = 50 * HEALTH_SCALE;
    let order = gwsim_engine::ai::foe::decide(&mut sim, priest);
    match order {
        Some(Order::UseSkill { target, .. }) if target == Target::Unit(zealot) => {}
        other => panic!(
            "the priest should help the hurt Zealot: {other:?} ({:?})",
            other.and_then(|o| match o {
                Order::UseSkill { slot, .. } =>
                    sim.slot_skill(priest, slot)
                        .map(|k| sim.fight.skills[usize::from(k)].slug.to_string()),
                _ => None,
            })
        ),
    }

    let seer = foe(&sim, "Seer");
    let hero1 = slot(&sim, 1);
    sim.units[hero1.index()].pos = sim.units[seer.index()].pos - Vec2::new(0.0, 900.0);
    sim.units[seer.index()].focus = Some(hero1);
    let panic = (0..8)
        .find(|s| {
            sim.slot_skill(hero1, *s)
                .is_some_and(|k| sim.fight.skills[usize::from(k)].slug.as_str() == "panic")
        })
        .unwrap();
    sim.units[hero1.index()].action = Action::Activating {
        slot: panic,
        target: Target::Unit(seer),
        started: sim.now,
        ends_at: gwsim_engine::time::SimTime(60_000),
        failed: false,
    };
    let order = gwsim_engine::ai::foe::decide(&mut sim, seer);
    let spike = (0..8)
        .find(|s| {
            sim.slot_skill(seer, *s)
                .is_some_and(|k| sim.fight.skills[usize::from(k)].slug.as_str() == "power-spike")
        })
        .unwrap();
    assert_eq!(
        order,
        Some(Order::UseSkill {
            slot: spike,
            target: Target::Unit(hero1)
        }),
        "the Seer interrupts a casting hero"
    );
}

#[test]
fn ai_f5_stationary_foes_never_move_and_kiters_step_back() {
    let mut sim = sim_for("kournan-patrol-hm", true);
    let bowman = foe(&sim, "Bowman");
    sim.units[bowman.index()].kiter = true;
    sim.aggroed = vec![true];
    let player = slot(&sim, 0);
    sim.units[player.index()].pos = sim.units[bowman.index()].pos + Vec2::new(50.0, 0.0);
    sim.units[bowman.index()].focus = Some(player);
    // The player holds a wand (ranged), so give a melee attacker instead.
    let guard_like = slot(&sim, 1);
    sim.units[guard_like.index()]
        .weapon
        .as_mut()
        .unwrap()
        .projectile_speed = None;
    sim.units[guard_like.index()].pos = sim.units[bowman.index()].pos + Vec2::new(60.0, 0.0);
    let order = gwsim_engine::ai::foe::decide(&mut sim, bowman);
    assert!(matches!(order, Some(Order::MoveTo(_))), "{order:?}");

    // A stationary unit is never sent walking.
    let seer = foe(&sim, "Seer");
    sim.units[seer.index()].stationary = true;
    let start = sim.units[seer.index()].pos;
    sim.units[player.index()].pos = start + Vec2::new(0.0, -3000.0);
    for _ in 0..40 {
        let next = sim.now.plus(50);
        sim.step_until(next);
    }
    assert_eq!(sim.units[seer.index()].pos, start);
}

#[test]
fn ai_f6_hard_mode_foes_react_faster() {
    let nm = sim_for("kournan-patrol-hm", false);
    let hm = sim_for("kournan-patrol-hm", true);
    assert!(hm.fight.tunables.foe_reaction_hm_ms < nm.fight.tunables.foe_reaction_ms);
    let mut nm = nm;
    let mut hm = hm;
    let guard_nm = foe(&nm, "Guard");
    let guard_hm = foe(&hm, "Guard");
    assert!(
        gwsim_engine::ai::reaction_delay(&mut hm, guard_hm)
            < gwsim_engine::ai::reaction_delay(&mut nm, guard_nm)
    );
}
