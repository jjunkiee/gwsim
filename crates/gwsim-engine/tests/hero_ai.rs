//! T4.5.8: hero, minion and spirit AI scenarios, and a smoke test of the M1
//! party against the patrol.

use std::collections::BTreeMap;
use std::path::PathBuf;

use gwsim_data::core::CoreData;
use gwsim_data::pack::DataPack;
use gwsim_engine::ai::Controller;
use gwsim_engine::geom::Vec2;
use gwsim_engine::log::LogKind;
use gwsim_engine::pipeline::Order;
use gwsim_engine::sim::Sim;
use gwsim_engine::unit::{Action, HeroMode, Target, UnitId, UnitKind};
use gwsim_engine::{FightSetup, SeedList};

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn setup() -> FightSetup {
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
    FightSetup::new(&pack.data, &core, &party, &situation).unwrap()
}

/// Foes 500 ahead and noticed; nobody decides unless a test asks.
fn arena() -> Sim {
    let mut sim = setup().sim(SeedList::new(1, 1).get(0)).with_log();
    sim.prefight_step = sim.fight.prefight.len();
    sim.controllers.fill(Controller::Idle);
    for unit in &mut sim.units {
        if unit.foe_index.is_some() {
            unit.pos = unit.pos - Vec2::new(0.0, 1300.0);
        }
    }
    sim.aggroed = vec![true];
    sim
}

fn slot(sim: &Sim, index: usize) -> UnitId {
    sim.units
        .iter()
        .find(|u| u.slot_index == Some(index))
        .unwrap()
        .id
}

fn foe(sim: &Sim, name: &str) -> UnitId {
    sim.units
        .iter()
        .find(|u| u.name == format!("Kournan {name}"))
        .unwrap()
        .id
}

fn slug_of(sim: &Sim, unit: UnitId, slot: u8) -> String {
    sim.slot_skill(unit, slot)
        .map(|k| sim.fight.skills[usize::from(k)].slug.to_string())
        .unwrap_or_default()
}

#[test]
fn ai_h1_locked_then_called_then_the_players_target() {
    let mut sim = arena();
    let hero1 = slot(&sim, 1);
    let (guard, seer, scribe) = (foe(&sim, "Guard"), foe(&sim, "Seer"), foe(&sim, "Scribe"));
    let player = slot(&sim, 0);
    sim.units[player.index()].focus = Some(scribe);
    sim.called_target = None;
    assert_eq!(
        sim.hero_focus(hero1, true),
        Some(scribe),
        "the player's target"
    );
    sim.called_target = Some(seer);
    assert_eq!(
        sim.hero_focus(hero1, true),
        Some(seer),
        "the called target beats it"
    );
    sim.units[hero1.index()].locked_target = Some(guard);
    assert_eq!(
        sim.hero_focus(hero1, true),
        Some(guard),
        "a lock beats both"
    );
}

#[test]
fn ai_h2_guard_holds_and_avoid_combat_never_attacks() {
    let mut sim = arena();
    let hero1 = slot(&sim, 1);
    // Avoid Combat: no offensive skill, no attack.
    sim.units[hero1.index()].hero_mode = HeroMode::AvoidCombat;
    for _ in 0..5 {
        let order = gwsim_engine::ai::hero::decide(&mut sim, hero1);
        if let Some(Order::UseSkill { slot, .. }) = order {
            let slug = slug_of(&sim, hero1, slot);
            panic!("an Avoid Combat hero used {slug}");
        }
        assert!(!matches!(order, Some(Order::Attack(_))));
    }
    // Guard: no walking to a far target.
    sim.units[hero1.index()].hero_mode = HeroMode::Guard;
    let far = foe(&sim, "Priest");
    sim.called_target = Some(far);
    sim.units[far.index()].pos = sim.units[hero1.index()].pos + Vec2::new(0.0, 5_000.0);
    if let Some(Order::UseSkill {
        target: Target::Unit(t),
        ..
    }) = gwsim_engine::ai::hero::decide(&mut sim, hero1)
    {
        assert_ne!(t, far, "a guarding hero does not walk to a far foe");
    }
}

