---
phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship
plan: 04
subsystem: web
tags: [rust, axum, axum-extra, htmx, csrf, phase-14, wave-3, erg-01, erg-02]

requires:
  - phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship
    plan: 03
    provides: "queries::bulk_set_override + queries::get_overridden_jobs + tri-state filter on get_enabled_jobs"
provides:
  - "src/web/handlers/api.rs::bulk_toggle — CSRF-gated POST handler that sets enabled_override for a list of job ids; fires SchedulerCmd::Reload AFTER DB UPDATE; returns 200 + HX-Trigger toast"
  - "src/web/handlers/api.rs::BulkToggleForm — serde Deserialize struct (csrf_token, action, #[serde(default)] job_ids: Vec<i64>) for axum_extra::extract::Form"
  - "src/web/handlers/api.rs::error_bulk_toast + build_bulk_toast_message — private helpers that compose UI-SPEC Copywriting Contract toast strings"
  - "src/web/mod.rs route: POST /api/jobs/bulk-toggle → handlers::api::bulk_toggle"
  - "Cargo.toml axum-extra now enables the 'form' feature; Cargo.toml axum now enables the 'macros' feature"
affects: [14-05, 14-06]

tech-stack:
  added: []
  patterns:
    - "CSRF-first mutation handler — copy pattern from stop_run + reload; CSRF runs BEFORE any DB work (Landmine §5)"
    - "axum_extra::extract::Form (NOT stock axum::Form) for Vec<T> from repeated form keys — serde_html_form backend handles `job_ids=1&job_ids=2&...` correctly (Landmine §1)"
    - "#[serde(default)] on Vec field so empty-selection POSTs reach the handler's user-friendly toast path instead of opaque axum extractor rejection (Landmine §9)"
    - "#[axum::debug_handler] mandatory on mixed FromRequestParts + FromRequest extractor bundles (Landmine §8)"
    - "Extractor order State → CookieJar → ExtraForm enforced by axum 0.8 (body-consuming extractor last)"
    - "BTreeSet<i64> dedupes before SQL — rows_affected reflects unique ids only (D-12a)"
    - "Reload-after-DB ordering — `state.cmd_tx.send(Reload).await` fires AFTER `queries::bulk_set_override(...).await` returns; reload result intentionally NOT awaited (Landmine §6)"
    - "503 (not 500) on mpsc send failure — channel-closed indicates scheduler is shutting down (T-14-04-05)"
    - "Sticky error toasts (`duration: 0`) for every early-return branch via the error_bulk_toast helper"

key-files:
  created: []
  modified:
    - "Cargo.toml"
    - "Cargo.lock"
    - "src/web/handlers/api.rs"
    - "src/web/mod.rs"

key-decisions:
  - "Followed the plan's verbatim handler skeleton from 14-RESEARCH.md §axum 0.8 Form Handler Pattern + 14-PATTERNS.md §1/§8. Eight numbered steps in the handler body trace 1:1 to the plan's behavior table."
  - "Rule 3 auto-fix to Cargo.toml axum features: added `\"macros\"`. The plan mandates `#[axum::debug_handler]` (Landmine §8), but axum 0.8.9 with `default-features = false` does NOT include the macros feature by default. The attribute is re-exported from axum_macros and gated behind `\"macros\"`. Without this fix the lib does not compile."
  - "Used `Some(1) => format!(\"{} {}: override set.\", ...)` reserved branch in build_bulk_toast_message even though v1.1 UI never writes override=1. Defensive completeness; the schema allows it (D-13a tri-state)."
  - "Composed the running-job clause WITHOUT the dual-`will`-variable noise from the plan skeleton — both singular and plural use `will` (the verb does not change with number). Removed the dead `running_verb` binding so clippy stays clean."
  - "Did NOT use `HX-Refresh: true` on the success path — diverges from stop_run/reload/reroll. CONTEXT D-12b says the 3s dashboard poll picks up DB state on next cycle; HX-Refresh would force an unnecessary extra full-page reload."

