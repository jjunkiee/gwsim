# P7 Coverage (ongoing)

**Phase goal:** bring every player skill to `Reviewed`, profession by profession in the agreed order. Alongside that, add what the new skills and users need: engine mechanics, monster skills and curated encounters, generic archetypes, henchmen, consumables, a maintained benchmark list, and a balance-patch routine.

- **Design refs:** §8.7, §17.5, §19 (P7), §22 (encoding volume), G3, G6, Q31, Q36, D6, D11.
- **Starts when:** M1 is done. It runs in parallel with P5 and P6.
- **Ends when:** `gwsim data coverage` shows 100% `Reviewed` for every profession and for PvE-only skills. The supporting work packages continue for as long as the project is maintained.
- **Order** [Decided Q36]: Mesmer → Ritualist → Necromancer → **PvE-only** (including the title tables) → Paragon → Monk → Elementalist → Warrior → Ranger → Dervish → Assassin.
- **WP numbers** in this phase are assigned by this plan; the design gives none.

| WP | Title | Goal | Status |
| --- | --- | --- | --- |
| 7.1 | Coverage pipeline | Make the per-profession procedure fast and repeatable: full seeding, batch planning and progress tracking. | Todo |
| 7.2 | Mesmer | 100% of Mesmer skills `Reviewed`. | Todo |
| 7.3 | Ritualist | 100% of Ritualist skills `Reviewed`. | Todo |
| 7.4 | Necromancer | 100% of Necromancer skills `Reviewed`. | Todo |
| 7.5 | PvE-only skills and title tables | 100% of PvE-only skills `Reviewed`, with every title track's rank table. | Todo |
| 7.6 | Paragon | 100% of Paragon skills `Reviewed`. | Todo |
| 7.7 | Monk | 100% of Monk skills `Reviewed`. | Todo |
| 7.8 | Elementalist | 100% of Elementalist skills `Reviewed`. | Todo |
| 7.9 | Warrior | 100% of Warrior skills `Reviewed`. | Todo |
| 7.10 | Ranger | 100% of Ranger skills `Reviewed`, pets included. | Todo |
| 7.11 | Dervish | 100% of Dervish skills `Reviewed`. | Todo |
| 7.12 | Assassin | 100% of Assassin skills `Reviewed`. | Todo |
| 7.13 | Monster skills and curated encounters | New curated encounters, with their monster skills and any boss scripts. | Todo |
| 7.14 | Generic archetypes | Archetype groups from more rosters, plus "boss plus escort". | Todo |
| 7.15 | Henchmen and RC3 | Henchman data and behaviour, and the RC3 check. | Todo |
| 7.16 | Consumables, blessings and environment effects | Situation switches have real content. | Todo |
| 7.17 | Benchmark list | A maintained set of frozen benchmark teams, with checks. | Todo |
| 7.18 | Balance-patch routine | A documented, repeatable way to apply a game update. | Todo |

---

## WP7.1 Coverage pipeline

**Goal:** make the per-profession procedure (below) quick to run many times. That means seeding every player skill, planning batches from data, and showing progress at a glance.

- **Refs:** §8.7, §9.4, §17.5.
- **Depends on:** M1, WP2.7.
- **Done when:** every player skill has at least a `NumbersOnly` file, and the procedure below has been used once end to end (on the first Mesmer batch).

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T7.1.1 | Full skill crawl and seed | Run | — | Todo |
| T7.1.2 | Batch planning helper | Build | T7.1.1 | Todo |
| T7.1.3 | Coverage progress view | Build | T7.1.1 | Todo |
| T7.1.4 | Document the per-profession procedure | Docs | T7.1.2 | Todo |

### T7.1.1 Full skill crawl and seed

**Type:** Run · **Depends on:** — · **Owner:** approves the crawl (about 1,400 pages, about 70 minutes at 3 s each)

1. Run `gwsim-extract crawl --scope skills` in resumable chunks, at times the owner chooses.
2. Run `seed --scope skills`. Existing files are skipped.
3. Validate, commit on approval, and record the counts per profession.