#[test]
fn ai_h3_heroes_do_not_coordinate_a_removal() {
    // Two heroes see the same hex and both choose to remove it.
    let mut sim = arena();
    let hero2 = slot(&sim, 2);
    let oppressor = foe(&sim, "Oppressor");
    let life_siphon = (0..8)
        .find(|s| slug_of(&sim, oppressor, *s) == "life-siphon")
        .unwrap();
    sim.use_skill(oppressor, life_siphon, Target::Unit(hero2))
        .unwrap();
    sim.step_until(sim.now.plus(1_500));
    let hero1 = slot(&sim, 1);
    let hero7 = slot(&sim, 7);
    let pick = |sim: &mut Sim, unit: UnitId| match gwsim_engine::ai::hero::decide(sim, unit) {
        Some(Order::UseSkill { slot, target }) => Some((slug_of(sim, unit, slot), target)),
        _ => None,
    };
    let first = pick(&mut sim, hero1);
    let second = pick(&mut sim, hero7);
    assert_eq!(first, Some(("shatter-hex".to_owned(), Target::Unit(hero2))));
    assert_eq!(second, Some(("remove-hex".to_owned(), Target::Unit(hero2))));
}

#[test]
fn ai_h4_heroes_interrupt_inside_their_reaction_delay() {
    let mut sim = arena();
    let hero1 = slot(&sim, 1);
    sim.controllers[hero1.index()] = Controller::Hero;
    let scribe = foe(&sim, "Scribe");
    let player = slot(&sim, 0);
    sim.units[player.index()].focus = Some(scribe);
    sim.units[hero1.index()].focus = Some(scribe);
    sim.units[hero1.index()].next_decision_at = sim.now.plus(10_000);
    // The Scribe starts a Fireball; the hero answers at the next tick.
    let fireball = (0..8)
        .find(|s| slug_of(&sim, scribe, *s) == "fireball")
        .unwrap();
    sim.use_skill(scribe, fireball, Target::Unit(player))
        .unwrap();
    sim.step_until(sim.now.plus(50));
    let used: Vec<String> = sim
        .log
        .as_ref()
        .unwrap()
        .iter()
        .filter(|e| e.kind == LogKind::SkillStarted && e.source == Some(hero1.0))
        .filter_map(|e| e.skill)
        .map(|id| {
            sim.fight
                .skills
                .iter()
                .find(|s| s.skill.id.get() == id)
                .unwrap()
                .slug
                .to_string()
        })
        .collect();
    assert!(
        used.iter()
            .any(|s| s == "cry-of-frustration" || s == "power-drain"),
        "{used:?}"
    );
}

#[test]
fn ai_h5_heroes_do_not_reapply_an_effect_that_is_up() {
    let mut sim = arena();
    let hero4 = slot(&sim, 4);
    let target = foe(&sim, "Guard");
    let player = slot(&sim, 0);
    sim.units[player.index()].focus = Some(target);
    let bile = (0..8)
        .find(|s| slug_of(&sim, hero4, *s) == "putrid-bile")
        .unwrap();
    sim.use_skill(hero4, bile, Target::Unit(target)).unwrap();
    sim.step_until(sim.now.plus(2_000));
    sim.units[hero4.index()].bar[usize::from(bile)]
        .as_mut()
        .unwrap()
        .ready_at = sim.now;
    for _ in 0..3 {
        if let Some(Order::UseSkill { slot, target: t }) =
            gwsim_engine::ai::hero::decide(&mut sim, hero4)
        {
            assert!(
                !(slot == bile && t == Target::Unit(target)),
                "Putrid Bile again on a hexed foe"
            );
        }
    }
}

#[test]
fn ai_h6_batteries_go_on_drained_casters_and_shouts_wait_for_combat() {
    let mut sim = arena();
    let hero5 = slot(&sim, 5);
    let hero1 = slot(&sim, 1);
    // Full energy everywhere: no battery.
    let battery = (0..8)
        .find(|s| slug_of(&sim, hero5, *s) == "blood-is-power")
        .unwrap();
    if let Some(Order::UseSkill { slot, .. }) = gwsim_engine::ai::hero::decide(&mut sim, hero5) {
        assert_ne!(slot, battery, "no one is low on energy");
    }
    // A caster at a fifth of its energy gets one.
    sim.units[hero1.index()].energy = sim.max_energy(hero1) / 5 * gwsim_engine::unit::ENERGY_SCALE;
    let mut given = false;
    for _ in 0..8 {
        if let Some(Order::UseSkill { slot, target }) =
            gwsim_engine::ai::hero::decide(&mut sim, hero5)
            && slot == battery
        {
            assert_eq!(target, Target::Unit(hero1));
            given = true;
            break;
        }
        sim.step_until(sim.now.plus(500));
    }
    assert!(given, "Blood is Power goes on the drained caster");

    // Out of combat, a shout that is not a speed boost is held.
    let mut calm = setup().sim(SeedList::new(1, 1).get(0));
    calm.prefight_step = calm.fight.prefight.len();
    let hero4 = slot(&calm, 4);
    if let Some(Order::UseSkill { slot, .. }) = gwsim_engine::ai::hero::decide(&mut calm, hero4) {
        assert_ne!(slug_of(&calm, hero4, slot), "stand-your-ground");
    }
}