requirements-completed: []  # ERG-01 and ERG-02 cannot flip green until Plan 14-05 (dashboard UI bar) and Plan 14-06 (settings Clear buttons) wire the operator-facing surfaces

duration: ~30 min
completed: 2026-04-22
---

# Phase 14 Plan 04: Wave 3 Bulk-Toggle Handler Summary

**`POST /api/jobs/bulk-toggle` CSRF-gated handler lands; uses `axum_extra::extract::Form<BulkToggleForm>` (NOT stock `axum::Form`); CSRF runs BEFORE DB work; `SchedulerCmd::Reload` fires AFTER DB UPDATE commits; toast strings match UI-SPEC Copywriting Contract verbatim.**

## Performance

- **Duration:** ~30 min
- **Started:** 2026-04-22T22:09:00Z
- **Completed:** 2026-04-22T22:39:23Z
- **Tasks:** 2 / 2 (plus the planned Cargo.toml `"form"` feature add — separate atomic commit per plan instructions)
- **Files modified:** 4 (Cargo.toml, Cargo.lock, src/web/handlers/api.rs, src/web/mod.rs)

## Handler Sequence Diagram

```mermaid
flowchart TD
    A[POST /api/jobs/bulk-toggle] --> B{CSRF cookie<br/>== form token?}
    B -- no --> B1[403 + sticky error toast<br/>Session expired — refresh and try again.]
    B -- yes --> C{action ==<br/>'disable' or 'enable'?}
    C -- no --> C1[400 + sticky error toast<br/>Invalid bulk action.]
    C -- yes --> D{job_ids<br/>empty?}
    D -- yes --> D1[400 + sticky error toast<br/>No jobs selected.]
    D -- no --> E[Dedupe via BTreeSet i64<br/>D-12a]
    E --> F[Snapshot running_count from<br/>state.active_runs RwLock<br/>Research Open Q2]
    F --> G[queries::bulk_set_override<br/>UPDATE jobs SET enabled_override = ?<br/>WHERE id IN ...]
    G -- DB error --> G1[500 + sticky error toast<br/>Database error.<br/>+ tracing::error log]
    G -- Ok rows_affected --> H[state.cmd_tx.send<br/>SchedulerCmd::Reload<br/>AFTER DB commit]
    H -- channel closed --> H1[503 + sticky error toast<br/>Scheduler is shutting down — try again shortly.]
    H -- ok --> I[drop resp_rx<br/>do NOT await reload result]
    I --> J[Compose toast per<br/>UI-SPEC Copywriting Contract]
    J --> K[200 + HX-Trigger showToast<br/>level=info]

    classDef ok fill:#0a3d0a,stroke:#00ff7f,stroke-width:2px,color:#e0ffe0
    classDef err fill:#3d0a0a,stroke:#ff7f7f,stroke-width:2px,color:#ffe0e0
    classDef step fill:#1a1a3d,stroke:#7f7fff,stroke-width:1px,color:#e0e0ff
    class K ok
    class B1,C1,D1,G1,H1 err
    class A,B,C,D,E,F,G,H,I,J step
```

## Accomplishments

- **`bulk_toggle` handler implemented** — 116 LOC including doc comment, plus 14 LOC for `error_bulk_toast` helper and 30 LOC for `build_bulk_toast_message` helper. Eight numbered steps trace 1:1 to the plan's behavior table.
- **`BulkToggleForm` struct** added next to `CsrfForm` with the load-bearing `#[serde(default)]` on `job_ids: Vec<i64>` (Landmine §9).
- **Route registered** at `/api/jobs/bulk-toggle` immediately after `stop_run` in src/web/mod.rs — preserves the api-cluster ordering convention.
- **Cargo.toml** updated twice in two atomic commits: `axum-extra` gains `"form"` (planned landmine flagged by Plan 14-01); `axum` gains `"macros"` (Rule 3 auto-fix for `#[axum::debug_handler]`).
- **Toast strings match UI-SPEC Copywriting Contract verbatim:**
  - `"{N} jobs disabled."` / `"1 job disabled."` (singular)
  - `"{N} jobs disabled. {M} currently-running job(s) will complete naturally."` (running-clause on disable only)
  - `"{N} jobs disabled. ({K} not found)"` (D-12 suffix; running-clause precedes when both present)
  - `"{N} jobs: override cleared."` / `"1 job: override cleared."` (singular)
  - `"Session expired — refresh and try again."` (sticky)
  - `"Invalid bulk action."` (sticky)
  - `"No jobs selected."` (sticky)
  - `"Scheduler is shutting down — try again shortly."` (sticky)
  - `"Database error."` (sticky; full error stays in tracing log per T-14-04-09)
