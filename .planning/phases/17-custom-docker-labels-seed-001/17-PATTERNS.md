# Phase 17: Custom Docker Labels (SEED-001) - Pattern Map

**Mapped:** 2026-04-28
**Files analyzed:** 13 (8 source/example/doc to modify + 4 new tests + 1 seed frontmatter edit)
**Analogs found:** 13 / 13 (every modified or new file has an in-tree analog)

The change is purely additive across an existing five-layer config-plumbing invariant. Every new file or insertion has a directly-named template in the codebase — `cmd` (added in a prior phase) is the closest structural analog because it traversed the same five layers. Most "new" code is a copy-and-rename of `cmd`'s shape with a `HashMap<String, String>` payload instead of `Option<Vec<String>>`.

---

## File Classification

| New / Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `src/config/mod.rs` (modify `DefaultsConfig` + `JobConfig`) | config struct (TOML deserialize) | parse-time | `volumes` field on both structs at `mod.rs:79` and `mod.rs:102` | exact (peer field, same Option-of-collection shape) |
| `src/config/defaults.rs::apply_defaults` (extend) | merge function | transform | existing `volumes` merge at `defaults.rs:137-142` plus `use_defaults=false` short-circuit at `defaults.rs:112-114` | exact (same merge guard structure; semantics differ — labels need per-key MERGE not REPLACE) |
| `src/config/validate.rs` (4 new validators + registrations) | load-time validators | request-response (parse → errors) | `check_cmd_only_on_docker_jobs` at `validate.rs:89-101` and `check_network_mode` at `validate.rs:66-80` | exact (literal template per CONTEXT D-01) |
| `src/scheduler/sync.rs::serialize_config_json` (extend) | serializer (config → DB JSON) | transform | existing `cmd` serialization at `sync.rs:79-81` | exact (peer field, identical `if let Some` insert shape) |
| `src/config/hash.rs::compute_config_hash` (extend) | content-hash function | transform | existing `cmd` hash insert at `hash.rs:44-46` | exact (peer field, identical `if let Some` insert shape) |
| `src/scheduler/docker.rs::DockerJobConfig` (extend) | executor-side struct (JSON deserialize) | parse-time | existing `env: HashMap<String, String>` field at `docker.rs:33-34` | exact (same type signature, same `#[serde(default)]` pattern) |
| `src/scheduler/docker.rs` label-build site (extend) | container-create plumbing | transform | existing 3-line block at `docker.rs:157-160` | exact (this IS the merge site; 2 lines added) |
| `src/config/defaults.rs::tests` (new merge tests) | unit test | request-response | `apply_defaults_use_defaults_false_disables_merge` at `defaults.rs:316` | exact (literal template per CONTEXT canonical_refs) |
| `src/config/defaults.rs::tests::parity_*` (new round-trip test) | unit test | round-trip | `parity_with_docker_job_config_is_maintained` at `defaults.rs:488-556` | exact (literal template per RESEARCH §10) |
| `src/config/validate.rs::tests` (new validator unit tests) | unit test | request-response | `check_cmd_only_on_docker_jobs_*` at `validate.rs:291-351` (5 tests) | exact (literal template per RESEARCH §7) |
| `tests/v12_labels_merge.rs` (NEW) | integration test (Docker-daemon, ignored) | request-response | `tests/docker_executor.rs::test_docker_basic_echo` (lines 50-100) | role-match (testcontainers + bollard inspect) |
| `tests/v12_labels_use_defaults_false.rs` (NEW) | integration test (Docker-daemon, ignored) | request-response | `tests/docker_executor.rs` lifecycle test pattern | role-match |
| `tests/v12_labels_validators.rs` (NEW, no daemon) | integration test (parse-only) | request-response | `tests/defaults_merge.rs` (lines 1-80) | exact (write_toml + parse_and_validate harness) |
| `examples/cronduit.toml` (extend) | doc/example | doc | existing `[defaults]` block (lines 30-35) + `hello-world` job (lines 97-100) + `hello-world-container` (lines 131-135) | exact (extending the same file) |
| `README.md § Configuration` (extend) | doc | doc | existing § Default Job Settings (lines 190-203) + § Job Types (lines 205-250) | exact (peer subsection in the same H2) |
| `.planning/seeds/SEED-001-custom-docker-labels.md` (frontmatter edit) | planning doc | doc | none — first realized seed in repo | partial (no analog; planner picks the YAML keys per CONTEXT D-05) |

---

## Pattern Assignments

### `src/config/mod.rs` — `DefaultsConfig` + `JobConfig` field additions (config struct, parse-time)

**Analog:** `volumes` field on both structs.

**Imports already in scope** (`mod.rs:12-16`):
```rust
use secrecy::SecretString;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
```

