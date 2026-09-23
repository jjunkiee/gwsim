# gwsim

**gwsim** helps Guild Wars 1 / Guild Wars Reforged players find the most effective builds
for their own character and their party — heroes, henchmen and other players — in PvE. It
has two halves: an **evaluator**, which simulates a party in a situation many times and
reports how well it does, and an **optimiser**, which searches the space of builds for the
ones that do best across a weighted set of situations and shows the trade-offs between
competing goals.

> **Status: pre-alpha. Not usable yet.**
> The design is complete and implementation has just begun. There is nothing to run beyond
> `--version`. See the [plan of work](docs/plan/README.md) for what exists and what does
> not.

- **[Design document](docs/DESIGN.md)** — the full design, and the source of truth.
- **[Plan of work](docs/plan/README.md)** — phases, work packages and tasks.

gwsim runs fully offline: all game knowledge ships with the application.

## Getting started (Windows 11)

Windows is the supported platform. You need about 5 GB of free disk space.

### 1. Prerequisites

Three things: the Microsoft C++ build tools (Rust needs their linker), rustup, and Git.

#### Using winget (recommended)

Run these in PowerShell. The first one triggers a **UAC prompt** and takes a while — it is
a multi-gigabyte download. The other two do not need administrator rights.

```powershell
winget install --id Microsoft.VisualStudio.BuildTools --source winget --override "--quiet --wait --norestart --nocache --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.26100"
winget install --id Rustlang.Rustup --source winget
winget install --id Git.Git --source winget
```

Note that the two `--add` component lines are not optional decoration. In the Visual Studio
installer, the MSVC compiler and the Windows SDK are *recommended* components of the C++
workload rather than *required* ones, so installing the bare workload leaves you without a
linker — the cause of the `link.exe not found` error below.

#### Installing manually instead

