# Fallout tasks

Small outstanding items that fell out of finished work. Each one is here
because it was **noticed and deliberately not done**, not because it was
forgotten.

The phase files say what a phase set out to do. This file says what it left
behind. An item leaves when it is done, or when it is absorbed into a real
task in a phase file.

- **Status:** 2026-09-23, after P1. **Section 1 is empty and nothing left is P1 work.** O1–O7, G1–G3 and P4 are closed; G4, G5 and every E item wait on P2 or WP4.3, and §6's three remaining items are standing notes rather than tasks.
- **Format:** each item says what it is, why it was left, and where it goes.

---

## 1. Needs the owner

Things I cannot decide or do.

**Nothing, as of 2026-09-23.** Every item that was here has been decided; they
are in §8 with their outcomes. New ones go here rather than being guessed at.

---

## 2. Gaps in tasks marked Done

**These are honest admissions.** Each task's main goal is met and its done
criterion passes, but one listed action is not implemented. They are here
rather than left as a silently incomplete tick.

| # | Item | Where |
| --- | --- | --- |
| G4 | **`armor_profile` never returns conditional bonuses.** The `conditional` field exists and is always empty, because insignia conditions are DSL values and nothing evaluates them yet. Four of the five M1 insignias are conditional, so a build's real armor can be up to 15 higher than reported. | `crates/gwsim-data/src/derived.rs`, needs WP4.3 |
| G5 | **Part of the 136 Elementalist energy vector is asserted as a constant.** 98 is computed through `energy()`; the remaining 38 comes from insignia and inscription *effects*, which have no typed form. The test documents the split. | `crates/gwsim-data/tests/wiki_examples.rs` |

---

**A note on G3.** The obvious reading of T1.2.8 was "warn when a benchmark
names a skill we do not have". Implemented that way it broke 27 snapshots at
once, which was the useful signal: `data/skills/` is empty until P2 and stays
incomplete until P7, so that warning would sit on every benchmark on every run
for the length of the project. A warning that is always on teaches people to
ignore warnings. The decode check stayed in `validate`, where a failure is
unambiguous; the missing-skill count went to `gwsim data coverage`, which
already reports exactly this shape for foes.

## 3. Encodings that are deliberately incomplete

The data says so in its own comments, but they are collected here so they are
findable.

| # | Item | Blocked on |
| --- | --- | --- |
| E1 | **Tormentor's holy penalty is not encoded.** It differs per armor piece (+6 chest, +4 legs, +2 elsewhere) and the DSL has no per-piece amount. Only the uniform +10 armor is encoded. The same shape blocks Survivor and Radiant insignias. | A per-piece effect amount, WP4.3 |
| E2 | **The 40/40 upgrades are encoded too broadly.** Each applies only to spells of the *item's own* attribute, and the DSL has no item-relative scope, so the encodings currently apply to everything. | An item-relative scope, WP4.3 |
| E3 | **"Master of My Domain" has `effects: []`.** Its effect is "+1 to the item's attribute", which needs the same item-relative scope. Left empty on purpose: an encoding that validated but meant nothing would be worse than none. | Same as E2 |
| E4 | **`data/skills/` is empty.** The schemas, DSL, validator and renderer are built and tested against fixtures, but no real skill exists, so `data describe` finds nothing on a fresh checkout. | P2 seeds them; WP4.1 encodes them |
| E5 | **Hero 7's staff health modifiers are not encoded.** PvX names a Hale Spawning Power staff of fortitude. Both the staff head and the wrapping grant health, but the item data has no health-granting weapon upgrades and the wiki values have not been read. Hero 7's reported 400 maximum health is a floor, not the true figure. | Weapon upgrade effects, WP4.3 (A-034) |
| E6 | **Lamentation and Blood of the Master are unstudied.** They joined the party on 2026-09-23 when it moved to PvX's caster column. T1.3.1 read 72 skill ids and T1.5.1 classified 72 skills; both read the *old* roster, so 69 of the current 71 are covered. Neither new skill has an id, values, or a DSL classification. | Two wiki pages; blocks WP4.1 |
| E7 | **Three published template codes no longer encode their bars.** Heroes 4 and 6 carry PvX's melee-column codes (Withering Aura, Splinter Weapon). The codes still decode to the right profession and attributes, and the M1 tests only assert those, so nothing fails — but the code column and the skill column of §20.1 now disagree. Re-encoding is blocked on E6's two ids. | E6 |
| E8 | **M1 no longer exercises weapon spells.** Splinter Weapon was the only one, so §20.5's "one weapon spell per target" rule will be built against no M1 skill and needs its own fixture. | WP4.3 |