- **Done when:** the skill index and the skill files agree: every skill has a file.

### T7.1.2 Batch planning helper

**Type:** Build · **Depends on:** T7.1.1

1. `gwsim data coverage --plan --profession <p>` lists the profession's skills that aren't yet `Reviewed`, grouped by attribute and by skill type, as suggested batches of about 15–25.
2. Each skill is flagged with a likely class (formulaic, conditional or handler) from simple heuristics on the extracted numbers and its type. The flag is a hint only; the batch research confirms it.

- **Done when:** a Mesmer plan is produced.

### T7.1.3 Coverage progress view

**Type:** Build · **Depends on:** T7.1.1

1. Extend `gwsim data coverage` with a per-profession progress bar and the last-reviewed date. The desktop app's coverage display (T6.10.7) reuses it.

- **Done when:** progress is visible at a glance.

### T7.1.4 Document the per-profession procedure

**Type:** Docs · **Depends on:** T7.1.2

1. Add the procedure below to `docs/data-authoring.md`, with the commands for each step.

- **Done when:** the guide covers the procedure.

### Per-profession procedure

Every profession work package (WP7.2 to WP7.12) runs the same seven tasks. Each task ID is `T7.<n>.<step>`, e.g. `T7.8.4` is the Elementalist mechanics task.

| Step | Task | Type | What to do | Done when |
| --- | --- | --- | --- | --- |
| 1 | Research the profession | Research | Read the profession page, its attribute pages (inherent effects), the pages of the skill types it uses most, the hero-behaviour notes and 2026 update lines for it. Answer the profession-specific questions listed in its WP. Write `docs/findings/T7.<n>.1-<profession>.md`. | Every question is answered, or registered as an assumption |
| 2 | Seed the gaps | Run | Seed any skills still missing (normally none after T7.1.1). Re-run `diff` on the profession, to catch balance changes since seeding. | Every skill has current numbers |
| 3 | Plan the batches | Decision | Use T7.1.2 to write the batch table (skills per batch, class, handlers, mechanics each batch needs) in the findings file. The owner agrees the batch order. | Batch table agreed |
| 4 | Engine mechanics | Build | Implement the profession's new mechanics, each with tests, before the batches that need them. | Tests pass |
| 5 | Encode the batches | Data / Build | For each batch, follow the WP4.1 batch procedure (research, encode, handlers, tests, review sheet, owner review). | Each batch `Reviewed` |
| 6 | AI hints and roles | Data | Check every skill's hero and foe AI hints against the hero-vetted category and the update notes. Confirm the role tags (automatic and manual). | No skill is missing hints |
| 7 | Close | Review | `gwsim data coverage --profession <p>` shows 100% `Reviewed`. Run `gwsim check --level full`. Add a profession benchmark to WP7.17 if a good one exists. | 100% `Reviewed`; the checks pass |

---

## WP7.2 Mesmer

**Goal:** every Mesmer skill is `Reviewed`.

- **Depends on:** WP7.1.
- **Done when:** step 7 of the procedure passes for Mesmer.

Profession-specific research questions (step 1):

1. Fast Casting on signets (A-017); resolve the conflict if possible.
2. PvE Fast Casting recharge for every Mesmer spell type.
3. Illusion Magic's conditions and hexes that act on movement or attacking (e.g. "while moving").
4. Punishment hexes and how foe and hero AI avoid them.
5. Skill-disable and interrupt families ("disables that skill for N seconds").
6. Shatter families (effects on hex removal).
7. Inspiration's energy management and enchantment removal.
8. Every other 2026 AoE change (target 100%, others 75%).

**Expected new mechanics (step 4):** skill disabling tied to interrupts; hexes that trigger on the target's own actions; energy-loss scaling.

| Task | Title | Type | Status |
| --- | --- | --- | --- |
| T7.2.1 | Research Mesmer | Research | Todo |
| T7.2.2 | Seed the gaps | Run | Todo |
| T7.2.3 | Plan the batches | Decision | Todo |
| T7.2.4 | Mesmer mechanics | Build | Todo |
| T7.2.5 | Encode the batches | Data / Build | Todo |
| T7.2.6 | AI hints and roles | Data | Todo |
| T7.2.7 | Close | Review | Todo |

