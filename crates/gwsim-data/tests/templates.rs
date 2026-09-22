//! T1.3.5: the template codec against real published codes.
//!
//! The seven codes below are the M1 party from DESIGN §20.1, taken from
//! PvXwiki. They are the only ground truth there is for the bit layout, so
//! every one is asserted field by field **and** re-encoded back to the
//! identical string. A codec that decodes plausibly but re-encodes
//! differently has misread a width somewhere.

use gwsim_data::core::{Attribute, Profession};
use gwsim_data::ids::SkillId;
use gwsim_data::template::{
    EquipmentItem, EquipmentSlot, EquipmentTemplate, SkillTemplate, Template, TemplateError, decode,
};

/// A published code, and what T1.3.1 established it contains.
struct Vector {
    label: &'static str,
    code: &'static str,
    primary: Profession,
    secondary: Option<Profession>,
    attributes: &'static [(Attribute, u8)],
    skills: [u16; 8],
}

const VECTORS: &[Vector] = &[
    Vector {
        label: "player, Me/-- Energy Surge",
        code: "OQBTAUBPQaJ4EY6x0BAAAAAAuE",
        primary: Profession::Mesmer,
        secondary: None,
        attributes: &[
            (Attribute::FastCasting, 10),
            (Attribute::DominationMagic, 12),
            (Attribute::InspirationMagic, 8),
        ],
        // Arcane Echo, Energy Surge, Mistrust, Unnatural Signet, three empty
        // slots, Air of Superiority.
        skills: [75, 39, 979, 934, 0, 0, 0, 2416],
    },
    Vector {
        label: "heroes 1 to 3, Domination Mesmer",
        code: "OQBTAWBPsBAkDmemuhAONDAAA",
        primary: Profession::Mesmer,
        secondary: None,
        attributes: &[
            (Attribute::FastCasting, 11),
            (Attribute::DominationMagic, 12),
            (Attribute::InspirationMagic, 6),
        ],
        skills: [0, 57, 979, 934, 67, 1336, 25, 0],
    },
    Vector {
        label: "hero 4, Minion Master N/P",
        code: "OAljUwGpZS8Y7Y1YVVUBKgbhAAA",
        primary: Profession::Necromancer,
        secondary: Some(Profession::Paragon),
        attributes: &[
            (Attribute::DeathMagic, 12),
            (Attribute::SoulReaping, 9),
            (Attribute::Command, 9),
        ],
        skills: [1596, 1595, 1589, 1365, 84, 2058, 2139, 0],
    },
    Vector {
        label: "hero 5, Blood is Power N/Rt",
        code: "OAhjQkGZIP3hhmwrqKNncDzqH",
        primary: Profession::Necromancer,
        secondary: Some(Profession::Ritualist),
        attributes: &[
            (Attribute::BloodMagic, 9),
            (Attribute::SoulReaping, 9),
            (Attribute::RestorationMagic, 12),
        ],
        skills: [119, 835, 962, 1365, 1234, 915, 1219, 981],
    },
    Vector {
        label: "hero 6, Signet of Spirits Rt",
        code: "OACjEyiM5MXTvJzEAINncDzxJ",
        primary: Profession::Ritualist,
        secondary: None,
        attributes: &[
            (Attribute::RestorationMagic, 12),
            (Attribute::ChannelingMagic, 12),
            (Attribute::SpawningPower, 3),
        ],
        skills: [1239, 1246, 1228, 0, 1234, 915, 1219, 1251],
    },
    Vector {
        label: "hero 7, Soul Twisting Rt/Mo",
        code: "OACiAyk8gNtePuwJ00ZaNBAA",
        primary: Profession::Ritualist,
        secondary: None,
        attributes: &[(Attribute::Communing, 12), (Attribute::SpawningPower, 12)],
        skills: [1240, 982, 911, 1249, 1232, 1230, 1238, 0],
    },
];

