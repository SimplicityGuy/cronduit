# Phase 13: Observability Polish (rc.2) - Context

**Gathered:** 2026-04-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 13 delivers the three observability surfaces called out by OBS-01..OBS-05, then cuts the `v1.1.0-rc.2` tag:

1. **Dashboard sparkline + success-rate badge** (OBS-03) — a 20-cell status-colored sparkline plus a success-rate badge on every job in the dashboard table. Sample-size honest (N<5 renders `—`); `stopped` runs are visible in cells but excluded from the success-rate denominator. Zero-run jobs never crash the view.
2. **Job-detail duration p50/p95** (OBS-04, OBS-05) — a new "Duration" card on the job-detail page showing `p50 Xs` and `p95 Ys` over the last 100 successful runs. N<20 renders `—` with a tooltip. Percentile is computed in Rust via `src/web/stats.rs::percentile(samples, q)` — no SQL `percentile_cont`, even on Postgres, per the structural-parity constraint.
3. **`/timeline` page** (OBS-01, OBS-02) — a row-per-job gantt-style timeline for the last 24h (default) or 7d (toggle), rendered as inline server-side HTML + CSS grid. Single SQL query, `LIMIT 10000`, `idx_job_runs_start_time` verified via `EXPLAIN QUERY PLAN`. All timestamps in the operator's configured server timezone.
4. **`v1.1.0-rc.2` tag cut** — reuses the release mechanics locked in Phase 12 (release.yml D-10 patches + `docs/release-rc.md` runbook + `:rc` rolling tag). No new release-engineering work; just the tag + release notes.

**Out of scope (deferred to other phases):**
- Bulk enable/disable — Phase 14.
- Final `v1.1.0` GA promotion / `:latest` advancement — Phase 14 close-out.
- Drill-down analytics / Prometheus-scrape panels — post-v1.1.
- Multi-day trend graphs beyond the 7d toggle — post-v1.1.
- SQL-native percentile on Postgres — permanently rejected by OBS-05 structural parity.
- SSE live updates on timeline — 30s HTMX poll is sufficient for v1.1.

</domain>

<decisions>
## Implementation Decisions

### Dashboard Sparkline + Success-Rate Badge (OBS-03)

- **D-01:** **Placement: a new "Recent" column between `Last Run` and `Actions` in `templates/partials/job_table.html`.** Sparkline on top row of the cell, success-rate badge muted beneath. Keeps existing column meaning and HTMX sortability (name / next_run / status / last_run) untouched. The dashboard stays a table — no card-grid restructure, no accordion sub-rows. Smallest diff against Phase 10/11 template changes.

- **D-02:** **Cell rendering: 20 uniform-height status-colored bars.** Each bar is equal width and equal height; color comes from `var(--cd-status-success|failed|timeout|cancelled|stopped|running)` tokens already shipped in Phase 10 D-08. Jobs with fewer than 20 runs render the extra slots as transparent/border-only placeholders. No height-encoded duration, no letter glyphs, no two-row mini-chart. Scannable at `--cd-text-base` 14.4px; minimal layout risk.

- **D-03:** **Success-rate badge format: percent only, with muted count on hover.** Display `95%`; the `title` attribute exposes the denominator as `title="19 of 20 non-stopped runs"`. Keeps the column narrow. Below N=5 renders `—` (REQ lock OBS-03). Integer percent; round half-up. No ratio, no N= suffix, no inline denominator.

- **D-04:** **Sparkline interactivity: per-cell hover tooltip via native `title` attribute; no click-through.** Each cell carries `title="#{N} {status} {duration_display} {relative_time}"`. Operators drill into runs via the existing job-name link (row level) or via the run-history table on the job-detail page. Rejects click-through because 20 tiny hit targets per row creates accidental-click risk and mangles keyboard navigation.

- **D-05:** **Sparkline sample inclusion: last 20 terminal runs (including `stopped`), denominator excludes `stopped`.** Cells visualize ordering: status ∈ `{success, failed, timeout, cancelled, stopped}`. Stopped cells render in `--cd-status-stopped` (grey) but are NOT counted in the denominator — matches REQ lock "stopped runs excluded from the denominator". Running runs do NOT render as sparkline cells (deferred to timeline surface). Denominator = terminal_count - stopped_count; numerator = success_count. This explicitly rejects skipping stopped runs from the cells (operators lose visibility into manual interrupts) and rejects lenient "any last 20 runs including running" (cells would mutate per poll cycle).

