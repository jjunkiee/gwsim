//! Skill and equipment template codes.
//!
//! These are the strings players paste into chat to share a build. The codec
//! is deliberately **free of any `DataSet` lookup**: skill ids stay raw
//! numbers here, and turning them into skills is T1.4.9's job. That keeps the
//! codec testable against published codes without any data loaded.
//!
//! The bit layouts came from T1.3.1, which decoded the seven §20.1 codes field
//! by field before any of this was written.

mod bits;

pub use bits::{ALPHABET, BitReader, BitWriter, TemplateError, bits_needed};

use serde::{Deserialize, Serialize};

use crate::core::{Attribute, Profession};
use crate::ids::SkillId;

/// The type nibble of a skill template.
pub const SKILL_TEMPLATE_TYPE: u8 = 14;
/// The type nibble of an equipment template.
pub const EQUIPMENT_TEMPLATE_TYPE: u8 = 15;

/// How many skills a bar holds.
pub const BAR_SIZE: usize = 8;

/// The highest rank attribute points alone can buy.
pub const MAX_POINTS_RANK: u8 = 12;

/// Which kind of template a code holds.
#[derive(Debug, Clone, PartialEq)]
pub enum Template {
    Skill(SkillTemplate),
    Equipment(EquipmentTemplate),
}

/// Decodes a code of either kind, deciding from its type nibble.
pub fn decode(code: &str) -> Result<Template, TemplateError> {
    let mut reader = BitReader::new(code)?;
    let first = reader.read(4, "the template type")? as u8;

    match first {
        SKILL_TEMPLATE_TYPE => SkillTemplate::decode(code).map(Template::Skill),
        EQUIPMENT_TEMPLATE_TYPE => EquipmentTemplate::decode(code).map(Template::Equipment),
        // A pre-2007 code has no type nibble, only a version. Version 0 is a
        // skill template and version 1 is an equipment template, which is the
        // one place the two formats disagree about what a bare nibble means.
        0 => SkillTemplate::decode(code).map(Template::Skill),
        1 => EquipmentTemplate::decode(code).map(Template::Equipment),
        other => Err(TemplateError::WrongType {
            found: other,
            expected: SKILL_TEMPLATE_TYPE,
            found_is: None,
        }),
    }
}

// --------------------------------------------------------------- skill codes

/// A saved set of professions, attribute points and skills.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillTemplate {
    pub primary: Profession,
    /// [`None`] when the build has no secondary profession.
    #[serde(default)]
    pub secondary: Option<Profession>,
    /// Attributes and the rank **points alone** buy, in the code's own order.
    ///
    /// Runes and headgear are not in a skill template, so these are not the
    /// effective ranks a build ends up with.
    pub attributes: Vec<(Attribute, u8)>,
    /// The eight bar slots. [`None`] is an empty slot.
    pub skills: [Option<SkillId>; BAR_SIZE],
}

impl SkillTemplate {
    /// Reads a skill template code.
    pub fn decode(code: &str) -> Result<Self, TemplateError> {
        let mut reader = BitReader::new(code)?;

        // Pre-2007 codes have only a version nibble; later ones have a type
        // nibble first.
        let first = reader.read(4, "the template type")? as u8;
        match first {
            SKILL_TEMPLATE_TYPE => {
                let version = reader.read(4, "the version")? as u8;
                if version != 0 {
                    return Err(TemplateError::UnsupportedVersion { version });
                }
            }
            0 => {} // legacy: the nibble was the version
            EQUIPMENT_TEMPLATE_TYPE => {
                return Err(TemplateError::WrongType {
                    found: first,
                    expected: SKILL_TEMPLATE_TYPE,
                    found_is: Some("an equipment template"),
                });
            }
            other => {
                return Err(TemplateError::WrongType {
                    found: other,
                    expected: SKILL_TEMPLATE_TYPE,
                    found_is: None,
                });
            }
        }

        let profession_bits = reader.read(2, "the profession width")? * 2 + 4;
        let primary_index = reader.read(profession_bits, "the primary profession")?;
        let secondary_index = reader.read(profession_bits, "the secondary profession")?;

        let primary = profession(primary_index, "the primary profession")?.ok_or(
            TemplateError::BadValue {
                field: "the primary profession",
                value: primary_index,
                reason: "means no profession; a build must have a primary",
            },
        )?;
        let secondary = profession(secondary_index, "the secondary profession")?;

        let count = reader.read(4, "the attribute count")?;
        let attribute_bits = reader.read(4, "the attribute width")? + 4;

        let mut attributes = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let id = reader.read(attribute_bits, "an attribute id")?;
            let rank = reader.read(4, "an attribute rank")? as u8;

            let attribute = u8::try_from(id)
                .ok()
                .and_then(Attribute::from_template_id)
                .ok_or(TemplateError::BadValue {
                    field: "an attribute id",
                    value: id,
                    reason: "names no attribute (26 to 28 are unused)",
                })?;

            if rank > MAX_POINTS_RANK {
                return Err(TemplateError::BadValue {
                    field: "an attribute rank",
                    value: u32::from(rank),
                    reason: "is above 12, which attribute points cannot reach",
                });
            }
            attributes.push((attribute, rank));
        }

