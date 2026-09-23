//! Turning an encoding into English (§8.6).
//!
//! **The wording is ours.** Nothing here is taken from the game or the wiki,
//! and nothing may be: the whole point of generating descriptions is that
//! gwsim can show what a skill does without storing ArenaNet's text (C2,
//! §8.10).
//!
//! The text is also a **review aid**. A contributor reads the generated
//! sentence next to the wiki page and looks for a disagreement, so grammar is
//! not cosmetic here: "target foe take 20 damage" makes a reader distrust the
//! encoding when the fault is the renderer's. That is why selectors carry
//! their number and verbs agree with them.

use std::fmt::Write as _;

use crate::core::{CoreData, DamageType, RangeBand};
use crate::derived::{scaled, title_scaled};
use crate::dsl::{
    Action, Control, DamageSource, EffectDef, EffectKind, Event, Filter, HandlerRegistry, Quantity,
    Selector, Stat, Value,
};
use crate::skill::Skill;

/// What the renderer needs besides the skill itself.
pub struct DescribeContext<'a> {
    /// The rank to evaluate at. [`None`] shows the range.
    pub rank: Option<u8>,
    /// The title rank, for title-scaled values.
    pub title_rank: Option<u8>,
    /// Where core tables live, for title scaling.
    pub core: Option<&'a CoreData>,
    /// Where handler descriptions come from.
    pub handlers: &'a dyn HandlerRegistry,
}

impl<'a> DescribeContext<'a> {
    /// A context that shows value ranges and knows no handlers.
    pub fn ranges(handlers: &'a dyn HandlerRegistry) -> Self {
        DescribeContext {
            rank: None,
            title_rank: None,
            core: None,
            handlers,
        }
    }

    /// A context that evaluates at one rank.
    pub fn at_rank(rank: u8, handlers: &'a dyn HandlerRegistry) -> Self {
        DescribeContext {
            rank: Some(rank),
            title_rank: None,
            core: None,
            handlers,
        }
    }
}

/// Describes a whole skill.
pub fn describe(skill: &Skill, context: &DescribeContext<'_>) -> String {
    let Some(encoding) = &skill.encoding else {
        return "This skill has not been encoded yet.".to_owned();
    };

    let mut sentences: Vec<String> = Vec::new();

    if let Some(handler) = &encoding.handler {
        match context.handlers.get(&handler.name) {
            Some(described) => sentences.push(described.describe(&handler.params)),
            None => sentences.push(format!(
                "This skill is handled in code by `{}`, which has no description.",
                handler.name
            )),
        }
    }

    for action in &encoding.effects {
        if let Some(sentence) = describe_action(action, &encoding.effect_defs, context) {
            sentences.push(sentence);
        }
    }

    if sentences.is_empty() {
        return "This skill does nothing.".to_owned();
    }
    sentences.join(" ")
}

/// A noun phrase, and whether a verb after it should be plural.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phrase {
    pub text: String,
    pub plural: bool,
}

impl Phrase {
    fn singular(text: impl Into<String>) -> Self {
        Phrase {
            text: text.into(),
            plural: false,
        }
    }

    fn plural(text: impl Into<String>) -> Self {
        Phrase {
            text: text.into(),
            plural: true,
        }
    }

    /// The right form of a verb whose plural is the bare stem.
    fn verb(&self, stem: &str) -> String {
        if self.plural {
            stem.to_owned()
        } else if stem.ends_with('h') || stem.ends_with('s') {
            format!("{stem}es")
        } else {
            format!("{stem}s")
        }
    }

    /// "is" or "are".
    fn is(&self) -> &'static str {
        if self.plural { "are" } else { "is" }
    }

    /// The possessive. "you" becomes "your", not "you's".
    fn possessive(&self) -> String {
        if self.text == "you" {
            "your".to_owned()
        } else {
            format!("{}'s", self.text)
        }
    }
}

