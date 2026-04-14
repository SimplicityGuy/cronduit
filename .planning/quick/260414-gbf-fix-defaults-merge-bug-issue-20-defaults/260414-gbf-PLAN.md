---
phase: 260414-gbf-fix-defaults-merge-bug-issue-20-defaults
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/config/mod.rs
  - src/config/defaults.rs
  - src/config/validate.rs
  - src/config/hash.rs
  - src/scheduler/sync.rs
  - tests/defaults_merge.rs
  - examples/cronduit.toml
  - docs/SPEC.md
  - .planning/milestones/v1.0-REQUIREMENTS.md
  - .planning/milestones/v1.0-phases/01-foundation-security-posture-persistence-base/01-VERIFICATION.md
  - .github/workflows/release.yml
  - Dockerfile
autonomous: true
requirements:
  - CONF-03
  - CONF-04
  - CONF-06

must_haves:
  truths:
    - "A docker job that relies on `[defaults].image` passes `cronduit check` and classifies as `docker` in the sync engine"
    - "A docker job that relies on `[defaults].network = \"container:vpn\"` produces a DB `config_json` whose `network` field is `container:vpn` (VPN routing preserved, not silently bypassed)"
    - "A docker job that relies on `[defaults].volumes` / `[defaults].timeout` / `[defaults].delete` produces a DB `config_json` whose corresponding field is populated from defaults"
    - "A per-job value for image/network/volumes/timeout/delete always overrides the corresponding `[defaults]` value"
    - "A job with `use_defaults = false` sees only its raw fields — no defaults are merged in"
    - "A docker job with neither a job-level nor a `[defaults]` `image` still fails validation with an error that names `[defaults]` as a valid source of `image`"
    - "`compute_config_hash` returns identical output for the two equivalent TOML representations (field on job vs. field in defaults then merged)"
    - "`random_min_gap` remains a global `@random` scheduler knob read from `Config.defaults` in `cli/run.rs` and `scheduler/reload.rs` — it is NOT merged as a per-job field"
    - "`examples/cronduit.toml` contains at least one docker job that actively exercises the `[defaults]` merge path (relies on `[defaults].image` for its image)"
    - "A docker job with `cmd = ["echo", "hi"]` on the `[[jobs]]` block produces a DB `config_json` whose `cmd` field is `["echo", "hi"]`, and `DockerJobConfig` in the executor deserializes it into `Some(vec!["echo".to_string(), "hi".to_string()])`"
    - "`cmd` is NOT a defaults-eligible field per `docs/SPEC.md` — only `image`, `network`, `volumes`, `delete`, `timeout`, and `random_min_gap` live under `[defaults]`; `apply_defaults` must not touch `job.cmd`, and a `cmd` entry in `[defaults]` (if accepted at all by the TOML parser) must NOT leak into any `JobConfig`"
    - "`v1.0-REQUIREMENTS.md` lines 39 and 42 carry an honest retroactive note that CONF-03 and CONF-06 were satisfied by this quick task, not by the original phase"
    - "The Phase 1 Nyquist audit row for CONF-03/CONF-06 in `01-VERIFICATION.md` is updated to cite the new tests in this task"
    - "The release workflow's docker/build-push-action step consumes tags, labels, AND annotations from the docker/metadata-action@v5 step (id: meta), so the pushed multi-arch image carries `org.opencontainers.image.source` on BOTH the per-platform image configs and the top-level manifest index — the GHCR 'Connected to repository' sidebar link requires the annotation on the index manifest for multi-arch lists."
    - "Every field deserialized by `DockerJobConfig` from `config_json` (image, env, volumes, cmd, network, container_name) is either present in `serialize_config_json` AND `compute_config_hash` AND has an explicit decision in `apply_defaults` (merge-eligible or per-job-only), OR is explicitly allowlisted as a secret that must never be hashed/serialized (env values). The invariant is locked in code by a mermaid `classDiagram` parity table on `src/config/defaults.rs` and a structural unit test `parity_with_docker_job_config_is_maintained` that constructs a fully-populated `JobConfig`, runs it through `serialize_config_json`, and asserts every non-secret field `DockerJobConfig` reads is present in the output."
  artifacts:
    - path: "src/config/defaults.rs"
      provides: "Pure `apply_defaults(job, defaults)` merge function + unit tests for every field, override-wins, use_defaults=false, defaults=None cases, non-touch invariant tests for `cmd` AND `container_name` (both per-job-only per spec), a module-level mermaid `classDiagram` parity table documenting the `JobConfig` / `serialize_config_json` / `compute_config_hash` / `apply_defaults` / `DockerJobConfig` invariant, and a structural unit test `parity_with_docker_job_config_is_maintained` that locks the invariant against future regressions"
      contains: "pub fn apply_defaults"
    - path: "src/config/mod.rs"
      provides: "`JobConfig.delete: Option<bool>` AND `JobConfig.cmd: Option<Vec<String>>` fields + call to `apply_defaults` in `parse_and_validate` before `validate::run_all_checks`; doc comment on `Config.defaults` marking per-job fields as already-merged; `cmd` is per-job only and flows through `serialize_config_json` into `config_json` where `DockerJobConfig.cmd` consumes it"
      contains: "apply_defaults"
    - path: "src/config/validate.rs"
      provides: "Updated `check_one_of_job_type` error message mentioning `[defaults]`; regression test that a docker job with no image anywhere still fails"
      contains: "[defaults]"
    - path: "src/config/hash.rs"
      provides: "Unit tests asserting `compute_config_hash` is stable whether a field lives on the job directly or is merged in from `[defaults]`; hash now includes both the new `delete` and `cmd` fields so change-detection fires when either is toggled"
      contains: "hash_stable_across_defaults_merge"
    - path: "src/scheduler/sync.rs"
      provides: "`job_type()` unchanged but now reliably classifies defaults-image jobs as `docker` because merging happens upstream; `serialize_config_json` emits BOTH the new `delete` field AND the new `cmd` field so values flow all the way to the DB `config_json` column and the executor's `DockerJobConfig` deserialize boundary"
    - path: "tests/defaults_merge.rs"
      provides: "End-to-end `parse_and_validate` tests via NamedTempFile TOML fixtures: image, network=container:vpn, volumes, timeout, delete, per-job override wins, use_defaults=false, docker-job-missing-image-fails regression, defaults-section-absent regression, `cmd_preserved_on_docker_job` (new `cmd` field end-to-end), and `cmd_in_defaults_is_not_merged` (proves a `cmd` in `[defaults]` never leaks into jobs, whichever way the TOML parser handles the unknown key)"
      contains: "#[test]"
    - path: "examples/cronduit.toml"
      provides: "`hello-world` job that relies on `[defaults].image` (its explicit `image` and `delete` lines removed) AND sets `cmd = [\"echo\", \"Hello from cronduit defaults!\"]` so the quickstart actively exercises BOTH the `[defaults]` merge path AND the new per-job `cmd` override"
      contains: "hello-world"
    - path: "docs/SPEC.md"
      provides: "Docker job section updated to document the `cmd` field with an example, and to clarify that `cmd` is per-job only (NOT defaults-eligible)"
      contains: "cmd"
    - path: ".planning/milestones/v1.0-REQUIREMENTS.md"
      provides: "Retroactive honesty note on CONF-03, CONF-04, and CONF-06 lines"
      contains: "retroactively satisfied"
    - path: ".planning/milestones/v1.0-phases/01-foundation-security-posture-persistence-base/01-VERIFICATION.md"
      provides: "Updated evidence citations for CONF-03/CONF-04/CONF-06 rows pointing at the new merge implementation + tests"
      contains: "apply_defaults"
  key_links:
    - from: "src/config/mod.rs::parse_and_validate"
      to: "src/config/defaults.rs::apply_defaults"
      via: "direct call after `toml::from_str` and BEFORE `validate::run_all_checks`, transforming `cfg.jobs` in place"
      pattern: "apply_defaults"
    - from: "src/config/validate.rs::check_one_of_job_type"
      to: "merged JobConfig"
      via: "sees already-merged `job.image` so jobs that rely on `[defaults].image` pass validation naturally"
      pattern: "job.image.is_some"
    - from: "src/config/hash.rs::compute_config_hash"
      to: "merged JobConfig"
      via: "hash is computed on the post-merge job struct, so two equivalent representations produce the same SHA-256"
      pattern: "compute_config_hash"
    - from: "src/scheduler/sync.rs::job_type + serialize_config_json"
      to: "merged JobConfig"
      via: "reads `job.image` / `job.network` / `job.volumes` / `job.timeout` directly — values now come from defaults when the job block omits them"
      pattern: "job_type"
    - from: "src/cli/run.rs + src/scheduler/reload.rs"
      to: "Config.defaults.random_min_gap"
      via: "UNCHANGED direct read — the one legitimate remaining consumer of `Config.defaults`, because `random_min_gap` is a global scheduler knob, not a per-job field"
      pattern: "random_min_gap"
---

<objective>
Fix the v1.0 BLOCKER: `[defaults]` is parsed into `Config.defaults` but never merged into individual `JobConfig`s, so every `[defaults]` field except `random_min_gap` is silently ignored. This breaks CONF-03 and CONF-06 (marked satisfied in `v1.0-REQUIREMENTS.md` lines 39/42 with no implementation) and, worst of all, silently bypasses VPN routing when an operator puts `network = "container:vpn"` in `[defaults]` instead of on the job itself — the exact opposite of Cronduit's marquee feature promise.

Purpose: Re-cut v1.0.0 from a trustworthy commit. The git tag has been deleted; after this PR lands on `main`, v1.0.0 can be tagged again from the fixed commit.

Output:
- A pure `apply_defaults(job, defaults)` function called exactly ONCE in `parse_and_validate` so every downstream consumer (validator, sync, hash, executor) sees already-merged jobs and never needs to know about `[defaults]`.
- A new `delete: Option<bool>` field on `JobConfig` matching the spec (see Known Gap note on executor wiring below).
- Unit tests for the merge function + integration tests through `parse_and_validate` + a `compute_config_hash` stability test.
- A real docker job in `examples/cronduit.toml` that actively exercises the merge path.
- Retroactive honesty notes in `v1.0-REQUIREMENTS.md` and the Phase 1 Nyquist audit row.

Branch: `fix/defaults-merge-issue-20` (already checked out). NO direct commits to main — PR only.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@./CLAUDE.md

Source-of-truth files the executor MUST read before changing code:
@src/config/mod.rs
@src/config/validate.rs
@src/config/hash.rs
@src/scheduler/sync.rs
@src/scheduler/docker.rs
@src/cli/run.rs
@src/scheduler/reload.rs
@examples/cronduit.toml
@docs/SPEC.md
@.planning/milestones/v1.0-REQUIREMENTS.md
@.planning/milestones/v1.0-phases/01-foundation-security-posture-persistence-base/01-VERIFICATION.md

### Interface context (extracted so the executor does not need a scavenger hunt)

`Config`, `DefaultsConfig`, `JobConfig` currently look like this in `src/config/mod.rs` (the file the executor is about to modify):

```rust
pub struct Config {
    pub server: ServerConfig,
    #[serde(default)]
    pub defaults: Option<DefaultsConfig>,
    #[serde(default, rename = "jobs")]
    pub jobs: Vec<JobConfig>,
}

pub struct DefaultsConfig {
    pub image: Option<String>,
    pub network: Option<String>,
    pub volumes: Option<Vec<String>>,
    pub delete: Option<bool>,
    pub timeout: Option<Duration>,           // humantime_serde::option
    pub random_min_gap: Option<Duration>,    // NOT merged per-job — global knob only
}

pub struct JobConfig {
    pub name: String,
    pub schedule: String,
    pub command: Option<String>,
    pub script: Option<String>,
    pub image: Option<String>,
    pub use_defaults: Option<bool>,          // None or Some(true) = apply defaults; Some(false) = opt out
    pub env: BTreeMap<String, SecretString>, // NEVER merged (per-job only)
    pub volumes: Option<Vec<String>>,
    pub network: Option<String>,
    pub container_name: Option<String>,
    pub timeout: Option<Duration>,
    // ---- TASK 1 ADDS: ----
    // #[serde(default)]
    // pub delete: Option<bool>,
}

pub fn parse_and_validate(path: &Path) -> Result<ParsedConfig, Vec<ConfigError>> {
    // reads file, runs env interpolation, then:
    //   toml::from_str::<Config>(&interpolated)?
    // then:
    //   validate::run_all_checks(cfg, path, &raw, &mut errors)
    // TASK 1 INSERTS `apply_defaults` between those two steps and mutates cfg.jobs in place.
}
```