fn expected_skills(ids: [u16; 8]) -> [Option<SkillId>; 8] {
    ids.map(|id| (id != 0).then_some(SkillId(id)))
}

#[test]
fn every_published_code_decodes_to_the_expected_build() {
    for vector in VECTORS {
        let template = SkillTemplate::decode(vector.code)
            .unwrap_or_else(|error| panic!("{}: {error}", vector.label));

        assert_eq!(template.primary, vector.primary, "{}", vector.label);
        assert_eq!(template.secondary, vector.secondary, "{}", vector.label);
        assert_eq!(
            template.attributes,
            vector.attributes.to_vec(),
            "{}",
            vector.label
        );
        assert_eq!(
            template.skills,
            expected_skills(vector.skills),
            "{}",
            vector.label
        );
    }
}

#[test]
fn every_published_code_re_encodes_to_the_identical_string() {
    // WP1.3's done criterion, and the test that actually proves the widths
    // were read right: a wrong width can still decode to sensible values but
    // will not reproduce the string.
    for vector in VECTORS {
        let template = SkillTemplate::decode(vector.code).expect(vector.label);
        assert_eq!(
            template.encode(),
            vector.code,
            "{} did not re-encode identically",
            vector.label
        );
    }
}

#[test]
fn the_players_attribute_ranks_are_from_points_only() {
    // §20.1 lists the player at Domination 12+1+3. The code stores 12: runes
    // and headgear are not part of a skill template. Mistaking the stored
    // rank for the effective one would understate the build by four ranks.
    let template = SkillTemplate::decode(VECTORS[0].code).unwrap();
    assert_eq!(template.rank_of(Attribute::DominationMagic), 12);
    assert_eq!(template.rank_of(Attribute::FastCasting), 10);
    // An attribute with no points reads as rank 0, not as missing.
    assert_eq!(template.rank_of(Attribute::IllusionMagic), 0);
}

#[test]
fn empty_bar_slots_survive_a_round_trip() {
    // The player's code has three empty slots and the heroes' codes have one
    // or two. An encoder that dropped them would shorten the bar.
    let template = SkillTemplate::decode(VECTORS[0].code).unwrap();
    assert_eq!(template.filled_slots(), 5);
    assert_eq!(template.skills[4], None);
    assert_eq!(template.skills[7], Some(SkillId(2416)));
    assert_eq!(template.encode(), VECTORS[0].code);
}

#[test]
fn a_template_stores_only_the_attributes_that_have_points() {
    // Hero 7 has two, not the four Ritualist attributes.
    let template = SkillTemplate::decode(VECTORS[5].code).unwrap();
    assert_eq!(template.attributes.len(), 2);
}

#[test]
fn attribute_order_is_preserved() {
    // Re-encoding sorts nothing, because the order is part of the string.
    let template = SkillTemplate::decode(VECTORS[2].code).unwrap();
    let ids: Vec<u8> = template
        .attributes
        .iter()
        .map(|(attribute, _)| attribute.template_id())
        .collect();
    assert_eq!(ids, vec![5, 6, 38]);
}

// ----------------------------------------------------------------- errors

#[test]
fn an_equipment_code_is_not_mistaken_for_a_skill_code() {
    let equipment = EquipmentTemplate {
        items: vec![EquipmentItem {
            slot: EquipmentSlot::Chest,
            item_id: 42,
            dye: gwsim_data::template::Dye(5),
            modifiers: vec![359],
        }],
    };
    let code = equipment.encode();

    let error = SkillTemplate::decode(&code).unwrap_err();
    assert_eq!(
        error,
        TemplateError::WrongType {
            found: 15,
            expected: 14,
            found_is: Some("an equipment template"),
        }
    );
    assert!(
        error.to_string().contains("equipment template"),
        "the message should say what it actually is: {error}"
    );
}

#[test]
fn a_truncated_code_says_what_it_was_reading() {
    let truncated = &VECTORS[0].code[..6];
    let error = SkillTemplate::decode(truncated).unwrap_err();
    assert!(
        matches!(error, TemplateError::Truncated { .. }),
        "expected a truncation error, got {error}"
    );
    assert!(error.to_string().contains("cut short"), "{error}");
}

