# Phase 17: Custom Docker Labels (SEED-001) - Research

**Researched:** 2026-04-28
**Domain:** Rust config schema + bollard label-passthrough + load-time validators
**Confidence:** HIGH

---

## Executive Summary

Phase 17 is a near-pure additive change: introduce `labels: Option<HashMap<String, String>>` on `DefaultsConfig` and `JobConfig`, extend `apply_defaults` with merge semantics (per-job-wins on collision; `use_defaults = false` replaces), add four load-time validators (reserved-namespace, type-gate, size limits, key-char regex), merge operator labels into the existing internal labels HashMap at `src/scheduler/docker.rs:158`, and update `examples/cronduit.toml` + the README. **No new crate. No DB schema change. No bollard API surface change.** [VERIFIED: docs.rs/bollard/0.20.2 — `ContainerCreateBody.labels: Option<HashMap<String, String>>`]. The phase's load-bearing code-review risk is the **five-layer config plumbing parity invariant** documented at `src/config/defaults.rs:1-87` — `JobConfig`, `serialize_config_json`, `compute_config_hash`, `apply_defaults`, and `DockerJobConfig` MUST all gain the `labels` field in the same atomic-or-sequential commit set, or `apply_defaults` correctly merges labels in memory but the serialize/deserialize round-trip through `jobs.config_json` silently drops them and the executor sees no labels. **Primary recommendation:** plan the schema-additions plan as a single atomic commit covering all five layers (or order them strictly: JobConfig → DockerJobConfig+serialize → hash → apply_defaults → tests), and write a parity regression test that mirrors `parity_with_docker_job_config_is_maintained` at `src/config/defaults.rs:488` for the `labels` key.

---

## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01 — Validator error format.** Match the existing `validate.rs` pattern. All four new load-time validators emit `ConfigError { line: 0, col: 0, ... }` and produce **one `ConfigError` per job per violation type**. The error message enumerates ALL offending keys for that violation in a single line. No `toml::Spanned` plumbing — `check_cmd_only_on_docker_jobs` (`src/config/validate.rs:89`) is the literal template. Aggregate-not-fail-fast posture is already in place via the per-job validator loop at `src/config/validate.rs:88-92`.

  Example error shape (reserved-namespace):
  ```
  [[jobs]] `nightly-backup`: labels under reserved namespace `cronduit.*` are
  not allowed: cronduit.foo, cronduit.bar. Remove these keys; the cronduit.*
  prefix is reserved for cronduit-internal labels.
  ```

- **D-02 — Strict ASCII key validation at LOAD time.** Keys must match `^[a-zA-Z0-9_][a-zA-Z0-9._-]*$`. Empty keys, non-ASCII, spaces, slashes, and characters outside this set are rejected at config-load. Fourth new validator orthogonal to LBL-03/LBL-04/LBL-06.

- **D-03 — `examples/cronduit.toml` shows three integration patterns:** Watchtower exclusion in `[defaults]`; Traefik annotation per-job-merge on existing `hello-world`; backup-tool filter on a NEW `use_defaults = false` job (planner picks name, e.g. `isolated-batch`). `hello-world-container` left unchanged.

- **D-04 — README § Configuration full labels subsection (~30–40 lines).** Mermaid merge-precedence diagram (defaults → per-job → cronduit-internal-overrides), merge-semantics table, reserved-namespace rule, type-gate rule, size limits, env-var interpolation note (values yes, keys no).

- **D-05 — Seed lifecycle ceremony.** In the LAST plan of Phase 17, edit `.planning/seeds/SEED-001-custom-docker-labels.md` frontmatter: `status: dormant` → `status: realized`; add `realized_in: phase-17`, `milestone: v1.2`, `realized_date: <ISO>`. File stays at original path. Establishes the project's first realized-seed pattern.

- **D-06..D-10 — Project-rule reaffirmations.** PR-only on feature branch; mermaid diagrams only; UAT items reference `just` recipes; UAT validated by maintainer; tag and `Cargo.toml` version match (no version field changes in this phase — Plan 15-01 already bumped to `1.2.0`).

### Claude's Discretion

- **Plan count and grouping.** Suggested split: (1) schema + merge in `mod.rs` + `defaults.rs` + 4 parity layers; (2) four new validators in `validate.rs`; (3) bollard plumb-through in `docker.rs`; (4) examples + README; (5) integration tests; (6) seed close-out. Planner may collapse or expand. Atomic-commit-per-plan per project convention.
- **Validator function names.** Suggested: `check_label_reserved_namespace`, `check_labels_only_on_docker_jobs`, `check_label_size_limits`, `check_label_key_chars`. Planner may rename.
- **Whether the four checks live in one function or four.** Four parallels existing per-validator-per-concern shape; one combined walks each `(key, value)` once. Either acceptable.
- **`once_cell::sync::Lazy<Regex>` vs hand-rolled char-by-char match for D-02.** `once_cell` already a dep. Either acceptable.
- **Whether the labels merge lives inline in `apply_defaults` or in a helper `apply_label_defaults(...)`.**
- **`testcontainers` integration test naming.** Convention: `tests/v12_<feature>_<scenario>.rs`. Suggested: `v12_labels_merge.rs`, `v12_labels_use_defaults_false.rs`, `v12_labels_validators.rs`.
- **Whether `17-HUMAN-UAT.md` is produced.** Phase is largely CI-observable. Maintainer-facing UAT worthwhile for README-renders + examples-loads scenarios.
- **Whether to add a fail-on-empty-string-value check.** Default recommendation: skip — bollard accepts empty-string label values, rejecting would surprise operators.

### Deferred Ideas (OUT OF SCOPE)

- Display operator labels in the Web UI run-detail / job-detail page (v1.3 candidate).
- Substring-after-interpolation key gap (the `${VAR}` resolves to `traefik.enable` case — D-02 char regex catches the leftover-`${` case but not the fully-resolved-to-safe-chars case). Documented in README as "keys are NOT interpolated"; v1.3+ tightening if it becomes a UX problem.
- Generalizing the labels validator stack to non-docker label-equivalents (systemd unit annotations, log tags). SEED-001 explicitly defers.
- Label-based metric labels (Prometheus `cronduit_*` family) — unbounded cardinality, explicitly rejected.
- Label-based webhook routing keys (WH-09 includes labels in the payload but never AS a routing key).
- `cronduit.*` namespace expansion to `cronduit.image_digest` etc. — Phase 21 consideration.
- Empty-string label values rejection.
- Physical move of realized seed files to `.planning/seeds/realized/`.

---

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| LBL-01 | `labels: Option<HashMap<String, String>>` field on `DefaultsConfig` and `JobConfig`; TOML keys may contain dots; operator labels merged with cronduit-internal labels at container-create time, populating `bollard::Config::labels`. | File-by-file approach §1, §3, §4; bollard 0.20 API verified — `ContainerCreateBody.labels: Option<HashMap<String, String>>` is exactly the existing surface at `src/scheduler/docker.rs:182`. [VERIFIED: docs.rs/bollard/0.20.2] |
| LBL-02 | Merge semantics — `use_defaults = false` REPLACES; `use_defaults = true`/unset → defaults map ∪ per-job map with per-job wins on collision. | `apply_defaults` extension §4.2; mirrors existing `use_defaults = false` short-circuit at `src/config/defaults.rs:112-114`; merge implementation pattern §4.3. |
| LBL-03 | Reserved-namespace validator — operator labels under `cronduit.*` MUST fail config validation at LOAD time. | Validator §5.1 (function template `check_cmd_only_on_docker_jobs` at `src/config/validate.rs:89`). |
| LBL-04 | Type-gated validator — setting `labels` on a `type = "command"` or `type = "script"` job is a config-validation error. | Validator §5.2 (mirrors `check_cmd_only_on_docker_jobs` discriminant — uses post-`apply_defaults` `image.is_none()`). |
| LBL-05 | `${ENV_VAR}` interpolation works in label VALUES (free from existing pre-parse pass). Keys NOT interpolated. | Pre-parse interpolation §6 (existing `src/config/interpolate.rs:22` regex pass operates on raw TOML before `toml::from_str`; values pass through transparently; D-02 char regex enforces "no leftover `${...}`" in keys). Residual gap acknowledged in Edge Cases §8.5. |
| LBL-06 | Per-value 4 KB and total per-job 32 KB byte-length limits enforced at config-load. | Validator §5.3; size formula §5.3.2 (sum keys + values; delimiters not counted). |

