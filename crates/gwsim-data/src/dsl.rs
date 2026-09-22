//! The effect DSL: what a skill does, as data (§8.4).
//!
//! Every construct here earned its place by being needed to write one of the
//! 72 M1 skills out in full (T1.5.1). Nothing is here on speculation.
//!
//! The shape that carries the most weight is [`Control::Triggered`]. Twelve of
//! the sixteen skills §8.5 expected to need Rust turned out to be ordinary
//! data once a hex or enchantment could say "when X happens, do Y" — which is
//! why the handler count for M1 is four rather than sixteen.

use serde::{Deserialize, Serialize};

use crate::core::{Attribute, Condition, DamageType, RangeBand, SkillType, TitleTrack};
use crate::ids::Slug;

// ------------------------------------------------------------------- values

/// A number a skill uses, which may depend on rank or on what happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    /// The same whatever the rank.
    Fixed(i32),
    /// Scales on the skill's own attribute, from rank 0 to rank 15.
    Scaled(i32, i32),
    /// Scales on a named attribute instead.
    ScaledBy(Attribute, i32, i32),
    /// Scales on a title track's effective rank (A-020).
    TitleScaled(TitleTrack, i32, i32),
    /// A bare percentage, where what it is a percentage of is obvious from
    /// the action.
    Percent(f32),
    /// A percentage of something named.
    PercentOf {
        percent: f32,
        of: Quantity,
    },
    /// A value repeated once per unit of something.
    PerUnit {
        value: Box<Value>,
        of: Quantity,
    },
    /// The smaller of two values. How a stated maximum is written.
    Min(Box<Value>, Box<Value>),
    /// The larger of two values. How a stated minimum is written.
    Max(Box<Value>, Box<Value>),
    Sum(Vec<Value>),
}

impl Value {
    /// Whether this value, or anything inside it, scales on rank.
    pub fn scales(&self) -> bool {
        match self {
            Value::Scaled(at0, at15) | Value::ScaledBy(_, at0, at15) => at0 != at15,
            Value::TitleScaled(_, r0, rmax) => r0 != rmax,
            Value::PerUnit { value, .. } => value.scales(),
            Value::Min(left, right) | Value::Max(left, right) => left.scales() || right.scales(),
            Value::Sum(values) => values.iter().any(Value::scales),
            Value::Fixed(_) | Value::Percent(_) | Value::PercentOf { .. } => false,
        }
    }

    /// Every value inside this one, itself included.
    pub fn walk(&self) -> Vec<&Value> {
        let mut found = vec![self];
        match self {
            Value::PerUnit { value, .. } => found.extend(value.walk()),
            Value::Min(left, right) | Value::Max(left, right) => {
                found.extend(left.walk());
                found.extend(right.walk());
            }
            Value::Sum(values) => {
                for value in values {
                    found.extend(value.walk());
                }
            }
            _ => {}
        }
        found
    }
}

/// Something countable that a value can depend on.
///
/// Each of these is needed by at least one M1 skill; the list grows as
/// profession batches find more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Quantity {
    /// Energy the target just lost. Energy Surge.
    EnergyLost,
    /// Health something just lost. Spirit Transfer.
    HealthLost,
    /// The energy cost of the skill being cast. Aura of Restoration.
    EnergyCost,
    MaxHealth,
    CurrentHealth,
    /// Seconds a spirit was alive. Life.
    SecondsAlive,
    /// Hexes this skill just removed. Convert Hexes.
    HexesRemoved,
    /// Conditions this skill just removed. Cautery Signet.
    ConditionsRemoved,
    /// Enchantments this skill just removed. Strip Enchantment.
    EnchantmentsRemoved,
    /// Summoned creatures the user controls. Signet of Creation.
    CreaturesControlled,
    /// Spirits within earshot. Mend Body and Soul.
    SpiritsInEarshot,
}

// ---------------------------------------------------------------- selectors

