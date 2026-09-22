# P8 Weapon-set swapping

**Phase goal:** model weapon-set swapping (Q20, Q41):

- the engine supports several weapon sets per unit and a swap action;
- the player's priority plan swaps sets as a skilled player would (an energy set, then a casting set);
- the optimiser can search over several sets.

The M1 player benchmark is re-run with the PvX page's three weapon sets to measure what swapping is worth.

- **Design refs:** §7.2 (`WeaponSet`), §11.5, §13.1, §20.1 (the player row; 40/40 set), Q20, Q41.
- **Starts when:** M2 is done, and M3 or at least the desktop party editor (WP6.2) exists, so the extra sets can be edited in the app as well as in files.
- **Ends when:** the M1 player benchmark runs with three weapon sets. The result, compared with the single-set run, is recorded, and the optimiser can change the extra sets.
- **WP numbers** in this phase are assigned by this plan; the design gives none.

| WP | Title | Goal | Status |
| --- | --- | --- | --- |
| 8.1 | Swap mechanics | Know exactly what a weapon swap changes and costs in the game. | Todo |
| 8.2 | Engine support | Units carry up to four weapon sets and can swap, with stats recomputed correctly. | Todo |
| 8.3 | `PlanAi` swap rules | Human plans swap to energy and casting sets as a skilled player does. | Todo |
| 8.4 | Multi-set genome | The optimiser searches extra weapon sets. | Todo |
| 8.5 | Benchmark re-run | The value of swapping on the M1 player bar is measured. | Todo |

---

## WP8.1 Swap mechanics

**Goal:** establish from the wiki exactly what swapping weapon sets does: timing, the effect on current and maximum energy, attribute bonuses, interaction with activation and aftercast, and any restrictions.

- **Refs:** Q20, Q41.
- **Depends on:** —
- **Done when:** the findings are written, and any undocumented value has an assumption ID.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T8.1.1 | Weapon-swap mechanics | Research | — | Todo |
| T8.1.2 | The PvX weapon sets | Research | — | Todo |

### T8.1.1 Weapon-swap mechanics

**Type:** Research · **Depends on:** —

1. From the wiki pages Weapon set, Weapon, Energy, Attribute and Inscription (and any "weapon swapping" article), record:
   - how long a swap takes, and whether it can happen during activation or aftercast;
   - what happens to current energy when maximum energy changes (swapping to a set with less energy);
   - whether attribute mods ("+1 (20% chance)", or "Of the Profession") apply per activation from the held set;
   - whether swapping interrupts or cancels anything;
   - whether HCT and HSR mods apply at activation start from the set held at that moment;
   - any documented swap-based techniques and their exact effects (e.g. swapping to a high-energy set before a large energy cost).

- **Output:** `docs/findings/T8.1.1-weapon-swap.md`, with assumptions added for any gaps.

### T8.1.2 The PvX weapon sets

**Type:** Research · **Depends on:** —

1. From the §20.1 Wayback snapshot of "Me/any PvE Energy Surge", record the three weapon sets the page recommends (Q41), with their mods and purposes.
2. Freeze them in the player benchmark file as an optional extension (D16).

- **Done when:** the sets are recorded, with the snapshot URL.

---

## WP8.2 Engine support

**Goal:** units hold up to four weapon sets and can swap between them. Maximum energy, attribute bonuses, attack stats and chance mods are recomputed exactly as T8.1.1 found.

- **Refs:** §7.2, §10.4.
- **Depends on:** WP8.1.
- **Done when:** swap tests pass for the energy change, the attribute change, and the timing rules.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T8.2.1 | Several weapon sets in `Build` and data | Build | T8.1.1 | Todo |
| T8.2.2 | The swap action | Build | T8.2.1 | Todo |
| T8.2.3 | Swap tests | Test | T8.2.2 | Todo |

### T8.2.1 Several weapon sets in `Build` and data

**Type:** Build · **Depends on:** T8.1.1

1. Change `weapon_set` to `weapon_sets: Vec<WeaponSet>` (1 to 4) with an `active: usize`, keeping backwards compatibility with single-set files (a single set is read as a list of one).
2. Update the legality rules, the template mapping (equipment templates hold only the currently wielded set, per the wiki) and the derived stats (computed per set).

- **Done when:** existing data and tests are unchanged, and multi-set builds validate.

### T8.2.2 The swap action

**Type:** Build · **Depends on:** T8.2.1

