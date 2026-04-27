---
phase: 13
plan: 03
subsystem: web-job-detail-duration-card
tags: [observability, duration-card, percentile, job-detail, OBS-04, OBS-05]
requirements: [OBS-04, OBS-05]
wave: 2
depends_on: [13-01, 13-02]

dependency-graph:
  requires:
    - "crate::web::stats::percentile(samples, q) (shipped by plan 13-01)"
    - "crate::web::format::format_duration_ms_floor_seconds(ms) (shipped by plan 13-01)"
    - "queries::get_recent_successful_durations(pool, job_id, limit) (shipped by plan 13-02)"
    - "templates/pages/job_detail.html outer card shape (shipped in Phase 6)"
    - "JobDetailView shape (shipped in Phase 6, extended additively)"
  provides:
    - "DurationView view-model struct consumed by templates/pages/job_detail.html"
    - "job_detail handler renders p50/p95 card end-to-end for every job"
    - "tests/v13_duration_card.rs covers the full N-threshold subtitle matrix for OBS-04"
  affects:
    - "OBS-04 marked complete: p50/p95 visible on every job detail page with honest sample-size gating"
    - "OBS-05 preserved: percentile computed in Rust via stats::percentile (no SQL percentile_cont on Postgres or SQLite)"

tech-stack:
  added: []
  patterns:
    - "Additive-only extension of JobDetailView: new field + substruct; zero existing field modified"
    - "Consumer-side N-threshold enforcement (D-21): handler checks sample_count >= MIN before calling stats::percentile; helper has no threshold logic"
    - "Dual-formatter policy: Run History column still uses shipped format_duration_ms (preserves '42.0s' shape); Duration card uses format_duration_ms_floor_seconds ('42s' shape) per UI-SPEC Copywriting"
    - "Deterministic duration_ms in integration tests via raw SQL insert (finalize_run derives duration from tokio::time::Instant::elapsed() which is non-deterministic)"
    - "Byte-exact assertions against UI-SPEC Copywriting strings (not substrings) — Copywriting contract is load-bearing"

key-files:
  created:
    - path: "tests/v13_duration_card.rs"
      exports: []
      lines-added: 392
      purpose: "Four #[tokio::test] cases covering the N-threshold subtitle matrix end-to-end (query → percentile → format → template → rendered HTML)"
  modified:
    - path: "src/web/handlers/job_detail.rs"
      exports: ["DurationView", "JobDetailView (extended)"]
      lines-added: 75
      purpose: "DurationView substruct + hydration wiring in the full-page handler branch"
    - path: "templates/pages/job_detail.html"
      exports: []
      lines-added: 26
      purpose: "Duration card HTML block inserted between Configuration card (line 68) and Run History section (line 70)"

decisions:
  - "Enforced N<20 threshold in the consumer (job_detail handler) per D-21, not in stats::percentile — keeps the helper pure and leaves the threshold visible to downstream readers via MIN_SAMPLES_FOR_PERCENTILE constant"
  - "Adopted format_duration_ms_floor_seconds (Phase 13-01 helper), not format_duration_ms, for Duration card chip values — UI-SPEC Copywriting locks '42s' (not '42.0s') for observability surfaces"
  - "Integration-test seeder inserts job_runs directly via raw SQL (bypassing finalize_run) to guarantee deterministic duration_ms for the at-threshold test; finalize_run derives duration from wall-clock elapsed which is not test-deterministic"
  - "Hydration lives on the full-page branch only (not the HTMX partial); the HTMX partial refreshes the Run History table alone and never re-renders the Duration card"

metrics:
  duration: "~5 minutes"
  completed: "2026-04-21T10:58:31-07:00"
  tasks-completed: 3
  commits: 3
  files-created: 1
  files-modified: 2
  lines-added: 493
  tests-added: 4
  tests-passing: 198 # 194 lib + 4 new integration tests
  tests-regressed: 0
---

# Phase 13 Plan 03: Duration Card on Job Detail Summary

One-liner: Wired plan 13-01's `percentile()` helper and plan 13-02's `get_recent_successful_durations` query into the job detail handler + template, landing the Duration card (p50/p95 with locked N-threshold subtitle matrix) on every job detail page, backed by a 4-case integration test suite that asserts byte-exact against the UI-SPEC Copywriting contract.

## Scope

Plan 13-03 is the wave-2 consumer of the wave-1 observability foundations. It solves OBS-04 completely — operators now see `p50 1m 34s` / `p95 2m 12s` on every job detail page when ≥20 successful runs exist, and an honest `—` + tooltip when below threshold. OBS-05 (no SQL-native percentile) is preserved structurally: the `Vec<u64>` return type on `get_recent_successful_durations` plus the in-handler `stats::percentile` call form a type-enforced boundary that blocks any future `percentile_cont` rewrite.

