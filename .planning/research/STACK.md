# Stack Research — v1.1 "Operator Quality of Life"

**Domain:** Rust cron scheduler (Cronduit) — subsequent milestone, stack locked
**Researched:** 2026-04-14
**Confidence:** HIGH
**Scope:** Additions / version bumps required to deliver the v1.1 feature set on top of the already-shipped v1.0.1 stack. The full v1.0 stack was locked by prior research at `.planning/milestones/v1.0-research/STACK.md` and is NOT re-evaluated here.

---

## TL;DR

**No new runtime dependencies are required to deliver v1.1.** Every target feature can be implemented on the existing stack using application code plus (optionally) low-risk patch bumps on already-locked crates.

The single most consequential finding is a **SQL portability gotcha** for the p50/p95 duration trend feature: SQLite does not ship `percentile_cont` in stock builds (it requires `-DSQLITE_ENABLE_PERCENTILE` at compile time, which neither `rusqlite` nor `sqlx-sqlite` enable), while PostgreSQL has it natively. The application must compute percentiles in-process over a bounded window, or use a dialect-specific SQL path per backend. This is a **pattern decision**, not a crate change.

The only dependency hygiene item worth landing during v1.1 is the `rand` crate, which is pinned at `"0.8"` in `Cargo.toml` while the current line is 0.10.1. There is no published CVE on rand 0.8.5, but it is stale, and a polish milestone is the appropriate place to pay that down. See §3 below.

---

## 1. Question-by-Question Findings

### 1.1 Kill a running job (new `stopped` status)

**Verdict: NO CHANGE NEEDED — tokio + bollard already sufficient.**

- **Command / script jobs** (`type = "command" | "script"`) — terminated via `JoinHandle::abort()` on the tokio task that owns the child process, combined with the existing `tokio-util = "0.7.18"` feature `"rt"` (already enabled) for `CancellationToken` signalling. `tokio` 1.51.1 is locked and feature-complete for this; tokio 1.52.0 released today (2026-04-14) is a routine patch and not required for v1.1.
- **Docker jobs** (`type = "docker"`) — terminated via `bollard::Docker::kill_container(name, Some(KillContainerOptions { signal: "SIGKILL" }))`. Verified: `kill_container` is present in `bollard 0.20.2` with the signal parameter exposed through `KillContainerOptionsBuilder`. This is the same version already in `Cargo.lock` — no bump required.
- **Semantics**: the spec calls for a "single hard kill" — which matches SIGKILL directly for docker jobs and `JoinHandle::abort()` + child-process SIGKILL for command/script jobs. There is no need for a graceful-timeout → force-kill ladder in v1.1, which keeps the implementation minimal.
- **Feature flags**: `tokio = { features = ["full"] }` already enables `"process"`, `"signal"`, `"sync"`, `"rt-multi-thread"`, and `"time"` — all of which are needed. No feature flag additions required.
- **Existing code evidence**: `src/scheduler/mod.rs` already imports `tokio::task::{JoinHandle, JoinSet}` and has the scheduler loop shape needed to track running jobs by run_id. The v1.0 scheduler retains JoinHandles long enough to honor the graceful-shutdown drain, so the hooks for a "stop this run_id" path already exist conceptually.

**What needs to happen is purely application code**: an `mpsc` channel (symmetric with the existing "Run Now" channel) carrying `StopRun(run_id)` messages into the scheduler, a map from `run_id → JoinHandle` (or `run_id → CancellationToken` for cleaner cooperative cancellation on command/script), and a new `RunStatus::Stopped` variant with a dialect-compatible DB migration (CHECK constraint string on SQLite, enum on Postgres if one exists — checked at phase-plan time).

**Confidence: HIGH** — `kill_container` API verified against docs.rs/bollard/0.20.2.

---

### 1.2 SSE log backfill + ordering fixes

**Verdict: NO CRATE CHURN — all three bugs are application-code.**

Reviewed `src/web/handlers/sse.rs` directly. The current handler:

1. Looks up `active_runs` for the `run_id`, subscribes to the broadcast channel if found, otherwise emits `run_complete` and closes.
2. Yields log lines as `log_line` events until `RecvError::Closed`.
3. Has no backfill path — it does not read historical rows from the `run_logs` table before subscribing.

The three known v1.1 bugs map cleanly to fixes in this handler and its upstream partials, with zero dep changes:

