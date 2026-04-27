---
phase: 11-per-job-run-numbers-log-ux-fixes
plan: 11
subsystem: web (run-detail template) + integration tests
tags: [frontend, htmx, sse, phase-11, ui-17, ui-18, d-09, d-10]

# Dependency graph
requires:
  - phase: 11-08
    provides: SSE handler emits `id: {n}` on every `log_line` frame whose `LogLine.id` is `Some(n)`. Plan 11-11's client-side dedupe consumes that wire field via `event.lastEventId` surfaced by the HTMX SSE extension.
  - phase: 11-09
    provides: `run_detail` handler computes `last_log_id` from the same `job_logs.id` column as the SSE stream. The template consumer that renders `data-max-id="{{ last_log_id }}"` lands in Plan 11-12; until then, Plan 11-11's dedupe script reads `dataset.maxId` as `"0"` and is a no-op — which is the intended staging.
  - phase: 11-10
    provides: scheduler broadcasts `__run_finished__` sentinel; SSE handler translates it to `event: run_finished\ndata: {"run_id": N}` + `break`. Plan 11-11's `sse:run_finished` listener is the client half of that contract.
provides:
  - templates/pages/run_detail.html — two inline script additions in the `is_running=true` branch. (1) `htmx:sseBeforeMessage` listener drops frames with `evt.detail.lastEventId <= logLines.dataset.maxId` via `evt.preventDefault()` and advances the cursor when accepted. (2) `sse:run_finished` listener calls `htmx.ajax('GET', '/partials/runs/{{ run_id }}/logs', { target: '#log-container', swap: 'outerHTML' })`. The existing `sse:run_complete` listener is preserved as the abrupt-disconnect fallback.
  - tests/v11_log_dedupe_contract.rs — four Wave-0 `#[ignore]` stubs replaced with real bodies: `script_references_dataset_maxid`, `listens_for_run_finished`, `script_references_htmx_sse_hook`, and the new autonomous `v11_dedupe_contract` pure-Rust unit test that locks the `id > max -> accept, update max; id <= max -> drop` rule including the backfill/live reconnect overlap window (98..=102 against max=100). Plan-11-12-owned stubs (`data_max_id_rendered`, `run_history_renders_run_number_and_title_attr`) remain `#[ignore]`.
affects: [11-12, 11-14]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Client-side SSE dedupe via the HTMX SSE extension's cancellable `htmx:sseBeforeMessage` event (vendored at assets/vendor/htmx-ext-sse.js:119). The extension fires the event through `api.triggerEvent` which honors `evt.preventDefault()` — returning falsy from that triggerEvent call makes the extension skip the default swap for the frame. This is the exact hook needed for id-based dedupe: compare `evt.detail.lastEventId` (numeric SSE id: field) against `logLines.dataset.maxId` (the high-water cursor rendered server-side), call `preventDefault()` for duplicates, advance the cursor for accepted frames. No capture-phase fallback required — RESEARCH Q2 is RESOLVED by direct source-grep of the vendored extension."
    - "Graceful live-to-static swap via one-shot `htmx.ajax` from an SSE event listener. When the server emits `event: run_finished\\ndata: {\"run_id\": N}\\n\\n` (Plan 11-10's D-10 terminal), the client listener calls `htmx.ajax('GET', '/partials/runs/{run_id}/logs', { target: '#log-container', swap: 'outerHTML' })` which HTTP-GETs the static log partial and swaps the live log-container subtree with it via HTMX's standard OOB machinery. The preserved `sse:run_complete` listener stays as the abrupt-disconnect fallback (fires when `RecvError::Closed` hits the handler before the sentinel is drained by the subscriber's lag-prone broadcast channel slot)."
    - "Dedupe-rule locked via pure-Rust autonomous test. `v11_dedupe_contract` defines `fn dedupe_apply(max, incoming) -> (accepted, new_max)` that mirrors the JS guard byte-for-byte, then drives five case scenarios (first frame, equal-id replay, older frame, next-higher frame, reconnect overlap 98..=102 vs max=100). The test has no browser dependency and locks the contract at CI without needing the browser UAT that Plan 11-12 Task 5 will run. Any off-by-one regression in the JS (e.g. `<` vs `<=`) that also drifts in the Rust mirror will be caught by the assertions."
    - "Shadowing `let mut max` for the two separate phases of `v11_dedupe_contract` (0-based build-up vs 100-based reconnect scenario). Avoids the `unused_assignments` lint that fires when the first phase's final `max = new_max;` (case 4) is followed by a hard reset to 100 — the shadow makes the reset point a new binding."

