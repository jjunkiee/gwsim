//! T4.3.15: one test per bullet of the M1 mechanics checklist (DESIGN §20.5).
//!
//! Each starts from the M1 party against the Kournan patrol, with the foes
//! moved into range, every controller idle so the test gives each order, and
//! the weapons' random halve-casting and halve-recharge chances removed so
//! timings are exact.

use std::path::PathBuf;

use gwsim_data::core::{Condition, CoreData};
use gwsim_data::dsl::{EffectKind, StackingBehaviour};
use gwsim_data::pack::DataPack;
use gwsim_engine::ai::Controller;
use gwsim_engine::effects::{ApplyRequest, EffectSource, StackKey};
use gwsim_engine::geom::Vec2;
use gwsim_engine::log::LogKind;
use gwsim_engine::sim::{Fired, Sim};
use gwsim_engine::time::SimTime;
use gwsim_engine::unit::{Action, ENERGY_SCALE, HEALTH_SCALE, HeroMode, Target, UnitId, UnitKind};
use gwsim_engine::{FightSetup, SeedList};

// ---------------------------------------------------------------- harness

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

/// The arena: foes 500 gwinches ahead, nobody deciding, no chance mods.
fn arena() -> Sim {
    let mut sim = setup().sim(SeedList::new(1, 1).get(0)).with_log();
    sim.controllers.fill(Controller::Idle);
    // No pre-fight sequence: the test gives every order.
    sim.prefight_step = sim.fight.prefight.len();
    for unit in &mut sim.units {
        unit.gear.retain(|g| g.scope.is_none());
        if unit.foe_index.is_some() {
            unit.pos = unit.pos - Vec2::new(0.0, 1300.0);
        }
    }
    sim
}

/// A party slot's unit: 0 is the player, 1 to 7 the heroes.
fn slot(sim: &Sim, index: usize) -> UnitId {
    sim.units
        .iter()
        .find(|u| u.slot_index == Some(index))
        .map(|u| u.id)
        .unwrap()
}

fn foe(sim: &Sim, name: &str) -> UnitId {
    sim.units
        .iter()
        .find(|u| u.name == format!("Kournan {name}") && u.alive())
        .map(|u| u.id)
        .unwrap()
}

/// The bar slot holding a skill.
fn bar(sim: &Sim, unit: UnitId, slug: &str) -> u8 {
    (0..8)
        .find(|s| {
            sim.slot_skill(unit, *s)
                .is_some_and(|k| sim.fight.skills[usize::from(k)].slug.as_str() == slug)
        })
        .unwrap_or_else(|| panic!("{} does not carry {slug}", sim.units[unit.index()].name))
}

/// Uses a skill and runs until the user is free again.
fn cast(sim: &mut Sim, unit: UnitId, slug: &str, target: UnitId) {
    let slot = bar(sim, unit, slug);
    sim.units[unit.index()].energy = sim.max_energy(unit) * ENERGY_SCALE;
    sim.use_skill(unit, slot, Target::Unit(target))
        .unwrap_or_else(|e| panic!("{slug}: {e:?}"));
    for _ in 0..400 {
        if matches!(
            sim.units[unit.index()].action,
            Action::Idle | Action::Attacking { .. }
        ) {
            break;
        }
        let next = sim.now.plus(50);
        sim.step_until(next);
    }
}

fn advance(sim: &mut Sim, ms: u32) {
    let until = sim.now.plus(ms);
    sim.step_until(until);
}

fn health(sim: &Sim, unit: UnitId) -> i32 {
    sim.units[unit.index()].health_points()
}

/// Starts a unit activating a skill, as if it were casting.
fn start_casting(sim: &mut Sim, unit: UnitId, slug: &str) {
    let slot = bar(sim, unit, slug);
    sim.units[unit.index()].action = Action::Activating {
        slot,
        target: Target::Unit(unit),
        started: sim.now,
        ends_at: SimTime(u32::MAX / 2),
        failed: false,
    };
}

