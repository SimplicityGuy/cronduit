---
phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship
plan: 07
subsystem: ops
tags: [docs, security, just, docker-compose, threat-model, release-tooling, phase-14, wave-4]

requires:
  - phase: 12-docker-healthcheck-rc-1-cut
    provides: "release.yml D-10 metadata-action gating + docs/release-rc.md runbook (rc.3 reuses verbatim)"
  - phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship/14-02
    provides: "POST /api/jobs/bulk-toggle handler + bulk-toggle blast-radius surface that THREAT_MODEL.md D-21 documents"

provides:
  - "THREAT_MODEL.md D-21 bulk-toggle blast-radius bullet (parallel to Phase 10 Stop-button bullet)"
  - "justfile [group('release')] section with four HUMAN-UAT recipes: compose-up-rc3, reload, health, metrics-check"
  - "examples/docker-compose.yml CRONDUIT_IMAGE env-var parameterization (default unchanged)"
  - "Foundation for Plan 14-08 HUMAN-UAT.md to reference only `just` recipes (no raw curl/wget)"

affects:
  - 14-08 (HUMAN-UAT.md authors all eight UAT steps against `just` recipes added here)
  - 14-09 (final v1.1.0 promotion runbook references the same release-group recipes)
  - v1.2 milestone planning (THREAT_MODEL.md D-21 wording is the lead-in for AUTH-01 / AUTH-02 v2 work)

tech-stack:
  added: []
  patterns:
    - "[group('release')] section in justfile for HUMAN-UAT recipes (parallel to existing [group('dev')] / [group('docker')] / [group('quality')])"
    - "just escape for Docker `{{.Names}}` format token using `{{ \"{{.Names}}\" }}` wrapper, with top-of-group `# DO NOT EDIT` maintainer guard"
    - "docker-compose.yml image reference via `${CRONDUIT_IMAGE:-<default>}` env var so release-candidate pinning is one env-var hop away"

key-files:
  created: []
  modified:
    - "THREAT_MODEL.md (one paragraph appended after Stop-button bullet at L113)"
    - "justfile (47-line additive [group('release')] section appended after update-hooks)"
    - "examples/docker-compose.yml (one-line image: env-var swap, default behavior preserved)"

key-decisions:
  - "Used the verbatim D-21 wording from 14-RESEARCH.md § Security Domain — preserves planner-locked invariant of one paragraph, parallel structure to Stop bullet, same AUTH-01/02 deferral reference"
  - "Inserted bulk-toggle bullet immediately after the Stop-button paragraph in `## Threat Model 2: Untrusted Client → ### Residual Risk` (L113 area) — same heading as Phase 10 Stop bullet so future readers find both blast-radius notes co-located"
  - "Added FOUR `just` recipes (compose-up-rc3 + reload + health + metrics-check) instead of the two originally listed in 14-RESEARCH.md, per Warning #8 — wraps the raw curl in HUMAN-UAT Steps 1 and 8 so feedback_uat_use_just_commands.md is fully satisfied with no remaining raw `curl` references in any UAT step"
  - "Top-of-`[group('release')]` `# DO NOT EDIT` block explicitly documents the `{{ \"{{.Names}}\" }}` escape so a future maintainer cannot strip the wrapper and break `just reload` parse-time (Warning #4 mitigation)"
  - "examples/docker-compose.yml change is a single-line env-var swap with `:latest` default — `just compose-up-rc3` pins rc.3 via the env var, the README quickstart `docker compose up` continues to work unchanged"

patterns-established:
  - "HUMAN-UAT discipline: every UAT step in subsequent release plans (Plans 08 + 09) must reference an existing `just` recipe — adding the recipe to justfile is part of the prep-for-UAT plan, not the UAT plan itself"
  - "THREAT_MODEL.md blast-radius bullets follow the Phase 10 Stop-button shape: bold lead-in, single paragraph, explicit AUTH-01 / AUTH-02 deferral reference"
  - "justfile multi-recipe additions land in a single `[group('NAME')]` block with a single top-of-block `# DO NOT EDIT` maintainer comment when load-bearing escape syntax is involved"

requirements-completed: [ERG-01, ERG-02]

duration: ~3 min
completed: 2026-04-22
---

# Phase 14 Plan 07: Ops + Docs Scaffolding for rc.3 HUMAN-UAT Summary

**THREAT_MODEL.md D-21 blast-radius bullet appended; four `just` recipes (compose-up-rc3, reload, health, metrics-check) added under a new `[group('release')]` block; examples/docker-compose.yml parameterized via `${CRONDUIT_IMAGE:-…}` so Plan 08 HUMAN-UAT can reference only `just` recipes (no raw curl/wget) — feedback_uat_use_just_commands.md fully satisfied.**

## Performance