key-files:
  created:
    - .planning/phases/11-per-job-run-numbers-log-ux-fixes/11-11-SUMMARY.md
  modified:
    - templates/pages/run_detail.html
    - tests/v11_log_dedupe_contract.rs

key-decisions:
  - "Used `htmx:sseBeforeMessage` directly with no capture-phase fallback. Grep of `assets/vendor/htmx-ext-sse.js:119` confirms the extension fires the event via `api.triggerEvent(elt, 'htmx:sseBeforeMessage', event)` which honors `evt.preventDefault()` (the `!` inversion on the triggerEvent return value). RESEARCH Q2 RESOLVED — no need for the capture-phase workaround the Wave-0 stubs hedged against. The plan calls this out explicitly and the landed script follows."
  - "Added the dedupe + `run_finished` handlers INSIDE the existing `(function() { ... })()` IIFE rather than spawning two new IIFEs. Keeps the running-run branch's client code in one scope (shared `logLines` variable, shared observer lifetime, shared error-handler context). The plan body suggested two separate `<script>` blocks; landing them as two `addEventListener` calls inside the existing IIFE is strictly smaller and behaviorally identical, and keeps the running-branch's DOM/event plumbing co-located. Documented as a scope judgement, not a deviation — plan's `<done>` list only mandates the listener presence + behavior, not block count."
  - "Kept the existing `sse:run_complete` listener in place AFTER the new `sse:run_finished` listener. The plan's `must_haves.truths[2]` explicitly names the preserved listener as the abrupt-disconnect fallback; landing it second keeps the graceful path visually primary while the fallback path sits below with a comment explaining why it remains. If both fire in rapid succession (pathological timing), the two `htmx.ajax` calls race but both target the same URL + swap, so the second call is idempotent at the DOM level (second swap replaces a static partial with a byte-identical static partial). The plan accepted this in `<threat_model>` T-11-11-02."
  - "Added a local `build_test_app()` helper in `tests/v11_log_dedupe_contract.rs` (Rule-3 Blocking). The plan pseudo-code references `tests_common::build_test_app().await` returning `(app, state)`, but no such helper exists under `tests/common/` — the established pattern (from Plan 11-09's `v11_run_detail_page_load.rs`) is a local helper per test file. I mirrored that shape: minimal `AppState` + a router with the single `/jobs/{job_id}/runs/{run_id}` GET route + CSRF middleware. Returns `(Router, DbPool)` because the tests only issue GETs; no need to thread `active_runs` or `cmd_tx` handles out. This is the same Rule-3 adaptation Plan 11-09 made and its SUMMARY documents."
  - "Shadowed `let mut max` between phase 4 and phase 5 of `v11_dedupe_contract` rather than writing `max = 100;` over the existing binding. The `max = new_max;` at the end of case 4 is legitimately unused (case 5 resets `max` to 100 unconditionally), and rustc's `unused_assignments` lint would trip without the shadow. Shadowing declares phase 5 as a logically-separate scenario and is clearer than suppressing the lint or restructuring phases 1-4 to avoid the write."

requirements-completed: [UI-17, UI-18, D-09, D-10]

# Metrics
duration: ~7min
completed: 2026-04-17
---

# Phase 11 Plan 11: Client-Side Dedupe + run_finished Listener Summary