---

## WP7.3 Ritualist

**Goal:** every Ritualist skill is `Reviewed`.

- **Depends on:** WP7.2.
- **Done when:** step 7 passes for Ritualist.

Profession-specific research questions (step 1):

1. Item spells: urns and ashes as bundles, drop effects, one bundle at a time.
2. Offensive binding spirits that attack: their attack rates, damage and targeting.
3. Spirit health and armor for every spirit (A-015).
4. Weapon-spell duration with Spawning Power.
5. "Near a spirit" conditions, and how spirit range is measured.
6. Communing skills that change spirits.
7. Restoration's "spirit-linked" healing.
8. The 2026 ritual and Soul Twisting changes.

**Expected new mechanics:** bundles and drop effects; attacking spirits (`SpiritAi`); urn and ash timers.

| Task | Title | Type | Status |
| --- | --- | --- | --- |
| T7.3.1 | Research Ritualist | Research | Todo |
| T7.3.2 | Seed the gaps | Run | Todo |
| T7.3.3 | Plan the batches | Decision | Todo |
| T7.3.4 | Ritualist mechanics | Build | Todo |
| T7.3.5 | Encode the batches | Data / Build | Todo |
| T7.3.6 | AI hints and roles | Data | Todo |
| T7.3.7 | Close | Review | Todo |

---

## WP7.4 Necromancer

**Goal:** every Necromancer skill is `Reviewed`.

- **Depends on:** WP7.3.
- **Done when:** step 7 passes for Necromancer.

Profession-specific research questions (step 1):

1. Every minion type: level, armor, attack, health and special behaviour (e.g. Flesh Golem, Vampiric Horror, Jagged Horror, Bone Minions, Shambling Horror). Record which minions count against the cap.
2. Wells: area, duration, the corpse requirement, and the 66% sacrifice without a corpse.
3. Curses: degeneration hexes and punishment hexes.
4. Blood Magic's life stealing, and sacrifice modifiers.
5. The full Soul Reaping rules in Reforged.
6. Corpse exploitation conflicts (two skills wanting the same corpse).

**Expected new mechanics:** more minion kinds; wells as areas; life stealing in the damage order.

| Task | Title | Type | Status |
| --- | --- | --- | --- |
| T7.4.1 | Research Necromancer | Research | Todo |
| T7.4.2 | Seed the gaps | Run | Todo |
| T7.4.3 | Plan the batches | Decision | Todo |
| T7.4.4 | Necromancer mechanics | Build | Todo |
| T7.4.5 | Encode the batches | Data / Build | Todo |
| T7.4.6 | AI hints and roles | Data | Todo |
| T7.4.7 | Close | Review | Todo |

---

## WP7.5 PvE-only skills and title tables

**Goal:** every PvE-only skill is `Reviewed`, and every title track that scales one has a rank table (A-020).

- **Depends on:** WP7.4, WP5.7 (title tracks).
- **Done when:** step 7 passes for PvE-only skills, and every title track used by a skill has a table in `titles.ron`.

Research questions (step 1):

