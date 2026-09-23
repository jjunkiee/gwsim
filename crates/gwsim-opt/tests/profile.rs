//! T5.7.6 and T5.1.2: pools under account profiles and Melandru's Accord,
//! and the title-rank effect on a PvE-only skill.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use gwsim_data::core::{CoreData, TitleTrack};
use gwsim_data::dataset::DataSet;
use gwsim_data::ids::SkillId;
use gwsim_data::profile::{AccountProfile, Unlocks};
use gwsim_data::provenance::ReviewStatus;
use gwsim_data::source::DirSource;
use gwsim_engine::{FightSetup, SeedList};
use gwsim_opt::pools::{PoolOptions, SlotPools};

fn dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

fn data() -> &'static DataSet {
    static DATA: OnceLock<DataSet> = OnceLock::new();
    DATA.get_or_init(|| DataSet::load(&DirSource::new(dir())).unwrap())
}

const ENERGY_SURGE: SkillId = SkillId(39);
const AIR_OF_SUPERIORITY: SkillId = SkillId(2416);

#[test]
fn a_hero_pool_has_no_pve_only_or_numbers_only_skills() {
    let party = data().party(&"m1-mesmerway".parse().unwrap()).unwrap();
    let hero = &party.slots[1];
    let pools = SlotPools::for_slot(hero, data(), &PoolOptions::default());
    for id in &pools.skills {
        let skill = data().skill_by_id(*id).unwrap();
        assert!(!skill.pve_only, "{} is PvE-only", skill.name);
        assert_ne!(
            skill.provenance.review,
            ReviewStatus::NumbersOnly,
            "{}",
            skill.name
        );
    }
    assert_eq!(
        pools.primaries,
        vec![hero.build.primary],
        "a hero keeps its primary"
    );
    let player = SlotPools::for_slot(&party.slots[0], data(), &PoolOptions::default());
    assert!(
        player.skills.contains(&AIR_OF_SUPERIORITY),
        "a human may take PvE-only skills"
    );
    assert_eq!(player.primaries.len(), 10);
}

#[test]
fn a_profile_missing_energy_surge_never_offers_it() {
    let party = data().party(&"m1-mesmerway".parse().unwrap()).unwrap();
    let every: BTreeSet<SkillId> = data().skills.values().map(|e| e.value.id).collect();
    let profile = AccountProfile {
        unlocked_skills: Unlocks::Only(
            every.into_iter().filter(|id| *id != ENERGY_SURGE).collect(),
        ),
        ..AccountProfile::default()
    };
    let options = PoolOptions {
        profile,
        ..PoolOptions::default()
    };
    for slot in &party.slots {
        let pools = SlotPools::for_slot(slot, data(), &options);
        assert!(!pools.skills.contains(&ENERGY_SURGE), "{}", slot.name);
    }
}

#[test]
fn reviewed_only_empties_a_pool_of_drafts() {
    let party = data().party(&"m1-mesmerway".parse().unwrap()).unwrap();
    let options = PoolOptions {
        reviewed_only: true,
        ..PoolOptions::default()
    };
    let pools = SlotPools::for_slot(&party.slots[0], data(), &options);
    for id in &pools.skills {
        assert_eq!(
            data().skill_by_id(*id).unwrap().provenance.review,
            ReviewStatus::Reviewed
        );
    }
}

#[test]
fn melandrus_accord_removes_unlearned_skills() {
    let party = data().party(&"m1-mesmerway".parse().unwrap()).unwrap();
    let learned: BTreeSet<SkillId> = party.slots[0]
        .build
        .skills
        .iter()
        .flatten()
        .copied()
        .collect();
    let profile = AccountProfile {
        learned_skills: Some(BTreeMap::from([("player".to_owned(), learned.clone())])),
        ..AccountProfile::default()
    };
    let with_accord = SlotPools::for_slot(
        &party.slots[0],
        data(),
        &PoolOptions {
            profile: profile.clone(),
            accord: true,
            ..PoolOptions::default()
        },
    );
    assert!(with_accord.skills.iter().all(|id| learned.contains(id)));
    let without = SlotPools::for_slot(
        &party.slots[0],
        data(),
        &PoolOptions {
            profile,
            accord: false,
            ..PoolOptions::default()
        },
    );
    assert!(without.skills.len() > with_accord.skills.len());
}

#[test]
fn title_ranks_change_a_pve_only_skills_value() {
    // Air of Superiority's energy and heal scale on Asura rank (A-020): with
    // the title at 0 the player's maximum energy gain from it is smaller.
    let core = CoreData::load(dir().join("core")).unwrap();
    let party = data().party(&"m1-mesmerway".parse().unwrap()).unwrap();
    let situation = data().situations[&"dummies-hm".parse().unwrap()]
        .value
        .clone();
    let setup = FightSetup::new(data(), &core, party, &situation).unwrap();
    assert_eq!(
        setup.fight.title_rank(TitleTrack::Asura),
        10,
        "no profile: the maximum"
    );
    let low = FightSetup::new(data(), &core, party, &situation)
        .unwrap()
        .with_title_ranks(BTreeMap::from([(TitleTrack::Asura, 0)]));
    assert_eq!(low.fight.title_rank(TitleTrack::Asura), 0);
    // The two setups are otherwise identical, so any difference in a run is
    // the title's.
    let seed = SeedList::new(1, 1).get(0);
    let (a, b) = (setup.run(seed), low.run(seed));
    assert_ne!(a.digest(), b.digest(), "the Asura rank changed nothing");
}