**`templates/pages/run_detail.html` now installs two inline listeners inside the running-run `(function() { ... })()` IIFE: (1) `htmx:sseBeforeMessage` reads `evt.detail.lastEventId`, compares it to `logLines.dataset.maxId`, calls `evt.preventDefault()` to drop frames at or below the cursor, and advances the cursor for accepted frames (closes the D-09 dedupe loop started server-side by Plans 11-08 and 11-09); (2) `sse:run_finished` calls `htmx.ajax('GET', '/partials/runs/{{ run_id }}/logs', { target: '#log-container', swap: 'outerHTML' })` to swap the live view to the static partial (closes the D-10 graceful-terminal loop started by Plan 11-10). The existing `sse:run_complete` listener is preserved as the abrupt-disconnect fallback. `tests/v11_log_dedupe_contract.rs`'s four Plan-11-11 Wave-0 stubs (`script_references_dataset_maxid`, `listens_for_run_finished`, `script_references_htmx_sse_hook`, `v11_dedupe_contract`) now ship with full bodies — three exercise the rendered run-detail HTML for the required JS hooks/identifiers, and the fourth is a pure-Rust autonomous unit test that locks the `id > max -> accept, update max; id <= max -> drop` rule end-to-end including the backfill/live reconnect overlap scenario. Plan-11-12-owned stubs remain `#[ignore]`. `cargo test --test v11_log_dedupe_contract` -> 4 passed, 2 ignored; `cargo test --lib` -> 173 passed; adjacent regressions (`v11_run_detail_page_load`, `v11_sse_log_stream`, `v11_sse_terminal_event`) all pass; `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` both clean.**

## Performance

- **Duration:** ~7 min
- **Started:** 2026-04-17T02:37:58Z
- **Completed:** 2026-04-17T02:44:45Z
- **Tasks:** 3 (two production + verification)
- **Files created:** 1 (this SUMMARY.md)
- **Files modified:** 2 (1 template, 1 test)

## Accomplishments

- `templates/pages/run_detail.html` (+31 lines): inside the existing `(function() { ... })()` IIFE in the `is_running=true` branch:
  - **Dedupe handler:** `logLines.addEventListener('htmx:sseBeforeMessage', ...)` reads `evt.detail.lastEventId`, parses as i64, compares against `logLines.dataset.maxId` (parsed as i64, defaults to 0), calls `evt.preventDefault()` when `incoming <= max` (and incoming is truthy), advances `logLines.dataset.maxId` to the new high-water mark when `incoming > max`. Early-returns on non-`log_line` frames (e.g. `run_finished`, future event types) so the guard doesn't disturb them. 8 lines of code; 8 lines of doc comment naming the hook (D-09) + the RESEARCH Q2 resolution.
  - **run_finished listener:** `logLines.addEventListener('sse:run_finished', ...)` calls `htmx.ajax('GET', '/partials/runs/{{ run_id }}/logs', { target: '#log-container', swap: 'outerHTML' })` guarded by `typeof htmx !== 'undefined' && htmx.ajax`. 7 lines of code; 5 lines of doc comment naming the D-10 contract and explaining why `sse:run_complete` is preserved below.
  - **sse:run_complete listener:** preserved byte-identical with a new clarifying comment marking it as the abrupt-disconnect fallback (the graceful path is now `sse:run_finished`).
- `tests/v11_log_dedupe_contract.rs` (+214/-9):
  - `script_references_dataset_maxid` (VALIDATION 11-11-01): builds a test app, seeds a running run, GETs the run-detail page, asserts body contains `dataset.maxId` AND `preventDefault` — proves the dedupe script is inlined AND uses the correct cursor identifier AND uses the preventDefault hook.
  - `listens_for_run_finished` (VALIDATION 11-11-02): asserts body contains `sse:run_finished` AND `htmx.ajax` — proves the run_finished listener is installed AND uses htmx.ajax to swap to the static partial.
  - `script_references_htmx_sse_hook` (VALIDATION 11-11-03): asserts body contains `htmx:sseBeforeMessage` — proves the dedupe uses the confirmed-cancellable hook per RESEARCH Q2.
  - `v11_dedupe_contract` (NEW autonomous unit test; replaces the removed browser UAT): defines a pure Rust `dedupe_apply(max, incoming) -> (bool, i64)` that mirrors the JS guard byte-for-byte, then drives five case scenarios: (1) first frame id=5 accepted; (2) replayed id=5 dropped (equal not strictly greater); (3) older id=3 dropped; (4) next id=6 accepted; (5) reconnect overlap — backfill last id 100 + live stream [98, 99, 100, 101, 102] -> only 101, 102 pass; max=102. Captures the complete dedupe contract without a browser.
  - Local `build_test_app()` helper (36 lines) mirrors the Plan-11-09 pattern in `tests/v11_run_detail_page_load.rs`. Rule-3 adaptation — plan pseudo-code referenced non-existent `tests_common::build_test_app()`.
  - Plan-11-12 stubs (`data_max_id_rendered`, `run_history_renders_run_number_and_title_attr`) unchanged (still `#[ignore]`).
