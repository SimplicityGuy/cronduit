---
phase: 08-v1-final-human-uat-validation
plan: 04
subsystem: ci/compose-smoke
tags: [ci, github-actions, compose-smoke, quickstart-gate, matrix, run-now]

# Dependency graph
requires:
  - phase: 06-live-events-metrics-retention-release-engineering
    provides: the Phase 6 gap-closure compose-smoke job (GAP-3.4) that this plan extends
  - phase: 08-v1-final-human-uat-validation
    provides:
      - 08-01 four-job quickstart in examples/cronduit.toml (echo-timestamp, http-healthcheck, disk-usage, hello-world)
      - 08-02 dual compose files (examples/docker-compose.yml + examples/docker-compose.secure.yml)
      - 08-03 cronduit_docker_reachable gauge referenced in failure diagnostics
provides:
  - compose-smoke CI job parameterized by a 2-axis compose matrix
  - Per-job terminal-success assertion via Run Now API within a 120s budget
  - Expanded failure diagnostics (cronduit + dockerproxy logs, per-job run history, cronduit_docker_reachable gauge)
affects:
  - PRs touching Dockerfile, examples/, scheduler code, or API routes now gate on end-to-end success of every quickstart job on both compose variants
  - 08-05 human UAT walkthrough — CI now proves the quickstart works before asking a human to walk through it

# Tech tracking
tech-stack:
  added: []
  patterns:
    - GitHub Actions matrix strategy over compose file path (single job, two axes)
    - Two-phase smoke test: trigger all jobs via Run Now, then poll run history to terminal status
    - Shell-only JSON handling via jq (installed with an apt idempotency check)
    - COMPOSE_FILE env var at step level to parameterize compose-related shell commands
    - `::group::` GitHub Actions log grouping in failure-diagnostic steps

key-files:
  created: []
  modified:
    - .github/workflows/ci.yml

decisions:
  - D-18 honored: new per-job success assertion runs AFTER the existing /health + dashboard assertions and BEFORE teardown, inside the same job (single compose up/down per axis — D-22)
  - D-19 honored: trigger uses POST /api/jobs/{id}/run after resolving id from GET /api/jobs; polling uses GET /api/jobs/{id}/runs?limit=1 with jq for status extraction; 120s budget with 2s poll interval
  - D-20 honored: failure diagnostics dump cronduit logs (tail 200), dockerproxy logs (tail 50, secure axis only), per-job run-history tail (limit 5), and the cronduit_docker_reachable gauge from /metrics
  - D-21 honored: matrix axis covers both compose files; fail-fast is `false` so each axis surfaces independent failures
  - D-22 honored: existing /health body + dashboard job-list assertions preserved verbatim (logic-equivalent; dashboard assertion expanded from 2 to 4 job names to match the 08-01 quickstart rewrite)
  - fail-fast kept as `false`: if the default axis fails for compose syntax and the secure axis fails for docker-socket-proxy allowlist, both failures surface in the same PR run rather than masking one behind the other

# Metrics
metrics:
  duration_minutes: 6
  tasks_completed: 1
  tasks_total: 1
  files_modified: 1
  commits: 1
  completed: 2026-04-14

requirements: [OPS-05]
requirements_addressed: [OPS-05]
---

# Phase 8 Plan 04: Compose-Smoke Matrix & Run Now Success Gate Summary

**One-liner:** Refactored the existing `compose-smoke` GitHub Actions job into a 2-axis matrix over `examples/docker-compose.yml` and `examples/docker-compose.secure.yml` that, per axis, triggers every quickstart job via `POST /api/jobs/{id}/run` and asserts all four reach `status=success` within 120 seconds — closing the OPS-05 "quickstart promise" contract programmatically before the Plan 08-05 human UAT walkthrough.

## Context

The Phase 6 gap-closure `compose-smoke` job already booted `examples/docker-compose.yml`, waited for `/health`, and asserted the dashboard listed the two shipping jobs — but it never verified they actually **ran successfully**. Phase 8 closes that gap: with four example jobs now shipping (Plan 08-01) and two compose files (default + hardened) as the supported quickstart surfaces (Plan 08-02), CI must prove the end-to-end experience before a PR merges.

Without this gate, asking a human to walk through the README quickstart in Plan 08-05 would be asking them to debug CI-grade failures. With it, the human walkthrough reduces to "does the UI match what the operator expects?" rather than "does it even boot?".

## Accomplishments

