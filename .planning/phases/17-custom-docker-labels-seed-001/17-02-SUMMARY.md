---
phase: 17-custom-docker-labels-seed-001
plan: 02
subsystem: config
tags: [docker, labels, validation, validators, regex, deterministic-errors]

# Dependency graph
requires:
  - phase: 17-custom-docker-labels-seed-001
    plan: 01
    provides: "labels: Option<HashMap<String, String>> on JobConfig + DefaultsConfig + DockerJobConfig; apply_defaults labels merge; forward-pin contract for LBL-04 error formatter"
provides:
  - "check_label_reserved_namespace validator (LBL-03 / SC-3) — rejects operator labels under cronduit.* namespace"
  - "check_labels_only_on_docker_jobs validator (LBL-04 / SC-4) — rejects labels on command/script jobs"
  - "check_label_size_limits validator (LBL-06 / SC-5b) — enforces 4 KB per-value, 32 KB per-set caps"
  - "check_label_key_chars validator (D-02; partial LBL-05) — strict ASCII regex on label keys"
  - "LABEL_KEY_RE Lazy<Regex> with pattern ^[a-zA-Z0-9_][a-zA-Z0-9._-]*$"
  - "MAX_LABEL_VALUE_BYTES (4 * 1024) and MAX_LABEL_SET_BYTES (32 * 1024) constants"
  - "Determinism guard test pinning alphabetical key ordering across HashMap iteration"
affects: [17-03-bollard-plumb-through, 17-04-examples-readme, 17-05-integration-tests, 17-06-seed-closeout]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Per-job validator emit pattern reused verbatim from check_cmd_only_on_docker_jobs (validate.rs:110-122) — single function takes (&JobConfig, &Path, &mut Vec<ConfigError>); pushes one ConfigError per violation type with line:col=0"
    - "Sort-before-format determinism rule for HashMap-derived offending-key lists — sort the Vec<&str> before .join(\", \") so test assertions on substring positions are flake-resistant on CI"
    - "let-else early return for Option<HashMap>: `let Some(labels) = &job.labels else { return };` is the idiomatic 1-line guard for validators that no-op when the field is absent"

key-files:
  created: []
  modified:
    - src/config/validate.rs

key-decisions:
  - "Strict-ASCII regex `^[a-zA-Z0-9_][a-zA-Z0-9._-]*$` — enforces leading char alphanumeric/underscore (rejects leading dot); body chars include hyphen and dot for traefik.http.routers.x.rule and com.centurylinklabs.watchtower.enable patterns. Rejects spaces, slashes, and the `${`/`}` sequences leaked from a misused interpolation on a key (D-02 + partial LBL-05)."
  - "Two independent size checks in check_label_size_limits — per-value (4 KB) and per-set (32 KB) may both fire for one job. Per D-01 they emit two ConfigErrors when both trigger; aggregation matches the project pattern of one error per violation type (not per offending key)."
  - "Empty-string key is rejected by LABEL_KEY_RE (the regex requires at least one leading-class char) — so the `\"\"` test case is covered by the same validator as `my key`, but the test explicitly asserts it lands inside the same single ConfigError emission. No separate empty-key validator needed."
  - "Per-job total uses `k.len() + v.len()` (clippy::needless_as_bytes flagged the as_bytes().len() form). Rust String::len() returns byte count, so the rewrite is semantically identical for the LBL-06 byte-budget check."

requirements-completed:
  - LBL-03
  - LBL-04
  - LBL-06

# Metrics
duration: ~10min
completed: 2026-04-29
---

# Phase 17 Plan 02: Four LOAD-time Label Validators (LBL-03 / LBL-04 / LBL-06 / D-02) Summary

**Four config-LOAD validators added to src/config/validate.rs (reserved-namespace, type-gate, size limits, strict ASCII key regex) — each emits one ConfigError per job per violation type with offending keys sorted alphabetically for deterministic test output. 13 new unit tests pin accept/reject paths.**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-04-29T00:50:00Z (approximate — after worktree base reset)
- **Completed:** 2026-04-29T01:00:26Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments

