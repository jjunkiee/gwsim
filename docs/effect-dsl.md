Written for: contributors encoding skills into `data/skills/`.

# The effect DSL

A skill's `encoding.effects` says what the skill **does**, as data. gwsim reads
it to simulate the skill and to generate the skill's description, so the
encoding is the single source of truth for both.

Every construct here exists because one of the 72 M1 skills needed it
([T1.5.1](findings/T1.5.1-dsl-coverage.md)). Nothing is here on speculation,
and the list grows as new skills need it.

> **You will need Rust for fewer skills than you expect.** The design predicted
> 16 of the M1 skills would need a hand-written handler. Four do. Before
> reaching for one, check whether a `Triggered` effect says what you mean —
> that construct alone turned twelve would-be handlers into ordinary data.

## Check your work

```powershell
cargo run -p gwsim-cli -- data validate
cargo run -p gwsim-cli -- data describe energy-surge
```

`gwsim` is a workspace binary, so it is run through `cargo run` unless you have
installed it with `cargo install --path crates/gwsim-cli`.

`describe` prints the sentence your encoding generates. **Read it next to the
wiki page.** If the two disagree, the encoding is almost always what is wrong.
Add `--rank 12` to see the numbers at a rank instead of as ranges.

> **`data/skills/` is empty today.** The schemas, the DSL and the renderer are
> built, but no skill has been written yet: the extractor seeds them in P2 and
> they are encoded in WP4.1. Until then `data describe` has nothing to find,
> and the way to see the renderer's output is the golden tests in
> `crates/gwsim-data/tests/descriptions.rs`.

## Values

A number a skill uses.

| Construct | Means | Example |
| --- | --- | --- |
| `Fixed(n)` | The same at every rank | `Fixed(20)` |
| `Scaled(at0, at15)` | Scales on the skill's own attribute | `Scaled(7, 112)` |
| `ScaledBy(attr, at0, at15)` | Scales on a different attribute | `ScaledBy(SoulReaping, 1, 5)` |
| `TitleScaled(track, r0, rmax)` | Scales on a title rank | `TitleScaled(Asura, 10, 50)` |
| `Percent(n)` | A bare percentage | `Percent(75.0)` |
| `PercentOf(percent, of)` | A percentage of something named | `PercentOf(percent: 200.0, of: EnergyCost)` |
| `PerUnit(value, of)` | Once per unit of something | `PerUnit(value: Fixed(7), of: EnergyLost)` |
| `Min(a, b)` | How a stated **maximum** is written | `Min(PerUnit(...), Scaled(15, 80))` |
| `Max(a, b)` | How a stated **minimum** is written | `Max(Fixed(5), Scaled(3, 12))` |
| `Sum([...])` | Several values added | |

`Scaled` interpolates: `round(at0 + rank × (at15 − at0) / 15)`. It keeps going
past rank 15, up to the cap of 20.

**Quantities** a `PerUnit` or `PercentOf` can count: `EnergyLost`,
`HealthLost`, `EnergyCost`, `MaxHealth`, `CurrentHealth`, `SecondsAlive`,
`HexesRemoved`, `ConditionsRemoved`, `EnchantmentsRemoved`,
`CreaturesControlled`, `SpiritsInEarshot`.

```ron
// "target foe takes 7 damage per point of energy lost"
Damage(to: TargetFoe, amount: PerUnit(value: Fixed(7), of: EnergyLost))
```

## Selectors

Who an action reaches.

| Construct | Renders as |
| --- | --- |
| `Self` | you |
| `TargetFoe`, `TargetAlly`, `TargetOtherAlly` | target foe, target ally, … |
| `Adjacent(x)`, `Nearby(x)`, `InTheArea(x)` | x **and** its neighbours |
| `Earshot(x)`, `SpiritRange(x)` | x within earshot / spirit range |
| `InRangeOf(x)` | allies within **x's own** range — every binding ritual |
| `Party`, `PartyInRange(band)` | all party members |
| `Foes`, `Allies`, `Spirits`, `Minions` | everything of that kind |
| `Nearest(x)` | exactly one, the closest |
| `Filtered(of, filter)` | x narrowed by a test |
| `Secondary(of, factor)` | the **others**, taking a share |