/// Describes one action as a sentence, if it says anything.
pub fn describe_action(
    action: &Action,
    effects: &[EffectDef],
    context: &DescribeContext<'_>,
) -> Option<String> {
    let value = |value: &Value| render_value(value, context);

    let sentence = match action {
        Action::Damage {
            to,
            kind,
            amount,
            armor_ignoring,
        } => {
            let noun = match kind {
                Some(kind) => format!("{} damage", damage_word(*kind)),
                None => "damage".to_owned(),
            };
            let ignoring = if *armor_ignoring {
                ", ignoring armor"
            } else {
                ""
            };
            let who = render_phrase(to);
            format!(
                "{} {} {}{ignoring}.",
                capitalise(&who.text),
                who.verb("take"),
                quantity(amount, &noun, context)
            )
        }
        Action::LifeSteal { to, amount } => format!(
            "Steal {} from {}.",
            quantity(amount, "health", context),
            render_selector(to)
        ),
        Action::HealthLoss { to, amount } => {
            let who = render_phrase(to);
            format!(
                "{} {} {}.",
                capitalise(&who.text),
                who.verb("lose"),
                quantity(amount, "health", context)
            )
        }
        Action::Heal { to, amount } | Action::HealthGain { to, amount } => {
            let who = render_phrase(to);
            format!(
                "{} {} {}.",
                capitalise(&who.text),
                who.verb("gain"),
                quantity(amount, "health", context)
            )
        }
        Action::SacrificeHealth { percent } => {
            format!("You sacrifice {}% of your maximum health.", value(percent))
        }

        Action::GainEnergy { to, amount } => {
            let who = render_phrase(to);
            format!(
                "{} {} {}.",
                capitalise(&who.text),
                who.verb("gain"),
                quantity(amount, "energy", context)
            )
        }
        Action::LoseEnergy { to, amount } => {
            let who = render_phrase(to);
            format!(
                "{} {} {}.",
                capitalise(&who.text),
                who.verb("lose"),
                quantity(amount, "energy", context)
            )
        }
        Action::DrainEnergy { from, amount } => format!(
            "Take {} from {}.",
            quantity(amount, "energy", context),
            render_selector(from)
        ),
        Action::GainAdrenaline { to, strikes } => {
            let who = render_phrase(to);
            format!(
                "{} {} {}.",
                capitalise(&who.text),
                who.verb("gain"),
                quantity(strikes, "strikes of adrenaline", context)
            )
        }
        Action::LoseAdrenaline { from, strikes } => {
            let who = render_phrase(from);
            format!(
                "{} {} {}.",
                capitalise(&who.text),
                who.verb("lose"),
                quantity(strikes, "strikes of adrenaline", context)
            )
        }

        Action::ApplyCondition {
            to,
            condition,
            duration,
        } => {
            let who = render_phrase(to);
            format!(
                "{} {} {} for {}.",
                capitalise(&who.text),
                who.verb("suffer"),
                condition_word(*condition),
                seconds(duration, context)
            )
        }
        Action::RemoveConditions { from, count, which } => {
            let what = which
                .map(|condition| condition_word(condition).to_owned())
                .unwrap_or_else(|| "condition".to_owned());
            format!(
                "Remove {} from {}.",
                counted(count, &what, context),
                render_selector(from)
            )
        }
        Action::ApplyEffect {
            to,
            effect,
            duration,
        } => match effects.iter().find(|definition| definition.id == *effect) {
            Some(definition) => describe_effect_def(definition, to, duration, context),
            None => {
                let who = render_phrase(to);
                format!(
                    "For {}, {} {} affected by {}.",
                    seconds(duration, context),
                    who.text,
                    who.is(),
                    humanise(effect)
                )
            }
        },
        Action::RemoveEffects { from, kind, count } => format!(
            "Remove {} from {}.",
            counted(count, effect_word(*kind), context),
            render_selector(from)
        ),

        Action::Interrupt { to, disable } => match disable {
            None => format!("Interrupt {}.", render_selector(to)),
            Some(duration) => format!(
                "Interrupt {}; an interrupted skill is disabled for an additional {}.",
                render_selector(to),
                seconds(duration, context)
            ),
        },
        Action::FailSkill { to } => {
            let who = render_phrase(to);
            format!("The skill {} {} using fails.", who.text, who.is())
        }
        Action::KnockDown { to, duration } => {
            let who = render_phrase(to);
            format!(
                "{} {} knocked down for {}.",
                capitalise(&who.text),
                who.is(),
                seconds(duration, context)
            )
        }
        Action::DisableSkills {
            to,
            which,
            duration,
        } => {
            let what = which
                .map(|kind| format!("{kind:?} skills"))
                .unwrap_or_else(|| "skills".to_owned());
            format!(
                "{} {what} are disabled for {}.",
                capitalise(&render_phrase(to).possessive()),
                seconds(duration, context)
            )
        }
        Action::ModifyRecharge { to, percent } => format!(
            "{} skills recharge {}% faster.",
            capitalise(&render_phrase(to).possessive()),
            value(percent)
        ),
        Action::RechargeSkill { which } => match which {
            Some(name) => format!("{} recharges.", humanise(name)),
            None => "This skill recharges.".to_owned(),
        },

        Action::Summon { creature, level } => format!(
            "Create a level {} {}.",
            value(level),
            humanise(creature.as_str())
        ),
        Action::CreateSpirit {
            spirit,
            level,
            duration,
            attack_damage,
        } => match attack_damage {
            None => format!(
                "Create a level {} {} spirit, which dies after {}.",
                value(level),
                humanise(spirit.as_str()),
                seconds(duration, context)
            ),
            Some(damage) => format!(
                "Create a level {} {} spirit, which attacks for {} damage and dies after {}.",
                value(level),
                humanise(spirit.as_str()),
                value(damage),
                seconds(duration, context)
            ),
        },
        Action::CreateArea { area, duration } => format!(
            "Create a {} for {}.",
            humanise(area.as_str()),
            seconds(duration, context)
        ),

        Action::Resurrect {
            to,
            health_percent,
            energy_percent,
        } => format!(
            "Resurrect {} with {}% health and {}% energy.",
            render_selector(to),
            value(health_percent),
            value(energy_percent)
        ),
        Action::ShadowStep { to } => format!("Shadow step to {}.", render_selector(to)),
        Action::Teleport { to } => format!("Teleport to {}.", render_selector(to)),

        Action::ModifyStat {
            to, stat, amount, ..
        } => {
            let who = render_phrase(to);
            let rendered = value(amount);
            // A range may be negative at both ends ("-1…-3"), so stripping a
            // single leading sign would report a different number from the
            // one encoded. Drop every sign, or none.
            let negative = rendered.starts_with('-');
            let (verb, magnitude) = if negative {
                ("lose", rendered.replace('-', ""))
            } else {
                ("gain", rendered)
            };
            format!(
                "{} {} {magnitude} {}.",
                capitalise(&who.text),
                who.verb(verb),
                stat_word(*stat)
            )
        }
        Action::SetStat { to, stat, value: v } => format!(
            "{} {} {} set to {}.",
            capitalise(&render_phrase(to).possessive()),
            stat_word(*stat),
            if stat_is_plural(*stat) { "are" } else { "is" },
            value(v)
        ),
        Action::ReduceIncomingDamage {
            to,
            flat,
            percent,
            cap_percent_of_max_health,
            only_from,
            limit,
            heals,
            cost_to_source,
        } => {
            let source = only_from
                .map(|source| format!(" from {}", source_word(source)))
                .unwrap_or_default();
            let who = render_phrase(to);
            let cost = cost_to_source
                .as_ref()
                .map(|v| format!(" Each time, the spirit loses {} health.", value(v)))
                .unwrap_or_default();
            if *heals {
                let most = limit
                    .as_ref()
                    .map(|v| format!(", up to {}", value(v)))
                    .unwrap_or_default();
                format!(
                    "Damage{source} to {} heals instead of harming{most}.",
                    who.text
                )
            } else if let Some(cap) = cap_percent_of_max_health {
                format!(
                    "{} cannot lose more than {}% of their maximum health from one hit.{cost}",
                    capitalise(&who.text),
                    value(cap)
                )
            } else if let Some(flat) = flat {
                format!(
                    "Damage{source} to {} is reduced by {}.{cost}",
                    who.text,
                    value(flat)
                )
            } else if let Some(percent) = percent {
                format!(
                    "{} {} {}% less damage{source}.",
                    capitalise(&who.text),
                    who.verb("take"),
                    value(percent)
                )
            } else {
                return None;
            }
        }
        Action::SetUnblockable => "This attack cannot be blocked.".to_owned(),
        Action::SetCriticalImmune { to } => {
            let who = render_phrase(to);
            format!(
                "{} {} immune to critical hits.",
                capitalise(&who.text),
                who.is()
            )
        }

        Action::HoldBundle { bundle, duration } => format!(
            "Hold {} for up to {}.",
            humanise(bundle.as_str()),
            seconds(duration, context)
        ),
        Action::DropBundle => "Drop what you are holding.".to_owned(),

        Action::RunHandler { name } => context.handlers.get(name)?.describe(&Default::default()),
        Action::EndEffect => "This effect ends.".to_owned(),
        Action::WaiveSacrifice => "You do not sacrifice health.".to_owned(),

        Action::Control(control) => return describe_control(control, effects, context),
    };

    Some(sentence)
}

