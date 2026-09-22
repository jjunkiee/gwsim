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

From milestone M1 onwards, this is where the interesting command lives:

```powershell
# Available from M1 — does not work yet
gwsim evaluate --party data/parties/mesmerway-dual-resto.ron --situation kournan-patrol-hm
```

### 5. Tests

```powershell
cargo test --workspace
```

`gwsim check`, which runs the game-behaviour checks rather than the unit tests, arrives in
M1.

### 6. Contributors: the wiki extractor

gwsim's game data is built from the [Guild Wars Wiki](https://wiki.guildwars.com/) by a
separate tool, `gwsim-extract`. **It is a developer tool and is not shipped with the
application** — you do not need it to use gwsim, because the data it produces is committed
to this repository.

If you do run it, the crawl etiquette is not optional: article pages only, at least three
seconds between requests, an honest User-Agent, and stop immediately on `403` or `429`. A
full crawl takes roughly 3.3 hours. See [CONTRIBUTING.md](CONTRIBUTING.md) before you start.

*This section will be expanded once the extractor exists.*

### Troubleshooting

**`error: linker 'link.exe' not found`**
The C++ build tools are missing, or the workload was installed without the MSVC component.
Re-run the Build Tools step above, making sure both `--add` components are included. You do
not need a "Developer Command Prompt" — Rust finds MSVC through the registry, so an
ordinary PowerShell window is fine.

**`cargo` is not recognised**
You have not opened a new terminal since installing rustup. If a fresh terminal still fails,
check that `%USERPROFILE%\.cargo\bin` is in your `PATH`.

**Builds are slow**
Real-time antivirus scanning of the `target/` directory is a common cause. Excluding that
directory speeds builds up noticeably — your call whether that trade-off is one you want.

## Licence

gwsim is split into two differently licensed parts.

| Part | Licence |
| --- | --- |
| **Code** (`crates/`, and everything not under `data/`) | [GPL-3.0-or-later](LICENSE) |
| **Game data** (`data/`) | [CC BY-SA 4.0](data/LICENSE) |

The data files contain **game facts** — numbers, names and relationships — expressed
originally, together with gwsim's own encodings of what skills do. They contain **no
ArenaNet text and no art**: no in-game descriptions, no icons, no screenshots. Skill
descriptions shown in the application are generated from gwsim's own data. See
[data/ATTRIBUTION.md](data/ATTRIBUTION.md) for the details and for how to credit this data
if you reuse it.

Guild Wars, ArenaNet and NCSOFT are trademarks or registered trademarks of NCSOFT
Corporation. **gwsim is a fan project and is not affiliated with, endorsed by, or sponsored
by ArenaNet or NCSOFT.**

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Please read the licensing ground rules and the wiki
etiquette before your first contribution — they are the two areas where a well-meant change
can cause real problems.
