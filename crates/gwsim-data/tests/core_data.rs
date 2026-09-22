//! T1.1.5: `data/core/` loads, and its values match the wiki.
//!
//! These tests read the real `data/core/` tree rather than a fixture, so a
//! typo in a shipped data file fails the build rather than waiting for the
//! engine to behave oddly.

use std::path::PathBuf;

use gwsim_data::core::{
    Attribute, Condition, CoreData, DamageClass, HmLevelKind, InherentEffect, Profession, RangeBand,
};

fn core_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/core")
}

fn load() -> CoreData {
    match CoreData::load(core_dir()) {
        Ok(data) => data,
        Err(errors) => panic!("data/core did not load:\n{errors}"),
    }
}

#[test]
fn the_whole_core_tree_loads() {
    let data = load();
    // Completeness is checked by the loader, so reaching here means every
    // profession, attribute, condition and range band has exactly one record.
    assert_eq!(data.professions_file().professions.len(), 10);
    assert_eq!(data.attributes_file().attributes.len(), 42);
    assert_eq!(data.conditions_file().conditions.len(), 10);
    assert_eq!(data.ranges_file().bands.len(), RangeBand::ALL.len());
}

#[test]
fn every_core_file_is_accounted_for() {
    for name in CoreData::FILES {
        let path = core_dir().join(name);
        assert!(path.is_file(), "{} is missing", path.display());
    }
}

/// The wiki's Skill template format profession index, copied out by hand.
///
/// Hard-coded on purpose: if `Profession::template_index` and this list ever
/// disagree, one of them is wrong, and a template code would decode to the
/// wrong profession without anything else noticing.
#[test]
fn profession_template_indices_match_the_wiki_table() {
    let wiki: [(u8, &str); 11] = [
        (0, "None"),
        (1, "Warrior"),
        (2, "Ranger"),
        (3, "Monk"),
        (4, "Necromancer"),
        (5, "Mesmer"),
        (6, "Elementalist"),
        (7, "Assassin"),
        (8, "Ritualist"),
        (9, "Paragon"),
        (10, "Dervish"),
    ];

    for (index, name) in wiki {
        let decoded = Profession::from_template_index(index);
        match name {
            "None" => assert_eq!(decoded, None, "index 0 must mean no profession"),
            _ => {
                let profession = decoded.unwrap_or_else(|| {
                    panic!("index {index} should decode to {name}, but decoded to nothing")
                });
                assert_eq!(
                    format!("{profession:?}"),
                    name,
                    "index {index} decoded to the wrong profession"
                );
                assert_eq!(profession.template_index(), index);
            }
        }
    }
}

/// The wiki's Skill template format attribute index, copied out by hand.
#[test]
fn attribute_template_ids_match_the_wiki_table() {
    let wiki: [(u8, &str); 42] = [
        (0, "FastCasting"),
        (1, "IllusionMagic"),
        (2, "DominationMagic"),
        (3, "InspirationMagic"),
        (4, "BloodMagic"),
        (5, "DeathMagic"),
        (6, "SoulReaping"),
        (7, "Curses"),
        (8, "AirMagic"),
        (9, "EarthMagic"),
        (10, "FireMagic"),
        (11, "WaterMagic"),
        (12, "EnergyStorage"),
        (13, "HealingPrayers"),
        (14, "SmitingPrayers"),
        (15, "ProtectionPrayers"),
        (16, "DivineFavor"),
        (17, "Strength"),
        (18, "AxeMastery"),
        (19, "HammerMastery"),
        (20, "Swordsmanship"),
        (21, "Tactics"),
        (22, "BeastMastery"),
        (23, "Expertise"),
        (24, "WildernessSurvival"),
        (25, "Marksmanship"),
        (29, "DaggerMastery"),
        (30, "DeadlyArts"),
        (31, "ShadowArts"),
        (32, "Communing"),
        (33, "RestorationMagic"),
        (34, "ChannelingMagic"),
        (35, "CriticalStrikes"),
        (36, "SpawningPower"),
        (37, "SpearMastery"),
        (38, "Command"),
        (39, "Motivation"),
        (40, "Leadership"),
        (41, "ScytheMastery"),
        (42, "WindPrayers"),
        (43, "EarthPrayers"),
        (44, "Mysticism"),
    ];

    for (id, name) in wiki {
        let attribute = Attribute::from_template_id(id)
            .unwrap_or_else(|| panic!("id {id} should decode to {name}"));
        assert_eq!(format!("{attribute:?}"), name, "id {id} decoded wrongly");
        assert_eq!(attribute.template_id(), id);
    }

    // The three ids the format skips.
    for id in [26, 27, 28] {
        assert_eq!(
            Attribute::from_template_id(id),
            None,
            "id {id} is unused and must not decode"
        );
    }
}

