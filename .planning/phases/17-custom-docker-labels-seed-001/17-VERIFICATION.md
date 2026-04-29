---
phase: 17-custom-docker-labels-seed-001
verified: 2026-04-29T01:53:49Z
status: human_needed
score: 4/5 must-haves verified (SC-1 deferred to user UAT, fully passed; SC-2/3/5 verified in code; SC-4 partial — core verified; SC-5 partial — value-side verified, key-side contract silently broken under env-set conditions per CR-01)
overrides_applied: 0
gaps:
  - truth: "SC-5 (LBL-05): label KEYS are NEVER interpolated — env-var interpolation runs only on label VALUES"
    status: partial
    reason: "Interpolation is a textual pre-parse pass that runs over the entire TOML BEFORE parsing (src/config/interpolate.rs:22-77). The regex `\\$\\{([A-Z_][A-Z0-9_]*)\\}` matches everywhere in the file, with no awareness of TOML key vs value position. When an operator sets `labels = { \"${TEAM}\" = \"v\" }` and `TEAM=ops` is exported in the env, the source is rewritten to `labels = { \"ops\" = \"v\" }` BEFORE the validator sees it. The validator's strict-char regex catches only the env-UNSET case (leftover `${`/`}` literals fail D-02). README at line 257 promises 'Label KEYS are NEVER interpolated'; this is silently false when the env var is set. REPRODUCED on the actual binary: `TEAM=ops cronduit check <toml>` exits 0 for `labels = { \"${TEAM}\" = \"v\" }`."
    artifacts:
      - path: "src/config/interpolate.rs"
        issue: "Pre-parse textual pass — no AST awareness — operates on label keys identically to label values"
      - path: "src/config/validate.rs"
        issue: "check_label_key_chars docstring (lines 247-249) says 'Partially enforces LBL-05 keys-not-interpolated' — the gap is unenforced when interpolation succeeds"
      - path: "README.md"
        issue: "Lines 257 and 264-268 state an absolute guarantee that the implementation does not provide in the env-set case"
      - path: "tests/v12_labels_interpolation.rs"
        issue: "Covers value-side interpolation only; no negative test for the key-side claim"
    missing:
      - "EITHER: relax the README/validator docstring to document the actual behavior (interpolation runs on keys too; only the post-interpolation result is validated; if a leftover `${` survives D-02 catches it; otherwise the resolved key is accepted)"
      - "OR: implement a pre-interpolation key check that walks the raw TOML AST for `[defaults].labels` and `[[jobs]].labels` tables and rejects any key matching `\\$\\{[A-Z_]` BEFORE interpolation runs (preserves the stated invariant)"
      - "Add a negative integration test: `labels = { \"${TEAM}\" = \"v\" }` with `TEAM=ops` set must fail config-load"
  - truth: "SC-4 (LBL-04): clear config-load error explaining labels apply only to type = docker"
    status: partial
    reason: "The error message text is appropriate when the operator actually sets `labels` on a command/script job. But when `[defaults].labels` is set and a command/script job exists without `use_defaults = false`, `apply_defaults` unconditionally merges defaults labels into that job (intentional, defaults.rs:166-176 deliberately not gated on is_non_docker). The LBL-04 validator then fires and tells the operator to 'Remove the `labels` block' — but the operator NEVER set a labels block on that job. REPRODUCED on the binary with `[defaults].labels = { ... }` + a bare command job: error attributes blame to a labels block the operator did not write. Operators must read examples/cronduit.toml comments (jobs 1, 2, 3) to learn the actual fix is `use_defaults = false`. The unit test `lbl_04_error_does_not_leak_defaults_keys_for_non_docker_jobs` (defaults.rs:447-509) explicitly pins a contract that the LBL-04 formatter should set-diff against defaults to recover operator-only keys — this contract is NOT implemented in `check_labels_only_on_docker_jobs`."
    artifacts:
      - path: "src/config/validate.rs"
        issue: "check_labels_only_on_docker_jobs (lines 192-204) does not distinguish operator-supplied labels from defaults-merged labels; emits the same 'Remove the `labels` block' message in both cases"
      - path: "src/config/defaults.rs"
        issue: "apply_defaults intentionally merges defaults.labels into command/script jobs (lines 166-176) to keep the LBL-04 validator's error path active — but the resulting error is misleading"
    missing:
      - "EITHER: implement the set-diff the unit test contract demands: when defaults.labels is set and the merged label set on a non-docker job is exactly the defaults keys, emit a distinct error: 'job is a command/script job; [defaults].labels is set and would attach to it via apply_defaults merge — set `use_defaults = false` on this job to opt out, OR remove [defaults].labels'"
      - "OR: change the merge gate so labels are NOT merged into non-docker jobs and emit a different LBL-04 error specifically for the 'defaults.labels exists, this command job does not opt out' case"