- **D-06:** **Zero-run jobs: render 20 transparent cells + `—` badge.** Job card never crashes; sparkline area still reserves space so table rows stay aligned. REQ lock OBS-03: "zero-run jobs never crash the view".

### Timeline Page (OBS-01, OBS-02)

- **D-07:** **Layout: row-per-job gantt, alphabetical by job name.** Each enabled job gets a horizontal row; bars span from `start_time` to `end_time` along a shared X-axis. Alphabetical ordering is stable across reloads — operators don't lose their place when a different job runs. Rejects "sort by most-recent-run" (row-shuffle harms scan-ability) and "single-lane flame-graph" (loses the per-job mental model). Disabled and hidden jobs do NOT appear (REQ lock).

- **D-08:** **Window toggle: pill buttons linked via `?window=24h`/`?window=7d`.** Two `<a>` elements styled as pill buttons at the top of the page. Active pill highlighted with `--cd-text-accent`. URL reflects state → back-button + bookmark + Slack-link friendly. Default is 24h when the param is absent. Matches the existing `filter`/`sort`/`order` query-string pattern from `dashboard.html`. Explicitly rejects dropdown select (invites 1h/12h/30d scope creep) and client-side-only toggle (breaks URL sharing).

- **D-09:** **Bar hover: rich inline HTML tooltip via CSS `:hover`.** Each bar is a positioned container with a child `.cd-tooltip` node; CSS toggles `visibility: hidden → visible` on hover. Tooltip shows `{job_name} #{N}`, a status dot + status label, duration, and `start_time → end_time` (both in server timezone). Also emit a fallback `title="..."` attribute on every bar for (a) touch-device long-press and (b) screen-reader announcement. Adds ~50 LOC CSS under a new `cd-timeline-*` selector family. Does NOT introduce JS.

- **D-10:** **Bar click-through: `<a href="/jobs/{job_id}/runs/{run_id}">` to the specific run detail.** Each bar is a proper anchor element; the existing run-detail page is the drill-down target. Keeps the timeline a pure navigation surface (no modal, no in-page expand). Works as middle-click / cmd-click "open in new tab". Running bars link to the in-flight run detail (which already handles the live SSE log stream from Phase 11).

- **D-11:** **Running runs render as pulsing bars extending from `start_time` to server `now`.** Color = `var(--cd-status-running)` (blue). CSS `@keyframes cd-pulse` subtly animates `opacity: 1 → 0.7 → 1` on a 2s cycle so operators visually recognize "this is live". On the next 30s HTMX poll the bar either redraws extended (still running) or switches to the terminal color (finalized). Rejects "hide running runs" (timeline incomplete during active work), "thin 1px placeholder" (loses elapsed-time signal), and "extend to window edge" (visually misleading about elapsed time).

- **D-12:** **Auto-refresh: HTMX poll every 30s on the timeline main container.** `<div id="timeline-body" hx-get="/timeline" hx-trigger="every 30s" hx-swap="outerHTML" hx-include="[name='window']">`. Cadence deliberately slower than the dashboard's 3s polling (D-01 in Phase 3) because timeline's single SQL query is heavier (10k-row cap). Preserves query param via `hx-include` so the 30s poll doesn't clobber the operator's window choice. No SSE — polling is sufficient for the observability surface.

- **D-13:** **Nav discoverability: add "Timeline" to `templates/base.html` nav between "Dashboard" and "Settings".** Adds a `{% block nav_timeline_active %}{% endblock %}` pattern mirroring `nav_dashboard_active` / `nav_settings_active`. Rejects dashboard-button-only discoverability (less prominent; timeline is a first-class surface, not a detail of the dashboard).