---

## Project Constraints (from CLAUDE.md)

Mandatory directives that constrain the plan:

- **Tech stack locked.** `bollard` 0.20.2 for Docker (no shelling out); `sqlx` 0.8.6; `askama` 0.15.6 + `askama_web` 0.15.2 (with `axum-0.8` feature, NOT `askama_axum`); `croner` 3.0.1; `axum` 0.8.8 + `tower-http` 0.6.8.
- **No new crate in Phase 17.** All required deps are present: `serde` 1.0.228, `toml` 1.1.2 (TOML spec 1.1.0), `regex` 1, `once_cell` 1, `std::collections::HashMap`. [VERIFIED: `Cargo.toml`]
- **Config format: TOML.** YAML rejected at requirements time.
- **Documentation: all diagrams MUST be mermaid.** D-04's README diagram is the load-bearing instance. NO ASCII art diagrams anywhere (planning docs, README, PR descriptions, code comments).
- **Workflow: PR on feature branch.** No direct commits to main.
- **Tag = `Cargo.toml` version.** Version stays at `1.2.0` for this phase.
- **Quality bar: tests + CI from phase 1.** Clippy + fmt + cargo-deny gate. CI matrix `linux/{amd64,arm64} × {SQLite, Postgres}`. README sufficient for a stranger to self-host.
- **UAT items must reference an existing `just` recipe** — never ad-hoc `cargo`/`docker`/`curl` invocations.
- **UAT validated by maintainer** — never marked passed from Claude's own runs.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| TOML schema & deserialization | Config (`src/config/mod.rs`) | — | Source of truth for the operator-facing surface |
| Defaults merge (`use_defaults = false` + per-key merge) | Config (`src/config/defaults.rs`) | — | Centralized merge invariant; per the five-layer parity rule, must move in lock-step with the four other layers |
| Load-time validation (4 new checks) | Config (`src/config/validate.rs`) | — | Aggregate-not-fail-fast `Vec<ConfigError>` accumulator; `parse_and_validate` chokepoint |
| Env-var `${VAR}` interpolation in values | Config (`src/config/interpolate.rs`) | — | Pre-parse regex on raw TOML; no labels-specific code |
| Config persistence (`config_json` column) | Scheduler/sync (`src/scheduler/sync.rs::serialize_config_json`) | — | Layer 2 of five-layer parity; labels must serialize so the executor reads them back |
| Config change detection | Config (`src/config/hash.rs::compute_config_hash`) | — | Layer 3 of five-layer parity; otherwise an operator's label edit doesn't trigger an `updated` upsert |
| Container-create label merge | Scheduler/docker (`src/scheduler/docker.rs:158-160`) | — | Single chokepoint; internal labels remain authoritative |
| Orphan reconciliation `cronduit.run_id` consumer | Scheduler/docker_orphan (`src/scheduler/docker_orphan.rs:31`) | — | Load-bearing reason for the LBL-03 reserved-namespace validator |
| Documentation / examples | Docs (`README.md`, `examples/cronduit.toml`) | — | Operator-facing surface for the three integration patterns |
| Seed lifecycle bookkeeping | Planning (`.planning/seeds/SEED-001-...md`) | — | Establishes first realized-seed pattern |

---

## File-by-File Approach

The change is small but has a load-bearing parity invariant. Suggested order — atomic-commit-per-plan:

### Plan A: Schema + five-layer parity (recommended single atomic plan)

1. **`src/config/mod.rs:75-85` — `DefaultsConfig`.** Add `pub labels: Option<std::collections::HashMap<String, String>>` field. Field placement: after `volumes` (closest semantic peer).
2. **`src/config/mod.rs:88-120` — `JobConfig`.** Add the same field. Field placement: after `volumes` for symmetry.
3. **`src/scheduler/docker.rs` — `DockerJobConfig` (L29-59).** Add `pub labels: Option<HashMap<String, String>>` so the executor can deserialize the field from `jobs.config_json`. **This is the silent-loss class of bug** documented at `src/config/defaults.rs:1-87` — without this layer the labels merge correctly in `apply_defaults` but disappear on the way through `serialize_config_json` → DB → `from_str::<DockerJobConfig>` round-trip.
4. **`src/scheduler/sync.rs::serialize_config_json` (L51-88).** Add an `if let Some(l) = &job.labels { map.insert("labels".into(), serde_json::json!(l)); }` block. Mirrors the existing `cmd`/`volumes`/`network` pattern.
5. **`src/config/hash.rs::compute_config_hash` (L16-58).** Add the same `if let Some(l) = &job.labels` insertion into the `BTreeMap`. Without this, an operator editing labels does not trigger an `updated` upsert and the new labels never reach the DB.
6. **`src/config/defaults.rs::apply_defaults` (L108-159).** Implement the merge per LBL-02 — see §4 below for the exact shape.
7. **Parity regression test** mirroring `parity_with_docker_job_config_is_maintained` at `src/config/defaults.rs:488`. Construct a JobConfig with `labels = Some(...)`, serialize via `sync::serialize_config_json`, assert the JSON contains `"labels"`, and round-trip through `serde_json::from_str::<DockerJobConfig>`.

### Plan B: Validators

8. **`src/config/validate.rs:88-92` — per-job loop registration.** Add four new validator calls. (Or one combined call if planner picks the consolidated function shape.)
9. **`src/config/validate.rs` — four new validator functions** (or one combined). See §5 below.
10. Unit tests in the same file's `mod tests`, mirroring the `check_cmd_only_on_docker_jobs_*` pattern at `src/config/validate.rs:291-351`.

### Plan C: Bollard plumb-through

11. **`src/scheduler/docker.rs:157-160` — labels HashMap construction.** Extend with operator-defined labels per §3 below. **CRITICAL ordering:** insert operator labels FIRST, then cronduit-internal labels (`run_id`, `job_name`) — so that even if the LBL-03 validator were ever bypassed, internal labels structurally win. (Validator means there can never be a real conflict, but defense-in-depth is free here.)

### Plan D: Examples + README

12. **`examples/cronduit.toml`** — three integration patterns per D-03. Add a `[defaults]` `labels = { "com.centurylinklabs.watchtower.enable" = "false" }` line. Add a Traefik label to existing `hello-world` job (currently L97-100 — three lines, no docker-only fields). Add a NEW `[[jobs]]` block (suggested name `isolated-batch`, planner picks) with `use_defaults = false`, `image = "..."`, `schedule = "..."`, and `labels = { "backup.exclude" = "true" }`. `hello-world-container` (L131-135) untouched.
13. **`README.md` § Configuration** — add full labels subsection per D-04, including the mermaid merge-precedence diagram.

### Plan E: Integration tests (testcontainers)

