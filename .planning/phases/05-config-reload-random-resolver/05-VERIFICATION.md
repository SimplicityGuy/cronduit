---
phase: 05-config-reload-random-resolver
verified: 2026-04-12T02:00:00Z
status: "code_complete, human_needed"
score: 10/13 must-haves verified
gaps:
  - truth: "An explicit re-randomize request (Re-roll Schedule button) generates a NEW concrete value for the job's resolved_schedule"
    status: failed
    reason: "do_reroll() in src/scheduler/reload.rs copies job.resolved_schedule.clone() instead of calling random::resolve_schedule(). The function contains a TODO comment explicitly stating 'Plan 01 will add the random module with proper @random resolution. When that lands, this will call random::resolve_schedule().' This merge never happened. The Re-roll button is a no-op — it writes the current resolved value back to DB unchanged."
    artifacts:
      - path: "src/scheduler/reload.rs"
        issue: "do_reroll() at lines 173-176: let new_resolved = job.resolved_schedule.clone() — no random::resolve_schedule() call. TODO comment still present."
    missing:
      - "Replace `let new_resolved = job.resolved_schedule.clone()` with `let mut rng = rand::thread_rng(); let new_resolved = crate::scheduler::random::resolve_schedule(&job.schedule, None, &mut rng);`"
      - "Remove the TODO comment referencing Plan 01"

  - truth: "do_reload() returns ReloadResult with correct unchanged count (not hardcoded 0)"
    status: failed
    reason: "do_reload() in src/scheduler/reload.rs at line 92 hardcodes `unchanged: 0` with a TODO comment. sync_config_to_db() correctly tracks and returns sync_result.unchanged, but do_reload() ignores it. The reload API response always reports unchanged=0, even when most jobs are unmodified."
    artifacts:
      - path: "src/scheduler/reload.rs"
        issue: "Line 92: `unchanged: 0, // TODO: track unchanged count in SyncResult` — sync_result.unchanged is computed but not forwarded to ReloadResult"
    missing:
      - "Replace `unchanged: 0, // TODO: ...` with `unchanged: sync_result.unchanged,` in the Ok branch of do_reload()"

  - truth: "Visual checkpoint: UI surfaces for @random badge, resolved schedule display, Re-roll button, settings reload card, and toast behavior have been confirmed by the operator"
    status: failed
    reason: "05-05 Plan Task 2 is a blocking human-verify checkpoint. 05-05-SUMMARY.md documents the checklist but all items show as unchecked (empty checkboxes). 05-VALIDATION.md shows task 05-05-02 status as 'pending'. No approval signal was recorded."
    artifacts:
      - path: ".planning/phases/05-config-reload-random-resolver/05-05-SUMMARY.md"
        issue: "Visual checkpoint checklist (lines 103-115) contains 13 items, all unchecked ([ ]). User approval not recorded."
      - path: ".planning/phases/05-config-reload-random-resolver/05-VALIDATION.md"
        issue: "Task 05-05-02 status: pending. Approval field: pending."
    missing:
      - "Human must start the application with an @random job configured and visually confirm all 13 checklist items from 05-05-SUMMARY.md Task 2"

human_verification:
  - test: "Confirm Re-roll Schedule generates a new random value (after code fix)"
    expected: "Clicking Re-roll Schedule on a job with schedule='@random 14 * * *' changes the resolved value (e.g., from '42 14 * * *' to '17 14 * * *'). A second click produces yet another value."
    why_human: "Requires visual confirmation of UI change after the do_reroll() fix lands"
  - test: "Full UI visual checkpoint per 05-05-SUMMARY.md Task 2"
    expected: "All 13 checklist items pass: @random badge on dashboard, resolved schedule on job detail, Re-roll button, settings reload card, config watcher badge WATCHING, toast auto-dismiss (5s success) and persistent error dismiss"
    why_human: "Visual layout and interactive behavior cannot be verified programmatically"