/// Who or what an action reaches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Selector {
    #[serde(rename = "Self")]
    SelfUnit,
    Target,
    TargetFoe,
    TargetAlly,
    TargetOtherAlly,

    Adjacent(Box<Selector>),
    Nearby(Box<Selector>),
    InTheArea(Box<Selector>),
    Earshot(Box<Selector>),
    SpiritRange(Box<Selector>),
    /// Within the aura of a created thing, whose radius is its own rather
    /// than a named band. Every binding ritual needs this.
    InRangeOf(Box<Selector>),

    Party,
    PartyInRange(RangeBand),

    Foes,
    Allies,
    Spirits,
    Minions,
    Corpse,
    Location,

    /// Exactly one, the closest. Spirit Siphon, Spirit Transfer.
    Nearest(Box<Selector>),
    /// A selector narrowed by a filter.
    Filtered {
        of: Box<Selector>,
        filter: Filter,
    },
    /// The others an area skill also reaches, taking a share of the effect.
    ///
    /// Seven M1 skills apply 75% of their damage this way, which makes this
    /// far more load-bearing than its single line in §8.4 suggests.
    Secondary {
        of: Box<Selector>,
        factor: f32,
    },
}

impl Selector {
    /// Whether this selector can only ever pick foes.
    pub fn is_foe_only(&self) -> bool {
        match self {
            Selector::TargetFoe | Selector::Foes => true,
            Selector::Adjacent(inner)
            | Selector::Nearby(inner)
            | Selector::InTheArea(inner)
            | Selector::Earshot(inner)
            | Selector::SpiritRange(inner)
            | Selector::InRangeOf(inner)
            | Selector::Nearest(inner) => inner.is_foe_only(),
            Selector::Filtered { of, .. } | Selector::Secondary { of, .. } => of.is_foe_only(),
            _ => false,
        }
    }

    /// Whether this selector can only ever pick allies, or the user.
    pub fn is_ally_only(&self) -> bool {
        match self {
            Selector::SelfUnit
            | Selector::TargetAlly
            | Selector::TargetOtherAlly
            | Selector::Party
            | Selector::PartyInRange(_)
            | Selector::Allies => true,
            Selector::Adjacent(inner)
            | Selector::Nearby(inner)
            | Selector::InTheArea(inner)
            | Selector::Earshot(inner)
            | Selector::SpiritRange(inner)
            | Selector::InRangeOf(inner)
            | Selector::Nearest(inner) => inner.is_ally_only(),
            Selector::Filtered { of, .. } | Selector::Secondary { of, .. } => of.is_ally_only(),
            _ => false,
        }
    }
}

/// A test that narrows a selector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Filter {
    Hexed,
    Enchanted,
    HasCondition(Condition),
    Casting,
    CastingSpell,
    Attacking,
    Moving,
    KnockedDown,
    BelowHealth {
        percent: f32,
    },
    AboveHealth {
        percent: f32,
    },
    CreatureType(String),
    IsSpirit,
    IsSummoned,
    IsMinion,
    HoldingMartialWeapon,
    HoldingCasterWeapon,
    /// Created or controlled by the user. "Your spirits".
    Owned,
    /// On the other side. "Hostile summoned creatures".
    Hostile,
    Allied,

    // Threshold filters. These came from the M1 *insignias* rather than from
    // the skills T1.5.1 studied, which is why they are not in that findings
    // file's construct list: Prodigy's, Minion Master's and Shaman's all
    // grant armor in steps, and the step is the whole mechanic.
    /// At least this many of the user's skills are recharging. Prodigy's.
    RechargingSkills {
        at_least: u8,
    },
    /// The user controls at least this many minions. Minion Master's.
    ControllingMinions {
        at_least: u8,
    },
    /// The user controls at least this many spirits. Shaman's.
    ControllingSpirits {
        at_least: u8,
    },
    /// The skill being used exploits a corpse. Bloodstained.
    ExploitsCorpse,

    Not(Box<Filter>),
    All(Vec<Filter>),
    Any(Vec<Filter>),
}

// ------------------------------------------------------------------ actions