- **D-14:** **Empty-window state: render the axis + centered message inline.** When the window contains zero runs, the axis still renders with tick labels (so operators see the correct time range), and a centered message reads "No runs in the last 24h. Try widening the window to 7d." (or vice-versa for 7d). Rejects full-page empty state that hides the axis.

### Job-Detail Duration p50/p95 (OBS-04)

- **D-15:** **Placement: new "Duration" card between "Configuration" and "Run History" on the job-detail page.** Own card with `<h2>Duration</h2>` heading, consistent with the existing "Configuration" card shape. Rejects header-chip placement (tight on space, competes with Run Now), Configuration-card nesting (mixes static config with derived stats), and run-history-header placement (below the fold on long pages).

- **D-16:** **Value format: labeled chips — `p50 1m 34s` + `p95 2m 12s`.** Human-readable duration using the SAME formatter as the `Duration` column in `templates/partials/run_history.html`'s rendering of `run.duration_display`. Formats: `850ms` (<1s), `42s` (1s..59s), `1m 34s` (1m..59m), `2h 15m` (1h+). Rejects raw seconds (diverges from table below on same page), HH:MM:SS clock form (novel for this app), and delta-vs-p50 form (forces mental math).

- **D-17:** **Sub-threshold state (N<20): both values render as `—` with a `title` attribute.** Title content: `"insufficient samples: need 20 successful runs, currently have {N}"`. Card remains visible so operators know the feature exists and how close they are to it appearing. Rejects hiding the card (silent discoverability), partial-with-dagger rendering (teaches trust in small-N stats; violates OBS-04 "render as `—` instead of meaningless numbers"), and error-banner treatment (too alarmist for a non-problem).

