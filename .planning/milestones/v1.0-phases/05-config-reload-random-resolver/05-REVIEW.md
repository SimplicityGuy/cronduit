---
phase: 05-config-reload-random-resolver
reviewed: 2026-04-11T00:00:00Z
depth: standard
files_reviewed: 22
files_reviewed_list:
  - assets/src/app.css
  - src/cli/run.rs
  - src/config/validate.rs
  - src/scheduler/cmd.rs
  - src/scheduler/mod.rs
  - src/scheduler/random.rs
  - src/scheduler/reload.rs
  - src/scheduler/sync.rs
  - src/web/handlers/api.rs
  - src/web/handlers/dashboard.rs
  - src/web/handlers/job_detail.rs
  - src/web/handlers/settings.rs
  - src/web/mod.rs
  - templates/base.html
  - templates/pages/dashboard.html
  - templates/pages/job_detail.html
  - templates/pages/settings.html
  - templates/partials/job_table.html
  - tests/health_endpoint.rs
  - tests/reload_file_watch.rs
  - tests/reload_inflight.rs
  - tests/reload_random_stability.rs
  - tests/reload_sighup.rs
  - tests/scheduler_integration.rs
findings:
  critical: 0
  warning: 5
  info: 4
  total: 9
status: issues_found
---

# Phase 05: Code Review Report

**Reviewed:** 2026-04-11
**Depth:** standard
**Files Reviewed:** 22
**Status:** issues_found

## Summary

This phase delivers three distinct capabilities: config hot-reload (SIGHUP + file-watcher + API), `@random` cron field resolution with gap enforcement, and the supporting web UI (settings page, job detail re-roll button). The overall implementation is solid and well-structured. The reload coalescing logic, `@random` batch resolver, and scheduler shutdown drain are all careful and correct.

Five warnings were found — none are crashes, but two could produce silently incorrect behaviour at runtime (double DB query per job in sync, and a silent missing-job case in re-roll). Four info items cover minor quality concerns.

---

## Warnings

### WR-01: `sync_config_to_db` fetches each job from DB twice per sync cycle

**File:** `src/scheduler/sync.rs:103-173`

**Issue:** `sync_config_to_db` calls `get_job_by_name` for each job in the batch-input-building loop (line 106), then calls `get_job_by_name` again for the same set of jobs inside the upsert loop (line 134). This means every job is fetched twice per reload. For a config with N jobs this is 2N sequential DB round-trips where N would suffice. More importantly, there is a logical inconsistency: the `existing_resolved` computed in the first loop (lines 106-111) uses the DB state at that moment, but the hash comparison on line 137 uses a second DB read that could theoretically return a different row if a concurrent reroll happened between the two reads. In practice this race is unlikely, but the double fetch is wasteful and fragile.

**Fix:** Consolidate into a single pass. Fetch all existing jobs up front (or use the `jobs` map already passed to `do_reload`), then use that cache for both the batch-input building and the upsert decision:

```rust
// Build a name->DbJob map from a single batch fetch instead of per-job queries.
let existing_jobs: HashMap<String, DbJob> = get_all_jobs_by_name(pool).await?
    .into_iter()
    .map(|j| (j.name.clone(), j))
    .collect();

for job in &config.jobs {
    let existing = existing_jobs.get(&job.name);
    let existing_resolved = existing
        .filter(|db| db.config_hash == compute_config_hash(job) && db.enabled)
        .map(|db| db.resolved_schedule.clone());
    batch_input.push((job.name.clone(), job.schedule.clone(), existing_resolved));
}
// ... then use existing_jobs again in the upsert loop instead of fetching again.
```

---

### WR-02: `do_reroll` updates the in-memory job's `resolved_schedule` but not the fire-heap entry if the job is not in the `jobs` map

**File:** `src/scheduler/reload.rs:191-197`