**Pattern — `DefaultsConfig` field shape** (current state, `mod.rs:75-85`):
```rust
#[derive(Debug, Deserialize)]
pub struct DefaultsConfig {
    pub image: Option<String>,
    pub network: Option<String>,
    pub volumes: Option<Vec<String>>,
    pub delete: Option<bool>,
    #[serde(default, with = "humantime_serde::option")]
    pub timeout: Option<Duration>,
    #[serde(default, with = "humantime_serde::option")]
    pub random_min_gap: Option<Duration>,
}
```

**Pattern — `JobConfig` field shape** (current state, `mod.rs:87-120`):
```rust
#[derive(Debug, Deserialize)]
pub struct JobConfig {
    pub name: String,
    pub schedule: String,
    #[serde(default)]
    pub command: Option<String>,
    // ... fields ...
    pub volumes: Option<Vec<String>>,
    pub network: Option<String>,
    // ... fields ...
    #[serde(default)]
    pub cmd: Option<Vec<String>>,
}
```

**Insertion point per RESEARCH §A.1-2:** add `pub labels: Option<HashMap<String, String>>` immediately after `volumes` on each struct. Use fully-qualified `std::collections::HashMap` (the file imports `BTreeMap` not `HashMap` — adding a top-level import is also fine). The `#[serde(default)]` attribute is required so omitting the field deserializes to `None` rather than failing.

**Sample shape from RESEARCH (verbatim):**
```rust
/// Operator-defined Docker labels attached to spawned containers.
/// Per LBL-01..06 / SEED-001. Merged with cronduit-internal labels
/// at container-create time. `cronduit.*` namespace reserved (LBL-03).
/// Type-gated to docker jobs only (LBL-04). Per-value 4 KB / per-set
/// 32 KB byte-length limits (LBL-06).
#[serde(default)]
pub labels: Option<std::collections::HashMap<String, String>>,
```

---

### `src/config/defaults.rs::apply_defaults` — labels merge (merge function, transform)

**Analog:** `volumes` merge at `defaults.rs:137-142` (peer collection field, same `is_non_docker` gate); `use_defaults=false` short-circuit at `defaults.rs:112-114`.

**Pattern — existing `volumes` merge** (`defaults.rs:137-142`):
```rust
if !is_non_docker
    && job.volumes.is_none()
    && let Some(v) = &defaults.volumes
{
    job.volumes = Some(v.clone());
}
```

**Pattern — `use_defaults=false` short-circuit** (`defaults.rs:112-114`):
```rust
if job.use_defaults == Some(false) {
    return job;
}
```
This already covers LBL-02's "REPLACE" semantic — when `use_defaults=false`, the function returns early so `job.labels` stays as whatever the operator set per-job (or `None`).

**Diverges from `volumes`:** `volumes` is `None`-or-replace per the peer field; labels need per-KEY MERGE on collision with per-job-wins. The shape from RESEARCH §11 (verbatim):
```rust
if !is_non_docker
    && let Some(defaults_labels) = &defaults.labels
{
    let merged: std::collections::HashMap<String, String> = match job.labels.take() {
        Some(per_job) => {
            let mut m = defaults_labels.clone();
            m.extend(per_job);  // per-job-wins on collision
            m
        }
        None => defaults_labels.clone(),
    };
    job.labels = Some(merged);
}
```

**Field-placement:** add the merge after the existing `delete` merge at `defaults.rs:148-153`. Mirror its `is_non_docker` gate and the `// NOTE:` comment block at `defaults.rs:154-156`.

---

### `src/config/validate.rs` — four new validators (load-time validators, request-response)

**Analog (literal template per CONTEXT D-01):** `check_cmd_only_on_docker_jobs` at `validate.rs:89-101` and `check_network_mode` at `validate.rs:66-80` (regex variant).

**Imports already in scope** (`validate.rs:1-8`):
```rust
use super::{Config, ConfigError, JobConfig};
use croner::Cron;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;
```
All required imports for the four new validators (`Lazy`, `Regex`, `HashMap`, `Path`, `ConfigError`, `JobConfig`) are present. No new imports required.

**Pattern — `Lazy<Regex>` declaration** (`validate.rs:10-13`, the literal idiom for D-02):
```rust
static NETWORK_RE: Lazy<Regex> = Lazy::new(|| {
    // bridge | host | none | container:<name> | <named>
    Regex::new(r"^(bridge|host|none|container:[a-zA-Z0-9_.-]+|[a-zA-Z0-9_.-]+)$").unwrap()
});
```

