---
phase: 22-job-tagging-schema-validators
plan: 03
subsystem: database
tags: [sqlx, sqlite, postgres, parity-pair, json-column, hash-exclusion, regression-lock]

# Dependency graph
requires:
  - phase: 22-01
    provides: "JobConfig.tags: Vec<String> field; jobs.tags TEXT NOT NULL DEFAULT '[]' migration pair"
  - phase: 22-02
    provides: "tag validators (charset+reserved, count cap, substring collision, dedup WARN)"
  - phase: 16
    provides: "DbRunDetail row-mapping pattern; image_digest field analog"
provides:
  - "upsert_job widened to accept tags_json: &str (TAG-02 persistence)"
  - "DbRunDetail.tags: Vec<String> field (D-07 read path)"
  - "get_run_by_id row-map deserializes j.tags JSON column to Vec<String> on both backends"
  - "compute_config_hash D-01 negative invariant locked with comment + tags_excluded_from_hash regression test"
  - "production sync.rs caller normalizes (sort+dedup+lowercase+trim) tags before serializing for stable column values"
affects:
  - 22-04 (webhook payload BUILD-SITE backfill — reads from DbRunDetail.tags this plan adds)
  - 22-05 (integration tests — exercise the full TOML → DB → fetch round-trip)
  - 22-06 (UI surfacing — joins on jobs.tags column this plan wires)

# Tech tracking
tech-stack:
  added: []  # No new external crates (D-17)
  patterns:
    - "JSON-column round-trip on TEXT (sqlx parameterized bind &str at write site, serde_json::from_str().unwrap_or_default() at read site — forgiving on corrupt JSON to avoid breaking webhook delivery)"
    - "Sorted-canonical JSON serialization at the production caller for stable column values across re-uploads of identical TOML (D-09 sub-bullet)"
    - "Negative invariant lock pattern: comment-only breadcrumb in production code + regression test in #[cfg(test)] mod (mirrors the existing env-exclusion lock at hash.rs L50)"

key-files:
  created: []
  modified:
    - src/db/queries.rs
    - src/scheduler/sync.rs
    - src/scheduler/mod.rs
    - src/scheduler/run.rs
    - src/webhooks/coalesce.rs
    - src/webhooks/payload.rs
    - src/webhooks/dispatcher.rs
    - src/config/hash.rs
    - tests/api_run_now.rs
    - tests/dashboard_jobs_pg.rs
    - tests/dashboard_render.rs
    - tests/docker_executor.rs
    - tests/docker_orphan_guard.rs
    - tests/job_detail_partial.rs
    - tests/jobs_api.rs
    - tests/metrics_stopped.rs
    - tests/process_group_kill.rs
    - tests/stop_executors.rs
    - tests/stop_handler.rs
    - tests/stop_race.rs
    - tests/v11_bulk_toggle.rs
    - tests/v11_bulk_toggle_pg.rs
    - tests/v11_run_now_sync_insert.rs
    - tests/v12_exit_histogram.rs
    - tests/v12_fctx_explain.rs
    - tests/v12_webhook_filter_position_explain.rs
    - tests/v13_duration_card.rs
    - tests/v13_sparkline_render.rs
    - tests/v13_timeline_explain.rs
    - tests/v13_timeline_render.rs
    - tests/v13_timeline_timezone.rs

key-decisions:
  - "tags_json arg is &str (not String), bound directly to sqlx — keeps hot-path zero-copy"
  - "Sorted-canonical serialization is the production caller's responsibility, not upsert_job's, so #[cfg(test)] callers can pass `\"[]\"` literally without forcing every fixture to construct a real JobConfig"
  - "DbRunDetail.tags is Vec<String> (not Option<Vec<String>>) because the column is NOT NULL DEFAULT '[]' — the read site always produces a Vec (possibly empty); this distinguishes from image_digest/config_hash which are nullable"
  - "Row-map deserialization uses serde_json::from_str().unwrap_or_default() (forgiving) rather than ? (propagating) — corrupt JSON falls back to empty Vec rather than breaking webhook delivery; column constraints make corruption structurally impossible from cronduit-controlled writes anyway"
  - "Plan 03 leaves the payload.rs:88 `tags: vec![]` placeholder in place — Plan 04 owns the cutover (separation of concerns: Plan 03 provides the data; Plan 04 wires it)"

patterns-established:
  - "JSON-on-TEXT round-trip (write: bind &str of serde_json::to_string(&Vec<String>); read: serde_json::from_str(&str).unwrap_or_default())"
  - "Negative-invariant lock: comment + regression test for fields excluded from compute_config_hash (extends the prior env-exclusion lock at L50 to a second case at L51-55)"

requirements-completed: [TAG-02]

# Metrics
duration: 21min
completed: 2026-05-04
---

# Phase 22 Plan 03: DB Layer Wiring for jobs.tags Summary

