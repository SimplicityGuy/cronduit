# Phase 13: Observability Polish (rc.2) - Research

**Researched:** 2026-04-21
**Domain:** Rust server-side observability surfaces (dashboard sparkline, duration percentiles, cross-job gantt timeline) + rc.2 release mechanics
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Dashboard sparkline + success-rate badge (OBS-03)**
- **D-01** New "Recent" column between `Last Run` and `Actions` in `templates/partials/job_table.html`. Sparkline on top row, success-rate badge muted beneath.
- **D-02** Cell rendering is 20 uniform-height, uniform-width status-colored bars. Color comes from existing `--cd-status-{success|failed|timeout|cancelled|stopped|running}` tokens (Phase 10 D-08).
- **D-03** Success-rate badge is percent-only (integer, round half-up, e.g. `95%`); `title` attribute carries `"{num} of {den} non-stopped runs"`. `—` below N=5.
- **D-04** Per-cell hover uses native `title` only; no click-through from cells.
- **D-05** Cells = last 20 terminal runs including `stopped`; denominator excludes `stopped`. Running runs are NOT rendered as cells (deferred to timeline).
- **D-06** Zero-run jobs render 20 transparent placeholder cells + `—` badge. Row never crashes.

**Timeline page (OBS-01, OBS-02)**
- **D-07** Row-per-job gantt, alphabetical order, stable across reloads. Disabled/hidden jobs do NOT appear.
- **D-08** Pill-button window toggle via `?window=24h|7d` query param; default 24h.
- **D-09** Rich inline-HTML hover tooltip via CSS `:hover` + fallback `title` attribute for touch/a11y.
- **D-10** Bar is an `<a href="/jobs/{job_id}/runs/{run_id}">` (styled anchor directly).
- **D-11** Running runs render as pulsing bars `start_time → now`; CSS `@keyframes cd-pulse` 2s ease-in-out; `prefers-reduced-motion: reduce` disables the animation.
- **D-12** `hx-trigger="every 30s"` on `#timeline-body`; `hx-include="[name='window']"` preserves window across poll.
- **D-13** "Timeline" nav link in `templates/base.html` between "Dashboard" and "Settings". New `{% block nav_timeline_active %}{% endblock %}` mirroring `nav_dashboard_active`.
- **D-14** Empty-window state renders axis + centered message inline (not a full-page empty state).

**Job-detail duration p50/p95 (OBS-04, OBS-05)**
- **D-15** New "Duration" card between Configuration and Run History on `templates/pages/job_detail.html`.
- **D-16** Labeled chips: `p50 1m 34s` + `p95 2m 12s`, using the same human-readable formatter as `run.duration_display`.
- **D-17** N<20 renders `—` with `title="insufficient samples: need 20 successful runs, currently have {N}"`.
- **D-18** Muted subtitle: `last {N} successful runs` (N≥20), `{N} of 20 successful runs required` (N<20), capped at `last 100 successful runs` when N≥100.

**Math conventions**
- **D-19** `fn percentile(samples: &[u64], q: f64) -> Option<u64>`. Nearest-rank, 1-indexed: sort, `rank = ceil(q * n) as usize`, return `sorted[rank.saturating_sub(1).min(n-1)]`. Empty → `None`. Always returns an observed sample.
- **D-20** p50/p95 input set is strictly `status = 'success'`. SQL: `ORDER BY id DESC LIMIT 100`.
- **D-21** Helper edge cases T-V11-DUR-01..04 locked: empty → None, `[42]` at any q → `Some(42)`, `[10..100 step 10]` at 0.5 → `Some(50)`, at 0.95 → `Some(100)`. Pre-sorted vs reverse-sorted produce identical output. Consumer enforces N<20 threshold BEFORE calling `percentile()`.

**rc.2 release mechanics**
- **D-22** Reuse Phase 12 `.github/workflows/release.yml` (D-10), `docs/release-rc.md` runbook (D-11), and manual-tag-cut policy (D-13) verbatim. Phase 13 makes NO changes to workflow or runbook files. Close-out runs `scripts/verify-latest-retag.sh` and follows `docs/release-rc.md`.
- **D-23** `git-cliff` output is authoritative release-notes source. No hand-editing (per Phase 12 D-12). `:latest` GHCR tag stays pinned to `v1.0.1`. `:rc` rolling tag advances to rc.2 digest.

### Claude's Discretion

- Exact SQL shape for the sparkline query (single window function with `ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id DESC)` + LIMIT vs LATERAL/subquery). Either works on both SQLite and Postgres.
- `stats.rs::percentile` signature choice: `&[u64]` (pure, internal `Vec::from + sort_unstable`) vs `&mut Vec<u64>` (in-place). Lean toward `&[u64]` (UI-SPEC resolved → `&[u64]`).
- Exact HTML shape for the timeline bar: `<a>` wrapping `<div>` vs styled `<a>` directly. (UI-SPEC resolved → styled `<a>` directly.)
- Pulse animation cadence (2s vs 1.5s vs 3s). (UI-SPEC resolved → 2s.)
- Vertical "now" indicator line spanning timeline rows. (UI-SPEC resolved → NOT included.)
- "Recent" column header casing ("RECENT" uppercase via CSS text-transform). (UI-SPEC resolved → source title-case "Recent", CSS uppercase.)
- Tooltip positioning (above vs below the bar). (UI-SPEC resolved → above, with downward caret.)
- `hx-trigger` tab-hidden power-save modifier. (UI-SPEC resolved → NOT used.)
- `cliff.toml` section header for rc.2 release notes. (UI-SPEC resolved → `## Observability`; default git-cliff grouping is fine.)

### Deferred Ideas (OUT OF SCOPE)

- Timeline SSE live updates (v1.2).
- Additional timeline windows (1h, 12h, 30d) — scope creep.
- Drill-down click from sparkline cells — accidental-click risk.
- Height-encoded duration in sparkline — p50/p95 card already surfaces it.
- SQL-native `percentile_cont` on Postgres — permanently rejected (OBS-05 structural parity).
- Linear-interpolation percentile — synthetic values confuse cross-checks with run_history.
- Lenient p50/p95 sample definitions (include failed/timeout/stopped) — population mixing.
- Timeline row reordering by activity — destabilizing; alphabetical locked.
- Vertical "now" line on timeline — UI-SPEC deferred it.
- Auto-refresh pause when tab hidden — UI-SPEC deferred it.
- Prometheus histogram-based percentile (bucket-based) — out of scope for v1.1; `metrics` facade + prometheus exporter already ship per-run-duration histogram samples.
- Export timeline data as CSV/JSON — consistent with v1.0 "no export" stance.
- Hand-edited release notes for rc.2 — `git-cliff` authoritative.
- `workflow_dispatch` shortcut for rc.2 tag cut — Phase 12 D-13 rejection.

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| **OBS-01** | `/timeline` page with gantt-style cross-job run timeline for last 24h (default) or 7d (toggle); color-coded by terminal status using `--cd-status-*` tokens; inline server-rendered HTML + CSS grid only; hidden/disabled jobs excluded. | § Implementation Approach → OBS-01. SQL shape + timezone handling + CSS grid bar positioning all specified below. |
| **OBS-02** | Timeline handler executes a single SQL query (not N+1) with hard `LIMIT 10000`; `EXPLAIN QUERY PLAN` confirms `idx_job_runs_start_time` is used on both SQLite and Postgres; timestamps render in operator's `[server].timezone`. `T-V11-TIME-01`, `T-V11-TIME-02`, `T-V11-TIME-04`. | § Implementation Approach → OBS-02. Canonical query below; test strategy in § Validation Architecture. |
| **OBS-03** | Dashboard 20-run column sparkline + success-rate badge; N<5 → `—` (not a fake number); `stopped` excluded from denominator; zero-run jobs never crash. `T-V11-SPARK-01..04`. | § Implementation Approach → OBS-03. View-model hydration + window-function SQL below. |
| **OBS-04** | Job detail `p50: Xs` / `p95: Ys` over last 100 successful runs; `src/web/stats.rs::percentile(samples, q)` with tests for empty / single / min-sample-size; N<20 → `—`. `T-V11-DUR-01..04`. | § Implementation Approach → OBS-04. Nearest-rank algorithm + canonical test vectors below. |
| **OBS-05** | SQL-native `percentile_cont` is NOT used, even on Postgres. Structural parity: same code path on SQLite + Postgres. | § Implementation Approach → OBS-04. Rust-side helper is the only path; no Postgres-conditional branch anywhere. |

</phase_requirements>

## Phase Summary

Phase 13 adds three read-only observability surfaces on top of Cronduit's shipped infrastructure — no schema migrations, no new runtime dependencies, no scheduler-core changes. Every feature plugs into an existing handler or template pattern (window-function `ROW_NUMBER`, `state.tz` timezone threading, `cd-badge--{status}` token family, HTMX outer-HTML polling, `overflow-x-auto` table wrapper, Configuration-card outer shape). The locked design decisions in CONTEXT.md and UI-SPEC.md already resolved every architectural ambiguity; this research verifies the technical assumptions (SQLite window-function support on the shipped sqlx 0.8 build, `idx_job_runs_start_time` index, existing view-model hooks) and prescribes the concrete code and SQL the planner will task.