## Tasks Completed

### Task 1 — Extend `JobDetailView` with `DurationView` + hydrate in handler

- **Commit:** `4e9f5d8 feat(13-03): extend JobDetailView with DurationView for p50/p95 card (OBS-04)`
- **File:** `src/web/handlers/job_detail.rs` (+75 lines)
- **Shape shipped (verbatim):**
  ```rust
  pub struct DurationView {
      /// "1m 34s" when has_min_samples=true, else "—" (em dash, U+2014).
      pub p50_display: String,
      pub p95_display: String,
      /// True iff sample_count >= MIN_SAMPLES_FOR_PERCENTILE (20).
      pub has_min_samples: bool,
      /// Raw count of successful runs considered (0..=100).
      pub sample_count: usize,
      /// Subtitle text per UI-SPEC § Duration card subtitle matrix.
      pub sample_count_display: String,
  }
  ```
  `JobDetailView` gained a single additive field `pub duration: DurationView,` — all prior fields unchanged.

- **Hydration (verbatim from the handler body):**
  ```rust
  const MIN_SAMPLES_FOR_PERCENTILE: usize = 20;
  const PERCENTILE_SAMPLE_LIMIT: i64 = 100;

  let durations =
      queries::get_recent_successful_durations(&state.pool, job_id, PERCENTILE_SAMPLE_LIMIT)
          .await
          .unwrap_or_default();

  let sample_count = durations.len();
  let has_min = sample_count >= MIN_SAMPLES_FOR_PERCENTILE;

  let (p50_display, p95_display) = if has_min {
      let p50 = stats::percentile(&durations, 0.5)
          .expect("non-empty when sample_count >= MIN_SAMPLES_FOR_PERCENTILE (D-21)");
      let p95 = stats::percentile(&durations, 0.95)
          .expect("non-empty when sample_count >= MIN_SAMPLES_FOR_PERCENTILE (D-21)");
      (
          format_duration_ms_floor_seconds(Some(p50 as i64)),
          format_duration_ms_floor_seconds(Some(p95 as i64)),
      )
  } else {
      ("—".to_string(), "—".to_string())
  };

  let sample_count_display = match sample_count {
      0 => "0 of 20 successful runs required".to_string(),
      1..=19 => format!("{sample_count} of 20 successful runs required"),
      20..=99 => format!("last {sample_count} successful runs"),
      _ => "last 100 successful runs".to_string(),
  };
  ```

- **Single-branch hydration:** the handler only constructs `DurationView` on the full-page path (the `else` branch where `is_htmx == false`). The HTMX partial renders only the Run History table and never re-renders the Duration card — matches UI-SPEC's "HTMX partial does not render the Duration card" locking.

- **Imports added:** `use crate::web::format::format_duration_ms_floor_seconds;` and `use crate::web::stats;`. Existing `use crate::web::format::format_duration_ms;` retained — still consumed by `RunHistoryView` hydration for the Run History column.

### Task 2 — Insert Duration card HTML block

- **Commit:** `2a2ed64 feat(13-03): insert Duration card HTML between Config and Run History (OBS-04)`
- **File:** `templates/pages/job_detail.html` (+26 lines)
- **Insertion point:** between the Configuration card's closing `</div>` (line 68 of the pre-edit file) and the `<!-- Run History -->` comment (line 70). Zero lines outside this range were modified.
- **HTML shape shipped (key excerpt):**
  ```html
  <!-- Duration (OBS-04) -->
  <div style="background:var(--cd-bg-surface);border:1px solid var(--cd-border);border-radius:8px;padding:var(--cd-space-6)" class="mb-6">
    <h2 style="font-size:var(--cd-text-xl);font-weight:700;letter-spacing:-0.02em;margin-bottom:var(--cd-space-4)">Duration</h2>
    <div style="display:flex;gap:var(--cd-space-6);align-items:baseline">
      <div>
        <span style="…;font-weight:700">p50</span>
        <div style="…;margin-top:2px"
             {% if !job.duration.has_min_samples %}title="insufficient samples: need 20 successful runs, currently have {{ job.duration.sample_count }}"{% endif %}>
          {{ job.duration.p50_display }}
        </div>
      </div>
      <div>…p95 symmetric…</div>
    </div>
    <div style="font-size:var(--cd-text-sm);color:var(--cd-text-secondary);margin-top:var(--cd-space-2)">
      {{ job.duration.sample_count_display }}
    </div>
  </div>
  ```