human_verification:
  - test: "U5 — End-to-end docker labels spot-check (already ticked by maintainer 2026-04-29)"
    expected: "After `just docker-compose-up` and waiting for hello-world to fire, `docker inspect <container>` shows `cronduit.run_id`, `cronduit.job_name`, `com.centurylinklabs.watchtower.enable=false` (defaults inherited), `traefik.enable=true` (per-job), and `traefik.http.routers.hello.rule=Host(\\`hello.local\\`)` (per-job, backticks preserved)"
    why_human: "SC-1 cannot be verified programmatically without spawning real Docker containers and running the full scheduler. The integration tests (`tests/v12_labels_*.rs`) carry `#[ignore]` and require a live Docker daemon. Maintainer ran `just docker-compose-up`, waited for hello-world, and confirmed all four label categories on the spawned container per 17-HUMAN-UAT.md U5. STATUS: PASSED 2026-04-29 by Robert."
  - test: "Verify CR-01 fix decision before the phase ships in a tagged release"
    expected: "Maintainer decides: relax the README's absolute guarantee to match the actual textual-interpolation behavior, OR implement pre-interpolation AST-aware key validation."
    why_human: "Architectural decision — both options are valid; relaxing the doc is cheap and matches the existing partial enforcement, while AST-aware validation preserves the intent at non-trivial implementation cost. Maintainer judgment required."
  - test: "Verify CR-02 fix decision before the phase ships in a tagged release"
    expected: "Maintainer decides: implement set-diff in `check_labels_only_on_docker_jobs` to produce a distinct, accurate error when defaults.labels is the cause, OR change the merge gate to skip non-docker jobs and add a separate validator."
    why_human: "Architectural decision involving operator-experience trade-offs and backwards-compat (every existing command/script job in any operator's config must add `use_defaults = false` whenever `[defaults].labels` is added). Maintainer judgment required."
---

# Phase 17: Custom Docker Labels (SEED-001) Verification Report

**Phase Goal:** Operators can attach arbitrary Docker labels to cronduit-spawned containers (Traefik, Watchtower, backup tooling interop) with locked merge semantics, a reserved cronduit.* namespace, and type-gated validation.

**Verified:** 2026-04-29T01:53:49Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #   | Truth (paraphrased Success Criterion)                                                                                                                                                                  | Status      | Evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | SC-1: Operator-set `labels` on a `[[jobs]]` block reaches the spawned container; `cronduit.run_id` + `cronduit.job_name` remain intact.                                                                | ✓ VERIFIED  | Five-layer parity wired (`JobConfig.labels` in mod.rs:115-116; `serialize_config_json` in sync.rs:98-100; `compute_config_hash` in hash.rs:47-49; `apply_defaults` merge in defaults.rs:166-176; `DockerJobConfig.labels` in docker.rs:62-63). Executor at docker.rs:172-177 inserts operator labels FIRST, internal labels AFTER (defense-in-depth). Integration test `tests/v12_labels_merge.rs` exercises the full path. UAT U5 ticked by maintainer 04-29. |
| 2   | SC-2: `use_defaults = false` causes per-job labels to fully replace defaults; otherwise defaults merge with per-job winning on collision.                                                              | ✓ VERIFIED  | `apply_defaults` short-circuits on `use_defaults == Some(false)` at defaults.rs:112-114 (returns BEFORE label merge). Merge logic at defaults.rs:167-175 starts from defaults clone and `extend()`s per-job — per-job wins on collision per HashMap::extend semantics. Unit tests `apply_defaults_merges_labels_per_job_wins` + `apply_defaults_use_defaults_false_replaces_labels` (defaults.rs:357-445) pin both contracts. Integration test `tests/v12_labels_use_defaults_false.rs` confirms end-to-end replace via `docker inspect`. |
| 3   | SC-3: `cronduit.foo` (or any `cronduit.*` key) yields a config-load error pointing at the offending key.                                                                                               | ✓ VERIFIED  | `check_label_reserved_namespace` validate.rs:166-187 lists offending keys (sorted, deterministic) with the message "labels under reserved namespace `cronduit.*` are not allowed: cronduit.foo. Remove these keys; the cronduit.* prefix is reserved for cronduit-internal labels." Confirmed on the binary: `cronduit check` exits 1 with the offending key, the rule, and the job name in the message.                                                       |
| 4   | SC-4: `labels = ...` on `type = command` or `type = script` yields a clear config-load error explaining labels are docker-only.                                                                        | ⚠️ PARTIAL  | Operator-set case verified (validate.rs:192-204; reproduced on the binary). HOWEVER: when `[defaults].labels` exists and a command/script job has no `use_defaults = false`, the same error fires but tells the operator to "Remove the `labels` block" — the operator never set such a block. Error mis-attributes blame. See gap CR-02 below.                                                                                                                |
| 5   | SC-5: `${VAR}` interpolated in label VALUES; keys NEVER interpolated; >4 KB value or >32 KB total set rejected at load.                                                                                | ⚠️ PARTIAL  | Value-side interpolation: VERIFIED (interpolate.rs:22-77; integration test `tests/v12_labels_interpolation.rs` confirms `${DEPLOYMENT_ID}` resolves at LOAD and reaches the container). 4 KB / 32 KB limits: VERIFIED (validate.rs:208-245; binary confirms both errors fire). KEY interpolation NEVER happens claim: SILENTLY BROKEN when env var is set — REPRODUCED on the binary: `TEAM=ops cronduit check <toml-with-${TEAM}-as-key>` exits 0. See gap CR-01 below. |