**Primary recommendation:** Land the phase in five tightly scoped plans — (1) `stats.rs::percentile` with exhaustive unit tests, (2) Duration card wiring (job_detail handler + new SQL query + template block), (3) Sparkline column (dashboard handler + window-function SQL + view-model + table partial + CSS), (4) `/timeline` page (new handler + template + nav link + CSS + HTMX poll), (5) rc.2 cut following `docs/release-rc.md` verbatim. The structural-parity rule (OBS-05) is the single invariant that can silently regress, so every new SQL query must be dual-path (SQLite branch + Postgres branch) and every derived metric must pipe through `src/web/stats.rs`.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| 20-cell sparkline per job card | API / Backend (Rust view-model hydration) | Browser (CSS grid layout of already-rendered spans) | CONTEXT D-02 mandates solid status-colored cells rendered server-side. No client state; HTMX 3s poll re-renders the whole row. |
| Success-rate badge percent | API / Backend (Rust integer math on numerator/denominator) | — | CONTEXT D-03 + D-05 lock server-side denominator (`terminal_count − stopped_count`); browser only displays the pre-computed string. |
| p50 / p95 Duration card | API / Backend (Rust `percentile()` helper) | — | OBS-05 forbids SQL-native percentile; computation MUST happen in Rust regardless of backend. Template renders the `duration_display` string only. |
| Last-100-successful-runs query | Database / Storage | — | Pure persistence concern — `SELECT duration_ms ... WHERE status='success' ORDER BY id DESC LIMIT 100`. |
| `/timeline` gantt layout | API / Backend (Rust computes `left_pct` / `width_pct` per bar) | Browser (CSS grid + absolute-position child) | CONTEXT D-09/D-11 mandate server-computed bar geometry so page renders correctly before JS runs. Browser handles hover state and pulse animation only (CSS). |
| Timeline query (last 24h / 7d) | Database / Storage | — | Single SQL with `LIMIT 10000`; `EXPLAIN QUERY PLAN` asserts index usage. T-V11-TIME-01/02. |
| Server timezone handling | API / Backend (read once from `state.tz`; render via `chrono_tz`) | — | `[server].timezone` is already threaded into `AppState.tz`; all axis ticks + tooltip times format through it. No client-side timezone logic. |
| HTMX 30s timeline poll | Browser (native HTMX) | API / Backend (serve timeline_body partial) | Standard poll pattern; identical mechanism to dashboard's 3s poll. No SSE. |
| rc.2 release tag | CI / CD (release.yml) | Maintainer (local `git tag -a -s`) | Existing Phase 12 D-10 workflow + D-11 runbook; Phase 13 touches nothing in this tier. |

## Standard Stack

All Phase 13 work uses already-installed dependencies. **No new runtime crates.** Every version below was verified against `Cargo.toml` on 2026-04-21.

### Core (already in Cargo.toml)

| Library | Version | Purpose | Why Standard | Source |
|---------|---------|---------|--------------|--------|
| `sqlx` | 0.8.6 | Database access (SQLite + Postgres) | Phase 13 queries use the existing `PoolRef::{Sqlite,Postgres}` dual-path pattern established in `src/db/queries.rs`. | [VERIFIED: Cargo.toml:32] |
| `axum` | 0.8.9 | HTTP handlers for `/timeline` | Same framework as all other handlers. | [VERIFIED: Cargo.toml:25] |
| `askama` / `askama_web` | 0.15 / 0.15 (axum-0.8) | New timeline templates | `WebTemplateExt::into_web_template()` pattern already used by every handler. | [VERIFIED: Cargo.toml:92-93] |
| `axum-htmx` | 0.8 | `HxRequest` extractor (not needed for new timeline handler — full response always) | Available if a partial-vs-page split is later needed. | [VERIFIED: Cargo.toml:99] |
| `chrono` + `chrono-tz` | 0.4.44 / 0.10.4 | Server-tz timestamp formatting, `idx_job_runs_start_time` RFC3339 text storage | Already threaded via `state.tz` in every handler. | [VERIFIED: Cargo.toml:72-73] |
| `rust-embed` | 8.11 | Embedded CSS (Tailwind-built `app.css`) | Existing edit-refresh loop continues unchanged; CSS edits hot-reload in debug. | [VERIFIED: Cargo.toml:96] |

### Supporting (dev / test only)

| Library | Version | Purpose | Source |
|---------|---------|---------|--------|
| `testcontainers` + `testcontainers-modules` | 0.27.3 / 0.15.0 (postgres) | Real Postgres for `EXPLAIN ANALYZE` timeline assertion | [VERIFIED: Cargo.toml:133-135] |
| `tokio` | 1.51 (full + test-util) | Async runtime for tests | [VERIFIED: Cargo.toml:142] |
| `tower` (util) | 0.5 | `.oneshot()` in dashboard_render test pattern | [VERIFIED: Cargo.toml:138] |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-rolled 40-LOC `percentile()` | `statrs` crate | Adds runtime dep + 30+ transitive crates; hand-rolled nearest-rank is ~15 LOC, exhaustively testable, and matches the locked D-19 semantics. Rejected. |
| Hand-rolled percentile | `noisy_float`-backed `quantile` helpers | Same — 40 LOC of pure Rust beats every available crate for this scope. |
| HTMX 30s poll | SSE stream for timeline | OBS-01 + deferred-ideas lock: SSE is v1.2. 30s is sufficient for a per-job gantt. |
| Inline CSS in askama template | External `.cd-timeline.css` file | Existing pattern mixes inline `style="..."` and class-based selectors in `app.css`. Phase 13 continues that pattern (new classes in `assets/src/app.css`). |

**Installation:**
```bash
# No new crates. All Phase 13 features build against the shipped Cargo.lock.
cargo check  # should succeed with no dep changes
```

**Version verification:** All crate versions verified against `Cargo.toml` on 2026-04-21 (research date). [VERIFIED: /Users/Robert/Code/public/cronduit/Cargo.toml]

## Architecture Patterns

### System Data Flow

```mermaid
flowchart TD
    subgraph browser[Browser]
        dash_view[Dashboard page]
        job_view[Job detail page]
        tl_view[/timeline page]
        htmx_poll3s[HTMX every 3s]
        htmx_poll30s[HTMX every 30s]
    end

    subgraph backend[Rust / axum]
        dash_h[dashboard::dashboard]
        jd_h[job_detail::job_detail]
        tl_h[timeline::timeline NEW]
        stats[stats::percentile NEW]
    end

    subgraph db[SQLite or Postgres]
        q_spark[get_dashboard_job_sparks NEW]
        q_dur[get_recent_successful_durations NEW]
        q_tl[get_timeline_runs NEW]
        idx[idx_job_runs_start_time]
    end

    dash_view -->|GET /partials/job-table| dash_h
    htmx_poll3s -->|every 3s| dash_h
    dash_h -->|existing query| q_spark
    q_spark --> idx
    q_spark -->|20 terminal runs per job| dash_h

    job_view -->|GET /jobs/{id}| jd_h
    jd_h --> q_dur
    q_dur -->|up to 100 duration_ms| stats
    stats -->|p50/p95| jd_h

    tl_view -->|GET /timeline?window=24h| tl_h
    htmx_poll30s -->|every 30s| tl_h
    tl_h --> q_tl
    q_tl --> idx
    q_tl -->|up to 10000 runs| tl_h

    classDef new fill:#1a3d1a,stroke:#00ff7f,color:#e0ffe0
    class tl_h,stats,q_spark,q_dur,q_tl new
```

### Recommended Project Structure

```
src/
├── web/
│   ├── stats.rs                   # NEW — percentile() helper (~40 LOC + tests)
│   ├── mod.rs                     # + pub mod stats; + .route("/timeline", ...)
│   └── handlers/
│       ├── mod.rs                 # + pub mod timeline;
│       ├── dashboard.rs           # extend DashboardJobView with spark_cells, spark_badge, ...
│       ├── job_detail.rs          # add DurationView substruct to JobDetailView
│       └── timeline.rs            # NEW — handler + view models (~120 LOC)
├── db/
│   └── queries.rs                 # + get_dashboard_job_sparks, get_recent_successful_durations, get_timeline_runs
└── ...
templates/
├── base.html                      # + Timeline nav link + nav_timeline_active block
├── pages/
│   ├── dashboard.html             # + <th>Recent</th>
│   ├── job_detail.html            # + Duration card block
│   └── timeline.html              # NEW (~80 LOC)
└── partials/
    ├── job_table.html             # + Recent <td> cell
    └── timeline_body.html         # NEW — HTMX swap target
assets/
└── src/
    └── app.css                    # + cd-sparkline-*, cd-timeline-*, cd-tooltip-*, cd-pill-*, @keyframes cd-pulse
tests/
├── v13_stats_percentile.rs        # NEW — covers T-V11-DUR-01..04
├── v13_timeline_explain.rs        # NEW — covers T-V11-TIME-01, T-V11-TIME-02 (SQLite + Postgres)
├── v13_timeline_timezone.rs       # NEW — covers T-V11-TIME-04
├── v13_sparkline_render.rs        # NEW — covers T-V11-SPARK-01..04
└── v13_duration_card.rs           # NEW — covers T-V11-DUR-05 style "card visible with —"
```

### Pattern 1: Dual-path SQL (SQLite + Postgres) with `PoolRef` match

**What:** Every new query MUST branch on `pool.reader()` returning `PoolRef::Sqlite(p)` vs `PoolRef::Postgres(p)`.
**When to use:** Any new function in `src/db/queries.rs` — mandatory for Phase 13's three new queries.
**Example** (from the shipped `get_dashboard_jobs`):
```rust
// Source: src/db/queries.rs:580-653 (verified)
match pool.reader() {
    PoolRef::Sqlite(p) => {
        let rows = sqlx::query(&sqlite_sql).bind(...).fetch_all(p).await?;
        Ok(rows.into_iter().map(|r| DashboardJob { ... }).collect())
    }
    PoolRef::Postgres(p) => {
        let rows = sqlx::query(&pg_sql).bind(...).fetch_all(p).await?;
        Ok(rows.into_iter().map(|r| DashboardJob { ... }).collect())
    }
}
```

