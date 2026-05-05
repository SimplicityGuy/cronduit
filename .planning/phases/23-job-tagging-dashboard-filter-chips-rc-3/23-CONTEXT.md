# Phase 23: Job Tagging Dashboard Filter Chips — rc.3 - Context

**Gathered:** 2026-05-04
**Status:** Ready for planning

<domain>
## Phase Boundary

Operators get CSS-only filter chips on the dashboard for every distinct tag in the running fleet. Multiple chips compose with **AND** semantics; the active set composes with the existing v1.0 name-filter via **AND**; untagged jobs hide whenever ANY tag filter is active (TAG-07 least-surprise). The active set is encoded in repeated `?tag=` URL params so dashboard URLs are shareable + bookmarkable. Chip toggles trigger an HTMX swap of both the chip strip (OOB) and the table body — no JS required for the active/inactive toggle. Phase ends with a `v1.2.0-rc.3` tag cut via `docs/release-rc.md` verbatim.

The change is dashboard-and-query-layer only. Schema is already in place from Phase 22; this phase reads from `jobs.tags`, never writes:

1. **`DashboardJob` widening** — extend the struct with `tags: Vec<String>`, project `j.tags` into the existing `get_dashboard_jobs` SELECT (`src/db/queries.rs:818`), deserialize the JSON column at the row-mapping site for both sqlite and postgres branches.
2. **Filter SQL** — compose `tags LIKE '%"' || ?N || '"%'` AND-chained per active tag onto the existing WHERE in `get_dashboard_jobs`, plus a `tags != '[]'` clause when the active set is non-empty (TAG-07 untagged-hidden). The pattern is parity-friendly across sqlite + postgres without dialect-specific JSON ops; TAG-05 (substring-collision) already gates fleet config-load so the LIKE is structurally safe.
3. **URL extractor** — replace `Query<DashboardParams>` with `axum_extra::extract::Query<DashboardParams>` (already in tree); add `#[serde(default, rename = "tag")] pub tags: Vec<String>` to the params struct; supports repeated `?tag=` keys via serde_html_form.
4. **Distinct-tag fold** — in `dashboard()` handler, after `get_dashboard_jobs()`, fold `jobs.iter().flat_map(|j| &j.tags).collect::<BTreeSet<_>>()` into a sorted `Vec<String>` and pass to the template alongside the active-tag set. Mirrors P13 OBS-03 sparkline aggregation living in the handler not queries.rs.
5. **Template inserts** — new chip strip (`<div id="cd-tag-chip-strip">`) inserted into `templates/pages/dashboard.html` as a **dedicated row above the existing name-filter input** at L19 (chip strip on its own line, name-filter on the line below). Chip strip is hidden entirely when the fleet has zero tagged jobs (mirrors how `cd-bulk-action-bar` at L46 is `hidden` until relevant). Each chip is `<a hx-get="/?...&tag=..." hx-target="#job-table-body" hx-swap-oob="true" hx-push-url="true">`. Active state = `cd-tag-chip cd-tag-chip--active` (teal-bordered + bold per TAG-06); inactive = `cd-tag-chip cd-tag-chip--inactive` (grey).
6. **Hidden-input filter preservation** — for each active tag, render a sibling `<input type="hidden" name="tag" value="X">` inside the chip strip. Update the existing `every 3s` poll on `#job-table-body` (`dashboard.html:138`) `hx-include` to add `[name='tag']` so polling preserves the active filter set. Hidden inputs get OOB-replaced together with the chip strip on toggle.
7. **Sort-header href composition** — each sortable column anchor (Name / Next Fire / Status / Last Run at `dashboard.html:88-128`) must include `&tag=...` for every active tag in BOTH the `href` attribute and the `hx-get` attribute, so non-HTMX navigation and copy-link both round-trip the active set.
8. **CSS additions** — new `cd-tag-chip` family in `assets/src/app.css` `@layer components` (active/inactive variants per design system; `flex-wrap` strip for mobile reflow). Mirrors P21's `cd-fctx-*` / `cd-exit-*` namespacing precedent.
9. **Tests** — new `tests/v12_tags_dashboard.rs` integration test covering chip strip render, AND filter SQL correctness, TAG-07 untagged-hidden, AND with name-filter, repeated `?tag=` URL parsing, sort+chip URL composition, OOB swap response shape; extend `src/web/handlers/dashboard.rs` unit tests for handler-side fold + active-set parsing.
10. **UAT** — three new `just` recipes (`uat-chips-render`, `uat-chips-and-filter`, `uat-chips-share-url`) following the P22 `uat-tags-*` recipe-calls-recipe pattern; `23-HUMAN-UAT.md` autonomous=false maintainer plan.
11. **rc.3 cut** — autonomous=false `23-RC3-PREFLIGHT.md` final wave, mirrors P21 D-22..D-26 verbatim. Cargo.toml stays at `1.2.0`; `:latest` GHCR tag stays at `v1.1.0`; rolling `:rc` tag updates to `v1.2.0-rc.3` on push; tag command `git tag -a -s v1.2.0-rc.3 -m "v1.2.0-rc.3 — dashboard tag filter chips (P23)"`. NO modifications to `release.yml` / `cliff.toml` / `docs/release-rc.md`.

**Routing note (load-bearing):** Phase 23 is a UI phase. Roadmap labels it "**UI hint**: yes". After this CONTEXT.md commits, the next step is `/gsd-ui-phase 23` to author `23-UI-SPEC.md` BEFORE planning — same discipline that made P21 implementation plumbing-only. The chip primitive (`cd-tag-chip-*`) needs explicit hover/focus/active visual states, keyboard/screen-reader contract, mobile-wrap behavior, and label-rendering specifics that benefit from the locked visual contract.

