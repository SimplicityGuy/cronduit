---
phase: 17-custom-docker-labels-seed-001
plan: 01
subsystem: config
tags: [docker, labels, toml, serde, hashmap, sqlx, bollard]

# Dependency graph
requires:
  - phase: 15-foundation-preamble
    provides: foundation hygiene (cargo-deny posture, CI matrix); Phase 17 inherits with no additions
provides:
  - "labels: Option<HashMap<String, String>>" field on DefaultsConfig + JobConfig + DockerJobConfig
  - serialize_config_json includes labels (Layer 2)
  - compute_config_hash includes labels — change-detection contract pinned by hash_differs_on_labels_change (Layer 3)
  - apply_defaults merges defaults+per-job labels with per-job-wins on collision; use_defaults=false replaces (Layer 4)
  - Five-layer parity round-trip pinned by EXTENDED parity_with_docker_job_config_is_maintained + sibling parity_labels_round_trip_through_docker_job_config
  - Forward-pinning contract for Plan 17-02's LBL-04 error formatter (operator-key set-diff recoverability)
affects: [17-02-validators, 17-03-bollard-plumb-through, 17-04-examples-readme, 17-05-integration-tests, 17-06-seed-closeout]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Five-layer parity invariant — every new field that flows to the executor MUST land on JobConfig + serialize_config_json + compute_config_hash + apply_defaults + DockerJobConfig in lock-step"
    - "Plan 17-02 forward-pinning shape test — when a downstream plan needs a contract from this plan's data (operator-key recoverability via set-diff), pin it with a test in this plan rather than re-discovering it later"

key-files:
  created: []
  modified:
    - src/config/mod.rs
    - src/config/defaults.rs
    - src/config/hash.rs
    - src/config/validate.rs
    - src/scheduler/docker.rs
    - src/scheduler/sync.rs
    - tests/scheduler_integration.rs

key-decisions:
  - "labels merge runs UNCONDITIONALLY on apply_defaults — no `is_non_docker` gate. The LBL-04 type-gate validator (Plan 17-02) handles the type-mismatch case explicitly; gating here would silently drop defaults labels for command/script jobs and mask the validator's intended error."
  - "labels values are NOT secrets (per CONTEXT.md `<code_context>`), so labels are included in compute_config_hash. An operator's label edit produces a different config_hash so sync_config_to_db classifies the row as `updated`."
  - "DockerJobConfig has zero literal sites in the codebase — only the struct definition exists. Step 0 audit (per BLOCKER #3) confirmed no per-site DockerJobConfig edit is needed; the new field's #[serde(default)] handles deserialize-time absorption."

patterns-established:
  - "Five-layer parity field-add: enumerate JobConfig + DefaultsConfig literal sites with grep, walk every site adding the new field, build to confirm compile-time fan-out completeness, add a parity round-trip test as the structural guard"
  - "WARNING-style forward-pin tests document multi-plan contract surfaces inline so the next plan's executor (Plan 17-02 here) inherits the contract via test failure rather than via plan-doc archaeology"

requirements-completed:
  - LBL-01
  - LBL-02

# Metrics
duration: 28min
completed: 2026-04-29
---

# Phase 17 Plan 01: Five-Layer Labels Plumbing Parity (LBL-01 / LBL-02) Summary

**Labels field plumbed end-to-end (parse → merge → serialize → hash → deserialize) across DefaultsConfig + JobConfig + DockerJobConfig with apply_defaults per-job-wins merge and a parity round-trip regression test as the structural guard.**

## Performance

- **Duration:** ~28 min
- **Started:** 2026-04-29T00:17:00Z (approximate)
- **Completed:** 2026-04-29T00:44:55Z
- **Tasks:** 4
- **Files modified:** 7

## Accomplishments

- `labels: Option<HashMap<String, String>>` added to all three load-bearing structs (`DefaultsConfig`, `JobConfig`, `DockerJobConfig`) with `#[serde(default)]` so omitting the field deserializes to `None`.
- All seven+ `JobConfig { ... }` literal sites and all DefaultsConfig literal sites updated with `labels: None,` in lock-step — `cargo build --all-targets` is the compile-time fan-out check.
- `serialize_config_json` writes labels to the DB `config_json` blob (Layer 2). `compute_config_hash` includes labels (Layer 3) so change-detection works.
- `apply_defaults` merges defaults+per-job labels with per-job-wins on collision; `use_defaults=false` short-circuit cleanly drops defaults labels (Layer 4 — LBL-02).
- Five-layer parity round-trip is structurally guarded by an EXTENDED `parity_with_docker_job_config_is_maintained` PLUS a sibling test for explicit multi-key + dotted-key coverage. Future field-add reviews see labels as a first-class parity check.
- Forward-pin test for Plan 17-02's LBL-04 error formatter contract: confirms the merge runs unconditionally on non-docker jobs and that operator-only keys are recoverable downstream by set-diffing against the defaults map.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add labels field to DefaultsConfig + JobConfig + DockerJobConfig (Layers 1, 5)** — `283b3e1` (feat)
2. **Task 2: Add labels to serialize_config_json + compute_config_hash (Layers 2, 3)** — `c2212f5` (feat)
3. **Task 3: apply_defaults labels merge with per-job-wins (Layer 4 / LBL-02)** — `99bf7d9` (feat)
4. **Task 4: Five-layer parity guard — EXTEND existing parity test + add sibling labels parity test** — `d125fff` (test)