- `LABEL_KEY_RE: Lazy<Regex>` declared once with the D-02 pattern `^[a-zA-Z0-9_][a-zA-Z0-9._-]*$` — mirrors the `NETWORK_RE` once_cell idiom at validate.rs:10-13.
- `MAX_LABEL_VALUE_BYTES = 4 * 1024` and `MAX_LABEL_SET_BYTES = 32 * 1024` constants declared at module level.
- Four validator functions added (in this order: reserved-namespace → type-gate → size limits → key chars), each ~15-30 LoC, doc-commented with the requirement ID and the determinism caveat.
- All four registered in the `run_all_checks` per-job loop AFTER `apply_defaults` runs (reusing the established pipeline order — see RESEARCH Pitfall 4).
- Each ConfigError uses `line: 0, col: 0` per D-01 (post-parse idiom — no span available).
- Sort-before-format applied at three sites (`offending.sort()`, `oversized_keys.sort()`, `invalid.sort()`) so test assertions on substring position are deterministic.
- 13 new unit tests added, mirroring the `check_cmd_only_on_docker_jobs_*` style: 4 reserved-namespace, 3 type-gate, 3 size limits, 3 key chars. The lists-multiple-keys-sorted test pins the determinism guard explicitly via substring-position assertions.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add four validator functions + LABEL_KEY_RE Lazy regex + size constants** — `4b8d4d5` (feat)
2. **Task 2: Unit tests for all four validators (~13 tests covering accept/reject paths + determinism)** — `014f45b` (test)

## Files Created/Modified

- `src/config/validate.rs` — added LABEL_KEY_RE static, MAX_LABEL_VALUE_BYTES + MAX_LABEL_SET_BYTES constants, four validator functions, four registration call sites in run_all_checks, 13 new unit tests in the existing mod tests block.

## Decisions Made

- **`String::len()` instead of `String::as_bytes().len()` in the size-budget arithmetic** — clippy 1.94 flags `needless_as_bytes` because `String::len()` already returns byte count (not char count). The plan example used `as_bytes().len()`; rewriting to `.len()` is semantically identical for the LBL-06 byte budget and silences clippy under `-D warnings` (CI gate).
- **Two independent size errors may fire for one job in `check_label_size_limits`** — per the plan + D-01 aggregation rule. If a job has a value > 4 KB *and* a total > 32 KB, both errors emit. The per-set test (`check_label_size_limits_rejects_per_set_over_32kb`) deliberately keeps each value ≤ 4 KB so only the per-set check fires, ensuring the assertion `e.iter().any(|err| err.message.contains("32 KB"))` is robust.
- **Empty-string key handled by `LABEL_KEY_RE` not a dedicated empty-key validator** — the regex `^[a-zA-Z0-9_][a-zA-Z0-9._-]*$` requires at least one leading-class character, so `""` fails to match and is reported as an invalid key. One validator covers space, slash, empty, and leading-dot uniformly.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] clippy::needless_as_bytes on `as_bytes().len()` calls in `check_label_size_limits`**
- **Found during:** Task 1 verification (`cargo clippy --all-targets --all-features -- -D warnings`)
- **Issue:** Clippy 1.94 flags `v.as_bytes().len()` and `k.as_bytes().len() + v.as_bytes().len()` as preferring direct `len()` calls on strings. CI runs with `-D warnings`, so this would block the build.
- **Fix:** Rewrote both arithmetic sites in `check_label_size_limits` from `.as_bytes().len()` to `.len()`. Semantically identical for byte-count budgets (Rust `String::len()` returns byte count, not char count).
- **Files modified:** `src/config/validate.rs`
- **Verification:** `cargo clippy --all-targets --all-features -- -D warnings` exits 0.
- **Committed in:** `4b8d4d5` (Task 1 commit, applied during the same verification cycle).