14. `tests/v12_labels_merge.rs` (T-V12-LBL-01, T-V12-LBL-02, T-V12-LBL-08).
15. `tests/v12_labels_use_defaults_false.rs` (T-V12-LBL-02 replace path).
16. `tests/v12_labels_validators.rs` (T-V12-LBL-03..07, T-V12-LBL-09, T-V12-LBL-10) — config-load rejection paths; pure parse-and-validate, no Docker daemon needed.

### Plan F: Seed close-out (LAST plan)

17. **`.planning/seeds/SEED-001-custom-docker-labels.md`** frontmatter promotion per D-05.

---

## TOML + bollard Integration Specifics

### 1. TOML deserialization of label maps with dotted keys

The `toml = "1.1.2"` crate (TOML spec 1.1.0) handles dotted keys in `HashMap<String, String>` deserialization via inline-table syntax verbatim. [VERIFIED: docs.rs/toml/1.1.2 + manual confirmation against the existing `test_docker_basic_echo` test at `tests/docker_executor.rs` which uses `cmd: ["echo", "hello-cronduit"]` inline arrays without quoting issues.]

**Operator-facing TOML syntax options:**

```toml
# Inline-table (recommended for short label sets)
[[jobs]]
name = "hello-world"
schedule = "*/5 * * * *"
image = "alpine:latest"
labels = { "com.centurylinklabs.watchtower.enable" = "false", "traefik.http.routers.hello.rule" = "Host(`hello.local`)" }

# OR block-table (recommended for long label sets — more readable)
[[jobs]]
name = "hello-world"
schedule = "*/5 * * * *"
image = "alpine:latest"

[jobs.labels]
"com.centurylinklabs.watchtower.enable" = "false"
"traefik.http.routers.hello.rule" = "Host(`hello.local`)"
```

**Why `HashMap` not `BTreeMap`:** bollard's `ContainerCreateBody.labels: Option<HashMap<String, String>>` [VERIFIED: docs.rs/bollard/0.20.2]. Using `BTreeMap` would force a clone-and-convert on every container create; `HashMap` matches the existing v1.0 `DockerJobConfig.env: HashMap<String, String>` pattern at `src/scheduler/docker.rs:34`. CONTEXT.md confirms this. [CITED: 17-CONTEXT.md § Established Patterns]

**Watch out for:** dotted keys like `com.centurylinklabs.watchtower.enable` in TOML inline-table syntax MUST be quoted (`"com.centurylinklabs.watchtower.enable" = "false"`). Without quotes, TOML interprets the dots as nested-table accessors and the parse fails with a structural error. Block-table syntax has the same requirement. The README + examples MUST show quoted keys; an inline operator comment would be wise.

### 2. bollard 0.20.2 `ContainerCreateBody.labels` signature

```rust
// Existing at src/scheduler/docker.rs:174-185 (verbatim, current code)
let container_body = ContainerCreateBody {
    image: Some(config.image.clone()),
    cmd: config.cmd.clone(),
    env: if env_vec.is_empty() { None } else { Some(env_vec) },
    labels: Some(labels),                      // ← this line, no signature change
    host_config: Some(host_config),
    ..Default::default()
};
```

[VERIFIED: docs.rs/bollard/0.20.2 — `pub labels: Option<HashMap<String, String>>` documented as "User-defined key/value metadata."]

bollard 0.20.2 (released March 2026) is on hyper 1; no API surface change is required for Phase 17. The merge happens before this struct is built — see §3 below. [CITED: docs.rs/bollard/0.20.2/bollard/models/struct.ContainerCreateBody.html]

### 3. Label merge at container-create time

Existing code at `src/scheduler/docker.rs:157-160`:

```rust
// Build labels (T-04-03: only run_id and job_name, never secrets).
let mut labels = HashMap::new();
labels.insert("cronduit.run_id".to_string(), run_id.to_string());
labels.insert("cronduit.job_name".to_string(), job_name.to_string());
```

Extension after Phase 17 (recommended exact shape):

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

Note `config` here is the `DockerJobConfig` at L97 — which is why `DockerJobConfig` MUST also gain the `labels` field (Plan A, item 3 above). The deserialization is automatic via `serde::Deserialize` derive.

---

## Validator Design

### 5.1 LBL-03 reserved-namespace (`check_label_reserved_namespace`)

```rust
fn check_label_reserved_namespace(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let Some(labels) = &job.labels else { return };
    let offending: Vec<&String> = labels.keys()
        .filter(|k| k.starts_with("cronduit."))
        .collect();
    if !offending.is_empty() {
        let mut keys: Vec<&str> = offending.iter().map(|s| s.as_str()).collect();
        keys.sort();  // deterministic error output
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: labels under reserved namespace `cronduit.*` are not allowed: {}. Remove these keys; the cronduit.* prefix is reserved for cronduit-internal labels.",
                job.name,
                keys.join(", ")
            ),
        });
    }
}
```

**Sorting**: error output is sorted for deterministic test assertions. This is critical — `HashMap` iteration order is non-deterministic, and a test asserting "error contains `cronduit.foo, cronduit.bar`" would flake without sort.

### 5.2 LBL-04 type-gate (`check_labels_only_on_docker_jobs`)

Mirrors `check_cmd_only_on_docker_jobs` at `src/config/validate.rs:89`. Runs AFTER `apply_defaults` so `image.is_none()` reliably distinguishes non-docker from docker (`apply_defaults` skips the docker-only fields for non-docker jobs per `is_non_docker` gate at L123).

```rust
fn check_labels_only_on_docker_jobs(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    if job.labels.is_some() && job.image.is_none() {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: `labels` is only valid on docker jobs (job with `image = \"...\"` set either directly or via `[defaults].image`); command and script jobs cannot set `labels` because there is no container to attach them to. Remove the `labels` block, or switch the job to a docker job by setting `image`.",
                job.name
            ),
        });
    }
}
```

**Edge case:** `labels = {}` (empty inline table) — `Some(empty HashMap)` triggers the validator. Recommendation: planner picks. Either accept as a no-op (treat empty map as `None`) OR reject (operator wrote it deliberately, fail loud). The conservative choice is to **treat `Some(empty)` as legal** — operators may set defaults labels then write `labels = {}` per-job to "clear" them; rejecting would surprise. With `use_defaults = false` they get a different (cleaner) escape hatch. See Edge Cases §8.1.

### 5.3 LBL-06 size limits (`check_label_size_limits`)

Two enforcement levels per LBL-06:

- **Per-value:** `value.as_bytes().len() > 4 * 1024` → reject.
- **Per-job total:** sum of `key.as_bytes().len() + value.as_bytes().len()` for every entry > 32 * 1024 → reject.

```rust
const MAX_LABEL_VALUE_BYTES: usize = 4 * 1024;       // 4 KB per value
const MAX_LABEL_SET_BYTES:   usize = 32 * 1024;      // 32 KB per job total