#[test]
fn ai_h8_heroes_cast_no_spirits_before_the_fight() {
    let mut calm = setup().sim(SeedList::new(1, 1).get(0));
    calm.prefight_step = calm.fight.prefight.len();
    let hero7 = slot(&calm, 7);
    for _ in 0..5 {
        if let Some(Order::UseSkill { slot, .. }) = gwsim_engine::ai::hero::decide(&mut calm, hero7)
        {
            let slug = slug_of(&calm, hero7, slot);
            assert!(
                !["shelter", "union", "displacement"].contains(&slug.as_str()),
                "{slug}"
            );
        }
    }
}

#[test]
fn ai_h9_disabled_skills_are_never_used() {
    let mut sim = arena();
    let hero1 = slot(&sim, 1);
    sim.units[hero1.index()].disabled_slots = 0xFF;
    assert!(!matches!(
        gwsim_engine::ai::hero::decide(&mut sim, hero1),
        Some(Order::UseSkill { .. })
    ));
}

#[test]
fn minions_attack_their_masters_target_and_spirits_stay_put() {
    let mut sim = arena();
    let hero4 = slot(&sim, 4);
    let guard = foe(&sim, "Guard");
    let seer = foe(&sim, "Seer");
    sim.units[guard.index()].pos = sim.units[hero4.index()].pos + Vec2::new(150.0, 0.0);
    sim.kill(guard, None);
    let fiend_slot = (0..8)
        .find(|s| slug_of(&sim, hero4, *s) == "animate-bone-fiend")
        .unwrap();
    sim.use_skill(hero4, fiend_slot, Target::Unit(hero4))
        .unwrap();
    sim.step_until(sim.now.plus(1_500));
    let fiend = sim
        .units
        .iter()
        .find(|u| u.kind == UnitKind::Minion)
        .unwrap()
        .id;
    sim.units[hero4.index()].attack_target = Some(seer);
    sim.units[fiend.index()].attack_target = None;
    let order = sim.decide_by_rule(fiend, Controller::Minion);
    assert_eq!(order, Some(Order::Attack(seer)));

    let hero7 = slot(&sim, 7);
    let shelter_slot = (0..8)
        .find(|s| slug_of(&sim, hero7, *s) == "shelter")
        .unwrap();
    sim.use_skill(hero7, shelter_slot, Target::Unit(hero7))
        .unwrap();
    sim.step_until(sim.now.plus(1_500));
    let shelter = sim.units.iter().find(|u| u.name == "Shelter").unwrap();
    let (id, start) = (shelter.id, shelter.pos);
    sim.controllers[id.index()] = Controller::Spirit;
    sim.step_until(sim.now.plus(5_000));
    assert_eq!(sim.units[id.index()].pos, start);
}

#[test]
fn the_m1_party_uses_its_bars_against_the_patrol() {
    let setup = setup();
    let mut used: BTreeMap<usize, std::collections::BTreeSet<u16>> = BTreeMap::new();
    for seed in SeedList::new(3, 8).seeds() {
        let result = setup.run_logged(*seed);
        let units = setup.units();
        for event in result.log.unwrap() {
            if event.kind != LogKind::SkillStarted {
                continue;
            }
            let Some(source) = event.source else { continue };
            if let Some(slot) = units.get(usize::from(source)).and_then(|u| u.slot_index) {
                used.entry(slot).or_default().insert(event.skill.unwrap());
            }
        }
    }
    for slot in 1..8 {
        let count = used.get(&slot).map_or(0, |s| s.len());
        assert!(count >= 4, "hero {slot} used only {count} of its skills");
    }
    let _ = Action::Idle;
}
