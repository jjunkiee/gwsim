# Fallout tasks

Small outstanding items that fell out of finished work. Each one is here
because it was **noticed and deliberately not done**, not because it was
forgotten.

The phase files say what a phase set out to do. This file says what it left
behind. An item leaves when it is done, or when it is absorbed into a real
task in a phase file.

- **Status:** 2026-09-22, after P1.
- **Format:** each item says what it is, why it was left, and where it goes.

---

## 1. Needs the owner

Things I cannot decide or do.

| # | Item | Why it needs you |
| --- | --- | --- |
| O1 | **T1.5.9 — review the effect DSL's shape.** The one P1 task still formally `Todo`. See [effect-dsl.md](../effect-dsl.md); the five M1 encodings to judge it against are in [T1.5.1](../findings/T1.5.1-dsl-coverage.md). | P3 builds an interpreter on these types. Changing them after that is expensive. |
| O2 | **Promote `data/core/*.ron` from `Draft` to `Reviewed`.** T1.1.4 asked for `Reviewed`; I set `Draft` because §8.7 defines `Reviewed` as "reviewed by the owner or a second contributor" and I am neither. | Self-certifying a review defeats the point of having one. The values are cross-checked against the wiki's own worked tables in T1.1.5. |
| O3 | **Confirm A-033, the player's runes.** PvX gives effective ranks but names no runes. A-033 infers three, which leaves two armor slots empty and is the *only* reason the player has 405 maximum health where every hero has 430. | A 25-health difference that comes from an inference, not a source. Worth a second opinion before it shows up in a result. |
| O4 | **Decide the hero weapon assumption.** §20.1 names weapons for the player and heroes 1–3 only. Heroes 4–7 compute to 30 energy rather than 42. | Inventing gear for four party members is a bigger call than I should make alone. Wants an assumption in A-033's spirit. |
| O5 | **Merge `setup/p0` and push.** Both P0 and P1 live on that branch. A remote now exists (`jjunkiee/gwsim`). | Committing and pushing is yours (C5). |

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

Nothing yet. Items move here briefly when closed, then leave.