The validator to update (`src/config/validate.rs:49-63`):

```rust
fn check_one_of_job_type(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let count = job.command.is_some() as u8 + job.script.is_some() as u8 + job.image.is_some() as u8;
    if count != 1 {
        errors.push(ConfigError { /* message lacks any mention of [defaults] */ });
    }
}
```

The sync classifier (`src/scheduler/sync.rs:30-41`) — NO edit needed, it will naturally start returning `"docker"` for defaults-image jobs once the merge runs upstream:

```rust
fn job_type(job: &JobConfig) -> &'static str {
    if job.command.is_some() { "command" }
    else if job.script.is_some() { "script" }
    else if job.image.is_some() { "docker" }
    else { "unknown" }
}
```

The ONLY legitimate post-parse consumer of `Config.defaults` (DO NOT touch — `random_min_gap` is a global scheduler knob):

```rust
// src/cli/run.rs lines 72-76 and src/scheduler/reload.rs lines 60-66
let random_min_gap = cfg.defaults.as_ref()
    .and_then(|d| d.random_min_gap)
    .unwrap_or(std::time::Duration::from_secs(0));
```

### Known Gap — `delete` executor wiring is out of scope

`src/scheduler/docker.rs::cleanup_container` currently ALWAYS force-removes the container at the end of `execute_docker`, regardless of any `delete` setting. `DockerJobConfig` has no `delete` field and no branch gating on it.