---

## 4. Assumptions still `Pending`

Eleven of the 33 have no value. Each names its owning task in the register, so
this is a summary rather than a second source of truth — read
`data/assumptions.ron` for the statements and rationales.

| Settled by | Assumptions |
| --- | --- |
| T3.2.1 | A-001 movement speed · A-002 collision radius · A-003 projectile speeds · A-009 aggro range |
| T3.5.1 | A-023 armor penetration rounding |
| T4.2.1 | A-004 foe weapon damage · **A-032 hard-mode recharge reduction** |
| T4.4.1 | A-010 foe reaction delay · A-016 scatter thresholds |
| T4.5.1 | A-011 hero reaction delay |
| WP4.3 | A-015 spirit health and armor |

**A-032 is the awkward one.** Every other pending assumption is waiting for
someone to look something up. A-032 cannot be looked up: the Hard mode page
says only "shorter recharges" and gives no number anywhere. T4.2.1 has to
*measure* it in-game, which is a different kind of work.

---

## 5. Open questions from the findings

Recorded in findings files, repeated here so they are not lost in them.

| # | Question | From |
| --- | --- | --- |
| Q1 | **Is the 6% handler rate representative?** §8.5 predicted 16 M1 handlers; four are needed. If that holds, §22's ~320-handler estimate across ~1,400 skills is nearer 80 — a materially different project. M1 is Mesmer-heavy, so **re-measure at the first P7 profession batch** before revising the estimate. | [T1.5.1](../findings/T1.5.1-dsl-coverage.md) |
| Q2 | **Gear needs its own DSL study.** T1.5.1 studied *skills*. The M1 insignias then needed four constructs no skill did (`RechargingSkills`, `ControllingMinions`, `ControllingSpirits`, `ExploitsCorpse`). Runes, weapons and consumables have not been studied at all. | T1.5.1 vs T1.4.2 |
| Q3 | **The template attribute-width floor is inferred.** Professions and skills use the smallest width that fits; attributes do not. `max(1, needed − 4)` reproduces all seven published codes, but both codes that constrain it are Mesmer bars. If a future code fails to round-trip, start here. | [T1.3.1](../findings/T1.3.1-templates.md) §3 |
| Q4 | **Equipment template version 1 is unspecified.** A second, older layout exists. Nothing needs it. | T1.3.1 §4 |
| Q5 | **No test vector has a full bar of 8 real skills.** Every published code has at least one empty slot, so nothing proves the skill width behaves at 8. | T1.3.1 §9 |
| Q6 | **"Adjacent to target" has no published gwinch value.** It is the scythe and melee-AoE radius. M1 has no scythe user, so it waits for WP4.2. | [T1.1.1](../findings/T1.1.1-core-values.md) §4 |
| Q7 | **The hard-mode level mapping has gaps** at normal levels 32 and 35+, and one row (34) maps to a *range*. The loader returns nothing rather than guessing, so a foe at those levels must state its own hard-mode level. | T1.1.1 §5 |
| Q9 | **Four DSL shape questions went unasked.** T1.5.9 was closed without the review packet, so these are open against the first real encodings rather than settled: (a) is `Control::Triggered` one abstraction or four mechanisms in a trenchcoat — it is what took the handler estimate from 16 to 4; (b) is the `Selector`/`Filter` split real, or an invented distinction; (c) does `Value` carry too much through `Quantity`; (d) are the missing item-relative and per-piece scopes (E1–E3) additions or type changes. | T1.5.9 |
| Q8 | **Is 2,400 files the right full-coverage scale?** The data pack decision rests on a 36 ms release load at that size. If the real tree lands far above it, revisit. `crates/gwsim-data/tests/pack_scale.rs` re-runs the measurement. | [T1.7.1](../findings/T1.7.1-data-pack.md) §8 |

---

## 6. Tooling and process

| # | Item | Note |
| --- | --- | --- |
| P1 | **The CI workflow has never been machine-validated.** `actionlint` would do it but needs installing, which needs your go-ahead. It was reviewed by eye, which T0.6.5 allows. | |
| P2 | **No property testing.** T1.3.5 asked for `proptest`; the template suite sweeps every width boundary, attribute, slot and profession pair instead, plus a no-panic pass over malformed input. Add `proptest` if a real bug ever escapes that. | |
| P3 | **Debug builds load the data ten times slower than release** — 348 ms against 36 ms at full coverage. Fine today. If a developer workflow starts loading repeatedly (a watch mode), caching becomes worth it. | T1.7.1 §8 |

---

## 7. Raised during P2–P5 (unattended session, 2026-09-23)