| Bug | Root cause | Fix surface |
|-----|-----------|-------------|
| Lines out of order across live→static transition | HTMX swaps the static `<pre>` from polled partial while SSE is still inserting into the live container — two separate DOM targets reach "render done" out of order | Handler + template coordination: emit a single `run_complete` event, front-end drops live container and shows the DB-rendered static `<pre>` **only after** receiving `run_complete`; server guarantees the static partial contains every line up to `completed_at` before returning it |
| Transient "error getting logs" on page load | Race: page renders → HTMX requests static log partial → partial handler queries DB before the first log line has been flushed (or while the run exists in `active_runs` but has no rows yet) → returns empty / error | Partial handler returns an empty-state marker (never an error) when the run is active but has no logs yet; SSE handler yields the pending/backfill path below |
| Need to backfill logs on navigation | SSE handler joins mid-stream and the broadcast channel has no replay | SSE handler reads `run_logs WHERE run_id = ? AND id > ? ORDER BY id` from the write pool (or a fresh SELECT against the read pool) **before** calling `.subscribe()`, yields those rows first, then attaches to the broadcast. Race-free because broadcast `.subscribe()` captures all future messages regardless of when it's called — the handler only needs to avoid *gaps* by remembering the max `id` it yielded from the SELECT and filtering the broadcast stream against that watermark. |

**Stack items already present and sufficient:**
- `axum::response::sse::{Sse, Event, KeepAlive}` — core axum module, reachable under current features. (No separate `"sse"` feature flag needed in axum 0.8; the module compiles under `default-features = false` with the currently enabled feature set. Verified by the v1.0.1 build which already uses it.)
- `async-stream = "0.3.6"` — already enabled, already used for the `stream! { … }` block in the SSE handler. Still the right tool.
- `tokio::sync::broadcast` — already used, with `RecvError::Lagged` handled via a `[skipped N lines]` marker. Keep as-is; backfill on re-navigation sidesteps Lagged in the common case because the reconnected client begins from the DB watermark.
- `axum-htmx = "0.8"` — already used; `HxRequest` extractor handles the partial-vs-full render branch on the page-load path.

**Rejected alternatives** (and why they are noise for this milestone):
- `axum-extra::sse::*` — no such module exists; SSE lives in `axum::response::sse`. No switch needed.
- `tokio-stream 0.1.x` — would add a dep for what `async-stream` already does. Rejected.
- Any "SSE library" (e.g., `eventsource-stream`) — those are *client* libraries. Rejected.
- Websockets — already enabled on axum via `ws` feature? It is not. Do **not** add `"ws"` feature to axum for this milestone; SSE is the right transport for one-way log tail and HTMX has first-class SSE support. Rejected.

**Confidence: HIGH.** Reviewed the actual SSE handler source; confirmed the fix is pure application logic plus coordinated template updates.

---

### 1.3 Gantt-style run timeline (last 24h / 7d)

**Verdict: HAND-ROLL inline SVG or CSS-grid inside an askama template. NO CRATE.**

**Ecosystem survey:**

| Crate | Type | Verdict for Cronduit | Reason |
|-------|------|---------------------|--------|
| `gantt_chart` (jlyonsmith) | **CLI binary**, not a library | REJECTED | Not a reusable crate. Emits an SVG file via a command-line entry point. Useless for server-rendering inside askama. |
| `plotters` | Plotting framework with SVG backend | REJECTED | Designed for static chart output (images), pulls a large dep tree (font rendering, color spaces, backend abstractions). Overkill for what is effectively a row of colored `<rect>` elements. |
| `charming` | Apache ECharts wrapper | **REJECTED — VIOLATES CONSTRAINT** | Outputs a chart definition that requires the ECharts **JavaScript** runtime in the browser to render. Adds a JS framework dep, violating "no SPA, no JS framework, no CDN, no WASM". |
| `lodviz-rs` | Pure-Rust SVG visualization | **REJECTED — VIOLATES CONSTRAINT** | Compiled to WASM and designed for Leptos apps. Adds a WASM bundle to the frontend, violating the single-binary / no-WASM constraint. |
| `svg` (jeremyletang) | Low-level SVG DOM builder | Technically usable, not needed | 30 KB runtime on top of askama's `format!` — an `<svg>` block inside an askama template is shorter and zero-dep. |

**Recommendation:** a plain askama template partial that renders one `<svg>` per job row (or one big `<svg>` with rows grouped by job) using a time-range-to-pixel helper in Rust. Structure:

```
Rust (handler):
  - Load runs WHERE started_at >= now() - window ORDER BY job_id, started_at
  - Group into Vec<JobTimelineRow { job_id, segments: Vec<{start_frac, width_frac, status}> }>
  - Pass to askama template

Template (askama):
  - One <svg viewBox="0 0 1000 N"> per page, or one per job row
  - Each segment is a <rect x="{{ seg.start_px }}" width="{{ seg.width_px }}" …
    fill="var(--cd-status-{{ seg.status }})">
  - Hour/day gridlines as <line>; HTMX tooltip on hover via hx-get to a detail partial
```