#[test]
fn ranges_increase_in_the_expected_order() {
    let data = load();

    let expected = [
        (RangeBand::Touch, 144.0),
        (RangeBand::Adjacent, 166.0),
        (RangeBand::Aoe240, 240.0),
        (RangeBand::Nearby, 252.0),
        (RangeBand::InTheArea, 322.0),
        (RangeBand::Spear, 1004.0),
        (RangeBand::Earshot, 1012.0),
        (RangeBand::Casting, 1248.0),
        (RangeBand::Hornbow, 1273.0),
        (RangeBand::Longbow, 1498.0),
        (RangeBand::SpiritRange, 2512.0),
        (RangeBand::NatureRitual, 3000.0),
        (RangeBand::Party, 5020.0),
    ];

    for (band, gwinches) in expected {
        assert_eq!(
            data.gwinches(band),
            gwinches,
            "{band:?} has the wrong range"
        );
    }

    let mut previous = f32::MIN;
    for band in RangeBand::ALL {
        let gwinches = data.gwinches(band);
        assert!(
            gwinches > previous,
            "{band:?} at {gwinches} does not exceed the band before it"
        );
        previous = gwinches;
    }
}

#[test]
fn a_level_twenty_non_boss_foe_is_level_twenty_six_in_hard_mode() {
    // DESIGN 20.2: the Kournan patrol is level 20 in normal mode.
    let data = load();
    assert_eq!(
        data.levels.hm_levels.hm_level(HmLevelKind::NonBoss, 20),
        Some(26)
    );
    // And its health there is 600 base plus 120 hard-mode bonus.
    assert_eq!(data.levels.health_at(26), 600);
    assert_eq!(data.levels.hm_bonus_health(26), 120);
}

#[test]
fn health_and_armor_match_the_wiki_level_table() {
    let data = load();
    for (level, health, armor) in [
        (1u8, 100u32, 3i16),
        (5, 180, 15),
        (10, 280, 30),
        (15, 380, 45),
        (20, 480, 60),
        (26, 600, 78),
        (42, 920, 126),
    ] {
        assert_eq!(data.levels.health_at(level), health, "health at {level}");
        assert_eq!(data.levels.foe_armor_at(level), armor, "armor at {level}");
    }
}

#[test]
fn energy_and_regeneration_match_the_profession_table() {
    let data = load();

    // Maximum energy and regeneration pips at level 20 in basic armor, from
    // the wiki's profession table.
    let expected = [
        (Profession::Warrior, 20u16, 2u8),
        (Profession::Ranger, 25, 3),
        (Profession::Monk, 30, 4),
        (Profession::Necromancer, 30, 4),
        (Profession::Mesmer, 30, 4),
        (Profession::Elementalist, 30, 4),
        (Profession::Assassin, 25, 4),
        (Profession::Ritualist, 30, 4),
        (Profession::Paragon, 30, 2),
        (Profession::Dervish, 25, 4),
    ];

    for (profession, energy, pips) in expected {
        assert_eq!(
            data.max_energy(profession),
            energy,
            "{profession:?} maximum energy"
        );
        assert_eq!(
            data.energy_regen_pips(profession),
            pips,
            "{profession:?} energy regeneration"
        );
        // Foes of every profession regenerate one pip faster than players.
        assert_eq!(data.foe_energy_regen_pips(profession), pips + 1);
    }
}

