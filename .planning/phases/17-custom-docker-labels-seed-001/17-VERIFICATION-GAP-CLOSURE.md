---
phase: 17-custom-docker-labels-seed-001
verified: 2026-04-29T00:00:00Z
status: passed
score: 5/5 must-haves verified
re_verification:
  previous_status: human_needed
  previous_score: 3/5 fully verified (2 partial — CR-01 + CR-02 BLOCKER gaps)
  gaps_closed:
    - "CR-01: README + docstrings now accurately describe whole-file textual interpolation; false absolute guarantee removed; two new tests pin both env-set and env-unset key-position contracts"
    - "CR-02: check_labels_only_on_docker_jobs rewrote with set-diff; Branch B emits accurate use_defaults = false remediation; defaults keys do not leak into error; three unit tests + one integration test pin all branches"
    - "Tracking-table drift: LBL-01..LBL-06 flipped from Pending to Complete in REQUIREMENTS.md"
  gaps_remaining: []
  regressions: []
supersedes: 17-VERIFICATION.md (gap-tracking section only; original report is historical record)
---

# Phase 17: Custom Docker Labels — Gap-Closure Verification Report

**Phase Goal:** Operators can attach arbitrary Docker labels to cronduit-spawned containers (Traefik, Watchtower, backup tooling interop) with locked merge semantics, a reserved cronduit.* namespace, and type-gated validation.

**Verified:** 2026-04-29
**Status:** COMPLETE — all gaps closed; no regressions
**Re-verification:** Yes — after gap closure plans 17-07 (CR-01), 17-08 (CR-02), 17-09 (bookkeeping)
**Supersedes:** The gap-tracking section of `17-VERIFICATION.md`. The original report is preserved as historical evidence.

---

## Gap-Closure Verdicts

### CR-01 — LBL-05 Doc Contract: README False Absolute Guarantee

**Original gap:** README line 257 stated "Label KEYS are NEVER interpolated" — a false absolute. The whole-file textual interpolation pass at `src/config/interpolate.rs` operates on raw TOML text before parsing, treating key positions identically to value positions. When `TEAM=ops` is set and the operator writes `labels = { "${TEAM}" = "v" }`, the key resolves to `"ops"` before the validator runs and the load succeeds. The BLOCKER was the mismatch between the documented guarantee and the binary's actual behavior.

**Fix applied (Option A — Relax docs, plan 17-07):** Documentation-only; no semantic changes.

**Verdict: CLOSED**

Evidence:

| Check | Result |
| ----- | ------ |
| `grep -c 'Label KEYS are NEVER interpolated' README.md` | 0 — false claim removed |
| `grep -c 'whole-file textual' README.md` | 1 — accurate replacement prose present at line 257 |
| `grep -c 'Partially enforces LBL-05' src/config/validate.rs` | 0 — false docstring claim removed |
| `grep -c 'WHOLE-FILE TEXTUAL REPLACEMENT' src/config/interpolate.rs` | 1 — interpolate docstring updated |
| `lbl_05_key_position_interpolation_env_set_resolves_to_literal_when_pattern_matches` | PASS — env-set key resolves to `"ops"`, load succeeds |
| `lbl_05_key_position_interpolation_env_unset_caught_by_strict_chars` | PASS — TEAM unset produces load failure with `missing environment variable` error |
| Binary: `TEAM=ops cronduit check <toml-with-${TEAM}-as-key>` | Exit 0 — behavior unchanged; now accurately documented |
| Binary: `TEAM unset cronduit check <toml-with-${TEAM}-as-key>` | Exit 1 — two errors: `missing environment variable TEAM` + `invalid label keys` |

**README prose (post-fix, lines 257-280):** The env-var paragraph now explicitly enumerates four cases: value-side env-set (accepted), value-side env-unset (fails at interpolation), key-side env-set (resolves and accepted if pattern matches), key-side env-unset (fails at interpolation then strict-char). A "Recommended pattern" section steers operators toward value-only interpolation and labels key-side use as "SUPPORTED BUT DISCOURAGED." The mermaid diagram, merge-semantics table, and other rule paragraphs are untouched.

