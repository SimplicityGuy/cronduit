---
phase: 06-live-events-metrics-retention-release-engineering
reviewed: 2026-04-13T00:00:00Z
depth: standard
files_reviewed: 6
files_reviewed_list:
  - src/telemetry.rs
  - src/scheduler/retention.rs
  - tests/metrics_endpoint.rs
  - tests/retention_integration.rs
  - .github/workflows/ci.yml
  - .gitignore
findings:
  critical: 0
  warning: 2
  info: 5
  total: 7
status: issues_found
---

# Phase 6 Gap-Closure Code Review Report

**Reviewed:** 2026-04-13
**Depth:** standard
**Files Reviewed:** 6
**Status:** issues_found

## Summary

Scope is the Phase 6 gap-closure work for plans 06-06 (metrics `describe_*` + retention startup log + tests) and 06-07 (`.gitignore` UAT pattern + compose-smoke CI job). Earlier plans 06-01..06-05 are already merged and out of scope. This review overwrites the prior 06-REVIEW.md from the original phase run.

Overall the changes are small, well-motivated, and correctly targeted at the gaps identified in the UAT post-mortem. No critical bugs or security issues were found. Two warnings are worth attention:

1. A pre-existing latent bug in `setup_metrics()`'s fallback path (detached handle will not render facade-recorded metrics). It is not introduced by 06-06 but the new eager-observe code now silently depends on the happy-path arm being taken.
2. The `retention_pruner_emits_startup_log_on_spawn` test relies on a 50 ms wall-clock sleep as its "give the task time to emit" signal, which is vulnerable to CI starvation; a deterministic replacement is suggested.

The remaining items are informational: CI hardening opportunities (SHA pinning, `down -v` teardown, debuggability of the `/health` wait loop), a style nit about log-target consistency, and a footgun note about `DbPool::connect("sqlite::memory:")` for future tests in the same file.

The CI `compose-smoke` job itself looks sound: `sed -i` operates on a committed repo file (no user input — no injection surface), there is an explicit post-rewrite guard (`grep -q 'image: cronduit:ci'`), secrets are not exposed (job inherits the top-level `contents: read` only and has no `packages: write`), and teardown runs under `if: always()` after log-dump-on-failure.

## Warnings

### WR-01: `setup_metrics()` fallback path returns a handle that will not render facade-recorded metrics

**File:** `src/telemetry.rs:58-64`

**Issue:**
```rust
let handle = match builder.install_recorder() {
    Ok(handle) => handle,
    Err(_) => {
        tracing::warn!("metrics recorder already installed, building detached handle");
        PrometheusBuilder::new().build_recorder().handle()
    }
};
```
If `install_recorder()` fails because a global recorder is already installed (which happens whenever a single test binary calls `setup_metrics()` twice), the fallback constructs a brand-new `Recorder` locally via `build_recorder()` and returns *its* handle. That handle is not wired into the global `metrics::` facade. The subsequent `describe_*!` / `gauge!(…).set(…)` / `counter!(…).increment(0)` / `histogram!(…).record(0.0)` calls on lines 82–112 all route through the global facade (= the *already-installed* recorder from the first call), not the detached one. The returned handle therefore renders an empty body, and the eagerly described metric families (the whole point of GAP-1) will not appear in it.

This is a pre-existing bug that the 06-06 GAP-1 fix now silently depends on. In practice the only caller that hits this branch is the new integration test `metrics_families_described_from_boot`, and it runs once per test binary, so the `Ok(handle)` arm is taken and the bug is invisible. But it is a latent footgun: any future test added to `tests/metrics_endpoint.rs` that also calls `setup_metrics()` will see an empty render plus a confusing "metrics recorder already installed" warning and will fail for reasons that look nothing like "the global recorder is singleton".

