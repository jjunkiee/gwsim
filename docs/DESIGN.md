# gwsim design

> **gwsim** is a Guild Wars Reforged PvE build simulator.
>
> - **Status:** agreed design, 2026-09-22.
> - **Origin:** a design interview (questions **Q1–Q43** plus accepted defaults **D1–D31**, all logged in [§3](#3-decision-log)).
> - **Audience:** anyone turning this design into a plan of work. It assumes familiarity with Guild Wars, but not with the interview.
>
> **How to read this document**
>
> - **[Decided Qn]** / **[Default Dn]**: a decision made with the project owner. Changing it means reopening the design.
> - **[Proposed]**: an engineering detail added while writing this document. It was not put to a decision and can change during implementation without reopening the design. Record significant changes in §3.
> - **Requirement IDs** such as `ENG-12` mark individual, testable requirements. Work packages in [§19](#19-roadmap-and-work-packages) refer to them.
> - **Wiki facts** are cited with links to [wiki.guildwars.com](https://wiki.guildwars.com/wiki/Main_Page), the project's source of truth for game data. Values are as of the wiki on 2026-09-22 (game updates up to 2026-09-01).

---

## Contents

1. [Purpose and scope](#1-purpose-and-scope)
2. [Goals, constraints and non-goals](#2-goals-constraints-and-non-goals)
3. [Decision log](#3-decision-log)
4. [Glossary](#4-glossary)
5. [Users and use cases](#5-users-and-use-cases)
6. [Architecture](#6-architecture)
7. [Domain model](#7-domain-model)
8. [Data](#8-data)
9. [Wiki extractor (developer tool)](#9-wiki-extractor-developer-tool)
10. [Simulation engine](#10-simulation-engine)
11. [AI and tactics](#11-ai-and-tactics)
12. [Situations, encounters and chains](#12-situations-encounters-and-chains)
13. [Optimiser](#13-optimiser)
14. [Results and reports](#14-results-and-reports)
15. [Command-line interface](#15-command-line-interface)
16. [Desktop app](#16-desktop-app)
17. [Validation and testing](#17-validation-and-testing)
18. [Repository, tooling and engineering practice](#18-repository-tooling-and-engineering-practice)
19. [Roadmap and work packages](#19-roadmap-and-work-packages)
20. [First milestone (M1) content inventory](#20-first-milestone-m1-content-inventory)
21. [Assumptions register (initial)](#21-assumptions-register-initial)
22. [Risks and mitigations](#22-risks-and-mitigations)
23. [Out of scope](#23-out-of-scope)
24. [References](#24-references)

---

## 1. Purpose and scope

gwsim helps Guild Wars 1 / Guild Wars Reforged players find the most effective builds for their own character and their party (heroes, henchmen and other players) in PvE. It has two halves:

- **Evaluator:** simulate a party with given builds in a situation many times and report how well it does.
- **Optimiser:** search the space of builds for the ones that do best across a weighted set of situations. It shows the trade-offs between competing goals.

The product aims at community grade [Decided Q6]:

- It covers every player skill in time (about 1,400 player skills, plus about 396 monster skills where encounters need them).
- It is fully offline: all game knowledge ships with the app.
- It is open source.
- It is distributed as a single Windows download.

It is built command line first; the desktop app comes after the optimiser [Decided Q4, Q9, Q43].

---

## 2. Goals, constraints and non-goals

### 2.1 Goals

| ID | Goal |
| --- | --- |
| G1 | A faithful PvE combat simulation for Normal and Hard Mode, at the level of detail in §10 (timeline, open 2D field, no terrain). |
| G2 | Build optimisation for any subset of party slots, against weighted sets of situations, with a success threshold, a chosen goal and a trade-off view. |
| G3 | Eventually cover every player skill. Coverage is always visible, so results are never quietly limited. |
| G4 | Fully offline and self-contained. Every result can be reproduced exactly from its seed. |
| G5 | Usable by the community: a single Windows download, clear results, and in-game template codes. |
| G6 | Maintainable across balance patches: a developer tool re-crawls the wiki and produces a change report. |

### 2.2 Constraints

| ID | Constraint | Source |
| --- | --- | --- |
| C1 | **Wiki access.** [robots.txt](https://wiki.guildwars.com/robots.txt) disallows `/api.php`, `/index.php` and `Special:` pages for all user agents. Only ordinary article URLs (`/wiki/<Title>`) may be fetched, slowly. The wiki documents no rate limits and offers no public database dump. | Q24, D15 |
| C2 | **Licensing.** (1) In-game skill description text and skill icons are ArenaNet content, not licensed to third parties: "The terms of the permission do not include third party use" ([Template:ArenaNet image](https://wiki.guildwars.com/wiki/Template:ArenaNet_image), [GWW:Copyrights](https://wiki.guildwars.com/wiki/Guild_Wars_Wiki:Copyrights)). (2) Text written by wiki editors is GFDL. (3) Numbers are facts; the wiki says facts may be used "if expressed originally" ([GWW:Copyrighted content](https://wiki.guildwars.com/wiki/Guild_Wars_Wiki:Copyrighted_content)). The app therefore bundles numbers and our own encodings only. **The whole project, `data/` included, is GPL-3.0-or-later [D33].** *Not legal advice.* | Q25, Q38 |
| C3 | **PvXwiki access.** gwpvx.fandom.com returns 403 to scripted requests. No attempt is made to get around this. Benchmark builds come from Wayback Machine snapshots or are typed in by hand. | D16 |
| C4 | **Staleness.** The balance baseline is the wiki as of 2026-09-22. Later patches are applied by hand, and the app going out of date between updates is acceptable. | Q5, D2 |
| C5 | **Platform and toolchain.** Rust. Windows is the official platform. Assume nothing is installed; the README's Getting Started section covers everything. Nothing is installed on a machine without asking. | Q23, D9, D21 |
| C6 | **Performance.** One 8-v-8 fight simulated in under 10 ms on one CPU core. | D19 |
| C7 | **Privacy and locale.** No telemetry. English only. | D14 |

### 2.3 Non-goals

See [§23](#23-out-of-scope). In brief:

- PvP;
- terrain, pathfinding and line of sight;
- live wiki import;
- historical balance versions;
- bundling ArenaNet text or art;
- scripted mid-fight micro;
- optimising consumables;
- calibration against in-game results as a release requirement.

---

## 3. Decision log

### 3.1 Interview decisions

| ID | Topic | Decision |
| --- | --- | --- |
| Q1 | Purpose | Evaluator **and** optimiser. The evaluator is built first; the optimiser is a search loop over it. |
| Q2 | Game modes | **PvE only.** |
| Q3 | Build unit | The whole party is always simulated. Any slot can be locked or left free for the optimiser. |
| Q4 | Audience and interface | Personal first, open source. Command line first. (The "local browser UI" was later replaced by the desktop app; see Q9 and D4.) |
| Q5 | Data freshness | **No live wiki import.** All mechanics, skills and creatures are modelled in code and data inside the app. Going out of date after game updates is acceptable. |
| Q6 | Ambition | **Community-grade product:** full skill coverage, polished UI, maintained through balance patches. |
| Q7 | Encounter types | Generic scenarios, curated real encounters and user-made encounters. Foes are stored as data. |
| Q8 | Combat detail | Timeline simulation on an **open 2D field**: ranges, AoE geometry, projectiles, movement, fleeing and body-blocking. **No terrain.** |
| Q9 | Distribution | A **single Windows desktop download**. Built last among the major features (see Q43). |
| Q10 | Data authoring | A developer-only extractor seeds the data files. It can be re-run to produce a **change report**, and it never overwrites data. |
| Q11 | Filling gaps and checks | The wiki is the data source. In-game measurements are allowed as optional input. PvX builds are used **only as tests** (benchmarks), never as data. |
| Q12 | Code authorship | Claude writes most of the code; the project owner reviews, decides and tests. |
| Q13 | Party slots | **Any mix** of human players, heroes and henchmen. |
| Q14 | Account state | An **optional profile**. The default is everything unlocked and maxed. |
| Q15 | Encounter scale | **Single fights and chains** of fights with carry-over. |
| Q16 | Optimisation target | **Weighted sets of situations.** A set of one is allowed. |
| Q17 | Ranking | A success threshold first, then a goal the user picks, plus a **trade-off frontier**. |
| Q18 | Foe AI | Whatever mix of generic rules, per-skill usage rules and boss scripts **best matches the real game** in Normal and Hard Mode. |
| Q19 | Party AI | Heroes use **emulated hero AI**. Humans follow a **generated, editable priority plan**. |
| Q20 | Gear the optimiser may change | Runes, insignias and **one weapon set** now. **Weapon-set swapping later.** |
| Q21 | Consumables | **Switches on the situation**, simulated exactly, never chosen by the optimiser. |
| Q22 | How skill effects are encoded | **Hybrid:** declarative data (RON) for formulaic and conditional skills, Rust handlers for one-of-a-kind skills. |
| Q23 | Language | **Rust**, for everything (see D10). |
| Q24 | How the extractor reads the wiki | A slow crawl of ordinary article pages, parsing the rendered HTML. |
| Q25 | What the app bundles | **Numbers and our own effect encodings only.** Descriptions are generated from the effect data, each skill links to its wiki page, and there are **no icons and no in-game text**. |
| Q26 | Data format | **RON**, one file per skill, foe and encounter. |
| Q27 | Search method | **Evolutionary multi-objective** search (NSGA-II style) **plus an exhaustive mode** for small pools. |
| Q28 | Results | All seven result types (§14). |
| Q29 | Party tactics | A **tactics plan per situation**: formation, hero modes, called or locked targets, pre-fight setup. **No scripted mid-fight actions.** |
| Q30 | First milestone | **7 Hero Mesmerway** against a few generic scenarios and one curated encounter. The first internal step is one character against training dummies. |
| Q31 | Coverage order | **By profession.** |
| Q32 | Player build in the milestone | PvX **"Me/any PvE Energy Surge"**. |
| Q33 | Mesmerway variant | **Dual Resto, regular bars** (not "Of the Profession" weapon versions). |
| Q34 | What counts as correct | **Relative checks only:** benchmarks must beat weakened versions and simple baselines. No in-game calibration gate. |
| Q35 | Curated encounter | **Kournan military patrol, Vehtendi Valley, Hard Mode.** |
| Q36 | Profession order | Mesmer → Ritualist → Necromancer → **PvE-only skills** → Paragon → Monk → Elementalist → Warrior → Ranger → Dervish → Assassin. |
| Q37 | Where tactics plans come from | **Generated from each build.** The user can override them. |
| Q38 | Data licence | ~~CC BY-SA 4.0 (one-way compatible with GPL-3.0).~~ **Superseded by D33 (2026-09-22): the whole project, data included, is GPL-3.0-or-later.** |
| Q39 | Desktop UI framework | **egui (eframe).** |
| Q40 | Player's optional slots (M1) | **Cry of Frustration, Spiritual Pain, Power Drain.** |
| Q41 | Player weapon swapping (M1) | **Deferred.** The player uses the 40/40 Domination set. |
| Q42 | The `research/` documents | They **stay local and out of the repo.** |
| Q43 | When the desktop app arrives | **After the optimiser works.** Coverage continues alongside it. |

### 3.2 Accepted defaults

| ID | Default |
| --- | --- |
| D1 | PvE covers Normal and Hard Mode. **Reforged Mode**, **Dhuum's Covenant** and **Melandru's Accord** are switches on each situation. |
| D2 | The balance baseline is the wiki on 2026-09-22 (after the 2026-09-01 fixes). Later patches are applied by hand. |
| D3 | PvP versions of skills are not modelled. |
| D4 | The desktop app is the only graphical UI. There is no separate web UI. |
| D5 | Every run is seeded and reproducible. Scores average many runs, and runs are added until the result is statistically stable. |
| D6 | Monster-only skills use the same effect system as player skills. They are added when an encounter needs them. |
| D7 | Account profiles are entered by hand and saved locally. |
| D8 | The wiki extractor lives in the repo as a developer tool and is **not shipped**. |
| D9 | Rust is installed through rustup with the MSVC toolchain, plus the Visual Studio C++ Build Tools if missing, **with the owner's permission first**. The `.gitignore` switches to the Rust template. |
| D10 | Developer tools (extractor, change report) are also written in Rust. |
| D11 | Skill effects are translated by Claude in Claude Code sessions, in batches, and reviewed by the owner. No bulk Claude API calls and no API key. |
| D12 | Build order: evaluator (command line) → optimiser (command line) → desktop app. |
| D13 | Foe AI: for each case, the mix of rules and scripts that best matches the game. Hard Mode differences are modelled explicitly. |
| D14 | Scripted mid-fight actions are out of scope. English only. Official builds for Windows only. No telemetry. Releases through GitHub Releases. |
| D15 | The extractor fetches article pages only, one every few seconds. Its User-Agent names the project and contains no personal details. It keeps a local cache. The change report re-crawls everything, which takes hours. |
| D16 | Benchmark builds are frozen into test data as template codes, with source URL and date. They come from Wayback snapshots or are typed in by the owner. No attempt is made to get around Fandom's block. |
| D17 | A hero's identity only affects its primary profession and availability. The simulation treats heroes as level-20 slots with that profession. Flagged as an assumption. |
| D18 | Hard Mode foe attribute ranks missing from the wiki are filled from the wiki's general Hard Mode rules and labelled as assumptions. |
| D19 | Performance target: one 8-v-8 fight in under 10 ms on one core. |
| D20 | Cargo workspace with separate crates for the engine, data, command line, extractor and desktop app. |
| D21 | The README's Getting Started section assumes nothing is installed: rustup, Visual Studio C++ Build Tools, clone, build, run. |
| D22 | Absolute numbers (clear time, damage taken, …) are labelled **uncalibrated**. In-game measurements remain optional input for filling gaps. |
| D23 | The player's M1 bar uses the PvX page's attributes, 5 Prodigy's insignias, no secondary profession, and current wiki values for every skill. |
| D24 | The Kournan patrol is one of each of the 8 foe types, Hard Mode level 26, with no boss. |
| D25 | The M1 generic scenarios are training dummies plus melee-heavy, caster-heavy and healer-heavy groups built from the Kournan roster, so they add no new foe skills. |
| D26 | M1 includes a two-fight chain (two Kournan patrols with a short rest between them) to test carry-over. |
| D27 | Every data file records its wiki source, crawl date and review status (draft or reviewed). Draft skills can be used, but they are flagged in results. |
| D28 | Unit tests check each mechanic against the wiki's formulas and worked examples, and each skill's numbers against the extracted values. |
| D29 | This document links to wiki pages, not into `research/`, because `research/` stays local. |
| D30 | The optimiser's first real test is "the best player build for this Mesmerway team". Solo Resto becomes the second benchmark. |
| D31 | M0 (internal step): the player's Energy Surge bar against training dummies. |
| D32 | The code licence identifier is **`GPL-3.0-or-later`** (the FSF's recommended form), not `GPL-3.0-only`. The `LICENSE` text is unchanged either way; this sets whether later GPL versions may apply. Decided in T0.5.1, 2026-09-22. |
| D33 | **One licence for the whole project: `GPL-3.0-or-later`, `data/` included. This supersedes Q38.** The facts in `data/` are not copyrightable and their expression is ours, so the project is free to license them as it likes; CC BY-SA was a choice, not an inheritance from the wiki. Unifying removes a real question raised by the WP1.7 data pack, which embeds `data/` into the shipped binary and so makes "one combined work or mere aggregation?" a live issue rather than a theoretical one. **Known cost, accepted:** other community tools can no longer reuse gwsim's data without adopting the GPL, which CC BY-SA would have allowed. Decided 2026-09-22. |

---

## 4. Glossary

| Term | Meaning |
| --- | --- |
| **Build** | One character's professions, attribute point spread, 8-skill bar and equipment (armor with runes and insignias, a weapon set). |
| **Bar** | The 8 skills of a build. At most 1 elite and at most 3 PvE-only skills. Heroes can't use PvE-only skills. |
| **Template code** | The game's base64 skill template (type 14) or equipment template (type 15) string. See [Skill template format](https://wiki.guildwars.com/wiki/Skill_template_format) and [Equipment template format](https://wiki.guildwars.com/wiki/Equipment_template_format). |
| **Slot** | One party position. Its kind is **Human**, **Hero** or **Henchman**. A slot is **locked** (fixed build) or **free** (the optimiser may change it). |
| **Encounter** | One or more foe groups with positions, levels and behaviour tags, placed on an open field. |
| **Situation** | An encounter or chain of encounters, plus context: mode switches, consumables, party size, tactics overrides, timeout. |
| **Chain** | An ordered list of encounters with rest periods between them. State carries over between fights. |
| **Situation set** | A weighted list of situations that one optimisation run targets. |
| **Run** | One seeded simulation of a situation. |
| **Evaluation** | Many runs of the same party and situation, aggregated with confidence intervals. |
| **Tactics plan** | Party-level instructions for a situation: formation, hero modes, called or locked targets, pre-fight setup, and hero skills turned off for automatic use. |
| **Effect DSL** | The declarative skill-effect language written in RON (§8.4). |
| **Handler** | Rust code implementing a one-of-a-kind skill or mechanic (§8.5). |
| **Provenance** | The source URL(s), crawl date, review status and assumption references stored in each data file. |
| **Review status** | `numbers-only` → `draft` → `reviewed`. See §8.7. |
| **Coverage** | Which skills, foes and encounters are implemented, and at what review status. |
| **Benchmark** | A frozen, known-strong build or team (from PvX), used in relative checks. |
| **Relative check** | A test that a benchmark outperforms weakened variants and baselines under identical conditions (Q34). |
| **Assumption** | A labelled value for something the wiki doesn't document (§21). |
| **Gwinch** | The game's distance unit. Ranges: touch 144, adjacent 166, nearby 252, in the area 322, earshot 1012, casting 1248, longbow 1498, spirit 2512, nature rituals 3000 (since 2026-08-26), party 5020 ([Range](https://wiki.guildwars.com/wiki/Range)). |
| **Pip** | A unit of regeneration. Health: 2 HP/s per pip. Energy: 1 energy per 3 s per pip. Net cap ±10 pips. |
| **NM / HM** | Normal Mode / Hard Mode. |
| **CRN** | Common random numbers: the same seed stream used across candidate builds so that comparisons are paired (§10.13). |
| **Frontier** | The Pareto front of builds that trade one goal against another (Q17). |

---

## 5. Users and use cases

### 5.1 Personas

- **Solo hero player:** plays with 7 heroes and wants the best bar for their own character, or a better hero team.
- **Theory-crafter:** compares variants and wants to know *why* one build wins.
- **Data contributor:** encodes skills, foes and encounters, and reviews other contributions.
- **Maintainer:** applies balance patches using the change report.

### 5.2 Use cases

| ID | Use case | Main features involved |
| --- | --- | --- |
| UC1 | **Evaluate a team.** Load 8 builds (template codes or files), pick one or more situations, run, and read the metrics. | Evaluator, reports |
| UC2 | **Optimise my character** with the hero team locked. | Optimiser (one free slot) |
| UC3 | **Design a hero team** around my locked character. | Optimiser (several free slots), tactics generation |
| UC4 | **Find robust builds** across a set of situations (e.g. every curated HM encounter in a campaign). | Situation sets, weights |
| UC5 | **Respect my account:** only unlocked skills, my title ranks, the heroes I own. | Account profile |
| UC6 | **Compare** two builds or teams side by side. | Comparison report |
| UC7 | **Understand why:** contribution breakdown, energy timeline, combat log, 2D replay. | Reports, desktop replay |
| UC8 | **Search a small pool exhaustively** ("the best 3 of these 20 skills"). | Exhaustive mode |
| UC9 | **Author an encounter** from wiki foe data and a hand-written group layout. | Encounter files, validation |
| UC10 | **Contribute a skill encoding** and have it reviewed. | Effect DSL, generated descriptions, per-skill tests |
| UC11 | **Apply a balance patch** (maintainer): crawl, read the change report, edit data, re-run the checks. | Extractor diff, relative checks |
| UC12 | **Play by optional-mode rules:** Melandru's Accord skill restrictions, Dhuum's Covenant stakes. | Situation switches, account profile |

---

## 6. Architecture

### 6.1 Crates

The code is a Cargo workspace [Default D20]:

| Crate | Kind | Responsibility |
| --- | --- | --- |
| `gwsim-data` | library | Domain types; RON schemas; loading, validation and cross-reference checking; data packs; template-code codec; derived-stat formulas that don't need a timeline; the generated-description renderer; the coverage and assumption registries. |
| `gwsim-engine` | library | The deterministic combat simulation (§10) and the AI controllers (§11). It depends on `gwsim-data`. It has no I/O. |
| `gwsim-opt` [Proposed split] | library | The optimiser (§13): genome, constraints, NSGA-II, adaptive evaluation, exhaustive mode. It depends on `gwsim-engine`. (D20 names five crates; splitting the optimiser into its own library keeps the command line and desktop app thin. It can be folded into `gwsim-engine` if preferred.) |
| `gwsim-cli` | binary `gwsim` | The command-line interface (§15) and text/JSON reports (§14). |
| `gwsim-desktop` | binary | The egui desktop app (§16). |
| `gwsim-extractor` | binary `gwsim-extract`, **not shipped** | The wiki crawler, seeder and change report (§9). It writes the data format defined in `gwsim-data`. |

### 6.2 Dependency graph

```text
gwsim-extractor ──► gwsim-data ◄── gwsim-engine ◄── gwsim-opt ◄── gwsim-cli
                                                         ▲
                                                         └──────── gwsim-desktop
```

### 6.3 Data flow

```text
 wiki.guildwars.com (article pages only)
        │  slow crawl, local cache (.cache/, git-ignored)
        ▼
 gwsim-extract ──seed──► data/*.ron  ◄── hand editing: effect encodings, reviews, encounters,
        │                   │            assumptions (by Claude + owner / contributors)
        └──diff──► change report (markdown/JSON) for maintainers
                            │
                   build time: validate + compile to a data pack [Proposed]
                            ▼
                   gwsim / gwsim-desktop (embedded data pack)
                            │  + user data dir: profiles, user encounters/situations, parties
                            ▼
                   engine runs ► aggregates ► reports / frontier / replays
```

### 6.4 Runtime data loading [Proposed]

- **Bundled data:** at build time, the `data/` tree is validated and compiled into a compact binary data pack that is embedded in the executables. The desktop app is then a single file (Q9).
- **User data:** account profiles, user-made encounters, situations, parties and saved results live in a user data directory (`%APPDATA%\gwsim\`). They use the same RON formats and are validated on load.
- **Developer mode:** the command line can load `data/` directly (`--data-dir`) so contributors can test edits without rebuilding.

---

## 7. Domain model

This section is conceptual. The Rust types are defined in `gwsim-data`.

### 7.1 Static game data

| Entity | Key fields | Notes |
| --- | --- | --- |
| **Profession** | id (template index), name, abbreviation, primary attribute, attribute list, base armor rating, armor energy and regeneration, weapon affinities | 10 professions. Template indices come from [Skill template format](https://wiki.guildwars.com/wiki/Skill_template_format). |
| **Attribute** | id (template index), name, profession, is_primary, inherent effect (e.g. Fast Casting, Soul Reaping, Spawning Power, Energy Storage, Mysticism, Critical Strikes, Expertise, Strength, Leadership, Divine Favor) | Inherent effects are engine mechanics (§10.9). |
| **Skill** | id, name, wiki page, profession, attribute, type and subtype, elite, pve_only, title track (for PvE-only skills), costs, activation, recharge, target, range, AoE size, flags (projectile, touch, easily interrupted), effects (DSL) and/or handler reference, AI usage hints, role tags, provenance, review status | See §8.3–§8.5. |
| **Effect definition** | kind (hex, enchantment, stance, preparation, glyph, weapon spell, form, shout/chant/echo buff, condition, spirit aura, well/ward area, environment, consumable, title, blessing, party bonus), stacking key, duration rule, removable-by, modifiers while active, triggers, end actions, upkeep | Used by skills, consumables, environments and foes. |
| **Condition** | The 10 conditions with fixed effects: Bleeding −3, Burning −7, Poison −4, Disease −4 regeneration; Blind (90% miss); Crippled (−50% speed); Dazed; Deep Wound; Weakness; Cracked Armor ([Condition](https://wiki.guildwars.com/wiki/Condition)) | Only duration varies. |
| **Weapon type** | name, damage type, damage range at max requirement, attack interval, range, projectile speed, mastery attribute, two-handed | Values from the wiki's weapon pages ([Weapon](https://wiki.guildwars.com/wiki/Weapon), [Bow](https://wiki.guildwars.com/wiki/Bow)). Bow values come from the Bow page. |
| **Weapon upgrade** | slot (prefix, suffix, inscription, inherent), effect (DSL modifiers), conditions | [List of weapon upgrades](https://wiki.guildwars.com/wiki/List_of_weapon_upgrades), [Inscription](https://wiki.guildwars.com/wiki/Inscription). |
| **Rune** | attribute (+1/+2/+3) or generic (Vigor, Vitae, Attunement, Clarity, Purity, Recovery, Restoration, …), health penalty, non-stacking key | [Rune](https://wiki.guildwars.com/wiki/Rune). |
| **Insignia** | profession, per-piece or global effects | [Insignia](https://wiki.guildwars.com/wiki/Insignia). Insignia armor applies only to the piece it's on. |
| **Consumable** | effect, duration, scope (self or party), survives death? | [Consumable](https://wiki.guildwars.com/wiki/Consumable). |
| **Title track** | rank → effective-rank table for PvE-only skills | [Title](https://wiki.guildwars.com/wiki/Title). The individual rank pages are used; the old ×1.5/×1.25 formula is outdated. |
| **Hero** | name, fixed primary profession, availability (campaign/EotN/Reforged), mercenary flag | D17: identity matters only for profession and availability. |
| **Henchman** | name, fixed build(s) per campaign region and Reforged Mode | Later phase (§19 P7). |
| **Foe** | wiki page, name, affiliation, species/creature type, traits (fleshy, spirit, undead, …), professions, level NM/HM, attribute ranks NM/HM, skills (with HM-only flags), armor by damage type, health override, energy, weapon (type and damage; assumption if missing), boss flag, AI tags (kiter, stationary, special script), provenance | Built from `{{NPC infobox}}` plus the Skills and Armor ratings sections. |
| **Monster skill** | as Skill, without an attribute | [Monster skill](https://wiki.guildwars.com/wiki/Monster_skill). |

### 7.2 Builds, party and account

```text
Build {
  primary: Profession, secondary: Option<Profession>,
  attribute_points: Map<Attribute, 0..=12>,            // cost table, ≤ 200 points total
  headgear_attribute: Option<Attribute>,               // +1, primary profession only
  skills: [Option<SkillId>; 8],
  armor: [ArmorPiece; 5] { slot, insignia, rune },     // head, chest, hands, legs, feet
  weapon_set: WeaponSet { main, offhand, prefix, suffix, inscription, offhand_upgrades },
}
PartySlot { kind: Human | Hero(HeroId|AnyOfProfession) | Henchman(HenchmanId),
            build, locked: bool, ai_overrides }
Party { slots: Vec<PartySlot> }                        // size comes from the situation
AccountProfile { unlocked_skills: All | Set<SkillId>, title_ranks: Map<TitleTrack, rank>,
                 heroes_owned: All | Set<HeroId>, eotn_owned: bool,
                 available_upgrades: All | Set<UpgradeId>, learned_skills_per_character: Option<…> }
```

- **Attribute point cost** (to rank 12): 1, 2, 3, 4, 5, 6, 7, 9, 11, 13, 16, 20. Rank 12 costs 97 in total. The budget is 200 points ([Attribute point](https://wiki.guildwars.com/wiki/Attribute_point)).
- **Effective rank** = points rank (≤ 12) + rune (0–3, primary-profession attributes only, non-stacking) + headgear (0–1) + temporary effects. It is capped at 20 (21 with a weapon's +1 chance mod) ([Attribute](https://wiki.guildwars.com/wiki/Attribute)).
- **Legality:**
  - primary ≠ secondary;
  - skills must belong to the primary or secondary profession, or be common / PvE-only;
  - at most 1 elite;
  - at most 3 PvE-only skills, and none on heroes;
  - runes and insignias must match the primary profession's armor.

### 7.3 Simulation inputs and outputs

```text
Situation { name, encounters: Single(EncounterId) | Chain(Vec<(EncounterId, rest_after_s)>),
            mode: { hard_mode, reforged_mode, dhuums_covenant, melandrus_accord },
            party_size, consumables: Vec<ConsumableId>, starting_dp, starting_morale,
            timeout_s, tactics_overrides: Option<TacticsPlan>, notes, assumption_refs }
SituationSet { entries: Vec<(SituationId, weight)> }
RunResult { seed, outcome: Win|Wipe|Timeout, clear_time, deaths, dp_end, per_slot stats,
            per_skill stats, per_foe time-to-kill, energy samples, event log (optional),
            replay frames (optional), assumptions_touched, draft_skills_used }
Evaluation { party, situation, runs: n, aggregates with confidence intervals }
```

---

## 8. Data

### 8.1 Repository layout of `data/`

```text
data/
├── ATTRIBUTION.md              # facts derived from Guild Wars Wiki; no ArenaNet text or art
├── core/
│   ├── professions.ron         # incl. template indices, base armor, energy
│   ├── attributes.ron          # incl. template indices, inherent effects
│   ├── conditions.ron
│   ├── titles.ron              # title tracks → effective ranks for PvE-only skills
│   ├── ranges.ron              # named range bands (gwinches)
│   ├── levels.ron              # level → health, HM bonus health, base armor, damage multiplier
│   └── modes.ron               # HM, Reforged Mode, optional-mode rules
├── skills/
│   ├── mesmer/<skill-slug>.ron
│   ├── ritualist/… necromancer/… (one folder per profession)
│   ├── common/<skill-slug>.ron # PvE-only and no-profession skills
│   └── monster/<skill-slug>.ron
├── items/
│   ├── armor.ron  runes.ron  insignias.ron
│   ├── weapons.ron  weapon_upgrades.ron
│   └── consumables.ron
├── creatures/
│   ├── heroes.ron  henchmen/…  minions.ron  spirits.ron
│   └── foes/<affiliation>/<foe-slug>.ron
├── encounters/
│   ├── generic/<name>.ron
│   └── curated/<campaign>/<area>/<name>.ron
├── situations/<name>.ron
├── situation_sets/<name>.ron
├── benchmarks/<name>.ron       # frozen PvX builds (template codes + URL + date) [Default D16]
└── assumptions.ron             # the assumptions register (§21)
```

- One file per skill, foe and encounter [Decided Q26].
- File names are slugs of the wiki page title, e.g. `energy-surge.ron`, `kournan-guard.ron`.

### 8.2 Provenance block (every entity file) [Default D27]

```ron
provenance: (
    sources: ["https://wiki.guildwars.com/wiki/Energy_Surge"],
    crawled: "2026-09-22",
    review: Draft,                 // NumbersOnly | Draft | Reviewed
    reviewed_by: None,             // e.g. Some("owner")
    assumptions: [],               // e.g. ["A-004"]
    notes: "",
),
```

### 8.3 Skill file

Skill files have two parts:

- **Extracted numbers.** The extractor writes these, and they are later maintained by hand.
- **The encoding.** Written by hand: the effect DSL and/or a handler, AI hints and role tags.

The numbers in the example below are **illustrative only, not real game values**:

```ron
// data/skills/elementalist/fireball.ron   (illustrative values)
Skill(
    id: 0000,                          // template-code skill id
    name: "Fireball",
    wiki: "Fireball",
    profession: Elementalist,
    attribute: Some(FireMagic),
    kind: Spell,                       // SkillType + subtype (e.g. Spell, HexSpell, BindingRitual, Shout…)
    elite: false,
    pve_only: false,
    cost: (energy: 10, adrenaline: 0, sacrifice_pct: 0, upkeep: 0, overcast: 0),
    activation: 2.0,
    recharge: 5.0,
    target: Foe,
    projectile: Some(Standard),
    effects: [
        Damage(to: TargetAndAdjacentFoes, kind: Fire, amount: Scaled(7, 112)),
    ],
    ai: (use_when: [TargetInRange], target: Default),
    roles: [Damage, AoE],
    provenance: ( … ),
)
```

- **Scaling** follows the wiki's `{{gr|at0|at15}}` convention. The value at rank *r* is `round(at0 + r·(at15 − at0)/15)` ([Skill](https://wiki.guildwars.com/wiki/Skill), [Template:Gr](https://wiki.guildwars.com/wiki/Template:Gr)). Rounding rules that differ from the default are marked per value.
- **Costs** mirror the wiki infobox fields: energy, adrenaline, sacrifice, upkeep, and overcast (formerly exhaustion). Activation and recharge are in seconds.

### 8.4 Effect DSL (declarative encoding) [Decided Q22]

The DSL is a Rust enum tree, serialised as RON. It must express the formulaic (~20%) and conditional (~57%) skills. One-of-a-kind skills (~23%) use handlers (§8.5). The percentages come from a 30-skill sample of the wiki.

#### Values

- `Fixed(n)`, `Scaled(at0, at15)` (the skill's own attribute), `ScaledBy(attr, at0, at15)`, `TitleScaled(track, r0, rmax)` (PvE-only skills, `{{gr2}}`), `Percent(…)`, `PerUnit(value, of: <quantity>)` (e.g. damage per point of energy lost), and `Min`/`Max`/`Sum` combinators.

**Selectors** (who is affected)

- `Self_`, `Target`, `TargetFoe`, `TargetAlly`, `TargetOtherAlly`
- `Adjacent(of)`, `Nearby(of)`, `InTheArea(of)`, `Earshot(of)`, `SpiritRange(of)`, `Party`, `PartyInRange(range)`
- `Foes`, `Allies`, `Spirits`, `Minions`, `Corpse`, `Location`
- Filters: `Hexed`, `Enchanted`, `HasCondition(c)`, `Casting`, `CastingSpell`, `Attacking`, `Moving`, `KnockedDown`, `BelowHealth(%)`, `AboveHealth(%)`, `CreatureType(t)`, `IsSpirit`, `IsSummoned`, `HoldingMartialWeapon`, `HoldingCasterWeapon`, `Not(…)`
- Secondary-target scaling for area skills, e.g. the 2026 Mesmer rule that other foes take 75% of the target's damage: `Secondary(factor: 0.75)`.

#### Actions

- `Damage(to, kind, amount, armor_ignoring?)`, `LifeSteal`, `HealthLoss`, `Heal`, `HealthGain`, `SacrificeHealth`
- `GainEnergy`, `LoseEnergy`, `DrainEnergy`, `GainAdrenaline`, `LoseAdrenaline`
- `ApplyCondition(c, duration)`, `RemoveConditions(n, which)`
- `ApplyEffect(effect_def, duration)` for hexes, enchantments, stances and so on; `RemoveEffects(kind, n, selector)`
- `Interrupt`, `KnockDown(duration)`, `DisableSkills(…)`, `ModifyRecharge(…)`
- `Summon(creature_def)`, `CreateSpirit(spirit_def)`, `CreateArea(area_def)` (wells, wards, ground effects)
- `Resurrect(health%, energy%)`, `ShadowStep(to)`, `Teleport(to)`
- `ModifyStat(stat, amount, category)`: armor (core, bonus or special), attack speed, movement speed, activation time, recharge, damage multiplier, block chance, health and energy regeneration pips, max energy, …

#### Control

- `If(cond, then, else)`, `ForEach(selector, actions)`, `Chance(p, actions)`, `Sequence([…])`
- `Triggered(event, filter, actions, charges/duration)`, for effects such as "the next N attacks …" or "when target foe casts a spell …"

**Events** (shared with handlers, §8.5)

- `OnSkillActivationStart/End`, `OnSpellCast`, `OnAttack`, `OnHit`, `OnBlocked`, `OnMiss`
- `OnDamageTaken/Dealt`, `OnHeal`, `OnEffectApplied/Removed/Ended`
- `OnInterrupted`, `OnKnockedDown`, `OnDeath`, `OnKill`, `OnCreatureCreated`, `OnEnergyChanged`, `OnTick`

#### Effect definitions

- An effect definition carries:
  - its kind (§7.1);
  - a stacking key and rule: replace, keep longer, or stack by source;
  - its duration and removal flags;
  - `while_active` modifiers and `triggers`;
  - `on_end` actions;
  - `upkeep`, for maintained enchantments.
- One-at-a-time families (stance, preparation, glyph, weapon spell on a target, form, bundle, party bonus, one binding-ritual spirit of each type per side) are enforced from the stacking key ([Effect stacking](https://wiki.guildwars.com/wiki/Effect_stacking)).

#### AI hints

- `use_when`, `never_when`, target preference, and priority.
- Foes and heroes share them (§11.2). This mirrors the wiki's statement that monster skill use "is embedded into the skill itself".

#### Role tags

- Damage, AoE, pressure, spike, interrupt, hex, enchantment removal, hex removal, condition removal, healing, protection, energy management, snare, resurrection, party buff, minion, spirit, defence, and so on.
- Most tags are derived automatically from the DSL. Manual additions are allowed.
- The optimiser uses them to narrow searches (§13.5), and the tactics generator uses them to plan (§11.6).

### 8.5 Handlers (one-of-a-kind skills)

- A handler is a Rust implementation of a trait, roughly `SkillHandler { on_use, on_event, modify_* , describe }`. It is registered by name and referenced from the skill file as `handler: "soul_twisting"`, together with `params` read from the extracted numbers.
- Handlers may reuse DSL actions internally.
- Every handler needs:
  - unit tests;
  - a `describe` implementation for generated descriptions (§8.6);
  - listed AI hints.
- M1 examples: Panic, Mistrust, Blood Bond, Putrid Bile, Animate Bone Fiend, Soul Twisting, Shelter, Union, Displacement, Life, Protective Was Kaolai, Splinter Weapon, Flesh of My Flesh, Resurrection Chant, Arcane Echo, Air of Superiority. The classification is a judgement made during research; confirm it while encoding.

### 8.6 Generated descriptions [Decided Q25]

- A renderer turns the DSL or handler `describe` into concise English, e.g. "Target foe and adjacent foes take 7…112 fire damage."
- The app shows only generated text. Each skill links to its wiki page.
- **Review aid:** a contributor compares the generated text with the wiki page's description by eye. A mismatch usually means an encoding error. The wiki text itself is never stored in the repo.

### 8.7 Coverage and review status

| Status | Meaning | Used by the evaluator? | Used by the optimiser? |
| --- | --- | --- | --- |
| `NumbersOnly` | Seeded by the extractor; no effect encoding. | No (loading a build with it is an error) | No |
| `Draft` | Encoded and not yet reviewed. | Yes, flagged in results | Yes, flagged (a `--reviewed-only` switch excludes these) |
| `Reviewed` | Reviewed by the owner or a second contributor. | Yes | Yes |

- `gwsim data coverage` reports counts per profession, per status and per campaign, plus a list of missing monster skills per curated encounter.

### 8.8 Assumptions register

- `data/assumptions.ron` lists every value the wiki doesn't document or that the wiki contradicts. Each entry has an ID, statement, value, rationale, status (`Assumed`, `Measured` or `Confirmed`) and source.
- Data files reference assumptions by ID.
- Every report lists the assumptions a run relied on (§14).
- The initial list is in §21.

### 8.9 Validation

- **Schema validation:** RON parse errors report the file, line and column.
- **Cross-references:** skill IDs in foe bars, effect definitions, assumption IDs and wiki page names must all resolve.
- **Consistency:**
  - Scaled values have both endpoints.
  - Elite and PvE-only flags agree with the skill category.
  - Every foe has a level for each mode.
  - Encounters reference existing foes.
  - Every situation's encounters exist.
- **Command:** `gwsim data validate`. It runs in CI (§18.4) and at build time.

### 8.10 Licensing and attribution

- Data files are GPL-3.0-or-later, like the rest of the project [D33, superseding Q38]. `data/ATTRIBUTION.md` credits the Guild Wars Wiki as the source of the facts and records what is deliberately excluded.
- **Never commit:** in-game description text, skill icons or other ArenaNet art, or copied text written by wiki editors.
- The extractor's cache (§9) contains raw wiki HTML, so it is **git-ignored** and never committed.
- *Not legal advice.*

---

## 9. Wiki extractor (developer tool)

### 9.1 Rules (hard requirements)

| ID | Requirement |
| --- | --- |
| EXT-1 | Fetch and obey `robots.txt` at the start of every session. Never request `/api.php`, `/index.php` or `Special:` pages [C1]. |
| EXT-2 | Request only `https://wiki.guildwars.com/wiki/<Title>` article URLs, including `Category:` and project-namespace pages under `/wiki/`. |
| EXT-3 | Wait at least **3 s** between requests [Proposed default, configurable, never below 2 s]. |
| EXT-4 | User-Agent: `gwsim-extractor/<version> (+<project repo URL>)`. **No personal details** (no email) [D15]. |
| EXT-5 | Cache every page locally in `.cache/wiki/` (git-ignored) with its fetch time, so re-runs are resumable. Use conditional requests (`If-Modified-Since` / `ETag`) if the server supports them [Proposed; verify]. |
| EXT-6 | Back off exponentially on 5xx. **Stop** on 403 or 429 and report; never retry aggressively. |
| EXT-7 | Never store wiki prose in `data/`. Description text is read only from the cache, to extract numbers and to help translation. |

### 9.2 Page discovery

- On MediaWiki sites, category page pagination normally goes through `/index.php?…&pagefrom=…`, which is disallowed here. Discovery must therefore use article pages that list everything [Proposed; confirm in the spike]:
  - [Guild Wars Wiki:Game integration/Skills/1-500](https://wiki.guildwars.com/wiki/Guild_Wars_Wiki:Game_integration/Skills/1-500), and its sibling pages up to 3001–3500. These list every skill ID and page. Last edited 2026-08-08, so they lack the newest IDs (e.g. 3443, Frenzy (PvP)).
  - [List of all skills](https://wiki.guildwars.com/wiki/List_of_all_skills) and the per-profession skill list pages.
  - [Skill template format/Skill list](https://wiki.guildwars.com/wiki/Skill_template_format/Skill_list), which has skill IDs.
  - Area pages' `Foes` / `Bosses` sections, for creature pages.
- The first page of a category (under 200 members, or the first 200) is reachable at `/wiki/Category:…` and can be used as a cross-check.

### 9.3 Parsing (from rendered HTML)

| Page kind | What to extract | Notes |
| --- | --- | --- |
| Skill | Infobox fields (profession, attribute, type, energy, adrenaline, sacrifice, upkeep, overcast, activation, recharge, elite, PvE-only, campaign, target, range, AoE, causes/removes tags), scaled values (rank 0 and 15) from the rendered progression table or from `x…y…z` text, and the skill ID | **Spike first:** confirm the rendered HTML exposes every field. The skill ID may not be displayed, so fall back to the game-integration lists. Normalise values such as `{{3/2}}` (fractions), `8%` and `-1`. |
| Foe (`{{NPC infobox}}`) | Name, professions, level `NM (HM)`, species/type, boss flag, the Skills section (skill links, attribute text such as "15 Blood Magic (20 … in Hard mode)", `(Hard mode only)` tags, monster skills), the Armor ratings table (`{{NPC statistics}}`: blunt, piercing, slashing, cold, earth, fire, lightning), locations | Health isn't recorded per creature; use `20 × level + 80`, plus 20 per level above 20 in HM ([Creature](https://wiki.guildwars.com/wiki/Creature), [Level](https://wiki.guildwars.com/wiki/Level)). Weapon damage isn't recorded (assumption A-004). |
| Area | `Foes` / `Bosses` lists with levels, spawn notes | No group composition or positions are available; those are hand-authored. |
| Game update | Per-skill change lines | Context for the change report only. |

### 9.4 Modes

| Command | Behaviour |
| --- | --- |
| `gwsim-extract crawl [--scope skills\|foes\|areas\|all] [--only <titles>]` | Refreshes the cache. Resumable. |
| `gwsim-extract seed [--scope …]` | Writes initial RON files with status `NumbersOnly` (skills) or `Draft` (foes). **Refuses to overwrite existing files** [Decided Q10]. |
| `gwsim-extract diff [--out report.md]` | Re-derives values from the cache and compares them with the committed data. It reports changed fields, new and removed pages, and skills whose description changed (flagged for re-translation). It never writes to `data/`. |

- A full crawl of about 4,000 pages at 3 s per page takes about 3.3 hours. The change report needs a full crawl [D15].

### 9.5 Testing the extractor

- Parser tests use **small synthetic HTML fixtures** that copy the wiki's structure without its prose, so no licensing problem arises.
- Optional local-only tests run against real cached pages in `.cache/` (git-ignored).

---

## 10. Simulation engine

### 10.1 Principles

| ID | Requirement |
| --- | --- |
| ENG-1 | The engine is a pure, deterministic library: its inputs are data, a party, a situation and a seed; its output is a `RunResult`. It does no I/O. |
| ENG-2 | The same inputs and seed produce bit-identical results on the same build [D5]. |
| ENG-3 | Every rule that comes from a wiki formula is implemented once, named after the wiki concept, and unit-tested against the wiki's worked examples (§17.2). |
| ENG-4 | Every non-wiki value comes from the assumptions register, never from a literal in code. |
| ENG-5 | Performance: one 8-v-8 fight in under 10 ms on one core (D19). Measured with a benchmark suite (§17.6). |

### 10.2 Time model [Proposed; confirm in the M0 spike]

- Time is an **integer number of milliseconds**.
- A **priority event queue** schedules discrete events: activation end, aftercast end, recharge end, effect expiry, projectile impact, trigger timers, spawn times.
- A **fixed 50 ms tick** handles continuous processes: movement integration, AI decisions, and regeneration and degeneration accumulation (pips turned into fractional HP and energy per tick, in fixed point).
- Event order within one millisecond is deterministic: sorted by (time, priority class, insertion sequence).

### 10.3 Space [Decided Q8]

- **Field:** an open 2D plane with coordinates in gwinches (`f32`). There is no terrain, obstacles, line of sight or height.
  - Height advantage for ranged attacks is ignored ([Damage calculation](https://wiki.guildwars.com/wiki/Damage_calculation)).
- **Units** are circles with a collision radius (A-002).
  - **Body-blocking** applies between hostile units only: in PvE, friendly characters walk through each other ([Body block](https://wiki.guildwars.com/wiki/Body_block)).
- **Range bands:** range checks use the named bands (§4 Glossary). Some AoE skills use 240 instead of 252; that is a per-skill field.
- **Movement:**
  - Each unit has a base speed (A-001). Modifiers multiply together, capped at +34% and −50% ([Speed boost](https://wiki.guildwars.com/wiki/Speed_boost), [Effect stacking](https://wiki.guildwars.com/wiki/Effect_stacking)). Crippled is −50%.
  - Knocked-down units can't move.
  - Moving cancels an activating skill (see §10.5).
- **Projectiles:**
  - Launched with the weapon's or skill's travel speed (A-003), capped at +100% and −50%.
  - Aimed at the target's predicted position (**leading**) at launch. They miss if the target changes speed or direction enough ([Projectile](https://wiki.guildwars.com/wiki/Projectile)).
  - Projectile spells are not blocked by "block projectile" effects.
- **Areas:** circles centred on a unit or a location. Wells and wards are stationary areas. Spirits affect units within their range (earshot or spirit range, per the skill).

### 10.4 Units and derived stats

- **Unit state:**
  - identity, allegiance, slot kind or foe definition;
  - position, velocity;
  - health, max health, energy, max energy (including overcast), adrenaline per skill;
  - skill bar state (recharge, disabled-until);
  - current action (idle, moving, attacking, activating, aftercast, knocked down, dead);
  - effects list, conditions, upkeep list;
  - weapon, armor per piece and damage type;
  - effective attribute ranks, level;
  - creature traits (fleshy, spirit, minion, undead, …), master (for minions and spirits).
- **Derived stats at start** (from the build, the level and the wiki formulas):
  - **Health:** 100 at level 1 plus 20 per level after, i.e. `100 + 20 × (level − 1)`, the same as `20 × level + 80` (480 at level 20), plus runes (Vigor, Vitae), insignias and rune health penalties ([Health](https://wiki.guildwars.com/wiki/Health)).
  - **Energy:** 20 base, plus profession armor energy (0/5/10), plus Energy Storage, weapon, focus, Attunement runes and Radiant insignias. Regeneration: 2 pips base, plus profession armor (+0/+1/+2) ([Energy](https://wiki.guildwars.com/wiki/Energy)).
  - **Armor:** per piece, per damage type (core, bonus and special categories; §10.7).
  - **Skill values:** a table per skill of every scaled value at the unit's effective rank, recomputed when ranks change temporarily.
- **Foes:**
  - Health: `20 × level + 80`, plus 20 per level above 20 in HM.
  - Armor: from the wiki's armor table, or else `3 × level + profession bonus`.
  - Energy: by profession (W 20, R 30, Mo 40, N 40, Me 40, E 40, A 30, Rt 40, P 30, D 30), with **+1 energy pip** over players.
  - Foes below level 20 in NM get level-20 armor in HM; foes above 20 keep their NM armor ([Creature](https://wiki.guildwars.com/wiki/Creature), [Hard mode](https://wiki.guildwars.com/wiki/Hard_mode)).

### 10.5 Skill use pipeline

| ID | Requirement |
| --- | --- |
| ENG-10 | **Can the skill be used?** The unit is alive; not knocked down (stances, shouts and pet attacks are allowed while knocked down, per [Activation time](https://wiki.guildwars.com/wiki/Activation_time)); the skill is recharged and not disabled; the cost is affordable (energy, adrenaline strikes, and health for sacrifice); the target is valid (allegiance, filters, alive or corpse, as the skill requires); one-at-a-time rules; not mid-action, unless the skill type activates outside the queue (shouts, stances, pet attacks). Flash enchantments can't be used during another activation. |
| ENG-11 | **Range.** If the target is out of range, move to casting range (or melee range for melee), then start. |
| ENG-12 | **Activation time.** Base × item and skill modifiers (capped at −25% and +150%) × Fast Casting 0.5^(rank/15) (outside the cap) × Dazed ×2 (spells). In HM, **foe** skills over 2 s take half as long. Attack skills scale with attack speed. |
| ENG-13 | **Costs.** Energy, adrenaline and overcast are paid at activation start. Sacrifice is taken **after** successful activation ([Resource cost](https://wiki.guildwars.com/wiki/Resource_cost)). |
| ENG-14 | **Outcomes.** (a) **Complete:** effects resolve, then ¾ s aftercast for most skills with an activation time (per-type rules from [Skill type](https://wiki.guildwars.com/wiki/Skill_type)), then recharge starts. (b) **Interrupted:** cost lost, **recharge starts**, queued action cleared. (c) **Cancelled** (moving, retargeting): initial costs paid, **no recharge, no aftercast**. (d) **Fail** (prerequisite missing, low-rank failure, target lost for single-target skills): cost lost, **recharges instantly**. (e) **Fizzle** from an invalid target: no recharge ([Activation time](https://wiki.guildwars.com/wiki/Activation_time), [Interrupt](https://wiki.guildwars.com/wiki/Interrupt)). |
| ENG-15 | **Attack skills without a stated activation time** use the first half of the next attack interval. **With a stated activation time**, the hit lands halfway through the activation. |
| ENG-16 | **Recharge.** Base × modifiers (item and effect reductions capped at −50%; a single skill's own effect may exceed the cap). In PvE, Fast Casting also reduces **Mesmer spell** recharge by 3% per rank, not subject to the cap ([Fast Casting](https://wiki.guildwars.com/wiki/Fast_Casting)). |
| ENG-17 | **Upkeep.** Each maintained enchantment costs −1 energy pip. If degeneration before the cap would be 11 or more, maintained enchantments drop until it is back to 10 ([Energy](https://wiki.guildwars.com/wiki/Energy)). |
| ENG-18 | **Interruptibility.** Anything with an activation time and every attack can be interrupted. Running, instant skills and emotes can't. Knockdown, death, failure and disabling are **not** interrupts, and they bypass interrupt prevention. Dazed spells are easily interrupted, and applying Dazed interrupts a spell in progress. |

### 10.6 Attacks

- **Attack cycle.** The weapon's attack interval is modified by attack speed (+33% and −50% caps; multiplicative). HM foes get about +25% attack speed ([Attack speed](https://wiki.guildwars.com/wiki/Attack_speed), [Hard mode](https://wiki.guildwars.com/wiki/Hard_mode)).
- **Hit resolution** follows this order [Proposed; confirm against the wiki]:
  1. Miss chance: Blind 90%, plus effects; chances stack multiplicatively.
  2. Block or evade chance: multiplicative stacking, e.g. 50% + 50% = 75% ([Block](https://wiki.guildwars.com/wiki/Block)).
  3. Hit location, which chooses the armor piece (§10.7).
  4. Critical hit: base chance from Critical Strikes and Expertise rules; always a crit from behind on a moving target for melee. A crit uses max damage with strike level +20 (×√2) ([Critical hit](https://wiki.guildwars.com/wiki/Critical_hit)).
- **Adrenaline** gained per hit, **weapon conditions** from upgrades, **on-hit triggers**.
- **Attack skills.** "+X damage" is armor-ignoring and is added in the same damage packet. Conditions and effects per the DSL.
- **Weapons in M1:** axe (Kournan Guard), scythe (Zealot), spear (Phalanx), bow (Bowman), staves and wands (casters). Party casters use caster weapons.

### 10.7 Damage, armor and healing

- **Damage per packet:** `base × 2^((strike − AL)/40)`, then armor-ignoring bonus, then damage modifiers in the wiki's listed order ([Order of damage modifiers](https://wiki.guildwars.com/wiki/Order_of_damage_modifiers); the wiki warns that page has errors, see A-022).
  - **Strike level:** 5 × weapon rank up to the threshold `(level + 4)/2` (12 at level 20), then +2 per rank above it. Skills, wands and staves use `3 × character level` ([Damage calculation](https://wiki.guildwars.com/wiki/Damage_calculation)).
- **Armor calculation:**
  1. Core armor.
  2. Net bonus armor:
     - if ≥ 26, add 25 or the largest single bonus;
     - if ≤ 25, add it;
     - if negative, reduce only down to 60, or to core armor if core is below 60.
  3. Armor penetration: `× (1 − AP)`. The result may be 1 lower than expected (A-023).
  4. Special armor, which is uncapped.
  - Known bug: reductions are ignored when net bonus ≥ 26 (A-031).
  - **Insignia armor is local to its piece**; rune bonuses are global ([Armor calculation](https://wiki.guildwars.com/wiki/Armor_calculation)).
- **Hit locations** (normal/ranged): head 12.5%, chest 37.5%, hands 12.5%, legs 25%, feet 12.5%. Low and high attack profiles from the wiki table.
- **Damage types** ([Damage type](https://wiki.guildwars.com/wiki/Damage_type), [Creature type](https://wiki.guildwars.com/wiki/Creature_type)):
  - Physical: blunt, piercing, slashing. Some physical damage is armor-ignoring (e.g. Splinter Weapon, Whirling Defense) but still counts as physical.
  - Elemental: cold, earth, fire, lightning.
  - **Chaos and dark respect armor** (they come mostly from weapons).
  - **Holy:** most holy-damage skills ignore armor, while holy damage from weapons respects it. Creatures with undead sensitivity to light take double holy damage. Tormentor's Insignia increases holy damage taken, which matters for the M1 Blood is Power hero.
  - **Shadow always ignores armor.** Typeless skill damage generally ignores armor.
  - Life stealing and health loss are applied before other damage.
- **Level scaling of skill damage:** `× 2^((3 × level − 60)/40)` (level-30 caster, 70 damage vs 60 AL ≈ 117.7).
- **Healing:** healing modifiers stack multiplicatively; healing reduction is capped at −40%; Deep Wound reduces healing received by 20%.
- **Regeneration and degeneration:** health pips at 2 HP/s, net cap ±10.
  - Natural regeneration starts 5 s out of combat: +1 pip, then +1 more every 2 s, up to +7 ([Health regeneration](https://wiki.guildwars.com/wiki/Health_regeneration)).
  - Units with net degeneration get no natural regeneration.
- **Damage caps and shares:** the damage-cap and damage-redistribution effects used by M1's binding rituals (Shelter, Union, Displacement, Armor of Unfeeling) go through the modifier order above.

### 10.8 Effects and stacking

| ID | Requirement |
| --- | --- |
| ENG-30 | Implement all 10 conditions. Reapplying keeps the longer duration. Duration reductions from separate sources apply and round separately (e.g. an 8 s Blind with two 20% reductions → 4 s). Bleeding, Poison and Disease affect fleshy creatures only. Spirits are immune to hexes and to every condition except Burning. Disease spreads between adjacent creatures of the same type ([Condition](https://wiki.guildwars.com/wiki/Condition)). |
| ENG-31 | Hexes and enchantments are removable by the appropriate skills. Beneficial effects of hexes stack across different targets and different casters. Most skill effects don't stack: the newest or strongest wins, per the stacking key. |
| ENG-32 | One at a time: stance, preparation, glyph, form, party bonus, one weapon spell per target, one bundle (a new item spell drops the old one and triggers its drop effect). Binding rituals replace an **allied** spirit of the same type. Nature rituals replace allied **and** enemy spirits of the same type. |
| ENG-33 | Caps: attack speed +33%/−50%; movement +34%/−50%; activation −25%/+150%; recharge −50%; healing −40%; adrenaline gain +100%/−50%; regeneration ±10; bonus armor +25; attribute 20. A single skill's own effect may exceed the caps where the wiki says so ([Effect stacking](https://wiki.guildwars.com/wiki/Effect_stacking)). |
| ENG-34 | Percentage modifiers stack multiplicatively: attack speed, movement speed, healing, adrenaline gain, recharge, damage multipliers, block and miss chances. |
| ENG-35 | Knockdown: a duration during which the unit can't act (except stances, shouts and pet attacks) and its activation is stopped. It bypasses interrupt prevention ([Knock down](https://wiki.guildwars.com/wiki/Knock_down)). |

### 10.9 Attribute inherent effects

- **Fast Casting:** activation and Mesmer recharge (ENG-12, ENG-16). Signets: the wiki has two conflicting formulas (A-017).
- **Soul Reaping:** energy gained when non-spirit creatures near the Necromancer die. The Reforged-era rules and cap come from the wiki page.
- **Spawning Power:** +4% health per rank for creatures the Ritualist creates, and weapon-spell duration.
- **Energy Storage:** +3 max energy per rank.
- **Strength, Expertise, Critical Strikes, Mysticism, Leadership, Divine Favor:** as their wiki pages describe. Needed when their professions come into coverage. M1 needs Fast Casting, Soul Reaping, Spawning Power (ST and SoS Ritualists) and the Kournan foes' primary attributes (Strength, Expertise, Divine Favor, Mysticism, Leadership).

### 10.10 Creatures created by skills

- **Minions:**
  - Control cap: 2, plus 1 per 2 ranks of Death Magic.
  - Health degeneration starts at −1 and worsens by 1 pip every 20 s.
  - Not fleshy.
  - Bone Fiend: armor `2.84 × level + 3.1`; ranged piercing attack every 1.86 s at half longbow range.
  - Level comes from the skill ([Minion](https://wiki.guildwars.com/wiki/Minion)). Animate skills need an exploitable corpse (a fleshy creature's corpse, not already used).
- **Spirits:**
  - Created by rituals.
  - Stationary.
  - Affect creatures within range, per the skill.
  - Killable. Health and armor come from the wiki's unofficial values (A-015). In HM, hostile spirits have 100 armor.
  - Binding-ritual spirit health scales with Spawning Power.
- **Summons** (later phases): e.g. Ebon Vanguard Assassin Support has its own skill bar and a lifetime.

### 10.11 Death, resurrection and chain state

- Death clears most effects. Corpses remain exploitable until used.
- **Death Penalty** is −15% per death, max −60%. **Morale Boost** is up to +10% ([Death](https://wiki.guildwars.com/wiki/Death)).
- Resurrection skills restore a percentage of health and energy.
- Resurrection shrines are **not modelled**. A wipe ends the situation as a loss.
- **Dhuum's Covenant switch:** any party death is reported as "covenant broken", while the run continues. [Proposed]
- **Chains:** after each fight a rest period runs (default A-028).
  - During rest: regeneration, natural regeneration, recharges, effect durations, minion degeneration, overcast recovery, and hero AI's out-of-combat rules (e.g. maintained enchantments are dropped out of combat except the wiki's listed exceptions).
  - Carried into the next fight: health, energy, recharges, effects, minions, spirits (if in range and alive), DP, morale, consumable timers.

### 10.12 Game-mode switches [Default D1]

- **Hard Mode:**
  - foe levels from the HM column (per-foe data first, otherwise the wiki's level mapping);
  - +20 health per level above 20;
  - higher attributes (per-foe data, otherwise A-005);
  - about +33% movement and about +25% attack speed;
  - skills over 2 s activate in half the time;
  - shorter recharges (A-032);
  - non-boss foes gain an elite skill (per-foe HM bars);
  - "superior AI" (§11.3).
  - Allies at levels 0–20 become 20.
- **Reforged Mode:** pre-Searing foes −20% health and about −20% armor; henchman bar and level changes; new heroes. It has no effect on the M1 content.
- **Melandru's Accord:** restricts skill pools to learned skills, via the account profile's learned-skills list, and heroes to their starting, trainer-bought and hero-trainer skills. Account titles and their passive effects are unavailable ([Optional game modes](https://wiki.guildwars.com/wiki/Optional_game_modes)).
- **Dhuum's Covenant:** see §10.11.

### 10.13 Randomness and reproducibility

| ID | Requirement |
| --- | --- |
| ENG-40 | A seeded, portable PRNG (e.g. ChaCha8 or PCG). Every random draw goes through the engine's RNG. |
| ENG-41 | **Separate RNG streams** per purpose (hits and crits, AI choices, spawn jitter), derived from the run seed, so that changing one build changes as little of the random sequence as possible. |
| ENG-42 | **Common random numbers:** the optimiser runs all candidates on the same list of seeds at each evaluation stage, so comparisons are paired (§13.4). |
| ENG-43 | A result is reproduced exactly from (data pack version, party, situation, seed). |

### 10.14 Performance design [Proposed]

- Units live in flat arrays indexed by `UnitId`; the event queue uses a pre-allocated binary heap. No allocation in the hot loop after setup (arena / `SmallVec` patterns).
- Skill values are precomputed per unit at fight start and recomputed only when a rank changes.
- Spatial queries: brute force is fine for 16–40 units. A uniform grid is added only if profiling shows the need.
- Runs execute in parallel across cores (e.g. rayon), one run per task. Parallelism is across runs, not within a run.
- The combat log and replay frames are recorded only when requested, so the hot path pays nothing for them.

---

## 11. AI and tactics

### 11.1 Controllers

Each unit has a controller that is asked to decide at every AI tick, or on relevant events:

| Controller | Used by |
| --- | --- |
| `FoeAi` | Foes (NM/HM variants) |
| `HeroAi` | Hero slots, and henchmen until henchman AI differs [Proposed] |
| `PlanAi` | Human slots (priority plan) |
| `MinionAi`, `SpiritAi`, `SummonAi` | Created creatures (minions follow and attack; spirits are stationary and act by rule) |
| `ScriptAi` | Boss or special scripts (handler-based), layered over `FoeAi` |

#### Shared by all controllers

- reaction delays (A-010 to A-012);
- the decision cadence;
- validity checks (§10.5);
- targeting helpers;
- movement helpers: approach to range, keep range, flee area damage, follow a flag or formation position.

### 11.2 Per-skill usage rules (shared)

- Each skill file's `ai` block says when the skill is worth using and on whom. Foes use these rules for their skills, and heroes use them too, adjusted by `HeroAi` quirks.
- Sources:
  - the wiki's foe behaviour rules ([Foe](https://wiki.guildwars.com/wiki/Foe)). For example, Backfire-type punishments only on targets holding a spellcasting weapon; some Blind skills only on martial-weapon users in NM, and only on martial-primary users with martial weapons in HM;
  - [Hero behavior](https://wiki.guildwars.com/wiki/Hero_behavior) and [Hero-vetted skills](https://wiki.guildwars.com/wiki/Category:Hero-vetted_skills);
  - 2026 update notes (e.g. Judge's Insight and "Find Their Weakness!" only on martial-weapon users; Splinter Weapon targets allies again; wells only when targets are in range).

### 11.3 Foe AI [Decided Q18, Default D13]

| ID | Behaviour |
| --- | --- |
| AI-F1 | **Aggro:** a group engages when a party member enters its aggro range (A-009). All members of a group aggro together. |
| AI-F2 | **Targeting:** prefer the **weakest** (by health and armor) and the **closest**. Body-blocking can hold melee foes back. |
| AI-F3 | **Skill use:** per-skill rules (§11.2). Monsters with the same skill use it the same way. |
| AI-F4 | **Scatter:** foes run out of damage-over-time AoE. Single-hit AoE doesn't cause scatter. All targets in a group scatter together. In PvE they also step away when targeted by AoE. **HM foes react faster** ([Scatter](https://wiki.guildwars.com/wiki/Scatter), [Area of effect](https://wiki.guildwars.com/wiki/Area_of_effect)). |
| AI-F5 | **Special movers:** some Monk and Ritualist foes kite, and some foes are stationary (per-foe AI tags). |
| AI-F6 | **HM "superior AI":** reacts faster to damage-over-time AoE, fights longer, balls up less on one target, kites more, skips pure attack-speed skills ([Hard mode](https://wiki.guildwars.com/wiki/Hard_mode)). |
| AI-F7 | **Boss scripts:** only where a curated encounter needs them (none in M1). |
| AI-F8 | **Casters:** keep casting range and don't move while activating. Melee foes chase their target. |

### 11.4 Hero AI [Decided Q19]

The goal is to emulate documented hero behaviour, including its weaknesses, updated with the 2026 AI changes:

| ID | Behaviour |
| --- | --- |
| AI-H1 | **Targeting order:** player-locked target → called target → the target a player is attacking. In Fight mode, prefer the foe with the lowest armor, then the lowest health ([Hero](https://wiki.guildwars.com/wiki/Hero)). |
| AI-H2 | **Modes:** Fight, Guard, Avoid Combat (set in the tactics plan). |
| AI-H3 | **No coordination:** several heroes may heal, remove or resurrect the same target. Hex, condition and interrupt targets are chosen roughly at random. |
| AI-H4 | **Interrupts** are never late: no reaction delay for interrupts (A-011). |
| AI-H5 | Won't reapply effects that are already active. Reads health bars, hexes, conditions, enchantments and energy. |
| AI-H6 | Batteries (e.g. Blood is Power) go only on casters, or on martials holding a caster weapon, at about 50% energy. Attunements are always cast. Maintained enchantments are dropped out of combat except the listed exceptions. Shouts are used only in combat (except speed boosts). Wards and wells are cast in combat, and wells only when targets are in range (2026). |
| AI-H7 | **2026 updates:** melee heroes stick to their previous target; heroes tolerate AoE damage down to **65%** health before escaping (was 80%); melee heroes give auto-attacks lower priority until low on energy; per-skill AI tweaks from the May–August 2026 notes. |
| AI-H8 | **No pre-casting** of protection or spirits before a fight on their own. Pre-fight setup comes from the tactics plan, which models the player ordering it (§11.6). |
| AI-H9 | Skills can be **turned off** for automatic use (tactics plan), matching the in-game hero skill toggles. |

- The wiki flags [Hero behavior](https://wiki.guildwars.com/wiki/Hero_behavior) as outdated after the 2019 and June 2026 changes. Where the notes and the page disagree, the update notes win, and the choice is recorded as an assumption (A-026).

### 11.5 Human plan AI [Decided Q19]

- **Generation:** a **priority plan** is generated per build from the DSL, role tags and AI hints. It is an ordered list of rules: `(condition → skill → target selector)`, plus maintenance rules ("keep X up") and a default action.
  - Example for the M1 player: keep Air of Superiority up; use Arcane Echo, then Energy Surge on the foe with the most energy in range; Mistrust on a caster; Unnatural Signet on hexed or enchanted targets; Cry of Frustration and Power Drain as interrupts on spells; Spiritual Pain on a target with nearby foes (exact rules are written during M1).
- **Editing:** the plan is RON, and users can edit it. Edited plans are stored with the party.
- **Execution:** the plan runs exactly as written, with a human reaction delay (A-012) and no hero quirks.

### 11.6 Tactics plan [Decided Q29, Q37]

```text
TacticsPlan {
  formation: Vec<(SlotRef, RelativePosition)>,     // frontline / midline / backline offsets
  hero_modes: Map<SlotRef, Fight|Guard|AvoidCombat>,
  called_targets: Vec<TargetRule>,                 // e.g. "call the Kournan Priest first"
  locked_targets: Map<SlotRef, TargetRule>,
  pre_fight: Vec<(SlotRef, SkillRef, TargetRule)>, // ordered pre-cast sequence
  disabled_hero_skills: Map<SlotRef, Set<SkillRef>>,
  engage_order: Vec<GroupRef>,                     // which group to engage first (multi-group)
  spread_against_aoe: bool,                        // static flags apart
}
```

- **Generated from each candidate build** by rules [Decided Q37]. For example:
  - backline heroes on **Guard**, midline on Fight or Guard;
  - pre-cast **Shelter → Union → Displacement → Armor of Unfeeling** when they are on a bar (from the PvX Mesmerway notes);
  - raise minions from any available corpses;
  - call the foe healer first;
  - spread heroes apart when the foes have AoE.
- **Overrides:** the user can edit the plan in the situation or party file. The optimiser keeps user overrides fixed and regenerates everything else per candidate.
- **No scripted mid-fight actions** [Decided Q29, D14]. The flags are static; there are no timed orders.

---

## 12. Situations, encounters and chains

### 12.1 Encounter files

```ron
// data/encounters/curated/nightfall/vehtendi-valley/kournan-patrol.ron (shape only)
Encounter(
    name: "Kournan patrol",
    area: "Vehtendi Valley",
    campaign: Nightfall,
    groups: [
        Group(
            foes: [
                (foe: "kournan-guard", variant: "axe", count: 1),
                (foe: "kournan-zealot", count: 1),
                (foe: "kournan-phalanx", count: 1),
                (foe: "kournan-bowman", count: 1),
                (foe: "kournan-scribe", count: 1),
                (foe: "kournan-seer", count: 1),
                (foe: "kournan-oppressor", count: 1),
                (foe: "kournan-priest", count: 1),
            ],
            formation: Cluster(radius: 150),            // [Proposed] layout primitive
            position: (x: 0, y: 1800),                  // relative to party start
            ai_tags: [],
        ),
    ],
    party_start: (x: 0, y: 0),
    provenance: ( … assumptions: ["A-006", "A-008"] ),
)
```

- **Layout primitives** [Proposed]: `Cluster(radius)`, `Line(spacing)`, `Explicit([...])`, plus `patrol: Option<Path>` for later.
- **Levels:** taken from each foe's NM/HM data according to the situation's HM switch. They can be overridden per group.

### 12.2 Generic scenarios [Decided Q7, Default D25]

- **Training dummies:** stationary, configurable level, armor, health and energy. The energy matters for testing energy-damage skills such as Energy Surge.
- **Archetype groups** built from real foe data (the Kournan roster for M1): melee-heavy, caster-heavy and healer-heavy mixes at HM level 26.
- Later: archetype groups from other rosters, and "boss plus escort".

### 12.3 Situations [Default D1]

- A situation sets:
  - a single encounter or a chain;
  - the mode switches;
  - party size;
  - active consumables [Decided Q21];
  - starting DP and morale;
  - the timeout (A-030; a timeout counts as a loss);
  - tactics overrides.
- **Win:** all hostile units of the encounter (or chain) are dead.
- **Loss:** a party wipe or a timeout.

### 12.4 Chains [Decided Q15]

- An ordered list of `(encounter, rest_after_s)`. See §10.11 for what carries over.
- Chain metrics: total time, total deaths, DP at the end, and the fight in which a chain first failed.

### 12.5 Situation sets [Decided Q16]

- A weighted list of situations. Weights are relative.
- Validation: every situation must be valid for the party's size.

### 12.6 User-made encounters

- Users write encounters in the same RON format, in their user data directory. The desktop app offers an editor (§16).
- Validation rejects unknown foes and missing foe skills, and flags `NumbersOnly` skills (a foe using them can't be simulated).

---

## 13. Optimiser

### 13.1 Search space

- **Per free slot:**
  - secondary profession (and primary, for human slots; hero slots have a fixed primary);
  - attribute allocation;
  - 8 skills;
  - 5 armor pieces (insignia and rune each) and the headgear attribute;
  - one weapon set (weapon type, off-hand, prefix, suffix, inscription).
- The space is far too large to enumerate.

### 13.2 Constraints and repair

| ID | Constraint |
| --- | --- |
| OPT-1 | Build legality (§7.2): professions, elite ≤ 1, PvE-only ≤ 3 (heroes 0), skills from allowed professions, attribute budget 200 with ranks ≤ 12 from points, rune and insignia legality, non-stacking runes (only the highest attribute rune per attribute counts; every rune's health penalty applies). |
| OPT-2 | Account profile: unlocked skills, heroes owned, available upgrades, title ranks (for PvE-only skill values), Melandru's Accord restrictions. |
| OPT-3 | Locked slots are never changed. Locked skills or gear within a free slot are allowed [Proposed]. |
| OPT-4 | Coverage: only `Draft` or `Reviewed` skills are candidates (§8.7). |
| OPT-5 | **Repair operators** turn any offspring into a legal build: drop extra elites and PvE-only skills, re-derive attributes for the skills chosen, fix runes after a profession change. |

- **Attribute allocation:** a heuristic proposes a spread that raises the attributes the chosen skills use (weighted by how many skills use each attribute and how they scale), within the 200-point budget. Mutations then shift points [Proposed].

### 13.3 Search algorithm [Decided Q27]

- **Evolutionary multi-objective search (NSGA-II style):**
  - Non-dominated sorting with crowding distance on the chosen objectives.
  - **Constrained domination:** builds below the success threshold are dominated by builds above it, and are ranked among themselves by success rate.
  - Mutation operators:
    - replace a skill (sampled from a role-compatible pool, §13.5);
    - swap the elite;
    - shift attribute points;
    - change the secondary profession (then repair);
    - change a rune or insignia;
    - change weapon upgrades.
  - Crossover operators:
    - slot-level (swap whole builds per slot between parents);
    - bar-level (uniform crossover over skills, then repair).
  - **Seeding:** benchmark builds, the user's current build, and heuristic builds (§13.5).
  - Defaults [Proposed]: population 64; the run stops at a time budget (default 5 min) or when the frontier has stopped improving. Anytime results: the current frontier can be read while the search runs.
- **Exhaustive mode** [Decided Q27]:
  - Enumerates every combination from a user-given pool (e.g. choose 3 of 20 skills for a slot's free positions) with everything else fixed.
  - Refuses to run if the count exceeds a limit (default 50,000 candidates [Proposed]) and suggests evolutionary mode instead.
  - Uses the same adaptive evaluation (§13.4).

### 13.4 Adaptive evaluation [Proposed]

- **Racing / successive halving:**
  - Every candidate is first evaluated with 16 runs on the shared seed list (CRN, ENG-42).
  - The top half is promoted to 64 runs, and the frontier candidates to 256.
  - Success-rate confidence uses Wilson intervals. A candidate is eliminated only when it is significantly worse.
- **Cache:** evaluations are keyed by (canonical build hash, situation, seed list, data pack version).
- **Parallelism:** runs are spread across all cores. Progress and ETA are reported.

### 13.5 Role tags and narrowing

- Each free slot's candidate skills are weighted by role tags. There is always some exploration: a small probability of drawing any legal skill.
- A slot's **role** can be inferred from its current or seed build (e.g. healer, protection, damage, interrupt), or set by the user [Proposed].
- **Heuristic seed builds:** for each role and profession, a greedy bar of the highest-rated skills for that role, used to seed the population.

### 13.6 Objectives and ranking [Decided Q17]

- **Success threshold:** default **≥ 95%** wins, judged per situation [Proposed: per situation; option: weighted aggregate].
- **Goals the user can pick:**
  - fastest clear;
  - fewest deaths;
  - least damage taken;
  - most energy left;
  - lowest DP at the end of a chain;
  - weighted combinations of these.
  - Default: **fastest clear**.
- **Situation sets:** a build's objective values are weighted means over the situations; its success must meet the threshold in every situation with non-zero weight [Proposed].
- **Output:** the frontier (trade-off view) plus a ranked list by the chosen goal.

### 13.7 First optimiser tests [Default D30]

- **O1:** "Best player build for the M1 Mesmerway team" against the M1 situation set. The PvX Energy Surge bar should rank highly among sampled player builds. That is a relative check (§17.4).
- **O2:** the Solo Resto variant as the second benchmark. It adds the Ineptitude and Splinter Support bars.

---

## 14. Results and reports

All seven result types are required [Decided Q28]:

| # | Report | Command line (text + JSON) | Desktop |
| --- | --- | --- | --- |
| 1 | **Ranked builds with template codes.** Skill template codes (type 14) are loadable by PvE characters and heroes. Equipment is shown as a readable list plus an equipment code (type 15) for sharing. The game lets only PvP characters load equipment codes, and hero equipment codes contain weapons only ([Template](https://wiki.guildwars.com/wiki/Template)). | ✓ | ✓ |
| 2 | **Metrics with confidence ranges:** win rate (Wilson 95%), clear time, deaths, damage taken, energy left, DP (chains). | ✓ | ✓ |
| 3 | **Trade-off chart** (frontier). | JSON data only | ✓ chart |
| 4 | **Contribution breakdown:** damage, healing and mitigation per skill and per party member; energy over time (sampled every second). | ✓ | ✓ charts |
| 5 | **Combat log:** timestamped events for any seeded run. | ✓ (JSON Lines + text) | ✓ |
| 6 | **2D replay** of a run. | exports a replay file | ✓ viewer |
| 7 | **Side-by-side comparison** of two builds or teams. | ✓ | ✓ |

### 14.1 Rules common to every report

- Absolute numbers are labelled **"uncalibrated"** [D22].
- Each report lists:
  - the **assumptions** the run relied on (IDs and statements);
  - any **draft skills** used (with a warning);
  - **coverage** limits that affected the search (e.g. "3 of 146 Mesmer skills unavailable: NumbersOnly");
  - the data pack version and seeds.

### 14.2 Contribution attribution [Proposed]

- **Damage:** credited to the unit and skill that dealt it. Minion damage goes to the master. Spirit damage and effects go to the caster. Damage-over-time goes to whoever applied the effect.
- **Healing:** credited to the healer and skill. Overhealing is tracked separately.
- **Mitigation:** damage prevented by damage caps, damage reduction, redistribution, blocks and prevented hits, credited to the effect's source (for M1: Shelter, Union, Displacement, Armor of Unfeeling, Protective Was Kaolai and similar).
- **Interrupts:** landed interrupts per skill, and what they stopped.

### 14.3 File formats [Proposed]

- **Result JSON:** a schema-versioned document holding the inputs, aggregates, per-run summaries, the frontier and references.
- **Combat log:** JSON Lines, one line per event, e.g. `{t_ms, kind, source, target, skill, amount, flags}`.
- **Replay:** a compact file with a header (units, data version, seed), 10 Hz position frames, and the event stream.

---

## 15. Command-line interface

Command names are [Proposed]. The capabilities are required.

| Command | Purpose |
| --- | --- |
| `gwsim evaluate --party <file\|codes> --situation <id\|file> [--runs N] [--seed S] [--json out.json]` | UC1: evaluate a party. |
| `gwsim optimise --party <file> --free <slot…> --situations <set\|id> [--goal …] [--threshold 0.95] [--budget 5m] [--mode evolutionary\|exhaustive --pool <file>] [--reviewed-only]` | UC2–UC5, UC8. |
| `gwsim compare <partyA> <partyB> --situations …` | UC6. |
| `gwsim log --result <file> --run <n>` | Print or export the combat log of a seeded run (re-simulated deterministically). |
| `gwsim replay export --result <file> --run <n> -o <file>` | Write a replay file for the desktop viewer. |
| `gwsim template decode <code>` / `gwsim template encode <build file>` | Template codec. |
| `gwsim data validate` · `gwsim data coverage [--profession …]` · `gwsim data describe <skill>` | Data tools: validation, coverage, generated description. |
| `gwsim profile list\|show\|edit <name>` | Manage account profiles. |
| `gwsim check [--suite relative\|unit\|all]` | Run the relative checks (§17.4) outside `cargo test`, e.g. after a patch update. |

- **Party file:** RON with one entry per slot, holding the slot kind, a template code or build, the equipment, and optional AI plan overrides.

---

## 16. Desktop app

The desktop app is built with **egui (eframe)** [Decided Q39] and arrives **after the optimiser** [Decided Q43].

### 16.1 Screens

- **Party and build editor:**
  - template code import and export;
  - skill picker with filters: profession, attribute, type, role tags, coverage status;
  - attribute allocator that shows the points budget;
  - armor, runes and insignias;
  - weapon set;
  - lock toggles.
- **Tactics editor:** formation diagram, hero modes, called targets, pre-fight sequence, disabled hero skills. It starts from the generated plan.
- **Situation picker and editor:** browse curated, generic and user encounters; set the mode switches, consumables, chain editor, situation sets and weights; plus an encounter layout editor.
- **Account profile editor:** unlocked skills (bulk toggles), title ranks, heroes, EotN, available upgrades.
- **Run panel:** evaluate or optimise, with progress, ETA and a live frontier.
- **Results:**
  - ranked list with template codes (copy buttons);
  - trade-off chart (egui_plot);
  - contribution charts, energy timeline;
  - assumption and coverage notes.
- **Replay viewer:**
  - a 2D canvas with a timeline scrubber and play/pause/speed controls;
  - units drawn as circles coloured by allegiance and profession;
  - AoE circles, projectiles, spirit ranges;
  - a floating health and energy bar;
  - the event log synced to the timeline.
- **Comparison view:** two builds or teams side by side, with the difference in each metric.

### 16.2 Visual identity without ArenaNet art [Decided Q25]

- There are no skill icons. Skills appear as chips showing the name, coloured by profession, with a small type glyph (spell, hex, enchantment, signet, shout, ritual, attack, …) drawn by the app. Every chip links to its wiki page.

### 16.3 Packaging

- A single self-contained Windows executable with the data pack embedded, published through GitHub Releases [D14].
- No installer is required [Proposed]. There are no auto-updates and no telemetry.

---

## 17. Validation and testing

### 17.1 Test layers

| Layer | What it proves | Where |
| --- | --- | --- |
| Unit: formulas | Each wiki formula reproduces the wiki's worked examples (§17.2). | `gwsim-data`, `gwsim-engine` |
| Unit: skills | Each encoded skill's values at ranks 0, 12 and 15 match the extracted numbers. The generated description renders. Its handler hooks (if any) behave as described. | per skill (generated tests + handler tests) |
| Data | `gwsim data validate`: schemas, cross-references, consistency. | CI |
| Determinism | The same seed gives the same result. Changing an unrelated build doesn't change another unit's hit rolls beyond what CRN allows. | engine |
| Properties | Caps are never exceeded; health never exceeds max; energy only goes below 0 by max-energy reduction, as the wiki allows; attribute ranks ≤ 20; no event is scheduled in the past. | proptest |
| Scenario snapshots | Short fixed-seed fights produce a stable combat-log snapshot (changes must be reviewed). | insta-style snapshots |
| Relative checks | Benchmarks beat weakened variants and baselines (Q34). | `gwsim check`, CI (reduced run counts) |
| Performance | Fight time per core under target (D19). | benchmark suite |

### 17.2 Wiki worked examples to encode as tests (initial list)

| Test | Expected | Source |
| --- | --- | --- |
| Skill scaling | `round(at0 + r·(at15−at0)/15)`; `x…y…z` = ranks 0, 12, 15 | [Skill](https://wiki.guildwars.com/wiki/Skill) |
| Attribute cost | Rank 12 costs 97 points; costs 1, 2, 3, 4, 5, 6, 7, 9, 11, 13, 16, 20 | [Attribute point](https://wiki.guildwars.com/wiki/Attribute_point) |
| Health at level 20 | 480 | [Health](https://wiki.guildwars.com/wiki/Health) |
| Energy by profession | Warrior 20/2 pips; Ranger 25/3; Dervish and Assassin 25/4; Paragon 30/2; casters 30/4 | [Energy](https://wiki.guildwars.com/wiki/Energy) |
| Max Elementalist energy | 136 (without temporary effects), using the wiki's itemised sum | [Energy](https://wiki.guildwars.com/wiki/Energy) |
| Armor calculation | Core 106, net bonus −25 → 81; 25% AP → 61 (±1, A-023); special +24 → 85 | [Armor calculation](https://wiki.guildwars.com/wiki/Armor_calculation) |
| Skill damage vs level | Level 15 → 77.1%; level 30 caster, 70 damage vs 60 AL ≈ 117.7 | [Damage calculation](https://wiki.guildwars.com/wiki/Damage_calculation) |
| Weapon damage vs rank | Rank 8 → 70.7%; rank 16 → 114.9% of rank-12 damage (level 20) | [Damage calculation](https://wiki.guildwars.com/wiki/Damage_calculation) |
| Block stacking | 50% + 50% → 75% | [Effect stacking](https://wiki.guildwars.com/wiki/Effect_stacking) |
| Movement stacking | Flail + "Fall Back!" → 89.1% | [Effect stacking](https://wiki.guildwars.com/wiki/Effect_stacking) |
| Condition duration | 8 s Blind with two 20% reductions → 4 s | [Condition](https://wiki.guildwars.com/wiki/Condition) |
| Minion cap | 2 + ⌊Death Magic / 2⌋ (8 at 12) | [Minion](https://wiki.guildwars.com/wiki/Minion) |
| Natural regeneration | +1 pip at 5 s, +1 every 2 s, max +7 | [Health regeneration](https://wiki.guildwars.com/wiki/Health_regeneration) |
| Template decode | `OQBTAUBPQaJ4EY6x0BAAAAAAuE` → Me / no secondary; Fast Casting 10, Domination 12, Inspiration 8 | PvX (§20), [Skill template format](https://wiki.guildwars.com/wiki/Skill_template_format) |
| HM foe stats | Health +20 per level above 20; skills > 2 s activate in half the time; level mapping table | [Hard mode](https://wiki.guildwars.com/wiki/Hard_mode) |

### 17.3 Unit tests per skill [Default D28]

- For every `Draft` or `Reviewed` skill, generated tests check:
  - the scaled values at ranks 0, 12 and 15 against the provenance numbers;
  - the costs, activation and recharge against the extracted values;
  - that the description renders.
- Handlers add behaviour tests (e.g. Shelter caps damage; Union reduces damage; Soul Twisting changes ritual behaviour).

### 17.4 Relative checks [Decided Q34]

| ID | Check | Pass rule [Proposed thresholds] |
| --- | --- | --- |
| RC1 | M1 Mesmerway team + player vs **weakened variants** in the Kournan patrol (HM). The variants are: each hero's elite removed; 20 random off-role skill swaps; attributes misallocated (points moved to unused attributes); no runes or insignias. | Mesmerway's win rate and clear time beat at least 90% of the variants, with statistical confidence, and beat each "elite removed" variant. |
| RC2 | M1 Mesmerway vs a **naive baseline** team (7 heroes with minimal, auto-attack-centred bars). | Win rate higher by a wide margin (≥ 30 percentage points), or equal win rate with a clear-time improvement of ≥ 30%. |
| RC3 | *(Once henchman data exists)* vs an **all-henchman** party. | Mesmerway wins more and faster. |
| RC4 | *(M2)* The PvX player Energy Surge bar ranks in the top 10% of 1,000 randomly generated legal player builds for this team. | As stated. |
| RC5 | *(M2)* The optimiser's best player build is at least as good as the PvX bar under identical conditions. | No worse, within confidence. |
| RC6 | **Monotonicity sanity checks:** raising Domination Magic doesn't lower Energy Surge damage; adding a healer to a failing team doesn't lower its win rate; and so on. | Always holds (property tests). |

- The relative checks run in CI with reduced run counts. The full versions run with `gwsim check`.

### 17.5 Review workflow (skills)

1. The extractor seeds `NumbersOnly` files.
2. Claude encodes a batch (DSL or handler, AI hints, role tags) → status `Draft`.
3. Tests run: per-skill value tests, and the generated description is shown.
4. The owner reviews the generated description against the wiki page and spot-checks behaviour in the M1 scenarios → status `Reviewed`.

### 17.6 Performance tests

- A benchmark suite (criterion-style) times:
  - M1 Kournan patrol HM fights (8 party + 8 foes + spirits and minions);
  - the two-fight chain;
  - a synthetic 8-v-16 stress fight.
- Target: median under 10 ms per fight on one core, on the development machine (D19).
- Regressions over 20% fail CI [Proposed].

---

## 18. Repository, tooling and engineering practice

### 18.1 Layout

```text
gwsim/
├── Cargo.toml                    # virtual workspace manifest
├── rust-toolchain.toml           # channel = "stable" + rustfmt, clippy [Proposed]
├── crates/
│   ├── gwsim-data/  gwsim-engine/  gwsim-opt/  gwsim-cli/  gwsim-desktop/  gwsim-extractor/
│   │   └── each with its own src/, tests/ and benches/ [Proposed]
├── data/                         # GPL-3.0-or-later, like the rest (§8) [D33]
│   └── ATTRIBUTION.md            # source of the facts; what is excluded (§8.10)
├── docs/
│   ├── DESIGN.md                 # this document
│   ├── plan/                     # phase > work package > task plan (§19) [Proposed]
│   ├── findings/                 # research task outputs [Proposed]
│   ├── effect-dsl.md             # DSL reference (generated from types + examples) [Proposed]
│   └── data-authoring.md         # contributor guide for skills, foes, encounters [Proposed]
├── .github/
│   ├── workflows/ci.yml          # once the repo has a GitHub remote
│   └── pull_request_template.md  # incl. the licensing checklist [Proposed]
├── LICENSE                       # GPL-3.0-or-later, whole project [D32, D33]
├── README.md                     # incl. Getting Started from zero [D21]
├── CONTRIBUTING.md
└── .gitignore                    # Rust template + research/ + .cache/ [D9, Q42]
```

- `research/` stays local and must be git-ignored [Decided Q42]. This is a licensing
  requirement as well as a tidiness one: the corpus contains wiki prose under the GFDL,
  which is not compatible with the GPL-3.0-or-later licence on this project (T0.5.2).
- `.cache/` (the extractor's cache, holding raw wiki HTML) must be git-ignored (§8.10).
- **Tests and benchmarks are per-crate, not at the root** [Proposed, T0.3.1]. A virtual
  manifest has no package of its own, so it cannot host root `tests/` or `benches/`
  directories. Cross-crate integration tests and the relative checks (§17.4) therefore live
  in `crates/gwsim-cli`.
- **`gwsim-cli` has a library target** (`src/lib.rs`) alongside `src/main.rs` [Proposed,
  T0.3.1], so that `gwsim check` and `cargo test` run the same check code rather than two
  copies of it. This keeps the six-crate decision of §6.1 intact.

### 18.2 README "Getting Started" (content outline) [Default D21]

1. **Prerequisites (Windows 11):**
   - install the **Visual Studio C++ Build Tools** (the "Desktop development with C++" workload);
   - install **rustup** (stable MSVC toolchain);
   - install **Git**.
2. **Clone the repository.**
3. **Build:** `cargo build --release`.
4. **Run your first evaluation:** `gwsim evaluate --party data/…/mesmerway-dual-resto.ron --situation kournan-patrol-hm`.
5. **Run the tests:** `cargo test`, and `gwsim check`.
6. **(Contributors) run the extractor**, with a note about crawl etiquette and duration.
7. **Troubleshooting:** linker errors (Build Tools missing), PATH problems.

### 18.3 Candidate dependencies [Proposed]

| Need | Candidates |
| --- | --- |
| Serialisation | `serde`, `ron` |
| RNG | `rand_core` with `rand_chacha` or `rand_pcg` |
| Parallelism | `rayon` |
| Command line | `clap` |
| Errors | `thiserror` (libraries), `anyhow` (binaries) |
| HTTP (extractor) | `ureq` (blocking, simple) |
| HTML parsing (extractor) | `scraper` |
| GUI | `eframe` / `egui`, `egui_plot` |
| Tests | `proptest`, `insta`, `criterion` |
| Base64 (templates) | `base64`, or hand-rolled (the template bit layout is custom anyway) |

### 18.4 Continuous integration [Proposed]

- **When:** once the repository has a GitHub remote.
- **Checks on every push:** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `gwsim data validate`, the reduced relative checks, and the performance smoke test.
- **Releases:** tagged commits build the Windows desktop binary and attach it to GitHub Releases (Phase P6).

### 18.5 Conventions [Proposed]

- **Toolchain:** latest stable Rust. `#![forbid(unsafe_code)]` in libraries unless profiling proves `unsafe` is needed.
- **Naming:** engine concepts use wiki names (e.g. `aftercast`, `strike_level`, `armor_penetration`).
- **Assumptions:** every non-wiki constant lives in `assumptions.ron`.
- **Commits:** nothing is installed, scaffolded or committed without the owner's go-ahead.

---

## 19. Roadmap and work packages

Phases run in order unless a phase is marked parallel. Every work package (WP) lists its main requirement IDs and a done criterion.

The detailed plan of work (tasks per work package, research tasks and status tracking) is in [docs/plan/](plan/README.md).

### P0 Setup

| WP | Work | Done when |
| --- | --- | --- |
| 0.1 | README Getting Started (§18.2). | A clean machine can follow it |
| 0.2 | Install the toolchain on the dev machine (**ask first**): rustup (MSVC) and the VS C++ Build Tools. | `cargo --version` works |
| 0.3 | Workspace skeleton with the six crates; `gwsim --version`. | `cargo build` and `cargo test` pass |
| 0.4 | `.gitignore`: the Rust template plus `research/` and `.cache/`. | `git status` doesn't show `research/` |
| 0.5 | Licences: GPL-3.0-or-later for the whole project (existing `LICENSE`); `data/ATTRIBUTION.md`. | Files present |
| 0.6 | CONTRIBUTING skeleton. CI workflow ready for when a remote exists. | Files present |

### P1 Data foundations

| WP | Work | Reqs | Done when |
| --- | --- | --- | --- |
| 1.1 | Core types: professions and attributes with template indices, conditions, ranges, levels, modes. | §7.1 | `core/*.ron` loads |
| 1.2 | RON schemas, loader, provenance block, validation, cross-reference checks, `gwsim data validate`. | §8.2, §8.9 | Invalid fixtures are rejected with good errors |
| 1.3 | Template codec: skill (type 14) and equipment (type 15) encode and decode. | §14 #1 | Round-trips the PvX codes in §20 |
| 1.4 | Derived stats: attribute costs and ranks, rune rules, health, energy, armor per piece and damage type, skill value tables. | §10.4 | §17.2 tests pass |
| 1.5 | Effect DSL v0 types plus the description renderer v0. | §8.4, §8.6 | Fireball-style and conditional examples render |
| 1.6 | Assumptions register and coverage tracking; `gwsim data coverage`. | §8.7, §8.8 | Reports are produced |
| 1.7 | Data pack compilation and embedding; user data directory. | §6.4 | The binary runs with no `data/` directory present |

### P2 Extractor (in parallel with P3 once P1.2 is done)

| WP | Work | Done when |
| --- | --- | --- |
| 2.1 | **Spike:** fetch about 10 representative pages (skills, foes, an area, a game-integration list) politely and confirm the rendered HTML exposes every needed field. Decide how skill IDs and scaled values are found. | Findings written up; parsing strategy fixed |
| 2.2 | Polite HTTP client: `robots.txt`, delays, back-off, stop rules, User-Agent, cache. | EXT-1 to EXT-7 tests |
| 2.3 | Discovery through allowed list pages. | The full skill list and M1 foe list are resolved |
| 2.4 | Skill page parser and normaliser. | M1's 72 skills parse correctly (checked by hand) |
| 2.5 | Foe and area parsers. | The 8 Kournan foes and the Vehtendi Valley roster parse |
| 2.6 | `seed` (no overwrite). | M1 skill and foe files seeded as `NumbersOnly` / `Draft` |
| 2.7 | `diff` and the change report. | Re-running straight after seeding reports no changes |

### P3 Engine core → **M0** (the player's Energy Surge bar vs training dummies) [Default D31]

| WP | Work | Reqs |
| --- | --- | --- |
| 3.1 | Time model and event queue (spike: 50 ms tick vs pure events) | §10.2 |
| 3.2 | Space: positions, range bands, movement, body-blocking, projectiles | §10.3 |
| 3.3 | Units and derived stats at start | §10.4 |
| 3.4 | Skill use pipeline: validity, range approach, activation, costs, complete / interrupt / cancel / fail / fizzle, aftercast, recharge, upkeep | ENG-10 to ENG-18 |
| 3.5 | Damage, armor calculation, hit locations, crits, healing, regeneration, natural regeneration | §10.7 |
| 3.6 | Effects framework: durations, stacking keys, one-at-a-time rules, caps, triggers, removal | ENG-30 to ENG-35 |
| 3.7 | RNG streams and CRN plumbing | ENG-40 to ENG-43 |
| 3.8 | Run harness: many seeded runs, aggregation, confidence intervals, stop-when-stable rule | D5 |
| 3.9 | Combat log | §14 #5 |
| 3.10 | Training dummies (with energy); encode the 8 M0 skills (the player bar) as `Draft`; `PlanAi` v0 | §12.2, §11.5 |

#### M0 done when

- The player bar runs against dummies deterministically.
- Energy Surge, Mistrust, Unnatural Signet, Cry of Frustration, Spiritual Pain and Power Drain values match hand calculations from the wiki formulas.
- Arcane Echo and Air of Superiority behave as described.
- Formula tests pass.

### P4 **M1**: 7 Hero Mesmerway vs Kournans (evaluator, command line)

| WP | Work | Reqs |
| --- | --- | --- |
| 4.1 | Encode the remaining M1 skills in batches by profession (Mesmer, Ritualist, Necromancer, Paragon, Monk, PvE-only, then the Kournan foe skills). Handlers for the one-of-a-kind list. Review. | §8.3 to §8.5, §17.5 |
| 4.2 | Kournan foe data: armor tables (confirm the level context), HM attributes (A-005), weapons (A-004), AI tags. | §7.1, §21 |
| 4.3 | Mechanics M1 needs (checklist in §20.5). | §10 |
| 4.4 | `FoeAi` (NM/HM) | §11.3 |
| 4.5 | `HeroAi` including the 2026 rules; `MinionAi`, `SpiritAi` | §11.4 |
| 4.6 | `PlanAi` generation for the player | §11.5 |
| 4.7 | Tactics-plan generator and overrides | §11.6 |
| 4.8 | Encounters, situations, chains: the Kournan patrol HM, the generic scenarios (D25), the two-fight chain (D26) | §12 |
| 4.9 | Command line: `evaluate`, `compare`, `log`, `template`, `data *`; reports 1, 2, 4, 5 and 7 | §14, §15 |
| 4.10 | Relative checks RC1, RC2 and RC6 | §17.4 |
| 4.11 | Performance suite and optimisation to the target | §17.6 |

#### M1 done when

1. `gwsim evaluate` runs the M1 party against every M1 situation, deterministically.
2. All M1 skills are `Reviewed`.
3. The generated descriptions are reviewed against the wiki.
4. RC1, RC2 and RC6 pass.
5. Performance meets D19.
6. Reports show assumptions, draft warnings and the "uncalibrated" label.
7. The §17.2 tests pass.

### P5 Optimiser → **M2**

| WP | Work | Reqs |
| --- | --- | --- |
| 5.1 | Genome, constraints, repair operators | OPT-1 to OPT-5 |
| 5.2 | NSGA-II with constrained domination; seeding | §13.3 |
| 5.3 | Adaptive evaluation, CRN, cache, parallelism | §13.4 |
| 5.4 | Role tags (automatic plus manual); narrowing; heuristic seed builds | §13.5 |
| 5.5 | Exhaustive mode | §13.3 |
| 5.6 | Situation sets and weights; objectives menu; frontier output | §13.6 |
| 5.7 | Account profile constraints; `profile` commands | OPT-2 |
| 5.8 | `optimise` command; report 3 as JSON | §15 |
| 5.9 | O1: best player build for Mesmerway; RC4 and RC5 | §13.7 |
| 5.10 | O2: the Solo Resto benchmark (encode the extra Ineptitude and Splinter Support skills) | §13.7 |

**M2 done when:** O1 runs within the default time budget, RC4 and RC5 pass, and the frontier and ranked output are correct and reproducible.

### P6 Desktop app → **M3** (after P5 [Decided Q43])

| WP | Work |
| --- | --- |
| 6.1 | App shell, data loading, user data directory |
| 6.2 | Party and build editor (template import and export) |
| 6.3 | Tactics editor |
| 6.4 | Situation, encounter, chain and set editors |
| 6.5 | Account profile editor |
| 6.6 | Run panel with progress and live frontier |
| 6.7 | Results views: ranked list, trade-off chart, contributions, energy timeline |
| 6.8 | Replay viewer |
| 6.9 | Comparison view |
| 6.10 | Packaging as a single .exe; GitHub Releases pipeline |

**M3 done when:** every use case UC1–UC12 that the data coverage allows can be done in the app, and a release build installs and runs on a clean Windows 11 machine.

### P7 Coverage (ongoing; starts after M1, in parallel with P5 and P6) [Decided Q31, Q36]

- **Order:** Mesmer → Ritualist → Necromancer → **PvE-only** (including the title tables) → Paragon → Monk → Elementalist → Warrior → Ranger → Dervish → Assassin.
- **Per profession:**
  1. seed any missing numbers;
  2. encode in batches;
  3. write the handlers;
  4. add the AI hints;
  5. review;
  6. finish when the coverage report shows 100% `Reviewed` for that profession.
- **Alongside:**
  - monster skills and new curated encounters, as they are needed;
  - more generic archetypes;
  - henchman data (which enables RC3);
  - a maintained benchmark list.

### P8 Weapon-set swapping [Decided Q20, Q41]

- Engine support for several weapon sets and swap actions.
- `PlanAi` swap rules: energy sets, casting sets.
- An optimiser genome with multiple sets.
- Re-run the M1 player benchmark with the page's three weapon sets.

### Backlog (not planned; §23)

Calibration against in-game runs (optional), scripted mid-fight actions, terrain, PvP, historical balance.

### Dependency summary

```text
P0 → P1 → P3 → M0 → P4 → M1 → P5 → M2 → P6 → M3
          ↘ P2 (parallel with P3; needed for P4.1/4.2 seeding)
                         M1 → P7 (ongoing) ; M2/M3 → P8
```

---

## 20. First milestone (M1) content inventory

### 20.1 Party [Decided Q30, Q32, Q33, Q40, Q41; Default D23]

PvX sources:

- The team: [Build:Team - 7 Hero Mesmerway](https://gwpvx.fandom.com/wiki/Build:Team_-_7_Hero_Mesmerway), read from the [Wayback snapshot of 2026-07-27](https://web.archive.org/web/20260727151256/https://gwpvx.fandom.com/wiki/Build:Team_-_7_Hero_Mesmerway). Page revision 2026-07-23; rated Great / Meta.
- The player: [Build:Me/any PvE Energy Surge](https://gwpvx.fandom.com/wiki/Build:Me/any_PvE_Energy_Surge), read from the [Wayback snapshot of 2025-08-03](https://web.archive.org/web/20250803220419/https://gwpvx.fandom.com/wiki/Build:Me/any_PvE_Energy_Surge). Rated Great / Meta.

**Current wiki values are used for every skill**, not the snapshots' tooltips. Both pages predate some 2026 changes (§20.4).

| Slot | Kind | Build | Attributes (points + rune + headgear) | Equipment | PvX template (regular) |
| --- | --- | --- | --- | --- | --- |
| Player | Human | **Me/—** Arcane Echo, **Energy Surge**, Mistrust, Unnatural Signet, Cry of Frustration, Spiritual Pain, Power Drain, Air of Superiority | Domination 12+1+3, Fast Casting 10+1, Inspiration 8+1 | 5× Prodigy's insignia; 40/40 Domination set; runes inferred as superior Domination, minor Fast Casting, minor Inspiration (A-033) | `OQBTAUBPQaJ4EY6x0BAAAAAAuE` (optional slots empty on PvX) |
| Hero 1 | Hero (Mesmer) | **Dom Mesmer:** **Panic**, Cry of Frustration, Mistrust, Unnatural Signet, Shatter Hex, Spiritual Pain, Power Drain, Drain Enchantment | Domination 12+1+3, Fast Casting 11+2, Inspiration 6+1 | 5× Prodigy's; Superior Vigor, Vitae; 40/40 Domination set | `OQBTAWBPsBAkDmemuhAONDAAA` (encodes one choice of the alternatives; the benchmark file records the exact bar) |
| Hero 2 | Hero (Mesmer) | **Dom Mesmer (Me/Rt):** **Energy Surge**, Cry of Frustration, Mistrust, Unnatural Signet, Shatter Hex, Spiritual Pain, Power Drain, **Flesh of My Flesh** [Proposed pick] | as Hero 1 | as Hero 1 | as Hero 1 |
| Hero 3 | Hero (Mesmer) | **Dom Mesmer (Me/Mo):** **Energy Surge**, …, **Resurrection Chant** [Proposed pick] | as Hero 1 | as Hero 1 | as Hero 1 |
| Hero 4 | Hero (Necromancer) | **Minion Master N/P:** **"Incoming!"**, "Fall Back!", "Stand Your Ground!", Signet of Lost Souls, Animate Bone Fiend, Putrid Bile, Masochism, **Withering Aura** [Proposed pick over Blood of the Master: no Blood Magic points; Dark Aura is only for a Soul Taker player] | Death 12+1+3, Soul Reaping 9+2, Command 9 | Bloodstained + 4× Minion Master's insignia; Superior Vigor, Vitae | `OAljUwGpZS8Y7Y1YVVUBKgbhAAA` |
| Hero 5 | Hero (Necromancer) | **Blood is Power N/Rt:** **Blood is Power**, Blood Bond, Spirit Transfer, Signet of Lost Souls, Mend Body and Soul, Spirit Light, Protective Was Kaolai, **Recuperation** (Dual Resto) | Restoration 12, Blood 9+1+3, Soul Reaping 9+2 | 5× Tormentor's; Superior Vigor, Vitae | `OAhjQkGZIP3hhmwrqKNncDzqH` |
| Hero 6 | Hero (Ritualist) | **Signet of Spirits Rt:** **Signet of Spirits**, Ancestors' Rage, Spirit Siphon, **Splinter Weapon** [Proposed pick over Lamentation: Splinter Weapon's hero AI was fixed on 2026-08-26], Mend Body and Soul, Spirit Light, Protective Was Kaolai, Life | Channeling 12+1+3, Restoration 12+2, Spawning Power 3+1 | 5× Shaman's; Superior Vigor, Vitae | `OACjEyiM5MXTvJzEAINncDzxJ` |
| Hero 7 | Hero (Ritualist) | **Soul Twisting Rt/Mo:** **Soul Twisting**, Shelter, Union, Displacement, Armor of Unfeeling, Boon of Creation, Signet of Creation, **Remove Hex** [Proposed pick: Strength of Honor is for melee players, and the player is a caster] | Communing 12+1+3, Spawning Power 12+3 | 5× Shaman's; Superior Vigor, Vitae | `OACiAyk8gNtePuwJ00ZaNBAA` |

- **Tactics notes from PvX:** backline on Guard, midline on Fight or Guard; pre-cast Shelter → Union → Displacement → Armor of Unfeeling before hard fights; flag heroes apart against AoE. The tactics generator must produce these (§11.6).
- **Heroes:** three Mesmer heroes are possible because of Ghost of Althea (Reforged Mode). Identity doesn't affect the simulation (D17).

### 20.2 Kournan patrol (Vehtendi Valley, Nightfall) [Decided Q35; Default D24]

- NM level 20, **HM level 26**, one of each foe type, no boss, no environment effects. Area: [Vehtendi Valley](https://wiki.guildwars.com/wiki/Vehtendi_Valley). The group composition is hand-authored (A-006).
- The skills and ranks below are from the wiki as read during design; they are re-verified by the extractor.

| Foe | Profession | Skills | Attribute ranks (wiki) | Armor (wiki table) |
| --- | --- | --- | --- | --- |
| [Kournan Guard](https://wiki.guildwars.com/wiki/Kournan_Guard) | Warrior (axe variant, A-007) | Disrupting Chop, Executioner's Strike, **Magehunter Strike** (elite), Sprint | Strength 14 | 116 / 96 |
| [Kournan Zealot](https://wiki.guildwars.com/wiki/Kournan_Zealot) | Dervish | Armor of Sanctity, Eremite's Attack, **Pious Renewal** (elite), Veil of Thorns | not given (A-005) | 70 |
| [Kournan Phalanx](https://wiki.guildwars.com/wiki/Kournan_Phalanx) | Paragon | "Never Surrender!", "Stand Your Ground!", **Cautery Signet** (elite), Mighty Throw, Wild Throw | not given (A-005) | 95 |
| [Kournan Bowman](https://wiki.guildwars.com/wiki/Kournan_Bowman) | Ranger | Crossfire, **Infuriating Heat** (elite, nature ritual), Precision Shot, Troll Unguent, Whirling Defense | Expertise 14 (HM 16) | 70 / 100 |
| [Kournan Scribe](https://wiki.guildwars.com/wiki/Kournan_Scribe) | Elementalist | Aftershock, Aura of Restoration, Fireball, **Master of Magic** (elite), Meteor | Fire 15 | 60 |
| [Kournan Seer](https://wiki.guildwars.com/wiki/Kournan_Seer) | Mesmer | Drain Enchantment, **Enchanter's Conundrum** (elite), Power Spike, Shatter Enchantment | Domination 15, Inspiration 14 (HM Domination 20) | 60 |
| [Kournan Oppressor](https://wiki.guildwars.com/wiki/Kournan_Oppressor) | Necromancer | **Blood is Power** (elite), Life Siphon, Signet of Lost Souls, Strip Enchantment | Blood 15 (HM 20) | 60 |
| [Kournan Priest](https://wiki.guildwars.com/wiki/Kournan_Priest) | Monk | Convert Hexes, Reversal of Fortune, Shielding Hands, **Zealous Benediction** (elite) | Divine Favor 15, Protection 15 | 60 |

- **Armor to verify (WP4.2):** "a / b" means two values from the wiki's armor table (physical and elemental differences). Confirm which damage types and which level the table refers to.
- There are no monster-only skills. The group gives little hex or condition pressure on the party (a known limitation of this encounter).

### 20.3 Skill inventory

| Group | Skills | Count |
| --- | --- | --- |
| Mesmer (party) | Panic, Energy Surge, Cry of Frustration, Mistrust, Unnatural Signet, Shatter Hex, Spiritual Pain, Power Drain, Drain Enchantment, Arcane Echo | 10 |
| Ritualist (party) | Flesh of My Flesh, Spirit Transfer, Mend Body and Soul, Spirit Light, Protective Was Kaolai, Recuperation, Signet of Spirits, Ancestors' Rage, Spirit Siphon, Splinter Weapon, Life, Soul Twisting, Shelter, Union, Displacement, Armor of Unfeeling, Boon of Creation, Signet of Creation | 18 |
| Necromancer (party) | Blood is Power, Blood Bond, Signet of Lost Souls, Animate Bone Fiend, Putrid Bile, Masochism, Withering Aura | 7 |
| Paragon (party) | "Incoming!", "Fall Back!", "Stand Your Ground!" | 3 |
| Monk (party) | Resurrection Chant, Remove Hex | 2 |
| PvE-only (party) | Air of Superiority (Asura title) | 1 |
| Kournan skills not already listed | Disrupting Chop, Executioner's Strike, Magehunter Strike, Sprint, Armor of Sanctity, Eremite's Attack, Pious Renewal, Veil of Thorns, "Never Surrender!", Cautery Signet, Mighty Throw, Wild Throw, Crossfire, Infuriating Heat, Precision Shot, Troll Unguent, Whirling Defense, Aftershock, Aura of Restoration, Fireball, Master of Magic, Meteor, Enchanter's Conundrum, Power Spike, Shatter Enchantment, Life Siphon, Strip Enchantment, Convert Hexes, Reversal of Fortune, Shielding Hands, Zealous Benediction | 31 |
| **Total** | | **72** |

- **Complexity** (research-time judgement, to confirm while encoding):
  - **Formulaic:** Energy Surge, Blood is Power, Recuperation, Signet of Creation, Ancestors' Rage, Remove Hex.
  - **One-of-a-kind (handlers):** Panic, Mistrust, Blood Bond, Putrid Bile, Animate Bone Fiend, Soul Twisting, Shelter, Union, Displacement, Life, Protective Was Kaolai, Splinter Weapon, Flesh of My Flesh, Resurrection Chant, Arcane Echo, Air of Superiority (random Asura benefit on kills that give XP).
  - **Conditional:** everything else in the party. The Kournan skills haven't been classified yet.

### 20.4 2026 balance changes touching M1 (from the wiki's update notes)

| Date | Change | Effect on the build |
| --- | --- | --- |
| [2026-06-24](https://wiki.guildwars.com/wiki/Feedback:Game_updates/20260624) | Cry of Frustration: recharge 15 → 20; AoE damage 100% → 75% | Weaker |
| 2026-06-24 | Energy Surge: damage per energy 9 → 7; AoE damage 75% (the target takes full damage) | Weaker |
| 2026-06-24 | Mistrust (PvE): 10…82…100 → 10…66…80; AoE 75%. The wiki notes the target also takes only 75%. | Weaker |
| 2026-06-24 | Animate Bone Fiend: energy 25 → 15, activation 3 s → 1 s. Protective Was Kaolai: recharge 25 → 20. Spirit Siphon energy gain changed. | Stronger |
| 2026-06-24 | Hero AI: AoE tolerance down to 65% health; Judge's Insight and "Find Their Weakness!" only on martial-weapon users | AI rules |
| [2026-08-26](https://wiki.guildwars.com/wiki/Feedback:Game_updates/20260826) | Soul Twisting minimum ritual cost 10 → 5; Protective Was Kaolai armor and healing increased; Putrid Bile duration changed; Splinter Weapon hero AI fixed | Stronger |

### 20.5 Mechanics checklist for M1 (WP4.3)

- **Casting:** Fast Casting (activation and PvE Mesmer recharge); interrupts that distinguish "using a skill" from "casting a spell"; forced spell failure; Dazed.
- **Hexes and enchantments:** application and removal (Shatter Hex, Remove Hex, Convert Hexes, Drain Enchantment, Shatter Enchantment, Strip Enchantment); maintained effects.
- **Energy:** gain, loss and drain; energy regeneration pips; Soul Reaping; health sacrifice; batteries.
- **Area effects:** range bands and area damage with 75% to secondary targets; the summoned-creature tag.
- **Spirits:** binding-ritual spirits with party auras (damage cap, damage reduction, redistribution, block); spirit health scaling with Spawning Power; one spirit per type; the foe nature ritual (Infuriating Heat), which replaces allied and enemy spirits of the same type.
- **Minions and corpses:** Bone Fiends from corpses; minion degeneration; the control cap.
- **Weapon spells:** one per target (Splinter Weapon).
- **Resurrection:** Resurrection Chant, Flesh of My Flesh; Life.
- **Shouts and chants:** "Incoming!", "Fall Back!", "Stand Your Ground!", "Never Surrender!"; speed boosts.
- **Conditions and movement:** conditions (from foe skills), movement state, snares and speed modifiers.
- **Knockdown:** Meteor, Aftershock [confirm], and others from foe skills.
- **Attacks:**
  - melee and ranged weapons, attack skills, adrenaline (Disrupting Chop, Executioner's Strike, Magehunter Strike, Mighty Throw, Wild Throw, Precision Shot, Crossfire, Eremite's Attack);
  - projectiles;
  - block (Whirling Defense, Shielding Hands [confirm]);
  - damage reduction and damage conversion on foes (Reversal of Fortune, Shielding Hands).
- **Gear:** runes, insignias, inscriptions.
- **Party AI:** hero modes and flags; turning hero skills off; the 2026 AI rules.
- **Energy Surge:** energy-based damage (damage scales with energy lost).
- **Air of Superiority:** on-kill triggers with random outcomes.

### 20.6 M1 situations [Default D25, D26]

| Situation | Content |
| --- | --- |
| `dummies-hm` | Stationary dummies at level 26 with energy (M0 and M1 smoke test) |
| `kournan-melee-heavy-hm` | Guard ×2, Zealot ×2, Phalanx ×2, Priest ×1, Bowman ×1 |
| `kournan-caster-heavy-hm` | Scribe ×2, Seer ×2, Oppressor ×2, Priest ×1, Guard ×1 |
| `kournan-healer-heavy-hm` | Priest ×3, Guard, Zealot, Bowman, Seer, Scribe |
| `kournan-patrol-hm` | The curated encounter (§20.2) |
| `kournan-patrol-hm-chain2` | Two patrols with a rest between (default A-028) |

- The generic compositions above are [Proposed] and can be tuned during M1.
- M1 situation set: all of the above with equal weights [Proposed].

---

## 21. Assumptions register (initial)

Each entry becomes a record in `data/assumptions.ron`. The status of each starts as `Assumed`.

| ID | Assumption | Initial handling | Why |
| --- | --- | --- | --- |
| A-001 | Base movement speed of characters and foes | Find the value on the wiki during WP3.2; otherwise a documented community value, flagged | Not captured in the research |
| A-002 | Collision radius of units (for body-blocking) | Single default radius; larger for big creatures later | Not documented |
| A-003 | Projectile travel speeds per weapon and spell type | From wiki weapon pages if present; otherwise defaults per category | Not documented per item |
| A-004 | Foe weapon damage and attack interval | Player weapon-type tables at the foe's level and strike rules | Not recorded on creature pages |
| A-005 | Missing HM attribute ranks | Use given HM ranks; otherwise NM rank + 5, capped at 20 (matching Seer and Oppressor +5; Bowman shows +2), flagged per foe | The wiki gives HM ranks for few foes |
| A-006 | Kournan patrol composition | One of each of 8 types | No composition on the wiki |
| A-007 | Kournan Guard weapon | Axe variant | The wiki lists axe and hammer variants |
| A-008 | Encounter start geometry | Foes start in a cluster about 1,800 gwinches ahead; the party walks in until aggro | No positions on the wiki |
| A-009 | Aggro range | Documented community value, flagged; groups aggro as one | The wiki describes the aggro bubble without a number |
| A-010 | Foe reaction delay (NM / HM) | NM default; HM shorter | Not documented |
| A-011 | Hero reaction delay | 0 for interrupts ("never late"); a small default for other decisions | Partly documented |
| A-012 | Human plan reaction delay | A small default (e.g. 250 ms), configurable | Not a game fact |
| A-013 | Hero identity | Irrelevant beyond profession and availability (D17) | Simplification |
| A-014 | Hit-location distribution for foe attacks | The wiki's normal/ranged column | Size classes not modelled |
| A-015 | Spirit health and armor | The wiki's unofficial values; friendly spirit armor default | Unofficial or unknown |
| A-016 | Foe scatter thresholds and timing | Scatter on damage-over-time AoE after a short delay; shorter in HM | Qualitative only |
| A-017 | Fast Casting on signets | 0.5^(rank/15) per the Fast Casting page (conflicts with −3%/rank on another page) | Wiki conflict |
| A-018 | Nature ritual range | 3000 (since 2026-08-26) | The Range page is out of date |
| A-019 | Consumables surviving death | Per the consumable item pages (conflicts with the Death page) | Wiki conflict |
| A-020 | Title effective ranks | From the individual rank pages | The old formula is outdated |
| A-021 | Bleeding ±1 s prefix/inscription rule | Not applied | Only on one page |
| A-022 | Order of damage modifiers | The wiki's list, as written | The wiki says it has errors |
| A-023 | Armor penetration rounding | Floor after multiplying (may be 1 lower) | Unknown exact mechanics |
| A-024 | Adjacent size | 166 gwinches | Pages disagree on the ratio |
| A-025 | AoE of 240 vs 252 | A per-skill field | Varies by skill |
| A-026 | Hero AI after 2026 | Update notes take precedence over the outdated Hero behavior page | Page flagged outdated |
| A-027 | Reforged Mode bug (foes with negative armor gain armor) | Not modelled until pre-Searing content is added | Irrelevant to M1 |
| A-028 | Chain rest time | 20 s default, configurable per chain | Not a game fact |
| A-029 | Human play quality | Plan executed exactly as written, with A-012 delay | Modelling choice |
| A-030 | Fight timeout | 180 s, counted as a loss | Modelling choice |
| A-031 | Armor bug with net bonus ≥ 26 (reductions ignored) | Modelled as documented | Documented bug |
| A-032 | HM recharge reduction for foes | Amount to be found on the wiki; otherwise a default, flagged | "Shorter recharges" isn't quantified in the research |
| A-033 | Player rune choice | Superior Domination, minor Fast Casting, minor Inspiration (inferred from the attribute numbers) | PvX page doesn't name the runes |

---

## 22. Risks and mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| **Encoding volume** (~1,400 skills; ~320 need handlers) | Very long timeline | DSL-first; handlers only where needed; batches; coverage-driven order; generated tests; clear contributor guide |
| **Wiki errors** (133 skill pages flagged `{{sic}}`; mechanics that appear only in Notes) | Wrong behaviour | Owner review against the whole page; assumptions register; relative checks |
| **Undocumented mechanics** (foe weapons, AI timing, aggro ranges) | Uncalibrated absolute numbers | Labelled assumptions; "uncalibrated" label; relative checks only rely on comparisons |
| **Hero AI fidelity** after the 2026 changes | Hero builds misjudged | Encode the documented quirks; per-skill AI hints; flag A-026; revisit as the wiki updates |
| **Performance** (Rust helps, but many effects) | Slow optimiser | Performance budget in CI; data-oriented engine; adaptive evaluation; CRN |
| **Extractor fragility** (HTML layout changes; category pagination is disallowed) | Seeding and diff break | WP2.1 spike; synthetic-fixture tests; discovery via list pages; the diff tolerates missing fields |
| **Crawl etiquette** | Blocked by the wiki | EXT-1 to EXT-7; stop on 403/429; long delays; resumable cache |
| **Licensing** | Takedown or contamination | Bundle numbers and our own encodings only; never commit the cache, prose or icons; one GPL-3.0-or-later licence throughout [D33]; *not legal advice* |
| **PvX access blocked** | Benchmarks stale | Frozen benchmarks with URL and date; Wayback; manual entry |
| **Balance patches** | Stale data | Change report workflow (UC11); stale data is acceptable (C4) |
| **Rust barrier for contributors** | Fewer contributors | Data contributions need only RON; docs and validation with good errors |
| **Scope creep** | Milestones slip | Milestone acceptance criteria; the out-of-scope list (§23) |

---

## 23. Out of scope

- **PvP**, including PvP skill versions [Q2, D3].
- **Terrain,** obstacles, pathfinding, line of sight and height advantage [Q8].
- **Live wiki import** at run time [Q5].
- **Historical balance** versions [Q5 discussion].
- **Bundling ArenaNet content:** skill descriptions, icons, art [Q25].
- **Scripted mid-fight actions:** timed orders, moving flags, split pulls [Q29, D14].
- **Optimising consumables** (they are switches only) [Q21].
- **Calibration gates** against in-game runs. Measurements remain optional input [Q34, D22].
- **Telemetry;** non-English UIs; official non-Windows builds [D14].
- **A separate web UI** [D4].
- **Committing `research/`** [Q42].

---

## 24. References

### Guild Wars Wiki: core mechanics

- [Skill](https://wiki.guildwars.com/wiki/Skill) · [Template:Gr](https://wiki.guildwars.com/wiki/Template:Gr) · [Template:Skill infobox](https://wiki.guildwars.com/wiki/Template:Skill_infobox) · [Skill type](https://wiki.guildwars.com/wiki/Skill_type) · [Activation time](https://wiki.guildwars.com/wiki/Activation_time) · [Aftercast delay](https://wiki.guildwars.com/wiki/Aftercast_delay) · [Recharge time](https://wiki.guildwars.com/wiki/Recharge_time) · [Resource cost](https://wiki.guildwars.com/wiki/Resource_cost) · [Interrupt](https://wiki.guildwars.com/wiki/Interrupt)
- [Damage calculation](https://wiki.guildwars.com/wiki/Damage_calculation) · [Armor calculation](https://wiki.guildwars.com/wiki/Armor_calculation) · [Order of damage modifiers](https://wiki.guildwars.com/wiki/Order_of_damage_modifiers) · [Critical hit](https://wiki.guildwars.com/wiki/Critical_hit) · [Damage type](https://wiki.guildwars.com/wiki/Damage_type)
- [Effect stacking](https://wiki.guildwars.com/wiki/Effect_stacking) · [Condition](https://wiki.guildwars.com/wiki/Condition) · [Health](https://wiki.guildwars.com/wiki/Health) · [Health regeneration](https://wiki.guildwars.com/wiki/Health_regeneration) · [Energy](https://wiki.guildwars.com/wiki/Energy) · [Death](https://wiki.guildwars.com/wiki/Death)
- [Range](https://wiki.guildwars.com/wiki/Range) · [Area of effect](https://wiki.guildwars.com/wiki/Area_of_effect) · [Scatter](https://wiki.guildwars.com/wiki/Scatter) · [Movement](https://wiki.guildwars.com/wiki/Movement) · [Projectile](https://wiki.guildwars.com/wiki/Projectile) · [Body block](https://wiki.guildwars.com/wiki/Body_block) · [Aggro](https://wiki.guildwars.com/wiki/Aggro)
- [Attribute](https://wiki.guildwars.com/wiki/Attribute) · [Attribute point](https://wiki.guildwars.com/wiki/Attribute_point) · [Fast Casting](https://wiki.guildwars.com/wiki/Fast_Casting) · [Rune](https://wiki.guildwars.com/wiki/Rune) · [Insignia](https://wiki.guildwars.com/wiki/Insignia) · [Inscription](https://wiki.guildwars.com/wiki/Inscription) · [Weapon](https://wiki.guildwars.com/wiki/Weapon) · [Consumable](https://wiki.guildwars.com/wiki/Consumable)
- [Creature](https://wiki.guildwars.com/wiki/Creature) · [Foe](https://wiki.guildwars.com/wiki/Foe) · [Minion](https://wiki.guildwars.com/wiki/Minion) · [Spirit](https://wiki.guildwars.com/wiki/Spirit) · [Monster skill](https://wiki.guildwars.com/wiki/Monster_skill) · [Level](https://wiki.guildwars.com/wiki/Level)
- [Hero](https://wiki.guildwars.com/wiki/Hero) · [Hero behavior](https://wiki.guildwars.com/wiki/Hero_behavior) · [Hero-vetted skills](https://wiki.guildwars.com/wiki/Category:Hero-vetted_skills) · [Title](https://wiki.guildwars.com/wiki/Title)
- [Hard mode](https://wiki.guildwars.com/wiki/Hard_mode) · [Reforged Mode](https://wiki.guildwars.com/wiki/Reforged_Mode) · [Optional game modes](https://wiki.guildwars.com/wiki/Optional_game_modes)
- [Skill template format](https://wiki.guildwars.com/wiki/Skill_template_format) · [Equipment template format](https://wiki.guildwars.com/wiki/Equipment_template_format) · [Game integration/Skills/1-500](https://wiki.guildwars.com/wiki/Guild_Wars_Wiki:Game_integration/Skills/1-500)

### Guild Wars Wiki: update notes (2026)

- [2026-02-05](https://wiki.guildwars.com/wiki/Feedback:Game_updates/20260205) · [2026-04-28](https://wiki.guildwars.com/wiki/Feedback:Game_updates/20260428) · [2026-06-24](https://wiki.guildwars.com/wiki/Feedback:Game_updates/20260624) · [2026-08-26](https://wiki.guildwars.com/wiki/Feedback:Game_updates/20260826)

### Guild Wars Wiki: policy and licensing

- [robots.txt](https://wiki.guildwars.com/robots.txt) · [GWW:Copyrights](https://wiki.guildwars.com/wiki/Guild_Wars_Wiki:Copyrights) · [GWW:Copyrighted content](https://wiki.guildwars.com/wiki/Guild_Wars_Wiki:Copyrighted_content) · [Template:ArenaNet image](https://wiki.guildwars.com/wiki/Template:ArenaNet_image) · [GWW:Bots](https://wiki.guildwars.com/wiki/Guild_Wars_Wiki:Bots)

### Encounter (M1)

- [Vehtendi Valley](https://wiki.guildwars.com/wiki/Vehtendi_Valley) · [Kournan military](https://wiki.guildwars.com/wiki/Kournan_military) · foe pages linked in §20.2

### PvXwiki benchmarks (read via the Wayback Machine)

- [Build:Team - 7 Hero Mesmerway](https://web.archive.org/web/20260727151256/https://gwpvx.fandom.com/wiki/Build:Team_-_7_Hero_Mesmerway) (snapshot 2026-07-27)
- [Build:Me/any PvE Energy Surge](https://web.archive.org/web/20250803220419/https://gwpvx.fandom.com/wiki/Build:Me/any_PvE_Energy_Surge) (snapshot 2025-08-03)