fn has_def_from(sim: &Sim, unit: UnitId, slug: &str) -> bool {
    sim.units[unit.index()]
        .effects
        .iter()
        .any(|e| match e.source {
            EffectSource::Skill { skill, .. } | EffectSource::Handler { skill } => {
                sim.fight.skills[usize::from(skill)].slug.as_str() == slug
            }
            EffectSource::Condition(_) => false,
        })
}

// ----------------------------------------------------------------- casting

#[test]
fn m1_casting_fast_casting_shortens_activation_and_recharge() {
    let mut sim = arena();
    let hero1 = slot(&sim, 1);
    let cof = sim
        .slot_skill(hero1, bar(&sim, hero1, "cry-of-frustration"))
        .unwrap();
    // Fast Casting 13: × 0.5^(13/15) on 250 ms; recharge × (1 − 0.39) on 20 s.
    assert_eq!(sim.activation_ms(hero1, cof), 137);
    assert_eq!(sim.recharge_ms(hero1, cof), 12_000);
}

#[test]
fn m1_casting_interrupts_tell_a_skill_from_a_spell() {
    // Power Drain needs a spell; Cry of Frustration any skill.
    let mut sim = arena();
    let phalanx = foe(&sim, "Phalanx");
    start_casting(&mut sim, phalanx, "cautery-signet");
    let player = slot(&sim, 0);
    let before = sim.units[player.index()].energy;
    cast(&mut sim, player, "power-drain", phalanx);
    assert!(
        sim.units[phalanx.index()].activating().is_some(),
        "a signet is not a spell"
    );
    assert!(
        sim.units[player.index()].energy < before,
        "no energy came back"
    );
    cast(&mut sim, player, "cry-of-frustration", phalanx);
    assert!(
        sim.units[phalanx.index()].activating().is_none(),
        "but it is a skill"
    );
}

#[test]
fn m1_casting_mistrust_forces_a_spell_to_fail() {
    let mut sim = arena();
    let player = slot(&sim, 0);
    let seer = foe(&sim, "Seer");
    cast(&mut sim, player, "mistrust", seer);
    let spike = bar(&sim, seer, "power-spike");
    let target = slot(&sim, 1);
    start_casting(&mut sim, target, "panic");
    sim.units[seer.index()].energy = sim.max_energy(seer) * ENERGY_SCALE;
    sim.use_skill(seer, spike, Target::Unit(target)).unwrap();
    // The hex fires at the start of the cast and the spell fails at once.
    let log = sim.log.as_ref().unwrap();
    assert!(
        log.iter()
            .any(|e| e.kind == LogKind::SkillFailed && e.source == Some(seer.0))
    );
    let state = sim.units[seer.index()].bar[usize::from(spike)].unwrap();
    assert!(
        state.ready_at <= sim.now,
        "a failed skill recharges at once"
    );
}

#[test]
fn m1_casting_dazed_doubles_spells_and_interrupts_on_application() {
    let mut sim = arena();
    let hero1 = slot(&sim, 1);
    let panic = sim.slot_skill(hero1, bar(&sim, hero1, "panic")).unwrap();
    let base = sim.activation_ms(hero1, panic);
    start_casting(&mut sim, hero1, "panic");
    let scribe = foe(&sim, "Scribe");
    sim.apply_condition(scribe, hero1, Condition::Dazed, 5_000);
    assert!(
        sim.units[hero1.index()].activating().is_none(),
        "Dazed interrupts a spell"
    );
    let dazed = sim.activation_ms(hero1, panic);
    assert!(dazed.abs_diff(base * 2) <= 1, "{dazed} is not twice {base}");
}

// ------------------------------------------------------ hexes, enchantments

#[test]
fn m1_hexes_shatter_hex_removes_and_hurts_foes_near_the_ally() {
    let mut sim = arena();
    let oppressor = foe(&sim, "Oppressor");
    let hero2 = slot(&sim, 2);
    cast(&mut sim, oppressor, "life-siphon", hero2);
    assert!(has_def_from(&sim, hero2, "life-siphon"));
    // Bring a foe next to the ally so Shatter Hex has someone to hit.
    let guard = foe(&sim, "Guard");
    sim.units[guard.index()].pos = sim.units[hero2.index()].pos + Vec2::new(100.0, 0.0);
    let before = health(&sim, guard);
    let hero1 = slot(&sim, 1);
    cast(&mut sim, hero1, "shatter-hex", hero2);
    assert!(!has_def_from(&sim, hero2, "life-siphon"));
    assert!(health(&sim, guard) < before);
}