**Fix:** Either make the fallback explicit (`panic!` so tests fail loudly instead of silently rendering blank) or memoize the handle via `OnceLock` so repeated calls return the same handle that *is* attached to the global recorder:
```rust
use std::sync::OnceLock;
static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

pub fn setup_metrics() -> PrometheusHandle {
    HANDLE
        .get_or_init(|| {
            let handle = PrometheusBuilder::new()
                .set_buckets_for_metric(
                    Matcher::Full("cronduit_run_duration_seconds".to_string()),
                    &[1.0, 5.0, 15.0, 30.0, 60.0, 300.0, 900.0, 1800.0, 3600.0],
                )
                .expect("valid bucket config")
                .install_recorder()
                .expect("metrics recorder not yet installed");
            // ... describe_* and zero-obs calls here ...
            handle
        })
        .clone()
}
```
The memoized form also prevents the "fallback builds a recorder without the configured histogram buckets" silent regression that the current fallback branch has.

---

### WR-02: `retention_pruner_emits_startup_log_on_spawn` uses a wall-clock sleep that can race under CI starvation

**File:** `tests/retention_integration.rs:75-79`

**Issue:**
```rust
let handle = tokio::spawn(pruner_future);

// Give the task ~50ms to emit its startup log, then cancel so it exits cleanly.
tokio::time::sleep(Duration::from_millis(50)).await;
cancel.cancel();
```
The test spawns `retention_pruner`, sleeps 50 ms, cancels, joins. The implicit contract is "the startup `tracing::info!` must have emitted before we look at the captured buffer". In practice the spawned task will almost always be polled within 50 ms on a healthy runner, emit the line (which runs before the first `.await`), and park on `interval.tick()` by the time we cancel. But under CI load (GitHub shared runners can stall arbitrarily long), a 50 ms budget is not a bound, it is a hope. When this test flakes, the failure will look like "retention pruner did not emit startup log" despite the fix being correct.

**Fix:** Replace the fixed sleep with a bounded poll against the captured buffer:
```rust
let handle = tokio::spawn(pruner_future);

let start = std::time::Instant::now();
loop {
    {
        let buf = captured.0.lock().unwrap();
        if std::str::from_utf8(&buf)
            .map(|s| s.contains("retention pruner started"))
            .unwrap_or(false)
        {
            break;
        }
    }
    if start.elapsed() > Duration::from_secs(5) {
        panic!(
            "retention pruner did not emit startup log within 5s; \
             captured so far: {:?}",
            captured.0.lock().unwrap()
        );
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
}
cancel.cancel();
```
This turns a brittle 50 ms race into a 5 s upper bound while keeping the happy-path latency to ~10 ms. Note you cannot use `tokio::time::pause()` + `advance()` here because the test is gated on a real tracing subscriber writer, not virtual time.

## Info

### IN-01: CI third-party actions pinned by tag, not commit SHA

**File:** `.github/workflows/ci.yml:119-134`

**Issue:** The new `compose-smoke` job adds `docker/build-push-action@v6` pinned by tag. The rest of the workflow is likewise tag-pinned (`actions/checkout@v4`, `docker/setup-buildx-action@v3`, `docker/setup-qemu-action@v3`, `Swatinem/rust-cache@v2`, `extractions/setup-just@v2`, `taiki-e/install-action@v2`, `dtolnay/rust-toolchain@stable`). Tag pinning is vulnerable to the actions-tag-repoint supply-chain class (a maintainer or attacker force-pushes `v6` to a malicious SHA; the next CI run executes that code with whatever secrets the job has).

This specific job builds and `docker load`s an image from the PR checkout and runs `docker compose up` against it on the runner. Blast radius is limited (no `packages: write`, no `GITHUB_TOKEN` beyond the default read-only) but still real.

**Fix:** Pin every third-party action to a full 40-char commit SHA with the tag as a comment:
```yaml
- uses: docker/build-push-action@2634353e9bccb8ab31e9e35c0a5a7d0ba3d25a80 # v6.9.0
```
This is a workflow-wide hardening, not specific to the new job; flag it for a follow-up issue rather than blocking 06-07.

---

### IN-02: `compose-smoke` tears down with `-v` even on failure, losing state useful for post-mortem

**File:** `.github/workflows/ci.yml:197-200`

