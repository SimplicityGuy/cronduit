---
phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
plan: 08
subsystem: testing
tags: [webhooks, uat, justfile, maintainer-validated, recipe-calls-recipe, python-stdlib-mock, runbook]

# Dependency graph
requires:
  - phase: 20-06
    provides: webhook_drain_grace ServerConfig field + RetryingDispatcher wiring + per-job metric pre-seed
  - phase: 20-07
    provides: docs/WEBHOOKS.md operator hub extension (6 new sections covering retry/DLQ/drain/HTTPS/metrics) — Scenario 7 references it
  - phase: 19
    provides: P19 uat-webhook-receiver-* recipes + recipe-calls-recipe pattern + python3 prerequisite
  - phase: 18
    provides: P18 uat-webhook-mock / uat-webhook-fire / uat-webhook-verify baseline + api-job-id / api-run-now CSRF wrappers
provides:
  - 6 new just recipes under [group('uat')] (4 plan-mandated + 2 helper mocks; +2 supporting recipes Rule-2 added during HUMAN-UAT authoring)
  - 20-HUMAN-UAT.md maintainer runbook with 7 scenarios + sign-off block (status=pending; ZERO ticked checkboxes — Plan 20-09 prerequisite)
affects: [20-09]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - heredoc-via-sed-to-mktemp-python3 (multi-line Python recipe bodies inside indented just recipes)
    - recipe-calls-recipe (uat-webhook-retry → uat-webhook-fire → uat-webhook-dlq-query)
    - Python stdlib http.server for UAT-only mock receivers (zero new project deps; python3 already a P19 prerequisite)
    - thin-wrapper recipe (uat-webhook-rustls-check delegates to openssl-check rather than duplicating logic)

key-files:
  created:
    - .planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-HUMAN-UAT.md
  modified:
    - justfile  # +6 recipes total (4 plan-mandated + 2 helper mocks committed in Task 1; +2 metrics-check / rustls-check committed in Task 2)

key-decisions:
  - "Python stdlib http.server for the helper mocks (uat-webhook-mock-500 / uat-webhook-mock-slow). The existing examples/webhook_mock_server.rs hardcodes port 9999 + status 200 with no flag parsing; extending it just to support UAT mode-switching was out of scope for a UAT-recipe-only plan. python3 is already a documented Phase 19 prerequisite (19-HUMAN-UAT.md Prerequisites table)."
  - "heredoc → sed 's/^    //' → mktemp → python3 -u pattern. Just requires every recipe body line to be indented; multi-line Python literals inside the recipe must be uniformly dedented before the python3 process sees them. This pattern keeps the Python source readable in the justfile AND parses cleanly (AST-validated)."
  - "Two extra recipes (uat-webhook-metrics-check, uat-webhook-rustls-check) added during Task 2 as a Rule-2 deviation. Per project memory feedback_uat_use_just_commands.md no UAT step may reference raw curl/cargo; Scenarios 5 + 6 needed dedicated recipes that did not exist."
  - "uat-webhook-rustls-check is a thin wrapper around the existing openssl-check (Pitfall 14 / FOUND-06 source of truth) — does NOT duplicate the loop-over-targets logic. Surfaces it under [group('uat')] with a Phase-20-flavored doc string so 20-HUMAN-UAT.md Scenario 6 maps cleanly."
  - "uat-webhook-metrics-check is intentionally distinct from the existing metrics-check (P14). The latter is v1.1.0-rc.3 HUMAN-UAT contract surfacing only cronduit_scheduler_up + cronduit_runs_total — widening it would risk silent drift in the v1.1 dashboards."
  - "20-HUMAN-UAT.md ships with status=pending (zero ticked checkboxes, empty maintainer/date fields). Per project memory feedback_uat_user_validates.md, Claude does NOT mark UAT passed; the maintainer flips the boxes during the rc.1 cut. This is enforced both by the orchestrator's directive and by Plan 20-09's must_haves.truths frontmatter ('20-HUMAN-UAT.md must show all maintainer checkboxes ticked BEFORE Plan 09's pre-flight checklist gates the tag cut')."