fn describe_control(
    control: &Control,
    effects: &[EffectDef],
    context: &DescribeContext<'_>,
) -> Option<String> {
    match control {
        Control::If {
            condition,
            of,
            then,
            otherwise,
        } => {
            let subject = of
                .as_ref()
                .map(render_selector)
                .unwrap_or_else(|| "the target".to_owned());
            let mut sentence = format!(
                "If {subject} {}, {}",
                filter_phrase(condition),
                lower_first(&join(then, effects, context))
            );
            if !otherwise.is_empty() {
                let _ = write!(
                    sentence,
                    " Otherwise, {}",
                    lower_first(&join(otherwise, effects, context))
                );
            }
            Some(sentence)
        }
        Control::ForEach { selector, actions } => Some(format!(
            "For each of {}: {}",
            render_selector(selector),
            join(actions, effects, context)
        )),
        Control::Chance { percent, actions } => Some(format!(
            "{percent}% of the time, {}",
            lower_first(&join(actions, effects, context))
        )),
        Control::Sequence(actions) => {
            let text = join(actions, effects, context);
            (!text.is_empty()).then_some(text)
        }
        Control::Triggered {
            event,
            actions,
            charges,
            ..
        } => {
            let times = match charges {
                Some(1) => "The next time".to_owned(),
                Some(n) => format!("The next {n} times"),
                None => "Whenever".to_owned(),
            };
            Some(format!(
                "{times} {}, {}",
                event_phrase(*event),
                lower_first(&join(actions, effects, context))
            ))
        }
    }
}