P2 to P5 were implemented in one unattended session. The owner gave standing
permission for every action and asked for no questions, so **every decision
the plan reserves for the owner was taken here instead, and is logged beside
the item that needed it** for review. Status is one of: **Done** (handled in
the phase that raised it), **Deferred → Pn** (logged for a later phase), or
**Owner** (only the owner can close it, such as a review sign-off).

| # | Item | Decision taken | Status |
| --- | --- | --- | --- |
| F2.1 | **T2.1.1, T2.1.2 and T2.1.4 need the owner to approve the fetches and the parsing strategy.** | Fetched 18 pages at ≥ 3 s intervals with the project User-Agent, and fixed the strategy in [T2.1](../findings/T2.1-extractor-spike.md) §5. The main departure from the plan: `Skill_template_format/Skill_list` replaces the game-integration pages as the primary ID source, because it is newer (to 3473) and one page; and the ID is read from the skill page itself, which the design did not expect to be possible. | Done; owner to review the strategy |
| F2.2 | **The plan names "List of Mesmer skills"; the real title is lower case.** | Discovery uses "List of mesmer skills" and its ten siblings. | Done |
| F2.3 | **T2.4.5 proposes SHA-256 for the description hash.** | Used BLAKE3, already a workspace dependency for the data pack, rather than adding `sha2`. The stored value is prefixed `blake3:` so a later change of algorithm is visible in the data. Nothing depends on the algorithm beyond equality. | Done |
| F2.4 | **Q10 says seeding never overwrites; the skill index must be regenerable.** | `gwsim-extract index` replaces `data/skills/index.ron` wholesale. The index is facts only and never hand-edited, so the rule Q10 exists for — protecting hand-written encodings — does not apply; `seed` itself still never overwrites anything. | Done; owner to confirm |
| F2.5 | **The Kournan Guard and Bowman pages name 12 skills outside M1** (their hammer, sword and Jahai Bluffs loadouts), and validation requires every foe skill reference to resolve. | Seeded those 12 as `NumbersOnly` alongside the 71, so the foe files keep every variant the wiki lists. They are never simulated, and coverage reports them as seeded-not-encoded. | Done |
| F2.6 | **T2.4.8 and T2.5.7 ask the owner to spot-check the parse tables.** | Every row was checked by machine instead: 69 skills against the independent P1 record (0 differences), 2 against the spike pages, 8 foes against §20.2 (all agree). The owner's spot-check (≥ 10 skill rows including Energy Surge, the only special-rounding row) has not happened. | **Owner** |
| F2.7 | **The Zealot and Phalanx pages give no attribute ranks at all**, and A-005 only fills hard-mode ranks *from* normal-mode ones. | Seeded with empty ranks and a note. WP4.2 must supply normal-mode ranks by a new rule, recorded as an assumption. | Deferred → P4 (WP4.2) |
| F2.8 | **PvE-only skills with a profession broke two rules at once**: the layout check wanted them in their profession's folder, the PvE-only check in `common/`. 60-odd skills in the index (the allegiance skills among them) are affected; none is in M1. | Fixed in `DataSet::check_layout`: a PvE-only skill belongs in `common/` whatever its profession. The seeder already wrote them there. | Done |
| F2.9 | **T2.6.5 says to commit the seeded files once the owner approves.** | Committed on `extractor/p2` and merged without approval, like the rest of the phase. `git revert` of the seeding commit removes them cleanly. | **Owner** to review |
| F2.10 | **Monster skill ids above 3473 appear on no list page** the extractor may read, and boss pages were never sampled (no M1 foe is a boss). | Recorded as open questions in [T2.1](../findings/T2.1-extractor-spike.md) §7. The boss flag is read from a `Boss` row when present. | Deferred → P7 |
| F2.11 | **The seeded Kournan Guard keeps only its first armor table.** Its hammer loadout has its own (100 / 80), but `FoeVariant` has no armor field. | The axe/sword table is the foe's armor (A-007 makes it the axe variant), and the note says further tables exist. WP4.2 decides whether variants need their own armor. | Deferred → P4 (WP4.2) |

