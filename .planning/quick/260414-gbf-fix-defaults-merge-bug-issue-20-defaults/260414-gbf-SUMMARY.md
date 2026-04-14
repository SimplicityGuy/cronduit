---
phase: 260414-gbf-fix-defaults-merge-bug-issue-20-defaults
plan: 01
subsystem: config
tags:
  - issue-20
  - v1.0-retag-blocker
  - config-defaults
  - docker-labels
dependency-graph:
  requires:
    - src/config/mod.rs (JobConfig, DefaultsConfig, Config)
    - src/config/hash.rs (compute_config_hash)
    - src/config/validate.rs (check_one_of_job_type)
    - src/scheduler/sync.rs (serialize_config_json, job_type)
    - src/scheduler/docker.rs (DockerJobConfig -- read-only, NOT modified)
  provides:
    - src/config/defaults.rs::apply_defaults
    - JobConfig.delete field
    - JobConfig.cmd field
    - parity regression test locking JobConfig / serialize_config_json / DockerJobConfig invariant
  affects:
    - tests/defaults_merge.rs (new integration test binary)
    - examples/cronduit.toml (hello-world job exercises [defaults] merge + cmd override)
    - docs/SPEC.md (documents cmd field)
    - .planning/milestones/v1.0-REQUIREMENTS.md (retroactive notes on CONF-03/04/06)
    - .planning/milestones/v1.0-phases/01-foundation-security-posture-persistence-base/01-VERIFICATION.md
    - .github/workflows/release.yml (docker/metadata-action@v5 labels + annotations)
    - Dockerfile (expanded static LABEL comment block)
tech-stack:
  added: []
  patterns:
    - "parity_with_docker_job_config_is_maintained: structural regression test locking TOML-side struct, DB-side serializer, and executor-side deserialize struct in one assertion"
    - "apply_defaults() single-call merge pattern: downstream consumers read already-merged JobConfigs and never re-consult Config.defaults for per-job fields"
    - "docker/metadata-action@v5 labels + annotations: single source of truth for OCI metadata across platform image configs AND top-level manifest index"
key-files:
  created:
    - src/config/defaults.rs
    - tests/defaults_merge.rs
    - .planning/quick/260414-gbf-fix-defaults-merge-bug-issue-20-defaults/260414-gbf-SUMMARY.md
  modified:
    - src/config/mod.rs
    - src/config/hash.rs
    - src/config/validate.rs
    - src/scheduler/sync.rs
    - tests/scheduler_integration.rs
    - examples/cronduit.toml
    - docs/SPEC.md
    - .planning/milestones/v1.0-REQUIREMENTS.md
    - .planning/milestones/v1.0-phases/01-foundation-security-posture-persistence-base/01-VERIFICATION.md
    - .github/workflows/release.yml
    - Dockerfile
decisions:
  - "[defaults] merges docker-only fields (image/network/volumes/delete) ONLY into docker jobs. Command/script jobs inherit `timeout` but NOT image/network/volumes/delete -- otherwise check_one_of_job_type would fire on every non-docker job in a config that has [defaults].image. This was a deviation from the plan (Rule 1 - bug fix) discovered when tests/fixtures/valid-everything.toml broke."
  - "`cmd` is per-job only. DefaultsConfig does NOT gain a cmd field. `Some(vec![])` hashes and serializes distinctly from `None` (explicit empty-CMD override vs. image's baked-in CMD)."
  - "`container_name` decision (per-job only, container names must be unique) locked in code via `apply_defaults_does_not_touch_container_name` + mermaid parity table."
  - "Cargo.toml version stays at EXACTLY 1.0.0. User will re-tag v1.0.0 from the merged commit."
metrics:
  duration_estimate: "~1h wall time for 5 tasks + verification"
  tasks_completed: 5
  commits: 6
  tests_added: 37
  files_touched: 13
---

# Quick Task 260414-gbf: [defaults] Merge Bug Fix + Docker Labels Summary

Fixes issue #20 (`[defaults]` silently ignored) and the GHCR "Connected to repository" sidebar link for multi-arch images, with zero Cargo.toml version bump so the user can re-tag `v1.0.0` from the merged commit.

## Objective Recap

Close a v1.0.0 retag blocker: `[defaults]` was parsed into `Config.defaults` but never merged into `JobConfig`s, so every per-job field except `random_min_gap` was silently ignored — breaking CONF-03/04/06 and, worst of all, silently bypassing VPN routing when an operator set `network = "container:vpn"` in `[defaults]`. Fix the merge at a single call site, thread `delete` and a new `cmd` field through the full TOML → JobConfig → config_json → DockerJobConfig plumbing, and fix the unrelated GHCR Docker labels bug in the same PR since both block v1.0 retag.