fn describe_effect_def(
    definition: &EffectDef,
    to: &Selector,
    duration: &Value,
    context: &DescribeContext<'_>,
) -> String {
    let who = render_phrase(to);
    let mut sentence = format!("For {}, ", seconds(duration, context));

    let inner = join(&definition.while_active, &[], context);
    if inner.is_empty() {
        let _ = write!(sentence, "{} {} affected.", who.text, who.is());
    } else {
        let _ = write!(sentence, "{}", lower_first(&inner));
    }

    for trigger in &definition.triggers {
        if let Some(text) = describe_control(trigger, &[], context) {
            let _ = write!(sentence, " {text}");
        }
    }
    if !definition.on_end.is_empty() {
        let _ = write!(
            sentence,
            " When it ends, {}",
            lower_first(&join(&definition.on_end, &[], context))
        );
    }
    sentence
}

fn join(actions: &[Action], effects: &[EffectDef], context: &DescribeContext<'_>) -> String {
    actions
        .iter()
        .filter_map(|action| describe_action(action, effects, context))
        .collect::<Vec<_>>()
        .join(" ")
}

// ------------------------------------------------------------------- values

/// A value with the noun it counts, so the unit reads in the right place.
///
/// `PerUnit` is why this exists: "7 damage per point of energy lost" has the
/// noun in the middle, which no amount of formatting the value alone can do.
fn quantity(value: &Value, noun: &str, context: &DescribeContext<'_>) -> String {
    match value {
        Value::PerUnit { value, of } => format!(
            "{} {noun} per {}",
            render_value(value, context),
            quantity_unit(*of)
        ),
        Value::Min(inner, limit) => format!(
            "{}, up to {}",
            quantity(inner, noun, context),
            render_value(limit, context)
        ),
        Value::Max(inner, floor) => format!(
            "{}, at least {}",
            quantity(inner, noun, context),
            render_value(floor, context)
        ),
        Value::PercentOf { percent, of } => {
            format!("{noun} equal to {percent}% of {}", quantity_word(*of))
        }
        other => format!("{} {noun}", render_value(other, context)),
    }
}

/// A duration, pluralised.
fn seconds(value: &Value, context: &DescribeContext<'_>) -> String {
    let rendered = render_value(value, context);
    if rendered == "1" {
        "1 second".to_owned()
    } else {
        format!("{rendered} seconds")
    }
}

/// A count with a noun, pluralised.
fn counted(value: &Value, noun: &str, context: &DescribeContext<'_>) -> String {
    let rendered = render_value(value, context);
    if rendered == "1" {
        format!("1 {}", singularise(noun))
    } else {
        format!("{rendered} {}", pluralise(noun))
    }
}