fn check_label_size_limits(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let Some(labels) = &job.labels else { return };

    // Per-value check
    let oversized_values: Vec<&String> = labels.iter()
        .filter(|(_, v)| v.as_bytes().len() > MAX_LABEL_VALUE_BYTES)
        .map(|(k, _)| k)
        .collect();
    if !oversized_values.is_empty() {
        let mut keys: Vec<&str> = oversized_values.iter().map(|s| s.as_str()).collect();
        keys.sort();
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: label values exceed 4 KB limit: {}. Each label value must be ≤ 4096 bytes.",
                job.name,
                keys.join(", ")
            ),
        });
    }

    // Per-job total check
    let total_bytes: usize = labels.iter()
        .map(|(k, v)| k.as_bytes().len() + v.as_bytes().len())
        .sum();
    if total_bytes > MAX_LABEL_SET_BYTES {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: total label-set size {} bytes exceeds 32 KB limit. Sum of all key+value byte lengths must be ≤ 32768 bytes.",
                job.name,
                total_bytes
            ),
        });
    }
}
```

#### 5.3.1 Size formula clarification

**Counted:** key bytes + value bytes (UTF-8 byte length). **NOT counted:** TOML delimiters, JSON serialization overhead, framing. This matches the operator's mental model — "the labels themselves should fit in 32 KB" — and avoids tying the contract to an internal serialization shape that could change.

#### 5.3.2 Docker's actual label-set size limit

For context only — NOT a load-bearing reference for our implementation. dockerd has historically rejected label sets totaling > ~250 KB (it varies by API version and is not formally documented). Our 32 KB ceiling is well below dockerd's, leaving operators a wide safety margin. Cronduit-side enforcement is the right design choice — it surfaces the error at config-load with a clear cronduit message rather than at container-create time as a confusing dockerd 400 response. [CITED: GitHub issue moby/moby various — informal lower-than-formal-limit observed]

### 5.4 D-02 key-char validator (`check_label_key_chars`)

```rust
use once_cell::sync::Lazy;
use regex::Regex;

static LABEL_KEY_RE: Lazy<Regex> = Lazy::new(|| {
    // Strict ASCII: leading char alphanumeric or underscore; subsequent chars
    // alphanumeric, dot, hyphen, or underscore. Mirrors the once_cell idiom
    // used at src/config/interpolate.rs:23 and src/config/validate.rs:10.
    Regex::new(r"^[a-zA-Z0-9_][a-zA-Z0-9._-]*$").unwrap()
});

fn check_label_key_chars(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let Some(labels) = &job.labels else { return };
    let invalid: Vec<&String> = labels.keys()
        .filter(|k| !LABEL_KEY_RE.is_match(k))
        .collect();
    if !invalid.is_empty() {
        let mut keys: Vec<&str> = invalid.iter().map(|s| s.as_str()).collect();
        keys.sort();
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: invalid label keys: {}. Keys must match the pattern `^[a-zA-Z0-9_][a-zA-Z0-9._-]*$` (alphanumeric/underscore start; alphanumeric/dot/hyphen/underscore body).",
                job.name,
                keys.join(", ")
            ),
        });
    }
}
```

**Recommendation: regex via `once_cell::sync::Lazy<Regex>`.** Reasons: (1) the project idiom — `src/config/validate.rs:10` and `src/config/interpolate.rs:23-24` are both `Lazy<Regex>`; (2) `once_cell` is already a direct dep; (3) the regex is short enough that the compile cost is negligible (microseconds, once per process); (4) future extension (e.g., adding stricter rules in v1.3) is one-line; (5) clippy and other static analysis tools recognize the pattern. A hand-rolled `char::is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')` loop is acceptable but loses the documentation value of the regex literal.

### 5.5 Validator function decomposition: four functions vs one

**Recommendation: four functions** (one per validator concern) matching the existing per-validator-per-concern shape at `src/config/validate.rs:21-25`. Rationale:
- Mirrors the existing `check_one_of_job_type` / `check_cmd_only_on_docker_jobs` / `check_network_mode` / `check_schedule` pattern.
- Each function is independently testable in `mod tests` — same idiom as `check_cmd_only_on_docker_jobs_rejects_on_command_job` etc. at `src/config/validate.rs:291-351`.
- Easier to reason about ordering and error output (one validator per error in `Vec<ConfigError>`).
- Per-job-per-check error aggregation idiom (D-01) is naturally one-function-per-check.

A single combined `check_labels` function would be ~10% faster (one HashMap walk instead of four) but the cost is irrelevant at config-load scale (microseconds). Rejected for style symmetry.

### 5.6 Registration in `run_all_checks`

Append four call sites inside the per-job loop at `src/config/validate.rs:20-25`:

```rust
for job in &cfg.jobs {
    check_one_of_job_type(job, path, errors);
    check_cmd_only_on_docker_jobs(job, path, errors);
    check_network_mode(job, path, errors);
    check_schedule(job, path, errors);
    // Phase 17 / SEED-001
    check_label_reserved_namespace(job, path, errors);
    check_labels_only_on_docker_jobs(job, path, errors);
    check_label_size_limits(job, path, errors);
    check_label_key_chars(job, path, errors);
}
```

---

## Pre-parse Env-Var Interpolation Behavior on Label Maps

The existing `src/config/interpolate.rs:22` pre-parse pass operates on the **raw TOML string** before `toml::from_str` runs (see `src/config/mod.rs:144-149`). It treats keys and values uniformly — neither is special-cased. The regex `\$\{([A-Z_][A-Z0-9_]*)\}` matches anywhere on a non-comment line and substitutes the env-var value.

**Consequence for label values (LBL-05):** `labels = { "deployment.id" = "${DEPLOYMENT_ID}" }` works transparently. The `${DEPLOYMENT_ID}` token is replaced before TOML parsing; the parser sees `labels = { "deployment.id" = "abc123" }`.

**Consequence for label keys:** technically operator-writable as `labels = { "${KEY_VAR}" = "v" }`. After interpolation:
- If `KEY_VAR=traefik.enable` → key resolves to `traefik.enable`, **passes** the D-02 char regex, becomes a valid label. **This is the residual gap CONTEXT.md `<specifics>` flagged.**
- If `KEY_VAR=` (unset) → produces a missing-var error from `interpolate.rs` at config-load (existing behavior; satisfies LBL-05 "keys are NOT interpolated" by triggering the error path).
- If `KEY_VAR=" with space"` → interpolated value contains chars D-02 rejects → key-char validator fires.

The fully-resolved-to-safe-chars case (`KEY_VAR=traefik.enable`) silently passes. **Treatment in Phase 17:**
1. The D-02 char regex catches the most common typo: leftover `${`/`}` (operator wrote `${VAR` without closing brace, or interpolation failed differently).
2. README labels subsection (D-04) explicitly documents "keys are NOT interpolated" to set operator expectations.
3. The full tightening (post-parse cross-reference of resolved keys against the raw TOML byte range) is deferred as a v1.3+ candidate per CONTEXT.md `<deferred>`.

**No labels-specific interpolation code is needed.** The pre-parse pass handles values for free.

---

## testcontainers Test Pattern for Label Assertions

### bollard `inspect_container` returns labels

```rust
// Verified pattern from tests/docker_executor.rs:204-215 (existing code)
let mut labels = HashMap::new();
labels.insert("cronduit.run_id".to_string(), run_id.to_string());
let body = ContainerCreateBody {
    image: Some("alpine:latest".to_string()),
    labels: Some(labels),
    ..Default::default()
};
```

After `docker.start_container(&id, None).await`, inspect returns the labels at `info.config.labels`:

```rust
let info = docker.inspect_container(&id, None).await?;
let labels = info.config.and_then(|c| c.labels).unwrap_or_default();
assert_eq!(labels.get("cronduit.run_id").map(String::as_str), Some("42"));
```

[VERIFIED: docs.rs/bollard/0.20.2 — `ContainerInspectResponse.config: Option<ContainerConfig>` and `ContainerConfig.labels: Option<HashMap<String, String>>`]

### Recommended test scenarios