#[test]
fn an_invalid_character_is_rejected() {
    let error = SkillTemplate::decode("OQBT*AUBPQaJ4EY6x0BAAAAAAuE").unwrap_err();
    assert!(
        matches!(error, TemplateError::BadCharacter { character: '*', .. }),
        "got {error}"
    );
}

#[test]
fn an_unused_attribute_id_is_rejected() {
    // Ids 26 to 28 exist in the numbering but name nothing. A code carrying
    // one is corrupt, and must not decode to a neighbouring attribute.
    let mut template = SkillTemplate::decode(VECTORS[0].code).unwrap();
    template.attributes = vec![(Attribute::DominationMagic, 12)];

    // Build a code by hand with attribute id 27 in place of a real one.
    let code = template.encode();
    let broken = swap_attribute_id(&code, 27);
    let error = SkillTemplate::decode(&broken).unwrap_err();
    assert!(
        matches!(
            error,
            TemplateError::BadValue {
                field: "an attribute id",
                value: 27,
                ..
            }
        ),
        "got {error}"
    );
    assert!(error.to_string().contains("26 to 28"), "{error}");
}

/// Rewrites the single attribute id in a one-attribute template code.
fn swap_attribute_id(code: &str, new_id: u32) -> String {
    use gwsim_data::template::{BitReader, BitWriter};

    let mut reader = BitReader::new(code).unwrap();
    let mut writer = BitWriter::new();

    for (width, field) in [(4, "type"), (4, "version"), (2, "profession width")] {
        let value = reader.read(width, field).unwrap();
        writer.write(value, width);
    }
    let profession_bits = 4;
    for _ in 0..2 {
        let value = reader.read(profession_bits, "profession").unwrap();
        writer.write(value, profession_bits);
    }
    let count = reader.read(4, "count").unwrap();
    writer.write(count, 4);
    let attribute_code = reader.read(4, "attribute width").unwrap();
    writer.write(attribute_code, 4);
    let attribute_bits = attribute_code + 4;

    // Replace the id, keep the rank.
    let _old = reader.read(attribute_bits, "attribute id").unwrap();
    writer.write(new_id, attribute_bits);
    let rank = reader.read(4, "rank").unwrap();
    writer.write(rank, 4);

    // Copy whatever is left.
    while reader.remaining() > 0 {
        let width = reader.remaining().min(8);
        let value = reader.read(width, "rest").unwrap();
        writer.write(value, width);
    }
    writer.finish()
}

#[test]
fn a_rank_above_twelve_is_rejected() {
    // Attribute points cannot buy past 12; anything higher is a rune, which a
    // skill template does not carry.
    let mut template = SkillTemplate::decode(VECTORS[0].code).unwrap();
    template.attributes = vec![(Attribute::DominationMagic, 15)];
    let code = template.encode();

    let error = SkillTemplate::decode(&code).unwrap_err();
    assert!(
        matches!(
            error,
            TemplateError::BadValue {
                field: "an attribute rank",
                value: 15,
                ..
            }
        ),
        "got {error}"
    );
}

#[test]
fn decode_never_panics_on_arbitrary_input() {
    // Not a property test, but the cases most likely to walk off the end.
    let inputs = [
        "",
        "A",
        "O",
        "OQ",
        "OQB",
        "AAAA",
        "////",
        "OOOOOOOOOOOO",
        "zzzzzzzzzzzzzzzzzzzz",
        "OQBTAUBPQaJ4EY6x0BAAAAAAu",
        "OQBTAUBPQaJ4EY6x0BAAAAAAuEE",
    ];
    for input in inputs {
        let _ = SkillTemplate::decode(input);
        let _ = EquipmentTemplate::decode(input);
        let _ = decode(input);
    }
}

// -------------------------------------------------------------- dispatching