/// One thing a skill does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Action {
    Damage {
        to: Selector,
        #[serde(default)]
        kind: Option<DamageType>,
        amount: Value,
        #[serde(default)]
        armor_ignoring: bool,
    },
    LifeSteal {
        to: Selector,
        amount: Value,
    },
    HealthLoss {
        to: Selector,
        amount: Value,
    },
    Heal {
        to: Selector,
        amount: Value,
    },
    HealthGain {
        to: Selector,
        amount: Value,
    },
    SacrificeHealth {
        percent: Value,
    },

    GainEnergy {
        to: Selector,
        amount: Value,
    },
    LoseEnergy {
        to: Selector,
        amount: Value,
    },
    DrainEnergy {
        from: Selector,
        amount: Value,
    },
    GainAdrenaline {
        to: Selector,
        strikes: Value,
    },
    LoseAdrenaline {
        from: Selector,
        strikes: Value,
    },

    ApplyCondition {
        to: Selector,
        condition: Condition,
        duration: Value,
    },
    RemoveConditions {
        from: Selector,
        count: Value,
        #[serde(default)]
        which: Option<Condition>,
    },
    ApplyEffect {
        to: Selector,
        /// The id of an `EffectDef` in the same skill, or a global slug.
        effect: String,
        duration: Value,
    },
    RemoveEffects {
        from: Selector,
        kind: EffectKind,
        count: Value,
    },

    /// Stops a skill in progress. The cost is lost and recharge starts.
    Interrupt {
        to: Selector,
    },
    /// Makes a skill fail. **Not** the same as interrupting: a failed skill
    /// recharges instantly, an interrupted one does not (§10.5 ENG-14).
    FailSkill {
        to: Selector,
    },
    KnockDown {
        to: Selector,
        duration: Value,
    },
    DisableSkills {
        to: Selector,
        #[serde(default)]
        which: Option<SkillType>,
        duration: Value,
    },
    ModifyRecharge {
        to: Selector,
        percent: Value,
    },
    /// Recharges a skill immediately. Pious Renewal recharges itself.
    RechargeSkill {
        #[serde(default)]
        which: Option<String>,
    },

    Summon {
        creature: Slug,
        level: Value,
    },
    CreateSpirit {
        spirit: Slug,
        level: Value,
        duration: Value,
    },
    CreateArea {
        area: Slug,
        duration: Value,
    },

    Resurrect {
        to: Selector,
        health_percent: Value,
        energy_percent: Value,
    },
    ShadowStep {
        to: Selector,
    },
    Teleport {
        to: Selector,
    },

    ModifyStat {
        to: Selector,
        stat: Stat,
        amount: Value,
        #[serde(default)]
        category: ModCategory,
    },
    /// Sets a stat rather than adding to it. Master of Magic sets four
    /// attributes to a value, which no `ModifyStat` can express.
    SetStat {
        to: Selector,
        stat: Stat,
        value: Value,
    },
    /// Reduces damage before it lands.
    ///
    /// Six M1 skills do this in four different shapes, which is why the
    /// fields are separate rather than one number.
    ReduceIncomingDamage {
        to: Selector,
        #[serde(default)]
        flat: Option<Value>,
        #[serde(default)]
        percent: Option<Value>,
        /// A ceiling on a single hit, as a share of maximum health. Shelter.
        #[serde(default)]
        cap_percent_of_max_health: Option<Value>,
        /// Only damage from this source is reduced. Veil of Thorns reduces
        /// spell damage only.
        #[serde(default)]
        only_from: Option<DamageSource>,
    },
    /// This attack cannot be blocked.
    SetUnblockable,
    /// Immune to critical hits. Armor of Unfeeling.
    SetCriticalImmune {
        to: Selector,
    },

    HoldBundle {
        bundle: Slug,
        duration: Value,
    },
    DropBundle,

    /// A handler does the rest. Four M1 skills need one.
    RunHandler {
        name: String,
    },

    Control(Box<Control>),
}

/// Where damage came from, for effects that only reduce some of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DamageSource {
    Spells,
    Attacks,
    /// From a creature suffering a condition. Armor of Sanctity.
    FoesWithConditions,
}