`tests/v12_labels_merge.rs` (Docker-daemon required, `#[ignore]`-gated):
- `labels_merge_defaults_and_per_job_into_container` — config has both `[defaults].labels` and per-job `labels`; spawn job; inspect container; assert merged set + cronduit-internal labels present.
- `labels_per_job_wins_on_collision` — same key in defaults and per-job; assert per-job value wins.
- `labels_value_env_var_interpolated` — set `${TEST_LABEL_VAL}=hello`; assert label value is `hello` on inspected container.
- `labels_internal_labels_intact_alongside_operator_labels` — assert `cronduit.run_id` and `cronduit.job_name` still present alongside operator labels.

`tests/v12_labels_use_defaults_false.rs` (Docker-daemon required, `#[ignore]`-gated):
- `use_defaults_false_replaces_label_set` — `[defaults].labels = {a=1}`, per-job `use_defaults = false` and `labels = {b=2}`; assert spawned container has `b=2` but NOT `a=1` (and still has cronduit-internal labels).

`tests/v12_labels_validators.rs` (no Docker daemon needed — pure `parse_and_validate`):
- `reserved_namespace_rejects_cronduit_prefix` — config with `cronduit.foo`; assert error message lists the key.
- `reserved_namespace_lists_multiple_keys_in_one_error` — config with `cronduit.foo` and `cronduit.bar`; assert ONE ConfigError per job listing both keys (D-01 aggregation).
- `type_gate_rejects_labels_on_command_job` — command-type job with labels; assert error.
- `type_gate_rejects_labels_on_script_job` — script-type job with labels; assert error.
- `type_gate_accepts_labels_on_docker_job` — docker job with labels; assert no error.
- `type_gate_accepts_labels_on_docker_job_via_defaults_image` — job with no `image` directly but `[defaults].image` set + `labels`; assert no error (post-`apply_defaults` discriminant).
- `size_limit_rejects_per_value_over_4kb` — single label value 4097 bytes; assert error.
- `size_limit_rejects_per_set_over_32kb` — many small labels summing > 32 KB; assert error.
- `key_chars_rejects_space_in_key` — `"my key" = "v"`; assert error.
- `key_chars_rejects_slash_in_key` — `"foo/bar" = "v"`; assert error.
- `key_chars_rejects_empty_key` — `"" = "v"`; assert error.
- `key_chars_accepts_dotted_key` — `"com.centurylinklabs.watchtower.enable" = "false"`; assert no error.
- `key_chars_accepts_underscore_prefixed_key` — `"_internal" = "v"`; assert no error.
- `key_chars_rejects_dot_prefixed_key` — `".foo" = "v"`; assert error (leading char must be alphanumeric or underscore).

The validator-only tests are FAST and Docker-daemon-independent — they should run in the standard `cargo test` suite, not gated behind `#[ignore]`. Mirror the pattern in `src/config/validate.rs::tests` (in-source `mod tests`) — these can live there OR in a top-level `tests/v12_labels_validators.rs`. Recommendation: **in-source unit tests** for individual validators (matches existing pattern), `tests/v12_labels_validators.rs` for full-config end-to-end parse-and-validate scenarios.

### Test conventions verified

- `tests/v12_<feature>_<scenario>.rs` — confirmed via `tests/v12_fctx_*.rs`, `tests/v12_run_rs_277_bug_fix.rs`, `tests/v12_webhook_*.rs`. [VERIFIED: `ls tests/`]
- `#[ignore]`-gating for Docker-daemon-required tests, with comment block explaining `cargo test --test ... -- --ignored --nocapture --test-threads=1`. [CITED: tests/docker_executor.rs:1-9]
- Setup helpers in `tests/common/` (already exists). [VERIFIED: `ls tests/common/`]

---

## Edge Cases the Planner Must Address

### 8.1 Empty label maps `labels = {}`

TOML parses `labels = {}` as `Some(empty HashMap)`. **Recommended handling: accept as legal no-op.** Operators may write this to "clear" defaults labels per-job (alongside `use_defaults = false` which already exists for the same purpose). All four validators no-op cleanly on empty maps (their `let Some(labels) = ...` guards return early on `None` but iterate-zero-times on `Some(empty)`). The Bollard plumb-through merges an empty map; cronduit-internal labels still attach. **Test:** `labels_empty_map_accepted` — config with `labels = {}` parses without error and the spawned container has only `cronduit.run_id` + `cronduit.job_name`.

### 8.2 Job with both `command` AND `image`

Already rejected by existing `check_one_of_job_type` at `src/config/validate.rs:50-64` with "must declare exactly one of command, script, or image (found 2)". The new `check_labels_only_on_docker_jobs` may ALSO fire on the same job (command-set → image_is_none after apply_defaults skips merge), producing two errors per problematic job. This is acceptable — the aggregate-not-fail-fast posture (`Vec<ConfigError>`) means operators see all problems at once. No special handling needed.

### 8.3 Job with `use_defaults = false` AND no `labels` field

Per LBL-02, defaults are replaced (with nothing). `apply_defaults` short-circuits at L112-114 so `job.labels` stays `None`. Spawned container has only the cronduit-internal labels (`cronduit.run_id`, `cronduit.job_name`). **Test:** `use_defaults_false_with_no_per_job_labels_clears_defaults` — config with `[defaults].labels = {a=1}` and per-job `use_defaults = false` + no `labels` field; assert spawned container does NOT have `a=1`.

### 8.4 Labels with empty-string values `key = ""`

TOML and bollard both accept empty-string values. Per CONTEXT.md "Claude's Discretion" closing bullet: **default recommendation is to skip the value-non-empty check** — empty values are valid Docker labels and rejecting would surprise operators. The size validator (5.3) does NOT fire (`value.as_bytes().len() == 0` is not `> 4096`). The key-char validator only inspects keys.

### 8.5 Label keys that look reserved but aren't (`cronduit_foo`)

`cronduit_foo` (underscore) does NOT start with `cronduit.` (dot). The reserved-namespace validator uses `k.starts_with("cronduit.")`, which means `cronduit_foo`, `cronduitfoo`, `cronduit-foo` all PASS (no error). Only `cronduit.<anything>` is rejected. **Test:** `reserved_namespace_accepts_cronduit_underscore_keys` — config with `cronduit_foo = "v"`; assert no error.

### 8.6 Label keys containing `${...}` after interpolation failure

If `${VAR}` interpolation fails (var unset), `interpolate.rs` reports a `MissingVar` error and substitutes the empty placeholder, leaving the post-interpolation key as `""`. Empty key → key-char regex fails → `check_label_key_chars` fires AS WELL. Both errors surface. This is correct aggregate behavior.

### 8.7 Label value > 4 KB AND total set > 32 KB simultaneously

Both checks in `check_label_size_limits` fire independently — operator gets two ConfigErrors per problematic job (one for the per-value violation, one for the per-set violation). Acceptable per D-01.

### 8.8 Dotted key in inline-table syntax without quoting

`labels = { com.centurylinklabs.watchtower.enable = "false" }` (NO quotes around the dotted key) is interpreted by the TOML parser as a NESTED inline table (`labels.com.centurylinklabs.watchtower.enable = "false"`), which fails type-checking against `HashMap<String, String>`. Operator gets a TOML parse error from `toml::from_str` with line:col info via `src/config/mod.rs:170-183`. The README + examples MUST show quoted dotted keys to prevent this.

### 8.9 Per-job key wins on collision — operator MUST be able to nullify a defaults label

If operator wants to *remove* a single label inherited from defaults, options are: (a) per-job `labels = { "x" = "" }` to set the key to empty string (the key still exists on the container but with empty value); (b) `use_defaults = false` to nuke ALL defaults including labels (whole-section escape). There is no "delete a single defaults key" syntax. This matches every other field in the config — there is no per-key-suppression escape hatch elsewhere in cronduit. Document this in the README as part of D-04.