- **All wave-3 critical invariants honored:**
  - CSRF validated BEFORE any DB work (Landmine §5)
  - axum_extra::Form (NOT axum::Form) so repeated `job_ids=` keys deserialize (Landmine §1)
  - `#[serde(default)]` on Vec so empty-list reaches handler toast path (Landmine §9)
  - Extractor order: `State` → `CookieJar` → `ExtraForm` (axum 0.8 trait bound)
  - `#[axum::debug_handler]` present (Landmine §8)
  - `BTreeSet<i64>` dedupe before SQL (D-12a)
  - Reload sent AFTER DB UPDATE commits (Landmine §6); reload result NOT awaited
  - 503 (not 500) on mpsc send failure (T-14-04-05)
- **Test scoreboard delta** (SQLite `tests/v11_bulk_toggle.rs`):
  - Before Plan 04: 3 errors (`E0432: bulk_toggle` + `E0432: OverriddenJobView` + `E0603: SettingsPage private`)
  - After Plan 04: 2 errors (the 2 Plan-06 deliverables)
  - Plan 04 cleared the `E0432: bulk_toggle` import as predicted by the wave plan

## Task Commits

Each task was committed atomically with `--no-verify`:

1. **Pre-Task: Cargo.toml `axum-extra` `"form"` feature** — `8bf5515` (feat)
2. **Task 1: bulk_toggle handler + BulkToggleForm + helpers (+ Rule-3 axum `"macros"` feature add)** — `fd9f0cf` (feat)
3. **Task 2: Route registration in src/web/mod.rs** — `75f8f98` (feat)

_Plan metadata commit (this SUMMARY.md) follows._

## Files Modified

- `Cargo.toml` — +5 lines / -2 lines (added `"form"` to axum-extra; added `"macros"` to axum)
- `Cargo.lock` — refreshed for the new feature pulls
- `src/web/handlers/api.rs` — +220 lines net (BulkToggleForm + bulk_toggle + error_bulk_toast + build_bulk_toast_message + 2 import lines)
- `src/web/mod.rs` — +4 lines (route registration after stop_run)

## Verification

### Plan-mandated commands

```
$ cargo build --quiet
exit=0

$ cargo clippy --quiet -- -D warnings
exit=0

$ cargo nextest run --lib
194 tests run: 194 passed, 0 skipped

$ cargo nextest run --test stop_handler
4 tests run: 4 passed, 0 skipped
(verifies the most-similar existing handler still works)

$ cargo nextest run --test v11_bulk_toggle_pg
5 tests run: 5 passed, 0 skipped
(Plan 03's Postgres parity tests for bulk_set_override + get_overridden_jobs + dashboard_filter)
```

### v11_bulk_toggle compile state (wave-ordering reality)

```
$ cargo test --test v11_bulk_toggle --no-run 2>&1 | grep -E "^error\[" | sort | uniq -c
   1 error[E0432]: unresolved import `cronduit::web::handlers::settings::OverriddenJobView`
   1 error[E0603]: struct `SettingsPage` is private
```

The 2 remaining errors both belong to Plan 14-06 (Settings page). The `bulk_toggle` import (Plan 04 deliverable) now resolves cleanly. The 8 handler-tier tests this plan targets — `handler_csrf`, `handler_disable`, `handler_enable`, `handler_partial_invalid`, `handler_partial_invalid_toast_uses_rows_affected`, `handler_dedupes_ids`, `handler_rejects_empty`, `handler_accepts_repeated_job_ids`, `handler_fires_reload_after_update` — will turn green as soon as Plan 06 lands the remaining imports.