re_verification:
  re_verified_at: "2026-04-13T21:03:34Z"
  re_verifier: "Claude (Phase 7)"
  status_change:
    from: "gaps_found"
    to: "code_complete, human_needed"
  gap_resolutions:
    - gap: "do_reroll stub -- RAND-03 explicit re-randomize was a no-op (original gap 1)"
      closed_by: "PR #9 (commit 8b69cb8)"
      fix: "src/scheduler/reload.rs:170-172 -- do_reroll now calls `let mut rng = rand::thread_rng(); crate::scheduler::random::resolve_schedule(&job.schedule, None, &mut rng)`, replacing the stub that cloned job.resolved_schedule"
      regression: "tests/reload_random_stability.rs exercises @random stability across reloads; manual re-roll visual confirmation is deferred to Phase 8 human UAT"
    - gap: "do_reload() ReloadResult unchanged count hardcoded to 0 (original gap 2)"
      closed_by: "PR #9 (commit 8b69cb8)"
      fix: "src/scheduler/reload.rs:88 -- `unchanged: sync_result.unchanged,` now forwards the real sync-engine count to the ReloadResult, replacing the previous hardcoded `unchanged: 0` line"
      regression: "existing tests/reload_sighup.rs exercises the ReloadResult plumbing end-to-end through do_reload()"
    - gap: "Visual checkpoint (Plan 05 Task 2) -- UI surfaces not operator-confirmed (original gap 3)"
      closed_by: "deferred"
      fix: "Phase 8 human UAT owns the visual checkpoint walkthrough (terminal-green theme, @random badge, Re-roll button, settings reload card, toast behavior). No code work remains."
      regression: "human-only -- Phase 8 scope; tracked in 08 phase planning, not Phase 7"
    - gap: "Settings page Reload Config card does not auto-refresh after POST /api/reload (filed in 05-UAT.md section 5)"
      closed_by: "PR #9 (commit 8b69cb8)"
      fix: "src/web/handlers/api.rs:175-181 -- reload handler response now includes `HX-Refresh: true` header so HTMX triggers a full-page refresh after a successful reload, surfacing the new Last Reload timestamp without a manual refresh (live tree: `headers.insert(\"HX-Refresh\", \"true\".parse().unwrap())` at line 181; range widened from plan-cited 175-177 to cover the HxEvent + header block in the current tree)"
      regression: "tests/reload_api.rs::reload_response_includes_hx_refresh_header (added in Phase 7 Plan 04) asserts `HX-Refresh: true` on the reload response at the HTTP handler level via `tower::ServiceExt::oneshot`"
---

# Phase 5: Config Reload & @random Resolver Verification Report