**Why this is the right call:**
- Pure server-rendered HTML — the timeline is just another partial, swappable by HTMX.
- Zero new deps. Zero runtime JS beyond vendored HTMX 2.0.4.
- Matches the existing `design/DESIGN_SYSTEM.md` terminal palette using the existing `--cd-status-*` CSS variables (already in the Tailwind layer).
- Server-side grouping is O(runs_in_window), trivially under 10k rows for a 7-day window on a homelab.
- Time-to-pixel math is ~15 lines of Rust. Color-by-status is a `match`.

**Rejected alternatives reiterated for the roadmap:**
- Do NOT pull `plotters`, `charming`, `chartjs-rs`, or any SVG-via-Rust-DSL crate. Every option inspected either violates the no-JS/no-WASM constraint or is heavier than the askama `format!` it would replace.

**Confidence: HIGH.** Ecosystem survey confirms no small, well-maintained, server-only SVG gantt crate exists in Rust. Hand-rolling is the idiomatic 2026 answer.

---

### 1.4 Sparklines and p50/p95 duration trend

**Verdict: HAND-ROLL inline SVG sparklines. Compute percentiles in Rust (or via dialect-specific SQL — see pitfall).**

**Sparkline crate survey:**

| Crate | Verdict | Reason |
|-------|---------|--------|
| `sparkline` (ferrouswheel/rust-sparkline) | REJECTED | Renders **Unicode** sparklines (▁▂▃▄▅▆▇█) plus optional PNG via iTerm2 escape codes. No SVG output. Useful for CLI, not web UI. |
| `embedded-graphics-sparklines` | REJECTED | Targets `embedded-graphics` (no_std, for tiny OLED displays). Not a web SVG renderer. |
| `plotters` (sparkline mode) | REJECTED | Same reasons as §1.3 — too heavy, designed for standalone chart files. |
| `svg` (jeremyletang) | Technically usable | Still shorter to `format!` a `<polyline>` inside askama. |

**Recommendation:** a single askama helper partial `sparkline.html` that takes a `&[f64]` (or `&[i64]` duration-ms) and renders:

```html
<svg viewBox="0 0 {{ width }} {{ height }}" class="cd-sparkline">
  <polyline fill="none" stroke="var(--cd-accent)"
            points="{{ self.points() }}"/>
</svg>
```

With `points()` computed by a 20-line helper: normalize values to the viewBox, emit `x,y` pairs. Done.

**Success-rate badge** is trivial (`COUNT(status='success') / COUNT(*)` over a window) and needs no crate at all — a `<span>` with a color-coded class.

**Percentile (p50 / p95) — the portability pitfall:**

This is the one place v1.1 hits a genuine cross-backend SQL dialect difference:

| Backend | `percentile_cont` / `percentile_disc`? | Availability in our build |
|---------|----------------------------------------|---------------------------|
| **PostgreSQL** (`postgres` feature of sqlx) | Native since 9.4 (2014) | **Available.** `SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY duration_ms) FROM job_runs WHERE job_id = $1 AND started_at > $2` works out of the box. |
| **SQLite** (`sqlite` feature of sqlx) | Added as an *optional* amalgamation extension in SQLite 3.51.0 (2025-11-04), gated by `-DSQLITE_ENABLE_PERCENTILE` | **NOT available.** `libsqlite3-sys` (pulled transitively by `sqlx-sqlite`) does not set this compile flag, and none of the standard sqlx/rusqlite feature flags enable it. We'd have to maintain a custom sqlite build to get it, which is a non-starter. |

**Three options for the roadmap, in descending order of preference:**

1. **In-process percentile (RECOMMENDED)** — SELECT the raw `duration_ms` column for the last N runs (e.g. 100) into a `Vec<i64>`, sort, pick `v[50]` and `v[95]` (or the interpolated equivalent). Zero dialect drift, zero new SQL surface, identical code on SQLite and Postgres. The N is bounded and small, so the network + memory cost is negligible. **Use this.**
2. **Dialect-specific query path** — a `percentile_p50_p95(job_id)` helper with two `cfg`-selected or runtime-dispatched implementations (Postgres uses `percentile_cont`, SQLite uses the row-ranking window-function workaround `NTH_VALUE ... OVER (ORDER BY duration_ms ROWS BETWEEN …)`). Pure DB approach, but adds dialect drift right where v1.0 has structural parity. Only consider if (1) measurably slow at homelab scale, which it will not be.
3. **Approximate with the metrics facade** — `metrics-exporter-prometheus` already captures a `cronduit_run_duration_seconds{job}` histogram. Histograms emit quantile approximations natively for Prometheus scraping, but the *raw histogram buckets* are not easily reused to render a number in the UI. **Do not** try to round-trip through Prometheus. Rejected.