/// A kind of effect, for removal and for stacking families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EffectKind {
    Hex,
    Enchantment,
    Stance,
    Preparation,
    Glyph,
    WeaponSpell,
    Form,
    ShoutOrChant,
    Condition,
    SpiritAura,
    AreaEffect,
    Environment,
    Consumable,
    Title,
    Blessing,
    PartyBonus,
    Bundle,
}

impl EffectKind {
    /// The family this effect competes in, when only one may be active.
    ///
    /// A stance replaces a stance; a weapon spell replaces the weapon spell
    /// on that target; a bundle replaces the held bundle. Returning [`None`]
    /// means any number may coexist.
    pub fn one_at_a_time_family(self) -> Option<&'static str> {
        Some(match self {
            EffectKind::Stance => "stance",
            EffectKind::Preparation => "preparation",
            EffectKind::Glyph => "glyph",
            EffectKind::Form => "form",
            EffectKind::WeaponSpell => "weapon-spell-per-target",
            EffectKind::Bundle => "bundle",
            EffectKind::PartyBonus => "party-bonus",
            _ => return None,
        })
    }
}

/// Something a skill can change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stat {
    Armor,
    MovementSpeed,
    AttackSpeed,
    ActivationTime,
    Recharge,
    AdrenalineRate,
    ProjectileSpeed,
    BlockChance,
    HealthRegeneration,
    EnergyRegeneration,
    MaxEnergy,
    MaxHealth,
    DamageDealt,
    DamageTaken,
    HealingReceived,
    /// A temporary rank change, which ripples through every scaled value on
    /// the bar (§10.14). Masochism is the M1 skill that needs it.
    AttributeRank(Attribute),
    /// Every elemental attribute at once. Master of Magic.
    ElementalAttributes,
}

/// How a modifier combines with others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ModCategory {
    /// Counts as core armor, outside the bonus cap.
    Core,
    /// Subject to the +25 armor cap.
    #[default]
    Bonus,
    /// Its own rules.
    Special,
    Additive,
    Multiplicative,
}

// ------------------------------------------------------------------ control

/// Flow control around actions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Control {
    If {
        condition: Filter,
        #[serde(default)]
        of: Option<Selector>,
        then: Vec<Action>,
        #[serde(default)]
        otherwise: Vec<Action>,
    },
    ForEach {
        selector: Selector,
        actions: Vec<Action>,
    },
    Chance {
        percent: f32,
        actions: Vec<Action>,
    },
    Sequence(Vec<Action>),
    /// Waits for something to happen, then acts.
    ///
    /// The construct that turns most would-be handlers into data.
    Triggered {
        event: Event,
        #[serde(default)]
        filter: Option<Filter>,
        actions: Vec<Action>,
        /// How many times it may fire. [`None`] is unlimited.
        #[serde(default)]
        charges: Option<u8>,
    },
}

/// Something that happens, which a trigger can wait for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Event {
    OnSkillActivationStart,
    OnSkillActivationEnd,
    /// Any creature uses a skill. Panic watches for this.
    OnSkillUsed,
    OnSpellCast,
    OnAttack,
    OnHit,
    OnBlocked,
    OnMiss,
    OnDamageTaken,
    OnDamageDealt,
    OnHeal,
    OnEffectApplied,
    OnEffectRemoved,
    OnEffectEnded,
    OnConditionRemoved,
    OnEnchantmentRemoved,
    OnInterrupted,
    OnKnockedDown,
    OnDeath,
    OnKill,
    /// A kill that granted experience. Air of Superiority.
    OnExperienceKill,
    OnCreatureCreated,
    OnSpiritDeath,
    OnBundleDropped,
    OnEnergyChanged,
    OnTick,
}

// -------------------------------------------------------- effect definitions