## Files Created/Modified

- `src/config/mod.rs` — `labels: Option<HashMap<String, String>>` on `DefaultsConfig` + `JobConfig`
- `src/config/defaults.rs` — apply_defaults labels merge (LBL-02); 3 new merge tests; EXTENDED parity test; sibling parity test
- `src/config/hash.rs` — labels included in compute_config_hash; new `hash_differs_on_labels_change` test; DefaultsConfig literals updated with `labels: None,`
- `src/config/validate.rs` — `stub_job` test fixture updated with `labels: None,`
- `src/scheduler/docker.rs` — `labels: Option<HashMap<String, String>>` on `DockerJobConfig`
- `src/scheduler/sync.rs` — labels written to config_json in `serialize_config_json`; test fixtures updated
- `tests/scheduler_integration.rs` — `make_job` helper updated with `labels: None,`

## Decisions Made

- **No `is_non_docker` gate on the labels merge in `apply_defaults`** — diverges from the volumes/image/network/delete merges. Per RESEARCH.md `<code-examples>` (canonical shape) and CONTEXT.md `<decisions>` D-01: the LBL-04 type-gate validator (Plan 17-02) handles the type-mismatch case explicitly. Gating here would silently drop defaults labels for command/script jobs and mask the validator's intended error.
- **Labels are NOT secrets** — they are included in `compute_config_hash`. An operator's label edit produces a different hash so `sync_config_to_db` classifies the row as `updated`.
- **`mk_job()` over `mk_docker_job()` for `hash_differs_on_labels_change`** — the plan's example used the inner-scoped `mk_docker_job()` helper, but that helper lives inside another test function and is not visible at the outer test scope. Using the outer `mk_job()` mirrors `hash_differs_on_cmd_change` exactly and produces the same pass/fail signal (two jobs differing only in labels must hash differently).
- **The third Task 3 test was rewritten to avoid `JobConfig::clone()`** — JobConfig only derives `Debug, Deserialize` (no Clone). The plan's example called `job.clone()`; instead, the test snapshots the defaults-key set BEFORE moving the map into DefaultsConfig, then uses that snapshot for the set-diff assertion. Same intent, no Clone requirement.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] clippy::unnecessary_get_then_check on `labels.get(...).is_none()`**
- **Found during:** Task 4 verification (`just clippy`)
- **Issue:** Clippy 1.94 flags `labels.get("watchtower.enable").is_none()` as preferring `!labels.contains_key("watchtower.enable")`. CI runs with `-D warnings`, so this would block CI.
- **Fix:** Rewrote the assertion in `apply_defaults_use_defaults_false_replaces_labels` to use `!labels.contains_key(...)`.
- **Files modified:** `src/config/defaults.rs`
- **Verification:** `just clippy` exits 0.
- **Committed in:** `d125fff` (Task 4 commit, alongside the parity-test changes — same commit because both surfaced from the same `just clippy` run).

**2. [Rule 3 — Blocking] rustfmt re-broke an `assert_eq!` line in the sibling parity test**
- **Found during:** Task 4 verification (`just fmt-check`)
- **Issue:** The single-line `assert_eq!(djc_labels, job_labels, "round-trip labels must equal source");` exceeded rustfmt's line-width budget after a tab indentation, forcing a multi-line break.
- **Fix:** Ran `just fmt` — rustfmt rewrote it to a 4-line form.
- **Files modified:** `src/config/defaults.rs`
- **Verification:** `just fmt-check` exits 0.
- **Committed in:** `d125fff` (Task 4 commit, alongside the parity-test changes).