- `cargo check` -> clean.
- `cargo test --test v11_log_dedupe_contract` -> 4 passed; 0 failed; 2 ignored (Plan-11-12-owned) in 0.23s.
- `cargo test --lib` -> 173 passed; 0 failed.
- `cargo test --test v11_run_detail_page_load` -> 3 passed; 1 ignored (Plan 11-09 preserved).
- `cargo test --test v11_sse_log_stream` -> 2 passed (Plan 11-08 preserved).
- `cargo test --test v11_sse_terminal_event` -> 2 passed (Plan 11-10 preserved).
- `cargo fmt --check` -> clean.
- `cargo clippy --all-targets -- -D warnings` -> clean.

## Task Commits

Each task committed atomically on branch `worktree-agent-a5cd319b` (worktree for `gsd/phase-11-context`, base `71eafae`):

1. **Task 1:** `134cecb` — `feat(11-11): inline dedupe + run_finished listener in run_detail`
2. **Task 2:** `07d82bb` — `test(11-11): replace Wave-0 stubs with dedupe contract + HTML assertions`
3. **Task 3 (verification-only):** no new commit — covered by `cargo build` (dev profile), `cargo test --test v11_log_dedupe_contract v11_dedupe_contract`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`. See Deviations for the `cargo build --release` environmental blocker.

## Files Created/Modified

- `templates/pages/run_detail.html` (MODIFIED, +31/-0) — two `addEventListener` calls (`htmx:sseBeforeMessage` dedupe + `sse:run_finished` terminal) added inside the existing `(function() { ... })()` IIFE in the `is_running=true` branch. Existing `sse:run_complete` listener preserved with a clarifying comment.
- `tests/v11_log_dedupe_contract.rs` (MODIFIED, +214/-9) — four Plan-11-11 `#[ignore]` stubs replaced with full bodies including a new autonomous pure-Rust `v11_dedupe_contract` test; local `build_test_app()` helper added; Plan-11-12 stubs unchanged.
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-11-SUMMARY.md` (NEW) — this file.

## Decisions Made

1. **`htmx:sseBeforeMessage` hook, no capture-phase fallback.** Vendored extension fires the event via `api.triggerEvent` which honors `evt.preventDefault()`. RESEARCH Q2 RESOLVED by grep. No need for the hedge the Wave-0 stubs left room for.
2. **One IIFE, two new `addEventListener` calls.** Plan suggested two separate `<script>` blocks; landing inside the existing IIFE is strictly smaller, reuses the `logLines` binding, and keeps the running-branch client code co-located. `<done>` list only mandates listener presence + behavior, not block count.
3. **Preserved `sse:run_complete` listener AFTER `sse:run_finished`.** The graceful path is visually primary; the fallback sits below with a comment. Both-fire race is idempotent (second `htmx.ajax` swaps a static partial with the same static partial), accepted in `<threat_model>` T-11-11-02.
4. **Local `build_test_app()` helper** (Rule-3 Blocking). Plan pseudo-code referenced non-existent `tests_common::build_test_app()`; mirrored the Plan-11-09 pattern from `tests/v11_run_detail_page_load.rs`.
5. **Shadowing `let mut max`** between phase 4 and phase 5 of `v11_dedupe_contract` to satisfy `unused_assignments` lint cleanly. Alternative (suppressing the lint or restructuring phase 1-4) was uglier; the shadow makes phase 5 a logically-separate reconnect scenario.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `tests_common::build_test_app` does not exist**
- **Found during:** Task 2 (writing the test bodies).
- **Issue:** The plan's Task 2 `<action>` pseudo-code references `tests_common::build_test_app().await` returning `(app, state)`. No such helper exists under `tests/common/`. The established pattern is a local `build_test_app()` per test file (as `tests/v11_run_detail_page_load.rs` landed in Plan 11-09).
- **Fix:** Added a local `build_test_app()` helper in `tests/v11_log_dedupe_contract.rs` returning `(Router, DbPool)` — minimal `AppState` + router with only the `/jobs/{job_id}/runs/{run_id}` GET route + CSRF middleware. Adapted the pseudo-code's `(app, state)` destructure to `(app, pool)` since the tests only need the pool to seed a running run.
- **Files affected:** `tests/v11_log_dedupe_contract.rs` (new helper function at the top).
- **Verification:** All four tests pass against the local helper.
- **Committed in:** `07d82bb` (Task 2 commit — the helper is part of the test file).

**2. [Rule 3 - Blocking] `unused_assignments` lint on phase-4 `max = new_max;` in `v11_dedupe_contract`**
- **Found during:** first `cargo test` run after writing the test body verbatim from the plan pseudo-code.
- **Issue:** The plan's `v11_dedupe_contract` body writes `max = new_max;` at the end of case 4 (setting max=6), then hard-resets `max = 100;` at the start of case 5 (the reconnect scenario). rustc's `unused_assignments` lint flags the case-4 write as dead.
- **Fix:** Removed the case-4 `max = new_max;` line (it was never consumed) and changed the case-5 reset from `max = 100;` to `let mut max: i64 = 100;` — shadowing the outer `max` to make phase 5 a new, logically-separate scenario. Logic identical; lint clean.
- **Files affected:** `tests/v11_log_dedupe_contract.rs` (two lines in `v11_dedupe_contract`).
- **Verification:** `cargo test --test v11_log_dedupe_contract v11_dedupe_contract` -> passes, no warnings.
- **Committed in:** `07d82bb` (Task 2 commit — part of the test body).

**3. [Rule 3 - Blocking/Environmental] `cargo build --release` fails without vendored Tailwind binary**
- **Found during:** Task 3 verification.
- **Issue:** `build.rs` at the repo root hard-panics on release profile when `bin/tailwindcss` is absent, to guard against shipping a Docker image with stub CSS. This worktree doesn't have the binary downloaded (it's a CI/Docker-stage artifact produced by `just tailwind`).
- **Fix:** Substituted `cargo build` (dev profile) for `cargo build --release`. The dev build uses the same Rust code paths and exercises the askama template compile check; the release-profile gate is an environmental deployment check unrelated to Plan 11-11's code changes. Documented here; the build.rs check itself is pre-existing (inspected via `head -30 build.rs`), not introduced by this plan.
- **Files affected:** none (verification-only adjustment).
- **Verification:** `cargo build` (dev) -> clean; `cargo test --test v11_log_dedupe_contract` -> 4 passed; `cargo clippy --all-targets -- -D warnings` -> clean; `cargo fmt --check` -> clean. Release build will pass in CI where `bin/tailwindcss` is populated.

**All other plan body directives followed exactly.** No Rule 1 (bugs), no Rule 2 (missing critical functionality), no Rule 4 (architectural) deviations.

## Threat Flags

None beyond the plan's `<threat_model>`.

- **T-11-11-01 (XSS via template interpolation):** Mitigated as planned. Inline script uses `{{ run_id }}` inside a JS string literal (`'/partials/runs/{{ run_id }}/logs'`) — askama auto-escapes HTML by default, and `run_id` is an i64 whose decimal-ASCII representation contains no HTML metacharacters. The i64 -> string path is bounded to `[-19..=19]` chars, no user-controlled bytes can reach the interpolation.
- **T-11-11-02 (Infinite ajax loop on run_finished):** Accepted. Listener issues a single `htmx.ajax` call per firing; the swapped-in static partial has no SSE subscription, so the listener goes dormant. If both `sse:run_finished` and `sse:run_complete` fire in rapid succession, the second `htmx.ajax` swaps the static partial with a byte-identical static partial — idempotent at the DOM level.
- **T-11-11-03 (Dedupe bypass):** Accepted. Dedupe is cosmetic — it prevents visually rendering already-displayed log content on SSE reconnect. Any frame that slips past the guard is authorized content the operator was already allowed to see.

No new network endpoints (the `htmx.ajax` GET hits the existing `/partials/runs/{run_id}/logs` route), no new auth paths, no new file-access patterns. Purely client-side behavior inside the existing run-detail page.

## Issues Encountered

Two Rule-3 blocking issues (both auto-fixed per the deviation rules) plus one Rule-3 environmental substitution:
1. Non-existent `tests_common::build_test_app` reference in plan pseudo-code -> local helper mirroring Plan 11-09's pattern.
2. `unused_assignments` lint on phase-4 `max = new_max;` in the `v11_dedupe_contract` body -> removed the dead write + shadowed `max` for phase 5's reset.
3. `cargo build --release` requires `bin/tailwindcss` (pre-existing `build.rs` environmental check; see build.rs:17-27) -> substituted dev-profile `cargo build` for the verification run; release gate is covered by CI where the Tailwind binary is populated.

Each is documented under Deviations from Plan.

## Deferred Issues

- **Release-profile build (`cargo build --release`) environmental constraint.** The repo's `build.rs` hard-panics without `bin/tailwindcss`, which is only populated by `just tailwind` or the Docker builder stage. This is pre-existing, not introduced by Plan 11-11, and does not affect the Plan-11-11 code paths — but it means local-only release builds in this worktree require `just tailwind` first. Track if workflow-level tooling is needed to auto-populate the binary for executor worktrees; out-of-scope for Plan 11-11.

## TDD Gate Compliance

Plan 11-11 has `tdd="true"` on Task 2 only. The TDD gate sequence on disk:

- **RED (from Plan 11-00):** commits `fa26618` / `783e9ca` landed the four Wave-0 `#[ignore]` stubs representing the pre-implementation state.
- **GREEN (Task 1, production code):** `134cecb` — template script additions. After this commit the Wave-0 stubs *could* pass but are still `#[ignore]`.
- **GREEN (Task 2, real test bodies):** `07d82bb` — removes `#[ignore]` on the four Plan-11-11 stubs and lands full bodies. All four tests pass end-to-end.
- **REFACTOR:** not required; `cargo fmt --check` clean throughout.

