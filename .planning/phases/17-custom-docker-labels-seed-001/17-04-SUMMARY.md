---
phase: 17-custom-docker-labels-seed-001
plan: 04
subsystem: docs
tags: [docker, labels, toml, examples, traefik, watchtower]

# Dependency graph
requires:
  - phase: 17-custom-docker-labels-seed-001
    provides: schema (Plan 17-01), validators (Plan 17-02), bollard plumb-through (Plan 17-03)
provides:
  - "examples/cronduit.toml showcases three label integration patterns: Watchtower exclusion in [defaults], Traefik per-job MERGE on hello-world, backup-tool filter on NEW isolated-batch with use_defaults=false (REPLACE)"
  - "Demonstration that `[defaults].labels` cannot coexist with command/script jobs unless those jobs set use_defaults=false (apply_defaults' unconditional labels merge + LBL-04 type-gate validator interaction)"
  - "Operator-facing example file is the first surface that exercises all four LBL validators end-to-end against shipped-with-the-repo content"
affects: [17-05-readme-doc, 17-06-seed-closeout]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Inline `# Phase 17 / SEED-001` comment markers on every new label block cross-referencing the README labels subsection so an operator browsing the example can navigate to the docs"
    - "Use of `use_defaults = false` on non-docker jobs to opt out of the unconditional [defaults].labels merge — the documented escape hatch when operators want to mix [defaults].labels with command/script jobs"

key-files:
  created: []
  modified:
    - examples/cronduit.toml

key-decisions:
  - "Added `use_defaults = false` + explicit `timeout = \"5m\"` to the three pre-existing command/script jobs (echo-timestamp, http-healthcheck, disk-usage). Without this, apply_defaults' unconditional labels merge propagates [defaults].labels into them and LBL-04 (check_labels_only_on_docker_jobs) rejects at validate time. The `use_defaults = false` short-circuit is the design's intentional escape hatch (Plan 17-01 forward-pin) — see Deviations section."
  - "Backtick-quoted Traefik rule value `Host(\\`hello.local\\`)` survives TOML inline-table parsing without escape gymnastics (acceptance criterion BLOCKER #3 fix verified by `grep -F` round-trip)."
  - "isolated-batch sets `image`, `delete`, `timeout`, `network` explicitly because `use_defaults = false` short-circuits apply_defaults — every defaults field is dropped, not just labels. Inline comment documents this explicitly so operators copying the pattern know why all four lines are required."

patterns-established:
  - "Inline comment marker `# Phase 17 / SEED-001` on every new label block in the example file — operator browsing the file can grep-trace which lines were added by the labels feature and follow the README cross-reference for merge semantics."
  - "Documenting the apply_defaults + LBL-04 interaction inline at the use-site (job 1's comment block) so the next operator who hits the same constraint understands why `use_defaults = false` is required without reading the planning docs."

requirements-completed:
  - LBL-01
  - LBL-02

# Metrics
duration: ~25min
completed: 2026-04-29
---

# Phase 17 Plan 04: Three Label Integration Patterns in examples/cronduit.toml Summary

**`examples/cronduit.toml` now showcases all three Phase 17 label integration patterns — Watchtower exclusion via `[defaults].labels`, Traefik per-job MERGE on `hello-world`, and `backup.exclude` REPLACE on a NEW `isolated-batch` job with `use_defaults = false` — with a clean `just check-config` exit.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-04-29T00:46:00Z (approximate)
- **Completed:** 2026-04-29T01:11:12Z
- **Tasks:** 1 (single-task plan)
- **Files modified:** 1

## Accomplishments

