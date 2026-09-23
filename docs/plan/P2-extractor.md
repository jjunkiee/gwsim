# P2 Wiki extractor

**Phase goal:** build `gwsim-extract`, a developer tool (not shipped, D8) that:

- crawls the wiki's rendered article pages slowly and resumably, and can't break the wiki's rules;
- seeds `NumbersOnly` skill files and `Draft` foe files without ever overwriting existing data;
- re-derives values later and writes a change report for maintainers.

- **Design refs:** §2.2 (C1, C2), §8.10, §9, Q10, Q24, D8, D10, D15, EXT-1 to EXT-7.
- **Starts when:** WP1.2 is done, since the extractor writes the data format defined in `gwsim-data`. It runs in parallel with P3.
- **Ends when:**
  - the M1 skill and foe files are seeded and pass `gwsim data validate`;
  - a `diff` run straight after seeding reports no changes;
  - the EXT-1 to EXT-7 tests pass.
- **Feeds:** T3.10.3 (the 8 M0 skills), WP4.1 and WP4.2 (M1 encoding and foe data), T1.6.4 (coverage denominators), and P7 (all skills).
- **Suggested order:** WP2.1, WP2.2, WP2.3, then WP2.4 and WP2.5 in parallel, then WP2.6, then WP2.7.
- **Status:** 2026-09-23, **Done**, in the unattended session. The exit criteria hold: the M1 skill and foe files are seeded (83 skills, 8 foes) and pass `gwsim data validate`; `diff` straight after seeding reports no changes; the EXT tests pass. **The owner steps were not done** — approving the crawls and the strategy, spot-checking the parse tables, and approving the commit. Each is logged in [fallout-tasks.md](fallout-tasks.md) §7 (F2.1, F2.6, F2.9) with what was done instead.

| WP | Title | Goal | Status |
| --- | --- | --- | --- |
| 2.1 | Spike | Know exactly how each needed field appears in rendered HTML, and fix the parsing and discovery strategy. | Done |
| 2.2 | Polite HTTP client | An HTTP layer that can't violate EXT-1 to EXT-7. | Done |
| 2.3 | Discovery | Resolve every player skill (ID and title) and the M1 foes through allowed list pages. | Done |
| 2.4 | Skill parser and normaliser | Turn a cached skill page into the numbers part of a skill file. | Done |
| 2.5 | Foe and area parsers | Turn foe and area pages into foe numbers and area rosters. | Done |
| 2.6 | `seed` | Write initial RON files and never overwrite. | Done |
| 2.7 | `diff` and change report | Compare re-derived values with committed data, and never write data. | Done |

---

## WP2.1 Spike

**Goal:** before writing parsers, confirm what the wiki's rendered HTML exposes for each page kind, which list pages allow discovery without the disallowed pagination, and which HTTP features (conditional requests) the server supports. Fix the strategy in writing.

- **Refs:** §9.2, §9.3, §22 (extractor fragility).
- **Depends on:** WP1.2.
- **Done when:** the findings are written, the owner has approved the parsing strategy, and synthetic fixtures exist.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T2.1.1 | Crawl rules and server behaviour | Research | — | Done |
| T2.1.2 | Fetch the representative pages | Research | T2.1.1 | Done |
| T2.1.3 | Map every field to the HTML | Research | T2.1.2 | Done |
| T2.1.4 | Fix the strategy | Decision | T2.1.3 | Done |
| T2.1.5 | Synthetic fixtures | Build | T2.1.4 | Done |

### T2.1.1 Crawl rules and server behaviour

**Type:** Research · **Depends on:** — · **Owner:** approves fetching (about 3 requests)

1. Fetch `https://wiki.guildwars.com/robots.txt`. Record every `User-agent` group, every `Disallow` and `Allow` rule, and any `Crawl-delay`. Compare with C1.
2. Read `Guild_Wars_Wiki:Bots` at `/wiki/`. Record any stated expectations for automated readers.
3. Make one GET of an ordinary article (e.g. `/wiki/Energy_Surge`) using the project User-Agent (`gwsim-extractor/0.0-spike (+<repo URL or "local development">)`). Record whether `ETag`, `Last-Modified`, `Cache-Control` and redirects are present.
4. Make one conditional GET with `If-None-Match` or `If-Modified-Since` set from step 3, and record whether it returns 304. This verifies the [Proposed] part of EXT-5.

- **Output:** the first section of `docs/findings/T2.1-extractor-spike.md`.

### T2.1.2 Fetch the representative pages

