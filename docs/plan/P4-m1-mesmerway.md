# P4 M1: 7 Hero Mesmerway vs Kournans (evaluator, command line)

**Phase goal:** `gwsim evaluate` runs the M1 party against every M1 situation, deterministically. The party is 7 Hero Mesmerway (Dual Resto) plus the player's Energy Surge bar; the situations are the dummies, three Kournan archetype groups, the curated Kournan patrol in Hard Mode, and a two-fight chain. Every one of the 72 skills is encoded and reviewed. Foe, hero, minion, spirit and player AI behave as documented. The tactics plans are generated. Reports 1, 2, 4, 5 and 7 are produced. The relative checks RC1, RC2 and RC6 pass. One fight simulates in under 10 ms.

- **Design refs:** §8.3–§8.5, §10, §11, §12, §14, §15, §17.3–§17.6, §20, §21, D22–D26, Q30–Q35, Q37, Q40, Q41.
- **Starts when:** M0 is done (P3), and the M1 files are seeded (T2.6.5).
- **Ends when:** every M1 criterion holds (see "M1 acceptance" at the end).
- **Suggested order:**
  1. **Start together:** WP4.2 (foe data), WP4.3 (mechanics) and WP4.1 (skill batches), interleaved: each batch needs the mechanics it uses.
  2. **Then:** WP4.4, WP4.5 and WP4.6.
  3. **Then:** WP4.7 and WP4.8.
  4. **Then:** WP4.9.
  5. **Then:** WP4.10 and WP4.11.
  6. **Throughout:** keep a fixed-seed patrol fight running from as early as possible, and read its log after every batch.

| WP | Title | Goal | Status |
| --- | --- | --- | --- |
| 4.1 | Encode the M1 skills | All 72 skills encoded, tested and `Reviewed`. | Todo |
| 4.2 | Kournan foe data | The eight Kournan foes are complete, with every gap labelled as an assumption. | Todo |
| 4.3 | Mechanics for M1 | Every §20.5 mechanic is implemented and tested. | Todo |
| 4.4 | `FoeAi` | Foes behave as the wiki documents, in NM and HM. | Todo |
| 4.5 | `HeroAi`, `MinionAi`, `SpiritAi` | Heroes act as documented, quirks and 2026 changes included; minions and spirits act by rule. | Todo |
| 4.6 | `PlanAi` generation | The player's priority plan is generated from the build, and the user can edit it. | Todo |
| 4.7 | Tactics plans | Per-situation tactics are generated from builds and can be overridden. | Todo |
| 4.8 | Encounters, situations, chains | The six M1 situations and the M1 set exist and run, and chains carry state over. | Todo |
| 4.9 | Command line and reports | `evaluate`, `compare`, `log`, `template` and `data`, with reports 1, 2, 4, 5 and 7. | Todo |
| 4.10 | Relative checks | RC1, RC2 and RC6 run in `gwsim check` and in CI, and they pass. | Todo |
| 4.11 | Performance | One M1 fight runs in under 10 ms on one core, with a CI guard. | Todo |

---

## WP4.1 Encode the remaining M1 skills

**Goal:** encode the 64 M1 skills not done in M0, in batches by profession:

1. Mesmer;
2. Ritualist;
3. Necromancer;
4. Paragon;
5. Monk;
6. PvE-only;
7. the Kournan foe skills.

Each gets DSL effects or a handler, AI hints and role tags. Each passes its generated tests, and the owner reviews it.

- **Refs:** §8.3–§8.6, §17.3, §17.5, §20.3, §20.4, D11, D28.
- **Depends on:** M0; T2.6.5; the WP4.3 mechanics each batch uses.
- **Done when:** all 72 M1 skills are `Reviewed`, and their generated tests pass.

### Batch procedure

Every batch task below follows these steps.

1. **Research.** Read each skill's wiki page in full (description, notes, related updates) and the §20.4 changes. Confirm the formulaic, conditional or handler classification from T1.5.1. Note open questions, and record them as assumptions where the wiki is silent.
2. **Numbers.** Confirm that the seeded numbers are present and correct (T2.4.8 checked them).
3. **Encode.** Write the DSL effects and effect definitions, or a `handler` reference with `params`. Add AI hints (from the per-skill hero and foe rules in T4.4.1 and T4.5.1) and role tags. Set the status to `Draft`.
4. **Handlers.** Implement each handler in `crates/gwsim-engine/src/handlers/<slug>.rs`, with unit tests for its behaviour and `describe`.
5. **Test.** Run the generated per-skill tests (T4.1.1). Add a micro-scenario for any non-trivial behaviour; for example, "Shelter caps a big hit".
6. **Review sheet.** Run `gwsim data describe --profession <p> --status draft`. The output lists generated text and wiki links only, so it is safe to share.
7. **Review (owner).** The owner compares each description with the wiki page and spot-checks behaviour in the fixed-seed patrol log. Fix what they flag, then set `Reviewed` with `reviewed_by`.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T4.1.1 | Generated per-skill tests and batch tooling | Build | M0 | Todo |
| T4.1.2 | Mesmer batch (3) | Data / Build | T4.1.1, T4.3.3, T4.3.4 | Todo |
| T4.1.3 | Ritualist batch A: spirits and Communing (10) | Data / Build | T4.1.1, T4.3.7 | Todo |
| T4.1.4 | Ritualist batch B: Restoration and Channeling (8) | Data / Build | T4.1.3, T4.3.9, T4.3.10 | Todo |
| T4.1.5 | Necromancer batch (7) | Data / Build | T4.1.1, T4.3.5, T4.3.8 | Todo |
| T4.1.6 | Paragon batch (3) | Data / Build | T4.1.1, T4.3.11 | Todo |
| T4.1.7 | Monk batch (2) | Data / Build | T4.1.1, T4.3.4, T4.3.10 | Todo |
| T4.1.8 | PvE-only batch (Air of Superiority, confirm) | Review | T3.10.8 | Todo |
| T4.1.9 | Kournan batch 1: Guard, Zealot, Phalanx (12) | Data / Build | T4.1.1, T4.3.13 | Todo |
| T4.1.10 | Kournan batch 2: Bowman, Scribe (10) | Data / Build | T4.1.9, T4.3.7, T4.3.12 | Todo |
| T4.1.11 | Kournan batch 3: Seer, Oppressor, Priest (9) | Data / Build | T4.1.9, T4.3.4 | Todo |
| T4.1.12 | Data authoring guide: skills and handlers | Docs | T4.1.2 | Todo |

### T4.1.1 Generated per-skill tests and batch tooling

**Type:** Build · **Depends on:** M0

1. Add `crates/gwsim-engine/tests/skills.rs`, a single table-driven test over every `Draft` and `Reviewed` skill in `data/` (§17.3). For each skill it checks that:
   - every `Scaled` value at ranks 0, 12 and 15 equals the provenance triple;
   - the costs, activation and recharge equal the extracted numbers;
   - `describe` renders without placeholders;
   - the handler (if any) is registered.
2. On failure, print one readable line per skill ("energy-surge: Scaled #1 at rank 12 = 97, extracted = 96"), so a batch sees every problem at once.
3. Extend `gwsim data describe` with `--review-sheet`, which prints a markdown table (name, wiki link, type and costs, generated text, role tags, status) for a filter.

- **Done when:** the test runs over the 8 M0 skills and passes, and the review sheet prints.

### T4.1.2 Mesmer batch

**Type:** Data / Build · **Depends on:** T4.1.1, T4.3.3, T4.3.4

1. **Skills:** Panic (elite; handler), Shatter Hex and Drain Enchantment. The other 7 Mesmer skills were done in M0.
2. **Research focus:**
   - Panic's trigger and its interaction with nearby foes;
   - Shatter Hex's removal and damage rule;
   - Drain Enchantment's energy gain and removal.
3. **Handler:** `panic`.

- **Done when:** the three skills are `Reviewed`.

### T4.1.3 Ritualist batch A: spirits and Communing

**Type:** Data / Build · **Depends on:** T4.1.1, T4.3.7