**Score:** 3 fully verified + 2 partial (with documented BLOCKER-class gaps from REVIEW.md).

### Required Artifacts

| Artifact                                  | Expected                                                                                | Status     | Details                                                                                                                                  |
| ----------------------------------------- | --------------------------------------------------------------------------------------- | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `src/config/mod.rs`                       | `labels: Option<HashMap<String, String>>` on `JobConfig` and `DefaultsConfig`           | ✓ VERIFIED | Lines 86 (DefaultsConfig) and 116 (JobConfig); both `#[serde(default)]`.                                                                |
| `src/config/defaults.rs`                  | `apply_defaults` merges defaults+per-job labels, per-job wins on collision               | ✓ VERIFIED | Lines 166-176; merge logic correct; short-circuit at line 112 covers use_defaults=false case.                                            |
| `src/config/validate.rs`                  | Four LOAD-time validators (LBL-03, LBL-04, LBL-06, D-02 key chars)                       | ✓ VERIFIED | Lines 166-272. All four registered in run_all_checks (lines 42-45). 13 unit tests added per Plan 17-02. Determinism (sort) applied at every list-emitting site. |
| `src/config/hash.rs`                      | `compute_config_hash` includes labels                                                    | ✓ VERIFIED | Lines 47-49. `hash_differs_on_labels_change` regression test at hash.rs:299-323.                                                         |
| `src/scheduler/sync.rs`                   | `serialize_config_json` emits labels into `config_json`                                  | ✓ VERIFIED | Lines 98-100. Test-only re-export `serialize_config_json_for_tests` for integration tests. WR-04 caveat: `unwrap_or_default` swallows serialize failures into empty string — pre-existing pattern, not introduced by this phase. |
| `src/scheduler/docker.rs`                 | `DockerJobConfig.labels`; operator labels inserted FIRST, internal AFTER                 | ✓ VERIFIED | Lines 62-63 (struct field); lines 172-177 (build site); ordering correct per defense-in-depth contract.                                  |
| `tests/v12_labels_merge.rs`               | Integration test for defaults+per-job merge end-to-end through bollard inspect           | ✓ VERIFIED | Drives parse_and_validate → serialize → execute_docker → inspect_container. Asserts watchtower.enable=false (defaults), traefik.* (per-job), backticks preserved, cronduit.run_id + cronduit.job_name intact. `#[ignore]` (manual run). |
| `tests/v12_labels_use_defaults_false.rs`  | Integration test for use_defaults=false replace                                          | ✓ VERIFIED | Drives end-to-end. Asserts `backup.exclude=true` reaches container; `watchtower.enable` MUST NOT be on the container (negative assert). |
| `tests/v12_labels_interpolation.rs`       | Integration test for `${VAR}` interpolation in label VALUES                              | ✓ VERIFIED | Sets `DEPLOYMENT_ID=12345`, drives end-to-end, asserts container has `deployment.id=12345` AND no literal `${DEPLOYMENT_ID}`. NOTE: no negative test for key-side claim (CR-01). |
| `examples/cronduit.toml`                  | Three integration patterns + cross-references to README                                  | ✓ VERIFIED | `[defaults].labels` Watchtower (line 44); `hello-world` Traefik per-job MERGE (line 131); NEW `isolated-batch` job with `use_defaults=false` REPLACE (line 190). `just check-config` exits 0. |
| `README.md`                               | Labels subsection with mermaid diagram + 3-row table + five rule paragraphs              | ⚠️ PARTIAL  | Subsection exists at line 205-272. Mermaid diagram present (211-225). 3-row merge-semantics table present (229-233). All five rule paragraphs present (reserved namespace, type-gate, size limits, env-var interpolation, security note). HOWEVER: the env-var interpolation paragraph (line 257-268) makes an absolute guarantee ("Label KEYS are NEVER interpolated") that is silently broken when the env var is set — see CR-01. |
| `.planning/seeds/SEED-001-custom-docker-labels.md` | Frontmatter promoted dormant → realized                                       | ✓ VERIFIED | `status: realized`; `realized_in: phase-17`; `milestone: v1.2`; `realized_date: 2026-04-29`. File stays at original path per D-05.        |
| `.planning/phases/17-.../17-HUMAN-UAT.md` | UAT checklist citing existing `just` recipes; ticked by maintainer                       | ✓ VERIFIED | 6 items, all `[x]` ticked, "Validated by: Maintainer (Robert) on 2026-04-29 — all 6 UAT items passed locally per D-09."                  |