## What Changed

### Defaults merge fix (Tasks 1-3, 5)

- `src/config/defaults.rs` (new): `apply_defaults(job, defaults)` pure function with a mermaid classDiagram + parity table doc block covering the five-layer invariant (JobConfig / serialize_config_json / compute_config_hash / apply_defaults / DockerJobConfig). 18 unit tests including the `random_min_gap` / `cmd` / `container_name` non-touch invariants and the structural `parity_with_docker_job_config_is_maintained` regression guard.
- `src/config/mod.rs`: new `JobConfig.delete: Option<bool>` and `JobConfig.cmd: Option<Vec<String>>` fields; doc comment on `Config.defaults` warning against post-parse re-consultation; `parse_and_validate` now calls `apply_defaults` between `toml::from_str` and `run_all_checks` using `std::mem::take` + `.map().collect()`.
- `src/config/hash.rs`: `compute_config_hash` now includes `delete` and `cmd`. 3 new tests (`hash_stable_across_defaults_merge` for image/network/volumes/timeout/delete, `hash_differs_on_delete_change`, `hash_differs_on_cmd_change` pairwise-distinct for Some(["a"])/Some(["b"])/None/Some([])).
- `src/config/validate.rs`: `check_one_of_job_type` error message now points users at `[defaults].image` and `use_defaults = false`. 1 new unit test.
- `src/scheduler/sync.rs`: `serialize_config_json` promoted to `pub(crate)` and now emits `delete` and `cmd`. 2 new tests (`serialize_config_json_includes_delete`, `serialize_config_json_includes_cmd`).
- `tests/defaults_merge.rs` (new): 13 end-to-end integration tests exercising every defaults-eligible field through `parse_and_validate`, including the marquee `defaults_network_container_vpn_preserved` VPN regression, `hash_stable_across_defaults_representations`, `cmd_preserved_on_docker_job`, and `cmd_in_defaults_is_not_merged`.
- `tests/scheduler_integration.rs`: struct-literal update for the new `delete` and `cmd` fields (out-of-scope compile fix).
- `examples/cronduit.toml`: `hello-world` job now omits `image` and `delete` (inherited from `[defaults]`) AND sets `cmd = ["echo", "Hello from cronduit defaults!"]`. Block comment rewritten to explain both the merge demo and the `cmd` override.
- `docs/SPEC.md`: documents the new `cmd` field with an example code block and a paragraph explaining `docker run IMAGE CMD...` override semantics.
- `.planning/milestones/v1.0-REQUIREMENTS.md`: retroactive honesty notes on CONF-03, CONF-04, and CONF-06.
- `.planning/milestones/v1.0-phases/01-foundation-security-posture-persistence-base/01-VERIFICATION.md`: CONF-03/04/06 rows updated with evidence pointing at `src/config/defaults.rs::apply_defaults` and `tests/defaults_merge.rs`.

### Docker labels fix (Task 4)