**2. [Rule 3 — Blocking] rustfmt rewrote two long lines after Tasks 1 and 2**
- **Found during:** Task 1 + Task 2 verification (`cargo fmt --check`)
- **Issue:** rustfmt's line-width budget triggered two rewrites:
  1. Task 1: the per-set `total_bytes` map+sum chain was collapsed from a 4-line form to a single line.
  2. Task 2: the `check_label_reserved_namespace_lists_multiple_keys_sorted` `assert_eq!` second-arg string was wrapped to a multi-line form.
- **Fix:** Ran `cargo fmt` to apply rustfmt's preferred form.
- **Files modified:** `src/config/validate.rs`
- **Verification:** `cargo fmt --check` exits 0; `cargo clippy --all-targets --all-features -- -D warnings` exits 0.
- **Committed in:** Task 1 fix in `4b8d4d5`; Task 2 fix in `014f45b`.

---

**Total deviations:** 2 auto-fixed (both Rule 3 — blocking lint/format gates). Both inside the plan's stated file. No scope creep.

## Issues Encountered

- **None functional** — the plan's verbatim code blocks lined up with the post-Wave-1 codebase shape exactly. Wave 1's `stub_job` fixture already had `labels: None,` so Task 2 tests compiled on first try.
- **Clippy + fmt CI gates** are stricter than the plan's surface-level "compiles cleanly" — both deviations above are toolchain-level adjustments, not logic bugs.

## Next Phase Readiness

- **Plan 17-03 (bollard plumb-through) inputs are ready:**
  - Operator-supplied label config that reaches `apply_defaults` is now guaranteed to satisfy: no `cronduit.*` keys, only on docker jobs, ≤ 4 KB per value + ≤ 32 KB total, keys match the strict ASCII regex.
  - The label-build site at `src/scheduler/docker.rs:157-160` can rely on these invariants when merging operator labels into the cronduit-internal label map (no defensive re-checks needed there).
- **No blockers.**

## Self-Check: PASSED

Files claimed in this summary verified to exist:
- `src/config/validate.rs` — FOUND (modified)

Commits claimed in this summary verified to exist:
- `4b8d4d5` (Task 1) — FOUND
- `014f45b` (Task 2) — FOUND

Acceptance criteria verified:
- `grep -c 'static LABEL_KEY_RE: Lazy<Regex>' src/config/validate.rs` = 1 — PASS
- `grep -c 'MAX_LABEL_VALUE_BYTES: usize = 4 \* 1024' src/config/validate.rs` = 1 — PASS
- `grep -c 'MAX_LABEL_SET_BYTES: usize = 32 \* 1024' src/config/validate.rs` = 1 — PASS
- `grep -c 'fn check_label_reserved_namespace' src/config/validate.rs` = 1 — PASS
- `grep -c 'fn check_labels_only_on_docker_jobs' src/config/validate.rs` = 1 — PASS
- `grep -c 'fn check_label_size_limits' src/config/validate.rs` = 1 — PASS
- `grep -c 'fn check_label_key_chars' src/config/validate.rs` = 1 — PASS
- `grep -c 'check_label_reserved_namespace(job, path, errors)' src/config/validate.rs` = 1 — PASS (registration site)
- `grep -c 'oversized_keys.sort\|invalid.sort\|offending.sort' src/config/validate.rs` = 3 — PASS (determinism rule applied at all three list-emitting sites)
- `grep -c '#\[test\]' src/config/validate.rs` = 26 (was 13 — added 13, exceeds required +12) — PASS
- `cargo build --all-targets` exits 0 — PASS
- `cargo clippy --all-targets --all-features -- -D warnings` exits 0 — PASS
- `cargo fmt --check` exits 0 — PASS
- `cargo test --lib config::validate` 26 passed; 0 failed — PASS
- `just test` 0 failures across all suites — PASS

---
*Phase: 17-custom-docker-labels-seed-001*
*Completed: 2026-04-29*