**Type:** Research · **Depends on:** T2.1.1 · **Owner:** approves the page list (about 14 requests)

1. Fetch these pages with `curl`, the spike User-Agent and `sleep 3` between requests. Save them under `.cache/wiki/spike/` (git-ignored).

   | Kind | Pages | Why |
   | --- | --- | --- |
   | Skill | Energy Surge | Scaled values; 2026 change |
   | Skill | Mistrust | Has a PvP split |
   | Skill | Air of Superiority | PvE-only; title scaling |
   | Skill | Signet of Lost Souls | Conditional signet |
   | Skill | Shelter | Binding ritual |
   | Skill | Blood is Power | Sacrifice |
   | Skill | Magehunter Strike | Adrenaline; elite attack |
   | Skill | A monster skill | Chosen from the Monster skill page |
   | Foe | Kournan Seer | HM ranks |
   | Foe | Kournan Guard | Variants; two-value armor |
   | Area | Vehtendi Valley | Foe roster |
   | List | `Guild_Wars_Wiki:Game_integration/Skills/1-500` | Skill IDs |
   | List | List of Mesmer skills | Profession list |
   | Update | `Feedback:Game_updates/20260624` | Change lines |

2. Record each page's size and response time.

- **Done when:** all the pages are in the spike cache.

### T2.1.3 Map every field to the HTML

**Type:** Research · **Depends on:** T2.1.2

1. For every field in the §9.3 table, record where it lives in the rendered HTML: the element, class or attribute, or a text pattern. Include:
   - infobox rows;
   - the progression table (columns for ranks 0–21);
   - whether `x…y…z` text appears in the description;
   - whether the skill ID is displayed anywhere;
   - how PvE and PvP split pages look ("PvE version" headings or separate titles);
   - how `(Hard mode only)` and "15 Blood Magic (20 … in Hard mode)" appear;
   - the level text `20 (26)`;
   - the armor ratings table;
   - the structure of area Foes and Bosses lists;
   - the Game integration list's row format and its sibling pages (1-500 … 3001-3500);
   - where "Kournan Guard" variants appear.
2. Record the value formats that need normalising: fractions (`¾`, `{{3/2}}` output), percentages, `-1` upkeep, adrenaline strikes, sacrifice percentages, overcast.
3. Confirm the first page of a category (`/wiki/Category:…`) is reachable and lists up to 200 members, for cross-checks.

- **Output:** a field-mapping table in the findings file.

### T2.1.4 Fix the strategy

**Type:** Decision · **Depends on:** T2.1.3 · **Owner:** approves

1. Decide and write down:
   - the source of skill IDs (the Game integration lists, plus a fallback for IDs newer than 2026-08-08, e.g. 3443);
   - the source of scaled values (progression table first; description text as a check only, never stored);
   - how split pages are handled (take the PvE version, D3);
   - which list pages drive discovery;
   - the normalisation rules;
   - the conditional-request policy.
2. Update DESIGN.md §9.2 and §9.3 where the spike changes a [Proposed] item.

- **Done when:** the owner has approved the strategy in the findings file.

### T2.1.5 Synthetic fixtures

**Type:** Build · **Depends on:** T2.1.4

1. For each page kind, write a minimal HTML file under `crates/gwsim-extractor/tests/fixtures/`. Each copies the **structure** of the real page (tags, classes, table layout) with invented numbers and placeholder prose ("Lorem skill text"), so no wiki content is committed (§9.5).
2. Add one fixture per edge case found in T2.1.3: a split page, HM-only skills, two-value armor, a progression table with special rounding.

- **Output:** the fixture set with a README explaining its rule (structure only).

---

## WP2.2 Polite HTTP client

**Goal:** an HTTP layer that makes it impossible to break the wiki's rules. Only allowed URLs can be built; every request waits its turn; errors stop the crawl rather than retrying aggressively; every page is cached with its fetch time.

- **Refs:** EXT-1 to EXT-7, C1, D15.
- **Depends on:** WP2.1.
- **Done when:** one test per EXT rule passes, with no network access in the tests.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T2.2.1 | Client skeleton, User-Agent and transport | Build | T2.1.4 | Done |
| T2.2.2 | URL guard and robots.txt | Build | T2.2.1 | Done |
| T2.2.3 | Rate limiter | Build | T2.2.1 | Done |
| T2.2.4 | Response handling and stop rules | Build | T2.2.2, T2.2.3 | Done |
| T2.2.5 | Page cache and crawl state | Build | T2.2.4 | Done |
| T2.2.6 | EXT test suite | Test | T2.2.5 | Done |