- **Duration:** ~3 min (per-task execution; commits 15:50:13Z → 15:52:24Z UTC-7)
- **Started:** 2026-04-22T22:50:00Z (approx; reset-to-base + first edit)
- **Completed:** 2026-04-22T22:52:39Z
- **Tasks:** 2 / 2
- **Files modified:** 3 (THREAT_MODEL.md, justfile, examples/docker-compose.yml)

## Accomplishments

- THREAT_MODEL.md L113-area now has a parallel `**Bulk toggle (v1.1 blast radius):**` paragraph immediately after the Stop-button bullet; explicit "Running jobs are NOT terminated by bulk disable (D-02 / ERG-02)" + AUTH-01 / AUTH-02 v2 deferral.
- justfile gained `[group('release')]` with four recipes Plan 08 HUMAN-UAT requires (`just compose-up-rc3`, `just reload`, `just health`, `just metrics-check`) — `just --list` enumerates all four and parses cleanly under `just 1.50.0`.
- The load-bearing `{{ "{{.Names}}" }}` escape inside `reload` is guarded by a top-of-group `# DO NOT EDIT` comment block so future maintainers do not accidentally strip the wrapper and break parse-time.
- examples/docker-compose.yml `image:` line is parameterized as `${CRONDUIT_IMAGE:-ghcr.io/simplicityguy/cronduit:latest}`; default behavior unchanged for the README quickstart, but `just compose-up-rc3` can now pin `1.1.0-rc.3` via the env var.
- All changes purely additive (no existing recipe re-ordered or removed; no existing THREAT_MODEL prose modified; one image: line swapped 1-for-1 in docker-compose.yml).

## Task Commits

Each task was committed atomically with `--no-verify` (parallel-executor protocol):

1. **Task 1: Append D-21 bulk-toggle blast-radius bullet to THREAT_MODEL.md** — `e02d1a0` (docs)
2. **Task 2: Add release-group `just` recipes + parameterize compose image** — `d377b12` (feat)

(SUMMARY commit follows at the end of plan execution.)

## Files Created/Modified

- `THREAT_MODEL.md` — One bold-prefixed paragraph appended after Stop-button bullet at L113 (Threat Model 2 → Residual Risk). +2 lines, 0 removed.
- `justfile` — `[group('release')]` block with four recipes appended at end-of-file after `update-hooks`. +47 lines, 0 removed (no existing recipe touched).
- `examples/docker-compose.yml` — One-line swap: `image: ghcr.io/simplicityguy/cronduit:latest` → `image: ${CRONDUIT_IMAGE:-ghcr.io/simplicityguy/cronduit:latest}`. +1, -1.

## Diff Excerpts

### THREAT_MODEL.md (additive — bullet appended at L113)

```diff
@@
 **Stop button (v1.1+ blast radius):** The Stop button added in v1.1 lets anyone with Web UI access terminate any running job via `POST /api/runs/{id}/stop`. ... deferred to v2 (AUTH-01 / AUTH-02).
+
+**Bulk toggle (v1.1 blast radius):** The bulk-toggle endpoint added in v1.1 lets anyone with Web UI access disable every configured job in a single `POST /api/jobs/bulk-toggle` request. This further widens the blast radius of an unauthenticated UI compromise — an attacker can now silently stop the entire schedule without terminating any running execution. Running jobs are NOT terminated by bulk disable (D-02 / ERG-02), so an in-flight attacker-triggered run continues to completion even after all jobs are bulk-disabled. Mitigation posture is identical to the rest of the v1 Web UI: loopback default or reverse-proxy auth. Bulk-action authorization (including a per-action confirmation step) is deferred to v2 (AUTH-01 / AUTH-02).
```

### examples/docker-compose.yml (one-line env-var swap)

```diff
@@ services:
   cronduit:
-    image: ghcr.io/simplicityguy/cronduit:latest
+    image: ${CRONDUIT_IMAGE:-ghcr.io/simplicityguy/cronduit:latest}
```

### justfile (additive [group('release')] block)