**Issue:**
```yaml
- name: Tear down compose stack
  if: always()
  working-directory: examples
  run: docker compose -f docker-compose.yml down -v
```
`down -v` destroys named volumes. On a green run this is what you want (ephemeral runner anyway). On a red run, the teardown fires *after* the `Dump compose logs on failure` step, so logs are preserved — good. But if a future failure mode required inspecting the SQLite volume (e.g., suspected migration corruption), the teardown has already wiped it. Since GitHub runners are themselves ephemeral, `down -v` is not strictly necessary for cleanup.

**Fix:** Drop `-v` from the teardown step; ephemeral-runner teardown handles volume cleanup implicitly:
```yaml
run: docker compose -f docker-compose.yml down
```
Very minor. The current behavior is defensible (matches the default `docker compose down -v` most homelab users run).

---

### IN-03: `/health` wait loop discards `curl` stderr, reducing debuggability on failure

**File:** `.github/workflows/ci.yml:153-166`

**Issue:**
```bash
for i in $(seq 1 30); do
  if curl -sSf http://localhost:8080/health >/tmp/health.json 2>/dev/null; then
    echo "health responded after ${i}s"
    cat /tmp/health.json
    exit 0
  fi
  sleep 1
done
echo "ERROR: /health never responded after 30s"
docker compose -f examples/docker-compose.yml logs
exit 1
```
When the polling loop fails, the diagnostic output is only "ERROR: /health never responded after 30s" followed by compose logs. There is no indication whether the service was reachable-but-5xx, unreachable (port not published), or a DNS failure, because `curl`'s stderr (the actually useful line) is redirected to `/dev/null` inside the loop.

**Fix:** On loop exhaustion, do one more `curl -v` without `-f` and without stderr suppression, and emit `docker compose ps`, before calling `docker compose logs`:
```bash
echo "ERROR: /health never responded after 30s"
echo "--- final curl attempt (verbose) ---"
curl -v http://localhost:8080/health || true
echo "--- docker compose ps ---"
docker compose -f examples/docker-compose.yml ps
echo "--- docker compose logs ---"
docker compose -f examples/docker-compose.yml logs
exit 1
```
Strictly a debuggability improvement, not a bug.

---

### IN-04: Retention pruner startup log uses inconsistent message framing vs. its siblings

**File:** `src/scheduler/retention.rs:21-25,37,49-53,149-154`

**Issue:** Most log lines in this module use identifier-underscore phrasing ("retention prune cycle started", "retention prune cycle completed", "retention_pruner shutting down"). The new startup line uses a different style ("retention pruner started" — space in the noun, not symmetric with `"retention_pruner shutting down"`). This is purely cosmetic but operators grep for these strings.

**Fix:** Pick one style and use it consistently. E.g.:
```rust
tracing::info!(
    target: "cronduit.retention",
    retention_secs = retention.as_secs(),
    "retention_pruner started"
);
```
so "retention_pruner started" / "retention_pruner shutting down" / "retention prune cycle started" / "retention prune cycle completed" form a consistent family. The GAP-2 closure test asserts `output.contains("retention pruner started")` (line 95) and would need to be updated in lockstep. Non-blocking.

---

### IN-05: `DbPool::connect("sqlite::memory:")` creates two disjoint in-memory databases — fine for this test, footgun for future ones

**File:** `tests/retention_integration.rs:57-59`

**Issue:** `DbPool::connect("sqlite::memory:")` (via `src/db/mod.rs::connect_sqlite`) opens *two* separate `SqlitePool`s against the same URL, one for writes (`max_connections = 1`) and one for reads (`max_connections = 8`). For a `:memory:` URL each of those pools gets its own isolated in-memory database — they do not share state. The current test does not care, because it cancels before the first interval tick and never touches the DB, but any future test in this file that actually exercises `queries::delete_old_logs_batch` through the `DbPool` returned by `DbPool::connect("sqlite::memory:")` will write through the write pool and then read `0` rows back through the read pool.

**Fix:** No action needed for the current test. When the `#[ignore]`d retention tests later in this file are implemented, use a shared-cache URL (`"sqlite:file:test_<unique>?mode=memory&cache=shared"`) or a temp-file SQLite to avoid the write-pool/read-pool split-brain. Worth a `// NOTE:` comment above the `sqlite::memory:` line so the next author is warned.

---

_Reviewed: 2026-04-13_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