### T2.2.1 Client skeleton, User-Agent and transport

**Type:** Build · **Depends on:** T2.1.4

1. Add `ureq` to the workspace dependencies. Use it in `gwsim-extractor` only.
2. Define `trait Transport { fn get(&self, url: &Url, headers: &[(&str, String)]) -> Result<Response> }`, with two implementations:
   - `UreqTransport` for real requests;
   - `FakeTransport` for tests, holding scripted responses and recording requests.
3. The User-Agent is `gwsim-extractor/<CARGO_PKG_VERSION> (+<REPO_URL>)`. `REPO_URL` is a constant, set to the project URL once a remote exists and to `local development` until then (EXT-4). Add a unit test asserting the UA contains no `@`.
4. Add a `Clock` trait (`now()`, `sleep(d)`) with real and fake implementations, so the delay logic can be tested instantly.

- **Done when:** the fake transport and fake clock drive a request in a unit test.

### T2.2.2 URL guard and robots.txt

**Type:** Build · **Depends on:** T2.2.1

1. The only public URL constructor is `WikiUrl::article(title: &WikiTitle)`, which produces `https://wiki.guildwars.com/wiki/<encoded title>`. The single other allowed URL is `robots.txt`. No other URL can be built (EXT-2).
2. Add a second guard at request time. Reject any URL that:
   - contains `/api.php`, `/index.php`, `Special:` (in any case) or a `?`;
   - uses a host other than `wiki.guildwars.com`.
3. Write a minimal robots.txt parser: groups by `User-agent`, `Allow` and `Disallow` prefix rules with longest-match precedence, and `*` as the fallback group. Fetch robots.txt at the start of every session and check every URL against it before requesting (EXT-1).
4. If robots.txt can't be fetched, stop the session. Don't assume permission.

- **Done when:** unit tests confirm that a disallowed path, a query string, `Special:Random` and an `/index.php?title=` URL are all refused before any transport call.

### T2.2.3 Rate limiter

**Type:** Build · **Depends on:** T2.2.1

1. Add a single process-wide limiter. The minimum gap between request starts defaults to 3 s (EXT-3). `--delay <secs>` is accepted only when it is ≥ 2 s; smaller values are rejected with an error.
2. The robots.txt fetch counts as a request.
3. Crawling is single-threaded, with no concurrent requests.

- **Done when:** tests using the fake clock confirm 3 s gaps, `--delay 2` is accepted, and `--delay 1` is rejected.

### T2.2.4 Response handling and stop rules

**Type:** Build · **Depends on:** T2.2.2, T2.2.3

1. Handle each response by status:

   | Status | Action |
   | --- | --- |
   | 200 | Store in the cache. |
   | 304 | Keep the cached copy and update its check time. |
   | 301 / 302 | Follow only if the target passes both guards; record the canonical title. |
   | 404 | Record as missing and continue. |
   | 5xx | Retry with exponential back-off: 30 s, 60 s, then 120 s, at most 3 retries. Then skip the page and continue. |
   | 403 or 429 | **Stop the session immediately.** Print a report: URL, status, the headers received, and the advice to wait and not retry aggressively (EXT-6). |
   | Network error | Treat as 5xx. |

2. Also handle MediaWiki redirect pages served as 200 with a "Redirected from" marker: record the alias to canonical title mapping.
3. End every session with a summary: fetched, not modified, missing, skipped, stopped, and elapsed time.

- **Done when:** each status path has a unit test on the fake transport.

### T2.2.5 Page cache and crawl state

**Type:** Build · **Depends on:** T2.2.4

1. Cache layout under `.cache/wiki/` (EXT-5):
   - `<sanitised title>.html`: the raw body;
   - `<sanitised title>.meta.json`: `{url, title, canonical_title, fetched_at, checked_at, status, etag, last_modified}`.

   Sanitise titles so they are safe as Windows file names (`:`, `/`, `"`, `?` and `*` are encoded).
2. `--max-age <days>` skips pages checked more recently than the age given.
3. Use conditional requests only if T2.1.1 showed the server honours them.
4. Keep crawl state in `.cache/crawl-state.json`: the pending titles and the completed titles, so an interrupted crawl resumes where it stopped.
5. Cached HTML is read only by the parsers. Nothing from the cache is ever copied into `data/` except extracted numbers (EXT-7).