#[test]
fn base_armor_and_bonuses_match_the_profession_table() {
    let data = load();

    for (profession, armor) in [
        (Profession::Warrior, 80i16),
        (Profession::Ranger, 70),
        (Profession::Monk, 60),
        (Profession::Necromancer, 60),
        (Profession::Mesmer, 60),
        (Profession::Elementalist, 60),
        (Profession::Assassin, 70),
        (Profession::Ritualist, 60),
        (Profession::Paragon, 80),
        (Profession::Dervish, 70),
    ] {
        assert_eq!(
            data.profession(profession).base_armor,
            armor,
            "{profession:?} base armor"
        );
    }

    // Only two professions have an inherent armor bonus, and both name a whole
    // damage family rather than a single type.
    let warrior = data.profession(Profession::Warrior).armor_bonus.unwrap();
    assert_eq!(warrior.against, DamageClass::Physical);
    assert_eq!(warrior.amount, 20);

    let ranger = data.profession(Profession::Ranger).armor_bonus.unwrap();
    assert_eq!(ranger.against, DamageClass::Elemental);
    assert_eq!(ranger.amount, 30);

    for profession in Profession::ALL {
        if matches!(profession, Profession::Warrior | Profession::Ranger) {
            continue;
        }
        assert!(
            data.profession(profession).armor_bonus.is_none(),
            "{profession:?} should have no inherent armor bonus"
        );
    }
}

#[test]
fn only_the_dervish_gets_health_from_armor() {
    let data = load();
    for profession in Profession::ALL {
        let health = data.profession(profession).armor_health_total();
        let expected = if profession == Profession::Dervish {
            25
        } else {
            0
        };
        assert_eq!(health, expected, "{profession:?} armor health");
    }
}

#[test]
fn foe_energy_matches_the_design_table() {
    let data = load();
    for (profession, energy) in [
        (Profession::Warrior, 20u16),
        (Profession::Ranger, 30),
        (Profession::Monk, 40),
        (Profession::Necromancer, 40),
        (Profession::Mesmer, 40),
        (Profession::Elementalist, 40),
        (Profession::Assassin, 30),
        (Profession::Ritualist, 40),
        (Profession::Paragon, 30),
        (Profession::Dervish, 30),
    ] {
        assert_eq!(
            data.profession(profession).foe_energy,
            energy,
            "{profession:?} foe energy"
        );
    }
}

#[test]
fn every_primary_attribute_has_an_inherent_effect() {
    let data = load();
    for profession in Profession::ALL {
        let primary = profession.primary_attribute();
        let record = data.attribute(primary);
        assert!(record.primary, "{primary:?} should be marked primary");
        assert!(
            !record.inherent.is_empty(),
            "{primary:?} is primary but has no inherent effect"
        );
    }
}

#[test]
fn inherent_effects_are_not_limited_to_primary_attributes() {
    // The trap this guards: treating "has an inherent effect" as a synonym for
    // "is primary" would drop weapon damage scaling, the minion cap and pet
    // scaling on the floor.
    let data = load();

    let non_primary_with_effects: Vec<Attribute> = Attribute::ALL
        .into_iter()
        .filter(|attribute| {
            !attribute.is_primary() && !data.attribute(*attribute).inherent.is_empty()
        })
        .collect();

    assert_eq!(
        non_primary_with_effects.len(),
        9,
        "expected the seven weapon masteries plus Death Magic and Beast Mastery, \
         got {non_primary_with_effects:?}"
    );

    for attribute in [
        Attribute::DeathMagic,
        Attribute::BeastMastery,
        Attribute::DaggerMastery,
    ] {
        assert!(
            non_primary_with_effects.contains(&attribute),
            "{attribute:?} should carry an inherent effect"
        );
    }
}