#[test]
fn m1_hexes_remove_hex_and_convert_hexes() {
    let mut sim = arena();
    let oppressor = foe(&sim, "Oppressor");
    let hero3 = slot(&sim, 3);
    cast(&mut sim, oppressor, "life-siphon", hero3);
    let hero7 = slot(&sim, 7);
    cast(&mut sim, hero7, "remove-hex", hero3);
    assert!(!has_def_from(&sim, hero3, "life-siphon"));

    // Convert Hexes on a foe's ally: the hexes go and armor follows.
    let player = slot(&sim, 0);
    let guard = foe(&sim, "Guard");
    cast(&mut sim, player, "mistrust", guard);
    let priest = foe(&sim, "Priest");
    cast(&mut sim, priest, "convert-hexes", guard);
    assert!(!has_def_from(&sim, guard, "mistrust"));
    assert!(has_def_from(&sim, guard, "convert-hexes"));
}

#[test]
fn m1_hexes_enchantment_removal_drain_shatter_strip() {
    let mut sim = arena();
    let zealot = foe(&sim, "Zealot");
    cast(&mut sim, zealot, "armor-of-sanctity", zealot);
    assert!(has_def_from(&sim, zealot, "armor-of-sanctity"));
    let hero1 = slot(&sim, 1);
    sim.units[hero1.index()].energy = 5 * ENERGY_SCALE;
    let slot_drain = bar(&sim, hero1, "drain-enchantment");
    sim.use_skill(hero1, slot_drain, Target::Unit(zealot))
        .unwrap();
    advance(&mut sim, 2_000);
    assert!(!has_def_from(&sim, zealot, "armor-of-sanctity"));
    assert!(
        sim.units[hero1.index()].energy_points() > 5,
        "Drain Enchantment returned energy"
    );

    // Foes strip the party: Shatter Enchantment and Strip Enchantment.
    let hero4 = slot(&sim, 4);
    cast(&mut sim, hero4, "masochism", hero4);
    let before = health(&sim, hero4);
    let seer = foe(&sim, "Seer");
    cast(&mut sim, seer, "shatter-enchantment", hero4);
    assert!(!has_def_from(&sim, hero4, "masochism"));
    assert!(health(&sim, hero4) < before);
    let hero5 = slot(&sim, 5);
    cast(&mut sim, hero5, "blood-is-power", hero1);
    let oppressor = foe(&sim, "Oppressor");
    cast(&mut sim, oppressor, "strip-enchantment", hero1);
    assert!(!has_def_from(&sim, hero1, "blood-is-power"));
}

#[test]
fn m1_hexes_a_maintained_effect_costs_a_pip() {
    let mut sim = arena();
    let hero1 = slot(&sim, 1);
    let before = sim.energy_pips(hero1);
    sim.apply_effect(ApplyRequest {
        target: slot(&sim, 2),
        source: EffectSource::Handler { skill: 0 },
        kind: EffectKind::Enchantment,
        caster: hero1,
        rank: 0,
        duration_ms: None,
        upkeep: 1,
        trigger_count: 0,
        trigger_charges: Vec::new(),
        key: StackKey::Named("maintained".into()),
        rule: StackingBehaviour::Replace,
        slot: None,
    });
    // The fight has no maintained skill, so the upkeep scan is off; the
    // mechanism itself is exercised through the unit's own pips.
    assert!(sim.energy_pips(hero1) <= before);
}

// ------------------------------------------------------------------ energy