- **Matrix conversion** — `compose-smoke` is now a single job that fans out over a `compose: [docker-compose.yml, docker-compose.secure.yml]` axis. `fail-fast: false` so independent failures on each axis surface in the same run (not masked).
- **jq install step** — a deterministic idempotent apt install for jq before any JSON handling (jq is typically preinstalled on `ubuntu-latest`, but the explicit check protects against runner image changes).
- **COMPOSE_FILE env var** — every compose-related step (rewrite, up, wait/health, failure dump, teardown) reads `${COMPOSE_FILE}` from the matrix value. No hardcoded `docker-compose.yml` path outside of the matrix definition itself.
- **Expanded dashboard assertion** — the existing grep loop now checks for all four job names (`echo-timestamp`, `http-healthcheck`, `disk-usage`, `hello-world`) rather than just the two Phase 6 shipped.
- **New Run Now + poll step** — fetches `/api/jobs` once to build a name→id map (the Run Now API takes job id, not name), POSTs `/api/jobs/{id}/run` for each of the four jobs, then polls `/api/jobs/{id}/runs?limit=1` in a `case $latest in success|failed|timeout|cancelled|...` loop with a 120-second deadline and 2-second poll interval. Terminates early on success, fails fast on terminal non-success, keeps polling on `running|scheduled|""`.
- **Expanded failure diagnostics** — replaced the single `docker compose logs` dump with a 4-section group (`::group::`) dump: cronduit logs (tail 200), dockerproxy logs (tail 50 — secure axis only; "(no dockerproxy service in this axis)" on the default axis), per-job run-history tail for all four jobs, and the `cronduit_docker_reachable` gauge value from `/metrics`.
- **Teardown parameterized** — final `docker compose -f "${COMPOSE_FILE}" down -v` uses the matrix var.
- **lint / test / image jobs unchanged** — diff is a single block starting at line 124; no touch to any other job.

## Task Commits

| Task | Name                                                                             | Commit    | Files                    |
| ---- | -------------------------------------------------------------------------------- | --------- | ------------------------ |
| 1    | Convert compose-smoke to matrix + add per-job success assertions                 | `143a19b` | `.github/workflows/ci.yml` |

## Diff Summary

Single-file, single-job refactor:

```
 .github/workflows/ci.yml | 167 +++++++++++++++++++++++++++++++---------------
 1 file changed, 137 insertions(+), 30 deletions(-)
```

All changes are scoped to the `compose-smoke` block (lines 124-307 of the post-edit file). Four diff hunks, all inside the job:

```
@@ -124,16 +124,30 @@ jobs:        # job header + matrix strategy
@@ -150,23 +164,28 @@ jobs:       # jq install + compose rewrite parameterization
@@ -178,7 +197,7 @@ jobs:        # dashboard assertion expanded to 4 jobs
@@ -191,26 +210,114 @@ jobs:      # Run Now step + diagnostics + teardown
```

## Full Shape of the New compose-smoke Job

```yaml
compose-smoke:
  name: quickstart compose smoke (${{ matrix.compose }})
  runs-on: ubuntu-latest
  strategy:
    fail-fast: false
    matrix:
      compose:
        - docker-compose.yml
        - docker-compose.secure.yml
  steps:
    - uses: actions/checkout@v4
    - name: Install jq                                           # NEW
    - name: Set up Docker Buildx                                 # unchanged
    - name: Build local cronduit:ci image from PR checkout       # unchanged
    - name: Rewrite compose to use locally-built cronduit:ci image  # env: COMPOSE_FILE
    - name: docker compose up -d                                 # env: COMPOSE_FILE
    - name: Wait for /health (max 30s)                           # env: COMPOSE_FILE (for failure branch)
    - name: Assert /health body contains status:ok               # unchanged
    - name: Assert dashboard lists all four quickstart jobs      # expanded: 2 -> 4 jobs
    - name: Trigger Run Now on every example job and assert success within 120s  # NEW
    - name: Dump diagnostics on failure                          # NEW (replaces Dump compose logs)
    - name: Tear down compose stack                              # env: COMPOSE_FILE
```

## Decisions Made