- `.github/workflows/release.yml`: new `docker/metadata-action@v5` step (id: `meta`) that generates tags (via `type=semver` + `type=raw,value=latest`), labels, AND annotations from a single source of truth. `build-push-action@v6` step now references `${{ steps.meta.outputs.tags }}`, `${{ steps.meta.outputs.labels }}`, and `${{ steps.meta.outputs.annotations }}`. The hand-rolled inline tags and labels blocks are removed. `org.opencontainers.image.source` is emitted explicitly on BOTH the labels and annotations paths (belt-and-suspenders alongside metadata-action's auto-population).
- `Dockerfile`: LABEL values unchanged; comment block expanded to document which three labels GHCR reads on the package page and to link to GitHub's labelling docs. The LABELs remain as a fallback for local `docker build .` runs outside the release workflow.

## Test Coverage Added

| File                          | Kind          | New tests |
| ----------------------------- | ------------- | --------- |
| `src/config/defaults.rs`      | unit          | 18        |
| `src/config/hash.rs`          | unit          | 3         |
| `src/config/validate.rs`      | unit          | 1         |
| `src/scheduler/sync.rs`       | unit          | 2         |
| `tests/defaults_merge.rs`     | integration   | 13        |
| **Total**                     |               | **37**    |

Breakdown of `src/config/defaults.rs` (18): 13 behavior tests from the plan (5 fill-from-defaults + 5 override-wins + use_defaults=false + defaults=None + random_min_gap non-touch) + `apply_defaults_does_not_touch_cmd` + `apply_defaults_skips_docker_fields_on_command_jobs` + `apply_defaults_skips_docker_fields_on_script_jobs` (deviation-driven regression guards) + Task 5's `apply_defaults_does_not_touch_container_name` + `parity_with_docker_job_config_is_maintained`.

`cargo nextest run --workspace` now reports 245 tests run (up from ~211 before). All pass.

## Requirements Re-satisfied

| ID      | Status | Retroactive note |
| ------- | ------ | ---------------- |
| CONF-03 | ✓      | `[defaults]` fields now actually merge into every job via `apply_defaults` (was parse-only before) |
| CONF-04 | ✓      | `use_defaults = false` now actually disables merging via the early return in `apply_defaults` (was a no-op field before) |
| CONF-06 | ✓      | Per-job field override precedence now actually enforced via `apply_defaults` (Phase 1 said this was "Phase 2 work"; it never landed in Phase 2) |

## Docker Labels Rationale

GitHub Container Registry's package page UI recognizes exactly three OCI labels: `org.opencontainers.image.source` (powers the "Connected to repository" sidebar link), `org.opencontainers.image.description` (subtitle under the package name), and `org.opencontainers.image.licenses` (license badge in the sidebar). For multi-arch (manifest list) images, the `source` value must be present on BOTH the per-platform image configs AND the top-level manifest index. The previous workflow only set `labels:` on `docker/build-push-action@v6`, which writes to the platform image configs, leaving the manifest index without `org.opencontainers.image.source` as an annotation. `docker/metadata-action@v5` is the canonical source for both labels and annotations — it auto-populates `source`/`revision`/`version`/`created`/`url` from repo context and supports explicit overrides that metadata-action deduplicates by key. This commit replaces the hand-rolled `tags:`/`labels:` blocks in `build-push-action` with `steps.meta.outputs.*` references and adds the new `annotations:` input so the manifest index carries the same metadata as the platform images. `source` is also emitted explicitly (belt-and-suspenders) so the guarantee is load-bearing instead of dependent on metadata-action internals.

## Known Gap / Follow-up

**`delete = false` is not honored by `src/scheduler/docker.rs::cleanup_container`.** The field flows all the way through `JobConfig` → `config_json` and will reach the executor's deserialize boundary if `DockerJobConfig` adds a `delete` field in a future issue. Today, `delete = true` matches current behavior (cronduit always force-removes containers to avoid the moby#8441 race), so only `delete = false` is a no-op. File a follow-up issue "Honor `delete = false` to preserve failed containers for inspection (references moby#8441 race + DOCKER-06 constraint)" when beginning the next milestone.

**Reject `cmd` on command/script jobs.** Plan-check NIT-1: a command job with `cmd = [...]` is nonsense but is currently not a validation error — the `cmd` field is simply dropped silently. File a follow-up issue to make `check_one_of_job_type` (or a new sibling check) flag `cmd` on non-docker jobs as a parse error.

**Out-of-scope systemic follow-ups** (NOT bundled into this PR, file as separate issues when starting v1.1):

1. Audit all v1.0 requirements goal-backward to catch any other retroactively-satisfied-only rows. CONF-03/04/06 were all checked off in `v1.0-REQUIREMENTS.md` based on struct existence rather than behavior — the same pattern may exist for other requirements.
2. Add a CI check that `examples/cronduit.toml` minimally exercises every defaults-eligible field and every per-job-only docker field. Without this, a future edit could re-break the merge demo.
3. Extend the parity audit to cover `command`/`script` executor argument paths once those grow beyond single-string fields (the current `JobExecConfig` is trivially in parity because it only carries one field per job type).

## Parity Audit Outcome

The audit enumerated `DockerJobConfig` (6 fields: `image`, `env`, `volumes`, `cmd`, `network`, `container_name`) and `JobExecConfig` (2 fields: `command`, `script`) as the only executor-side deserialize structs that read `config_json`. Findings:

- Two in-flight gaps (closed by this PR): (a) the `[defaults]` merge itself, (b) the missing `cmd` field on `JobConfig` / `serialize_config_json` / `compute_config_hash`.
- One additional undocumented decision: `container_name` has no explicit branch in `apply_defaults` and no documentation explaining WHY it is per-job only (container names must be unique — two containers cannot share a name, so there is no `DefaultsConfig.container_name` by design). Resolved in Task 5 by the module-level mermaid parity table + `apply_defaults_does_not_touch_container_name` unit test.
- No additional executor-side code fixes required.

`parity_with_docker_job_config_is_maintained` is now the regression guard for the JSON surface: it constructs a fully-populated `JobConfig` with every non-secret field `DockerJobConfig` reads, runs it through `serialize_config_json`, asserts every expected key is present in the output, asserts no raw `SecretString` value leaked (T-02-03), and asserts the output is structurally deserializable back into a `DockerJobConfig` (catches field renames like `image` → `image_ref` on one side but not the other). This test does NOT catch `compute_config_hash` or `apply_defaults` drift — those still rely on PR review discipline and the parity table doc block.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] apply_defaults must skip docker-only fields on command/script jobs**

