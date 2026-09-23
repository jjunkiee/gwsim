//! T4.11.5: the performance smoke test CI runs (T4.11.1's recommendation).
//!
//! Wall-clock time on a shared CI runner is too noisy to hold to the 20%
//! regression rule of §17.6, so the test guards two things instead:
//!
//! - **the work a fight takes**, counted as events dispatched, which is
//!   deterministic: more than 20% above the recorded baseline fails. A model
//!   change that legitimately adds work updates the baseline in the same
//!   commit, with the reason;
//! - **a generous absolute budget** for the release build, five times the
//!   10 ms target (D19), which catches only gross regressions such as an
//!   accidental quadratic loop.

// The engine bans wall clocks so results cannot depend on them; this test
// times the engine from outside, which is the one place a clock belongs.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

use std::path::PathBuf;
use std::time::Instant;

use gwsim_data::core::CoreData;
use gwsim_data::pack::DataPack;
use gwsim_engine::{FightSetup, SeedList};

/// Mean events per Kournan patrol HM fight over [`SEEDS`], recorded at the
/// commit that added this test. Update it with the reason when the model
/// changes the amount of work on purpose.
const BASELINE_EVENTS: f64 = 1085.0;

/// The allowed rise over the baseline (§17.6).
const TOLERANCE: f64 = 0.20;

/// The release build's budget per fight: five times the D19 target.
const BUDGET_MS: f64 = 50.0;

const SEEDS: usize = 16;

fn setup() -> FightSetup {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let pack = DataPack::from_dir(&dir).unwrap();
    let core = CoreData::load(dir.join("core")).unwrap();
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

/// Mean events per fight, and mean milliseconds per fight.
fn measure(setup: &FightSetup) -> (f64, f64) {
    let seeds = SeedList::new(1, SEEDS);
    let start = Instant::now();
    let mut events = 0u64;
    for seed in seeds.seeds() {
        let mut sim = setup.sim(*seed);
        while sim.outcome.is_none() {
            let next = sim.now.plus(gwsim_engine::time::TICK_MS);
            sim.step_until(next);
        }
        events += sim.events_processed;
    }
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    (events as f64 / SEEDS as f64, elapsed / SEEDS as f64)
}

#[test]
fn a_patrol_fight_does_not_take_more_work_than_it_did() {
    let (events, ms) = measure(&setup());
    println!("{events:.0} events and {ms:.2} ms per fight (baseline {BASELINE_EVENTS})");
    assert!(
        events <= BASELINE_EVENTS * (1.0 + TOLERANCE),
        "{events:.0} events per fight is more than 20% above the baseline {BASELINE_EVENTS}"
    );
    assert!(
        events >= BASELINE_EVENTS * (1.0 - TOLERANCE * 2.0),
        "{events:.0} events per fight is far below the baseline {BASELINE_EVENTS}: \
         if the model changed on purpose, update BASELINE_EVENTS"
    );
}

#[test]
fn a_patrol_fight_fits_its_budget_in_release() {
    if cfg!(debug_assertions) {
        // Timing a debug build says nothing about the release target.
        return;
    }
    let setup = setup();
    // Once to warm caches, once to measure.
    let _ = measure(&setup);
    let (_, ms) = measure(&setup);
    assert!(
        ms <= BUDGET_MS,
        "{ms:.2} ms per fight is over the {BUDGET_MS} ms smoke budget"
    );
}