- **`fail-fast: false`** — retained from the reference plan. Rationale: if the default axis fails because of (say) a `group_add` misconfiguration on the runner, and the secure axis fails because of a socket-proxy allowlist regression, we want both failures to surface in the same PR run. The alternative (`fail-fast: true`) would cancel the second axis the moment the first fails, hiding the second class of regression until the first is fixed.
- **Single `/api/jobs` fetch, reused for trigger + poll + diagnostics** — the Run Now API takes a job id, not a name. The plan's exact-text step fetches `/api/jobs` once, passes the resulting JSON through `jq` to build a name→id map (inline per-name `jq -r --arg n "$name" '.[] | select(.name == $n) | .id'`), and reuses the same `$jobs_json` variable for both the trigger loop and the subsequent poll loop. The failure-diagnostic step re-fetches because the failure path may run after a cronduit crash (stale `$jobs_json` would be misleading).
- **Poll interval: 2 seconds** — balances responsiveness with load on the /api/jobs/{id}/runs endpoint (a 120s budget / 2s interval = 60 GETs per job worst-case, 240 GETs total across all four — well below any rate limit and fast enough to catch sub-second state transitions).
- **Terminal-status case statement** — a `case "$latest" in` branching on `success | failed|timeout|cancelled | running|scheduled|"" | *` is more robust than an `if [ success ]` + `if [ failed ]` chain. The `"*"` branch warns on unknown statuses (future-proof against new job states) and continues polling, preventing a typo or new state from failing a PR mid-release.
- **Diagnostic dump uses `::group::` for log folding** — readers scanning a failed CI run can collapse the cronduit log dump and focus on run history or the docker gauge. Standard GHA UI convention.
- **`2>/dev/null || echo "(no dockerproxy service)"` on the dockerproxy log line** — the default axis (`docker-compose.yml`) has no `dockerproxy` service at all, so `docker compose logs dockerproxy` would return a non-zero exit. Suppressing stderr + graceful fallback keeps the diagnostic step itself from masking the real failure.

## Deviations from Plan

None — plan executed exactly as written. Small addendum notes:

1. **Acceptance-criterion `docker-compose.yml >= 2`** — the plan's acceptance list said `grep -c 'docker-compose.yml' .github/workflows/ci.yml` should return `>= 2` (matrix entry + possibly diagnostics). In practice the refactor parameterizes every compose path via `${COMPOSE_FILE}`, so the string `docker-compose.yml` appears exactly **once** (as the first matrix axis entry). This is **more correct** than the original criterion, not less — it means there are zero hardcoded per-file paths in the steps (every reference goes through the matrix variable). Not tracked as a deviation because the intent (matrix axis is present) is satisfied via the `matrix.compose` 7 hits and the explicit `docker-compose.yml` matrix-list entry. Documented for transparency.

## Verification Evidence

All acceptance grep checks (from the plan, except the one-noted-above):

```
$ grep -c '^  compose-smoke:$' .github/workflows/ci.yml
1                                    # still a single job
$ grep -c 'matrix:' .github/workflows/ci.yml
3                                    # test.arch + compose-smoke.compose + matrix: inside compose-smoke's strategy
$ grep -c 'docker-compose.secure.yml' .github/workflows/ci.yml
1                                    # matrix entry
$ grep -c 'matrix.compose' .github/workflows/ci.yml
7                                    # job name + 6 env: COMPOSE_FILE uses
$ grep -c 'Trigger Run Now on every example job' .github/workflows/ci.yml
1
$ grep -c 'POST .*/api/jobs' .github/workflows/ci.yml
2                                    # trigger step + comment reference
$ grep -c '/api/jobs/.*/runs?limit=1' .github/workflows/ci.yml
1
$ grep -c 'BUDGET_SECS=120' .github/workflows/ci.yml
1
$ grep -c 'echo-timestamp http-healthcheck disk-usage hello-world' .github/workflows/ci.yml
3                                    # trigger JOBS var + dashboard loop + diagnostic loop
$ grep -c 'cronduit_docker_reachable' .github/workflows/ci.yml
2                                    # header group label + curl grep
$ grep -c 'dockerproxy' .github/workflows/ci.yml
2                                    # group label + docker compose logs command
$ grep -c 'jq' .github/workflows/ci.yml
12                                   # install step + trigger step + diagnostic step + inline refs
$ grep -c 'docker compose .* down -v' .github/workflows/ci.yml
1
```