**Phase Goal:** Production-grade config reload via SIGHUP / POST /api/reload / debounced file-watch, the slot-based @random algorithm with feasibility checks and daily re-roll cadence, and the UI surfaces that make resolved schedules visible — addressing the two highest-risk / highest-novelty features together because they share the same reload lifecycle.
**Verified:** 2026-04-12T02:00:00Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A schedule string '@random 14 * * *' resolves to a concrete cron string with random minute and fixed hour | VERIFIED | src/scheduler/random.rs: resolve_schedule() with FIELD_RANGES, 14 unit tests pass |
| 2 | Non-random schedules pass through unchanged | VERIFIED | random.rs: is_random_schedule guard returns raw unchanged; test resolve_non_random_passthrough |
| 3 | Sync engine calls @random resolver at sync time and persists resolved_schedule | VERIFIED | sync.rs lines 110-129: resolve_random_schedules_batch called, resolved_map used in upsert |
| 4 | config_hash-matched jobs preserve resolved_schedule across reload | VERIFIED | sync.rs: existing_resolved passed when config_hash matches; random_stability integration test confirms |
| 5 | random_min_gap slot-based enforcement separates fire times by configured gap | VERIFIED | random.rs: resolve_random_schedules_batch with slot tracking; batch_gap_enforcement test with 3 jobs x 90min |
| 6 | Infeasible gap logs WARN and relaxes instead of failing | VERIFIED | random.rs lines 180-193: feasibility pre-check, relaxed = MINUTES_IN_DAY / num_random; infeasible_gap_relaxes test with 30 jobs |
| 7 | SchedulerCmd has Reload and Reroll variants with oneshot channels | VERIFIED | cmd.rs: Reload { response_tx: oneshot::Sender<ReloadResult> }, Reroll { job_id, response_tx } |
| 8 | do_reload() parses config, validates, syncs DB, returns ReloadResult with correct counts on success | PARTIAL | parse+sync+heap rebuild wired correctly; but unchanged count hardcoded to 0 (see gap) |
| 9 | do_reload() returns ReloadResult with status=Error on parse failure, leaving running config untouched | VERIFIED | reload.rs lines 36-57: parse error returns Error without touching jobs map or DB |
| 10 | SIGHUP triggers reload via SchedulerCmd::Reload | VERIFIED | shutdown.rs: install_sighup sends SchedulerCmd::Reload; reload_sighup integration test |
| 11 | File watcher sends SchedulerCmd::Reload with 500ms debounce after config file changes | VERIFIED | reload.rs: spawn_file_watcher with Duration::from_millis(500) debounce; reload_file_watch integration test |
| 12 | An explicit re-randomize request generates a NEW resolved value | FAILED | do_reroll() copies existing resolved_schedule unchanged (TODO comment present, random::resolve_schedule not called) |
| 13 | UI surfaces confirmed by operator: @random badge, resolved schedule, Re-roll button, settings reload card, toast behavior | FAILED | Visual checkpoint (Plan 05 Task 2) not completed — all 13 checklist items remain unchecked |