        let skill_bits = reader.read(4, "the skill width")? + 8;
        let mut skills = [None; BAR_SIZE];
        for slot in skills.iter_mut() {
            let id = reader.read(skill_bits, "a skill id")?;
            let id = u16::try_from(id).map_err(|_| TemplateError::BadValue {
                field: "a skill id",
                value: id,
                reason: "is too large to be a skill id",
            })?;
            *slot = (id != SkillId::NONE).then_some(SkillId(id));
        }

        // The trailing bit is always zero. Reading it is what proves the
        // widths were right: if any were wrong, the stream would be out of
        // step by here.
        let tail = reader.read(1, "the trailing bit")?;
        if tail != 0 {
            return Err(TemplateError::BadValue {
                field: "the trailing bit",
                value: tail,
                reason: "should be zero; the code is malformed or was read with the \
                         wrong field widths",
            });
        }

        Ok(SkillTemplate {
            primary,
            secondary,
            attributes,
            skills,
        })
    }

    /// Writes this template as a code.
    ///
    /// Field widths are chosen the way the game's codes do, as established in
    /// T1.3.1 §3, so a decoded code re-encodes to the identical string.
    pub fn encode(&self) -> String {
        let mut writer = BitWriter::new();

        writer.write(u32::from(SKILL_TEMPLATE_TYPE), 4);
        writer.write(0, 4); // version

        let profession_code = self.profession_width_code();
        writer.write(profession_code, 2);
        let profession_bits = profession_code * 2 + 4;
        writer.write(u32::from(self.primary.template_index()), profession_bits);
        writer.write(
            self.secondary
                .map(|profession| u32::from(profession.template_index()))
                .unwrap_or(0),
            profession_bits,
        );

        writer.write(self.attributes.len() as u32, 4);
        let attribute_code = self.attribute_width_code();
        writer.write(attribute_code, 4);
        let attribute_bits = attribute_code + 4;
        for (attribute, rank) in &self.attributes {
            writer.write(u32::from(attribute.template_id()), attribute_bits);
            writer.write(u32::from(*rank), 4);
        }

        let skill_code = self.skill_width_code();
        writer.write(skill_code, 4);
        let skill_bits = skill_code + 8;
        for slot in &self.skills {
            writer.write(slot.map(|id| u32::from(id.get())).unwrap_or(0), skill_bits);
        }

        writer.write(0, 1); // trailing bit
        writer.finish()
    }

    /// The smallest width code that fits every profession index.
    fn profession_width_code(&self) -> u32 {
        let largest = u32::from(self.primary.template_index()).max(
            self.secondary
                .map(|profession| u32::from(profession.template_index()))
                .unwrap_or(0),
        );
        let needed = bits_needed(largest);
        // bits = code * 2 + 4
        (0..=3).find(|code| code * 2 + 4 >= needed).unwrap_or(3)
    }

    /// The smallest width code that fits every attribute id, with a floor.
    ///
    /// **The floor of 1 is inferred, not documented.** Both §20.1 Mesmer codes
    /// store ids no larger than 3 — which code 0's four bits would hold — yet
    /// both use code 1. Every published code we have agrees with this rule;
    /// see T1.3.1 §3. If a code ever fails to round-trip, start here.
    fn attribute_width_code(&self) -> u32 {
        let largest = self
            .attributes
            .iter()
            .map(|(attribute, _)| u32::from(attribute.template_id()))
            .max()
            .unwrap_or(0);
        let needed = bits_needed(largest);
        (0..=15)
            .find(|code| code + 4 >= needed)
            .unwrap_or(15)
            .max(1)
    }

    /// The smallest width code that fits every skill id.
    fn skill_width_code(&self) -> u32 {
        let largest = self
            .skills
            .iter()
            .map(|slot| slot.map(|id| u32::from(id.get())).unwrap_or(0))
            .max()
            .unwrap_or(0);
        let needed = bits_needed(largest);
        (0..=15).find(|code| code + 8 >= needed).unwrap_or(15)
    }

    /// The rank this template gives an attribute, from points alone.
    pub fn rank_of(&self, attribute: Attribute) -> u8 {
        self.attributes
            .iter()
            .find(|(candidate, _)| *candidate == attribute)
            .map(|(_, rank)| *rank)
            .unwrap_or(0)
    }

    /// How many bar slots hold a skill.
    pub fn filled_slots(&self) -> usize {
        self.skills.iter().filter(|slot| slot.is_some()).count()
    }
}