This is the same wave-ordering reality Plan 03 documented (it could only verify its DB-layer changes via the Postgres twin file `tests/v11_bulk_toggle_pg.rs` for the same reason).

### Acceptance-criteria grep checks (all PASS)

```
OK: pub async fn bulk_toggle
OK: ExtraForm import
axum::Form raw imports (must be 0): 0
OK: debug_handler attr
OK: BulkToggleForm struct
OK: serde default
OK: BTreeSet dedupe
OK: SchedulerCmd::Reload
OK: 503
OK: shutdown msg
OK: no jobs msg
OK: invalid action msg
OK: session expired msg
OK: route registered
Route count: 1
```

Note on the route grep: the route registration was broken across multiple lines for readability:

```rust
.route(
    "/api/jobs/bulk-toggle",
    post(handlers::api::bulk_toggle),
)
```

The stricter "single-line" pattern from the plan's acceptance criterion does not match this form, so verification falls back to the two-grep proof: the URL string appears once AND `post(handlers::api::bulk_toggle)` appears.

### Handler ordering verification (CRITICAL)

```
$ awk '/pub async fn bulk_toggle/,/^}$/' src/web/handlers/api.rs \
    | grep -n 'bulk_set_override\|SchedulerCmd::Reload'
50:    // 6. DB UPDATE (awaited) — bulk_set_override is the writer-pool helper
54:    let updated = match queries::bulk_set_override(&state.pool, &ids, new_override).await {
76:        .send(SchedulerCmd::Reload {
```

`bulk_set_override` at handler-line 54 < `SchedulerCmd::Reload` at handler-line 76. The DB UPDATE awaits before the reload command is sent. Landmine §6 / T-14-04-04 satisfied.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Add `"macros"` feature to axum in Cargo.toml**

- **Found during:** Task 1 verification (`cargo build --quiet`)
- **Issue:** `#[axum::debug_handler]` failed to resolve with `error[E0433]: failed to resolve: could not find 'debug_handler' in 'axum'`. The compiler note explained: `the item is gated behind the 'macros' feature`. The plan mandates `#[axum::debug_handler]` (action step 4 + Landmine §8 — readable extractor-order errors). Cargo.toml has `axum = { ..., default-features = false, features = ["tokio", "http1", "http2", "json", "query", "form"] }` — `"macros"` is NOT in the explicit feature list and `default-features = false` excludes it.
- **Why latent:** No prior handler in the codebase uses `#[axum::debug_handler]` (verified via `grep -r "debug_handler" src/`). Plan 14-01 flagged the axum-extra `"form"` feature gap but missed this one because `axum::debug_handler` is a NEW requirement introduced by this plan (mixed-extractor handler is the first of its kind in this codebase).
- **Fix:** Add `"macros"` to the axum feature list. Mirrors how `"form"` was added in the prior commit.
- **Files modified:** Cargo.toml (axum dependency line)
- **Commit:** Folded into `fd9f0cf` (Task 1 handler commit) since the lib does not compile without it — splitting it out would have left an intermediate broken state on the branch.

### Adaptations

**2. [Rule N/A — design polish within plan latitude] Removed dead `running_verb` binding from build_bulk_toast_message**

- **Found during:** Task 1 implementation
- **Issue:** Plan skeleton has:
  ```rust
  let running_verb = if running_count == 1 { "will" } else { "will" }; // both "will"
  ```
  Both branches return `"will"`, making the `if/else` and the third format argument dead. Clippy under `-D warnings` would reject this as `clippy::if_same_then_else`.
- **Resolution:** Hard-coded the verb in the format string (since UI-SPEC also confirms both forms use `"will"`):
  ```rust
  msg.push_str(&format!(" {} currently-running {} will complete naturally.", running_count, running_noun));
  ```
  Output strings are byte-identical to the UI-SPEC Copywriting Contract rows.