Postgres uses `$1`-style placeholders; SQLite uses `?1`-style. Both accept `ROW_NUMBER() OVER (PARTITION BY ... ORDER BY ...)` (SQLite ≥3.25; sqlx 0.8 bundles SQLite ≥3.46 per [CITED: docs.rs/libsqlite3-sys](https://docs.rs/crate/libsqlite3-sys/latest)).

### Pattern 2: View-model hydration in `to_view()`

**What:** Handler fetches rows from `queries.rs`, then converts to typed view structs (`DashboardJobView`, `JobDetailView`). Templates consume only view structs — never raw DB rows.
**When to use:** All three observability surfaces follow this. Sparkline cells, percentile chips, and timeline bars are built in Rust; templates render already-formatted strings.
**Example** (from shipped `src/web/handlers/dashboard.rs:76-132`):
```rust
// Source: src/web/handlers/dashboard.rs:76 (verified)
fn to_view(job: DashboardJob, tz: Tz) -> DashboardJobView {
    let now = Utc::now();
    // ... compute next_fire via croner, last_run_relative via format_relative_past ...
    DashboardJobView { id: job.id, name: job.name, ..., next_fire, last_run_relative }
}
```

### Pattern 3: Section-card outer shape (Duration card)

**What:** Copy the Configuration card container verbatim for visual consistency.
**When to use:** Duration card on job_detail.html. UI-SPEC locks the exact shape.
**Example** (from shipped `templates/pages/job_detail.html:23`):
```html
<!-- Source: templates/pages/job_detail.html:23 (verified) -->
<div style="background:var(--cd-bg-surface);border:1px solid var(--cd-border);border-radius:8px;padding:var(--cd-space-6)" class="mb-6">
  <h2 style="font-size:var(--cd-text-xl);font-weight:700;letter-spacing:-0.02em;margin-bottom:var(--cd-space-4)">Duration</h2>
  <!-- chips + subtitle -->
</div>
```
Note: UI-SPEC's typography revision collapsed heading from `--cd-text-lg` to `--cd-text-xl`; the Configuration card heading (line 24) currently renders at `--cd-text-lg`. Phase 13's new Duration card MUST use `--cd-text-xl` per the locked 4-size scale. Do NOT edit the existing Configuration card — leave it at `--cd-text-lg` (only additive changes per UI-SPEC).

### Pattern 4: HTMX outer-HTML poll with external state input

**What:** `#target-id` wrapped by a container that carries `hx-trigger="every Ns"`; hidden inputs with `[name=...]` outside the swap target are captured via `hx-include`.
**When to use:** `/timeline` page. Pill toggle lives outside `#timeline-body` so the `outerHTML` swap doesn't destroy the `<input type="hidden" name="window">`.
**Example** (adapted from dashboard's pattern):
```html
<!-- Source: templates/pages/dashboard.html:89-95 (verified) -->
<tbody id="job-table-body"
       hx-get="/partials/job-table"
       hx-trigger="every 3s"
       hx-swap="innerHTML"
       hx-include="[name='filter'],[name='sort'],[name='order']">
```

### Anti-Patterns to Avoid

- **Don't use SQL-native `percentile_cont` on Postgres.** Even though Postgres has it, OBS-05 + D-19 forbid it. Enforce by code review and by ensuring `queries::get_recent_successful_durations` only returns raw `Vec<u64>` — never scalars.
- **Don't invoke `SELECT strftime('now')` inside the timeline query for running-run end_time.** Use `chrono::Utc::now()` in the handler (Rust-side) so the timezone is unambiguous and test clocks (`tokio::time::pause` elsewhere in the codebase) behave predictably.
- **Don't add a second round-trip per job for the sparkline.** The existing `get_dashboard_jobs` is one query; `get_dashboard_job_sparks` MUST also be one query across all enabled jobs (window function, not `SELECT ... FROM job_runs WHERE job_id = ?` per job).
- **Don't introduce client-side JS for the gantt layout.** All `left_pct` / `width_pct` compute in Rust; CSS does layout only. UI-SPEC lock.
- **Don't touch the existing Configuration card heading size.** UI-SPEC explicitly says Phase 13 is additive; existing `--cd-text-lg` stays.
- **Don't hand-edit rc.2 release notes.** `git-cliff` is authoritative per D-23 (Phase 12 D-12 precedent).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| RFC3339 / SQLite-text timestamp parsing | Ad-hoc string slicing | `chrono::DateTime::parse_from_rfc3339` + `chrono::NaiveDateTime::parse_from_str` (both already used in dashboard.rs:102-107) | Existing fallback-parse idiom handles both storage formats consistently. |
| Duration human-readable formatter | New `format_ms_pretty()` | `src/web/format.rs::format_duration_ms` (verified in place) | D-16 explicitly says "reuse the shared formatter". Note: current formatter uses `1.2s` (one decimal) for < 1 minute, not `42s` (floor). UI-SPEC copywriting says `42s`. See § Open Questions. |
| Relative-time "2h ago" strings | New helper | `format_relative_past` in `src/web/handlers/dashboard.rs:142-149` | Sparkline cell `title` includes relative time; reuse verbatim. |
| Status → CSS class mapping | Per-surface match expressions | Existing `cd-badge--{status}` / new `cd-sparkline-cell--{status}` / `cd-timeline-bar--{status}` pattern (class name derived from the lowercase status string) | Matches Phase 10/11 pattern. |
| Status-color semantics | New token family | `--cd-status-{success|failed|timeout|stopped|running}` (shipped) + `--cd-status-cancelled` (UI-SPEC adds as alias of `--cd-text-secondary`) | Phase 10 D-08 already enumerated. |
| SQLite in-memory test harness | New fixture | `tests/common/v11_fixtures.rs::setup_sqlite_with_phase11_migrations` | Every v11 test uses this. Phase 13 reuses unchanged. |
| Postgres test container | Raw `testcontainers` dance | `testcontainers_modules::postgres::Postgres::default().start()` (pattern in `tests/db_pool_postgres.rs:10-14` and `tests/schema_parity.rs`) | 3-line setup; migration + seed is trivial after. |
| rc.2 tag cut pipeline | New workflow | `docs/release-rc.md` + `.github/workflows/release.yml` Phase 12 D-10 patches | D-22 locks reuse-verbatim. |

**Key insight:** Phase 13 is a composition phase — every capability it needs already exists in the codebase. The risk isn't "Claude writes bad code"; it's "Claude re-invents something that's already there and the two copies drift." Before adding any helper, grep for a near-match in `src/web/`, `src/db/queries.rs`, or `tests/common/v11_fixtures.rs`.

## Implementation Approach

### OBS-04: `stats.rs::percentile` helper (smallest, build first)

**New file:** `src/web/stats.rs` (~40 LOC).
**Module wiring:** add `pub mod stats;` to `src/web/mod.rs` after `pub mod format;`.

```rust
// src/web/stats.rs
//! Pure-Rust percentile helper (Phase 13 OBS-04 / D-19).
//!
//! Algorithm: nearest-rank, 1-indexed. Always returns an observed sample —
//! never an interpolated value that didn't occur. Matches the percentile
//! semantics documented in `.planning/phases/13-.../13-CONTEXT.md` § D-19.
//!
//! OBS-05 structural-parity: this module is the ONLY path by which p50/p95
//! are computed, regardless of whether the backend is SQLite or Postgres.
//! Do NOT introduce a SQL-native variant on Postgres.

/// Returns the q-th percentile of `samples` using the 1-indexed nearest-rank
/// method. `q` is a fraction in `[0.0, 1.0]`. Returns `None` for empty input.
///
/// ## Semantics
///
/// `rank = ceil(q * n) as usize` over the sorted samples; the helper returns
/// `sorted[rank.saturating_sub(1).min(n - 1)]`. For a single-element slice,
/// every quantile collapses to that element. For q = 1.0 on any non-empty
/// slice, the helper returns `sorted[n - 1]` (the max).
pub fn percentile(samples: &[u64], q: f64) -> Option<u64> {
    if samples.is_empty() {
        return None;
    }
    let n = samples.len();
    let mut sorted: Vec<u64> = samples.to_vec();
    sorted.sort_unstable();
    let rank = (q * n as f64).ceil() as usize;
    let idx = rank.saturating_sub(1).min(n - 1);
    Some(sorted[idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    // T-V11-DUR-01
    #[test]
    fn empty_slice_returns_none() {
        assert_eq!(percentile(&[], 0.5), None);
        assert_eq!(percentile(&[], 0.95), None);
        assert_eq!(percentile(&[], 0.0), None);
        assert_eq!(percentile(&[], 1.0), None);
    }

    // T-V11-DUR-02
    #[test]
    fn single_element_any_quantile() {
        assert_eq!(percentile(&[42], 0.5), Some(42));
        assert_eq!(percentile(&[42], 0.95), Some(42));
        assert_eq!(percentile(&[42], 0.0), Some(42));
        assert_eq!(percentile(&[42], 1.0), Some(42));
    }

    // T-V11-DUR-03 (locked convention: ceil → index 5 of [10..100 step 10] = 50)
    #[test]
    fn median_of_ten_returns_fifth_sample() {
        let s = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        assert_eq!(percentile(&s, 0.5), Some(50));
    }

    // T-V11-DUR-04
    #[test]
    fn p95_of_ten_returns_last_sample() {
        let s = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        assert_eq!(percentile(&s, 0.95), Some(100));
    }

    // T-V11-DUR-03 extended: pre-sorted vs reverse-sorted parity
    #[test]
    fn sort_internal_regardless_of_input_order() {
        let sorted = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        let reverse = [100, 90, 80, 70, 60, 50, 40, 30, 20, 10];
        assert_eq!(percentile(&sorted, 0.5), percentile(&reverse, 0.5));
        assert_eq!(percentile(&sorted, 0.95), percentile(&reverse, 0.95));
    }

    #[test]
    fn q_zero_returns_min() {
        // ceil(0.0 * 10) = 0; saturating_sub(1) = 0 → sorted[0] = min
        let s = [5, 1, 9, 3, 7];
        assert_eq!(percentile(&s, 0.0), Some(1));
    }

    #[test]
    fn q_one_returns_max() {
        let s = [5, 1, 9, 3, 7];
        assert_eq!(percentile(&s, 1.0), Some(9));
    }

    // 100-sample distribution
    #[test]
    fn p50_p95_over_hundred_samples() {
        let samples: Vec<u64> = (1..=100).collect();
        // ceil(0.5 * 100) = 50 → sorted[49] = 50
        assert_eq!(percentile(&samples, 0.5), Some(50));
        // ceil(0.95 * 100) = 95 → sorted[94] = 95
        assert_eq!(percentile(&samples, 0.95), Some(95));
    }
}
```

**Validation:** Five locked test vectors from D-21 plus four boundary tests above. Consumer (`job_detail` handler) enforces N<20 threshold BEFORE calling `percentile()` (D-21 explicitly puts the threshold in the consumer).

---

### OBS-04 (cont.): Duration card on job_detail

**New query** in `src/db/queries.rs`:

```rust
/// Fetch the last N successful durations for a job, newest first. Used by
/// Phase 13 Duration card (OBS-04 D-20). Returns only rows with
/// `status = 'success'` AND `duration_ms IS NOT NULL`.
pub async fn get_recent_successful_durations(
    pool: &DbPool,
    job_id: i64,
    limit: i64,
) -> anyhow::Result<Vec<u64>> {
    match pool.reader() {
        PoolRef::Sqlite(p) => {
            let rows = sqlx::query(
                "SELECT duration_ms FROM job_runs
                 WHERE job_id = ?1
                   AND status = 'success'
                   AND duration_ms IS NOT NULL
                 ORDER BY id DESC
                 LIMIT ?2",
            )
            .bind(job_id)
            .bind(limit)
            .fetch_all(p)
            .await?;
            Ok(rows.into_iter().map(|r| r.get::<i64, _>("duration_ms") as u64).collect())
        }
        PoolRef::Postgres(p) => {
            let rows = sqlx::query(
                "SELECT duration_ms FROM job_runs
                 WHERE job_id = $1
                   AND status = 'success'
                   AND duration_ms IS NOT NULL
                 ORDER BY id DESC
                 LIMIT $2",
            )
            .bind(job_id)
            .bind(limit)
            .fetch_all(p)
            .await?;
            Ok(rows.into_iter().map(|r| r.get::<i64, _>("duration_ms") as u64).collect())
        }
    }
}
```

**Note on index:** The existing `idx_job_runs_job_id_start` covers `(job_id, start_time DESC)`. The query above orders by `id DESC` — since rows within a single job are inserted in start-time order, `id DESC` and `start_time DESC` are equivalent. Using `id DESC` matches the shipped `get_run_history` pattern (queries.rs:721 uses `start_time DESC`; `id DESC` is a trivial planner alternative that also hits the composite index's job_id prefix — either is fine, pick one and be consistent with CONTEXT D-20's SQL which uses `id DESC`).

**Handler wiring** in `src/web/handlers/job_detail.rs`:

```rust
// Add to JobDetailView:
pub struct JobDetailView {
    // ... existing fields ...
    pub duration: DurationView,
}

pub struct DurationView {
    pub p50_display: String,       // "1m 34s" when N≥20, else "—"
    pub p95_display: String,       // same
    pub has_min_samples: bool,     // N >= 20
    pub sample_count: usize,       // raw N (capped at 100 by query LIMIT)
    pub sample_count_display: String,  // subtitle matrix per D-18
}

// Inside the handler, after fetching the job:
const MIN_SAMPLES_FOR_PERCENTILE: usize = 20;
const PERCENTILE_SAMPLE_LIMIT: i64 = 100;

let durations = queries::get_recent_successful_durations(
    &state.pool, job_id, PERCENTILE_SAMPLE_LIMIT,
).await.unwrap_or_default();

let sample_count = durations.len();
let has_min = sample_count >= MIN_SAMPLES_FOR_PERCENTILE;

let (p50_display, p95_display) = if has_min {
    let p50 = crate::web::stats::percentile(&durations, 0.5).expect("non-empty when N>=20");
    let p95 = crate::web::stats::percentile(&durations, 0.95).expect("non-empty when N>=20");
    (format_duration_ms(Some(p50 as i64)), format_duration_ms(Some(p95 as i64)))
} else {
    ("—".to_string(), "—".to_string())
};

let sample_count_display = match sample_count {
    0 => "0 of 20 successful runs required".to_string(),
    1..=19 => format!("{sample_count} of 20 successful runs required"),
    20..=99 => format!("last {sample_count} successful runs"),
    _ => "last 100 successful runs".to_string(),
};

let duration_view = DurationView {
    p50_display,
    p95_display,
    has_min_samples: has_min,
    sample_count,
    sample_count_display,
};
```

**Template block** (inserted between the Configuration card closing `</div>` and the Run History opening `<div class="mb-6">`): see UI-SPEC § "Surface B: Duration card" for verbatim HTML.

---

### OBS-03: Dashboard sparkline + success-rate badge

**New query** in `src/db/queries.rs`:

```rust
/// Sparkline cell for one terminal job run.
#[derive(Debug, Clone)]
pub struct DashboardSparkRow {
    pub job_id: i64,
    pub id: i64,                     // global run id (for tooltip diagnostic)
    pub job_run_number: i64,         // #N display label
    pub status: String,              // success / failed / timeout / cancelled / stopped
    pub duration_ms: Option<i64>,    // for tooltip
    pub start_time: String,          // for tooltip relative-time
    pub rn: i64,                     // 1..=20 (window ordinal)
}

/// Fetch the last 20 terminal runs per job for every enabled job, in a single
/// query, using `ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id DESC)`.
/// "Terminal" = status IN ('success','failed','timeout','cancelled','stopped')
/// (i.e. excludes 'running'). Returned rows are ready for the sparkline
/// view-model folder below. NO N+1: one query, all jobs.
pub async fn get_dashboard_job_sparks(
    pool: &DbPool,
) -> anyhow::Result<Vec<DashboardSparkRow>> {
    let sql = r#"
        SELECT job_id, id, job_run_number, status, duration_ms, start_time, rn
        FROM (
            SELECT job_id, id, job_run_number, status, duration_ms, start_time,
                   ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY id DESC) AS rn
            FROM job_runs
            WHERE status IN ('success','failed','timeout','cancelled','stopped')
        ) t
        WHERE rn <= 20
    "#;
    match pool.reader() {
        PoolRef::Sqlite(p) => { /* bind-less query, fetch_all, map via r.get */ }
        PoolRef::Postgres(p) => { /* same */ }
    }
}
```

**Handler wiring** in `src/web/handlers/dashboard.rs` — extend `DashboardJobView` and `to_view`:

```rust
pub struct SparkCell {
    pub kind: String,     // "success" | "failed" | "timeout" | "cancelled" | "stopped" | "empty"
    pub title: String,    // per-cell tooltip; "" when kind == "empty"
}

pub struct DashboardJobView {
    // ... existing fields ...
    pub spark_cells: Vec<SparkCell>,     // always exactly 20; oldest-to-newest left-to-right
    pub spark_badge: String,             // "95%" or "—"
    pub spark_total: usize,              // all 20 cells' non-empty count (for aria-label)
    pub spark_numerator: usize,          // success count (for title)
    pub spark_denominator: usize,        // terminal_count - stopped_count (for title)
}

// In dashboard() handler, after fetching DashboardJob rows:
let spark_rows = queries::get_dashboard_job_sparks(&state.pool).await.unwrap_or_default();

// Bucket by job_id
let mut spark_by_job: HashMap<i64, Vec<DashboardSparkRow>> = HashMap::new();
for row in spark_rows {
    spark_by_job.entry(row.job_id).or_default().push(row);
}

// Per job, fold into SparkCell[20] + badge + denominator stats
const MIN_SAMPLES_FOR_RATE: usize = 5;
for job_view in &mut job_views {
    let rows = spark_by_job.remove(&job_view.id).unwrap_or_default();
    // rows are ordered by id DESC (newest first); reverse for display (oldest-to-newest left-to-right)
    let mut rows_asc: Vec<_> = rows.into_iter().rev().collect();
    let filled = rows_asc.len();
    // ... build 20 cells, padding with SparkCell { kind: "empty", title: "".into() } ...
    // ... compute numerator (success count), denominator (total - stopped count) ...
    if denominator < MIN_SAMPLES_FOR_RATE {
        job_view.spark_badge = "—".to_string();
    } else {
        let pct = ((numerator as f64 / denominator as f64) * 100.0).round() as i64;
        job_view.spark_badge = format!("{pct}%");
    }
}
```

**Templates:** `templates/pages/dashboard.html` gets one extra `<th>Recent</th>` column header between Last Run and Actions. `templates/partials/job_table.html` gets one extra `<td>` per row (contents per UI-SPEC § "Surface A"). CSS additions go into `assets/src/app.css` (UI-SPEC § "Surface A" enumerates the full selector set).

**Zero-run behavior (D-06 / T-V11-SPARK-01):** if `rows_asc.is_empty()`, all 20 cells get `kind: "empty"` and the badge renders `—`. Handler never panics.

---

### OBS-01, OBS-02: `/timeline` page

**New file:** `src/web/handlers/timeline.rs`.

**Query shape:** single SQL that returns every relevant run across every enabled job in the window, sorted for deterministic rendering, with a hard `LIMIT 10000`.

```rust
/// A terminal or in-flight run for the timeline view. Rendered as one gantt bar.
#[derive(Debug, Clone)]
pub struct TimelineRun {
    pub run_id: i64,
    pub job_id: i64,
    pub job_name: String,
    pub job_run_number: i64,
    pub status: String,            // success | failed | timeout | cancelled | stopped | running
    pub start_time: String,        // RFC3339 or "YYYY-MM-DD HH:MM:SS"
    pub end_time: Option<String>,  // None iff status == "running"
    pub duration_ms: Option<i64>,
}

pub async fn get_timeline_runs(
    pool: &DbPool,
    window_start_rfc3339: &str,   // Rust formats Utc::now() - 24h (or - 7d)
) -> anyhow::Result<Vec<TimelineRun>> {
    let sql = r#"
        SELECT jr.id AS run_id,
               jr.job_id,
               j.name AS job_name,
               jr.job_run_number,
               jr.status,
               jr.start_time,
               jr.end_time,
               jr.duration_ms
        FROM job_runs jr
        JOIN jobs j ON j.id = jr.job_id
        WHERE j.enabled = 1
          AND (jr.end_time >= ?1 OR jr.status = 'running')
        ORDER BY j.name ASC, jr.start_time ASC
        LIMIT 10000
    "#;
    // Postgres variant uses $1 and `j.enabled = true`.
    // Both hit idx_job_runs_start_time on the jr.start_time half of the predicate
    //   — though note the filter is on end_time, not start_time, for correctness:
    //   a run that started 25h ago but ended 23h ago IS in the window.
    // See § Open Questions — query-shape discretion allows swapping end_time for
    // start_time if EXPLAIN shows a better plan.
}
```

**CRITICAL index consideration:** The locked requirement T-V11-TIME-02 asserts `idx_job_runs_start_time` is used. The shipped index is defined on `job_runs.start_time` (not `end_time`). The CONTEXT says `T-V11-TIME-01` requires single-query shape; the PITFALLS.md sketch at line 630 filters on `jr.end_time >= ?1 OR jr.status = 'running'`. If we filter on `end_time` but the index is on `start_time`, SQLite will NOT use the index. Two options, planner to pick:

1. **Filter on `start_time >= window_start` instead.** Runs that started before the window but ended inside it are excluded; operators might miss an 8-hour job that started before the window but ended in it. Cleaner index usage. **Recommended** for v1.1 given the 24h/7d windows — a 24h run is an edge case, a 7d run is out-of-scope.
2. **Filter on `start_time >= window_start - MAX_RUN_AGE`** (e.g. window_start minus 24h) and post-filter in Rust. Keeps the index active; adds Rust-side filter complexity.

Option 1 is simpler and matches how operators think about the timeline ("what ran in the last 24h?"). Lock this in the plan. Query becomes:

```sql
WHERE j.enabled = 1
  AND jr.start_time >= ?1          -- uses idx_job_runs_start_time
ORDER BY j.name ASC, jr.start_time ASC
LIMIT 10000
```

Then `status = 'running'` bars that started before the window are simply not shown (but the pulse + HTMX 30s poll means a run that starts 1s after the page loads appears on the next poll — acceptable). [ASSUMED: operators agree "last 24h" means "started in last 24h", not "was active within last 24h"] — see Open Questions.

**Handler** computes bar geometry:

```rust
pub async fn timeline(
    HxRequest(is_htmx): HxRequest,
    State(state): State<AppState>,
    Query(params): Query<TimelineParams>,
) -> impl IntoResponse {
    let window = params.window.as_deref().unwrap_or("24h");
    let (duration, tick_step) = match window {
        "7d" => (chrono::Duration::days(7), chrono::Duration::days(1)),
        _    => (chrono::Duration::hours(24), chrono::Duration::hours(2)),
    };
    let now_utc = chrono::Utc::now();
    let window_start_utc = now_utc - duration;

    let runs = queries::get_timeline_runs(
        &state.pool,
        &window_start_utc.to_rfc3339(),
    ).await.unwrap_or_default();

    // Group into jobs (alphabetical; stable)
    // For each bar, compute:
    //   left_pct  = (start_utc - window_start_utc) / duration * 100
    //   end_utc   = bar.end_time.unwrap_or(now_utc)       // running → extend to now
    //   width_pct = (end_utc - start_utc) / duration * 100
    // Render times via state.tz.
    // ...
}
```

**Template:** see UI-SPEC § "Surface C: Timeline page" for verbatim HTML (page skeleton `templates/pages/timeline.html` + partial `templates/partials/timeline_body.html`).

---

### OBS-05: Structural parity is a code review invariant

There is no code to write for OBS-05 — it's the rule that the p50/p95 computation stays in Rust forever. Enforcement:

1. **Grep guard in CI (optional):** `rg -n 'percentile_cont|percentile_disc' src/` — fail build if present.
2. **No Postgres-only branch in `get_recent_successful_durations`.** Both `PoolRef` arms return raw `Vec<u64>`; neither computes a scalar.
3. **Documented at `src/web/stats.rs` top.**

---

### rc.2 Release Cut

Per D-22, Phase 13 makes ZERO changes to release mechanics. Execution steps (from `docs/release-rc.md`, which is the authoritative runbook):

1. **Verify all Phase 13 PRs merged to `main`.**
2. **Verify CI + compose-smoke green on main.**
3. **Verify `Cargo.toml` version is `1.1.0`.** [VERIFIED: Cargo.toml:3 — already at `1.1.0`]
4. **Run `scripts/verify-latest-retag.sh`** (Phase 12.1) to confirm `:latest` still equals `:1.0.1` digest before pushing rc.2.
5. **Preview release notes:** `git cliff --unreleased --tag v1.1.0-rc.2 -o /tmp/rc2-preview.md`. Inspect; if sections are wrong, fix conventional-commit messages on main before tagging.
6. **Cut tag locally** (maintainer-signed if possible): `git tag -a -s v1.1.0-rc.2 -m "v1.1.0-rc.2 — release candidate"` (or `-a` without `-s` for unsigned-annotated fallback).
7. **Push:** `git push origin v1.1.0-rc.2` → triggers `.github/workflows/release.yml`.
8. **Verify post-push checklist** in `docs/release-rc.md` § "Post-push verification" — all user-validated, not Claude-self-asserted (per `feedback_uat_user_validates.md`).
9. **Flip OBS-01..OBS-05 checkboxes in `.planning/REQUIREMENTS.md`** from `[ ]` to `[x]` as part of the close-out commit.

**Trust-anchor policy** (Phase 12 D-13): no `workflow_dispatch` tag cut. Maintainer's local git identity is the trust anchor.

## Code Patterns to Follow

| Pattern | File-path analog | How Phase 13 uses it |
|---------|------------------|----------------------|
| Dual-path SQL with `PoolRef` match | `src/db/queries.rs:580-653` (`get_dashboard_jobs`) | Template for `get_dashboard_job_sparks`, `get_recent_successful_durations`, `get_timeline_runs`. |
| Window-function dedup query | `src/db/queries.rs:555-577` (inner subquery with `ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY start_time DESC)`) | Sparkline query uses the identical pattern with a 20-row cap. |
| View-model `to_view()` hydration | `src/web/handlers/dashboard.rs:76-132` | Extend with `spark_cells`, `spark_badge`, etc. |
| HTMX partial/full dual response | `src/web/handlers/dashboard.rs:200-218` (`HxRequest` branch) | Timeline handler follows the same shape if we split into `/timeline` (full page) vs `/partials/timeline-body`. UI-SPEC locks a single route + `hx-swap="outerHTML"` on the body div, so a partial route is NOT required — but keep the option for future live updates. |
| Configuration card outer shape | `templates/pages/job_detail.html:23-68` | Duration card copies the `background:...;border:...;border-radius:...;padding:var(--cd-space-6)` envelope verbatim. |
| `cd-badge--{status}` class derivation | `templates/partials/job_table.html:12` (`cd-badge--{{ job.last_status }}`) | Sparkline cells use the same pattern with `cd-sparkline-cell--{{ cell.kind }}`. |
| Timezone threading via `state.tz` | `src/web/handlers/dashboard.rs:195` | Timeline handler reads `state.tz` once at the top; every axis tick and tooltip time is `dt.with_timezone(&state.tz).format(...)`. |
| HTMX poll wrapper | `templates/pages/dashboard.html:89-95` | Timeline body mirrors with `every 30s` + `hx-include="[name='window']"`. |
| In-memory SQLite test pool | `tests/common/v11_fixtures.rs:22-28` (`setup_sqlite_with_phase11_migrations`) | Every Phase 13 test file reuses this. |
| Real Postgres via testcontainers | `tests/db_pool_postgres.rs:10-14` + `tests/schema_parity.rs` | Timeline EXPLAIN-ANALYZE test and dual-backend percentile test both use this. |
| Dashboard render integration test | `tests/dashboard_render.rs:25-60` | Template for `tests/v13_sparkline_render.rs` (scan HTML for 20 cells + percent text). |
| Job-detail partial integration test | `tests/job_detail_partial.rs` | Template for `tests/v13_duration_card.rs`. |

## Risks & Pitfalls

1. **SQLite + Postgres schema drift on new queries.** The three new queries are SELECT-only with no migration, but a typo (e.g. `status = 'SUCCESS'` uppercase on one branch and `'success'` on the other) would skew the sparkline denominator silently on Postgres. **Mitigation:** every new `queries::*` fn gets a dual-backend integration test (SQLite in-memory + Postgres testcontainer) that seeds identical fixtures and asserts identical outputs. See `tests/schema_parity.rs` for the precedent.

2. **`idx_job_runs_start_time` not used on timeline query.** If the WHERE clause filters on `end_time` instead of `start_time`, the index is bypassed. **Mitigation:** Filter on `start_time >= window_start` per § "Implementation Approach → OBS-01/02"; add an assertion test `v13_timeline_explain.rs` that runs `EXPLAIN QUERY PLAN` on SQLite and scans for `USING INDEX idx_job_runs_start_time` (or `idx_job_runs_job_id_start` if the planner picks the composite). On Postgres, run `EXPLAIN (FORMAT JSON) ...` and assert `"Index Scan"` on the node touching `job_runs`. See § Validation Architecture.

3. **`stopped` runs miscounted in sparkline denominator.** D-05 is unambiguous: cells include stopped, denominator excludes them. But a naive `numerator=success_count, denominator=cells.len()` would include stopped and skew the rate. **Mitigation:** compute `denominator = terminal_count - stopped_count` explicitly in the handler, with a tight unit test for the "15 success, 4 stopped, 1 failed" case (expected: 15/16 = 94%, NOT 15/20 = 75%).

4. **Zero-run job crashes the dashboard view.** D-06 promises "never crashes"; a bug where `spark_by_job.get(&job_view.id)` returns `None` and the code unwraps would crash. **Mitigation:** unwrap_or_default() everywhere; T-V11-SPARK-01 test (seed a job with zero runs) is the canary.

5. **Running-run bar width goes negative or > 100%.** If a running run's `start_time` is before `window_start_utc`, the computed `left_pct` is negative. If `now - start_time > window_duration`, `width_pct > 100`. **Mitigation:** clamp `left_pct.max(0.0)` and `(width_pct).min(100.0 - left_pct)` in Rust before rendering. Add a test with a running run that started 30h ago on a 24h window.

6. **Divide-by-zero in success-rate when denominator = 0.** A job with 20 stopped runs has `terminal_count=20`, `stopped_count=20`, `denominator=0`. **Mitigation:** already handled by `if denominator < MIN_SAMPLES_FOR_RATE` check; `denominator=0 < 5 = MIN` → badge is `—`. Unit test with this fixture.

7. **Timezone DST transition at day boundary on 7d timeline.** If operator tz is `America/New_York` and the 7d window crosses the spring-forward Sunday, the "midnight" tick for that day is offset by an hour. **Mitigation:** compute tick positions using the tz-aware arithmetic in `chrono-tz` (`.with_timezone(&tz).date()` then roll days). Do NOT compute tick positions in UTC then format. Add a deliberate test at `v13_timeline_timezone.rs` that seeds a run across DST.

8. **`stopped` status color token resolution.** `--cd-status-stopped` is shipped (Phase 10 D-08). `--cd-status-cancelled` is NEW in this phase (UI-SPEC § "New `--cd-status-cancelled` color tokens"). Forgetting to declare the cancelled token in all three CSS locations (`:root` dark, `[data-theme="light"]`, `@media (prefers-color-scheme: light)`) would render cancelled cells as `initial` (invisible). **Mitigation:** UI-SPEC locks all three declaration sites; executor checklist verifies.

9. **rust-embed asset invalidation after CSS changes.** In debug, `rust-embed` reads from disk on every request, so Tailwind edits hot-reload. In release, the CSS is compiled-in. Operators upgrading from v1.0.1 → v1.1.0-rc.2 via the shipped Docker image need a fresh browser cache bust. **Mitigation:** not a v1.1 problem — the image is versioned, and operators already hard-reload after major upgrades. Do NOT add fingerprinted filenames in v1.1 (scope creep).

10. **p50/p95 `expect()` panics if someone calls without the N>=20 guard.** D-21 puts the threshold in the consumer. If a future surface calls `percentile()` on < 20 samples and unwraps, it will panic. **Mitigation:** percentile() returns `Option<u64>` so empty is safe. Handler uses `.expect("non-empty when N>=20")` with the guard above; change to `.unwrap_or(0)` if paranoid, but `expect()` is more informative. Include a comment referencing D-21.

11. **Sparkline oldest-to-newest vs newest-to-oldest ordering.** Query returns `ORDER BY id DESC` (newest first). Cells are displayed left-to-right. UI convention + UI-SPEC is oldest-on-the-left. **Mitigation:** `rows.into_iter().rev().collect()` in the handler before building cells. Add a unit test asserting cell order matches timestamp order.

12. **Duration formatter decimal-seconds vs floor-seconds drift.** `src/web/format.rs::format_duration_ms` emits `"1.2s"` for `< 60s` (one decimal, floating point). UI-SPEC Copywriting says `"42s"` for 1..59s (floor int). If we reuse `format_duration_ms` verbatim, the Duration card renders `"42.0s"` not `"42s"`. **Mitigation:** see § Open Questions. Planner picks: (a) extend the shared formatter to emit integer seconds for `1s..59s` (breaks all existing `duration_display` usages — risky), or (b) add a new `format_duration_ms_integer_seconds` helper in `src/web/format.rs` used ONLY for Phase 13 surfaces. Recommend (b).

13. **HTMX 30s poll races with window-pill navigation.** If operator clicks "7d" at T=29s, the existing `every 30s` timer fires at T=30s with the old `?window=24h` still in the hidden input. **Mitigation:** UI-SPEC puts the hidden input OUTSIDE `#timeline-body` so the pill click immediately re-renders the whole page (regular nav, not HTMX), resetting the window param. Verify in a manual test during UAT.

14. **Release-candidate tag format regression.** Must be `v1.1.0-rc.2` (dot before `rc.N`), NEVER `v1.1.0-rc2`. Phase 12 D-10 metadata-action depends on the dot for prerelease detection. **Mitigation:** `docs/release-rc.md` footnote already calls this out; Phase 13 inherits. PR-review check.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` + `cargo-nextest` (both configured; CI uses `cargo nextest run --all-features --profile ci`) |
| Config file | `Cargo.toml` + `.config/nextest.toml` (if present) |
| Quick run command | `cargo test -p cronduit --lib stats::` (runs only new `stats.rs` unit tests in <5s) |
| Full suite command | `cargo nextest run --all-features` (full integration suite; requires Docker daemon for Postgres testcontainer) |
| Phase gate | Full suite green before `/gsd-verify-work`; user-validated UAT per `feedback_uat_user_validates.md`. |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| **OBS-01** | `/timeline` renders gantt rows, alphabetical job order, bars color-coded | integration | `cargo test --test v13_timeline_render -- timeline_renders_rows_per_job` | ❌ Wave 0 (new test file) |
| **OBS-01** | Running bar pulses; status classes render for all 6 statuses | integration (HTML-scan) | same file: `test_pulse_class_on_running` | ❌ Wave 0 |
| **OBS-01** | Disabled jobs excluded; empty window message renders | integration | same file: `disabled_jobs_not_in_timeline`, `empty_window_message` | ❌ Wave 0 |
| **OBS-01** | `?window=24h` vs `?window=7d` both return 200 and render | integration | same file: `window_toggle_both_render` | ❌ Wave 0 |
| **OBS-02** | `T-V11-TIME-01` — single SQL query (query counter middleware or sqlx log scan), 10 jobs × 1000 runs under 100ms | integration | `cargo test --test v13_timeline_explain -- query_count_single` | ❌ Wave 0 |
| **OBS-02** | `T-V11-TIME-02` — `EXPLAIN QUERY PLAN` on SQLite shows `idx_job_runs_start_time` (or composite); on Postgres shows Index Scan | integration | same file: `explain_uses_index_sqlite`, `explain_uses_index_postgres` | ❌ Wave 0 |
| **OBS-02** | `T-V11-TIME-04` — tz=America/Los_Angeles, UTC 10:00Z run renders as 03:00 | integration | `cargo test --test v13_timeline_timezone -- pdt_label_correct` | ❌ Wave 0 |
| **OBS-02** | Hard LIMIT 10000 enforced (seed 15k runs; query returns ≤10000) | integration | `cargo test --test v13_timeline_explain -- limit_10000_enforced` | ❌ Wave 0 |
| **OBS-03** | `T-V11-SPARK-01` — 0 runs → empty sparkline, badge `—`; view doesn't crash | integration | `cargo test --test v13_sparkline_render -- zero_runs_no_crash` | ❌ Wave 0 |
| **OBS-03** | `T-V11-SPARK-02` — 3 runs → badge `—` (below N=5) | integration | same file: `below_threshold_shows_dash` | ❌ Wave 0 |
| **OBS-03** | `T-V11-SPARK-03` — 5 successful runs → badge `100%` | integration | same file: `at_threshold_all_success_hundred_percent` | ❌ Wave 0 |
| **OBS-03** | `T-V11-SPARK-04` — 15 success + 5 failed (20 total) → badge `75%` | integration | same file: `mixed_runs_integer_percent` | ❌ Wave 0 |
| **OBS-03** | `T-V11-SPARK-05` (project-adapted) — 15 success + 4 stopped + 1 failed → badge `94%` (stopped excluded from denominator) | integration | same file: `stopped_excluded_from_denominator` | ❌ Wave 0 |
| **OBS-03** | `T-V11-SPARK-06` (project-adapted) — rendered HTML contains exactly 20 `<span class="cd-sparkline-cell...">` per job row | integration (DOM-scan) | same file: `exactly_twenty_cells_rendered` | ❌ Wave 0 |
| **OBS-04** | `T-V11-DUR-01` — `percentile(&[], 0.5)` → `None` | unit | `cargo test --lib stats::tests::empty_slice_returns_none` | ❌ Wave 0 (new file `src/web/stats.rs`) |
| **OBS-04** | `T-V11-DUR-02` — `percentile(&[42], any_q)` → `Some(42)` | unit | `stats::tests::single_element_any_quantile` | ❌ Wave 0 |
| **OBS-04** | `T-V11-DUR-03` — `percentile(&[10..100 step 10], 0.5)` → `Some(50)` (ceil convention locked) | unit | `stats::tests::median_of_ten_returns_fifth_sample` | ❌ Wave 0 |
| **OBS-04** | `T-V11-DUR-04` — `percentile(&[1..100], 0.95)` → `Some(95)` | unit | `stats::tests::p95_of_ten_returns_last_sample` + `stats::tests::p50_p95_over_hundred_samples` | ❌ Wave 0 |
| **OBS-04** | N<20 → card renders `—` with correct `title` tooltip | integration | `cargo test --test v13_duration_card -- below_threshold_renders_dash` | ❌ Wave 0 |
| **OBS-04** | N=20 → card renders `p50 Xs` + `p95 Ys` from fixture | integration | same file: `at_threshold_renders_values` | ❌ Wave 0 |
| **OBS-04** | Subtitle matrix: N=0/1..19/20..99/100+ → correct strings per D-18 | integration | same file: `subtitle_matrix_all_ranges` | ❌ Wave 0 |
| **OBS-04** | p50/p95 query excludes failed/timeout/cancelled/stopped | integration | same file: `only_successful_runs_included` | ❌ Wave 0 |
| **OBS-05** | No `percentile_cont` / `percentile_disc` in `src/**` | static-grep | `! rg -q 'percentile_cont\|percentile_disc' src/ && echo PASS` | ❌ Wave 0 (new test file or justfile recipe) |
| **OBS-05** | Postgres path returns raw Vec<u64> (not scalar) — type-level enforcement | compile-time | `cargo check` (if `get_recent_successful_durations` ever returned `f64` the function signature would have to change) | ✅ via type system |

### Sampling Rate

- **Per task commit:** `cargo test -p cronduit --lib stats:: && cargo check` (< 10s).
- **Per wave merge:** `cargo nextest run --all-features --profile ci` (Docker daemon required for Postgres testcontainer tests; matches CI).
- **Phase gate:** full suite green + user-validated UAT of rendered surfaces (`/`, `/jobs/{id}`, `/timeline`) before `/gsd-verify-work`.

### Wave 0 Gaps

- [ ] `tests/v13_stats_percentile.rs` — covers OBS-04 unit (T-V11-DUR-01..04). *(Note: if unit tests live inline in `src/web/stats.rs` per § Implementation Approach, this file is NOT needed — merge the rows in the table above into the lib-test entry.)*
- [ ] `tests/v13_sparkline_render.rs` — covers OBS-03 integration (T-V11-SPARK-01..04 + stopped-denominator + exact-20-cells).
- [ ] `tests/v13_duration_card.rs` — covers OBS-04 integration (N threshold rendering, subtitle matrix, only-success filter).
- [ ] `tests/v13_timeline_render.rs` — covers OBS-01 integration (alphabetical rows, status classes, empty-window, window toggle).
- [ ] `tests/v13_timeline_explain.rs` — covers OBS-02 (single-query assertion, EXPLAIN index check, LIMIT 10000 enforcement). Dual-backend: SQLite + Postgres testcontainer.
- [ ] `tests/v13_timeline_timezone.rs` — covers T-V11-TIME-04 (DST / tz label correctness).
- [ ] (optional) `justfile` recipe or CI step: `just grep-no-percentile-cont` for OBS-05 static-grep guard.
- [ ] No framework install needed — all Phase 13 tests use the shipped test harness (`cargo test`, `testcontainers`, `tower::ServiceExt::oneshot`).

### Integration test plan: SQLite vs Postgres matrix

| Test class | SQLite | Postgres (testcontainer) | Rationale |
|------------|--------|--------------------------|-----------|
| `stats::percentile` unit tests | n/a (pure Rust) | n/a | No DB. |
| `v13_duration_card` render | YES | OPTIONAL | Tests view-model shape; DB-agnostic modulo the query. |
| `v13_sparkline_render` render | YES | OPTIONAL | Same. |
| `v13_timeline_render` basic | YES | OPTIONAL | Same. |
| `v13_timeline_explain` | **YES (required)** | **YES (required)** | T-V11-TIME-02 explicitly calls out "on both SQLite and Postgres". Dual-backend. |
| DST / tz test | YES | n/a (doesn't exercise tz at DB level) | Rust-side formatting, tz-config driven. |

The dual-backend requirement on `v13_timeline_explain` is the load-bearing test for OBS-02. The rest use SQLite in-memory for speed; Postgres is optional-but-encouraged for the schema-parity canary.

## Project Constraints (from CLAUDE.md)

These CLAUDE.md directives bind Phase 13 work:

- **Rust + bollard + sqlx + croner + askama_web 0.15 (axum-0.8 feature) locked.** Phase 13 uses no new crates.
- **rustls everywhere; `cargo tree -i openssl-sys` must return empty.** Phase 13 doesn't change deps; invariant holds.
- **TOML config; `[server].timezone` is mandatory.** Phase 13 reads `state.tz` only; no new config keys.
- **croner 3.0 locked.** Phase 13 uses existing `next_fire` computation unchanged.
- **Separate read/write SQLite pools.** All Phase 13 queries use `pool.reader()` — no writes.
- **Tests + GitHub Actions CI from phase 1.** All new test files land in `tests/` and run in the existing CI matrix (SQLite × Postgres × amd64 × arm64).
- **Clippy + fmt gate on CI.** Every new source file must pass `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt --check`.
- **Design-system fidelity: match `design/DESIGN_SYSTEM.md` (terminal-green).** Phase 13 uses only shipped `--cd-*` tokens plus the two UI-SPEC-locked additive tokens (`--cd-status-cancelled{-bg}`, `--cd-timeline-*`).
- **All diagrams are mermaid.** This research file's architecture diagram uses mermaid; PLAN.md and release notes MUST also use mermaid, not ASCII.
- **All changes via feature branch + PR; no direct commits to main.** Current branch is `gsd/phase-13-context` — next step is `gsd/phase-13-observability-polish` (or similar) before PLAN.md creation per GSD workflow.
- **Tag = Cargo.toml version.** `Cargo.toml` = `1.1.0`; tag will be `v1.1.0-rc.2` (full semver pre-release notation with dot before `rc.N`).
- **UAT requires user validation.** Post-cut verification checklist in `docs/release-rc.md` is user-run, not Claude-asserted.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `serde-yaml` YAML config | `toml 1.1.2` | v1.0 | Locked; Phase 13 does not add config. |
| `cron` crate (5-field only) | `croner 3.0` (DST-aware + `L`/`#`/`W`) | v1.0 | Phase 13 consumes unchanged. |
| `askama_axum 0.5+deprecated` | `askama_web 0.15` with `axum-0.8` feature | v1.0 | Phase 13 adds new `askama_web::WebTemplateExt` calls consistent with shipped handlers. |
| SQL-native percentile_cont | Rust-side `percentile()` helper | v1.1 (Phase 13) | OBS-05 lock; irreversible for v1.x. |
| Manual tag cut for releases | Phase 12 D-13 runbook | v1.1 Phase 12 | Phase 13 reuses verbatim. |

**Deprecated/outdated** (none introduced or removed by Phase 13):
- Phase 13 does NOT touch HTMX 2.0.4 vendored at `assets/vendor/htmx.min.js`. Do NOT upgrade to HTMX 4.x in v1.1 (SUMMARY.md § "What NOT to Change During v1.1" #5).

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `idx_job_runs_start_time` is used by a `WHERE start_time >= ?1` predicate on both SQLite and Postgres at the shipped index shape. | § OBS-01/02 query shape | If the planner chooses a different index, timeline query scans full `job_runs`. Test `v13_timeline_explain` catches this before merge. |
| A2 | "Last 24h" means "runs that STARTED in the last 24h", NOT "runs that were ACTIVE in the last 24h". | § OBS-01/02 | If operators expect the latter semantic (an 8h job that started 25h ago but ended 17h ago SHOULD show), the timeline silently excludes that case. Low frequency at homelab scale. Recommend confirming with user during /gsd-plan-phase Q&A. |
| A3 | `duration_display` shared formatter emits `"1.2s"` for sub-minute durations (decimal float), but UI-SPEC copywriting says `"42s"` (floor int). This is a visible divergence between Phase 13's locked copy and the existing `run_history.html` column. | § OBS-04 + § Risks #12 | Card shows `"42.0s"` not `"42s"`. Either extend shared formatter (touches every page with `duration_display`) or add a Phase-13-only variant. Planner decides. |
| A4 | Prometheus `histogram_quantile` uses linear interpolation on bucketed histograms — NOT nearest-rank on raw samples. CONTEXT D-19 says "matches Prometheus semantics" but in practice the semantics differ: Prometheus `quantile_over_time(phi, v)` uses linear interpolation too. D-19's nearest-rank choice is still valid (it's what CONTEXT locked), but the Prometheus-semantics rationale in the discussion log is not a perfect match. | § OBS-04 | Zero functional impact — D-19 is locked on nearest-rank regardless of rationale. Flagged for documentation hygiene only. |
| A5 | `stopped` status is written only by the Stop-button handler (Phase 10 SCHED-14), not by any orphan-recovery path. The sparkline assumes `stopped` ≈ "operator interrupt". | § OBS-03 denominator semantics | If a future refactor writes `stopped` for non-operator reasons, the denominator exclusion becomes counter-intuitive. Mitigated by the T-V11-BULK-01-style invariant locks; no active risk in Phase 13. |
| A6 | Dashboard's existing 3s poll cadence is sufficient for sparkline freshness. Phase 13 adds no new polling. | § OBS-03 | If the extra sparkline query slows down `/partials/job-table` below 3s cycle, polling backs up. At homelab scale (< 50 jobs) this is negligible; verify in UAT. |
| A7 | `v1.1.0-rc.2` tag-cut workflow verified on rc.1 (Phase 12). All release-mechanics invariants held. `:latest` pin verified by Phase 12.1. | § rc.2 Release Cut | If Phase 12.1 `verify-latest-retag.sh` regressed (unrelated to Phase 13), the rc.2 cut could move `:latest`. Mitigated by Phase 12 D-10 `enable=${{ !contains(github.ref, '-') }}` gate in `release.yml`. |

**Planner action on [ASSUMED]:** A2 and A3 should be confirmed during planning before being written into task descriptions.

## Open Questions for Planner (RESOLVED)

1. **Timeline filter on `start_time` vs `end_time`.**
   - What we know: `idx_job_runs_start_time` is on `start_time`. PITFALLS.md sketch used `end_time >= ?1 OR status = 'running'`.
   - What's unclear: whether "last 24h" should include runs that started BEFORE the window but ended INSIDE it.
   - Recommendation: Filter on `start_time >= window_start` (Option 1 in § Implementation Approach → OBS-01/02). Simpler, uses the index cleanly, matches "runs that fired in the last 24h" intuition. Confirm with user if they disagree.
   - RESOLVED: Filter on `start_time >= window_start`. Implemented via `queries::get_timeline_runs` shipped in plan 02 (WHERE `jr.start_time >= ?1`) and consumed by `timeline` handler in 13-05-PLAN.md Task 1 (`window_start_utc = now_utc - window_duration`).

2. **Duration formatter: extend shared or add Phase-13-only variant.**
   - What we know: `src/web/format.rs::format_duration_ms` emits `"1.2s"` (decimal). UI-SPEC copywriting wants `"42s"` (floor int) for 1s..59s.
   - What's unclear: whether to mutate the shared formatter (touches Run History column across all pages — visible regression) or add `format_duration_ms_display_floor` used only in Phase 13.
   - Recommendation: Add a new helper `format_duration_ms_floor_seconds` in `src/web/format.rs` used by Duration card + timeline tooltip + sparkline cell tooltip (three new surfaces). Leaves shipped `run_history.html` behavior unchanged. Alternative: change the shared formatter — small user-visible change, matches UI-SPEC more cleanly. Planner to decide.
   - RESOLVED: Add new helper, do not mutate shared formatter. Implemented in 13-01-PLAN.md Task 2 as `format_duration_ms_floor_seconds` (shipped formatter `format_duration_ms` left byte-identical). Consumed by plans 03 (Duration card), 04 (sparkline cell tooltip), and 05 (timeline tooltip).

3. **`stats.rs::percentile` test location: inline in `src/web/stats.rs` vs external `tests/v13_stats_percentile.rs`.**
   - What we know: UI-SPEC says "`src/web/stats.rs` ~40 LOC with tests" — inline. Codebase precedent (`src/web/format.rs:23-34`) has inline unit tests.
   - What's unclear: whether external integration tests add value.
   - Recommendation: Inline `#[cfg(test)] mod tests`. No external test file. Reduces task count by one.
   - RESOLVED: Inline `#[cfg(test)] mod tests` in `src/web/stats.rs`. Implemented in 13-01-PLAN.md Task 1 (8 `#[test]` fns covering T-V11-DUR-01..04 + edge cases). No external test file added.

4. **Integration test for OBS-05 static guard: CI-only grep or also a Rust test?**
   - What we know: OBS-05 is a policy invariant, not a runtime behavior.
   - What's unclear: whether to encode it as a `#[test]` in a module like `tests/v13_obs05_guard.rs` that uses `include_dir!` to scan `src/` strings, or as a CI step in `justfile` / GitHub Actions.
   - Recommendation: CI grep step (`just grep-no-percentile-cont`) invoked from `ci.yml`. Lower cognitive load than a reflection-style Rust test. Record as plan.
   - RESOLVED: CI grep guard only. Implemented in 13-06-PLAN.md Task 3 as `just grep-no-percentile-cont` recipe invoked from the lint job in `.github/workflows/ci.yml`. No Rust-side guard added.

5. **rc.2 close-out commit shape.**
   - What we know: D-22 mandates no workflow file edits. Flipping OBS-01..OBS-05 checkboxes in `.planning/REQUIREMENTS.md` happens in the same commit as the final feature PR.
   - What's unclear: whether REQUIREMENTS.md gets its own commit on the feature branch or rolls into one of the feature commits.
   - Recommendation: Separate `docs(13): mark OBS-01..OBS-05 complete` commit on the feature branch, landing with the final Phase 13 PR merge. Keeps commit history clean for `git-cliff` release notes.
   - RESOLVED: Separate commit. Implemented in 13-06-PLAN.md Task 4 with commit message `docs(13): mark OBS-01..OBS-05 complete` on the feature branch (distinct from ci/test commits for clean git-cliff grouping).

6. **Timeline query: apply `MAX_WINDOW_RUNS` cap via ORDER BY + LIMIT, or OFFSET pagination?**
   - What we know: `LIMIT 10000` is the hard cap (OBS-02 lock).
   - What's unclear: if the query returns 10000 rows, does the UI show a "truncated — showing first 10000" banner?
   - Recommendation: show the banner. Pragmatic. One extra `<p>` in the template when `runs.len() == 10000`. Add to plan.
   - RESOLVED: Show the banner. Implemented in 13-05-PLAN.md Task 1 (handler sets `truncated = runs.len() == 10_000`) and Task 3 (`{% if truncated %}<p>Showing first 10000 of many runs — narrow the window for a complete view.</p>{% endif %}` rendered above `#timeline-body` swap target).

## Sources

### Primary (HIGH confidence)
- `/Users/Robert/Code/public/cronduit/.planning/phases/13-observability-polish-rc-2/13-CONTEXT.md` — all 23 locked decisions (D-01..D-23).
- `/Users/Robert/Code/public/cronduit/.planning/phases/13-observability-polish-rc-2/13-UI-SPEC.md` — approved visual contract, selector set, copy strings.
- `/Users/Robert/Code/public/cronduit/.planning/REQUIREMENTS.md` — OBS-01..OBS-05 wording and T-V11-* identifiers.
- `/Users/Robert/Code/public/cronduit/src/db/queries.rs:528-653` — `get_dashboard_jobs` shows dual-backend + window-function pattern Phase 13 clones.
- `/Users/Robert/Code/public/cronduit/src/web/handlers/dashboard.rs:76-219` — `to_view()` hydration, `state.tz` usage, HTMX partial/full dual response.
- `/Users/Robert/Code/public/cronduit/src/web/handlers/job_detail.rs:37-234` — Duration card insertion target, existing `RunHistoryView::duration_display`.
- `/Users/Robert/Code/public/cronduit/src/web/mod.rs:47-87` — router shape, `AppState.tz` field.
- `/Users/Robert/Code/public/cronduit/migrations/sqlite/20260410_000000_initial.up.sql:45-46` — `idx_job_runs_start_time` definition.
- `/Users/Robert/Code/public/cronduit/templates/pages/job_detail.html:23-76` — Configuration card shape for Duration card template.
- `/Users/Robert/Code/public/cronduit/templates/base.html:20-38` — nav link pattern for Timeline addition.
- `/Users/Robert/Code/public/cronduit/assets/src/app.css:30-165` — declared `--cd-*` tokens (dark + light + auto-detect).
- `/Users/Robert/Code/public/cronduit/docs/release-rc.md` — rc.2 cut runbook (Phase 12 D-11).
- `/Users/Robert/Code/public/cronduit/.github/workflows/release.yml:107-135` — Phase 12 D-10 tag-gating patches (rc.2 inherits).
- `/Users/Robert/Code/public/cronduit/Cargo.toml:3` — version = "1.1.0".
- `/Users/Robert/Code/public/cronduit/.planning/research/PITFALLS.md:620-830` — T-V11-TIME / T-V11-SPARK / T-V11-DUR test-case definitions.
- `/Users/Robert/Code/public/cronduit/tests/common/v11_fixtures.rs` — reusable test fixtures.
- `/Users/Robert/Code/public/cronduit/tests/schema_parity.rs` + `/Users/Robert/Code/public/cronduit/tests/db_pool_postgres.rs` — testcontainer Postgres pattern.

### Secondary (MEDIUM confidence)
- [Context7 `/statrs-dev/statrs` docs fetch](https://docs.rs/statrs/latest/statrs/) — confirmed statrs offers no drop-in `percentile` helper; hand-rolled 40-LOC implementation is the right call for Phase 13 scope. [VERIFIED: ctx7 query 2026-04-21]
- [SQLite ROW_NUMBER window function support ≥3.25](https://sqlite.org/windowfunctions.html) — shipped via `libsqlite3-sys` ≥0.30 (SQLite ≥3.46) in sqlx 0.8. [CITED: docs.rs/crate/libsqlite3-sys]

### Tertiary (LOW confidence)
- Prometheus `histogram_quantile` semantics (linear interpolation on bucketed histograms, not nearest-rank on raw samples). Affects A4 assumption only; no functional impact. [CITED: prometheus.io/docs/prometheus/latest/querying/functions/#histogram_quantile]

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all versions verified against Cargo.toml; no new deps.
- Architecture: HIGH — every integration point verified against shipped code line-by-line.
- Pitfalls: HIGH — 14 risks catalogued, each with mitigation grounded in existing code pattern.
- Validation: HIGH — test-map fully covers all T-V11-* identifiers.

**Research date:** 2026-04-21
**Valid until:** 2026-05-21 (30 days — Phase 13 scope is stable; CONTEXT locked 2026-04-20; UI-SPEC approved 2026-04-21).