**Pattern — type-gate validator (literal template for LBL-04)** (`validate.rs:89-101`):
```rust
fn check_cmd_only_on_docker_jobs(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    if job.cmd.is_some() && job.image.is_none() {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: `cmd` is only valid on docker jobs (job with `image = \"...\"` set either directly or via `[defaults].image`); command and script jobs cannot set `cmd` because there is no container to receive it. Remove the `cmd` line, or switch the job to a docker job by setting `image`.",
                job.name
            ),
        });
    }
}
```

**Pattern — registration site** (`validate.rs:20-25`):
```rust
for job in &cfg.jobs {
    check_one_of_job_type(job, path, errors);
    check_cmd_only_on_docker_jobs(job, path, errors);
    check_network_mode(job, path, errors);
    check_schedule(job, path, errors);
}
```
Append four new calls to this loop per RESEARCH §5.6:
```rust
check_label_reserved_namespace(job, path, errors);
check_labels_only_on_docker_jobs(job, path, errors);
check_label_size_limits(job, path, errors);
check_label_key_chars(job, path, errors);
```

**Error-emit pattern (D-01 line/col=0):** every existing validator uses `line: 0, col: 0` for post-parse errors. The `ConfigError` struct at `errors.rs:3-9`:
```rust
#[derive(Debug)]
pub struct ConfigError {
    pub file: PathBuf,
    pub line: usize, // 1-indexed
    pub col: usize,  // 1-indexed
    pub message: String,
}
```
The `Display` impl at `errors.rs:11-26` formats `line==0` as `"<path>: error: <msg>"` so this is the documented "post-parse, no span" idiom — not a bug, the intended shape for all four new validators.

**Determinism (RESEARCH Pitfall 2):** when iterating a `HashMap` to build the offending-key list, sort the result before joining — `HashMap` iteration is non-deterministic and unsorted output flakes tests. Pattern:
```rust
let mut keys: Vec<&str> = offending.iter().map(|s| s.as_str()).collect();
keys.sort();
```

---

### `src/scheduler/sync.rs::serialize_config_json` — labels JSON insert (serializer, transform)

**Analog:** `cmd` insert at `sync.rs:79-81` (peer field, identical shape).

**Imports already in scope** (`sync.rs:10-17`): `JobConfig`, `serde_json` are accessible.

**Pattern — peer field insert** (`sync.rs:79-81`):
```rust
if let Some(c) = &job.cmd {
    map.insert("cmd".into(), serde_json::json!(c));
}
```

**Insertion point:** add the labels insert right after the `cmd` block, before the `env_keys` block at `sync.rs:82-86` (env_keys is the secret-allowlist tail — labels go above it):
```rust
if let Some(l) = &job.labels {
    map.insert("labels".into(), serde_json::json!(l));
}
```

**Why this matters (RESEARCH §10 / Pitfall 1):** this is layer 2 of the five-layer parity invariant. Without it, `apply_defaults` correctly merges labels in memory but `serialize_config_json` writes a JSON blob without `labels` to `jobs.config_json`, and `DockerJobConfig::deserialize` later sees `labels = None`. The parity regression test catches this.

---

### `src/config/hash.rs::compute_config_hash` — labels hash insert (content-hash function, transform)

**Analog:** `cmd` insert at `hash.rs:44-46` (peer field, identical shape).

**Imports already in scope** (`hash.rs:1-4`): `JobConfig`, `BTreeMap`, `serde_json::Value`.

**Pattern — peer field insert** (`hash.rs:44-46`):
```rust
if let Some(c) = &job.cmd {
    map.insert("cmd", serde_json::json!(c));
}
```

**Insertion point:** add immediately after the `cmd` insert at `hash.rs:46`, before the `env` exclusion comment at `hash.rs:47`:
```rust
if let Some(l) = &job.labels {
    map.insert("labels", serde_json::json!(l));
}
```

**Why this matters (RESEARCH §10 / Pitfall 1):** layer 3 of the five-layer parity. Without it, an operator editing `[defaults].labels` or per-job `labels` produces an identical hash, `sync_config_to_db` classifies the row as `unchanged`, and the new label set never reaches the DB. The change-detection regression mirrors `hash_differs_on_delete_change` at `hash.rs:246-256` and `hash_differs_on_cmd_change` at `hash.rs:258-287`.

---

### `src/scheduler/docker.rs::DockerJobConfig` — labels field on executor-side struct (executor struct, parse-time)

**Analog:** `env: HashMap<String, String>` at `docker.rs:33-34` (same type signature) and `cmd` at `docker.rs:39-40` (same `#[serde(default)]` pattern).

**Pattern — peer field shape** (`docker.rs:33-40`):
```rust
/// Environment variables to pass to the container.
#[serde(default)]
pub env: HashMap<String, String>,
/// Volume bind mounts (e.g. `["/host:/container:ro"]`).
#[serde(default)]
pub volumes: Option<Vec<String>>,
/// Command to run inside the container (overrides image CMD).
#[serde(default)]
pub cmd: Option<Vec<String>>,
```

**Insertion point:** add after `delete` at `docker.rs:58`:
```rust
/// Operator-defined Docker labels merged into the cronduit-internal
/// label set at container-create time. Reserved-namespace and type-gate
/// validators at config-load mean this is always operator-safe content.
#[serde(default)]
pub labels: Option<HashMap<String, String>>,
```

