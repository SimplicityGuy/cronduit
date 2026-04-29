---
phase: 17-custom-docker-labels-seed-001
plan: 03
subsystem: scheduler
tags: [docker, labels, bollard, integration-tests, testcontainers, end-to-end]

# Dependency graph
requires:
  - phase: 17-custom-docker-labels-seed-001
    plan: 01
    provides: "DockerJobConfig.labels field; serialize_config_json includes labels; apply_defaults merges labels; five-layer parity invariant"
  - phase: 17-custom-docker-labels-seed-001
    plan: 02
    provides: "check_label_reserved_namespace, check_labels_only_on_docker_jobs, check_label_size_limits, check_label_key_chars validators (LBL-03 / LBL-04 / LBL-06 / D-02)"
provides:
  - "Operator labels reach the spawned container's bollard ContainerCreateBody.labels (LBL-01 end-to-end)"
  - "Cronduit-internal labels (cronduit.run_id, cronduit.job_name) win on collision via insert-after ordering — defense-in-depth complement to LBL-03 validator"
  - "tests/v12_labels_merge.rs — pins defaults+per-job merge round-trip (LBL-01 / LBL-02 / SC-1 / SC-2)"
  - "tests/v12_labels_use_defaults_false.rs — pins use_defaults=false replace semantic at the apply_defaults short-circuit (LBL-02 / SC-2)"
  - "tests/v12_labels_interpolation.rs — pins ${VAR} resolution in label VALUES at config-LOAD (LBL-05 / SC-5a)"
  - "src/scheduler/sync.rs::serialize_config_json_for_tests — #[doc(hidden)] pub re-export of the canonical serializer for integration-test crates"
affects: [17-04-examples-readme, 17-05-integration-tests, 17-06-seed-closeout]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Operator-then-internal HashMap insert ordering — operator labels go in FIRST so HashMap::insert of cronduit-internal labels structurally overwrites any (impossible-to-reach via LBL-03) collision. Pairs with the LBL-03 validator at config-LOAD as a two-layer defense."
    - "Doc-hidden public re-export bridge for `pub(crate)` functions whose contract integration tests must pin — lets tests stay on the canonical code path without exposing the function in the public API."
    - "End-to-end integration tests drive parse_and_validate -> apply_defaults -> serialize -> execute_docker -> bollard -> inspect_container so a future drift in any single layer surfaces here, not just in the lib unit tests."

key-files:
  created:
    - tests/v12_labels_merge.rs
    - tests/v12_labels_use_defaults_false.rs
    - tests/v12_labels_interpolation.rs
  modified:
    - src/scheduler/docker.rs
    - src/scheduler/sync.rs

key-decisions:
  - "Took the BLOCKER #4 PREFERRED path — added `#[doc(hidden)] pub fn serialize_config_json_for_tests` to src/scheduler/sync.rs that delegates to the canonical `pub(crate) serialize_config_json`. Trades one extra function in the lib for zero test-fixture drift risk on future field-adds."
  - "Each integration test file is its own crate (Rust convention) — `docker_client()` helper is duplicated verbatim across the three new files, mirroring the project's existing tests/docker_executor.rs pattern. No shared test-helpers module added (out of scope for this plan)."
  - "Container preservation strategy: each test sets `delete = false` in TOML so the container survives long enough to inspect, then issues a best-effort `remove_container(force=true)` at end-of-test. Mirrors the plan's recommended pattern."

requirements-completed:
  - LBL-01
  - LBL-02
  - LBL-05

# Metrics
duration: ~14min
completed: 2026-04-29
---

# Phase 17 Plan 03: Bollard Plumb-Through + Three Integration Tests (LBL-01 / LBL-02 / LBL-05) Summary

**Operator-defined labels now reach the spawned container's bollard `ContainerCreateBody.labels` via a one-site insert-before-internals change in `execute_docker`, with three testcontainers integration tests driving end-to-end through `parse_and_validate` → bollard → `inspect_container` to pin merge / replace / interpolation contracts.**

## Performance

- **Duration:** ~14 min
- **Started:** 2026-04-29T00:00:00Z (worktree start, approximate)
- **Completed:** 2026-04-29 (this commit)
- **Tasks:** 2
- **Files modified:** 2 (src/scheduler/docker.rs, src/scheduler/sync.rs)
- **Files created:** 3 (tests/v12_labels_*.rs)

## Accomplishments