This plan adds `delete: Option<bool>` to `JobConfig`, merges it from defaults, and serializes it into `config_json` so the data flows all the way to the executor's deserialize boundary. Honoring `delete = false` — i.e. KEEP the container so operators can inspect a failed run — is a separate behavior change that:
1. Must coexist with DOCKER-06 (`auto_remove = false`, avoiding the moby#8441 race).
2. Needs a lifecycle rule for when orphaned containers eventually get cleaned up.
3. Is not what issue #20 is about — #20 is about `[defaults]` being silently ignored.

The SUMMARY for this task MUST explicitly record this gap under a "Known Gap / Follow-up" heading so a future issue can pick it up. Note that `delete = true` is effectively the current behavior (cronduit always removes), so only `delete = false` is a no-op today.

### Historical context

- Issue #20: `[defaults]` merge bug. v1.0 BLOCKER.
- v1.0.0 git tag and GitHub release were DELETED so a fixed v1.0.0 can be re-cut from `main` after this PR merges. Tag and Cargo.toml version string MUST match when re-tagging (user memory constraint).
- `v1.0-REQUIREMENTS.md` CONF-03 (line 39) and CONF-06 (line 42) are marked `[x]` but had no implementation. The Phase 1 Nyquist audit in `01-VERIFICATION.md` lines 133/136 carries the same false "SATISFIED" claim (CONF-06 even admits "execution-time override is Phase 2"; Phase 2 never did it). Both must be updated with honest retroactive notes pointing at this quick task. DO NOT uncheck them — the spec now IS satisfied, it's the history that needs annotation.
- Phase 8 plan instruction said "byte-identical preservation" of the SECURITY comment block, `[server]`, and `[defaults]` sections in `examples/cronduit.toml` (see `08-01-SUMMARY.md:46, 81, 225`). That freeze no longer applies — this is a v1.0 blocker fix post-release. Task 3 deliberately reverses that constraint for the `hello-world` job ONLY (the `[defaults]` section itself stays byte-identical).
- Workflow constraints (from user memory): all changes land via PR on a feature branch — NEVER commit to `main`. Branch `fix/defaults-merge-issue-20` is already checked out. UAT cannot be self-declared passed — a human must run and confirm. Any diagrams in planning artifacts must be mermaid, never ASCII art.
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Add apply_defaults merge function + delete field + wire into parse_and_validate</name>
  <files>src/config/defaults.rs (new), src/config/mod.rs, src/config/hash.rs</files>
  <behavior>
    Unit tests in `src/config/defaults.rs` (write these FIRST — RED commit):

    1. `apply_defaults_fills_image_from_defaults` — job.image=None, defaults.image=Some("alpine:latest") → merged.image=Some("alpine:latest")
    2. `apply_defaults_fills_network_from_defaults` — job.network=None, defaults.network=Some("container:vpn") → merged.network=Some("container:vpn")
    3. `apply_defaults_fills_volumes_from_defaults` — job.volumes=None, defaults.volumes=Some(vec!["/a:/a","/b:/b"]) → merged.volumes=Some(vec!["/a:/a","/b:/b"]) (fully cloned, not shared)
    4. `apply_defaults_fills_timeout_from_defaults` — job.timeout=None, defaults.timeout=Some(300s) → merged.timeout=Some(300s)
    5. `apply_defaults_fills_delete_from_defaults` — job.delete=None, defaults.delete=Some(true) → merged.delete=Some(true)
    6. `apply_defaults_job_override_wins_image` — job.image=Some("nginx:1.25"), defaults.image=Some("alpine:latest") → merged.image=Some("nginx:1.25")
    7. `apply_defaults_job_override_wins_network` — job.network=Some("host"), defaults.network=Some("container:vpn") → merged.network=Some("host")
    8. `apply_defaults_job_override_wins_volumes` — job.volumes=Some(vec!["/job"]), defaults.volumes=Some(vec!["/def"]) → merged.volumes=Some(vec!["/job"]) (per-job REPLACES, arrays are NOT concatenated)
    9. `apply_defaults_job_override_wins_timeout` — job.timeout=Some(60s), defaults.timeout=Some(300s) → merged.timeout=Some(60s)
    10. `apply_defaults_job_override_wins_delete` — job.delete=Some(false), defaults.delete=Some(true) → merged.delete=Some(false)
    11. `apply_defaults_use_defaults_false_disables_merge` — job.use_defaults=Some(false), job.image=None, defaults.image=Some("alpine") → merged.image=None (CONF-04)
    12. `apply_defaults_none_returns_job_unchanged` — defaults=None → returned job has identical field values to input for every merged field
    13. `apply_defaults_does_not_touch_random_min_gap` — build DefaultsConfig with random_min_gap=Some(90min); merged JobConfig must be bit-identical to running the same call with random_min_gap=None (random_min_gap must never leak into per-job state because it has no JobConfig field)
    13b. `apply_defaults_does_not_touch_cmd` — build a JobConfig with `cmd = Some(vec!["a".to_string(), "b".to_string()])` and pass through `apply_defaults` with a full DefaultsConfig (image/network/volumes/delete/timeout all set). Assert merged `job.cmd == Some(vec!["a".to_string(), "b".to_string()])` (unchanged). Also build a JobConfig with `cmd = None` and pass through `apply_defaults`: assert `job.cmd == None` (apply_defaults does NOT invent a cmd from thin air because DefaultsConfig has no `cmd` field). This mirrors `apply_defaults_does_not_touch_random_min_gap` for the new per-job-only `cmd` field.

    Unit tests in `src/config/hash.rs` (add alongside existing tests):

    14. `hash_stable_across_defaults_merge` — construct two JobConfigs manually: (a) image set directly on the job, (b) image=None then passed through `apply_defaults` with defaults.image=Some(same value). `compute_config_hash(&a)` MUST equal `compute_config_hash(&b)`. Repeat the same pattern for network, volumes, timeout, AND `delete` to prove cross-field stability (per field-on-job vs field-from-defaults must produce identical hashes for every merged field, including the new `delete` field).
    15. `hash_differs_on_delete_change` — construct two `JobConfig`s identical except one has `delete: Some(true)` and the other `delete: Some(false)`; assert `compute_config_hash` returns DIFFERENT hex digests. This guards the inverse invariant: change-detection still works after a `delete` flip, so `sync_config_to_db` will classify the job as `updated` (not `unchanged`) when an operator toggles `[defaults].delete`.
    15b. `hash_differs_on_cmd_change` — construct two `JobConfig`s identical except one has `cmd: Some(vec!["a".to_string()])` and the other `cmd: Some(vec!["b".to_string()])`; assert `compute_config_hash` returns DIFFERENT hex digests. Also assert that `cmd: None` vs `cmd: Some(vec![])` produce DIFFERENT hashes — an explicit empty arg list is a valid override that says "run the image with NO command" and is semantically distinct from `None` (which falls through to the image's baked-in CMD). This guards change-detection when an operator adds, changes, or clears `cmd` on a docker job.

    Unit test in `src/scheduler/sync.rs::tests` (add alongside existing serializer tests):

    16. `serialize_config_json_includes_delete` — construct a `JobConfig` with `delete: Some(true)`, call `serialize_config_json(&job)`, parse the returned string as JSON, assert the top-level object contains `"delete": true`. Repeat with `delete: Some(false)` (assert `"delete": false`) and with `delete: None` (assert the key is ABSENT, matching the pattern used for `image`/`network`/`volumes`/`timeout_secs`). Without this test, the serializer change is a silent semantic shift the compiler will not catch.
    16b. `serialize_config_json_includes_cmd` — construct a `JobConfig` with `cmd: Some(vec!["a".to_string(), "b".to_string()])`, call `serialize_config_json(&job)`, parse the returned string as JSON, and assert the top-level object contains `"cmd": ["a","b"]`. Repeat with `cmd: Some(vec![])` (assert the key is present with value `[]` — explicit "no CMD" override, semantically distinct from None) and with `cmd: None` (assert the key is ABSENT). Without this test, the serializer wiring is a silent semantic shift and the plan's own must_haves truth about `cmd` flowing end-to-end to `DockerJobConfig` is unachievable because the value never reaches the DB column.

    GREEN commit: implement `apply_defaults`, add `delete: Option<bool>` AND `cmd: Option<Vec<String>>` to `JobConfig`, fix every compilation error from struct-literal construction, add `delete` and `cmd` to `compute_config_hash` in `src/config/hash.rs`, and add `delete` and `cmd` to `serialize_config_json` in `src/scheduler/sync.rs`.
  </behavior>
  <action>
    Step 1 — Add BOTH the `delete` and `cmd` fields to JobConfig in `src/config/mod.rs`. Insert the two lines after the `timeout` field, grouped with the other docker-job sidecar fields (insertion point: between `image` and `use_defaults` would also be valid, but keeping them after `timeout` keeps the diff localized):

    ```rust
    #[serde(default)]
    pub delete: Option<bool>,
    /// Override the Docker image's baked-in CMD. Per-job ONLY — NOT
    /// defaults-eligible. When None, the container runs with the image's
    /// default CMD; when Some(vec), the vec is passed verbatim to bollard's
    /// ContainerCreateBody.cmd (note: `Some(vec![])` is a valid override
    /// meaning "run with NO command", semantically distinct from None).
    #[serde(default)]
    pub cmd: Option<Vec<String>>,
    ```

    The compiler will now complain about several struct literals in the workspace: fix each by adding BOTH `delete: None,` AND `cmd: None,`. Known sites:
    - `src/config/validate.rs::tests::stub_job`
    - `src/config/hash.rs::tests::mk_job`
    - `src/scheduler/sync.rs::tests::make_job` and `sync_config_json_excludes_secret_values` (inline literal)
    - Any struct literal added by Task 2 inside `tests/defaults_merge.rs` (unlikely — the integration tests parse TOML, but if the executor adds a helper that builds `JobConfig` by hand, both fields must be populated)

    Use `cargo check --all-targets` after the field adds to surface any missed sites.

    `cmd` invariant in `apply_defaults`: the spec's `[defaults]` section (see `docs/SPEC.md`) only lists `image`, `network`, `volumes`, `delete`, `timeout`, and `random_min_gap` as defaults-eligible fields. `cmd` is per-job only. `apply_defaults` MUST NOT read from `defaults.cmd` (which does not exist on `DefaultsConfig` in the first place — do NOT add a `cmd` field to `DefaultsConfig`), and the merge function must pass `job.cmd` through untouched whether the caller started with `Some(vec)` or `None`. Because `apply_defaults` only touches explicitly-listed fields, simply NOT adding a `cmd` branch to the function body is sufficient — but add the `apply_defaults_does_not_touch_cmd` unit test to lock the invariant against a future refactor that grows a `cmd` branch by accident.

    Step 2 — Create `src/config/defaults.rs` with this structure:

    ```rust
    //! Merge [defaults] into each JobConfig exactly once, during parse_and_validate.
    //!
    //! After this runs, every downstream consumer (validator, sync, hash, executor)
    //! reads the already-merged JobConfig directly and MUST NOT consult
    //! Config.defaults for per-job fields. The only remaining consumer of
    //! Config.defaults is `random_min_gap`, which is a global @random scheduler
    //! knob and NOT a per-job field — see src/cli/run.rs and src/scheduler/reload.rs.

    use super::{DefaultsConfig, JobConfig};

    /// Apply [defaults] to a single job. Per-job fields always win.
    ///
    /// - Returns `job` unchanged if `defaults` is `None` or `job.use_defaults == Some(false)` (CONF-04).
    /// - Otherwise fills `image`, `network`, `volumes`, `timeout`, `delete` from defaults
    ///   when the job field is `None`. Per-job values ALWAYS override (CONF-06).
    /// - Never merges `random_min_gap` — that field does not exist on JobConfig;
    ///   it is a global scheduler knob consumed directly from Config.defaults.
    pub fn apply_defaults(mut job: JobConfig, defaults: Option<&DefaultsConfig>) -> JobConfig {
        let Some(defaults) = defaults else { return job; };
        if job.use_defaults == Some(false) { return job; }

        if job.image.is_none() {
            if let Some(v) = &defaults.image { job.image = Some(v.clone()); }
        }
        if job.network.is_none() {
            if let Some(v) = &defaults.network { job.network = Some(v.clone()); }
        }
        if job.volumes.is_none() {
            if let Some(v) = &defaults.volumes { job.volumes = Some(v.clone()); }
        }
        if job.timeout.is_none() {
            if let Some(v) = defaults.timeout { job.timeout = Some(v); }
        }
        if job.delete.is_none() {
            if let Some(v) = defaults.delete { job.delete = Some(v); }
        }
        // NOTE: random_min_gap is intentionally NOT merged — see module doc.

        job
    }

    #[cfg(test)]
    mod tests {
        // Tests 1-13 from <behavior>, all written BEFORE the function body compiles.
    }
    ```

    Register the module in `src/config/mod.rs` by adding `pub mod defaults;` near the top of the file alongside the other `pub mod` lines.

    Step 3 — Wire the merge into `parse_and_validate` in `src/config/mod.rs` around line 147-165. Change `let parsed = match toml::from_str::<Config>(&interpolated) { ... }` to `let mut parsed = match toml::from_str::<Config>(&interpolated) { ... }`, then insert a merge block BEFORE the existing `if let Some(cfg) = &parsed { validate::run_all_checks(...) }` block:

    ```rust
    if let Some(cfg) = &mut parsed {
        // Apply [defaults] to every job before validation so downstream consumers
        // (validator, sync, hash, executor) see already-merged jobs and never need
        // to re-read Config.defaults for per-job fields.
        let defaults = cfg.defaults.as_ref();
        cfg.jobs = std::mem::take(&mut cfg.jobs)
            .into_iter()
            .map(|j| crate::config::defaults::apply_defaults(j, defaults))
            .collect();
    }

    if let Some(cfg) = &parsed {
        validate::run_all_checks(cfg, path, &raw, &mut errors);
    }
    ```

    Borrow checker note: the immutable borrow block (`&parsed`) comes AFTER the `&mut parsed` block ends, so there is no overlap.

    Step 4 — Add a doc comment to the `defaults` field on `Config`:

    ```rust
    /// `[defaults]` section. After `parse_and_validate` returns, per-job
    /// merging has already happened (see `crate::config::defaults::apply_defaults`)
    /// so downstream code MUST NOT re-consult `Config.defaults` for
    /// `image`/`network`/`volumes`/`timeout`/`delete`. The ONLY legitimate
    /// post-parse consumer is `random_min_gap` in `src/cli/run.rs` and
    /// `src/scheduler/reload.rs` — that field is a global scheduler knob,
    /// not a per-job field.
    #[serde(default)]
    pub defaults: Option<DefaultsConfig>,
    ```

    Step 5 — Add BOTH `delete` and `cmd` to `compute_config_hash` in `src/config/hash.rs`, matching the existing field-handling pattern. After the existing `timeout_secs` block (around line 37-39), insert:

    ```rust
    if let Some(d) = &job.delete {
        map.insert("delete", serde_json::json!(d));
    }
    if let Some(c) = &job.cmd {
        map.insert("cmd", serde_json::json!(c));
    }
    ```

    Field order matters less than the presence of the key (BTreeMap sorts keys), but keep insertion order aligned with `JobConfig` field order for readability: `delete` and `cmd` go after `timeout_secs`. DO NOT add them before the `// DO NOT include env` comment — they belong with the merged/per-job-field group, not with env. Important: `Some(vec![])` must hash differently from `None` (serde_json renders `[]` vs omits the key), which is the whole point of `hash_differs_on_cmd_change`'s None-vs-empty-vec assertion.

    Then add the hash unit tests to `src/config/hash.rs::tests`:
    - `hash_stable_across_defaults_merge` — pure-function test: construct two `JobConfig` literals, hash both, assert equal. Exercise image, network, volumes, timeout, AND `delete` in one test body (multiple assertions) or split per field. Either is acceptable. The `delete` case is REQUIRED — without it, Warning 3's concern about future refactors dropping fields during the `parse_and_validate` rebuild is not guarded for the new field. `cmd` is NOT exercised in this stability test because `cmd` is NOT defaults-eligible (there is no "field-on-job vs field-from-defaults" pair for `cmd` by design); the `hash_differs_on_cmd_change` test below is the one that locks in cmd's hash wiring.
    - `hash_differs_on_delete_change` — construct two jobs identical except for `delete: Some(true)` vs `delete: Some(false)`, assert their hashes differ. Guards change-detection.
    - `hash_differs_on_cmd_change` — construct three jobs: (a) `cmd: Some(vec!["a".to_string()])`, (b) `cmd: Some(vec!["b".to_string()])`, (c) `cmd: None`, (d) `cmd: Some(vec![])`. Assert all four hashes are pairwise distinct. Guards change-detection when an operator edits, clears, or adds `cmd`.

    Also update `mk_job` in `src/config/hash.rs::tests` to include BOTH `delete: None,` AND `cmd: None,` in the struct literal (Step 1's compiler pass should already have flagged both, but call them out explicitly so the executor does not skip the test helper).

    Step 6 — Add BOTH `delete` and `cmd` to `serialize_config_json` in `src/scheduler/sync.rs` (around lines 46-77). After the `timeout_secs` insertion (line 68-70) and BEFORE the `env_keys` block (line 71-75), insert:

    ```rust
    if let Some(d) = job.delete {
        map.insert("delete".into(), serde_json::json!(d));
    }
    if let Some(c) = &job.cmd {
        map.insert("cmd".into(), serde_json::json!(c));
    }
    ```

    This is a semantic change the compiler will NOT flag — without it, the plan's own must_haves truths ("A docker job that relies on `[defaults].delete` produces a DB `config_json` whose corresponding field is populated from defaults" AND the new "A docker job with `cmd = [\"echo\",\"hi\"]` ... produces a DB `config_json` whose `cmd` field is `[\"echo\",\"hi\"]`, and `DockerJobConfig` in the executor deserializes it") are unachievable because the data never reaches the DB column. The receiving side is already wired: `src/scheduler/docker.rs:39` has `#[serde(default)] pub cmd: Option<Vec<String>>` on `DockerJobConfig`, and the executor is ready to pass it to bollard via `ContainerCreateBody.cmd`. The gap is only on the write path (TOML → JobConfig → config_json), which Step 1 + Step 6 close together.

    Also add BOTH the `serialize_config_json_includes_delete` test (behavior test 16) AND the new `serialize_config_json_includes_cmd` test (behavior test 16b) inside `src/scheduler/sync.rs::tests`; `serialize_config_json` is module-private so both tests MUST live in the same file's `#[cfg(test)] mod tests` block, NOT in the integration test binary.

    Do NOT modify `DockerJobConfig` in `src/scheduler/docker.rs` — the `cmd` field already exists there (verified during planning at line 39) and the `delete` executor wiring is the Known Gap documented above. Adding `delete` and `cmd` to `config_json` flows both values to the executor's deserialize boundary. For `cmd`, that is sufficient to make docker jobs runnable with operator-controlled arguments — the `cmd` gap is fully closed at the end of this task. For `delete`, honoring `delete = false` (keep-container-on-exit) is still out of scope.

    Commit discipline: RED commit = `test(260414-gbf): failing tests for apply_defaults merge + delete/cmd hash/serialize`, GREEN commit = `feat(260414-gbf): merge [defaults] into jobs + thread delete and cmd through hash and config_json`. The RED commit should fail with "cannot find function `apply_defaults`" plus failing hash/serialize tests for the missing `delete` and `cmd` fields; GREEN makes it pass. Two commits.
  </action>
  <verify>
    <automated>cargo check --all-targets 2>&1 | tail -30 &amp;&amp; cargo nextest run -p cronduit config::defaults:: config::hash::tests::hash_stable_across_defaults_merge 2>&amp;1 | tail -30</automated>
  </verify>
  <done>
    - `src/config/defaults.rs` exists with `pub fn apply_defaults` and 14 passing unit tests (tests 1-13 plus `apply_defaults_does_not_touch_cmd` from `<behavior>`).
    - `JobConfig.delete: Option<bool>` exists AND `JobConfig.cmd: Option<Vec<String>>` exists; every manual `JobConfig { ... }` literal in the workspace compiles with both `delete: None` and `cmd: None` populated.
    - `apply_defaults` does NOT reference `job.cmd` in its body (cmd is per-job only per spec), and the `apply_defaults_does_not_touch_cmd` test locks that invariant.
    - `parse_and_validate` calls `apply_defaults` for every job BEFORE `run_all_checks`, via `std::mem::take` + `.map().collect()`.
    - `Config.defaults` carries a doc comment marking per-job fields as already-merged.
    - `compute_config_hash` in `src/config/hash.rs` INCLUDES both the `delete` and `cmd` fields (insertions placed after `timeout_secs`), so toggling `[defaults].delete` OR editing a job's `cmd` produces a different hash and `sync_config_to_db` correctly classifies the job as `updated` (not `unchanged`).
    - `src/config/hash.rs::tests` has `hash_stable_across_defaults_merge` (covering image/network/volumes/timeout/delete), `hash_differs_on_delete_change`, AND `hash_differs_on_cmd_change` (covering the four pairwise-distinct cases: Some(["a"]), Some(["b"]), None, Some([])) — all passing.
    - `serialize_config_json` in `src/scheduler/sync.rs` INCLUDES BOTH the `delete` field AND the `cmd` field (insertions placed between `timeout_secs` and `env_keys`), so the stored `config_json` now carries the merged `delete` value and the per-job `cmd` override.
    - `src/scheduler/sync.rs::tests` has `serialize_config_json_includes_delete` (Some(true)/Some(false)/None) AND `serialize_config_json_includes_cmd` (Some(["a","b"])/Some([])/None) — both passing.
    - `cargo check --all-targets` is clean, no warnings that didn't exist before.
    - RED and GREEN commits exist on branch `fix/defaults-merge-issue-20`.
  </done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: Integration tests through parse_and_validate + validator error message update</name>
  <files>tests/defaults_merge.rs (new), src/config/validate.rs, Cargo.toml (only if tempfile is missing)</files>
  <behavior>
    End-to-end tests in `tests/defaults_merge.rs` that write a TOML fixture to a NamedTempFile and call `cronduit::config::parse_and_validate`, asserting on the returned `ParsedConfig.config.jobs[0]` fields. This exercises the full parse → interpolate → merge → validate pipeline:

    1. `defaults_image_passes_validation_and_classifies_as_docker` — TOML with `[defaults] image = "alpine:latest"` + a `[[jobs]]` block that sets ONLY `name` and `schedule` (no command/script/image) → parse succeeds, `jobs[0].image == Some("alpine:latest")`, and an inline classifier returns `"docker"`. This is the primary regression test for CONF-03.
    2. `defaults_network_container_vpn_preserved` — `[defaults] network = "container:vpn"` + docker job → merged job has `network == Some("container:vpn")`. MARQUEE FEATURE regression test.
    3. `defaults_volumes_preserved_and_cloned` — `[defaults] volumes = ["/host/a:/a", "/host/b:/b"]` + docker job → merged job has `volumes == Some(vec!["/host/a:/a","/host/b:/b"])`.
    4. `defaults_timeout_preserved` — `[defaults] timeout = "5m"` → merged `jobs[0].timeout == Some(Duration::from_secs(300))`.
    5. `defaults_delete_preserved` — `[defaults] delete = true` → merged `jobs[0].delete == Some(true)`.
    6. `job_override_wins_image` — `[defaults] image = "alpine:latest"` + `[[jobs]] image = "nginx:1.25"` → merged `jobs[0].image == Some("nginx:1.25")`.
    7. `job_override_wins_network` — `[defaults] network = "bridge"` + `[[jobs]] network = "container:vpn"` → `Some("container:vpn")`.
    8. `use_defaults_false_disables_merge` — `[defaults] image = "alpine:latest"` + `[[jobs]] command = "echo hi", use_defaults = false` → parses successfully, merged `jobs[0].image == None` and `jobs[0].command == Some("echo hi")`.
    9. `docker_job_with_no_image_anywhere_still_fails` — NO `[defaults].image`, NO `[[jobs]].image`, NO command/script → `parse_and_validate` returns `Err(errors)`; at least one error message contains BOTH the job name AND the literal substring `[defaults]` (the updated error text from Task 2 Step 2).
    10. `defaults_section_absent_is_legal` — config with NO `[defaults]` section at all + a plain command job → parses cleanly; `jobs[0]` has `image == None`, `network == None`, `volumes == None`, `delete == None`. Regression test for the `defaults=None` early return in `apply_defaults`.
    11. `hash_stable_across_defaults_representations` — end-to-end guard that the `std::mem::take` + `.map().collect()` rebuild in `parse_and_validate` does not drop fields during a future refactor. For EACH of `image`, `network`, `volumes`, `timeout`, `delete`:
        - Fixture A: TOML with the field set on the `[[jobs]]` block directly (no `[defaults]` entry for that field).
        - Fixture B: TOML with the field set in `[defaults]` and omitted on the `[[jobs]]` block.
        - Parse both via `parse_and_validate`.
        - Assert `cronduit::config::hash::compute_config_hash(&a.config.jobs[0]) == cronduit::config::hash::compute_config_hash(&b.config.jobs[0])`.
      This can be ONE test with five assertion blocks, or five smaller tests — either is acceptable. `compute_config_hash` is `pub fn` in `src/config/hash.rs` (verified during planning) so it is reachable from the integration test binary via `cronduit::config::hash::compute_config_hash`. If the visibility has changed at implementation time, promote it back to `pub` (it was `pub` already, DO NOT downgrade it). `cmd` is NOT exercised by this test because `cmd` is per-job only — there is no "field-from-defaults" representation for it by design. Tests 12 and 13 below cover `cmd` end-to-end.
    12. `cmd_preserved_on_docker_job` — TOML with `[defaults] image = "alpine:latest"` (or no `[defaults]` at all) AND a `[[jobs]]` block that declares `image = "alpine:latest"` AND `cmd = ["echo", "hi"]`. Call `parse_and_validate(f.path()).expect("must parse")`. Assert `parsed.config.jobs[0].cmd == Some(vec!["echo".to_string(), "hi".to_string()])`. This is the primary end-to-end regression test proving TOML → `JobConfig.cmd` wiring works and that operators can finally override a docker image's baked-in CMD from their cronduit.toml.
    13. `cmd_in_defaults_is_not_merged` — TOML with `[defaults] image = "alpine:latest"` AND a `cmd = ["echo", "defaulted"]` line in the `[defaults]` section (which is NOT a defaults-eligible field per spec), AND a `[[jobs]]` block that omits `cmd`. Two acceptable outcomes, both assert-able:
        (a) The TOML parser silently accepts the unknown `cmd` key under `[defaults]` (because `#[derive(Deserialize)]` without `#[serde(deny_unknown_fields)]` ignores extras): assert `parse_and_validate` succeeds AND `parsed.config.jobs[0].cmd == None` (the spurious `[defaults].cmd` never leaked into the job).
        (b) The TOML parser rejects `cmd` as an unknown key under `[defaults]`: assert `parse_and_validate` returns `Err` AND at least one error message mentions `cmd` and `[defaults]`.
      The test MUST match on whichever outcome the current `DefaultsConfig` produces (try parsing in a `match`); DO NOT add `#[serde(deny_unknown_fields)]` in this task just to force case (b). Document in the test body comment which case was observed at implementation time and why. The invariant under test is: however the parser handles the unknown key, `cmd` MUST NEVER appear on a job that omits it.

    Unit test added inside `src/config/validate.rs::tests`:

    14. `check_one_of_job_type_error_mentions_defaults` — construct a stub job with `command=None, script=None, image=None`, call `check_one_of_job_type`, assert `e[0].message.contains("[defaults]")` AND `e[0].message.contains("use_defaults")`.
  </behavior>
  <action>
    Step 1 — Verify preconditions:
    - `grep -n '^tempfile' Cargo.toml` — if `tempfile` is NOT already a `[dev-dependencies]` entry, add `tempfile = "3"` under `[dev-dependencies]`. (It is almost certainly already there because many existing tests use it; do not add blindly.)
    - Read `Cargo.toml` `[package] name` to confirm the crate name used in `use` imports. Expected: `cronduit`. If different, use the actual name in the integration test's `use cronduit::config::...` lines.

    Step 2 — Create `tests/defaults_merge.rs` at the workspace root `tests/` directory (this makes it an integration test binary — it canNOT see `pub(crate)` items, only `pub` items). Pattern:

    ```rust
    use cronduit::config::parse_and_validate;
    use std::io::Write;
    use std::time::Duration;
    use tempfile::NamedTempFile;

    fn write_toml(contents: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().expect("tempfile");
        f.write_all(contents.as_bytes()).expect("write");
        f.flush().expect("flush");
        f
    }

    // Minimal valid [server] header reused by every test.
    const SERVER_HEADER: &str = r#"
    [server]
    bind = "127.0.0.1:8080"
    timezone = "UTC"
    "#;

    #[test]
    fn defaults_image_passes_validation_and_classifies_as_docker() {
        let toml = format!(
            r#"{SERVER_HEADER}
    [defaults]
    image = "alpine:latest"

    [[jobs]]
    name = "dockerjob"
    schedule = "*/5 * * * *"
    "#
        );
        let f = write_toml(&toml);
        let parsed = parse_and_validate(f.path()).expect("must parse");
        let job = &parsed.config.jobs[0];
        assert_eq!(job.image.as_deref(), Some("alpine:latest"));
        // Inline classifier — sync::job_type is pub(crate), so replicate here.
        let jt = if job.command.is_some() { "command" }
                 else if job.script.is_some() { "script" }
                 else if job.image.is_some() { "docker" }
                 else { "unknown" };
        assert_eq!(jt, "docker");
    }

    // ... 9 more tests following the same template.
    ```

    Integration test gotchas:
    - Integration tests canNOT see `pub(crate)` items. Classifier is inlined; do NOT try to `use cronduit::scheduler::sync::job_type`.
    - `parse_and_validate` takes `&Path`, so use `f.path()`.
    - When asserting on the error case (test 9), call `parse_and_validate(...).unwrap_err()` and iterate through the returned `Vec<ConfigError>` looking for `e.message.contains("[defaults]")`. `ConfigError` is `pub use errors::{ConfigError, ...}` so it is reachable via `cronduit::config::ConfigError`.
    - TOML fixtures indent with leading whitespace inside `r#"..."#` — TOML parser tolerates leading whitespace on top-level keys but NOT inside a `[[jobs]]` array table header. Use dedented strings: place `[[jobs]]` and friends at column 0 inside the raw string by NOT indenting the closing `"#`. Test this once interactively if unsure.

    Step 3 — Update `check_one_of_job_type` in `src/config/validate.rs:49-63`. New error message:

    ```
    format!(
        "[[jobs]] `{}` must declare exactly one of `command`, `script`, or `image` (found {count}). Note: `image` may also come from `[defaults].image` unless the job sets `use_defaults = false`.",
        job.name
    )
    ```

    Then add the `check_one_of_job_type_error_mentions_defaults` unit test inside the existing `mod tests` block in `validate.rs`.

    Step 4 — Run the full workspace test suite to confirm no collateral regressions. Pay special attention to `src/scheduler/sync.rs::tests` (updated struct literals from Task 1) and `src/config/validate.rs::tests::schedule_*` (stub_job update from Task 1).

    Commit discipline: Because Task 1's `apply_defaults` + `delete` hash/serialize already exist at this point, tests 1-8, 10, and 11 should PASS immediately when added — only test 9 (error message check, depends on this task's validator rewrite) and the validate unit test (test 12) should fail against the old error message. That's fine. Single combined commit: `test+feat(260414-gbf): integration tests for [defaults] merge + validator error message update`. Do NOT fake a two-commit RED/GREEN split when the tests can't all fail simultaneously. Include `use cronduit::config::hash::compute_config_hash;` at the top of `tests/defaults_merge.rs` if test 11 is implemented as a shared-utility rather than per-field tests.
  </action>
  <verify>
    <automated>cargo nextest run --test defaults_merge 2>&amp;1 | tail -30 &amp;&amp; cargo nextest run -p cronduit config::validate::tests::check_one_of_job_type_error_mentions_defaults 2>&amp;1 | tail -10 &amp;&amp; cargo nextest run --workspace 2>&amp;1 | tail -20</automated>
  </verify>
  <done>
    - `tests/defaults_merge.rs` exists with 13 passing integration tests covering every merged field, per-job override (image + network), `use_defaults = false`, the missing-image regression, the defaults-section-absent regression, `hash_stable_across_defaults_representations` (end-to-end hash stability through `parse_and_validate` for image/network/volumes/timeout/delete), `cmd_preserved_on_docker_job` (TOML `cmd = ["echo","hi"]` flows through to `JobConfig.cmd == Some(vec!["echo","hi"])`), AND `cmd_in_defaults_is_not_merged` (a spurious `cmd` under `[defaults]` never leaks into jobs, whichever way the parser handles the unknown key).
    - `check_one_of_job_type` error message contains `[defaults]` and `use_defaults`; matching unit test passes.
    - `cargo nextest run --workspace` shows ZERO failures across all test binaries.
    - `cargo clippy --all-targets --all-features -- -D warnings` is clean (CI gate).
    - `cargo fmt --check` is clean (CI gate).
    - Single commit with `(260414-gbf)` scope exists on `fix/defaults-merge-issue-20`.
  </done>
</task>

<task type="auto">
  <name>Task 3: Update example TOML + requirements note + Phase 1 Nyquist audit row</name>
  <files>examples/cronduit.toml, .planning/milestones/v1.0-REQUIREMENTS.md, .planning/milestones/v1.0-phases/01-foundation-security-posture-persistence-base/01-VERIFICATION.md</files>
  <action>
    Step 1 — `examples/cronduit.toml`. The `[defaults]` section at lines 29-34 already has `image = "alpine:latest"`, `network = "bridge"`, `delete = true`, `timeout = "5m"`. The `hello-world` docker job at lines 84-88 redundantly specifies `image = "hello-world:latest"` and `delete = true`, so neither currently exercises the merge path, AND it has no `cmd` — so it couldn't demo the new `cmd` override either.

    Edit `hello-world` ONLY:
    - Remove the explicit `image = "hello-world:latest"` line (so it inherits `[defaults].image = "alpine:latest"`).
    - Remove the explicit `delete = true` line (so it inherits `[defaults].delete = true`).
    - ADD `cmd = ["echo", "Hello from cronduit defaults!"]` to the same block. alpine:latest has NO baked-in CMD and would otherwise exit immediately with no output, so this `cmd` override is load-bearing: it gives the quickstart a visible "Hello from cronduit defaults!" line in the run logs, which is the whole reason an operator runs the hello-world job in the first place. The `cmd` line also actively exercises the new per-job `cmd` override added in Task 1, so the quickstart demonstrates BOTH the `[defaults]` merge AND the `cmd` feature in a single block.
    - Replace the existing block comment above the `[[jobs]]` header (lines 80-83) with new text explaining the merge-path AND `cmd` demonstration. Exact suggested wording: *"4. Docker job demonstrating BOTH the `[defaults]` merge path (CONF-03/CONF-06) AND the per-job `cmd` override. This block deliberately omits `image`, `network`, `delete`, and `timeout` so they are inherited from `[defaults]` (alpine:latest on the bridge network, cronduit-removes-after-run, 5m timeout). The `cmd = [...]` line overrides alpine:latest's (empty) baked-in CMD so the container actually prints something visible in the run logs — alpine without an explicit cmd would exit immediately with no output. Run `cronduit check examples/cronduit.toml` to validate; confirm on the web UI that the job shows up as type=docker with image=alpine:latest and that a run emits `Hello from cronduit defaults!` to stdout."*

    Do NOT modify: SECURITY comment block (lines 9-15), `[server]` section (lines 17-28), `[defaults]` section (lines 29-34), or the `echo-timestamp` / `http-healthcheck` / `disk-usage` jobs. Phase 8's "byte-identical preservation" constraint on `[defaults]` is honored by this edit — we're only touching one job block.

    After editing, run `cargo run --quiet --bin cronduit -- check examples/cronduit.toml` to confirm the config still validates cleanly. Exit code MUST be 0. Optionally (human-only, not automated): run `cargo run --bin cronduit -- run --config examples/cronduit.toml` long enough to trigger one `hello-world` execution and visually confirm `Hello from cronduit defaults!` lands in the run logs, proving end-to-end `cmd` flow all the way to bollard.

    Step 2 — `.planning/milestones/v1.0-REQUIREMENTS.md`. DO NOT uncheck CONF-03, CONF-04, or CONF-06. Append retroactive notes to each line:

    - CONF-03 (line 39): append ` (retroactively satisfied by quick task `260414-gbf-fix-defaults-merge-bug-issue-20-defaults` after v1.0.0 tag deletion — the Phase 1 parse-only implementation silently ignored every [defaults] field except random_min_gap; see issue #20)`
    - CONF-04 (line 40): append ` (retroactively satisfied by the same task — Phase 1 only parsed `use_defaults`; the early return in `apply_defaults` is what actually makes it functional)`
    - CONF-06 (line 42): append ` (retroactively satisfied by quick task `260414-gbf-fix-defaults-merge-bug-issue-20-defaults` after v1.0.0 tag deletion — the Phase 1 "execution-time override is Phase 2" note never landed in Phase 2; see issue #20)`

    Step 3 — `.planning/milestones/v1.0-phases/01-foundation-security-posture-persistence-base/01-VERIFICATION.md` lines 133-136. Rewrite the CONF-03, CONF-04, and CONF-06 rows with honest citations. Template for CONF-03:

    ```
    | CONF-03 | 01-02 + quick/260414-gbf | [defaults] section with image/network/volumes/delete/timeout/random_min_gap + per-job merge | ✓ SATISFIED (retroactive — see issue #20) | Phase 1: `DefaultsConfig` struct + `valid-everything.toml` fixture. Merge was missing until issue #20 fix: `src/config/defaults.rs::apply_defaults`, called from `parse_and_validate`; `tests/defaults_merge.rs` covers image/network/volumes/timeout/delete + per-job override + use_defaults=false + no-image-fails + defaults-absent |
    ```

    Apply the same pattern to CONF-04 (`execution-time enforcement via apply_defaults early return`) and CONF-06 (`override precedence verified by job_override_wins_* tests in tests/defaults_merge.rs`). Keep table alignment readable; update the `Plan` column, `Status` column, and `Evidence` column.

    Before writing, grep `01-VERIFICATION.md` for any other rows whose status notes mention "Phase 2" or "execution-time" in a way that depends on the defaults merge working. If any are found, update them with the same retroactive-note pattern. Do NOT change any row that already passes for reasons unrelated to defaults.

    Step 3.5 — `docs/SPEC.md`. Locate the Docker job section (approximately lines 145-155; grep for "Docker Jobs" or "image =" to find it). Update the existing example to document the new `cmd` field by adding a `cmd = [...]` line to one of the example docker jobs AND adding a one-paragraph note explaining it. Suggested additions:

    1. Example code block addition (add a new fenced block or extend an existing one):

       ```toml
       [[jobs]]
       name = "curl-healthcheck"
       schedule = "*/5 * * * *"
       image = "curlimages/curl:8.5.0"
       cmd = ["curl", "-sf", "https://example.com/health"]
       ```

    2. Explanatory paragraph (place immediately after the example):

       > `cmd` is an optional list of strings that overrides the Docker image's baked-in `CMD`. When set, the vec is passed verbatim to the container at start time. When unset, the container runs with whatever `CMD` the image defines (which may be nothing at all — e.g. `alpine:latest` has no default `CMD`, so a docker job using `alpine` with no `cmd` will exit immediately with no output). `cmd` is a PER-JOB-ONLY field and is NOT available under `[defaults]` — every job must declare its own override (or inherit the image default). This matches the semantics of `docker run IMAGE CMD...` on the command line.

    Also confirm the Defaults section of the same doc lists EXACTLY `image`, `network`, `volumes`, `delete`, `timeout`, and `random_min_gap` as the only defaults-eligible fields — if any note elsewhere in the doc implies `cmd` is in `[defaults]`, correct it. If the spec already has a full `[[jobs]]` field reference table, add `cmd` to that table with a note "per-job only".

    Step 4 — Commit with `docs(260414-gbf): update example, requirements, Phase 1 audit, and SPEC for [defaults] merge + cmd field`.
  </action>
  <verify>
    <automated>cargo run --quiet --bin cronduit -- check examples/cronduit.toml; echo "---"; grep -c 'retroactively satisfied' .planning/milestones/v1.0-REQUIREMENTS.md; grep -c '260414-gbf' .planning/milestones/v1.0-phases/01-foundation-security-posture-persistence-base/01-VERIFICATION.md; grep -c 'image = "hello-world:latest"' examples/cronduit.toml</automated>
  </verify>
  <done>
    - `cargo run --bin cronduit -- check examples/cronduit.toml` exits 0.
    - `examples/cronduit.toml` has zero occurrences of `image = "hello-world:latest"` (the grep -c count is 0).
    - The `hello-world` job has no `image` or `delete` lines inside its block; the block has a `cmd = ["echo", "Hello from cronduit defaults!"]` line; the block comment explains the merge-path AND `cmd` demonstration.
    - `.planning/milestones/v1.0-REQUIREMENTS.md` contains at least 3 occurrences of `retroactively satisfied` (CONF-03, CONF-04, CONF-06).
    - `01-VERIFICATION.md` CONF-03/CONF-04/CONF-06 rows cite `src/config/defaults.rs::apply_defaults` and `tests/defaults_merge.rs` as evidence.
    - `docs/SPEC.md` documents `cmd` as a per-job-only docker field with an example code block AND a paragraph explaining override semantics; the Defaults section does NOT list `cmd` as defaults-eligible.
    - Single docs commit with `(260414-gbf)` scope exists on `fix/defaults-merge-issue-20`.
  </done>
</task>

<task type="auto">
  <name>Task 4: Fix Docker image labels for GHCR via docker/metadata-action + annotations</name>
  <files>.github/workflows/release.yml, Dockerfile</files>
  <behavior>
    GHCR's "Connected to repository" sidebar link on the package page reads `org.opencontainers.image.source`. For multi-arch (manifest list) images, that value must be present on BOTH the per-platform image config (as a LABEL) AND the top-level manifest index (as an OCI ANNOTATION). The current workflow only passes `labels:` to `docker/build-push-action@v6`, which sets LABELs on the platform images but leaves the manifest index without annotations. That is the real-world "labels are broken" symptom: GHCR sometimes fails to link the image back to the source repo for multi-arch manifests depending on which manifest its UI queries first.

    Authoritative source: https://docs.github.com/packages/working-with-a-github-packages-registry/working-with-the-container-registry#labelling-container-images — GitHub recommends EXACTLY three labels:
    - `org.opencontainers.image.source` (enables the Connected to repository sidebar link)
    - `org.opencontainers.image.description` (text under package name, ≤512 chars)
    - `org.opencontainers.image.licenses` (SPDX identifier, ≤256 chars, shown in sidebar)

    All three are currently set as LABELs. None are currently set as ANNOTATIONs on the manifest index. This task adds `docker/metadata-action@v5` to generate labels + tags + annotations from a single source of truth, then feeds all three outputs into `build-push-action@v6` so the manifest INDEX carries the same metadata as the platform images. The action also auto-populates `source`, `revision`, `version`, `created`, and `url` from repo context, eliminating the hand-maintained values in the current workflow.
  </behavior>
  <action>
    Step 1 — Read the current state of `.github/workflows/release.yml` and `Dockerfile` end-to-end so every edit below is based on the real file content, not a mental model. In particular, note:
    - The `Compute lowercase image name` step exports `IMAGE_NAME` to `$GITHUB_ENV` — metadata-action's `images:` input must still use `${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}`, NOT `${{ github.repository }}`, so the lowercase-owner fix from #19 stays in effect.
    - The `Extract version from tag` step outputs `version`, `minor`, `major`. metadata-action's `type=semver` tag templates replace these directly; the manual step can stay for the `Create GitHub Release` step that references `steps.version.outputs.version`, OR it can be removed if the release step is updated to use metadata-action outputs. Prefer KEEPING the version step and ONLY replacing the tag/label generation in build-push-action, to minimize blast radius on the `softprops/action-gh-release` step.
    - The existing `Build and push multi-arch image` step uses `docker/build-push-action@v6`, which supports the `annotations:` input. Do NOT downgrade to v5.

    Step 2 — Insert a `docker/metadata-action@v5` step in `.github/workflows/release.yml` BETWEEN the existing `Log in to GHCR` step and the existing `Build and push multi-arch image` step. The new step:

    ```yaml
    - name: Extract Docker metadata (labels, tags, annotations)
      id: meta
      uses: docker/metadata-action@v5
      with:
        images: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}
        # Tag templates replace the hand-rolled multi-tag list below. The
        # `type=semver` entries derive semver-aware tags from the pushed git
        # tag (v1.0.0 -> 1.0.0, 1.0, 1). `type=raw,value=latest` keeps the
        # floating latest tag pointed at every release.
        tags: |
          type=semver,pattern={{version}}
          type=semver,pattern={{major}}.{{minor}}
          type=semver,pattern={{major}}
          type=raw,value=latest
        # Labels GitHub Container Registry recognizes for the package page:
        #   - org.opencontainers.image.source       (Connected to repository)
        #   - org.opencontainers.image.description  (package subtitle)
        #   - org.opencontainers.image.licenses     (sidebar license badge)
        # `revision`, `version`, `created`, `url` are auto-populated by
        # metadata-action from repo context. `source` is ALSO auto-populated,
        # but we emit it explicitly below as belt-and-suspenders: if auto-
        # population ever silently fails (image-name parsing quirk, version
        # drift), the explicit value still lands on the image. metadata-action
        # deduplicates by key, so there is no risk of two conflicting values.
        labels: |
          org.opencontainers.image.title=Cronduit
          org.opencontainers.image.description=Self-hosted Docker-native cron scheduler with a web UI
          org.opencontainers.image.licenses=MIT OR Apache-2.0
          org.opencontainers.image.vendor=SimplicityGuy
          org.opencontainers.image.source=https://github.com/${{ github.repository }}
        # Annotations are written to the manifest INDEX (and per-platform
        # manifests) rather than the image config. GHCR's "Connected to
        # repository" link for multi-arch images reads the index manifest,
        # so `org.opencontainers.image.source` MUST be present here, not
        # only as a label on the platform images. metadata-action emits an
        # `index:` prefix internally so the value lands on the top-level
        # manifest list -- do not add the prefix manually.
        annotations: |
          org.opencontainers.image.title=Cronduit
          org.opencontainers.image.description=Self-hosted Docker-native cron scheduler with a web UI
          org.opencontainers.image.licenses=MIT OR Apache-2.0
          org.opencontainers.image.vendor=SimplicityGuy
          org.opencontainers.image.source=https://github.com/${{ github.repository }}
    ```

    Belt-and-suspenders note on the explicit `source` line: `docker/metadata-action@v5` DOES auto-populate `org.opencontainers.image.source` in both `labels:` and `annotations:` outputs, but emitting it explicitly makes the guarantee load-bearing instead of dependent on action internals. This is the exact thing #20's Docker-labels prong is about — if the retagged `v1.0.0` ships without `source` on the manifest index, the GHCR "Connected to repository" sidebar link breaks again and we are back where we started. metadata-action dedupes by key, so the explicit and auto-populated values merge cleanly.

    Step 3 — Replace the hand-rolled `tags:` and `labels:` block in the existing `Build and push multi-arch image` step with references to the metadata-action outputs, and add an `annotations:` input. After the edit, that step body should read:

    ```yaml
    - name: Build and push multi-arch image
      uses: docker/build-push-action@v6
      with:
        context: .
        platforms: linux/amd64,linux/arm64
        push: true
        tags: ${{ steps.meta.outputs.tags }}
        labels: ${{ steps.meta.outputs.labels }}
        annotations: ${{ steps.meta.outputs.annotations }}
        cache-from: type=gha,scope=cronduit-release
        cache-to: type=gha,mode=max,scope=cronduit-release
    ```

    Do NOT touch the `Create GitHub Release` step — it continues to use `steps.version.outputs.version` from the existing `Extract version from tag` step (kept intentionally for the prerelease-detection `contains(steps.version.outputs.version, '-')` check).

    Step 4 — Update the static `LABEL` block in `Dockerfile` lines 84-88. The LABEL values stay the same (workflow wins via metadata-action anyway), but the comment block above them expands to document:
    - Which three labels GHCR recognizes for the package page UI.
    - That dynamic labels/annotations are injected by `docker/metadata-action@v5` in `.github/workflows/release.yml` and override these at release time.
    - That these LABELs are the fallback for local `docker build .` usage outside the release workflow.
    - A link to the authoritative GitHub docs page.

    Replacement text for lines 84-88:

    ```dockerfile
    # Static OCI labels -- fallback for local `docker build .` outside the
    # release workflow. The three labels below are the ones GitHub Container
    # Registry recognizes on the package page:
    #   org.opencontainers.image.source       -> "Connected to repository" link
    #   org.opencontainers.image.description  -> subtitle under the package name
    #   org.opencontainers.image.licenses     -> license badge in the sidebar
    # At release time, docker/metadata-action@v5 in .github/workflows/release.yml
    # generates a fuller label + annotation set (title, vendor, version,
    # revision, created, url) and writes them to BOTH the per-platform image
    # configs and the top-level manifest INDEX, which is what GHCR reads for
    # multi-arch images. See:
    # https://docs.github.com/packages/working-with-a-github-packages-registry/working-with-the-container-registry#labelling-container-images
    LABEL org.opencontainers.image.source="https://github.com/SimplicityGuy/cronduit"
    LABEL org.opencontainers.image.description="Self-hosted Docker-native cron scheduler with a web UI"
    LABEL org.opencontainers.image.licenses="MIT OR Apache-2.0"
    ```

    Step 5 — Workflow validation (no CI run; this is a structural check):
    - `grep -c 'docker/metadata-action@v5' .github/workflows/release.yml` → 1
    - `grep -c 'id: meta' .github/workflows/release.yml` → 1
    - `grep -c 'steps.meta.outputs.tags' .github/workflows/release.yml` → 1
    - `grep -c 'steps.meta.outputs.labels' .github/workflows/release.yml` → 1
    - `grep -c 'steps.meta.outputs.annotations' .github/workflows/release.yml` → 1
    - `grep -c '${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:${{ steps.version.outputs' .github/workflows/release.yml` → 0 (old inline tags removed)
    - `python3 -c 'import yaml,sys; yaml.safe_load(open(".github/workflows/release.yml"))' && echo YAML_OK` → prints `YAML_OK` (sanity check the file parses)

    Step 6 — Commit discipline: single commit with the message:

    ```
    ci(260414-gbf): generate OCI labels + annotations via docker/metadata-action

    Multi-arch images pushed to GHCR need org.opencontainers.image.source on
    the top-level manifest INDEX (not only the per-platform image configs)
    for the "Connected to repository" sidebar link to resolve. The previous
    build-push-action step set labels: but not annotations:, so the index
    manifest was missing the required metadata.

    This commit replaces the hand-rolled tags: and labels: block with
    docker/metadata-action@v5 outputs, adds annotations: sourced from the
    same action, and expands the Dockerfile LABEL comment block to document
    why both label and annotation paths exist.

    GitHub-recognized labels (unchanged in value):
      - org.opencontainers.image.source
      - org.opencontainers.image.description
      - org.opencontainers.image.licenses

    Refs issue #20.
    ```
  </action>
  <verify>
    <automated>grep -c 'docker/metadata-action@v5' .github/workflows/release.yml; grep -c 'steps.meta.outputs.tags' .github/workflows/release.yml; grep -c 'steps.meta.outputs.labels' .github/workflows/release.yml; grep -c 'steps.meta.outputs.annotations' .github/workflows/release.yml; python3 -c 'import yaml; yaml.safe_load(open(".github/workflows/release.yml"))' && echo YAML_OK</automated>
  </verify>
  <done>
    - `.github/workflows/release.yml` has a `docker/metadata-action@v5` step with `id: meta` that sets `images`, `tags`, `labels`, and `annotations` inputs.
    - `labels:` includes `title`, `description`, `licenses`, `vendor`, AND an EXPLICIT `org.opencontainers.image.source=https://github.com/${{ github.repository }}` line (belt-and-suspenders; metadata-action also auto-populates it). Auto-populated-only fields: `revision`, `version`, `created`, `url`.
    - `annotations:` mirrors `labels:` INCLUDING the explicit `source` line, so the manifest INDEX carries `org.opencontainers.image.source` for GHCR's "Connected to repository" link even if metadata-action's auto-population ever regresses.
    - `build-push-action@v6` step references `${{ steps.meta.outputs.tags }}`, `${{ steps.meta.outputs.labels }}`, and `${{ steps.meta.outputs.annotations }}`.
    - Old inline tag list (`:${version}`, `:${minor}`, `:${major}`, `:latest`) is removed from `build-push-action`; those tags are now generated by metadata-action's `type=semver` + `type=raw,value=latest` templates.
    - `Dockerfile` LABEL values unchanged; comment block expanded to explain the workflow override and link to GitHub's labelling docs.
    - `python3 yaml.safe_load` of the workflow passes (no accidental YAML breakage).
    - Single commit with `ci(260414-gbf)` scope exists on `fix/defaults-merge-issue-20`.
  </done>
</task>

<task type="auto" tdd="true">
  <name>Task 5: Parity audit — lock JobConfig / serialize_config_json / compute_config_hash / apply_defaults / DockerJobConfig invariant in code</name>
  <files>src/config/defaults.rs</files>
  <behavior>
    **Audit finding (performed during planning, 2026-04-14):**

    Two executor structs deserialize from `config_json`:
    1. `src/scheduler/docker.rs::DockerJobConfig` (lines 26-46). Fields: `image: String`, `env: HashMap<String,String>`, `volumes: Option<Vec<String>>`, `cmd: Option<Vec<String>>`, `network: Option<String>`, `container_name: Option<String>`.
    2. `src/scheduler/run.rs::JobExecConfig` (lines 49-53). Fields: `command: Option<String>`, `script: Option<String>`.

    `command.rs::execute_command` and `script.rs::execute_script` do NOT deserialize from `config_json` directly — they receive already-extracted `&str` arguments. The parity surface is therefore `DockerJobConfig` + `JobExecConfig`.

    Parity table (`DockerJobConfig`):

    | Executor field | `JobConfig` field | `serialize_config_json` | `compute_config_hash` | `apply_defaults` decision |
    |---|---|---|---|---|
    | `image: String` | `image: Option<String>` | YES (sync.rs:56-58) | YES (hash.rs:25-27) | Mergeable (Task 1) |
    | `env: HashMap<String,String>` | `env: BTreeMap<String,SecretString>` | KEYS-ONLY (sync.rs:72-75) | EXCLUDED — secret | Per-job only, secret allowlist |
    | `volumes: Option<Vec<String>>` | `volumes: Option<Vec<String>>` | YES (sync.rs:59-61) | YES (hash.rs:28-30) | Mergeable (Task 1) |
    | `cmd: Option<Vec<String>>` | `cmd: Option<Vec<String>>` (Task 1) | YES (Task 1) | YES (Task 1) | Per-job only (Task 1) |
    | `network: Option<String>` | `network: Option<String>` | YES (sync.rs:62-64) | YES (hash.rs:31-33) | Mergeable (Task 1) |
    | `container_name: Option<String>` | `container_name: Option<String>` | YES (sync.rs:65-67) | YES (hash.rs:34-36) | **GAP — undocumented** |

    Parity table (`JobExecConfig`):

    | Executor field | `JobConfig` field | `serialize_config_json` | `compute_config_hash` | `apply_defaults` decision |
    |---|---|---|---|---|
    | `command: Option<String>` | `command: Option<String>` | YES (sync.rs:50-52) | YES (hash.rs:19-21) | Per-job only (job identity) |
    | `script: Option<String>` | `script: Option<String>` | YES (sync.rs:53-55) | YES (hash.rs:22-24) | Per-job only (job identity) |

    Also plumbed (NOT on any executor deserialize struct, extracted from `DbJob.timeout_secs`): `timeout: Option<Duration>` — in `JobConfig`, in `serialize_config_json` as `timeout_secs` (sync.rs:68-70), in `compute_config_hash` as `timeout_secs` (hash.rs:37-39), mergeable via Task 1. Parity OK.

    **Gaps found beyond Tasks 1/2 scope:**

    1. `container_name` is read by `DockerJobConfig`, serialized, hashed — but has NO explicit decision in `apply_defaults` and is NOT documented as per-job-only. It is NOT on `DefaultsConfig` (correctly — two containers cannot share a name), but the "why it is not mergeable" decision is nowhere in code. A future contributor reviewing `apply_defaults` could reasonably wonder why `image`/`network`/`volumes`/`delete`/`timeout` appear but `container_name` does not. This is exactly the class of silent-gap the audit is designed to catch.

    **Gap NOT found:**

    - No other `DockerJobConfig` or `JobExecConfig` fields are missing from the parity matrix. `cmd` is the only recently-discovered gap and is already closed by Task 1. `delete` is a new field added by Task 1 that has no executor counterpart yet (Known Gap — intentional, not in scope).

    **Task 5 scope (small):**

    Because the audit found exactly ONE additional undocumented gap (`container_name`), and that gap is a pure decision/documentation issue — not a behavior fix — Task 5 ships as THREE things, all in `src/config/defaults.rs`:

    1. A module-level mermaid `classDiagram` parity table (doc comment block at the top of the file) making the invariant visible to every future PR reviewer who touches defaults / sync / hash.
    2. A new unit test `apply_defaults_does_not_touch_container_name` mirroring `apply_defaults_does_not_touch_cmd` — constructs a `JobConfig` with `container_name = Some("fixed-name")` and one with `container_name = None`, runs both through `apply_defaults` with a fully-populated `DefaultsConfig`, asserts both come back unchanged.
    3. A new unit test `parity_with_docker_job_config_is_maintained` — constructs a `JobConfig` with EVERY non-secret docker-reachable field set to a sentinel value (image, volumes, network, container_name, cmd, timeout, delete — and env with one entry so the env-keys path is exercised), calls `crate::scheduler::sync::serialize_config_json` on it, parses the returned string as `serde_json::Value`, and asserts each of the keys `DockerJobConfig` reads is present in the JSON (or, for `env`, that `env_keys` is present while the raw value is NOT). The test uses `crate::scheduler::docker::DockerJobConfig` as a compile-time smoke check (`let _: DockerJobConfig = serde_json::from_str(...)?;`) so any future addition to `DockerJobConfig` that lacks a matching `serialize_config_json` entry fails to deserialize and the test goes red.

    Unit tests (write these FIRST — RED commit):

    T5-1. `apply_defaults_does_not_touch_container_name` — same shape as `apply_defaults_does_not_touch_cmd`. Two cases: `container_name = Some("fixed")` passes through unchanged, `container_name = None` stays `None`. Proves `apply_defaults` never invents a container name from any source (there is no `DefaultsConfig.container_name` field, and the merge branch must never grow one).

    T5-2. `parity_with_docker_job_config_is_maintained` — structural, not behavioral:

    ```rust
    use crate::scheduler::docker::DockerJobConfig;
    use crate::scheduler::sync;  // serialize_config_json is pub(super) — see Action Step 3

    let mut env = std::collections::BTreeMap::new();
    env.insert("SECRET_KEY".to_string(), secrecy::SecretString::from("super-secret"));
    let job = JobConfig {
        name: "parity-test".to_string(),
        schedule: "*/5 * * * *".to_string(),
        command: None,
        script: None,
        image: Some("alpine:latest".to_string()),
        use_defaults: None,
        env,
        volumes: Some(vec!["/host:/container".to_string()]),
        network: Some("container:vpn".to_string()),
        container_name: Some("parity-test-container".to_string()),
        timeout: Some(Duration::from_secs(300)),
        delete: Some(true),
        cmd: Some(vec!["echo".to_string(), "parity".to_string()]),
    };

    let json_str = sync::serialize_config_json(&job);
    let v: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
    let obj = v.as_object().expect("top-level object");

    // Every non-secret field DockerJobConfig reads MUST be in the output.
    assert!(obj.contains_key("image"), "image missing from config_json — DockerJobConfig would fail to deserialize");
    assert!(obj.contains_key("volumes"), "volumes missing from config_json");
    assert!(obj.contains_key("network"), "network missing from config_json");
    assert!(obj.contains_key("container_name"), "container_name missing from config_json");
    assert!(obj.contains_key("cmd"), "cmd missing from config_json");
    // env is the secret allowlist: env_keys present, raw env values ABSENT.
    assert!(obj.contains_key("env_keys"), "env_keys missing — key-name allowlist broken");
    let json_body = serde_json::to_string(obj).unwrap();
    assert!(!json_body.contains("super-secret"), "T-02-03 breach: raw SecretString value in config_json");

    // DockerJobConfig compile-time smoke: confirm the emitted JSON is at least
    // structurally deserializable as a DockerJobConfig. This is a one-way
    // assertion — DockerJobConfig only consumes a subset — but it fails loudly
    // if a typed field name drifts (e.g. someone renames `image` → `image_ref`
    // on JobConfig without updating both sides).
    let _check: DockerJobConfig = serde_json::from_str(&json_str)
        .expect("serialize_config_json output must be a valid DockerJobConfig");
    ```

    Note on `serialize_config_json` visibility: it is currently `fn serialize_config_json(...)` (module-private) in `src/scheduler/sync.rs:46`. The parity test lives in `src/config/defaults.rs::tests` which is in a DIFFERENT module and cannot see `pub(super)` items, let alone `fn`. Action Step 3 promotes it to `pub(crate) fn` so the test can call it. This is a visibility change only — no runtime cost, no public-API surface change (the `pub(crate)` scope keeps it internal to the crate).

    GREEN commit: add the mermaid parity table as module-level doc on `src/config/defaults.rs`, promote `serialize_config_json` to `pub(crate)`, add both unit tests. If `DockerJobConfig` deserialize check fails at test-run time, the failure message is self-explanatory — a contributor who added a field without updating the serializer gets a direct pointer to the three files that need to stay in sync.
  </behavior>
  <action>
    Step 1 — Add the mermaid parity table as the FIRST doc-comment block in `src/config/defaults.rs`, ABOVE the existing `//! Merge [defaults] into each JobConfig exactly once...` line. The block MUST be a fenced mermaid code block inside Rust doc comments (per user-memory rule: all diagrams in any project artifact are mermaid, never ASCII).

    ```rust
    //! # Config plumbing parity invariant
    //!
    //! This module is the single point of truth for how `[defaults]` merges into
    //! per-job `JobConfig`s, but it is ALSO load-bearing as documentation for the
    //! broader "config-to-executor plumbing" invariant. Five layers must stay in
    //! lock-step for any field that ends up on an executor deserialize struct:
    //!
    //! 1. `JobConfig` in `src/config/mod.rs` — the TOML-side struct.
    //! 2. `serialize_config_json` in `src/scheduler/sync.rs` — writes to the DB
    //!    `config_json` column that the executor reads back.
    //! 3. `compute_config_hash` in `src/config/hash.rs` — change-detection for
    //!    `sync_config_to_db` so an operator's edit triggers an `updated` upsert.
    //! 4. `apply_defaults` in THIS file — decides whether `[defaults]` merges
    //!    into the field or the field is per-job-only.
    //! 5. `DockerJobConfig` in `src/scheduler/docker.rs` — the executor-side
    //!    deserialize struct that reads the serialized JSON.
    //!
    //! When one of the five drifts without the others, silent behavior regressions
    //! slip through unit tests that construct hand-rolled fixtures. The class of
    //! bug that produced both the `[defaults]` merge bug (issue #20) AND the
    //! missing `cmd` field was the same root cause: the executor-side struct was
    //! never cross-referenced with the TOML-side struct or the DB path.
    //!
    //! ```mermaid
    //! classDiagram
    //!     class JobConfig {
    //!         +name: String
    //!         +schedule: String
    //!         +command: Option~String~
    //!         +script: Option~String~
    //!         +image: Option~String~
    //!         +volumes: Option~Vec~String~~
    //!         +network: Option~String~
    //!         +container_name: Option~String~
    //!         +cmd: Option~Vec~String~~
    //!         +delete: Option~bool~
    //!         +timeout: Option~Duration~
    //!         +env: BTreeMap~String,SecretString~
    //!         +use_defaults: Option~bool~
    //!     }
    //!     class DefaultsConfig {
    //!         +image: Option~String~
    //!         +network: Option~String~
    //!         +volumes: Option~Vec~String~~
    //!         +delete: Option~bool~
    //!         +timeout: Option~Duration~
    //!         +random_min_gap: Option~Duration~
    //!     }
    //!     class DockerJobConfig {
    //!         +image: String
    //!         +env: HashMap~String,String~
    //!         +volumes: Option~Vec~String~~
    //!         +cmd: Option~Vec~String~~
    //!         +network: Option~String~
    //!         +container_name: Option~String~
    //!     }
    //!     JobConfig --> DefaultsConfig : apply_defaults merges image/network/volumes/delete/timeout
    //!     JobConfig --> DockerJobConfig : serialize_config_json -> config_json -> deserialize
    //! ```
    //!
    //! ## Parity table
    //!
    //! | DockerJobConfig field | JobConfig field | serialize_config_json | compute_config_hash | apply_defaults decision | Notes |
    //! |---|---|---|---|---|---|
    //! | `image`           | `image`          | yes | yes | mergeable                 | Falls back to `[defaults].image` |
    //! | `env`             | `env`            | keys only (`env_keys`) | excluded | per-job only (secret)    | T-02-03: values are `SecretString`, never hashed/logged |
    //! | `volumes`         | `volumes`        | yes | yes | mergeable                 | Per-job REPLACES defaults (no concatenation) |
    //! | `cmd`             | `cmd`            | yes | yes | per-job only              | NOT in `DefaultsConfig`. `Some(vec![])` is distinct from `None` |
    //! | `network`         | `network`        | yes | yes | mergeable                 | Includes `container:<name>` VPN mode — marquee feature |
    //! | `container_name`  | `container_name` | yes | yes | per-job only              | NOT in `DefaultsConfig` — container names must be unique |
    //!
    //! Fields in `JobConfig` that are NOT read by `DockerJobConfig` but still
    //! flow through the plumbing: `name`, `schedule`, `command`, `script`,
    //! `timeout` (becomes `DbJob.timeout_secs`, used by every executor),
    //! `delete` (serialized + hashed but not yet honored by the docker executor
    //! — see Known Gap in the plan's objective), `use_defaults` (consumed by
    //! `apply_defaults` itself and then dropped — not serialized, not hashed).
    //!
    //! ## Adding a new field
    //!
    //! Any future PR that adds a field to any ONE of these five layers MUST
    //! update the other four in the same commit. The `parity_with_docker_job_config_is_maintained`
    //! unit test below is a regression guard for the JSON surface — it will
    //! fail loudly if `serialize_config_json` drops a field that `DockerJobConfig`
    //! reads. It does NOT catch `compute_config_hash` or `apply_defaults` drift;
    //! those still rely on PR review discipline and the parity table above.
    //!
    ```

    Step 2 — Promote `serialize_config_json` from module-private to `pub(crate)` in `src/scheduler/sync.rs:46`:

    ```rust
    pub(crate) fn serialize_config_json(job: &JobConfig) -> String {
    ```

    No other signature change. No behavior change. This makes the function reachable from `src/config/defaults.rs::tests` so the parity test can call it without reflection. `pub(crate)` is the minimum visibility needed — do NOT use `pub`.

    Step 3 — Add both unit tests (T5-1 and T5-2) to the existing `#[cfg(test)] mod tests` block in `src/config/defaults.rs`. Place them AFTER the existing `apply_defaults_does_not_touch_cmd` test (behavior 13b from Task 1) so the three non-touch invariant tests (`random_min_gap`, `cmd`, `container_name`) sit together.

    For T5-2 (`parity_with_docker_job_config_is_maintained`), use the full test body shown in the `<behavior>` block. Key imports at the top of the test module (ADD if missing):

    ```rust
    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::scheduler::docker::DockerJobConfig;
        use crate::scheduler::sync;
        use std::collections::BTreeMap;
        use std::time::Duration;
        // ... existing imports ...
    }
    ```

    The `DockerJobConfig` import is REQUIRED — the test MUST use the existing executor struct as the source of truth, not a hand-rolled duplicate field list. That is the whole point of the audit: a hand-rolled parity list would drift the same way the missing `cmd` / missing `[defaults]` merge drifted.

    Step 4 — Run `cargo check --all-targets` to confirm the visibility promotion in Step 2 does not break the rest of the workspace (it should not — `pub(crate)` is purely additive). Run `cargo nextest run config::defaults::tests::parity` to confirm the new test passes.

    Step 5 — Commit discipline: single commit. Because Task 5 is a documentation + structural-test task and the parity audit found no code fixes needed beyond the already-in-flight Task 1 work, RED/GREEN splitting would be artificial. Commit message:

    ```
    docs(260414-gbf): lock config plumbing parity invariant in src/config/defaults.rs

    Add a mermaid classDiagram + parity table as module-level doc on
    src/config/defaults.rs covering the five layers that must stay in
    sync for any field on an executor deserialize struct: JobConfig,
    serialize_config_json, compute_config_hash, apply_defaults, and
    DockerJobConfig.

    Promote serialize_config_json to pub(crate) so the regression test
    parity_with_docker_job_config_is_maintained in src/config/defaults.rs
    can call it directly and assert that every non-secret field
    DockerJobConfig reads is present in the JSON output.

    Add apply_defaults_does_not_touch_container_name as a mirror of the
    existing apply_defaults_does_not_touch_cmd test — container_name is
    per-job-only (container names must be unique) and NOT in
    DefaultsConfig, a decision that was implicit in v1.0 and is now
    explicit in both the parity table and a unit test.

    Audit finding: no additional code fixes required beyond Tasks 1 and
    2 — container_name was the only undocumented decision found and is
    resolved here by documentation + test, not by a behavior change.

    Refs issue #20.
    ```
  </action>
  <verify>
    <automated>cargo check --all-targets 2>&amp;1 | tail -20 &amp;&amp; cargo nextest run -p cronduit config::defaults::tests::parity_with_docker_job_config_is_maintained config::defaults::tests::apply_defaults_does_not_touch_container_name 2>&amp;1 | tail -20</automated>
  </verify>
  <done>
    - `src/config/defaults.rs` opens with a module-level doc comment containing a mermaid `classDiagram` block AND a parity table covering every `DockerJobConfig` field with its `JobConfig` / `serialize_config_json` / `compute_config_hash` / `apply_defaults` status.
    - `src/scheduler/sync.rs::serialize_config_json` is `pub(crate)` (up from module-private); no other callers changed; no clippy/compiler warnings introduced.
    - `src/config/defaults.rs::tests::apply_defaults_does_not_touch_container_name` exists and passes — proves `apply_defaults` leaves `container_name` untouched in both `Some` and `None` cases.
    - `src/config/defaults.rs::tests::parity_with_docker_job_config_is_maintained` exists and passes — imports `crate::scheduler::docker::DockerJobConfig`, constructs a fully-populated `JobConfig`, calls `sync::serialize_config_json`, and asserts every non-secret field `DockerJobConfig` reads is present in the JSON output AND that the output is structurally deserializable back into a `DockerJobConfig` AND that no raw `SecretString` value leaked through (T-02-03 regression guard included in the same test).
    - `cargo check --all-targets` clean.
    - Single commit with `docs(260414-gbf)` scope exists on `fix/defaults-merge-issue-20`.
    - No scope creep: no changes to `src/scheduler/docker.rs`, `src/scheduler/run.rs`, or any other executor file (the audit explicitly found no gaps requiring such edits).
  </done>
</task>

</tasks>

<verification>
After all tasks complete, the executor MUST confirm (in order):

1. `cargo fmt --check` — clean.
2. `cargo clippy --all-targets --all-features -- -D warnings` — clean.
3. `cargo nextest run --workspace` — zero failures. Total test count increased by 14 (unit in `src/config/defaults.rs` from Task 1: 13 behavior tests + `apply_defaults_does_not_touch_cmd`) + 2 new Task 5 unit tests in `src/config/defaults.rs` (`apply_defaults_does_not_touch_container_name`, `parity_with_docker_job_config_is_maintained`) + 3 unit tests in `src/config/hash.rs` (`hash_stable_across_defaults_merge`, `hash_differs_on_delete_change`, `hash_differs_on_cmd_change`) + 2 unit tests in `src/scheduler/sync.rs::tests` (`serialize_config_json_includes_delete`, `serialize_config_json_includes_cmd`) + 12 integration tests in `tests/defaults_merge.rs` (10 base merge + `hash_stable_across_defaults_representations` + `cmd_preserved_on_docker_job` + `cmd_in_defaults_is_not_merged`, which is 13 total — the count says 12 because hash_stable is counted once even though it covers five fields) + 1 unit test in `src/config/validate.rs::tests::check_one_of_job_type_error_mentions_defaults` = roughly 34 new tests minimum.
4. `cargo run --bin cronduit -- check examples/cronduit.toml` — exit 0.
5. Manual smoke: write a TOML with `[defaults] image = "alpine:latest"` and a bare `[[jobs]]` block with just name+schedule, run `cronduit check` on it — exit 0.
6. Manual smoke: write a TOML with neither defaults nor job image and run `cronduit check` — exit non-zero and the error message contains `[defaults]`.
7. Git history on `fix/defaults-merge-issue-20` contains at least 6 commits with `(260414-gbf)` scope (Task 1 RED + GREEN = 2, Task 2 = 1, Task 3 = 1, Task 4 = 1, Task 5 = 1 docs-only commit; Task 5 grows to 2 commits ONLY if the parity audit found additional gaps requiring code fixes — per the Task 5 audit finding in `<behavior>`, no such gaps were found, so 1 commit is the expected shape).
8. `git diff main...HEAD --stat` shows changes ONLY in: `src/config/`, `tests/defaults_merge.rs`, `examples/cronduit.toml`, `.planning/milestones/v1.0-REQUIREMENTS.md`, `.planning/milestones/v1.0-phases/01-foundation-security-posture-persistence-base/01-VERIFICATION.md`, `.github/workflows/release.yml`, `Dockerfile`, `Cargo.toml` (only if tempfile was missing), and optionally `src/scheduler/sync.rs` (only for struct literal updates).
9. `git diff main...HEAD src/scheduler/docker.rs` is EMPTY — the executor wiring for `delete` is out of scope (Known Gap).
10. `grep -c 'docker/metadata-action@v5' .github/workflows/release.yml` returns `1`.
11. `grep -c 'steps.meta.outputs.annotations' .github/workflows/release.yml` returns `1` — the manifest-index annotations path exists.
12. `python3 -c 'import yaml; yaml.safe_load(open(".github/workflows/release.yml"))' && echo YAML_OK` prints `YAML_OK`.
13. `grep -c 'pub cmd: Option<Vec<String>>' src/config/mod.rs` returns `1` — the new `JobConfig.cmd` field is present.
14. `grep -c '"cmd"' src/scheduler/sync.rs` returns at least `1` — the serialize path emits `cmd`.
15. `grep -c '"cmd"' src/config/hash.rs` returns at least `1` — the hash path includes `cmd`.
16. `grep -c 'cmd' docs/SPEC.md` returns at least `1` — the spec documents the field.
17. `grep -c 'classDiagram' src/config/defaults.rs` returns `1` — the parity mermaid diagram is present.
18. `grep -c 'pub(crate) fn serialize_config_json' src/scheduler/sync.rs` returns `1` — the visibility promotion from Task 5 is in place so the parity test can reach the function.
19. `cargo nextest run -p cronduit config::defaults::tests::parity_with_docker_job_config_is_maintained` passes.
20. **Cargo.toml version is EXACTLY `1.0.0`** — no bump, no prerelease suffix. The user will re-tag `v1.0.0` from the merged commit, so tag and version must match (user-memory rule).

After executor verification, the USER must:
- Review the PR locally.
- Confirm the UAT smoke tests pass on their machine (remember the user-memory rule: UAT cannot be self-declared by Claude).
- Merge the PR to `main`.
- Re-tag v1.0.0 from the merged commit ONLY after confirming Cargo.toml version string matches `v1.0.0` (user-memory rule: tag and release version must match).
</verification>

<success_criteria>
- Issue #20 reproduction is fixed: a TOML with `[defaults] image = "alpine:latest"` + a docker `[[jobs]]` block that omits `image` passes `cronduit check`, classifies as `docker` in the sync engine, and the DB `config_json` contains `"image":"alpine:latest"`.
- VPN marquee feature is protected: a TOML with `[defaults] network = "container:vpn"` + a docker job that omits `network` produces a merged job whose `network == Some("container:vpn")`, proved by `tests/defaults_merge.rs::defaults_network_container_vpn_preserved`.
- Per-job override precedence is proved for image, network, volumes, timeout, delete (unit tests) and image + network (integration tests).
- `use_defaults = false` disables merging (unit + integration coverage).
- `compute_config_hash` is stable across the two equivalent TOML representations (unit test).
- `random_min_gap` remains a global scheduler knob — `src/cli/run.rs` and `src/scheduler/reload.rs` are NOT touched, and `tests/defaults_merge.rs` never asserts on random_min_gap being merged.
- `examples/cronduit.toml` actively exercises the merge path via the `hello-world` job, AND the same job exercises the new per-job `cmd` override (`cmd = ["echo", "Hello from cronduit defaults!"]`).
- **New `cmd` field exposed end-to-end:** `JobConfig.cmd: Option<Vec<String>>` exists, is populated from TOML, flows through `serialize_config_json` into the DB `config_json` column, and is consumed by the already-existing `DockerJobConfig.cmd` field in `src/scheduler/docker.rs` so bollard passes the operator's override to `ContainerCreateBody.cmd` at container creation. Proven by `tests/defaults_merge.rs::cmd_preserved_on_docker_job` + `serialize_config_json_includes_cmd` + `hash_differs_on_cmd_change`. The marquee value prop ("run recurring Docker jobs reliably with operator-controlled arguments") is now actually deliverable — before this fix, every docker job ran with its image's baked-in `CMD` with no override path.
- `cmd` is NOT defaults-eligible: `docs/SPEC.md` documents it as per-job only, `apply_defaults` never touches it, and the integration test `cmd_in_defaults_is_not_merged` proves a spurious `[defaults].cmd` entry never leaks into jobs.
- **Config plumbing parity invariant locked in code:** `src/config/defaults.rs` opens with a mermaid `classDiagram` + parity table documenting the five layers (`JobConfig` / `serialize_config_json` / `compute_config_hash` / `apply_defaults` / `DockerJobConfig`) that must stay in sync. The unit test `parity_with_docker_job_config_is_maintained` constructs a fully-populated `JobConfig`, runs it through `serialize_config_json`, and asserts every non-secret field `DockerJobConfig` reads is present in the JSON output — AND asserts the JSON is structurally deserializable back into a `DockerJobConfig` — AND asserts no `SecretString` value leaked (T-02-03 regression guard). `apply_defaults_does_not_touch_container_name` locks the decision that `container_name` is per-job-only (container names must be unique, never defaults-eligible). The parity audit performed during planning found exactly ONE undocumented decision (`container_name`) beyond the two gaps already being fixed (`[defaults]` merge, `cmd`); no additional executor-side code fixes were required.
- `v1.0-REQUIREMENTS.md` and `01-VERIFICATION.md` carry honest retroactive notes for CONF-03, CONF-04, CONF-06.
- **Docker image labels fixed for GHCR:** `.github/workflows/release.yml` generates labels AND annotations via `docker/metadata-action@v5`, so the top-level manifest index (not only the per-platform image configs) carries `org.opencontainers.image.source`. This restores the "Connected to repository" sidebar link on the GHCR package page for multi-arch images. GitHub-recognized labels (source, description, licenses) all present on both label and annotation paths.
- **Cargo.toml version remains `1.0.0`** — the user will re-tag `v1.0.0` from the merged fix commit, so the tag/version must match.
- PR is open on branch `fix/defaults-merge-issue-20` targeting `main`; the branch is NOT merged by Claude — the user merges after review.
- Known Gap on `delete = false` executor wiring is recorded in SUMMARY.md as a follow-up, not silently closed.
</success_criteria>

<output>
After all tasks complete, create `.planning/quick/260414-gbf-fix-defaults-merge-bug-issue-20-defaults/260414-gbf-SUMMARY.md` with:

1. **Objective recap** — one sentence: fix issue #20 + add Docker labels + annotations for GHCR, without bumping Cargo.toml.
2. **What changed** — per-file bullet list, grouped under "Defaults merge fix" and "Docker labels fix".
3. **Test coverage added** — count unit tests in `src/config/defaults.rs`, the hash stability test, integration tests in `tests/defaults_merge.rs`, and the validate unit test. State the total.
4. **Requirements re-satisfied** — CONF-03, CONF-04, CONF-06 with one-line retroactive notes.
5. **Docker labels rationale** — one paragraph explaining: GitHub recognizes three labels on GHCR package pages (`source`, `description`, `licenses`); for multi-arch images the value must be on the manifest INDEX as an annotation, not only the platform image configs; `docker/metadata-action@v5` is the canonical source for both labels and annotations; the old hand-rolled `tags:`/`labels:` blocks in `build-push-action` are replaced.
6. **Known Gap / Follow-up** — `delete = false` is not yet honored by `src/scheduler/docker.rs::cleanup_container`. The field flows through `JobConfig` → `config_json` → `DockerJobConfig` (or will when `DockerJobConfig` adds the field in a future issue). Today, `delete = true` matches current behavior (cronduit always force-removes); only `delete = false` is a no-op. File a follow-up issue "Honor `delete = false` to preserve failed containers for inspection (references moby#8441 race)" with a link to DOCKER-06 as the constraint.
   - **Out-of-scope systemic follow-ups** (NOT to be bundled into this PR, file as separate issues): (a) audit all v1.0 requirements goal-backward to catch any other retroactively-satisfied-only rows; (b) add a CI check that `examples/cronduit.toml` minimally exercises every defaults-eligible field and every per-job-only docker field; (c) extend the parity audit to cover `command`/`script` executor argument paths once those grow beyond single-string fields.
7. **Parity audit outcome** — one paragraph: the audit enumerated `DockerJobConfig` (6 fields) and `JobExecConfig` (2 fields) as the only executor-side deserialize structs; found `cmd` and `[defaults]` merge as the two in-flight gaps; found `container_name` as the only additional undocumented decision (resolved in Task 5 by mermaid parity table + `apply_defaults_does_not_touch_container_name` test); found no other gaps requiring code fixes. State that `parity_with_docker_job_config_is_maintained` is the regression guard for the JSON surface going forward.
8. **Cargo.toml version check** — explicitly confirm that `Cargo.toml` was NOT modified and still reads `version = "1.0.0"`. The user will re-tag `v1.0.0` from the merged fix commit.
9. **Next steps** — user reviews PR, merges, re-tags `v1.0.0` from the merged commit; the retag will trigger the release workflow which now emits OCI-compliant labels + annotations via metadata-action.
</output>