**Why this matters (RESEARCH §10 / Pitfall 1):** this is layer 5 of the five-layer parity. The `serde::Deserialize` derive on `DockerJobConfig` (line 28) makes this a single-field addition — no manual deserialize logic needed.

---

### `src/scheduler/docker.rs` label-build site — operator labels merge (executor plumbing, transform)

**Analog:** the existing 3-line block at `docker.rs:157-160` is itself the merge site — Phase 17 extends it.

**Pattern — current state** (`docker.rs:157-160`, verbatim):
```rust
// Build labels (T-04-03: only run_id and job_name, never secrets).
let mut labels = HashMap::new();
labels.insert("cronduit.run_id".to_string(), run_id.to_string());
labels.insert("cronduit.job_name".to_string(), job_name.to_string());
```

**Extension shape from RESEARCH §3 (verbatim):**
```rust
// Build labels. Operator-defined labels merge in first; cronduit-internal
// labels are inserted second so they structurally win on any key collision
// (the LBL-03 reserved-namespace validator at config-load already prevents
// any real collision, but this ordering is defense-in-depth + makes the
// invariant readable from this site).
let mut labels: HashMap<String, String> = config
    .labels
    .clone()
    .unwrap_or_default();
labels.insert("cronduit.run_id".to_string(), run_id.to_string());
labels.insert("cronduit.job_name".to_string(), job_name.to_string());
```

**Critical ordering (RESEARCH Pitfall 5):** operator labels go FIRST so cronduit-internal `cronduit.run_id` / `cronduit.job_name` insertions overwrite any colliding key (defense-in-depth — LBL-03 validator already prevents the collision at config-load).

**Downstream `ContainerCreateBody` build** at `docker.rs:174-185` is unchanged — `labels: Some(labels)` already plumbs the merged map into bollard.

---

### `src/config/defaults.rs::tests` — new merge unit tests (unit test, request-response)

**Analog:** `apply_defaults_use_defaults_false_disables_merge` at `defaults.rs:316-325` (literal template per CONTEXT canonical_refs).

**Pattern — fixture helper** (`defaults.rs:168-184`):
```rust
fn empty_job() -> JobConfig {
    JobConfig {
        name: "t".into(),
        schedule: "*/5 * * * *".into(),
        command: None,
        script: None,
        image: None,
        use_defaults: None,
        env: BTreeMap::new(),
        volumes: None,
        network: None,
        container_name: None,
        timeout: None,
        delete: None,
        cmd: None,
    }
}
```
**Update required:** when `JobConfig` gains the `labels` field, this constructor (and the four other in-test JobConfig literals at `defaults.rs:329-343`, `defaults.rs:498-514`, plus equivalents in `validate.rs:218-233`, `hash.rs:65-81` and `hash.rs:117-133`) must add `labels: None` to keep compilation green. This is THE class of fan-out CONTEXT calls out as the five-layer parity risk.

**Pattern — merge test shape** (`defaults.rs:316-325`):
```rust
#[test]
fn apply_defaults_use_defaults_false_disables_merge() {
    let mut job = empty_job();
    job.use_defaults = Some(false);
    let merged = apply_defaults(job, Some(&full_defaults()));
    assert_eq!(merged.image, None);
    assert_eq!(merged.network, None);
    assert_eq!(merged.volumes, None);
    assert_eq!(merged.timeout, None);
    assert_eq!(merged.delete, None);
}
```

**New tests (RESEARCH §11 verbatim):** `apply_defaults_merges_labels_per_job_wins` and `apply_defaults_use_defaults_false_replaces_labels`. Both follow the existing pattern: build job → build defaults with labels → call `apply_defaults` → assert the merged map contents.

---

### `src/config/defaults.rs::tests::parity_*` — labels round-trip parity test (unit test, round-trip)

**Analog:** `parity_with_docker_job_config_is_maintained` at `defaults.rs:488-556` (literal template per RESEARCH §10).

**Pattern — full template** (`defaults.rs:488-556`, the test that already exists):
```rust
#[test]
fn parity_with_docker_job_config_is_maintained() {
    use crate::scheduler::docker::DockerJobConfig;
    use crate::scheduler::sync;

    let mut env = BTreeMap::new();
    env.insert("SECRET_KEY".to_string(), SecretString::from("super-secret"));
    let job = JobConfig {
        name: "parity-test".to_string(),
        // ... every JobConfig field populated ...
        cmd: Some(vec!["echo".to_string(), "parity".to_string()]),
    };

    let json_str = sync::serialize_config_json(&job);
    let v: serde_json::Value = serde_json::from_str(&json_str).expect("valid JSON");
    let obj = v.as_object().expect("top-level object");

    assert!(obj.contains_key("image"), "image missing from config_json");
    assert!(obj.contains_key("volumes"), "volumes missing from config_json");
    // ... assert every key DockerJobConfig reads ...
    assert!(obj.contains_key("cmd"), "cmd missing from config_json");

    let _check: DockerJobConfig = serde_json::from_str(&json_str)
        .expect("serialize_config_json output must be a valid DockerJobConfig");
}
```