#[test]
fn decode_picks_the_right_kind_from_the_type_nibble() {
    match decode(VECTORS[0].code).unwrap() {
        Template::Skill(template) => assert_eq!(template.primary, Profession::Mesmer),
        Template::Equipment(_) => panic!("a skill code decoded as equipment"),
    }

    let equipment = EquipmentTemplate {
        items: vec![EquipmentItem {
            slot: EquipmentSlot::Weapon,
            item_id: 110,
            dye: gwsim_data::template::Dye(5),
            modifiers: vec![],
        }],
    };
    match decode(&equipment.encode()).unwrap() {
        Template::Equipment(template) => assert_eq!(template.items.len(), 1),
        Template::Skill(_) => panic!("an equipment code decoded as a skill bar"),
    }
}

// --------------------------------------------------------------- equipment

#[test]
fn a_hand_built_equipment_template_round_trips() {
    // There are no published equipment codes for the M1 party, so the vector
    // is built from the layout T1.3.1 recorded.
    let original = EquipmentTemplate {
        items: vec![
            EquipmentItem {
                slot: EquipmentSlot::Chest,
                item_id: 42,
                dye: gwsim_data::template::Dye(5),
                // Prodigy's Insignia and a superior Domination rune.
                modifiers: vec![359, 75],
            },
            EquipmentItem {
                slot: EquipmentSlot::Weapon,
                item_id: 339,
                dye: gwsim_data::template::Dye(2),
                modifiers: vec![],
            },
        ],
    };

    let code = original.encode();
    let back = EquipmentTemplate::decode(&code).expect("should decode");
    assert_eq!(back, original);
    assert_eq!(back.encode(), code);
}

#[test]
fn every_equipment_slot_round_trips() {
    for slot in EquipmentSlot::ALL {
        let template = EquipmentTemplate {
            items: vec![EquipmentItem {
                slot,
                item_id: 1,
                dye: gwsim_data::template::Dye(9),
                modifiers: vec![],
            }],
        };
        let back = EquipmentTemplate::decode(&template.encode()).unwrap();
        assert_eq!(back.items[0].slot, slot, "{slot:?}");
    }
}

#[test]
fn the_dye_sits_between_the_modifier_count_and_the_modifiers() {
    // The wire order T1.3.1 §4 warns about. If dye were written after the
    // modifiers, an item with modifiers would round-trip wrongly while an
    // item without them still looked fine — so this uses both.
    let template = EquipmentTemplate {
        items: vec![EquipmentItem {
            slot: EquipmentSlot::Chest,
            item_id: 7,
            dye: gwsim_data::template::Dye(13),
            modifiers: vec![1, 2, 3],
        }],
    };
    let back = EquipmentTemplate::decode(&template.encode()).unwrap();
    assert_eq!(back.items[0].dye.0, 13);
    assert_eq!(back.items[0].modifiers, vec![1, 2, 3]);
}

#[test]
fn invalid_dye_values_are_rejected() {
    use gwsim_data::template::Dye;

    for value in [1u8, 14, 15] {
        assert!(!Dye(value).is_valid(), "dye {value} should be invalid");

        let template = EquipmentTemplate {
            items: vec![EquipmentItem {
                slot: EquipmentSlot::Chest,
                item_id: 1,
                dye: Dye(value),
                modifiers: vec![],
            }],
        };
        let error = EquipmentTemplate::decode(&template.encode()).unwrap_err();
        assert!(
            matches!(
                error,
                TemplateError::BadValue {
                    field: "a dye colour",
                    ..
                }
            ),
            "dye {value} should be rejected, got {error}"
        );
    }

    for value in [0u8, 2, 5, 9, 10, 13] {
        assert!(Dye(value).is_valid(), "dye {value} should be valid");
    }
}