1. **Skills:**
   - Shelter, Union and Displacement (handlers);
   - Armor of Unfeeling;
   - Soul Twisting (elite; handler);
   - Boon of Creation;
   - Signet of Creation;
   - Life (handler);
   - Recuperation;
   - Signet of Spirits (elite).
2. **Research focus:**
   - binding-ritual spirit rules (spirit level, range, health scaling with Spawning Power, A-015);
   - each spirit's party aura (damage cap, damage reduction, redistribution or block);
   - Soul Twisting's changes to ritual cost and recharge, including the 2026-08-26 minimum cost of 5;
   - what Life does when it ends or dies;
   - which of these a hero pre-casts (T4.5.1).
3. **Handlers:** `shelter`, `union`, `displacement`, `soul_twisting`, `life`. Each hooks into the damage pipeline (T3.5.5) or the ritual pipeline, and credits mitigation to its caster (§14.2).

- **Done when:** the ten skills are `Reviewed`, with micro-scenarios for each spirit's mitigation.

### T4.1.4 Ritualist batch B: Restoration and Channeling

**Type:** Data / Build · **Depends on:** T4.1.3, T4.3.9, T4.3.10

1. **Skills:**
   - Spirit Transfer;
   - Mend Body and Soul;
   - Spirit Light;
   - Protective Was Kaolai (elite; handler);
   - Ancestors' Rage;
   - Spirit Siphon;
   - Lamentation.
2. **Research focus:**
   - the "near a spirit" and "if you control a spirit" conditions;
   - Protective Was Kaolai's 2026-08-26 armor and healing values and its trigger;
   - Spirit Siphon's 2026 energy change;
   - **Lamentation, which no P1 research covered** — it replaced Splinter Weapon on 2026-09-23 when the party moved to PvX's caster column, so it needs its skill id, its values and a DSL classification before it can be encoded.
3. **Handlers:** `protective_was_kaolai`.

- **Done when:** the seven skills are `Reviewed`.

### T4.1.5 Necromancer batch

**Type:** Data / Build · **Depends on:** T4.1.1, T4.3.5, T4.3.8

1. **Skills:**
   - Blood is Power (elite);
   - Blood Bond (handler);
   - Signet of Lost Souls;
   - Animate Bone Fiend (handler);
   - Putrid Bile (handler);
   - Masochism;
   - Blood of the Master.
2. **Research focus:**
   - Blood is Power's sacrifice and energy regeneration (and the hero battery rule, AI-H6);
   - Animate Bone Fiend's 2026 cost and activation, corpse exploitation and minion level;
   - Putrid Bile's 2026 duration and its on-death explosion;
   - Blood Bond's per-hit healing;
   - Signet of Lost Souls' conditional heal and energy;
   - Masochism's energy on sacrifice.
3. **Handlers:** `blood_bond`, `animate_bone_fiend`, `putrid_bile`.

- **Done when:** the seven skills are `Reviewed`.

### T4.1.6 Paragon batch

**Type:** Data / Build · **Depends on:** T4.1.1, T4.3.11

1. **Skills:** "Incoming!", "Fall Back!" and "Stand Your Ground!".
2. **Research focus:**
   - Command scaling;
   - shout range (earshot, party);
   - "Fall Back!"'s speed and the rule that ends it early;
   - how shouts are used while knocked down;
   - the hero rule of using shouts only in combat, except speed boosts (AI-H6).

- **Done when:** the three skills are `Reviewed`.

### T4.1.7 Monk batch

**Type:** Data / Build · **Depends on:** T4.1.1, T4.3.4, T4.3.10

1. **Skills:** Resurrection Chant (handler) and Remove Hex.
2. **Research focus:**
   - Resurrection Chant's long activation, and the health and energy it restores;
   - Remove Hex's removal rule.
3. **Handler:** `resurrection_chant`.

- **Done when:** both skills are `Reviewed`.

### T4.1.8 PvE-only batch: Air of Superiority

**Type:** Review · **Depends on:** T3.10.8

1. Air of Superiority was encoded in M0. Confirm it with M1 conditions:
   - the default account profile's maximum Asura rank (Q14);
   - Kournans give experience;
   - the outcome distribution matches T3.10.1.
2. No other PvE-only skill is in M1.

- **Done when:** the skill is `Reviewed` under M1 conditions.

### T4.1.9 Kournan batch 1: Guard, Zealot, Phalanx

**Type:** Data / Build · **Depends on:** T4.1.1, T4.3.13

1. **Skills:**
   - **Guard:** Disrupting Chop, Executioner's Strike, Magehunter Strike (elite) and Sprint;
   - **Zealot:** Armor of Sanctity, Eremite's Attack, Pious Renewal (elite) and Veil of Thorns;
   - **Phalanx:** "Never Surrender!", Cautery Signet (elite), Mighty Throw and Wild Throw.
2. **Research focus:**
   - adrenaline costs and gain;
   - the attack-skill timing (ENG-15);
   - scythe behaviour for Eremite's Attack;
   - spear projectiles;
   - Disrupting Chop's interrupt;
   - Magehunter Strike's conditions;
   - Strength and Expertise effects (T4.3.2).
3. Classify each skill (T1.5.1 did it first) and write handlers only where needed.
4. **Foe AI hints:** when each foe uses each skill, from the Foe page's rules and T4.4.1.

- **Done when:** the twelve skills are `Reviewed`.

### T4.1.10 Kournan batch 2: Bowman, Scribe

**Type:** Data / Build · **Depends on:** T4.1.9, T4.3.7, T4.3.12

1. **Skills:**
   - **Bowman:** Crossfire, Infuriating Heat (elite, nature ritual), Precision Shot, Troll Unguent and Whirling Defense;
   - **Scribe:** Aftershock, Aura of Restoration, Fireball, Master of Magic (elite) and Meteor.
2. **Research focus:**
   - Infuriating Heat as a foe nature ritual that replaces allied **and** enemy spirits of the same type (ENG-32, T4.3.7);
   - Whirling Defense's block and armor-ignoring damage;
   - Meteor's and Aftershock's knockdown [confirm];
   - Master of Magic's attribute boost;
   - Aura of Restoration's healing on casting.

- **Done when:** the ten skills are `Reviewed`.

### T4.1.11 Kournan batch 3: Seer, Oppressor, Priest

**Type:** Data / Build · **Depends on:** T4.1.9, T4.3.4

1. **Skills:**
   - **Seer:** Enchanter's Conundrum (elite), Power Spike and Shatter Enchantment;
   - **Oppressor:** Life Siphon and Strip Enchantment;
   - **Priest:** Convert Hexes, Reversal of Fortune, Shielding Hands and Zealous Benediction (elite).
2. **Research focus:**
   - interrupts on the party (Power Spike);
   - enchantment removal on the party (which matters for Protective Was Kaolai and Boon of Creation);
   - Reversal of Fortune's damage-to-healing conversion;
   - Shielding Hands' damage reduction or block [confirm];
   - Zealous Benediction's healing and energy;
   - Divine Favor's bonus healing (T4.3.2).

- **Done when:** the nine skills are `Reviewed`.

### T4.1.12 Data authoring guide: skills and handlers

**Type:** Docs · **Depends on:** T4.1.2

1. Extend `docs/data-authoring.md`:
   - the batch procedure above;
   - how to choose between the DSL and a handler, with examples from M1;
   - how to write a handler (trait, registry, tests, `describe`);
   - how to write AI hints and role tags;
   - how to run the review sheet.

- **Done when:** a new contributor could encode a formulaic skill from the guide alone.

---

## WP4.2 Kournan foe data

**Goal:** the eight Kournan foe files are complete and `Reviewed`. Armor has the right level context, HM attributes and weapons are set, and AI tags are added. Every value the wiki lacks is a labelled assumption.

- **Refs:** §7.1 (Foe), §10.4, §20.2, A-004, A-005, A-007, A-032.
- **Depends on:** T2.6.5.
- **Done when:** the foe files are `Reviewed`, their spawned HM-26 stats match hand values, and A-004, A-005 and A-007 have values.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T4.2.1 | Kournan armor, ranks, weapons and behaviour | Research | T2.5.7 | Todo |
| T4.2.2 | Complete the foe files | Data | T4.2.1 | Todo |
| T4.2.3 | Foe spawn tests | Test | T4.2.2 | Todo |
| T4.2.4 | Owner review of the foe data | Review | T4.2.3 | Todo |

