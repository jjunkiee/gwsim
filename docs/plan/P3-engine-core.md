# P3 Engine core → M0

**Phase goal:** build the deterministic, allocation-light combat core in `gwsim-engine`. It covers time, space, units, the skill-use pipeline, damage and armor, effects, RNG streams, the multi-run harness and the combat log. **M0** proves it: the player's Energy Surge bar runs against training dummies, with values that match hand calculations from the wiki formulas.

- **Design refs:** §10 (all), §11.5, §12.2, §14.3, §17.1–§17.3, ENG-1 to ENG-43, D5, D19, D31.
- **Starts when:** WP1.1 to WP1.6 are done. WP1.7 and P2 can run alongside.
- **Ends when:** the M0 criteria (§19) hold:
  - the player bar runs against dummies deterministically;
  - Energy Surge, Mistrust, Unnatural Signet, Cry of Frustration, Spiritual Pain and Power Drain values match hand calculations;
  - Arcane Echo and Air of Superiority behave as described;
  - the formula tests pass.
- **Engine principles for every task:**
  - no I/O (ENG-1);
  - every random draw goes through the engine RNG (ENG-40);
  - every non-wiki value is read from the assumptions register (ENG-4);
  - wiki names in code;
  - no allocation in the per-tick hot path once a fight is set up (§10.14).
- **Suggested order:**
  1. WP3.1 and WP3.7 (the time and randomness foundations).
  2. WP3.2 and WP3.3.
  3. WP3.5 and WP3.6.
  4. WP3.4 (it uses both).
  5. WP3.9, then WP3.8.
  6. WP3.10.
- **Status:** 2026-09-23, **Done** in the unattended session, except the owner review of the M0 encodings (T3.10.8, logged as F3.9). The M0 criteria hold: `crates/gwsim-cli/tests/m0.rs` reproduces the T3.10.1 hand calculations exactly for all six damage and energy skills, checks Arcane Echo and Air of Superiority's behaviour, and shows identical results at 1 and 6 threads; the `wiki_examples` suites in `gwsim-data` and `gwsim-engine` pass. Decisions taken without the owner are in [fallout-tasks.md](fallout-tasks.md) §7 (F3.1–F3.20).

| WP | Title | Goal | Status |
| --- | --- | --- | --- |
| 3.1 | Time model and event queue | A deterministic simulation clock and loop, with the tick-vs-events question settled by a spike. | Done |
| 3.2 | Space | An open 2D field with range bands, movement, body-blocking, projectiles and area queries. | Done |
| 3.3 | Units and derived stats | Units spawned from builds and foe data, with a stat-modifier framework that enforces caps. | Done |
| 3.4 | Skill use pipeline | Skills used exactly as the wiki's rules describe, from validity checks through to recharge. | Done |
| 3.5 | Damage, armor and healing | Wiki-exact damage, armor, hits, crits, healing and regeneration. | Done |
| 3.6 | Effects framework | Effects, conditions, stacking, triggers and the DSL interpreter. | Done |
| 3.7 | RNG streams and CRN | Seeded, purpose-separated random streams and shared seed lists. | Done |
| 3.8 | Run harness | Many seeded runs, aggregated with confidence intervals, until the result is stable. | Done |
| 3.9 | Combat log | An opt-in, zero-cost-when-off event log in JSON Lines and text. | Done |
| 3.10 | Dummies, M0 skills and `PlanAi` v0 | M0: the player bar against dummies, matching hand calculations. | Done |

---

## WP3.1 Time model and event queue

**Goal:** a deterministic clock and event loop. The spike settles whether continuous processes (movement, regeneration, AI cadence) run on a fixed 50 ms tick or purely on events.

- **Refs:** §10.2, ENG-1, ENG-2, A-010 to A-012, A-030.
- **Depends on:** WP1.6 (assumptions).
- **Done when:** the model is chosen and recorded, and the loop runs empty and trivial fights deterministically, ending on win, wipe or timeout.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T3.1.1 | Timing facts | Research | — | Done |
| T3.1.2 | Simulation time and the event queue | Build | — | Done |
| T3.1.3 | Spike: tick vs pure events | Build | T3.1.2 | Done |
| T3.1.4 | Choose the time model | Decision | T3.1.3 | Done |
| T3.1.5 | Main loop, stop conditions and controller hook | Build | T3.1.4 | Done |
| T3.1.6 | Loop tests | Test | T3.1.5 | Done |

### T3.1.1 Timing facts

**Type:** Research · **Depends on:** —

1. From `research/` (the Skills and Game_mechanics folders) and the wiki pages Activation time, Aftercast delay, Recharge time, Attack speed and Interrupt, record:
   - the default aftercast (¾ s) and every per-type exception;
   - whether a skill can be queued during aftercast, and what the queue does on retargeting;
   - which skills can be used during another skill's activation (shouts, stances, pet attacks, flash enchantments);
   - how the attack interval relates to the attack animation and to the moment the hit lands;
   - whether the game applies regeneration continuously or in discrete ticks, and if in ticks, how long they are.
2. Record anything the wiki says about AI reaction or decision timing. It will likely say little, which is why A-010 to A-012 exist.

- **Output:** `docs/findings/T3.1.1-timing.md`, with per-type timing rules as a table.

### T3.1.2 Simulation time and the event queue

**Type:** Build · **Depends on:** —

1. Add `SimTime(u32)` in milliseconds, with checked addition. A fight longer than 49 days can't happen; assert on overflow.
2. `Event { at: SimTime, class: PriorityClass, seq: u32, kind: EventKind }` is ordered by `(at, class, seq)`. `seq` is a per-run insertion counter, so ties are deterministic (§10.2).
3. `PriorityClass` fixes the order of simultaneous events. Proposed order: effect expiry, then projectile impact, then activation end, then aftercast end, then recharge end, then triggers and AI. Record the reasoning in a doc comment.
4. The queue is a `BinaryHeap` with capacity reserved at fight start. Cancellation uses generation counters: each schedulable thing (an activation, an effect instance) has a generation, and stale events are skipped when popped.
5. Debug assertions: no event is ever scheduled before `now`.

- **Done when:** unit tests show same-time events come out in class-then-sequence order, and cancelled events never fire.

### T3.1.3 Spike: tick vs pure events

**Type:** Build · **Depends on:** T3.1.2

1. Build two throwaway prototypes in `crates/gwsim-engine/examples/` or a spike module:
   - **A, pure events:** movement is computed analytically between events (position is a function of start point, velocity and time), regeneration is integrated exactly between events, and AI decisions are scheduled as events at the decision cadence.
   - **B, hybrid:** discrete events plus a fixed 50 ms tick for movement, regeneration accumulation (fixed point) and AI.
2. Toy workload: 16 units that move towards targets, attack every 1.5–2 s, regenerate, and stop at range. It runs for 60 s of game time.
3. Measure each prototype:
   - run time (a quick `std::time::Instant` loop over 1,000 runs);
   - determinism (same seed gives the same hash);
   - code complexity for movement-cancels-activation and "enter range, then start the cast".

- **Output:** measurements in `docs/findings/T3.1.3-time-model-spike.md`.

### T3.1.4 Choose the time model

**Type:** Decision · **Depends on:** T3.1.3

1. Choose A or B from the measurements and complexity notes. The default is B unless A is clearly simpler and fast enough.
2. Record the choice in DESIGN.md §10.2 (still [Proposed], now confirmed by the spike) and in the findings.
3. Delete the losing prototype.

- **Done when:** §10.2 states the chosen model.

### T3.1.5 Main loop, stop conditions and controller hook

**Type:** Build · **Depends on:** T3.1.4

1. Add `Sim` (one fight): time, event queue, units (WP3.3), RNG streams (WP3.7), log sink (WP3.9), recorded assumptions.
2. `Sim::run(self) -> RunResult` loops:
   1. pop the due events;
   2. run the tick processes (under model B);
   3. call controllers at their cadence;
   4. check the stop conditions.