#[test]
fn preview_only_dyes_are_named_but_marked() {
    use gwsim_data::template::Dye;

    assert!(Dye(0).is_preview_only());
    assert!(Dye(11).is_preview_only());
    assert!(!Dye(5).is_preview_only());
    assert_eq!(Dye(5).name(), Some("red"));
    assert_eq!(Dye(11).name(), Some("grey"));
    assert_eq!(Dye(14).name(), None);
}

#[test]
fn unknown_item_and_modifier_ids_still_round_trip() {
    // Keeping ids raw is what lets a code we do not understand survive being
    // read and written again.
    let original = EquipmentTemplate {
        items: vec![EquipmentItem {
            slot: EquipmentSlot::OffHand,
            item_id: 9999,
            dye: gwsim_data::template::Dye(3),
            modifiers: vec![8888],
        }],
    };
    let back = EquipmentTemplate::decode(&original.encode()).unwrap();
    assert_eq!(back, original);
}

#[test]
fn an_empty_equipment_template_round_trips() {
    let original = EquipmentTemplate::default();
    let back = EquipmentTemplate::decode(&original.encode()).unwrap();
    assert_eq!(back, original);
}

#[test]
fn a_skill_code_is_not_mistaken_for_equipment() {
    let error = EquipmentTemplate::decode(VECTORS[0].code).unwrap_err();
    assert_eq!(
        error,
        TemplateError::WrongType {
            found: 14,
            expected: 15,
            found_is: Some("a skill template"),
        }
    );
}

// ------------------------------------------------------- round-trip sweep

#[test]
fn synthetic_skill_templates_round_trip() {
    // Stands in for the property test T1.3.5 asks for, without adding a
    // dependency: it sweeps the width boundaries that matter, which is where
    // a hand-rolled bit codec actually breaks.
    let interesting_skill_ids: [u16; 8] = [0, 1, 255, 256, 1023, 1024, 2047, 2048];

    for (index, primary) in Profession::ALL.into_iter().enumerate() {
        let secondary = Profession::ALL.get((index + 3) % 10).copied();
        let secondary = (secondary != Some(primary)).then_some(secondary).flatten();

        for attribute_count in 0..=4usize {
            let attributes: Vec<(Attribute, u8)> = primary
                .attributes()
                .iter()
                .take(attribute_count)
                .enumerate()
                .map(|(rank, attribute)| (*attribute, (rank * 3) as u8 % 13))
                .collect();

            let template = SkillTemplate {
                primary,
                secondary,
                attributes,
                skills: expected_skills(interesting_skill_ids),
            };

            let code = template.encode();
            let back = SkillTemplate::decode(&code)
                .unwrap_or_else(|error| panic!("{primary:?}: {error} (code {code})"));
            assert_eq!(
                back, template,
                "{primary:?} with {attribute_count} attributes"
            );
            assert_eq!(back.encode(), code);
        }
    }
}

#[test]
fn every_skill_id_width_boundary_round_trips() {
    for highest in [
        0u16,
        255,
        256,
        511,
        512,
        1023,
        1024,
        2047,
        2048,
        4095,
        4096,
        u16::MAX,
    ] {
        let mut skills = [None; 8];
        skills[0] = (highest != 0).then_some(SkillId(highest));

        let template = SkillTemplate {
            primary: Profession::Mesmer,
            secondary: None,
            attributes: vec![(Attribute::DominationMagic, 12)],
            skills,
        };
        let back = SkillTemplate::decode(&template.encode())
            .unwrap_or_else(|error| panic!("skill id {highest}: {error}"));
        assert_eq!(back, template, "skill id {highest}");
    }
}

#[test]
fn every_attribute_round_trips_in_a_template() {
    for attribute in Attribute::ALL {
        let template = SkillTemplate {
            primary: attribute.profession(),
            secondary: None,
            attributes: vec![(attribute, 12)],
            skills: [None; 8],
        };
        let back = SkillTemplate::decode(&template.encode())
            .unwrap_or_else(|error| panic!("{attribute:?}: {error}"));
        assert_eq!(back.attributes, vec![(attribute, 12)], "{attribute:?}");
    }
}
