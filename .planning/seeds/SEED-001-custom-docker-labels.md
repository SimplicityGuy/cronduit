---
id: SEED-001
status: dormant
planted: 2026-04-24
planted_during: between v1.1 and v1.2 — milestone close-out for v1.1 just completed
trigger_when: v1.2 milestone kickoff during the requirements pass (operator must explicitly raise this seed at `/gsd-new-milestone` for v1.2; do not auto-promote)
scope: Small
---

# SEED-001: Custom Docker labels on spawned containers

`labels` map in `[defaults]` and per `[[jobs]]`, plumbed through to bollard `Config::labels` so cronduit-spawned containers carry operator-defined labels in addition to the existing cronduit-internal labels (`cronduit.run_id`, `cronduit.job_name`).

## Why This Matters

**Both reasons apply — general label-passthrough is foundational:**

1. **Reverse-proxy + watchtower interop.** Operators integrating cronduit-spawned containers with Traefik/Caddy reverse proxies, Watchtower exclusion (`com.centurylinklabs.watchtower.enable=false`), backup tooling that filters by label, and other label-driven sidecar tooling currently can't do so — cronduit only emits its internal `cronduit.run_id` and `cronduit.job_name` labels, with no way for the operator to add their own. This is the most common homelab integration pattern and it's blocked today.

2. **Cost allocation + ops categorization.** Shared infra deployments need labels for cost allocation (`team`, `env`, `cost-center`) and `docker ps` filtering. Adds operational hygiene for cronduit users running in non-trivial environments.

The underlying capability — "cronduit doesn't pass operator-defined labels through to bollard" — is a foundational gap regardless of the motivating use case. Closing it once unlocks the entire ecosystem of label-driven Docker tooling.

## When to Surface

**Trigger:** v1.2 milestone kickoff during the requirements pass.

This seed should be presented during `/gsd-new-milestone` for v1.2 when the milestone scope is being defined. The operator may also raise it explicitly during the questioning phase. Conditions that should make this seed obviously in-scope:

- v1.2 includes operator-ergonomics work (label passthrough is operator-facing UX).
- Any milestone in which `[defaults]` / `[[jobs]]` schema is being touched (avoid double-rev of the schema).
- Any milestone considering interop with reverse-proxy / sidecar tooling (Traefik, Watchtower, backup labels).

## Scope Estimate

**Small** — one phase plan, a few hours of execution. Concrete shape:

1. **TOML schema addition.**
   - Add `labels: Option<HashMap<String, String>>` to `DefaultsConfig` and `JobConfig` in `src/config/`.
   - `humantime`/`serde-style` map deserializer; TOML keys may contain dots (`com.centurylinklabs.watchtower.enable = "false"`).

2. **Bollard plumb-through.**
   - Extend the `labels` HashMap built at `src/scheduler/docker.rs:146-149` (currently only `cronduit.run_id` + `cronduit.job_name`) to merge in operator-defined labels.
   - The merged map populates `Config::labels: Some(...)` at `src/scheduler/docker.rs:171`.

3. **Merge semantics — LOCKED at seed time:**
   - **`use_defaults = false`** → per-job `labels` REPLACE defaults entirely (whole-section escape hatch consistent with rest of config; matches `apply_defaults` at `src/config/defaults.rs:112`).
   - **`use_defaults = true` or unset** → defaults map ∪ per-job map; **on key collision, per-job key wins** (standard override semantics; matches every other field).
   - Add a unit test in `src/config/defaults.rs` mirroring `apply_defaults_use_defaults_false_disables_merge` (line 316) for the labels case.

4. **Two new validators in `src/config/validate.rs`** (parallel the `check_cmd_only_on_docker_jobs` pattern at line 89):
   - **Reserved-namespace validator.** Operator labels under `cronduit.*` MUST fail config validation at load time. The `cronduit.*` prefix is reserved for cronduit-internal labels (currently `cronduit.run_id` for orphan reconciliation per `src/scheduler/docker_orphan.rs`, plus `cronduit.job_name`). Validator emits a GCC-style error pointing at the offending key.
   - **Type-gated validator.** Setting `labels` on a `type = "command"` or `type = "script"` job is a config-validation error (commands and scripts have no container; the labels would be silently dropped). Mirrors the existing `cmd`-on-non-docker validator's error-message pattern.

5. **Tests.**
   - Unit tests for both validators (reject + accept paths) in `src/config/validate.rs` test module.
   - Integration test that spawns a docker job with operator labels and asserts via `bollard::container::inspect_container` that the labels land on the running container alongside `cronduit.run_id` / `cronduit.job_name`. Reuses the `testcontainers` harness pattern from existing Phase 4 docker tests.

6. **Docs.**
   - Add `labels` to the `[defaults]` and `[[jobs]]` examples in `examples/cronduit.toml` with realistic values (Traefik annotation + Watchtower exclusion).
   - README `## Configuration` section: short subsection on the merge semantics + reserved-namespace rule.

## Breadcrumbs

Existing code that this seed integrates with:

- **`src/scheduler/docker.rs:146-171`** — current label-building site (`cronduit.run_id`, `cronduit.job_name`). The merge point for operator-defined labels.
- **`src/scheduler/docker_orphan.rs`** — consumer of `cronduit.run_id` for orphan reconciliation. Justifies the `cronduit.*` reserved namespace.
- **`src/config/defaults.rs:112`** (`apply_defaults`) and `:316` (`apply_defaults_use_defaults_false_disables_merge` test) — reference implementation for `use_defaults = false` short-circuit; labels code follows the same shape.
- **`src/config/validate.rs:89`** (`check_cmd_only_on_docker_jobs`) — reference implementation for the type-gated validator pattern; labels code follows the same shape.
- **`src/config/validate.rs:22`** — registration site for new validators (called from `validate_jobs` per-job loop).
- **`examples/cronduit.toml`** — example-config update site.

## Decisions LOCKED at seed time

These decisions do NOT need to be re-litigated when this seed surfaces at v1.2 kickoff. They were resolved in the seed-planting conversation on 2026-04-24:

| Decision | Resolution |
|----------|-----------|
| Merge semantics — replace vs per-key merge vs both? | **Both.** `use_defaults = false` → replace; otherwise per-key merge with per-job-wins on collision. Consistent with rest of config. |
| Reserved namespace? | **Yes — `cronduit.*` reserved.** Operator labels under `cronduit.*` are a config-validation error at load time. |
| Type gating? | **Yes — labels only valid on `type = "docker"` jobs.** Setting `labels` on command/script jobs is a config-validation error (parallels the v1.0.1 `cmd`-on-non-docker validator). |

## Notes

- This was the first concrete v1.2 candidate raised post-v1.1-close. v1.2 scope was not yet defined when the seed was planted; the operator chose to capture the design context immediately rather than re-derive it at milestone kickoff.
- Feature is additive — no scheduler-core change, no schema migration (TOML schema only, no DB column), no new external dependency. Bollard's `Config::labels` field already accepts `Option<HashMap<String, String>>`.
- Future generalization (deferred): if v1.3+ adds non-docker label-equivalents (e.g., systemd unit annotations, environment-tag emission to logs), revisit whether the type-gated validator should soften.
