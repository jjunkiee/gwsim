# P6 Desktop app → M3

**Phase goal:** ship `gwsim-desktop`, an egui (eframe) app distributed as a single self-contained Windows executable through GitHub Releases. Through it, a player can do every use case the data coverage allows: build and import parties, edit tactics and situations, set up an account profile, run evaluations and optimisations with live progress, read results and charts, watch a 2D replay, and compare teams. No ArenaNet art or text is used.

- **Design refs:** §5 (UC1–UC12), §14, §16, §18.4, Q9, Q25, Q39, Q43, D4, D14.
- **Starts when:** M2 is done (Q43).
- **Ends when:** the M3 criteria hold (see "M3 acceptance" at the end):
  - every use case UC1–UC12 that the data coverage allows can be done in the app;
  - a release build installs and runs on a clean Windows 11 machine.
- **Architecture rules for every task:**
  - The UI holds no simulation logic. It calls `gwsim-data`, `gwsim-engine` and `gwsim-opt`, and shares report data structures with `gwsim-cli`'s library target.
  - Long work runs on a worker thread and reports through channels. The UI thread never blocks.
  - View state is kept in plain structs ("view-models") that can be tested without rendering.
- **Suggested order:**
  1. WP6.1;
  2. WP6.2;
  3. WP6.6;
  4. WP6.7 (the evaluate-and-read loop is usable early);
  5. WP6.3, WP6.4, WP6.5, WP6.9 and WP6.8;
  6. WP6.10.

| WP | Title | Goal | Status |
| --- | --- | --- | --- |
| 6.1 | App shell | A running app with navigation, data loading, a background worker and the shared skill-chip widget. | Todo |
| 6.2 | Party and build editor | Parties and builds can be created, imported from and exported to template codes, and edited legally. | Todo |
| 6.3 | Tactics editor | The generated tactics plan can be viewed and overridden. | Todo |
| 6.4 | Situation, encounter, chain and set editors | Situations, chains, sets and user encounters can be browsed and authored. | Todo |
| 6.5 | Account profile editor | Account profiles can be managed without editing files. | Todo |
| 6.6 | Run panel | Evaluations and optimisations can be started, watched (with a live frontier) and cancelled. | Todo |
| 6.7 | Results views | Ranked lists, trade-off charts, contributions, energy timelines and notes are shown. | Todo |
| 6.8 | Replay viewer | Any seeded run can be watched on a 2D canvas with a timeline. | Todo |
| 6.9 | Comparison view | Two builds or teams can be compared side by side. | Todo |
| 6.10 | Packaging and releases | A single `.exe` is built and published by a tagged release. | Todo |

---

## WP6.1 App shell

**Goal:** a running eframe app with:

- navigation between the screens;
- the embedded data pack and the user data directory loaded;
- a worker thread for long jobs;
- error and notice handling;
- the skill-chip widget that stands in for ArenaNet icons.

- **Refs:** §16, §16.2, §6.4.
- **Depends on:** M2.
- **Done when:** `cargo run -p gwsim-desktop` opens the app, navigation works, the data loads from the embedded pack, and a dummy long job reports progress without freezing the UI.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T6.1.1 | The current eframe/egui stack | Research | — | Todo |
| T6.1.2 | App skeleton and navigation | Build | T6.1.1 | Todo |
| T6.1.3 | Data and user directory loading | Build | T6.1.2 | Todo |
| T6.1.4 | Background worker and messages | Build | T6.1.2 | Todo |
| T6.1.5 | Skill chips and type glyphs | Build | T6.1.2 | Todo |
| T6.1.6 | UI test harness | Build | T6.1.2 | Todo |

### T6.1.1 The current eframe/egui stack

**Type:** Research · **Depends on:** —

1. From docs.rs and the egui repository, record:
   - the current `eframe`, `egui` and `egui_plot` versions;
   - the renderer choice on Windows (`wgpu` or `glow`) and its effect on binary size and compatibility (older GPUs, remote desktop);
   - built-in persistence (the `persistence` feature) versus our own RON files;
   - clipboard support, for copying template codes;
   - native file dialogs (`rfd`);
   - how to open URLs (`ctx.open_url`), for the skill wiki links;
   - DPI handling;
   - whether the default fonts include the glyphs we use (…, →, ×, ±);
   - the UI testing options (`egui_kittest`).