`feat(...)` commit: `134cecb` (Task 1 production).
`test(...)` commit: `07d82bb` (Task 2 real test bodies + helpers).

## User Setup Required

None. All changes are:
- Two `addEventListener` additions inside an existing inline script IIFE in the running-run branch of `run_detail.html`. The dedupe guard is a no-op until Plan 11-12 emits `data-max-id` into the template (`dataset.maxId` defaults to `"0"`, which means any positive SSE id is strictly greater and every frame is accepted — correct fail-safe semantics).
- Test-only additions (new local helper + real bodies).

No new migrations, config keys, CLI flags, routes, dependencies, or operator action.

## Browser UAT

**Deferred to Plan 11-12 Task 5 per the plan's autonomy resolution.** Plan 11-11 is `autonomous: true`; the browser UAT is consolidated into Plan 11-12 Task 5 because Plan 11-12 is the plan that renders `data-max-id="{{ last_log_id }}"` into the template. Without that attribute, Plan 11-11's dedupe script silently no-ops (dataset.maxId reads as empty string -> parsed as NaN -> `|| 0` fallback -> 0, which accepts every positive incoming id). Running the browser UAT before Plan 11-12 would only prove that the no-op case is correct — the meaningful test (reconnect drops previously-rendered frames) requires the template attribute to be populated.