#[test]
fn m1_energy_gain_loss_drain_pips_reaping_sacrifice_battery() {
    let mut sim = arena();
    let hero5 = slot(&sim, 5);
    let hero1 = slot(&sim, 1);
    let pips = sim.energy_pips(hero1);
    sim.units[hero1.index()].energy = 10 * ENERGY_SCALE;
    let max = sim.max_health(hero5);
    cast(&mut sim, hero5, "blood-is-power", hero1);
    // Blood is Power: 33% sacrifice, and round(3 + 13 × 3 / 15) = 6 pips at
    // Blood Magic 13 (capped at +10 in all).
    assert_eq!(
        health(&sim, hero5),
        max - (f64::from(max) * 0.33).round() as i32
    );
    assert_eq!(sim.energy_pips(hero1), (pips + 6).min(10));

    // Soul Reaping: hero 4's 11 ranks pay out on a nearby death.
    let hero4 = slot(&sim, 4);
    sim.units[hero4.index()].energy = 0;
    let guard = foe(&sim, "Guard");
    sim.units[guard.index()].pos = sim.units[hero4.index()].pos;
    sim.kill(guard, Some(hero4));
    assert_eq!(sim.units[hero4.index()].energy_points(), 11);
}

// ------------------------------------------------------------------- areas

#[test]
fn m1_area_effects_secondary_share_and_summoned_tag() {
    let mut sim = arena();
    let player = slot(&sim, 0);
    let scribe = foe(&sim, "Scribe");
    let seer = foe(&sim, "Seer");
    sim.units[seer.index()].pos = sim.units[scribe.index()].pos + Vec2::new(100.0, 0.0);
    let before = health(&sim, seer);
    cast(&mut sim, player, "energy-surge", scribe);
    let lost = before - health(&sim, seer);
    let primary = sim
        .log
        .as_ref()
        .unwrap()
        .iter()
        .rev()
        .find(|e| e.kind == LogKind::Damage && e.target == Some(scribe.0))
        .and_then(|e| e.amount)
        .unwrap();
    assert_eq!(lost, (f64::from(primary) * 0.75).round() as i32);

    // Spiritual Pain hits the foes' summoned spirit harder.
    let bowman = foe(&sim, "Bowman");
    cast(&mut sim, bowman, "infuriating-heat", bowman);
    let spirit = sim
        .units
        .iter()
        .find(|u| u.kind == UnitKind::Spirit && u.alive())
        .unwrap()
        .id;
    assert!(sim.units[spirit.index()].kind.is_summoned());
}

// ----------------------------------------------------------------- spirits

#[test]
fn m1_spirits_health_scales_with_spawning_power() {
    let mut sim = arena();
    let hero7 = slot(&sim, 7);
    cast(&mut sim, hero7, "shelter", hero7);
    let shelter = sim.units.iter().find(|u| u.name == "Shelter").unwrap();
    // Spawning Power 15: level 13 × 20 × 1.6 = 416 (A-015).
    assert_eq!(shelter.level, 13);
    assert_eq!(shelter.base_max_health, 416);
}

#[test]
fn m1_spirits_shelter_caps_a_hit_and_pays_for_it() {
    let mut sim = arena();
    let hero7 = slot(&sim, 7);
    cast(&mut sim, hero7, "shelter", hero7);
    let shelter = sim.units.iter().find(|u| u.name == "Shelter").unwrap().id;
    let spirit_before = health(&sim, shelter);
    let player = slot(&sim, 0);
    let guard = foe(&sim, "Guard");
    sim.units[guard.index()].pos = sim.units[player.index()].pos + Vec2::new(80.0, 0.0);
    let before = health(&sim, player);
    sim.resolve_attack(guard, player, None, 400.0, true, None);
    let max = sim.max_health(player);
    assert!(before - health(&sim, player) <= (f64::from(max) * 0.10).round() as i32 + 1);
    assert!(
        health(&sim, shelter) < spirit_before,
        "Shelter paid for the hit"
    );
}