**Issue:** Lines 191-197 check `jobs.get_mut(&job_id)` before updating the in-memory resolved schedule. If the job exists in the DB but is absent from the in-memory `jobs` map (e.g., because it was disabled then re-enabled in a concurrent reload that hasn't settled yet), the `mem_job.resolved_schedule` update is silently skipped. The heap is then rebuilt from `jobs.values()` (line 196) which will use the old resolved schedule for that job. The DB was updated correctly, but the running scheduler will continue using the stale in-memory value until the next full reload.

**Fix:** Log a warning when the in-memory update is skipped so the discrepancy is visible:

```rust
if let Some(mem_job) = jobs.get_mut(&job_id) {
    mem_job.resolved_schedule = new_resolved;
} else {
    tracing::warn!(
        target: "cronduit.reload",
        job_id,
        "reroll: job not in in-memory map; DB updated but scheduler will use stale schedule until next reload"
    );
}
```

---

### WR-03: File watcher drops the `ReloadResult` response entirely — errors are invisible

**File:** `src/scheduler/reload.rs:287`

**Issue:** The file watcher sends a `Reload` command with a fresh oneshot channel (line 287), then immediately drops the receiving end `_` without reading from it. If the reload fails (parse error, DB error), the error is logged inside `do_reload` but there is no observable outcome at the file-watcher layer — the watcher will keep looping and firing on every subsequent file event, potentially hiding a persistent broken config. This is consistent with the current design intent but makes the watcher appear to succeed even when the scheduler silently ignored the reload.

**Fix:** Await the response and log the outcome at the watcher level:

```rust
let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
if cmd_tx.send(SchedulerCmd::Reload { response_tx: resp_tx }).await.is_err() {
    tracing::debug!(target: "cronduit.reload", "scheduler channel closed, stopping file watcher");
    break;
}
// Log the result so file-watch reloads are fully observable
match resp_rx.await {
    Ok(result) if result.status == ReloadStatus::Error => {
        tracing::warn!(
            target: "cronduit.reload",
            error = ?result.error_message,
            "file-watch triggered reload failed"
        );
    }
    _ => {}
}
```

---

### WR-04: `resolve_schedule` silently falls through to an unvalidated "best effort" on the final retry

**File:** `src/scheduler/random.rs:80-88`

**Issue:** After `MAX_RESOLVE_RETRIES` (10) failed attempts to produce a valid croner-parseable cron string, the function calls `resolve_fields` a final time and returns whatever it produces — including potentially an invalid expression (line 88). The comment says "last-ditch, accepting best effort". If this unvalidated result is persisted to the DB and later fed to `croner::Cron::from_str`, it will silently fail in `to_view` (dashboard.rs:82) and display "invalid" in the next-fire column. There is no downstream guard.

In practice, `@random` fields are drawn from validated ranges and croner is permissive, so this path is almost never exercised. But the absence of a log line that includes the *actual* final value makes diagnosis hard when it does occur.

**Fix:** Log the final unvalidated value so it is visible in tracing output:

```rust
tracing::warn!(
    target: "cronduit.random",
    schedule = %raw,
    resolved = %resolve_fields(&fields, rng),  // compute and log
    "failed to resolve valid cron after {} attempts, returning unvalidated result",
    MAX_RESOLVE_RETRIES
);
resolve_fields(&fields, rng)
```

(Or, stronger: return `raw` unchanged so the caller can detect the failure path.)

---

### WR-05: `check_schedule` in validation replaces only `@random` with `"0"`, but `"0"` may not be a valid stand-in for all cron fields

**File:** `src/config/validate.rs:86-94`

**Issue:** When validating a schedule containing `@random`, each `@random` token is replaced with the literal string `"0"` before passing to croner. This works correctly for the minute and hour fields (where `0` is in range) and for day-of-month (minimum is `1`, not `0`). For the day-of-month field (index 2, range 1-31), substituting `0` produces an expression like `"0 0 0 1 *"` where day-of-month is `0` — an invalid value. If croner validates that the day-of-month must be in `[1,31]`, the validation will incorrectly reject a valid schedule like `"* * @random * *"`.

**Fix:** Use field-specific fallback values rather than a universal `"0"`:

```rust
const RANDOM_FALLBACKS: [&str; 5] = ["0", "0", "1", "1", "0"];

let schedule_to_validate = if is_random_schedule(&job.schedule) {
    job.schedule
        .split_whitespace()
        .enumerate()
        .map(|(i, f)| if f == "@random" { RANDOM_FALLBACKS[i] } else { f })
        .collect::<Vec<_>>()
        .join(" ")
} else {
    job.schedule.clone()
};
```

---

## Info

### IN-01: `settings.html` references undefined CSS variable `--cd-text-muted`

**File:** `templates/pages/settings.html:25`

**Issue:** The "Never" state for last-reload uses `color:var(--cd-text-muted)` but `--cd-text-muted` is not defined in `assets/src/app.css` (which defines `--cd-text-secondary` and `--cd-text-primary`). Browsers will silently fall back to the inherited colour, which is likely acceptable but is still a bug — the colour will be whatever the browser default is rather than the intended muted appearance.

**Fix:** Replace `--cd-text-muted` with the correct token `--cd-text-secondary`:

```html
<div style="font-size:var(--cd-text-base);color:var(--cd-text-secondary);margin-top:4px">Never</div>
```

---

### IN-02: `job_detail.html` nav active block points to dashboard rather than a neutral state

**File:** `templates/pages/job_detail.html:3`

**Issue:** The job detail page sets `{% block nav_dashboard_active %}` with the active-link border style, meaning the Dashboard nav item appears highlighted when viewing a job detail page. This is probably intentional (job detail is a child of dashboard), but it creates a visual inconsistency: navigating to the Settings page from job detail leaves the Dashboard item still visually active in the breadcrumb trail.

**Fix:** If the intent is to show "Dashboard" as the active section, this is fine as-is. If each top-level page should exclusively claim its own nav highlight, consider adding a `nav_none_active` state or removing the block override on job detail.

---

### IN-03: `sync_config_to_db` uses `rand::thread_rng()` which is `!Send` held across no await points, but the scoping is fragile

**File:** `src/scheduler/sync.rs:113-118`

**Issue:** The comment on line 101-103 correctly notes that `ThreadRng` must not be held across await points. The current fix — wrapping the rng creation and batch call in a separate block — works but only because `resolved_map` is computed synchronously before any `.await`. This is a latent footgun: if someone adds an async call inside the braces in the future, the code will not compile (Send bound failure on tokio::spawn) but the original comment may not be noticed. The fix is self-documenting but fragile.

**Fix:** Use `rand::rngs::SmallRng` or `StdRng` seeded from `thread_rng` outside the block, which is `Send`:

```rust
let mut rng = {
    use rand::SeedableRng;
    rand::rngs::SmallRng::from_entropy()
};
let resolved_map: HashMap<String, String> =
    random::resolve_random_schedules_batch(&batch_input, random_min_gap, &mut rng)
        .into_iter()
        .collect();
```

This makes `rng` a `Send` type that can be held across await points safely.

---

### IN-04: `reload_file_watch.rs` test `rapid_edits_coalesced_by_debounce` allows `extra_count <= 1` which makes the debounce assertion too loose

**File:** `tests/reload_file_watch.rs:116-120`

**Issue:** The test assertion `extra_count <= 1` means the test passes if the debouncer emits up to 2 reload commands for 5 rapid writes. The debounce window is 500ms and each rapid write is 50ms apart, so 5 writes all fall within a single 500ms window. The expected outcome is exactly 1 reload command total, not 2. The loose bound was likely added to tolerate timing on slow CI machines, but it reduces the test's value as a correctness check.

**Fix:** The assertion is acceptable for flakiness tolerance; add a comment explaining why `<= 1` is used rather than `== 0`:

```rust
// Allow at most one extra reload: on very slow CI the debounce window may
// expire between writes #4 and #5, producing a second batch. Asserting <= 1
// catches pathological cases (5 separate reloads) while tolerating timing jitter.
assert!(
    extra_count <= 1,
    "debounce should coalesce rapid edits; got {} extra reload commands",
    extra_count
);
```

---

_Reviewed: 2026-04-11_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
