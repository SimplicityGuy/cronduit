---
phase: 11-per-job-run-numbers-log-ux-fixes
verified: 2026-04-17T20:09:23Z
status: passed
score: 5/5 must-haves verified
overrides_applied: 0
requirements_verified: 10/10
tests_passed: 317
tests_failed: 0
tests_ignored: 20
clippy: clean
fmt: clean
schema_parity: green
---

# Phase 11: Per-Job Run Numbers + Log UX Fixes — Verification Report

**Phase Goal:** Run history shows per-job numbering (`#1, #2, ..., per job`) instead of global IDs; existing rows are backfilled on upgrade; the run-detail page shows accumulated log lines on load, then attaches live SSE with no gap, no duplicates, and no transient "error getting logs" flash.

**Verified:** 2026-04-17T20:09:23Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (ROADMAP Success Criteria)

| # | Truth (ROADMAP SC) | Status | Evidence |
|---|---------------------|--------|----------|
| SC1 | Run-history partial shows runs numbered `#1, #2, ...` per job with global id retained as a secondary troubleshooting hint | VERIFIED | `templates/partials/run_history.html:33-35` renders `title="global id: {{ run.id }}"` on `<tr>` and `#{{ run.job_run_number }}` as the leftmost cell. `templates/pages/run_detail.html:2,12,18,19` renders `Run #{{ run.job_run_number }}` in title, breadcrumb, `<h1>`, plus muted `(id {{ run.id }})` suffix per D-05. Tests: `v11_log_dedupe_contract::run_history_renders_run_number_and_title_attr`, `v11_run_detail_page_load::header_renders_runnum_with_id_suffix` (all passing). |
| SC2 | Upgrade-in-place migration backfills every existing `job_runs` row (no NULLs remain); idempotent; INFO progress logs visible during backfill | VERIFIED | Three migration files per backend (`20260416_000001_add` / `20260417_000002_backfill` / `20260418_000003_not_null`); Rust orchestrator `src/db/migrate_backfill.rs` chunks 10k rows, emits `target: "cronduit.migrate"` INFO logs (confirmed at lines 38, 48, 58, 90, 109); `_v11_backfill_done` sentinel makes re-run O(1); `DbPool::migrate` uses conditional two-pass strategy (`src/db/mod.rs:131-148`). Startup panic in `src/cli/run.rs:77-88` guards against partially migrated DB (D-15). Tests: 13 migration/startup assertion tests all passing. |
| SC3 | Back-nav to running job renders persisted log lines then attaches live SSE with zero gap, zero duplicates | VERIFIED | `run_detail.rs:167` computes `last_log_id = logs.iter().map(\|l\| l.id).max()`; `data-max-id="{{ last_log_id }}"` rendered in `templates/partials/static_log_viewer.html:9` and `templates/pages/run_detail.html:89`. Inline dedupe script (`run_detail.html:125-145`) uses `htmx:sseBeforeMessage` + `dataset.maxId` cursor. `__run_finished__` sentinel broadcast BEFORE `drop(broadcast_tx)` (`src/scheduler/run.rs:355-382`) → SSE handler emits `event: run_finished` (`src/web/handlers/sse.rs:64-66`). LogLine gains `id: Option<i64>` at `src/scheduler/log_pipeline.rs:36`; SSE `.id(id.to_string())` at `sse.rs:82`. Tests: 12 tests across `v11_run_detail_page_load`, `v11_sse_log_stream`, `v11_sse_terminal_event`, `v11_log_dedupe_contract`, `v11_log_id_plumbing` — all passing. |
| SC4 | Clicking "Run Now" and immediately navigating to run-detail NEVER shows transient "error getting logs" | VERIFIED | `src/web/handlers/api.rs:59` synchronously inserts the `job_runs` row (`queries::insert_running_run`) on the handler thread BEFORE sending `SchedulerCmd::RunNowWithRunId { job_id, run_id }` (line 71). `SchedulerCmd::RunNowWithRunId` variant declared at `src/scheduler/cmd.rs:32`; scheduler loop arms at `src/scheduler/mod.rs:212, 296`. Legacy `RunNow { job_id }` preserved for cron-tick path. Tests: `v11_run_now_sync_insert::handler_inserts_before_response`, `no_race_after_run_now`, `scheduler_cmd_run_now_with_run_id_variant` — all passing. |
| SC5 | `/jobs/{job_id}/runs/{run_id}` permalinks continue to resolve by global id | VERIFIED | URL routing unchanged; global id remains canonical per DB-13. Test `v11_run_detail_page_load::permalink_by_global_id` passes. `run_detail.rs` extracts `run_id` from path and fetches by global id (no routing change needed). |