**Phase 17 changes to make:**
1. Add `labels: Some(HashMap::from([("a.b".to_string(), "v".to_string())])),` to the `JobConfig` literal.
2. Add `assert!(obj.contains_key("labels"), "labels missing from config_json");`
3. The final `DockerJobConfig::from_str` round-trip will fail loudly if `DockerJobConfig.labels` was forgotten, completing the five-layer guard.

A new test mirrored on this template (e.g. `parity_labels_round_trip_through_docker_job_config`) is the explicit Phase 17 regression. Either extending the existing test or adding a new sibling is correct — RESEARCH §10 prefers a sibling so the test name reads in a `cargo test` filter.

---

### `src/config/validate.rs::tests` — new validator unit tests (unit test, request-response)

**Analog:** `check_cmd_only_on_docker_jobs_*` block at `validate.rs:291-351` (5 tests covering accept/reject paths).

**Pattern — fixture helper** (`validate.rs:218-233`):
```rust
fn stub_job(schedule: &str) -> JobConfig {
    JobConfig {
        name: "test-job".into(),
        schedule: schedule.into(),
        command: Some("echo hi".into()),
        script: None,
        image: None,
        use_defaults: None,
        env: Default::default(),
        volumes: None,
        network: None,
        container_name: None,
        timeout: None,
        delete: None,
        cmd: None,
    }
}
```
Add `labels: None` to this literal. (Same fan-out caveat as `defaults.rs::empty_job()`.)

**Pattern — reject test** (`validate.rs:291-318`):
```rust
#[test]
fn check_cmd_only_on_docker_jobs_rejects_on_command_job() {
    let mut job = stub_job("*/5 * * * *");
    job.cmd = Some(vec!["echo".to_string(), "hi".to_string()]);
    let mut e = Vec::new();
    check_cmd_only_on_docker_jobs(&job, Path::new("x"), &mut e);
    assert_eq!(e.len(), 1);
    assert!(e[0].message.contains("test-job"), "...");
    assert!(e[0].message.contains("cmd"), "...");
    assert!(e[0].message.contains("docker jobs"), "...");
}
```

**Pattern — accept test** (`validate.rs:333-342`):
```rust
#[test]
fn check_cmd_only_on_docker_jobs_accepts_docker_job_with_cmd() {
    let mut job = stub_job("*/5 * * * *");
    job.command = None;
    job.image = Some("alpine:latest".to_string());
    job.cmd = Some(vec!["echo".to_string(), "hi".to_string()]);
    let mut e = Vec::new();
    check_cmd_only_on_docker_jobs(&job, Path::new("x"), &mut e);
    assert!(e.is_empty(), "docker job with cmd must pass: got {e:?}");
}
```

**New tests per RESEARCH §7:** ~14 unit tests covering accept/reject paths for each of the four validators (reserved-namespace, type-gate, size-limits, key-chars). All follow the `let mut e = Vec::new(); validator(&job, Path::new("x"), &mut e); assert_*` shape verbatim.

---

### `tests/v12_labels_validators.rs` — full-config parse-and-validate scenarios (integration test, request-response)

**Analog:** `tests/defaults_merge.rs` (lines 1-80) — the literal template for a parse-and-validate-from-temp-TOML test harness.

**Pattern — harness** (`tests/defaults_merge.rs:9-27`):
```rust
use cronduit::config::ConfigError;
use cronduit::config::hash::compute_config_hash;
use cronduit::config::parse_and_validate;
use std::io::Write;
use std::time::Duration;
use tempfile::NamedTempFile;

const SERVER_HEADER: &str = r#"
[server]
bind = "127.0.0.1:8080"
timezone = "UTC"
"#;

fn write_toml(contents: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().expect("tempfile");
    f.write_all(contents.as_bytes()).expect("write");
    f.flush().expect("flush");
    f
}
```

**Pattern — pass-case test** (`tests/defaults_merge.rs:29-56`):
```rust
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
    // ...
}
```

**Reject-case shape** (planner extends per RESEARCH §7 for each validator):
```rust
let parsed = parse_and_validate(f.path());
let errs = parsed.expect_err("must reject");
assert!(errs.iter().any(|e: &ConfigError| e.message.contains("cronduit.foo")));
```

**Naming convention (RESEARCH §7):** `tests/v12_<feature>_<scenario>.rs` — verified against `tests/v12_fctx_*.rs`, `tests/v12_run_rs_277_bug_fix.rs`, `tests/v12_webhook_*.rs`. No daemon required for this file (pure parse-and-validate).

