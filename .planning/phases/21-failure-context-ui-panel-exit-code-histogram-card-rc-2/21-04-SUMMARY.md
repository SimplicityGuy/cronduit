---
phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
plan: 04
subsystem: ui
tags: [askama, fctx, view-model, run-detail, soft-fail, tracing]

# Dependency graph
requires:
  - phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
    plan: 02
    provides: "DbRunDetail.scheduled_for: Option<String>; insert_running_run widened with scheduled_for: Option<&str>"
  - phase: 16-failure-context-schema-run-rs-277-bug-fix
    plan: 01
    provides: "queries::get_failure_context + FailureContext struct (5 fields)"
  - phase: 13-observability-polish-rc-2
    plan: 04
    provides: "queries::get_recent_successful_durations + stats::percentile + format_duration_ms_floor_seconds"
provides:
  - "RunDetailView extended with image_digest, config_hash, scheduled_for, duration_ms (raw)"
  - "RunDetailPage extended with show_fctx_panel: bool, fctx: Option<FctxView>"
  - "FctxView struct (11 fields, pre-formatted view-model per research §H)"
  - "build_fctx_view async helper assembling FctxView per UI-SPEC § Copywriting Contract"
  - "FCTX gating in run_detail handler: status ∈ {failed, timeout} (excludes 'error' per landmine §11)"
  - "Soft-fail tracing::warn! on get_failure_context Err with locked field shape (target cronduit.web, structured fields, error = %e Display)"
  - "Helpers: truncate_hex (12-char digest truncation), format_relative_time (chrono-only, no new crate)"
  - "Foundation for plan 21-06 (askama template insert + CSS) — template substitutes {{ value }} with zero logic"
affects: [21-05, 21-06, 21-08]

# Tech tracking
tech-stack:
  added: []  # zero new external crates (D-32 invariant preserved)
  patterns:
    - "Pre-formatted view-model — askama template carries zero logic; every conditional copy variant assembled Rust-side per UI-SPEC § Copywriting Contract"
    - "Soft-fail with tracing::warn! — locked field shape from src/web/handlers/api.rs:127-132 (target cronduit.web, structured fields, error = %e Display); upgrade over the v1.1 OBS-03 dashboard-sparkline silent .unwrap_or_default() pattern per landmine §1"
    - "Don't short-circuit handler on degraded surface (landmine §12) — log fetch + page render proceed even when FCTX query fails; operators learn from /metrics + log warns"
    - "Job-type lookup soft-fail — get_job_by_id Err treated as non-docker (defensive hide of IMAGE DIGEST row) rather than 500-ing the page"

key-files:
  created: []
  modified:
    - src/web/handlers/run_detail.rs

key-decisions:
  - "Added duration_ms: Option<i64> to RunDetailView (Rule 3 auto-fix) — plan pseudo-code references run.duration_ms but RunDetailView only had duration_display: String. Mirrors the image_digest/config_hash/scheduled_for pass-through pattern from Task 1; harmless extension"
  - "build_fctx_view stub landed in Task 2 commit so the handler wire-up compiled before Task 3 fleshed out the body — keeps each task's commit individually buildable per atomic-commit discipline"
  - "Defensive hide of IMAGE DIGEST row when current run has no captured digest (avoids implying a change against an absent value); same pattern for CONFIG row when last_success_config_hash is NULL (D-13 never-succeeded)"
  - "Defensive p50==0 + cur_ms==0 branches in DURATION row factor computation — guards against divide-by-zero and infinite-factor display on edge cohorts"
  - "format_relative_time renders 'just now' for sub-minute durations + 'N days' for >24h (no week/month rollup) — bounded display fits the FCTX panel's single-line copy contract"

patterns-established:
  - "FctxView field-declaration order matches research §H verbatim — locks the struct surface so template authors in plan 21-06 can scan {{ field }} substitutions linearly without cross-referencing"
  - "Per-section comment header in build_fctx_view (1. Streak summary / 2. Job-type lookup / 3. last_success_run_url / etc.) so each UI-SPEC § Copywriting Contract row maps to a numbered code block — future visual-deviation amendments are easy to locate"
  - "Const-named thresholds (FCTX_MIN_DURATION_SAMPLES, FCTX_DURATION_SAMPLE_LIMIT, FCTX_DIGEST_TRUNCATE_LEN) — call out the FCTX-05 N>=5 distinction from the v1.1 OBS-04 N>=20 Duration card and lock the 12-char digest truncation per UI-SPEC"