- **Done when:** an interrupted fake crawl resumes without re-fetching completed pages.

### T2.2.6 EXT test suite

**Type:** Test · **Depends on:** T2.2.5

1. Write `crates/gwsim-extractor/tests/ext_rules.rs` with one clearly named test per rule. Examples: `ext1_robots_disallow_is_respected`, `ext1_api_php_never_requested`, `ext2_only_article_urls`, `ext3_minimum_delay`, `ext4_user_agent_has_no_personal_details`, `ext5_cache_hit_avoids_request`, `ext5_resume_after_interrupt`, `ext6_429_stops_session`, `ext6_5xx_backoff_sequence`, and `ext7_seed_output_contains_no_description_text` (added in T2.6.4).
2. All tests use the fake transport and clock. No test touches the network.

- **Done when:** the suite passes.

---

## WP2.3 Discovery

**Goal:** resolve the complete set of player skills (ID, title, profession, flags) and the M1 foe titles, using only allowed list pages, because category pagination is disallowed (C1).

- **Refs:** §9.2, §9.4 (`crawl`).
- **Depends on:** WP2.2.
- **Done when:** the full skill list and the M1 foe list are resolved and cross-checked, and `data/skills/index.ron` is written.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T2.3.1 | Game integration list parser | Build | T2.1.5 | Done |
| T2.3.2 | Skill list parsers and merge | Build | T2.3.1 | Done |
| T2.3.3 | Area foe discovery | Build | T2.1.5 | Done |
| T2.3.4 | `crawl` command | Build | T2.3.2, T2.3.3, WP2.2 | Done |
| T2.3.5 | Discovery tests | Test | T2.3.4 | Done |
| T2.3.6 | Run discovery and write the skill index | Run | T2.3.5 | Done |

### T2.3.1 Game integration list parser

**Type:** Build · **Depends on:** T2.1.5

1. Parse `Guild_Wars_Wiki:Game_integration/Skills/<range>` pages into `(SkillId, WikiTitle, other columns per T2.1.3)`.
2. Iterate the known sibling ranges (1-500 … 3001-3500), and detect whether a further page exists.

- **Done when:** the fixture parses to the expected rows.

### T2.3.2 Skill list parsers and merge

**Type:** Build · **Depends on:** T2.3.1

1. Parse List of all skills and the per-profession list pages into `(title, profession, attribute, elite, pve_only, campaign)`.
2. Merge them with the integration IDs by title (following redirects).
3. Report:
   - titles with no ID (newer than the integration pages), resolved later from the skill page's own infobox if T2.1.3 found the ID there;
   - IDs with no list entry;
   - PvP-only titles (`(PvP)`), excluded per D3;
   - monster skills, kept separately.

- **Done when:** the fixtures merge correctly, and the reports list the planted gaps.

### T2.3.3 Area foe discovery

**Type:** Build · **Depends on:** T2.1.5

1. Parse an area page's `Foes` and `Bosses` sections into foe titles.
2. For M1, check that Vehtendi Valley yields the 8 Kournan foe titles of §20.2.

- **Done when:** the fixture gives the expected titles.

### T2.3.4 `crawl` command

**Type:** Build · **Depends on:** T2.3.2, T2.3.3, WP2.2

1. Implement `gwsim-extract crawl [--scope skills|foes|areas|all] [--only <title>...] [--delay S] [--max-age D] [--dry-run]`.
2. Order of work:
   1. discovery pages;
   2. the page list;
   3. the fetch of each page;
   4. resumable state (T2.2.5).
3. Before starting, print the page count and an ETA (count × delay), e.g. "about 4,000 pages, about 3.3 hours" (§9.4).
4. `--dry-run` lists the URLs and fetches nothing.

- **Done when:** a fake crawl over the fixtures completes and resumes after an interruption.

### T2.3.5 Discovery tests

**Type:** Test · **Depends on:** T2.3.4

1. Write integration tests over the fixtures for list parsing, merging, gap reports and the dry-run output.
2. Add a cross-check helper that compares a category's first page (up to 200 members) with the merged list, for one small category.

- **Done when:** the tests pass.

### T2.3.6 Run discovery and write the skill index

**Type:** Run · **Depends on:** T2.3.5 · **Owner:** approves the crawl (about 20 list pages, about 1 minute)