### T4.2.1 Kournan armor, ranks, weapons and behaviour

**Type:** Research · **Depends on:** T2.5.7

1. **Armor tables:** for each foe page, establish what level the armor table refers to (NM 20 or HM 26?), and what the "a / b" pairs mean (for example, Guard 116 / 96 and Bowman 70 / 100) in terms of damage types. Compare with the rule `3 × level + profession bonus`.
2. **HM attribute ranks:** where the page gives them, use them. Otherwise apply A-005 (NM + 5, capped at 20). Note Bowman's +2 exception.
3. **Weapons:**
   - Guard: axe (A-007);
   - Zealot: scythe;
   - Phalanx: spear;
   - Bowman: bow (which type?);
   - Scribe, Seer, Oppressor and Priest: which caster weapon?

   Where the wiki is silent, use A-004's player weapon tables at the foe's level.
4. **Behaviour tags:** does the Priest kite (AI-F5)? Are any foes stationary?
5. **HM recharge (A-032):** look for the number again, if T1.1.1 and T3.4.1 didn't find it.

- **Output:** `docs/findings/T4.2.1-kournan-data.md`, plus the assumption updates.

### T4.2.2 Complete the foe files

**Type:** Data · **Depends on:** T4.2.1

1. Fill in each foe's levels, attributes (NM and HM), HM-only skills, armor (with its level context), weapon, `ai_tags` and variants. Put the assumption IDs in `provenance.assumptions`.
2. Update `data/assumptions.ron` for A-004, A-005, A-007 and A-032.

- **Done when:** `gwsim data validate` passes.

### T4.2.3 Foe spawn tests

**Type:** Test · **Depends on:** T4.2.2

1. For each foe spawned at HM 26, assert its health (the level formula), energy and regeneration, attribute ranks, armor per damage type, weapon, and skill bar (including HM-only skills), against the hand values in T4.2.1.

- **Done when:** the tests pass.

### T4.2.4 Owner review of the foe data

**Type:** Review · **Depends on:** T4.2.3 · **Owner:** reviews

1. The owner reviews each foe file next to its wiki page.
2. On approval, set `Reviewed`.

- **Done when:** all eight foes are `Reviewed`.

---

## WP4.3 Mechanics M1 needs

**Goal:** every mechanic on the §20.5 checklist works in the engine and has a test. Each skill batch can then rely on the mechanics it uses.

- **Refs:** §10.3–§10.12, §20.5, ENG-30 to ENG-35.
- **Depends on:** P3.
- **Done when:** `tests/m1_mechanics.rs` has a passing test for every §20.5 item (T4.3.15).

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T4.3.1 | Inherent attribute effects | Research | — | Todo |
| T4.3.2 | Implement the inherent attribute effects | Build | T4.3.1 | Todo |
| T4.3.3 | Casting mechanics | Build | — | Todo |
| T4.3.4 | Hex and enchantment removal; maintained effects | Build | — | Todo |
| T4.3.5 | Energy mechanics | Build | T4.3.2 | Todo |
| T4.3.6 | Area effects | Build | — | Todo |
| T4.3.7 | Spirits | Build | T4.3.2 | Todo |
| T4.3.8 | Minions and corpses | Build | T4.3.2 | Todo |
| T4.3.9 | Weapon spells | Build | — | Todo |
| T4.3.10 | Death, resurrection, DP and morale | Build | — | Todo |
| T4.3.11 | Shouts, chants and speed boosts | Build | — | Todo |
| T4.3.12 | Conditions, snares and knockdown from foe skills | Build | — | Todo |
| T4.3.13 | Foe attacks, adrenaline, blocks and damage conversion | Build | T4.3.2 | Todo |
| T4.3.14 | Gear effects | Build | — | Todo |
| T4.3.15 | Mechanics checklist tests | Test | T4.3.2–T4.3.14 | Todo |

### T4.3.1 Inherent attribute effects

**Type:** Research · **Depends on:** —

1. From the wiki pages for Fast Casting, Soul Reaping, Spawning Power, Strength, Expertise, Divine Favor, Mysticism, Leadership, Death Magic (minion cap) and Critical Strikes (for later), record:
   - each effect's formula;
   - the Reforged-era Soul Reaping rules and cap (§10.9);
   - Spawning Power's effect on creature health and on weapon-spell duration;
   - Strength's armor penetration for attack skills;
   - Expertise's cost reduction (which skill types);
   - Divine Favor's bonus healing (when it applies);
   - Mysticism's Dervish effects;
   - Leadership's energy per ally for shouts and chants.

- **Output:** `docs/findings/T4.3.1-inherent-attributes.md`.

### T4.3.2 Implement the inherent attribute effects

**Type:** Build · **Depends on:** T4.3.1

1. Implement each inherent effect as an engine rule, keyed by the `inherent` tag in `attributes.ron` and applied at spawn or at the relevant hook:
   - Soul Reaping: energy on nearby non-spirit deaths, with its cap;
   - Spawning Power: creature health +4% per rank, and weapon-spell duration;
   - Strength, Expertise, Divine Favor, Mysticism and Leadership.

   Fast Casting was done in WP3.4.

- **Done when:** there is one test per attribute.

### T4.3.3 Casting mechanics

**Type:** Build · **Depends on:** —

1. Interrupts that tell "using a skill" from "casting a spell" (Cry of Frustration, Power Drain and Power Spike have different targets).
2. Forced spell failure.
3. Dazed: easily interrupted, and interrupts when applied.
4. Energy-based damage (Energy Surge: done in M0; re-test here).

- **Done when:** each has a test.

### T4.3.4 Hex and enchantment removal; maintained effects

**Type:** Build · **Depends on:** —

1. Removal skills take effects in the order from T3.6.1 (Shatter Hex, Remove Hex, Convert Hexes, Drain Enchantment, Shatter Enchantment, Strip Enchantment), and their "if removed" follow-ups fire.
2. Maintained enchantments are removable. Removing one ends its upkeep.

- **Done when:** a removal test passes for each removal skill family.

### T4.3.5 Energy mechanics

**Type:** Build · **Depends on:** T4.3.2

1. Gain, loss and drain (drain moves energy from the target to the caster).
2. Energy regeneration pips.
3. Soul Reaping (T4.3.2).
4. Health sacrifice.
5. Batteries: Blood is Power's energy regeneration on another unit.

- **Done when:** each has a test.

### T4.3.6 Area effects

**Type:** Build · **Depends on:** —

1. Area damage uses the range bands. Secondary targets take 75% (the 2026 Mesmer rule, `Secondary(0.75)`).
2. The summoned-creature tag is set on created creatures, for skills that treat them differently.

- **Done when:** Energy Surge's and Mistrust's secondary damage is 75% in a three-dummy test.

### T4.3.7 Spirits

**Type:** Build · **Depends on:** T4.3.2

1. Add spirit creation from ritual skills.
2. Spirit behaviour:
   - stationary;
   - level from the skill;
   - health and armor from `spirits.ron` (A-015), with health scaled by Spawning Power;
   - killable;
   - hostile spirits have 100 armor in HM.
3. Aura effects apply to allies (or foes) within the spirit's range (earshot or spirit range, per the skill), and are re-evaluated as units move.
4. One spirit per type: binding rituals replace an allied spirit of the same type; nature rituals replace allied and enemy spirits of the same type (ENG-32).
5. Fill in `data/creatures/spirits.ron` for the M1 spirits.

- **Done when:** the tests cover aura range, replacement rules, Spawning Power health and spirit death.

### T4.3.8 Minions and corpses

**Type:** Build · **Depends on:** T4.3.2

1. **Corpses:** fleshy creatures leave exploitable corpses, and each can be used once.
2. **Bone Fiend:**
   - armor `2.84 × level + 3.1`;
   - a ranged piercing attack every 1.86 s at half longbow range;
   - level from the skill (§10.10).
3. **Degeneration:** starts at −1 and worsens by 1 pip every 20 s.
4. **Control cap:** 2 + ⌊Death Magic / 2⌋ (§17.2). When a minion over the cap is raised, remove the oldest, per the Minion page.
5. **Traits:** minions aren't fleshy.
6. Fill in `data/creatures/minions.ron`.

