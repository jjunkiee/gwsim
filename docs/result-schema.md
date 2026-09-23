# Result file schema

`gwsim evaluate --json out.json` writes a result file (DESIGN §14.3, T4.9.2). `gwsim evaluate --json` with no path prints the same JSON instead of the text report. The types are in [crates/gwsim-cli/src/results.rs](../crates/gwsim-cli/src/results.rs).

**Current version: 1.** Readers must check `schema_version` first. `gwsim log` refuses a file with any other version.

## Rules

- A result file holds everything needed to re-run it: the party and the situations are stored inline, with the master seed and each run's seed. `gwsim log --result out.json --run N` re-simulates run *N* from the file alone (T4.9.6).
- It also records the data pack it was computed against (`inputs.pack`). A replay against different data refuses unless `--force` is given (ENG-43).
- Aggregates are **per-run means**, so two results with different run counts can be compared.
- Absolute numbers are uncalibrated (D22). Compare results with each other, not with the game.
- Raise `SCHEMA_VERSION` for any change a reader could trip over, and add a line to the history below.

## Top level

| Field | Type | Meaning |
| --- | --- | --- |
| `schema_version` | integer | `1` |
| `inputs` | object | What was evaluated (below) |
| `builds` | array | Report 1: one entry per slot |
| `situations` | array | Reports 2 and 4: one entry per situation, in input order |
| `set` | object or null | The weighted aggregate, when `--set` was given |
| `notes` | object | The §14.1 notes every report carries |

## `inputs`

| Field | Type | Meaning |
| --- | --- | --- |
| `party` | object | The party file, as gwsim read it (the `PartyFile` shape of `data/parties/*.ron`) |
| `situations` | array of `{slug, weight, situation}` | `slug` is null for a situation read from a file; `situation` is the full `Situation` |
| `set` | string or null | The situation set's slug |
| `seed` | integer | The master seed. Run *i* of every situation uses seed *i* of `SeedList::new(seed, …)` |
| `runs` | `{"Fixed": n}` or `"Auto"` | `Auto` means runs were added until stable (T3.8.4) |
| `reviewed_only` | bool | Whether `--reviewed-only` was given |
| `pack` | `{content_hash, baseline, gwsim_version, origin}` | The data pack's identity |

## `builds[]` (report 1)

| Field | Type | Meaning |
| --- | --- | --- |
| `slot` | string | The slot's label, such as `player` or `hero 3` |
| `kind` | string | `Human`, `Hero` or `Henchman` |
| `professions` | string | Such as `Me/Mo` |
| `skills` | array of string or null | The bar in order; null marks an empty slot |
| `skill_code` | string | The skill template code (type 14). It round-trips through `gwsim template decode` |
| `equipment` | array of string | One line per armor piece, then the weapon set |
| `equipment_code` | string or null | The equipment template code (type 15). It carries only runes and insignias with known ids. PvE characters cannot load it, and in game a hero's carries weapons only |

## `situations[]`

| Field | Type | Meaning |
| --- | --- | --- |
| `slug`, `name`, `weight` | | Which situation, and its weight in the set |
| `metrics` | object | Report 2 (below) |
| `contributions` | array | Report 4: per slot and skill, per run (below) |
| `stopped` | array of `{by, stopped, per_run}` | Which interrupt stopped which skill, per run |
| `energy` | array of `{slot, per_second}` | Energy each second, averaged over the runs still going at that second |
| `runs` | array | One summary per run (below) |

### `metrics` (report 2)

Each metric is `{n, mean, median, low, high}`, where `low` and `high` bound a 95% interval. `win_rate` uses the Wilson interval; the others use the normal approximation for the mean.

| Field | Meaning |
| --- | --- |
| `runs`, `wins` | Counts |
| `win_rate` | A fraction from 0 to 1 |
| `clear_time_s` | Seconds from aggro to the last kill, **over wins only**. For a chain, it includes the rests |
| `deaths` | Party deaths per run |
| `damage_taken` | Damage the party took, per run |
| `energy_left` | The party's total energy at the end |
| `dp_end` | Death penalty at the end, in percent |
| `stop` | `Fixed`, `Stable` or `MaxRuns` |

### `contributions[]` (report 4)

`{slot, skill, uses, damage, healing, overhealing, mitigation, interrupts}`, all per run.

- `skill` is `"attacks and other"` for anything no skill caused. That covers weapon attacks and minion attacks.
- Damage done by minions and spirits is credited to their master or caster (§14.2). Healing is split into `healing`, the health actually gained, and `overhealing`.
- `mitigation` is damage prevented by damage reductions (Shelter, Union, Armor of Unfeeling, Protective Was Kaolai, …). It is credited to the slot that cast the reduction.
- Blocks are not yet counted as prevented damage (F4.23).

### `runs[]`

`{index, seed, outcome, clear_time_ms, ended_ms, deaths, dp_end, damage_taken, energy_left, first_failed, digest}`

- `outcome` is `Win`, `Wipe` or `Timeout`.
- `first_failed` is the index of the first failed fight of a chain.
- `digest` is the engine's hash of the whole run (`RunResult::digest`, in hex). A replay that matches it reproduced the run exactly.

## `set`

`{name, total_weight, weighted_win_rate, weighted_clear_time_s}`. The weighted clear time is taken over the situations with at least one win.

## `notes` (T4.9.3)

| Field | Meaning |
| --- | --- |
| `uncalibrated` | The D22 label |
| `assumptions` | `{id, statement, status}` for every assumption any run touched |
| `drafts` | Names of skills used while still `Draft`, on either side |
| `coverage` | Coverage limits that bear on this result |
| `pack` | As in `inputs.pack` |
| `seed`, `runs` | The master seed and the runs per situation |

## `gwsim compare --json`

This is a separate shape with the same `schema_version`:

```json
{
  "schema_version": 1,
  "a": "...",
  "b": "...",
  "seed": 1,
  "bootstrap": {"resamples": 1000, "seed": 7455...},
  "situations": [{"name", "slug", "runs", "verdict", "metrics": [{"metric", "a", "b", "difference", "better"}]}],
  "notes": {...}
}
```

- `difference` is `{n, mean, ci: {low, high}}` for A − B, from a paired bootstrap over seeds.
- `better` says which direction is better: `Higher` or `Lower`.
- `verdict` is `ABetter`, `BBetter` or `NotDifferent`, by the T4.10.2 rule. A is better when the win-rate interval lies above zero. If the win rates are equal within the interval, A is better when the clear-time interval lies below zero.

## History

- **1** (T4.9.2): the first version.