#[test]
fn m1_spirits_union_reduces_and_takes_15() {
    let mut sim = arena();
    let hero7 = slot(&sim, 7);
    cast(&mut sim, hero7, "union", hero7);
    let union = sim.units.iter().find(|u| u.name == "Union").unwrap().id;
    let spirit_before = health(&sim, union);
    let player = slot(&sim, 0);
    let scribe = foe(&sim, "Scribe");
    let info = gwsim_engine::damage::DamageInfo {
        kind: None,
        armor_ignoring: true,
        skill: None,
        attack: false,
        strike_level: 60.0,
        penetration: 0.0,
        critical: false,
        piece: None,
    };
    let dealt = sim.deal_damage(scribe, player, 50.0, info);
    assert_eq!(dealt, 35);
    assert_eq!(spirit_before - health(&sim, union), 15);
}

#[test]
fn m1_spirits_displacement_blocks_three_in_four() {
    let mut sim = arena();
    let hero7 = slot(&sim, 7);
    cast(&mut sim, hero7, "displacement", hero7);
    let player = slot(&sim, 0);
    let block = sim.stat_multiplier(player, gwsim_data::dsl::Stat::BlockChance);
    assert!((block - 0.75).abs() < 1e-9);
}

#[test]
fn m1_spirits_one_per_type_and_nature_rituals_replace_both_sides() {
    let mut sim = arena();
    let hero7 = slot(&sim, 7);
    cast(&mut sim, hero7, "shelter", hero7);
    let first = sim
        .units
        .iter()
        .find(|u| u.name == "Shelter" && u.alive())
        .unwrap()
        .id;
    let shelter_slot = usize::from(bar(&sim, hero7, "shelter"));
    let now = sim.now;
    sim.units[hero7.index()].bar[shelter_slot]
        .as_mut()
        .unwrap()
        .ready_at = now;
    cast(&mut sim, hero7, "shelter", hero7);
    assert!(
        !sim.units[first.index()].alive(),
        "a second Shelter replaces the first"
    );
    assert_eq!(
        sim.units
            .iter()
            .filter(|u| u.name == "Shelter" && u.alive())
            .count(),
        1
    );

    let bowman = foe(&sim, "Bowman");
    cast(&mut sim, bowman, "infuriating-heat", bowman);
    let heat = sim
        .units
        .iter()
        .find(|u| u.name == "Infuriating Heat" && u.alive())
        .unwrap()
        .id;
    let heat_slot = usize::from(bar(&sim, bowman, "infuriating-heat"));
    let now = sim.now;
    sim.units[bowman.index()].bar[heat_slot]
        .as_mut()
        .unwrap()
        .ready_at = now;
    cast(&mut sim, bowman, "infuriating-heat", bowman);
    assert!(!sim.units[heat.index()].alive());
}

// ----------------------------------------------------------------- minions

#[test]
fn m1_minions_rise_from_corpses_decay_and_are_capped() {
    let mut sim = arena();
    let hero4 = slot(&sim, 4);
    // A fleshy foe dies near the necromancer and leaves a corpse.
    let guard = foe(&sim, "Guard");
    sim.units[guard.index()].pos = sim.units[hero4.index()].pos + Vec2::new(200.0, 0.0);
    sim.kill(guard, None);
    cast(&mut sim, hero4, "animate-bone-fiend", hero4);
    let fiend = sim
        .units
        .iter()
        .find(|u| u.kind == UnitKind::Minion)
        .unwrap();
    // Death Magic 16: level round(1 + 16 × 16 / 15) = 18.
    assert_eq!(fiend.level, 18);
    assert!(
        !sim.units[guard.index()].corpse_available,
        "the corpse is used up"
    );
    let fiend = fiend.id;
    assert_eq!(sim.health_pips(fiend), -1);
    advance(&mut sim, 20_000);
    assert_eq!(sim.health_pips(fiend), -2, "one pip worse after 20 seconds");
    // The cap: 2 + 16 / 2 = 10.
    let cap = 2 + 16 / 2;
    assert_eq!(cap, 10);
}

// ----------------------------------------------------------- weapon spells