---

### `tests/v12_labels_merge.rs` + `tests/v12_labels_use_defaults_false.rs` — Docker-daemon integration tests (integration test, request-response)

**Analog:** `tests/docker_executor.rs` (lines 1-100) — the literal template for testcontainers + bollard `inspect_container`.

**Pattern — `#[ignore]` gating + serial execution comment** (`tests/docker_executor.rs:1-9`):
```rust
//! Docker executor integration tests.
//!
//! These tests require a running Docker daemon and are gated with `#[ignore]`.
//! Run with: `cargo test --test docker_executor -- --ignored --nocapture --test-threads=1`
//!
//! IMPORTANT: These tests MUST run serially (`--test-threads=1`). Parallel execution
//! causes Docker resource contention on some runtimes (e.g. Rancher Desktop) where
//! `wait_container` falls back to inspect polling, and concurrent container operations
//! interfere with each other.
```

**Pattern — Docker client + DB setup** (`tests/docker_executor.rs:34-45`):
```rust
async fn docker_client() -> Docker {
    Docker::connect_with_local_defaults()
        .expect("Docker daemon must be running for integration tests")
}

async fn setup_test_db() -> DbPool {
    let pool = DbPool::connect("sqlite::memory:").await.unwrap();
    pool.migrate().await.unwrap();
    pool
}
```

**Pattern — execute_docker call shape** (`tests/docker_executor.rs:51-95`):
```rust
#[tokio::test]
#[ignore]
async fn test_docker_basic_echo() {
    let docker = docker_client().await;
    let (sender, receiver) = log_pipeline::channel(256);
    let cancel = CancellationToken::new();
    let control = RunControl::new(cancel.clone());
    // ... collector spawn ...
    let config_json = r#"{"image": "alpine:latest", "cmd": ["echo", "hello-cronduit"]}"#;
    let result = execute_docker(&docker, config_json, "test-echo", 1,
        Duration::from_secs(30), cancel, sender, &control).await;
    assert_eq!(result.exec.status, RunStatus::Success, ...);
}
```

**Pattern — label assertion via inspect_container** (RESEARCH §6 verbatim):
```rust
let info = docker.inspect_container(&id, None).await?;
let labels = info.config.and_then(|c| c.labels).unwrap_or_default();
assert_eq!(labels.get("cronduit.run_id").map(String::as_str), Some("42"));
```

For Phase 17 the `config_json` will need to embed a `labels` map (e.g. `"labels": {"watchtower.enable": "false"}`), then assert the inspect result contains both operator labels AND the cronduit-internal labels. The end-to-end "operator label visible alongside cronduit.run_id" assertion is the load-bearing test for ROADMAP success criterion (1).

---

### `examples/cronduit.toml` — three integration patterns (doc, doc)

**Analog:** the file itself — Phase 17 extends three existing blocks plus adds one new job.

**Pattern — current `[defaults]` block** (`examples/cronduit.toml:30-35`):
```toml
[defaults]
image = "alpine:latest"
network = "bridge"
delete = true
timeout = "5m"
random_min_gap = "0s"
```
Extension per CONTEXT D-03: add `labels = { "com.centurylinklabs.watchtower.enable" = "false" }` (Watchtower exclusion — single-line inline-table — quoted dotted key per RESEARCH Pitfall 3).

**Pattern — current `hello-world` job** (`examples/cronduit.toml:97-100`):
```toml
[[jobs]]
name = "hello-world"
schedule = "*/5 * * * *"
cmd = ["echo", "Hello from cronduit defaults!"]
```
Extension per CONTEXT D-03: append a Traefik-style `labels = { "traefik.http.routers.hello.rule" = "Host(\`hello.local\`)" }` line — demonstrates per-job MERGE (job ends up with both inherited Watchtower AND its own Traefik label). Note: TOML literal-string requires backticks be escape-quoted or wrapped in single-quoted literal strings.

**Pattern — new `use_defaults = false` job** (CONTEXT D-03 — planner picks name, e.g. `isolated-batch`). Use the existing `hello-world-container` block at lines 131-135 as the structural template:
```toml
[[jobs]]
name = "hello-world-container"
schedule = "*/5 * * * *"
image = "hello-world:latest"
delete = false
```
Add `use_defaults = false` and `labels = { "backup.exclude" = "true" }` to demonstrate REPLACE semantic.

**Pattern — inline comment block style** (`examples/cronduit.toml:84-96`): every existing job has a 5-15 line comment block above it explaining what the block demonstrates. Phase 17 adds parallel comment blocks cross-referencing the README labels subsection per CONTEXT D-03.

---

### `README.md § Configuration` — labels subsection (doc, doc)

**Analog:** existing `### Default Job Settings` (lines 190-203) and `### Job Types` (lines 205-250) — peer subsections in the same H2.