- **Output:** `docs/findings/T6.1.1-egui-stack.md`, with the recommended crates and features.

### T6.1.2 App skeleton and navigation

**Type:** Build · **Depends on:** T6.1.1

1. Add `eframe` to `gwsim-desktop` and write an `App` struct holding `AppState`.
2. Add left-hand navigation with these screens: Party, Tactics, Situations, Profiles, Run, Results, Replay, Compare.
3. Add a status bar showing the data pack version, draft or coverage warnings, and worker status.
4. Use `#![windows_subsystem = "windows"]` in release builds, so no console window appears.

- **Done when:** every screen shows a placeholder and navigation works.

### T6.1.3 Data and user directory loading

**Type:** Build · **Depends on:** T6.1.2

1. Reuse the pack build helper (T1.7.3) in `gwsim-desktop/build.rs` to embed the data pack.
2. Load the user directory (T1.7.5). Show validation errors in user files in a notices panel, without crashing.
3. Add a settings screen: user directory path (read-only, with "open folder"), the worker thread count, and a `--data-dir` developer option.

- **Done when:** a broken user file shows a readable notice, and the app still runs.

### T6.1.4 Background worker and messages

**Type:** Build · **Depends on:** T6.1.2

1. Run one worker thread (which uses rayon internally) receiving `Job` messages (Evaluate, Optimise, Resimulate, ExportReplay), and sending back `Progress`, `FrontierSnapshot`, `Done(Result)` and `Failed(Error)`.
2. Cancellation uses the `CancelToken` from T5.2.5.
3. The UI polls the channel each frame and requests repaints while a job runs.

- **Done when:** a dummy job updates a progress bar, and cancelling stops it.

### T6.1.5 Skill chips and type glyphs

**Type:** Build · **Depends on:** T6.1.2

1. Add a `SkillChip` widget (§16.2): the skill name on a rounded rectangle coloured by profession, with a small type glyph drawn with egui shapes (spell, hex, enchantment, signet, shout, ritual, attack, stance, …).
2. It has an elite border and a draft marker.
3. Its tooltip shows the generated description (§8.6) and the review status. Clicking it opens the wiki page.
4. **No icons or in-game text** (C2).

- **Done when:** a gallery view shows every type glyph and profession colour.

### T6.1.6 UI test harness

**Type:** Build · **Depends on:** T6.1.2

1. Set up the testing approach from T6.1.1:
   - view-model unit tests;
   - `egui_kittest` (or equivalent) smoke tests that render each screen headless and click through the main flows.
2. Add them to CI.

- **Done when:** one smoke test per screen runs in `cargo test`.

---

## WP6.2 Party and build editor

**Goal:** create and edit a party of up to 8 slots (Human, Hero or Henchman; locked or free). Each build is edited with a filtered skill picker, an attribute allocator that shows the point budget, a gear editor and live legality feedback. Builds can be imported from and exported to template codes.

- **Refs:** §7.2, §14 #1, §16.1, UC1, UC2, UC3.
- **Depends on:** WP6.1.
- **Done when:**
  - the M1 party can be built from template codes in the app, saved, reloaded and exported with identical codes;
  - illegal choices are explained, not silently allowed.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T6.2.1 | Party view | Build | WP6.1 | Todo |
| T6.2.2 | Professions and attribute allocator | Build | T6.2.1 | Todo |
| T6.2.3 | Skill picker | Build | T6.2.1, T6.1.5 | Todo |
| T6.2.4 | Gear editor | Build | T6.2.1 | Todo |
| T6.2.5 | Template import and export | Build | T6.2.2–T6.2.4 | Todo |
| T6.2.6 | Save and load parties | Build | T6.2.1 | Todo |
| T6.2.7 | Editor tests | Test | T6.2.2–T6.2.6 | Todo |

### T6.2.1 Party view

**Type:** Build · **Depends on:** WP6.1