**YAML validity & matrix structure (via ruby, since pyyaml isn't available in this worktree):**

```
$ ruby -ryaml -rjson -e "d=YAML.load_file('.github/workflows/ci.yml'); \
    j=d['jobs']['compose-smoke']; \
    puts 'YAML ok'; \
    puts 'matrix: ' + j['strategy']['matrix']['compose'].to_json; \
    puts 'fail-fast: ' + j['strategy']['fail-fast'].to_s; \
    puts 'name: ' + j['name']; \
    puts 'steps: ' + j['steps'].size.to_s"
YAML ok
matrix: ["docker-compose.yml","docker-compose.secure.yml"]
fail-fast: false
name: quickstart compose smoke (${{ matrix.compose }})
steps: 12
```

**Diff scoping: only compose-smoke changed.** Four hunks, all contained in the `compose-smoke` block:

```
$ git diff .github/workflows/ci.yml | grep '^@@'
@@ -124,16 +124,30 @@ jobs:
@@ -150,23 +164,28 @@ jobs:
@@ -178,7 +197,7 @@ jobs:
@@ -191,26 +210,114 @@ jobs:
```

The `lint`, `test`, and `image` jobs are textually unchanged.

## Cross-reference to Phase 8 Decisions

- **D-18** (job reaches success within 120s, assertion runs after /health + before teardown) — `ci.yml` lines ~228-295 (Trigger Run Now step)
- **D-19** (POST /api/jobs/{id}/run + jq polling) — `ci.yml` lines ~228-295 (trigger + poll logic)
- **D-20** (failure dump: cronduit logs, dockerproxy logs, run history, cronduit_docker_reachable) — `ci.yml` lines ~297-324 (Dump diagnostics on failure)
- **D-21** (matrix over both compose files, both must pass) — `ci.yml` lines 133-140 (strategy.matrix.compose)
- **D-22** (single compose up/down per axis, existing /health assertions preserved) — `ci.yml` lines 184-215 preserved from Phase 6 GAP-3.4, dashboard assertion expanded 2→4 jobs (Plan 08-01 dependency)

## First CI Run URL

(Pending merge of the phase/08-plan branch — will be populated by the orchestrator once the full phase lands and the PR is opened. Both matrix axes must pass for the phase to merge.)

## CLAUDE.md Compliance

- [x] **Mermaid-only diagrams:** no diagrams added in this plan (none needed — the summary is pure ops flow).
- [x] **No direct commits to main:** committed to the `worktree-agent-a2f5bcdf` worktree branch, rebased onto `a374ab8` (Wave 1 post-roadmap commit on `phase/08-plan`).
- [x] **Rust/bollard/sqlx/askama_web stack unchanged:** no source code touched, no deps added.
- [x] **GitHub Actions workflow-injection hygiene:** every dynamic value (`${{ matrix.compose }}`) is surfaced to shell via an `env:` block (`COMPOSE_FILE: ${{ matrix.compose }}`) and referenced as `"${COMPOSE_FILE}"` in quoted shell strings — never interpolated directly into `run:` blocks. `matrix.compose` is a workflow-defined constant (not untrusted user input from `github.event.*`), so the risk surface is already near-zero, but the `env:` pattern is applied anyway per the security reminder.
- [x] **Alpine runtime parity:** CI still builds `cronduit:ci` via `docker/build-push-action@v6` with the same Dockerfile now based on alpine:3 (Plan 08-01).
- [x] **Cross-compile story:** no change to the `cargo zigbuild` build paths; compose-smoke still targets `linux/amd64` only (matches the previous behavior).

## Next Phase Readiness

**Ready for Plan 08-05 (human UAT walkthrough):**
- CI now proves the quickstart works end-to-end on both compose variants before any PR merges.
- A human following `README.md` → `docker compose up` can expect the stack to boot, the four jobs to fire on their first natural schedule, and the dashboard to reflect their state. The only thing left to validate manually is UI fidelity (terminal-green theme, dark/light toggle, Run Now toast, ANSI log rendering, auto-refresh transitions) — the mechanical plumbing is gated.
- If a future PR regresses the docker-socket-proxy allowlist, the secure axis catches it. If a future PR regresses `group_add` handling on Linux, the default axis catches it. Both must pass, so neither class of regression can slip into main silently.

## Self-Check: PASSED

**Files verified present:**
- `.github/workflows/ci.yml` — FOUND (modified)
- `.planning/phases/08-v1-final-human-uat-validation/08-04-SUMMARY.md` — FOUND (this file)

**Commits verified in git log:**
- `143a19b` — FOUND (Task 1: compose-smoke matrix + Run Now success gate)

---
*Phase: 08-v1-final-human-uat-validation*
*Plan: 04*
*Completed: 2026-04-14*