**Out of scope for Phase 23** (deferred — do not creep):
- Tag autocomplete / search-as-you-type in the chip strip — v1.3 candidate (P22 deferred ideas list).
- Tag-based bulk operations on the row-checkbox bulk-action bar — explicit v1.3 candidate per `.planning/REQUIREMENTS.md` § Out of Scope.
- Tag chips on `/jobs/{id}` (job detail) page — Phase 23 is dashboard-only per TAG-06; job-detail tag display is deferred.
- Tags as Prometheus label — explicit out-of-scope per `.planning/REQUIREMENTS.md` (cardinality discipline; same posture as exit codes per EXIT-06).
- Tag-based webhook routing keys — WH-09 carries tags in payload but never AS a routing key (P18 D-17 lock; reaffirmed P22 deferred ideas list).
- Per-tag job count badge on chips (e.g., `backup (3)`) — Claude's-discretion candidate; if planner picks it up the count is computed Rust-side from the same fold; UI-SPEC decides whether to render it.
- Browser-based playwright smoke test for HTMX chip clicks — adds new test infrastructure not in tree; v1.3 candidate at most.
- THREAT_MODEL.md TM5 / TM6 updates — Phase 24 milestone close-out per ROADMAP.
- README configuration subsection on tag filtering — Claude's discretion; the labels-precedent README subsection (P17 D-04) is the template if planner picks it up. P22's deferred ideas list noted Phase 23 was the natural place to add it once filter UI lands.
- `release.yml` / `cliff.toml` / `docs/release-rc.md` modifications for v1.2-specific rc behavior — reused verbatim per P20 D-30 / P21 D-22..D-26.

</domain>

<decisions>
## Implementation Decisions

### Chip strip layout + CSS (Gray Area 1)

- **D-01:** **Chip strip placement: dedicated row above the name-filter input.** Insert `<div id="cd-tag-chip-strip">` as its own row at the top of `templates/pages/dashboard.html` (above the existing filter row at L19-36). Reads top-to-bottom as "narrow by tag, then narrow by name" — matches the AND-composition semantics. Visual hierarchy: tags filter the corpus, name filters within the filtered set.

  **Rejected:** inline with name-filter (cramps the input below ~16 chips at narrow widths; muddier hierarchy). **Rejected:** below name-filter / above table (reads as "name filters first, then tags" — opposite of how AND composition feels).

- **D-02:** **Empty-state: hide the chip strip entirely when the fleet has zero tagged jobs.** Computed from the distinct-tag fold (D-08): when the resulting `Vec<String>` is empty, the entire `<div id="cd-tag-chip-strip">` renders as `hidden` (or skipped via `{% if !tags.is_empty() %}`). Dashboard looks identical to v1.0 until any job has tags. Mirrors how `cd-bulk-action-bar` at `dashboard.html:46` is `hidden` until at least one row is checked.

  **Rejected:** placeholder copy ("No tags configured — add `tags = [...]` to a job in your TOML") — adds chrome operators don't need; the docs/README cover discoverability. **Rejected:** empty bordered area with `?` info-icon — heaviest, needs another tooltip primitive.

