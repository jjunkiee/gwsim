//! Determinism, common random numbers, the combat log, and property tests
//! (T3.7.5, T3.9.5, WP3.5).

use std::path::PathBuf;

use gwsim_data::core::CoreData;
use gwsim_data::pack::DataPack;
use gwsim_engine::harness::{Summary, run_seeds, wilson};
use gwsim_engine::rng::{Purpose, Stream, Streams};
use gwsim_engine::unit::UnitId;
use gwsim_engine::{FightSetup, RunSeed, SeedList, combat, log};
use proptest::prelude::*;

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

// ------------------------------------------------------------ determinism

#[test]
fn a_seed_always_gives_the_same_result() {
    let setup = setup();
    for seed in SeedList::new(11, 8).seeds() {
        let first = setup.run(*seed);
        let second = setup.run(*seed);
        assert_eq!(first, second);
        assert_eq!(first.digest(), second.digest());
    }
}

#[test]
fn logging_does_not_change_the_result() {
    let setup = setup();
    let seed = SeedList::new(5, 1).get(0);
    let mut logged = setup.run_logged(seed);
    assert!(logged.log.as_ref().is_some_and(|l| !l.is_empty()));
    logged.log = None;
    assert_eq!(logged.digest(), setup.run(seed).digest());
}

#[test]
fn parallel_runs_come_back_in_seed_order() {
    let setup = setup();
    let seeds = SeedList::new(3, 32);
    let parallel = run_seeds(&setup, seeds.seeds());
    let serial: Vec<_> = seeds.seeds().iter().map(|s| setup.run(*s)).collect();
    assert_eq!(parallel, serial);
}

#[test]
fn different_seeds_give_different_fights() {
    let setup = setup();
    let digests: std::collections::BTreeSet<u64> = SeedList::new(9, 16)
        .seeds()
        .iter()
        .map(|s| setup.run(*s).digest())
        .collect();
    assert!(digests.len() > 1, "wand damage should vary between seeds");
}

// --------------------------------------------------- common random numbers

#[test]
fn a_units_streams_do_not_depend_on_how_many_units_there_are() {
    let seed = RunSeed(42);
    let mut small = Streams::new(seed, 4);
    let mut large = Streams::new(seed, 12);
    for purpose in Purpose::ALL {
        let a: Vec<u64> = (0..16)
            .map(|_| small.unit(UnitId(2), purpose).next_u64())
            .collect();
        let b: Vec<u64> = (0..16)
            .map(|_| large.unit(UnitId(2), purpose).next_u64())
            .collect();
        assert_eq!(a, b, "{purpose:?}");
    }
}

#[test]
fn drawing_from_one_stream_leaves_the_others_alone() {
    let seed = RunSeed(7);
    let mut busy = Streams::new(seed, 3);
    for _ in 0..100 {
        busy.unit(UnitId(0), Purpose::Hits).next_u64();
    }
    let mut fresh = Streams::new(seed, 3);
    assert_eq!(
        busy.unit(UnitId(1), Purpose::Hits).next_u64(),
        fresh.unit(UnitId(1), Purpose::Hits).next_u64()
    );
    assert_eq!(
        busy.unit(UnitId(0), Purpose::Crits).next_u64(),
        fresh.unit(UnitId(0), Purpose::Crits).next_u64()
    );
}

#[test]
fn seed_lists_share_their_prefixes() {
    let short = SeedList::new(99, 16);
    let long = SeedList::new(99, 256);
    assert_eq!(short.seeds(), &long.seeds()[..16]);
}

// --------------------------------------------------------------------- log

#[test]
fn the_m0_log_opens_as_recorded() {
    let setup = setup();
    let result = setup.run_logged(SeedList::new(1, 1).get(0));
    let events = result.log.unwrap();
    let text = log::to_text(
        &events[..events.len().min(40)],
        |id| setup.unit_name(id),
        |id| setup.skill_name(id),
    );
    insta::assert_snapshot!(text);
}

#[test]
fn the_json_log_has_one_object_per_line() {
    let setup = setup();
    let result = setup.run_logged(SeedList::new(1, 1).get(0));
    let events = result.log.unwrap();
    let json = log::to_json_lines(&events);
    assert_eq!(json.lines().count(), events.len());
    for line in json.lines() {
        let value: serde_json::Value = serde_json::from_str(line).expect("each line is JSON");
        assert!(value.get("t_ms").is_some());
    }
}

// -------------------------------------------------------------- properties

proptest! {
    #[test]
    fn a_range_roll_stays_in_range(seed in any::<u64>(), low in -1000i32..1000, span in 0i32..1000) {
        let mut stream = Stream::derive(RunSeed(seed), Purpose::Hits, None);
        for _ in 0..32 {
            let roll = stream.range_inclusive(low, low + span);
            prop_assert!(roll >= low && roll <= low + span);
        }
    }

    #[test]
    fn a_unit_roll_is_in_zero_to_one(seed in any::<u64>()) {
        let mut stream = Stream::derive(RunSeed(seed), Purpose::Crits, Some(UnitId(3)));
        for _ in 0..32 {
            let roll = stream.unit_f64();
            prop_assert!((0.0..1.0).contains(&roll));
        }
    }

    #[test]
    fn more_armor_never_means_more_damage(base in 1.0f64..500.0, strike in 0.0f64..120.0, armor in 0.0f64..200.0, extra in 0.0f64..100.0) {
        prop_assert!(combat::damage_packet(base, strike, armor + extra) <= combat::damage_packet(base, strike, armor));
    }

    #[test]
    fn a_higher_rank_never_lowers_strike_level(level in 1u8..=30, rank in 0u8..20) {
        prop_assert!(combat::strike_level(rank + 1, level) >= combat::strike_level(rank, level));
    }

    #[test]
    fn armor_rises_with_core_armor(core in 0.0f64..150.0, bonus in -40.0f64..40.0, penetration in 0.0f64..0.5) {
        prop_assert!(combat::armor_level(core + 1.0, &[bonus], penetration, 0.0) >= combat::armor_level(core, &[bonus], penetration, 0.0));
    }

    #[test]
    fn critical_chance_is_a_probability(attacker in 1u8..=30, rank in 0u8..=20, defender in 1u8..=30) {
        let chance = combat::critical_chance(attacker, rank, defender);
        prop_assert!((0.0..=1.0).contains(&chance));
    }

    #[test]
    fn the_wilson_interval_holds_the_observed_rate(n in 1usize..2000, wins_fraction in 0.0f64..=1.0) {
        let wins = ((n as f64) * wins_fraction).floor() as usize;
        let interval = wilson(wins, n);
        let rate = wins as f64 / n as f64;
        prop_assert!(interval.low <= rate + 1e-12 && rate <= interval.high + 1e-12);
        prop_assert!(interval.low >= 0.0 && interval.high <= 1.0);
    }

    #[test]
    fn a_summary_interval_is_centred_on_its_mean(values in proptest::collection::vec(-1000.0f64..1000.0, 1..64)) {
        let summary = Summary::of(&values);
        prop_assert!(summary.min <= summary.mean + 1e-9 && summary.mean <= summary.max + 1e-9);
        prop_assert!(((summary.ci.low + summary.ci.high) / 2.0 - summary.mean).abs() < 1e-9);
    }
}