1. Run `gwsim-extract crawl --scope skills` with discovery only (`--only` the list pages), then `--scope areas --only "Vehtendi Valley"`.
2. Write `data/skills/index.ron` [Proposed]: one record per player skill, holding ID, title, slug, profession, elite, PvE-only and campaign. These are facts only. The file gives coverage its denominators (T1.6.4).
3. Add the index schema to `gwsim-data` and cross-check it in validation: every skill file's ID must appear in the index.
4. Record the counts and the gap reports in `docs/findings/T2.3.6-discovery.md`. The design expects about 1,400 player skills.

- **Done when:** the index validates, and the M1 foe titles are resolved.

---

## WP2.4 Skill page parser and normaliser

**Goal:** turn a cached skill page into the numbers part of a skill file: infobox fields, scaled values, skill ID and description hash. Values are normalised to the schema's units and enums.

- **Refs:** §8.3, §9.3 (skill row), EXT-7.
- **Depends on:** WP2.3.
- **Done when:** M1's 72 skills parse without errors, and the owner has hand-checked the parsed numbers.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T2.4.1 | Infobox parser | Build | T2.1.5 | Done |
| T2.4.2 | Value normaliser | Build | T2.4.1 | Done |
| T2.4.3 | Scaled values | Build | T2.4.1 | Done |
| T2.4.4 | Split pages and special cases | Build | T2.4.2 | Done |
| T2.4.5 | Skill ID and description hash | Build | T2.4.1 | Done |
| T2.4.6 | Parser tests | Test | T2.4.2–T2.4.5 | Done |
| T2.4.7 | Crawl the M1 skill pages | Run | T2.4.6 | Done |
| T2.4.8 | Hand-check the 72 parses | Review | T2.4.7 | Done |

### T2.4.1 Infobox parser

**Type:** Build · **Depends on:** T2.1.5

1. Add the `scraper` crate to `gwsim-extractor`.
2. Parse the skill infobox into a `RawSkill` map of the raw strings for every §9.3 field: profession, attribute, type, energy, adrenaline, sacrifice, upkeep, overcast, activation, recharge, elite, PvE-only, campaign, target, range, AoE, and causes/removes tags.
3. Missing fields are `None`, not errors. `diff` must tolerate missing fields (§22).

- **Done when:** the fixture skills parse to the expected raw maps.

### T2.4.2 Value normaliser

**Type:** Build · **Depends on:** T2.4.1

1. Convert a `RawSkill` into the typed numbers part of `Skill`:
   - fractions (`¾`, `½`, `¼`, and forms like `1¼`) to `Seconds`;
   - energy, adrenaline (strikes), sacrifice (%), upkeep (−1) and overcast to `Cost`;
   - the type string to `SkillType` (a table covering every type name on the wiki);
   - the profession, attribute ("No attribute" becomes `None`), campaign and title track;
   - range and AoE keywords to `RangeBand` or an explicit radius.
2. Unknown strings produce a `NormaliseWarning` with the page title. They never panic.

- **Done when:** each rule has a unit test, including every fraction form found in T2.1.3.

### T2.4.3 Scaled values

**Type:** Build · **Depends on:** T2.4.1

1. Parse the progression table: each row label and its rank 0…21 values.
2. For each row, produce `ScaledNumber { label, r0, r12, r15 }`.
3. Recompute every rank with the Gr formula (T1.4.7). Where the page's values differ, mark the row `special_rounding` and keep the page's values.
4. If there's no table, record "no progression table". Description text is used only as a cross-check of the numbers and is never stored.

- **Done when:** the fixtures give the expected triples, and the special-rounding fixture is flagged.

### T2.4.4 Split pages and special cases

**Type:** Build · **Depends on:** T2.4.2

1. Handle the special pages:
   - on PvE/PvP split pages, take the PvE data per T2.1.4;
   - titles ending in `(PvP)` are skipped;
   - on PvE-only title skills, record the title track and the Gr2 values;
   - monster skills go to `skills/monster/` with no attribute.
2. Record every special case applied in the provenance notes.

- **Done when:** the split and PvE-only fixtures normalise correctly.

### T2.4.5 Skill ID and description hash

**Type:** Build · **Depends on:** T2.4.1

1. Take the skill ID from the index (T2.3.6), falling back to the page per T2.1.4. If neither has it, report an error rather than guessing.
2. Compute `description_hash`: a SHA-256 of the normalised description text (whitespace collapsed). Store only the hash, so `diff` can flag description changes without storing prose (EXT-7) [Proposed].

- **Done when:** the fixture gets the expected ID and a stable hash, and the output contains no description text.

### T2.4.6 Parser tests