/// Renders a value: a range when no rank is given, a number when one is.
pub fn render_value(value: &Value, context: &DescribeContext<'_>) -> String {
    match value {
        Value::Fixed(number) => number.to_string(),
        Value::Scaled(at0, at15) | Value::ScaledBy(_, at0, at15) => match context.rank {
            Some(rank) => scaled(*at0, *at15, rank).to_string(),
            // An ellipsis, not the wiki's three dots: this is our wording.
            None => format!("{at0}…{at15}"),
        },
        Value::TitleScaled(track, r0, rmax) => match (context.title_rank, context.core) {
            (Some(rank), Some(core)) => title_scaled(*r0, *rmax, rank, core, *track)
                .map(|value| value.to_string())
                .unwrap_or_else(|| format!("{r0}…{rmax}")),
            _ => format!("{r0}…{rmax}"),
        },
        Value::Percent(percent) => format!("{percent}"),
        Value::PercentOf { percent, of } => {
            format!("{percent}% of {}", quantity_word(*of))
        }
        Value::ShareOf { percent, of } => format!(
            "{}% of {}",
            render_value(percent, context),
            quantity_word(*of)
        ),
        Value::PerUnit { value, of } => format!(
            "{} per {}",
            render_value(value, context),
            quantity_unit(*of)
        ),
        Value::Min(left, right) => format!(
            "{}, up to {}",
            render_value(left, context),
            render_value(right, context)
        ),
        Value::Max(left, right) => format!(
            "{}, at least {}",
            render_value(left, context),
            render_value(right, context)
        ),
        Value::Sum(values) => values
            .iter()
            .map(|value| render_value(value, context))
            .collect::<Vec<_>>()
            .join(" plus "),
    }
}

// ---------------------------------------------------------------- selectors

/// Renders a selector as a noun phrase.
pub fn render_selector(selector: &Selector) -> String {
    render_phrase(selector).text
}

/// Renders a selector, keeping track of whether it is plural.
pub fn render_phrase(selector: &Selector) -> Phrase {
    match selector {
        // "you take", not "you takes".
        Selector::SelfUnit => Phrase::plural("you"),
        Selector::Target => Phrase::singular("target"),
        Selector::TargetFoe => Phrase::singular("target foe"),
        Selector::TargetAlly => Phrase::singular("target ally"),
        Selector::TargetOtherAlly => Phrase::singular("target other ally"),
        Selector::Other => Phrase::singular("that creature"),
        Selector::Around { of, band, side } => Phrase::plural(format!(
            "{} {} {}",
            match side {
                crate::dsl::Side::Foes => "foes",
                crate::dsl::Side::Allies => "allies",
            },
            match band {
                RangeBand::Adjacent => "adjacent to".to_owned(),
                RangeBand::Earshot => "within earshot of".to_owned(),
                other => format!("within {} of", band_word(*other)),
            },
            render_selector(of)
        )),

        // `Adjacent(x)` means x *and* what is next to it. Which side those
        // neighbours are on follows x: "foes adjacent to target ally" is a
        // real shape, and rendering it as "target ally and adjacent foes"
        // would read as hitting the ally too.
        Selector::Adjacent(inner) => spread(inner, "adjacent"),
        Selector::Nearby(inner) => spread(inner, "nearby"),
        Selector::InTheArea(inner) => spread(inner, "in the area"),
        Selector::Earshot(inner) => {
            Phrase::plural(format!("{} within earshot", render_selector(inner)))
        }
        Selector::SpiritRange(inner) => {
            Phrase::plural(format!("{} within spirit range", render_selector(inner)))
        }
        Selector::InRangeOf(inner) => Phrase::plural(format!(
            "allies within the range of {}",
            render_selector(inner)
        )),

        Selector::Party => Phrase::plural("all party members"),
        Selector::PartyInRange(band) => {
            Phrase::plural(format!("all party members within {}", band_word(*band)))
        }

        Selector::Foes => Phrase::plural("all foes"),
        Selector::Allies => Phrase::plural("all allies"),
        Selector::Spirits => Phrase::plural("spirits"),
        Selector::Minions => Phrase::plural("minions"),
        Selector::Corpse => Phrase::singular("the nearest corpse"),
        Selector::Location => Phrase::singular("that location"),

        Selector::Nearest(inner) => Phrase::singular(format!(
            "the nearest {}",
            singularise(&render_selector(inner))
        )),
        Selector::Filtered { of, filter } => {
            let inner = render_phrase(of);
            Phrase {
                text: format!("{} that {}", inner.text, filter_phrase(filter)),
                plural: inner.plural,
            }
        }
        Selector::Secondary { of, factor } => {
            let percent = (factor * 100.0).round();
            // `Secondary` names the *others* an area skill reaches. Wrapping
            // a spread and rendering it whole would read as though the
            // primary target took the reduced amount too, which is the
            // opposite of what the construct means.
            let text = match of.as_ref() {
                Selector::Adjacent(inner) => {
                    format!("other foes adjacent to {}", render_selector(inner))
                }
                Selector::Nearby(inner) => {
                    format!("other foes near {}", render_selector(inner))
                }
                Selector::InTheArea(inner) => {
                    format!("other foes in the area of {}", render_selector(inner))
                }
                other => format!("other {}", render_selector(other)),
            };
            Phrase::plural(format!("{text} ({percent}% of that)"))
        }
    }
}