**`upsert_job` widened to bind `tags_json: &str` on both backends + `DbRunDetail.tags: Vec<String>` field with forgiving JSON deserialization, plus a comment + regression test locking the D-01 hash exclusion.**

## Performance

- **Duration:** 21 min 11 sec
- **Started:** 2026-05-04T19:26:03Z
- **Completed:** 2026-05-04T19:47:14Z
- **Tasks:** 3 (all auto)
- **Files modified:** 31 (8 production/source + 23 integration tests)

## Accomplishments

- `upsert_job` now accepts `tags_json: &str` after `timeout_secs`; both SQLite and Postgres branches bind it via INSERT VALUES + ON CONFLICT UPDATE SET (parity-pair invariant) — `INSERT INTO jobs (..., tags, ...)` and `tags = excluded.tags` / `tags = EXCLUDED.tags` rows present on both arms.
- `DbRunDetail` has a new `pub tags: Vec<String>` field (NOT `Option`); `get_run_by_id` projects `j.tags AS tags_json` from the existing `JOIN jobs j` and row-maps deserialize forgivingly via `serde_json::from_str(&s).unwrap_or_default()`.
- Production caller in `src/scheduler/sync.rs` builds sorted-canonical `tags_json` (trim + lowercase + filter empty + sort + dedup, then `serde_json::to_string`) immediately before each `upsert_job` call (both update and insert arms, L182 + L198 in the post-edit file).
- 33 caller sites updated across 28 files (15 in-file `#[cfg(test)]` in queries.rs + 4 in-tree elsewhere + 2 in `src/webhooks/{payload,dispatcher}.rs` for `DbRunDetail` constructors + 33 across 23 `tests/*.rs` integration test files — all compile-only, passing `"[]"` to match the column default).
- D-01 negative invariant locked: `// DO NOT include tags` comment block added to `compute_config_hash` (parallels the existing env-exclusion comment), and `tags_excluded_from_hash` regression test added to `hash::tests` asserting tag-only edits produce identical hashes.

## Data Flow

```mermaid
flowchart LR
    A[cronduit.toml<br/>tags = backup, weekly] --> B[JobConfig.tags: Vec&lt;String&gt;]
    B --> C[validators<br/>Plan 22-02]
    C --> D[sync.rs sorted-canonical<br/>trim+lowercase+sort+dedup<br/>serde_json::to_string]
    D --> E[upsert_job tags_json: &str]
    E --> F[(jobs.tags TEXT NOT NULL<br/>DEFAULT '[]')]
    F --> G[get_run_by_id<br/>SELECT j.tags AS tags_json]
    G --> H[serde_json::from_str&lt;br/&gt;.unwrap_or_default]
    H --> I[DbRunDetail.tags: Vec&lt;String&gt;]
    I -.Plan 22-04 cutover.-> J[WebhookPayload.tags<br/>currently vec!]
```

Plan 22-03 owns the path A → I. The dotted edge I → J is owned by Plan 22-04, which will replace the `tags: vec![]` placeholder at `src/webhooks/payload.rs:88` with `run.tags.clone()`.

## Task Commits

Each task was committed atomically. Plan 22-03 split into 5 commits to keep logical units coherent:

1. **Task 1 (queries.rs widening): `upsert_job` + `DbRunDetail.tags` + `get_run_by_id` row-map** — `ba6cef3` (feat)
   - Includes the 15 in-file `#[cfg(test)]` test caller updates (passing `"[]"`)