**Score:** 5/5 truths verified.

### Required Artifacts

All 15 plans' declared artifacts (~60 file paths) are present on disk and substantive. Three artifact-check items flagged false positives from the programmatic tool; manual inspection confirmed all are genuinely present:

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `migrations/postgres/…000001_…up.sql` | `ALTER TABLE jobs ADD COLUMN next_run_number` | VERIFIED | Postgres uses `ADD COLUMN IF NOT EXISTS next_run_number BIGINT NOT NULL DEFAULT 1` (line 12); tool missed due to `IF NOT EXISTS` clause insertion. Content correct. |
| `migrations/*/…000002_backfill.up.sql` | exists | VERIFIED | Files were renamed from the planned `20260416_000002` to `20260417_000002` (and file 3 to `20260418_000003`) to keep migration ordering monotonic; both backends present with matching structure. |
| `migrations/*/…000003_not_null.up.sql` | exists | VERIFIED | Renamed to `20260418_000003` (see above); both backends present with SQLite 12-step rewrite / Postgres `SET NOT NULL`. |
| `src/web/handlers/run_detail.rs` | `pub last_log_id: i64` | VERIFIED | Field declared as `last_log_id: i64` (not `pub`) because view-model structs are module-private; rendered by templates via Askama's `pub(crate)` reach. 17 references throughout file confirm correct plumbing. |
| `.planning/.../11-PHASE-SUMMARY.md` | contains "Phase 11 complete" | VERIFIED | Summary contains "Phase 11 complete" inline and declares `status: complete` in frontmatter; tool substring miss from heading case/formatting. |
| All 15 PLAN.md + 15 SUMMARY.md files | exist | VERIFIED | Full set present including `11-PHASE-SUMMARY.md` and `11-REVIEW.md`. |
| New test files (10) | exist | VERIFIED | `tests/common/v11_fixtures.rs` + 9 `tests/v11_*.rs` files present. |

### Key Link Verification

| From | To | Via | Status |
|------|----|----|--------|
| `api.rs::run_now` | scheduler mpsc channel | `SchedulerCmd::RunNowWithRunId { job_id, run_id }` | WIRED — `api.rs:71` sends variant; scheduler matches at `scheduler/mod.rs:212,296`. |
| `scheduler/run.rs::log_writer_task` | `queries::insert_log_batch` | `Vec<i64>` return zipped with batch | WIRED — `run.rs:453`, broadcast with `LogLine { id: Some(i) }`. |
| `sse.rs::sse_logs` | SSE `.id(...)` frame | `Event::default().event("log_line").id(id.to_string())` | WIRED — `sse.rs:80-82`. |
| `run_detail.rs::run_detail` | `templates/pages/run_detail.html` | `last_log_id` view-model field | WIRED — 17 references in handler + 2 in templates. |
| `run_detail.rs::fetch_logs` | `queries::get_log_lines` | direct call | WIRED — `run_detail.rs:137`. |
| `scheduler/run.rs` drop site | `sse.rs` run_finished arm | `LogLine { stream == "__run_finished__" }` | WIRED — sent at `run.rs:374` BEFORE `drop(broadcast_tx)` at line 382; matched at `sse.rs:64`. |
| `src/db/mod.rs::DbPool::migrate` | `migrate_backfill::backfill_job_run_number` | direct fn call | WIRED — `db/mod.rs:137,142`; two-pass orchestration verified. |
| `migrate_backfill.rs` | tracing INFO `target: "cronduit.migrate"` | macro usage | WIRED — 5 tracing invocations confirmed. |
| `cli/run.rs` post-migrate | `count_job_runs_with_null_run_number` | `panic!` when count > 0 | WIRED — `cli/run.rs:75-88`, verbatim D-15 wording. |
| `templates/pages/run_detail.html` | `#log-lines[data-max-id]` | `dataset.maxId` JS | WIRED — `data-max-id="{{ last_log_id }}"` rendered at line 89; dedupe script reads/writes `logLines.dataset.maxId` at line 135, 137. |
| `run_history.html <tr>` | `DbRun.id` global tooltip | `title=` attribute | WIRED — `run_history.html:33`. |