requirements-completed: [FCTX-01, FCTX-02, FCTX-03, FCTX-05, FCTX-06]

# Metrics
duration: ~10min
completed: 2026-05-02
---

# Phase 21 Plan 04: FCTX panel data flow wire-up Summary

**run_detail handler now gates `get_failure_context()` to status ∈ {failed, timeout}, soft-fails with the canonical tracing::warn! shape on DB error, and assembles a fully pre-formatted 11-field FctxView per UI-SPEC § Copywriting Contract — plan 21-06's askama template insert can substitute `{{ value }}` with zero logic.**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-05-02T20:18:34Z
- **Completed:** 2026-05-02T20:28:47Z
- **Tasks:** 3 (all atomic-committed)
- **Files modified:** 1 (`src/web/handlers/run_detail.rs`)

## Accomplishments
- `RunDetailView` extended with `image_digest`, `config_hash`, `scheduled_for`, `duration_ms` (raw) — pass-through from `DbRunDetail` (Phase 16 + Phase 21-02 fields)
- `RunDetailPage` extended with `show_fctx_panel: bool` + `fctx: Option<FctxView>` — both gated `#[allow(dead_code)]` until plan 21-06 lands the template
- `FctxView` struct shipped with all 11 locked fields per research §H — pre-formatted view-model so the template carries zero logic
- Handler gating: `matches!(run_view.status.as_str(), "failed" | "timeout")` per FCTX-01; `error` status intentionally excluded per landmine §11
- Soft-fail `tracing::warn!` with the locked field shape from `src/web/handlers/api.rs:127-132` verbatim (`target: "cronduit.web"`, structured `job_id`/`run_id`/`error = %e` Display fields, final message string) per D-12 + landmine §1 — explicit upgrade over the v1.1 OBS-03 dashboard-sparkline silent `.unwrap_or_default()` pattern
- Handler does NOT short-circuit on FCTX failure (landmine §12) — log fetch + page render still proceed
- `build_fctx_view` async helper assembles all 11 FctxView fields per UI-SPEC § Copywriting Contract
- All locked copy strings present:
  - `"{N} consecutive failures"` / `"1 failure (no streak)"` (summary meta)
  - `"First failure: {rel} ago • {N} consecutive failures"` (with `last_success_run_url` for prior-success path) / `"... • No prior successful run"` (D-13 never-succeeded suffix)
  - `"{old_12hex}… → {new_12hex}…"` (12-char locked truncation per UI-SPEC) / `"unchanged"`
  - `"Config changed since last success: Yes"` / `"Config changed since last success: No"` (D-14 literal compare)
  - `"{this}; typical p50 is {p50} ({factor}× longer/shorter than usual)"` (FCTX-05 N>=5)
  - `"Scheduled: {hh:mm:ss} • Started: {hh:mm:ss} (+{skew} ms)"` (D-04 NULL-hides)
- Helpers added inline (no new crate): `truncate_hex` (12-char digest truncation per UI-SPEC), `format_relative_time` (chrono-only `just now` / `{N} minutes` / `{N} hours` / `{N} days` rendering)
- `cargo build --workspace` green
- `cargo nextest run --no-fail-fast` 528 passed / 9 failed (all 9 = pre-existing `SocketNotFoundError("/var/run/docker.sock")` sandbox-Docker testcontainer issues, identical to plan 21-02 wave-end gate; not regressions)
- `cargo tree -i openssl-sys` empty (D-32 rustls invariant holds)

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend RunDetailView + RunDetailPage; add FctxView struct** — `5e53ade` (feat)
2. **Task 2: Wire data fetch + soft-fail + FctxView assembly in run_detail handler** — `94b0325` (feat)
3. **Task 3: Implement build_fctx_view per UI-SPEC § Copywriting Contract** — `31c0b07` (feat)

**Plan metadata:** _added in the final docs commit at SUMMARY-write time_

## Files Created/Modified

**Production source (1):**
- `src/web/handlers/run_detail.rs` — extended `RunDetailView` with 4 new fields (`image_digest`, `config_hash`, `scheduled_for`, `duration_ms`); extended `RunDetailPage` with 2 new fields (`show_fctx_panel`, `fctx`); added `FctxView` struct (11 fields per research §H); added `build_fctx_view` async helper (~150 lines, all 11 fields populated per UI-SPEC § Copywriting Contract); added `truncate_hex` + `format_relative_time` helpers; wired `get_failure_context()` gating + soft-fail `tracing::warn!` in the `run_detail` handler.