3. `Sim::step_until(t)` supports tests and replays.
4. **Stop conditions:**
   - **win:** all hostile units of the encounter are dead;
   - **wipe:** all party members are dead;
   - **timeout:** `A-030` (180 s by default), or the situation's own timeout, counted as a loss (§12.3).
5. **Controller hook:** a `Controller` trait with `fn decide(&mut self, unit, view: &SimView, rng) -> Option<Order>`. `Order` is one of: use skill (slot, target), attack (target), move to (point), stop, or cancel.
   - The loop turns orders into pipeline requests (WP3.4).
   - Reaction delays are applied by scheduling the decision's effect after the delay from the assumptions register.
   - `SimView` is a read-only view, so controllers can't mutate state directly.

- **Done when:** an empty fight ends in a timeout at 180 s, and a fight whose foes all have 0 health ends in a win at t = 0.

### T3.1.6 Loop tests

**Type:** Test · **Depends on:** T3.1.5

1. Unit tests: event order, cancellation, the timeout, and win or wipe detection.
2. A `proptest` property test: random schedules of events never deliver an event with `at < now`, and every delivery order is sorted.

- **Done when:** the tests pass.

---

## WP3.2 Space

**Goal:** an open 2D field in gwinches. It covers range bands, movement with speed modifiers and caps, body-blocking between hostile units, projectiles aimed at predicted positions, and area queries. There is no terrain (Q8).

- **Refs:** §10.3, ENG-33, ENG-34, A-001, A-002, A-003, A-009, A-024, A-025.
- **Depends on:** WP3.1.
- **Done when:** the movement-stacking example passes (Flail with "Fall Back!" gives 89.1%), the projectile hit and miss tests pass, and body-blocking holds a melee unit.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T3.2.1 | Movement, collision, projectile and aggro values | Research | — | Done |
| T3.2.2 | Geometry and range checks | Build | — | Done |
| T3.2.3 | Movement and speed modifiers | Build | T3.2.2, T3.2.1 | Done |
| T3.2.4 | Body-blocking | Build | T3.2.3 | Done |
| T3.2.5 | Projectiles | Build | T3.2.3 | Done |
| T3.2.6 | Area and target queries | Build | T3.2.2 | Done |
| T3.2.7 | Space tests | Test | T3.2.3–T3.2.6 | Done |

### T3.2.1 Movement, collision, projectile and aggro values

**Type:** Research · **Depends on:** —

1. Find the following on the wiki pages Movement, Speed boost, Projectile, Aggro, Body block, Bow, Spear and Range. Where the wiki has no value, find a documented community value and label it as one.
   - **A-001:** the base movement speed of characters and foes, in gwinches per second.
   - **A-002:** the collision radius of a normal-sized unit, and whether the wiki mentions size classes.
   - **A-003:** projectile travel speeds per weapon type (flatbow, longbow, shortbow, recurve, hornbow; spear; wand and staff) and for projectile spells.
   - **A-009:** the aggro range.
   - **A-024 and A-025:** confirm adjacent is 166, and the 240 vs 252 AoE convention.
2. Record the Flail and "Fall Back!" example used in §17.2.
3. Record what the Projectile page says about leading and missing: how much change in direction or speed makes a projectile miss.
4. Update `data/assumptions.ron` with each value, status `Assumed` or `Confirmed`, and its sources.

- **Output:** `docs/findings/T3.2.1-space-values.md`, plus the assumption updates.

### T3.2.2 Geometry and range checks

**Type:** Build · **Depends on:** —

1. Add `Vec2 { x: f32, y: f32 }` with the usual operations, and no platform-dependent maths in the hot path (sqrt only; no `sin` or `cos` needed).
2. Range checks compare squared distances with the squared band radius from `ranges.ron`.
3. Add a per-skill AoE radius (A-025) through `Aoe` from the skill file.
4. Distances are measured centre to centre. Decide whether touch and adjacent add the collision radii, per T3.2.1, and record the choice.

- **Done when:** the unit tests cover each band's boundary (inside, exactly on it, just outside).

### T3.2.3 Movement and speed modifiers

**Type:** Build · **Depends on:** T3.2.2, T3.2.1

1. `MoveOrder`:
   - to a point;
   - to within distance d of a unit (for approaching to cast or attack range);
   - away from a point (for fleeing, used later).
2. The speed is the base speed (A-001) × the product of all movement modifiers. It is clamped to +34% and −50% (ENG-33), unless the modifier's source is marked "exceeds cap" (a single skill's own effect, per the wiki). Crippled is −50%.
3. Knocked-down units don't move. Starting to move cancels an activating skill; T3.4.6 implements the cancellation, and this task raises the `MovementStarted` event.
4. Units have a facing (their movement direction), needed for "from behind" crit rules.

- **Done when:** the movement-stacking test passes (Flail with "Fall Back!" gives 89.1%, from the §17.2 Effect stacking row), and the caps hold in property tests.

### T3.2.4 Body-blocking

**Type:** Build · **Depends on:** T3.2.3

1. Units are circles with the A-002 radius.
2. When a unit moves into a **hostile** unit's circle, it stops at contact. It then slides along the tangent if its target is beyond the blocker; otherwise it waits.
3. Friendly units pass through each other in PvE (Body block page).
4. Keep it simple and deterministic: resolve collisions in `UnitId` order.

- **Done when:** a melee foe held behind a party unit standing in its path never reaches the backline, and friendly units overlap freely.

### T3.2.5 Projectiles

**Type:** Build · **Depends on:** T3.2.3

1. `Projectile { source, target, speed, launched_at, aim_point, payload }`.
2. At launch, predict the target's position at impact from its current velocity (leading), solving the intercept time from the launch position and speed.
3. Schedule an impact event at the predicted time. On impact, the projectile hits if the target is within a tolerance of the aim point (from T3.2.1) and misses otherwise.
4. Projectile speed modifiers are clamped to +100% and −50%.
5. The payload is either an attack or a skill effect. The "block projectile" type of effect doesn't apply to projectile spells (§10.3).

- **Done when:** a stationary target is always hit; a target that reverses direction at launch is missed at long range; the flight time equals distance divided by speed.

### T3.2.6 Area and target queries

**Type:** Build · **Depends on:** T3.2.2

1. Add `units_within(center, radius, filter)`, `nearest(from, filter)`, `adjacent_to(unit)`, `nearby(unit)` and `in_the_area(unit)`.
2. Filters: allegiance, alive or dead, has a corpse, is a spirit, creature type, and a predicate closure. Use brute force over flat arrays (§10.14).
3. Return results in `UnitId` order, to keep them deterministic.

- **Done when:** the unit tests cover each query and filter.

### T3.2.7 Space tests

**Type:** Test · **Depends on:** T3.2.3–T3.2.6

1. Collect the §17.2 movement-stacking row.
2. Add property tests: speed always within its caps unless marked "exceeds cap"; positions always finite.
3. Add a small approach scenario: a caster moves to casting range and stops.

- **Done when:** the tests pass.

---

## WP3.3 Units and derived stats at start

**Goal:** units are spawned from party builds and foe data with the right starting stats. A single stat-modifier framework applies caps and stacking rules in one place, so later work can't bypass them.

- **Refs:** §10.4, §10.12, ENG-33, ENG-34.
- **Depends on:** WP3.1, WP1.4.
- **Done when:** a spawned M0 player and a spawned level-26 HM foe have exactly the WP1.4 hand values.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T3.3.1 | Unit storage | Build | — | Done |
| T3.3.2 | Spawn party units | Build | T3.3.1 | Done |
| T3.3.3 | Spawn foe units | Build | T3.3.1 | Done |
| T3.3.4 | Stat-modifier framework | Build | T3.3.1 | Done |
| T3.3.5 | Spawn tests | Test | T3.3.2–T3.3.4 | Done |