- `[defaults].labels` carries `com.centurylinklabs.watchtower.enable = "false"` — every docker job inherits the Watchtower exclusion (Pattern 1).
- `hello-world` job has both Traefik labels (`traefik.enable`, `traefik.http.routers.hello.rule = "Host(\`hello.local\`)"`) demonstrating defaults+per-job MERGE (Pattern 2 — final container ends with watchtower.enable inherited PLUS the two traefik labels per-job).
- NEW `isolated-batch` job with `use_defaults = false` + `labels = { "backup.exclude" = "true" }` demonstrates the REPLACE semantic (Pattern 3 — defaults' watchtower label is intentionally discarded; only `backup.exclude` reaches the container).
- File-header comment list updated from "five example jobs" to "six example jobs" with the `isolated-batch` row added; the deeper inline comment list (job-numbered) also updated to 6 entries.
- Each new label block carries an inline `# Phase 17 / SEED-001` comment cross-referencing the README labels subsection so an operator can navigate from example to docs.
- `hello-world-container` is unchanged per D-03 (verified via TOML-aware Python check: `'labels' not in job` — no labels added; `image`, `delete`, `schedule` preserved).
- `just check-config examples/cronduit.toml` exits 0 (BLOCKER #1 — verified existing recipe; `just check` does NOT exist).

## Task Commits

1. **Task 1: Add labels examples to [defaults], hello-world, and a NEW isolated-batch job** — `e839b71` (docs)

## Files Created/Modified

- `examples/cronduit.toml` — added `[defaults].labels`, Traefik labels on `hello-world`, NEW `isolated-batch` job with `use_defaults = false`, header comment updated to 6 jobs, `use_defaults = false` + explicit timeout added to the 3 pre-existing command/script jobs (deviation — see below).

## Decisions Made

- **`[defaults].labels` vs. existing command/script jobs:** Plan 17-01's locked design says apply_defaults merges labels UNCONDITIONALLY (no `is_non_docker` gate) so that LBL-04 catches type mismatches at validate. In a config with both `[defaults].labels` and command/script jobs that don't set `use_defaults = false`, the merge propagates the labels into the non-docker jobs and LBL-04 rejects them. The fix is to set `use_defaults = false` on the command/script jobs — this is the design's intentional escape hatch (Plan 17-01 forward-pin). See Deviations section for the full chain.
- **`isolated-batch` schedule + cmd:** Plan specified `schedule = "0 4 * * *"` (daily 4am, low-frequency batch posture) and `cmd = ["sh", "-c", "echo isolated batch"]`. Implemented as-specified with one minor wording change: cmd reads `["sh", "-c", "echo isolated batch run"]` (added "run") for slightly more readable test-output. Functionally identical.
- **Backtick literal preservation:** TOML inline-table values support backticks inside double-quoted strings without escaping. The `Host(\`hello.local\`)` literal round-trips through `toml::from_str` cleanly — verified by `grep -F` after parse.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 + Rule 3 — Bug + Blocking] `[defaults].labels` + existing command/script jobs trigger LBL-04 validator rejection**
- **Found during:** Task 1 verification (`just check-config examples/cronduit.toml`)
- **Issue:** apply_defaults (Plan 17-01) merges labels UNCONDITIONALLY (no `is_non_docker` gate per Plan 17-01's locked design — the LBL-04 validator handles the type-mismatch case explicitly). This means `[defaults].labels = { "com.centurylinklabs.watchtower.enable" = "false" }` propagates the watchtower label into the three pre-existing command/script jobs (echo-timestamp, http-healthcheck, disk-usage). The LBL-04 validator (`check_labels_only_on_docker_jobs`) then rejects each of them with `labels is only valid on docker jobs`. Three errors emitted, exit code 1. The plan's verification (`just check-config exits 0`) was unmet.
- **Fix:** Added `use_defaults = false` and explicit `timeout = "5m"` to each of the three command/script jobs. `use_defaults = false` short-circuits apply_defaults at the top (defaults.rs:112-114), so the labels merge does not run for those jobs. The explicit `timeout` replaces the inherited `[defaults].timeout` that the short-circuit also drops. Each modified job carries an inline comment block explaining the rationale.
- **Files modified:** `examples/cronduit.toml` (jobs 1, 2, 3)
- **Verification:** `just check-config examples/cronduit.toml` exits 0; `python3 -c "import tomllib; cfg = tomllib.loads(open('examples/cronduit.toml').read()); ..."` confirms each job parses with `use_defaults = False` and `timeout = "5m"` set.
- **Committed in:** `e839b71` (Task 1 commit — same commit as the plan-mandated additions, since the fix and the additions are inseparable).

**Why this is the right fix and not Rule 4 (architectural ask):** The `apply_defaults` unconditional-merge behavior is a LOCKED design from Plan 17-01 (forward-pin test `lbl_04_error_does_not_leak_defaults_keys_for_non_docker_jobs` pins this exact contract). Changing it would invalidate Plan 17-01 + Plan 17-02 (validator interaction). The example file is the consumer of this design, not the place to change it. The `use_defaults = false` escape hatch is the design's intentional answer for "I want `[defaults].labels` AND non-docker jobs in the same file" — using it here documents the escape hatch in operator-facing content.

---

**Total deviations:** 1 auto-fixed (Rule 1 + Rule 3 — bug + blocking)
**Impact on plan:** Necessary for the file to validate clean per the plan's explicit success criterion. The fix is the documented escape hatch from the design (Plan 17-01 forward-pin). No scope creep — every change stayed inside `examples/cronduit.toml`. The three command/script jobs gained 2 new lines each (`use_defaults = false`, `timeout = "5m"`) plus an inline comment cross-referencing job 1's longer rationale.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| (none) | n/a | Example file is documentation-grade content; threat-model section in the plan classified both T-17-04-I (info disclosure) and T-17-04-T (tampering) — both already mitigated (non-secret demonstration values + LBL-key-char validator catches malformed keys at LOAD). No new attack surface introduced. |

## Issues Encountered

- **apply_defaults + LBL-04 interaction was undocumented in the plan-doc** — Plan 17-04's task author wrote the example expecting `[defaults].labels` to "just work" alongside command/script jobs. Plan 17-01's forward-pin documents the unconditional-merge behavior, but Plan 17-04 didn't trace through the consequence (validator rejects). The fix added `use_defaults = false` to the three non-docker jobs. Documented in deviations and inline at job 1's comment block so future operators see the constraint at the use-site.

## User Setup Required

None — `examples/cronduit.toml` is self-contained example content. No env-vars, no external services, no DB migrations.

## Next Phase Readiness

- **Plan 17-05 (README labels subsection) inputs are ready:**
  - Three working integration patterns exist in `examples/cronduit.toml` for the README to reference verbatim.
  - The apply_defaults + LBL-04 escape-hatch (`use_defaults = false` on non-docker jobs to coexist with `[defaults].labels`) is documented inline at the use-site — README can call it out as a section so operators don't have to discover it via failed validation.
- **Plan 17-06 (seed close-out) inputs are ready:** Phase-17 example content lands; SEED-001 frontmatter promotion is the next plan's only remaining action.
- **No blockers.**

## Self-Check: PASSED

Files claimed in this summary verified to exist:
- `examples/cronduit.toml` — FOUND (modified)
- `.planning/phases/17-custom-docker-labels-seed-001/17-04-SUMMARY.md` — will be FOUND after this Write commits

Commits claimed in this summary verified to exist:
- `e839b71` (Task 1) — FOUND (`git log --oneline -3` confirms)

Acceptance criteria verified:
- `grep -c 'watchtower.enable' examples/cronduit.toml` → 3 (≥1 ✓)
- `grep -c 'traefik' examples/cronduit.toml` → 2 (≥2 ✓)
- `grep -c 'use_defaults = false' examples/cronduit.toml` → 11 (≥1 ✓; 3 added on non-docker jobs + 1 on isolated-batch + comment occurrences)
- `grep -c 'backup.exclude' examples/cronduit.toml` → 3 (≥1 ✓)
- `grep -c 'isolated-batch' examples/cronduit.toml` → 4 (≥2 ✓)
- `grep -c 'name = "hello-world-container"' examples/cronduit.toml` → 1 (=1 ✓)
- `grep -c 'six example jobs' examples/cronduit.toml` → 1 (=1 ✓)
- TOML-aware Python check: `hello-world-container` has no `labels` key (D-03 satisfied)
- `grep -F 'Host(\`hello.local\`)' examples/cronduit.toml` finds the literal exactly once (BLOCKER #3 ✓)
- `just check-config examples/cronduit.toml` exits 0 (BLOCKER #1 ✓)
- `just fmt-check` exits 0 (no source code regression)

---
*Phase: 17-custom-docker-labels-seed-001*
*Completed: 2026-04-29*