/// A named effect a skill applies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectDef {
    /// Local to the skill, or a global slug for a shared effect.
    pub id: String,
    pub kind: EffectKind,
    #[serde(default)]
    pub stacking: StackingRule,
    #[serde(default)]
    pub duration: Option<Value>,
    #[serde(default)]
    pub removable_by: Vec<EffectKind>,
    /// Changes that hold while the effect is active.
    #[serde(default)]
    pub while_active: Vec<Action>,
    #[serde(default)]
    pub triggers: Vec<Control>,
    /// What happens when it ends.
    ///
    /// Also how a delayed effect is written: an empty `while_active` with a
    /// populated `on_end` is "wait, then act", which is what Ancestors' Rage
    /// and Pious Renewal are. Adding a `Delay` action would give two ways to
    /// say one thing.
    #[serde(default)]
    pub on_end: Vec<Action>,
    /// Energy pips consumed while maintained.
    #[serde(default)]
    pub upkeep: Option<i8>,
}

/// What happens when an effect is applied twice.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackingRule {
    /// Effects sharing a key compete. Defaults to the effect's own id.
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub rule: StackingBehaviour,
}

/// How two applications of one effect combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum StackingBehaviour {
    /// The new one replaces the old.
    Replace,
    /// Whichever has longer left wins. How conditions behave.
    #[default]
    KeepLonger,
    /// One instance per source, all active at once.
    StackBySource,
}

// ----------------------------------------------------------------- ai hints

/// When the AI should use a skill.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AiHints {
    #[serde(default)]
    pub use_when: Vec<Filter>,
    #[serde(default)]
    pub never_when: Vec<Filter>,
    #[serde(default)]
    pub target: TargetPreference,
    /// Higher goes first when several skills are usable.
    #[serde(default)]
    pub priority: i8,
}

/// Which of several valid targets the AI prefers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum TargetPreference {
    #[default]
    Default,
    LowestHealth,
    HighestHealth,
    Casting,
    Attacking,
    Nearest,
    MostFoesNearby,
}

/// A reference to a Rust handler.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandlerRef {
    pub name: String,
    #[serde(default)]
    pub params: std::collections::BTreeMap<String, Value>,
}

/// Something that can describe a handler in words.
///
/// The engine's `SkillHandler` (WP3.6) extends this. The data crate takes a
/// registry at render time so it never depends on the engine.
pub trait HandlerDescribe {
    /// A sentence saying what the handler does.
    fn describe(&self, params: &std::collections::BTreeMap<String, Value>) -> String;
}

/// Somewhere the renderer can look up a handler's description.
pub trait HandlerRegistry {
    /// The handler registered under a name.
    fn get(&self, name: &str) -> Option<&dyn HandlerDescribe>;
}

/// A registry with nothing in it, for rendering without the engine.
pub struct NoHandlers;

impl HandlerRegistry for NoHandlers {
    fn get(&self, _name: &str) -> Option<&dyn HandlerDescribe> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T>(text: &str) -> T
    where
        T: serde::de::DeserializeOwned + Serialize + PartialEq + std::fmt::Debug,
    {
        let value: T = ron::from_str(text).expect("should parse");
        let written = ron::to_string(&value).expect("should serialise");
        let back: T = ron::from_str(&written).expect("should parse again");
        assert_eq!(back, value);
        value
    }

    #[test]
    fn the_design_example_effect_parses() {
        // §8.3's illustrative Fireball line, verbatim in shape.
        let action: Action =
            round_trip("Damage(to: Nearby(TargetFoe), kind: Some(Fire), amount: Scaled(7, 112))");
        match action {
            Action::Damage { amount, kind, .. } => {
                assert_eq!(amount, Value::Scaled(7, 112));
                assert_eq!(kind, Some(DamageType::Fire));
            }
            other => panic!("expected damage, got {other:?}"),
        }
    }

    #[test]
    fn a_secondary_target_share_parses() {
        // The construct seven M1 skills need: nearby foes take 75%.
        let selector: Selector = round_trip("Secondary(of: Nearby(TargetFoe), factor: 0.75)");
        assert!(selector.is_foe_only());
    }