- **Design fidelity:** heading uses `--cd-text-xl` (NOT `--cd-text-lg`) per the checker-locked 4-size typography scale; p50/p95 chip labels use `--cd-text-xs` uppercase weight 700; display values use `--cd-text-xl` primary weight 700 with 2px documented sub-grid margin; subtitle uses `--cd-text-sm` secondary. Configuration card's `<h2>` at line 24 still uses `--cd-text-lg` (unchanged — additive-only rule).
- **Askama compile-time check:** `cargo build --lib` green after the edit — confirms the template bindings (`job.duration.p50_display`, `job.duration.p95_display`, `job.duration.sample_count`, `job.duration.sample_count_display`, `job.duration.has_min_samples`) all match the `DurationView` field names shipped in Task 1.

### Task 3 — Integration tests covering N-threshold matrix

- **Commit:** `da9de65 test(13-03): v13_duration_card integration tests for N-threshold matrix (OBS-04)`
- **File:** `tests/v13_duration_card.rs` (new, 392 lines)
- **Four `#[tokio::test]` functions:**

| Test                                                | N (success) | N (failed) | Subtitle asserted                                   | Display asserted              |
| --------------------------------------------------- | ----------- | ---------- | --------------------------------------------------- | ----------------------------- |
| `zero_runs_renders_card_without_crashing`           | 0           | 0          | `0 of 20 successful runs required`                  | `—` × 2 + "currently have 0"  |
| `nineteen_successful_runs_below_threshold`          | 19          | 0          | `19 of 20 successful runs required`                 | `—` × 2 + "currently have 19" |
| `twenty_successful_runs_at_threshold`               | 20          | 0          | `last 20 successful runs`                           | `10s` × 2 (p50 + p95)         |
| `only_success_counted_excluded_statuses_ignored`    | 10          | 15         | `10 of 20 successful runs required`                 | `—` × 2 + "currently have 10" |

- **Assertion style:** byte-exact against UI-SPEC Copywriting strings. E.g. `assert!(body.contains("0 of 20 successful runs required"))` — not `assert!(body.contains("0 of 20"))`. The Copywriting contract is load-bearing, so slack in the assertion would let a future refactor silently break the UI-SPEC lock.
- **Test-app harness:** pattern cloned from `tests/dashboard_render.rs` — full `cronduit::web::router(state)` wired to an in-memory SQLite pool with all migrations applied. Hits the real `/jobs/{id}` route end-to-end so the test exercises the full pipeline: query → `stats::percentile` → `format_duration_ms_floor_seconds` → `DurationView` → askama render.
- **Deterministic seeder (discretion-resolved):** `seed_runs_with_duration` inserts `job_runs` rows directly via raw SQL with explicit `duration_ms` + `end_time` columns. This intentionally bypasses the shipped `finalize_run` helper because `finalize_run` derives duration from `tokio::time::Instant::elapsed()` — which would make the at-threshold test's "10s" assertion depend on wall-clock timing. Explicit comment in the test file's header documents the rationale so future readers don't "fix" it.
- **Test output:**
  ```
       Starting 4 tests across 1 binary
          PASS [   0.018s] (1/4) cronduit::v13_duration_card zero_runs_renders_card_without_crashing
          PASS [   0.024s] (2/4) cronduit::v13_duration_card nineteen_successful_runs_below_threshold
          PASS [   0.024s] (3/4) cronduit::v13_duration_card only_success_counted_excluded_statuses_ignored
          PASS [   0.222s] (4/4) cronduit::v13_duration_card twenty_successful_runs_at_threshold
          Summary [   0.222s] 4 tests run: 4 passed, 0 skipped
  ```

## Verification Results

```bash
$ cargo build --lib
Finished `dev` profile

$ cargo clippy --lib --tests -- -D warnings
Finished `dev` profile (zero warnings)

$ cargo fmt --check -- src/web/handlers/job_detail.rs tests/v13_duration_card.rs
(clean)

$ cargo nextest run --test v13_duration_card
4 tests run: 4 passed, 0 skipped

$ cargo nextest run --lib   # regression check
194 tests run: 194 passed, 0 skipped
```

Green across every gate. `194 passed` on the lib suite confirms Wave 1's deliveries (9 new tests shipped in plan 13-01) are preserved and Wave 2's other parallel landings (plan 13-04 dashboard sparkline) did not regress any existing behavior.

## Plan Discretion Resolved

