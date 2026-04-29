# Phase 16: Failure-Context Schema + run.rs:277 Bug Fix - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-26
**Phase:** 16-failure-context-schema-run-rs-277-bug-fix
**Areas discussed:** Migration file shape, config_hash backfill policy, get_failure_context SQL shape, Plan atomicity / PR shape

---

## Migration file shape

| Option | Description | Selected |
|--------|-------------|----------|
| One combined add file | Single migration per backend that adds both columns: `ALTER TABLE job_runs ADD COLUMN image_digest TEXT; ADD COLUMN config_hash TEXT;` Both nullable forever. Smallest surface area. | |
| One file per column | Separate `..._image_digest_add.up.sql` and `..._config_hash_add.up.sql` files per backend. Cleaner per-column history; per-requirement attribution. | |
| Three-file pattern (v1.1 precedent) | Mirror Phase 11 / `job_run_number`: add nullable → backfill → NOT NULL. PITFALLS explicitly says NOT to add a NOT NULL step here. | ✓ (initial) |

**User's choice:** Three-file pattern (v1.1 precedent).

**Follow-up clarification:** Asked what the third file should do given research's no-NOT-NULL constraint. Options: (a) skip NOT NULL, third file does indexes/markers; (b) reconsider to two files (drop NOT NULL stage); (c) trust research — separate concerns: 1 file for image_digest add, 2 files for config_hash (add + backfill).

**User's final choice:** "I trust research — follow PITFALLS exactly."

**Locked outcome:**
- `20260427_000005_image_digest_add.up.sql` (one file, nullable forever, no backfill)
- `20260428_000006_config_hash_add.up.sql` (add column, nullable forever)
- `20260429_000007_config_hash_backfill.up.sql` (separate backfill file)
- 3 files × 2 backends = 6 migration files; no NOT NULL step on either column.

---

## config_hash backfill policy

| Option | Description | Selected |
|--------|-------------|----------|
| Backfill + UI marker | PITFALLS-recommended: bulk UPDATE from current `jobs.config_hash`; document semantic compromise; Phase 21 UI distinguishes backfilled rows. | ✓ |
| No backfill — NULL forever | Skip backfill entirely; pre-v1.2 rows stay NULL; UI renders "—". Honest signal, no risk of lying. | |
| Backfill chunked + marker | PITFALLS chunked-loop pattern for ≥100k rows (mirrors v1.1 job_run_number backfill). Most defensive. | |

**User's choice:** Backfill + UI marker (Recommended).

**Notes:** Bulk single UPDATE is fine for homelab DBs; chunked-loop deferred to v1.3 if scaling pain emerges. Marker convention is a comment in the backfill migration documenting the cutoff timestamp; Phase 21's FCTX UI is responsible for the visual marker. No new schema column needed — `end_time < cutoff` heuristic suffices. Image_digest gets NO backfill (pre-v1.2 docker runs simply did not capture).

---

## get_failure_context SQL shape

| Option | Description | Selected |
|--------|-------------|----------|
| CTE + LATERAL/correlated subquery | Two-CTE shape (`WITH last_success AS ..., streak AS ...`); readable; both arms hit the index independently. | ✓ |
| Window functions (LAG/MAX OVER) | `MAX(...) FILTER` + window-counted streak in a single SELECT. Most compact; structural-parity hazard on `FILTER` edge cases. | |
| Flat correlated subqueries | Single SELECT with multiple correlated subqueries, one per output field. Most portable, most verbose. | |

**User's choice:** CTE + LATERAL/correlated subquery.

**Notes:** Sketch locked in CONTEXT.md D-05. `LEFT JOIN ... ON 1=1` returns one row even when last_success is empty. `'1970-01-01T00:00:00Z'` epoch sentinel as COALESCE fallback (RFC3339 TEXT comparison is consistent across SQLite + Postgres). `streak_position` label computed Rust-side (D-06); SQL returns counts/lookups only. `FailureContext` struct in `src/db/queries.rs` (D-07). EXPLAIN tests mirror v1.1's `tests/v13_timeline_explain.rs` precedent (D-08).

---

## Plan atomicity / PR shape

| Option | Description | Selected |
|--------|-------------|----------|
| (a+b) bundled, (c) separate | PR 1 = bug fix + schema migrations + signature changes + write sites. PR 2 = get_failure_context helper + EXPLAIN test. Bundled (a+b) avoids churning `finalize_run` signature twice. | ✓ |
| All three in one PR | Single atomic Phase 16 PR. Simplest mental model; harder to review carefully; poor revertability (Phase 15 D-12 rationale). | |
| Three separate PRs (a / b / c) | Mirror Phase 15 D-12 verbatim. Cost: bug fix PR introduces a temporary `finalize_run` signature that schema PR immediately rewrites. Bad churn shape. | |

**User's choice:** (a+b) bundled, (c) separate (Recommended).

**Notes:** Two PRs, six plans:
- PR 1 (Plans 16-01..04): migrations + DockerExecResult.container_id field + run.rs:301 fix + finalize_run signature change + insert_running_run write site + struct/SELECT updates.
- PR 2 (Plans 16-05..06): `get_failure_context` query + struct + EXPLAIN tests.
Asymmetric on purpose — PR 1 is review-heavy; PR 2 is small and isolates SQL-correctness review.

---

## Claude's Discretion

Areas where the planner has latitude per CONTEXT.md:

- **Migration file naming & date prefix** — date prefix is the day Plan 16-01 lands; numbering must be `_000005`, `_000006`, `_000007` per the v1.1 sequence.
- **Backfill cutoff timestamp format** — comment header (D-03); planner picks RFC3339 vs ISO date vs Unix.
- **`FailureContext` struct location** — `src/db/queries.rs` recommended; sibling file `src/db/failure_context.rs` acceptable.
- **`get_failure_context` final SQL** — D-05's CTE sketch is structural; planner may inline the streak CTE as a correlated subquery if EXPLAIN tests still pass.
- **EXPLAIN test phrasing** — string-match vs structural-walk; Postgres `EXPLAIN` JSON vs text.
- **Whether `16-HUMAN-UAT.md` is needed** — Phase 16 deliverables are mostly DB-internal; planner picks scope.
- **Test file names** — follow `vNN_<feature>_<scenario>.rs` convention.
- **Schema parity test extension** — planner decides if `tests/schema_parity.rs` needs explicit allowlist updates.

---

## Deferred Ideas

Items mentioned during discussion but explicitly out of Phase 16 scope:

- Backfilled-row UI marker rendering — Phase 21 (FCTX UI panel).
- Webhook payload field serialization (streak_position, consecutive_failures, image_digest, config_hash) — Phase 18 / WH-09.
- Failure-context UI panel itself — Phase 21 / FCTX-01..06.
- Chunked-loop backfill for ≥100k rows — v1.3 if scaling pain emerges.
- `config_hash_backfilled BOOLEAN` column — over-engineering; rejected.
- `last_success_start_time` field on FailureContext — Phase 21 can call `get_run_by_id(last_success_run_id)` if needed.
- Window-function SQL shape — structural-parity hazard.
- NOT NULL step on either column — both nullable forever; no v1.2 phase tightens.
- Per-job `streak_position` column — streak is derived, not stored.
- Promote `cargo-deny` from non-blocking to blocking — Phase 24 close-out.
- `webhook_drain_grace = "30s"` — Phase 20 / WH-10.