```diff
@@ -- end of file, after update-hooks --
+
+# -------------------- release candidate smoke --------------------
+# DO NOT EDIT — paste recipes VERBATIM. just escapes the Docker `{{.Names}}`
+# format string as `{{ "{{.Names}}" }}` because bare `{{...}}` is reserved for
+# just's own interpolation. Removing the outer `{{ "..." }}` wrapper will make
+# `just reload` fail at parse time with "Unknown identifier `.Names`".
+
+# Bring up the compose stack pinned to v1.1.0-rc.3 for HUMAN-UAT.
+# Phase 14 D-17 / feedback_uat_use_just_commands.
+[group('release')]
+[doc('Bring up the compose stack pinned to v1.1.0-rc.3 for HUMAN-UAT')]
+compose-up-rc3:
+    CRONDUIT_IMAGE=ghcr.io/simplicityguy/cronduit:1.1.0-rc.3 \
+    docker compose -f examples/docker-compose.yml up -d
+
+# Trigger a config reload of the running cronduit by SIGHUP.
+# HUMAN-UAT steps 4 + 7 per D-17.
+[group('release')]
+[doc('Send SIGHUP to the running cronduit process (config reload)')]
+reload:
+    #!/usr/bin/env bash
+    set -euo pipefail
+    # NOTE: `{{ "{{.Names}}" }}` is the just-escaped form of Docker's
+    # `{{ "{{.Names}}" }}` (the literal Docker --format token). DO NOT remove
+    # the outer `{{ "..." }}` wrapper — see top-of-group comment.
+    if docker ps --format '{{ "{{.Names}}" }}' | grep -q '^cronduit$'; then
+        docker kill -s HUP cronduit
+        echo "SIGHUP sent to cronduit container"
+    else
+        pkill -HUP cronduit && echo "SIGHUP sent to cronduit process" \
+            || { echo "no running cronduit found"; exit 1; }
+    fi
+
+# Probe the running cronduit /health endpoint and print the status.
+# HUMAN-UAT Step 1 — replaces raw `curl | jq` per Warning #8.
+[group('release')]
+[doc('Curl /health and print .status (expect "healthy")')]
+health:
+    curl -sf http://127.0.0.1:8080/health | jq -r '.status'
+
+# Check key Prometheus metrics. HUMAN-UAT Step 8 — replaces raw curl per Warning #8.
+# Prints scheduler liveness + runs_total series lines only (no noisy full dump).
+[group('release')]
+[doc('Grep /metrics for cronduit_scheduler_up and cronduit_runs_total lines')]
+metrics-check:
+    curl -sf http://127.0.0.1:8080/metrics \
+        | grep -E '^cronduit_scheduler_up\b|^cronduit_runs_total\b'
```

## just --list (release group only)

```
[release]
compose-up-rc3          # Bring up the compose stack pinned to v1.1.0-rc.3 for HUMAN-UAT
health                  # Curl /health and print .status (expect "healthy")
metrics-check           # Grep /metrics for cronduit_scheduler_up and cronduit_runs_total lines
reload                  # Send SIGHUP to the running cronduit process (config reload)
```

(Full `just --list` output enumerates every existing recipe unchanged plus the four new release recipes; `just --list` exits 0.)

## just --show reload (parse-sanity dry-run, Warning #4 mitigation)

```
[doc('Send SIGHUP to the running cronduit process (config reload)')]
[group('release')]
reload:
    #!/usr/bin/env bash
    set -euo pipefail
    # NOTE: `{{ "{{.Names}}" }}` is the just-escaped form of Docker's
    # `{{ "{{.Names}}" }}` (the literal Docker --format token). DO NOT remove
    # the outer `{{ "..." }}` wrapper — see top-of-group comment.
    if docker ps --format '{{.Names}}' | grep -q '^cronduit$'; then
        docker kill -s HUP cronduit
        echo "SIGHUP sent to cronduit container"
    else
        pkill -HUP cronduit && echo "SIGHUP sent to cronduit process" \
            || { echo "no running cronduit found"; exit 1; }
    fi
```

Note that `just --show` performs the `{{ "..." }}` escape resolution and emits the literal Docker `{{.Names}}` format token in the rendered body — exactly the value Docker's `--format` flag expects at runtime. This confirms the escape is correct.

## Decisions Made

- Used the verbatim D-21 wording from `14-RESEARCH.md § Security Domain → D-21 THREAT_MODEL.md Wording` (also reproduced in `14-PATTERNS.md §19`) — no editorial deviation since the planner explicitly approved the wording as the invariant.
- Took the planner's option in 14-07-PLAN.md task 2 to add FOUR recipes (compose-up-rc3, reload, health, metrics-check) rather than the two `14-RESEARCH.md § Missing-just-Recipes` originally enumerated. Rationale: Warning #8 in the plan explicitly extends the recipe set to wrap the raw `curl` in HUMAN-UAT Steps 1 and 8, completing the "no raw curl in HUMAN-UAT" guarantee.
- Placed the new `[group('release')]` block at the end of the justfile (after `update-hooks` in `[group('deps')]`) so it sits visually adjacent to the other meta/release-adjacent recipes (`release` lives in `[group('meta')]`) without disturbing recipe ordering.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] Inner `{{.Names}}` reference in NOTE comment broke `just --list` parse**

- **Found during:** Task 2 (justfile recipe addition)
- **Issue:** The plan's verbatim NOTE comment inside the `reload` recipe body read:
  ```
  # NOTE: `{{ "{{.Names}}" }}` is the just-escaped form of Docker's `{{.Names}}`.
  ```
  The trailing bare `{{.Names}}` (intended as prose reference) was tokenized by `just 1.50.0` as a real interpolation, producing:
  ```
  error: Unknown start of token '.'
     ——▶ justfile:321:74
  321 │     # NOTE: `{{ "{{.Names}}" }}` is the just-escaped form of Docker's `{{.Names}}`.
      │                                                                          ^
  ```
