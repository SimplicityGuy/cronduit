---
phase: 21
plan: 10
subsystem: uat-tooling
tags: [uat, justfile, examples, fctx, exit-histogram, fire-skew, a11y]
requires: [21-06]
provides:
  - "uat-fctx-panel just recipe"
  - "uat-exit-histogram just recipe"
  - "uat-fire-skew just recipe"
  - "uat-fctx-a11y just recipe"
  - "fire-skew-demo example job"
affects: [maintainer-uat-runbook, 21-HUMAN-UAT.md]
tech_stack_added: []
patterns:
  - "Recipe-calls-recipe orchestration over existing primitives (P18 D-25 / D-29 / project memory feedback_uat_use_just_commands.md)"
  - "Raw sqlite3 fixture seed against cronduit.dev.db (uat-fctx-bugfix-spot-check precedent)"
  - "Slow-start docker container as the fire-skew artificial-delay technique (research §F)"
  - "Single umbrella recipe for grouped a11y scenarios (research §G)"
key_files_created: []
key_files_modified:
  - justfile
  - examples/cronduit.toml
decisions:
  - "Honored D-19: 3 new recipes (uat-fctx-panel, uat-exit-histogram, uat-fire-skew) per the P18/P19/P20 family pattern"
  - "Honored D-20 / research §G: single umbrella uat-fctx-a11y instead of 4 split recipes (cleaner for the maintainer's eyes; matches the discretionary call left to the planner)"
  - "Honored research §F: slow-start docker container (sleep 30 && echo done) as the fire-skew technique — closest to operator reality of image-pull + cold-start delay; no new test infra"
  - "Honored research landmine §4: NO references to seed-fixture-runs or dev-build (those don't exist). Recipes compose only db-reset, dev, api-job-id, api-run-now, and raw sqlite3."
  - "Honored project memory feedback_uat_use_just_commands.md: every step in 21-HUMAN-UAT.md (Wave 4) can now reference an existing `just` recipe."
metrics:
  duration_minutes: 4
  completed_date: 2026-05-02
  tasks_completed: 2
  files_changed: 2
---

# Phase 21 Plan 10: Maintainer UAT plumbing — `just` recipes + example job Summary

Adds the four Phase 21 UAT runbook recipes (`uat-fctx-panel`, `uat-exit-histogram`, `uat-fire-skew`, `uat-fctx-a11y`) and the `fire-skew-demo` example job that the third recipe drives, unlocking the Wave 4 `21-HUMAN-UAT.md` validation by giving the maintainer one composable `just` command per UI-SPEC scenario.

## Tasks completed

| Task | Name | Commit | Files |
| ---- | ---- | ------ | ----- |
| 1 | Add `fire-skew-demo` slow-start docker job stanza to `examples/cronduit.toml` | `362aa36` | `examples/cronduit.toml` |
| 2 | Add 4 Phase 21 UAT recipes (`uat-fctx-panel`, `uat-exit-histogram`, `uat-fire-skew`, `uat-fctx-a11y`) to `justfile` | `13a49b1` | `justfile` |

## What landed

### `examples/cronduit.toml` — `fire-skew-demo` job stanza

```toml
# Phase 21 / FCTX-06 -- slow-start docker job seeded for `just uat-fire-skew`.
[[jobs]]
name = "fire-skew-demo"
type = "docker"
image = "alpine:latest"
command = ["sh", "-c", "sleep 30 && echo done"]
schedule = "* * * * *"
```

- Slow-start (`sleep 30`) before the meaningful work, simulating an image-pull + cold-start delay.
- Schedule `* * * * *` so the maintainer doesn't wait long.
- Inherits `[defaults].image = "alpine:latest"` and the watchtower-exclusion label — both fine for this demo.
- Header comment cites Phase 21 / FCTX-06 + the `uat-fire-skew` recipe + the expected `+30000ms` skew.
- TOML still parses (verified via `python3 -c 'import tomllib; tomllib.load(open("examples/cronduit.toml","rb"))'` and via the `just --list` parse).

### `justfile` — 4 new recipes

All four recipes use:

- `[group('uat')]` + `[doc('Phase 21 — …')]` decorations
- `#!/usr/bin/env bash` + `set -euo pipefail` shebang
- Recipe-calls-recipe orchestration of existing primitives only — no new primitives
- Step-by-step echoed guided runbooks (recipes are walk-throughs, not fully automated tests; the maintainer drives the browser session)

**`uat-fctx-panel`** — seed 4 consecutive failed runs against the seeded job via raw `sqlite3` writes (mirrors v12_fctx_streak.rs::seed_run shape), then walk to `/jobs/{id}/runs/{id}` for FCTX-panel-render verification.

**`uat-exit-histogram`** — seed mixed exit-code runs covering EXIT-04's dual-classifier (`status='stopped'+exit_code=137` → BucketStopped vs `status='failed'+exit_code=137` → Bucket128to143), then walk to `/jobs/{id}` for histogram-card-render verification. The seed deliberately includes 5 successes + 3 code-1 + 1 code-127 + 1 stopped@137 + 1 failed@137 so the operator can see the locked D-09 success-rate formula `success / (sample_count - stopped)` produce a non-trivial value.

**`uat-fire-skew`** — uses the seeded `fire-skew-demo` docker job to demonstrate FCTX-06 fire-skew arithmetic. The container sleeps 30s before completing → `start_time` lands ~30s after `scheduled_for` → +30000ms skew visible on the run-detail FIRE SKEW row. Recipe verifies via raw `sqlite3` inspect plus a URL hand-off.