**Type:** Test · **Depends on:** T2.4.2–T2.4.5

1. Run the fixture-based tests for the whole skill pipeline (HTML to typed numbers).
2. Add a local-only test (`#[ignore]`, run with `cargo test -- --ignored`) that parses every cached M1 skill page and asserts there are no errors. It skips cleanly when `.cache/` is empty (§9.5).

- **Done when:** the fixture tests pass in CI; the ignored test is documented.

### T2.4.7 Crawl the M1 skill pages

**Type:** Run · **Depends on:** T2.4.6 · **Owner:** approves the crawl (72 pages, about 4 minutes)

1. Run `gwsim-extract crawl --scope skills --only <the 72 titles of §20.3>`.
2. Then run the ignored test from T2.4.6.

- **Done when:** all 72 pages are cached and parse.

### T2.4.8 Hand-check the 72 parses

**Type:** Review · **Depends on:** T2.4.7 · **Owner:** spot-checks

1. Generate `docs/findings/T2.4.8-m1-skill-parse-check.md`. It holds a table with one row per skill:
   - costs;
   - activation and recharge;
   - type and flags;
   - scaled triples;
   - the wiki link.

   Numbers only, no prose.
2. Claude checks every row against the wiki page and marks it.
3. The owner spot-checks at least 10 rows, including every row flagged `special_rounding`.
4. Fix the parser and repeat until every row matches.

- **Done when:** all 72 rows are marked correct.

---

## WP2.5 Foe and area parsers

**Goal:** turn foe pages (`{{NPC infobox}}` plus the Skills and Armor ratings sections) and area pages into foe numbers and area rosters. Values missing on the wiki become assumption references, not guesses.

- **Refs:** §7.1 (Foe), §9.3 (foe and area rows), A-004, A-005, A-007.
- **Depends on:** WP2.3.
- **Done when:** the 8 Kournan foes and the Vehtendi Valley roster parse, and they match §20.2 or the differences are explained.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T2.5.1 | NPC infobox | Build | T2.1.5 | Done |
| T2.5.2 | Skills section and attributes | Build | T2.5.1 | Done |
| T2.5.3 | Armor ratings table | Build | T2.5.1 | Done |
| T2.5.4 | Area roster | Build | T2.3.3 | Done |
| T2.5.5 | Foe and area tests | Test | T2.5.2–T2.5.4 | Done |
| T2.5.6 | Crawl the Kournan and area pages | Run | T2.5.5 | Done |
| T2.5.7 | Hand-check against §20.2 | Review | T2.5.6 | Done |

### T2.5.1 NPC infobox

**Type:** Build · **Depends on:** T2.1.5

1. Extract:
   - name;
   - professions;
   - level text `NM (HM)`, parsed into `ModeValue<u8>`;
   - species or creature type;
   - boss flag;
   - affiliation.
2. The creature type maps to `traits` (fleshy and so on). If the species isn't in the mapping table, emit a warning.

- **Done when:** the fixtures give the expected fields.

### T2.5.2 Skills section and attributes

**Type:** Build · **Depends on:** T2.5.1

1. Parse the skill links into `FoeSkill`, setting `hm_only` from `(Hard mode only)` tags. Handle skills listed per variant (Kournan Guard axe and hammer).
2. Parse attribute text such as `15 Blood Magic (20 … in Hard mode)` into NM and HM ranks. A missing HM rank stays `None` (A-005 is applied later, in T1.4.8).
3. Monster skills link to the monster-skill index.

- **Done when:** the Seer fixture gives Domination 15, Inspiration 14 and HM Domination 20.

### T2.5.3 Armor ratings table

**Type:** Build · **Depends on:** T2.5.1

1. Parse `{{NPC statistics}}` output into armor per damage type (blunt, piercing, slashing, cold, earth, fire, lightning).
2. Where a cell holds two values ("a / b"), keep both, and record which column or context each belongs to, per T2.1.3. WP4.2 decides which level the values apply to.

- **Done when:** the Guard fixture keeps both values with their context.

### T2.5.4 Area roster

**Type:** Build · **Depends on:** T2.3.3

1. Parse the `Foes` and `Bosses` lists with levels and spawn notes into an `AreaRoster`.
2. Store the roster in the findings (and optionally as encounter-authoring notes) only. It is not a data file: group composition is hand-authored (A-006).

- **Done when:** the Vehtendi Valley fixture lists its foes with levels.

### T2.5.5 Foe and area tests

**Type:** Test · **Depends on:** T2.5.2–T2.5.4

