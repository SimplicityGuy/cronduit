---
phase: 17-custom-docker-labels-seed-001
reviewed: 2026-04-28T00:00:00Z
depth: standard
files_reviewed: 12
files_reviewed_list:
  - examples/cronduit.toml
  - README.md
  - src/config/defaults.rs
  - src/config/hash.rs
  - src/config/mod.rs
  - src/config/validate.rs
  - src/scheduler/docker.rs
  - src/scheduler/sync.rs
  - tests/scheduler_integration.rs
  - tests/v12_labels_interpolation.rs
  - tests/v12_labels_merge.rs
  - tests/v12_labels_use_defaults_false.rs
findings:
  critical: 2
  warning: 5
  info: 4
  total: 11
status: issues_found
---

# Phase 17: Code Review Report

**Reviewed:** 2026-04-28
**Depth:** standard
**Files Reviewed:** 12
**Status:** issues_found

## Summary

Phase 17 adds operator-defined Docker labels (`[defaults].labels` + per-job
`labels`) end-to-end through the parse → merge → validate → hash → serialize →
executor pipeline. The parity story is well-tested (the regression guard test
in `defaults.rs` was extended to exercise `labels` round-tripping through
`DockerJobConfig`), the executor wiring inserts cronduit-internal labels AFTER
operator labels (defense-in-depth ordering), and three real-Docker integration
tests cover the merge / replace / interpolation surface.

Two correctness defects must be addressed before this ships:

1. **CR-01 — LBL-05 contract violation: label KEYS are interpolated.** The
   README, validator, and integration test all promise "label keys are NEVER
   interpolated." But interpolation is a textual pass that runs BEFORE TOML
   parsing and matches `${VAR}` everywhere in the file (excluding comments).
   So `labels = { "${TEAM}" = "v" }` with `TEAM=ops` set in the env silently
   becomes `labels = { "ops" = "v" }` and the validator never sees the
   placeholder. The "strict char regex rejects leftover `${`" comment only
   applies when interpolation FAILS — successful interpolation produces a
   valid key.

2. **CR-02 — Misleading LBL-04 error attribution.** When `[defaults].labels`
   is set and any command/script job exists without `use_defaults = false`,
   `apply_defaults` unconditionally merges defaults labels into that job
   (defaults.rs:166-176, deliberately not gated on `is_non_docker`). The
   LBL-04 validator then fires "Remove the `labels` block" — but the operator
   never set a `labels` block on that job. The example file documents this
   footgun (jobs 1, 2, 3 all set `use_defaults = false` precisely to silence
   it), making it a known design choice, but the error message still
   mis-attributes blame and the resulting "every existing command/script job
   now needs `use_defaults = false`" backwards-compat tax is hidden behind a
   confusing diagnostic.

The remaining findings cover documentation / test-isolation / robustness
issues that should be fixed but do not block correctness.

## Critical Issues

### CR-01: LBL-05 contract violation — label keys ARE interpolated when the env var is set

**File:** `src/config/interpolate.rs:22-77`, `src/config/validate.rs:247-272`,
`README.md:257-268`

**Issue:** Interpolation is a TEXTUAL pass over the raw TOML before parsing
(see `parse_and_validate` in `src/config/mod.rs:158`). The regex
`\$\{([A-Z_][A-Z0-9_]*)\}` matches `${VAR}` everywhere in the file (excluding
comments), with no awareness of TOML key vs value position. When
`TEAM=ops` is set in the environment and the operator writes:

```toml
labels = { "${TEAM}" = "v" }
```

the interpolation step rewrites the source to `labels = { "ops" = "v" }`
BEFORE TOML parses the key. The validator only ever sees `ops`, which passes
`LABEL_KEY_RE`, and the configuration is accepted.

The README at line 257 explicitly states "Label KEYS are NEVER interpolated"
and the example at line 264-268 claims "key remains literal `${TEAM}` and is
rejected by the strict char regex" — this is true ONLY when the env var is
unset (interpolation fails, leaves `${TEAM}` literal, validator rejects).
When the var is set, the contract silently breaks.