| F3.1 | **The DSL validator rejected Power Drain**: its side check flagged any ally-side selector in a foe-targeted skill, including `Self` ("you gain energy"). | Narrowed the check to selectors built on the *target* (`TargetAlly` in a foe skill, `TargetFoe` in an ally skill); `Self`, `Party` and the like may appear in any skill. | Done |
| F3.2 | **`CoreData` could only load from a directory**, so the embedded data pack could not supply core data to `gwsim evaluate`. | Added `CoreData::from_source` and `pack::source_from_bytes`; the CLI loads core data from whichever source it chose for the rest. | Done |
| F3.3 | **T3.1.4 is an owner decision: the time model.** | Chose **model B** (integer-millisecond events plus a 50 ms tick for movement, regeneration and AI), per the spike in [T3.1.3](../findings/T3.1.3-time-model-spike.md). DESIGN §10.2 is marked confirmed. | Done; owner to review |
| F3.4 | **T3.10.3/T3.10.4 list Mistrust as a handler.** | Encoded Mistrust as data (a hex with a one-charge `Triggered` on `OnSpellCast`, filtered to spells cast on the caster's allies), as T1.5.1 predicted was possible. Only Arcane Echo and Air of Superiority have handlers. | Done; owner to review |
| F3.5 | **T3.10.5 leaves the plan file's location to T4.6.2.** | A human slot's plan lives **inside its party file** (`PartySlot.plan`), so a party is one self-contained file; `data/parties/` holds them. Rules name skills, not slots, so an Arcane Echo copy is used by the copied skill's rule. T4.6.2 can still move plans out. | Done; P4 to confirm |
| F3.6 | **T3.10.2 asks for the three dummies in a `Cluster`.** | Used `Explicit` positions instead, so one dummy is exactly adjacent (100) and one exactly nearby-but-not-adjacent (224), which the AoE value tests need. `Cluster` layout exists and is deterministic without a random stream. | Done |
| F3.7 | **Which skill damage ignores armor.** | Typeless, shadow and skill-dealt holy damage ignore armor (Damage type); the interpreter at first treated only typeless damage so, and was fixed while writing T3.5.1. Weapon damage (Mesmer wands: chaos) respects armor. | Done |
| F3.8 | **§10.14 says skill values are precomputed per unit at fight start.** | Stats and ranks are computed on demand from effects and gear each time they are needed. M0 runs 64 fights in 0.18 s unoptimised, so this is not yet a problem. | Deferred → P4 (WP4.11 profiling) |
| F3.9 | **T3.10.8: the owner reviews the 8 M0 encodings** and marks them `Reviewed`. | Not done: the skills stay `Draft`. What to review: `gwsim data describe <skill>` for each, next to its wiki page and `gwsim evaluate --party m0-player --situation dummies-hm --log text`. | **Owner** |
| F3.10 | **A-032 (hard-mode foe recharge) has no amount on the wiki.** | The engine applies **no** reduction and marks every hard-mode foe recharge as touching A-032, so results say they depend on it. | Deferred → P4 (WP4.2 sets a default) |
| F3.11 | **Named aftercast exceptions are not modelled** (the Aftercast delay page lists skills without it despite their type, and four 0 s "Skill"-type skills that have it besides Air of Superiority). | Aftercast is per skill type (0.75 s for types that activate), with Air of Superiority right because it is a "Skill". No other M0 skill is affected. | Deferred → P4 (check the M1 bars in WP4.1) |
| F3.12 | **Mechanics outside M0 left unmodelled:** Disease spreading, the low and high hit-location profiles, Critical Strikes, and the always-critical hit from behind. | Recorded in [T3.5.1](../findings/T3.5.1-damage-rules.md) and [T3.6.1](../findings/T3.6.1-effects.md). | Deferred → P4 if an M1 bar needs one, else P7 |
| F3.13 | **Seven new assumptions were needed** (A-035 to A-041: auto-attack hit timing, experience range, caster weapon damage type, combining chance mods, skill-damage hit locations, target lost mid-activation, "next spell" trigger timing). | Added to `data/assumptions.ron` and DESIGN §21 with the values chosen. | Done; owner to review the values |
| F3.14 | **Foe and hero reaction times** are researched in T4.4.1 and T4.5.1. | Until then both fall back to the human delay (A-012) and are marked as touching A-010 and A-011. | Deferred → P4 |
| F3.15 | **The M0 hand calculation said 601 ms for the 1 s skills.** | Fast Casting 11 is ×0.60151, so 1,000 ms becomes 601.5 → **602 ms**. Corrected in [T3.10.1](../findings/T3.10.1-m0-hand-calcs.md); the tests use 602. | Done |
| F3.16 | **Power Drain's "spell or chant"**: the DSL has `CastingSpell` but no chant filter. | Encoded with `CastingSpell` only; no M0 target casts chants. | Deferred → P4 (add a chant filter if a Kournan foe chants) |
| F3.17 | **Units of the same foe share a name in logs** ("Training dummy" three times). | Left as is for M0; the logs carry unit ids. | Deferred → P4 (WP4.9 reports) |
| F3.18 | **Air of Superiority's benefit chances, heal (50) and energy (5)** come from the wiki's notes table. | Kept as named constants in the handler with the table in its doc comment (they are wiki values, so ENG-4 allows them in code); only the duration is a skill-file parameter. | Done |
| F3.19 | **The plan wants an `insta` snapshot of the combat log** (T3.9.5). | Snapshotted the first 40 lines of the seed-1 M0 fight (`crates/gwsim-engine/tests/snapshots/`). Any change to engine behaviour or the RNG shows up there first; accept a deliberate change with `cargo insta review`. | Done |
| F3.20 | **The legality tests copy the real data tree** minus skills and creatures; the new parties, encounters and situations then pointed at missing files. | The fixture now also skips `parties/`, `encounters/` and `situations/`. | Done |
| F3.21 | **The description renderer ignores a trigger's filter**: Mistrust renders as "the next time a spell is cast…" without "on one of your allies", and its sentence is clumsy. `gwsim data describe` also showed no sentence for handler skills, because the CLI passed no handler registry. | The CLI now passes the engine's registry, so Arcane Echo and Air of Superiority describe themselves. The trigger-filter wording is left for WP4.1, when many more triggered skills are encoded. | Deferred → P4 (WP4.1) |

**Closed by P3:** none of §1–§6.

**Closed by P2:** **E4** (`data/skills/` now holds 83 seeded skills) and **E6**
(Lamentation is id 916, Blood of the Master id 120, both parsed and checked in
T2.4.8). **E7** is unblocked by E6 and moves to T4.9.1, where the benchmark
files are written.

## 8. Done and removable

Items move here briefly when closed, then leave.

| # | Item | Outcome |
| --- | --- | --- |
| O2 | **Promote `data/core/*.ron` to `Reviewed`.** | **Closed 2026-09-22: reviewed by the owner.** All seven files now carry `review: Reviewed, reviewed_by: Some("Jake Mansell")`. |
| O3 | **Confirm A-033, the player's runes.** | **Closed 2026-09-23 from the current PvX page:** superior Domination Magic (head), minor Fast Casting, minor Inspiration Magic, superior Vigor, Vitae. A-033 is `Confirmed` and is no longer an inference. Player health 405 → 465; Inspiration stays at 9. An intermediate loadout that dropped minor Inspiration for a second Vitae was corrected before it reached `main`. |
| O4 | **Decide the hero weapon assumption.** | **Closed 2026-09-23: no assumption needed.** The current PvX page names every weapon set, so this became sourced data rather than a guess. Heroes 4–6 carry 40/40 sets (Death, Restoration, Restoration); hero 7 carries a Spawning Power staff. Heroes 4–6 move 30 → 42 energy, hero 7 30 → 40. |
| O5 | **Merge `setup/p0` and push.** | **Closed 2026-09-23.** P0 and P1 each merge into `main` through their own branch (`setup/p0`, `data/p1`), all pushed to `jjunkiee/gwsim`. |
| O6 | **Three hero slots ran the melee-player variant.** | **Closed 2026-09-23: switched to the caster variants.** Hero 2 Flesh of My Flesh → Resurrection Chant (and Me/Rt → Me/Mo), hero 4 Withering Aura → Blood of the Master, hero 6 Splinter Weapon → Lamentation. The roster drops from 72 skills to 71. Left behind as E6, E7 and E8. |
| O7 | **The PvX team page carries an update warning.** | **Closed 2026-09-23: ignored by the owner.** §20.1 keeps the page's Great / Meta rating. |
| G1 | **`gwsim template encode` did not accept a build file.** | **Closed 2026-09-23.** It now tries `Build`, then `SkillTemplate`, then `EquipmentTemplate`. A build prints both codes, with the equipment one labelled as the partial copy it is. |
| G2 | **`data describe --all` was declared but never read.** | **Closed 2026-09-23.** `--all` now means every skill and clap rejects it alongside a filter; a bare `describe` asks what you want rather than dumping the tree. |
| G3 | **Benchmark skill ids were not validated.** | **Closed 2026-09-23.** `validate` reports codes that do not decode, naming the slot. Whether gwsim *has* the skills moved to `coverage` — see the note below. |
| P4 | **`cargo install --path` was only in troubleshooting.** | **Closed 2026-09-23.** It is now in the README's first-run section, where someone tired of typing `cargo run -p gwsim-cli --` will find it. |
| O1 | **T1.5.9 — review the effect DSL's shape.** | **Closed 2026-09-22: approved as it stands**, to be evolved during implementation rather than reviewed up front. The packet was never built, so no M1 skill has been encoded in the real types and rendered. The shape questions it would have raised are now Q9. |