**Pattern — peer subsection with TOML fence + inline annotations** (`README.md:190-203`):
```markdown
### Default Job Settings

\`\`\`toml
[defaults]
image = "alpine:latest"     # Default Docker image for container jobs
network = "bridge"          # Default Docker network mode
delete = true               # When true, cronduit removes the container after wait_container drains.
                            # NOT bollard auto_remove -- ...
timeout = "5m"              # Default job timeout
random_min_gap = "90m"      # Minimum gap between @random-scheduled jobs on the same day.
\`\`\`
```

**Insertion point:** add `### Custom Docker Labels` as a new H3 between `### Default Job Settings` (ends at line 203) and `### Job Types` (starts at line 205), OR append after `### Job Types` ends at line 250. Planner picks; the H3-between-peers placement matches the visual flow of the existing section.

**Required content per CONTEXT D-04 (~30-40 lines):**
1. Short prose intro.
2. **Mermaid merge-precedence diagram** (CONTEXT D-07, project-rule-mandated; example flow: `[defaults].labels → per-job [[jobs]].labels (merge or replace per use_defaults) → cronduit-internal labels [overrides]`). The README has zero existing mermaid diagrams — Phase 17 adds the first; planner uses standard ` ```mermaid ... ``` ` fenced block.
3. Merge-semantics table (3 rows: `use_defaults` unset/true/false × per-job labels set/unset).
4. Reserved-namespace + type-gate + size-limit rule subsections with single-line examples.
5. Env-var interpolation note ("values yes, keys no") per RESEARCH §6.

**Tone analog** (`README.md:248`): existing TOML sections lead with the example, then explain consequences in 1-2 sentences with cross-references to deeper docs (`docs/SPEC.md`, `docs/CONFIG.md`). Match this density.

---

### `.planning/seeds/SEED-001-custom-docker-labels.md` — frontmatter promotion (planning doc, doc)

**Analog:** none in repo (first realized seed per CONTEXT D-05).

**Pattern — current frontmatter** (`SEED-001-custom-docker-labels.md:1-8`):
```yaml
---
id: SEED-001
status: dormant
planted: 2026-04-24
planted_during: between v1.1 and v1.2 — milestone close-out for v1.1 just completed
trigger_when: v1.2 milestone kickoff during the requirements pass ...
scope: Small
---
```

**Edit per CONTEXT D-05 (in the LAST plan of Phase 17):**
- `status: dormant` → `status: realized`
- Add `realized_in: phase-17`
- Add `milestone: v1.2`
- Add `realized_date: <ISO date of merge or close-out commit>`

**No physical file move.** File stays at `.planning/seeds/SEED-001-custom-docker-labels.md`.

---

## Shared Patterns

### S-1: `Lazy<Regex>` for static patterns

**Source:** `src/config/validate.rs:10-13` (NETWORK_RE), `src/config/interpolate.rs:23-24` (VAR_RE, DEFAULT_RE).

**Apply to:** the D-02 key-char validator regex (`LABEL_KEY_RE`).

```rust
static LABEL_KEY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-zA-Z0-9_][a-zA-Z0-9._-]*$").unwrap()
});
```

Existing imports at `validate.rs:3-5` (`once_cell::sync::Lazy`, `regex::Regex`) cover this — no new imports.

---

### S-2: Per-job validator emit pattern (D-01 line/col=0)

**Source:** every existing function in `src/config/validate.rs` — `check_one_of_job_type` (50-64), `check_cmd_only_on_docker_jobs` (89-101), `check_network_mode` (66-80), `check_schedule` (103-138).

**Apply to:** all four new label validators.

```rust
errors.push(ConfigError {
    file: path.into(),
    line: 0,
    col: 0,
    message: format!("[[jobs]] `{}`: <reason>: <offending detail>. <remediation>.", job.name),
});
```

`Display` impl at `errors.rs:11-26` formats `line==0` as the documented "post-parse" shape — no changes needed downstream.

---

### S-3: `HashMap` iteration determinism (RESEARCH Pitfall 2)

**Source:** RESEARCH §5.1, §5.3, §5.4 sample code (sort-before-format).

**Apply to:** every validator that emits a list of offending keys (LBL-03, LBL-06 per-value, D-02). `HashMap` iteration order is non-deterministic; tests asserting "error contains `cronduit.foo, cronduit.bar`" would flake without:

```rust
let mut keys: Vec<&str> = offending.iter().map(|s| s.as_str()).collect();
keys.sort();
let _ = keys.join(", ");
```

---

### S-4: Five-layer parity invariant — atomic-commit-or-strict-order