**Score:** 10/13 truths verified (2 failed, 1 partial)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/scheduler/random.rs` | @random resolver with slot-based gap enforcement | VERIFIED | Exports is_random_schedule, resolve_schedule, resolve_random_schedules_batch; FIELD_RANGES defined; 14 unit tests |
| `src/scheduler/cmd.rs` | Extended SchedulerCmd with Reload and Reroll | VERIFIED | Reload { response_tx }, Reroll { job_id, response_tx }, ReloadResult, ReloadStatus |
| `src/scheduler/sync.rs` | Sync engine wired to @random resolver | VERIFIED | resolve_random_schedules_batch called; unchanged count tracked in SyncResult |
| `src/scheduler/reload.rs` | do_reload(), do_reroll(), spawn_file_watcher() | PARTIAL | do_reload and spawn_file_watcher fully implemented; do_reroll is a stub (copies existing value, no actual re-randomization) |
| `src/shutdown.rs` | SIGHUP handler sending SchedulerCmd::Reload | VERIFIED | install_sighup with SignalKind::hangup(); non-unix fallback |
| `src/config/mod.rs` | ServerConfig with watch_config field | VERIFIED | watch_config: bool with default_watch_config() = true |
| `src/web/handlers/api.rs` | reload() and reroll() HTTP handlers | VERIFIED | Both handlers exist, CSRF validated, reload returns JSON diff, reroll returns toast |
| `src/web/mod.rs` | Routes /api/reload and /api/jobs/{id}/reroll | VERIFIED | Lines 58-59 register both routes |
| `src/scheduler/mod.rs` | Scheduler loop with Reload and Reroll branches | VERIFIED | Both branches present with do_reload/do_reroll calls and D-09 coalescing |
| `templates/partials/job_table.html` | @random badge pill on dashboard | VERIFIED | cd-badge--random applied when job.has_random_schedule |
| `templates/pages/job_detail.html` | Resolved schedule display with re-roll button | VERIFIED | "Resolved to" text and Re-roll Schedule button with hx-post |
| `templates/pages/settings.html` | Enhanced reload card, reload button, watcher status | VERIFIED | hx-post="/api/reload" on Reload Config button; Last Reload card; Config Watcher card |
| `templates/base.html` | Variable-duration toast JS with error dismiss | VERIFIED | duration read from event, autoDismiss, aria-label="Dismiss notification" on error close button |
| `tests/reload_sighup.rs` | SIGHUP reload integration test | VERIFIED | Tests create/update/disable counts and parse error safety |
| `tests/reload_inflight.rs` | In-flight run survival during reload | VERIFIED | Inserts running row, calls do_reload, asserts status still 'running' |
| `tests/reload_random_stability.rs` | @random stability across reload test | VERIFIED | Both unchanged-stability and schedule-change-triggers-rerandomization tested |
| `tests/reload_file_watch.rs` | File watcher triggers reload integration test | VERIFIED | file_change_triggers_reload_command and rapid_edits_coalesced_by_debounce |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| src/scheduler/sync.rs | src/scheduler/random.rs | resolve_random_schedules_batch | WIRED | Line 115: random::resolve_random_schedules_batch called |
| src/scheduler/cmd.rs | tokio::sync::oneshot | oneshot::Sender on Reload/Reroll | WIRED | Both variants carry oneshot::Sender<ReloadResult> |
| src/scheduler/reload.rs | src/config/mod.rs | parse_and_validate() call | WIRED | Line 36: config::parse_and_validate(config_path) |
| src/scheduler/reload.rs | src/scheduler/sync.rs | sync_config_to_db() call | WIRED | Line 66: sync::sync_config_to_db(pool, &parsed.config, random_min_gap) |
| src/shutdown.rs | src/scheduler/cmd.rs | SchedulerCmd::Reload sent | WIRED | Line 19-27: cmd_tx.send(SchedulerCmd::Reload { response_tx: resp_tx }) |
| src/web/handlers/api.rs | src/scheduler/cmd.rs | SchedulerCmd::Reload via AppState.cmd_tx | WIRED | Lines 99-100: state.cmd_tx.send(SchedulerCmd::Reload { response_tx: resp_tx }) |
| src/scheduler/mod.rs | src/scheduler/reload.rs | do_reload/do_reroll calls | WIRED | Lines 180-184, 253-257 in scheduler loop |
| src/scheduler/reload.rs | src/scheduler/random.rs | random::resolve_schedule in do_reroll | NOT WIRED | do_reroll uses job.resolved_schedule.clone() — TODO comment present, resolve_schedule never called |
| templates/pages/settings.html | /api/reload | hx-post on Reload Config button | WIRED | Line 8: hx-post="/api/reload" |
| templates/pages/job_detail.html | /api/jobs/{id}/reroll | hx-post on Re-roll button | WIRED | Line 48: hx-post="/api/jobs/{{ job.id }}/reroll" |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| RAND-01 | 05-01 | @random fields resolved to concrete values at sync time | SATISFIED | random.rs resolve_schedule; sync.rs wiring |
| RAND-02 | 05-01 | resolved_schedule stable across restarts/reloads when schedule unchanged | SATISFIED | resolve_schedule preserves existing_resolved when passed; reload_random_stability test confirms |
| RAND-03 | 05-01 | Re-randomize only on (a) new job, (b) schedule field change, (c) explicit request | PARTIAL | (a) and (b) satisfied by sync engine config_hash logic; (c) explicit re-randomize is broken — do_reroll does not call resolve_schedule |
| RAND-04 | 05-01 | random_min_gap enforced with retry and WARN on failure | SATISFIED | resolve_random_schedules_batch with has_sufficient_gap and MAX_SLOT_RETRIES |
| RAND-05 | 05-01 | Infeasible gap logs WARN, relaxes, continues (never fails to boot) | SATISFIED | Feasibility pre-check in resolve_random_schedules_batch with relaxed gap |
| RAND-06 | 05-04 | UI shows both raw schedule and resolved_schedule on job detail | SATISFIED | job_detail.html shows raw + "Resolved to" + @random badge; dashboard badge too |
| RELOAD-01 | 05-02 | SIGHUP triggers config reload | SATISFIED | shutdown.rs install_sighup; reload_sighup integration test |
| RELOAD-02 | 05-03 | POST /api/reload triggers reload | SATISFIED | api.rs reload() handler; route registered in mod.rs |
| RELOAD-03 | 05-02 | File watcher with 500ms debounce triggers reload on config change | SATISFIED | spawn_file_watcher with 500ms debounce; reload_file_watch integration test |
| RELOAD-04 | 05-02 | Failed parse leaves running config untouched | SATISFIED | do_reload returns Error before any DB mutation on parse failure; reload_sighup test confirms |
| RELOAD-05 | 05-01/02 | Reload diffs DB by config_hash: create/update/disable idempotently | SATISFIED | sync_config_to_db with config_hash comparisons |
| RELOAD-06 | 05-03 | In-flight runs not cancelled on reload | SATISFIED | do_reload rebuilds heap without draining JoinSet; reload_inflight test confirms DB row survives |
| RELOAD-07 | 05-01/02 | Removed jobs marked enabled=0 | SATISFIED | sync.rs disable_missing_jobs; reload_sighup test asserts enabled=false |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| src/scheduler/reload.rs | 173-176 | `let new_resolved = job.resolved_schedule.clone()` — TODO comment states this should call random::resolve_schedule() | Blocker | Re-roll Schedule button is a no-op; RAND-03 condition (c) is not implemented |
| src/scheduler/reload.rs | 92 | `unchanged: 0, // TODO: track unchanged count in SyncResult` | Warning | reload API response always reports unchanged=0, even when most jobs are unmodified; inaccurate diff summary |