/// "x and adjacent foes", or "foes adjacent to x" when x is on the other side.
fn spread(inner: &Selector, word: &str) -> Phrase {
    let subject = render_selector(inner);
    match (inner.is_ally_only(), word) {
        (true, "in the area") => Phrase::plural(format!("foes in the area of {subject}")),
        (true, _) => Phrase::plural(format!("foes {word} to {subject}")),
        (false, "in the area") => Phrase::plural(format!("{subject} and foes in the area")),
        (false, _) => Phrase::plural(format!("{subject} and {word} foes")),
    }
}

fn filter_phrase(filter: &Filter) -> String {
    match filter {
        Filter::Hexed => "is hexed".to_owned(),
        Filter::Enchanted => "is enchanted".to_owned(),
        Filter::HasCondition(condition) => {
            format!("suffers from {}", condition_word(*condition))
        }
        Filter::Casting => "is using a skill".to_owned(),
        Filter::CastingSpell => "is casting a spell".to_owned(),
        Filter::Attacking => "is attacking".to_owned(),
        Filter::Moving => "is moving".to_owned(),
        Filter::KnockedDown => "is knocked down".to_owned(),
        Filter::BelowHealth { percent } => format!("is below {percent}% health"),
        Filter::AboveHealth { percent } => format!("is above {percent}% health"),
        Filter::CreatureType(kind) => format!("is a {kind}"),
        Filter::IsSpirit => "is a spirit".to_owned(),
        Filter::IsSummoned => "is a summoned creature".to_owned(),
        Filter::IsMinion => "is a minion".to_owned(),
        Filter::HoldingMartialWeapon => "holds a martial weapon".to_owned(),
        Filter::HoldingCasterWeapon => "holds a caster weapon".to_owned(),
        Filter::Owned => "you control".to_owned(),
        Filter::Hostile => "is hostile".to_owned(),
        Filter::Allied => "is allied".to_owned(),
        Filter::RechargingSkills { at_least } => {
            format!("has {at_least} or more skills recharging")
        }
        Filter::ControllingMinions { at_least } => {
            format!("controls {at_least} or more minions")
        }
        Filter::ControllingSpirits { at_least } => {
            format!("controls {at_least} or more spirits")
        }
        Filter::ExploitsCorpse => "exploits a corpse".to_owned(),
        Filter::Removed(kind) => format!("has just lost {}", with_article(effect_word(*kind))),
        Filter::SpiritsInEarshot { at_least } => match at_least {
            1 => "has a spirit within earshot".to_owned(),
            n => format!("has {n} or more spirits within earshot"),
        },
        Filter::CorpsesInEarshot { at_least } => match at_least {
            1 => "has a corpse within earshot".to_owned(),
            n => format!("has {n} or more corpses within earshot"),
        },
        Filter::NearAllies => "is near one of your allies".to_owned(),
        Filter::Not(inner) => negate(&filter_phrase(inner)),
        Filter::All(filters) => filters
            .iter()
            .map(filter_phrase)
            .collect::<Vec<_>>()
            .join(" and "),
        Filter::Any(filters) => filters
            .iter()
            .map(filter_phrase)
            .collect::<Vec<_>>()
            .join(" or "),
    }
}

/// Negates a filter phrase without producing "does not is".
fn negate(phrase: &str) -> String {
    for (prefix, replacement) in [
        ("is ", "is not "),
        ("has ", "does not have "),
        ("holds ", "does not hold "),
        ("suffers ", "does not suffer "),
        ("controls ", "does not control "),
        ("exploits ", "does not exploit "),
    ] {
        if let Some(rest) = phrase.strip_prefix(prefix) {
            return format!("{replacement}{rest}");
        }
    }
    format!("does not {phrase}")
}