1. `Order::SwapWeapons(set)` carries out the swap with the timing and restrictions from T8.1.1.
2. It recomputes the stats through `StatBlock`, applies the energy rules and logs a `WeaponSwap` event.
3. Chance mods and attribute bonuses read the set that is active at each activation.

- **Done when:** the order works in a scripted test.

### T8.2.3 Swap tests

**Type:** Test · **Depends on:** T8.2.2

1. Test each T8.1.1 rule: the energy on a swap to a lower-energy set, the attribute bonus applied or not, a swap during activation, and chance mods using the held set.

- **Done when:** the tests pass.

---

## WP8.3 `PlanAi` swap rules

**Goal:** generated player plans swap sets as a skilled player does. For example, they hold a high-energy set while regenerating and swap to a casting set with the right attribute mod before casting, following the T8.1.1 techniques. Users can edit the rules.

- **Refs:** §11.5, A-029.
- **Depends on:** WP8.2.
- **Done when:** the generated plan for the player bar includes swap rules, and they execute in the log as intended.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T8.3.1 | Swap rules in the plan format | Build | WP8.2 | Todo |
| T8.3.2 | Generate the swap rules | Build | T8.3.1 | Todo |
| T8.3.3 | Swap-plan tests and review | Test / Review | T8.3.2 | Todo |

### T8.3.1 Swap rules in the plan format

**Type:** Build · **Depends on:** WP8.2

1. Add a swap rule to `PriorityPlan`: "before using a skill of type or attribute X, hold set N", plus "while idle, hold set M". This keeps it editable in RON.

- **Done when:** the rules round-trip and validate.

### T8.3.2 Generate the swap rules

**Type:** Build · **Depends on:** T8.3.1

1. From the sets' properties (energy, attribute mods, HCT and HSR), derive:
   - an idle or energy set;
   - a casting set per attribute used on the bar.
2. Generate the rules only when a swap is worth doing under T8.1.1's rules.

- **Done when:** the generator produces the expected rules for the three PvX sets.

### T8.3.3 Swap-plan tests and review

**Type:** Test / Review · **Depends on:** T8.3.2 · **Owner:** reviews the plan

1. Snapshot the generated plan, and check the log excerpts showing the swaps.
2. The owner confirms the plan matches skilled play.

- **Done when:** the owner approves.

---

## WP8.4 Multi-set genome

**Goal:** the optimiser can search extra weapon sets, keeping the search space manageable.

- **Refs:** §13.1, §13.3, Q20.
- **Depends on:** WP8.3, P5.
- **Done when:** an optimisation with multi-set search enabled produces legal multi-set builds, and a property test holds.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T8.4.1 | Genome and operators | Build | WP8.3 | Todo |
| T8.4.2 | Multi-set tests | Test | T8.4.1 | Todo |

### T8.4.1 Genome and operators

**Type:** Build · **Depends on:** WP8.3

1. Extend `FreeGenome` with 1 to 4 sets.
2. Add three mutations: add a set, remove a set, and change a set's weapon or mods.
3. Repair removes useless sets: a set that no swap rule would ever use.
4. Turn it on with `--weapon-sets N`, off by default so older runs stay comparable.

- **Done when:** repair keeps multi-set genomes legal.

### T8.4.2 Multi-set tests

**Type:** Test · **Depends on:** T8.4.1

1. Property tests: legality, and locked sets unchanged.
2. A small optimisation run with multi-set search enabled.

- **Done when:** the tests pass.

---

## WP8.5 Benchmark re-run

**Goal:** measure what weapon swapping is worth on the M1 player bar, using the PvX page's three sets (Q41).

- **Refs:** §20.1, Q41.
- **Depends on:** WP8.3, T8.1.2.
- **Done when:** the results are recorded, and the single-set and three-set runs are compared on the M1 set.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T8.5.1 | Run and compare | Run | WP8.3, T8.1.2 | Todo |

### T8.5.1 Run and compare

**Type:** Run · **Depends on:** WP8.3, T8.1.2

1. Run `gwsim compare` with the player on one set (the 40/40 Domination set) against the player on three sets with generated swap rules, on the M1 set, paired seeds and full run counts.
2. Optionally, run O1 again with multi-set search enabled.
3. Record the metrics, the differences and the observations.

- **Output:** `docs/findings/T8.5.1-weapon-swap-benchmark.md`.
- **Done when:** the comparison is recorded. This closes P8.