2. **Task 2a (production caller): sorted-canonical normalization in sync.rs** — `938c64a` (feat)
3. **Task 2b (in-tree #[cfg(test)] callers): mod.rs + run.rs + coalesce.rs + payload.rs + dispatcher.rs** — `7def931` (feat)
4. **Task 2c (integration test callers): 33 call sites across 23 `tests/*.rs` files** — `84ba123` (feat)
5. **Task 3 (D-01 lock): comment + `tags_excluded_from_hash` regression test in hash.rs** — `71a6f4a` (feat)

(Plan metadata commit will be created separately after this SUMMARY is committed.)

### Per-call-site enumeration

**Production callers (sorted-canonical real values):**
- `src/scheduler/sync.rs` (post-edit lines `L182, L198` after the inserted `tags_json` build block at L171-180)

**In-tree `#[cfg(test)]` callers (compile-only `"[]"`):**
- `src/scheduler/mod.rs:654, 726` (originally L654, L726 — passes `"[]"`)
- `src/scheduler/run.rs:735` (passes `"[]"`)
- `src/webhooks/coalesce.rs:200` (passes `"[]"`)

**`DbRunDetail` constructor sites updated to include `tags: Vec::new()`:**
- `src/db/queries.rs` two production row-map sites (sqlite + postgres branches of `get_run_by_id`)
- `src/webhooks/payload.rs:124` (`fixture_run_detail` test helper)
- `src/webhooks/dispatcher.rs:507` (a `#[cfg(test)]` `DbRunDetail` literal in the dispatcher test module — discovered during Task 2; not enumerated in the plan but caught by `cargo build --tests`)

**In-file `#[cfg(test)]` callers in `src/db/queries.rs` (15 sites passing `"[]"`):**
- L2033, L2060, L2079, L2107, L2129, L2142, L2172, L2185, L2207, L2220, L2233, L2263, L2305, L2354, L2414 (post-edit line numbers)

**Crate-level integration tests (33 call sites across 23 files passing `"[]"`):**

| File | Sites |
|------|------:|
| tests/v13_timeline_explain.rs | 4 |
| tests/v12_fctx_explain.rs | 4 |
| tests/v11_bulk_toggle.rs | 2 |
| tests/v11_bulk_toggle_pg.rs | 2 |
| tests/v12_webhook_filter_position_explain.rs | 2 |
| tests/process_group_kill.rs | 2 |
| (17 other files) | 1 each |
| **Total** | **33** |

## Files Created/Modified

- `src/db/queries.rs` — widened `upsert_job` (tags_json: &str + ?9/$9 placeholders + tags = excluded.tags / EXCLUDED.tags), added `pub tags: Vec<String>` to `DbRunDetail`, widened `get_run_by_id` SELECT (project `j.tags AS tags_json`) + both backend row-maps (forgiving `serde_json::from_str(&s).unwrap_or_default()`), updated 15 in-file `#[cfg(test)]` callers
- `src/scheduler/sync.rs` — added sorted-canonical `tags_json` build block; widened both `upsert_job(...)` call sites
- `src/scheduler/mod.rs`, `src/scheduler/run.rs`, `src/webhooks/coalesce.rs` — compile-only updates to `#[cfg(test)]` `upsert_job` callers
- `src/webhooks/payload.rs` — `fixture_run_detail` now includes `tags: Vec::new()` (Plan 04 widens further); `tags: vec![]` placeholder at L88 INTENTIONALLY UNCHANGED — Plan 04 owns that cutover
- `src/webhooks/dispatcher.rs` — `#[cfg(test)]` `DbRunDetail` literal updated with `tags: Vec::new()` (compile-only)
- `src/config/hash.rs` — added 5-line `// DO NOT include tags (Phase 22 / D-01)` comment block immediately after the existing env-exclusion comment; added `tags_excluded_from_hash` regression test in `mod tests`
- 23 `tests/*.rs` integration test files — compile-only updates to all `upsert_job` call sites (33 total) passing `"[]"`

## Decisions Made

- **Combined comment + regression test in a single feat commit (not chore + feat).** The plan suggested `chore` for the comment-only edit and `feat` for code, but the comment and the test address the same D-01 negative invariant and live in the same file. A single coherent feat commit is cleaner than artificial splitting. The git log shows `feat(22-03): lock D-01 — exclude tags from compute_config_hash` covering both edits.
- **Discovered an extra `DbRunDetail` constructor not in the plan.** The plan enumerated `src/db/queries.rs` (two row-map sites) and `src/webhooks/payload.rs:124` (fixture). `cargo build --tests` surfaced a third site at `src/webhooks/dispatcher.rs:507` — a `#[cfg(test)]` literal in the dispatcher test module. Updated with `tags: Vec::new()` (compile-only); documented under Deviations as a Rule 3 (blocking issue) auto-fix.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated extra `DbRunDetail` test literal in `src/webhooks/dispatcher.rs:507`**
- **Found during:** Task 2 (after running `cargo build --tests`)
- **Issue:** Plan enumeration listed only two `DbRunDetail { ... }` sites in non-queries.rs files (`src/webhooks/payload.rs:124` `fixture_run_detail`). `cargo build --tests` surfaced a third site at `src/webhooks/dispatcher.rs:507` — a `#[cfg(test)]` literal in the dispatcher test module that constructs a `DbRunDetail` for a webhook payload signature test.
- **Fix:** Added `tags: Vec::new(),` to the literal (compile-only update — the test does not exercise tag semantics).
- **Files modified:** `src/webhooks/dispatcher.rs` (one line + comment).
- **Verification:** `cargo build --tests` clean post-edit; `cargo test --lib webhooks::dispatcher` passes.
- **Committed in:** `7def931` (Task 2b commit).

---

**Total deviations:** 1 auto-fixed (Rule 3, blocking).
**Impact on plan:** No scope creep — the missed enumeration was a planner oversight, not new work. Same compile-only treatment as the planner-enumerated `fixture_run_detail`.

## Issues Encountered

- **Pre-existing flaky test discovered in `cargo test --lib`:** `webhooks::retry::tests::compute_sleep_delay_honors_retry_after_within_cap` failed once during the first lib-test run due to randomness in the `jitter()` helper. The test asserts `min(cap=360s, max(jitter(300s), 350s)) == 350s`; for jitter factors above 1.167, the result becomes ~360s instead of 350s. This is approximately an 8% failure rate and is unrelated to Plan 22-03 (the function under test does not touch jobs.tags or hash). Reproduced flakiness on the prior commit (Plan 22-02 tip) — confirmed pre-existing. Logged for a future fix (probably needs a clamped jitter or seeded RNG in tests).

- **Pre-existing test failure: `tests/v12_labels_interpolation.rs::lbl_05_key_position_interpolation_env_unset_caught_by_strict_chars`.** Reproduces on the prior commit (Plan 22-02 tip). Failure is in label-key-position interpolation behavior, completely orthogonal to tags/upsert. Not a regression.

- **Docker-required tests cannot run on this dev machine** (`SocketNotFoundError("/var/run/docker.sock")`). All `tests/*_pg.rs` tests and the postgres parity check in `tests/schema_parity.rs::sqlite_and_postgres_schemas_match_structurally` fail to even start. The 2 deterministic `normalize_tests::*` in schema_parity.rs PASS — these are the meaningful checks for Plan 22-03 because they confirm the TEXT-family normalization absorbs the new `tags TEXT NOT NULL DEFAULT '[]'` column without parity drift. The Docker-gated tests will run on CI.

## Verification Gates

| Gate | Result | Notes |
|------|--------|-------|
| `cargo build` | PASS | clean |
| `cargo test --lib` (323 tests) | PASS | including new `tags_excluded_from_hash`; pre-existing flake in `compute_sleep_delay_honors_retry_after_within_cap` (unrelated; see Issues) |
| `cargo test --test schema_parity normalize_tests` | PASS | TEXT-family normalization absorbs the new column |
| `cargo test --test schema_parity sqlite_and_postgres_schemas_match_structurally` | SKIPPED (env) | Requires Docker; runs on CI |
| `cargo build --tests` | PASS | All 33 integration-test caller sites updated |
| `cargo fmt --all -- --check` | PASS | clean |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS | clean |
| `cargo tree -i openssl-sys` | PASS (empty) | D-17 invariant maintained — no new deps |
| `compute_config_hash` body byte-identical pre/post (except 5-line comment block) | PASS | `git show 71a6f4a -- src/config/hash.rs` confirms diff is comment + regression test only |
| `payload.rs:88 vec![]` placeholder still present | PASS | Plan 04 owns the cutover |

## `cargo tree -i openssl-sys` (D-17)

```
$ cargo tree -i openssl-sys
error: package ID specification `openssl-sys` did not match any packages
```

Empty (rustls-only TLS path maintained).

## Self-Check

Verifying claims before proceeding:

- `src/db/queries.rs` widened `upsert_job` — FOUND (signature includes `tags_json: &str`)
- `src/db/queries.rs` `DbRunDetail.tags: Vec<String>` field — FOUND
- `src/db/queries.rs` `get_run_by_id` projects `j.tags AS tags_json` on both backends — FOUND (2 matches)
- `src/db/queries.rs` row-maps deserialize via `serde_json::from_str(&s).unwrap_or_default()` — FOUND (2 matches)
- `src/scheduler/sync.rs` sorted-canonical `serde_json::to_string` build site — FOUND
- `src/config/hash.rs` `// DO NOT include \`tags\`` comment — FOUND
- `src/config/hash.rs` `fn tags_excluded_from_hash` regression test — FOUND
- `src/webhooks/payload.rs:88` `tags: vec![]` placeholder still present (Plan 04 owns cutover) — FOUND
- Commits ba6cef3, 938c64a, 7def931, 84ba123, 71a6f4a present in git log — FOUND

## Self-Check: PASSED

## Cross-References

- **Plan 04 (next)** — webhook payload BUILD-SITE backfill at `src/webhooks/payload.rs:88` reads `run.tags` from the `DbRunDetail.tags` field this plan adds. Plan 04 will replace `tags: vec![]` with `run.tags.clone()` and add the corresponding payload backfill test using a new `fixture_run_detail_with_tags` sibling helper.
- **Plan 05** — integration tests cover the full TOML → DB → fetch round-trip. Plan 03 unblocks Plan 05 by providing the read-side surface (`DbRunDetail.tags`).
- **Plan 06** — UI surfacing of tags in dashboard/job-detail views joins on `jobs.tags` written by this plan.

## Next Phase Readiness

- Plan 04 unblocked: `DbRunDetail.tags` is populated from the database, ready for the webhook payload cutover.
- Plan 05 unblocked: end-to-end round-trip test plumbing complete.
- No blockers identified.

---
*Phase: 22-job-tagging-schema-validators*
*Completed: 2026-05-04*