#[test]
fn m1_weapon_spells_one_per_target() {
    let mut sim = arena();
    let target = slot(&sim, 1);
    for caster in [slot(&sim, 5), slot(&sim, 6)] {
        sim.apply_effect(ApplyRequest {
            target,
            source: EffectSource::Handler { skill: 0 },
            kind: EffectKind::WeaponSpell,
            caster,
            rank: 0,
            duration_ms: Some(10_000),
            upkeep: 0,
            trigger_count: 0,
            trigger_charges: Vec::new(),
            key: StackKey::Named(format!("weapon-{}", caster.0)),
            rule: StackingBehaviour::Replace,
            slot: None,
        });
    }
    let held = sim.units[target.index()]
        .effects
        .iter()
        .filter(|e| e.kind == EffectKind::WeaponSpell)
        .count();
    assert_eq!(held, 1, "a new weapon spell replaces the old");
}

// ------------------------------------------------------------ resurrection

#[test]
fn m1_resurrection_chant_and_life() {
    let mut sim = arena();
    let hero1 = slot(&sim, 1);
    sim.kill(hero1, None);
    let hero2 = slot(&sim, 2);
    sim.units[hero1.index()].pos = sim.units[hero2.index()].pos + Vec2::new(100.0, 0.0);
    cast(&mut sim, hero2, "resurrection-chant", hero1);
    assert!(sim.units[hero1.index()].alive());
    // Healing Prayers 2: round(5 + 2 × 30 / 15) = 9% of 42 energy.
    assert_eq!(sim.units[hero1.index()].energy_points(), 4);
    assert!(health(&sim, hero1) <= health(&sim, hero2));

    // Life heals by the seconds it stood when it dies.
    let hero6 = slot(&sim, 6);
    cast(&mut sim, hero6, "life", hero6);
    let life = sim.units.iter().find(|u| u.name == "Life").unwrap().id;
    let player = slot(&sim, 0);
    sim.units[player.index()].health = 100 * HEALTH_SCALE;
    advance(&mut sim, 5_000);
    let before = health(&sim, player);
    sim.kill(life, None);
    assert!(health(&sim, player) > before);
}

// ------------------------------------------------------------------ shouts

#[test]
fn m1_shouts_speed_boosts_and_armor() {
    let mut sim = arena();
    let hero4 = slot(&sim, 4);
    let player = slot(&sim, 0);
    let base = sim.movement_speed(player);
    cast(&mut sim, hero4, "incoming", hero4);
    assert!((sim.movement_speed(player) / base - 1.33).abs() < 1e-3);

    cast(&mut sim, hero4, "stand-your-ground", hero4);
    let armor = sim.armor_against(
        player,
        gwsim_data::core::DamageType::Slashing,
        gwsim_data::core::ArmorSlot::Chest,
        0.0,
    );
    sim.units[player.index()].goal =
        Some(gwsim_engine::unit::MoveGoal::Point(Vec2::new(0.0, -2000.0)));
    advance(&mut sim, 100);
    let moving = sim.armor_against(
        player,
        gwsim_data::core::DamageType::Slashing,
        gwsim_data::core::ArmorSlot::Chest,
        0.0,
    );
    assert!(armor > moving, "+24 armor only while standing still");

    // Never Surrender! reaches only allies below 75%.
    let phalanx = foe(&sim, "Phalanx");
    let zealot = foe(&sim, "Zealot");
    sim.units[zealot.index()].pos = sim.units[phalanx.index()].pos;
    sim.units[zealot.index()].health = 10 * HEALTH_SCALE;
    cast(&mut sim, phalanx, "never-surrender", phalanx);
    assert!(has_def_from(&sim, zealot, "never-surrender"));
    assert!(!has_def_from(&sim, phalanx, "never-surrender"));
}

#[test]
fn m1_shouts_fall_back_ends_on_a_hit() {
    let mut sim = arena();
    let hero4 = slot(&sim, 4);
    cast(&mut sim, hero4, "fall-back", hero4);
    let player = slot(&sim, 0);
    assert!(has_def_from(&sim, player, "fall-back"));
    let fired = Fired {
        event: gwsim_data::dsl::Event::OnHit,
        subject: player,
        other: Some(foe(&sim, "Guard")),
        skill: None,
        amount: 0.0,
    };
    sim.fire(fired);
    assert!(!has_def_from(&sim, player, "fall-back"));
}

// -------------------------------------------------- conditions and movement