### Key Link Verification

| From                             | To                                          | Via                                                       | Status     | Details                                                                                                                       |
| -------------------------------- | ------------------------------------------- | --------------------------------------------------------- | ---------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `JobConfig.labels`               | `DockerJobConfig.labels`                    | `serialize_config_json` → `config_json` column → `serde_json::from_str` | ✓ WIRED    | Round-trip pinned by `parity_with_docker_job_config_is_maintained` and `parity_labels_round_trip_through_docker_job_config` unit tests (defaults.rs:674-803). |
| `apply_defaults`                 | `DockerJobConfig` build site                | sync.rs serialize → docker.rs deserialize → `labels.extend` at docker.rs:174 | ✓ WIRED    | Operator labels inserted FIRST (line 174), internal labels AFTER (lines 176-177). Order = defense-in-depth.                     |
| `run_all_checks` per-job loop    | Four label validators                       | Direct call sites in validate.rs:42-45                    | ✓ WIRED    | All four validators called inside the per-job loop. Per-job error reporting matches D-01 (one ConfigError per job per violation type). |
| `examples/cronduit.toml`         | Plan 17-01 schema + Plan 17-02 validators   | `parse_and_validate` end-to-end                          | ✓ WIRED    | `cronduit check examples/cronduit.toml` exits 0 (verified). UAT U2 ticked.                                                     |
| `README.md § Configuration > Labels` | Plan 17-01 apply_defaults + Plan 17-03 docker.rs label-build site | Mermaid merge-precedence diagram                       | ⚠️ PARTIAL | Diagram and table accurately reflect implemented behavior. README's absolute "keys never interpolated" claim does NOT match implementation in env-set case. |

### Data-Flow Trace (Level 4)

| Artifact                    | Data Variable               | Source                                                  | Produces Real Data | Status     |
| --------------------------- | --------------------------- | ------------------------------------------------------- | ------------------ | ---------- |
| `DockerJobConfig.labels`    | operator-defined labels     | `serialize_config_json` → DB `config_json` → `serde_json::from_str` | ✓ Yes              | ✓ FLOWING  |
| Container `Config.Labels`   | full label set              | `bollard::Docker::create_container(.., labels: HashMap)` | ✓ Yes              | ✓ FLOWING  |
| `[defaults].labels` inheritance | merged HashMap          | `apply_defaults` `defaults_labels.clone()` + `m.extend(per_job)` | ✓ Yes              | ✓ FLOWING  |

### Behavioral Spot-Checks