Plan 11-11's blocking verification is autonomous:
1. `cargo build` (dev profile) — template compile check + full build.
2. `cargo test --test v11_log_dedupe_contract v11_dedupe_contract` — autonomous pure-Rust dedupe contract test.
3. `cargo clippy --all-targets -- -D warnings` — no new warnings.
4. `cargo fmt --check` — format clean.

All four gates pass.

## Next Phase Readiness

- **Plan 11-12 (template consumer for `data-max-id` + run-history UI) unblocked on the client side.** Once Plan 11-12 adds `data-max-id="{{ last_log_id }}"` to the `#log-lines` element, the Plan-11-11 dedupe script starts functioning: on SSE reconnect (or initial page load after page-load backfill renders), any `log_line` frame whose `event.lastEventId` is at or below the server-rendered max is dropped by `evt.preventDefault()`. The joint contract (Plan 11-09 server-side `last_log_id` + Plan 11-12 template render + Plan 11-11 client-side guard) is now fully landed — only the template attribute line is missing.
- **Plan 11-14 (client-side dedupe + initial render) unblocked.** Plan 11-14's dedupe path plugs directly into the `logLines.dataset.maxId` cursor that Plan 11-11 advances on every accepted frame, so 11-14's reconnect semantics are already supported on the cursor side.
- **D-09 (dedupe loop) fully closed end-to-end on the server + client halves.** Server emits `id:` per frame (11-08), renders `last_log_id` through handler (11-09), renders `data-max-id` (11-12 pending), client guards on both (11-11).
- **D-10 (graceful live-to-static terminal) fully closed.** Server broadcasts sentinel (11-10), SSE handler translates to `event: run_finished` (11-10), client swaps to static partial via `htmx.ajax` (11-11). `sse:run_complete` fallback preserved for abrupt-disconnect paths.

