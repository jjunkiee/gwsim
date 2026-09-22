# Fallout tasks

Small outstanding items that fell out of finished work. Each one is here
because it was **noticed and deliberately not done**, not because it was
forgotten.

The phase files say what a phase set out to do. This file says what it left
behind. An item leaves when it is done, or when it is absorbed into a real
task in a phase file.

- **Status:** 2026-09-23, after P1. O1–O4 closed; O6 and O7 opened by the PvX page that closed O4.
- **Format:** each item says what it is, why it was left, and where it goes.

---

## 1. Needs the owner

Things I cannot decide or do.

| # | Item | Why it needs you |
| --- | --- | --- |
| O5 | **Merge `setup/p0` and push.** Both P0 and P1 live on that branch. A remote now exists (`jjunkiee/gwsim`). | Committing and pushing is yours (C5). |
| O6 | **Three hero slots run the melee-player variant while the player is a caster.** PvX splits four optional slots by player type. The caster column gives hero 2 Resurrection Chant, hero 4 Blood of the Master and hero 6 Lamentation; §20.1 has Flesh of My Flesh, Withering Aura and Splinter Weapon, which are the melee picks. Two have written rationales (no Blood Magic points; the 2026-08-26 Splinter Weapon AI fix) that were formed before the split was visible. | The player is **Me/— Energy Surge**, a caster, so the caster column is the one that applies. Either the picks change or the rationales need to say why they beat PvX's own advice. |
| O7 | **The PvX team page carries an update warning.** It flags the 2026-06-24 build update — mesmer nerfs, buffs to competing builds — and asks for the article to be rewritten and possibly re-rated. §20.1 records the page as Great / Meta. | M1's whole party is this page. If it gets re-rated or rewritten, the benchmark M1 is built to reproduce moves. |

---

## 2. Gaps in tasks marked Done

**These are honest admissions.** Each task's main goal is met and its done
criterion passes, but one listed action is not implemented. They are here
rather than left as a silently incomplete tick.

| # | Item | Where |
| --- | --- | --- |
| G1 | **`gwsim template encode` does not accept a build file.** T1.4.9 action 3 asked for it. `Build::to_templates` exists and is tested; the CLI only reads a `SkillTemplate` or `EquipmentTemplate`. | `crates/gwsim-cli/src/template.rs` |
| G2 | **`data describe --all` is declared but never read.** The flag parses and does nothing. Either wire it up or remove it — a flag that silently does nothing is worse than no flag. | `crates/gwsim-cli/src/describe.rs` |
| G3 | **Benchmark skill ids are not validated.** T1.2.8 item 1 deferred this until WP1.3 existed. WP1.3 now exists, so a benchmark's `skill_code` can be decoded and its ids checked against the skill files. | `crates/gwsim-data/src/checks.rs` |
| G4 | **`armor_profile` never returns conditional bonuses.** The `conditional` field exists and is always empty, because insignia conditions are DSL values and nothing evaluates them yet. Four of the five M1 insignias are conditional, so a build's real armor can be up to 15 higher than reported. | `crates/gwsim-data/src/derived.rs`, needs WP4.3 |
| G5 | **Part of the 136 Elementalist energy vector is asserted as a constant.** 98 is computed through `energy()`; the remaining 38 comes from insignia and inscription *effects*, which have no typed form. The test documents the split. | `crates/gwsim-data/tests/wiki_examples.rs` |

---

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
| P4 | **`cargo install --path crates/gwsim-cli`** gives a bare `gwsim` command instead of `cargo run -p gwsim-cli --`. Worth mentioning in the README's getting-started rather than only in troubleshooting. | |

---

## 7. Done and removable

Items move here briefly when closed, then leave.

| # | Item | Outcome |
| --- | --- | --- |
| O2 | **Promote `data/core/*.ron` to `Reviewed`.** | **Closed 2026-09-22: reviewed by the owner.** All seven files now carry `review: Reviewed, reviewed_by: Some("Jake Mansell")`. |
| O3 | **Confirm A-033, the player's runes.** | **Closed 2026-09-23 from the current PvX page:** superior Domination Magic (head), minor Fast Casting, minor Inspiration Magic, superior Vigor, Vitae. A-033 is `Confirmed` and is no longer an inference. Player health 405 → 465; Inspiration stays at 9. An intermediate loadout that dropped minor Inspiration for a second Vitae was corrected before it reached `main`. |
| O4 | **Decide the hero weapon assumption.** | **Closed 2026-09-23: no assumption needed.** The current PvX page names every weapon set, so this became sourced data rather than a guess. Heroes 4–6 carry 40/40 sets (Death, Restoration, Restoration); hero 7 carries a Spawning Power staff. Heroes 4–6 move 30 → 42 energy, hero 7 30 → 40. |
| O1 | **T1.5.9 — review the effect DSL's shape.** | **Closed 2026-09-22: approved as it stands**, to be evolved during implementation rather than reviewed up front. The packet was never built, so no M1 skill has been encoded in the real types and rendered. The shape questions it would have raised are now Q9. |