- **Files affected:** src/web/handlers/api.rs (build_bulk_toast_message)
- **Commit:** fd9f0cf

**3. [Rule N/A — clippy compliance] Doc-comment list-item indentation**

- **Found during:** Task 1 verification (`cargo clippy`)
- **Issue:** Initial pass aligned doc-comment continuation lines with the field-name column (visual table style). Clippy 1.94's `clippy::doc_overindented_list_items` rejects this and demands 2-space indentation under list items.
- **Resolution:** Reformatted the `build_bulk_toast_message` doc list to use 2-space continuation indent. Content unchanged.
- **Files affected:** src/web/handlers/api.rs
- **Commit:** fd9f0cf (folded into the same Task 1 commit since it was a within-commit clippy iteration)

**4. [Rule N/A — wave-ordering reality] SQLite tests/v11_bulk_toggle.rs cannot compile end-to-end yet**

- **Found during:** Plan-end verification
- **Issue:** The test file imports `cronduit::web::handlers::settings::{OverriddenJobView, SettingsPage}`. `OverriddenJobView` does not yet exist; `SettingsPage` is still private (declared `struct SettingsPage` not `pub struct`). Both are Plan 14-06 deliverables. The 8 handler-tier tests this plan targets cannot run until Plan 06 lands.
- **Why expected:** This is wave-ordering reality, not a defect. Plan 03's SUMMARY documented the same situation (Plan 03 verified its DB-layer changes via the Postgres twin file `tests/v11_bulk_toggle_pg.rs` for the same reason). Plan 14-04 verifies via the `bulk_toggle` import resolving + lib tests + Postgres parity tests + the existing analog handler tests (`stop_handler`).
- **Files affected:** none in this plan (Plan 06 owns)
- **Commit:** N/A — documented in this SUMMARY's Notes for Plan 06.

### Deferred Items

None — every task within budget. The 4 deviations above were all necessary to verify the plan's own success criteria and complete the handler.

### Out-of-Scope Discoveries

The /dev/disk3s1s1 partition reached 100% during the SUMMARY.md write step, requiring a `cargo clean` to free 6.2 GiB. Same environment hygiene issue Plans 02 and 03 encountered. Recovered without affecting verification outcomes.

## Authentication Gates

None encountered. Build/clippy/tests ran without prompting for credentials.

## Self-Check: PASSED

**Files exist and contain expected content:**
```
$ grep -n "pub async fn bulk_toggle" src/web/handlers/api.rs
516:pub async fn bulk_toggle(
$ grep -n "/api/jobs/bulk-toggle" src/web/mod.rs
83:            "/api/jobs/bulk-toggle",
$ grep -n "axum-extra.*form" Cargo.toml
105:axum-extra = { version = "0.12", features = ["cookie", "form", "query"] }
$ grep -n "axum.*macros" Cargo.toml
25:axum = { version = "0.8.9", default-features = false, features = ["tokio", "http1", "http2", "json", "macros", "query", "form"] }
```

**Commits exist:**
```
$ git log --oneline | grep -E "(8bf5515|fd9f0cf|75f8f98)"
75f8f98 feat(14-04): wire POST /api/jobs/bulk-toggle route in web router
fd9f0cf feat(14-04): add bulk_toggle handler + BulkToggleForm (ERG-01 / ERG-02)
8bf5515 feat(14-04): enable axum-extra "form" feature for serde_html_form-backed Form
```

**T-V11-BULK-01 freeze (Plan-04 does not touch queries.rs):**
```
$ git diff f2b332e..HEAD -- src/db/queries.rs
(no output — Plan 04 did not modify queries.rs; upsert_job source-level lock preserved by inheritance)
```

**Test results:**
- 194 lib unit tests: PASSED (zero new failures, zero skipped)
- 4 stop_handler integration tests: PASSED (most-similar analog still works)
- 5 v11_bulk_toggle_pg Postgres parity tests: PASSED (Plan 03 query helpers still green)
- v11_bulk_toggle SQLite: 2 expected wave-ordering errors (Plan 06 deliverables)