/// Turns a template profession index into a profession.
fn profession(index: u32, field: &'static str) -> Result<Option<Profession>, TemplateError> {
    if index == 0 {
        return Ok(None);
    }
    u8::try_from(index)
        .ok()
        .and_then(Profession::from_template_index)
        .map(Some)
        .ok_or(TemplateError::BadValue {
            field,
            value: index,
            reason: "names no profession",
        })
}

// ----------------------------------------------------------- equipment codes

/// A saved set of equipment.
///
/// **These describe PvP equipment.** Only PvP characters can load one, and the
/// item id list is of PvP items. gwsim simulates PvE, so this exists to read
/// codes a published build happens to include, not to describe gwsim gear
/// (T1.3.1 §4).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EquipmentTemplate {
    pub items: Vec<EquipmentItem>,
}

/// One equipped item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EquipmentItem {
    pub slot: EquipmentSlot,
    /// The item's id. Kept raw, so an unknown item still round-trips.
    pub item_id: u32,
    pub dye: Dye,
    /// The upgrades on it. Kept raw, for the same reason.
    #[serde(default)]
    pub modifiers: Vec<u32>,
}

/// Where an item is worn or held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EquipmentSlot {
    Weapon,
    OffHand,
    Chest,
    Legs,
    Head,
    Feet,
    Hands,
}

impl EquipmentSlot {
    /// Every slot, in the order the format numbers them.
    pub const ALL: [EquipmentSlot; 7] = [
        EquipmentSlot::Weapon,
        EquipmentSlot::OffHand,
        EquipmentSlot::Chest,
        EquipmentSlot::Legs,
        EquipmentSlot::Head,
        EquipmentSlot::Feet,
        EquipmentSlot::Hands,
    ];

    /// The number the format uses.
    pub fn index(self) -> u32 {
        self as u32
    }

    /// The slot with this number.
    pub fn from_index(index: u32) -> Option<Self> {
        Self::ALL.get(index as usize).copied()
    }
}

/// An item's dye colour.
///
/// Held as the raw value because the preview-only colours are meaningful —
/// they say "this was dyed something the PvP window cannot express" — and
/// flattening them to silver at load would lose that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Dye(pub u8);

impl Dye {
    /// Whether this value can appear in a valid code.
    ///
    /// 1, 14 and 15 cannot: the wiki is explicit that they make a code
    /// invalid.
    pub fn is_valid(self) -> bool {
        !matches!(self.0, 1 | 14 | 15) && self.0 < 16
    }

    /// Whether the colour only shows in the preview, becoming silver once
    /// loaded. 0 and 10 to 13.
    pub fn is_preview_only(self) -> bool {
        matches!(self.0, 0 | 10..=13)
    }