**Source:** `src/config/defaults.rs:1-87` module docstring (the project's load-bearing reminder).

**Apply to:** every plan/commit that touches the `labels` field. Either ship all five layers (`JobConfig` → `serialize_config_json` → `compute_config_hash` → `apply_defaults` → `DockerJobConfig`) in one atomic commit, OR strictly order the commits with passing tests as gates.

**Regression guard:** `parity_with_docker_job_config_is_maintained` at `defaults.rs:488-556` is the existing structural test. Phase 17 either extends it (add a `labels` key assertion + populate `labels` on the `JobConfig` literal) or adds a sibling `parity_labels_round_trip_through_docker_job_config`. Either is correct — extending is fewer LOC; siblings filter cleaner in `cargo test`.

---

### S-5: `JobConfig` test-fixture fan-out (compile-time guard)

**Source:** every `JobConfig { ... }` literal in the repo — at least 7 sites:
- `src/config/defaults.rs:168-184` (`empty_job`)
- `src/config/defaults.rs:329-343` (`apply_defaults_none_returns_job_unchanged`)
- `src/config/defaults.rs:498-514` (`parity_with_docker_job_config_is_maintained`)
- `src/config/validate.rs:218-233` (`stub_job`)
- `src/config/hash.rs:65-81` (`mk_job`)
- `src/config/hash.rs:117-133` (inner `mk_docker_job` helper)
- Any test fixture in `tests/*.rs` that constructs `JobConfig` directly (most use `parse_and_validate` so this is rare)

**Apply to:** every plan that adds the `labels` field to `JobConfig`. The compiler will refuse to build any literal that omits the new field — that's the safety net. Walk every site listed above and add `labels: None,` to keep `cargo build` green. This is the boring half of S-4; missing one site fails CI immediately, not silently.

---

### S-6: Integration test naming + `#[ignore]` gating

**Source:** `tests/docker_executor.rs:1-9` (header comment), `tests/v12_fctx_*.rs` (filename pattern).

**Apply to:** all three new tests in `tests/v12_labels_*.rs`.

- File naming: `tests/v12_<feature>_<scenario>.rs` (verified via `ls tests/`).
- Daemon-required tests: `#[ignore]` + module-doc explaining `cargo test --test ... -- --ignored --nocapture --test-threads=1`.
- Daemon-independent tests (the validators file): no `#[ignore]`, runs in standard `cargo test` suite.

---

### S-7: Mermaid-only diagrams (project rule D-07)

**Source:** `src/config/defaults.rs:24-59` (the existing five-layer parity diagram) — the only mermaid diagram currently in the repo.

**Apply to:** the README labels subsection (CONTEXT D-04). The diagram is the load-bearing visual proof of the merge-precedence chain.

```markdown
\`\`\`mermaid
flowchart LR
  A[\"[defaults].labels\"] --> B[\"per-job [[jobs]].labels\"]
  B -->|use_defaults=false| C[\"per-job replaces defaults\"]
  B -->|otherwise| D[\"merge with per-job-wins\"]
  C --> E[\"+ cronduit-internal labels (always)\"]
  D --> E
\`\`\`
```

Planner picks the diagram shape; CONTEXT D-04 mandates "all four steps and the 'internal labels override' arrow direction."

---

## No Analog Found

| File | Role | Data Flow | Reason | Mitigation |
|---|---|---|---|---|
| `.planning/seeds/SEED-001-...md` frontmatter promotion | planning ceremony | doc | First realized seed in repo — no prior pattern for `status: dormant → realized` lifecycle. | Planner uses CONTEXT D-05 verbatim YAML keys. This phase establishes the pattern for all future realized seeds. |
| README mermaid merge-precedence diagram | doc visual | doc | First mermaid diagram in `README.md` (one exists in `src/config/defaults.rs:24-59` doc-comment). | Planner uses standard ` ```mermaid ... ``` ` fenced block; project rule D-07 mandates mermaid (no ASCII). |

Both gaps are doc-only; there is zero source-code-level "no analog" risk for Phase 17.

---

## Metadata

**Analog search scope:**
- `src/config/{mod,defaults,validate,interpolate,hash,errors}.rs` (all read)
- `src/scheduler/{docker,sync}.rs` (relevant ranges read)
- `tests/{docker_executor,defaults_merge,v12_fctx_config_hash_backfill}.rs` (templates verified)
- `tests/` directory listing (naming convention)
- `examples/cronduit.toml` (full file)
- `README.md` (sections 170-330)
- `.planning/seeds/SEED-001-custom-docker-labels.md` (frontmatter)

**Files scanned:** 13 source/test/doc files; 4 directory listings.

**Pattern extraction date:** 2026-04-28

**Confidence:** HIGH. Every modification has a directly-named, recently-touched analog in the same file or in a peer file. The `cmd` field added in a prior phase traversed the identical five-layer surface and is the literal template for `labels`. The four validators copy `check_cmd_only_on_docker_jobs` shape with a payload swap. No file in this phase has a "best guess" analog — all 13 are exact or near-exact role+data-flow matches.

## PATTERN MAPPING COMPLETE
