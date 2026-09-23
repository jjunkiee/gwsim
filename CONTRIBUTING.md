# Contributing to gwsim

Thanks for your interest. gwsim is pre-alpha: the design is settled, and the work is now
following a written plan.

## Start here

1. **[README](README.md)** — getting a build running on Windows.
2. **[docs/DESIGN.md](docs/DESIGN.md)** — the design, and the source of truth. If code and
   design disagree, one of them is a bug.
3. **[docs/plan/README.md](docs/plan/README.md)** — the plan of work. Every change should
   trace back to a task ID such as `T1.2.3`. If what you want to do has no task, say so in
   an issue first; the plan can be extended.

## Ground rules

These two areas are where a well-meant contribution can cause real problems. Please read
them even if you skip the rest.

### 1. Licensing

**gwsim is GPL-3.0-or-later throughout**, and that includes the game data in `data/`. There
is one licence and one `LICENSE` file.

**Inbound equals outbound.** By contributing — code or data — you agree your contribution
is licensed under GPL-3.0-or-later.

**Never commit:**

- in-game description text, or any other text written by ArenaNet;
- skill icons, screenshots, or any other ArenaNet art — no images, in any form;
- text written by wiki editors, copied or lightly reworded;
- anything from the extractor's `.cache/` directory;
- text from the wiki's `Feedback:` namespace.

**Why the prose rule is strict.** The Guild Wars Wiki's editor text is licensed under the
**GFDL**, which is not compatible with GPL-3.0-or-later. So wiki prose cannot legally be
relicensed into this repository, however convenient it would be.

What we take instead is **facts** — numbers, names and relationships — expressed
originally. This mirrors the wiki's own policy for material from incompatible sources:
factual information may be used only if it is expressed originally, and that allowance does
not extend to images. Skill descriptions shown by gwsim are *generated* from our own effect
encodings, never transcribed.

See [data/ATTRIBUTION.md](data/ATTRIBUTION.md) and
[docs/findings/T0.5.2-data-licence.md](docs/findings/T0.5.2-data-licence.md).

### 2. Wiki etiquette

This applies to anyone running `gwsim-extract` **and to anyone doing research by hand**.
The wiki is a volunteer-run service doing us a favour.

| Rule | |
| --- | --- |
| **EXT-1** | Obey `robots.txt`. Never request `/api.php`, `/index.php` or `Special:` pages. |
| **EXT-2** | Only `https://wiki.guildwars.com/wiki/<Title>` article URLs. |
| **EXT-3** | At least **3 seconds** between requests. Never below 2, whatever the flag says. |
| **EXT-4** | An honest User-Agent identifying the tool and linking the project. **No personal details, and no email addresses.** |
| **EXT-5** | Cache locally in `.cache/wiki/`, so re-runs do not re-fetch. |
| **EXT-6** | Back off on 5xx. **Stop on 403 or 429** and report it. Never retry around a block. |
| **EXT-7** | Never put wiki prose in `data/`. |

A full crawl takes about 3.3 hours. Do not run one casually, and do not parallelise it.

If you get a blanket 403 on every page, check your network path before assuming you have
been blocked — a VPN exit IP will do it.

## Data contributions

Every data file carries a `provenance` block naming its source pages and the date read, and
a review status:

| Status | Meaning |
| --- | --- |
| `NumbersOnly` | Values taken from the wiki. Nobody has checked the behaviour. |
| `Draft` | An effect encoding exists, but has not been reviewed. |
| `Reviewed` | A person has checked the encoding against the game's actual behaviour. |

Only raise a status when you have genuinely done the checking — an incorrectly `Reviewed`
skill is worse than an honest `Draft`, because it stops anyone looking again. Wiki articles
are not reviewed by the game's developers and are sometimes wrong, outdated, or internally
inconsistent; treat their numbers as claims.

Anything you could not confirm becomes a registered **assumption** rather than a confident
value.

*A `docs/data-authoring.md` guide will be added in T1.2.11.*

## Code contributions

**Definition of done** for a code change — all four must pass locally before you open a
pull request:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
gwsim data validate        # once it exists (T1.2.12)
```

Also:

- **Use the wiki's names in code.** A skill called "Energy Surge" on the wiki is
  `EnergySurge` here. Do not invent tidier names; matching the wiki is what makes the data
  reviewable.
- **No magic numbers for game values.** Anything that came from the wiki belongs in `data/`
  with provenance, not as a literal in Rust (ENG-4). Values that are genuinely ours —
  buffer sizes, thresholds — are fine as constants.
- **Register new assumptions.** If you had to guess at game behaviour, add it to the
  assumptions register with an ID instead of burying the guess in a comment.
- **`unsafe` is forbidden** workspace-wide.
- The engine crate does **no I/O** (ENG-1).

### Determinism

The same seed must give the same fight on every machine, at any thread count. That is
what makes results reproducible and what lets the optimiser compare two builds on the same
luck (ENG-42). `crates/gwsim-engine/clippy.toml` and `crates/gwsim-opt/clippy.toml` ban
the usual ways it breaks:

- **ambient randomness** (`rand::thread_rng`, `rand::random`): draw from a stream in
  `gwsim_engine::rng`, which is derived from the run seed, a purpose and a unit;
- **`HashMap` and `HashSet`**: their iteration order changes between runs. Use
  `BTreeMap` and `BTreeSet`. If a hash map is genuinely needed for speed, it must never be
  iterated for logic; allow the lint on that one line and say why in a comment;
- **the wall clock** (`Instant`, `SystemTime`): simulated time is `SimTime`, in whole
  milliseconds.

Floating point is fine: the engine uses it in a fixed order, and results are compared by
digest in the determinism tests (`crates/gwsim-engine/tests/determinism.rs`).

## Commits and pull requests

- A branch per work package, named after it: `setup/p0`, `data/wp1.2`.
- Small, focused commits. A commit that changes data and engine behaviour together is hard
  to review and harder to revert.
- Reference the task ID in the commit message.
- Fill in the pull request template, including the licensing checklist.