- **D-03:** **Mobile / narrow-viewport behavior: `flex-wrap` to multiple rows.** Single `display: flex; flex-wrap: wrap; gap: var(--cd-space-2)` strip — chips reflow to as many rows as needed. Always fully visible, no horizontal scroll, no `<details>` collapse. With the 16-tag-per-job cap (P22 D-08), fleet-wide unique tags can plausibly hit ~30-50 in a busy homelab; wrapping is the readability-honest call.

  **Rejected:** horizontal-scroll single row (some chips always off-screen on mobile; touch-scroll discoverability suffers). **Rejected:** `<details>` collapse threshold (the `<details>` summary doesn't auto-expand on URL-active state — if `/?tag=foo` where chip is collapsed, would need a special render path).

- **D-04:** **CSS class namespace: `cd-tag-chip` + `cd-tag-chip--active` / `cd-tag-chip--inactive`.** New family explicitly named for tags. Mirrors P21's `cd-fctx-*` and `cd-exit-*` namespacing precedent. Container is `cd-tag-chip-strip`. Future tag-related UI (autocomplete in v1.3+) extends naturally into the `cd-tag-*` namespace.

  **Rejected:** generic `cd-chip` (less self-documenting; risks future non-tag chip use drifting the styling). **Rejected:** extend `cd-badge` with `--filter` variant (badges in this codebase are non-interactive read-only labels per design system; muddies semantics).

### Distinct-tag source + ordering (Gray Area 2)

- **D-05:** **Source: Rust-side fold over `DashboardJob`.** Extend `DashboardJob` (`src/db/queries.rs:590`) with `pub tags: Vec<String>`; the dashboard handler folds the loaded job rows into the distinct-tag union. Reuses rows we already fetch — single query, no dialect divergence, mirrors P21 OBS-04 raw-fetch-then-aggregate pattern.

  **Rejected:** SQL `DISTINCT` over JSON column (sqlite has no native JSON unnest; needs `json_each` on sqlite + `jsonb_array_elements_text` on postgres — two divergent dialect arms; breaks the parity-friendly TEXT-family abstraction P22 deliberately maintains; no perf win at homelab scale). **Rejected:** walk in-memory `CronConfig.jobs` at runtime (web handler currently reads everything from DB, no config snapshot in `AppState`; coupling web layer to in-memory config creates a freshness window if config is reloaded mid-request).

- **D-06:** **DB projection: project `j.tags` into the existing `get_dashboard_jobs` SELECT.** Add `j.tags AS tags_json` to both the sqlite and postgres SELECT lists (`src/db/queries.rs:841` and `:895`); deserialize JSON → `Vec<String>` at the row-mapping site (`:877` and `:928`). Single query, single round-trip, identical shape across dialects (TEXT column). Mirrors how `j.enabled_override` was added to the same SELECT for Phase 14.

  **Rejected:** separate query for tags joined Rust-side (two queries for no benefit — the column lives on the same row we're already reading).

- **D-07:** **Tag ordering in the strip: alphabetical.** Stable, predictable, no-info-dependency ordering. Operators learn the position of their commonly-used tags. Matches the sorted-canonical JSON storage form chosen in P22 D-09 — the chip strip order matches what operators see in the DB column and the WH-09 webhook payload. Easiest a11y story (screen reader narrates predictably).

  **Rejected:** by-frequency / most-used-first (order shifts as fleet evolves; muscle memory breaks; "most useful" metric is debatable). **Rejected:** config-declared order (P22 D-09 sorted-canonical JSON form already loses insert order at the storage layer; re-deriving config order needs a second source).

- **D-08:** **Aggregation site: in the `dashboard()` handler after the `get_dashboard_jobs()` call.** Keep DB layer pure (returns rows); composition lives in `src/web/handlers/dashboard.rs`. Handler folds `let fleet_tags: Vec<String> = job_views.iter().flat_map(|j| &j.tags).cloned().collect::<BTreeSet<String>>().into_iter().collect();` and passes alongside the active-tag set. Mirrors P13 OBS-03 sparkline aggregation pattern that lives in the handler not `queries.rs`.

  **Rejected:** inside `get_dashboard_jobs()` returning `(Vec<DashboardJob>, Vec<String>)` (couples two concerns; harder to test the aggregation in isolation). **Rejected:** dedicated `get_distinct_fleet_tags()` query (second query for data we already have in scope).

### Filter SQL + URL parsing + HTMX swap (Gray Area 3)

- **D-09:** **Filter SQL: AND-chained `tags LIKE '%"' || ?N || '"%'` per active tag, plus `tags != '[]'` when active set is non-empty.** Compose onto the existing `get_dashboard_jobs` WHERE (`src/db/queries.rs:818`):

  ```sql
  WHERE j.enabled = 1
    AND LOWER(j.name) LIKE ?1                      -- existing v1.0 name filter
    AND tags LIKE '%"backup"%'                     -- per-active-tag (D-09)
    AND tags LIKE '%"weekly"%'                     -- per-active-tag (D-09)
    AND tags != '[]'                               -- TAG-07 untagged-hidden (gated to active set non-empty)
  ```

  Implementation shape: format-string the count of `AND tags LIKE ?N` clauses from the whitelist-bound active set (active tags are validated by P22 charset+reserved+collision validators at config-load; the runtime active set is filtered to known fleet tags before binding). Bind values per active tag at sequential `?N` / `$N` positions following the existing name-filter `?1`. The `tags != '[]'` clause is statically appended (no bind value) and gated by `if !active_tags.is_empty()`. Parity-friendly across sqlite + postgres without dialect-specific JSON ops. TAG-05 substring-collision validator (P22 D-03) gates fleet config-load, so the LIKE is structurally safe — `back` and `backup` cannot both exist in the same fleet.

  **Rejected:** `json_each` / `jsonb_array_elements_text` per backend (two divergent dialect arms; breaks parity-friendly TEXT-family abstraction; no correctness benefit since TAG-05 prevents substring false-positives). **Rejected:** fetch-all-then-filter-Rust-side (works at homelab scale but wastes rows when the SQL form is trivial).

- **D-10:** **URL extractor: `axum_extra::extract::Query<DashboardParams>`.** Replace `axum::extract::Query<DashboardParams>` (`src/web/handlers/dashboard.rs:242`) with `axum_extra::extract::Query<DashboardParams>` — `axum_extra` is already in tree (Cargo.toml). Add `#[serde(default, rename = "tag")] pub tags: Vec<String>` to `DashboardParams`. `axum_extra::Query` supports repeated keys via `serde_html_form` (unlike `axum::Query` which silently drops duplicates). Single-line change.

  **Rejected:** keep `axum::Query` and parse raw query string manually (reinvents what `axum_extra` already does). **Rejected:** `serde_qs` (adds a new dep for what `axum_extra` does for free; cargo tree invariant holds but adds a crate).

- **D-11:** **HTMX swap mechanics: OOB swap of chip strip + targeted swap of `#job-table-body` in a single response.** Each chip is `<a hx-get="/?filter=...&sort=...&order=...&tag=X&tag=Y" hx-target="#job-table-body" hx-swap-oob="true" hx-push-url="true">`. The dashboard partial response renders BOTH the chip strip (with new active state, OOB-swapped into `#cd-tag-chip-strip`) AND the table body (swapped into `#job-table-body`). Single round-trip, both pieces refresh, no JS needed for the toggle. The `every 3s` poll on `#job-table-body` keeps working unchanged because it targets only the table body.

  **Rejected:** swap a wrapper `#dashboard-content` containing both chips and table (larger swap target trips the existing 3s polling target; risk of UX flicker). **Rejected:** static chips + table-body-only swap (chip clicks would navigate but visual state stays fixed until full page reload — breaks the active=teal/inactive=grey toggle UX explicitly required by TAG-06).

- **D-12:** **Poll filter preservation: hidden `<input type="hidden" name="tag" value="X">` per active tag.** For each active tag, render a sibling hidden input inside the chip strip. Update the existing 3s poll `hx-include` (`dashboard.html:140`) from `"[name='filter'],[name='sort'],[name='order']"` to `"[name='filter'],[name='sort'],[name='order'],[name='tag']"`. Standard HTMX idiom; `axum_extra::Query` deserializes the repeated `name=tag` form fields the same way it does the URL query. Hidden inputs get OOB-replaced together with the chip strip on toggle.

  **Rejected:** `data-tag-name` attribute + selector hx-include (data-attributes don't form-encode automatically; needs hx-vals or custom serializer). **Rejected:** encode active tags into polling URL via hx-vals JSON (JSON keys can't repeat; needs array serialization that `axum_extra` parses correctly — brittle vs the standard form-field approach).

- **D-13:** **Sort-header href composition: each sort-header `<a>` must include `&tag=...` for every active tag in the `href` attribute.** Each sortable column anchor (Name / Next Fire / Status / Last Run at `dashboard.html:88-128`) currently composes hrefs as `?filter={{ filter }}&sort=name&order={% if ... %}desc{% else %}asc{% endif %}`. P23 widens these to also include every active tag: `?filter={{ filter }}&sort=name&order=...&{% for t in active_tags %}tag={{ t }}&{% endfor %}` (or equivalent template idiom). Both `href` and `hx-get` attributes get the same params — the `href` for non-HTMX navigation + copy-link, the `hx-get` for the HTMX swap path. Mechanically required for shareable URL state to round-trip.

  **Note:** template surface gets slightly busier; planner / UI-SPEC may extract a small askama macro or filter to render the active-tag suffix to keep the sort-header anchors readable.

### rc.3 cut + UI-SPEC routing (Gray Area 4)

- **D-14:** **Author `23-UI-SPEC.md` BEFORE planning via `/gsd-ui-phase 23`.** Mirrors P21 discipline — UI-SPEC.md locks typography / color / spacing / markup / copy / a11y / empty-state / hover/focus/active states / keyboard contract / mobile-wrap behavior / chip-label rendering BEFORE planning starts. Implementation becomes plumbing-only (template inserts + CSS additions matching the locked contract). Roadmap labels Phase 23 as "**UI hint**: yes" which signals this routing.

  **Workflow:** `/gsd-discuss-phase 23` (this) → commit CONTEXT.md → `/gsd-ui-phase 23` → commit UI-SPEC.md → `/gsd-plan-phase 23` → `/gsd-execute-phase 23`.

  **Rejected:** inline visual contract in CONTEXT.md / PLAN.md (visual decisions get re-litigated during implementation — P21 hit this when UI-SPEC was missing in earlier phases). **Rejected:** defer UI-SPEC to PLAN-time (highest re-litigation risk; pattern explicitly retired after v1.1).

- **D-15:** **rc.3 cut: mirror P21 D-22..D-26 verbatim.**
  - Reuse `docs/release-rc.md` verbatim (no edits in this phase).
  - `Cargo.toml` stays at `1.2.0` (no in-source version change).
  - `:latest` GHCR tag stays at `v1.1.0` — the `release.yml` hyphen-gate from P12 D-10 enforces this on tags containing `-`.
  - Rolling `:rc` tag updates to `v1.2.0-rc.3` on push.
  - Tag command: `git tag -a -s v1.2.0-rc.3 -m "v1.2.0-rc.3 — dashboard tag filter chips (P23)"`.
  - Pre-flight: P23 PR merged to `main` + green CI + green compose-smoke + `git cliff --unreleased --tag v1.2.0-rc.3` preview clean.
  - Release body: `git-cliff` output is authoritative (per v1.1 P12 D-12). Phase 23 does NOT hand-edit the release body post-publish.
  - Final wave is the autonomous=false `23-RC3-PREFLIGHT.md` — maintainer runs the human UAT scenarios from D-17 + cuts the `v1.2.0-rc.3` tag locally per `docs/release-rc.md`. Plans 23-01..23-NN run autonomously through verification; rc.3 cut is maintainer-only. Mirrors P21 D-26.

- **D-16:** **Phase 23 does NOT modify `release.yml`, `cliff.toml`, or `docs/release-rc.md`.** Any maintainer-discovered runbook gap during the rc.3 cut becomes a hotfix PR before tagging (mirrors v1.1 P12 + v1.2 P20/P21 discipline).

### Test + UAT shape (Gray Area 4 cont.)

- **D-17:** **Test + UAT shape: mirror P22 D-09..D-11.**
  - **New `tests/v12_tags_dashboard.rs`** integration test covering: chip strip render with N tags from a multi-tag fleet, AND filter SQL correctness (jobs with both tags pass; with only one fail), TAG-07 untagged-hidden when active set is non-empty, AND with name-filter (job must match BOTH name LIKE AND every active tag), repeated `?tag=` URL parsing via `axum_extra::Query<Vec<String>>`, sort+chip URL composition (sort header href round-trips active tags), OOB swap response shape (response contains both `#cd-tag-chip-strip` and `#job-table-body` content). Runs in the existing CI matrix (`linux/{amd64,arm64} × {SQLite, Postgres}`).
  - **Extend `src/web/handlers/dashboard.rs::tests`** with unit tests for the handler-side fold (distinct-tag union from `Vec<DashboardJob>`) + active-set parsing (empty `Vec<String>`, single tag, multi-tag, deduplication of duplicate `?tag=foo&tag=foo`).
  - **Three new `just` recipes** mirroring the P22 `uat-tags-*` family (recipe-calls-recipe pattern):
    - `uat-chips-render` — seed a multi-tag fleet → start cronduit → walk dashboard → confirm chip strip renders with every distinct tag, alphabetical, hidden-when-empty.
    - `uat-chips-and-filter` — toggle two chips → confirm AND semantics (only jobs with both tags appear) + untagged jobs hidden + composes with name-filter via AND.
    - `uat-chips-share-url` — paste a shareable URL with `?tag=backup&tag=weekly` directly → confirm chips render in active state on page load + URL push round-trips on toggle.
  - **`23-HUMAN-UAT.md`** autonomous=false maintainer plan covering the scenarios above plus mobile viewport (chip strip wraps below 640px), light-mode rendering, keyboard navigation (Tab onto chips; Enter/Space toggles), screen-reader narration of active state, and end-to-end with the v1.0 name-filter combined with active chips.

  **Rejected:** browser-based playwright smoke test (adds new test infrastructure not in tree; v1.3 candidate at most). **Rejected:** lighter — just extend existing dashboard handler tests (SQL composition + URL extractor + OOB swap mechanics are exactly where bugs hide; integration test is mandatory).

### Universal project constraints (carried forward)

> The decisions below are **[informational]** — repo-wide process constraints honored by absence (mermaid-only diagrams, PR-only branch state, maintainer-validated UAT, just-recipe UAT). They are not phase-implementation tasks.

- **D-18:** [informational] All Phase 23 changes land via PR on a feature branch. No direct commits to `main`. (Project memory `feedback_no_direct_main_commits.md`.)
- **D-19:** [informational] All diagrams in any Phase 23 artifact (PLAN, UI-SPEC, SUMMARY, README addition, PR description, code comments) are mermaid code blocks. No ASCII art. (Project memory `feedback_diagrams_mermaid.md`.)
- **D-20:** [informational] UAT recipes reference existing/new `just` commands per D-17; no ad-hoc `cargo` / `docker` / curl-URL invocations. (Project memory `feedback_uat_use_just_commands.md`.)
- **D-21:** [informational] Maintainer validates UAT — Claude does NOT mark UAT passed from its own runs. (Project memory `feedback_uat_user_validates.md`.)
- **D-22:** [informational] Tag and version match — `Cargo.toml` stays at `1.2.0` (matches `v1.2.0-rc.3` tag's prefix per project memory `feedback_tag_release_version_match.md`); the `-rc.3` is tag-only.
- **D-23:** [informational] `cargo tree -i openssl-sys` must remain empty. Phase 23 adds zero new external crates — `axum_extra` is already in tree (used elsewhere in the codebase); `serde_html_form` is its transitive dep, not a new direct dep. No new TLS/cross-compile surface.

### Claude's Discretion

The planner picks freely on each of the following — none of these were discussed in the gray-area selection:

- **Plan count and grouping.** A natural split is (1) `DashboardJob.tags` field add + `get_dashboard_jobs` SELECT widening + JSON deserialize at row-mapping; (2) AND-chained `LIKE` filter SQL + `tags != '[]'` clause + `axum_extra::Query<DashboardParams>` extractor swap; (3) `dashboard()` handler distinct-tag fold + active-set wiring + template view-model fields; (4) `23-CSS` chip primitive + chip strip template insert + sort-header href widening + hidden-input filter preservation; (5) integration tests + handler unit tests + `just` recipes; (6) `23-RC3-PREFLIGHT.md` + `23-HUMAN-UAT.md`. Planner may collapse plans (e.g., 1+2 into one DB-layer plan) or expand. Atomic-commit-per-plan per project convention.
- **Per-tag job count badge on chips** (e.g., `backup (3)`). Computed Rust-side from the same fold for free. UI-SPEC.md decides whether to render it; both shapes (`backup` vs `backup (3)`) are CSS-friendly. If rendered, the count refreshes via the same OOB chip-strip swap on toggle.
- **Chip label rendering shape** — plain text (`backup`) vs prefixed (`# backup`) vs CSS-decorated. UI-SPEC.md owns this.
- **Whether to include disabled-job tags in the fleet-tag union.** `get_dashboard_jobs` already filters `WHERE j.enabled = 1`, so the natural decision is "tags from the rendered row set" — disabled-job tags don't appear in the chip strip. Planner / UI-SPEC may decide otherwise (e.g., always show all fleet tags including disabled jobs' tags) but the default-and-cheapest is to mirror what's rendered.
- **Whether the active-tag URL is canonicalized (sorted alphabetically before push-url).** Recommendation: sort the active set alphabetically before serializing back to URL so `/?tag=weekly&tag=backup` and `/?tag=backup&tag=weekly` produce the same shareable URL. Makes copy-link deterministic; matches the alphabetical chip strip ordering. Planner picks; if insert-order is chosen, document it.
- **Stale-tag handling** — what happens when a URL contains a tag that's no longer in the fleet (e.g., bookmarked URL after operator removed `tags = ["foo"]` from a job). Recommendation: silently drop the unknown tag from the active set during deserialization (the fold sees only known tags; unknown tags can't match any LIKE clause anyway, but skipping them keeps the chip strip and active set in sync). Planner picks.
- **Sort-header href template idiom** — repeating `{% for t in active_tags %}tag={{ t }}&{% endfor %}` inline at L88-128 four times (once per sortable column) is busier than ideal. Planner may extract a small askama macro / filter (`{{ active_tags|tag_query }}`) for readability. Inline is fine if planner picks it.
- **Template file split** — chip strip can be a sibling partial `templates/partials/chip_strip.html` included from `dashboard.html`, or live inline. Mirrors how `partials/job_table.html` is split out from `dashboard.html:141`. Planner picks; the OOB swap response composes both partials cleanly either way.
- **README addition** — a short subsection on tag filtering (forward from P22's deferred ideas list). Mirrors P17 D-04 labels precedent. Optional in this phase; if planner picks it up the shape is: "Tag your jobs in TOML; the dashboard auto-renders chips for filtering."
- **`23-CSS` placement** — `assets/src/app.css` `@layer components` is the established pattern for new component primitives (P21 added `cd-fctx-*` and `cd-exit-*` there). Planner adds `cd-tag-chip-*` to the same layer.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level

- `.planning/PROJECT.md` § Current Milestone (v1.2 scope; tagging is one of five v1.2 features; dashboard chips are the consumer half of TAG) and § Constraints (locked tech stack — `sqlx`, TOML, askama+askama_web with axum-0.8 feature, rustls invariant, Tailwind standalone, CSS-only chip primitive per TAG-08).
- `.planning/REQUIREMENTS.md` § Job Tagging / Grouping (TAG) — **TAG-06**, **TAG-07**, **TAG-08** are the canonical requirement IDs Phase 23 satisfies. Verification anchors: T-V12-TAG-08, T-V12-TAG-09, T-V12-TAG-10, T-V12-TAG-11. Note: TAG-06's "active=teal-bordered+bold; inactive=grey" + URL state contract is LOCKED text in REQUIREMENTS — the UI-SPEC inherits this verbatim.
- `.planning/ROADMAP.md` § "Phase 23: Job Tagging Dashboard Filter Chips — rc.3" (L279-292) — goal, five operator-observable success criteria (the fifth is the rc.3 GHCR publish criterion), depends-on (Phase 22), UI hint = yes (signals UI-SPEC routing).
- `.planning/STATE.md` — current phase state; v1.2 milestone progress; Phase 22 was the last milestone PR shipped (#58).

### Phase 22 precedent (closest data-layer analog — SCHEMA SHIPPED)

- `.planning/phases/22-job-tagging-schema-validators/22-CONTEXT.md` — the entire tagging foundation Phase 23 reads from. Specifically:
  - `<decisions>` D-01 (tags excluded from `compute_config_hash`) and D-02 (tags excluded from `serialize_config_json`) — explain why the dashboard can read `jobs.tags` directly without worrying about hash/snapshot semantics.
  - D-04 (validator order: normalize → reject → dedup → fleet check) — Phase 23 trusts the validators ran at config-load; the runtime active-tag set always contains charset-clean, lowercase, deduplicated tags.
  - D-08 (16-tag per-job cap) — caps the chip count next to a single row; the fleet-wide chip strip can plausibly hit 30-50.
  - D-09 (sorted-canonical JSON storage) — the chip strip alphabetical ordering (D-07) matches what's in the column.
  - `<canonical_refs>` § Source files the phase touches — same files Phase 23 widens (`src/db/queries.rs:818`, `src/web/handlers/dashboard.rs`, `templates/pages/dashboard.html`, `assets/src/app.css`).
- `.planning/phases/22-job-tagging-schema-validators/22-PATTERNS.md` — the four-validator family + JSON-column read pattern. Phase 23 inherits the Vec<String> shape end-to-end.
- `migrations/sqlite/20260504_000010_jobs_tags_add.up.sql` and the postgres pair — the column Phase 23 reads from. Read header comments for the additive-forever shape + parity invariant.

### Phase 21 precedent (closest UI + rc cut analog)

- `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-CONTEXT.md` § rc.2 Tag Cut (D-22..D-26) — the verbatim shape Phase 23 mirrors for the rc.3 cut. D-26 in particular for the autonomous=false `*-PREFLIGHT.md` final-wave pattern.
- `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-UI-SPEC.md` — the template for what `23-UI-SPEC.md` should produce. Read for the Component Inventory / Tokens / Copywriting Contract / Output Escaping section shapes.
- `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-11-PLAN.md` — `21-RC2-PREFLIGHT.md` plan shape. Phase 23's `23-RC3-PREFLIGHT.md` mirrors structurally (preflight checklist + literal tag command + autonomous=false maintainer wave).

### Phase 20 precedent (rc.1 cut shape — earlier rc cut)

- `.planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-RC1-PREFLIGHT.md` — sibling preflight precedent for `23-RC3-PREFLIGHT.md`. Same shape, same maintainer-cuts-locally posture.

### Phase 14 precedent (similar dashboard-handler widening)

- `src/web/handlers/dashboard.rs:115` `to_view` — the `is_disabled` field added by Phase 14 mirrors how Phase 23 will add the per-job tag handling to the view model (per-row data + global active-set).
- `src/db/queries.rs:603` `enabled_override` field on `DashboardJob` — direct precedent for adding `tags: Vec<String>` to the same struct (D-05).
- `templates/pages/dashboard.html:46` `cd-bulk-action-bar` `hidden` until relevant — the empty-state pattern D-02 mirrors.

### Phase 13 precedent (handler-side aggregation pattern)

- `src/web/handlers/dashboard.rs:262` sparkline hydration — the pattern D-08 mirrors for the distinct-tag fold living in the handler not `queries.rs`.

### Source files the phase touches

- `src/db/queries.rs:590` (`DashboardJob` struct) — add `pub tags: Vec<String>` field per D-05. Field placement is planner discretion.
- `src/db/queries.rs:818` (`get_dashboard_jobs`) — extend SELECT with `j.tags AS tags_json` per D-06; deserialize JSON at row-mapping site for both sqlite (`:877`) and postgres (`:928`) branches; compose AND-chained `tags LIKE` predicates + `tags != '[]'` clause per D-09.
- `src/web/handlers/dashboard.rs:23-31` (`DashboardParams`) — add `#[serde(default, rename = "tag")] pub tags: Vec<String>` field per D-10.
- `src/web/handlers/dashboard.rs:242` (`Query<DashboardParams>` extractor) — replace with `axum_extra::extract::Query<DashboardParams>` per D-10.
- `src/web/handlers/dashboard.rs:46-53` (`DashboardPage`) and `:55-60` (`JobTablePartial`) — add `fleet_tags: Vec<String>` and `active_tags: Vec<String>` template fields per D-08; widen `JobTablePartial` to also carry the chip strip data so the OOB swap response includes both pieces (D-11).
- `src/web/handlers/dashboard.rs:115` (`to_view`) — extend the view to carry per-job tags (used at minimum for the row-level data; planner may choose to skip this if chip-only display is sufficient).
- `src/web/handlers/dashboard.rs:262` (sparkline hydration site) — sibling pattern; the distinct-tag fold lives nearby.
- `templates/pages/dashboard.html:19-36` (filter row) — insert new chip strip ABOVE this per D-01.
- `templates/pages/dashboard.html:88-128` (sort-header anchors) — widen each anchor's `href` and `hx-get` to include `&tag=...` for every active tag per D-13.
- `templates/pages/dashboard.html:138-141` (job-table-body 3s poll) — update `hx-include` to add `[name='tag']` per D-12.
- `templates/partials/job_table.html` — no change unless planner extracts the chip strip into a sibling partial.
- `assets/src/app.css` `@layer components` — add `cd-tag-chip-strip`, `cd-tag-chip`, `cd-tag-chip--active`, `cd-tag-chip--inactive` per D-04 (UI-SPEC.md authoritative for the actual values).

### NEW test files

- `tests/v12_tags_dashboard.rs` — integration test per D-17. Runs in the existing CI matrix.
- (Extension only) `src/web/handlers/dashboard.rs::tests` — handler-side fold + active-set parsing unit tests per D-17.

### Release runbook (UNCHANGED — reused verbatim)

- `docs/release-rc.md` — the rc cut runbook. Phase 23 reuses verbatim per D-15. NO edits in this phase.
- `.github/workflows/release.yml` — `:latest` hyphen-gate from P12 D-10 enforces the `:latest` pin during rc cuts. Unchanged.
- `cliff.toml` — `git-cliff` config. Authoritative source for release body. Unchanged.

### Cross-reference

- `.planning/REQUIREMENTS.md` TAG-06..08 + WH-09 + LBL — Phase 23's SUMMARY should mark TAG-06..08 Validated end-to-end (TOML → validators → column → chip strip → AND filter → URL state) and note that v1.2 tagging is feature-complete pending P24 close-out.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`DashboardJob` struct** (`src/db/queries.rs:590`) — direct extension point for D-05. Existing `enabled_override: Option<i64>` field added by Phase 14 is the structural precedent.
- **`get_dashboard_jobs` SELECT** (`src/db/queries.rs:841` sqlite, `:895` postgres) — direct widening site for D-06. Both backends have identical column lists; tags projection slots in alongside `j.enabled_override`.
- **`get_dashboard_jobs` WHERE clause** (`src/db/queries.rs:849` sqlite, `:903` postgres) — direct composition site for D-09. Existing `WHERE j.enabled = 1 AND LOWER(j.name) LIKE ?1` is the predicate AND-chain Phase 23 extends.
- **`DashboardParams`** (`src/web/handlers/dashboard.rs:23-31`) — direct extension point for D-10. `#[derive(Default)]` + `#[serde(default = "...")]` patterns already in use; `tags: Vec<String>` slots in.
- **`axum_extra` crate** — already in tree per Cargo.toml. Direct swap target for D-10's URL extractor.
- **`dashboard()` handler** (`src/web/handlers/dashboard.rs:239`) — direct insertion point for D-08's distinct-tag fold (after `get_dashboard_jobs` returns, before template construction). Sparkline hydration loop at `:262` is the structural precedent.
- **`JobTablePartial`** (`src/web/handlers/dashboard.rs:55-60`) — extension point for the partial response shape that carries both chip strip data and table rows for D-11's OOB swap.
- **`HxRequest` extractor** (`src/web/handlers/dashboard.rs:240`) — already detects HTMX vs full-page requests; the OOB+target swap response branch lives in the `if is_htmx` arm at `:341`.
- **`cd-bulk-action-bar`** (`templates/pages/dashboard.html:46`) — `hidden`-until-relevant pattern for D-02 empty-state.
- **Filter row at L19-36** — direct visual neighbor for D-01 chip strip placement (above this row).
- **Sort-header anchors at L88-128** — direct widening site for D-13 href composition. Existing template string idiom is a single-line `?...` query; adding active-tag suffix is mechanical.
- **3s polling on `#job-table-body`** (`dashboard.html:138-141`) — direct extension site for D-12 `hx-include` widening. Existing `[name='filter'],[name='sort'],[name='order']` adds `,[name='tag']`.
- **`cd-badge`** primitive (`design/DESIGN_SYSTEM.md` L196-211) — visual primitive REJECTED for chips per D-04 but documents the existing pill/badge baseline UI-SPEC.md will reference for visual coherence.

### Established Patterns

- **Parity-friendly LIKE pattern across sqlite + postgres.** P22 D-03 confirmed substring-collision validation gates the LIKE structurally; Phase 23 inherits this guarantee. No JSON-specific SQL on either backend.
- **Format-string SQL with whitelist-bound active set.** Existing `get_dashboard_jobs` already format-strings the `ORDER BY` clause from a whitelist (`:825-835`). Phase 23 mirrors this for the variadic `AND tags LIKE ?N` chain — the count comes from `active_tags.len()` (a server-controlled set), and the bind values come from `active_tags.iter()`. NO user-controlled SQL string interpolation.
- **Handler-side aggregation over fetched rows.** P13 OBS-03 sparkline + P21 EXIT-* histogram both aggregate Rust-side from the rows already in scope. Phase 23 fold continues the pattern.
- **OOB swap + targeted swap composition.** Standard HTMX 2.0 idiom; documented in HTMX docs but not yet used elsewhere in the codebase. Phase 23 introduces the pattern; UI-SPEC.md may codify it for future reuse.
- **`hx-include` chains for poll-time filter preservation.** Existing 3s poll at `dashboard.html:138-141` already includes filter/sort/order; Phase 23 extends with `[name='tag']`. Pattern stays identical.
- **Single-binary asset embedding.** `rust-embed` (Cargo.toml) embeds `assets/static/app.css`; the new `cd-tag-chip-*` family compiles into the same CSS bundle via Tailwind standalone build (no edit-loop change). Project memory: HTMX vendored, no CDN.
- **CI matrix unchanged.** `linux/{amd64,arm64} × {SQLite, Postgres}` covers all of Phase 23. New tests run inside the existing test job. No new feature flag, no new lint gate.

### Integration Points

- **`DashboardJob.tags` field add** + JSON deserialize at row-mapping site.
- **`get_dashboard_jobs` SELECT widening** (project `j.tags` into both backend SELECT lists).
- **`get_dashboard_jobs` WHERE composition** (AND-chained `tags LIKE` predicates + `tags != '[]'` clause).
- **`DashboardParams.tags` field add** + extractor swap to `axum_extra::Query`.
- **`dashboard()` handler distinct-tag fold** + active-set wiring.
- **`DashboardPage` / `JobTablePartial` template view-model widening** to carry chip strip data.
- **Template inserts**: chip strip above filter row, sort-header href widening, 3s poll hx-include widening.
- **CSS additions**: `cd-tag-chip-strip` + `cd-tag-chip` + `--active` / `--inactive` variants in `@layer components`.
- **Integration test** (`tests/v12_tags_dashboard.rs`) + handler unit tests + 3 new `just` recipes + `23-HUMAN-UAT.md` + `23-RC3-PREFLIGHT.md`.
- **No `release.yml` / `cliff.toml` / `docs/release-rc.md` change** per D-15 / D-16.
- **No `serialize_config_json` change** (P22 D-02 — tags not in execution snapshot).
- **No `compute_config_hash` change** (P22 D-01 — tags not in hash input).
- **No webhook payload change** (WH-09 already shipped in P22 D-05; tags reach receivers via the existing payload field).

</code_context>

<specifics>
## Specific Ideas

- **The chip strip is the most operator-visible v1.2 feature.** Webhooks (P18-20), labels (P17), FCTX panel (P21), exit histogram (P21), and tagging schema (P22) all take per-feature setup or specific failure scenarios to see. The chip strip lights up the moment any job has tags. UI-SPEC.md should treat the visual contract with proportional care.

- **`axum_extra::Query` is the load-bearing technical pivot.** The repeated `?tag=` URL contract from TAG-06 cannot be deserialized by `axum::Query` (silently drops duplicates). Verify upfront in research that `axum_extra::extract::Query<DashboardParams>` with the `serde_html_form` body (default in axum-extra 0.12.x) actually supports the repeated-key shape — the docs claim it but a quick `serde_html_form` round-trip test is worth the planner's first integration test.

- **OOB swap is new to this codebase.** No existing template uses `hx-swap-oob`. The pattern is well-documented in HTMX 2.0 docs (htmx.org/attributes/hx-swap-oob) but Phase 23 introduces it. UI-SPEC.md should call out the OOB target ID (`#cd-tag-chip-strip`) explicitly + document the contract that the partial response renders both `#cd-tag-chip-strip` (OOB) and the table body (target) in the SAME response. Future phases reusing OOB get a precedent to follow.

- **Sort-header href composition is the most fragile template surface.** Each of the four sortable columns (Name / Next Fire / Status / Last Run) at L88-128 currently has a single-line `href=?...` with sort/order/filter. Adding active-tag iteration inline four times bloats the markup. Strongly consider an askama macro / filter (`{{ active_tags|tag_query }}`) for readability — the planner should call this out in PLAN.md if not done at template-author time.

- **Stale-tag URLs from old bookmarks are a real edge case.** When an operator removes `tags = ["foo"]` from a job and reloads a `?tag=foo` bookmark, the handler should silently drop the unknown tag (Claude discretion noted in `<decisions>`). A test case in `tests/v12_tags_dashboard.rs` for "URL contains a tag no job has" is worth including; the natural behavior is "active set deserializes empty → chip strip renders no active state → table renders all jobs" but explicit test coverage prevents regressions.

- **Phase 23 ships within rc.2 → rc.3 cycle.** The PR description should cross-reference rc.3 readiness target so the milestone close-out audit (Phase 24) has a clear input.

- **Phase 22's "see Phase 23 dashboard chips" forward-reference.** P22 deferred the README configuration subsection on tags as Claude's discretion (P22 deferred ideas list). Phase 23 is the natural place to land that subsection — the labels-precedent README from P17 D-04 is the template if planner picks it up. Optional but recommended now that the operator has a complete picture (TOML → validators → column → webhook payload → dashboard chips).

- **Tag count badge on chips (e.g., `backup (3)`)** is a UI-SPEC decision. The count is computed Rust-side from the same fold for free. Both shapes (`backup` vs `backup (3)`) are CSS-friendly. UI-SPEC.md owns the call.

</specifics>

<deferred>
## Deferred Ideas

- **Tag autocomplete in the chip strip / search-as-you-type** — UI-SPEC may sketch the gesture but implementation is deferred to v1.3 (P22 deferred ideas list).
- **Tag-based bulk operations** (bulk enable/disable BY TAG) — explicit v1.3 candidate per `.planning/REQUIREMENTS.md` § Out of Scope. The bulk-action bar (`dashboard.html:46`) is row-checkbox-based in v1.2.
- **Tag chips on `/jobs/{id}` (job detail) page** — Phase 23 is dashboard-only per TAG-06; job-detail tag display is deferred to a future phase.
- **Tags as Prometheus label** — explicit out-of-scope (cardinality discipline; same posture as labels and exit codes per EXIT-06).
- **Tag-based webhook routing keys** — WH-09 carries tags in payload but never AS a routing key. Same posture as label-based routing. Out of v1.2 (P18 D-17 lock; reaffirmed P22 deferred).
- **Per-tag job count badge on chips** (`backup (3)`) — Claude's-discretion / UI-SPEC call. If not picked up in Phase 23, deferred to v1.3 polish.
- **Browser-based playwright smoke test for HTMX chip clicks** — adds new test infrastructure not in tree; v1.3 candidate at most.
- **Sort-header href askama macro** for active-tag suffix — Claude's discretion. If planner picks inline iteration, the macro is a v1.3 readability cleanup candidate.
- **Active-set URL canonicalization** (sort alphabetically before push-url) — Claude's discretion / planner picks. Recommendation is to canonicalize so copy-link is deterministic; if insert-order is chosen, document it.
- **Stale-tag handling beyond silent-drop** (e.g., "render disabled chip with strikethrough" for tags in URL but not in fleet) — over-engineered for v1.2; silent-drop is sufficient.
- **README configuration subsection on tag filtering** — Claude's discretion; if not landed in Phase 23, deferred to Phase 24 close-out documentation pass.
- **Tag display on disabled jobs** (currently filtered out by `WHERE j.enabled = 1`) — out of scope; the dashboard hides disabled jobs entirely so the chip strip naturally excludes their tags.
- **`docs/release-rc.md` modifications** — not in this phase; rc.3 reuses the runbook verbatim per D-15 / D-16 (mirroring P20 D-30 / P21 D-22..D-26).

</deferred>

---

*Phase: 23-job-tagging-dashboard-filter-chips-rc-3*
*Context gathered: 2026-05-04*