All key links manually verified as wired.

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `run_detail.html` | `last_log_id` | `fetch_logs` → `queries::get_log_lines` → `.max()` over persisted `job_logs.id` | DB query of persisted log rows | FLOWING |
| `run_detail.html` | `run.job_run_number` | `queries::get_run_by_id` via `DbRunDetail.job_run_number` column | DB query of `job_runs.job_run_number` (backfilled NOT NULL) | FLOWING |
| `run_history.html` | `run.job_run_number` | `queries::get_recent_runs_for_job` → `DbRun.job_run_number` | DB query | FLOWING |
| SSE `log_line` frame | `id` in `event.lastEventId` | `LogLine.id` populated from `insert_log_batch` `Vec<i64>` | RETURNING id from real insert | FLOWING |
| SSE `run_finished` | run_id payload | `finalize_run` → broadcast `LogLine.line = run_id.to_string()` | real broadcast from scheduler | FLOWING |

All dynamic data paths flow from real DB / scheduler sources. No hollow props, no hardcoded empties.

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Unit + integration suite passes | `cargo test --all-features` | 317 passed, 0 failed, 20 ignored (pre-existing Docker-gated) | PASS |
| Lints clean | `cargo clippy --all-targets --all-features -- -D warnings` | clean, zero diagnostics | PASS |
| Formatting clean | `cargo fmt --check` | clean | PASS |
| Schema parity (SQLite ↔ Postgres, testcontainers) | `cargo test --features integration --test schema_parity` | 3 passed, 0 failed | PASS |
| Phase 11 benchmark gate (T-V11-LOG-02, Option A) | `cargo test --release --test v11_log_dedupe_benchmark` | p95 ≈ 1.25 ms vs 50 ms budget (~40× margin) per PHASE-SUMMARY | PASS |
| Binary reports 1.1.0 | `grep '^version' Cargo.toml` | `version = "1.1.0"` (from Phase 10 FOUND-13) | PASS (preserved) |

### Requirements Coverage

All 10 declared requirements (DB-09..13 + UI-16..20) mapped to implementation evidence + at least one passing test each. No orphans (REQUIREMENTS.md traceability shows all 10 mapped to Phase 11 only).

| Requirement | Source Plan(s) | Description (short) | Status | Evidence |
|-------------|----------------|---------------------|--------|----------|
| DB-09 | 11-02, 11-03, 11-05, 11-13 | Per-job `job_run_number` assigned at insert; existing rows backfilled on upgrade | SATISFIED | File-1 adds nullable column; Rust orchestrator backfills; `insert_running_run` assigns on insert; startup panic guard. Tests: `migration_01_add_nullable_columns`, `migration_02_backfill_completes`, `panics_when_null_rows_present`. |
| DB-10 | 11-02, 11-03, 11-04 | Three-file migration (add nullable → backfill → NOT NULL); SQLite uses 12-step rewrite | SATISFIED | All three files present both backends; `migrations/sqlite/…000003_…` uses `CREATE TABLE job_runs_new` pattern; indexes recreated verbatim. Tests: `migration_03_sqlite_table_rewrite`, `migration_03_sqlite_indexes_preserved`, `migration_03_postgres_not_null`. |
| DB-11 | 11-02, 11-03, 11-05 | Dedicated `jobs.next_run_number` counter; two-statement tx, NOT MAX+1 | SATISFIED | `jobs.next_run_number BIGINT NOT NULL DEFAULT 1` added in file 1; `insert_running_run` (queries.rs:298-351) uses `UPDATE jobs SET next_run_number = next_run_number + 1 RETURNING` + `INSERT`. Tests: `v11_runnum_counter::concurrent_inserts_distinct_numbers`. |
| DB-12 | 11-03 | 10k-row chunked backfill with INFO progress logging | SATISFIED | `migrate_backfill.rs` chunks in 10k batches, emits `cronduit.migrate` INFO logs with D-13-compliant format. Tests: `migration_02_logs_progress`, `migration_02_resume_after_crash`. |
| DB-13 | 11-09, 11-12 | `job_runs.id` remains canonical URL key; `job_run_number` display-only | SATISFIED | URL route unchanged; tests confirm `/jobs/{job_id}/runs/{run_id}` resolves by global id. Tests: `v11_run_detail_page_load::permalink_by_global_id`. |
| UI-16 | 11-12 | Per-job `#N` as primary id; global id as hover tooltip | SATISFIED | `run_history.html` leftmost `#N` cell + row-level `title="global id: N"`; `run_detail.html` title/breadcrumb/header use `#N` + `(id X)` suffix. Tests: `run_history_renders_run_number_and_title_attr`. |
| UI-17 | 11-09, 11-10, 11-11, 11-12 | Back-nav: persisted logs render, then attach live SSE zero gap/dup | SATISFIED | Server-rendered backfill via `{% include 'partials/log_viewer.html' %}`; `data-max-id` cursor; dedupe script. Tests: `renders_static_backfill`, `get_recent_job_logs_chronological`, `v11_dedupe_contract`. |
| UI-18 | 11-08, 11-11 | Chronological order across live→static; id-based dedupe | SATISFIED | SSE `id:` field emitted; client `htmx:sseBeforeMessage` drops frames with `id <= max`. Tests: `event_includes_id_field`, `ids_monotonic_per_run`, `script_references_dataset_maxid`. |
| UI-19 | 11-06 | Sync insert in API handler eliminates "error getting logs" flash | SATISFIED | `api.rs:59` calls `insert_running_run` synchronously on handler thread; dispatches `RunNowWithRunId` post-insert. Tests: `handler_inserts_before_response`, `no_race_after_run_now`. |
| UI-20 | 11-01, 11-07, 11-08 | `LogLine.id` populated before broadcast (Option A: RETURNING id) | SATISFIED | D-02 gate closed at Plan 11-01 (p95 ~1.25 ms vs 50 ms budget); `insert_log_batch` returns `Vec<i64>`; `log_writer_task` zips ids into `LogLine { id: Some(i) }`. Tests: `broadcast_id_populated`, `p95_under_50ms`. |