- **Task 1 — Bollard plumb-through.** One-site change at the existing label-build block in `src/scheduler/docker.rs` (`execute_docker`): operator labels (`config.labels`) are inserted FIRST into the labels HashMap; cronduit-internal labels (`cronduit.run_id`, `cronduit.job_name`) are inserted AFTER. On the impossible-due-to-LBL-03-validator collision case, `HashMap::insert` structurally overwrites any operator-supplied `cronduit.*` value (defense-in-depth). 16-line additive change, zero behavior change when `config.labels = None`.
- **Task 2 — Three integration tests.** `tests/v12_labels_merge.rs`, `tests/v12_labels_use_defaults_false.rs`, `tests/v12_labels_interpolation.rs` — all `#[tokio::test] #[ignore]`, all drive end-to-end through `parse_and_validate` (BLOCKER #4 fix), all assert against `inspect_container().config.labels`.
- **Preferred-path serializer re-export.** Added `#[doc(hidden)] pub fn serialize_config_json_for_tests` in `src/scheduler/sync.rs` that delegates to the canonical `pub(crate) serialize_config_json`. The integration tests call the re-export so a future field-add cannot drift their fixture from the production serializer.
- **All verification gates green:** `cargo build --all-targets`, `cargo build --tests`, `cargo test --lib` (215 passed, 0 failed), `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --check` — all exit 0.

## Task Commits

Each task was committed atomically:

1. **Task 1: Plumb operator labels into ContainerCreateBody (LBL-01)** — `3d45c0d` (feat)
2. **Task 2: Three testcontainers integration tests covering merge, replace, ${VAR} interpolation** — `48d64d4` (test)

## Files Created/Modified

- `src/scheduler/docker.rs` — operator labels merged into bollard label HashMap with cronduit-internal-wins ordering. (Picked up rustfmt-preferred single-line `labels.extend(...)` form.)
- `src/scheduler/sync.rs` — added `#[doc(hidden)] pub fn serialize_config_json_for_tests` re-export.
- `tests/v12_labels_merge.rs` — defaults+per-job merge round-trip integration test.
- `tests/v12_labels_use_defaults_false.rs` — `use_defaults=false` replace semantic integration test.
- `tests/v12_labels_interpolation.rs` — `${VAR}` value-only interpolation integration test (LBL-05 owner).

## Decisions Made

- **Preferred path for `pub(crate) serialize_config_json` access** — added `#[doc(hidden)] pub fn serialize_config_json_for_tests` re-export rather than hand-emit the JSON in each test file. Trade-off: one extra function in the lib (intentionally `#[doc(hidden)]` so it does not appear in rendered API docs) vs. zero drift risk on future field-adds. The plan offered both paths; preferred wins because the entire point of the BLOCKER #4 fix is "drive through the canonical pipeline" — mirroring the serializer in tests would re-introduce the very drift the fix is meant to eliminate.
- **`!labels.contains_key("...")` instead of `labels.get("...").is_none()`** — Plan 17-01 already established this clippy-clean idiom (see `apply_defaults_use_defaults_false_replaces_labels` in src/config/defaults.rs). Reused verbatim in `tests/v12_labels_use_defaults_false.rs` so clippy under `-D warnings` is green from the first cargo invocation.
- **`std::env::set_var` + `std::env::remove_var` wrapped in `unsafe` blocks** — required in Rust 1.85+ (Edition 2024). Single-threaded test execution (`--test-threads=1`, project convention for docker tests) makes this safe; documented in the file-level doc comment of `tests/v12_labels_interpolation.rs`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] rustfmt rewrote the new `labels.extend(...)` block in src/scheduler/docker.rs**
- **Found during:** Task 2 verification (`cargo fmt --check`)
- **Issue:** The plan's literal multi-line form for `labels.extend( operator_labels.iter().map(...) )` exceeded rustfmt's narrow-form line-width budget after the surrounding tab indentation, so rustfmt collapsed it to a single line.
- **Fix:** Ran `cargo fmt` to apply rustfmt's preferred form (`labels.extend(operator_labels.iter().map(|(k, v)| (k.clone(), v.clone())));`). Semantically identical.
- **Files modified:** `src/scheduler/docker.rs`
- **Verification:** `cargo fmt --check` exits 0; `cargo clippy --all-targets --all-features -- -D warnings` exits 0.
- **Committed in:** `48d64d4` (Task 2 commit, alongside the new test files — same commit because the rustfmt rewrite surfaced during Task 2's `fmt --check` gate).

**2. [Rule 3 — Blocking] rustfmt collapsed the multi-line `result.container_id.clone()` chains in tests/v12_labels_use_defaults_false.rs and tests/v12_labels_interpolation.rs**
- **Found during:** Task 2 verification (`cargo fmt --check`)
- **Issue:** The plan's verbatim literal had the `.container_id.clone().expect(...)` chain spread across multiple lines; rustfmt prefers a single-line form when it fits.
- **Fix:** Ran `cargo fmt` to apply rustfmt's preferred single-line form. Semantically identical.
- **Files modified:** `tests/v12_labels_use_defaults_false.rs`, `tests/v12_labels_interpolation.rs`.
- **Verification:** `cargo fmt --check` exits 0.
- **Committed in:** `48d64d4` (Task 2 commit).

---

**Total deviations:** 2 auto-fixed (both Rule 3 — blocking format gate). All inside the plan's stated files. No scope creep. No bug fixes needed (the codebase shape lined up with the plan's verbatim code blocks; the only deltas were rustfmt aesthetics).