1. Show the party size (from the chosen situation, or set by hand).
2. Each slot card shows its kind (Human; Hero with profession or name, D17; Henchman once data exists), a lock toggle, the eight skill chips, a summary of professions and key attributes, and legality status.
3. Slots can be reordered.

- **Done when:** the M1 party displays correctly.

### T6.2.2 Professions and attribute allocator

**Type:** Build · **Depends on:** T6.2.1

1. Primary (fixed for heroes) and secondary pickers.
2. An attribute list with +/- buttons, the rank from points, the effective rank (with runes and headgear), and the remaining budget out of 200.
3. An "auto-allocate" button using T5.1.5.

- **Done when:** the budget never goes negative, and the effective ranks match T1.4.4.

### T6.2.3 Skill picker

**Type:** Build · **Depends on:** T6.2.1, T6.1.5

1. A searchable list with filters: profession, attribute, type, role tags, elite, PvE-only, and coverage status (`NumbersOnly` shown greyed and unusable).
2. Drag or click to fill a bar position.
3. A limits banner shows the elite and PvE-only counts, and whether this is a hero slot.

- **Done when:** the filters combine correctly, and `NumbersOnly` skills can't be placed.

### T6.2.4 Gear editor

**Type:** Build · **Depends on:** T6.2.1

1. Five armor pieces, each with an insignia and a rune picker (only legal choices for the primary profession); the headgear attribute.
2. The weapon set: weapon type, off-hand, prefix, suffix and inscription.
3. Show derived health, energy, regeneration and armor per piece live (WP1.4).

- **Done when:** the derived stats update and match the command line's values.

### T6.2.5 Template import and export

**Type:** Build · **Depends on:** T6.2.2–T6.2.4

1. Paste a skill code or an equipment code into a slot, or paste up to 8 skill codes for the whole party.
2. Copy buttons put each slot's codes on the clipboard, with the §14 #1 note about equipment codes.

- **Done when:** the M1 codes round-trip through the UI.

### T6.2.6 Save and load parties

**Type:** Build · **Depends on:** T6.2.1