### 8.10 Five-layer parity drift on labels (silent loss)

The most dangerous bug class for this phase. If `JobConfig.labels` is added but `DockerJobConfig.labels` is NOT, `apply_defaults` correctly merges in memory, `serialize_config_json` may or may not serialize (depending on whether someone added the layer-2 step), and the executor reads the JSON back as a `DockerJobConfig` with no `labels` field — silently dropped. **Mitigation:** the parity regression test from Plan A item 7 above; the docstring at `src/config/defaults.rs:1-87` explicitly warns about this; planner ensures one atomic plan covers all five layers, OR strictly orders them with a passing test gate between commits.

---

## Open Questions / Risks for the Planner (RESOLVED)

The phase is heavily pre-locked. Two minor open items only — both RESOLVED at planning time:

1. **Examples job naming.** D-03 says "NEW job `isolated-batch` (or similarly-named — planner picks)". Suggested names: `isolated-batch`, `backup-only`, `vendor-managed-image`. Pick one that reads as "this job intentionally opts out of cronduit defaults." Planner discretion.
   **RESOLVED: Plan 17-04 uses `isolated-batch`.**

2. **`17-HUMAN-UAT.md` scope.** The four CI-observable items (validator errors, three integration-test scenarios) cover most of the surface. Maintainer-facing UAT worth adding for: (a) `just check examples/cronduit.toml` — does the new example file load cleanly? (b) `just docker-up && open http://localhost:8080` — does the README labels subsection render correctly on GitHub? Planner decides scope.
   **RESOLVED: Plan 17-06 ships HUMAN-UAT covering README render, `just check-config examples/cronduit.toml` parse, end-to-end docker spot-check via `just docker-compose-up`, reserved-namespace error UX.**

No technical risks. No scope-creep risk (CONTEXT.md exhaustively pre-locked the design surface). No new-crate risk (zero new deps). No DB migration risk (zero schema change). The five-layer parity invariant is the only failure mode that could ship silently — addressed by the §10 plan ordering and the parity regression test.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Label key validation | Hand-rolled char-by-char loop | `once_cell::sync::Lazy<Regex>` with the documented pattern | Project idiom (validate.rs:10, interpolate.rs:23); regex literal documents the contract; one-line extension for v1.3 |
| Env-var interpolation in label values | Labels-specific interpolation pass in validators or executor | The existing pre-parse `src/config/interpolate.rs:22` regex pass on raw TOML | Pass operates BEFORE `toml::from_str`; values pass through transparently; reimplementing would duplicate the logic and create a drift surface |
| Label map deserialization | Custom `Deserialize` impl for label-specific quirks | Plain `HashMap<String, String>` with serde derive on the field | TOML 1.1.2 + serde 1.0.228 handles dotted keys in inline tables and block tables verbatim |
| Docker label-passthrough | Shelling to `docker run --label key=value` | `bollard::Config::labels: Option<HashMap<String, String>>` | Project rule (CLAUDE.md): no CLI shelling out. The existing `ContainerCreateBody` literal already populates `labels: Some(labels)`. |
| HashMap vs BTreeMap | Convert HashMap → BTreeMap or vice-versa anywhere in the pipeline | Plain `HashMap<String, String>` end-to-end | Bollard expects HashMap; existing v1.0 `DockerJobConfig.env: HashMap<String, String>` is the precedent |
| Per-key suppression syntax | Invent a `labels.delete = ["key1", "key2"]` mini-DSL | `use_defaults = false` (whole-section escape) or empty-string value | Consistent with every other field in the config; no per-key suppression elsewhere |

---

## Common Pitfalls

### Pitfall 1: Five-layer config plumbing parity drift

**What goes wrong:** `JobConfig.labels` added; `DockerJobConfig.labels` NOT added. `apply_defaults` merges in memory; `serialize_config_json` writes labels to `jobs.config_json`; executor reads back via `serde_json::from_str::<DockerJobConfig>` and silently drops the field; spawned container has no operator labels.

**Why it happens:** The five layers (`JobConfig`, `serialize_config_json`, `compute_config_hash`, `apply_defaults`, `DockerJobConfig`) are documented at `src/config/defaults.rs:1-87` but easy to miss when adding a single field.

**How to avoid:** Single atomic plan covering all five layers; parity regression test mirroring `parity_with_docker_job_config_is_maintained` at `src/config/defaults.rs:488`.

**Warning signs:** Integration test passes for `apply_defaults` but the `docker inspect` integration test fails to find operator labels.

### Pitfall 2: Non-deterministic error message ordering

**What goes wrong:** Test asserts "error contains `cronduit.foo, cronduit.bar`" but the iteration order of the underlying `HashMap` produces `cronduit.bar, cronduit.foo` on some runs. Test flakes.

**Why it happens:** `HashMap` iteration order is non-deterministic by design.

**How to avoid:** Sort the offending-keys vector before formatting (shown in §5.1, §5.3, §5.4 above).

**Warning signs:** Validator unit test passes locally but flakes on CI.

### Pitfall 3: Dotted key interpreted as nested table

**What goes wrong:** Operator writes `labels = { com.centurylinklabs.watchtower.enable = "false" }` (no quotes). TOML parses this as nested inline tables, fails type-check against `HashMap<String, String>`, error message points to the `labels` line but doesn't tell the operator the issue is the missing quotes.

**Why it happens:** TOML dotted-key syntax IS legal; it just doesn't produce a flat HashMap.

**How to avoid:** README + `examples/cronduit.toml` show quoted dotted keys; inline comments call this out.

**Warning signs:** Operator-reported config-load error on a paste from a Docker run example that uses unquoted labels.

### Pitfall 4: Validator runs BEFORE `apply_defaults`

**What goes wrong:** `check_labels_only_on_docker_jobs` checks `job.image.is_none()` to decide non-docker, but the value of `job.image` depends on whether `apply_defaults` has run. Runs before → false positive on docker jobs that inherit `image` from `[defaults]`. Runs after → correct.

**Why it happens:** The order in `parse_and_validate` is interpolate → toml::from_str → apply_defaults → validate (`src/config/mod.rs:144-200`). The `apply_defaults` happens at L185-196, validation at L198. Provided the new validators are registered in `run_all_checks` (which validate.rs L20-25 already calls AFTER apply_defaults), the discriminant is correct.

**How to avoid:** Register new validators inside `run_all_checks` per-job loop at `src/config/validate.rs:20-25`, NOT outside it.

**Warning signs:** Test `type_gate_accepts_labels_on_docker_job_via_defaults_image` fails with a false-positive type-gate error.

### Pitfall 5: HashMap iteration mutating cronduit-internal labels

**What goes wrong:** Operator label key `cronduit.run_id` somehow reaches the docker.rs label-build site (LBL-03 validator was bypassed somewhere). The `labels.insert("cronduit.run_id", ...)` AFTER the operator HashMap merge would overwrite the operator's value with the real run_id — defense-in-depth holds.

**Why it happens:** Validator was disabled, or somehow a code path inserts labels after validation.

**How to avoid:** ALWAYS insert cronduit-internal labels AFTER operator labels (shown in §3 above). The LBL-03 validator at config-load is the primary defense; this ordering is belt-and-braces.

**Warning signs:** N/A — this is a defense-in-depth measure; the LBL-03 validator means the situation should be unreachable.

---

## Code Examples

### Adding the `labels` field to `JobConfig` and `DefaultsConfig`

