# P5 Optimiser → M2

**Phase goal:** `gwsim optimise` searches build space for any set of free slots against a weighted situation set, and returns a success-constrained trade-off frontier plus a ranked list. It uses NSGA-II-style evolutionary search with adaptive, CRN-paired evaluation, or exhaustive search for small pools. It respects locks, legality, coverage and the account profile. It is proven on two benchmarks:

- **O1:** the best player build for the Mesmerway team.
- **O2:** the Solo Resto variant.

- **Design refs:** §5.2 (UC2–UC5, UC8, UC12), §13, §14 #1 and #3, §15 (`optimise`, `profile`), §17.4 (RC4, RC5), OPT-1 to OPT-5, Q16, Q17, Q20, Q27, D30.
- **Starts when:** M1 is done.
- **Ends when:** the M2 criteria hold (see "M2 acceptance" at the end): O1 runs within the default time budget, RC4 and RC5 pass, and the frontier and ranked output are correct and reproducible.
- **Crate:** `gwsim-opt` [Proposed split, §6.1]. It depends on `gwsim-engine` and `gwsim-data`, and has no I/O except progress callbacks.
- **Suggested order:**
  1. WP5.1;
  2. WP5.3 and WP5.6 (the evaluation and objective layers);
  3. WP5.2;
  4. WP5.4;
  5. WP5.5;
  6. WP5.7;
  7. WP5.8;
  8. WP5.9, then WP5.10.

| WP | Title | Goal | Status |
| --- | --- | --- | --- |
| 5.1 | Genome, constraints, repair | Every candidate is a legal build that respects locks, coverage and the profile. | Todo |
| 5.2 | NSGA-II | Multi-objective evolutionary search with constrained domination, seeding and an anytime frontier. | Todo |
| 5.3 | Adaptive evaluation | Evaluations are cheap where possible and precise where it matters, paired by CRN, cached and parallel. | Todo |
| 5.4 | Role tags and narrowing | Mutation draws role-compatible skills, and heuristic builds seed the search. | Todo |
| 5.5 | Exhaustive mode | Every combination from a small pool is enumerated and ranked. | Todo |
| 5.6 | Situation sets and objectives | Objectives, thresholds and weighted sets are defined and aggregated correctly. | Todo |
| 5.7 | Account profile | Searches respect unlocked skills, heroes, upgrades, title ranks and Melandru's Accord. | Todo |
| 5.8 | `optimise` command | The optimiser is exposed on the command line with report 3 as JSON. | Todo |
| 5.9 | O1, RC4 and RC5 | The best player build for Mesmerway is found and checked against the PvX bar. | Todo |
| 5.10 | O2: Solo Resto | The second benchmark, with its extra skills encoded. | Todo |

---

## WP5.1 Genome, constraints and repair

**Goal:** a compact, canonical representation of the free parts of a party. It is sampled only from legal pools and repaired into a legal build after any mutation or crossover. It never changes anything locked.

- **Refs:** §7.2, §13.1, §13.2, OPT-1 to OPT-5.
- **Depends on:** M1.
- **Done when:** a property test shows that repairing any random genome gives a legal build with its locked parts unchanged.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T5.1.1 | Genome representation | Build | — | Todo |
| T5.1.2 | Candidate pools per slot | Build | T5.1.1 | Todo |
| T5.1.3 | Legality with non-stacking runes | Build | T5.1.1 | Todo |
| T5.1.4 | Repair operators | Build | T5.1.2, T5.1.3 | Todo |
| T5.1.5 | Attribute allocation heuristic | Build | T5.1.1 | Todo |
| T5.1.6 | Canonical form and hashing | Build | T5.1.1 | Todo |
| T5.1.7 | Genome tests | Test | T5.1.2–T5.1.6 | Todo |

### T5.1.1 Genome representation

**Type:** Build · **Depends on:** —

