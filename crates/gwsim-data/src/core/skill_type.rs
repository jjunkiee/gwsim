//! The skill type hierarchy.

use serde::{Deserialize, Serialize};

/// A skill's type, as a node in the wiki's skill type hierarchy.
///
/// Types form a tree: an effect that names a supertype usually reaches its
/// subtypes too, so "remove an enchantment" also removes flash enchantments.
/// [`SkillType::is_a`] answers those questions, and [`SkillType::parent`]
/// walks one step up.
///
/// **Elite is not a type.** The wiki lists "elite skill" alongside the real
/// types, but it is a designation that cuts across all of them, so it is a
/// `bool` on the skill rather than a variant here. The same goes for touch
/// skills, which are a range property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SkillType {
    /// A skill with no more specific type. Also the root every other type
    /// descends from, so `anything.is_a(Skill)` holds.
    Skill,

    // Attacks
    AttackSkill,
    MeleeAttack,
    PetAttack,
    AxeAttack,
    DaggerAttack,
    LeadAttack,
    OffHandAttack,
    DualAttack,
    HammerAttack,
    ScytheAttack,
    SwordAttack,
    RangedAttack,
    BowAttack,
    SpearAttack,

    // Spells
    Spell,
    EnchantmentSpell,
    FlashEnchantmentSpell,
    HexSpell,
    ItemSpell,
    WardSpell,
    WeaponSpell,
    WellSpell,

    // Rituals
    Ritual,
    BindingRitual,
    NatureRitual,
    EbonVanguardRitual,

    // Everything else
    Chant,
    Echo,
    Form,
    Glyph,
    Preparation,
    Shout,
    Signet,
    Stance,
    Title,
    Trap,
}

impl SkillType {
    /// Every skill type.
    pub const ALL: [SkillType; 37] = [
        SkillType::Skill,
        SkillType::AttackSkill,
        SkillType::MeleeAttack,
        SkillType::PetAttack,
        SkillType::AxeAttack,
        SkillType::DaggerAttack,
        SkillType::LeadAttack,
        SkillType::OffHandAttack,
        SkillType::DualAttack,
        SkillType::HammerAttack,
        SkillType::ScytheAttack,
        SkillType::SwordAttack,
        SkillType::RangedAttack,
        SkillType::BowAttack,
        SkillType::SpearAttack,
        SkillType::Spell,
        SkillType::EnchantmentSpell,
        SkillType::FlashEnchantmentSpell,
        SkillType::HexSpell,
        SkillType::ItemSpell,
        SkillType::WardSpell,
        SkillType::WeaponSpell,
        SkillType::WellSpell,
        SkillType::Ritual,
        SkillType::BindingRitual,
        SkillType::NatureRitual,
        SkillType::EbonVanguardRitual,
        SkillType::Chant,
        SkillType::Echo,
        SkillType::Form,
        SkillType::Glyph,
        SkillType::Preparation,
        SkillType::Shout,
        SkillType::Signet,
        SkillType::Stance,
        SkillType::Title,
        SkillType::Trap,
    ];

    /// The type one step up the hierarchy, or [`None`] for the root.
    pub fn parent(self) -> Option<SkillType> {
        use SkillType as T;
        Some(match self {
            T::Skill => return None,

            T::AttackSkill => T::Skill,
            T::MeleeAttack | T::RangedAttack => T::AttackSkill,
            T::PetAttack
            | T::AxeAttack
            | T::DaggerAttack
            | T::HammerAttack
            | T::ScytheAttack
            | T::SwordAttack => T::MeleeAttack,
            T::LeadAttack | T::OffHandAttack | T::DualAttack => T::DaggerAttack,
            T::BowAttack | T::SpearAttack => T::RangedAttack,

            T::Spell => T::Skill,
            T::EnchantmentSpell
            | T::HexSpell
            | T::ItemSpell
            | T::WardSpell
            | T::WeaponSpell
            | T::WellSpell => T::Spell,
            T::FlashEnchantmentSpell => T::EnchantmentSpell,

            T::Ritual => T::Skill,
            T::BindingRitual | T::NatureRitual | T::EbonVanguardRitual => T::Ritual,

            T::Chant
            | T::Echo
            | T::Form
            | T::Glyph
            | T::Preparation
            | T::Shout
            | T::Signet
            | T::Stance
            | T::Title
            | T::Trap => T::Skill,
        })
    }