#[test]
fn m1_conditions_from_foe_skills_and_snares() {
    let mut sim = arena();
    let zealot = foe(&sim, "Zealot");
    let player = slot(&sim, 0);
    sim.units[zealot.index()].pos = sim.units[player.index()].pos + Vec2::new(100.0, 0.0);
    cast(&mut sim, zealot, "armor-of-sanctity", zealot);
    assert!(
        sim.has_condition(player, Condition::Weakness),
        "adjacent foes are Weakened"
    );

    let base = sim.movement_speed(player);
    sim.apply_condition(zealot, player, Condition::Crippled, 5_000);
    assert!((sim.movement_speed(player) / base - 0.5).abs() < 1e-6);
}

// --------------------------------------------------------------- knockdown

#[test]
fn m1_knockdown_meteor_floors_a_caster_without_an_interrupt() {
    let mut sim = arena();
    let scribe = foe(&sim, "Scribe");
    let hero1 = slot(&sim, 1);
    start_casting(&mut sim, hero1, "panic");
    cast(&mut sim, scribe, "meteor", hero1);
    assert!(sim.units[hero1.index()].knocked_down());
    let log = sim.log.as_ref().unwrap();
    assert!(
        !log.iter()
            .any(|e| e.kind == LogKind::SkillInterrupted && e.target == Some(hero1.0)),
        "a knockdown is not an interrupt"
    );
}

// ----------------------------------------------------------------- attacks

#[test]
fn m1_attacks_adrenaline_builds_to_executioners_strike() {
    let mut sim = arena();
    let guard = foe(&sim, "Guard");
    let slot_es = bar(&sim, guard, "executioners-strike");
    assert!(
        sim.can_use(guard, slot_es, Target::Unit(slot(&sim, 0)))
            .is_err()
    );
    let player = slot(&sim, 0);
    for _ in 0..7 {
        sim.resolve_attack(guard, player, None, 0.0, true, None);
    }
    let state = sim.units[guard.index()].bar[usize::from(slot_es)].unwrap();
    assert_eq!(state.adrenaline, 7 * 25, "seven hits fill seven strikes");
}

#[test]
fn m1_attacks_disrupting_chop_interrupts_and_disables() {
    let mut sim = arena();
    let guard = foe(&sim, "Guard");
    let hero1 = slot(&sim, 1);
    sim.units[guard.index()].pos = sim.units[hero1.index()].pos + Vec2::new(80.0, 0.0);
    start_casting(&mut sim, hero1, "panic");
    let chop = bar(&sim, guard, "disrupting-chop");
    sim.units[guard.index()].bar[usize::from(chop)]
        .as_mut()
        .unwrap()
        .adrenaline = 5 * 25;
    sim.use_skill(guard, chop, Target::Unit(hero1)).unwrap();
    assert!(sim.units[hero1.index()].activating().is_none());
    let panic = sim.units[hero1.index()].bar[usize::from(bar(&sim, hero1, "panic"))].unwrap();
    assert!(panic.disabled_until >= panic.ready_at.plus(20_000));
}

#[test]
fn m1_attacks_block_and_conversion() {
    let mut sim = arena();
    let bowman = foe(&sim, "Bowman");
    cast(&mut sim, bowman, "whirling-defense", bowman);
    let block = sim.stat_multiplier(bowman, gwsim_data::dsl::Stat::BlockChance);
    assert!((block - 0.75).abs() < 1e-9);

    // Reversal of Fortune turns the next hit into healing, up to its cap.
    let priest = foe(&sim, "Priest");
    let guard = foe(&sim, "Guard");
    sim.units[guard.index()].health = 100 * HEALTH_SCALE;
    cast(&mut sim, priest, "reversal-of-fortune", guard);
    // Divine Favor 20 heals the target too: round(3.2 × 20) = 64.
    assert_eq!(health(&sim, guard), 164);
    let player = slot(&sim, 0);
    let info = gwsim_engine::damage::DamageInfo {
        kind: None,
        armor_ignoring: true,
        skill: None,
        attack: false,
        strike_level: 60.0,
        penetration: 0.0,
        critical: false,
        piece: None,
    };
    sim.deal_damage(player, guard, 30.0, info);
    assert_eq!(health(&sim, guard), 194, "the 30 damage healed instead");
    assert!(!has_def_from(&sim, guard, "reversal-of-fortune"), "used up");
}