1. Save and load party files (T4.9.1 format) in `%APPDATA%\gwsim\parties\`.
2. Add "open benchmark" for the bundled benchmarks.

- **Done when:** a saved party loads identically.

### T6.2.7 Editor tests

**Type:** Test · **Depends on:** T6.2.2–T6.2.6

1. View-model tests for the budget, legality, import and export.
2. Smoke tests for the flows.

- **Done when:** the tests pass.

---

## WP6.3 Tactics editor

**Goal:** show the tactics plan generated for the current party and situation, and let the user override formation, hero modes, called and locked targets, the pre-fight sequence and disabled hero skills. Overrides persist and survive regeneration.

- **Refs:** §11.6, §16.1, Q29, Q37, UC3.
- **Depends on:** WP6.2.
- **Done when:** an override made in the app is saved with the party or situation, and applied in the next run.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T6.3.1 | Plan view | Build | WP6.2 | Todo |
| T6.3.2 | Formation diagram | Build | T6.3.1 | Todo |
| T6.3.3 | Modes, targets, pre-fight and disabled skills | Build | T6.3.1 | Todo |
| T6.3.4 | Override persistence | Build | T6.3.2, T6.3.3 | Todo |
| T6.3.5 | Tactics editor tests | Test | T6.3.4 | Todo |

### T6.3.1 Plan view

**Type:** Build · **Depends on:** WP6.2

1. Show every field of the generated plan (T4.7.2). Mark which values are generated and which are overridden.
2. Add a "reset to generated" button per field.

- **Done when:** the M1 plan displays.

### T6.3.2 Formation diagram

**Type:** Build · **Depends on:** T6.3.1

1. A 2D canvas of the slot positions relative to the direction of travel. Positions can be dragged, with snapping.
2. Range rings (earshot, casting) help place slots.
3. Show a spread-against-AoE toggle.

- **Done when:** dragging a slot updates the override.

### T6.3.3 Modes, targets, pre-fight and disabled skills

**Type:** Build · **Depends on:** T6.3.1

1. A per-hero mode picker (Fight, Guard, Avoid Combat).
2. A called-target rule editor (by foe or by role).
3. A per-slot locked target.
4. An ordered pre-fight list (add, remove, reorder rows of slot, skill and target).
5. Disabled-skill toggles on each hero's chips.

- **Done when:** each field edits its override.

### T6.3.4 Override persistence

**Type:** Build · **Depends on:** T6.3.2, T6.3.3

1. Save the overrides to the party file or the situation (the user chooses which), using the T4.7.5 merge.

- **Done when:** a reload shows the same overrides, and the generated fields refresh when a build changes.

### T6.3.5 Tactics editor tests

**Type:** Test · **Depends on:** T6.3.4

1. View-model tests for the merge and reset.

- **Done when:** the tests pass.

---

## WP6.4 Situation, encounter, chain and set editors

**Goal:** browse curated, generic and user encounters and situations. Create situations (mode switches, consumables, DP and morale, timeout), chains and weighted sets. Author user encounters on a layout canvas from bundled foe data. Everything is validated as it is edited.

- **Refs:** §12, §16.1, UC4, UC9, UC12.
- **Depends on:** WP6.1.
- **Done when:** a user can author a new encounter from Kournan foes, wrap it in a chain situation, add it to a set, and run it.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T6.4.1 | Encounter and situation browser | Build | WP6.1 | Todo |
| T6.4.2 | Situation editor | Build | T6.4.1 | Todo |
| T6.4.3 | Chain editor | Build | T6.4.2 | Todo |
| T6.4.4 | Situation set editor | Build | T6.4.2 | Todo |
| T6.4.5 | Encounter layout editor | Build | T6.4.1 | Todo |
| T6.4.6 | Validation and saving | Build | T6.4.2–T6.4.5 | Todo |
| T6.4.7 | Editor tests | Test | T6.4.6 | Todo |

### T6.4.1 Encounter and situation browser

**Type:** Build · **Depends on:** WP6.1

1. Show a tree of curated encounters (by campaign and area), generic encounters and user encounters, with situations and sets beside them.
2. The details pane lists the foes with levels, the source links, the assumptions, and coverage warnings (foes with missing skills).

- **Done when:** all the M1 data is browsable.

### T6.4.2 Situation editor

**Type:** Build · **Depends on:** T6.4.1

1. Fields:
   - the encounter or chain;
   - the mode switches (HM, Reforged Mode, Dhuum's Covenant, Melandru's Accord);
   - party size;
   - consumables (from `consumables.ron` when it has data);
   - starting DP and morale;
   - the timeout;
   - notes.

- **Done when:** a new situation saves to the user directory and loads.

### T6.4.3 Chain editor

**Type:** Build · **Depends on:** T6.4.2

1. An ordered list of (encounter, rest after) rows, with add, remove and reorder.

- **Done when:** a two-fight chain can be built and run.

### T6.4.4 Situation set editor

**Type:** Build · **Depends on:** T6.4.2

1. A list of situations with weight sliders, with the normalised weights shown.
2. It warns if any situation doesn't suit the party size (§12.5).

- **Done when:** a set saves and is selectable in the run panel.

### T6.4.5 Encounter layout editor

**Type:** Build · **Depends on:** T6.4.1

1. A canvas with the party start and draggable groups.
2. Each group has a foe list (a picker over bundled foes, with variant and count), a formation primitive (Cluster radius, Line spacing, or Explicit positions dragged by hand), an optional level override and AI tags.
3. Aggro-range rings help placement.

- **Done when:** a new encounter can be laid out and saved.

### T6.4.6 Validation and saving

**Type:** Build · **Depends on:** T6.4.2–T6.4.5

1. Run the WP1.2 validation live, showing field-level messages. `NumbersOnly` foe skills are flagged (§12.6).
2. Save to the user directory with a `user:` ID.

- **Done when:** invalid input can't be saved, and the reason is shown.

### T6.4.7 Editor tests

**Type:** Test · **Depends on:** T6.4.6

1. View-model tests for each editor.
2. A smoke test for the "author, then run" flow.

- **Done when:** the tests pass.

---

## WP6.5 Account profile editor

**Goal:** manage account profiles in the app: unlocked skills (with bulk toggles), title ranks, heroes, EotN ownership and available upgrades.

- **Refs:** §7.2, §16.1, OPT-2, UC5.
- **Depends on:** WP6.1, WP5.7.
- **Done when:** a profile created in the app restricts an optimisation run as it does on the command line.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T6.5.1 | Profile list | Build | WP6.1 | Todo |
| T6.5.2 | Skill unlocks with bulk toggles | Build | T6.5.1 | Todo |
| T6.5.3 | Titles, heroes, EotN and upgrades | Build | T6.5.1 | Todo |
| T6.5.4 | Profile editor tests | Test | T6.5.2, T6.5.3 | Todo |

### T6.5.1 Profile list

**Type:** Build · **Depends on:** WP6.1

1. List, create (from the default), duplicate, rename and delete profiles.
2. Choose the active profile for runs.

- **Done when:** the profiles in the user directory show and switch.

### T6.5.2 Skill unlocks with bulk toggles

**Type:** Build · **Depends on:** T6.5.1

1. A skill grid grouped by profession and campaign, with bulk toggles (per profession, per campaign, elites only, all) and a search.

- **Done when:** toggling updates the profile file.

### T6.5.3 Titles, heroes, EotN and upgrades

**Type:** Build · **Depends on:** T6.5.1

1. Title-rank sliders per track (T5.7.2).
2. Hero checkboxes.
3. An EotN toggle.
4. Upgrade toggles by category.

- **Done when:** each field saves.

### T6.5.4 Profile editor tests

**Type:** Test · **Depends on:** T6.5.2, T6.5.3

1. View-model tests.
2. An integration test: a profile made in the UI restricts the optimiser's pools.

- **Done when:** the tests pass.

---

## WP6.6 Run panel

**Goal:** start evaluations and optimisations from the current party, situation or set and profile, with every option the command line has. Watch progress, the ETA and (for optimisation) the frontier as it improves. Cancel cleanly.

- **Refs:** §13.3 (anytime results), §16.1, UC1–UC5, UC8.
- **Depends on:** WP6.1, WP6.2; T5.2.5.
- **Done when:** an O1 run started in the app shows a live frontier and finishes with results identical to the command line's, given the same seed.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T6.6.1 | Evaluate configuration and progress | Build | WP6.2 | Todo |
| T6.6.2 | Optimise configuration | Build | T6.6.1 | Todo |
| T6.6.3 | Live frontier | Build | T6.6.2 | Todo |
| T6.6.4 | Cancel and hand-off to results | Build | T6.6.1 | Todo |
| T6.6.5 | Run panel tests | Test | T6.6.3, T6.6.4 | Todo |

### T6.6.1 Evaluate configuration and progress

**Type:** Build · **Depends on:** WP6.2

1. Choose the situation or set, the runs (fixed number or auto), the seed, and reviewed-only.
2. Show a progress bar with runs done, the ETA, and the provisional win rate.

- **Done when:** an evaluation runs, and the numbers equal the command line's for the same seed.

### T6.6.2 Optimise configuration

**Type:** Build · **Depends on:** T6.6.1

1. Free slots come from the lock toggles.
2. Set the goal, threshold, budget, and mode (evolutionary, or exhaustive with a pool editor), reviewed-only, the profile and the seed.
3. Show the candidate count for exhaustive mode, with a refusal above the limit.

- **Done when:** both modes start.

### T6.6.3 Live frontier

**Type:** Build · **Depends on:** T6.6.2

1. Plot the frontier snapshots (T5.2.5) as they arrive, using `egui_plot`, on the two chosen objectives.
2. Show the generation count and the best values.

- **Done when:** the plot updates during an O1 run.

### T6.6.4 Cancel and hand-off to results

**Type:** Build · **Depends on:** T6.6.1

1. The cancel button keeps the current frontier and marks the result "interrupted".
2. Finished results open in Results (WP6.7), and are saved to `results/` automatically.

- **Done when:** a cancelled run shows a valid partial result.

### T6.6.5 Run panel tests

**Type:** Test · **Depends on:** T6.6.3, T6.6.4

1. Check that a same-seed run gives the same result in the app and on the command line.
2. Test cancellation.

- **Done when:** the tests pass.

---

## WP6.7 Results views

**Goal:** present every result type (§14) visually:

- the ranked list with copyable template codes;
- metrics with intervals;
- the trade-off chart;
- contribution charts;
- the energy timeline;
- the assumption, draft, coverage and "uncalibrated" notes.

Results can be saved and reopened.

- **Refs:** §14 (reports 1–4), §14.1, §16.1, UC7.
- **Depends on:** WP6.6.
- **Done when:** every report the command line prints has an equivalent view, with the same numbers.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T6.7.1 | Ranked list with copy buttons | Build | WP6.6 | Todo |
| T6.7.2 | Metrics with confidence ranges | Build | WP6.6 | Todo |
| T6.7.3 | Trade-off chart | Build | WP6.6 | Todo |
| T6.7.4 | Contribution charts | Build | WP6.6 | Todo |
| T6.7.5 | Energy timeline | Build | WP6.6 | Todo |
| T6.7.6 | Notes panel | Build | WP6.6 | Todo |
| T6.7.7 | Save and open results | Build | T6.7.1 | Todo |
| T6.7.8 | Results tests | Test | T6.7.1–T6.7.7 | Todo |

### T6.7.1 Ranked list with copy buttons

**Type:** Build · **Depends on:** WP6.6

1. Rows show the rank, key metrics, each slot's bar as chips, and copy buttons for skill and equipment codes.
2. Selecting a row fills the detail views below.
3. "Open as party" loads a candidate into the editor.

- **Done when:** the codes copied from a row decode to that row's build.

### T6.7.2 Metrics with confidence ranges

**Type:** Build · **Depends on:** WP6.6

1. A report 2 table with interval bars, per situation and aggregated over the set.

- **Done when:** the numbers match the command line.

### T6.7.3 Trade-off chart

**Type:** Build · **Depends on:** WP6.6

1. A scatter of the candidates on two chosen objectives (axis pickers), with the frontier highlighted and infeasible candidates greyed.
2. Clicking a point selects the candidate.

- **Done when:** selection syncs with the ranked list.

### T6.7.4 Contribution charts

**Type:** Build · **Depends on:** WP6.6

1. Bar charts of damage, healing and mitigation per party member and per skill (report 4), plus interrupts landed.

- **Done when:** the totals equal the command line's report 4.

### T6.7.5 Energy timeline

**Type:** Build · **Depends on:** WP6.6

1. A line chart of energy over time per slot (1 s samples, mean with a band).

- **Done when:** it renders for the M1 party.

### T6.7.6 Notes panel

**Type:** Build · **Depends on:** WP6.6

1. Always visible with results:
   - the "uncalibrated" label (D22);
   - the assumptions used, with their statements;
   - the draft skills used;
   - coverage limits;
   - the pack version and seeds (§14.1).

- **Done when:** it matches the command line's footer.

### T6.7.7 Save and open results

**Type:** Build · **Depends on:** T6.7.1

1. Results save as the command line's result JSON, so both tools can open them.
2. Show a list of recent results.

- **Done when:** a result saved by the command line opens in the app.

### T6.7.8 Results tests

**Type:** Test · **Depends on:** T6.7.1–T6.7.7

1. View-model tests that every view's numbers equal the report structures.
2. Smoke tests.

- **Done when:** the tests pass.

---

## WP6.8 Replay viewer

**Goal:** watch any seeded run on a 2D canvas, with a timeline scrubber, play and pause, speed controls, and the combat log synced to the timeline. This needs a replay file format and a command-line exporter.

- **Refs:** §14 #6, §14.3, §15 (`replay export`), §16.1, UC7.
- **Depends on:** WP6.7, WP3.9.
- **Done when:** a replay exported from the command line, or produced in the app, plays back faithfully (positions and events match the log).

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T6.8.1 | Replay format and recorder | Build | WP3.9 | Todo |
| T6.8.2 | `gwsim replay export` | Build | T6.8.1 | Todo |
| T6.8.3 | Canvas rendering | Build | T6.8.1 | Todo |
| T6.8.4 | Timeline and playback | Build | T6.8.3 | Todo |
| T6.8.5 | Synced event log | Build | T6.8.4 | Todo |
| T6.8.6 | Replay tests | Test | T6.8.2–T6.8.5 | Todo |

### T6.8.1 Replay format and recorder

**Type:** Build · **Depends on:** WP3.9

1. The file (§14.3) has three parts:
   - a header: format version, data pack version, seed, party, situation, and a unit table (ID, name, allegiance, profession, radius);
   - 10 Hz frames: per unit, position, health, energy and state;
   - the event stream (the log events).
2. Encode it compactly (e.g. `postcard`, plus optional compression).
3. The recorder is a `LogSink` plus a frame sampler, and costs nothing when off.

- **Done when:** a recorded run round-trips, and the file size of an M1 fight is noted.

### T6.8.2 `gwsim replay export`

**Type:** Build · **Depends on:** T6.8.1

1. `gwsim replay export --result <file> --run <n> -o <file>` re-simulates the run and writes the replay (§15).

- **Done when:** the exported file opens in the viewer.

### T6.8.3 Canvas rendering

**Type:** Build · **Depends on:** T6.8.1

1. Draw:
   - units as circles coloured by allegiance and profession, with name labels;
   - floating health and energy bars;
   - AoE circles and wells or wards;
   - spirit ranges (toggle);
   - projectiles in flight (interpolated between frames);
   - the aggro range (toggle).
2. Pan and zoom.

- **Done when:** an M1 fight renders every element.

### T6.8.4 Timeline and playback

**Type:** Build · **Depends on:** T6.8.3

1. A scrubber, play and pause, speed (0.25× to 8×), and step by event.
2. Markers on the timeline for deaths, interrupts and big hits.

- **Done when:** scrubbing is smooth, and positions interpolate between frames.

### T6.8.5 Synced event log

**Type:** Build · **Depends on:** T6.8.4

1. A log panel that scrolls to the current time. Clicking an event jumps the timeline to it.
2. Filters by unit and by kind.

- **Done when:** the sync works in both directions.

### T6.8.6 Replay tests

**Type:** Test · **Depends on:** T6.8.2–T6.8.5

1. Test that the replay's event stream equals the command line's log for the same run.
2. Test the frame positions against the simulation at sample times.

- **Done when:** the tests pass.

---

## WP6.9 Comparison view

**Goal:** compare two builds or teams side by side on the same situations and seeds, with the difference in each metric and its significance.

- **Refs:** §14 #7, §16.1, UC6.
- **Depends on:** WP6.7.
- **Done when:** the app's comparison equals `gwsim compare` for the same inputs.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T6.9.1 | Choose A and B | Build | WP6.7 | Todo |
| T6.9.2 | Side-by-side metrics and differences | Build | T6.9.1 | Todo |
| T6.9.3 | Comparison tests | Test | T6.9.2 | Todo |

### T6.9.1 Choose A and B

**Type:** Build · **Depends on:** WP6.7

1. Pick A and B from saved parties, results or benchmarks. Choose the situations and seeds, and run the comparison through the worker (paired seeds).

- **Done when:** a comparison runs.

### T6.9.2 Side-by-side metrics and differences

**Type:** Build · **Depends on:** T6.9.1

1. Show the metrics of both parties, the differences with paired intervals, and highlighting for significant ones.
2. Show a bar diff of the two builds (the skills added and removed).

- **Done when:** the numbers equal the command line's report 7.

### T6.9.3 Comparison tests

**Type:** Test · **Depends on:** T6.9.2

1. Test that comparing a party with itself shows zero differences.
2. Test parity with the command line.

- **Done when:** the tests pass.

---

## WP6.10 Packaging and releases → **M3**

**Goal:** one self-contained Windows `.exe` with the data pack embedded, without needing the Visual C++ runtime installed. A tagged commit builds it and attaches it to a GitHub Release, and it is verified on a clean Windows 11 machine.

- **Refs:** §16.3, §18.4, D14, Q9.
- **Depends on:** WP6.1 to WP6.9.
- **Done when:** the M3 acceptance below holds.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T6.10.1 | Single-exe packaging and the release pipeline | Research | — | Todo |
| T6.10.2 | Release build configuration | Build | T6.10.1 | Todo |
| T6.10.3 | Release workflow | Build | T6.10.2 | Todo |
| T6.10.4 | Release notes and the README download section | Docs | T6.10.3 | Todo |
| T6.10.5 | First release candidate | Run | T6.10.3 | Todo |
| T6.10.6 | Clean-machine test | Test | T6.10.5 | Todo |
| T6.10.7 | Use-case walkthrough | Review | T6.10.6 | Todo |

### T6.10.1 Single-exe packaging and the release pipeline

**Type:** Research · **Depends on:** —

1. Find out:
   - whether `-C target-feature=+crt-static` for `x86_64-pc-windows-msvc` removes the Visual C++ runtime dependency, and what trade-offs it has;
   - how to embed an app icon and version information (`winresource` or `embed-resource`);
   - how unsigned executables behave with SmartScreen, and what to tell users. Code signing is out of scope unless the owner decides otherwise;
   - GitHub Actions release practice: triggering on a tag, building on `windows-latest`, and uploading assets with `gh release` or an action;
   - how to name artifacts and record their checksums.

- **Output:** `docs/findings/T6.10.1-packaging.md`.

### T6.10.2 Release build configuration

**Type:** Build · **Depends on:** T6.10.1

1. Enable static CRT for the desktop release (through `.cargo/config.toml` or the workflow's `RUSTFLAGS`).
2. Add the icon and version resource (an app icon of our own design, not ArenaNet art).
3. Use the release profile from WP4.11.
4. Build `gwsim.exe` (the command line) the same way, to ship alongside.

- **Done when:** `dumpbin /dependents` (or an equivalent check) shows no VC runtime DLLs, and the icon shows in Explorer.

### T6.10.3 Release workflow

**Type:** Build · **Depends on:** T6.10.2

1. Add `.github/workflows/release.yml`, triggered by `v*` tags. It:
   1. runs the full checks;
   2. builds both executables;
   3. computes SHA-256 checksums;
   4. creates a draft GitHub Release with the assets and generated notes.
2. The owner publishes the draft.

- **Done when:** a test tag on a fork or branch produces a draft release. This needs the owner's remote and approval.

### T6.10.4 Release notes and the README download section

**Type:** Docs · **Depends on:** T6.10.3

1. Add a README "Download" section: where to get the release; the SmartScreen note; no installer; no auto-update; no telemetry (D14); where user data lives.
2. Add a release-notes template listing the coverage and the known limitations.

- **Done when:** the docs are updated.

### T6.10.5 First release candidate

**Type:** Run · **Depends on:** T6.10.3 · **Owner:** approves tagging and publishing

1. Tag `v0.1.0-rc1` and review the draft release assets.

- **Done when:** the draft release exists.

### T6.10.6 Clean-machine test

**Type:** Test · **Depends on:** T6.10.5 · **Owner:** approves enabling Windows Sandbox or a VM

1. On a clean Windows 11 environment (Windows Sandbox or a fresh VM), download the release asset, run it with no prerequisites installed, and check:
   - the app opens;
   - the M1 party evaluates;
   - the replay plays;
   - the user data folder is created on the first save.
2. Also follow the README Getting Started section from zero in the same environment. This closes WP0.1's "clean machine" criterion.

- **Done when:** both pass, with notes recorded.

### T6.10.7 Use-case walkthrough

**Type:** Review · **Depends on:** T6.10.6 · **Owner:** walks through

1. For each use case UC1–UC12, do it in the app, within what the data coverage allows, and tick it off in `docs/findings/T6.10.7-m3-walkthrough.md`.
2. UC10 (contribute a skill encoding) and UC11 (apply a balance patch) are contributor and maintainer workflows done with data files, the command line and the extractor. Confirm with the owner that the app's part (the skill browser's generated descriptions and coverage display) is enough for M3.

- **Done when:** every use case is ticked or has an owner-accepted note.

---

## M3 acceptance

| Criterion (§19) | Evidence |
| --- | --- |
| Every use case UC1–UC12 that the data coverage allows can be done in the app | T6.10.7 |
| A release build installs and runs on a clean Windows 11 machine | T6.10.6 |