**`Adjacent(x)` includes x.** `Adjacent(TargetFoe)` is "target foe and adjacent
foes". When `x` is an ally, the neighbours are foes and the ally is *not*
included: `Adjacent(TargetAlly)` renders as "foes adjacent to target ally",
which is what a skill like Ancestors' Rage does.

**`Secondary` names the others, not the whole group.** An area skill that hits
its target fully and nearby foes for 75% is **two** actions:

```ron
effects: [
    Damage(to: TargetFoe, amount: Scaled(10, 80)),
    Damage(to: Secondary(of: Nearby(TargetFoe), factor: 0.75), amount: Scaled(10, 80)),
],
```

Seven M1 skills have this shape, so it is worth getting into your fingers.

### Filters

`Hexed` · `Enchanted` · `HasCondition(c)` · `Casting` · `CastingSpell` ·
`Attacking` · `Moving` · `KnockedDown` · `BelowHealth(percent)` ·
`AboveHealth(percent)` · `CreatureType(name)` · `IsSpirit` · `IsSummoned` ·
`IsMinion` · `HoldingMartialWeapon` · `HoldingCasterWeapon` · `Owned` ·
`Hostile` · `Allied` · `RechargingSkills(at_least)` ·
`ControllingMinions(at_least)` · `ControllingSpirits(at_least)` ·
`ExploitsCorpse`

Combine them with `Not(f)`, `All([...])` and `Any([...])`.

## Actions

**Damage and health:** `Damage` · `LifeSteal` · `HealthLoss` · `Heal` ·
`HealthGain` · `SacrificeHealth`

**Resources:** `GainEnergy` · `LoseEnergy` · `DrainEnergy` · `GainAdrenaline` ·
`LoseAdrenaline`

**Effects:** `ApplyCondition` · `RemoveConditions` · `ApplyEffect` ·
`RemoveEffects`

**Interference:** `Interrupt` · `FailSkill` · `KnockDown` · `DisableSkills` ·
`ModifyRecharge` · `RechargeSkill`

**Creation:** `Summon` · `CreateSpirit` · `CreateArea` · `HoldBundle` ·
`DropBundle`

**Movement and revival:** `Resurrect` · `ShadowStep` · `Teleport`

**Stats:** `ModifyStat` · `SetStat` · `ReduceIncomingDamage` ·
`SetUnblockable` · `SetCriticalImmune`

**Escape hatch:** `RunHandler`

### Three pairs that are easy to confuse

**`Interrupt` is not `FailSkill`.** An interrupted skill loses its cost and
**starts recharging**; a failed one loses its cost and **recharges instantly**
(§10.5 ENG-14). Mistrust makes a spell *fail*; Power Spike *interrupts* one.

**`ModifyStat` is not `SetStat`.** Masochism *adds* +2 to two attributes;
Master of Magic *sets* four attributes to a value. Adding where the game sets
gives a build whatever it already had, plus the value.

**`ReduceIncomingDamage` has four shapes**, and they are not interchangeable:

```ron
// flat: Shielding Hands reduces each hit by an amount
ReduceIncomingDamage(to: TargetAlly, flat: Some(Scaled(3, 18)))
// percent: Armor of Unfeeling halves damage
ReduceIncomingDamage(to: Self, percent: Some(Fixed(50)))
// scoped: Veil of Thorns reduces spell damage only
ReduceIncomingDamage(to: Self, percent: Some(Scaled(5, 35)), only_from: Some(Spells))
// capped: Shelter limits a single hit as a share of maximum health
ReduceIncomingDamage(to: InRangeOf(Spirits), cap_percent_of_max_health: Some(Fixed(10)))
```

## Control

| Construct | Means |
| --- | --- |
| `If(condition, of, then, otherwise)` | A test on a target's state |
| `ForEach(selector, actions)` | Once per creature |
| `Chance(percent, actions)` | A roll |
| `Sequence([...])` | Several actions as one |
| `Triggered(event, filter, actions, charges)` | **Wait for something, then act** |

Wrap any of these in `Control(...)` to use it where an action is expected.

**`Triggered` is the most useful construct in the DSL.** It is what makes a hex
that punishes a foe for casting, or a weapon spell with five charges, ordinary
data rather than Rust.

```ron
// "the next spell target foe casts fails and deals damage"
Triggered(
    event: OnSpellCast,
    charges: Some(1),
    actions: [
        FailSkill(to: Target),
        Damage(to: TargetFoe, amount: Scaled(10, 80)),
    ],
)
```