| Behavior                                                                              | Command                                                                | Result                                                                                                                                                                                                                | Status |
| ------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| Library unit test suite passes                                                        | `cargo test --lib`                                                     | 215 passed, 0 failed                                                                                                                                                                                                  | ✓ PASS |
| `cronduit check` accepts the example config                                           | `cronduit check examples/cronduit.toml`                                | `ok: examples/cronduit.toml` (exit 0)                                                                                                                                                                                 | ✓ PASS |
| SC-3: cronduit.* key rejected at LOAD                                                 | `cronduit check <toml with labels = { "cronduit.foo" = "bar" }>`       | Exit 1; error names key, namespace rule, job                                                                                                                                                                          | ✓ PASS |
| SC-4: operator-set labels on command job rejected at LOAD                             | `cronduit check <toml with command + labels>`                          | Exit 1; "labels is only valid on docker jobs..."                                                                                                                                                                      | ✓ PASS |
| SC-5b: per-value > 4 KB rejected at LOAD                                              | `cronduit check <toml with 5000-byte label value>`                     | Exit 1; "label values exceed 4 KB limit: k. Each label value must be ≤ 4096 bytes."                                                                                                                                    | ✓ PASS |
| SC-5b: per-set > 32 KB rejected at LOAD                                               | `cronduit check <toml with ~40020 bytes total>`                        | Exit 1; "total label-set size 40020 bytes exceeds 32 KB limit."                                                                                                                                                       | ✓ PASS |
| **SC-5 contract test (CR-01):** `${TEAM}` as label key with TEAM=ops exported        | `TEAM=ops cronduit check <toml with labels = { "${TEAM}" = "v" }>`     | **Exit 0 (silently accepted) — README contract "Label KEYS are NEVER interpolated" violated**                                                                                                                          | ✗ FAIL |
| **SC-4 attribution test (CR-02):** [defaults].labels + bare command job              | `cronduit check <toml with [defaults].labels + command job, no use_defaults=false>` | Exit 1 with message: "Remove the `labels` block" — but operator never set such a block; defaults were merged in by apply_defaults                                                                              | ✗ FAIL (mis-attribution) |

### Requirements Coverage

| Requirement | Source Plan(s)               | Description                                                                          | Status         | Evidence                                                                                              |
| ----------- | ---------------------------- | ------------------------------------------------------------------------------------ | -------------- | ----------------------------------------------------------------------------------------------------- |
| LBL-01      | 17-01, 17-03, 17-04, 17-05   | `labels` field on JobConfig+DefaultsConfig+DockerJobConfig; merged into bollard      | ✓ SATISFIED    | Five-layer parity verified; integration test pins end-to-end.                                          |
| LBL-02      | 17-01, 17-03, 17-04, 17-05   | use_defaults=false replaces; otherwise per-job-wins-on-collision merge               | ✓ SATISFIED    | apply_defaults short-circuit + extend-from-defaults; both unit and integration tests pin both cases.   |
| LBL-03      | 17-02, 17-05                 | cronduit.* namespace rejected at LOAD                                                | ✓ SATISFIED    | check_label_reserved_namespace verified on binary; sorts offending keys deterministically.            |
| LBL-04      | 17-02, 17-05                 | labels on command/script jobs rejected at LOAD with clear error                      | ⚠️ PARTIAL     | Validator fires for the operator-set case. Error mis-attributes blame in the [defaults].labels-merged-into-command-job case (CR-02). |
| LBL-05      | 17-03, 17-05                 | `${VAR}` interpolated in label VALUES; keys NEVER interpolated                       | ⚠️ PARTIAL     | Value-side: ✓ pinned by integration test. Key-side: silently broken when env var set (CR-01).         |
| LBL-06      | 17-02, 17-05                 | per-value ≤ 4 KB; per-set ≤ 32 KB enforced at LOAD                                   | ✓ SATISFIED    | Both error paths verified on binary.                                                                  |

**No orphaned requirements.** All six LBL-XX requirements are claimed by at least one plan and implementation evidence is present.

**Tracking-table drift (Info):** REQUIREMENTS.md lines 186-191 still show LBL-01..LBL-06 as `Pending`; should be flipped to `Complete` (mirroring FCTX-04 / FCTX-07 at lines 195/198). Not a correctness gap — bookkeeping.

### Anti-Patterns Found

(Mostly carried over from the standard Code Review at 17-REVIEW.md; only the BLOCKER-class items affect goal achievement.)