- **Done when:** the §17.2 minion-cap row passes (8 at 12), along with the degeneration schedule and corpse-once tests.

### T4.3.9 Weapon spells

**Type:** Build · **Depends on:** —

1. Only one weapon spell can be on a target: a new one replaces the old.
2. Weapon spells can't be cast on spirits.
3. Weapon-spell duration scales with Spawning Power (T4.3.2).

- **Done when:** a replacement test passes.

### T4.3.10 Death, resurrection, DP and morale

**Type:** Build · **Depends on:** —

1. Finish the death rules (§10.11): which effects clear (T3.6.1); corpses stay until used.
2. Resurrection restores a percentage of health and energy (Resurrection Chant, which two heroes now carry). Its sacrifice is handled by the skill.
3. Death Penalty is −15% per death, down to −60%, and reduces maximum health and energy. Morale boost goes up to +10%.
4. Starting DP and morale come from the situation.
5. The Dhuum's Covenant switch records "covenant broken" on any party death. The run continues [Proposed].
6. Resurrection shrines are not modelled.

- **Done when:** the tests cover DP after 2 deaths (−30%), resurrection values and the covenant flag.

### T4.3.11 Shouts, chants and speed boosts

**Type:** Build · **Depends on:** —

1. Shouts affect the party within earshot. They are instant, usable while knocked down, and don't use the action queue.
2. Chants follow T3.4.1's rules.
3. Speed boosts stack multiplicatively and are capped at +34% (T3.2.3).
4. Leadership energy (T4.3.2).

- **Done when:** the tests pass.

### T4.3.12 Conditions, snares and knockdown from foe skills

**Type:** Build · **Depends on:** —

1. Conditions and snares applied by Kournan skills work through WP3.6.
2. Knockdown from Meteor (and Aftershock, if T4.1.10 confirms it) uses T3.6.9.
3. Movement state is observable: "moving" filters and "stop to cast".

- **Done when:** a Meteor knocks down a casting hero without an interrupt event.

### T4.3.13 Foe attacks, adrenaline, blocks and damage conversion

**Type:** Build · **Depends on:** T4.3.2

1. Foe melee (axe, scythe) and ranged (spear, bow) weapons, with A-004 values and projectiles (T3.2.5).
2. Adrenaline for foe attack skills.
3. Block effects: Whirling Defense, and Shielding Hands [confirm].
4. Damage reduction and conversion on foes: Reversal of Fortune turns damage into healing; Shielding Hands.
5. The HM foe attack-speed bonus (T3.3.3) applies.

- **Done when:** the tests cover a blocked attack, a conversion, and adrenaline building to a skill.

### T4.3.14 Gear effects

**Type:** Build · **Depends on:** —

1. Conditional insignia effects become permanent engine effects (T3.3.2), e.g. an armor bonus while a condition holds. Use the T1.4.1 findings for Prodigy's, Minion Master's, Shaman's, Bloodstained and Tormentor's (holy-damage vulnerability).
2. Rune effects at spawn were done in WP1.4.
3. Weapon inscriptions and the 40/40 chance mods (T3.4.4, T3.4.7).
4. The "+1 attribute (20% chance)" roll per activation.

- **Done when:** there is a test per insignia effect and per chance mod.

### T4.3.15 Mechanics checklist tests

**Type:** Test · **Depends on:** T4.3.2–T4.3.14

1. Write `crates/gwsim-engine/tests/m1_mechanics.rs` with one test per bullet of §20.5, each naming the bullet in its test name (e.g. `m1_spirits_one_per_type`).

- **Done when:** every §20.5 bullet has a passing test.

---

## WP4.4 `FoeAi`

**Goal:** foes aggro, target, use skills, scatter and move the way the wiki documents, with the Hard Mode "superior AI" differences modelled explicitly (Q18, D13).

- **Refs:** §11.1–§11.3, AI-F1 to AI-F8, A-009, A-010, A-016.
- **Depends on:** WP4.3, WP4.1 (AI hints on foe skills).
- **Done when:** each AI-F behaviour has a scenario test, and NM and HM differ where documented.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T4.4.1 | Foe behaviour | Research | — | Todo |
| T4.4.2 | Shared controller helpers | Build | — | Todo |
| T4.4.3 | Aggro and group engagement (AI-F1) | Build | T4.4.2 | Todo |
| T4.4.4 | Targeting (AI-F2) | Build | T4.4.2 | Todo |
| T4.4.5 | Skill use from AI hints (AI-F3) | Build | T4.4.2 | Todo |
| T4.4.6 | Scatter and AoE avoidance (AI-F4) | Build | T4.4.2 | Todo |
| T4.4.7 | Movement styles (AI-F5, AI-F8) | Build | T4.4.2 | Todo |
| T4.4.8 | HM superior AI (AI-F6) | Build | T4.4.3–T4.4.7 | Todo |
| T4.4.9 | Foe AI scenario tests | Test | T4.4.8 | Todo |

### T4.4.1 Foe behaviour

**Type:** Research · **Depends on:** —

1. From `research/` and the wiki pages Foe, Aggro, Scatter, Area of effect, Hard mode and Monster skill, record:
   - aggro range and group aggro (A-009);
   - target preferences;
   - per-skill foe usage rules (e.g. Backfire-type skills only on caster-weapon holders; the NM and HM rules for Blind);
   - scatter triggers and timing (A-016);
   - which foes kite;
   - what "superior AI" changes in HM;
   - anything on reaction delays (A-010).
2. Write down a proposed value for each Pending assumption, with its reasoning.

- **Output:** `docs/findings/T4.4.1-foe-ai.md`, plus the assumption updates.

### T4.4.2 Shared controller helpers

**Type:** Build · **Depends on:** —

1. Add the shared helpers (§11.1):
   - reaction-delay scheduling from the assumptions;
   - decision cadence;
   - validity through `can_use`;
   - targeting helpers (weakest, closest, most energy, casting, hexed, …);
   - movement helpers (approach to range, keep range, flee an area, follow a flag or formation point).
2. Every controller uses them, so the timing rules live in one place.

- **Done when:** unit tests pass for each helper.

### T4.4.3 Aggro and group engagement (AI-F1)

**Type:** Build · **Depends on:** T4.4.2

1. A group engages when a party member enters its aggro range (A-009). All of its members engage together.
2. Before aggro, foes are idle at their positions.

- **Done when:** a party member at aggro range + 1 doesn't trigger aggro, and at aggro range − 1 the whole group engages.

### T4.4.4 Targeting (AI-F2)

**Type:** Build · **Depends on:** T4.4.2

1. Score targets by weakness (health and armor) and distance. Body-blocking can hold melee foes back from their preferred target.
2. Retargeting rules follow T4.4.1.

- **Done when:** a scenario shows a melee foe choosing the lower-armor caster when both are reachable.

### T4.4.5 Skill use from AI hints (AI-F3)

**Type:** Build · **Depends on:** T4.4.2

1. Evaluate each bar skill's `ai.use_when`, `never_when`, target preference and priority. Use the highest-priority usable skill; otherwise attack.
2. Monsters with the same skill use it the same way.

- **Done when:** the Priest heals the lowest-health foe, and the Seer interrupts a casting party member.

### T4.4.6 Scatter and AoE avoidance (AI-F4)

**Type:** Build · **Depends on:** T4.4.2

1. Foes run out of damage-over-time AoE (wells, spirits with damage auras, ground effects) after the A-016 delay. The whole group scatters together.
2. In PvE, foes also step away when targeted by AoE.
3. Single-hit AoE doesn't cause scatter.

- **Done when:** a scenario shows the group leaving a damage-over-time area, and staying for a single Fireball.

### T4.4.7 Movement styles (AI-F5, AI-F8)

**Type:** Build · **Depends on:** T4.4.2

1. Casters keep casting range and don't move while activating.
2. Melee foes chase their target.
3. Kiters (tag) back off when approached.
4. Stationary foes (tag) never move.

- **Done when:** there is a test per style.