```rust
// src/config/mod.rs (add to both DefaultsConfig and JobConfig)
#[derive(Debug, Deserialize)]
pub struct JobConfig {
    pub name: String,
    pub schedule: String,
    // ... existing fields ...
    pub volumes: Option<Vec<String>>,
    /// Operator-defined Docker labels attached to spawned containers.
    /// Per LBL-01..06 / SEED-001. Merged with cronduit-internal labels
    /// at container-create time. `cronduit.*` namespace reserved (LBL-03).
    /// Type-gated to docker jobs only (LBL-04). Per-value 4 KB / per-set
    /// 32 KB byte-length limits (LBL-06).
    #[serde(default)]
    pub labels: Option<std::collections::HashMap<String, String>>,
    // ... remaining fields ...
}
```

### Extending `apply_defaults` with the labels merge

```rust
// src/config/defaults.rs (inside apply_defaults, after the existing merges)

// Labels merge per LBL-02:
//   * use_defaults = false  → already short-circuited at L112-114 (per-job
//                              labels stay as-is; defaults discarded entirely)
//   * use_defaults true/unset → defaults map ∪ per-job map; per-job key wins
//                                on collision (standard override semantics)
//
// Type-gate (docker-only) is checked at validation time, NOT here. We do NOT
// gate this merge on `is_non_docker` because:
//   1. The validator handles the type-mismatch case with a clear error
//      (`labels` on command/script jobs is rejected at load).
//   2. Skipping the merge here would silently drop defaults labels for
//      command/script jobs, masking the validator's intended error.
if !is_non_docker
    && let Some(defaults_labels) = &defaults.labels
{
    let merged: std::collections::HashMap<String, String> = match job.labels.take() {
        // Per-job map present: defaults map ∪ per-job map; per-job wins.
        // `extend` overwrites existing keys; insert defaults FIRST then
        // overlay per-job keys.
        Some(per_job) => {
            let mut m = defaults_labels.clone();
            m.extend(per_job);  // per-job-wins on collision
            m
        }
        // Per-job map absent: inherit defaults wholesale.
        None => defaults_labels.clone(),
    };
    job.labels = Some(merged);
}
```

[Source: extends `src/config/defaults.rs:108-159` per CONTEXT.md `<canonical_refs>`. The `use_defaults = false` short-circuit at L112-114 already handles the replace case.]

### Test mirror of `apply_defaults_use_defaults_false_disables_merge`

```rust
// src/config/defaults.rs::tests (mirror L316-325)

#[test]
fn apply_defaults_merges_labels_per_job_wins() {
    let mut job = empty_job_docker();  // helper constructing a docker job
    let mut per_job_labels = HashMap::new();
    per_job_labels.insert("a".to_string(), "from-job".to_string());
    per_job_labels.insert("traefik.http.routers.x.rule".to_string(), "Host(`x`)".to_string());
    job.labels = Some(per_job_labels);

    let mut defaults_labels = HashMap::new();
    defaults_labels.insert("a".to_string(), "from-defaults".to_string());  // collision
    defaults_labels.insert("watchtower.enable".to_string(), "false".to_string());

    let defaults = DefaultsConfig {
        image: Some("alpine:latest".into()),
        labels: Some(defaults_labels),
        // ... other fields None ...
    };

    let merged = apply_defaults(job, Some(&defaults));
    let labels = merged.labels.expect("labels merged");

    assert_eq!(labels.get("a").map(String::as_str), Some("from-job"),
        "per-job key must win on collision");
    assert_eq!(labels.get("watchtower.enable").map(String::as_str), Some("false"),
        "defaults key without collision must be inherited");
    assert_eq!(labels.get("traefik.http.routers.x.rule").map(String::as_str), Some("Host(`x`)"),
        "per-job key must be present");
    assert_eq!(labels.len(), 3, "expected 3 merged keys");
}

#[test]
fn apply_defaults_use_defaults_false_replaces_labels() {
    let mut job = empty_job_docker();
    let mut per_job_labels = HashMap::new();
    per_job_labels.insert("backup.exclude".to_string(), "true".to_string());
    job.labels = Some(per_job_labels);
    job.use_defaults = Some(false);

    let mut defaults_labels = HashMap::new();
    defaults_labels.insert("watchtower.enable".to_string(), "false".to_string());
    let defaults = DefaultsConfig {
        labels: Some(defaults_labels),
        // ... other fields ...
    };

    let merged = apply_defaults(job, Some(&defaults));
    let labels = merged.labels.expect("per-job labels preserved");

    assert!(labels.get("watchtower.enable").is_none(),
        "use_defaults=false must replace defaults labels entirely");
    assert_eq!(labels.get("backup.exclude").map(String::as_str), Some("true"));
    assert_eq!(labels.len(), 1);
}
```

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` (Rust 2024 edition, rust-version 1.94.1) + `cargo nextest` for parallel runs |
| Config file | `Cargo.toml` (workspace) — no separate test config; in-source `#[cfg(test)] mod tests`; `tests/` for integration |
| Quick run command | `just test` (existing recipe) |
| Full suite command | `just test` + `just test-ignored` (Docker-daemon required tests) |
| Validator unit tests | `cargo test --lib config::validate` |
| Defaults merge tests | `cargo test --lib config::defaults` |
| Integration (Docker-daemon) | `cargo test --test v12_labels_merge -- --ignored --nocapture --test-threads=1` |
| Integration (parse-only) | `cargo test --test v12_labels_validators` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| LBL-01 | `labels` field accepted on DefaultsConfig + JobConfig; merged into `bollard::Config::labels` at container-create | integration (Docker-daemon) | `cargo test --test v12_labels_merge -- --ignored` | ❌ Wave 0 |
| LBL-02 | Merge: `use_defaults = false` replaces; otherwise per-key merge with per-job-wins | unit + integration | `cargo test --lib config::defaults::apply_defaults_merges_labels` + `cargo test --test v12_labels_use_defaults_false -- --ignored` | ❌ Wave 0 |
| LBL-03 | Reserved-namespace `cronduit.*` validator at LOAD time | unit | `cargo test --lib config::validate::reserved_namespace` | ❌ Wave 0 |
| LBL-04 | Type-gate validator (labels only on docker jobs) | unit | `cargo test --lib config::validate::type_gate` | ❌ Wave 0 |
| LBL-05 | `${ENV_VAR}` interpolation works in label VALUES; keys NOT interpolated | unit + integration | `cargo test --lib config::interpolate` (existing, no new tests required for the interpolation pass itself) + `cargo test --test v12_labels_merge labels_value_env_var_interpolated -- --ignored` | ⚠ Wave 0 (env-var integration test) |
| LBL-06 | Per-value 4 KB and per-job 32 KB byte-length limits at config-load | unit | `cargo test --lib config::validate::size_limit` | ❌ Wave 0 |
| D-02 (locked) | Strict ASCII key char regex `^[a-zA-Z0-9_][a-zA-Z0-9._-]*$` | unit | `cargo test --lib config::validate::key_chars` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `just test` (full unit suite; ~5-10 seconds; covers all validator + merge unit tests).
- **Per wave merge:** `just test` + targeted integration `cargo test --test v12_labels_merge -- --ignored`.
- **Phase gate (before `/gsd-verify-work`):** `just test` + `just test-ignored` (full integration suite) green on amd64 + arm64 SQLite + Postgres CI matrix.

### Wave 0 Gaps