**3. [Rule 1 — Bug] `JobConfig` does not derive `Clone`**
- **Found during:** Task 3 (LBL-04 forward-pin test implementation)
- **Issue:** The plan's example for `lbl_04_error_does_not_leak_defaults_keys_for_non_docker_jobs` called `job.clone()` and `defaults.labels.as_ref().unwrap().contains_key(*k)` (the latter requires the `defaults` binding to outlive the merge call). `JobConfig` derives `Debug, Deserialize` only — no `Clone` — so `job.clone()` does not compile.
- **Fix:** Rewrote to (a) avoid `job.clone()` (let `apply_defaults` consume `job` directly) and (b) snapshot the defaults-key set as a `HashSet<String>` BEFORE moving the map into `DefaultsConfig`. The set-diff assertion uses the snapshot. Same test intent (operator-only keys are recoverable downstream); no Clone requirement.
- **Files modified:** `src/config/defaults.rs` (test code)
- **Verification:** `cargo test --lib config::defaults::tests::lbl_04_error_does_not_leak_defaults_keys_for_non_docker_jobs -- --exact` exits 0.
- **Committed in:** `99bf7d9` (Task 3 commit).

**4. [Rule 3 — Blocking] More `JobConfig` and `DefaultsConfig` literal sites than the plan enumerated**
- **Found during:** Task 1 (compile-time fan-out)
- **Issue:** The plan listed 7 `JobConfig { ... }` literal sites; my Step 0 grep found 9 (including 2 in `src/scheduler/sync.rs::tests` and 1 in `tests/scheduler_integration.rs`). Additionally, the plan did not flag DefaultsConfig literal sites, but the new `labels` field on DefaultsConfig forces all 13 DefaultsConfig literals (8 in defaults.rs::tests + 5 in hash.rs::tests) to be updated as well — the compiler would otherwise reject the build.
- **Fix:** Walked every site found by `grep -rn "JobConfig {" src/ tests/` and `grep -rn "DefaultsConfig {" src/ tests/` and added `labels: None,` (mirror of the `cmd: None,` placement). The compile-time fan-out check (`cargo build --all-targets`) is the safety net.
- **Files modified:** `src/config/defaults.rs`, `src/config/hash.rs`, `src/config/validate.rs`, `src/scheduler/sync.rs`, `tests/scheduler_integration.rs`
- **Verification:** `cargo build --all-targets` exits 0; `cargo test --lib`: 202 passed.
- **Committed in:** `283b3e1` (Task 1 commit).

---

**Total deviations:** 4 auto-fixed (3 blocking, 1 bug)
**Impact on plan:** All four were necessary to keep the build + clippy + fmt + tests green. No scope creep — every fix stayed inside the plan's stated files. The literal-site overshoot (deviation #4) is the most material — the actual fan-out is larger than the plan's pre-flight count anticipated, but the safety net (compile-time refusal) caught it before runtime.

## Issues Encountered

- **`JobConfig` lack of `Clone` derive** — surfaced during Task 3 test writing; fixed by snapshotting the relevant key set before moving the value. Documented in deviations.
- **None other** — plan was technically tight; the plan's "verbatim" code blocks lined up with the actual codebase shape after minor adaptations for `Clone`/clippy/fmt.

## Next Phase Readiness

- **Plan 17-02 (validators) inputs are ready:**
  - `JobConfig.labels` and `DefaultsConfig.labels` fields exist for the validator to read.
  - The forward-pin test `lbl_04_error_does_not_leak_defaults_keys_for_non_docker_jobs` documents the contract Plan 17-02's LBL-04 error formatter must satisfy: the operator-only key set is recoverable by set-diffing the merged map against `[defaults].labels`.
  - The merge runs unconditionally for non-docker jobs (per Decision above), so Plan 17-02's validator can rely on labels being present in the merged view.
- **Plan 17-03 (bollard plumb-through) inputs are ready:**
  - `DockerJobConfig.labels` is in place and round-trips correctly through `serialize_config_json` → `config_json` → `serde_json::from_str` (pinned by parity tests).
  - The label-build site at `src/scheduler/docker.rs:157-160` is unchanged — Plan 17-03 will extend it to merge `config.labels` into the cronduit-internal labels map.
- **No blockers.**

## Self-Check: PASSED

Files claimed in this summary verified to exist:
- `src/config/mod.rs` — FOUND (modified)
- `src/config/defaults.rs` — FOUND (modified)
- `src/config/hash.rs` — FOUND (modified)
- `src/config/validate.rs` — FOUND (modified)
- `src/scheduler/docker.rs` — FOUND (modified)
- `src/scheduler/sync.rs` — FOUND (modified)
- `tests/scheduler_integration.rs` — FOUND (modified)

Commits claimed in this summary verified to exist:
- `283b3e1` (Task 1) — FOUND
- `c2212f5` (Task 2) — FOUND
- `99bf7d9` (Task 3) — FOUND
- `d125fff` (Task 4) — FOUND

---
*Phase: 17-custom-docker-labels-seed-001*
*Completed: 2026-04-29*