### T4.4.8 HM superior AI (AI-F6)

**Type:** Build · **Depends on:** T4.4.3–T4.4.7

1. In HM, foes:
   - react faster to damage-over-time AoE (a shorter A-016 delay);
   - fight longer;
   - ball up less on one target;
   - kite more;
   - skip pure attack-speed skills.
2. Each change is a parameter set from `modes.ron` or the assumptions.

- **Done when:** paired NM and HM scenarios show each difference.

### T4.4.9 Foe AI scenario tests

**Type:** Test · **Depends on:** T4.4.8

1. Write `crates/gwsim-engine/tests/foe_ai.rs` with one scenario per AI-F item (fixed seeds, asserting behaviour from logs or states).

- **Done when:** the tests pass.

---

## WP4.5 `HeroAi`, `MinionAi`, `SpiritAi`

**Goal:** heroes behave as documented, weaknesses included, updated with the 2026 AI changes. Minions follow their master and fight; spirits act by rule.

- **Refs:** §11.1, §11.2, §11.4, AI-H1 to AI-H9, A-011, A-026.
- **Depends on:** WP4.4 (shared helpers), WP4.1 (hero AI hints).
- **Done when:** each AI-H item has a scenario test, and every M1 hero skill has its documented hero rule.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T4.5.1 | Hero behaviour and the 2026 notes | Research | — | Todo |
| T4.5.2 | Targeting and modes (AI-H1, AI-H2) | Build | T4.5.1, T4.4.2 | Todo |
| T4.5.3 | Quirks (AI-H3 to AI-H5) | Build | T4.5.2 | Todo |
| T4.5.4 | Skill-type rules (AI-H6) | Build | T4.5.2 | Todo |
| T4.5.5 | 2026 rules and per-skill tweaks (AI-H7) | Build | T4.5.4 | Todo |
| T4.5.6 | No pre-casting; disabled skills (AI-H8, AI-H9) | Build | T4.5.2 | Todo |
| T4.5.7 | `MinionAi` and `SpiritAi` | Build | T4.4.2, T4.3.7, T4.3.8 | Todo |
| T4.5.8 | Hero AI scenario tests | Test | T4.5.3–T4.5.7 | Todo |

### T4.5.1 Hero behaviour and the 2026 notes

**Type:** Research · **Depends on:** —

1. From `research/Player_versus_Environment/Hero.md`, the wiki's Hero behavior page, Category:Hero-vetted skills (first page), and the 2026 update notes (May 12, June 24, July 29, August 26), record:
   - every general hero rule (AI-H1 to AI-H9);
   - every change that alters one;
   - for each of the 41 M1 party skills, the documented hero usage rule, where one exists.
2. Where the notes and the Hero behavior page disagree, the notes win (A-026). Record each such case.
3. Record the AoE-escape threshold (65%) and the melee "stickiness" rule.

- **Output:** `docs/findings/T4.5.1-hero-ai.md`: general rules, plus a per-skill table.

### T4.5.2 Targeting and modes (AI-H1, AI-H2)

**Type:** Build · **Depends on:** T4.5.1, T4.4.2

1. Target order: player-locked target, then the called target, then the target a player is attacking.
2. In Fight mode with no call, prefer the lowest-armor foe, then the lowest health.
3. Modes:
   - **Fight:** as above;
   - **Guard:** stays at the flag or formation point and fights only when engaged;
   - **Avoid Combat:** never attacks, kites, avoids direct offensive skills, but still uses indirect ones (wards, wells, summons, item spells).

- **Done when:** there is a test per mode and per step of the target order.

### T4.5.3 Quirks (AI-H3 to AI-H5)

**Type:** Build · **Depends on:** T4.5.2

1. Heroes don't coordinate: several may choose the same heal, removal or resurrection target. Hex, condition and interrupt targets are chosen roughly at random, using the AI stream.
2. Interrupts have no reaction delay (A-011).
3. Heroes don't reapply active effects. They read health, hexes, conditions, enchantments and energy.

- **Done when:** a scenario shows two heroes double-removing the same hex; an interrupt lands within the 0 ms reaction delay.

### T4.5.4 Skill-type rules (AI-H6)

**Type:** Build · **Depends on:** T4.5.2

1. Batteries (Blood is Power) go only on casters, or on martials holding a caster weapon, at about 50% energy.
2. Attunements are always cast.
3. Maintained enchantments are dropped out of combat, except the listed exceptions.
4. Shouts are used only in combat, except speed boosts.
5. Wards and wells are cast in combat; wells only when targets are in range.

- **Done when:** there is a test per rule.

### T4.5.5 2026 rules and per-skill tweaks (AI-H7)

**Type:** Build · **Depends on:** T4.5.4

1. Melee heroes stick to their previous target.
2. Heroes tolerate AoE down to 65% health before escaping.
3. Melee heroes give auto-attacks lower priority until low on energy.
4. Add the per-skill rules from the T4.5.1 table to the M1 skills' `ai` blocks (e.g. Protective Was Kaolai's bundle handling). Rules that don't fit the hint vocabulary go in small per-skill hero overrides.

- **Done when:** the per-skill table is fully reflected in the data or the overrides, and each has a test.

### T4.5.6 No pre-casting; disabled skills (AI-H8, AI-H9)

**Type:** Build · **Depends on:** T4.5.2

1. Heroes never pre-cast protection or spirits on their own. Pre-fight casting comes only from the tactics plan (WP4.7).
2. Skills the tactics plan disables are never used automatically.

- **Done when:** the tests pass.

### T4.5.7 `MinionAi` and `SpiritAi`

**Type:** Build · **Depends on:** T4.4.2, T4.3.7, T4.3.8

1. **Minions** follow their master, attack the master's target or the nearest foe in range, and don't flee.
2. **Spirits** are stationary. Aura spirits are passive. Attacking spirits (if any in M1) attack foes in range. Spirit skills, if any, run by their AI hints.

- **Done when:** a Bone Fiend attacks the master's target; a spirit never moves.

### T4.5.8 Hero AI scenario tests

**Type:** Test · **Depends on:** T4.5.3–T4.5.7

1. Write `crates/gwsim-engine/tests/hero_ai.rs` with one scenario per AI-H item, plus a smoke test: the full M1 party against the patrol, where every hero uses at least its main skills.

- **Done when:** the tests pass.

---

## WP4.6 `PlanAi` generation

**Goal:** a human slot's priority plan is generated from its build (DSL, role tags, AI hints), is stored as editable RON with the party, and runs exactly as written with a human reaction delay.

- **Refs:** §11.5, Q19, A-012, A-029.
- **Depends on:** T3.10.5, WP4.1 (role tags and hints).
- **Done when:** the generated M1 player plan matches the §11.5 example (snapshot), and user edits override it.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T4.6.1 | Plan generator | Build | T3.10.5 | Todo |
| T4.6.2 | Plan storage and editing | Build | T4.6.1 | Todo |
| T4.6.3 | Plan generation tests | Test | T4.6.2 | Todo |
| T4.6.4 | Owner review of the player plan | Review | T4.6.3 | Todo |

### T4.6.1 Plan generator

**Type:** Build · **Depends on:** T3.10.5

1. Generate rules from each skill's role tags and hints:
   - self-maintained effects become "maintain" rules;
   - interrupt-role skills become "on a foe casting a spell (or using a skill) in range";
   - energy-denial skills target the foe with the most energy in range;
   - hex and condition removal target the ally with the matching effect;
   - area skills prefer targets with the most foes nearby;
   - healing targets the lowest-health ally below a threshold.
2. Order the rules by hint priority, then role precedence: maintenance, interrupts, removal, damage.
3. The default action is attacking the called target, or the nearest.

- **Done when:** the generator produces a plan for every M1 bar.

### T4.6.2 Plan storage and editing

**Type:** Build · **Depends on:** T4.6.1

1. The plan is a field of the party slot (`ai_overrides.plan`). It is written in full, so users can edit it.
2. When a party file has a plan, it is used as written. Otherwise one is generated.
3. Validation rejects references to skills not on the bar.
4. Remove the hand-written M0 plan file, or keep it as a test fixture.

- **Done when:** the M0 party runs with a generated plan, and an edited plan changes the behaviour.