- **Found during:** Task 1 GREEN verification (`cargo nextest run --workspace` failed on `tests/config_parser.rs::valid_everything_parses`).
- **Issue:** The plan's straightforward `apply_defaults` body merges `image`/`network`/`volumes`/`delete` unconditionally. But the existing fixture `tests/fixtures/valid-everything.toml` mixes a docker job, a script job, and a command job under one `[defaults] image = "alpine:latest"` block. After merging, the script and command jobs ended up with BOTH their per-job field AND the inherited `image`, tripping `check_one_of_job_type` ("found 2"). Every existing config in the wild with mixed job types and a `[defaults].image` would have broken.
- **Fix:** Gate the docker-specific merge branches (`image`, `network`, `volumes`, `delete`) on `!is_non_docker` where `is_non_docker = job.command.is_some() || job.script.is_some()`. `timeout` still merges into every job type because it is not docker-specific. Added two regression tests: `apply_defaults_skips_docker_fields_on_command_jobs` and `apply_defaults_skips_docker_fields_on_script_jobs`.
- **Files modified:** `src/config/defaults.rs` (function body + 2 tests).
- **Commit:** `1eee77b` (Task 1 GREEN).

This is a behavior-preserving semantic refinement of the plan's intent: the plan's must_haves all describe docker job behaviors, and the existing fixture demonstrates the intended user pattern. Skipping docker-only fields on non-docker jobs is the only way the merge can satisfy both `check_one_of_job_type` AND the plan's must_haves.

### Authentication Gates

None.

## Cargo.toml Version Check

`Cargo.toml` was **NOT modified**. Current reading:

```toml
[package]
name = "cronduit"
version = "1.0.0"
```

The user will re-tag `v1.0.0` from the merged commit per the user-memory rule "tag and release version must match". `tempfile` (used by `tests/defaults_merge.rs`) is already present at `Cargo.toml:79` in `[dependencies]`, reachable from integration tests, so no dev-dependency add was needed.

## Next Steps (user)

1. Review the PR locally on branch `fix/defaults-merge-issue-20`.
2. Run UAT smoke tests on your machine (UAT cannot be self-declared by Claude):
   - `cargo run --bin cronduit -- check examples/cronduit.toml` → exit 0
   - `cargo run --bin cronduit -- run --config examples/cronduit.toml` long enough to trigger one `hello-world` run; confirm `Hello from cronduit defaults!` appears in the web UI logs for that run (proves the `[defaults]` merge AND the per-job `cmd` override flow all the way to bollard).
   - Any docker/command/script job you care about still parses and runs.
3. Merge the PR to `main`.
4. Re-tag `v1.0.0` from the merged commit — the release workflow now emits OCI-compliant labels + annotations via `docker/metadata-action`, so the GHCR "Connected to repository" sidebar link will resolve on the rebuilt multi-arch image.

## Self-Check: PASSED

Verified by direct re-inspection after SUMMARY.md write:

- `src/config/defaults.rs` exists and contains `pub fn apply_defaults`, `classDiagram`, `parity_with_docker_job_config_is_maintained`, `apply_defaults_does_not_touch_container_name`.
- `tests/defaults_merge.rs` exists with 13 integration tests listed.
- All 6 commits are present in `git log main...HEAD` with `(260414-gbf)` scope: `be60cbe`, `1eee77b`, `5ab1a09`, `f968e42`, `ae22620`, `3d0d15f`.
- `Cargo.toml` version line reads `version = "1.0.0"` (unchanged).
- `git diff main...HEAD src/scheduler/docker.rs` produces zero lines (Known Gap preserved).
- `.github/workflows/release.yml` parses via `yq` (YAML_OK).
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo nextest run --workspace` all clean (245 tests passed, 19 skipped).
- `cronduit check examples/cronduit.toml` exits 0.
- Smoke test with bare `[defaults] image = "alpine:latest"` + minimal docker job exits 0.
- Smoke test with no image anywhere exits 1 AND the error message contains `[defaults]` and `use_defaults`.