**No new crate required for percentiles.** The existing `metrics`/`metrics-exporter-prometheus` stack continues serving the Prometheus scrape use case unchanged; the UI badge/trend is computed independently from the SQL source of truth.

**Confidence: HIGH** on the "no crate needed" conclusion; **HIGH** on the SQLite percentile_cont gap (verified against sqlite.org/percentile.html).

---

### 1.5 Schema migration on startup for per-job run numbers

**Verdict: Existing `sqlx migrate` infrastructure is sufficient. Apply an idempotent backfill pattern, no new crate.**

The v1.0 migration setup already runs `sqlx migrate` on startup against both SQLite and Postgres, with dialect-specific migration folders where needed. The `job_run_number` column is a straightforward additive migration:

1. `ALTER TABLE job_runs ADD COLUMN job_run_number INTEGER NULL;`
2. Backfill: assign sequential numbers per `job_id` ordered by `started_at, id`.
3. Add an index on `(job_id, job_run_number)` and make it `NOT NULL` (or keep nullable if a soft migration is preferred — see pattern below).

**On idempotent long-running backfill** — the question asked about a "batched UPDATE in a loop with a resumable checkpoint" pattern.

**Reality check for Cronduit's scale:** a homelab user will have thousands to low-tens-of-thousands of `job_runs` rows (daily backups × 90-day retention × a handful of jobs ≈ a few thousand rows). A single `UPDATE ... SET job_run_number = ... FROM (SELECT ... ROW_NUMBER() OVER (...)) ...` executes in **under a second** at this scale on SQLite (WAL mode) and Postgres alike. A chunked/resumable backfill pattern is over-engineering here.

**Recommended simple pattern (works on both backends):**

- Migration `N_add_job_run_number.sql` adds the column nullable.
- Migration `N+1_backfill_job_run_number.sql` (dialect-specific if needed) does a single statement:
  - **Postgres:** `UPDATE job_runs AS jr SET job_run_number = sub.rn FROM (SELECT id, ROW_NUMBER() OVER (PARTITION BY job_id ORDER BY started_at, id) AS rn FROM job_runs) sub WHERE jr.id = sub.id AND jr.job_run_number IS NULL;`
  - **SQLite:** equivalent using a CTE with `ROW_NUMBER()` (supported since 3.25). The `UPDATE ... FROM` form is supported since SQLite 3.33 (2020-08-14) — well below the floor `libsqlite3-sys` bundles.
- Migration `N+2_job_run_number_not_null.sql` flips the column to `NOT NULL` with the conventional SQLite table-rewrite or Postgres `ALTER COLUMN` dance.
- Runtime code in `src/scheduler/mod.rs` assigns `MAX(job_run_number) + 1` at run start, inside the same transaction that inserts the `job_runs` row, guarded by a per-job advisory path (row lock on Postgres, serialized writer on SQLite — the existing single-writer pool already provides this).

**Idempotence notes:**
- `sqlx migrate` tracks applied migrations in `_sqlx_migrations`, so backfill runs exactly once.
- If a user downgrades and re-upgrades (or the backfill is ever re-run for any reason), the `WHERE jr.job_run_number IS NULL` guard keeps it a no-op for already-numbered rows.
- Adding a new column to a hot table requires the sqlite writer pool to briefly be exclusive, which it already is during startup (migrations run before the HTTP server opens its socket — verified as the v1.0 startup order).

**For the roadmap:**
- **No sqlx feature flags to add.** `migrate` is already enabled in the sqlx feature set.
- **No crate change.** `sqlx = "0.8.6"` is still current stable (0.9.0-alpha.1 exists but is pre-release).
- **Pattern note:** if v1.2+ ever wants a *true* chunked/resumable backfill (e.g., rewriting log contents), consider the "watermark table" pattern — a `backfill_state` row tracking the max processed id — driven from application code rather than a migration SQL file. Out of scope for v1.1.

**Confidence: HIGH** — `sqlx 0.8.6` is current stable, the migration approach is standard, and the dataset size makes resumable backfill unnecessary.

---

### 1.6 Cargo.toml version bump

**Verdict: Bump to `1.1.0` on the first v1.1 commit. Tag `v1.1.0-rc.1` (note the dot) at the first rc cut.**