### T4.6.3 Plan generation tests

**Type:** Test · **Depends on:** T4.6.2

1. Snapshot the generated plan for the M1 player and check it against the §11.5 example.
2. Test that an edited plan is honoured.
3. Test that the reaction delay A-012 is applied.

- **Done when:** the tests pass.

### T4.6.4 Owner review of the player plan

**Type:** Review · **Depends on:** T4.6.3 · **Owner:** reviews

1. The owner reads the generated player plan and judges whether a competent player would play the bar that way. Adjust the generator rules, not just this plan.

- **Done when:** the owner approves.

---

## WP4.7 Tactics-plan generator and overrides

**Goal:** each situation gets a tactics plan generated from the party's builds: formation, hero modes, called targets, pre-fight sequence, disabled skills, and spreading against AoE. The user can override any part. There are no scripted mid-fight actions (Q29).

- **Refs:** §11.6, §20.1 (tactics notes), Q29, Q37, AI-H8, AI-H9.
- **Depends on:** WP4.5.
- **Done when:** the plan generated for the M1 party matches the PvX tactics notes, and overrides are kept while the rest is regenerated.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T4.7.1 | `TacticsPlan` types and RON | Build | — | Todo |
| T4.7.2 | Generator rules | Build | T4.7.1 | Todo |
| T4.7.3 | Pre-fight phase and walk-in | Build | T4.7.1, WP4.8 (T4.8.1) | Todo |
| T4.7.4 | Formation and flags | Build | T4.7.1 | Todo |
| T4.7.5 | Overrides merge | Build | T4.7.2 | Todo |
| T4.7.6 | Tactics tests | Test | T4.7.3–T4.7.5 | Todo |

### T4.7.1 `TacticsPlan` types and RON

**Type:** Build · **Depends on:** —

1. Add the types following §11.6:
   - `formation` (slot to relative position);
   - `hero_modes`;
   - `called_targets` (target rules such as "foe with the Healing role", "foe `kournan-priest`");
   - `locked_targets`;
   - `pre_fight` (an ordered list of slot, skill and target);
   - `disabled_hero_skills`;
   - `engage_order`;
   - `spread_against_aoe`.
2. Replace the placeholder in `Situation.tactics_overrides` and the party-level overrides.

- **Done when:** the types round-trip in RON.

### T4.7.2 Generator rules

**Type:** Build · **Depends on:** T4.7.1