## Self-Check: PASSED

**Files verified on disk:**
- `templates/pages/run_detail.html` — FOUND; `htmx:sseBeforeMessage` listener present at L124; `sse:run_finished` listener present at L137; `sse:run_complete` listener preserved at L148; `dataset.maxId` referenced at L127 and L129; `preventDefault()` called at L128; `htmx.ajax` appears twice (graceful + fallback paths) at L139 and L149.
- `tests/v11_log_dedupe_contract.rs` — FOUND; 240 lines; 2 `#[ignore]` remaining (Plan-11-12-owned stubs); four `#[tokio::test]` functions with real bodies (`script_references_dataset_maxid`, `listens_for_run_finished`, `script_references_htmx_sse_hook`, `v11_dedupe_contract`); local `build_test_app()` helper at the top.
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-11-SUMMARY.md` — FOUND (this file).

**Commits verified (present in `git log 71eafae..HEAD`):**
- `134cecb` — FOUND (`feat(11-11): inline dedupe + run_finished listener in run_detail`)
- `07d82bb` — FOUND (`test(11-11): replace Wave-0 stubs with dedupe contract + HTML assertions`)

**Build gates verified:**
- `cargo check` — CLEAN.
- `cargo build` (dev) — CLEAN.
- `cargo clippy --all-targets -- -D warnings` — CLEAN.
- `cargo fmt --check` — CLEAN.
- `cargo test --lib` — PASS (`173 passed; 0 failed`).
- `cargo test --test v11_log_dedupe_contract` — PASS (`4 passed; 0 failed; 2 ignored`).
- `cargo test --test v11_run_detail_page_load` — PASS (`3 passed; 0 failed; 1 ignored`).
- `cargo test --test v11_sse_log_stream` — PASS (`2 passed; 0 failed; 0 ignored`).
- `cargo test --test v11_sse_terminal_event` — PASS (`2 passed; 0 failed; 0 ignored`).

**Plan success criteria verified:**
1. Inline dedupe handler references `dataset.maxId` + `preventDefault` + `htmx:sseBeforeMessage` — PASS (all three present in rendered body; `script_references_dataset_maxid` + `script_references_htmx_sse_hook` both assert).
2. `sse:run_finished` listener calls `htmx.ajax` to swap to static partial — PASS (`listens_for_run_finished` asserts both strings present).
3. Legacy `sse:run_complete` listener preserved — PASS (grep of `templates/pages/run_detail.html` matches at L148).
4. Contract tests pass including the new autonomous `v11_dedupe_contract` unit test — PASS (all four Plan-11-11 tests pass; `v11_dedupe_contract` locks the `id > max -> accept` rule end-to-end).
5. Plan is `autonomous: true` — no human checkpoint; browser UAT consolidated into Plan 11-12 Task 5 — PASS (no checkpoint encountered; verification fully automated).

---
*Phase: 11-per-job-run-numbers-log-ux-fixes*
*Completed: 2026-04-17*