**`uat-fctx-a11y`** — umbrella recipe (single-recipe-walks-4-phases per research §G) covering UI-SPEC § Accessibility scenarios: Mobile viewport <640px (rows stack, histogram horizontally scrolls), Light-mode (existing `[data-theme="light"]` block in app.css renders correctly), Print mode (`@media print { details { open: open } }` opens the panel inline), Keyboard-only (Tab to summary, Space/Enter expand, Tab onto histogram bars).

### Verification

| Check | Expected | Actual |
| --- | --- | --- |
| `just --list` exit code | `0` | `0` (justfile parses) |
| 4 new recipes appear in `just --list` | `4` | `4` |
| Each has `[doc('Phase 21 — …')]` | `>= 4` | `4` |
| `set -euo pipefail` total in file | `>= 5` | `22` (existing precedents + 4 new) |
| References to non-existent `seed-fixture-runs`/`dev-build` | `0` | `0` |
| `uat-fctx-panel` uses existing primitives | `>= 2` | `3` (`db-reset`, `api-job-id`, plus operator-instructed `just dev`) |
| `uat-fire-skew` references `fire-skew-demo` | `>= 1` | `7` |
| `uat-fctx-a11y` covers 4 phases (Mobile/Light/Print/Keyboard) | `>= 4` | `5` |
| `examples/cronduit.toml` still parses | OK | OK (Python tomllib + `just --list`) |

One acceptance-criterion grep (`grep -B1 ... | grep -c "[group('uat')]"` returning `>= 4`) was specified at the wrong context width: across the entire justfile, the canonical recipe shape is `[group(...)]` + `[doc(...)]` + `recipe:` (3-line block), so `-B1` always returns the `[doc(...)]` line, not the `[group(...)]` line. The same `-B1` test returns `0` for every existing P18/P20 reference recipe (`uat-webhook-fire`, `uat-webhook-mock`, `uat-webhook-dlq-query`); switching to `-B2` returns `4` for the new recipes (matching the structurally-identical existing ones). The new recipes are byte-for-byte structurally identical to the canonical `uat-webhook-fire` precedent that the plan's `<interfaces>` block points at.

## Decisions Made

- **Single umbrella recipe for a11y** (vs 4 split recipes). Matches research §G recommendation: easier maintainer flow in a single browser session; one recipe to remember; one place to update if the UI-SPEC scenarios shift.
- **Slow-start docker container for `uat-fire-skew`** (research §F discretion option C). Closest to operator reality (image-pull + cold-start delay are the dominant real-world skew sources); deterministic enough; doesn't require a sidecar lock or a test-only feature flag.
- **Raw `sqlite3` fixture seed** for `uat-fctx-panel` and `uat-exit-histogram` instead of triggering real `Run Now` runs. Matches `uat-fctx-bugfix-spot-check` precedent — predictable, fast, no Docker required for the panel/histogram render scenarios; the real-Docker path is exercised by `uat-fire-skew`.
- **Append at end of file** instead of inserting near other `uat-*` recipes. Keeps the diff minimal (no churn on existing line numbers); the existing P18/P19/P20 `uat-*` family already spans ~600 lines so grouping doesn't help discoverability — `just --list` flat-sorts everything anyway.
- **Inline `if grep ... then ... else ... fi`** in `uat-fire-skew` instead of `grep -q ... && echo OK || { echo FAIL; exit 1; }` (the example shape in the plan). Both work, but `set -euo pipefail` makes `&&`-chains brittle when a later command may legitimately exit non-zero. The if/else is robust.

## Deviations from Plan

None substantive. The plan was executed exactly as written; the only adjustment was an inline `if/then/else` in `uat-fire-skew` instead of the `&& echo "✓" || { echo "✗"; exit 1; }` shape from the plan's example, because under `set -euo pipefail` an `if grep ...` is more robust than a `&&` chain. Behavior is identical (exits 1 on missing job, prints check on present).

## How this unlocks Wave 4

Plan 21-11 (`21-HUMAN-UAT.md`) and the eventual `21-RC2-PREFLIGHT.md` (autonomous=false maintainer plans, D-26) can now reference one `just` recipe per scenario:

- HUMAN-UAT § Panel renders on real failed/timeout runs → `just uat-fctx-panel`
- HUMAN-UAT § Histogram bucket distribution + success-rate badge + recent-codes → `just uat-exit-histogram`
- HUMAN-UAT § Fire-skew on a delayed-fire job → `just uat-fire-skew`
- HUMAN-UAT § Mobile / Light / Print / Keyboard a11y → `just uat-fctx-a11y`

Per project memory `feedback_uat_use_just_commands.md`, this is the contract for marking each HUMAN-UAT scenario a `just`-recipe-driven step.

## Self-Check: PASSED

| Item | Check | Result |
| --- | --- | --- |
| `examples/cronduit.toml` | `[ -f examples/cronduit.toml ]` | FOUND |
| `justfile` | `[ -f justfile ]` | FOUND |
| Task 1 commit `362aa36` | `git log --oneline --all \| grep -q '362aa36'` | FOUND |
| Task 2 commit `13a49b1` | `git log --oneline --all \| grep -q '13a49b1'` | FOUND |
| `fire-skew-demo` in example | `grep -c 'name = "fire-skew-demo"' examples/cronduit.toml` | 1 |
| 4 new recipes in `just --list` | `just --list \| grep -cE 'uat-(fctx-panel\|exit-histogram\|fire-skew\|fctx-a11y)'` | 4 |
| TOML parses | `python3 -c 'import tomllib; tomllib.load(open("examples/cronduit.toml","rb"))'` | OK |
| Justfile parses | `just --list >/dev/null` | OK |