- [ ] `tests/v12_labels_merge.rs` — covers LBL-01, LBL-02 (merge path), LBL-05 (env-var in values)
- [ ] `tests/v12_labels_use_defaults_false.rs` — covers LBL-02 (replace path)
- [ ] `tests/v12_labels_validators.rs` (OR in-source unit tests in `src/config/validate.rs::tests`) — covers LBL-03, LBL-04, LBL-06, D-02
- [ ] Parity regression test in `src/config/defaults.rs::tests::parity_labels_round_trip_through_docker_job_config` — guards against the five-layer drift pitfall
- [ ] Defaults-merge unit test `apply_defaults_merges_labels_per_job_wins` and `apply_defaults_use_defaults_false_replaces_labels` mirroring `apply_defaults_use_defaults_false_disables_merge` at `src/config/defaults.rs:316`

### CI-Observable vs Maintainer UAT Mapping

The five operator-observable success criteria from ROADMAP.md (Phase 17 § Success Criteria) map to validation as follows:

| Success Criterion | Validation Type | Owner |
|-------------------|----------------|-------|
| (1) Operator adding `labels = {...}` on a job sees that label via `docker inspect` alongside `cronduit.run_id`/`cronduit.job_name` | CI integration (Docker-daemon) | `tests/v12_labels_merge.rs::labels_internal_labels_intact_alongside_operator_labels` |
| (2) `use_defaults = false` per-job replaces; merged otherwise with per-job-wins | CI unit (apply_defaults) + CI integration | `apply_defaults_merges_labels_per_job_wins` + `tests/v12_labels_use_defaults_false.rs` |
| (3) Operator setting `cronduit.foo` gets a config-load error pointing at the offending key | CI unit | `check_label_reserved_namespace_*` tests |
| (4) Operator setting `labels` on `type = "command"`/`"script"` gets a config-load error | CI unit | `check_labels_only_on_docker_jobs_*` tests |
| (5a) Operator writing `${DEPLOYMENT_ID}` in a label value sees the env var interpolated; (5b) value > 4 KB or set > 32 KB rejected at load | CI integration (5a) + CI unit (5b) | `tests/v12_labels_merge.rs::labels_value_env_var_interpolated` + `check_label_size_limits_*` tests |

**All five success criteria are CI-testable.** No maintainer UAT is strictly required for the success criteria themselves. Maintainer UAT is worthwhile (per CONTEXT.md Claude's Discretion) for **operator-experience surfaces:** (a) `just check examples/cronduit.toml` loads cleanly with the three new label patterns; (b) the README labels subsection renders correctly on GitHub with the mermaid diagram; (c) `just docker-up` followed by `docker inspect <hello-world container>` shows the merged labels (this is a manual sanity-check on the otherwise CI-tested path). Planner picks scope.

---

## Sources

### Primary (HIGH confidence)

- `.planning/phases/17-custom-docker-labels-seed-001/17-CONTEXT.md` — locked decisions, Claude's discretion, deferred ideas, code-context insights
- `.planning/REQUIREMENTS.md` § Custom Docker Labels (LBL-01..LBL-06, T-V12-LBL-01..10)
- `.planning/seeds/SEED-001-custom-docker-labels.md` — pre-locked design
- `.planning/PROJECT.md` § Constraints (locked tech stack)
- `.planning/ROADMAP.md` § Phase 17 — goal + 5 operator-observable success criteria
- `.planning/research/STACK.md` — confirms zero new crates required; existing deps cover everything
- `src/config/mod.rs` (read in full) — `JobConfig`/`DefaultsConfig` struct shape, `parse_and_validate` flow
- `src/config/defaults.rs` (read in full) — `apply_defaults` + five-layer parity invariant docstring + existing test pattern
- `src/config/validate.rs` (read in full) — validator function shape, `run_all_checks` registration site, test pattern
- `src/config/interpolate.rs` (read in full) — pre-parse env-var interpolation; `once_cell::sync::Lazy<Regex>` idiom
- `src/scheduler/docker.rs` (lines 1-280 read) — `DockerJobConfig`, label-build site, `inspect_container` API
- `src/scheduler/sync.rs` (lines 1-120 read) — `serialize_config_json` shape (layer 2 of five-layer parity)
- `src/config/hash.rs` (lines 1-100 read) — `compute_config_hash` (layer 3 of five-layer parity)
- `src/scheduler/docker_orphan.rs` (lines 1-80 read) — `cronduit.run_id` consumer (load-bearing reason for LBL-03)
- `tests/docker_executor.rs` (lines 1-120 read) — testcontainers pattern + `Docker::connect_with_local_defaults` + label assertion shape
- `tests/docker_orphan_guard.rs` (label assertion lines confirmed) — `ContainerCreateBody.labels: Some(HashMap)` literal usage
- `tests/` directory listing — confirms `v12_<feature>_<scenario>.rs` naming convention, existing v12 tests at `v12_fctx_*.rs`, `v12_run_rs_277_bug_fix.rs`, `v12_webhook_*.rs`
- `Cargo.toml` (read in full) — confirms zero new deps required; `regex 1`, `once_cell 1`, `serde 1.0.228`, `toml 1.1.2`, `bollard 0.20`, `std::collections::HashMap` all in tree
- `examples/cronduit.toml` (read in full) — current quickstart layout for D-03 extension

### Secondary (MEDIUM confidence — verified against authoritative source)

- docs.rs/bollard/0.20.2/bollard/models/struct.ContainerCreateBody.html — `labels: Option<HashMap<String, String>>` documented as "User-defined key/value metadata" [VERIFIED via WebFetch 2026-04-28]
- docs.rs/bollard/0.20.2 `ContainerInspectResponse.config: Option<ContainerConfig>` and `ContainerConfig.labels: Option<HashMap<String, String>>` — for test `inspect_container` label assertions
- TOML 1.1.2 + serde 1.0.228 — dotted-key handling in inline-table syntax for `HashMap<String, String>` deserialization [VERIFIED via WebSearch + cross-reference with existing TOML usage in `src/config/`]

### Tertiary (LOW confidence — informational only, not load-bearing)

- Docker label-set size limit (~250 KB observed at dockerd; not formally documented) — context-only for our 32 KB cronduit-side ceiling

---

## Assumptions Log

> Every claim tagged `[ASSUMED]` in this research; planner and discuss-phase use to identify decisions needing user confirmation.

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| (none) | All claims in this research are either [VERIFIED] against tools/source or [CITED] from CONTEXT.md / REQUIREMENTS.md / SEED-001 | — | — |

**Empty Assumptions Log:** All factual claims in this research were either verified by direct source-read (15+ source files), tool-confirmed (bollard 0.20.2 docs.rs WebFetch), or cited from the heavily-pre-locked CONTEXT.md / SEED-001 / REQUIREMENTS.md. Phase 17 is unusually well-specified — no user confirmation is needed before planning.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — zero new deps; bollard 0.20.2 `ContainerCreateBody.labels` signature verified against docs.rs.
- Architecture: HIGH — five-layer parity invariant explicitly documented at `src/config/defaults.rs:1-87`; merge semantics mirror existing `apply_defaults` + `use_defaults = false` short-circuit.
- Validators: HIGH — direct port of existing `check_cmd_only_on_docker_jobs` template; regex idiom (`once_cell::sync::Lazy<Regex>`) is project-standard at two existing sites.
- Pitfalls: HIGH — five-layer parity drift is the documented load-bearing risk class with an existing regression test pattern (`parity_with_docker_job_config_is_maintained` at `src/config/defaults.rs:488`).
- Test strategy: HIGH — `v12_<feature>_<scenario>.rs` naming verified across 8 existing v12 tests; testcontainers pattern verified at `tests/docker_executor.rs`.

**Research date:** 2026-04-28
**Valid until:** 2026-05-28 (30 days; stable design with locked tech stack and zero new deps).
