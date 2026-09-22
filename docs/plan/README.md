# gwsim plan of work

This plan breaks the roadmap in [DESIGN.md §19](../DESIGN.md#19-roadmap-and-work-packages) down into phases, work packages and tasks.

- **DESIGN.md** remains the source of truth for requirements and decisions. This plan covers how the work gets done and in what order. If the two disagree, DESIGN.md wins and the plan is corrected.
- **Status:** draft plan, 2026-09-22. No task has started.
- **Where to start:** P0 Setup, beginning with WP0.4 (git-ignore `research/` before anything is committed).

## Phases

| Phase | File | Goal | Ends with |
| --- | --- | --- | --- |
| P0 | [P0-setup.md](P0-setup.md) | Give the repo a Rust workspace that builds and tests, with the toolchain, licences and git hygiene in place. | Workspace builds |
| P1 | [P1-data-foundations.md](P1-data-foundations.md) | Let `gwsim-data` represent, load, validate, describe and package all static game data, with verified core values and wiki-exact formulas. | Stable data format |
| P2 | [P2-extractor.md](P2-extractor.md) | Build a polite, resumable wiki extractor that seeds data files and reports changes without ever overwriting data. | M1 files seeded |
| P3 | [P3-engine-core.md](P3-engine-core.md) | Build a deterministic, fast combat core that runs the player's Energy Surge bar against training dummies. | **M0** |
| P4 | [P4-m1-mesmerway.md](P4-m1-mesmerway.md) | Evaluate 7 Hero Mesmerway plus the player against the Kournan situations from the command line, with every skill reviewed. | **M1** |
| P5 | [P5-optimiser.md](P5-optimiser.md) | Search build space with NSGA-II and exhaustive modes, and prove it on O1 and O2. | **M2** |
| P6 | [P6-desktop-app.md](P6-desktop-app.md) | Ship the egui desktop app as a single Windows executable covering UC1–UC12. | **M3** |
| P7 | [P7-coverage.md](P7-coverage.md) | Bring every player skill to `Reviewed`, profession by profession, with supporting foes, encounters and henchmen. | Ongoing |
| P8 | [P8-weapon-swapping.md](P8-weapon-swapping.md) | Model weapon-set swapping in the engine, the player AI and the optimiser. | Swap benchmark |

```text
P0 → P1 → P3 → M0 → P4 → M1 → P5 → M2 → P6 → M3
      └─(WP1.2 done)─► P2 (parallel with P3; its seeding feeds P3.10 and P4.1/4.2)
                              M1 → P7 (ongoing, parallel with P5/P6) ; M2/M3 → P8
```

Cross-phase dependencies at task level:

| From | To | Why |
| --- | --- | --- |
| T1.2.3 (skill schema) | P2 (all) | The extractor writes the data format defined in `gwsim-data`. |
| T2.3.6 (skill index) | T1.6.4 (coverage percentages) | The index gives coverage its denominators. |
| T2.6.5 (M1 seeding) | T3.10.3, WP4.1, WP4.2 | Seeded numbers are the starting point for encoding. If P2 is late, T3.10.3 hand-enters the 8 M0 skills. |
| WP3.6 (DSL interpreter) | WP4.1 | Encoded skills need an interpreter to run. |
| WP4.1 role tags (manual) | T4.10.3 (RC1 off-role swaps) | RC1 needs role tags before P5.4 automates them. |
| T5.2.5 (anytime frontier) | WP6.6 (live frontier) | The desktop app reads the frontier while a search runs. |

## How the plan is organised

- **Hierarchy.** Each phase has a goal and exit criteria. Each work package (WP) has a goal, design references, dependencies and a done criterion taken from DESIGN.md §19. Each task has a type and dependencies, followed by the exact actions, the output and its own done criterion.
- **IDs.** Phase `P3`, work package `WP3.4`, task `T3.4.2`. WP numbers match DESIGN.md §19. P7 and P8 have no WP numbers in the design, so this plan assigns them.
- **References.** "§8.4" means section 8.4 of [DESIGN.md](../DESIGN.md). Requirement IDs (`ENG-12`, `EXT-3`, `OPT-1`, `AI-H7`, …), decisions (`Q27`, `D19`) and assumptions (`A-005`) are defined there.
- **Status tracking.** Each work package has a task table with a Status column: `Todo`, `In progress`, `Blocked` or `Done`. Each phase file has the same column for its work packages. Update these as work proceeds.

### Task fields

| Field | Meaning |
| --- | --- |
| **Type** | One of the task types below. |
| **Depends on** | Tasks (or whole WPs) that must be done first. `—` means none beyond the WP's own dependencies. |
| **Owner** | Present only when the project owner must act: approve something before it starts (installs, crawls, commits), make a decision, or review. |
| Actions | Numbered steps: what to build, run, write or check. |
| **Output** | The files, code or records the task produces. |
| **Done when** | The check that closes the task. |

### Task types

| Type | Meaning | Typical output |
| --- | --- | --- |
| Research | Establish facts before building: wiki mechanics, library capabilities, platform details. | A findings file (see below), assumption updates, DESIGN.md corrections |
| Decision | Choose between options. Marked **Owner** where the owner decides. | A line in DESIGN.md §3 or in the findings file |
| Build | Write code, with its unit tests. | Code and tests |
| Data | Write or edit files under `data/`. | RON files that pass `gwsim data validate` |
| Test | Write tests or run a verification that doesn't fit inside a Build task. | Tests, recorded results |
| Docs | Write documentation. | Markdown |
| Review | The owner (or a second contributor) checks work. | Review status changes, fix lists |
| Setup | Install or configure tools or the repository. | A working environment |
| Run | Run a tool against real inputs (the wiki, the optimiser). | Outputs and reports |

## Working rules (every task)

1. **Nothing is installed, scaffolded or committed without the owner's go-ahead** (C5, §18.5). The same applies to extractor crawls and to any research batch of more than 20 wiki pages. Tasks that need it are marked **Owner**.
2. **Git:** commit or push only when the owner asks. Branch before committing on `main`. Suggested branch names: `wp/<n.n>-<slug>`, e.g. `wp/1.3-template-codec`.
3. **Code authorship:** Claude writes most code; the owner reviews, decides and tests (Q12). Skills are encoded by Claude in Claude Code sessions, in batches, and reviewed by the owner (D11). There are no bulk API calls.
4. **No literals for non-wiki values.** Every value the wiki doesn't document goes into `data/assumptions.ron` and is referenced by ID (ENG-4).
5. **Wiki names in code** (§18.5): `aftercast`, `strike_level`, `armor_penetration`, and so on.
6. **Licensing** (C2, §8.10): never commit in-game description text, skill icons or other ArenaNet art, copied wiki prose, or the extractor cache.

### Definition of done for Build tasks

A Build task is done only when:

- `cargo fmt --all --check` passes;
- `cargo clippy --workspace --all-targets -- -D warnings` passes;
- `cargo test --workspace` passes;
- from WP1.2 onwards, `gwsim data validate` passes;
- new public items have doc comments;
- new non-wiki constants are in the assumptions register.

## Research tasks

Research tasks come before the build tasks that depend on them.

### Rules for research

- **Start from `research/`.** It is local only (Q42) and was written on 2026-09-22 from the wiki. It cites its wiki pages. Fetch a wiki page only when `research/` lacks a value, has a conflict, or the page may have changed.
- **Wiki access follows the extractor's rules** (C1, EXT-1 to EXT-4):
  - fetch only `https://wiki.guildwars.com/wiki/<Title>` article pages;
  - never fetch `/api.php`, `/index.php` or `Special:` pages;
  - leave at least 3 s between requests;
  - use a User-Agent without personal details.
- **PvXwiki** only through Wayback Machine snapshots (C3). Never try to get around the 403.
- **The wiki is the source of truth for game data.** Library and platform research uses official documentation (docs.rs, crate repositories, Microsoft and GitHub docs).

### Findings files [Proposed]

- **Where:** each research task writes `docs/findings/<task-id>-<slug>.md`, e.g. `docs/findings/T1.4.1-derived-stat-formulas.md`.
- **Contents:**
  - the question;
  - the sources (links);
  - the findings, **in our own words**, with numbers and worked examples;
  - decisions taken and assumptions added or changed;
  - open questions.
- **Never include:** copied wiki prose, in-game description text or images. Numbers and links are fine (C2).
- **Committed**, unlike `research/`, because they explain the values in `data/`. The owner can instead choose to keep them local like `research/`.
- **When a finding changes a design value** (e.g. an assumption's value, or a formula in §10), update DESIGN.md and `data/assumptions.ron` in the same change.
- **If a finding contradicts a [Decided] item, stop and ask the owner.**

### Research tasks at a glance

| Task | Question | Feeds |
| --- | --- | --- |
| T0.1.1 | Current Windows install steps for Build Tools, rustup and Git | WP0.1, WP0.2 |
| T0.5.2 | CC BY-SA 4.0 text, GPL-3.0 compatibility, the wiki's GFDL terms | WP0.5 |
| T0.6.2 | GitHub Actions practice for a Rust workspace on Windows | WP0.6 |
| T1.1.1 | Core values: professions, attributes, ranges, levels, conditions, modes, Asura title ranks | WP1.1 |
| T1.2.1 | RON and serde: error positions, enum forms, field-path errors | WP1.2 |
| T1.3.1 | Template bit layouts, equipment item and modifier IDs, M1 skill IDs | WP1.3 |
| T1.4.1 | Derived-stat formulas; M1 runes, insignias, weapons and 40/40 mods | WP1.4 |
| T1.5.1 | DSL coverage study over the 72 M1 skills | WP1.5 |
| T1.7.1 | Data pack format and embedding approach | WP1.7 |
| T2.1.1–T2.1.3 | robots.txt, headers and HTML structure of each page kind | P2 |
| T3.1.1 | Timing facts: activation, aftercast, queueing, cadence | WP3.1 |
| T3.2.1 | Movement speed, collision radius, projectile speeds, aggro range | WP3.2 |
| T3.4.1 | Skill-use rules per skill type | WP3.4 |
| T3.5.1 | Damage, armor, crit, healing and regeneration rules with examples | WP3.5 |
| T3.6.1 | Effect stacking, conditions, removal order, knockdown | WP3.6 |
| T3.7.1 | PRNG choice and stream derivation | WP3.7 |
| T3.10.1 | The 8 M0 skills and hand calculations for M0 | WP3.10 |
| T4.1.2–T4.1.11 (batch step 1) | Each skill batch's pages, notes and 2026 changes | WP4.1 |
| T4.2.1 | Kournan armor tables, HM ranks, weapons, behaviour | WP4.2 |
| T4.3.1 | Inherent attribute effects needed in M1 | WP4.3 |
| T4.4.1 | Foe AI: aggro, scatter, kiting, HM behaviour | WP4.4 |
| T4.5.1 | Hero AI including every 2026 update note | WP4.5 |
| T4.10.1 | Composition of the RC2 naive baseline team | WP4.10 |
| T4.11.1 | Profiling and performance CI on Windows | WP4.11 |
| T5.2.1 | NSGA-II and constrained domination details | WP5.2 |
| T5.7.1 | Account profile contents: heroes, titles, upgrades, learned skills | WP5.7 |
| T5.10.1 | The Solo Resto benchmark bars (via Wayback) | WP5.10 |
| T6.1.1 | Current eframe/egui stack, testing harness, fonts | P6 |
| T6.10.1 | Single-exe packaging and release pipeline on Windows | WP6.10 |
| T7.2.1–T7.12.1 | Each profession's mechanics before its coverage batch | P7 |
| T7.15.1 | Henchman builds per region and Reforged Mode | WP7.15 |
| T7.16.1 | Consumables, blessings and environment effects | WP7.16 |
| T7.17.1 | Benchmark teams per profession (via Wayback) | WP7.17 |
| T8.1.1 | Weapon-swap mechanics | P8 |
| T8.1.2 | The PvX page's three player weapon sets | WP8.5 |

## Where the §17.2 worked examples are tested

| Worked example | Tested in |
| --- | --- |
| Skill scaling; attribute cost; health at level 20; energy by profession; max Elementalist energy | WP1.4 |
| Template decode | WP1.3 |
| HM foe stats (health, level mapping) | WP1.4 (formulas), WP3.3 (spawned foes) |
| HM foe activation halving | WP3.4 |
| Armor calculation; skill damage vs level; weapon damage vs rank; block stacking; natural regeneration | WP3.5 |
| Movement stacking | WP3.2 |
| Condition duration | WP3.6 |
| Minion cap | WP4.3 |

## Additions this plan makes to the design [Proposed]

These are engineering details the plan adds. Each can change without reopening the design. They're recorded here so the owner can review them in one place.

| Addition | Where |
| --- | --- |
| Findings files in `docs/findings/`, committed | This file |
| Root-level `tests/` and `benches/` (§18.1) become per-crate folders. Cross-crate integration tests and relative checks live in `gwsim-cli`, which gets a library target so `gwsim check` and `cargo test` share the check code. | T0.3.1 |
| `data/skills/index.ron`: every player skill's ID, title, profession, elite and PvE-only flags, used as the coverage denominator | T2.3.6 |
| A description hash (not the text) stored per skill, so `diff` can flag changed descriptions without storing prose | T2.4.5 |
| The data pack is built by a `build.rs` in each binary that loads and validates `data/` through `gwsim-data` | T1.7.1 |
| Clippy bans on `HashMap` iteration and ambient randomness in the engine, for determinism | T3.7.3 |
| Batch review sheets are generated by `gwsim data describe` filters, not stored | T4.1.1 |
| Validation checks that each seeded `r12` value matches the scaling formula | T1.2.8 |
| Stop-when-stable defaults: batches of 32, win-rate interval ±2.5 points, clear-time interval ±2%, at most 1,024 runs | T3.8.4 |
| "A beats B" means a paired bootstrap over shared seeds | T4.10.2 |
| Clear time counts from aggro; pre-fight casting happens before it | T4.7.3 |
| Party files live in `data/parties/` | T4.9.1 |
| `gwsim profile` uses set-style subcommands rather than an interactive editor | T5.7.4 |
| Static CRT linking for the release executables | T6.10.2 |
| WP numbers for P7 and P8 | P7, P8 |