- The locked convention (CLAUDE.md + PROJECT.md) requires `Cargo.toml` `version` to match the git tag. While developing v1.1 commits on `main` *between* rcs, the in-tree version should reflect the target: `1.1.0`. It is acceptable (and pragmatic) for the main branch to show `version = "1.1.0"` while the latest actually-shipped tag is still `v1.0.1`, because the user-facing matching rule applies **at tag time**.
- Alternative considered: keep `1.1.0-rc.1` in `Cargo.toml` and bump it for each rc. Rejected — noisier for no benefit, and pre-release semver strings interact badly with `cargo publish` and some resolver paths if Cronduit ever publishes to crates.io.
- Alternative considered: keep `1.0.1` until the first rc cut. Rejected — any operator who pulls a nightly/HEAD build will see `cronduit --version` lie about which milestone they're running.
- **Recommendation for the roadmap:** Phase 1 of v1.1 should include a single commit that bumps `Cargo.toml` `version` from `1.0.1` to `1.1.0` and updates any other version strings (CI constants, Docker label, README badges, `docs/CHANGELOG.md`). At first rc cut, tag `v1.1.0-rc.1` and publish GHCR `:v1.1.0-rc.1` + rolling `:rc`. At final cut, tag `v1.1.0` and move `:latest` off `v1.0.1`.

**Confidence: HIGH** — this is a workflow/policy decision, not a research question. The lean is consistent with PROJECT.md's stated release strategy.

---

### 1.7 Security audit flags

**Verdict: One dependency hygiene item. No blocking CVEs in the v1.0.1 lockfile.**

Cross-referenced the current `Cargo.toml` / `Cargo.lock` against RustSec advisories and crates.io latest versions (as of 2026-04-14):

| Crate | Pinned | Latest | Advisory? | Recommendation |
|-------|--------|--------|-----------|----------------|
| `tokio` | 1.51.1 | 1.52.0 (today) | None affecting our feature set | Optional: bump to `1.52.0` at leisure. Not required. |
| `axum` | 0.8.8 | 0.8.9 (today) | None | Optional patch bump. 0.8.9 adds WebSocket subprotocol helpers and a connect-endpoint routing fix; no SSE changes. Not required for v1.1. |
| `bollard` | 0.20.2 | 0.20.2 | None | Already current. |
| `sqlx` | 0.8.6 | 0.8.6 | None (0.9.0-alpha.1 exists, pre-release, do not adopt) | Already current. |
| `askama` / `askama_web` | 0.15.x | 0.15.x | None | Current. |
| `tower-http` | 0.6.8 | 0.6.8 | None | Current. |
| `hyper` | 1.x | 1.x | None | Current. |
| `chrono` | 0.4.44 | 0.4.44 | None | Current. |
| `serde` | 1.0.228 | 1.x | None | Current. |
| `croner` | 3.0.1 | 3.0.1 | None | Current. |
| `rust-embed` | 8.11 | 8.11 | None | Current. |
| `metrics` | 0.24.3 | 0.24.3 | None | Current. |
| `metrics-exporter-prometheus` | 0.18.1 | 0.18.1 | None | Current. |
| `notify` | 8.2.0 | 8.2.0 | None | Current. |
| `testcontainers` | 0.27.2 | 0.27.2 | None | Current. |
| `secrecy` | 0.10.3 | 0.10.3 | None | Current. |
| **`rand`** | **0.8** | **0.10.1 stable; 0.9.4 on the 0.9 line** | **None (not a CVE)** | **Stale by two major versions. See below.** |
| `clap` | 4.6 | 4.6.x | None | Current. |
| `tracing` / `tracing-subscriber` | 0.1.44 / 0.3.23 | current | None | Current. |
| `regex` | 1 | 1.x | None | Current. |
| `url` / `idna` (transitive) | inherited | current | RUSTSEC-2024-0421 was on `idna < 0.5.1`; any `url >= 2.5.4` pulls the fix | Verify `cargo tree -i idna` during v1.1 Phase 1 as a cheap sanity check. Expected: already patched. |

**The one actionable item: bump `rand` from "0.8" to "0.9.x"** (not 0.10 — see below).