1. Add `PartyGenome { slots: Vec<SlotGenome> }`. Each `SlotGenome` is either `Locked(Build)` or `Free(FreeGenome)`.
2. `FreeGenome` holds:
   - the primary profession (human slots only);
   - the secondary profession;
   - attribute points (a small array over the profession pair's attributes);
   - skills (`[Option<SkillId>; 8]`);
   - armor (`[(InsigniaId, RuneId); 5]`);
   - the headgear attribute;
   - the weapon set (weapon type, off-hand, prefix, suffix, inscription).
3. Partial locks within a free slot (OPT-3) are a `LockMask` of fixed skill positions and fixed gear [Proposed].
4. Conversion to and from `Build`.

- **Done when:** a genome converts to a `Build` and back unchanged.

### T5.1.2 Candidate pools per slot

**Type:** Build · **Depends on:** T5.1.1

1. For each free slot, precompute the legal skill pool:
   - the profession pair's skills, plus common skills, plus PvE-only skills for human slots only;
   - filtered by coverage (`Draft` or `Reviewed`; `Reviewed` only with `--reviewed-only`, OPT-4);
   - filtered by the account profile (OPT-2, via WP5.7; everything when there is no profile);
   - elite and PvE-only flags kept for repair.
2. Precompute the rune and insignia pools per profession, and the weapon pools.

- **Done when:** the pools for an M1 hero slot exclude PvE-only and `NumbersOnly` skills.

### T5.1.3 Legality with non-stacking runes

**Type:** Build · **Depends on:** T5.1.1

1. Reuse `Build::check` (T1.4.3).
2. Add the optimiser's rune rules: only the highest attribute rune per attribute counts, and every rune's health penalty applies.
3. Flag redundant runes (a lower rune for the same attribute) as a soft issue for repair to fix.

- **Done when:** each rule has a unit test.

### T5.1.4 Repair operators

**Type:** Build · **Depends on:** T5.1.2, T5.1.3

1. `repair(genome, pools, rng) -> genome` applies these fixes in order (OPT-5):
   1. After a profession change, replace any skill from a profession the build no longer has with a pool skill of the same role.
   2. Remove duplicate skills.
   3. Drop extra elites, keeping the one with the highest role weight.
   4. Drop extra PvE-only skills (any, on heroes).
   5. Fill empty positions from the pool by role.
   6. Re-derive attributes for the chosen skills (T5.1.5).
   7. Fix runes and insignias after a profession change.
   8. Remove redundant runes.
2. Never touch locked positions.

- **Done when:** property tests hold (T5.1.7).

### T5.1.5 Attribute allocation heuristic

**Type:** Build · **Depends on:** T5.1.1

1. Weight each attribute by the number of bar skills that use it × their scaling slope (the total of |at15 − at0| across their scaled values).
2. Allocate greedily by marginal weight per point cost, within 200 points and ranks ≤ 12 (§13.2).
3. The mutation "shift attribute points" moves one rank between two attributes and then re-legalises.

- **Done when:** the heuristic gives the player bar's skills a spread close to the PvX one (Domination highest). Record the comparison in a test comment.

### T5.1.6 Canonical form and hashing

**Type:** Build · **Depends on:** T5.1.1

1. Canonicalise a genome: the skill order sorted (bar order doesn't affect play, because the plans are generated), with locked positions kept in place; attribute maps normalised; gear per slot sorted where the order doesn't matter.
2. Compute a stable 128-bit hash of the canonical form. It keys the cache (T5.3.2) and the deduplication.

- **Done when:** two genomes that differ only in bar order hash equally.

### T5.1.7 Genome tests

**Type:** Test · **Depends on:** T5.1.2–T5.1.6

1. Property tests over random genomes and random lock masks:
   - after repair, `Build::check` passes;
   - locked parts are unchanged;
   - hashing is stable;
   - there are no PvE-only skills on heroes.

- **Done when:** the tests pass.

---

## WP5.2 NSGA-II

**Goal:** a correct NSGA-II implementation: non-dominated sorting, crowding distance, and constrained domination that pushes candidates below the success threshold behind those above it. It has mutation and crossover operators suited to builds, seeding, a time budget, a stop-on-stagnation rule, and a frontier that can be read while the search runs.

- **Refs:** §13.3, Q27.
- **Depends on:** WP5.1, WP5.3, WP5.6.
- **Done when:** the algorithm converges on known synthetic fronts, and a fixed seed reproduces the same run.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T5.2.1 | NSGA-II and constraint handling | Research | — | Todo |
| T5.2.2 | Sorting, crowding and constrained domination | Build | T5.2.1 | Todo |
| T5.2.3 | Mutation operators | Build | WP5.1 | Todo |
| T5.2.4 | Crossover operators | Build | WP5.1 | Todo |
| T5.2.5 | Main loop, budget, stopping and anytime frontier | Build | T5.2.2–T5.2.4 | Todo |
| T5.2.6 | Seeding | Build | T5.2.5 | Todo |
| T5.2.7 | Synthetic-problem tests | Test | T5.2.5 | Todo |

### T5.2.1 NSGA-II and constraint handling

**Type:** Research · **Depends on:** —

1. From the original NSGA-II paper (Deb et al., 2002) and its constraint-handling section, record:
   - fast non-dominated sorting;
   - crowding distance;
   - binary tournament selection with the crowded-comparison operator;
   - constrained domination (feasible beats infeasible; among infeasible candidates, less violation wins).
2. Map the paper's "violation" to our success threshold: violation = threshold − success rate, summed or maximised over situations per T5.6.2.
3. Choose starting operator probabilities and write them down as [Proposed] defaults.

- **Output:** `docs/findings/T5.2.1-nsga2.md`.

### T5.2.2 Sorting, crowding and constrained domination

**Type:** Build · **Depends on:** T5.2.1

1. Add `fn nondominated_sort(pop, objectives, feasibility) -> Vec<Front>`, `crowding_distance(front)` and `tournament(rng)`.
2. Objectives are minimised internally; "most energy left" is negated.
3. Ties are broken by the canonical hash, for determinism.

- **Done when:** the unit tests on hand-made populations pass.

### T5.2.3 Mutation operators

**Type:** Build · **Depends on:** WP5.1

1. Implement the operators from §13.3:
   - replace a skill (drawn from the role-compatible pool, WP5.4, with an exploration ε);
   - swap the elite;
   - shift attribute points;
   - change the secondary profession (then repair);
   - change a rune or insignia;
   - change a weapon upgrade.
2. Each operator has a probability, and each mutated child is repaired.

- **Done when:** each operator changes only its own part (before repair).

### T5.2.4 Crossover operators

**Type:** Build · **Depends on:** WP5.1

1. **Slot-level:** swap whole free-slot builds between parents.
2. **Bar-level:** uniform crossover over skill positions within a slot, then repair.

- **Done when:** unit tests confirm that children contain only genes from their parents, before repair.

### T5.2.5 Main loop, budget, stopping and anytime frontier

**Type:** Build · **Depends on:** T5.2.2–T5.2.4

1. The generation loop: evaluate (WP5.3), sort, select, vary, repair, deduplicate by hash.
2. The population defaults to 64 [Proposed].
3. **Stop** when the time budget runs out (default 5 minutes) or the frontier hasn't improved for G generations (by hypervolume, or by no new non-dominated member).
4. **Anytime:** after each generation, publish a `FrontierSnapshot` through a callback or channel (used by the command line's progress output and the desktop's live frontier, WP6.6).
5. Cancellation: a `CancelToken` stops the search cleanly and keeps the current frontier.

- **Done when:** a run stops on the budget and on stagnation, and snapshots arrive.

### T5.2.6 Seeding

**Type:** Build · **Depends on:** T5.2.5

1. Seed the initial population with:
   - benchmark builds that fit the free slots;
   - the user's current build;
   - heuristic builds (WP5.4);
   - random legal fill.
2. Record where each seed came from, for the reports.

- **Done when:** the seeds appear in generation 0 and are marked.

### T5.2.7 Synthetic-problem tests

**Type:** Test · **Depends on:** T5.2.5

1. Run the algorithm on cheap synthetic objectives with a fake evaluator (e.g. two-objective functions over a small combinatorial space with a known front, plus a feasibility constraint). Assert that most of the known front is found within N generations.
2. Test determinism: the same master seed gives the same final frontier.

- **Done when:** the tests pass.

---

## WP5.3 Adaptive evaluation, CRN, cache and parallelism

**Goal:** evaluate many candidates efficiently. Every candidate gets a cheap first look; promising ones get more runs. All candidates run on the same seed list (CRN), so comparisons are paired. Repeated work is cached, and runs use all cores.

- **Refs:** §13.4, ENG-42.
- **Depends on:** WP3.8, WP5.1.
- **Done when:** successive halving promotes and eliminates correctly, cache hits skip work, and results are independent of the thread count.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T5.3.1 | Successive halving | Build | — | Todo |
| T5.3.2 | Evaluation cache | Build | T5.1.6 | Todo |
| T5.3.3 | Scheduling, progress and ETA | Build | T5.3.1 | Todo |
| T5.3.4 | Evaluation tests | Test | T5.3.1–T5.3.3 | Todo |

### T5.3.1 Successive halving

**Type:** Build · **Depends on:** —

1. Evaluate every candidate first with 16 runs on the shared seed list prefix (T3.7.4).
2. Promote the top half to 64 runs, and frontier candidates to 256 (§13.4).
3. Eliminate a candidate only when it is significantly worse: its Wilson interval is entirely below the threshold, or entirely below a dominating candidate's.
4. Make the stage sizes configurable. Record the defaults as [Proposed].

- **Done when:** a toy population with known win rates promotes and eliminates as expected.

### T5.3.2 Evaluation cache

**Type:** Build · **Depends on:** T5.1.6

1. The key is (canonical party hash, situation ID, seed-list ID and length, data pack version).
2. Store in memory with a size bound (LRU). Extending a cached evaluation from 16 to 64 runs reuses the first 16.
3. An optional on-disk cache in the user directory is out of scope for M2; note it for later.

- **Done when:** re-evaluating the same candidate costs nothing, and extending reuses the prefix.

### T5.3.3 Scheduling, progress and ETA

**Type:** Build · **Depends on:** T5.3.1

1. Spread the (candidate, situation, seed) runs across cores with `rayon`.
2. Report progress: runs done and total, candidates per second, and the ETA to the budget.
3. Results are ordered by seed index, so aggregation doesn't depend on scheduling.

- **Done when:** the progress callbacks fire, and results match at 1 and N threads.

### T5.3.4 Evaluation tests

**Type:** Test · **Depends on:** T5.3.1–T5.3.3

1. CRN: the paired difference between two near-identical candidates has lower variance than an unpaired one (a toy measurement).
2. Cache and thread-count independence.

- **Done when:** the tests pass.

---

## WP5.4 Role tags, narrowing and heuristic seeds

**Goal:** role tags are derived automatically from the DSL, with manual additions allowed. Slot roles are inferred or set by the user. Mutation draws role-compatible skills with some exploration, and greedy heuristic bars per role and profession seed the search.

- **Refs:** §8.4 (role tags), §11.6, §13.5.
- **Depends on:** WP5.1.
- **Done when:** narrowed pools are role-compatible, the heuristic bars are legal, and the automatic tags agree with the M1 manual tags.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T5.4.1 | Automatic role tags | Build | — | Todo |
| T5.4.2 | Slot roles: inferred and user-set | Build | T5.4.1 | Todo |
| T5.4.3 | Role-weighted sampling with exploration | Build | T5.4.2 | Todo |
| T5.4.4 | Skill ratings and heuristic seed builds | Build | T5.4.3 | Todo |
| T5.4.5 | Role tests | Test | T5.4.1–T5.4.4 | Todo |

### T5.4.1 Automatic role tags

**Type:** Build · **Depends on:** —

1. Derive the tags from the DSL. For example:
   - a `Damage` action gives Damage;
   - area selectors give AoE;
   - `Interrupt` gives Interrupt;
   - `RemoveEffects(Hex)` gives Hex removal;
   - `Heal` gives Healing;
   - `CreateSpirit` gives Spirit;
   - `Summon` of a minion gives Minion;
   - energy gain or drain gives Energy management;
   - a movement-speed penalty gives Snare;
   - `Resurrect` gives Resurrection;
   - party selectors with buffs give Party buff.
2. Handlers declare their tags.
3. Manual tags in skill files are merged with the automatic ones.
4. `gwsim data describe` shows both kinds.

- **Done when:** the automatic tags for the 72 M1 skills cover every manual tag, or the difference is explained.

### T5.4.2 Slot roles: inferred and user-set

**Type:** Build · **Depends on:** T5.4.1

1. Infer a slot's role from its current or seed build: the dominant role tags weighted by skill count (e.g. healer, protection, damage, interrupt).
2. The user can set it in the party file or on the command line (`--role slot=healer`) [Proposed].

- **Done when:** the M1 heroes are inferred as expected (e.g. hero 7 is protection).

### T5.4.3 Role-weighted sampling with exploration

**Type:** Build · **Depends on:** T5.4.2

1. Weight skill draws for mutation by role compatibility with the slot's role.
2. With probability ε (default 0.1 [Proposed]), draw any legal skill instead.

- **Done when:** empirical draw frequencies match the weights within tolerance.

### T5.4.4 Skill ratings and heuristic seed builds

**Type:** Build · **Depends on:** T5.4.3

1. Rate each skill per role with a simple static score [Proposed]: magnitude at rank 12 per energy and per second of recharge, taken from the DSL values.
2. Refine the ratings later from evaluation data. Note this as a future improvement.
3. Heuristic bar per (role, profession pair): choose the highest-rated skills greedily, then repair and allocate attributes (T5.1.5).

- **Done when:** a heuristic bar is produced for every role and profession combination used by M1, and each is legal.

### T5.4.5 Role tests

**Type:** Test · **Depends on:** T5.4.1–T5.4.4

1. Test tag derivation, role inference, sampling and heuristic-bar legality.

- **Done when:** the tests pass.

---

## WP5.5 Exhaustive mode

**Goal:** for small, user-given pools (e.g. choose the best 3 of these 20 skills for slot 1), enumerate every combination with everything else fixed. Rank the results with the same adaptive evaluation, and refuse runs that are too large.

- **Refs:** §13.3 (exhaustive), UC8.
- **Depends on:** WP5.3, WP5.6.
- **Done when:** exhaustive results equal brute force on a small pool, and oversized pools are refused with a suggestion.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T5.5.1 | Pool file and counting | Build | — | Todo |
| T5.5.2 | Enumeration and evaluation | Build | T5.5.1 | Todo |
| T5.5.3 | Exhaustive tests | Test | T5.5.2 | Todo |

### T5.5.1 Pool file and counting

**Type:** Build · **Depends on:** —

1. The pool file (RON) holds a slot, the positions to fill (or `k`), the candidate skills, and optionally candidate runes, insignias or weapons.
2. Count the combinations. If the count is over the limit (default 50,000 [Proposed]), refuse, show the count, and suggest evolutionary mode.
3. Remove illegal combinations before counting (extra elites, and so on).

- **Done when:** the count matches C(n, k) minus illegal combinations on the fixtures.

### T5.5.2 Enumeration and evaluation

**Type:** Build · **Depends on:** T5.5.1

1. Enumerate deterministically (lexicographic order).
2. Evaluate through WP5.3's stages.
3. Output the frontier and the ranked list.

- **Done when:** a 3-from-8 pool runs end to end.

### T5.5.3 Exhaustive tests

**Type:** Test · **Depends on:** T5.5.2

1. On a small pool, check that the ranking equals full-run brute force.
2. Check the refusal message.

- **Done when:** the tests pass.

---

## WP5.6 Situation sets, objectives and frontier output

**Goal:** define the objectives the user can choose, the success threshold, and how a build's values are aggregated over a weighted situation set. Produce the frontier and the ranked list as data structures shared by the command line and the desktop.

- **Refs:** §12.5, §13.6, Q16, Q17.
- **Depends on:** WP3.8.
- **Done when:** aggregation, thresholds and ranking are unit-tested on known inputs.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T5.6.1 | Objectives | Build | — | Todo |
| T5.6.2 | Threshold rule | Decision | — | Todo |
| T5.6.3 | Set aggregation | Build | T5.6.1, T5.6.2 | Todo |
| T5.6.4 | Frontier and ranked outputs | Build | T5.6.3 | Todo |
| T5.6.5 | Objective tests | Test | T5.6.4 | Todo |

### T5.6.1 Objectives

**Type:** Build · **Depends on:** —

1. Add these objectives, each with a direction:
   - fastest clear (the default);
   - fewest deaths;
   - least damage taken;
   - most energy left;
   - lowest DP at the end of a chain;
   - weighted combinations of these.

- **Done when:** each objective is computed from `Evaluation` aggregates.

### T5.6.2 Threshold rule

**Type:** Decision · **Depends on:** — · **Owner:** confirms

1. Confirm the [Proposed] default: the success threshold (≥ 95%) must be met in **every** situation with non-zero weight. Offer the alternative, a weighted aggregate, as a flag.
2. Record the decision in DESIGN.md §13.6.

- **Done when:** the owner has confirmed.

### T5.6.3 Set aggregation

**Type:** Build · **Depends on:** T5.6.1, T5.6.2

1. Objective values are weighted means over the situations.
2. Feasibility and violation follow T5.6.2.
3. Validate that every situation suits the party size (§12.5).

- **Done when:** a hand-computed two-situation example matches.

### T5.6.4 Frontier and ranked outputs

**Type:** Build · **Depends on:** T5.6.3

1. `OptimisationResult`:
   - the frontier (non-dominated feasible candidates with their objective values and intervals);
   - the ranked list by the chosen goal;
   - each candidate's builds and template codes;
   - its evaluation depth;
   - where it came from (seed or generation);
   - the §14.1 notes.

- **Done when:** the structure serialises to JSON.

### T5.6.5 Objective tests

**Type:** Test · **Depends on:** T5.6.4

1. Test aggregation, feasibility, ranking ties and multi-objective dominance on fixed inputs.

- **Done when:** the tests pass.

---

## WP5.7 Account profile constraints and `profile` commands

**Goal:** searches can be limited to what an account actually has: unlocked skills, heroes owned, available upgrades, title ranks (which set PvE-only skill values) and EotN ownership. They also follow the Melandru's Accord restrictions. With no profile, everything is unlocked and maxed (Q14).

- **Refs:** §7.2 (AccountProfile), §10.12 (Melandru's Accord), OPT-2, UC5, UC12, D7.
- **Depends on:** WP5.1.
- **Done when:** a restricted profile removes the right candidates, and `gwsim profile` manages profiles.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T5.7.1 | Account profile contents | Research | — | Todo |
| T5.7.2 | Hero roster and title tracks | Data | T5.7.1 | Todo |
| T5.7.3 | `AccountProfile` type and storage | Build | T5.7.1 | Todo |
| T5.7.4 | `gwsim profile` commands | Build | T5.7.3 | Todo |
| T5.7.5 | Constraints in pools, and Melandru's Accord | Build | T5.7.3, T5.1.2 | Todo |
| T5.7.6 | Profile tests | Test | T5.7.4, T5.7.5 | Todo |

### T5.7.1 Account profile contents

**Type:** Research · **Depends on:** —

1. From `research/` and the wiki pages Hero, Mercenary Hero, Title, Optional game modes, Skill unlock and Hero skill point, record:
   - the full hero roster: name, profession and availability, including Devona, Ghost of Althea and M.O.X. in Reforged, and mercenary slots;
   - every title track that sets PvE-only skill values, with rank tables (Asura, Deldrimor, Ebon Vanguard, Norn, Lightbringer, Sunspear, Kurzick and Luxon…);
   - how upgrades (runes and insignias) are unlocked on an account;
   - what Melandru's Accord restricts (learned skills per character; hero starting, trainer-bought and hero-trainer skills; no account titles or their passive effects).

- **Output:** `docs/findings/T5.7.1-account-profile.md`.

### T5.7.2 Hero roster and title tracks

**Type:** Data · **Depends on:** T5.7.1

1. Complete `data/creatures/heroes.ron` (D17: profession and availability only).
2. Extend `data/core/titles.ron` with every track found in T5.7.1.

- **Done when:** both validate.

### T5.7.3 `AccountProfile` type and storage

**Type:** Build · **Depends on:** T5.7.1

1. Add `AccountProfile` (§7.2): unlocked skills (all, or a set), title ranks, heroes owned (all, or a set), EotN owned, available upgrades (all, or a set), and optionally the learned skills per character.
2. The default is everything unlocked and maxed.
3. Profiles are stored as RON in `%APPDATA%\gwsim\profiles\<name>.ron`.

- **Done when:** the default and a custom profile round-trip.

### T5.7.4 `gwsim profile` commands

**Type:** Build · **Depends on:** T5.7.3

1. `gwsim profile list`, `show <name>` and `new <name>` (from the default).
2. `set <name> title <track> <rank>`, `unlock <name> skill <slug…>`, `lock <name> skill <slug…>`, and `hero <name> add|remove <hero>` [Proposed].
3. `path <name>` prints the file path for hand editing, replacing an interactive `edit` on Windows.

- **Done when:** each subcommand works and validates.

### T5.7.5 Constraints in pools, and Melandru's Accord

**Type:** Build · **Depends on:** T5.7.3, T5.1.2

1. Candidate pools (T5.1.2) filter by unlocked skills and available upgrades. Hero slots are limited to owned heroes' professions. PvE-only skill values use the profile's title ranks.
2. When the situation has Melandru's Accord on:
   - skill pools are limited to learned skills per character, and to the heroes' allowed sources;
   - account titles and their passive effects are ignored.

- **Done when:** a profile missing Energy Surge never proposes it; Melandru's Accord removes unlearned skills.

### T5.7.6 Profile tests

**Type:** Test · **Depends on:** T5.7.4, T5.7.5

1. Test the pool filters, the title-rank effect on Air of Superiority, and Melandru's Accord rules.

- **Done when:** the tests pass.

---

## WP5.8 `optimise` command and report 3 as JSON

**Goal:** the optimiser on the command line, with every option in §15, live progress, clean interruption, and the frontier (report 3) as JSON plus a ranked text list.

- **Refs:** §14 #1 and #3, §15.
- **Depends on:** WP5.2 to WP5.7.
- **Done when:** a small-budget run works end to end from the command line, and the JSON follows its schema.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T5.8.1 | Command and options | Build | — | Todo |
| T5.8.2 | Output: ranked text and report 3 JSON | Build | T5.8.1 | Todo |
| T5.8.3 | Progress and interruption | Build | T5.8.1 | Todo |
| T5.8.4 | Command tests | Test | T5.8.2, T5.8.3 | Todo |

### T5.8.1 Command and options

**Type:** Build · **Depends on:** —

1. `gwsim optimise --party <file> --free <slot…> --situations <set|id> [--goal …] [--threshold 0.95] [--budget 5m] [--mode evolutionary|exhaustive --pool <file>] [--reviewed-only] [--profile <name>] [--seed S] [--json out.json]` (§15).
2. Validate the inputs: free slots exist; a pool is given in exhaustive mode.

- **Done when:** the help text and validation are complete.

### T5.8.2 Output: ranked text and report 3 JSON

**Type:** Build · **Depends on:** T5.8.1

1. **Text:** the top N ranked builds, each with its template codes (report 1), metrics with intervals (report 2), and a compact frontier table.
2. **JSON:** a schema-versioned `OptimisationResult` (T5.6.4) with the §14.1 notes.

- **Done when:** the snapshot tests pass.

### T5.8.3 Progress and interruption

**Type:** Build · **Depends on:** T5.8.1

1. Show a progress line: generation, evaluations, best objective values, and the ETA.
2. Ctrl-C cancels cleanly (`CancelToken`) and writes the current frontier as the result, marked "interrupted".

- **Done when:** an interrupted run writes a valid result.

### T5.8.4 Command tests

**Type:** Test · **Depends on:** T5.8.2, T5.8.3

1. Run a small-budget optimise over dummies, then an exhaustive run over a tiny pool, asserting the result schema and reproducibility with a fixed seed.

- **Done when:** the tests pass.

---

## WP5.9 O1: the best player build for Mesmerway; RC4 and RC5

**Goal:** the first real test of the optimiser (D30). With the 7 Mesmerway heroes locked, find the best player build against the M1 set. Check it in two ways:

- **RC4:** the PvX Energy Surge bar ranks highly among random legal builds.
- **RC5:** the optimiser does at least as well as the PvX bar.

- **Refs:** §13.7, §17.4 (RC4, RC5), D30.
- **Depends on:** WP5.8.
- **Done when:** O1 runs within the default budget, and RC4 and RC5 pass.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T5.9.1 | Random legal build sampler | Build | — | Todo |
| T5.9.2 | RC4 | Build | T5.9.1 | Todo |
| T5.9.3 | Run O1 | Run | — | Todo |
| T5.9.4 | RC5 | Build | T5.9.3 | Todo |
| T5.9.5 | Triage and record | Run | T5.9.2–T5.9.4 | Todo |

### T5.9.1 Random legal build sampler

**Type:** Build · **Depends on:** —

1. Sample a legal player build: a secondary profession (or none), a random bar from the pools with at most 1 elite and at most 3 PvE-only skills, attributes from the heuristic, and random legal gear.
2. Make it deterministic from a seed.

- **Done when:** 1,000 samples are all legal and varied (the professions and elites are spread out).

### T5.9.2 RC4

**Type:** Build · **Depends on:** T5.9.1

1. Evaluate the PvX player bar and 1,000 random legal player builds with the locked Mesmerway heroes, on the M1 set, using paired seeds.
2. **Pass rule:** the PvX bar ranks in the top 10% by the default goal, among feasible builds.
3. Add the check to `gwsim check`.

- **Done when:** the check runs.

### T5.9.3 Run O1

**Type:** Run · **Depends on:** —

1. Run `gwsim optimise --party data/parties/m1-mesmerway.ron --free player --situations m1 --budget 5m --seed <fixed>`.
2. Record the frontier, the top builds and the run statistics.

- **Output:** `docs/findings/T5.9.5-o1.md`.

### T5.9.4 RC5

**Type:** Build · **Depends on:** T5.9.3

1. Compare the optimiser's best build with the PvX bar under identical conditions (paired, full run counts).
2. **Pass rule:** no worse, within confidence.
3. Add the check to `gwsim check`.

- **Done when:** the check runs.

### T5.9.5 Triage and record

**Type:** Run · **Depends on:** T5.9.2–T5.9.4 · **Owner:** decides on failures

1. If RC4 or RC5 fails, investigate whether it is a model bug (fix it), a search weakness (tune it), or a genuine finding (discuss with the owner).
2. Record the conclusions.

- **Done when:** both pass, or the owner has accepted a documented exception.

---

## WP5.10 O2: Solo Resto benchmark

**Goal:** a second benchmark. The Solo Resto variant of Mesmerway adds the Ineptitude and Splinter Support bars. It widens coverage and tests the optimiser on hero slots.

- **Refs:** §13.7 (O2), D16, D30, C3.
- **Depends on:** WP5.9.
- **Done when:** the Solo Resto benchmark is frozen, its extra skills are `Reviewed`, and O2 runs, with results recorded.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T5.10.1 | The Solo Resto bars | Research | — | Todo |
| T5.10.2 | Encode the extra skills | Data / Build | T5.10.1 | Todo |
| T5.10.3 | Benchmark file | Data | T5.10.1 | Todo |
| T5.10.4 | Run O2 and record | Run | T5.10.2, T5.10.3 | Todo |

### T5.10.1 The Solo Resto bars

**Type:** Research · **Depends on:** —

1. Read the Solo Resto variant from the same Wayback snapshot as §20.1, or a later snapshot if one exists. **Never fetch Fandom directly** (C3). Record:
   - every bar (the Ineptitude Mesmer and Splinter Support Ritualist included), with attributes, gear and template codes;
   - the snapshot URL and date;
   - every skill not yet encoded.

- **Output:** `docs/findings/T5.10.1-solo-resto.md`.

### T5.10.2 Encode the extra skills

**Type:** Data / Build · **Depends on:** T5.10.1

1. Seed any missing numbers (extractor, owner-approved crawl).
2. Encode the skills with the WP4.1 batch procedure, and get them reviewed.

- **Done when:** the extra skills are `Reviewed`.

### T5.10.3 Benchmark file

**Type:** Data · **Depends on:** T5.10.1

1. Write `data/benchmarks/mesmerway-solo-resto.ron` with its codes, URL and date (D16).

- **Done when:** it validates and loads as a party.

### T5.10.4 Run O2 and record

**Type:** Run · **Depends on:** T5.10.2, T5.10.3

1. Evaluate Solo Resto against Dual Resto on the M1 set.
2. Run the optimiser with one hero slot free (e.g. the Splinter Support slot).
3. Record the results.

- **Output:** `docs/findings/T5.10.4-o2.md`.

---

## M2 acceptance

| Criterion (§19) | Evidence |
| --- | --- |
| O1 runs within the default time budget | T5.9.3 (5-minute budget) |
| RC4 and RC5 pass | T5.9.2, T5.9.4, T5.9.5 |
| The frontier and ranked output are correct | T5.6.5, T5.8.4 |
| The frontier and ranked output are reproducible | T5.2.7 determinism; the same seed reproduces the O1 result |