**`check_label_key_chars` docstring (validate.rs lines 323-344):** Drops the false "Partially enforces LBL-05's keys-not-interpolated" claim. Accurately describes D-02 character enforcement on the post-interpolation resolved key string. Explicitly documents both env-set and env-unset cases.

**`interpolate::interpolate` docstring (interpolate.rs lines 17-27):** States the pass is "a WHOLE-FILE TEXTUAL REPLACEMENT" that "does NOT respect TOML structure." Warns downstream consumers not to assume keys are exempt from interpolation. Points to `check_label_key_chars` as the post-resolution character validator.

**LBL-05 contract (updated):** LBL-05's "keys are NOT interpolated" intent is now expressed as: the strict char regex catches any leftover `${`/`}` chars in the env-unset case; the env-set case resolves the key and accepts it if it matches the strict pattern. This is a valid Option-A resolution per the maintainer's locked decision recorded in `17-VERIFICATION.md` § Human Verification Required #2.

---

### CR-02 — LBL-04 Error Mis-attribution: Defaults-Only Command Job

**Original gap:** When `[defaults].labels` is set and a command/script job has no `use_defaults = false`, `apply_defaults` (intentionally) merges defaults labels into the job. The LBL-04 validator then fired "Remove the `labels` block" — a message that attributed blame to a block the operator never wrote. The correct fix is `use_defaults = false`, which the error never mentioned.

**Fix applied (Option A — Set-diff in formatter, plan 17-08):** `check_labels_only_on_docker_jobs` rewritten with set-diff against `cfg.defaults.labels`. Two branches now emerge. `apply_defaults` merge semantics are UNCHANGED.

**Verdict: CLOSED**

Evidence:

| Check | Result |
| ----- | ------ |
| `check_labels_only_on_docker_jobs` signature | `(job, defaults_labels: Option<&HashMap<String, String>>, path, errors)` — set-diff arg added |
| Branch A (operator-set): legacy message preserved | `"Remove the \`labels\` block"` text present verbatim — backwards-compatible |
| Branch B (defaults-only): new remediation | `"set \`use_defaults = false\`"` present; `"Remove the \`labels\` block"` absent |
| Set-diff: defaults keys absent from error | Confirmed by `lbl_04_command_job_with_defaults_only_emits_distinct_use_defaults_false_message` and `lbl_04_defaults_only_command_job_emits_use_defaults_false_remediation` |
| `apply_defaults` unchanged | `defaults.rs:166-176` unmodified — labels merge gate identical to pre-gap-closure |
| Binary: `[defaults].labels` + bare command job | Exit 1; message: `"this is a command job; labels are docker-only. \`[defaults].labels\` is set and would be merged into this job by \`apply_defaults\` — set \`use_defaults = false\` on this job to opt out, OR remove \`[defaults].labels\`."` |
| Binary: `"Remove the \`labels\` block"` absent from Branch B error | Confirmed (binary output above does not contain the phrase) |
| `"watchtower.enable"` absent from Branch B error | Confirmed — set-diff hides the defaults key |

**Unit tests added (validate.rs `mod tests`):**

1. `lbl_04_command_job_with_operator_set_labels_emits_legacy_message` — Branch A; legacy text present; Branch B phrase absent.
2. `lbl_04_command_job_with_defaults_only_emits_distinct_use_defaults_false_message` — Branch B; remediation present; `"Remove the \`labels\` block"` absent; defaults key absent (no leak); job type discriminator ("command job") present.
3. `lbl_04_command_job_with_mixed_operator_and_defaults_emits_legacy_message_only_for_operator_keys` — Mixed case; Branch A wins; defaults key absent from error.

**Integration test added (tests/v12_labels_use_defaults_false.rs):**

4. `lbl_04_defaults_only_command_job_emits_use_defaults_false_remediation` — End-to-end through `parse_and_validate`; non-`#[ignore]` (no Docker daemon); asserts Branch B phrase present, Branch A phrase absent, job name present, defaults key absent.

---

### Tracking-Table Drift — REQUIREMENTS.md LBL-01..LBL-06

**Original gap (Info-level):** REQUIREMENTS.md lines 186-191 showed LBL-01..LBL-06 as `Pending` despite the implementation shipping and UAT passing.