| File                                  | Line(s)        | Pattern / Concern                                                                                                                                                            | Severity     | Impact                                                                                                                                                                                                                                                                                       |
| ------------------------------------- | -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/config/interpolate.rs`           | 22-77          | Pre-parse textual interpolation runs over the entire TOML; no AST awareness; matches `${VAR}` in key positions identically to value positions                                | 🛑 Blocker (CR-01) | README's absolute "Label KEYS are NEVER interpolated" guarantee silently broken when env var is set. SC-5 partially fails. Reproduced on binary.                                                                                                                                              |
| `src/config/validate.rs`              | 192-204        | `check_labels_only_on_docker_jobs` does not distinguish operator-supplied labels from defaults-merged labels; emits the same "Remove the labels block" message in both cases | 🛑 Blocker (CR-02) | LBL-04 error mis-attributes blame. Operator following the message gets confused; the actual fix is `use_defaults = false`. Backwards-compat tax (every existing command/script job in any operator's config must add `use_defaults=false` whenever `[defaults].labels` is added) is hidden behind a confusing diagnostic. |
| `src/config/validate.rs`              | 32-47          | All four label validators iterate `cfg.jobs`; defaults.labels is not validated standalone                                                                                    | ⚠️ Warning (WR-01) | Configs with zero docker jobs (or every job opting out via use_defaults=false) silently accept malformed [defaults].labels (reserved namespace, oversized values, invalid char keys). Not a goal-blocker for THIS phase but is a correctness gap in the validator surface. |
| `tests/v12_labels_*.rs`               | various        | Container cleanup runs only on success path; assertion panic leaks named container                                                                                            | ⚠️ Warning (WR-02) | Local debugging paper-cut; subsequent runs hit name-conflict.                                                                                                                                                                                                                                  |
| `tests/v12_labels_interpolation.rs`   | 43-46, 144-147 | `set_var("DEPLOYMENT_ID")` on success path; assertion panic leaks env var                                                                                                     | ⚠️ Warning (WR-03) | Local debugging paper-cut; affects subsequent runs in same process.                                                                                                                                                                                                                            |
| `src/scheduler/sync.rs`               | 106            | `serialize_config_json` returns `unwrap_or_default()` (empty string) on serde_json failure                                                                                    | ⚠️ Warning (WR-04) | Pre-existing; not introduced by this phase. Labels expand the surface (operator-controlled bytes via interpolation) so worth tracking for v1.3.                                                                                                                                                |
| `src/scheduler/docker.rs`             | 144            | `let _image_digest` discards the digest then re-fetches via inspect at line 281                                                                                              | ℹ️ Info (IN-02) | Pre-existing performance niggle; not phase-17 introduced.                                                                                                                                                                                                                                       |
| `src/config/hash.rs`                  | 11-15          | Stale doc comment claims function "is therefore unit-tested but not called from the run path" — outdated since Phase 2                                                       | ℹ️ Info (IN-03) | Documentation hygiene only.                                                                                                                                                                                                                                                                     |
| `src/config/validate.rs`              | 24, 29         | "4 KB" / "32 KB" hard-coded in messages alongside `MAX_LABEL_VALUE_BYTES` / `MAX_LABEL_SET_BYTES` constants                                                                  | ℹ️ Info (IN-04) | Cosmetic; constants and messages drift if limits change.                                                                                                                                                                                                                                        |
| `src/config/mod.rs`, others           | 86, 116        | `labels` is `HashMap<String, String>` (matches `JobConfig.env`'s style? actually env is BTreeMap<String, SecretString>); hash stability relies on serde_json's BTreeMap-backed ordering | ℹ️ Info (IN-01) | Works correctly today via serde_json::Value::Object internal sort; not a goal-blocker. Switching to BTreeMap would make ordering load-bearing-by-construction.                                                                                                                                  |
| `.planning/REQUIREMENTS.md`           | 186-191        | LBL-01..LBL-06 still show `Pending` in the status table                                                                                                                       | ℹ️ Info       | Tracking-table drift; mirror the FCTX-04 / FCTX-07 `Complete` pattern.                                                                                                                                                                                                                          |

### Human Verification Required

#### 1. UAT U5 — End-to-end docker labels spot-check

**Test:** `just docker-compose-up` and wait for `hello-world` job to fire (`*/5 * * * *`). Then `docker inspect <container-id> | jq '.[0].Config.Labels'`.

**Expected:** Container shows all four label categories: `cronduit.run_id`, `cronduit.job_name`, `com.centurylinklabs.watchtower.enable=false`, `traefik.enable=true`, `traefik.http.routers.hello.rule=Host(\`hello.local\`)`.

**Why human:** Cannot be verified programmatically without spawning real Docker containers and running the full scheduler. Integration tests for these paths carry `#[ignore]` and require a live Docker daemon.

**Status:** **PASSED** by the maintainer on 2026-04-29 per 17-HUMAN-UAT.md (`Validated by: Maintainer (Robert) on 2026-04-29 — all 6 UAT items passed locally per D-09.`)

#### 2. CR-01 fix decision

**Test:** Maintainer reviews CR-01 in 17-REVIEW.md and chooses one of:
- **Option A (cheap, recommended for shipping):** Relax the README + validator docstring to document the actual behavior — interpolation runs on keys too; the validator catches only the post-interpolation result; if the env var is unset the leftover `${...}` is rejected by the strict char regex; if the env var is set the resolved key is accepted as long as it matches the strict char pattern. Update integration tests accordingly.
- **Option B (preserves invariant):** Implement an AST-aware pre-interpolation key check that walks the raw TOML (pre-interpolate.rs) for `[defaults].labels` and `[[jobs]].labels` tables and rejects any key matching `\$\{[A-Z_]` BEFORE interpolation runs.

**Expected:** Decision recorded; implementation lands; integration test added.

**Why human:** Architectural trade-off requiring maintainer judgment.

#### 3. CR-02 fix decision

**Test:** Maintainer reviews CR-02 in 17-REVIEW.md and chooses one of:
- **Option A (preserves current merge invariant):** Implement set-diff in `check_labels_only_on_docker_jobs` per the unit-test contract in defaults.rs:447-509. Two distinct error messages: one for "operator set labels on a command/script job", one for "[defaults].labels merged into a command/script job; set `use_defaults = false` to opt out".
- **Option B (changes merge gate):** Skip merging labels into non-docker jobs in `apply_defaults` (gate on `is_non_docker`) and add a separate validator that fires when `[defaults].labels` is set and any job has `use_defaults != Some(false)`.

**Expected:** Decision recorded; implementation lands; appropriate test added.

**Why human:** Architectural trade-off involving operator-experience and backwards-compat trade-offs (Option B changes the merge invariant; Option A keeps it but requires set-diff plumbing).

### Gaps Summary

Phase 17 ships substantive correctness for the core feature: operator labels reach the spawned container; defaults+per-job merge with per-job-wins; `use_defaults=false` replaces; the four LOAD-time validators (reserved namespace, type-gate, size limits, char regex) all fire correctly for the cases they were designed to catch; mermaid documentation and integration patterns in the example file are good. UAT was passed by the maintainer on 2026-04-29.

Two REVIEW-flagged BLOCKER findings prevent a clean PASS:

- **CR-01:** README + validator docstrings promise "Label KEYS are NEVER interpolated", but textual interpolation runs over the whole TOML (including key positions) BEFORE TOML parsing. Reproduced on the binary: `TEAM=ops cronduit check <toml-with-labels-key-${TEAM}>` exits 0. SC-5's "keys are never interpolated" sub-clause is silently broken in the env-var-set case. Fix is either documentation relax or AST-aware key check.

- **CR-02:** When `[defaults].labels` is set and a command/script job has no `use_defaults = false`, `apply_defaults` (intentionally) merges defaults into that job, the LBL-04 validator fires, and the resulting error blames a "labels block" the operator never wrote. Confusing diagnostic + hidden backwards-compat tax (every existing command/script job needs `use_defaults = false` retrofit). Fix is set-diff in the formatter or change the merge gate.

Both findings are in REVIEW.md, both are reproducible on the actual binary, both require maintainer architectural judgment to resolve. Neither is a stub, missing artifact, or wiring break — they are subtle correctness/usability defects in messaging that affect the operator-facing contract documented in README.md.

Recommendation: human_needed pending architectural decisions on CR-01 and CR-02 before final v1.2.0 ship. The phase has shipped correct core behavior and human UAT has passed; fixes for CR-01 and CR-02 should land before the next tagged release that promises this feature in the changelog.

---

_Verified: 2026-04-29T01:53:49Z_
_Verifier: Claude (gsd-verifier)_