    /// Whether this type is `ancestor`, or descends from it.
    ///
    /// A type is its own ancestor, so `HexSpell.is_a(HexSpell)` is true.
    pub fn is_a(self, ancestor: SkillType) -> bool {
        let mut current = Some(self);
        while let Some(kind) = current {
            if kind == ancestor {
                return true;
            }
            current = kind.parent();
        }
        false
    }

    /// This type and every ancestor of it, nearest first.
    pub fn ancestry(self) -> Vec<SkillType> {
        let mut chain = Vec::new();
        let mut current = Some(self);
        while let Some(kind) = current {
            chain.push(kind);
            current = kind.parent();
        }
        chain
    }

    /// The aftercast delay a skill of this type gets unless its own file says
    /// otherwise, in milliseconds.
    ///
    /// Three quarters of a second for most types, and none for the types that
    /// activate instantly: stances, shouts, flash enchantments and pet attacks
    /// ([Aftercast delay](https://wiki.guildwars.com/wiki/Aftercast_delay),
    /// [Skill type](https://wiki.guildwars.com/wiki/Skill_type)).
    ///
    /// **Attack skills are the weak entry here.** The wiki describes their
    /// recovery as animation-based rather than a flat ¾ s, so the 750 they get
    /// from this default is a placeholder. T3.4.1 confirms the per-type rules
    /// and is expected to change it.
    pub fn default_aftercast_ms(self) -> u32 {
        if self.activates_instantly() { 0 } else { 750 }
    }

    /// Whether using a skill of this type occupies the action queue, so that
    /// it cannot start while another action is running.
    ///
    /// Shouts, stances and pet attacks sit outside the queue and can be used
    /// mid-action. Flash enchantments do **not**: they have no activation
    /// time, but the wiki is explicit that they cannot be used during another
    /// activation, which is why this is a separate question from
    /// [`Self::default_aftercast_ms`] ([Activation time](https://wiki.guildwars.com/wiki/Activation_time)).
    pub fn uses_action_queue(self) -> bool {
        !matches!(
            self.outside_queue_root(),
            Some(SkillType::Shout | SkillType::Stance | SkillType::PetAttack)
        )
    }

    /// Whether a knocked-down creature can still use a skill of this type.
    ///
    /// Stances, shouts and pet attacks can; everything else cannot
    /// ([Activation time](https://wiki.guildwars.com/wiki/Activation_time)).
    pub fn usable_while_knocked_down(self) -> bool {
        !self.uses_action_queue()
    }

    /// Whether this type has no activation time at all.
    fn activates_instantly(self) -> bool {
        self.is_a(SkillType::Stance)
            || self.is_a(SkillType::Shout)
            || self.is_a(SkillType::PetAttack)
            || self.is_a(SkillType::FlashEnchantmentSpell)
    }