All Plan-04 acceptance criteria from `<acceptance_criteria>` blocks satisfied.

## Threat Flags

None — Plan 04 introduces only:

- One CSRF-gated mutation endpoint that follows the project's established double-submit pattern (cookie + form token, constant-time compare via `csrf::validate_csrf`). Same shape as `run_now`, `reload`, `reroll`, `stop_run`. T-14-04-01 mitigated.
- One serde-Deserialize struct that uses `axum_extra::extract::Form` (sqlx parameter binding inherited transitively from Plan 03's `bulk_set_override`). T-14-04-02 mitigated.
- Generic `"Database error."` toast string + full error logged at `tracing::error!` level — no DB error detail leaked to the operator. T-14-04-09 mitigated.
- No new file-access patterns, no new schema reads at trust boundaries.

The threat register's T-14-04-01 through T-14-04-09 dispositions all mark `mitigate`; every mitigation lands in this plan as documented in the handler doc comment and the build_bulk_toast_message helper. No new threat surface beyond what the plan's `<threat_model>` enumerated.

## Known Stubs

None — Plan 04 introduces:

- A fully-functioning HTTP handler with all 4 error paths wired (403 CSRF, 400 invalid action, 400 empty list, 503 channel closed, 500 DB error)
- A toast-composition helper that exhaustively handles the 5 UI-SPEC Copywriting scenarios (singular/plural × disable/enable × running-job × not-found suffix)
- A route registration that goes live the moment the binary starts

The handler is operational today — it can be exercised via `curl` against a running server. The reason the operator-facing buttons aren't usable yet is that the dashboard checkbox UI (Plan 14-05) and the Settings Clear buttons (Plan 14-06) aren't built yet. Those plans wire the front-end surfaces; the back-end is complete.

The reserved `Some(_)` arm in `build_bulk_toast_message` (handles the unused override=1 case with `"override set."` text) is NOT a stub — it's defensive completeness against a future plan adding Force-On semantics.

## Notes for Plans 05 + 06

- **Plan 05 (dashboard UI bar):** the endpoint is live at `POST /api/jobs/bulk-toggle`. Form body shape (URL-encoded):
  ```
  csrf_token=<token>&action=disable&job_ids=1&job_ids=2&job_ids=3
  ```
  The HTMX form needs a hidden `csrf_token` input + an `action` input that flips between `"disable"` and `"enable"` based on which button is pressed + repeated `job_ids` checkbox inputs (one per ticked row). The bulk-bar `<form>` should `hx-post="/api/jobs/bulk-toggle"` and `hx-swap="none"` (handler returns empty body; toast fires via HX-Trigger; dashboard's existing 3s poll picks up the DB state on the next cycle).
- **Plan 05/06:** the handler does NOT emit `HX-Refresh: true`. This diverges from `stop_run`/`reload`/`reroll`. The 3s dashboard poll picks up DB state on the next cycle (CONTEXT D-12b). If a synchronous refresh is needed for some flow, the front-end can call `htmx.trigger(document.body, 'refreshDashboard')` (via the existing `hx-trigger="every 3s, refreshDashboard from:body"` on the job-table partial) — no handler change required.
- **Plan 06 (settings page):** the per-row Clear button posts to the same endpoint with `action=enable&job_ids=<single id>`. The toast then renders `"1 job: override cleared."` per the singular rule. No new endpoint needed.
- **Plan 06 (settings page):** the test file `tests/v11_bulk_toggle.rs` will compile and the 8 handler tests will turn green as soon as you make `SettingsPage` `pub` and add `pub struct OverriddenJobView { pub id: i64, pub name: String, pub enabled_override: i64 }`. The `settings_empty_state_hides_section` test additionally requires the askama template to wrap the "Currently Overridden" section in `{% if !overridden_jobs.is_empty() %}`.
- **For all subsequent plans:** the `axum` and `axum-extra` Cargo.toml feature additions are now permanent. Future handlers can use `#[axum::debug_handler]` and `axum_extra::extract::Form` without further Cargo.toml changes.
