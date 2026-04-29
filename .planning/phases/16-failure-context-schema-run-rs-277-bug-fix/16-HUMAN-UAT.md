---
phase: 16
gathered: 2026-04-27
validated: 2026-04-28
status: validated
scope: minimal — one spot check for FOUND-14 operator-observable
---

# Phase 16 — Human UAT

This file holds the maintainer-validated post-merge checks for Phase 16. Phase 16's deliverables are mostly DB-internal (schema columns, query helper) and code-internal (bug fix + signature change), so most validation lands in automated integration tests (Plans 16-01 / 16-03 / 16-05 / 16-06). The single spot check below covers the FOUND-14 operator-observable that automated tests confirm only with Docker-gated integration tests; the maintainer's eyeballed sanity check on a real homelab DB is the high-confidence signal.

**Project rules in force:**
- D-12: every UAT step references an existing `just` recipe. No ad-hoc `cargo` / `docker` / `curl` / SQL strings.
- D-13: Claude does NOT mark these passed from its own runs. The maintainer runs them locally and confirms results.

## Pre-conditions

- [ ] Phase 16 PRs (PR 1: Plans 16-01..04, PR 2: Plans 16-05..06) merged to `main`.
- [ ] Local dev environment has Docker available (required for the bug-fix observable).
- [ ] Local SQLite dev DB exists (created by a prior `just dev` run; the recipe targets `cronduit.dev.db` in the repo root, matching the existing `db-reset` and `sqlx-prepare` conventions).
- [ ] At least one `type = "docker"` job has fired since the v1.2 commit (the spot check inspects the most recent `job_runs` row).

## Spot Check 1 — FOUND-14: `job_runs.container_id` is a real container ID, not a `sha256:` digest

**Maps to:** Phase 16 Success Criterion 1 — "An operator inspecting a v1.2 docker job run via the database sees `job_runs.container_id` populated with the real Docker container ID (not a `sha256:...` image digest)".

**Why this needs a human:** Automated tests in `tests/v12_run_rs_277_bug_fix.rs` validate this observable but require Docker (`#[ignore]`-gated). The maintainer's local-dev-DB inspection is the canonical sanity check operators will run after upgrade, so we run the same shape.

**Steps:**

1. Ensure a docker job has fired recently against your dev DB. From the repo root, with `cronduit.dev.db` present:

       just uat-fctx-bugfix-spot-check

   This recipe runs `sqlite3 cronduit.dev.db "SELECT id, job_id, status, container_id, image_digest FROM job_runs ORDER BY id DESC LIMIT 1;"` and prints the most recent run's `container_id` and `image_digest`.

2. Inspect the printed `container_id` field for the most recent docker run row:

   - **PASS:** the value is a Docker container ID (typically a 64-char hex string; some daemons print a 12-char prefix). Example: `7f4c9b...` or `f3e8d72c1a5e...`.
   - **PASS (alternate):** the value is `NULL` — only valid for the `running` row of a docker run still in flight, OR for non-docker (`type = "command"` / `type = "script"`) runs.
   - **FAIL:** the value starts with `sha256:` (e.g., `sha256:abc123...`) — this is the v1.0/v1.1 bug observable; if it appears for a v1.2 docker run, the bug regressed. File a P0.

3. Inspect the printed `image_digest` field for the same row:

   - **PASS (docker job, succeeded):** value starts with `sha256:` (the digest from `inspect_container`).
   - **PASS (command/script job):** value is `NULL`.
   - **PASS (docker job where inspect_container failed):** value is `NULL` or empty (the existing fallback path; not a regression).
   - **FAIL:** value is a non-`sha256:` non-NULL string for a docker run — would indicate the parallel local in run.rs:301 swapped wrong values.

**Resolution:**

- All three cells PASS → record the result below; mark Phase 16 ready for `/gsd-verify-work`.
- Any FAIL → file an issue tagged `phase-16-regression`; do NOT merge to `main` until resolved.

## Maintainer Validation Result

| Date validated | Validator | Spot Check 1 (container_id) | Spot Check 1 (image_digest) | Notes |
|----------------|-----------|------------------------------|------------------------------|-------|
| 2026-04-28     | Robert    | PASS — 64-char hex (e.g. `1f1d24abb750…` on run id=119)  | PASS — `sha256:5b10f432…`        | Inspected via `sqlite3 cronduit.db` filtered to `j.job_type = 'docker'`; 3 consecutive `spot-check-docker` runs (id=114/116/119) all show real container IDs. NOTE: `just uat-fctx-bugfix-spot-check` recipe targets `cronduit.dev.db` while `cronduit run` writes to `cronduit.db` by default — recipe-path mismatch logged as follow-up todo. |

## Notes for the maintainer

- **Historical rows:** rows from v1.0/v1.1 will still show `sha256:` in `container_id` (the historical bug deviation). These age out via the Phase 6 retention pruner (90-day default). Per FOUND-14, no data migration is performed — only NEW v1.2 docker rows must show real container IDs. If the most recent row in your DB is pre-v1.2, fire a fresh docker job (`just dev`, then trigger via dashboard "Run Now") before re-running the spot check.

- **`just uat-fctx-bugfix-spot-check` recipe:** added in Plan 16-04. If the recipe is missing, Plan 16-04 did not land cleanly; check `git log --oneline -- justfile`.

- **No other UAT scenarios are needed for Phase 16.** Success Criteria 2 (config_hash distinct across reload-mid-fire) and 3 (EXPLAIN uses indexed access) are covered by automated tests in Plans 16-04 / 16-05 / 16-06 — validated entirely by `just nextest`.