- **Fix:** Rewrote the NOTE so both occurrences of the format-token reference use the escaped wrapper:
  ```
  # NOTE: `{{ "{{.Names}}" }}` is the just-escaped form of Docker's
  # `{{ "{{.Names}}" }}` (the literal Docker --format token). DO NOT remove
  # the outer `{{ "..." }}` wrapper — see top-of-group comment.
  ```
  Same intent (document the escape); just-parser-safe everywhere.
- **Files modified:** justfile (one comment block in the `reload` recipe body)
- **Verification:** `just --list` exits 0; `just --show reload` renders cleanly with the resolved Docker format token.
- **Committed in:** `d377b12` (Task 2 commit, before staging)

This is the canonical Warning #4 risk the plan explicitly named: removing the outer `{{ "..." }}` wrapper breaks parse. The auto-memory take-away is that ANY occurrence of `{{...}}` in a justfile (including inside hash-line comments) must be escaped — `just`'s tokenizer does not exempt comments from interpolation parsing.

---

**Total deviations:** 1 auto-fixed (Rule 3 — blocking parse error).
**Impact on plan:** Zero scope creep; the prose-reference fix preserved the plan's intent (document the escape) while making the recipe parse green.

## Issues Encountered

None beyond the auto-fixed parse-error above. THREAT_MODEL.md edit applied first try; docker-compose.yml swap was a single-line replace.

## Auto-Memory Compliance

- **`feedback_uat_use_just_commands.md`** — All four UAT steps that previously called raw `curl` / `docker` / `pkill` (Plan 08 HUMAN-UAT Steps 1, 4, 7, 8) now have a wrapping `just` recipe. Plan 08 will reference these by name (`just compose-up-rc3`, `just reload`, `just health`, `just metrics-check`) with no remaining raw shell commands.
- **`feedback_diagrams_mermaid.md`** — No diagrams added in this plan. The pre-existing mermaid diagrams at the top of `THREAT_MODEL.md` (Threat Model 1 + assets/trust-boundaries) are unchanged.
- **`feedback_no_direct_main_commits.md`** — Both commits (`e02d1a0`, `d377b12`) land on the parallel-executor worktree branch `worktree-agent-a7c9fd2a`; final merge to phase-14 branch happens via the orchestrator's wave-merge step.
- **`feedback_tag_release_version_match.md`** — `compose-up-rc3` pins `ghcr.io/simplicityguy/cronduit:1.1.0-rc.3` (full semver, hyphen-dot form), matching the plan's locked tag shape and `Cargo.toml = "1.1.0"`.

## User Setup Required

None — Plan 14-07 deliverables are pure scaffolding. The `just compose-up-rc3` recipe will require a published `ghcr.io/simplicityguy/cronduit:1.1.0-rc.3` image to actually pull, but cutting that image is the rc.3 release plan's responsibility (Plan 14-09), not this plan.

## Next Phase Readiness

Wave 4 ops/docs scaffolding is complete. Plan 14-08 (HUMAN-UAT.md authoring, in the next wave) can now reference `just compose-up-rc3 / reload / health / metrics-check` for every UAT step with no raw curl/wget anywhere. Plan 14-09 (rc.3 release-candidate cut + final v1.1.0 promotion runbook) inherits the THREAT_MODEL.md D-21 bullet for the rc.3 release notes' Security section.

## Self-Check: PASSED

- File presence (modified):
  - THREAT_MODEL.md — FOUND, modified L113-area (bullet appended)
  - justfile — FOUND, +47 lines / -0
  - examples/docker-compose.yml — FOUND, +1 / -1 (image: env-var swap)
- Commit existence:
  - `e02d1a0` — FOUND in `git log`
  - `d377b12` — FOUND in `git log`
- Acceptance criteria:
  - All 6 THREAT_MODEL.md greps PASS (bullet, literal phrase, not-terminated phrase, mitigation phrase, AUTH deferral, count == 1)
  - All 11 justfile greps PASS (compose-up-rc3, reload, health, metrics-check, # DO NOT EDIT, pinned rc.3 tag, compose-up command, docker kill, pkill fallback, [group('release')], curl health, curl metrics)
  - docker-compose.yml grep PASS (`${CRONDUIT_IMAGE:-ghcr.io/simplicityguy/cronduit:latest}`)
  - `just --list` exit 0; recipe-enumeration count == 4
  - `just --show reload` renders cleanly with resolved Docker format token

---
*Phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship*
*Plan: 07*
*Completed: 2026-04-22*