**Fix applied (plan 17-09):** Six rows flipped from `Pending` to `Complete`.

**Verdict: CLOSED**

Evidence:

| Check | Result |
| ----- | ------ |
| `grep -cE '^\| LBL-0[1-6]\s+\| 17\s+\| Complete \|' .planning/REQUIREMENTS.md` | 6 — all six rows flipped |
| `grep -cE '^\| LBL-0[1-6]\s+\| 17\s+\| Pending \|' .planning/REQUIREMENTS.md` | 0 — no Pending rows remain |

---

## Observable Truths — Updated Status

| # | Truth (Success Criterion) | Original Status | Gap-Closure Status | Evidence |
| - | ------------------------- | --------------- | ------------------ | -------- |
| 1 | SC-1: Operator labels reach spawned container; cronduit.run_id + cronduit.job_name intact | ✓ VERIFIED | ✓ VERIFIED | Unchanged; UAT U5 passed by maintainer 2026-04-29 |
| 2 | SC-2: use_defaults=false replaces defaults; otherwise per-job-wins merge | ✓ VERIFIED | ✓ VERIFIED | Unchanged; apply_defaults unmodified by gap-closure plans |
| 3 | SC-3: cronduit.* key yields config-load error naming the offending key | ✓ VERIFIED | ✓ VERIFIED | Unchanged |
| 4 | SC-4 (LBL-04): labels on command/script job yields clear error with correct attribution | PARTIAL (CR-02) | ✓ VERIFIED | Branch B now emits `use_defaults = false` remediation; Branch A preserves legacy text; binary confirmed exit 1 with correct message |
| 5 | SC-5 (LBL-05): ${VAR} interpolated in label VALUES; key-side behavior documented; >4 KB / >32 KB rejected | PARTIAL (CR-01) | ✓ VERIFIED | README + docstrings now accurately describe whole-file textual pass; both env-set and env-unset key-position contracts pinned by tests; size limits unchanged |

**Score:** 5/5 truths verified.

---

## Updated Requirements Coverage

| Requirement | Source Plan(s) | Description | Previous Status | Current Status | Evidence |
| ----------- | -------------- | ----------- | --------------- | -------------- | -------- |
| LBL-01 | 17-01, 17-03, 17-04, 17-05 | `labels` field on JobConfig+DefaultsConfig+DockerJobConfig; merged into bollard | ✓ SATISFIED | ✓ SATISFIED | Unchanged |
| LBL-02 | 17-01, 17-03, 17-04, 17-05 | use_defaults=false replaces; otherwise per-job-wins-on-collision merge | ✓ SATISFIED | ✓ SATISFIED | Unchanged; apply_defaults unmodified |
| LBL-03 | 17-02, 17-05 | cronduit.* namespace rejected at LOAD | ✓ SATISFIED | ✓ SATISFIED | Unchanged |
| LBL-04 | 17-02, 17-05, **17-08** | labels on command/script jobs rejected at LOAD with clear, correctly-attributed error | PARTIAL (CR-02) | ✓ SATISFIED | Set-diff in `check_labels_only_on_docker_jobs`; Branch B emits `use_defaults = false` remediation; 3 unit tests + 1 integration test pin all branches; binary confirmed |
| LBL-05 | 17-03, 17-05, **17-07** | `${VAR}` interpolation in label VALUES; key-side behavior accurately documented; size limits enforced | PARTIAL (CR-01) | ✓ SATISFIED | README + validator + interpolate docstrings updated; 2 new integration tests pin env-set and env-unset key-position contracts; binary confirmed both paths |
| LBL-06 | 17-02, 17-05 | per-value ≤ 4 KB; per-set ≤ 32 KB enforced at LOAD | ✓ SATISFIED | ✓ SATISFIED | Unchanged |

---

## Test Gate