#[test]
fn the_seven_weapon_masteries_all_scale_weapon_damage() {
    let data = load();
    let masteries: Vec<Attribute> = Attribute::ALL
        .into_iter()
        .filter(|attribute| {
            data.attribute(*attribute)
                .inherent
                .contains(&InherentEffect::WeaponMastery)
        })
        .collect();

    assert_eq!(
        masteries,
        vec![
            Attribute::AxeMastery,
            Attribute::HammerMastery,
            Attribute::Swordsmanship,
            Attribute::Marksmanship,
            Attribute::DaggerMastery,
            Attribute::SpearMastery,
            Attribute::ScytheMastery,
        ]
    );

    // Dagger Mastery carries both effects, which is why the field is a list.
    let daggers = &data.attribute(Attribute::DaggerMastery).inherent;
    assert!(daggers.contains(&InherentEffect::WeaponMastery));
    assert!(daggers.contains(&InherentEffect::DoubleStrike));
}

#[test]
fn burning_is_the_only_condition_spirits_suffer() {
    let data = load();
    for condition in Condition::ALL {
        let affects = data.condition(condition).affects_spirits;
        assert_eq!(
            affects,
            condition == Condition::Burning,
            "{condition:?} affects_spirits is wrong"
        );
    }
}

#[test]
fn only_bleeding_disease_and_poison_are_fleshy_only() {
    let data = load();
    let fleshy: Vec<Condition> = Condition::ALL
        .into_iter()
        .filter(|condition| data.condition(*condition).fleshy_only)
        .collect();
    assert_eq!(
        fleshy,
        vec![Condition::Bleeding, Condition::Disease, Condition::Poison]
    );
}

#[test]
fn condition_degeneration_matches_the_wiki() {
    use gwsim_data::core::ConditionEffect;

    let data = load();
    let degeneration = |condition: Condition| -> Option<i8> {
        data.condition(condition)
            .effects
            .iter()
            .find_map(|effect| match effect {
                ConditionEffect::HealthRegeneration(pips) => Some(*pips),
                _ => None,
            })
    };

    assert_eq!(degeneration(Condition::Bleeding), Some(-3));
    assert_eq!(degeneration(Condition::Burning), Some(-7));
    assert_eq!(degeneration(Condition::Poison), Some(-4));
    assert_eq!(degeneration(Condition::Disease), Some(-4));

    // The other six do not touch regeneration at all.
    for condition in [
        Condition::Blind,
        Condition::CrackedArmor,
        Condition::Crippled,
        Condition::Dazed,
        Condition::DeepWound,
        Condition::Weakness,
    ] {
        assert_eq!(degeneration(condition), None, "{condition:?}");
    }
}

#[test]
fn every_condition_does_something() {
    let data = load();
    for condition in Condition::ALL {
        assert!(
            !data.condition(condition).effects.is_empty(),
            "{condition:?} has no effects"
        );
    }
}

#[test]
fn the_asura_track_reaches_full_strength_at_rank_five() {
    use gwsim_data::core::TitleTrack;

    let data = load();
    let asura = data
        .title_track(TitleTrack::Asura)
        .expect("the Asura track should be modelled");

    assert_eq!(asura.max_rank, 10);
    assert_eq!(asura.effective_rank(0), Some(0));
    assert_eq!(asura.effective_rank(1), Some(3));
    assert_eq!(asura.effective_rank(2), Some(6));
    assert_eq!(asura.effective_rank(3), Some(9));
    assert_eq!(asura.effective_rank(4), Some(12));
    assert_eq!(asura.effective_rank(5), Some(15));
    assert_eq!(asura.effective_rank(10), Some(15));
}

#[test]
fn the_hard_mode_recharge_reduction_is_still_unknown() {
    // A-032. The Hard mode page says only "shorter recharges". This test is
    // here so that when someone finally measures it, they are reminded to
    // update the assumption register in the same change.
    let data = load();
    let reduction = &data.modes.hard_mode.recharge_reduction_percent;
    assert_eq!(reduction.known(), None);
    assert_eq!(
        reduction.pending_on().map(|id| id.to_string()),
        Some("A-032".to_owned())
    );
}

#[test]
fn a_missing_directory_reports_every_file() {
    let errors = CoreData::load("does/not/exist").expect_err("should not load");
    assert_eq!(
        errors.0.len(),
        CoreData::FILES.len(),
        "every missing file should be reported, not just the first"
    );
}