- **Why it matters for a polish milestone:** `rand 0.8.5` was released 2022-02-14, has not had a patch release in four years, and is two major versions behind the current line. There is **no known CVE** against it (RustSec has advisories on *transitive* issues in 0.6-era deps only), so this is **dependency hygiene, not security remediation**. A polish milestone is the right place to land it.
- **Why 0.9.x and not 0.10.x:** `rand 0.10.1` raised the MSRV to Rust 1.85 and switched to edition 2024. Cronduit already requires Rust 1.94.1 / edition 2024, so MSRV is not a blocker, **but** `rand 0.10` was a multi-trait rename (`Rng::gen` → `Rng::random`, `SeedableRng::from_seed` changes, `rand_core` split). `rand 0.9.x` is the less-churn landing spot: API cleanups without the trait rename avalanche. For a polish milestone, 0.9 is lower-risk. If a later milestone wants the full 0.10 transition, it can do it in isolation.
- **Scope of the bump:** `rand` is used in Cronduit for `@random` slot picking and CSRF token bytes. Both are trivial call sites (a few `rng.gen_range()` calls, one `rng.fill_bytes()`). The migration is mechanical.
- **Also verify:** `rand 0.8` and `rand 0.9` can coexist in the lockfile if a transitive dep still requires 0.8 — `cargo tree -i rand` during Phase 1 will confirm whether dropping our direct 0.8 pin eliminates one copy or just adds a second. If two copies appear, that is fine for v1.1 (no size regression worth worrying about) but worth a follow-up note.

**`cargo audit` / `cargo deny` integration:** v1.0 did not ship with `cargo-deny` in CI. v1.1 is a reasonable milestone to add it, but this is an **additive CI hygiene item** with zero runtime impact — it's a roadmapper call whether to include it in v1.1 scope or defer to v1.2.

**Confidence: HIGH** on the audit table (versions verified against crates.io API 2026-04-14, advisories cross-referenced against rustsec.org). MEDIUM on the "rand 0.9 vs 0.10" preference — either is defensible, but 0.9 is the more conservative landing for a polish milestone.

---

## 2. Recommended Stack Additions

### Core Technologies — **no changes**

All locked v1.0 core technologies remain current and correct. See `.planning/milestones/v1.0-research/STACK.md` for the full version table.

### Supporting Libraries

| Library | Version | Purpose | When to Use | Status in v1.1 |
|---------|---------|---------|-------------|----------------|
| **(none)** | — | — | — | **No new supporting libraries required.** Every v1.1 feature ships on the existing dep set. |

### Development Tools

| Tool | Purpose | Notes | Status |
|------|---------|-------|--------|
| `cargo-deny` (optional) | Supply-chain gate (licenses + advisories + dup check) | Already recommended in v1.0 stack research but not wired into CI. A polish milestone is a reasonable place to add it, gated by a separate CI job so it can't block feature PRs. | **Optional addition to v1.1 CI scope** — roadmapper decision |

## 3. Recommended Version Bumps

**One dependency hygiene bump to land during v1.1. Everything else is current or a no-op patch.**

| Crate | From | To | Reason | Risk | Milestone |
|-------|------|----|--------|------|-----------|
| **`rand`** | `0.8` | `0.9.x` | Two majors stale; no CVE but polish-milestone hygiene | **LOW** — handful of call sites (`@random` slot picker, CSRF bytes), mechanical rename | v1.1 Phase 1 (bundle with the `1.0.1 → 1.1.0` version bump commit) |
| `Cargo.toml` `version` | `1.0.1` | `1.1.0` | Match target milestone on all non-tag commits | None | v1.1 Phase 1 |
| `tokio` (optional) | `1.51.1` | `1.52.0` | Released today; routine | None | Opportunistic; not required |
| `axum` (optional) | `0.8.8` | `0.8.9` | Released today; bugfix + WS helpers | None | Opportunistic; not required |

**No other crate in the v1.0.1 lockfile requires a bump for v1.1 feature scope or for security.**

## 4. Installation

No `cargo add` / `cargo remove` commands are required for v1.1 beyond the single `rand` version bump. The edit to `Cargo.toml`:

```toml
# Before
rand = "0.8"

# After
rand = "0.9"
```

…followed by a mechanical sweep of call sites (see §1.7 for the likely shape). `cargo build && cargo test` will flag every call site.

## 5. Alternatives Considered

| Recommended | Alternative | When Alternative Makes Sense |
|-------------|-------------|------------------------------|
| Hand-rolled SVG gantt inside askama | `plotters` with SVG backend | Never for this project — too heavy, designed for static image output, not for HTMX-swapped partials |
| Hand-rolled SVG gantt inside askama | `charming` (Apache ECharts wrapper) | Never — requires the ECharts JS runtime in the browser, violates the no-SPA constraint |
| Hand-rolled SVG sparklines | `sparkline` (Unicode) | If the badge were CLI-only. Web UI needs inline SVG. |
| In-process p50/p95 computation | Dialect-specific SQL (`percentile_cont` on Postgres, window functions on SQLite) | Only if profiling shows the in-process path is a hotspot — which is implausible at homelab scale |
| Keep `sqlx 0.8.6` | `sqlx 0.9.0-alpha.1` | Never during a polish milestone — no alphas |
| `rand 0.9` | `rand 0.10` | If/when a later milestone budgets for the full trait-rename migration |
| Hand-roll chunked backfill pattern | `sqlx migrate` single-statement `UPDATE ... ROW_NUMBER()` | For the per-job run number feature, the dataset is too small to justify chunking. Revisit only if a v1.2+ migration is actually slow. |
| `tokio::sync::broadcast` for live log tail | `tokio-stream::wrappers::BroadcastStream` | Current handler already uses raw broadcast + async-stream cleanly; no benefit to swapping |

