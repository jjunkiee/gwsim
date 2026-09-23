//! T4.11.2: the M1 performance benchmarks (DESIGN §17.6, D19).
//!
//! Each benchmark runs a fixed seed list with logging off and reports the
//! time per fight. The target is a median under 10 ms for the Kournan patrol
//! in hard mode on one core.

use std::path::PathBuf;

use criterion::{Criterion, criterion_group, criterion_main};
use gwsim_data::core::CoreData;
use gwsim_data::pack::DataPack;
use gwsim_engine::{FightSetup, SeedList};

fn setup(party: &str, situation: &str) -> FightSetup {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let pack = DataPack::from_dir(&dir).expect("data/ loads");
    let core = CoreData::load(dir.join("core")).expect("core data loads");
    let party = pack.data.party(&party.parse().unwrap()).unwrap().clone();
    let situation = pack.data.situations[&situation.parse().unwrap()]
        .value
        .clone();
    FightSetup::new(&pack.data, &core, &party, &situation).expect("the fight prepares")
}

/// The synthetic 8-v-16 stress fight (§17.6): the Kournan patrol with every
/// foe doubled.
fn stress() -> FightSetup {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let mut pack = DataPack::from_dir(&dir).expect("data/ loads");
    let core = CoreData::load(dir.join("core")).expect("core data loads");
    let slug: gwsim_data::Slug = "kournan-patrol".parse().unwrap();
    for group in &mut pack.data.encounters.get_mut(&slug).unwrap().value.groups {
        for foe in &mut group.foes {
            foe.count *= 2;
        }
    }
    let party = pack
        .data
        .party(&"m1-mesmerway".parse().unwrap())
        .unwrap()
        .clone();
    let situation = pack.data.situations[&"kournan-patrol-hm".parse().unwrap()]
        .value
        .clone();
    FightSetup::new(&pack.data, &core, &party, &situation).expect("the stress fight prepares")
}

fn fights(c: &mut Criterion) {
    let seeds = SeedList::new(1, 16);
    let mut group = c.benchmark_group("fight");
    group.sample_size(10);
    for (name, party, situation) in [
        ("kournan-patrol-hm", "m1-mesmerway", "kournan-patrol-hm"),
        (
            "kournan-patrol-hm-chain2",
            "m1-mesmerway",
            "kournan-patrol-hm-chain2",
        ),
        (
            "kournan-healer-heavy-hm",
            "m1-mesmerway",
            "kournan-healer-heavy-hm",
        ),
    ] {
        let setup = setup(party, situation);
        group.bench_function(name, |b| {
            let mut next = 0;
            b.iter(|| {
                let seed = seeds.get(next % seeds.len());
                next += 1;
                setup.run(seed)
            })
        });
    }
    let setup = stress();
    group.bench_function("stress-8v16", |b| {
        let mut next = 0;
        b.iter(|| {
            let seed = seeds.get(next % seeds.len());
            next += 1;
            setup.run(seed)
        })
    });
    group.finish();
}

criterion_group!(benches, fights);
criterion_main!(benches);