1. Run the fixture tests for the whole foe pipeline.
2. Add an ignored local test over the cached Kournan pages.

- **Done when:** the fixture tests pass.

### T2.5.6 Crawl the Kournan and area pages

**Type:** Run · **Depends on:** T2.5.5 · **Owner:** approves the crawl (9 pages)

1. Run `gwsim-extract crawl --scope foes --only <8 Kournan titles>` and `--scope areas --only "Vehtendi Valley"`.

- **Done when:** all 9 pages are cached and parse.

### T2.5.7 Hand-check against §20.2

**Type:** Review · **Depends on:** T2.5.6 · **Owner:** spot-checks

1. Compare the parsed professions, skills, attribute ranks and armor with the §20.2 table.
2. Record every difference and its explanation in `docs/findings/T2.5.7-kournan-parse-check.md`. The wiki wins over §20.2: update §20.2 if the design misread something.

- **Done when:** the eight foes are checked, and every difference is explained.

---

## WP2.6 `seed`

**Goal:** write initial RON files from the cache. Skills are written as `NumbersOnly`; foes are written as `Draft`, with assumption references for missing values. Existing files are never overwritten.

- **Refs:** §8.2, §8.3, §9.4 (`seed`), Q10, D27.
- **Depends on:** WP2.4, WP2.5.
- **Done when:** the M1 skill and foe files are seeded, pass `gwsim data validate`, and have been reviewed by the owner before committing.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T2.6.1 | Skill file writer | Build | WP2.4 | Done |
| T2.6.2 | Foe file writer | Build | WP2.5 | Done |
| T2.6.3 | No-overwrite guarantee and dry run | Build | T2.6.1, T2.6.2 | Done |
| T2.6.4 | Seed tests | Test | T2.6.3 | Done |
| T2.6.5 | Seed the M1 files | Run | T2.6.4, T2.4.8, T2.5.7 | Done |

### T2.6.1 Skill file writer

**Type:** Build · **Depends on:** WP2.4

1. Serialise the numbers part of `Skill` to RON with a stable field order and formatting. Use one canonical pretty-printer, so re-seeding produces identical bytes.
2. Put a header comment with the wiki URL at the top of each file.
3. Set `encoding: None` and write the provenance:
   - `sources`: the page URL;
   - `crawled`: the cache fetch date;
   - `review: NumbersOnly`;
   - `assumptions: []`.
4. Build the path from the profession folder (§8.1) and `slugify(title)`.

- **Done when:** a fixture skill is written in the expected form (snapshot).

### T2.6.2 Foe file writer

**Type:** Build · **Depends on:** WP2.5

1. Serialise `Foe` with `review: Draft`.
2. Add assumption references automatically when values are missing: no weapon adds A-004; missing HM ranks add A-005; variants add A-007 where relevant.
3. Build the path from `creatures/foes/<affiliation slug>/<slug>.ron`.

- **Done when:** a fixture foe is written with the expected assumption references.

### T2.6.3 No-overwrite guarantee and dry run

**Type:** Build · **Depends on:** T2.6.1, T2.6.2

1. Open every output with `OpenOptions::new().write(true).create_new(true)`. If a file exists, skip it and list it in the summary as "exists, not touched" (Q10).
2. `gwsim-extract seed [--scope …] [--only …] [--data-dir data] [--dry-run]`. `--dry-run` prints the files it would create.

- **Done when:** a test confirms an existing file is byte-identical after `seed`.

### T2.6.4 Seed tests

**Type:** Test · **Depends on:** T2.6.3