1. Download the **Build Tools for Visual Studio** from
   [visualstudio.microsoft.com/downloads](https://visualstudio.microsoft.com/downloads/)
   (under "Tools for Visual Studio"). Run it, tick **Desktop development with C++**, and
   before you click Install check that **MSVC … build tools** and a **Windows 11 SDK** are
   ticked in the summary pane on the right.
2. Download and run `rustup-init.exe` from [rustup.rs](https://rustup.rs). Accept the
   default option 1 — stable, `x86_64-pc-windows-msvc`, default profile. That profile
   already includes `rustfmt` and `clippy`, which the project needs.
3. Install Git from [git-scm.com/download/win](https://git-scm.com/download/win).

#### Then open a new terminal

rustup adds `%USERPROFILE%\.cargo\bin` to your `PATH`, and existing terminals will not see
it. Close PowerShell and open it again, then check:

```powershell
rustc --version
cargo --version
```

Both should print a version of **1.98.1 or newer**. (1.93 is the true minimum: earlier
versions cannot find the 2026 build tools.)

### 2. Clone

```powershell
git clone <repository URL>
cd gwsim
```

*Placeholder: this repository has no public remote yet.*

### 3. Build

```powershell
cargo build --release
```

The first build downloads and compiles all dependencies and will take several minutes.
Later builds are much faster.

### 4. First run

```powershell
cargo run --release -p gwsim-cli -- --version
```

Every command below is written this way, because `gwsim` is a workspace binary
rather than something on your `PATH`. If you would rather type `gwsim` directly:

```powershell
cargo install --path crates/gwsim-cli
gwsim --version
```

This is where the interesting command lives. Since milestone M1 it runs the 7-hero
Mesmerway party against a hard-mode Kournan patrol in Vehtendi Valley:

```powershell
cargo run --release -p gwsim-cli -- evaluate --party data/parties/m1-mesmerway.ron --situation kournan-patrol-hm
```

It prints each slot's bar with its template codes, then the win rate, clear time, deaths,
damage taken and energy left, each with a 95% interval. It keeps adding runs until the
result is stable. The output ends with notes: which assumptions the result rests on, which
skills are still `Draft`, and which data pack produced it. An abridged sample:

```text
gwsim evaluate: M1 Mesmerway
  Absolute numbers are uncalibrated: compare builds with each other under the same conditions, not with the game (D22).
  ...
Kournan patrol HM [kournan-patrol-hm]
  runs:          64 (Stable)
  win rate:      100.0  (94.3 to 100.0) %   64 of 64 won
  clear time:    43.4  (42.3 to 44.6) s   median 43.0 s
  deaths:        0.08  (0.01 to 0.14)
  ...
Notes
  assumptions touched:
    A-001 [Assumed] The base movement speed of characters and foes, in gwinches per second.
  ...
```

Some useful options:

- `--runs 100` fixes the number of runs, and `--seed 7` changes the seed list.
- `--set m1` runs all six M1 situations and weights them.
- `--breakdown` adds who did what: damage, healing, prevented damage and interrupts per slot and skill, plus an energy timeline.
- `--json out.json` writes the full result file, described in [docs/result-schema.md](docs/result-schema.md).
- `gwsim log --result out.json --run 3` replays one run of that file and prints its combat log.
- `gwsim compare A B --situations kournan-patrol-hm` runs two parties on the same seeds and reports the differences with paired intervals.
- `--party` also accepts up to eight comma-separated skill template codes.

The same seed always gives the same result, on any machine and at any thread count.

Since milestone M2 gwsim can also **search** for builds. This command frees the player
slot and looks for the best player bar for the Mesmerway team on all six M1 situations:

```powershell
cargo run --release -p gwsim-cli -- optimise --party data/parties/m1-mesmerway.ron --free player --situations m1 --budget 5m
```

It prints a ranked list of builds with template codes and metrics, and a frontier of
trade-offs between clear time and deaths. A few options:

- `--generations 20` in place of `--budget` makes a run exactly reproducible from its `--seed`.
- `--mode exhaustive --pool pool.ron` tries every combination from a small list of skills.
- `--profile mine` limits the search to what an account owns. Manage profiles with `gwsim profile new mine`, `gwsim profile lock mine skill energy-surge` and similar.
- `--json result.json` writes the full result.

### 5. Tests

```powershell
cargo test --workspace
```

`gwsim check` runs the game-behaviour checks, which the unit tests do not cover. These
are the relative checks of DESIGN §17.4. RC1 checks that the benchmark party beats weakened
copies of itself. RC2 checks that it beats a naive baseline team. RC6 checks monotonicity
properties.

```powershell
cargo run --release -p gwsim-cli -- check --level reduced
```

`--level full` uses the full run counts and takes several minutes. `--only RC1` runs one
check. The command writes a JSON report next to its summary.

### 6. Contributors: the wiki extractor

gwsim's game data is built from the [Guild Wars Wiki](https://wiki.guildwars.com/) by a
separate tool, `gwsim-extract`. **It is a developer tool and is not shipped with the
application** — you do not need it to use gwsim, because the data it produces is committed
to this repository.

It does three jobs:

- **`crawl`** fetches wiki pages into a local cache, slowly and resumably;
- **`seed`** writes initial data files from the cache — numbers only for skills, `Draft`
  for foes — and **never overwrites a file that exists**;
- **`diff`** compares the cache with the committed data and writes a change report. It
  never writes to `data/`.

```powershell
# The skill lists (about 20 pages, 1 minute), then the index that coverage counts against
cargo run -p gwsim-extractor -- crawl --scope skills --discovery-only
cargo run -p gwsim-extractor -- index

# Particular pages, then seed them
cargo run -p gwsim-extractor -- crawl --only "Energy Surge" "Kournan Seer"
cargo run -p gwsim-extractor -- seed --scope skills --only "Energy Surge"
cargo run -p gwsim-extractor -- seed --scope foes --only "Kournan Seer"

# What changed since seeding
cargo run -p gwsim-extractor -- diff --out report.md --json report.json
```

`crawl --dry-run` prints the URLs it would fetch and fetches nothing. `crawl` prints the page
count and an estimate before it starts: a full crawl of every skill page is about 1,350 pages,
**roughly 70 minutes**, and the change report needs one.

**The etiquette is enforced, not optional** (DESIGN §9.1, EXT-1 to EXT-7):

- `robots.txt` is fetched first and obeyed; a session stops if it cannot be read;
- only `https://wiki.guildwars.com/wiki/<Title>` article pages are ever requested — never
  `/api.php`, `/index.php` or `Special:` pages;
- at least 3 seconds between requests (`--delay` refuses anything under 2);
- the User-Agent names the project and nobody else;
- `5xx` backs off 30, 60 then 120 seconds; **`403` or `429` stops the session at once**;
- wiki text never reaches `data/`: only numbers, flags and a hash of each description.

The cache lives in `.cache/wiki/`, one `.html` and one `.meta.json` per page. It holds raw
wiki HTML, so **it is git-ignored and must never be committed**. An interrupted crawl
resumes from `.cache/crawl-state.json`; delete that file to abandon it.

**Applying a balance patch** (UC11, detailed in WP7.18): crawl the changed pages (or
everything), run `diff`, read the report — it flags changed numbers, pages that vanished,
and descriptions that changed and so need their encoding re-checked — edit the data by
hand, then re-run `gwsim data validate` and `gwsim check`. Add `--updates
"Feedback:Game updates/<date>"` to `diff`, after crawling that page, to see the update
note beside each change.

See [CONTRIBUTING.md](CONTRIBUTING.md) before you start.

### Troubleshooting

**`error: linker 'link.exe' not found`**
The C++ build tools are missing, or the workload was installed without the MSVC component.
Re-run the Build Tools step above, making sure both `--add` components are included. You do
not need a "Developer Command Prompt" — Rust finds MSVC through the registry, so an
ordinary PowerShell window is fine.

**`cargo` is not recognised**
You have not opened a new terminal since installing rustup.

**In VS Code, a new terminal tab is not enough.** Tabs inherit VS Code's own environment, so
if VS Code was running when rustup installed, every tab it opens has the old `PATH`. Restart
VS Code itself, or reload the current session in place:

```powershell
$env:PATH = [Environment]::GetEnvironmentVariable("PATH","Machine") + ";" + [Environment]::GetEnvironmentVariable("PATH","User")
```

To tell which problem you have, check whether the setting took but the session missed it:

```powershell
[Environment]::GetEnvironmentVariable("PATH","User") -like "*.cargo*"   # should be True
$env:PATH -like "*.cargo*"                                             # False means a stale session
```

If the first is `False`, rustup did not add the entry; add `%USERPROFILE%\.cargo\bin` to your
user `PATH` yourself.

**Builds are slow**
Real-time antivirus scanning of the `target/` directory is a common cause. Excluding that
directory speeds builds up noticeably — your call whether that trade-off is one you want.

## Licence

**gwsim is [GPL-3.0-or-later](LICENSE) throughout — code and game data alike.**

The data files contain **game facts** — numbers, names and relationships — expressed
originally, together with gwsim's own encodings of what skills do. They contain **no
ArenaNet text and no art**: no in-game descriptions, no icons, no screenshots. Skill
descriptions shown in the application are generated from gwsim's own data. See
[data/ATTRIBUTION.md](data/ATTRIBUTION.md) for what the data is, where the facts come from,
and what is deliberately excluded.

If you want to reuse the data in your own Guild Wars tool, note that the GPL applies to it
as much as to the code: your tool has to be GPL-compatible too.

Guild Wars, ArenaNet and NCSOFT are trademarks or registered trademarks of NCSOFT
Corporation. **gwsim is a fan project and is not affiliated with, endorsed by, or sponsored
by ArenaNet or NCSOFT.**

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Please read the licensing ground rules and the wiki
etiquette before your first contribution — they are the two areas where a well-meant change
can cause real problems.