## Decisions Made

- **`build_fctx_view` stub in Task 2 commit, full body in Task 3 commit** — Task 2's wire-up references `build_fctx_view` so the file would not compile without an implementation. Shipping a stub-shaped helper in Task 2 (with a clear "plan 21-04 task 3 fills these in" doc comment) keeps each task's commit individually buildable, which atomic-commit discipline requires. Task 3 then replaces the body without touching the call site.
- **Added `duration_ms: Option<i64>` to `RunDetailView` (Rule 3 auto-fix)** — the plan's pseudo-code in Task 3 references `run.duration_ms` directly, but `RunDetailView` only carried `duration_display: String` (the pre-formatted version for the metadata card). The DURATION row needs the raw value to compute `current / p50` factor. Adding a `duration_ms: Option<i64>` pass-through from `DbRunDetail` mirrors the existing pattern of passing `image_digest`/`config_hash`/`scheduled_for` from the same source struct; harmless additive change.
- **Defensive hide of IMAGE DIGEST row when current run has no digest** — when `is_docker_job=true`, `last_success_image_digest=Some(_)`, but `run.image_digest=None`, the assembler renders `"unchanged"` rather than implying a change against an absent baseline. The "current digest is None for a docker job" path is unusual but possible (e.g., a docker job whose `inspect_container` failed mid-startup); rendering "unchanged" is the safer default since the operator already sees "this run failed" + the digest column is informational.
- **Defensive `p50==0` + `cur_ms==0` branches in DURATION row factor computation** — guards against divide-by-zero and infinite-factor display on edge cohorts. p50==0 forces a `1.0× longer` display (factor visible but not infinite); cur_ms==0 forces `1.0× shorter`.
- **`format_relative_time` rolls up at "{N} days" for >24h** — no week/month rollup. The FCTX panel TIME DELTAS row is single-line per UI-SPEC and operators looking at a recent failure want fine-grained recency (minutes/hours), not "3 weeks ago". A 30-day-old failure rendering "30 days ago" is acceptable; the gating expects most consumers see streaks of recent failures.
- **`use crate::db::queries::FailureContext;` import re-exported the helper struct** — `queries::FailureContext` is imported via the existing `use crate::db::queries;` line, and adding the explicit `use crate::db::queries::FailureContext;` makes the `build_fctx_view` signature read cleaner without `queries::` prefix everywhere.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] `RunDetailView.duration_ms` field missing**

- **Found during:** Task 3 (writing `build_fctx_view`)
- **Issue:** The plan's pseudo-code uses `run.duration_ms.unwrap_or(0)` to compute the FCTX-05 DURATION row factor, but `RunDetailView` only carried `duration_display: String` (pre-formatted by `format_duration_ms`). Without the raw millisecond value the factor cannot be computed Rust-side.
- **Fix:** Added `pub duration_ms: Option<i64>` to `RunDetailView` immediately after `duration_display`; populated it from `run.duration_ms` (the existing field on `DbRunDetail`) in the `RunDetailView { ... }` constructor in the `run_detail` handler. Pure pass-through; mirrors the image_digest/config_hash/scheduled_for additions in Task 1.
- **Files modified:** `src/web/handlers/run_detail.rs`
- **Verification:** `cargo build --workspace` green after the addition; `build_fctx_view` reads `run.duration_ms` without further plumbing.
- **Committed in:** `31c0b07` (Task 3 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking — Rule 3 plumbing the plan understated)
**Impact on plan:** The deviation is mechanical: a pass-through field that the plan's own pseudo-code requires. No scope creep, no behavioral change, no logic change. Production semantics align exactly with the plan's intent.

## Issues Encountered

- **Postgres testcontainer tests cannot run in this sandbox:** the same 9 tests that failed at plan 21-02's wave-end gate (`dashboard_jobs_pg`, `db_pool_postgres`, `schema_parity::sqlite_and_postgres_schemas_match_structurally`, all `v11_bulk_toggle_pg::*`, `v13_timeline_explain::explain_uses_index_postgres`) fail again here with `Client(Init(SocketNotFoundError("/var/run/docker.sock")))`. They require `testcontainers-modules::postgres::Postgres` which spins up a Postgres container via the host Docker daemon — the sandbox has no Docker daemon. All other 528 tests pass. Postgres parity verifies on CI where Docker is available.

## User Setup Required

