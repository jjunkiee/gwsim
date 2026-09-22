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
are in §7 with their outcomes. New ones go here rather than being guessed at.

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

## 7. Done and removable

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