fn event_phrase(event: Event) -> &'static str {
    match event {
        Event::OnSkillActivationStart => "a skill starts",
        Event::OnSkillActivationEnd => "a skill finishes",
        Event::OnSkillUsed => "a skill is used",
        Event::OnSpellCast => "a spell is cast",
        Event::OnAttack => "an attack is made",
        Event::OnHit => "an attack hits",
        Event::OnBlocked => "an attack is blocked",
        Event::OnMiss => "an attack misses",
        Event::OnDamageTaken => "damage is taken",
        Event::OnDamageDealt => "damage is dealt",
        Event::OnStruck => "it is hit by an attack",
        Event::OnHeal => "healing happens",
        Event::OnEffectApplied => "an effect is applied",
        Event::OnEffectRemoved => "an effect is removed",
        Event::OnEffectEnded => "an effect ends",
        Event::OnConditionRemoved => "a condition is removed",
        Event::OnEnchantmentRemoved => "an enchantment is removed",
        Event::OnInterrupted => "a skill is interrupted",
        Event::OnKnockedDown => "a knockdown happens",
        Event::OnDeath => "death happens",
        Event::OnKill => "a kill happens",
        Event::OnExperienceKill => "a kill grants experience",
        Event::OnCreatureCreated => "a creature is created",
        Event::OnSpiritDeath => "a spirit dies",
        Event::OnBundleDropped => "the held item is dropped",
        Event::OnEnergyChanged => "energy changes",
        Event::OnTick => "a second passes",
    }
}

fn damage_word(kind: DamageType) -> &'static str {
    match kind {
        DamageType::Blunt => "blunt",
        DamageType::Piercing => "piercing",
        DamageType::Slashing => "slashing",
        DamageType::Cold => "cold",
        DamageType::Earth => "earth",
        DamageType::Fire => "fire",
        DamageType::Lightning => "lightning",
        DamageType::Chaos => "chaos",
        DamageType::Dark => "dark",
        DamageType::Holy => "holy",
        DamageType::Shadow => "shadow",
    }
}

fn condition_word(condition: crate::core::Condition) -> &'static str {
    use crate::core::Condition as C;
    match condition {
        C::Bleeding => "bleeding",
        C::Blind => "blindness",
        C::Burning => "burning",
        C::CrackedArmor => "cracked armor",
        C::Crippled => "crippling",
        C::Dazed => "daze",
        C::DeepWound => "a deep wound",
        C::Disease => "disease",
        C::Poison => "poison",
        C::Weakness => "weakness",
    }
}

fn effect_word(kind: EffectKind) -> &'static str {
    match kind {
        EffectKind::Hex => "hex",
        EffectKind::Enchantment => "enchantment",
        EffectKind::Stance => "stance",
        EffectKind::Preparation => "preparation",
        EffectKind::Glyph => "glyph",
        EffectKind::WeaponSpell => "weapon spell",
        EffectKind::Form => "form",
        EffectKind::ShoutOrChant => "shout or chant",
        EffectKind::Condition => "condition",
        EffectKind::SpiritAura => "spirit aura",
        EffectKind::AreaEffect => "area effect",
        EffectKind::Environment => "environment effect",
        EffectKind::Consumable => "consumable effect",
        EffectKind::Title => "title effect",
        EffectKind::Blessing => "blessing",
        EffectKind::PartyBonus => "party bonus",
        EffectKind::Bundle => "held item",
        EffectKind::Skill => "effect",
    }
}

fn stat_word(stat: Stat) -> String {
    match stat {
        Stat::Armor => "armor".to_owned(),
        Stat::MovementSpeed => "movement speed".to_owned(),
        Stat::AttackSpeed => "attack speed".to_owned(),
        Stat::ActivationTime => "activation time".to_owned(),
        Stat::Recharge => "recharge".to_owned(),
        Stat::AdrenalineRate => "adrenaline gain".to_owned(),
        Stat::ProjectileSpeed => "projectile speed".to_owned(),
        Stat::BlockChance => "block chance".to_owned(),
        Stat::HealthRegeneration => "health regeneration".to_owned(),
        Stat::EnergyRegeneration => "energy regeneration".to_owned(),
        Stat::MaxEnergy => "maximum energy".to_owned(),
        Stat::MaxHealth => "maximum health".to_owned(),
        Stat::DamageDealt => "damage dealt".to_owned(),
        Stat::DamageTaken => "damage taken".to_owned(),
        Stat::HealingReceived => "healing received".to_owned(),
        Stat::AttributeRank(attribute) => humanise_camel(&format!("{attribute:?}")),
        Stat::ElementalAttributes => "elemental attributes".to_owned(),
        Stat::HealthPerSecond => "health per second".to_owned(),
        Stat::ActivationTimeOf(kind) => format!(
            "activation time on {}",
            pluralise(&humanise_camel(&format!("{kind:?}")))
        ),
    }
}