## 6. What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `plotters` (any feature set) | Heavy dep tree; designed for standalone chart files; wrong abstraction for HTMX partials | Plain `<svg>` blocks inside askama templates |
| `charming` / any ECharts-wrapping crate | Requires ECharts JS runtime in the browser — violates the locked no-SPA / no-JS-framework constraint | Hand-rolled SVG |
| `lodviz-rs` or any WASM-visualization crate | Adds a WASM bundle to the frontend — violates the single-binary / no-WASM constraint | Hand-rolled SVG |
| `gantt_chart` crate | CLI binary, not a reusable library | Hand-rolled SVG |
| `sparkline` crate | Emits Unicode, not SVG — wrong output format for the web UI | Hand-rolled SVG polyline |
| Any "SSE library" beyond what axum + async-stream already provide | Application bugs, not upstream problems | Fix the handler and template coordination |
| `sqlx 0.9.0-alpha.1` | Pre-release during a polish milestone | Stay on `sqlx 0.8.6` |
| `rand 0.10.x` (during v1.1) | Large trait rename surface (`gen` → `random`) — unnecessary churn for a polish milestone | `rand 0.9.x` for v1.1; revisit 0.10 in a later milestone |
| Loadable SQLite percentile extension | Requires custom libsqlite3-sys build; breaks the "it just works on every platform" deployment story | In-process percentile computation over a bounded window |
| `tokio-cron-scheduler` (still) | Rejected in v1.0 research; nothing in v1.1 changes the calculus | Existing hand-rolled `tokio::select!` scheduler |
| `cargo audit` without `cargo deny` | Overlaps and is less capable | `cargo deny` (if added) |

## 7. Stack Patterns by Variant

**For the "stop a running job" feature:**
- Use `CancellationToken` (from `tokio-util`, already enabled) for command/script job cancellation — cooperative, cleaner exit handling.
- Use `bollard::Docker::kill_container(name, Some(KillContainerOptions { signal: "SIGKILL" }))` for docker jobs.
- Track `run_id → (CancellationToken, Option<docker_name>)` in a new `RwLock<HashMap>` on `AppState`. The entry is inserted at run start and removed on completion, mirroring the existing `active_runs` log broadcast map.

**For SSE log backfill:**
- Store the last-yielded `log_id` watermark in the SSE handler.
- SELECT historical rows up to the watermark *before* calling `broadcast.subscribe()`.
- Yield historical rows first, then attach to the broadcast and filter `id > watermark` on received events.
- **Tradeoff:** a small duplication window is possible if a log row is flushed to DB between the SELECT and the `.subscribe()` call. Mitigate by keeping the watermark from the SELECT and comparing against a monotonic `LogLine.id` field (extend `LogLine` with `id: i64`).

**For percentile computation:**
- SELECT the last N (e.g. 100) `duration_ms` values for a job, ORDER BY `started_at DESC LIMIT N`.
- Sort in Rust, pick `v[(len * 0.5) as usize]` and `v[(len * 0.95) as usize]` (nearest-rank, no interpolation — the UX gap between nearest-rank and interpolated percentiles is invisible at homelab sample sizes).
- Render into askama via a simple struct `{ p50_ms, p95_ms, sample_size }`.

**For the gantt timeline:**
- Single handler returning a `timeline.html` askama partial. Time-window selector (24h / 7d) is an HTMX swap on the same URL with a query param.
- Inline `<svg>` with one `<rect>` per run segment, grouped by job_id via nested `{% for %}`.
- Color via `fill="var(--cd-status-{{ seg.status }})"` — the CSS variables already exist in the Tailwind layer from v1.0 Phase 3.

## 8. Version Compatibility

No new cross-crate compatibility constraints introduced by v1.1. The existing v1.0 compatibility matrix (see `.planning/milestones/v1.0-research/STACK.md`) remains valid.

**One note for the rand bump:** `rand 0.9` is compatible with `rand_core 0.9`. No transitive dep in the current lockfile pins `rand` at 0.8 exclusively, so the bump is a single-line edit. Verify with `cargo tree -i rand` after the bump — if a second `rand 0.8` copy appears, that's fine for v1.1 scope.