Omit `charges` for a trigger with no limit.

### Events

`OnSkillActivationStart` · `OnSkillActivationEnd` · `OnSkillUsed` ·
`OnSpellCast` · `OnAttack` · `OnHit` · `OnBlocked` · `OnMiss` ·
`OnDamageTaken` · `OnDamageDealt` · `OnHeal` · `OnEffectApplied` ·
`OnEffectRemoved` · `OnEffectEnded` · `OnConditionRemoved` ·
`OnEnchantmentRemoved` · `OnInterrupted` · `OnKnockedDown` · `OnDeath` ·
`OnKill` · `OnExperienceKill` · `OnCreatureCreated` · `OnSpiritDeath` ·
`OnBundleDropped` · `OnEnergyChanged` · `OnTick`

## Effect definitions

Anything that lasts — a hex, an enchantment, a stance — is an `EffectDef` in
the skill's `effect_defs`, applied by an `ApplyEffect`.

```ron
encoding: Some((
    effects: [
        ApplyEffect(to: TargetFoe, effect: "life-siphon", duration: Scaled(12, 24)),
    ],
    effect_defs: [
        (
            id: "life-siphon",
            kind: Hex,
            while_active: [
                ModifyStat(to: Target, stat: HealthRegeneration, amount: Scaled(-1, -3)),
                ModifyStat(to: Self, stat: HealthRegeneration, amount: Scaled(1, 3)),
            ],
        ),
    ],
)),
```

| Field | Means |
| --- | --- |
| `id` | Local to the skill, or a global slug (containing `:`) |
| `kind` | Hex, Enchantment, Stance, … — decides removal and stacking |
| `stacking` | A key and one of `Replace`, `KeepLonger`, `StackBySource` |
| `duration` | How long, when it is not the `ApplyEffect`'s own |
| `while_active` | What holds while it is up |
| `triggers` | `Triggered` controls |
| `on_end` | What happens when it ends |
| `upkeep` | Energy pips, for maintained enchantments |

### A delay is an effect with an empty body

Two M1 skills do nothing for a while and then act. **There is no `Delay`
action**, deliberately — an empty `while_active` with a populated `on_end`
already says it, and two ways to say one thing is worse than one:

```ron
// Ancestors' Rage: one second, then area damage.
(
    id: "ancestors-rage",
    kind: Enchantment,
    duration: Some(Fixed(1)),
    on_end: [
        Damage(to: Adjacent(TargetAlly), kind: Some(Lightning), amount: Scaled(5, 110)),
    ],
)
```

## When to use a handler instead

A handler is Rust, registered by name and referenced as
`handler: Some((name: "arcane_echo"))`. It is the escape hatch, and it is
expensive: a handler needs unit tests, a `describe` implementation, and its own
AI hints, and none of it is reviewable by someone who does not read Rust.

**Reach for one only when the DSL genuinely cannot say it.** The four M1 skills
that need one, and why:

| Skill | Why |
| --- | --- |
| Arcane Echo | It changes what is **on the skill bar**. No declarative effect describes that. |
| Air of Superiority | It grants a **random** benefit from a set. |
| Soul Twisting | It changes the **cost and recharge of a skill category**, and counts uses. |
| Master of Magic | It **sets** attributes *and* changes energy returned per spell, interacting with attunements. |

Everything else in M1 — including every binding ritual, every hex with a
trigger, and every minion and resurrection skill — is data.

If you think a skill needs a handler, write the sentence you want
`data describe` to produce. If the DSL can nearly say it, propose the missing
construct in an issue rather than writing a handler: a construct helps every
future skill, a handler helps one.

## What the validator checks

- a `Scaled` value on a skill with no attribute;
- a `TitleScaled` value on a skill with no `title_track`;
- an `ApplyEffect` naming an effect the skill does not define;
- a duration that is zero or negative at every rank;
- a `Chance` outside 0 to 100;
- a trigger with zero charges;
- a selector that points at the opposite side from the skill's `target`;
- two effect definitions sharing an id;
- a handler name that is not registered (when a registry is supplied).

Each of these produces a skill that loads and runs and quietly does the wrong
thing, which is why they are errors rather than warnings.