1. **Formation:** frontline, midline or backline from each slot's roles; backline heroes on Guard, midline on Fight or Guard.
2. **Pre-fight:** cast Shelter → Union → Displacement → Armor of Unfeeling when they are on a bar (§20.1); raise minions from available corpses.
3. **Called target:** a foe with a healing role first.
4. **Spread:** apart when the foes have AoE (from their skills' role tags).

- **Done when:** the M1 plan is generated.

### T4.7.3 Pre-fight phase and walk-in

**Type:** Build · **Depends on:** T4.7.1, T4.8.1

1. Before aggro, the sim runs the pre-fight list in order, spending real time and energy and respecting recharges, with foes idle.
2. Then the party walks towards the encounter (A-008) until aggro (AI-F1).
3. The clear time counts from aggro [Proposed]. Record this rule in DESIGN.md §12.3.

- **Done when:** a log shows the spirits up before aggro, and the energy spent on them.

### T4.7.4 Formation and flags

**Type:** Build · **Depends on:** T4.7.1

1. Formation offsets are relative to the party's direction of travel.
2. Guard-mode heroes hold their positions as static flags. Spread flags are placed apart by at least the AoE radius.
3. No flag moves during a fight (Q29).

- **Done when:** the tests cover positions at aggro for each role.

### T4.7.5 Overrides merge

**Type:** Build · **Depends on:** T4.7.2

1. The user's fields from the situation or party file are kept, and every other field is regenerated.
2. The optimiser (P5) uses the same merge for each candidate (§11.6).

- **Done when:** an override of hero 7's mode survives regeneration while the other fields change with the builds.

### T4.7.6 Tactics tests

**Type:** Test · **Depends on:** T4.7.3–T4.7.5

1. Snapshot the M1 plan and check it against the §20.1 tactics notes.
2. Check that the pre-fight order is honoured.
3. Check the overrides.

- **Done when:** the tests pass.

---

## WP4.8 Encounters, situations and chains

**Goal:** the curated Kournan patrol, the generic archetype groups, the six M1 situations and the M1 situation set exist as data and run. A chain carries state across a rest period.

- **Refs:** §10.11, §10.12, §12, §20.6, A-006, A-008, A-028, A-030, D24–D26.
- **Depends on:** WP4.2, T3.10.2.
- **Done when:** every M1 situation runs, and the chain's carry-over and chain metrics are tested.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T4.8.1 | Encounter spawning | Build | T3.10.2 | Todo |
| T4.8.2 | The curated Kournan patrol | Data | T4.8.1, WP4.2 | Todo |
| T4.8.3 | Generic encounters, M1 situations and the M1 set | Data | T4.8.2 | Todo |
| T4.8.4 | Chains | Build | T4.8.1, T4.3.10 | Todo |
| T4.8.5 | Situation switches | Build | T4.8.1 | Todo |
| T4.8.6 | Situation tests | Test | T4.8.3–T4.8.5 | Todo |

### T4.8.1 Encounter spawning

**Type:** Build · **Depends on:** T3.10.2

1. Implement the layout primitives: `Cluster(radius)` (deterministic positions from the spawn stream), `Line(spacing)` and `Explicit([...])`.
2. The group position is relative to `party_start`. Levels come from each foe's mode data, unless the group overrides them.
3. The party starts in formation at `party_start`.

- **Done when:** the tests show spawn positions are deterministic per seed and match the layout.

### T4.8.2 The curated Kournan patrol

**Type:** Data · **Depends on:** T4.8.1, WP4.2

1. Write `data/encounters/curated/nightfall/vehtendi-valley/kournan-patrol.ron` with one of each of the 8 foes, Guard as the axe variant, and a `Cluster` 1,800 gwinches ahead, following §12.1 and §20.2.
2. Provenance: the Vehtendi Valley page and A-006, A-007 and A-008.

- **Done when:** the file validates and spawns.

### T4.8.3 Generic encounters, M1 situations and the M1 set

**Type:** Data · **Depends on:** T4.8.2

1. **Generic encounters** (§20.6 compositions, HM 26): `kournan-melee-heavy`, `kournan-caster-heavy`, `kournan-healer-heavy`.
2. **Situations:**
   - `dummies-hm` (update from M0 to party size 8);
   - `kournan-melee-heavy-hm`;
   - `kournan-caster-heavy-hm`;
   - `kournan-healer-heavy-hm`;
   - `kournan-patrol-hm`;
   - `kournan-patrol-hm-chain2` (two patrols, with the A-028 rest between them).
3. **Situation set:** `data/situation_sets/m1.ron`, all six with equal weights [Proposed].

- **Done when:** everything validates.

### T4.8.4 Chains

**Type:** Build · **Depends on:** T4.8.1, T4.3.10

1. After a fight, simulate the rest period (A-028 by default):
   - regeneration and natural regeneration;
   - recharges and effect durations;
   - minion degeneration;
   - overcast recovery;
   - the hero out-of-combat rules (dropping maintained enchantments, except the exceptions).
2. Carry into the next fight:
   - health, energy, recharges and effects;
   - minions;
   - spirits, if in range and alive;
   - DP and morale;
   - consumable timers.
3. **Chain metrics:** total time, total deaths, DP at the end, and the fight in which the chain first failed.

- **Done when:** a test shows lowered health carried into fight 2 and regenerated during the rest by the expected amount.

### T4.8.5 Situation switches

**Type:** Build · **Depends on:** T4.8.1

1. **HM:** as done in WP3.3 and WP4.4.
2. **Reforged Mode:** implement the rules (pre-Searing foes get −20% health and about −20% armor). They have no effect on M1 content; test with a synthetic pre-Searing foe.
3. **Dhuum's Covenant:** the flag (T4.3.10).
4. **Melandru's Accord:** validation only, until profiles exist (WP5.7).
5. **Consumables:** the schema and switch are wired up; M1 uses none.
6. Starting DP and morale, and the timeout.

- **Done when:** there is a test per switch.

### T4.8.6 Situation tests

**Type:** Test · **Depends on:** T4.8.3–T4.8.5

1. Each of the six situations runs 32 seeds without panics and reaches win, wipe or timeout.
2. The chain metrics are filled in.

- **Done when:** the tests pass.

---

## WP4.9 Command line and reports

**Goal:** the command line exposes evaluation, comparison, logs, templates and data tools, with reports 1, 2, 4, 5 and 7. Every report follows the §14.1 rules: the "uncalibrated" label, the assumptions used, draft warnings, coverage limits, pack version and seeds.

- **Refs:** §14, §15, D16, D22, UC1, UC6, UC7.
- **Depends on:** WP4.8.
- **Done when:** the commands work as specified, the result JSON is schema-versioned, and the README's first-evaluation step works.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T4.9.1 | Party files and benchmark files | Data / Build | WP4.8 | Todo |
| T4.9.2 | `gwsim evaluate` and the result JSON | Build | T4.9.1 | Todo |
| T4.9.3 | Rules common to every report | Build | T4.9.2 | Todo |
| T4.9.4 | Report 1: ranked builds with template codes | Build | T4.9.3 | Todo |
| T4.9.5 | Report 2: metrics with confidence ranges | Build | T4.9.3 | Todo |
| T4.9.6 | Report 5: `gwsim log` | Build | T4.9.2, T3.9.4 | Todo |
| T4.9.7 | Report 4: contribution breakdown and energy timeline | Build | T4.9.3 | Todo |
| T4.9.8 | Report 7: `gwsim compare` | Build | T4.9.5 | Todo |
| T4.9.9 | Finish the `template` and `data` commands | Build | T4.9.1 | Todo |
| T4.9.10 | CLI integration tests | Test | T4.9.2–T4.9.9 | Todo |
| T4.9.11 | README first evaluation | Docs | T4.9.10 | Todo |

### T4.9.1 Party files and benchmark files

**Type:** Data / Build · **Depends on:** WP4.8

1. **Party file format** (§15), a RON file with one entry per slot:
   - kind (`Human`, `Hero(profession or name)`, or `Henchman`);
   - build (a template code plus equipment, or an inline `Build`);
   - `locked`;
   - `ai_overrides` (plan, disabled skills);
   - or `benchmark: "<name>"`.
2. **Benchmark files** (D16), each with source URL, snapshot URL and date:
   - `data/benchmarks/mesmerway-dual-resto.ron`: the 7 heroes, with the exact bars chosen in §20.1 (including the [Proposed] picks), their runes and insignias, and PvX template codes;
   - `data/benchmarks/me-any-pve-energy-surge.ron`: the player, with A-033.
3. **Party file:** `data/parties/m1-mesmerway.ron` references both benchmarks [Proposed location].

- **Done when:** the M1 party loads, and each slot's derived stats equal T1.4.1.

### T4.9.2 `gwsim evaluate` and the result JSON

**Type:** Build · **Depends on:** T4.9.1

1. `gwsim evaluate --party <file|codes> --situation <id|file> [--set <id>] [--runs N | --auto] [--seed S] [--json out.json] [--reviewed-only]`.
2. `--party` also accepts up to eight comma-separated skill codes, for quick use.
3. `--auto` uses the stop-when-stable rule (T3.8.4).
4. The result JSON (§14.3): `{schema_version, inputs {party, situation(s), seeds, data pack version}, aggregates, per_run summaries, assumptions, drafts, coverage notes}`. Write a schema document next to the code.

- **Done when:** the M1 party evaluates on every M1 situation and on the set.

### T4.9.3 Rules common to every report

**Type:** Build · **Depends on:** T4.9.2

1. Add a shared report footer and header:
   - "Absolute numbers are uncalibrated" (D22);
   - the assumptions touched, as ID and statement;
   - draft skills used, with a warning;
   - coverage limits (e.g. "N of M Mesmer skills unavailable: NumbersOnly");
   - the data pack version;
   - the seeds.

- **Done when:** every report includes them (tested).

### T4.9.4 Report 1: ranked builds with template codes

**Type:** Build · **Depends on:** T4.9.3

1. For each slot: skill template code (type 14), a readable equipment list, and an equipment code (type 15). Note that PvE characters can't load equipment codes and hero equipment codes carry weapons only (§14 #1).
2. In `evaluate`, the "ranked list" is the party as given. The optimiser supplies real rankings in P5.

- **Done when:** the M1 party's codes round-trip.

### T4.9.5 Report 2: metrics with confidence ranges

**Type:** Build · **Depends on:** T4.9.3

1. Win rate (Wilson 95%), clear time, deaths, damage taken, energy left, and DP (chains), each with its interval.
2. Text tables and JSON.

- **Done when:** the snapshot test passes.

### T4.9.6 Report 5: `gwsim log`

**Type:** Build · **Depends on:** T4.9.2, T3.9.4

1. `gwsim log --result <file> --run <n> [--format text|jsonl] [--verbose]` re-simulates run *n* from the result's inputs and seed, and prints or exports its log.
2. It refuses if the data pack version differs from the result's (ENG-43), unless `--force` is given, with a warning.

- **Done when:** a logged run reproduces its summary exactly.

### T4.9.7 Report 4: contribution breakdown and energy timeline

**Type:** Build · **Depends on:** T4.9.3

1. Attribute effects per §14.2:
   - damage to the unit and skill that dealt it (minions to their master; spirits and damage-over-time to their caster);
   - healing to the healer and skill, with overhealing tracked separately;
   - mitigation to the effect's source (Shelter, Union, Displacement, Armor of Unfeeling, Protective Was Kaolai, …);
   - interrupts landed per skill, and what they stopped.
2. Energy over time: per-second samples per slot, averaged over runs.
3. Text tables and JSON.

- **Done when:** a scenario with a known Shelter cap credits the prevented damage to hero 7.

### T4.9.8 Report 7: `gwsim compare`

**Type:** Build · **Depends on:** T4.9.5

1. `gwsim compare <partyA> <partyB> --situations <id|set>` runs both parties on the **same seeds** (CRN).
2. It prints the metrics side by side with differences, and paired intervals of the differences (paired bootstrap over seeds, with a fixed bootstrap seed).

- **Done when:** comparing a party with itself shows zero differences, and a weakened variant shows significant ones.

### T4.9.9 Finish the `template` and `data` commands

**Type:** Build · **Depends on:** T4.9.1

1. `template decode`: show resolved skill names and derived attributes when the data set has them.
2. `template encode`: accept party files and print every slot's codes.
3. Make sure the `data validate`, `coverage`, `describe` and `info` flags are consistent across commands.

- **Done when:** the help text for every command is complete and consistent.

### T4.9.10 CLI integration tests

**Type:** Test · **Depends on:** T4.9.2–T4.9.9

1. Add `assert_cmd` tests for each command's success and error paths.
2. Snapshot the result JSON's schema shape.

- **Done when:** the tests pass.

### T4.9.11 README first evaluation

**Type:** Docs · **Depends on:** T4.9.10

1. Update README Getting Started step 4 to the working command, `gwsim evaluate --party data/parties/m1-mesmerway.ron --situation kournan-patrol-hm`, with sample output.
2. Update step 5 to mention `gwsim check`.

- **Done when:** the commands run as written.

---

## WP4.10 Relative checks RC1, RC2 and RC6

**Goal:** correctness is checked as §17.4 defines it. The benchmark beats weakened variants and a naive baseline under identical conditions, and monotonicity properties always hold. The checks run in `gwsim check` and, with reduced run counts, in CI.

- **Refs:** §17.4, Q34, ENG-42.
- **Depends on:** WP4.9, and every M1 skill `Reviewed`.
- **Done when:** RC1, RC2 and RC6 pass at full run counts.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T4.10.1 | The RC2 baseline team | Research / Decision | — | Todo |
| T4.10.2 | Check framework and `gwsim check` | Build | WP4.9 | Todo |
| T4.10.3 | RC1: weakened variants | Build | T4.10.2 | Todo |
| T4.10.4 | RC2: naive baseline | Build | T4.10.1, T4.10.2 | Todo |
| T4.10.5 | RC6: monotonicity properties | Build | T4.10.2 | Todo |
| T4.10.6 | Reduced checks in CI | Setup | T4.10.3–T4.10.5 | Todo |
| T4.10.7 | Run the checks and triage | Run | T4.10.6 | Todo |

### T4.10.1 The RC2 baseline team

**Type:** Research / Decision · **Depends on:** — · **Owner:** approves

1. Propose a "naive baseline" (§17.4): 7 heroes with minimal, auto-attack-centred bars. Choose the professions and weapons so the team is plausible (e.g. a mix of martial heroes plus one healer with a basic heal), with runes and insignias removed.
2. Record it as `data/benchmarks/baseline-naive.ron`, with the reasoning.

- **Done when:** the owner approves the baseline.

### T4.10.2 Check framework and `gwsim check`

**Type:** Build · **Depends on:** WP4.9

1. In the `gwsim-cli` library target, add `Check { id, description, run(ctx, level: Reduced | Full) -> CheckResult { pass, details, evidence } }`.
2. `gwsim check [--suite relative|unit|all] [--level reduced|full] [--only RC1]` prints a summary and writes a JSON report.
3. Comparisons are paired on shared seeds (ENG-42). The rule "A beats B" means:
   - the 95% paired-bootstrap interval of the win-rate difference is above 0; or
   - the win rates are equal within the interval, and the clear-time difference's interval is below 0.

   The bootstrap uses 1,000 resamples with a fixed seed.

- **Done when:** a check comparing a team with itself reports "not better".

### T4.10.3 RC1: weakened variants

**Type:** Build · **Depends on:** T4.10.2

1. Generate the variants from the M1 party:
   - each hero's elite removed (the slot left empty): 7 variants;
   - 20 random off-role skill swaps, using role tags (a skill replaced by a legal skill with no shared role), with seeded choices;
   - attributes misallocated (points moved to unused attributes, keeping legality);
   - no runes or insignias.
2. **Pass rule:** Mesmerway beats at least 90% of the variants, and beats every "elite removed" variant, on `kournan-patrol-hm`.

- **Done when:** the check runs and reports each variant's result.

### T4.10.4 RC2: naive baseline

**Type:** Build · **Depends on:** T4.10.1, T4.10.2

1. **Pass rule:** the win rate is higher by ≥ 30 percentage points, or the win rate is equal and the clear time is ≥ 30% better, on the M1 set's combat situations.

- **Done when:** the check runs.

### T4.10.5 RC6: monotonicity properties

**Type:** Build · **Depends on:** T4.10.2

1. Property tests:
   - raising Domination Magic never lowers Energy Surge's damage (single use, fixed seed, all ranks);
   - adding a healer hero to a failing team never lowers its win rate (paired, within confidence);
   - adding a Vigor rune never lowers maximum health;
   - more Fast Casting never lengthens an activation.
2. They run in `cargo test` and in `gwsim check`.

- **Done when:** the properties hold.

### T4.10.6 Reduced checks in CI

**Type:** Setup · **Depends on:** T4.10.3–T4.10.5

1. Uncomment the CI step `gwsim check --suite relative --level reduced`.
2. Set reduced run counts so the step takes under 5 minutes on the runner.

- **Done when:** the step is active and inside its time budget.

### T4.10.7 Run the checks and triage

**Type:** Run · **Depends on:** T4.10.6 · **Owner:** decides on failures

1. Run `gwsim check --level full`.
2. For any failure, investigate whether it is a model bug (fix it) or a real property of the model (discuss with the owner).
3. Record the results and decisions in `docs/findings/T4.10.7-m1-relative-checks.md`.

- **Done when:** RC1, RC2 and RC6 pass, or the owner has accepted a documented exception.

---

## WP4.11 Performance

**Goal:** one M1 fight simulates in under 10 ms on one core (D19, C6). Benchmarks exist, and CI guards against regressions.

- **Refs:** §10.14, §17.6, D19, ENG-5.
- **Depends on:** WP4.8.
- **Done when:** the median time for the Kournan patrol HM fight is under 10 ms on the development machine, and a CI smoke test guards it.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T4.11.1 | Profiling and performance CI on Windows | Research | — | Todo |
| T4.11.2 | Benchmark suite | Build | WP4.8 | Todo |
| T4.11.3 | Profile the hot paths | Run | T4.11.1, T4.11.2 | Todo |
| T4.11.4 | Optimise | Build | T4.11.3 | Todo |
| T4.11.5 | Performance smoke test in CI | Setup | T4.11.4 | Todo |
| T4.11.6 | Verify and record the target | Test | T4.11.4 | Todo |

### T4.11.1 Profiling and performance CI on Windows

**Type:** Research · **Depends on:** —

1. Compare profilers usable on Windows: `samply` (supports Windows ETW), Visual Studio's profiler (in the Build Tools?) and Windows Performance Recorder with Analyzer. Recommend one. Installing it needs the owner's approval.
2. Find how to make a performance CI check useful despite noisy shared runners. Options:
   - a relative check against a baseline run in the same job;
   - a generous absolute threshold;
   - counting simulated events per fight as a proxy.

   Recommend one, keeping the §17.6 rule that a regression over 20% fails CI [Proposed].

- **Output:** `docs/findings/T4.11.1-performance-tooling.md`.

### T4.11.2 Benchmark suite

**Type:** Build · **Depends on:** WP4.8

1. Add `criterion` benchmarks in `crates/gwsim-engine/benches/fights.rs`:
   - the Kournan patrol HM fight (8 party + 8 foes + spirits and minions);
   - the two-fight chain;
   - a synthetic 8-v-16 stress fight (§17.6).
2. Each benchmark runs a fixed seed list with logging off.

- **Done when:** `cargo bench -p gwsim-engine` reports the median per fight.

### T4.11.3 Profile the hot paths

**Type:** Run · **Depends on:** T4.11.1, T4.11.2 · **Owner:** approves any profiler install

1. Profile the patrol benchmark and list the top hot spots, with their share of time.

- **Output:** a section of `docs/findings/T4.11.6-performance.md`.

### T4.11.4 Optimise

**Type:** Build · **Depends on:** T4.11.3

1. Address the hot spots in order of payoff. Typical candidates:
   - allocations in the loop (use `SmallVec` and scratch buffers);
   - repeated effect scans (index effects by kind);
   - stat recomputation (dirty flags);
   - spatial queries (a grid only if profiling shows the need);
   - event-queue churn.
2. Try LTO and `codegen-units = 1` in the release profile, measured.
3. Keep every optimisation deterministic: re-run the determinism tests after each one.

- **Done when:** the median is under 10 ms, or the remaining gap is explained.

### T4.11.5 Performance smoke test in CI

**Type:** Setup · **Depends on:** T4.11.4

1. Add the CI step chosen in T4.11.1.

- **Done when:** the step is active and passes.

### T4.11.6 Verify and record the target

**Type:** Test · **Depends on:** T4.11.4

1. Record the medians for all three benchmarks, the machine spec, and the commit, in `docs/findings/T4.11.6-performance.md`.

- **Done when:** the patrol median is under 10 ms (D19).

---

## M1 acceptance

| # | Criterion (§19) | Evidence |
| --- | --- | --- |
| 1 | `gwsim evaluate` runs the M1 party against every M1 situation, deterministically | T4.9.2, T4.8.6; determinism tests from T3.7.5 re-run on M1 |
| 2 | All M1 skills are `Reviewed` | `gwsim data coverage` shows 72/72 `Reviewed`; T4.1.2–T4.1.11 |
| 3 | The generated descriptions are reviewed against the wiki | The owner reviews in each batch (batch procedure step 7) |
| 4 | RC1, RC2 and RC6 pass | T4.10.7 |
| 5 | Performance meets D19 | T4.11.6 |
| 6 | Reports show assumptions, draft warnings and the "uncalibrated" label | T4.9.3 |
| 7 | The §17.2 tests pass | The `wiki_examples` suites in WP1.4 and WP3.5, T3.6.10, T4.3.8 |