- **D-18:** **Sample-count disclosure: muted subtitle — `last 87 successful runs`.** Styled with `--cd-text-secondary` (same treatment as "Resolved to ..." in the Configuration card). When N ≥ 20 and < 100, the subtitle reports the actual count (e.g., `last 42 successful runs`); when N ≥ 100, it caps at `last 100 successful runs`. When N < 20, the subtitle reports progress (`{N} of 20 successful runs required`). Rejects inline `(n=87)` parentheticals (jargon), tooltip-only (hides real data), and omitting entirely (operators can't gauge statistical confidence).

### Math Conventions & Helper Contract

- **D-19:** **Percentile algorithm: nearest-rank, 1-indexed.** Helper signature: `fn percentile(samples: &[u64], q: f64) -> Option<u64>`. Implementation sorts a copy, computes `rank = ceil(q * n) as usize`, indexes `sorted[rank.saturating_sub(1).min(n-1)]`. Returns `None` for empty input; `Some(v)` otherwise. Property: result is ALWAYS an observed sample — never an interpolated value that didn't actually occur. Rejects linear interpolation (synthetic values confuse operators cross-checking against run_history), lower-rank (biases low), and upper-rank (biases high).

- **D-20:** **p50/p95 input set: strict `status = 'success'` only.** Matches REQUIREMENTS.md OBS-04 wording "last 100 successful runs". Excludes `failed`, `timeout`, `cancelled`, `stopped`, `running`. SQL: `SELECT duration_ms FROM job_runs WHERE job_id = ?1 AND status = 'success' AND duration_ms IS NOT NULL ORDER BY id DESC LIMIT 100`. Rationale: mixing failure/timeout durations into a latency distribution skews p95 toward timeout_s and misrepresents typical work duration. Explicitly rejects the three lenient variants.

- **D-21:** **stats.rs helper edge cases (locked test cases for T-V11-DUR-01..04):**
  - `percentile(&[], 0.5)` → `None`
  - `percentile(&[42], 0.5)` → `Some(42)` (single element)
  - `percentile(&[42], 0.95)` → `Some(42)` (single element, any q)
  - `percentile(&[10, 20, 30, 40, 50, 60, 70, 80, 90, 100], 0.5)` → `Some(50)`
  - `percentile(&[10, 20, 30, 40, 50, 60, 70, 80, 90, 100], 0.95)` → `Some(100)`
  - Pre-sorted and reverse-sorted inputs produce identical output (sort is internal).
  - Consumer (`job_detail` handler) is responsible for applying the N<20 threshold BEFORE calling `percentile()`; the helper itself doesn't enforce thresholds.

### rc.2 Release Mechanics

- **D-22:** **Reuse Phase 12 release artifacts verbatim — no new release-engineering work.** The `release.yml` D-10 patches (pre-release tag gating, `:rc` rolling tag), `docs/release-rc.md` maintainer runbook (D-11), and manual-tag-cut policy (D-13) all ship as-is for rc.2. The close-out plan's only release-adjacent task is: (a) verify Phase 12.1's `:main` + `:latest`-pin is still healthy via `scripts/verify-latest-retag.sh`, and (b) follow `docs/release-rc.md` to cut `v1.1.0-rc.2`. No workflow file edits, no runbook edits.

- **D-23:** **Release notes: `git-cliff` output is authoritative, no hand-editing.** Same policy as Phase 12 D-12. Release body aggregates Phase 13's conventional commits. `:latest` GHCR tag stays pinned to `v1.0.1` (enforced by release.yml D-10 pre-release gating from Phase 12). The `:rc` rolling tag advances to the rc.2 digest.

### Claude's Discretion

- Exact SQL shape for the sparkline query (single window function with `ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id DESC)` + LIMIT, vs a LATERAL/subquery pattern). Either works on both SQLite and Postgres; planner picks after a performance check on a seeded dataset.
- Whether `stats.rs::percentile` takes `&[u64]` or `&mut Vec<u64>` (to allow in-place sort). Lean: `&[u64]` keeps the API pure; internal `Vec::from` + `sort_unstable` is fine at N≤100.
- Exact HTML shape of the timeline bar: `<a>` wrapping a styled `<div>` vs styled `<a>` directly. Pick whichever yields the cleaner hover-tooltip CSS.
- Pulse animation cadence (2s vs 1.5s vs 3s) for running bars — planner picks whichever feels "alive but not distracting" in manual review.
- Whether to emit a vertical "now" indicator (dashed line spanning all rows at the current time column) on the timeline. Nice-to-have; planner decides whether it's worth the extra CSS-grid positioning logic.
- Exact position tokens for the `Recent` sparkline column header label ("RECENT" uppercase vs "Recent" title-case) — follow whichever style the surrounding column headers use in `dashboard.html`.
- Tooltip positioning (above vs below the bar) on the timeline — planner picks based on viewport edge-case handling in the final CSS.
- Whether the `hx-trigger="every 30s"` on the timeline uses the `load` modifier to stop polling when the tab is hidden — nice for power savings, not load-bearing.
- Exact cliff.toml section header for the rc.2 release notes (whether OBS-* commits group under "Observability" vs "Added / Features") — `git-cliff` default grouping is fine; don't over-customize.

### Folded Todos

None — `.planning/STATE.md § Pending Todos` lists only Phase 10 carryover items (which have since completed); no pending todos match Phase 13 scope.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (researcher, planner, executor) MUST read these before planning or implementing.**

### Phase 13 scope and requirements
- `.planning/ROADMAP.md` § "Phase 13: Observability Polish (rc.2)" — phase goal, depends-on, success criteria, locked design decisions (lines 225–249).
- `.planning/ROADMAP.md` § "Strict Dependency Order" items #1, #2 — `stopped` status token and `job_run_number` display both carry forward from rc.1 into Phase 13 surfaces.
- `.planning/REQUIREMENTS.md` § OBS-01, OBS-02, OBS-03, OBS-04, OBS-05 — timeline (single-query LIMIT 10000, `idx_job_runs_start_time`, server timezone), sparkline + success-rate (N=5 threshold, stopped excluded from denominator), duration p50/p95 (N=20 threshold, last 100 successful runs), Rust-side percentile (no SQL `percentile_cont`).
- `.planning/REQUIREMENTS.md` § Traceability — `T-V11-TIME-01`, `T-V11-TIME-02`, `T-V11-TIME-04` (timeline query + timezone), `T-V11-SPARK-01..04` (sample-size honesty, zero-run crash-free), `T-V11-DUR-01..04` (percentile helper edge cases).
- `.planning/PROJECT.md` § Current Milestone — iterative rc strategy, `:latest` pinning policy, semver pre-release notation (`vX.Y.Z-rc.N`).
- `.planning/research/SUMMARY.md` § "rc.2 — Observability Polish" and § "Architecture Integration Map" — feature-by-file mapping, "No new runtime dependencies", "`src/web/stats.rs` ~40 LOC with tests", confirmed `idx_job_runs_start_time` index exists.
- `.planning/research/SUMMARY.md` § "What NOT to Change During v1.1" items #1, #5, #7, #8 — no scheduler-loop refactor, no JS framework for timeline or sparkline, no SQL `percentile_cont`, URLs continue to key on global `job_runs.id`.

### Carried decisions from earlier phases (MUST honor)
- `.planning/phases/10-stop-a-running-job-hygiene-preamble/10-CONTEXT.md` § D-08 — `--cd-status-stopped` + `cd-badge--stopped` tokens; Phase 13 consumes them unchanged for sparkline and timeline status colors.
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-CONTEXT.md` § D-05 — `Run #N` display convention; timeline tooltips (D-09) and sparkline cell tooltips (D-04) use the same `#N` shorthand.
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-CONTEXT.md` § D-04 — `title="global id: {run.id}"` pattern for muted diagnostic info; Phase 13 sparkline cells follow the same convention.
- `.planning/phases/12-docker-healthcheck-rc-1-cut/12-CONTEXT.md` § D-10, D-11, D-12, D-13 — rc tag release mechanics (release.yml patches, `docs/release-rc.md` runbook, `:rc` rolling tag, manual-tag-cut policy, `git-cliff` authoritative release notes). Phase 13 rc.2 cut REUSES all of these without modification.
- `.planning/phases/12.1-ghcr-tag-hygiene/` (Phase 12.1 artifacts) — `:latest` is correctly pinned to `v1.0.1`; `:main` floating tag live. rc.2 tag push MUST NOT move `:latest` (release.yml gating from Phase 12 D-10 already enforces).
- `.planning/STATE.md` § Accumulated Context — iterative rc cadence, `:latest` stays pinned until final `v1.1.0` ships from Phase 14.

### Project-level constraints
- `/Users/Robert/Code/public/cronduit/CLAUDE.md` § "Constraints" — Tech stack lock (sqlx, `askama_web 0.15` with `axum-0.8` feature, bollard, croner), rustls everywhere, TOML config, terminal-green design system fidelity, mermaid-only diagrams, PR-only landing (no direct commits to main), full-semver tag format.
- `design/DESIGN_SYSTEM.md` § 2.2 Status Colors — `--cd-status-{active|running|disabled|error|stopped}` tokens; Phase 13 adds NO new status tokens. Phase 13 MAY add new `--cd-timeline-*` tokens if needed for bar min-width, grid gap, or pulse animation.
- `design/DESIGN_SYSTEM.md` § 2.3 / 2.4 / 2.5 — surface, border, text tokens reused unchanged for Duration card (`--cd-bg-surface`, `--cd-border`, `--cd-text-secondary`).
- `THREAT_MODEL.md` — security posture; Phase 13 adds no new attack surface (`/timeline` is a read-only GET with no mutation; uses the same session-less model as `/` and `/jobs/{id}`).
- Auto-memory `feedback_diagrams_mermaid.md` — any diagram in Phase 13 docs or commit messages is mermaid, not ASCII.
- Auto-memory `feedback_no_direct_main_commits.md` — Phase 13 work lands via a feature branch + PR.
- Auto-memory `feedback_tag_release_version_match.md` — `v1.1.0-rc.2` must match `Cargo.toml`'s `1.1.0` base (Cargo.toml was bumped to `1.1.0` in Phase 10 D-12 via FOUND-13).
- Auto-memory `feedback_uat_user_validates.md` — Phase 13 UAT items require user validation, not Claude self-assessment.

### Code integration points (verified against v1.0.1 source + Phase 10/11/12 diffs)
- `src/web/handlers/dashboard.rs` + `src/db/queries.rs` — add a `get_dashboard_job_sparks(pool, job_ids)` query returning the last 20 terminal runs per job, folded into the existing `DashboardJob`/`DashboardJobView` hydration pipeline in `to_view()`.
- `src/db/queries.rs` — add `get_recent_successful_durations(pool, job_id, limit=100)` for p50/p95 input; add `get_timeline_runs(pool, window_start, window_end)` for the `/timeline` handler (single SQL, `LIMIT 10000`, verified against `idx_job_runs_start_time`).
- `src/web/stats.rs` — NEW file, ~40 LOC. `pub fn percentile(samples: &[u64], q: f64) -> Option<u64>`. Nearest-rank algorithm per D-19. Includes a `#[cfg(test)] mod tests` covering T-V11-DUR-01..04.
- `src/web/handlers/job_detail.rs` — add `Duration` section to the handler's view model; render `p50_display`, `p95_display`, `sample_count_display` via the new card.
- `src/web/handlers/timeline.rs` — NEW handler, ~120 LOC. Parses `?window=24h|7d`, calls `get_timeline_runs`, assembles the view model (jobs map, per-job bars with `left_pct`, `width_pct`, `status`, `run_id`, `job_run_number`, `duration_display`).
- `src/web/mod.rs` — add `.route("/timeline", get(timeline::timeline))` to the router.
- `templates/base.html` — add `<a href="/timeline" class="... {% block nav_timeline_active %}{% endblock %}">Timeline</a>` between Dashboard and Settings.
- `templates/pages/timeline.html` — NEW, ~80 LOC. Extends base.html; renders the axis, the pill toggle (`?window=24h|7d` anchors), row-per-job gantt bars via CSS grid, current-time line (if D-16 discretion picks it up), empty-window message block.
- `templates/pages/job_detail.html` — insert new Duration card block between the existing Configuration card (line 68) and the Run History section (line 71). Shape mirrors the Configuration card's outer `<div style="background:var(--cd-bg-surface);border:...;padding:...">`.
- `templates/partials/job_table.html` — add a new `<td>` for the Recent column between "Last Run" (line 15) and "Actions" (line 16). Cell contents: 20 status-colored `<span>` cells + muted badge span.
- `templates/pages/dashboard.html` — add matching `<th>Recent</th>` (non-sortable) between "Last Run" column (line 76) and "Actions" (line 86).
- `assets/static/app.css` (or whichever file currently holds `cd-badge--*` styles) — NEW selectors: `.cd-sparkline`, `.cd-sparkline-cell`, `.cd-sparkline-cell--{status}`, `.cd-timeline`, `.cd-timeline-row`, `.cd-timeline-bar`, `.cd-timeline-bar--{status}`, `.cd-tooltip`, `@keyframes cd-pulse`. All extend (don't replace) existing `--cd-status-*` tokens.
- `migrations/sqlite/`, `migrations/postgres/` — NO new migrations (OBS features are additive queries only per SUMMARY.md § rc.2 Research flag).
- `.planning/REQUIREMENTS.md` — flip OBS-01..OBS-05 checkboxes from `[ ]` to `[x]` as part of the Phase 13 close-out commit.

### Release engineering (reused from Phase 12)
- `.github/workflows/release.yml` — D-10 patches from Phase 12 already in place (pre-release tag gating, `:rc` rolling tag). Phase 13 makes NO changes to this file.
- `docs/release-rc.md` — maintainer runbook from Phase 12 D-11. Phase 13 follows the runbook for the rc.2 cut; no doc edits.
- `scripts/verify-latest-retag.sh` — Phase 12.1 verification script. Phase 13 close-out runs this to confirm `:latest` still equals `:1.0.1` digest before rc.2 push.

### External references
- `src/db/queries.rs` `idx_job_runs_start_time` — defined in `migrations/sqlite/20260410_000000_initial.up.sql` / `migrations/postgres/20260410_000000_initial.up.sql`. Phase 13 timeline query relies on this index; no new index needed.
- `chrono_tz::Tz` with `state.tz` — already threaded through `src/web/handlers/dashboard.rs::to_view()` for timestamp formatting. Timeline handler reuses the same `state.tz` field.
- `htmx` `hx-trigger="every 30s"` pattern — same idiom as the dashboard's 3s poll (`templates/pages/dashboard.html:91`). No new HTMX features needed.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`--cd-status-*` tokens + `cd-badge--{status}` CSS** (Phase 10 D-08) — six statuses (success/failed/timeout/cancelled/stopped/running) all have color + background + badge styles. Phase 13 sparkline cells and timeline bars consume these unchanged. No new status tokens.
- **`DashboardJobView` + `to_view()` hydration** (`src/web/handlers/dashboard.rs:76-132`) — already fetches one `DashboardJob` per job with status/next-fire/last-run. Phase 13 sparkline data folds into this pipeline: one extra query to hydrate `last_20_cells: Vec<SparkCell>` + `success_rate: Option<u8>` + `denominator: u32` per job.
- **`chrono_tz::Tz` via `state.tz`** (`src/web/handlers/dashboard.rs:195`) — server timezone already threaded into dashboard; timeline handler reuses the same field for axis label formatting (OBS-02 T-V11-TIME-04).
- **`format_relative_past` / `format_relative_future`** (`src/web/handlers/dashboard.rs:135-174`) — duration-to-human formatter. Timeline tooltips reuse for "2h ago" strings; the Duration card reuses the same unit scheme for `1m 34s`.
- **`run.duration_display` existing formatter** (present in `templates/partials/run_history.html` line 50) — the `{ms → "1m 34s"}` conversion already exists somewhere in the view-model pipeline (likely `src/web/handlers/job_detail.rs` or a shared helper). D-16 reuses it verbatim for p50/p95.
- **HTMX poll pattern** (`templates/pages/dashboard.html:91 hx-trigger="every 3s"`) — timeline's 30s poll (D-12) mirrors the shape. `hx-include="[name='window']"` preserves the query param across the poll.
- **`cd-badge` generic badge** (design system §6 Components) — Phase 13 "Recent" column badge (95%) reuses the family; no new badge variant required.
- **`idx_job_runs_start_time` index** — confirmed in the initial migration. Timeline single-query performance anchored on this; planner verifies via `EXPLAIN QUERY PLAN` per T-V11-TIME-01/02.

### Established Patterns
- **Table-based dashboard with HTMX partial refresh** (dashboard.html:40-97 wraps `partials/job_table.html`) — Phase 13 keeps the table shape; adds one column. The partial refresh still polls every 3s and gets the sparkline data baked into the same response (no second round-trip).
- **Server-side render then hydrate** — handlers return full HTML; HTMX swaps targeted fragments. No client-side data-fetching. Phase 13 timeline + duration card follow this pattern exactly.
- **Section cards on detail pages** (`templates/pages/job_detail.html:22-68` Configuration card, lines 71-76 Run History) — Phase 13's new "Duration" card copies the outer `<div>` shape (background surface, border, rounded, padding-6) + `<h2>` heading.
- **Query parameters for filter/sort state** — dashboard uses `?filter=...&sort=...&order=...`. Timeline `?window=...` follows the same convention.
- **Separate read/write pools** (project CLAUDE.md) — Phase 13's new queries (`get_dashboard_job_sparks`, `get_recent_successful_durations`, `get_timeline_runs`) all use the READER pool. No writes.
- **askama template inheritance** — every page extends `base.html`. Timeline page extends the same way; no new base template.

### Integration Points
- **`src/web/mod.rs` router** — single `.route("/timeline", get(timeline::timeline))` addition.
- **`src/web/handlers/mod.rs`** — new `pub mod timeline;` declaration.
- **`src/web/mod.rs` `pub mod stats;` (or `src/web/stats.rs` via `mod stats;` in `src/web/mod.rs`)** — new module; only the `percentile()` function needs to be `pub`.
- **`templates/base.html` nav block** — three-line addition for the Timeline link.
- **`templates/pages/dashboard.html` table header** — one `<th>Recent</th>` addition.
- **`templates/partials/job_table.html`** — one `<td>` addition.
- **`templates/pages/job_detail.html`** — one new section card insertion between existing sections.
- **`assets/static/app.css`** — extend with `cd-sparkline-*`, `cd-timeline-*`, `cd-tooltip`, `@keyframes cd-pulse` selectors. No global style changes.

</code_context>

<specifics>
## Specific Ideas

- **Literal REQ wording honored:** "shows a cross-job gantt-style run timeline for the last 24h (default) or 7d (toggle)" → D-07 (row-per-job gantt, jobs are the lanes) + D-08 (pill toggle w/ query param, default 24h).
- **Literal REQ wording honored:** "stopped runs are excluded from the denominator so operator-initiated stops do not skew the success rate" → D-05 (cells include stopped, denominator does not).
- **Literal REQ wording honored:** "below a minimum threshold of N=20 samples the values render as `—` instead of meaningless numbers" → D-17 (em-dash + tooltip with progress).
- **Literal REQ wording honored:** "Inline server-rendered HTML + CSS grid only. No JS framework, no canvas, no WASM" → D-09 uses CSS `:hover` (static CSS, no JS), D-12 uses HTMX poll (existing project pattern, not a framework), D-11 uses `@keyframes` (pure CSS animation).
- **Literal REQ wording honored:** "SQL-native percentile functions (`percentile_cont`) are NOT used even on Postgres" → D-19 Rust-side nearest-rank implementation is the only path.
- **Symmetry with Phase 10/11/12:** All three prior v1.1 phases kept design changes additive (no deleted tokens, no renamed selectors). Phase 13 continues: sparkline and timeline add new `cd-sparkline-*` / `cd-timeline-*` / `cd-tooltip` selectors but change zero existing selectors.
- **Symmetry with Phase 12:** Release mechanics fully reused. D-22 explicitly says "no new release-engineering work" — rc.2 follows Phase 12's runbook verbatim.
- **Auto-memory specific:** `feedback_tag_release_version_match.md` → rc.2 tag is `v1.1.0-rc.2` (full semver, annotated).
- **Auto-memory specific:** `feedback_diagrams_mermaid.md` → any mermaid used in Phase 13's PLAN.md or release notes stays mermaid; no ASCII art beyond the discussion previews that informed these decisions.

</specifics>

<deferred>
## Deferred Ideas

- **Timeline SSE live updates** — Phase 13 uses 30s HTMX polling. SSE-driven live timeline would require per-bar DOM updates; defer to v1.2 observability features.
- **Additional timeline windows (1h, 12h, 30d)** — Rejected by D-08; scope creep. If operator demand appears, revisit in v1.2.
- **Drill-down from sparkline cell to run detail** — Rejected by D-04 (20 tiny hit targets = accidental-click risk). Operators drill via run-history table.
- **Height-encoded duration bars in sparkline** — Rejected by D-02 (noisier; harder to scan). Duration is surfaced via p50/p95 card + hover tooltip.
- **SQL-native `percentile_cont` on Postgres** — Permanently rejected by OBS-05 + D-19 (structural parity constraint).
- **Linear-interpolation percentile** — Rejected by D-19 (synthetic values confuse operators).
- **Lenient sample definitions for p50/p95 (include failed/timeout/stopped)** — Rejected by D-20 (mixes populations; skews distribution).
- **Vertical "now" line on timeline spanning all rows** — Deferred to Claude's Discretion (nice-to-have; planner decides).
- **Auto-refresh pause when tab hidden** — Deferred to Claude's Discretion (power optimization; not load-bearing).
- **Timeline row reordering by activity** — Rejected by D-07 (destabilizing). Alphabetical is locked.
- **Prometheus histogram-based percentile (bucket-based)** — Out of scope for v1.1. The `metrics` facade + prometheus exporter already ship per-run-duration samples; Grafana/Prometheus-native percentile queries are available to operators who want them alongside the UI surface.
- **Export timeline data as CSV/JSON** — Out of scope; consistent with v1.0 "no export" stance (PROJECT.md § Out of Scope).
- **Hand-edited release notes for rc.2** — Rejected by D-23 (`git-cliff` is authoritative; same as Phase 12 D-12).
- **`workflow_dispatch` shortcut for rc.2 tag cut** — Rejected inherited from Phase 12 D-13. Trust anchor stays with maintainer.

</deferred>

---

*Phase: 13-observability-polish-rc-2*
*Context gathered: 2026-04-20*