| Question                                            | Resolution                                                                                                  |
| --------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Variable name for job id in handler                 | Shipped handler uses `job_id` (Path extractor) — matched without modification                               |
| HTMX partial path vs full-page path                 | Full page only; HTMX partial renders Run History table alone and never touches the Duration card            |
| Test seeder for deterministic `duration_ms`         | Raw SQL insert fixture, NOT `finalize_run` — documented in the test file header for future maintainers     |

## Deviations from Plan

None — plan 13-03 executed exactly as written. No Rule 1/2/3 auto-fixes needed:

- Handler changes compiled clean on first build (one warning burst appeared initially, but was caused by parallel wave 2 agent's unrelated scratch state in `src/web/handlers/dashboard.rs` and was resolved by their subsequent commits — not my scope per executor scope-boundary rules).
- Template askama compile-time check passed immediately after the edit.
- Integration tests passed 4/4 on first run with no debugging needed.

No Rule 4 architectural decisions encountered. The plan's `<context>` was complete enough that no discretion items required escalation.

**Template-edit transient state:** during Task 2 an external process appeared to revert the template edit once (see `system-reminder` mid-execution). The edit was re-applied verbatim and immediately verified via `grep` + `cargo build --lib`. The shipped template at HEAD is correct; no residual inconsistency.

## Threat Model Coverage

Plan 13-03's threat register (4 rows) had no `mitigate` dispositions:

- T-13-03-01 Info disclosure on p50/p95 rendering — `accept` (observability of job duration is the explicit OBS-04 goal; web UI v1 is unauthenticated per THREAT_MODEL.md).
- T-13-03-02 DoS on large samples vector — `mitigate` (query LIMIT 100 is hard-coded; handler enforces `PERCENTILE_SAMPLE_LIMIT: i64 = 100`).
- T-13-03-03 Tampering (template XSS) — `n/a` (all rendered strings are integer-derived formatter output or locked constant text; no user-controlled content flows into the Duration card).
- T-13-03-04 Repudiation — `n/a` (read-only surface; no audit-relevant action).

T-13-03-02 mitigation is inherent in the query (LIMIT 100) and in the handler constant (`PERCENTILE_SAMPLE_LIMIT`). Verified no code path accepts a larger return set.

## Threat Flags

None — plan 13-03 touches no security-relevant surface. All three modified/created files (handler, template, test) operate on data already inside the trust boundary. No new network endpoint, auth path, file access pattern, or schema change.

## Known Stubs

None. The Duration card is fully wired end-to-end: live query (plan 13-02) → live percentile helper (plan 13-01) → live formatter (plan 13-01) → live view-model → live template → rendered HTML. No placeholder values, no mock data, no hardcoded empty arrays. The em-dash display for N<20 is the locked UI-SPEC behavior, not a stub — it is the honest sample-size-gating signal operators rely on to know when to trust the numbers.

## Self-Check: PASSED

**Files verified on disk:**

```
$ [ -f src/web/handlers/job_detail.rs ] && echo FOUND
FOUND
$ [ -f templates/pages/job_detail.html ] && echo FOUND
FOUND
$ [ -f tests/v13_duration_card.rs ] && echo FOUND
FOUND
```

**Commits verified in git history:**

```
$ git log --oneline | grep -E '4e9f5d8|2a2ed64|da9de65'
da9de65 test(13-03): v13_duration_card integration tests for N-threshold matrix (OBS-04)
2a2ed64 feat(13-03): insert Duration card HTML between Config and Run History (OBS-04)
4e9f5d8 feat(13-03): extend JobDetailView with DurationView for p50/p95 card (OBS-04)
FOUND: 4e9f5d8
FOUND: 2a2ed64
FOUND: da9de65
```

**Acceptance-criteria greps verified:**

```
$ grep -q 'pub struct DurationView' src/web/handlers/job_detail.rs && echo OK
OK
$ grep -q 'get_recent_successful_durations' src/web/handlers/job_detail.rs && echo OK
OK
$ grep -q '"last 100 successful runs"' src/web/handlers/job_detail.rs && echo OK
OK
$ grep -q 'Duration (OBS-04)' templates/pages/job_detail.html && echo OK
OK
$ grep -c 'insufficient samples: need 20 successful runs, currently have' templates/pages/job_detail.html
2
$ grep -c '#\[tokio::test\]' tests/v13_duration_card.rs
4
$ grep -q '"0 of 20 successful runs required"' tests/v13_duration_card.rs && echo OK
OK
$ grep -q '"last 20 successful runs"' tests/v13_duration_card.rs && echo OK
OK
```

All structural and behavioral acceptance criteria green. Plan 13-03 complete.