None — pure handler-side wire-up. No new env vars, no config changes, no operator-visible surface yet (the askama template insert + CSS additions land in plan 21-06; operators see no UI change from this plan alone).

## Next Phase Readiness

- **Plan 21-05 (job_detail handler wire-up + ExitHistogramCardContext)** runs in parallel with this plan in Wave 2; no cross-dependency on the run_detail handler. The exit-buckets module + `get_recent_runs_for_histogram` query helper from plan 21-03 are the only inputs plan 21-05 needs.
- **Plan 21-06 (askama template insert + CSS)** can now substitute `{{ run.image_digest }}`, `{{ run.config_hash }}`, `{{ run.scheduled_for }}`, `{{ show_fctx_panel }}`, and `{{ fctx.* }}` (all 11 fields). The template carries zero logic per UI-SPEC § Copywriting Contract — every conditional copy variant is already pre-rendered in `time_deltas_value` / `image_digest_value` / `config_changed_value` / `duration_value` / `fire_skew_value`. Hide-row gating uses the `Option<...>` shape: `{% if let Some(value) = fctx.image_digest_value %}` renders the row, otherwise the row hides. The `last_success_run_url` field carries the `<a href="">` URL when populated; the template wraps the literal `[view last successful run]` link text per UI-SPEC § Copywriting Contract.
- **Plan 21-08 (integration tests)** can simulate the gating + soft-fail paths via the existing test fixtures: `seed_run_with_status("failed")` + `seed_run_with_status("success")` to populate a FailureContext with a prior success; `seed_run_with_status("failed")` only to exercise the D-13 never-succeeded path; injecting a Postgres connection error to confirm soft-fail emits the warn line without 500-ing the handler.

## Threat Flags

None — the new code is read-only and emits no new operator-visible surface yet (template + CSS land in plan 21-06). The `tracing::warn!` shape adds no PII / secrets — only `job_id`, `run_id`, and `%e` Display formatting of the sqlx error. The FCTX panel's threat surface (T-21-04-01 image_digest/config_hash render → askama auto-escape mitigates; T-21-04-02 tracing::warn! → fields are server-typed, no payload; T-21-04-03 get_recent_successful_durations DoS → existing P13 query with bounded LIMIT 100, accept) all remain valid as written. ASVS V5 input-validation: every FctxView field is Rust-owned; askama auto-escapes `{{ value }}` (no `|safe`) when the template lands in plan 21-06.

## Self-Check: PASSED

- Commit `5e53ade` (Task 1) — FOUND in `git log`
- Commit `94b0325` (Task 2) — FOUND in `git log`
- Commit `31c0b07` (Task 3) — FOUND in `git log`
- `src/web/handlers/run_detail.rs` exists; modifications confirmed via `git show --stat`
- `pub struct FctxView` count = 1
- 11 FctxView field declarations match research §H exactly (consecutive_failures, summary_meta, last_success_run_id, time_deltas_value, last_success_run_url, is_docker_job, image_digest_value, config_changed_value, has_duration_samples, duration_value, fire_skew_value)
- `RunDetailView` extended with `image_digest`, `config_hash`, `scheduled_for`, `duration_ms`
- `RunDetailPage` extended with `show_fctx_panel: bool`, `fctx: Option<FctxView>`
- Gating expression `matches!(run_view.status.as_str(), "failed" | "timeout")` present (excludes "error" per landmine §11)
- Soft-fail `tracing::warn!` includes `target: "cronduit.web"`, `job_id = run_view.job_id`, `run_id = run_view.id`, `error = %e`, message `"fctx panel: get_failure_context failed — hiding panel"`
- Locked copy strings present: `"Config changed since last success: Yes"`, `"Config changed since last success: No"`, `"unchanged"` (used in match arms), `"No prior successful run"` (in format string), `"1 failure (no streak)"`
- `FCTX_MIN_DURATION_SAMPLES = 5` const (NOT 20) per FCTX-05
- `FCTX_DIGEST_TRUNCATE_LEN = 12` const per UI-SPEC
- `cargo build --workspace` exits 0
- `cargo nextest run --no-fail-fast` — 528 passed, 9 failed (all 9 = `SocketNotFoundError("/var/run/docker.sock")`; sandbox limitation; verified by `grep "SocketNotFound"`)
- `cargo tree -i openssl-sys` returns "package ID specification ... did not match any packages" (D-32 invariant)

---
*Phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2*
*Plan: 04*
*Completed: 2026-05-02*