    /// The nearest ancestor that decides queue behaviour, if any.
    fn outside_queue_root(self) -> Option<SkillType> {
        self.ancestry().into_iter().find(|kind| {
            matches!(
                kind,
                SkillType::Shout | SkillType::Stance | SkillType::PetAttack
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::SkillType as T;

    #[test]
    fn only_the_root_has_no_parent() {
        for kind in T::ALL {
            if kind == T::Skill {
                assert_eq!(kind.parent(), None);
            } else {
                assert!(kind.parent().is_some(), "{kind:?} has no parent");
            }
        }
    }

    #[test]
    fn everything_descends_from_skill() {
        for kind in T::ALL {
            assert!(kind.is_a(T::Skill), "{kind:?} does not reach the root");
        }
    }

    #[test]
    fn the_hierarchy_has_no_cycles() {
        // ancestry() loops forever if a parent chain ever closes on itself.
        for kind in T::ALL {
            let chain = kind.ancestry();
            assert_eq!(*chain.last().unwrap(), T::Skill);
            assert!(chain.len() <= 5, "{kind:?} has a suspiciously deep chain");
        }
    }

    #[test]
    fn is_a_is_reflexive() {
        for kind in T::ALL {
            assert!(kind.is_a(kind));
        }
    }

    #[test]
    fn spell_subtypes() {
        for kind in [
            T::EnchantmentSpell,
            T::FlashEnchantmentSpell,
            T::HexSpell,
            T::ItemSpell,
            T::WardSpell,
            T::WeaponSpell,
            T::WellSpell,
        ] {
            assert!(kind.is_a(T::Spell), "{kind:?} should be a spell");
        }
        assert!(T::FlashEnchantmentSpell.is_a(T::EnchantmentSpell));
        assert!(!T::EnchantmentSpell.is_a(T::FlashEnchantmentSpell));
        assert!(!T::Signet.is_a(T::Spell));
        assert!(!T::Chant.is_a(T::Spell));
    }

    #[test]
    fn attack_subtypes() {
        for kind in [
            T::MeleeAttack,
            T::PetAttack,
            T::AxeAttack,
            T::DaggerAttack,
            T::LeadAttack,
            T::OffHandAttack,
            T::DualAttack,
            T::HammerAttack,
            T::ScytheAttack,
            T::SwordAttack,
            T::RangedAttack,
            T::BowAttack,
            T::SpearAttack,
        ] {
            assert!(kind.is_a(T::AttackSkill), "{kind:?} should be an attack");
        }
        assert!(T::LeadAttack.is_a(T::DaggerAttack));
        assert!(T::LeadAttack.is_a(T::MeleeAttack));
        assert!(!T::LeadAttack.is_a(T::RangedAttack));
        assert!(T::BowAttack.is_a(T::RangedAttack));
        assert!(!T::BowAttack.is_a(T::MeleeAttack));
    }

    #[test]
    fn ritual_subtypes() {
        for kind in [T::BindingRitual, T::NatureRitual, T::EbonVanguardRitual] {
            assert!(kind.is_a(T::Ritual), "{kind:?} should be a ritual");
        }
        assert!(!T::Ritual.is_a(T::Spell));
    }

    #[test]
    fn instant_types_have_no_aftercast() {
        for kind in [T::Stance, T::Shout, T::PetAttack, T::FlashEnchantmentSpell] {
            assert_eq!(
                kind.default_aftercast_ms(),
                0,
                "{kind:?} should have no aftercast"
            );
        }
    }

    #[test]
    fn other_top_level_types_take_three_quarters_of_a_second() {
        for kind in [
            T::Skill,
            T::Spell,
            T::EnchantmentSpell,
            T::HexSpell,
            T::Signet,
            T::Chant,
            T::Echo,
            T::Form,
            T::Glyph,
            T::Preparation,
            T::Ritual,
            T::Trap,
        ] {
            assert_eq!(kind.default_aftercast_ms(), 750, "{kind:?}");
        }
    }

    #[test]
    fn flash_enchantments_skip_aftercast_but_still_need_the_queue() {
        // The distinction that is easy to lose: no activation time does not
        // mean usable mid-action.
        assert_eq!(T::FlashEnchantmentSpell.default_aftercast_ms(), 0);
        assert!(T::FlashEnchantmentSpell.uses_action_queue());
        assert!(!T::FlashEnchantmentSpell.usable_while_knocked_down());
    }

    #[test]
    fn stances_shouts_and_pet_attacks_bypass_the_queue() {
        for kind in [T::Stance, T::Shout, T::PetAttack] {
            assert!(!kind.uses_action_queue(), "{kind:?}");
            assert!(kind.usable_while_knocked_down(), "{kind:?}");
        }
    }

    #[test]
    fn everything_else_uses_the_queue() {
        for kind in T::ALL {
            if matches!(kind, T::Stance | T::Shout | T::PetAttack) {
                continue;
            }
            assert!(kind.uses_action_queue(), "{kind:?} should use the queue");
            assert!(!kind.usable_while_knocked_down(), "{kind:?}");
        }
    }
}