| Suite | Command | Result |
| ----- | ------- | ------ |
| Library unit tests | `cargo nextest run --lib` | 218/218 passed, 0 failed |
| Full suite (lib + integration) | `cargo nextest run` | 419/419 passed, 26 skipped (docker `#[ignore]`), 0 failed |
| CR-01 key-position tests | `cargo test --test v12_labels_interpolation -- --test-threads=1 lbl_05_key_position` | 2/2 passed |
| CR-02 Branch B integration test | `cargo test --test v12_labels_use_defaults_false -- --test-threads=1 lbl_04_defaults_only_command_job_emits_use_defaults_false_remediation` | 1/1 passed |
| Clippy | `cargo clippy --all-targets -- -D warnings` | 0 warnings, exit 0 |
| Formatting | `cargo fmt --check` | exit 0 |

Note on test count: The objective specified 423 expected tests; the actual post-merge suite runs 419 tests (26 skipped `#[ignore]` docker tests not counted in this total). The 419 count includes all non-ignored lib and integration tests. No tests regressed.

---

## Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| SC-5 env-set key (CR-01 Option A): `TEAM=ops` key resolves and is accepted | `TEAM=ops cronduit check /tmp/cr01_key_set.toml` | `ok: /tmp/cr01_key_set.toml` (exit 0) | PASS |
| SC-5 env-unset key (CR-01): `TEAM` unset, literal `${TEAM}` rejected | `env -u TEAM cronduit check /tmp/cr01_key_set.toml` | Exit 1; two errors: `missing environment variable 'TEAM'` + `invalid label keys: .` | PASS |
| SC-4 attribution (CR-02 Branch B): `[defaults].labels` + bare command job | `cronduit check /tmp/cr02_test.toml` | Exit 1; `"this is a command job; labels are docker-only. \`[defaults].labels\` is set... set \`use_defaults = false\`..."` — Branch A phrase absent; defaults key absent | PASS |

---

## Anti-Patterns — Gap-Closure Plans Only

No new BLOCKER or WARNING anti-patterns introduced by plans 17-07, 17-08, or 17-09.

- Plans 17-07 and 17-08 touched documentation and a validator function body respectively; both are clippy-clean and fmt-clean.
- Plan 17-09 was a single-file documentation edit (REQUIREMENTS.md); no code touched.
- The WR-* warnings from the original verification report (WR-01 through WR-05) were explicitly out of scope for these gap-closure plans and are NOT re-flagged here.

---

## Regressions

None detected.

- `apply_defaults` semantics are identical to pre-gap-closure (merge gate unmodified; labels merge at defaults.rs:166-176 unchanged).
- Branch A of `check_labels_only_on_docker_jobs` preserves the legacy "Remove the `labels` block" text verbatim — backwards-compatible with existing CI grep and operator scripts.
- All 419 non-ignored tests pass.
- Clippy and fmt gates remain green.

---

## Phase Verdict

**COMPLETE.**

All three gap-closure items are closed with observable, testable evidence:

1. **CR-01 (LBL-05):** README, `check_label_key_chars` docstring, and `interpolate::interpolate` docstring now accurately describe the whole-file textual interpolation pass. Two new non-`#[ignore]` integration tests pin both the env-set (accepted, resolves to literal) and env-unset (rejected at load) key-position contracts. The binary behavior is unchanged; the documentation now matches it.

2. **CR-02 (LBL-04):** `check_labels_only_on_docker_jobs` set-diffs against `cfg.defaults.labels` to distinguish operator-set keys from defaults-merged keys. Branch B emits the correct `use_defaults = false` remediation. Defaults keys do not leak into the error. Three unit tests and one integration test pin all branches. The binary confirms the new error message on the failing path.

3. **Bookkeeping drift:** LBL-01..LBL-06 in `.planning/REQUIREMENTS.md` show `Complete` (six rows flipped; zero `Pending` rows remain).

Phase 17 is ready for the v1.2.0 release tag. The WR-* warnings from the original verification report (WR-01: defaults-only label validation gap; WR-02/WR-03: test-isolation paper-cuts; WR-04: serialize unwrap_or_default; WR-05: no per-field defaults opt-out) remain open but are correctly categorized as non-blocking for the feature goal. They should be tracked for v1.2.1 or v1.3 as appropriate.

---

_Verified: 2026-04-29_
_Verifier: Claude (gsd-verifier)_
_Re-verification of: 17-VERIFICATION.md (gap-closure wave: plans 17-07, 17-08, 17-09)_