fn stat_is_plural(stat: Stat) -> bool {
    matches!(stat, Stat::ElementalAttributes)
}

fn quantity_word(quantity: Quantity) -> &'static str {
    match quantity {
        Quantity::EnergyLost => "the energy lost",
        Quantity::HealthLost => "the health lost",
        Quantity::EnergyCost => "the energy cost",
        Quantity::MaxHealth => "maximum health",
        Quantity::CurrentHealth => "current health",
        Quantity::SecondsAlive => "the seconds it was alive",
        Quantity::HexesRemoved => "the hexes removed",
        Quantity::ConditionsRemoved => "the conditions removed",
        Quantity::EnchantmentsRemoved => "the enchantments removed",
        Quantity::CreaturesControlled => "the creatures you control",
        Quantity::SpiritsInEarshot => "the spirits within earshot",
        Quantity::CurrentEnergy => "its energy",
    }
}

fn quantity_unit(quantity: Quantity) -> &'static str {
    match quantity {
        Quantity::EnergyLost => "point of energy lost",
        Quantity::HealthLost => "point of health lost",
        Quantity::EnergyCost => "point of energy cost",
        Quantity::MaxHealth => "point of maximum health",
        Quantity::CurrentHealth => "point of current health",
        Quantity::SecondsAlive => "second it was alive",
        Quantity::HexesRemoved => "hex removed",
        Quantity::ConditionsRemoved => "condition removed",
        Quantity::EnchantmentsRemoved => "enchantment removed",
        Quantity::CreaturesControlled => "creature you control",
        Quantity::SpiritsInEarshot => "spirit within earshot",
        Quantity::CurrentEnergy => "point of its energy",
    }
}

fn source_word(source: DamageSource) -> &'static str {
    match source {
        DamageSource::Spells => "spells",
        DamageSource::Attacks => "attacks",
        DamageSource::FoesWithConditions => "foes suffering a condition",
    }
}

fn band_word(band: RangeBand) -> &'static str {
    match band {
        RangeBand::Touch => "touch range",
        RangeBand::Adjacent => "adjacent range",
        RangeBand::Aoe240 | RangeBand::Nearby => "nearby",
        RangeBand::InTheArea => "the area",
        RangeBand::Spear => "spear range",
        RangeBand::Earshot => "earshot",
        RangeBand::Casting => "casting range",
        RangeBand::Hornbow => "hornbow range",
        RangeBand::Longbow => "longbow range",
        RangeBand::SpiritRange => "spirit range",
        RangeBand::NatureRitual => "nature ritual range",
        RangeBand::Party => "the party area",
    }
}

// ------------------------------------------------------------------ helpers

fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn singularise(text: &str) -> String {
    if let Some(stem) = text.strip_suffix("es")
        && (stem.ends_with('x') || stem.ends_with("ss"))
    {
        return stem.to_owned();
    }
    text.strip_suffix('s').unwrap_or(text).to_owned()
}

fn pluralise(text: &str) -> String {
    if text.ends_with('s') || text.ends_with('x') {
        format!("{text}es")
    } else {
        format!("{text}s")
    }
}

/// Turns a slug into words: `bone-fiend` becomes `bone fiend`.
fn humanise(slug: &str) -> String {
    slug.replace(['-', '_'], " ")
}

/// Turns a variant name into words: `DeathMagic` becomes `Death Magic`.
fn humanise_camel(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (index, character) in name.chars().enumerate() {
        if index > 0 && character.is_uppercase() {
            out.push(' ');
        }
        out.push(character);
    }
    out
}

/// "an enchantment", "a hex".
fn with_article(noun: &str) -> String {
    match noun.chars().next() {
        Some('a' | 'e' | 'i' | 'o' | 'u') => format!("an {noun}"),
        _ => format!("a {noun}"),
    }
}