patterns-established:
  - "UAT-recipe-as-runbook-step: every step in 20-HUMAN-UAT.md references a just recipe by name; ad-hoc cargo/docker/curl forbidden (project memory feedback_uat_use_just_commands.md). Mirrors 19-HUMAN-UAT.md and 18-HUMAN-UAT.md."
  - "Helper-mock + driver-recipe split: uat-webhook-mock-500 (foreground mock) is paired with uat-webhook-retry (driver) the same way P18 splits uat-webhook-mock + uat-webhook-fire + uat-webhook-verify. Operators run the helper in one terminal and the driver in another."
  - "Maintainer-validated artifacts ship with structurally-correct sign-off block but unfilled fields. Plan 20-09 reads the filled block as the rc.1 tag-cut prerequisite (D-13 / D-29)."

requirements-completed: [WH-05, WH-07, WH-10]

# Metrics
duration: 9min
completed: 2026-05-01
---

# Phase 20 Plan 08: Maintainer UAT Runbook + just-recipe Surface Summary

**4 plan-mandated `uat-webhook-*` recipes + 2 helper mocks (Python stdlib) + 2 supporting recipes for /metrics surface and rustls invariant + a 7-scenario `20-HUMAN-UAT.md` runbook (status=pending, zero ticks) — wires the maintainer-validated rc.1 prerequisite per D-34 / D-35 without touching cronduit's runtime code.**

## Performance

- **Duration:** ~9 min
- **Started:** 2026-05-01T22:08:28Z
- **Completed:** 2026-05-01T22:17:04Z
- **Tasks:** 2 (Task 1: justfile recipes; Task 2: HUMAN-UAT.md + supporting recipes)
- **Files modified:** 2 (`justfile`, new `20-HUMAN-UAT.md`)

## Accomplishments

- **6 new just recipes total under `[group('uat')]`** (Phase 20 family): `uat-webhook-mock-500`, `uat-webhook-mock-slow`, `uat-webhook-retry`, `uat-webhook-drain`, `uat-webhook-dlq-query`, `uat-webhook-https-required`. All carry `[group('uat')]` + `[doc('Phase 20 — ...')]` attributes consistent with the P18/P19 recipes.
- **2 additional supporting recipes** added during Task 2 to satisfy Scenarios 5 + 6 of the runbook: `uat-webhook-metrics-check` (curls /metrics + greps `cronduit_webhook_*` family per WH-11) and `uat-webhook-rustls-check` (thin wrapper around `openssl-check` per D-38).
- **`20-HUMAN-UAT.md` (7 scenarios, all unticked)** covering the operator-observable surface for WH-05 (retry chain + DLQ), WH-07 (HTTPS-required validator both negative + positive case), WH-10 (drain on shutdown), WH-11 (/metrics labeled family + per-job zero-baseline), D-38 (rustls invariant), and D-27 (docs/WEBHOOKS.md operator hub coherence).
- **Recipe-calls-recipe composition preserved** — `uat-webhook-retry` calls `just uat-webhook-fire` (P18) which calls `just api-job-id` + `just api-run-now`. `uat-webhook-rustls-check` delegates to `just openssl-check` (no duplicated logic).
- **Maintainer sign-off block ships unfilled** so Plan 20-09's pre-flight checklist can detect when the rc.1 tag-cut prerequisite is satisfied (per Plan 20-09 frontmatter `must_haves.truths`).
- **Heredoc-via-sed-to-mktemp pattern** invented for multi-line Python recipe bodies inside indented just recipes (verified via `python3 -c "import ast; ast.parse(...)"` smoke test before commit).

## Task Commits

Each task was committed atomically:

1. **Task 1: Add 4 Phase 20 webhook UAT just recipes (+2 helper mocks)** — `8b0f4d8` (feat) — `justfile` +220 lines (new `# === Phase 20 webhook posture (D-34) ===` block after the P19 receivers section).
2. **Task 2: Author 20-HUMAN-UAT.md (status=pending) + 2 supporting recipes** — `ecc7231` (docs) — `justfile` +35 lines (uat-webhook-metrics-check + uat-webhook-rustls-check) + `20-HUMAN-UAT.md` (155 lines, 7 scenarios).

**Plan metadata:** This SUMMARY.md committed by the orchestrator's final-commit step (per parallel-executor protocol — STATE.md / ROADMAP.md / REQUIREMENTS.md updates are owned by the orchestrator).