    #[test]
    fn energy_surges_per_unit_damage_parses() {
        let action: Action =
            round_trip("Damage(to: TargetFoe, amount: PerUnit(value: Fixed(7), of: EnergyLost))");
        match action {
            Action::Damage { amount, .. } => match amount {
                Value::PerUnit { value, of } => {
                    assert_eq!(*value, Value::Fixed(7));
                    assert_eq!(of, Quantity::EnergyLost);
                }
                other => panic!("expected PerUnit, got {other:?}"),
            },
            other => panic!("expected damage, got {other:?}"),
        }
    }

    #[test]
    fn a_hex_with_a_trigger_round_trips() {
        // Mistrust's shape, and the reason it is not a handler.
        let effect: EffectDef = round_trip(
            r#"(
                id: "mistrust",
                kind: Hex,
                duration: Some(Fixed(6)),
                triggers: [
                    Triggered(
                        event: OnSpellCast,
                        charges: Some(1),
                        actions: [
                            FailSkill(to: Target),
                            Damage(to: TargetFoe, amount: Scaled(10, 80)),
                            Damage(
                                to: Secondary(of: Nearby(TargetFoe), factor: 0.75),
                                amount: Scaled(10, 80),
                            ),
                        ],
                    ),
                ],
            )"#,
        );
        assert_eq!(effect.kind, EffectKind::Hex);
        assert_eq!(effect.triggers.len(), 1);
    }

    #[test]
    fn a_maintained_enchantment_round_trips() {
        let effect: EffectDef = round_trip(
            r#"(
                id: "upkeep-test",
                kind: Enchantment,
                upkeep: Some(1),
                while_active: [
                    ModifyStat(to: Self, stat: Armor, amount: Fixed(20), category: Bonus),
                ],
            )"#,
        );
        assert_eq!(effect.upkeep, Some(1));
    }

    #[test]
    fn a_delayed_effect_is_an_empty_effect_with_an_on_end() {
        // Ancestors' Rage. No Delay action exists, deliberately.
        let effect: EffectDef = round_trip(
            r#"(
                id: "ancestors-rage",
                kind: Enchantment,
                duration: Some(Fixed(1)),
                on_end: [
                    Damage(
                        to: Adjacent(TargetAlly),
                        kind: Some(Lightning),
                        amount: Scaled(5, 110),
                    ),
                ],
            )"#,
        );
        assert!(effect.while_active.is_empty());
        assert_eq!(effect.on_end.len(), 1);
    }

    #[test]
    fn failing_and_interrupting_are_different_actions() {
        // They differ in whether the skill recharges (§10.5 ENG-14), so one
        // cannot stand in for the other.
        let fail: Action = ron::from_str("FailSkill(to: Target)").unwrap();
        let interrupt: Action = ron::from_str("Interrupt(to: Target)").unwrap();
        assert_ne!(fail, interrupt);
    }

    #[test]
    fn selectors_know_which_side_they_can_reach() {
        let foes: Selector = ron::from_str("Nearby(TargetFoe)").unwrap();
        assert!(foes.is_foe_only());
        assert!(!foes.is_ally_only());

        let allies: Selector = ron::from_str("Earshot(Party)").unwrap();
        assert!(allies.is_ally_only());
        assert!(!allies.is_foe_only());

        // A bare Target could be either, so it claims neither.
        let target: Selector = ron::from_str("Target").unwrap();
        assert!(!target.is_foe_only());
        assert!(!target.is_ally_only());
    }

    #[test]
    fn self_is_spelled_self_in_a_data_file() {
        let selector: Selector = ron::from_str("Self").unwrap();
        assert_eq!(selector, Selector::SelfUnit);
        assert_eq!(ron::to_string(&Selector::SelfUnit).unwrap(), "Self");
    }

    #[test]
    fn one_at_a_time_families_come_from_the_effect_kind() {
        assert_eq!(EffectKind::Stance.one_at_a_time_family(), Some("stance"));
        assert_eq!(
            EffectKind::WeaponSpell.one_at_a_time_family(),
            Some("weapon-spell-per-target")
        );
        assert_eq!(EffectKind::Bundle.one_at_a_time_family(), Some("bundle"));
        // Hexes and enchantments stack freely.
        assert_eq!(EffectKind::Hex.one_at_a_time_family(), None);
        assert_eq!(EffectKind::Enchantment.one_at_a_time_family(), None);
    }

    #[test]
    fn conditions_keep_the_longer_duration_by_default() {
        // Reapplying a condition keeps whichever has longer left.
        assert_eq!(StackingBehaviour::default(), StackingBehaviour::KeepLonger);
    }

    #[test]
    fn a_value_knows_whether_it_scales() {
        assert!(Value::Scaled(1, 10).scales());
        assert!(!Value::Scaled(7, 7).scales());
        assert!(!Value::Fixed(7).scales());
        assert!(
            Value::PerUnit {
                value: Box::new(Value::Scaled(1, 10)),
                of: Quantity::EnergyLost,
            }
            .scales()
        );
        assert!(
            !Value::PerUnit {
                value: Box::new(Value::Fixed(7)),
                of: Quantity::EnergyLost,
            }
            .scales()
        );
    }

    #[test]
    fn walking_a_value_finds_everything_inside_it() {
        let value = Value::Min(
            Box::new(Value::Sum(vec![Value::Fixed(1), Value::Scaled(2, 3)])),
            Box::new(Value::Fixed(10)),
        );
        assert_eq!(value.walk().len(), 5);
        assert!(value.scales());
    }

    #[test]
    fn a_capped_value_is_a_min() {
        // Reversal of Fortune's "maximum 15...80".
        let value: Value =
            round_trip("Min(PerUnit(value: Fixed(1), of: HealthLost), Scaled(15, 80))");
        assert!(value.scales());
    }

    #[test]
    fn every_reduce_damage_shape_parses() {
        // The four shapes six M1 skills need.
        let flat: Action =
            round_trip("ReduceIncomingDamage(to: TargetAlly, flat: Some(Scaled(3, 18)))");
        let percent: Action =
            round_trip("ReduceIncomingDamage(to: Self, percent: Some(Fixed(50)))");
        let capped: Action = round_trip(
            "ReduceIncomingDamage(to: InRangeOf(Spirits), cap_percent_of_max_health: Some(Fixed(10)))",
        );
        let scoped: Action = round_trip(
            "ReduceIncomingDamage(to: Self, percent: Some(Scaled(5, 35)), only_from: Some(Spells))",
        );
        for action in [flat, percent, capped, scoped] {
            assert!(matches!(action, Action::ReduceIncomingDamage { .. }));
        }
    }

    #[test]
    fn set_stat_and_modify_stat_are_different() {
        // Master of Magic sets attributes; Masochism adds to them.
        let set: Action =
            round_trip("SetStat(to: Self, stat: ElementalAttributes, value: Scaled(8, 14))");
        let modify: Action =
            round_trip("ModifyStat(to: Self, stat: AttributeRank(DeathMagic), amount: Fixed(2))");
        assert!(matches!(set, Action::SetStat { .. }));
        assert!(matches!(modify, Action::ModifyStat { .. }));
    }

    #[test]
    fn a_modifier_defaults_to_the_capped_bonus_category() {
        // Most armor bonuses are capped at +25; core armor is the exception,
        // so the safe default is Bonus.
        let action: Action =
            ron::from_str("ModifyStat(to: Self, stat: Armor, amount: Fixed(20))").unwrap();
        match action {
            Action::ModifyStat { category, .. } => assert_eq!(category, ModCategory::Bonus),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn nested_control_round_trips() {
        let action: Action = round_trip(
            r#"Control(If(
                condition: Enchanted,
                of: Some(TargetFoe),
                then: [Damage(to: TargetFoe, amount: Scaled(10, 100))],
                otherwise: [],
            ))"#,
        );
        assert!(matches!(action, Action::Control(_)));
    }

    #[test]
    fn ai_hints_default_to_nothing_in_particular() {
        let hints = AiHints::default();
        assert!(hints.use_when.is_empty());
        assert_eq!(hints.target, TargetPreference::Default);
        assert_eq!(hints.priority, 0);
    }

    #[test]
    fn an_empty_registry_finds_no_handler() {
        assert!(NoHandlers.get("arcane_echo").is_none());
    }
}
