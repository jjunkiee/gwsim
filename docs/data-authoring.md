Written for: contributors adding or correcting files under `data/`.

# Authoring gwsim data

You do not need to write Rust to contribute data. You need a text editor, the
Guild Wars Wiki open in another window, and the patience to record where each
number came from.

Before anything else, read the two rules in
[CONTRIBUTING.md](../CONTRIBUTING.md#ground-rules). The short version:

> **Numbers, names and relationships are welcome. Text is not.** Never paste
> in-game descriptions, wiki editors' prose, or anything from the `Feedback:`
> namespace. The wiki's text is GFDL, which is not compatible with this
> project's licence, so copying it is not something we can accept however
> convenient it would be.

## Check your work

```powershell
cargo run -p gwsim-cli -- data validate
```

It reads `./data` and prints every problem it finds, grouped by file, with a
line and column or a field path. It exits 0 when the data is usable and 1 when
it is not. **Warnings alone still exit 0** — they mean "this will work, but you
should know something".

`--data-dir <path>` checks a tree somewhere else. `--format json` prints the
same findings as an array of objects, for editors and scripts.

The command reports *everything* in one pass, so work through the list rather
than fixing one problem and re-running.

## Where files go

```text
data/
├── core/           the game's fixed vocabulary: professions, attributes,
│                   conditions, ranges, levels, modes, titles
├── skills/<profession>/   one file per skill, in its profession's folder
│   ├── common/     PvE-only skills, and skills with no profession
│   └── monster/    skills only foes have
├── items/          armor, runes, insignias, weapons, weapon upgrades,
│                   consumables — one file per kind, many records each
├── creatures/
│   ├── foes/<affiliation>/   one file per foe
│   └── heroes.ron, minions.ron, spirits.ron
├── encounters/
│   ├── generic/    training dummies and archetype groups
│   └── curated/<campaign>/<area>/   real fights from the game
├── situations/     an encounter plus the conditions it is fought under
├── situation_sets/ weighted collections of situations
├── benchmarks/     frozen builds from outside sources, for comparison
└── assumptions.ron the register of everything the wiki does not document
```

The loader rejects a file in a folder that is not on this list, so a typo in a
path fails loudly rather than being ignored.

## Naming files

**A file is named after what it contains.** The name is the entity's `name`
field, lowercased, with spaces turned into hyphens and punctuation dropped:

| Name | File |
| --- | --- |
| `Energy Surge` | `energy-surge.ron` |
| `"Fall Back!"` | `fall-back.ron` |
| `Ancestors' Rage` | `ancestors-rage.ron` |
| `Protective Was Kaolai` | `protective-was-kaolai.ron` |

That derived name is the **slug**, and it is how other files refer to this one.
The validator checks the file name against the contents and tells you what to
rename it to if they disagree.

Two entities of the same kind cannot share a name, because the name is the key.

## The provenance block

Every entity file ends with one. It is not paperwork: it is what lets the next
reader tell a checked value from a guess.

```ron
provenance: (
    sources: ["https://wiki.guildwars.com/wiki/Energy_Surge"],
    crawled: "2026-09-22",
    review: Draft,
    reviewed_by: None,
    assumptions: [],
    notes: "",
),
```

| Field | What it means |
| --- | --- |
| `sources` | The wiki pages these values came from. At least one. |
| `crawled` | When you read them, as `YYYY-MM-DD`. |
| `review` | How far the file has been checked. See below. |
| `reviewed_by` | Who checked it. Required once `review` is `Reviewed`. |
| `assumptions` | Ids from `assumptions.ron` this file leans on. |
| `notes` | Anything the next reader needs to know. |

## Review statuses

| Status | Means | The evaluator | The optimiser |
| --- | --- | --- | --- |
| `NumbersOnly` | Values are in; nobody has encoded what the skill *does*. | Refuses to run it | Ignores it |
| `Draft` | Encoded, but the encoding has not been checked. | Runs it, flagged | Uses it, flagged |
| `Reviewed` | Someone compared the encoding against the game. | Runs it | Uses it |

**Only raise a status when you have actually done the work.** A wrongly
`Reviewed` skill is worse than an honest `Draft`, because it stops anyone
looking again. The validator enforces the parts it can:

- a `NumbersOnly` file must have no encoding;
- a `Draft` or `Reviewed` file must have one;
- a `Reviewed` file must name its reviewer.

## Numbers that change with rank

The wiki publishes three points on every scaled value, and so do we:

```ron
extracted: (
    scaled: [
        (label: "energy lost", r0: 1, r12: 8, r15: 10),
    ],
),
```

The validator checks that `r12` agrees with `r0` and `r15` under the usual
rule, `round(r0 + 12 × (r15 − r0) / 15)`. When it does not, either the wiki has
a typo or the skill genuinely rounds differently — **check the page before
assuming which.** If the skill really is unusual, add `special_rounding: true`
and the mismatch becomes a warning instead of an error.

`label` is your own short description of what the number is. It is not the
wiki's wording.

## Durations

Written in seconds, as the wiki states them, and stored as whole milliseconds:

```ron
activation: 0.75,
recharge: 20.0,
```

A value that is not a whole number of milliseconds is **rejected** rather than
rounded, so the file and the engine cannot disagree about what the file says.

## When the wiki does not say

Do not invent a number. Add an entry to `data/assumptions.ron`, give it an id,
and reference that id from the file's `provenance.assumptions`:

```ron
(
    id: "A-004",
    statement: "Foe weapon damage and attack interval.",
    value: Pending,
    rationale: "Creature pages do not record weapons.",
    status: Assumed,
),
```

`value: Pending` is a real answer — it says "nobody has worked this out yet",
which is different from, and more useful than, a placeholder zero. Anything
depending on a `Pending` assumption gets a warning saying the result is
uncalibrated.

Every run reports which assumptions it relied on, so a result built on guesses
says so.

## References between files

Files point at each other by slug: a foe names its skills, an encounter names
its foes, a situation names its encounter. The validator resolves every one of
them and tells you which file holds the broken reference.

If a file fails to parse, the validator **does not** also complain that
everything pointing at it is dangling — fix the parse error and the references
will resolve.

## Placeholders

These sections arrive with the work packages that define them:

### The effect DSL

**See [docs/effect-dsl.md](effect-dsl.md)** for every construct, what it means,
and when to reach for a Rust handler instead.

```powershell
cargo run -p gwsim-cli -- data describe <skill>
```

prints the sentence your encoding generates. Read it next to the wiki page: if
the two disagree, the encoding is almost always what is wrong.

### Handlers

*Arrives in WP4.1.*

Some skills are too unusual for the DSL and need Rust. They are referenced by
name from `encoding.handler`, and the name is checked against the engine's
registry.

### Encounters and situations

*Arrives in WP4.8.*

The schemas exist and validate today; what is missing is guidance on choosing
group composition, positions and timeouts.