### T3.3.1 Unit storage

**Type:** Build · **Depends on:** —

1. Add `UnitId(u16)`. Units live in flat vectors in the `Sim` (§10.14), with their fields grouped by use:
   - identity and allegiance;
   - kinematics;
   - resources: health, max health, energy, max energy (overcast reduces the maximum), adrenaline per skill slot;
   - skill bar: per slot, recharge ready time, disabled-until time, and the resolved `SkillValueTable`;
   - action state: idle, moving, attacking, activating (skill, target, ends at), aftercast, knocked down, dead;
   - an effect list and conditions, stored in `SmallVec`;
   - an upkeep list;
   - weapon and armor profile;
   - effective ranks and level;
   - traits;
   - the master (for minions and spirits).
2. Choose a struct-of-arrays or array-of-structs layout. Start with a simple `Vec<Unit>`; WP4.11 changes it only if profiling says so.

- **Done when:** it compiles, and a unit can be created and inspected in a test.

### T3.3.2 Spawn party units

**Type:** Build · **Depends on:** T3.3.1

1. From a `Build`, a level (20) and the data set, call the WP1.4 functions for effective ranks, maximum health, energy and regeneration pips, the armor profile, and the skill value table.
2. Give the unit its weapon set's weapon (its damage, interval and range from `weapons.ron`) and turn conditional insignia bonuses into permanent engine effects (WP3.6).
3. Record the slot kind (`Human`, `Hero` or `Henchman`) for controller selection.

- **Done when:** the M0 player's spawned stats equal the T1.4.1 hand values.

### T3.3.3 Spawn foe units

**Type:** Build · **Depends on:** T3.3.1

1. From a `Foe`, the mode switches and an optional level override, call the WP1.4 foe functions for level, health, energy with +1 pip, armor per damage type, and attributes (A-005 when needed, recorded as used).
2. The weapon comes from the foe data or from A-004.
3. Build the skill bar: HM-only skills are included only in HM; variants are chosen from the encounter entry.
4. Apply HM movement (+33%) and attack speed (+25%) as permanent foe modifiers, marked with their `modes.ron` source.

- **Done when:** a level-26 HM Kournan Seer spawns with the health, energy and ranks of the formula.

### T3.3.4 Stat-modifier framework

**Type:** Build · **Depends on:** T3.3.1

1. Define `Stat` and `ModCategory` in the DSL (T1.5.2). Here, implement `StatBlock`:
   - the base values;
   - active modifiers, each with its source effect, value, category (additive, multiplicative, core, bonus or special armor) and an `exceeds_cap` flag;
   - cached final values with a dirty flag.
2. When a modifier is added or removed, recompute only the affected stat.
3. Apply all caps in one function, `apply_caps(stat, value)`, using the ENG-33 list. Percentage modifiers stack multiplicatively (ENG-34).
4. When an attribute rank changes temporarily, recompute that unit's skill value table (§10.14).

