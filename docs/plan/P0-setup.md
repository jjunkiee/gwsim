# P0 Setup

**Phase goal:** give the empty repository a Rust workspace that builds and tests on the development machine. The toolchain is installed by following the README's own instructions. Local-only material (`research/`, `.cache/`) is protected from commits. Licences and contributor scaffolding are in place, so P1 can start writing code.

- **Design refs:** §2.2 (C2, C5), §6.1, §8.10, §18, Q38, Q42, D9, D20, D21.
- **Starts when:** the design is agreed (done 2026-09-22).
- **Ends when:**
  - `cargo build` and `cargo test` pass;
  - `gwsim --version` prints a version;
  - `git status` doesn't list `research/`;
  - the licence files, CONTRIBUTING and the CI workflow exist;
  - the README's Getting Started section has been followed successfully on the development machine.
- **Suggested order:**
  1. WP0.4 first, so nothing local is ever committed.
  2. WP0.5.
  3. WP0.1, then WP0.2. This order is deliberate: installing the toolchain by following the README is the README's first test.
  4. WP0.3.
  5. WP0.6.

| WP | Title | Goal | Status |
| --- | --- | --- | --- |
| 0.1 | README Getting Started | A newcomer can go from a bare Windows 11 machine to a passing build using only the README. | In progress |
| 0.2 | Toolchain on the dev machine | Rust (MSVC) works on the development machine, installed exactly as the README says. | Done |
| 0.3 | Workspace skeleton | The six-crate workspace builds, tests and runs `gwsim --version`. | Done |
| 0.4 | `.gitignore` | Local-only and generated files can't be committed by accident. | Done |
| 0.5 | Licences | Code and data licensing are unambiguous and match C2 and Q38. | Done |
| 0.6 | CONTRIBUTING and CI | Contributors know the rules, and CI is ready for when a remote exists. | Done |

---

## WP0.1 README Getting Started

**Goal:** someone with a fresh Windows 11 machine can install the prerequisites, clone, build, run and test gwsim using only the README.

- **Refs:** §18.2, C5, D21.
- **Depends on:** —
- **Done when:** a clean machine can follow it. The first check is WP0.2 on the development machine. The clean-machine check is T6.10.6.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T0.1.1 | Confirm current Windows install steps | Research | — | Done |
| T0.1.2 | Write the README | Docs | T0.1.1 | Done |
| T0.1.3 | Owner reads the README | Review | T0.1.2 | Todo |

### T0.1.1 Confirm current Windows install steps

**Type:** Research · **Depends on:** —