## 9. Confidence Assessment

| Area | Confidence | Basis |
|------|------------|-------|
| No new runtime deps required | **HIGH** | Every target feature mapped to existing-dep implementation; code read directly where applicable (SSE handler, scheduler) |
| `bollard::kill_container` sufficient for stop-run | **HIGH** | API verified against docs.rs/bollard/0.20.2 |
| No upstream SSE crate churn needed | **HIGH** | Read the actual handler; axum 0.8.9 changelog confirms no SSE fixes; bugs are application-shape |
| Hand-roll SVG for gantt + sparkline | **HIGH** | Ecosystem survey: no small, server-only, no-JS Rust chart crate exists in 2026 that fits the constraints |
| SQLite lacks `percentile_cont` in stock builds | **HIGH** | Verified against sqlite.org/percentile.html — `-DSQLITE_ENABLE_PERCENTILE` required; not set by libsqlite3-sys |
| In-process percentile is the right answer | **HIGH** | Dataset size + structural parity priority both point this way |
| Migration pattern for per-job run numbers | **HIGH** | Standard `sqlx migrate` + `ROW_NUMBER()` window; dataset is thousands of rows, not millions |
| `rand 0.8 → 0.9` bump recommendation | **MEDIUM-HIGH** | No blocking CVE; judgment call on whether to land dep hygiene in v1.1. 0.9 is the less-risky target than 0.10. |
| `Cargo.toml` version bump to 1.1.0 on first commit | **HIGH** | Follows the locked "tag = Cargo.toml version at tag time" rule and PROJECT.md's iterative rc strategy |
| No security patches required | **HIGH** | Cross-referenced rustsec.org 2025-2026 advisories against the v1.0.1 dep set; no hits |

## 10. Open Questions (for roadmapper / phase planning)

1. **`cargo-deny` scope decision** — v1.1 or v1.2? Recommended for v1.1 as a non-blocking CI job, but the roadmapper can defer it cleanly.
2. **Gantt row grouping** — one `<svg>` per job row, or one big `<svg>` with grouped rows? Performance-equivalent at homelab scale; affects HTMX partial granularity (per-row refresh vs whole-timeline refresh). Design question for Phase 3-ish.
3. **Bulk enable/disable persistence model** — the config file is read-only, so "disabled" state has to live in the DB as a runtime override. Schema question, but no crate implications — flagged here only so the roadmapper knows it's a *pattern* decision, not a *stack* decision.
4. **In-process percentile sample size** — default N=100? N=500? Trivially configurable; not a stack decision.

None of these open questions block research handoff to the roadmapper.

## Sources

- **Cronduit `Cargo.toml`** (v1.0.1, read 2026-04-14) — current dep pins
- **Cronduit `Cargo.lock`** (v1.0.1, read 2026-04-14) — resolved versions verified
- **Cronduit `src/web/handlers/sse.rs`** (read 2026-04-14) — live SSE handler shape confirmed
- **Cronduit `src/scheduler/mod.rs`** (grep'd 2026-04-14) — JoinHandle/JoinSet presence confirmed
- **`.planning/milestones/v1.0-research/STACK.md`** — locked v1.0 stack baseline (not re-researched)
- **`.planning/PROJECT.md`** — v1.1 milestone scope, constraints, release strategy
- **docs.rs/bollard/0.20.2** — `Docker::kill_container` signature + `KillContainerOptions` — HIGH confidence
- **crates.io API 2026-04-14** — latest versions for tokio (1.52.0), axum (0.8.9), bollard (0.20.2), sqlx (0.8.6 stable), rand (0.10.1 / 0.9.4) — HIGH confidence
- **github.com/tokio-rs/axum CHANGELOG** — axum 0.8.9 has no SSE-related changes — HIGH confidence
- **sqlite.org/percentile.html** — `percentile_cont` gated by `SQLITE_ENABLE_PERCENTILE` compile flag since 3.51.0 — HIGH confidence
- **rustsec.org/advisories** — no 2026 advisories on cronduit's direct dep set — HIGH confidence
- **crates.io listings for `gantt_chart`, `plotters`, `charming`, `lodviz-rs`, `sparkline`, `embedded-graphics-sparklines`, `svg`** — confirmed each either (a) violates the no-JS/no-WASM constraint, (b) is a CLI binary not a library, or (c) is heavier than a hand-rolled askama template — HIGH confidence on each individual rejection

---
*Stack research for: Cronduit v1.1 "Operator Quality of Life"*
*Researched: 2026-04-14*
```