The validator comment at validate.rs:248-249 ("Partially enforces LBL-05's
'keys are NOT interpolated' — leftover `${` / `}` chars after a failed /
unintended interpolation are rejected here") understates the failure mode:
"partially" is doing all the work, and the gap (env-set case) is
unenforced.

The integration test `v12_labels_interpolation.rs` covers the value-side
interpolation but does not have a negative test for the key-side claim.

This affects three contracts: (a) the LBL-05 spec, (b) the README's stated
guarantee, (c) the validator's docstring. An operator who relies on the
guarantee (e.g., to enforce a corporate label-key policy) will see it break
silently.

**Fix:** Either (preferred) document the actual behavior — interpolation runs
on keys too, validator catches only the post-interpolation result — and update
the README, validator comment, and test to match. Or (stronger) implement key
exclusion: parse the TOML once with raw text, walk the AST, and re-interpolate
only string-value nodes. The simpler middle ground is a stand-alone validator
that detects `${...}` literals BEFORE interpolation in any TOML key position
and emits a hard error; this preserves the stated invariant but requires a
two-pass parse.

```rust
// At minimum, fix the README/validator docstring. Below is a sketch of an
// AST-aware key check that would actually enforce the invariant:
//
// fn check_label_key_no_pre_interp(raw: &str, errors: &mut Vec<ConfigError>) {
//     // Parse raw (pre-interpolation) TOML, walk every [defaults].labels and
//     // [[jobs]].labels table, and reject any key matching r"\$\{[A-Z_]"
//     // BEFORE interpolation runs. This is the only way to honor the
//     // "keys are NEVER interpolated" promise without restructuring
//     // interpolation into an AST-aware pass.
// }
```

---

### CR-02: LBL-04 error message blames the operator for labels they never set

**File:** `src/config/validate.rs:192-204`, `src/config/defaults.rs:166-176`

**Issue:** `apply_defaults` is intentionally NOT gated on `is_non_docker`
when merging labels (defaults.rs:160-166 explains this is to keep the
LBL-04 validator's error path active). But the resulting `JobConfig.labels`
contains the DEFAULTS keys, not operator-supplied keys. The LBL-04 validator
at validate.rs:198-200 says:

> `labels` is only valid on docker jobs (job with `image = "..."` set
> either directly or via `[defaults].image`); command and script jobs cannot
> set `labels` because there is no container to attach them to. **Remove the
> `labels` block**, or switch the job to a docker job by setting `image`.

For a command job with NO per-job `labels` block AND a `[defaults].labels`
section, this error fires and tells the operator to "Remove the `labels`
block" — but there IS NO `labels` block on the job. The operator must read
the example file's commentary (jobs 1, 2, 3) to learn that the actual fix
is `use_defaults = false`.

The unit test at `defaults.rs:447-509`
(`lbl_04_error_does_not_leak_defaults_keys_for_non_docker_jobs`) explicitly
acknowledges this path and pins a contract that the LBL-04 validator should
implement a set-diff to recover operator-only keys for the error message.
That contract is NOT implemented in `check_labels_only_on_docker_jobs` — the
validator does not even mention which keys are problematic, let alone do
a set-diff.

Beyond the misleading message, this design forces a backwards-compat tax:
adding `[defaults].labels` to an existing config breaks every command/script
job until each one is retrofit with `use_defaults = false`. This is
documented in `examples/cronduit.toml` (lines 73-77), but the failure mode
during retrofit is the confusing error above, not a clear "you added
defaults.labels and your command jobs need use_defaults = false."

**Fix:** Either change the merge gate so labels are NOT merged into
non-docker jobs (and emit a different, clearer LBL-04 error specifically for
"defaults.labels exists, this command job does not opt out"), or make the
LBL-04 error formatter implement the set-diff the unit test contract
demands and produce two distinct messages — one for "operator set labels on
command job" and one for "defaults.labels merged into command job; set
`use_defaults = false` to opt out." The second message is what the example
file's comments are already teaching operators by hand.

```rust
fn check_labels_only_on_docker_jobs(
    job: &JobConfig,
    defaults_labels: Option<&HashMap<String, String>>,  // pass through from caller
    path: &Path,
    errors: &mut Vec<ConfigError>,
) {
    if job.labels.is_none() || job.image.is_some() {
        return;
    }
    let merged = job.labels.as_ref().unwrap();
    let operator_only: BTreeSet<&str> = match defaults_labels {
        Some(d) => merged.keys()
            .filter(|k| !d.contains_key(k.as_str()))
            .map(String::as_str)
            .collect(),
        None => merged.keys().map(String::as_str).collect(),
    };
    if operator_only.is_empty() {
        // Defaults-only labels merged in. Tell operator to set use_defaults=false.
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}` is a command/script job; `[defaults].labels` is set \
                 and would attach to it via the apply_defaults merge. Set \
                 `use_defaults = false` on this job to opt out, OR remove \
                 `[defaults].labels` if no docker jobs need it.",
                job.name
            ),
        });
    } else {
        // Operator actually set labels on a non-docker job. Existing message OK.
        let mut keys: Vec<&str> = operator_only.into_iter().collect();
        keys.sort();
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: per-job labels {} are not allowed on command/script jobs (no container to attach). Remove these keys from the job's `labels` block, or switch to a docker job by setting `image`.",
                job.name,
                keys.join(", ")
            ),
        });
    }
}
```

## Warnings

### WR-01: `[defaults].labels` is not validated standalone

**File:** `src/config/validate.rs:32-47`

**Issue:** All four label validators iterate `cfg.jobs`, never `cfg.defaults`.
After `apply_defaults` runs, defaults labels are merged into per-job label
sets and indirectly validated via the per-job pass — BUT only for jobs that
actually inherit them. If the config has zero docker jobs (or every job sets
`use_defaults = false`), `[defaults].labels` is never inspected at all.

Concrete failure cases (all silently accepted):

- `[defaults].labels = { "cronduit.bad" = "v" }` with no docker jobs → reserved
  namespace not caught.
- `[defaults].labels = { " bad key" = "v" }` with no docker jobs → invalid char
  not caught.
- `[defaults].labels = { "k" = "<5KB blob>" }` with no docker jobs → size
  violation not caught at load.

If a docker job is later added (without `use_defaults = false`), the merge
will then propagate the bad defaults into the job and the per-job validator
fires — but the error attribution will be on the job, not the defaults
section, which is confusing.

**Fix:** Add a standalone defaults-label validator that runs the same four
checks (LBL-03 reserved namespace, LBL-06 sizes, D-02 char regex) directly
on `cfg.defaults.as_ref().and_then(|d| d.labels.as_ref())` before the per-job
loop. LBL-04 (type-gate) does not apply to defaults because defaults are not
typed.

```rust
pub fn run_all_checks(cfg: &Config, path: &Path, raw: &str, errors: &mut Vec<ConfigError>) {
    check_timezone(&cfg.server.timezone, path, errors);
    check_bind(&cfg.server.bind, path, errors);
    check_duplicate_job_names(&cfg.jobs, path, raw, errors);
    if let Some(defaults_labels) = cfg.defaults.as_ref().and_then(|d| d.labels.as_ref()) {
        check_defaults_label_reserved_namespace(defaults_labels, path, errors);
        check_defaults_label_size_limits(defaults_labels, path, errors);
        check_defaults_label_key_chars(defaults_labels, path, errors);
    }
    for job in &cfg.jobs { /* ... */ }
}
```

---

### WR-02: Integration tests leak Docker containers on test panic

**File:** `tests/v12_labels_merge.rs:170-178`,
`tests/v12_labels_use_defaults_false.rs:131-139`,
`tests/v12_labels_interpolation.rs:134-142`

**Issue:** All three integration tests use `delete = false` (necessary so the
post-execute `inspect_container` can read labels), and call
`docker.remove_container(...)` only at the END of the test, after every
assertion. If any assertion panics, the cleanup is skipped and the named
container — `cronduit-test-labels-{merge,replace,interp}-{pid}` — is left
running on the host. The next test invocation with the same PID will then
fail at create_container with a name conflict, and the operator must clean
up by hand with `docker rm -f`.

Combined with `--test-threads=1` and the same container-name pattern across
files, this is a real paper-cut for anyone debugging a failing assertion
locally.

**Fix:** Use a guard that runs `remove_container` in `Drop` (RAII), or use
`scopeguard::defer!` from the `scopeguard` crate. The pattern matches
existing code in the project that uses cleanup helpers. Alternatively, use
`assert!` macros that allow continuing on failure and gather assertions
before cleanup.

```rust
// Sketch using scopeguard:
let _cleanup = scopeguard::guard(container_id.clone(), |id| {
    let docker = docker.clone();
    tokio::task::spawn(async move {
        let _ = docker.remove_container(
            &id,
            Some(bollard::query_parameters::RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        ).await;
    });
});
```

---

### WR-03: Env-mutating integration test leaks `DEPLOYMENT_ID` on panic

**File:** `tests/v12_labels_interpolation.rs:43-46, 144-147`

**Issue:** `unsafe { std::env::set_var("DEPLOYMENT_ID", "12345") }` runs at
line 44, and the matching `remove_var` at line 146 is in the success path
after every assertion. Any earlier panic (failed `parse_and_validate`,
failed assertion, Docker error) skips the remove, leaving the env var set
for the remainder of the test process. With `--test-threads=1` this affects
test re-runs in the same process; combined with WR-02's container leak it
also affects local debugging.

**Fix:** Drop guard on the env var, or `defer!` macro. Same pattern as
WR-02.

```rust
struct EnvGuard(&'static str);
impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: same single-threaded justification as the set_var.
        unsafe { std::env::remove_var(self.0); }
    }
}
unsafe { std::env::set_var("DEPLOYMENT_ID", "12345"); }
let _guard = EnvGuard("DEPLOYMENT_ID");
```

---

### WR-04: `serialize_config_json` swallows failures into empty string

**File:** `src/scheduler/sync.rs:106`

**Issue:** Line 106 uses `.unwrap_or_default()` on the
`serde_json::to_string` result. On a serialization failure (extremely
unlikely for these types — all serde-derived `String` / `HashMap<String,
String>` fields — but possible if a non-UTF-8 byte snuck in via env-var
expansion), this silently produces `""`, which then gets stored to the DB
in the `config_json` column. When the executor later
`serde_json::from_str::<DockerJobConfig>("")`, deserialization fails and
the run is marked `RunStatus::Error` with a confusing "failed to parse
docker config: ..." message — but the root cause was upstream
serialization, not the executor.

This is a pre-existing pattern (not introduced by Phase 17), but the new
`labels` field expands the surface — `HashMap<String, String>` could now
receive operator-controlled byte sequences via env-var interpolation.

**Fix:** Make the function return `Result<String, serde_json::Error>` and
propagate the error to the caller, OR log a `tracing::error!` and use a
sentinel like `"{}"` instead of `""` so the executor sees a valid JSON
object and can produce a clearer "image required" error.

---

### WR-05: Defaults section without `[defaults]` table cannot opt out of label inheritance

**File:** `examples/cronduit.toml:73-78`, `src/config/defaults.rs:108-114`

**Issue:** `apply_defaults` short-circuits on `defaults.is_none()` (early
return at line 109) and on `use_defaults == Some(false)` (line 112), but
there is no per-FIELD opt-out for labels. An operator who wants the merged
non-label fields (image, network, volumes, timeout, delete) but NOT the
labels has to either disable the entire defaults block (`use_defaults =
false` and re-set every field by hand) or accept the label inheritance.

This is a usability gap that's likely to surface once operators start using
defaults heavily. The example file teaches `use_defaults = false` as the
big-hammer answer for command/script jobs, but for docker jobs that just
want the network/timeout from defaults but not Watchtower exclusion, there's
no clean answer. The workaround — set `labels = { "watchtower.enable" =
"true" }` (per-job-wins) — works but inverts the policy intent.

**Fix:** This is a v1.3 feature consideration, not a blocker. Document the
limitation in the README's Labels section so operators know the workaround.
Consider a future per-field exclusion mechanism (e.g., `inherit = ["timeout",
"network"]`) if this becomes a recurring complaint.

## Info

### IN-01: Inconsistent `BTreeMap` vs `HashMap` for the labels representation

**File:** `src/config/mod.rs:86, 116`, `src/scheduler/docker.rs:33, 63`

**Issue:** Both `JobConfig.labels` and `DefaultsConfig.labels` are
`Option<HashMap<String, String>>`. `JobConfig.env` is
`BTreeMap<String, SecretString>`. The hash function (`hash.rs:48`) and
serializer (`sync.rs:99`) both rely on `serde_json::json!()` producing
sorted output (which it does for `HashMap` → JSON object via internal
BTreeMap-backed `serde_json::Value::Object`). This works correctly today
but couples hash stability to a serde_json implementation detail.

**Fix:** Switch `labels` to `BTreeMap<String, String>` for explicit,
type-system-enforced ordering. Aligns with `env`'s representation and
makes the hash-stability assumption load-bearing-by-construction.

---

### IN-02: `image_digest` discard via `let _image_digest`

**File:** `src/scheduler/docker.rs:144`

**Issue:** `let _image_digest = match super::docker_pull::ensure_image(...)
{ Ok(digest) => digest, Err(e) => { ... } };` — the bound `_image_digest`
is intentionally discarded (a later `inspect_container` call at line 281
re-fetches the digest). The leading underscore makes this a no-op binding,
not a deliberate "we don't use this." If `ensure_image` returns a digest,
why not use it instead of round-tripping through inspect? Pre-existing,
not introduced by Phase 17, but worth re-evaluating.

**Fix:** Either delete the variable entirely (`let _ = ensure_image(...)`)
or use the digest directly and skip the inspect call when ensure_image
already produced one. Performance optimization, not correctness.

---

### IN-03: Stale TODO-class doc comment in hash.rs

**File:** `src/config/hash.rs:11-15`

**Issue:** The doc comment says

> NOTE: Phase 1 does NOT write data to the `config_hash` column yet;
> the column exists so Phase 2's sync engine does not require another
> migration. This function is therefore unit-tested but not called
> from the run path in Phase 1.

We are now past Phase 17, and `compute_config_hash` is called from
`sync.rs:140` and `sync.rs:157`. The note is stale.

**Fix:** Replace with current-state documentation. Either remove the note
or update to "Called by `sync_config_to_db` for change detection; see
parity table in `src/config/defaults.rs`."

---

### IN-04: Magic numbers in label-size limits

**File:** `src/config/validate.rs:24, 29`

**Issue:** `MAX_LABEL_VALUE_BYTES = 4 * 1024` and `MAX_LABEL_SET_BYTES =
32 * 1024` are constants but the error messages hard-code "4 KB" and
"32 KB" as text. If the limits ever change, the message strings have to
be updated separately.

**Fix:** Format the message using the constants, e.g.
`format!("each label value must be ≤ {} bytes ({} KB)", MAX_LABEL_VALUE_BYTES, MAX_LABEL_VALUE_BYTES / 1024)`.
Trivial. The current messaging is consistent and the limits are unlikely
to drift, so this is a pure tidying suggestion.

---

_Reviewed: 2026-04-28_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