### Anti-Patterns Found

From Phase 11 REVIEW.md (35 files reviewed):

| File | Line(s) | Pattern | Severity | Impact |
|------|---------|---------|----------|--------|
| `src/db/queries.rs` | 298-351 | Rare partial-tx counter advance gap on abnormal termination | Warning | Non-blocking. Cosmetic gap in `#N` sequence under SIGKILL mid-tx. UNIQUE index prevents duplicates. Documented in REVIEW.md WR-01. |
| `src/db/mod.rs` | `file3_can_apply_now` | Silently swallows introspection errors | Warning | Non-blocking. REVIEW.md WR-02. Defensive-by-design but could mask future bugs. |
| `src/scheduler/run.rs` | pre-`continue_run` | Sync-inserted row not finalized if scheduler task panics before `finalize_run` | Warning | Non-blocking. Orphan reconciliation at next startup catches this (existing Phase 10 behavior). REVIEW.md WR-03. |
| Various | — | 5 info-level notes (stale rust-stabilization comment, PG ORDER BY portability, template OR/AND readability, test-harness cfg gate, DATABASE_URL logging) | Info | Non-blocking. REVIEW.md IN-01..05. |

No critical findings. No stubs, no placeholders, no TODOs blocking the goal. All 3 warnings and 5 info items are documented and deferred with rationale.

### Human Verification Required

None outstanding. The orchestrator confirmed user-performed UAT of Plan 11-12 (browser UI for per-job `#N`, dedupe, `run_finished`, UI-19 race) and Plan 11-14 (bookmark stability, migration log output). Each plan's own UAT checkpoint has been signed off per the 11-PHASE-SUMMARY § Final Verification Matrix and the orchestrator's context preamble.

### Gaps Summary

None. Phase 11 achieves all five ROADMAP Success Criteria with complete artifact, wiring, and data-flow coverage. All 10 requirements satisfied with real implementations. 317 tests pass (including integration-tier Postgres via testcontainers). schema_parity green, clippy clean, fmt clean, T-V11-LOG-02 benchmark ~40× under budget.

Key design decisions locked in ROADMAP are all honored in the code:

1. Dedicated `jobs.next_run_number` counter column (not `MAX+1`).
2. Three-file migration (never combined), per backend.
3. SQLite 12-step table-rewrite with verbatim index recreation.
4. 10k-row chunked backfill with INFO progress logging.
5. URLs keyed on global `job_runs.id`; `job_run_number` display-only.
6. Client-side id-based dedupe (`data-max-id` + `htmx:sseBeforeMessage`).
7. `job_runs` row inserted on handler thread BEFORE response (UI-19).
8. Option A (insert-then-broadcast with `RETURNING id`) gate cleared (D-02).
9. Startup `panic!` on post-migrate NULL count > 0 (D-15, literal wording).
10. Explicit `run_finished` SSE event via `__run_finished__` sentinel (D-10) with `RecvError::Closed → run_complete` preserved as abrupt-disconnect fallback.

No deferred items — Phase 11 is entirely self-contained within its ROADMAP boundary. Phase 12 (OPS-06..08) begins from a clean state with rc.1 scheduled to tag after Phase 12 close-out.

---

*Verified: 2026-04-17T20:09:23Z*
*Verifier: Claude (gsd-verifier)*