1. Read the official rustup Windows installation page (rustup.rs and the rustup book's "Windows" / "MSVC prerequisites" pages). Record:
   - whether `rustup-init.exe` still offers to install the Visual Studio prerequisites itself;
   - which host triple it defaults to (`x86_64-pc-windows-msvc` is expected);
   - which profile it uses by default, and whether that includes `rustfmt` and `clippy`.
2. Find out which Visual Studio Build Tools edition rustup currently recommends (2022, or a newer edition if one has shipped), and record:
   - the workload ID for "Desktop development with C++" (`Microsoft.VisualStudio.Workload.VCTools` is expected);
   - whether the Windows SDK comes with `--includeRecommended`.
3. Find the `winget` package IDs and verify each command's syntax, including the `--override` needed to pass the workload to the Build Tools installer:
   - Build Tools;
   - rustup (`Rustlang.Rustup` is expected);
   - Git (`Git.Git`).
4. Record which steps need administrator rights, the rough download and disk sizes, and whether a new terminal (or a sign-out) is needed before `cargo` is on `PATH`.
5. Write both routes: a `winget` command route and a manual GUI-installer route.

- **Output:** `docs/findings/T0.1.1-windows-toolchain.md`.
- **Done when:** every Getting Started step has a verified command or official URL, and its admin and restart needs are recorded.

### T0.1.2 Write the README

**Type:** Docs · **Depends on:** T0.1.1

Replace the two-line README with these sections:

1. **What gwsim is:** two sentences from §1, and the status "pre-alpha, not usable yet". Link to `docs/DESIGN.md` and `docs/plan/README.md`.
2. **Getting Started (Windows 11)**, following §18.2:
   1. **Prerequisites:** Build Tools with the C++ workload, rustup (stable, MSVC), and Git. Give the `winget` commands from T0.1.1 plus the manual route. Say which steps need admin rights and when to open a new terminal.
   2. **Clone:** `git clone <repo URL>`. Use a placeholder until a remote exists, marked as such.
   3. **Build:** `cargo build --release`.
   4. **First run:** `cargo run --release -p gwsim-cli -- --version` for now. Add the §18.2 evaluation example, marked "available from milestone M1". T4.9.11 updates this step.
   5. **Tests:** `cargo test --workspace`. Note that `gwsim check` arrives in M1.
   6. **Contributors: the wiki extractor.** A placeholder paragraph covering:
      - it is a developer tool only (D8);
      - the crawl etiquette (article pages only, at least 3 s apart);
      - a full crawl takes about 3.3 hours.

      T2.7.6 fills this in.
   7. **Troubleshooting:**
      - `link.exe not found` or `linker 'link.exe' not found` means the Build Tools or the C++ workload are missing;
      - `cargo` not recognised means opening a new terminal, or checking that `%USERPROFILE%\.cargo\bin` is on `PATH`;
      - antivirus scanning `target/` can slow builds (optional exclusion, owner's choice).
3. **Licence:**
   - code GPL-3.0 (identifier from T0.5.1);
   - data under `data/` CC BY-SA 4.0;
   - no ArenaNet text or art is included;
   - not affiliated with ArenaNet or NCSOFT;
   - Guild Wars is their trademark.
4. **Contributing:** link to `CONTRIBUTING.md` (WP0.6).

- **Output:** `README.md`.
- **Done when:** every section exists, and every command matches T0.1.1.

### T0.1.3 Owner reads the README

**Type:** Review · **Depends on:** T0.1.2 · **Owner:** reviews

1. The owner reads the README for clarity, before following it in WP0.2.
2. Apply the fixes.

- **Done when:** the owner is happy to follow it in WP0.2.

---

## WP0.2 Toolchain on the development machine

**Goal:** the Rust toolchain works on the development machine, installed exactly as the README describes, so the README is proven at the same time.

- **Refs:** C5, D9, D21, §18.5.
- **Depends on:** WP0.1.
- **Done when:** `cargo --version` works, and a throwaway project links and runs.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T0.2.1 | Get approval to install | Decision | T0.1.3 | Done |
| T0.2.2 | Check what is already installed | Setup | T0.2.1 | Done |
| T0.2.3 | Install the Build Tools | Setup | T0.2.2 | Done |
| T0.2.4 | Install rustup and the stable MSVC toolchain | Setup | T0.2.3 | Done |
| T0.2.5 | Prove the linker with a throwaway project | Test | T0.2.4 | Done |
| T0.2.6 | Fold what was learned back into the README | Docs | T0.2.5 | Done |

### T0.2.1 Get approval to install

**Type:** Decision · **Depends on:** T0.1.3 · **Owner:** approves

1. Present the list:
   - Visual Studio Build Tools with the C++ workload (edition and size from T0.1.1);
   - rustup with the stable `x86_64-pc-windows-msvc` toolchain, including `rustfmt` and `clippy`.

   Git 2.55 is already installed.
2. Say which steps need admin rights and what will change on the machine (`PATH`, `%USERPROFILE%\.cargo`, `%USERPROFILE%\.rustup`).
3. Wait for an explicit yes. Don't proceed on silence.

- **Done when:** the owner has approved in the conversation.

### T0.2.2 Check what is already installed

**Type:** Setup · **Depends on:** T0.2.1

1. Run `"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`. Any output means the MSVC tools are already present.
2. Run `where link.exe`, `where rustup` and `where cargo`.
3. Skip any install step whose component is already present.

- **Done when:** the remaining installs are known.

### T0.2.3 Install the Build Tools

**Type:** Setup · **Depends on:** T0.2.2

1. Run the README's Build Tools command exactly as written.
2. If it fails, or needs something the README didn't mention, note the deviation for T0.2.6.
3. Re-run the `vswhere` check from T0.2.2 to confirm the C++ tools are installed.

- **Done when:** `vswhere` reports the VC tools component.

### T0.2.4 Install rustup and the stable MSVC toolchain

**Type:** Setup · **Depends on:** T0.2.3

1. Run the README's rustup command, accepting the defaults (stable, MSVC host).
2. Open a new terminal.
3. Run `rustc --version`, `cargo --version` and `rustup show`, and check that `rustup show` reports the host `x86_64-pc-windows-msvc`.
4. Run `rustup component list --installed` and confirm `rustfmt` and `clippy` are present. If not, run `rustup component add rustfmt clippy` and record this for T0.2.6.

- **Done when:** `cargo --version` prints a version.

### T0.2.5 Prove the linker with a throwaway project

**Type:** Test · **Depends on:** T0.2.4

1. Outside the repo (e.g. `%TEMP%\gwsim-hello`), run `cargo new hello` and then `cargo run`.
2. Confirm the program prints "Hello, world!", which proves `link.exe` works.
3. Delete the throwaway folder.

- **Done when:** the program ran.

### T0.2.6 Fold what was learned back into the README

**Type:** Docs · **Depends on:** T0.2.5

1. Fix every README deviation noted in T0.2.3 and T0.2.4.
2. Record the installed `rustc` version and the Build Tools edition in `docs/findings/T0.1.1-windows-toolchain.md`.

- **Done when:** the README matches what actually worked.

---

## WP0.3 Workspace skeleton

**Goal:** a six-crate Cargo workspace that builds, tests and runs `gwsim --version`, with the crate dependencies of §6.2.

- **Refs:** §6.1, §6.2, §18.1, §18.3, §18.5, D20.
- **Depends on:** WP0.2; T0.5.1 (licence identifier).
- **Done when:** `cargo build` and `cargo test` pass, and `cargo run -p gwsim-cli -- --version` prints `gwsim 0.1.0`.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T0.3.1 | Root manifest and toolchain files | Build | WP0.2, T0.5.1 | Done |
| T0.3.2 | Create the six crates | Build | T0.3.1 | Done |
| T0.3.3 | `gwsim --version` and stub binaries | Build | T0.3.2 | Done |
| T0.3.4 | Integration test for `--version` | Test | T0.3.3 | Done |
| T0.3.5 | Full local check | Test | T0.3.4 | Done |
| T0.3.6 | Update docs for the skeleton | Docs | T0.3.5 | Done |

### T0.3.1 Root manifest and toolchain files

**Type:** Build · **Depends on:** WP0.2, T0.5.1

1. Create the root `Cargo.toml` as a virtual manifest:
   - `[workspace]` with `members = ["crates/*"]` and `resolver = "3"` (edition 2024's default);
   - `[workspace.package]`:
     - `version = "0.1.0"`;
     - `edition = "2024"`;
     - `rust-version` set to the installed stable version;
     - `license` set to the identifier from T0.5.1;
     - `repository` left out until a remote exists;
     - `publish = false`;
   - `[workspace.lints.rust]`: `unsafe_code = "forbid"` (§18.5);
   - `[workspace.lints.clippy]`: `all = { level = "warn", priority = -1 }`;
   - `[workspace.dependencies]`: only `clap` (derive feature) and `assert_cmd` for now. Later WPs add each dependency here, once, so versions stay aligned;
   - `[profile.release]`: `debug = "line-tables-only"`, so profiles are readable in WP4.11. Leave LTO off until WP4.11 measures it.
2. Create `rust-toolchain.toml` with `channel = "stable"` and `components = ["rustfmt", "clippy"]`. This follows §18.5 ("latest stable") without pinning a version.
3. Decide where §18.1's root `tests/` and `benches/` go [Proposed]. A virtual manifest can't hold them, so:
   - per-crate `tests/` and `benches/` folders;
   - cross-crate integration tests and the relative checks (§17.4) live in `crates/gwsim-cli`;
   - `gwsim-cli` gets a library target (`src/lib.rs`) next to `src/main.rs`, so `gwsim check` and `cargo test` share the same check code.

- **Output:** `Cargo.toml`, `rust-toolchain.toml`.
- **Done when:** `cargo metadata --format-version 1` succeeds (with no members yet it may warn, which is fine).

### T0.3.2 Create the six crates

**Type:** Build · **Depends on:** T0.3.1

1. Create the six crates:

   | Crate | Command | Notes |
   | --- | --- | --- |
   | `gwsim-data` | `cargo new --lib crates/gwsim-data` | |
   | `gwsim-engine` | `cargo new --lib crates/gwsim-engine` | |
   | `gwsim-opt` | `cargo new --lib crates/gwsim-opt` | |
   | `gwsim-cli` | `cargo new crates/gwsim-cli` | Binary; add `src/lib.rs` as well (T0.3.1) |
   | `gwsim-desktop` | `cargo new crates/gwsim-desktop` | Binary |
   | `gwsim-extractor` | `cargo new crates/gwsim-extractor` | Binary |

2. In each crate's `Cargo.toml`, use `version.workspace = true`, `edition.workspace = true`, `license.workspace = true`, `publish.workspace = true` and `[lints] workspace = true`.
3. Name the binaries:
   - `gwsim-cli`: `[[bin]] name = "gwsim"`, `path = "src/main.rs"`;
   - `gwsim-extractor`: `[[bin]] name = "gwsim-extract"`;
   - `gwsim-desktop`: `[[bin]] name = "gwsim-desktop"`.
4. Add path dependencies exactly as in §6.2:

   | Crate | Depends on |
   | --- | --- |
   | `gwsim-engine` | `gwsim-data` |
   | `gwsim-opt` | `gwsim-engine`, `gwsim-data` |
   | `gwsim-cli` | `gwsim-opt`, `gwsim-engine`, `gwsim-data` |
   | `gwsim-desktop` | `gwsim-opt`, `gwsim-engine`, `gwsim-data` |
   | `gwsim-extractor` | `gwsim-data` only |

5. Give each library a crate-level doc comment stating its responsibility from the §6.1 table. `gwsim-engine`'s must also say "no I/O" (ENG-1).
6. Remove the `cargo new` sample code.

- **Output:** `crates/*`.
- **Done when:** `cargo build --workspace` succeeds.

### T0.3.3 `gwsim --version` and stub binaries

**Type:** Build · **Depends on:** T0.3.2

1. In `gwsim-cli`:
   - `src/lib.rs` defines a `Cli` struct with `#[derive(clap::Parser)]`, `#[command(name = "gwsim", version, about = "Guild Wars Reforged PvE build simulator")]` and an empty `Command` enum;
   - `src/main.rs` parses the arguments and dispatches.
2. In `gwsim-extractor`, do the same with the name `gwsim-extract`. It prints its version and "developer tool; not shipped".
3. In `gwsim-desktop`, write a `main` that prints "gwsim desktop: not yet implemented (phase P6)". **Don't add `eframe` yet**, to keep builds fast until P6.

- **Output:** three runnable binaries.
- **Done when:** `cargo run -p gwsim-cli -- --version` prints `gwsim 0.1.0`.

### T0.3.4 Integration test for `--version`

**Type:** Test · **Depends on:** T0.3.3

1. Add `assert_cmd` as a dev-dependency of `gwsim-cli`.
2. Write `crates/gwsim-cli/tests/cli.rs`, which runs `Command::cargo_bin("gwsim")` with `--version` and asserts success and stdout `gwsim 0.1.0` (built from `env!("CARGO_PKG_VERSION")`).
3. Add a second test: an unknown subcommand exits with a non-zero code.

- **Done when:** `cargo test -p gwsim-cli` passes.

### T0.3.5 Full local check

**Type:** Test · **Depends on:** T0.3.4

1. Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build --workspace` and `cargo test --workspace`.
2. Record the clean build time and the incremental build time in the T0.1.1 findings file, as a baseline.

- **Done when:** all four commands pass.

### T0.3.6 Update docs for the skeleton

**Type:** Docs · **Depends on:** T0.3.5

1. Update the §18.1 layout in DESIGN.md:
   - per-crate `tests/` and `benches/`;
   - `gwsim-cli` library target;
   - `docs/plan/` and `docs/findings/`.

   Mark each change [Proposed].
2. Check that the README "First run" step works as written.

- **Done when:** the docs match the tree.

---

## WP0.4 `.gitignore`

**Goal:** nothing that must stay local can be committed by accident: design research, the wiki cache, build output and pending test snapshots.

- **Refs:** §8.10, §18.1, Q42, D9.
- **Depends on:** — (do this first).
- **Done when:** `git status` doesn't show `research/`, and `git check-ignore` confirms the rules.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T0.4.1 | Inspect the current state | Setup | — | Done |
| T0.4.2 | Replace `.gitignore` | Build | T0.4.1 | Done |
| T0.4.3 | Verify the rules | Test | T0.4.2 | Done |
| T0.4.4 | First commits on a setup branch | Setup | T0.4.3 | Done |

### T0.4.1 Inspect the current state

**Type:** Setup · **Depends on:** —

1. Run `git ls-files`. Expect only `.gitignore`, `LICENSE` and `README.md`; nothing under `research/` or `.cache/` is tracked.
2. Read the current `.gitignore` (GitHub's Python template). Confirm nothing in it is needed: the repo has no Python code.
3. Check for a `.vscode/` folder. There is none today; if one appears later, commit only shared files (e.g. `extensions.json`).

- **Done when:** it's known that the whole file can be replaced.

### T0.4.2 Replace `.gitignore`

**Type:** Build · **Depends on:** T0.4.1

1. Write the new file. Comments go on their own lines, because `.gitignore` doesn't support comments after a pattern:

   ```gitignore
   # Rust build output
   /target/
   **/*.rs.bk
   *.pdb

   # insta snapshots awaiting review
   *.pending-snap
   *.snap.new

   # Design research: stays local (DESIGN Q42)
   /research/

   # Extractor cache: raw wiki HTML, never committed (DESIGN §8.10)
   /.cache/

   # OS files
   Thumbs.db
   Desktop.ini
   ```

2. Keep `Cargo.lock` tracked: the workspace ships binaries, so the lock file belongs in the repo.

- **Output:** `.gitignore`.

### T0.4.3 Verify the rules

**Type:** Test · **Depends on:** T0.4.2

1. `git status --porcelain` shows `docs/` but not `research/`.
2. `git check-ignore -v research/Skills/Template.md` names the `/research/` rule.
3. `git check-ignore -v --no-index .cache/wiki/Energy_Surge.html` names the `/.cache/` rule (the path doesn't need to exist).
4. `git check-ignore -v --no-index target/debug/gwsim.exe` names the `/target/` rule.

- **Done when:** all four checks behave as described.

### T0.4.4 First commits on a setup branch

**Type:** Setup · **Depends on:** T0.4.3 · **Owner:** approves each commit

1. Ask the owner before committing.
2. On approval, create the branch `setup/p0` from `main`.
3. Commit `.gitignore` on its own ("Replace Python gitignore with Rust template; ignore research/ and .cache/").
4. Commit `docs/DESIGN.md` and `docs/plan/` ("Add design document and plan of work").
5. End each message with the attribution line `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
6. Leave merging and pushing to the owner.
7. Later P0 work packages commit to this branch on the same terms.

- **Done when:** the commits exist on `setup/p0`, and `git show --stat` confirms nothing under `research/` was committed.

---

## WP0.5 Licences

**Goal:** code and data licensing are unambiguous, match C2 and Q38, and are stated where people look.

- **Refs:** C2, §8.10, Q38.
- **Depends on:** —
- **Done when:** `LICENSE` (existing), `data/LICENSE` and `data/ATTRIBUTION.md` exist, and the README and `Cargo.toml` state the licences.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T0.5.1 | Choose the code licence identifier | Decision | — | Done |
| T0.5.2 | Confirm the data licence details | Research | — | Done |
| T0.5.3 | Add `data/LICENSE` | Docs | T0.5.2 | Done |
| T0.5.4 | Add `data/ATTRIBUTION.md` | Docs | T0.5.2 | Done |
| T0.5.5 | State the licences in the README and manifests | Docs | T0.5.1, T0.5.3 | Done |

### T0.5.1 Choose the code licence identifier

**Type:** Decision · **Depends on:** — · **Owner:** decides

1. Explain the choice: the `LICENSE` text is the same either way; the SPDX identifier sets whether later GPL versions may apply.
   - `GPL-3.0-or-later` is the FSF's recommended form and the more common one.
   - `GPL-3.0-only` locks the project to version 3.

   Recommend `GPL-3.0-or-later`.
2. Record the choice in DESIGN.md §3 as a D-entry.

- **Done when:** the identifier is recorded; T0.3.1 uses it.

### T0.5.2 Confirm the data licence details

**Type:** Research · **Depends on:** —

1. Get the canonical plain-text legal code URL for CC BY-SA 4.0 from creativecommons.org.
2. Confirm on Creative Commons' ShareAlike compatibility page that CC BY-SA 4.0 is one-way compatible with GPLv3: adaptations of BY-SA 4.0 material may be licensed under GPLv3, not the reverse.
3. Record the recommended attribution form (title, author, source, licence), so `ATTRIBUTION.md` can tell reusers how to credit gwsim data.
4. Read the wiki's GWW:Copyrights and GWW:Copyrighted content pages. Record in our own words:
   - which licence the wiki's editor text uses;
   - the wiki's statement on facts "expressed originally";
   - the ArenaNet content notice (C2).

- **Output:** `docs/findings/T0.5.2-data-licence.md` (*not legal advice*).

### T0.5.3 Add `data/LICENSE`

**Type:** Docs · **Depends on:** T0.5.2

1. Create `data/LICENSE` containing the unmodified CC BY-SA 4.0 legal code text from creativecommons.org.

- **Done when:** the file matches the canonical text.

### T0.5.4 Add `data/ATTRIBUTION.md`

**Type:** Docs · **Depends on:** T0.5.2

Write these sections:

1. **What this data is:** game facts (numbers, names, relationships) and gwsim's own effect encodings.
2. **Source:** the facts come from the Guild Wars Wiki (link), expressed originally. Each file's `provenance` block names its source pages.
3. **What is not included:**
   - no in-game description text;
   - no icons or other ArenaNet art;
   - no copied wiki prose.
4. **Trademarks:** Guild Wars and ArenaNet are trademarks of their owners, and gwsim is not affiliated with them.
5. **Reusing gwsim data:** CC BY-SA 4.0, using the attribution line from T0.5.2.
6. *Not legal advice.*

- **Output:** `data/ATTRIBUTION.md`.

### T0.5.5 State the licences in the README and manifests

**Type:** Docs · **Depends on:** T0.5.1, T0.5.3

1. Make the README's Licence section (T0.1.2) name both licences and link both files.
2. Confirm `license` in `[workspace.package]` matches T0.5.1 (set in T0.3.1).

- **Done when:** the README, `Cargo.toml` and the licence files agree.

---

## WP0.6 CONTRIBUTING and CI

**Goal:** contributors know the rules (especially licensing and crawl etiquette), and a CI workflow is ready to run as soon as the repository has a GitHub remote.

- **Refs:** §18.4, §8.10, §9.1, §17.5, D14.
- **Depends on:** WP0.3.
- **Done when:** `CONTRIBUTING.md`, `.github/workflows/ci.yml` and `.github/pull_request_template.md` exist.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T0.6.1 | Write the CONTRIBUTING skeleton | Docs | WP0.5 | Done |
| T0.6.2 | Confirm current CI practice | Research | — | Done |
| T0.6.3 | Write the CI workflow | Build | T0.6.2, WP0.3 | Done |
| T0.6.4 | Add a pull request template | Docs | T0.6.1 | Done |
| T0.6.5 | Check the workflow | Test | T0.6.3 | Done |

### T0.6.1 Write the CONTRIBUTING skeleton

**Type:** Docs · **Depends on:** WP0.5

Write these sections:

1. **Start here:** links to the README Getting Started, DESIGN.md and the plan.
2. **Ground rules:**
   - the licensing rules from §8.10, stated plainly (never commit ArenaNet text or art, copied wiki prose or the `.cache/`);
   - inbound licences equal outbound: code under GPL-3.0, data under CC BY-SA 4.0.
3. **Wiki etiquette** (EXT-1 to EXT-7), for anyone running the extractor or researching:
   - article pages only;
   - at least 3 s apart;
   - no personal details in the User-Agent;
   - stop on 403 or 429.
4. **Data contributions:** review statuses (`NumbersOnly` → `Draft` → `Reviewed`, §8.7) and the review workflow (§17.5). Placeholder link to `docs/data-authoring.md` (written in T1.2.11 and WP4.1).
5. **Code contributions:**
   - the definition of done for Build tasks (plan README);
   - wiki names in code;
   - no literals for non-wiki values (ENG-4).
6. **Commits and pull requests:** small, focused commits; a branch per work package.

- **Output:** `CONTRIBUTING.md`.

### T0.6.2 Confirm current CI practice

**Type:** Research · **Depends on:** —

1. From GitHub's documentation and each action's repository, confirm the current major versions of:
   - `actions/checkout`;
   - `dtolnay/rust-toolchain`, or rustup commands directly;
   - `Swatinem/rust-cache`.
2. Confirm which image the `windows-latest` runner currently maps to, and whether it has MSVC preinstalled.
3. Decide whether to add an `ubuntu-latest` job for faster formatting and lint checks. Recommendation: Windows for build and test (the official platform), plus Linux for `fmt` and `clippy` only if Windows CI time becomes a problem.

- **Output:** `docs/findings/T0.6.2-ci.md`.

### T0.6.3 Write the CI workflow

**Type:** Build · **Depends on:** T0.6.2, WP0.3

1. Create `.github/workflows/ci.yml`:
   - **Triggers:** `push` and `pull_request`.
   - **Settings:** `permissions: contents: read`; `concurrency` grouped by ref with `cancel-in-progress: true`.
   - **Job `check`** on `windows-latest`:
     1. checkout;
     2. the toolchain step (stable, with `rustfmt` and `clippy`);
     3. the cache;
     4. `cargo fmt --all --check`;
     5. `cargo clippy --workspace --all-targets -- -D warnings`;
     6. `cargo test --workspace`.
2. Add commented placeholder steps, each naming the task that enables it:
   - `gwsim data validate` (T1.2.12);
   - reduced relative checks (T4.10.6);
   - performance smoke test (T4.11.5).

- **Output:** `.github/workflows/ci.yml`.

### T0.6.4 Add a pull request template

**Type:** Docs · **Depends on:** T0.6.1

1. Create `.github/pull_request_template.md` with:
   - a summary field;
   - the linked plan task IDs;
   - a checklist:
     - `fmt`, `clippy` and `test` pass;
     - `data validate` passes;
     - no ArenaNet text or art, no wiki prose, no cache files;
     - new assumptions registered;
     - skill review status set correctly.

- **Output:** the template.

### T0.6.5 Check the workflow

**Type:** Test · **Depends on:** T0.6.3

1. Check the workflow by eye against the T0.6.2 findings.
2. Optionally, lint it with `actionlint`, which is an install and needs the owner's approval first.
3. Note in the plan that the first real CI run happens when the owner adds a GitHub remote.

- **Done when:** the workflow has been reviewed, or linted if approved.