    /// The colour's name, where it has one.
    pub fn name(self) -> Option<&'static str> {
        Some(match self.0 {
            0 => "default",
            2 => "blue",
            3 => "green",
            4 => "purple",
            5 => "red",
            6 => "yellow",
            7 => "brown",
            8 => "orange",
            9 => "silver",
            10 => "black",
            11 => "grey",
            12 => "white",
            13 => "pink",
            _ => return None,
        })
    }
}

impl EquipmentTemplate {
    /// Reads an equipment template code.
    pub fn decode(code: &str) -> Result<Self, TemplateError> {
        let mut reader = BitReader::new(code)?;

        let first = reader.read(4, "the template type")? as u8;
        match first {
            EQUIPMENT_TEMPLATE_TYPE => {
                let version = reader.read(4, "the version")? as u8;
                if version != 0 {
                    return Err(TemplateError::UnsupportedVersion { version });
                }
            }
            // A pre-2007 equipment template's bare version nibble is 1, where
            // a skill template's is 0. This is the one place the two formats
            // disagree about what a bare nibble means.
            1 => {}
            SKILL_TEMPLATE_TYPE => {
                return Err(TemplateError::WrongType {
                    found: first,
                    expected: EQUIPMENT_TEMPLATE_TYPE,
                    found_is: Some("a skill template"),
                });
            }
            other => {
                return Err(TemplateError::WrongType {
                    found: other,
                    expected: EQUIPMENT_TEMPLATE_TYPE,
                    found_is: None,
                });
            }
        }

        // Raw bit counts, not width codes: there is no offset here, unlike
        // the skill format.
        let item_bits = reader.read(4, "the item id width")?;
        let modifier_bits = reader.read(4, "the modifier id width")?;
        let count = reader.read(3, "the item count")?;

        let mut items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let slot_index = reader.read(3, "an equipment slot")?;
            let slot = EquipmentSlot::from_index(slot_index).ok_or(TemplateError::BadValue {
                field: "an equipment slot",
                value: slot_index,
                reason: "names no slot; slots run 0 to 6",
            })?;

            let item_id = reader.read(item_bits, "an item id")?;
            let modifier_count = reader.read(2, "a modifier count")?;

            // The dye sits between the modifier count and the modifiers, not
            // after them. Reading these in struct order rather than wire order
            // shifts everything that follows.
            let dye = Dye(reader.read(4, "a dye colour")? as u8);
            if !dye.is_valid() {
                return Err(TemplateError::BadValue {
                    field: "a dye colour",
                    value: u32::from(dye.0),
                    reason: "is not a usable colour; 1, 14 and 15 make a code invalid",
                });
            }

            let mut modifiers = Vec::with_capacity(modifier_count as usize);
            for _ in 0..modifier_count {
                modifiers.push(reader.read(modifier_bits, "a modifier id")?);
            }

            items.push(EquipmentItem {
                slot,
                item_id,
                dye,
                modifiers,
            });
        }

        Ok(EquipmentTemplate { items })
    }

    /// Writes this template as a code.
    pub fn encode(&self) -> String {
        let mut writer = BitWriter::new();

        writer.write(u32::from(EQUIPMENT_TEMPLATE_TYPE), 4);
        writer.write(0, 4); // version

        let item_bits = self.item_id_bits();
        let modifier_bits = self.modifier_id_bits();
        writer.write(item_bits, 4);
        writer.write(modifier_bits, 4);
        writer.write(self.items.len() as u32, 3);

        for item in &self.items {
            writer.write(item.slot.index(), 3);
            writer.write(item.item_id, item_bits);
            writer.write(item.modifiers.len() as u32, 2);
            writer.write(u32::from(item.dye.0), 4);
            for modifier in &item.modifiers {
                writer.write(*modifier, modifier_bits);
            }
        }

        writer.finish()
    }

    fn item_id_bits(&self) -> u32 {
        bits_needed(
            self.items
                .iter()
                .map(|item| item.item_id)
                .max()
                .unwrap_or(0),
        )
    }

    fn modifier_id_bits(&self) -> u32 {
        bits_needed(
            self.items
                .iter()
                .flat_map(|item| item.modifiers.iter().copied())
                .max()
                .unwrap_or(0),
        )
    }
}