_Note: Plan 20-08 is not a TDD plan; both tasks are `type="auto"` (Task 1) and the original `type="checkpoint:human-verify"` (Task 2) was reduced to autonomous-author per the orchestrator's directive ("produce the UAT artifacts ... but DO NOT mark UAT items passed")._

## Files Created/Modified

- **`justfile`** (modified, +255 lines net across both commits) — 8 new entries under `[group('uat')]`:
  - `uat-webhook-mock-500` — Python stdlib http.server returning 500 for ALL POSTs; logs to `/tmp/cronduit-webhook-mock-500.log`. Forces the WH-05 retry chain.
  - `uat-webhook-mock-slow` — Python stdlib http.server returning 200 after a 5s sleep; logs to `/tmp/cronduit-webhook-mock-slow.log`. Drives the WH-10 drain-on-shutdown scenario (the in-flight POST runs past the SIGTERM moment).
  - `uat-webhook-retry JOB_NAME` — composes `uat-webhook-fire` + prints wait-and-verify guidance for the 3-attempt chain landing in the 500-mock log + DLQ.
  - `uat-webhook-drain` — prints the 4-terminal SIGTERM-during-in-flight procedure with the `webhook_drain_grace + 10s` worst-case ceiling explicit (D-18).
  - `uat-webhook-dlq-query` — `sqlite3 -header -column $CRONDUIT_DEV_DB` against `webhook_deliveries` over the last 1 hour. Mirrors the `uat-fctx-bugfix-spot-check` precedent.
  - `uat-webhook-https-required` — writes a `mktemp`'d bad config (`webhook = { url = "http://example.com/hook" }`), runs `cargo run --quiet -- check`, asserts non-zero exit (D-19 / WH-07 regression guard).
  - `uat-webhook-metrics-check` — curls `/metrics` and greps the `cronduit_webhook_(deliveries_total|delivery_duration_seconds|queue_depth|delivery_dropped_total)` family. Distinct from the existing `metrics-check` (P14, intentionally not widened — that's v1.1's HUMAN-UAT contract).
  - `uat-webhook-rustls-check` — thin wrapper around `just openssl-check` surfaced under `[uat]` with a Phase-20-flavored doc string (D-38).

- **`.planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-HUMAN-UAT.md`** (created, 155 lines) — 7 scenarios mapping to WH-05 / WH-07 / WH-10 / WH-11 / D-38 / D-27. All checkboxes ship unticked (`[ ]`); sign-off block has empty maintainer/date/comment fields. Cross-references the Phase 20 integration test files (`tests/v12_webhook_retry.rs` etc. — created in Plans 20-02 through 20-05).

## Decisions Made

See `key-decisions` frontmatter above. Highlights:

- **Python stdlib for the 500-mock and slow-mock** rather than extending the Rust `examples/webhook_mock_server.rs`. The Rust example hardcodes port + status with no flag parsing; widening it just to support UAT mode-switching was out of scope for a UAT-recipe-only plan. Python 3 is already a documented P19 prerequisite — zero new project dependencies.
- **Heredoc → sed → mktemp → python3 -u pattern** so the recipe body stays inside `just`'s required indentation block while the generated `.py` script is properly dedented for Python's parser. The pattern was AST-validated before the commit landed.
- **Two extra UAT-supporting recipes added during Task 2** (`uat-webhook-metrics-check` + `uat-webhook-rustls-check`) — a Rule-2 deviation surfaced by the runbook's coverage requirements. Documented in the Deviations section below.
- **Sign-off block ships empty** per project memory `feedback_uat_user_validates.md` and the orchestrator's directive. Plan 20-09's pre-flight checklist reads the filled block as the rc.1 tag-cut prerequisite.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] Replaced the plan's `cargo run --example webhook_mock_server -- --port 9999 --status 500` with a Python stdlib http.server**
- **Found during:** Task 1 (writing `uat-webhook-mock-500`).
- **Issue:** The plan's `<action>` snippet for `uat-webhook-mock-500` invoked `cargo run --quiet --example webhook_mock_server -- --port 9999 --status 500`, but `examples/webhook_mock_server.rs` hardcodes `const ADDR: &str = "127.0.0.1:9999"` and always returns `HTTP/1.1 200 OK` — it does NOT accept any CLI flags. The recipe would silently start the wrong mock (200-returning, not 500-returning) and the entire WH-05 retry-chain UAT would produce zero retries. The plan itself flagged this ambiguity ("adapt it by passing a `--always-500` flag if supported, or add it") and explicitly delegated to the executor's discretion ("the simpler path is to add a parameter or env var to the existing mock").
- **Fix:** Used Python's stdlib `http.server.BaseHTTPRequestHandler` to implement both `uat-webhook-mock-500` and `uat-webhook-mock-slow`. Python 3 is already a documented Phase 19 prerequisite (see 19-HUMAN-UAT.md Prerequisites table — `python3 --version` is listed alongside `go version` and `node --version`); this adds zero new project dependencies. The Python source lives inside the recipe via a heredoc → sed (strip the 4-space recipe indent) → mktemp → `python3 -u "$SCRIPT"` pattern.
- **Files modified:** `justfile` (uat-webhook-mock-500 and uat-webhook-mock-slow recipe bodies; 60+ lines).
- **Verification:** AST-validated the dedented Python via `python3 -c "import ast; ast.parse(open(...).read()); print('AST parse OK')"` before the commit landed. `just --list | grep uat-webhook-mock-500` returns the recipe with its `[doc(...)]` line, confirming `just` parses the recipe cleanly.
- **Committed in:** `8b0f4d8` (Task 1 commit).