- **Done when:** unit tests cover each cap in ENG-33 and multiplicative stacking (two 50% block chances give 75%, from §17.2's block-stacking row).

### T3.3.5 Spawn tests

**Type:** Test · **Depends on:** T3.3.2–T3.3.4

1. Write a test per M1 party slot comparing spawned stats with the T1.4.1 hand values. This extends `m1_builds.rs` to engine spawns.
2. Write foe spawn tests at NM 20 and HM 26.
3. Add property tests: health never above max; attribute ranks ≤ 20.

- **Done when:** the tests pass.

---

## WP3.4 Skill use pipeline

**Goal:** a skill is used exactly as the wiki describes. The pipeline covers:

- validity checks;
- approaching to range;
- activation time with every modifier;
- cost payment;
- the five outcomes;
- aftercast, recharge and upkeep;
- interrupts.

Auto-attacks and attack skills follow the same rules.

- **Refs:** §10.5, §10.6, ENG-10 to ENG-18, A-017, A-032.
- **Depends on:** WP3.2, WP3.3, WP3.6 (the pipeline resolves effects through the interpreter).
- **Done when:** a table-driven test per ENG-10 to ENG-18 passes.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T3.4.1 | Skill-use rules per skill type | Research | T3.1.1 | Done |
| T3.4.2 | Validity checks (ENG-10) | Build | T3.4.1 | Done |
| T3.4.3 | Approach to range (ENG-11) | Build | T3.4.2 | Done |
| T3.4.4 | Activation time (ENG-12) | Build | T3.4.1 | Done |
| T3.4.5 | Costs (ENG-13) | Build | T3.4.2 | Done |
| T3.4.6 | Outcomes and aftercast (ENG-14) | Build | T3.4.4, T3.4.5 | Done |
| T3.4.7 | Recharge (ENG-16) | Build | T3.4.6 | Done |
| T3.4.8 | Upkeep (ENG-17) | Build | T3.4.6 | Done |
| T3.4.9 | Interrupts (ENG-18) | Build | T3.4.6 | Done |
| T3.4.10 | Auto-attacks and attack skills (ENG-15) | Build | T3.4.6 | Done |
| T3.4.11 | Pipeline tests | Test | T3.4.2–T3.4.10 | Done |

### T3.4.1 Skill-use rules per skill type

**Type:** Research · **Depends on:** T3.1.1

1. Using `research/Skill_types/` and the wiki pages Activation time, Interrupt, Resource cost, Overcast, Fast Casting, Recharge time and Dazed, build a table with one row per skill type and these columns:
   - activation;
   - aftercast;
   - usable while knocked down;
   - uses the action queue;
   - can be used during another activation;
   - interruptible;
   - Fast Casting applies.
2. Answer these open points:
   - Which skills does Fast Casting affect in PvE: only spells, signets too (A-017), and is there a minimum activation time for non-Mesmer spells?
   - Does PvE Fast Casting's Mesmer recharge reduction of 3% per rank combine additively or multiplicatively, and does it apply to all Mesmer spells?
   - How does overcast work: how it is paid, how it reduces maximum energy, and how fast it recovers?
   - What exactly counts as "fail" and what as "fizzle"?
   - How do the weapon half-cast and half-recharge chance mods roll (per use, and how chances from two items combine)?
   - What is the HM foe recharge reduction (A-032), if the wiki gives one?
3. Update T1.1.3's per-type defaults if they differ.

- **Output:** `docs/findings/T3.4.1-skill-use-rules.md`.

### T3.4.2 Validity checks (ENG-10)

**Type:** Build · **Depends on:** T3.4.1

1. Add `can_use(unit, slot, target) -> Result<(), Invalid>`. The `Invalid` reasons are:
   - `Dead`;
   - `KnockedDown` (types allowed while knocked down are exempt);
   - `Recharging`;
   - `Disabled`;
   - `NoEnergy`, `NoAdrenaline` or `NoHealthForSacrifice`;
   - `BadTarget` (with detail: wrong allegiance, dead when it must be alive, not a corpse, or a filter failed);
   - `OneAtATime` (a family effect blocks use, where the rules say so);
   - `Busy` (mid-action, and the type can't be used outside the queue);
   - `FlashDuringActivation`.
2. Controllers and logs use the reasons, e.g. "why didn't the hero cast X".

- **Done when:** there is a unit test per reason.

### T3.4.3 Approach to range (ENG-11)

**Type:** Build · **Depends on:** T3.4.2

1. If the target is out of the skill's range (or melee range for melee attacks), issue a move order to reach it, and start the skill on arrival.
2. Cancel the approach if the target dies or becomes invalid, or if a new order arrives.

- **Done when:** a caster 2,000 gwinches away walks to casting range and then casts.

### T3.4.4 Activation time (ENG-12)

**Type:** Build · **Depends on:** T3.4.1

1. `activation_ms(unit, skill)` is computed as:

   ```text
   base
     × product of item and skill modifiers (clamped to −25% / +150%)
     × Fast Casting 0.5^(rank/15), where it applies (outside the cap)
     × 2 when Dazed (spells)
   ```

2. For HM foes, a skill over 2 s takes half as long.
3. Attack skills scale with attack speed.
4. The half-cast-time chance from weapon mods is rolled on the hits/crits RNG stream at activation start (per T3.4.1).
5. Round to whole milliseconds with one documented rounding rule.

- **Done when:** unit tests cover rank 15 halving the activation, the clamps, Dazed doubling, the HM halving of a 3 s foe skill, and HM leaving a 2 s skill unchanged.

### T3.4.5 Costs (ENG-13)

**Type:** Build · **Depends on:** T3.4.2

1. Pay energy (after cost modifiers), adrenaline strikes and overcast at activation start.
2. Take sacrifice (a percentage of maximum health) only after a successful activation.
3. Emit `OnEnergyChanged`.

- **Done when:** unit tests show energy leaves at the start and sacrifice only on completion.

### T3.4.6 Outcomes and aftercast (ENG-14)

**Type:** Build · **Depends on:** T3.4.4, T3.4.5

1. Implement the five outcomes:

   | Outcome | Rule |
   | --- | --- |
   | Complete | Resolve the effects (through the WP3.6 interpreter or the handler), then aftercast per type, then recharge starts. |
   | Interrupted | The cost is lost, recharge starts, and the queued action is cleared. |
   | Cancelled (moving, retargeting, a new order) | Costs are paid, with no recharge and no aftercast. |
   | Fail (a prerequisite is missing, or a single-target skill's target is lost) | The cost is lost, and the skill recharges instantly. |
   | Fizzle (invalid target at completion) | No recharge. |

2. Emit `OnSkillActivationStart`, `OnSkillActivationEnd`, `OnSpellCast` (spells only), `OnInterrupted` and the matching log events.

- **Done when:** there is one test per outcome.

### T3.4.7 Recharge (ENG-16)

**Type:** Build · **Depends on:** T3.4.6

1. The recharge is:

   ```text
   base
     × product of item and effect reductions (clamped at −50%, unless a skill's own effect exceeds the cap)
     × PvE Fast Casting reduction for Mesmer spells (3% per rank, per T3.4.1, uncapped)
   ```

2. Roll the half-recharge chance.
3. HM foe reduction: A-032.
4. The recharge-end event makes the slot ready again.

- **Done when:** unit tests cover the cap, the Fast Casting reduction and the chance roll (with a seeded RNG).

### T3.4.8 Upkeep (ENG-17)

**Type:** Build · **Depends on:** T3.4.6

1. Each maintained enchantment adds −1 energy pip to its caster while it lasts.
2. If the caster's degeneration before the cap would be 11 or more, maintained enchantments drop until it is back to 10. Choose the drop order per the Energy page, or the oldest first if the page doesn't say, and record that as an assumption.
3. Maintained enchantments can be cancelled by an order (for hero out-of-combat rules later).

- **Done when:** a unit maintaining 11 enchantments drops one immediately.

### T3.4.9 Interrupts (ENG-18)

**Type:** Build · **Depends on:** T3.4.6

1. Add `interrupt(unit, what: Any | Skill | Spell | Attack | Chant | …) -> bool`, which is true if something was interrupted.
2. Anything with an activation time can be interrupted, and so can every attack. Running, instant skills and emotes can't.
3. Dazed spells are easily interrupted, and applying Dazed interrupts a spell in progress.
4. Knockdown, death, failure and disabling are **not** interrupts: they stop the activation through their own paths and bypass interrupt prevention.
5. Add an interrupt-prevention hook for effects that prevent interruption.

- **Done when:** unit tests confirm that interrupting starts recharge, a spell-only interrupt ignores a signet, knockdown isn't counted as an interrupt, and Dazed interrupts a spell.

### T3.4.10 Auto-attacks and attack skills (ENG-15)

**Type:** Build · **Depends on:** T3.4.6

1. The auto-attack cycle uses the weapon's attack interval × attack-speed modifiers (clamped to +33% and −50%). Melee attacks swing at melee range; ranged attacks launch projectiles (T3.2.5).
2. Attack skills without a stated activation use the first half of the next attack interval. With a stated activation, the hit lands halfway through it.
3. "+X damage" is armor-ignoring and added to the same packet (§10.6), through WP3.5.
4. Adrenaline is gained per hit, following the adrenaline rules from T3.5.1.

- **Done when:** a wand user attacks at its interval, and an attack skill's hit lands at the expected time.

### T3.4.11 Pipeline tests

**Type:** Test · **Depends on:** T3.4.2–T3.4.10

1. Write `crates/gwsim-engine/tests/skill_pipeline.rs`: a table of scripted cases (unit, bar, order, time → expected state), with at least one case per ENG-10 to ENG-18 clause.
2. Add the §17.2 HM activation-halving row.

- **Done when:** the table passes.

---

## WP3.5 Damage, armor, hit locations, crits, healing and regeneration

**Goal:** implement the wiki's damage and healing mathematics exactly once, named after the wiki concepts and tested against the wiki's worked examples. Health reaching 0 is handled minimally: the unit dies and leaves a corpse.

- **Refs:** §10.6, §10.7, §17.2, ENG-3, A-014, A-021, A-022, A-023, A-031.
- **Depends on:** WP3.3.
- **Done when:** the §17.2 armor, skill-damage, weapon-damage, block-stacking and natural-regeneration rows pass.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T3.5.1 | Damage and healing rules with worked examples | Research | — | Done |
| T3.5.2 | Damage packet and strike level | Build | T3.5.1 | Done |
| T3.5.3 | Armor calculation | Build | T3.5.1 | Done |
| T3.5.4 | Hit resolution | Build | T3.5.2, T3.5.3 | Done |
| T3.5.5 | Damage modifier pipeline and mitigation hooks | Build | T3.5.2 | Done |
| T3.5.6 | Healing | Build | T3.5.1 | Done |
| T3.5.7 | Regeneration, degeneration and combat state | Build | T3.5.1 | Done |
| T3.5.8 | Death (minimal) | Build | T3.5.5 | Done |
| T3.5.9 | Wiki-example and property tests | Test | T3.5.2–T3.5.8 | Done |

### T3.5.1 Damage and healing rules with worked examples

**Type:** Research · **Depends on:** —

1. From `research/` and the wiki pages Damage calculation, Armor calculation, Order of damage modifiers (A-022), Critical hit, Damage type, Block, Evade, Healing, Health regeneration, Deep Wound, Life stealing and Health loss, record:
   - the strike-level rules (5 × weapon rank up to `(level + 4)/2`, then +2 per rank; 3 × level for skills, wands and staves);
   - the damage formula, with every worked example;
   - the armor-calculation steps, the A-023 rounding and the A-031 bug;
   - which damage types respect armor (§10.7);
   - the hit-location table (normal and ranged columns, and the low and high profiles);
   - the base critical-hit chance formula and what changes it (Critical Strikes, Expertise, attacks from behind on moving targets);
   - the order of the modifier list;
   - damage caps (per packet or per time);
   - the healing stacking rule, the −40% cap and Deep Wound's −20%;
   - natural regeneration and what counts as "in combat";
   - the adrenaline gained per hit and per attack.
2. For each test in §17.2's armor, damage and regeneration rows, write out the inputs and the expected result.

- **Output:** `docs/findings/T3.5.1-damage-rules.md`.

### T3.5.2 Damage packet and strike level

**Type:** Build · **Depends on:** T3.5.1

1. Add `strike_level(weapon_rank, level)` and `skill_strike_level(level)`, which is 3 × level.
2. Add `damage_packet(base, strike_level, armor_level)`, which is `base × 2^((strike − AL)/40)`.
3. Route damage by type:
   - physical, elemental, chaos and dark, and holy from weapons respect armor;
   - shadow, typeless skill damage and armor-ignoring holy skills ignore it (a per-skill flag).
4. Add the level scaling of skill damage.

- **Done when:** "skill damage vs level" (level 15 gives 77.1%; a level-30 caster's 70 damage against 60 AL gives about 117.7) and "weapon damage vs rank" (rank 8 gives 70.7%; rank 16 gives 114.9%) pass.

### T3.5.3 Armor calculation

**Type:** Build · **Depends on:** T3.5.1

1. Add `armor_level(core, bonuses: &[i16], penetration, special)`, which runs four steps:
   1. core armor;
   2. net bonus: if ≥ 26, add 25 or the largest single bonus; if ≤ 25, add it; if negative, reduce only down to 60, or to core armor if core is below 60;
   3. `× (1 − AP)`, floored (A-023);
   4. special armor, uncapped.
2. Model the A-031 bug: reductions are ignored when the net bonus is ≥ 26.
3. Per hit: choose the location (A-014 table), then the piece's armor for the damage type (from the T1.4.6 profile plus active modifiers).

- **Done when:** the §17.2 example passes: core 106 with a net bonus of −25 gives 81; 25% AP gives 61 (±1, A-023); special +24 gives 85.

### T3.5.4 Hit resolution

**Type:** Build · **Depends on:** T3.5.2, T3.5.3

1. Resolve an attack in order [Proposed; confirm with T3.5.1]:
   1. miss chance (Blind 90% plus effects, multiplicative);
   2. block or evade chance (multiplicative, so 50% + 50% gives 75%);
   3. hit location;
   4. crit: the base chance from T3.5.1, or always from behind on a moving target for melee. A crit uses maximum damage and strike level +20 (×√2).
2. Then apply the adrenaline gain, weapon conditions from upgrades, and the `OnHit`, `OnBlocked` and `OnMiss` events.
3. Draw all rolls from the hits/crits stream of the attacker (T3.7.2).

- **Done when:** block stacking gives 75%, and the observed crit rate over 100,000 seeded rolls matches the formula within 0.5 percentage points.

### T3.5.5 Damage modifier pipeline and mitigation hooks

**Type:** Build · **Depends on:** T3.5.2

1. Apply `apply_damage(source, target, packet)` in the order of the wiki's modifier list (A-022). Life stealing and health loss are applied first (§10.7).
2. Add hook points for handlers and effects:
   - `modify_damage_dealt`;
   - `modify_damage_taken` (damage reduction);
   - `cap_damage` (the Shelter family);
   - `redistribute` (damage shares);
   - `on_damage_taken`.
3. Record the mitigation credited to each hook's source, for the §14.2 attribution.
4. Record in the per-run statistics the damage dealt (by skill and unit), damage taken and overkill.

- **Done when:** unit tests show a cap limits a packet, a reduction reduces it, and the order is the documented one.

### T3.5.6 Healing

**Type:** Build · **Depends on:** T3.5.1

1. `apply_heal(source, target, amount, kind: Heal | HealthGain)`: modifiers stack multiplicatively, the reduction is capped at −40%, and Deep Wound gives −20%. Kinds that ignore modifiers follow T3.5.1.
2. Track overhealing separately (§14.2).

- **Done when:** the cap and Deep Wound tests pass.

### T3.5.7 Regeneration, degeneration and combat state

**Type:** Build · **Depends on:** T3.5.1

1. The net health pips are the sum of all sources, clamped to ±10, at 2 HP/s per pip. Energy pips run at 1 energy per 3 s per pip. Use fixed-point accumulation per tick, or exact integration under model A.
2. Track combat state per unit ("in combat" as defined in T3.5.1: last damage, attack, skill use or aggro within N s).
3. Natural regeneration: after 5 s out of combat, +1 pip, then +1 more every 2 s, up to +7. None while the net degeneration is below 0.

- **Done when:** the natural-regeneration row passes (+1 at 5 s, +1 every 2 s, maximum +7), and the ±10 clamp holds.

### T3.5.8 Death (minimal)

**Type:** Build · **Depends on:** T3.5.5

1. When health reaches ≤ 0:
   - the unit becomes `Dead`;
   - its activation stops (not as an interrupt);
   - most effects are cleared (per T3.6.1; full rules in WP4.3);
   - it leaves an exploitable corpse if fleshy;
   - `OnDeath` is emitted for it and `OnKill` for the killer, with "gives experience" true for foes.
2. Dead units are excluded from target queries unless a skill targets corpses.

- **Done when:** a dummy killed by damage raises `OnKill` once and stops acting.

### T3.5.9 Wiki-example and property tests

**Type:** Test · **Depends on:** T3.5.2–T3.5.8

1. Write `crates/gwsim-engine/tests/wiki_examples.rs` with the §17.2 rows assigned to WP3.5, each citing its page.
2. Add property tests: health ≤ max, damage ≥ 0, regeneration within ±10, and energy below 0 only through the wiki's maximum-energy-reduction case.

- **Done when:** the tests pass.

---

## WP3.6 Effects framework

**Goal:** effects are applied, stacked, expired, removed and triggered exactly once in shared code. Conditions, one-at-a-time families and caps are enforced there. The DSL interpreter and the handler registry run skill encodings.

- **Refs:** §8.4, §8.5, §10.8, ENG-30 to ENG-35.
- **Depends on:** WP3.3, WP1.5.
- **Done when:**
  - the condition-duration example passes (8 s Blind with two 20% reductions gives 4 s);
  - the stacking, one-at-a-time, removal, trigger and knockdown tests pass;
  - the interpreter runs the WP1.5 example encodings.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T3.6.1 | Effect stacking, conditions, removal and knockdown | Research | — | Done |
| T3.6.2 | Active effects and stacking rules | Build | T3.6.1 | Done |
| T3.6.3 | One-at-a-time families (ENG-32) | Build | T3.6.2 | Done |
| T3.6.4 | Conditions (ENG-30) | Build | T3.6.2 | Done |
| T3.6.5 | `while_active` modifiers and caps | Build | T3.6.2, T3.3.4 | Done |
| T3.6.6 | Event bus and triggers | Build | T3.6.2 | Done |
| T3.6.7 | DSL interpreter | Build | T3.6.5, T3.6.6 | Done |
| T3.6.8 | Handler trait and registry | Build | T3.6.7 | Done |
| T3.6.9 | Knockdown (ENG-35) | Build | T3.6.2 | Done |
| T3.6.10 | Effects tests | Test | T3.6.2–T3.6.9 | Done |

### T3.6.1 Effect stacking, conditions, removal and knockdown

**Type:** Research · **Depends on:** —

1. From `research/Effects/` and the wiki pages Effect stacking, Condition, Hex spell, Enchantment spell, Knock down, Disease and Death, record:
   - the stacking rule per effect kind (replace, keep longer, stack by source; what "strongest" means);
   - which effects removal skills take first (most recent? random?);
   - the condition details: fleshy-only conditions, spirit immunity, how Disease spreads (range, same creature type, timing), and how duration reductions round;
   - the knockdown duration and what may be used while knocked down;
   - which effects death clears and which it keeps;
   - how effect duration modifiers combine.

- **Output:** `docs/findings/T3.6.1-effects.md`.

### T3.6.2 Active effects and stacking rules

**Type:** Build · **Depends on:** T3.6.1

1. Add `ActiveEffect { def: EffectRef, source: UnitId, skill: Option<SkillId>, target, applied_at, ends_at, generation, charges, stacks, data: EffectData }`.
2. Keep a per-unit `SmallVec<[ActiveEffect; 8]>`.
3. `apply_effect` looks up the stacking key and applies the rule: replace, keep the longer, or stack by source (§8.4). It schedules the expiry event, emits `OnEffectApplied`, and logs.
4. `remove_effects(unit, kind, n, order)` removes effects in the T3.6.1 order, running `on_end` and emitting `OnEffectRemoved`.
5. Expiry runs `on_end` and emits `OnEffectEnded`.

- **Done when:** the unit tests cover each stacking rule and the removal order.

### T3.6.3 One-at-a-time families (ENG-32)

**Type:** Build · **Depends on:** T3.6.2

1. Enforce the families: stance, preparation, glyph, form, party bonus, one weapon spell per target, and one bundle (a new item spell drops the old one and fires its drop effect).
2. Binding rituals replace an **allied** spirit of the same type. Nature rituals replace allied **and** enemy spirits of the same type. The spirit side is finished in WP4.3; the rule lives here.

- **Done when:** there is one test per family.

### T3.6.4 Conditions (ENG-30)

**Type:** Build · **Depends on:** T3.6.2

1. Implement all 10 conditions from `conditions.ron`:
   - reapplying keeps the longer duration;
   - duration reductions from separate sources apply and round separately;
   - Bleeding, Poison and Disease affect fleshy creatures only;
   - spirits are immune to every condition except Burning;
   - Disease spreads to adjacent creatures of the same type.
2. Blind, Crippled, Dazed, Weakness, Deep Wound and Cracked Armor hook into their mechanics (miss chance, speed, activation and interrupts, attack damage, healing and max health per the wiki, armor).

- **Done when:** the condition-duration row passes (8 s Blind with two 20% reductions gives 4 s), along with the fleshy and spirit immunity tests.

### T3.6.5 `while_active` modifiers and caps

**Type:** Build · **Depends on:** T3.6.2, T3.3.4

1. Applying an effect registers its `while_active` modifiers in the target's `StatBlock` (T3.3.4), with the effect as source. Removing it unregisters them.
2. Caps and multiplicative stacking come only from `StatBlock` (ENG-33, ENG-34).

- **Done when:** a property test with random sets of speed and attack-speed effects never exceeds a cap unless `exceeds_cap` is set.

### T3.6.6 Event bus and triggers

**Type:** Build · **Depends on:** T3.6.2

1. `EventBus::emit(event)` dispatches synchronously to:
   - active effects' `triggers` whose event and filter match;
   - handlers subscribed to the event;
   - the log.
2. Triggers have charges ("the next N attacks") and durations. They are consumed deterministically, in unit order and then effect order.
3. A depth guard limits nested triggers (e.g. 8 levels), logs a warning if the limit is hit, and never loops forever.

- **Done when:** a "next 3 attacks" trigger fires 3 times and then expires, and the dispatch order is stable across runs.

### T3.6.7 DSL interpreter

**Type:** Build · **Depends on:** T3.6.5, T3.6.6

1. Add `execute(actions, ctx: ExecCtx { caster, target, skill, rank_values, rng })`. It evaluates:
   - `Value`s against the caster's skill value table and title ranks;
   - `Selector`s through the WP3.2 queries and filters;
   - every `Action` and `Control` node.
2. `Secondary(factor)` applies the factor to units other than the main target.
3. `Chance(p, …)` draws from the skill-effects stream.
4. Evaluate without allocating: reuse scratch buffers for selector results.
5. Record every assumption ID a node reads.

- **Done when:** the WP1.5 example encodings run on test units and give the expected state changes.

### T3.6.8 Handler trait and registry

**Type:** Build · **Depends on:** T3.6.7

1. Add `trait SkillHandler: HandlerDescribe { fn on_use(&self, ctx) {} fn on_event(&self, ev, ctx) {} fn modify_damage(&self, …) {} … }`, with default no-op methods (§8.5).
2. Add `HandlerRegistry`: name → `Box<dyn SkillHandler>`, built at start-up. It implements the data crate's `HandlerRegistry` (T1.5.3), so validation and `describe` can see the handler names.
3. Handlers may call the interpreter for shared actions.

- **Done when:** a test handler registered by name runs from a skill file's `handler: "…"` reference, and `gwsim data validate` sees the registered names.

### T3.6.9 Knockdown (ENG-35)

**Type:** Build · **Depends on:** T3.6.2

1. `knock_down(unit, duration)`:
   - stops the activation (not an interrupt, bypassing interrupt prevention);
   - stops movement;
   - blocks actions except stances, shouts and pet attacks;
   - emits `OnKnockedDown`.

- **Done when:** a knocked-down caster's spell stops without an interrupt event, and a stance is still usable.

### T3.6.10 Effects tests

**Type:** Test · **Depends on:** T3.6.2–T3.6.9

1. Write `crates/gwsim-engine/tests/effects.rs` covering:
   - the condition-duration row;
   - stacking;
   - families;
   - removal;
   - triggers;
   - knockdown;
   - a property test on caps.

- **Done when:** the tests pass.

---

## WP3.7 RNG streams and CRN

**Goal:** all randomness comes from a seeded, portable PRNG split into purpose- and unit-specific streams. Changing one build disturbs other units' rolls as little as possible, and candidates can share seed lists.

- **Refs:** §10.13, ENG-40 to ENG-43.
- **Depends on:** WP3.1.
- **Done when:** the same seed gives bit-identical results, and changing one unit's build leaves another unit's hit-roll sequence unchanged.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T3.7.1 | Choose the PRNG | Research / Decision | — | Done |
| T3.7.2 | Stream derivation | Build | T3.7.1 | Done |
| T3.7.3 | Determinism guards | Build | — | Done |
| T3.7.4 | Seed lists | Build | T3.7.2 | Done |
| T3.7.5 | Determinism tests | Test | T3.7.2–T3.7.4, WP3.8 | Done |

### T3.7.1 Choose the PRNG

**Type:** Research / Decision · **Depends on:** —

1. From docs.rs and the crates' documentation, compare `rand_chacha::ChaCha8Rng` with `rand_pcg::Pcg64Mcg`:
   - portability of output across platforms and versions;
   - speed;
   - support for independent streams (ChaCha's `set_stream`, or seeding from a hash).
2. Recommend one. ChaCha8 is expected, for well-defined stream support.
3. Record the exact crate versions, because output must not change across dependency updates without being noticed. Pin the PRNG crate's minor version.

- **Output:** `docs/findings/T3.7.1-prng.md`.

### T3.7.2 Stream derivation

**Type:** Build · **Depends on:** T3.7.1

1. `RunSeed(u64)`. `stream(seed, purpose, unit: Option<UnitId>)` derives an independent generator by hashing (seed, purpose, unit) with a fixed function (e.g. SplitMix64 steps) into a ChaCha seed.
2. The purposes are `Hits`, `Crits`, `SkillChance`, `Ai`, `Spawn` and `Consumables`. Keep one stream per unit per purpose.
3. Units are numbered by stable slot and foe order, so adding a skill to slot 3 doesn't renumber slot 5.

- **Done when:** unit tests show streams for different purposes or units don't overlap in their first 10,000 draws, and a stream is reproducible from its seed.

### T3.7.3 Determinism guards

**Type:** Build · **Depends on:** —

1. Add `clippy.toml` `disallowed-methods` and `disallowed-types` for `gwsim-engine` and `gwsim-opt`:
   - ban `rand::thread_rng` and `rand::random`;
   - ban `std::collections::HashMap` and `HashSet` in simulation state. Use `BTreeMap`, or a `FxHashMap` that is never iterated for logic; add an explaining comment where one is allowed;
   - ban `std::time::Instant` and `SystemTime` inside the engine (ENG-1, no I/O or wall clock).
2. Note in CONTRIBUTING why these bans exist.

- **Done when:** clippy flags a planted violation in a test build.

### T3.7.4 Seed lists

**Type:** Build · **Depends on:** T3.7.2

1. `SeedList::new(master: u64, n)` gives a deterministic list whose first k entries are the same for any n ≥ k. Adaptive stages (16 → 64 → 256, §13.4) then share prefixes, which gives common random numbers (ENG-42).

- **Done when:** a unit test confirms the prefix property.

### T3.7.5 Determinism tests

**Type:** Test · **Depends on:** T3.7.2–T3.7.4, WP3.8

1. Hash the full `RunResult` and assert it is identical across two runs with the same seed (ENG-2), and across thread counts in the harness (`RAYON_NUM_THREADS` of 1 vs 8).
2. Add a CRN test: change slot 2's build and assert that foe 1's sequence of hit rolls against slot 1 is unchanged over the first N attacks (ENG-41, §17.1 "Determinism").

- **Done when:** both pass.

---

## WP3.8 Run harness

**Goal:** evaluate a party in a situation over many seeded runs in parallel, and aggregate the results with confidence intervals. Keep adding runs until the result is statistically stable (D5).

- **Refs:** §7.3 (`RunResult`, `Evaluation`), §10.14, §13.4, §14 #2, D5.
- **Depends on:** WP3.1, WP3.7.
- **Done when:** `evaluate` returns aggregates with Wilson and mean confidence intervals, gives the same answer at any thread count, and stops by the stability rule.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T3.8.1 | `RunResult` and per-run statistics | Build | WP3.5 | Done |
| T3.8.2 | Parallel evaluation | Build | T3.8.1 | Done |
| T3.8.3 | Aggregates and confidence intervals | Build | T3.8.2 | Done |
| T3.8.4 | Stop-when-stable rule | Build | T3.8.3 | Done |
| T3.8.5 | Harness tests | Test | T3.8.4 | Done |

### T3.8.1 `RunResult` and per-run statistics

**Type:** Build · **Depends on:** WP3.5

1. Add `RunResult` following §7.3:
   - seed and outcome;
   - clear time, deaths, DP at the end;
   - per slot: damage dealt, taken, healing done and received, energy spent and gained, skills used, interrupts landed;
   - per skill: uses, damage, healing, mitigation;
   - per foe: time to kill;
   - energy samples every second (for report 4);
   - the assumptions touched and draft skills used;
   - an optional log and replay handle.
2. Store the statistics in fixed-size arrays sized at setup, not in maps.

- **Done when:** a dummy fight fills every field.

### T3.8.2 Parallel evaluation

**Type:** Build · **Depends on:** T3.8.1

1. Add `evaluate(party, situation, seeds: &SeedList, opts, data) -> Evaluation`.
2. Build the fight setup once. Run each seed as an independent `rayon` task on a cloned `Sim`.
3. Collect results into a vector in seed order, so aggregation is independent of scheduling.
4. Add `rayon` to the workspace dependencies. It is used by the harness, not inside a run (§10.14).

- **Done when:** 1,000 dummy runs complete and the results are in seed order.

### T3.8.3 Aggregates and confidence intervals

**Type:** Build · **Depends on:** T3.8.2

1. Win rate with a Wilson 95% interval.
2. For clear time (over wins), deaths, damage taken, energy left and DP: mean, median, 95% interval of the mean (t or normal approximation), and minimum and maximum.
3. Per-slot and per-skill sums and means.

- **Done when:** Wilson for 95 wins in 100 gives about [0.888, 0.978], and the mean interval matches a hand calculation on a fixed sample.

### T3.8.4 Stop-when-stable rule

**Type:** Build · **Depends on:** T3.8.3

1. Run in batches (default 32). Stop when both:
   - the win-rate interval's half-width is ≤ 2.5 percentage points (or the rate is 0 or 1 with n ≥ 64);
   - the clear-time mean's interval half-width is ≤ 2% of the mean.
2. Also stop at `max_runs` (default 1,024). Record which condition stopped the run.
3. These defaults are [Proposed]. Add them to DESIGN.md §10.13 or §13.4 when implemented.

- **Done when:** a deterministic scenario (always a win at the same time) stops after the minimum number of batches, and a noisy one runs more.

### T3.8.5 Harness tests

**Type:** Test · **Depends on:** T3.8.4

1. Test that aggregates are identical at 1 and N threads.
2. Test that the stop rule behaves as specified.
3. Test Wilson intervals at the edges (0/n and n/n).

- **Done when:** the tests pass.

---

## WP3.9 Combat log

**Goal:** a timestamped event log for any seeded run. It is recorded only when requested, so it costs nothing otherwise, and is written as JSON Lines and human-readable text.

- **Refs:** §14 #5, §14.3, §10.14.
- **Depends on:** WP3.4 to WP3.6 (the event sources).
- **Done when:** a fixed-seed dummy fight gives a stable snapshot log, and a run with logging on returns the same `RunResult` as one with it off.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T3.9.1 | Log sink | Build | — | Done |
| T3.9.2 | Emission points | Build | T3.9.1 | Done |
| T3.9.3 | JSON Lines and text writers | Build | T3.9.2 | Done |
| T3.9.4 | Re-simulation API | Build | T3.9.3, WP3.8 | Done |
| T3.9.5 | Log tests | Test | T3.9.4 | Done |

### T3.9.1 Log sink

**Type:** Build · **Depends on:** —

1. Add a `LogSink` trait with `record(&mut self, ev: &LogEvent)`, and a `NoLog` implementation.
2. `Sim<L: LogSink>` is generic over the sink, so a run without logging compiles the calls away. Alternatively use an `Option` check; measure which is faster in WP4.11.
3. `LogEvent { t_ms, kind, source, target, skill, amount, flags }` (§14.3).

- **Done when:** it compiles with both sinks.

### T3.9.2 Emission points

**Type:** Build · **Depends on:** T3.9.1

1. Emit an event for each of these:
   - skill start, end, interrupt, cancel, fail and fizzle (with the `Invalid` reason where relevant);
   - auto-attacks, and hits, misses, blocks and crits;
   - damage (type, armor used, mitigation);
   - heals, energy changes and adrenaline;
   - effects applied, removed and expired;
   - conditions;
   - knockdowns;
   - deaths and kills;
   - movement starts and stops (verbose level only);
   - controller decisions (verbose level only).

- **Done when:** a dummy fight's log contains each kind that occurred.

### T3.9.3 JSON Lines and text writers

**Type:** Build · **Depends on:** T3.9.2

1. The JSON Lines writer emits one object per line.
2. The text formatter produces lines like `  12.350s  Player → Dummy 1  Energy Surge  hit for 84 (energy −12)`. Names are resolved through the data set; the time column is fixed-width.

- **Done when:** the snapshot tests pass for both formats.

### T3.9.4 Re-simulation API

**Type:** Build · **Depends on:** T3.9.3, WP3.8

1. `resimulate(party, situation, seed, sink)` re-runs one seed with logging on (§15 `gwsim log`). The CLI arrives in T4.9.6.

- **Done when:** re-simulating run *n* reproduces its `RunResult` exactly.

### T3.9.5 Log tests

**Type:** Test · **Depends on:** T3.9.4

1. Take an `insta` snapshot of a 20 s fixed-seed dummy fight's text log. The snapshot is the §17.1 "scenario snapshot"; changes must be reviewed.
2. Test that results are equal with logging on and off.

- **Done when:** both pass.

---

## WP3.10 Training dummies, the 8 M0 skills and `PlanAi` v0 → **M0**

**Goal:** the player's Energy Surge bar runs against training dummies with hand-checked values. This proves the engine end to end on real skills before M1's volume of content.

- **Refs:** §11.5, §12.2, §20.1 (player row), §20.4, D23, D31, A-012, A-020, A-033.
- **Depends on:** WP3.1 to WP3.9; T2.6.5 (seeded numbers), or hand entry if P2 is late.
- **Done when:** the M0 criteria in §19 hold (see "M0 acceptance" below).

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T3.10.1 | The 8 M0 skills and M0 hand calculations | Research | — | Done |
| T3.10.2 | Dummies, minimal encounter and `dummies-hm` | Data | WP3.3 | Done |
| T3.10.3 | Encode the 8 player skills | Data | T3.10.1, T2.6.5 or hand entry | Done |
| T3.10.4 | Handlers: Arcane Echo, Air of Superiority, Mistrust | Build | T3.10.3, T3.6.8 | Done |
| T3.10.5 | `PlanAi` v0 | Build | T3.1.5 | Done |
| T3.10.6 | Minimal `gwsim evaluate` | Build | T3.10.2, WP3.8 | Done |
| T3.10.7 | M0 acceptance tests | Test | T3.10.3–T3.10.6 | Done |
| T3.10.8 | Owner review of the M0 encodings | Review | T3.10.7 | Owner (F3.9) |

### T3.10.1 The 8 M0 skills and M0 hand calculations

**Type:** Research · **Depends on:** —

1. For Arcane Echo, Energy Surge, Mistrust, Unnatural Signet, Cry of Frustration, Spiritual Pain, Power Drain and Air of Superiority, read each wiki page in full (description, notes, related updates) and the §20.4 changes. Record in our own words the exact behaviour to encode, including:
   - Energy Surge's damage per point of energy lost (7 after 2026-06-24) and its 75% to other foes;
   - Mistrust's PvE values (10…66…80), and the note that the target also takes only 75%;
   - Cry of Frustration's new recharge (20 s) and 75% AoE;
   - what Arcane Echo does to the skill bar, and for how long;
   - Air of Superiority's trigger ("kills that give experience") and its random Asura benefits, with each outcome and its probability, if the wiki gives them;
   - how the Asura title rank sets Air of Superiority's values (A-020, T1.1.1).
2. Using the player's effective ranks (Domination 16, Fast Casting 11, Inspiration 9; T1.4.1) and a dummy spec (level 26, a fixed energy such as 40, armor 60, fixed health), compute by hand for each damage or energy skill the expected results for one use:
   - energy removed;
   - damage to the target and to an adjacent dummy;
   - energy gained;
   - activation and recharge after Fast Casting.

   Show the working with each formula's source.

- **Output:** `docs/findings/T3.10.1-m0-hand-calcs.md`.

### T3.10.2 Dummies, minimal encounter and `dummies-hm`

**Type:** Data · **Depends on:** WP3.3

1. Add a training-dummy creature definition, `data/creatures/dummies.ron` (schema extended as needed): stationary, no skills, no attack, configurable level, armor, health and energy, with a `gives_experience` flag (so Air of Superiority can trigger), fleshy.
2. Add a generic encounter, `data/encounters/generic/training-dummies.ron`: a group of 3 dummies in a `Cluster`, with one at an adjacent distance from the first, for AoE tests.
3. Add a situation, `data/situations/dummies-hm.ron`: HM, party size 1 for M0, timeout 180 s.
4. Add minimal engine support to spawn an encounter's groups at their positions. Full layout support arrives in WP4.8.

- **Done when:** the situation loads and spawns its dummies.

### T3.10.3 Encode the 8 player skills

**Type:** Data · **Depends on:** T3.10.1, T2.6.5 or hand entry

1. Take the numbers from the seeded files. If P2 isn't ready, hand-write the numbers part from the wiki pages, with the same provenance fields, and let `diff` compare them later (T2.6.5).
2. Write each skill's encoding:
   - DSL effects and effect definitions;
   - a `handler` for Arcane Echo, Air of Superiority and Mistrust (per §8.5);
   - AI hints;
   - role tags.
3. Set the status to `Draft`.
4. Check each with `gwsim data describe` against the wiki page.

- **Done when:** `gwsim data validate` passes, and every skill renders a description.

### T3.10.4 Handlers: Arcane Echo, Air of Superiority, Mistrust

**Type:** Build · **Depends on:** T3.10.3, T3.6.8

1. Implement each handler per T3.10.1, with `describe` and unit tests:
   - `arcane_echo`: the skill-bar behaviour described on its page;
   - `air_of_superiority`: an `OnKill` trigger (experience-giving kills only) with random outcomes drawn from the skill-chance stream;
   - `mistrust`: its trigger and area damage with the 75% rule.
2. Where a probability or outcome isn't documented, add an assumption instead of a literal.

- **Done when:** each handler's tests pass.

### T3.10.5 `PlanAi` v0

**Type:** Build · **Depends on:** T3.1.5

1. Add a `PriorityPlan { rules: Vec<Rule { when: Cond, skill: SlotRef, target: TargetRule }>, maintain: Vec<SlotRef>, default: DefaultAction }` type in RON (§11.5).
2. The controller evaluates the maintenance rules first, then the rules in order. The first usable one wins; otherwise the default action runs (auto-attack the current target).
3. A human reaction delay (A-012) applies to each new decision.
4. Hand-write the M0 player plan as `data/…/plans/m0-player.ron` (the location is settled in T4.6.2). It follows the §11.5 example:
   - keep Air of Superiority up;
   - Arcane Echo, then Energy Surge on the foe with the most energy in range;
   - Mistrust on a caster;
   - Unnatural Signet on hexed or enchanted targets;
   - Cry of Frustration and Power Drain as interrupts on spells (dummies don't cast, so these fall through);
   - Spiritual Pain on a target with nearby foes.

   Generation from the build arrives in WP4.6.

- **Done when:** the player uses its skills in plan order against dummies.

### T3.10.6 Minimal `gwsim evaluate`

**Type:** Build · **Depends on:** T3.10.2, WP3.8

1. `gwsim evaluate --party <file> --situation <id|file> [--runs N] [--seed S]` prints the win rate, clear time and per-skill uses and damage.
2. The party file for now is a single slot with a template code, runes and insignias, and the plan. WP4.9 completes the command and its reports.

- **Done when:** the M0 party evaluates against `dummies-hm`.

### T3.10.7 M0 acceptance tests

**Type:** Test · **Depends on:** T3.10.3–T3.10.6

1. Write `crates/gwsim-cli/tests/m0.rs`:
   - **Determinism:** the same seed gives an identical `RunResult` hash, at 1 and N threads.
   - **Values:** for each of Energy Surge, Mistrust, Unnatural Signet, Cry of Frustration, Spiritual Pain and Power Drain, a scripted single use on the dummy spec reproduces the T3.10.1 hand calculations exactly (damage, energy, activation and recharge).
   - **Behaviour:** Arcane Echo's bar behaviour matches T3.10.1; Air of Superiority fires only on an experience-giving kill and applies one outcome.
   - **Formulas:** the whole `wiki_examples` suite (WP1.4 and WP3.5) passes.

- **Done when:** all pass.

### T3.10.8 Owner review of the M0 encodings

**Type:** Review · **Depends on:** T3.10.7 · **Owner:** reviews

1. Show the owner each skill's `gwsim data describe` output, its wiki link and the relevant lines of a fixed-seed combat log.
2. On approval, set `review: Reviewed` and `reviewed_by: Some("owner")`.

- **Done when:** the 8 skills are `Reviewed`, or have a fix list.

### M0 acceptance

| Criterion (§19) | Evidence |
| --- | --- |
| The player bar runs against dummies deterministically | T3.10.7 determinism test; T3.7.5 |
| Energy Surge, Mistrust, Unnatural Signet, Cry of Frustration, Spiritual Pain and Power Drain match hand calculations from the wiki formulas | T3.10.7 value tests against T3.10.1 |
| Arcane Echo and Air of Superiority behave as described | T3.10.7 behaviour tests; T3.10.8 review |
| Formula tests pass | The `wiki_examples` suites in `gwsim-data` and `gwsim-engine` |
