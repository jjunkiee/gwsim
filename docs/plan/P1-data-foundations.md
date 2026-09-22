# P1 Data foundations

**Phase goal:** make `gwsim-data` able to represent, load, validate, describe and package all the static game data M0 and M1 need. The core tables hold verified wiki values, template codes round-trip, and the derived-stat formulas reproduce the wiki's worked examples. After P1, the engine (P3) and the extractor (P2) have a stable data format to build on.

- **Design refs:** §6.4, §7, §8, §10.4, §14 #1, §17.1–§17.3, §21, D27, D28, Q22, Q25, Q26.
- **Starts when:** P0 is done.
- **Ends when:**
  - all seven work packages meet their done criteria;
  - `gwsim data validate`, `gwsim data coverage`, `gwsim data describe` and `gwsim template decode` all work;
  - the binary runs with no `data/` directory present.
- **Parallel work:**
  - P2 can start once WP1.2 is done.
  - P3 can start once WP1.1 to WP1.6 are done. WP1.7 can finish alongside P3.
- **Suggested order:**
  1. WP1.1;
  2. WP1.2;
  3. WP1.3, WP1.4 and WP1.5 in any order (1.4 uses 1.3 for T1.4.9);
  4. WP1.6;
  5. WP1.7.

| WP | Title | Goal | Status |
| --- | --- | --- | --- |
| 1.1 | Core types | Professions, attributes, conditions, ranges, levels, modes and titles exist as types and verified `data/core/*.ron` files. | Done |
| 1.2 | Schemas, loader and validation | Every data file kind has a schema, loads with precise errors, and is cross-checked by `gwsim data validate`. | Done |
| 1.3 | Template codec | Skill (type 14) and equipment (type 15) template codes encode and decode exactly. | Done |
| 1.4 | Derived stats | Pure functions compute a build's ranks, health, energy, armor and skill values, matching the wiki's examples. | Done |
| 1.5 | Effect DSL v0 and renderer | A typed effect language can encode the M1 formulaic and conditional skills and renders them as English. | Owner review |
| 1.6 | Assumptions and coverage | The assumptions register is data that code reads by ID, and coverage is reported. | Done |
| 1.7 | Data pack and user data | Binaries embed a validated data pack and load user files from `%APPDATA%\gwsim\`. | Done |

---

## WP1.1 Core types

**Goal:** the fixed vocabulary of the game exists as Rust types, and its numbers sit in verified `data/core/*.ron` files. The vocabulary covers professions, attributes, damage types, skill types, conditions, range bands, levels, modes and title tracks.

- **Refs:** §4 (Gwinch, Pip), §7.1, §8.1, §10.12.
- **Depends on:** P0.
- **Done when:** every `core/*.ron` file loads into `CoreData`, and the tests confirm completeness and the template indices.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T1.1.1 | Research the core values | Research | — | Done |
| T1.1.2 | Core enums and value types | Build | — | Done |
| T1.1.3 | Skill type hierarchy | Build | T1.1.2 | Done |
| T1.1.4 | Write `data/core/*.ron` | Data | T1.1.1, T1.1.2 | Done |
| T1.1.5 | Core loader and tests | Build | T1.1.3, T1.1.4 | Done |

> **Notes on what was built (2026-09-22).** Three departures from this work
> package as written, none of which change its goal:
>
> 1. **`Provenance`, `ReviewStatus`, `AssumptionId` and `IsoDate` landed here, not
>    in T1.2.2.** T1.1.4 requires a provenance block on every core file, and a
>    provenance block needs a date type and assumption ids, so WP1.2 cannot be the
>    first to define them. T1.2.2 still adds `SkillId`, `Slug`, `WikiTitle` and
>    `slugify` to the same module.
> 2. **An attribute's inherent effect is a list, not a single tag.** Dagger Mastery
>    has two — it scales dagger damage like every weapon mastery *and* grants the
>    double-strike chance — so a single tag would have silently dropped one.
> 3. **The core files are `review: Draft`, not `Reviewed`.** T1.1.4 asks for
>    `Reviewed`, but §8.7 defines that as "reviewed by the owner or a second
>    contributor", and nobody has yet. The values are cross-checked against the
>    wiki's own worked tables in T1.1.5, which is what `Draft` claims. The owner
>    promotes them.
>
> Also settled here: **A-032 stays `Pending`.** The plan allowed T1.1.1 to set the
> hard-mode recharge reduction, but the Hard mode page says only "shorter
> recharges" and gives no number, so there is nothing to read. It passes to T4.2.1.

### T1.1.1 Research the core values

**Type:** Research · **Depends on:** —

Using `research/` first (the Attributes, Game_mechanics, Game_Mode, Range_types and Effects folders) and the wiki only to fill gaps, record the following. Record each value with its source page.

1. **Professions** (all 10):
   - name and abbreviation;
   - template index (1–10);
   - primary attribute and attribute list;
   - base armor rating and any armor bonus against particular damage types;
   - armor energy and armor energy regeneration (the §10.4 "0/5/10" and "+0/+1/+2");
   - foe energy by profession (§10.4: W 20, R 30, Mo 40, …).
2. **Attributes:** all 42 (template IDs 0–25 and 29–44; 26–28 are unused). For each: profession, primary flag, and a one-line description of its inherent effect in our own words.
3. **Range bands** (from the Range page): touch 144, adjacent 166 (A-024), nearby 252, in the area 322, earshot 1012, casting 1248, longbow 1498, spirit 2512, nature rituals 3000 (A-018), party 5020. Also record any other named bands used by M1 skills, such as "half range" and the 240 AoE (A-025).
4. **Levels:**
   - character health per level;
   - foe health (`20 × level + 80`) and the HM bonus;
   - foe base armor by level (`3 × level + profession bonus`);
   - the skill-damage level multiplier (§10.7);
   - the HM level-mapping tables for non-boss foes, bosses and allies (from the Hard mode page, as in `research/Game_Mode/Hard_mode.md`).
5. **Conditions** (all 10):
   - effect values (Bleeding −3, Burning −7, Poison −4 and Disease −4 regeneration; Blind 90% miss; Crippled −50% speed; Dazed; Deep Wound; Weakness; Cracked Armor);
   - which conditions affect only fleshy creatures;
   - spirit immunity.
6. **Modes:**
   - HM rules: movement about +33%, attack speed about +25%, halved activation over 2 s, and shorter recharges (A-032: look for the actual number now);
   - Reforged Mode rules;
   - Melandru's Accord and Dhuum's Covenant rules (from the Optional game modes page).
7. **Title tracks:** the Asura rank → effective-rank table for PvE-only skills, needed for Air of Superiority (A-020). Record the table's structure so other tracks fit it later.
8. **Health formula.** Characters and foes share one formula: `100 + 20 × (level − 1)`, which equals `20 × level + 80` (480 at level 20; Level page). Confirm it and record it once in `levels.ron`.

- **Output:** `docs/findings/T1.1.1-core-values.md`, with one table per core file.
- **Done when:** every field that T1.1.4 needs has a sourced value or an assumption ID.

### T1.1.2 Core enums and value types

**Type:** Build · **Depends on:** —

In `crates/gwsim-data/src/core/`:

1. **Enums:**
   - `Profession`: 10 variants, with methods `template_index() -> u8`, `from_template_index(u8) -> Option<Self>` and `abbrev() -> &'static str`. A missing secondary is `Option<Profession>`.
   - `Attribute`: 42 variants, with `template_id()`, `from_template_id()` (returns `None` for 26–28) and `profession()`.
   - `DamageType`: `Blunt`, `Piercing`, `Slashing`, `Cold`, `Earth`, `Fire`, `Lightning`, `Chaos`, `Dark`, `Holy`, `Shadow`. `Option<DamageType>` stands for typeless damage. Add a method `is_physical()` and a method `is_elemental()`.
   - `Condition`: 10 variants.
   - `RangeBand`: the named bands above, with `gwinches() -> f32` read from `ranges.ron`, not hard-coded.
   - `Campaign`: `Core`, `Prophecies`, `Factions`, `Nightfall`, `EyeOfTheNorth`.
   - `TitleTrack`: start with `Asura`; the others are added in WP5.7 and WP7.5.
2. **Serialisation:** derive `serde::{Serialize, Deserialize}`, `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `PartialOrd` and `Ord` where sensible. RON variant names are the Rust variant names.
3. **Principle:** the game's fixed identities live in code, and their numbers live in data. Every numeric property (base armor, energy, ranges) is read from `core/*.ron`.
4. Add `serde` and `ron` to `[workspace.dependencies]`.

- **Output:** the `core` module.
- **Done when:** it compiles, and the unit tests confirm template-index round trips for every profession and attribute.

### T1.1.3 Skill type hierarchy

**Type:** Build · **Depends on:** T1.1.2

1. Add a `SkillType` enum covering the hierarchy on the wiki's Skill type page (see `research/Skill_types/Skill_type.md`):
   - attack skills and their weapon subtypes (axe, bow, dagger with lead, off-hand and dual, hammer, scythe, spear, sword, pet);
   - spells and their subtypes (enchantment, flash enchantment, hex, item, ward, weapon, well);
   - rituals (binding, nature, Ebon Vanguard);
   - signet, stance, shout, chant, echo, preparation, form, glyph and trap;
   - untyped skill and title.
2. Implement `parent(self) -> Option<SkillType>` and `is_a(self, ancestor) -> bool`. For example, `HexSpell.is_a(Spell)` is true, and `BindingRitual.is_a(Ritual)` is true.
3. Add per-type defaults, to be confirmed in T3.4.1:
   - `default_aftercast_ms()`: 750 for most types; 0 for stances, shouts, flash enchantments and pet attacks;
   - `uses_action_queue()`;
   - `usable_while_knocked_down()`.

   Mark each default with a comment naming the wiki page it comes from.

- **Done when:** the unit tests cover `is_a` for every subtype and the defaults for each top-level type.

### T1.1.4 Write `data/core/*.ron`

**Type:** Data · **Depends on:** T1.1.1, T1.1.2

1. Create the seven core files, each with a `provenance` block (sources, `crawled: "2026-09-22"`, `review: Reviewed`, assumption IDs):

   | File | Contents |
   | --- | --- |
   | `professions.ron` | One record per profession, keyed by the `Profession` variant, with the T1.1.1 fields |
   | `attributes.ron` | One record per attribute: key, primary flag, an inherent-effect tag (e.g. `FastCasting`, `SoulReaping`) for the engine to dispatch on |
   | `conditions.ron` | Per condition: regeneration change, miss chance, speed change, fleshy-only flag, affects spirits (Burning only) |
   | `ranges.ron` | Band → gwinches |
   | `levels.ron` | Formulas as parameters (health base and per-level values, HM bonus per level, foe armor per level, damage-scaling constants), plus the HM level-mapping tables as rows |
   | `modes.ron` | HM modifiers (movement and attack-speed multipliers, activation-halving threshold, recharge reduction as an assumption reference), Reforged Mode modifiers, optional-mode flags |
   | `titles.ron` | The Asura track: rank → effective rank |

2. Write values from the T1.1.1 findings only. Any value the wiki lacks gets an assumption ID instead of a number.

- **Done when:** every file parses (checked in T1.1.5).

### T1.1.5 Core loader and tests

**Type:** Build · **Depends on:** T1.1.3, T1.1.4

1. Implement `CoreData::load(dir) -> Result<CoreData, LoadErrors>`, which reads the seven files and builds lookup tables indexed by enum.
2. Add completeness checks: every `Profession`, `Attribute`, `Condition` and `RangeBand` variant has exactly one record, with no extras and no duplicates.
3. Write the tests:
   - the whole of `data/core/` loads;
   - the template indices match a hard-coded copy of the wiki's Skill template format tables (professions 0–10, attributes 0–44);
   - the ranges are strictly increasing in the expected order;
   - the HM level mapping gives 26 for a level-20 non-boss foe (§20.2).

- **Done when:** `cargo test -p gwsim-data` passes, and `core/*.ron` loads.

---

## WP1.2 RON schemas, loader and validation

**Goal:** every file kind in the §8.1 `data/` layout has a Rust schema. The loader reads a whole tree, reports every problem with its file, position and field path, and checks cross-references and consistency. `gwsim data validate` exposes this to people and to CI.

- **Refs:** §7.1, §8.1–§8.3, §8.9, §12.1, D16, D27.
- **Depends on:** WP1.1.
- **Done when:** each invalid fixture is rejected with a clear, specific error, and the valid fixture tree loads cleanly.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T1.2.1 | RON and serde error reporting | Research | — | Done |
| T1.2.2 | Identifiers, slugs and provenance | Build | T1.2.1 | Done |
| T1.2.3 | Skill file schema | Build | T1.2.2 | Done |
| T1.2.4 | Foe and creature schemas | Build | T1.2.2 | Done |
| T1.2.5 | Item schemas | Build | T1.2.2 | Done |
| T1.2.6 | Encounter, situation, set and benchmark schemas | Build | T1.2.2 | Done |
| T1.2.7 | Tree loader | Build | T1.2.3–T1.2.6 | Done |
| T1.2.8 | Cross-reference and consistency checks | Build | T1.2.7 | Done |
| T1.2.9 | `gwsim data validate` | Build | T1.2.8 | Done |
| T1.2.10 | Invalid-fixture test suite | Test | T1.2.9 | Done |
| T1.2.11 | Data authoring guide v0 | Docs | T1.2.9 | Done |
| T1.2.12 | Enable validation in CI | Setup | T1.2.9 | Done |

> **Notes on what was built (2026-09-22).** Four departures, none changing the goal:
>
> 1. **T1.2.1's premise was wrong for RON 0.12.** The task assumed RON gives no
>    span for a semantic error, so the plan split reporting into positions for
>    syntax and field paths for semantics. Measurement showed 0.12 gives spans
>    for *both*, plus structured error codes carrying the expected and found
>    names. Reporting is therefore one tier, richer than planned, and the codes
>    drive "did you mean ...?" suggestions.
> 2. **Fixtures are built in memory, not as `tests/fixtures/<case>/` trees.**
>    Each invalid case is the valid tree with one thing changed, so the defect
>    is visible in the test rather than buried in a directory diff, and
>    fourteen near-identical trees cannot drift apart as the schemas move.
>    `MemSource` exists for this.
> 3. **The assumptions *schema* landed here, not in T1.6.1.** T1.2.7 has to
>    route `assumptions.ron` and T1.2.8 has to check that every cited id
>    exists. T1.6.1 still adds typed keys and use-recording on top.
> 4. **The CI step runs in debug, not `--release`.** The placeholder said
>    release; using debug reuses the artifacts the test step already built
>    instead of recompiling the workspace a second time.
>
> **Deferred to the work package that defines them:** effect-definition and
> benchmark-skill-id reference checks (T1.2.8 items 1.5 and 1.7) need the DSL
> (WP1.5) and the template codec (WP1.3). Handler-name checking is built, and
> exposed as `checks::check_handlers`, which takes a registry from the caller
> so the data crate never depends on the engine.

### T1.2.1 RON and serde error reporting

**Type:** Research · **Depends on:** —

1. From the `ron` crate docs and source (current version), establish:
   - how parse errors report line and column (`ron::error::SpannedError`);
   - which extensions to enable (`implicit_some`, `unwrap_newtypes`, `unwrap_variant_newtypes`), and whether they can be enabled per file with `#![enable(...)]` or through `ron::Options`;
   - how externally tagged enums and struct-like variants read in RON.
2. Establish how to report semantic errors with a field path, e.g. `effects[2].amount`. Evaluate `serde_path_to_error` with RON's deserializer. Confirm that RON offers no span per field, so the plan is file plus field path for semantic errors and file plus line and column for syntax errors.
3. Check how `#[serde(deny_unknown_fields)]` reports errors in RON (typos in field names must fail).
4. Decide on dates: a small `IsoDate` newtype (YYYY-MM-DD, validated) rather than a date crate.

- **Output:** `docs/findings/T1.2.1-ron-serde.md`, with the decisions.

### T1.2.2 Identifiers, slugs and provenance

**Type:** Build · **Depends on:** T1.2.1

1. Add the newtypes:
   - `SkillId(u16)`: the template-code ID;
   - `Slug(String)`, with validation: lowercase ASCII, digits and single hyphens;
   - `WikiTitle(String)`;
   - `AssumptionId`: parses `A-` followed by three digits.
2. Implement `slugify(title: &str) -> Slug`. Lowercase; spaces and underscores become `-`; drop `"`, `'`, `!`, `?`, `,` and `.`; `&` becomes `and`; collapse repeated hyphens. Tests:

   | Title | Slug |
   | --- | --- |
   | `"Fall Back!"` | `fall-back` |
   | `Ancestors' Rage` | `ancestors-rage` |
   | `Protective Was Kaolai` | `protective-was-kaolai` |
   | `Energy Surge` | `energy-surge` |
   | `"Stand Your Ground!"` | `stand-your-ground` |

3. Implement `WikiTitle::url()`, which returns `https://wiki.guildwars.com/wiki/<Title>`, with spaces turned into underscores and percent-encoding as MediaWiki does.
4. Add `Provenance` (§8.2) with the fields `sources: Vec<WikiTitle or URL>`, `crawled: IsoDate`, `review: ReviewStatus`, `reviewed_by: Option<String>`, `assumptions: Vec<AssumptionId>` and `notes: String`.
5. Add `ReviewStatus` with the values `NumbersOnly`, `Draft` and `Reviewed`.

- **Done when:** the unit tests pass for each type, and the slug table above is covered.

### T1.2.3 Skill file schema

**Type:** Build · **Depends on:** T1.2.2

1. Add a `Skill` struct following §8.3, with `deny_unknown_fields`. Its fields fall into three groups.
2. **Header fields:**
   - `id: SkillId`, `name`, `wiki: WikiTitle`;
   - `profession: Option<Profession>` (`None` for common and monster skills), `attribute: Option<Attribute>`;
   - `kind: SkillType`, `elite`, `pve_only`, `title_track: Option<TitleTrack>`, `campaign`;
   - `cost: Cost { energy, adrenaline, sacrifice_pct, upkeep, overcast }`;
   - `activation: Seconds`, `recharge: Seconds`, `target: TargetKind`, `range: Option<RangeBand>`, `aoe: Option<Aoe>` (band, or explicit radius, for the 240 vs 252 case, A-025), `projectile: Option<ProjectileKind>`;
   - `flags: SkillFlags { easily_interrupted, touch, ... }`.
3. **Extracted numbers:** `extracted: Extracted { scaled: Vec<ScaledNumber { label, r0, r12, r15 }>, fixed: Vec<FixedNumber { label, value }>, description_hash: Option<String> }`. These are the numbers the extractor writes. The encoding must agree with them (§17.3).
4. **Encoding** (`None` while `NumbersOnly`): `encoding: Option<Encoding { effects: Vec<dsl::Action>, effect_defs: Vec<dsl::EffectDef>, handler: Option<HandlerRef { name, params }>, ai: AiHints, roles: Vec<RoleTag> }>`. Until WP1.5 fills in the DSL types, use placeholder DSL types.
5. `provenance: Provenance`.
6. Add the `Seconds` newtype. It deserialises from a RON number (e.g. `0.75`), converts to integer milliseconds with a check that the value is an exact multiple of 1 ms, and exposes `.ms()`.
7. `TargetKind` has the values `Self_`, `Foe`, `Ally`, `OtherAlly`, `AllyOrSelf`, `Spirit`, `Corpse`, `Location`, `None` and `Minion`. Extend it as the WP1.5 study needs.

- **Done when:** an illustrative skill file in the §8.3 shape round-trips (deserialise, serialise, deserialise) to an identical value.

### T1.2.4 Foe and creature schemas

**Type:** Build · **Depends on:** T1.2.2

1. Add a `Foe` struct following §7.1 and §9.3. Fields:
   - `name`, `wiki`, `affiliation`, `species`;
   - `traits: Vec<CreatureTrait>` (`Fleshy`, `Spirit`, `Undead`, `Construct`, …);
   - `professions: (Profession, Option<Profession>)`;
   - `level: ModeValue<u8> { nm, hm }`;
   - `attributes: ModeValue<Vec<(Attribute, u8)>>`, where `hm` is optional and a missing value means "use A-005";
   - `skills: Vec<FoeSkill { skill: SkillRef, hm_only: bool }>`;
   - `variants: Vec<FoeVariant>` for weapon or skill variants (Kournan Guard axe and hammer, A-007);
   - `armor: ArmorTable` (per damage type, with an optional note on the level context);
   - `health_override: Option<u32>`, `energy: Option<u32>`;
   - `weapon: Option<FoeWeapon { weapon_type, damage: Option<(u16, u16)>, attack_interval: Option<Seconds> }>`;
   - `boss`, `ai_tags: Vec<AiTag>` (`Kiter`, `Stationary`, `Script(name)`, …), `provenance`.
2. `SkillRef` accepts either a slug or an ID, and is resolved in T1.2.8.
3. Add the created-creature files (`creatures/heroes.ron`, `minions.ron`, `spirits.ron`) as record lists:
   - hero: name, profession, availability (`Campaign` / `Reforged`), mercenary flag (D17);
   - minion: slug, level rule, armor rule, attack, degeneration rule;
   - spirit: slug, health and armor rules (A-015), range.

   The exact fields are filled in WP4.3.

- **Done when:** the Kournan Seer as described in §20.2 can be written and round-trips.

### T1.2.5 Item schemas

**Type:** Build · **Depends on:** T1.2.2

1. Add a schema for each items file:
   - `items/armor.ron`: profession armor base per piece;
   - `runes.ron`: `Rune { slug, kind: Attribute(attr, tier) | Vigor(tier) | Vitae | Attunement | Clarity | …, bonus, health_penalty, non_stacking_key, template_modifier_id: Option<u16> }`;
   - `insignias.ron`: `Insignia { slug, profession: Option<Profession>, effects: Vec<dsl::Modifier or conditional effect>, template_modifier_id }`;
   - `weapons.ron`: `WeaponType { slug, damage_type, damage_range_at_max_req, attack_interval, range, projectile_speed: Option<…>, mastery: Option<Attribute>, two_handed, energy: Option<u8> }`;
   - `weapon_upgrades.ron`: `WeaponUpgrade { slug, slot: Prefix | Suffix | Inscription | Inherent, effects, conditions, template_modifier_id }`;
   - `consumables.ron`: `Consumable { slug, effect, duration, scope, survives_death (A-019) }`.
2. Where an effect needs the DSL, reference `dsl::Modifier` and the other DSL types (placeholders until WP1.5).

- **Done when:** each file kind has at least one round-trip test.

### T1.2.6 Encounter, situation, set and benchmark schemas

**Type:** Build · **Depends on:** T1.2.2

1. Add `Encounter` following §12.1: name, area, campaign, `groups: Vec<Group { foes: Vec<(FoeRef, variant, count)>, formation: Cluster{radius} | Line{spacing} | Explicit(Vec<Pos>), position, level_override: Option<ModeValue<u8>>, ai_tags }>`, `party_start` and provenance.
2. Add `Situation` following §7.3 and §12.3. Fields:
   - `name`;
   - `encounters: Single(EncounterRef) | Chain(Vec<(EncounterRef, rest_after: Seconds)>)`;
   - `mode: ModeSwitches { hard_mode, reforged_mode, dhuums_covenant, melandrus_accord }`;
   - `party_size`, `consumables`, `starting_dp`, `starting_morale`;
   - `timeout: Option<Seconds>` (A-030 when missing), `tactics_overrides: Option<…>` (placeholder until WP4.7), `notes`, `assumption_refs`.
3. Add `SituationSet { entries: Vec<(SituationRef, weight: f64)> }`.
4. Add `Benchmark` (D16): `name`, `source_url`, `snapshot_url`, `snapshot_date`, and `slots: Vec<BenchmarkSlot { kind, skill_code, equipment_code: Option, notes }>`.

- **Done when:** the §12.1 example encounter round-trips.

### T1.2.7 Tree loader

**Type:** Build · **Depends on:** T1.2.3–T1.2.6

1. Implement `DataSet::load(source: &dyn DataSource) -> Result<DataSet, Vec<DataError>>`. `DataSource` is a trait with two implementations:
   - `DirSource(path)` for `data/`;
   - `MemSource(Vec<(path, contents)>)` for embedded packs and tests.
2. Route files by path, following §8.1: `core/`, `skills/<profession>/`, `skills/common/`, `skills/monster/`, `items/`, `creatures/`, `creatures/foes/<affiliation>/`, `encounters/generic/`, `encounters/curated/<campaign>/<area>/`, `situations/`, `situation_sets/`, `benchmarks/`, `assumptions.ron`. Unknown paths are an error, except `LICENSE` and `*.md`.
3. Parse every file and collect all errors; never stop at the first. `DataError` holds:
   - the file path;
   - the kind (`Syntax { line, col }`, `Schema { field_path }` or `Reference`/`Consistency` from T1.2.8);
   - a message that tells the author what to fix.
4. Check that each entity file's name equals `slugify(name)`, and that its folder matches its profession (skills) or affiliation (foes).
5. Build indexes: skills by ID and by slug, foes by slug, and so on. Report duplicates as errors naming both files.
6. Sort all collections deterministically, by path.

- **Done when:** a valid fixture tree loads, and a tree with three broken files reports all three errors.

### T1.2.8 Cross-reference and consistency checks

**Type:** Build · **Depends on:** T1.2.7

1. **References** (§8.9). Each of these must resolve:
   - foe skill refs;
   - encounter foe refs;
   - situation encounter refs;
   - situation-set situation refs;
   - effect-definition refs inside skills;
   - assumption IDs anywhere;
   - benchmark skill IDs (after decoding, once WP1.3 is done);
   - handler names, against a registry supplied by the caller (the engine), so the data crate doesn't need the engine.
2. **Consistency.** Each of these is an error:
   - a scaled value missing an endpoint;
   - an `extracted.scaled` entry where `r12` doesn't equal `round(r0 + 12 × (r15 − r0)/15)` (a warning instead if the value is marked `special_rounding`);
   - `pve_only` set on a skill outside `skills/common/`;
   - `title_track` set without `pve_only`;
   - a skill marked `elite` in `skills/monster/` without a note;
   - a foe without both NM and HM levels;
   - a `NumbersOnly` skill with an encoding;
   - a `Draft` or `Reviewed` skill without one;
   - a `Reviewed` file without `reviewed_by`.
3. **Warnings** (reported, not fatal):
   - assumptions with `Pending` values that are referenced by data;
   - encounters whose foes use `NumbersOnly` skills (§12.6).

- **Done when:** each rule has a unit test with a minimal fixture.

### T1.2.9 `gwsim data validate`

**Type:** Build · **Depends on:** T1.2.8

1. Add a `data` subcommand group to `gwsim-cli` with `validate [--data-dir <path>] [--format text|json]`. The default directory is `./data` until WP1.7 adds the embedded pack.
2. Text output groups errors by file and shows `path:line:col` for syntax errors and `path: field.path` for schema errors, then a summary line (`N errors, M warnings in K files`).
3. The exit code is 0 when there are no errors and 1 when there are. Warnings alone exit 0.
4. JSON output is an array of `{file, kind, line, col, field_path, message, severity}`.

- **Done when:** running it on `data/` exits 0, and on a broken fixture exits 1 with readable output.

### T1.2.10 Invalid-fixture test suite

**Type:** Test · **Depends on:** T1.2.9

1. Under `crates/gwsim-data/tests/fixtures/`, create:
   - `valid/`: a small tree of core files, two skills, one foe, one encounter, one situation, one set and one benchmark;
   - `invalid/<case>/`: one tree per defect.
2. The defect cases:
   - bad RON syntax;
   - an unknown field;
   - an unknown enum variant;
   - a missing required field;
   - an unknown assumption ID;
   - a dangling foe skill;
   - a dangling encounter foe;
   - a duplicate skill ID;
   - a file name that doesn't match its slug;
   - a scaled value missing an endpoint;
   - `r12` inconsistent with the formula;
   - a `NumbersOnly` skill with an encoding;
   - a `Reviewed` skill without `reviewed_by`;
   - a foe missing its HM level.
3. Snapshot each case's error output with `insta` (add it as a dev-dependency). The snapshots are the "good errors" standard: each must name the file, the position or field, and what to do.

- **Done when:** every invalid case fails with its expected snapshot, and `valid/` passes.

### T1.2.11 Data authoring guide v0

**Type:** Docs · **Depends on:** T1.2.9

1. Create `docs/data-authoring.md` covering:
   - the `data/` layout;
   - file naming (slugs);
   - the provenance block;
   - review statuses and what each allows (§8.7);
   - how to run `gwsim data validate`;
   - the licensing rules (numbers only; no prose).
2. Leave headed placeholders for the DSL (WP1.5), handlers (WP4.1) and encounters (WP4.8).

- **Output:** `docs/data-authoring.md`.

### T1.2.12 Enable validation in CI

**Type:** Setup · **Depends on:** T1.2.9

1. Uncomment the `gwsim data validate` step in `.github/workflows/ci.yml`, running `cargo run -p gwsim-cli -- data validate`.

- **Done when:** the step is active.

---

## WP1.3 Template codec

**Goal:** encode and decode the game's skill (type 14) and equipment (type 15) template codes, reproducing the PvX codes in §20 byte for byte.

- **Refs:** §4 (Template code), §14 #1, §15 (`template`), §17.2 (template decode).
- **Depends on:** WP1.1.
- **Done when:** all seven §20.1 PvX codes decode to the expected professions and attributes, and re-encode to the identical string.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T1.3.1 | Template formats, IDs and test vectors | Research | — | Done |
| T1.3.2 | Base64 alphabet and bit stream | Build | T1.3.1 | Done |
| T1.3.3 | Skill template codec | Build | T1.3.2 | Done |
| T1.3.4 | Equipment template codec | Build | T1.3.2 | Done |
| T1.3.5 | Test vectors and property tests | Test | T1.3.3, T1.3.4 | Done |
| T1.3.6 | `gwsim template decode` / `encode` (raw) | Build | T1.3.5 | Done |

> **Notes on what was built (2026-09-22).** The codec reproduces all seven §20.1
> codes byte for byte. Three things worth carrying forward:
>
> 1. **The attribute width rule has an undocumented floor.** Professions and
>    skills use the smallest width that fits, as expected. Attributes do not:
>    both Mesmer codes store ids no larger than 3, which four bits would hold,
>    yet both use five. `max(1, needed - 4)` reproduces every published code,
>    but it is inferred from two codes that are both Mesmer bars. If a future
>    code fails to round-trip, this is the line to revisit (T1.3.1 §3).
> 2. **Equipment templates describe PvP gear.** The item id list is `PvP Axe`,
>    `PvP Longbow` and so on, and only PvP characters can load one. gwsim
>    simulates PvE, so the equipment codec is for *reading* codes a published
>    build includes, not for describing gwsim gear. Recorded because it changes
>    how much T1.4.9's `to_templates` can promise.
> 3. **Two format traps, both now covered by tests.** Equipment width fields
>    are raw bit counts with no `+4`/`+8` offset, unlike the skill format; and
>    an item's dye sits *between* the modifier count and the modifier list.
>
> **Property tests use a boundary sweep rather than `proptest`.** T1.3.5 asked
> for `proptest`; instead the suite sweeps every width boundary, every
> attribute, every slot and every profession pair, plus a no-panic pass over
> malformed input. That covers where a hand-rolled bit codec actually breaks
> without adding a dependency. `proptest` can be added if a real bug escapes.

### T1.3.1 Template formats, IDs and test vectors

**Type:** Research · **Depends on:** —

1. From `research/Skills/Template.md` and the wiki's Skill template format page, confirm:
   - that the header is `type(4) = 14`, `version(4) = 0`;
   - the profession width rule (`code × 2 + 4`);
   - the attribute width rule (`code + 4`) and the skill width rule (`code + 8`);
   - that the trailing bit is 0;
   - the legacy header (version only, no type);
   - the bit order (lowest bit first within each 6-bit character);
   - the alphabet.
2. Fetch the Equipment template format page. Record the full bit layout and the **Item IDs** and **Modifier IDs** tables for everything M1 uses:
   - weapons: staff, wand, focus, and the Kournan weapon types;
   - runes: superior, major and minor attribute runes, Vigor, Vitae;
   - insignias: Prodigy's, Bloodstained, Minion Master's, Tormentor's, Shaman's;
   - weapon upgrades in a 40/40 set.

   Tables of IDs are facts; record them in our own table format.
3. Record the skill IDs of all 72 M1 skills (§20.3). Use the Skill template format/Skill list page, or the Game integration/Skills pages (at most 8 pages, 3 s apart).
4. Hand-decode the player code `OQBTAUBPQaJ4EY6x0BAAAAAAuE` field by field on paper: expected Mesmer, no secondary, Fast Casting 10, Domination 12, Inspiration 8 (§17.2). Record each field's bit width, so T1.3.3 knows how the game chooses widths and padding.

- **Output:** `docs/findings/T1.3.1-templates.md`, with the bit layouts, ID tables and a worked decode.

### T1.3.2 Base64 alphabet and bit stream

**Type:** Build · **Depends on:** T1.3.1

1. In `gwsim-data::template`, add `BitReader` over a template string:
   - map each character through the standard Base64 alphabet (`A–Z a–z 0–9 + /`) to 6 bits;
   - read `n` bits lowest bit first;
   - report a position-aware error on an invalid character or on running out of data.
2. Add `BitWriter`, which writes `n` bits lowest bit first, pads the final character with zero bits, and returns the string.
3. Hand-roll both: the format is custom, so no base64 crate is needed.

- **Done when:** the unit tests pass: known single characters decode to known bit patterns, and `write(read(x)) == x` holds for every 6-bit value.

### T1.3.3 Skill template codec

**Type:** Build · **Depends on:** T1.3.2

1. Add `SkillTemplate { primary: Profession, secondary: Option<Profession>, attributes: Vec<(Attribute, u8)>, skills: [Option<SkillId>; 8] }`.
2. `decode(&str)`:
   - accepts type 14 and the legacy header;
   - rejects other types with a clear error (e.g. "this is an equipment template");
   - validates profession indices, attribute IDs (26–28 invalid) and ranks 0–12;
   - preserves attribute order.
3. `encode(&SkillTemplate)`:
   - chooses bit widths the way the game does, as established in T1.3.1 (the smallest width code that fits the largest value, or whatever rule reproduces the PvX strings);
   - writes the fields in order, then the trailing 0 bit, then the padding.
4. Keep the codec free of `DataSet` lookups: skill IDs stay raw here, and name resolution happens in T1.4.9.

- **Done when:** the player code decodes to the expected values and re-encodes to the identical string.

### T1.3.4 Equipment template codec

**Type:** Build · **Depends on:** T1.3.2

1. Add `EquipmentTemplate { items: Vec<EquipmentItem { slot: Weapon | OffHand | Chest | Legs | Head | Feet | Hands, item_id: u32, dye: u8, modifiers: Vec<u32> }> }`, with the widths from the header.
2. Implement decode and encode, validating slot values and dye values per the wiki table (0 and 10–13 are preview-only; 1, 14 and 15 are invalid).
3. Add `template_item_id` and `template_modifier_id` fields to the item schemas (T1.2.5), so T1.4.9 can map IDs to gear. Keep unknown IDs as raw numbers, so an unknown code still round-trips.

- **Done when:** a hand-built equipment template round-trips. There are no PvX equipment codes; vectors come from the T1.3.1 worked layout.

### T1.3.5 Test vectors and property tests

**Type:** Test · **Depends on:** T1.3.3, T1.3.4

1. Build a test table of the seven §20.1 codes. For each code, assert:
   - the expected primary and secondary professions;
   - the expected attribute ranks, from points only (e.g. player: Domination 12, Fast Casting 10, Inspiration 8);
   - the expected skill IDs, from T1.3.1;
   - that re-encoding gives the identical string.

   The hero codes encode one choice of the PvX alternatives (§20.1). Assert what the code contains, not the [Proposed] picks.
2. Add `proptest` property tests: `decode(encode(t)) == t` for random valid templates, and `decode` never panics on random strings.
3. Add error tests: a wrong type nibble, a truncated string, an invalid character, and attribute ID 27.

- **Done when:** all the tests pass.

### T1.3.6 `gwsim template decode` / `encode` (raw)

**Type:** Build · **Depends on:** T1.3.5

1. `gwsim template decode <code> [--json]`:
   - detects skill or equipment templates from the type nibble;
   - prints professions, attributes and ranks, and skills;
   - shows each skill as its raw ID plus its name when the data set has it (`#1234 Energy Surge`, or `#1234 (unknown)`).
2. `gwsim template encode <file.ron>` accepts a RON `SkillTemplate`. Encoding a full build file arrives in T1.4.9.
3. Add CLI integration tests for both.

- **Done when:** `gwsim template decode OQBTAUBPQaJ4EY6x0BAAAAAAuE` prints the player's professions and attributes.

---

## WP1.4 Derived stats

**Goal:** pure functions, needing no timeline, that turn a build and a level into:

- effective attribute ranks;
- maximum health;
- energy and energy regeneration;
- armor per piece and damage type;
- a table of every scaled skill value.

They must reproduce the wiki's worked examples. The work package also defines the `Build` type and its legality rules.

- **Refs:** §7.2, §10.4, §10.9, §17.2, OPT-1 (legality), A-020, A-033.
- **Depends on:** WP1.1, WP1.2; WP1.3 (for T1.4.9 only).
- **Done when:** the §17.2 rows assigned to WP1.4 (plan README) pass, and every M1 party member's derived health, energy and ranks match the hand values in T1.4.1.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T1.4.1 | Derived-stat formulas and M1 gear | Research | — | Done |
| T1.4.2 | Item data for M1 gear | Data | T1.4.1, T1.2.5 | Done |
| T1.4.3 | `Build` type and legality | Build | T1.2.3 | Done |
| T1.4.4 | Attribute costs and effective ranks | Build | T1.4.3 | Done |
| T1.4.5 | Health and energy | Build | T1.4.4, T1.4.2 | Done |
| T1.4.6 | Armor per piece and damage type | Build | T1.4.2 | Done |
| T1.4.7 | Skill value tables | Build | T1.4.4 | Done |
| T1.4.8 | Foe derived stats and HM level mapping | Build | WP1.1 | Done |
| T1.4.9 | Build ↔ template conversion | Build | T1.4.3, WP1.3 | Done |
| T1.4.10 | §17.2 and M1 hand-value tests | Test | T1.4.5–T1.4.9 | Done |

> **Notes on what was built (2026-09-22).** Every M1 slot's hand values are
> asserted, and they are checked against **two independently derived sources**:
> the attribute points come from decoding the §20.1 template codes (T1.3.1) and
> the gear from §20.1's equipment column. The two agreeing is what makes the
> ranks trustworthy.
>
> **One part of the 136 test vector is not yet computable.** The wiki's itemised
> maximum Elementalist energy is 20 base + 10 armor + 48 Energy Storage + 8
> Radiant + 8 Attunement + 15 wand + 27 focus. The 98 that comes from armor,
> Energy Storage, Attunement runes and the focus's inherent +12 is computed
> through `energy()`; the remaining 38 comes from insignia and inscription
> *effects*, which have no typed form until the DSL lands in WP1.5. The test
> asserts the computed 98 exactly and documents the rest, so the part most
> likely to be wrong — the rank-16 Energy Storage and the four-not-five
> Attunement runes — is genuinely under test.
>
> **Two open questions carried forward from T1.4.1 §10:**
>
> 1. **Heroes 4 to 7 have no weapons in §20.1**, so their energy is the
>    profession base of 30 rather than 42. `the_caster_weapon_set_is_what_makes_
>    the_difference_in_energy` pins the current answer so that filling the gap
>    is a visible change rather than a silent one. It wants an assumption in the
>    spirit of A-033.
> 2. **The player wears three runes, not five**, under A-033, which is the only
>    reason it is 25 health lighter than every hero. Worth confirming before it
>    shows up as a simulation result.
>
> **`armor_profile` returns resting armor only.** Four of the five M1 insignias
> are conditional (Prodigy's, Minion Master's, Shaman's) or grant no armor
> (Bloodstained), and their conditions are DSL values until WP1.5. The
> `conditional` list exists and is empty; folding those bonuses in would
> overstate every M1 caster by up to 15.

### T1.4.1 Derived-stat formulas and M1 gear

**Type:** Research · **Depends on:** —

Using `research/` (Attributes, Item_types, Game_mechanics) and the wiki pages Attribute, Attribute point, Health, Energy, Armor rating, Rune, Insignia, Inscription, Weapon, Staff, Wand, Focus item and Template:Gr / Gr2, record the following.

1. **Attributes:**
   - the cost table (1, 2, 3, 4, 5, 6, 7, 9, 11, 13, 16, 20; rank 12 costs 97) and the 200-point budget;
   - the effective-rank caps: 20, or 21 with a weapon's "+1 (20% chance)" mod;
   - headgear +1 (primary-profession attributes only);
   - the rune rules: attribute runes apply to primary-profession attributes only; only the highest rune per attribute counts; every rune's health penalty applies.
2. **Rune values:** attribute rune bonuses (+1, +2, +3) and health penalties by tier; Vigor (minor, major, superior; the best one counts); Vitae (does it stack?); Attunement (energy; does it stack?).
3. **Insignias** used in M1: Prodigy's, Bloodstained, Minion Master's, Tormentor's and Shaman's. Record the exact effect of each and whether it applies per piece or globally. Tormentor's holy-damage penalty matters for the Blood is Power hero (§10.7).
4. **Health:** the level formula (T1.1.1); how Vigor, Vitae and insignia health combine.
5. **Energy:** base 20; profession armor energy; Energy Storage (+3 per rank); weapon, focus and staff energy; Attunement; Radiant. Also the regeneration pips (base 2 plus profession armor). Record the wiki's itemised maximum-Elementalist-energy sum (136) as a test vector.
6. **Armor:** base armor per profession; per-piece rules; which insignia and rune bonuses are local to a piece and which are global; which bonuses depend on damage type.
7. **Weapons and the 40/40 set:**
   - which inscriptions and mods make a "40/40 Domination set" (half-cast and half-recharge chances);
   - how the chances from two items combine;
   - the "+1 Domination (20% chance)" type of mod;
   - staff, wand and focus energy values;
   - wand and staff damage ranges and attack intervals (for caster auto-attacks).
8. **Skill scaling:** confirm Template:Gr's rounding (the `round(at0 + r·(at15−at0)/15)` form, and how halves round) and Template:Gr2 for title-scaled values.
9. **Hand values for every M1 party member** (§20.1):
   - points spent (must be ≤ 200);
   - effective ranks (e.g. the player's Domination 16, Fast Casting 11 and Inspiration 9);
   - maximum health;
   - maximum energy;
   - energy regeneration pips;
   - armor per piece against one physical and one elemental type.

   Use the runes and insignias in §20.1, with A-033 for the player.

- **Output:** `docs/findings/T1.4.1-derived-stat-formulas.md`, with the formulas, worked examples and a hand-value table per M1 slot.
- **Done when:** every T1.4.4–T1.4.8 formula has a source, and every M1 slot has hand values.

### T1.4.2 Item data for M1 gear

**Type:** Data · **Depends on:** T1.4.1, T1.2.5

1. Write the item files for M1:
   - `data/items/runes.ron`: attribute runes (minor, major and superior, for every attribute used in M1), Vigor (three tiers), Vitae, Attunement;
   - `insignias.ron`: the five insignias above;
   - `weapons.ron`: staff, wand, focus, axe, scythe, spear and bow types (the bow type comes from WP4.2);
   - `weapon_upgrades.ron`: the 40/40 set's components and the attribute "+1 chance" mod;
   - `armor.ron`: base armor per profession and piece.
2. Include `template_item_id` and `template_modifier_id` from T1.3.1.
3. Give every file a provenance block.

- **Done when:** `gwsim data validate` passes.

### T1.4.3 `Build` type and legality

**Type:** Build · **Depends on:** T1.2.3

1. Add `Build` following §7.2, with the fields:
   - `primary`, `secondary`;
   - `attribute_points: BTreeMap<Attribute, u8>`;
   - `headgear_attribute`;
   - `skills: [Option<SkillId>; 8]`;
   - `armor: [ArmorPiece { slot, insignia, rune }; 5]`;
   - `weapon_set: WeaponSet { main, offhand, prefix, suffix, inscription, offhand_upgrades }`.
2. Implement `Build::check(&self, data, slot_kind) -> Vec<LegalityError>`. It returns every violation, not just the first. Errors:
   - primary equals secondary;
   - a skill that is not from the primary or secondary profession and not common or PvE-only;
   - more than one elite;
   - more than 3 PvE-only skills, or any on a hero;
   - a duplicate skill;
   - points over 200 or any rank over 12;
   - an attribute of a profession the build doesn't have;
   - an attribute rune for a non-primary attribute;
   - a profession insignia that doesn't match the primary;
   - a headgear attribute that isn't a primary attribute.

   Record the non-stacking rune rule for T1.4.4.

- **Done when:** there is one unit test per error.

### T1.4.4 Attribute costs and effective ranks

**Type:** Build · **Depends on:** T1.4.3

1. Add:
   - `attribute_cost(rank) -> u16`, with rank 12 → 97;
   - `points_spent(&Build)`;
   - `rank_for_points(budget)`, which T1.4.9 uses for template ranks.
2. Add `effective_ranks(&Build, data) -> BTreeMap<Attribute, u8>`: points rank + the highest rune for that attribute + headgear, capped at 20.
3. Expose the weapon "+1 (20% chance)" mod as a flag for the engine to roll per activation. It is not added here.

- **Done when:** the tests cover the cost table, the 97 total, the cap at 20, and rune non-stacking.

### T1.4.5 Health and energy

**Type:** Build · **Depends on:** T1.4.4, T1.4.2

1. Add `max_health(&Build, level, data)`: the level base, plus the best Vigor, plus Vitae (per the T1.4.1 stacking rule), plus insignia health, minus the sum of every rune's health penalty.
2. Add `energy(&Build, data) -> EnergyStats { max, regen_pips }`: 20 + profession armor energy + Energy Storage × 3 + weapon, off-hand and staff energy + Attunement + Radiant. Regeneration is 2 + profession armor pips + item pips.
3. Keep effects that depend on temporary state out of these functions (e.g. an insignia bonus "while enchanted"). They become engine effects in WP4.3.

- **Done when:**
  - the health at level 20 is 480;
  - the energy by profession matches §17.2;
  - the maximum Elementalist energy reaches the wiki's 136 with its itemised gear;
  - the M1 hand values match.

### T1.4.6 Armor per piece and damage type

**Type:** Build · **Depends on:** T1.4.2

1. Add `armor_profile(&Build, data) -> ArmorProfile { pieces: [PerDamageType<i16>; 5] }`: the profession base per piece, plus static insignia bonuses on their own piece only, plus global rune bonuses. Damage-type-specific bonuses are applied per type.
2. Return any conditional bonuses as a separate list (`ConditionalArmor { piece, condition, amount }`) for the engine.
3. Leave the armor calculation (core, bonus, penetration and special) to WP3.5: this function produces the inputs.

- **Done when:** the tests cover each M1 insignia's placement and the hand values from T1.4.1.

### T1.4.7 Skill value tables

**Type:** Build · **Depends on:** T1.4.4

1. Add `scaled(at0, at15, rank) -> i32` using Template:Gr's rounding.
2. Add `title_scaled(r0, rmax, title_rank, table)` using Gr2 and the title table.
3. Add `SkillValueTable::for_unit(&skills, &effective_ranks, &title_ranks)`. For each bar slot it evaluates every `Scaled`, `ScaledBy` and `TitleScaled` value at the unit's rank. It provides `recompute(attr)` for temporary rank changes (§10.14).
4. Add a consistency helper, `check_triplet(r0, r12, r15)`, used by T1.2.8 and by the per-skill tests (§17.3).

- **Done when:**
  - Mistrust's PvE values reproduce: 10 at rank 0, 66 at rank 12 and 80 at rank 15 (§20.4);
  - the rounding edge cases from T1.4.1 are covered.

### T1.4.8 Foe derived stats and HM level mapping

**Type:** Build · **Depends on:** WP1.1

1. Add `foe_level(foe, mode) -> u8`: per-foe data first, otherwise the `levels.ron` mapping.
2. Add `foe_max_health(level, hard_mode)`: `20 × level + 80`, plus 20 per level above 20 in HM, per the T1.1.1 confirmation. At level 26 in HM the design's formula gives 720.
3. Add `foe_energy(profession)`: from `professions.ron`, with regeneration of player pips + 1.
4. Add `foe_armor(foe, level, damage_type)`: the foe's wiki table if present, otherwise `3 × level + profession bonus`. HM rule: foes below level 20 in NM get level-20 armor; foes above 20 keep their NM armor.
5. Add `foe_attributes(foe, mode)`: HM ranks if given, otherwise NM + 5 capped at 20 (A-005). Record that A-005 was used.

- **Done when:** the tests cover the level mapping, health at 20 and 26 (NM and HM), and the A-005 fallback.

### T1.4.9 Build ↔ template conversion

**Type:** Build · **Depends on:** T1.4.3, WP1.3

1. `Build::from_templates(skill: &SkillTemplate, equipment: Option<&EquipmentTemplate>, data)`:
   - converts template ranks to points with the cost table;
   - resolves skill IDs, returning an error that lists any unknown IDs;
   - maps equipment IDs to gear where known, and reports unknown ones as warnings;
   - leaves runes empty when there is no equipment template (templates don't carry runes, per the wiki).
2. `Build::to_templates(&self, data) -> (SkillTemplate, EquipmentTemplate)`.
3. Extend `gwsim template encode` to accept a build file.

- **Done when:** decoding the player code into a `Build`, adding the A-033 runes and 5 Prodigy's insignias, gives the T1.4.1 hand values; and `to_templates` returns the original skill code.

### T1.4.10 §17.2 and M1 hand-value tests

**Type:** Test · **Depends on:** T1.4.5–T1.4.9

1. Create `crates/gwsim-data/tests/wiki_examples.rs` with one test per §17.2 row assigned to WP1.4:
   - skill scaling;
   - attribute cost;
   - health at level 20;
   - energy by profession;
   - maximum Elementalist energy;
   - HM foe health.

   Each test names its wiki page in a comment.
2. Create `crates/gwsim-data/tests/m1_builds.rs`. For each of the 8 M1 slots, assert that derived ranks, health, energy, regeneration and armor equal the T1.4.1 hand values.

- **Done when:** both files pass.

---

## WP1.5 Effect DSL v0 and description renderer v0

**Goal:** a typed effect language that can encode the formulaic and conditional M0 and M1 skills. It is serialised as RON, validated statically and rendered as concise English of our own wording. Handlers can plug into the renderer.

- **Refs:** §8.3–§8.6, Q22, Q25, §17.3.
- **Depends on:** WP1.2.
- **Done when:** a Fireball-style illustrative skill and a conditional skill render correctly (golden tests), and the owner has reviewed the DSL's shape against five M1 examples.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T1.5.1 | DSL coverage study of the 72 M1 skills | Research | — | Done |
| T1.5.2 | Values, selectors and actions | Build | T1.5.1 | Done |
| T1.5.3 | Effect definitions, AI hints and role tags | Build | T1.5.2 | Done |
| T1.5.4 | Static validation of encodings | Build | T1.5.3 | Done |
| T1.5.5 | Description renderer | Build | T1.5.3 | Done |
| T1.5.6 | `gwsim data describe` | Build | T1.5.5 | Done |
| T1.5.7 | Golden description tests | Test | T1.5.6 | Done |
| T1.5.8 | DSL reference v0 | Docs | T1.5.7 | Done |
| T1.5.9 | Owner review of the DSL shape | Review | T1.5.8 | Todo |

> **Notes on what was built (2026-09-22).** T1.5.1 through T1.5.8 are done.
> **T1.5.9 is an owner gate and is not.**
>
> **The headline finding is that the design over-estimates handlers.** §8.5
> names 16 M1 skills as likely handlers; writing all 72 out in pseudo-DSL
> found **four**. Twelve of the sixteen are ordinary data once a hex or
> enchantment can say "when X happens, do Y", which §8.4's `Triggered` already
> allowed for. If that rate holds, §22's estimate of ~320 handlers across
> ~1,400 skills is closer to ~80. **One sample is not proof** — M1 is
> Mesmer-heavy — so this should be re-measured at the first P7 profession
> batch rather than planned around.
>
> **The insignias found constructs the skill study missed.** T1.5.1 read
> *skills*; the M1 insignias needed threshold filters (`RechargingSkills`,
> `ControllingMinions`, `ControllingSpirits`) and `ExploitsCorpse` that no
> skill did. Gear is worth studying in its own right before P7.
>
> **Reviewing the golden snapshots caught real defects**, which is the argument
> for having them. A negative range rendered as `1…-3` rather than `-1…-3`,
> reporting a different number from the one encoded; `Secondary` read as though
> the primary target also took the reduced share; and subject-verb agreement
> was wrong throughout ("target foe take 20 damage"). Since the generated text
> exists to be compared against the wiki by eye, each of those would have made
> a reviewer distrust a correct encoding.
>
> **Two encodings are deliberately incomplete, and say so in the data:**
> Tormentor's holy penalty differs per armor piece and the DSL has no
> per-piece amount; and every 40/40 upgrade applies only to spells of the
> item's own attribute, which needs an item-relative scope. "Master of My
> Domain" is left with `effects: []` for the same reason — an encoding that
> validated but meant nothing would be worse than none.

### T1.5.1 DSL coverage study of the 72 M1 skills

**Type:** Research · **Depends on:** — · **Owner:** approves the fetch if it exceeds 20 pages

1. For each of the 72 skills in §20.3, read the skill's wiki page: description, notes and 2026 update lines (§20.4). Use `research/` where it has the page, and otherwise fetch it (at most 72 pages, 3 s apart; about 4 minutes).
2. Write each skill in pseudo-DSL, using only the constructs listed in §8.4.
3. Classify each skill as formulaic, conditional or handler. Confirm or correct the §20.3 judgement, and classify the 31 Kournan skills for the first time.
4. List:
   - every construct the pseudo-DSL needed that §8.4 lacks (e.g. "per point of energy lost", "if target is casting a spell", "75% to other foes");
   - every event the skills need;
   - every stat a `ModifyStat` touches;
   - every effect family.
5. For each proposed handler, write one sentence on why the DSL can't express it.
6. Write nothing in our files that paraphrases a description closely enough to count as copying. The pseudo-DSL is our own encoding.

- **Output:** `docs/findings/T1.5.1-dsl-coverage.md`: a table of skill, class, pseudo-DSL, constructs and handler reason, plus the construct and event lists.
- **Done when:** all 72 skills are classified and the construct list is complete.

### T1.5.2 Values, selectors and actions

**Type:** Build · **Depends on:** T1.5.1

1. In `gwsim-data::dsl`, add these enums:
   - `Value`: `Fixed`, `Scaled`, `ScaledBy`, `TitleScaled`, `Percent`, `PerUnit { value, of: Quantity }`, `Min`, `Max`, `Sum`;
   - `Selector`: every selector and filter in §8.4 plus those added by T1.5.1, `Secondary { factor }`, and `Not`;
   - `Action`: every action in §8.4 plus those added by T1.5.1;
   - `Control`: `If`, `ForEach`, `Chance`, `Sequence`, `Triggered`;
   - `Event`: the §8.4 events plus additions;
   - `Stat` and `ModCategory` (core, bonus and special armor; multiplicative or additive; capped or uncapped).
2. Derive serde for all of them. Choose RON-friendly forms, e.g. `Damage(to: TargetAndAdjacentFoes, kind: Fire, amount: Scaled(7, 112))`, as in the §8.3 example.
3. Add constructor helpers for tests.

- **Done when:** the §8.3 example's `effects` field parses into these types.

### T1.5.3 Effect definitions, AI hints and role tags

**Type:** Build · **Depends on:** T1.5.2

1. Add `EffectDef`, holding:
   - `id` (local to the skill, or a global slug for shared effects);
   - `kind` (§7.1 list);
   - `stacking: StackingRule { key, rule: Replace | KeepLonger | StackBySource }`;
   - `duration: Value`, `removable_by`;
   - `while_active: Vec<Modifier>`, `triggers: Vec<Trigger>`, `on_end: Vec<Action>`;
   - `upkeep: Option<i8>`.
2. Add a `one_at_a_time_family()` function derived from `kind`: stance, preparation, glyph, form, party bonus, weapon spell per target, bundle, binding-ritual spirit type.
3. Add `AiHints { use_when: Vec<Cond>, never_when: Vec<Cond>, target: TargetPreference, priority: i8 }` and a `RoleTag` enum (the §8.4 list).
4. Add `HandlerRef { name: String, params: BTreeMap<String, Value> }`.
5. Define a `HandlerDescribe` trait in `gwsim-data`: `fn describe(&self, params, ctx) -> String`. The engine's `SkillHandler` (WP3.6) extends it. The data crate receives a `&dyn HandlerRegistry` at render time, so it never depends on the engine.

- **Done when:** a hex with a trigger and a maintained enchantment round-trip in RON.

### T1.5.4 Static validation of encodings

**Type:** Build · **Depends on:** T1.5.3

1. Add these rules to `gwsim data validate`:
   - `Scaled` needs a skill attribute;
   - `TitleScaled` needs `title_track`;
   - every `ApplyEffect` references a defined `EffectDef`;
   - durations are greater than 0;
   - selectors suit the skill's `target` (e.g. `TargetAlly` on a foe-targeted skill is an error);
   - trigger charges are greater than 0;
   - `Chance` probabilities lie in (0, 1];
   - `handler` names are registered.
2. **Encoding vs numbers:** every `Scaled(a, b)` in the encoding must match one of the skill's `extracted.scaled` entries (`r0 == a`, `r15 == b`), unless it carries a `note` explaining why.

- **Done when:** there is one invalid fixture per rule (added to the T1.2.10 suite).

### T1.5.5 Description renderer

**Type:** Build · **Depends on:** T1.5.3

1. Add `describe(skill, rank: Option<u8>, registry) -> String`.
2. Value rendering: `7…112` when no rank is given (ranks 0…15), or the value at the rank when one is.
3. Selector rendering: noun phrases, e.g. "target foe and adjacent foes", "all party members in earshot", "other foes near target take 75% of this damage".
4. Action rendering: one sentence template per action; `If` becomes "If target foe is hexed, …"; `Triggered` becomes "The next time target foe casts a spell, …".
5. Effect definitions: "For 8…15 seconds, target foe …".
6. Handlers: call `describe` through the registry.
7. Keep the wording ours: plain, consistent and terse. Never pull from wiki text (C2).

- **Done when:** the illustrative Fireball renders as "Target foe and adjacent foes take 7…112 fire damage." (§8.6).

### T1.5.6 `gwsim data describe`

**Type:** Build · **Depends on:** T1.5.5

1. Add `gwsim data describe <skill slug|id|name> [--rank N] [--json]`. It prints:
   - the name;
   - the wiki link;
   - type and costs;
   - the generated description;
   - role tags;
   - AI hints;
   - review status.
2. Add filters for batch review (used in WP4.1): `--profession`, `--status`, `--all`.

- **Done when:** the command works on the fixture skills.

### T1.5.7 Golden description tests

**Type:** Test · **Depends on:** T1.5.6

1. Add `insta` snapshots for:
   - the Fireball-style skill;
   - a conditional skill ("if target is hexed, extra damage");
   - a hex with duration and a `while_active` modifier;
   - a triggered effect with charges;
   - a `TitleScaled` value;
   - a `Secondary(0.75)` area skill;
   - a stub handler's `describe`.

- **Done when:** the snapshots are reviewed and committed.

### T1.5.8 DSL reference v0

**Type:** Docs · **Depends on:** T1.5.7

1. Write `docs/effect-dsl.md`. For every construct, give its purpose, its RON form and a short example with its rendered description.
2. Add a section on when to use a handler instead (§8.5).
3. Link the reference from `docs/data-authoring.md`.

- **Output:** `docs/effect-dsl.md`.

### T1.5.9 Owner review of the DSL shape

**Type:** Review · **Depends on:** T1.5.8 · **Owner:** reviews

1. Show the owner five M1 encodings written in the DSL (e.g. Energy Surge, Mistrust's non-handler part, Remove Hex, Recuperation, Signet of Creation), with their rendered descriptions.
2. Collect the owner's changes and apply them before P3 builds the interpreter on these types.

- **Done when:** the owner approves the DSL shape.

---

## WP1.6 Assumptions register and coverage

**Goal:** every undocumented value lives in `data/assumptions.ron`. Code reads assumptions by ID, never as literals (ENG-4). Runs can record which assumptions they touched. Coverage is reported per profession, status and campaign.

- **Refs:** §8.7, §8.8, §14.1, §21, ENG-4.
- **Depends on:** WP1.2.
- **Done when:** `gwsim data coverage` produces the report, and the register loads with all 33 initial entries.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T1.6.1 | Assumption types and typed access | Build | — | Done |
| T1.6.2 | Write `data/assumptions.ron` | Data | T1.6.1 | Done |
| T1.6.3 | Assumption-use recording | Build | T1.6.1 | Done |
| T1.6.4 | Coverage computation and `gwsim data coverage` | Build | WP1.2 | Done |
| T1.6.5 | Tests | Test | T1.6.2–T1.6.4 | Done |

> **Notes on what was built (2026-09-22).** One deviation, in the direction of
> honesty rather than away from it.
>
> **Ten assumptions carry values, not the five T1.6.2 listed.** The task said
> A-012, A-018, A-024, A-028 and A-030 get values and the rest start
> `Pending`. But P1's own research settled A-005, A-020, A-025, A-033 and
> A-021 along the way, and `core/modes.ron` and `core/ranges.ron` already use
> those numbers. Marking them `Pending` would make the register contradict the
> data it exists to explain. A test asserts that every assumption the core
> files rely on has a value.
>
> **Every `Pending` entry names the task that will settle it**, enforced by a
> test. A pending assumption with no owner is one nobody comes back to, which
> defeats the register.
>
> **Coverage says what its percentages are of.** Until `data/skills/index.ron`
> arrives in T2.3.6 there is no list of every skill in the game, so a
> percentage is of the files that happen to exist — which is always 100%.
> `gwsim data coverage` prints that caveat rather than a flattering number.

### T1.6.1 Assumption types and typed access

**Type:** Build · **Depends on:** —

1. Add `Assumption { id, statement, value: AssumptionValue, unit: Option<Unit>, rationale, status: Assumed | Measured | Confirmed, sources: Vec<String> }`.
2. `AssumptionValue` has the variants `Number(f64)`, `Millis(u32)`, `Gwinches(f32)`, `Percent(f32)`, `Table(Vec<(String, f64)>)`, `Text(String)` and `Pending`.
3. Add typed keys in code: `pub const CHAIN_REST: AssumptionKey<Millis> = AssumptionKey::new("A-028");` and similar, one per assumption the code reads.
4. Add `Assumptions::get(key) -> T`, which returns the typed value and panics only in tests.
5. At load, check that every key in a static `ALL_KEYS` list exists and has the right value type. A key whose value is `Pending` is a warning until something uses it, and an error when a run reads it.

- **Done when:** the unit tests cover a wrong value type, a missing key and `Pending` handling.

### T1.6.2 Write `data/assumptions.ron`

**Type:** Data · **Depends on:** T1.6.1

1. Add all 33 entries from §21. Values that are decided now:

   | ID | Value |
   | --- | --- |
   | A-012 | 250 ms |
   | A-018 | 3000 |
   | A-024 | 166 |
   | A-028 | 20 s |
   | A-030 | 180 s |

2. The rest start as `Pending`, with the task that will set each one in `notes`:

   | IDs | Set by |
   | --- | --- |
   | A-001, A-002, A-003, A-009 | T3.2.1 |
   | A-004, A-005, A-006, A-007, A-008 | T4.2.1 / WP4.8 |
   | A-010, A-011, A-016 | T4.4.1 / T4.5.1 |
   | A-032 | T1.1.1 or T4.2.1 |

3. Every entry has status `Assumed` and a rationale taken from the §21 "Why" column.

- **Done when:** `gwsim data validate` passes, with only the expected `Pending` warnings.

### T1.6.3 Assumption-use recording

**Type:** Build · **Depends on:** T1.6.1

1. Add `AssumptionsUsed` (a small bitset or sorted set of IDs).
2. `Assumptions::get` gets a variant, `get_recorded(key, &mut AssumptionsUsed)`, for the engine to call.
3. Data-level references (a foe's `provenance.assumptions`) are added when the entity takes part in a run.
4. The report layer (§14.1) resolves the IDs to statements.

- **Done when:** unit tests confirm the recorded set is exactly the set of IDs read.

### T1.6.4 Coverage computation and `gwsim data coverage`

**Type:** Build · **Depends on:** WP1.2

1. `Coverage::compute(data)` counts skills by profession × review status × campaign, and lists:
   - per curated encounter, the missing monster or foe skills (skills referenced by foes with no file);
   - foes that reference `NumbersOnly` skills.
2. **Denominators:** until `data/skills/index.ron` exists (T2.3.6), percentages use only existing files and the report says so. Afterwards, they use the full index.
3. `gwsim data coverage [--profession P] [--json]` prints a table (profession × status, counts and percentages) and the missing-skill lists.

- **Done when:** the command runs on `data/` and on the fixtures.

### T1.6.5 Tests

**Type:** Test · **Depends on:** T1.6.2–T1.6.4

1. Test that the register loads all 33 IDs, in order, with no duplicates.
2. Test that a coverage fixture gives the expected table (snapshot).
3. Test that an unknown assumption ID in a skill's provenance fails validation.

- **Done when:** all pass.

---

## WP1.7 Data pack and user data directory

**Goal:** the shipped binaries carry a validated, versioned data pack, so they run with no `data/` directory. Developers can point them at `data/` directly, and user files load from `%APPDATA%\gwsim\`.

- **Refs:** §6.3, §6.4, §10.13 (ENG-43), §14.1 (pack version in reports).
- **Depends on:** WP1.2, WP1.6.
- **Done when:** the release binary, copied to an empty folder, runs `gwsim data coverage` from its embedded pack.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T1.7.1 | Choose the pack format and embedding | Research / Decision | — | Done |
| T1.7.2 | `DataPack` and its version | Build | T1.7.1 | Done |
| T1.7.3 | Build-time pack generation | Build | T1.7.2 | Done |
| T1.7.4 | Developer mode (`--data-dir`) | Build | T1.7.3 | Done |
| T1.7.5 | User data directory | Build | T1.7.2 | Done |
| T1.7.6 | Version and info output | Build | T1.7.3 | Done |
| T1.7.7 | Tests without `data/` | Test | T1.7.4–T1.7.6 | Done |

> **Notes on what was built (2026-09-22).** The measurement overturned the
> plan's expected answer.
>
> **T1.7.1 chose option (a), embedded RON text, not option (b) with
> `postcard`.** The plan expected (b) on start-up cost. A synthetic
> full-coverage tree — 2,400 files, 2.9 MB, every skill carrying a real
> encoding — loads in **36 ms in release** and 348 ms in debug. That is not a
> cost worth a second format, a second dependency, a format version, and a
> class of serialisation bug that (a) simply cannot have: the text in the
> binary is the text that was validated, going through the same parser.
> `crates/gwsim-data/tests/pack_scale.rs` keeps the measurement repeatable, so
> revisiting this is a measurement rather than an argument.
>
> **Build-time validation still happens.** `build.rs` fails the build on a
> broken tree, so a release binary cannot carry data that does not load. Only
> what gets *embedded* differs from the plan.
>
> **The content hash is BLAKE3 over sorted, forward-slash `(path, contents)`
> pairs**, with lengths hashed before each field so that two trees cannot
> collide by splitting a name differently. The forward-slash guarantee came
> from T1.2.7's `DataSource`, which is what makes the hash the same on Windows
> and Linux.
>
> **`gwsim --version` now carries the data hash**, because two builds of the
> same version can hold different data and a result is only reproducible
> against the data it came from (ENG-43).
>
> **Every command says which data it used.** A result computed against a
> developer's local edits and one computed against the shipped data are
> different results, and nothing else in the output would tell them apart.

### T1.7.1 Choose the pack format and embedding

**Type:** Research / Decision · **Depends on:** —

1. Compare three options:
   - (a) embed the RON files as text and parse them at start-up;
   - (b) validate at build time and embed a compact binary serialisation (`postcard` or `bincode`) of the `DataSet`;
   - (c) zero-copy (`rkyv`).
2. Estimate the start-up cost at full coverage: about 2,000 files and a few MB of RON. Time the parse of a synthetic tree of that size with the WP1.2 loader.
3. Confirm that a `build.rs` in `gwsim-cli` (and later `gwsim-desktop`) can list `gwsim-data` as a build-dependency, load and validate `../../data`, and write the pack to `OUT_DIR` for `include_bytes!`.
4. Recommend an option. The expectation is (b) with `postcard`, but choose (a) if the parse is fast enough and simpler.
5. Record the decision in DESIGN.md §6.4.

- **Output:** `docs/findings/T1.7.1-data-pack.md`.

### T1.7.2 `DataPack` and its version

**Type:** Build · **Depends on:** T1.7.1

1. Add `DataPack { version: PackVersion { content_hash, baseline: "2026-09-22", gwsim_version }, data: DataSet }`.
2. `content_hash` is a stable hash over the sorted (path, contents) pairs of `data/`. Use a fixed, dependency-light hash (e.g. `blake3`, or SHA-256 via `sha2`), because the hash is part of the reproducibility key (ENG-43).
3. Add `DataPack::from_source`, `to_bytes` and `from_bytes`, with a format-version byte.

- **Done when:** a pack round-trips to an equal `DataSet`, and the hash is stable across runs and path separators.

### T1.7.3 Build-time pack generation

**Type:** Build · **Depends on:** T1.7.2

1. Add `crates/gwsim-cli/build.rs` that:
   - declares `cargo:rerun-if-changed=../../data`;
   - loads and validates the tree;
   - on errors, prints the validation report and **fails the build**;
   - otherwise writes `OUT_DIR/data.pack`.
2. Load the pack in `gwsim-cli` with `include_bytes!(concat!(env!("OUT_DIR"), "/data.pack"))`.
3. Share the generator through a helper in `gwsim-data` (`pack::build_from_dir`), so the desktop app can reuse it in P6.

- **Done when:** editing a data file triggers a rebuild, and a broken data file fails `cargo build` with the report.

### T1.7.4 Developer mode (`--data-dir`)

**Type:** Build · **Depends on:** T1.7.3

1. Add a global option `--data-dir <path>` to `gwsim`. When set, data loads from the directory instead of the embedded pack.
2. `gwsim data validate` defaults to `./data` when it exists and to the embedded pack otherwise. It prints which source it used.

- **Done when:** both sources work, and output names the source.

### T1.7.5 User data directory

**Type:** Build · **Depends on:** T1.7.2

1. Resolve `%APPDATA%\gwsim\` with the `dirs` crate. Allow an override with `--user-dir` (tests) and `GWSIM_USER_DIR`.
2. Subfolders: `profiles/`, `encounters/`, `situations/`, `situation_sets/`, `parties/`, `results/`.
3. User encounters and situations load with the same schemas, validated against the pack, and are namespaced so they never shadow bundled IDs (e.g. `user:my-encounter`).
4. Never create the directory or write to it unless a command saves something.

- **Done when:** a user encounter referencing bundled foes loads, and one referencing an unknown foe is rejected with the file path.

### T1.7.6 Version and info output

**Type:** Build · **Depends on:** T1.7.3

1. Change `gwsim --version` to print the crate version and the short data pack hash.
2. Add `gwsim data info`, which prints the pack version, the baseline date, entity counts and the user directory path.

- **Done when:** both commands print the expected fields.

### T1.7.7 Tests without `data/`

**Type:** Test · **Depends on:** T1.7.4–T1.7.6

1. Add an `assert_cmd` test that runs `gwsim data info` and `gwsim data coverage` with the working directory set to an empty temporary folder. Both must succeed from the embedded pack.
2. Add a test that the pack builder rejects a broken fixture tree. Call the helper directly; don't do a real build.

- **Done when:** both pass. This closes P1.