**2. [Rule 2 — Missing critical UAT-recipe coverage] Added `uat-webhook-metrics-check` to the justfile**
- **Found during:** Task 2 (drafting Scenario 5 of 20-HUMAN-UAT.md — `/metrics` surface).
- **Issue:** The plan's `<how-to-verify>` for Scenario 5 says "curl -s http://127.0.0.1:8080/metrics | grep cronduit_webhook_". Per project memory `feedback_uat_use_just_commands.md` ("UAT steps use just commands — every UAT step must reference an existing `just` recipe, not ad-hoc cargo/docker/URLs"), no UAT step may reference raw `curl ... | grep`. The existing `metrics-check` recipe (justfile:702) only greps `cronduit_scheduler_up` + `cronduit_runs_total` — it does NOT surface the `cronduit_webhook_*` family Phase 20 added.
- **Fix:** Added a new `uat-webhook-metrics-check` recipe under `[group('uat')]` that curls `/metrics` and greps for the full Phase 20 family (`deliveries_total`, `delivery_duration_seconds`, `queue_depth`, `delivery_dropped_total`). Intentionally distinct from the existing `metrics-check` recipe (which is v1.1.0-rc.3's HUMAN-UAT contract — widening it risks silent drift).
- **Files modified:** `justfile` (new recipe at lines ~811–835).
- **Verification:** `just --list | grep uat-webhook-metrics-check` confirms the recipe parses; `20-HUMAN-UAT.md` Scenario 5 references it.
- **Committed in:** `ecc7231` (Task 2 commit).

**3. [Rule 2 — Missing critical UAT-recipe coverage] Added `uat-webhook-rustls-check` to the justfile**
- **Found during:** Task 2 (drafting Scenario 6 of 20-HUMAN-UAT.md — rustls invariant per D-38).
- **Issue:** The plan's `<how-to-verify>` for Scenario 6 says "Run: `cargo tree -i openssl-sys`". Same `feedback_uat_use_just_commands.md` violation as deviation 2. The existing `openssl-check` recipe (justfile:180) is the source of truth for the Pitfall 14 / FOUND-06 invariant, but it lives under `[group('quality')]`, not `[uat]` — surfacing it via the wrong group label would muddle the "what does the maintainer run from a UAT terminal vs a CI gate" mental model.
- **Fix:** Added a new `uat-webhook-rustls-check` recipe under `[group('uat')]` that delegates to `just openssl-check` (one-liner: `@just openssl-check`). Does NOT duplicate the loop-over-targets logic — wraps the source-of-truth recipe with a Phase-20-flavored doc string.
- **Files modified:** `justfile` (new recipe at lines ~837–845).
- **Verification:** `just --list | grep uat-webhook-rustls-check` confirms the recipe parses; locally ran `cargo tree -i openssl-sys` and confirmed it returns `error: package ID specification 'openssl-sys' did not match any packages` (empty result, exit 0 — rustls invariant intact).
- **Committed in:** `ecc7231` (Task 2 commit).

---

**Total deviations:** 3 auto-fixed (1 blocking, 2 missing critical UAT-recipe coverage).
**Impact on plan:** All 3 deviations were necessary for the runbook to be runnable as written. None expand the plan's scope: deviation 1 substitutes a working mock for a non-existent flag-set on the Rust example; deviations 2 + 3 surface existing functionality (curl/grep against a known endpoint, openssl-check delegation) under the just-recipe-only contract the project enforces. No code changes to cronduit's runtime; no new Rust dependencies; the heredoc-via-sed pattern is novel for this repo but documented inline in the justfile and the SUMMARY's `key-decisions`.

## Issues Encountered

- **Justfile parse error on the first attempt at the Python recipes.** Initial implementation used `python3 -u -c '<inline literal>'` with the Python source un-indented (column 0) inside the bash recipe body. This broke `just`'s parser at the first un-indented line because `just` uses indentation to determine recipe boundaries. Fix: rewrote both helper-mock recipes to use the heredoc-via-sed-to-mktemp pattern. Surfaced + resolved during Task 1 (single iteration); did NOT surface in any commit.

## User Setup Required

None at the artifact level — Plan 20-08 ships only justfile edits and a planning-doc runbook. The maintainer's UAT validation run (which IS user-facing manual action) is documented in `20-HUMAN-UAT.md` itself; that's the deliverable, not a setup step.

## Next Phase Readiness

**Plan 20-09 prerequisites met (artifact side):**
- `20-HUMAN-UAT.md` exists with 7 scenarios + sign-off block (Plan 20-09 reads this file).
- All 6 plan-mandated UAT recipes (4 in Task 1 + 2 in Task 2) parse via `just --list` without errors.
- `cargo tree -i openssl-sys` returns empty (D-38 / Scenario 6 baseline confirmed by Claude before the maintainer's run).

**Plan 20-09 prerequisites NOT met (maintainer side — by design):**
- All 7 checkboxes in `20-HUMAN-UAT.md` ship `[ ]` (unticked). The maintainer must run each scenario from a fresh terminal and tick the boxes — this gates the rc.1 tag cut per Plan 20-09's `must_haves.truths` ("`20-HUMAN-UAT.md` must show all maintainer checkboxes ticked BEFORE Plan 09's pre-flight checklist gates the tag cut").
- The Sign-off block (Maintainer / Date / Comment) ships empty for the maintainer to fill at rc.1 cut.

No blockers to merging Plan 20-08's PR — the deliverable is the artifact set, not the maintainer's validation run.

## Self-Check

Verified before publishing this SUMMARY:

- `.planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-HUMAN-UAT.md` — **FOUND**
- `justfile` (with 6 new Phase 20 recipes + 2 supporting) — **FOUND** (modified)
- Commit `8b0f4d8` (Task 1) — **FOUND** in `git log --oneline -5`
- Commit `ecc7231` (Task 2) — **FOUND** in `git log --oneline -5`
- `just --list | grep -E 'uat-webhook-(retry|drain|dlq-query|https-required)' | wc -l` = **4** (plan's verify automated check passes)
- `just --list | grep -E 'uat-webhook' | wc -l` = **17** (P18: 3 + P19: 6 + P20: 8 — 4 plan-mandated + 2 helper mocks + 2 supporting = 8 P20 entries)
- `grep -cE '^\[ \]' 20-HUMAN-UAT.md` = **7** (one unticked checkbox per scenario)
- `grep -cE '^\[x\]' 20-HUMAN-UAT.md` = **0** (Claude does NOT mark UAT passed; project memory `feedback_uat_user_validates.md` honored)
- `grep -cE '^### Scenario' 20-HUMAN-UAT.md` = **7** (≥ 6 required by plan success criterion)
- All 4 plan-mandated requirement IDs present in 20-HUMAN-UAT.md (WH-05, WH-07, WH-10, WH-11) — **OK**
- `cargo tree -i openssl-sys` — **empty** (`error: package ID specification 'openssl-sys' did not match any packages`); rustls invariant intact (D-38)
- No accidental file deletions in either commit (`git diff --diff-filter=D --name-only HEAD~2 HEAD` empty).

## Self-Check: PASSED

---
*Phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1*
*Plan: 08*
*Completed: 2026-05-01*