1. Every title track and its rank → effective-rank table, from the individual rank pages (A-020): Sunspear, Lightbringer, Asura, Deldrimor, Ebon Vanguard, Norn, Kurzick and Luxon.
2. Ebon Vanguard rituals and summons (e.g. Ebon Vanguard Assassin Support: own skill bar, lifetime). These need `SummonAi` (§10.10).
3. Norn forms and blessings that give **temporary skill bars** (e.g. Ursan Blessing).
4. Sunspear and Lightbringer skills' conditions.
5. How heroes are excluded (they can't use PvE-only skills).
6. Title passive effects.
7. How Melandru's Accord removes titles.

**Expected new mechanics:** `SummonAi` and summoned creatures with bars; forms that replace the skill bar; title passives.

| Task | Title | Type | Status |
| --- | --- | --- | --- |
| T7.5.1 | Research PvE-only skills and titles | Research | Todo |
| T7.5.2 | Seed the gaps | Run | Todo |
| T7.5.3 | Plan the batches | Decision | Todo |
| T7.5.4 | PvE-only mechanics (summons, temporary bars, titles) | Build | Todo |
| T7.5.5 | Encode the batches | Data / Build | Todo |
| T7.5.6 | AI hints and roles | Data | Todo |
| T7.5.7 | Close | Review | Todo |

---

## WP7.6 Paragon

**Goal:** every Paragon skill is `Reviewed`.

- **Depends on:** WP7.5.
- **Done when:** step 7 passes for Paragon.

Profession-specific research questions (step 1):

1. Leadership's energy return.
2. Chants and their triggers.
3. Echoes, refrains and finales (re-application on ending).
4. The hero AI maintaining "They're on Fire!" and Anthem of Weariness for echoes (2026).
5. Spear attacks and their projectile speeds.
6. Paragon adrenaline.
7. Motivation's armor and health effects.

**Expected new mechanics:** refrain and echo re-application loops; chant triggers.

| Task | Title | Type | Status |
| --- | --- | --- | --- |
| T7.6.1 | Research Paragon | Research | Todo |
| T7.6.2 | Seed the gaps | Run | Todo |
| T7.6.3 | Plan the batches | Decision | Todo |
| T7.6.4 | Paragon mechanics | Build | Todo |
| T7.6.5 | Encode the batches | Data / Build | Todo |
| T7.6.6 | AI hints and roles | Data | Todo |
| T7.6.7 | Close | Review | Todo |

---

## WP7.7 Monk

**Goal:** every Monk skill is `Reviewed`.

- **Depends on:** WP7.6.
- **Done when:** step 7 passes for Monk.

Profession-specific research questions (step 1):

1. When Divine Favor's bonus healing applies.
2. Protection Prayers' damage reduction, damage caps and prevention.
3. Smiting's holy damage, and extra damage against undead (undead sensitivity to light, §10.7).
4. Monk enchantments that heroes maintain.
5. Heal-over-time and conditional heals.
6. Spirit Bond-style triggered heals.
7. Hero healer quirks (resurrecting mid-combat, double healing).

**Expected new mechanics:** heal triggers on damage; more damage-prevention hooks; creature-type damage multipliers.

| Task | Title | Type | Status |
| --- | --- | --- | --- |
| T7.7.1 | Research Monk | Research | Todo |
| T7.7.2 | Seed the gaps | Run | Todo |
| T7.7.3 | Plan the batches | Decision | Todo |
| T7.7.4 | Monk mechanics | Build | Todo |
| T7.7.5 | Encode the batches | Data / Build | Todo |
| T7.7.6 | AI hints and roles | Data | Todo |
| T7.7.7 | Close | Review | Todo |

---

## WP7.8 Elementalist

**Goal:** every Elementalist skill is `Reviewed`.

- **Depends on:** WP7.7.
- **Done when:** step 7 passes for Elementalist.

Profession-specific research questions (step 1):

1. Energy Storage.
2. Attunements (energy return; always cast by heroes).
3. Glyphs (one at a time; affect the next spell or spells).
4. Overcast: how it is paid and how it recovers.
5. Knockdown families (Earth and Air).
6. Wards as stationary areas.
7. Elemental damage against armor.
8. The wiki's maximum-Elementalist-energy example, re-checked with real items.

**Expected new mechanics:** glyph consumption; overcast in depth; ward areas.

| Task | Title | Type | Status |
| --- | --- | --- | --- |
| T7.8.1 | Research Elementalist | Research | Todo |
| T7.8.2 | Seed the gaps | Run | Todo |
| T7.8.3 | Plan the batches | Decision | Todo |
| T7.8.4 | Elementalist mechanics | Build | Todo |
| T7.8.5 | Encode the batches | Data / Build | Todo |
| T7.8.6 | AI hints and roles | Data | Todo |
| T7.8.7 | Close | Review | Todo |

---

## WP7.9 Warrior

**Goal:** every Warrior skill is `Reviewed`.

- **Depends on:** WP7.8.
- **Done when:** step 7 passes for Warrior.

Profession-specific research questions (step 1):

1. Strength's armor penetration.
2. Adrenaline gain and loss (including loss on interruption).
3. Stances (one at a time; usable while knocked down).
4. Hammer knockdowns.
5. Sword, axe and hammer attack skills.
6. Deep Wound and Cracked Armor sources.
7. Shields: armor and block.
8. The 2026 melee hero AI (stickiness; auto-attacks deprioritised).

**Expected new mechanics:** adrenaline in depth; stance families; shield items.

| Task | Title | Type | Status |
| --- | --- | --- | --- |
| T7.9.1 | Research Warrior | Research | Todo |
| T7.9.2 | Seed the gaps | Run | Todo |
| T7.9.3 | Plan the batches | Decision | Todo |
| T7.9.4 | Warrior mechanics | Build | Todo |
| T7.9.5 | Encode the batches | Data / Build | Todo |
| T7.9.6 | AI hints and roles | Data | Todo |
| T7.9.7 | Close | Review | Todo |

---

## WP7.10 Ranger

**Goal:** every Ranger skill is `Reviewed`, including pets.

- **Depends on:** WP7.9.
- **Done when:** step 7 passes for Ranger, and a pet fights under the owner's combat mode.

Profession-specific research questions (step 1):

1. **Pets:** species, level, health, armor, attack rate and damage; Beast Mastery scaling; pet attacks (§7.1 "Pet attack"); pet AI (it follows the hero's mode, and in 2026 prioritises the locked, then called, then previous target); pet death and resurrection.
2. Expertise's cost reduction.
3. Preparations (one at a time; affect bow attacks).
4. Traps: easily interrupted, the 90 s lifetime, the trigger radius.
5. Nature rituals (affect every creature; range 3000, A-018).
6. The bow types' ranges, speeds and damage.
7. Arrow-dodging interactions with projectile leading.

**Expected new mechanics:** a pet unit kind and `PetAi`; traps as triggered areas; nature spirits affecting both sides.

| Task | Title | Type | Status |
| --- | --- | --- | --- |
| T7.10.1 | Research Ranger | Research | Todo |
| T7.10.2 | Seed the gaps | Run | Todo |
| T7.10.3 | Plan the batches | Decision | Todo |
| T7.10.4 | Ranger mechanics (pets, traps, preparations) | Build | Todo |
| T7.10.5 | Encode the batches | Data / Build | Todo |
| T7.10.6 | AI hints and roles | Data | Todo |
| T7.10.7 | Close | Review | Todo |

---

## WP7.11 Dervish

**Goal:** every Dervish skill is `Reviewed`.

- **Depends on:** WP7.10.
- **Done when:** step 7 passes for Dervish.

Profession-specific research questions (step 1):

1. Mysticism.
2. Flash enchantments (no activation; they disable other flash enchantments for 1 s).
3. Forms and avatars (one at a time; aftercast rules).
4. Scythe attacks hitting up to 3 adjacent foes.
5. "When this enchantment ends" effects.
6. The three Dervish spells that cost adrenaline.
7. Wind and Earth Prayers.
8. How heroes use avatars (not pre-cast).

**Expected new mechanics:** multi-target melee; enchantment-end triggers at scale; flash-enchantment lockout.

| Task | Title | Type | Status |
| --- | --- | --- | --- |
| T7.11.1 | Research Dervish | Research | Todo |
| T7.11.2 | Seed the gaps | Run | Todo |
| T7.11.3 | Plan the batches | Decision | Todo |
| T7.11.4 | Dervish mechanics | Build | Todo |
| T7.11.5 | Encode the batches | Data / Build | Todo |
| T7.11.6 | AI hints and roles | Data | Todo |
| T7.11.7 | Close | Review | Todo |

---

## WP7.12 Assassin

**Goal:** every Assassin skill is `Reviewed`.

- **Depends on:** WP7.11.
- **Done when:** step 7 passes for Assassin.

Profession-specific research questions (step 1):

1. Critical Strikes (crit chance and energy on crits).
2. Combo chains (lead attack → off-hand → dual attack prerequisites and their failure behaviour).
3. Shadow steps, and returns to the starting point.
4. The dagger double-strike chance.
5. Deadly Arts hexes.
6. Shadow Arts defence.
7. Hero AI handling of combos.

**Expected new mechanics:** combo state per target; shadow-step movement; double strikes.

| Task | Title | Type | Status |
| --- | --- | --- | --- |
| T7.12.1 | Research Assassin | Research | Todo |
| T7.12.2 | Seed the gaps | Run | Todo |
| T7.12.3 | Plan the batches | Decision | Todo |
| T7.12.4 | Assassin mechanics | Build | Todo |
| T7.12.5 | Encode the batches | Data / Build | Todo |
| T7.12.6 | AI hints and roles | Data | Todo |
| T7.12.7 | Close | Review | Todo |

---

## WP7.13 Monster skills and curated encounters

**Goal:** add curated encounters beyond the Kournan patrol when users need them, each with its monster skills and any boss scripts (AI-F7). Monster-only skills use the same effect system (D6).

- **Refs:** §7.1 (Monster skill), §11.3 (AI-F7), §12.1, D6, Q7.
- **Depends on:** M1. Individual encounters depend on the professions their foes use.
- **Done when:** the encounter list the owner chooses (T7.13.1) is implemented. The work package stays open for new requests.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T7.13.1 | Choose the next encounters | Decision | — | Todo |
| T7.13.2 | Per-encounter procedure | Docs | — | Todo |
| T7.13.3 | `ScriptAi` framework for bosses | Build | — | Todo |
| T7.13.4 | Implement the chosen encounters | Data / Build | T7.13.1–T7.13.3 | Todo |

### T7.13.1 Choose the next encounters

**Type:** Decision · **Depends on:** — · **Owner:** chooses

1. Propose candidates that exercise new mechanics or are popular hard areas, e.g. an HM elite area group, and groups heavy in hexes or conditions (the Kournans provide little of either, §20.2).
2. The owner picks the order.

- **Done when:** an ordered list is recorded in the plan.

### T7.13.2 Per-encounter procedure

**Type:** Docs · **Depends on:** —

1. Add to `docs/data-authoring.md` the steps for a new encounter:
   1. research the area page and the foe pages;
   2. crawl and seed the foes and their monster skills;
   3. encode the missing skills (the WP4.1 batch procedure);
   4. fill in the foe data gaps as assumptions;
   5. author the group layout (the area page has no composition or positions);
   6. write a situation;
   7. validate;
   8. run a smoke evaluation;
   9. owner review.

- **Done when:** the procedure is documented.

### T7.13.3 `ScriptAi` framework for bosses

**Type:** Build · **Depends on:** —

1. `ScriptAi` is layered over `FoeAi` (§11.1). A boss script is a handler that overrides decisions at defined points: health thresholds, timers, on events.
2. Register scripts by name, and reference them from a foe's `ai_tags: [Script("…")]`.

- **Done when:** a test boss switches behaviour at 50% health.

### T7.13.4 Implement the chosen encounters

**Type:** Data / Build · **Depends on:** T7.13.1–T7.13.3

1. Run T7.13.2 for each chosen encounter, one encounter per sub-task added to this table as work starts.

- **Done when:** each chosen encounter runs and has been reviewed.

---

## WP7.14 Generic archetypes

**Goal:** generic scenarios built from more rosters than the Kournans, and a "boss plus escort" archetype, so situation sets can cover varied threats (§12.2).

- **Refs:** §12.2, Q7, D25.
- **Depends on:** WP7.13, since archetypes reuse foes from curated rosters.
- **Done when:** at least one melee-heavy, caster-heavy and healer-heavy archetype exists per roster added, and a boss-plus-escort template exists.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T7.14.1 | Archetype templates | Build | — | Todo |
| T7.14.2 | Archetypes per new roster | Data | T7.14.1 | Todo |

### T7.14.1 Archetype templates

**Type:** Build · **Depends on:** —

1. Add a small generator (or documented pattern) that builds melee-heavy, caster-heavy, healer-heavy and boss-plus-escort encounters from a roster file (a list of foes with their roles).

- **Done when:** the Kournan archetypes regenerate identically from their roster.

### T7.14.2 Archetypes per new roster

**Type:** Data · **Depends on:** T7.14.1

1. For each roster added in WP7.13, generate and review its archetypes and situations.

- **Done when:** each roster has its archetypes.

---

## WP7.15 Henchmen and RC3

**Goal:** henchmen exist as party slots with their fixed builds per region, including the Reforged Mode changes. They are controlled like heroes until their AI is known to differ. RC3 checks Mesmerway against an all-henchman party.

- **Refs:** §7.1 (Henchman), §11.1, §17.4 (RC3), Q13.
- **Depends on:** the professions the henchmen use (their skills must be `Reviewed`).
- **Done when:** henchman slots can be used from the command line and the app, and RC3 passes.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T7.15.1 | Henchman builds | Research | — | Todo |
| T7.15.2 | Henchman data | Data | T7.15.1 | Todo |
| T7.15.3 | Henchman slots | Build | T7.15.2 | Todo |
| T7.15.4 | RC3 | Build | T7.15.3 | Todo |

### T7.15.1 Henchman builds

**Type:** Research · **Depends on:** —

1. From the Henchman page and the individual henchman pages, record:
   - each henchman's builds per campaign region and level;
   - the Reforged Mode changes to henchman bars and levels (§10.12);
   - any documented henchman AI differences from heroes.

- **Output:** `docs/findings/T7.15.1-henchmen.md`.

### T7.15.2 Henchman data

**Type:** Data · **Depends on:** T7.15.1

1. Write `data/creatures/henchmen/<name>.ron`: fixed builds per region and mode, with provenance.

- **Done when:** the files validate.

### T7.15.3 Henchman slots

**Type:** Build · **Depends on:** T7.15.2

1. Support the `Henchman(id)` slot kind in party files, the command line and the app.
2. The build comes from the region and mode, and can't be edited; the slot is always locked.
3. Use `HeroAi` unless the research found differences.

- **Done when:** an all-henchman party evaluates.

### T7.15.4 RC3

**Type:** Build · **Depends on:** T7.15.3

1. Add RC3 to `gwsim check`: Mesmerway plus the player against an all-henchman party (plus the player), on the M1 set.
2. **Pass rule:** Mesmerway wins more and faster (§17.4).

- **Done when:** RC3 passes.

---

## WP7.16 Consumables, blessings and environment effects

**Goal:** give the situation switches real content. Consumables are simulated exactly (Q21) and never optimised. Blessings, party bonuses and environment effects can be switched on per situation.

- **Refs:** §7.1 (Consumable), §10.11 (consumable timers in chains), Q21, A-019.
- **Depends on:** M1.
- **Done when:** the common PvE consumables are encoded and selectable per situation, and they survive or end at death per A-019.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T7.16.1 | Consumables and effects | Research | — | Todo |
| T7.16.2 | Consumable and effect data | Data | T7.16.1 | Todo |
| T7.16.3 | Engine support | Build | T7.16.2 | Todo |

### T7.16.1 Consumables and effects

**Type:** Research · **Depends on:** —

1. From the Consumable page and the item pages, record the commonly used PvE consumables (e.g. Essence of Celerity, Grail of Might, Armor of Salvation, speed-boost sweets and summoning stones): effect, duration, scope and death behaviour (A-019). Summoning stones need `SummonAi` (WP7.5).
2. Also record blessings, party bonuses and any environment effects needed by the chosen curated encounters.

- **Output:** `docs/findings/T7.16.1-consumables.md`.

### T7.16.2 Consumable and effect data

**Type:** Data · **Depends on:** T7.16.1

1. Fill in `data/items/consumables.ron` and the effect definitions.

- **Done when:** the file validates.

### T7.16.3 Engine support

**Type:** Build · **Depends on:** T7.16.2

1. Apply a situation's consumables at the start of the run (with a scope of self or party).
2. Carry their timers through chains.
3. Apply the death rules.

- **Done when:** a test consumable affects stats and times out correctly across a chain.

---

## WP7.17 Benchmark list

**Goal:** a maintained list of frozen, known-strong builds and teams (D16), each with RC1-style checks, so that model regressions show up across professions, not just Mesmerway.

- **Refs:** §17.4, D16, C3.
- **Depends on:** the professions each benchmark uses.
- **Done when:** each profession that is `Reviewed` has at least one benchmark with a passing check, where a good one exists.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T7.17.1 | Pick the benchmarks | Research / Decision | — | Todo |
| T7.17.2 | Freeze the benchmarks | Data | T7.17.1 | Todo |
| T7.17.3 | Benchmark checks | Build | T7.17.2 | Todo |

### T7.17.1 Pick the benchmarks

**Type:** Research / Decision · **Depends on:** — · **Owner:** approves the list

1. Using Wayback snapshots of PvXwiki's Great and Good PvE team and hero builds (C3), propose benchmarks per profession.
2. Record each benchmark's URL, snapshot date and rating.

- **Done when:** the list is approved.

### T7.17.2 Freeze the benchmarks

**Type:** Data · **Depends on:** T7.17.1

1. Write `data/benchmarks/<name>.ron` with the codes, URL and date for each.

- **Done when:** the files validate.

### T7.17.3 Benchmark checks

**Type:** Build · **Depends on:** T7.17.2

1. Generalise RC1 (elite removed, off-role swaps, misallocation, no runes) to any benchmark, and add each benchmark to `gwsim check`.

- **Done when:** the checks run for every frozen benchmark.

---

## WP7.18 Balance-patch routine

**Goal:** a documented, repeatable procedure for applying a game update (UC11, G6):

1. crawl;
2. read the change report;
3. update the data;
4. re-review;
5. re-run the checks;
6. move the baseline date (D2);
7. release.

- **Refs:** §9.4 (`diff`), C4, D2, UC11, G6.
- **Depends on:** WP2.7.
- **Done when:** `docs/maintenance.md` exists, and the procedure has been rehearsed on a real or simulated update.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T7.18.1 | Write the maintenance guide | Docs | — | Todo |
| T7.18.2 | Re-review rules | Build | — | Todo |
| T7.18.3 | Rehearse the routine | Run | T7.18.1, T7.18.2 | Todo |

### T7.18.1 Write the maintenance guide

**Type:** Docs · **Depends on:** —

1. Write `docs/maintenance.md`, covering each step of the routine:
   1. **When:** a game update appears on the wiki's update pages.
   2. **Crawl:** a full crawl (about 3.3 hours) at an owner-approved time, resumable.
   3. **Diff:** `gwsim-extract diff`, with the game-update context (T2.7.4).
   4. **Triage:** changed numbers are updated in the files; changed descriptions are re-encoded; new skills are seeded.
   5. **Re-review:** a `Reviewed` skill whose numbers or description changed goes back to `Draft` (T7.18.2).
   6. **Checks:** `gwsim check --level full`.
   7. **Baseline:** update the baseline date in DESIGN.md (D2) and in the pack version.
   8. **Release:** release notes listing the changes.

- **Output:** `docs/maintenance.md`.

### T7.18.2 Re-review rules

**Type:** Build · **Depends on:** —

1. Give `diff` a `--demote` report listing the skills that should go back to `Draft`.
2. Add a helper, `gwsim-extract apply-demotions`, that only changes `review` fields and only with explicit confirmation. It never touches numbers (Q10).

- **Done when:** a test demotes exactly the changed skills.

### T7.18.3 Rehearse the routine

**Type:** Run · **Depends on:** T7.18.1, T7.18.2 · **Owner:** approves the crawl

1. Rehearse the routine on the next real update, or simulate one with edited cached pages.
2. Record the timings and any friction in the guide.

- **Done when:** the rehearsal is recorded.