## Issues Encountered

- **None functional.** Wave 1's parity-test scaffolding and Wave 2's validators meant the bollard plumb-through was a literal one-site change; the integration tests use the canonical `parse_and_validate` + `serialize_config_json_for_tests` path so there's no drift surface.
- **rustfmt aesthetics** caught both deviations above. Both auto-fixed by `cargo fmt` in the same Task 2 verification cycle.

## UAT (per D-09)

The three new integration tests are gated `#[ignore]` per project convention. Maintainer (the user) validates them by running:

```bash
cargo test --test v12_labels_merge -- --ignored --nocapture --test-threads=1
cargo test --test v12_labels_use_defaults_false -- --ignored --nocapture --test-threads=1
cargo test --test v12_labels_interpolation -- --ignored --nocapture --test-threads=1
```

with a live Docker daemon. Claude does NOT self-mark them passed — these tests were authored to be exercised by the user as a UAT gate per Phase 17 / D-09.

## Next Phase Readiness

- **Plan 17-04 (examples + README labels subsection) inputs are ready:**
  - End-to-end LBL-01 / LBL-02 / LBL-05 are now contractually pinned by the v12_labels_*.rs tests, so the README's labels subsection can confidently reference the merge / replace / interpolation semantics as proven behavior.
  - The `serialize_config_json_for_tests` doc-hidden re-export is the canonical helper for any future plan that wants to integration-test against the serializer.
- **Plan 17-05 (integration tests follow-on) inputs are ready:**
  - The `docker_client()` helper pattern is now established in three more integration test files; subsequent integration tests can copy verbatim.
  - The "drive end-to-end through `parse_and_validate`" idiom is established for label-related integration tests.
- **No blockers.**

## Self-Check: PASSED

Files claimed in this summary verified to exist:
- `src/scheduler/docker.rs` — FOUND (modified)
- `src/scheduler/sync.rs` — FOUND (modified)
- `tests/v12_labels_merge.rs` — FOUND (created)
- `tests/v12_labels_use_defaults_false.rs` — FOUND (created)
- `tests/v12_labels_interpolation.rs` — FOUND (created)

Commits claimed in this summary verified to exist:
- `3d45c0d` (Task 1) — FOUND
- `48d64d4` (Task 2) — FOUND

Acceptance criteria verified:
- `grep -c 'if let Some(operator_labels) = &config.labels' src/scheduler/docker.rs` = 1 — PASS
- `grep -c 'labels.extend' src/scheduler/docker.rs` = 1 — PASS
- `labels.extend` line (174) precedes `labels.insert("cronduit.run_id"` (line 176) — PASS (operator-first ordering)
- `cargo build --all-targets` exits 0 — PASS
- `cargo build --tests` exits 0 — PASS
- `cargo test --lib` 215 passed, 0 failed — PASS
- `cargo clippy --all-targets --all-features -- -D warnings` exits 0 — PASS
- `cargo fmt --check` exits 0 — PASS
- All three v12_labels_*.rs files compile under `cargo test --test ... --no-run` — PASS
- Each new test file has 1 `#[tokio::test]` + 1 `#[ignore]` — PASS
- Each new test file calls `inspect_container` and `parse_and_validate` (BLOCKER #4) — PASS
- Each new test file uses `tempfile::NamedTempFile` (BLOCKER #4) — PASS
- Files 1 and 2 use `serialize_config_json_for_tests(...)` (BLOCKER #4 preferred path) — PASS
- `tests/v12_labels_use_defaults_false.rs` has negative `watchtower.enable` assertions — PASS
- `tests/v12_labels_interpolation.rs` has 8 `DEPLOYMENT_ID` references (set + assert + remove + TOML) — PASS

---
*Phase: 17-custom-docker-labels-seed-001*
*Completed: 2026-04-29*