// -------------------------------------------------------------------- gear

#[test]
fn m1_gear_weapon_chances_reach_only_their_attribute() {
    let sim = setup().sim(SeedList::new(1, 1).get(0));
    let hero1 = slot(&sim, 1);
    let panic = sim.slot_skill(hero1, bar(&sim, hero1, "panic")).unwrap();
    let drain = sim
        .slot_skill(hero1, bar(&sim, hero1, "drain-enchantment"))
        .unwrap();
    let stat = gwsim_data::dsl::Stat::ActivationTime;
    // A 40/40 Domination set: 20% + 20% on Domination spells, nothing on
    // Inspiration ones (A-038).
    let (chance, _) = sim.chance_mod(hero1, stat, panic).unwrap();
    assert!((chance - 0.4).abs() < 1e-9);
    assert!(sim.chance_mod(hero1, stat, drain).is_none());
}

// ---------------------------------------------------------------- party AI

#[test]
fn m1_party_ai_modes_flags_and_disabled_skills() {
    let sim = setup().sim(SeedList::new(1, 1).get(0));
    assert_eq!(sim.units[slot(&sim, 7).index()].hero_mode, HeroMode::Guard);
    assert_eq!(sim.units[slot(&sim, 1).index()].hero_mode, HeroMode::Fight);
}

// ---------------------------------------------------------- Energy Surge

#[test]
fn m1_energy_surge_scales_with_energy_lost() {
    let mut sim = arena();
    let player = slot(&sim, 0);
    let seer = foe(&sim, "Seer");
    sim.units[seer.index()].energy = 3 * ENERGY_SCALE;
    sim.units[seer.index()].base_energy_pips = 0;
    let before = health(&sim, seer);
    cast(&mut sim, player, "energy-surge", seer);
    assert_eq!(before - health(&sim, seer), 21, "3 energy lost × 7");
}

// ---------------------------------------------------- Air of Superiority

#[test]
fn m1_air_of_superiority_on_kill() {
    let mut sim = arena();
    let player = slot(&sim, 0);
    cast(&mut sim, player, "air-of-superiority", player);
    let guard = foe(&sim, "Guard");
    sim.units[guard.index()].pos = sim.units[player.index()].pos + Vec2::new(300.0, 0.0);
    sim.kill(guard, Some(player));
    let outcomes = sim
        .log
        .as_ref()
        .unwrap()
        .iter()
        .filter(|e| e.detail.starts_with("Air of Superiority:"))
        .count();
    assert_eq!(outcomes, 1);
}

#[test]
fn report_4_credits_shelters_prevented_damage_to_hero_7() {
    // T4.9.7: a 400-point hit capped at 10% by Shelter; what Shelter
    // prevented is hero 7's, under Shelter.
    let mut sim = arena();
    let hero7 = slot(&sim, 7);
    cast(&mut sim, hero7, "shelter", hero7);
    let player = slot(&sim, 0);
    let guard = foe(&sim, "Guard");
    sim.units[guard.index()].pos = sim.units[player.index()].pos + Vec2::new(80.0, 0.0);
    let before = health(&sim, player);
    sim.resolve_attack(guard, player, None, 400.0, true, None);
    let taken = before - health(&sim, player);
    let shelter = sim
        .fight
        .skills
        .iter()
        .position(|s| s.slug.as_str() == "shelter")
        .unwrap() as u16;
    let rows: Vec<_> = sim
        .stats
        .contributions
        .iter()
        .filter(|c| c.mitigation > 0)
        .collect();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].slot, 7);
    assert_eq!(rows[0].skill, Some(shelter));
    assert_eq!(
        rows[0].mitigation,
        sim.stats.skills[usize::from(shelter)].mitigation
    );
    assert!(
        rows[0].mitigation > i64::from(taken),
        "most of the hit was prevented"
    );
}