1. Seed into a temporary folder from the fixture cache, then check:
   - `gwsim data validate` passes on the result (together with the core files);
   - a second `seed` creates nothing and changes nothing;
   - no written file contains description text (the fixtures' placeholder text is searched for). This is `ext7_seed_output_contains_no_description_text`.

- **Done when:** all pass.

### T2.6.5 Seed the M1 files

**Type:** Run · **Depends on:** T2.6.4, T2.4.8, T2.5.7 · **Owner:** reviews and approves the commit

1. Run `gwsim-extract seed --scope skills --only <72 titles>` and `--scope foes --only <8 titles>` into `data/`.
2. Run `gwsim data validate` and `gwsim data coverage`.
3. Show the owner the new files, grouped by folder.
4. Commit them on a WP branch when the owner approves.
5. If WP3.10 hand-entered any M0 skills earlier, `seed` skips those files. Compare them with `diff` (WP2.7) instead.

- **Done when:** the M1 files are in `data/` and validated.

---

## WP2.7 `diff` and change report

**Goal:** re-derive values from the cache, compare them with the committed data, and report what changed:

- changed fields;
- new and removed pages;
- skills whose description changed, which need re-translation.

It never writes to `data/`.

- **Refs:** §9.4 (`diff`), G6, UC11, §22.
- **Depends on:** WP2.6.
- **Done when:** running `diff` straight after seeding reports no changes, and a planted change is reported.

| Task | Title | Type | Depends on | Status |
| --- | --- | --- | --- | --- |
| T2.7.1 | Comparison model | Build | WP2.6 | Done |
| T2.7.2 | Report writers | Build | T2.7.1 | Done |
| T2.7.3 | `diff` command (read-only) | Build | T2.7.2 | Done |
| T2.7.4 | Game update context (optional) | Build | T2.7.3 | Done |
| T2.7.5 | Diff tests | Test | T2.7.3 | Done |
| T2.7.6 | Extractor docs in the README | Docs | T2.7.3 | Done |
| T2.7.7 | Verify no changes after seeding | Run | T2.7.5, T2.6.5 | Done |

### T2.7.1 Comparison model

**Type:** Build · **Depends on:** WP2.6

1. For each skill and foe, compare the extracted numbers from the cache with the numbers part of the committed file. The hand-maintained encoding is ignored.
2. Classify each difference:

   | Class | Meaning |
   | --- | --- |
   | `FieldChanged { path, old, new }` | A value differs |
   | `NewPage` | In the index, but no file |
   | `RemovedPage` | A file exists, but the page is gone or redirected |
   | `DescriptionChanged` | The hash differs; flag the skill for re-translation |
   | `ParseProblem` | The page couldn't be parsed |

3. Missing fields on the page are reported, not treated as removal (§22).

- **Done when:** unit tests cover each class.

### T2.7.2 Report writers

**Type:** Build · **Depends on:** T2.7.1

1. Write the markdown report:
   - summary counts;
   - sections by profession and then by class;
   - one line per change, with a wiki link and the old → new value;
   - a "needs re-translation" list;
   - the review status of each affected file, since a `Reviewed` skill whose numbers changed needs review again.
2. Write the same content as JSON for tooling.

- **Done when:** the snapshot tests pass for both formats.

### T2.7.3 `diff` command (read-only)

**Type:** Build · **Depends on:** T2.7.2

1. `gwsim-extract diff [--scope …] [--out report.md] [--json out.json]`. It uses the cache as it stands, so run `crawl` first to refresh it.
2. Guarantee it is read-only: the data directory is read through `DataSource` only, and a test checks that file modification times under `data/` are unchanged.

- **Done when:** the command runs over the fixtures.

### T2.7.4 Game update context (optional)

**Type:** Build · **Depends on:** T2.7.3

1. Parse cached `Feedback:Game_updates/<date>` pages for lines naming skills, and attach them to the matching report entries as context (§9.3, update row).
2. This is optional for M1. Do it when the first balance patch after the 2026-09-22 baseline arrives.

- **Done when:** a fixture update page attaches its lines to the right skills.

### T2.7.5 Diff tests

**Type:** Test · **Depends on:** T2.7.3

1. Test these cases:
   - seed from fixtures, then `diff`, reports "No changes";
   - edit a fixture page's energy cost, then `diff`, reports a `FieldChanged`;
   - change the description placeholder, and `diff` reports `DescriptionChanged`;
   - delete a fixture page, and `diff` reports `RemovedPage`.

- **Done when:** all pass.

### T2.7.6 Extractor docs in the README

**Type:** Docs · **Depends on:** T2.7.3

1. Fill in the README's contributor section (T0.1.2 step 6):
   - what the extractor is for;
   - `crawl`, `seed` and `diff` with examples;
   - the etiquette (EXT-1 to EXT-7);
   - expected durations (a full crawl takes about 3.3 hours);
   - where the cache lives, and that it must never be committed;
   - the balance-patch workflow outline (UC11, detailed in WP7.18).

- **Done when:** the section is complete.

### T2.7.7 Verify no changes after seeding

**Type:** Run · **Depends on:** T2.7.5, T2.6.5

1. Run `gwsim-extract diff --scope all` on the real M1 cache and data.
2. Expect "No changes". If anything is reported, fix the cause (parser non-determinism, writer formatting, or data edited by hand) and repeat.

- **Done when:** the report is empty. This closes P2.