### Human Verification Required

#### 1. Full UI Visual Checkpoint

**Test:** Start the application with a config file containing at least one job with `schedule = "@random 14 * * *"` and one with a fixed schedule. Work through all 13 checklist items from 05-05-SUMMARY.md Task 2.
**Expected:** All items pass — @random badge on dashboard, raw and resolved schedules on job detail, Re-roll button works (after code fix), settings page shows reload timestamp, Config Watcher shows WATCHING badge, success toast auto-dismisses (~5s), error toast persists until X clicked.
**Why human:** Visual layout, interactive behavior, and real-time toast timing cannot be verified programmatically.

#### 2. Re-roll Schedule Generates New Value (after gap fix)

**Test:** After the do_reroll() fix is applied, click Re-roll Schedule on a job with schedule `@random 14 * * *`. Click it again.
**Expected:** Each click produces a different minute value. The hour field stays 14. The resolved schedule in the "Resolved to" display updates after each click.
**Why human:** Requires visual confirmation that the resolved value actually changes in the UI, not just that the API returns 200.

### Gaps Summary

Two code gaps and one unresolved human checkpoint block goal achievement:

**Gap 1 (Blocker): do_reroll() is a stub.** The Re-roll Schedule feature is non-functional. When a user clicks "Re-roll Schedule", `do_reroll()` in `reload.rs` copies `job.resolved_schedule.clone()` back to the DB unchanged. A TODO comment on line 175 acknowledges this was meant to be wired to `random::resolve_schedule()` after Plan 01 landed the random module — but that integration was never completed. Plans 01 and 02 ran in the same wave, the Plans note this as a deviation, but the fix was deferred and never applied. This breaks RAND-03 condition (c).

**Gap 2 (Warning): do_reload() drops the unchanged count.** `sync_config_to_db()` correctly computes and returns `sync_result.unchanged`, but `do_reload()` ignores it, reporting `unchanged: 0` in the API response and the Settings page "Last Reload" summary. This makes the diff summary misleading — a reload that touches no jobs reports "0 added, 0 updated, 0 disabled" without the "N unchanged" context.

**Gap 3 (Human checkpoint): UI visual verification not completed.** Plan 05 Task 2 is a blocking human-verify gate. The 05-05-SUMMARY.md documents 13 checklist items, all unchecked. This must be completed before the phase can be marked passed.

---

_Verified: 2026-04-12T02:00:00Z_
_Verifier: Claude (gsd-verifier)_
