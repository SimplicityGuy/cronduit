---
phase: 04-docker-executor-container-network-differentiator
audited_date: 2026-04-11
threats_total: 13
threats_closed: 13
threats_open: 0
status: SECURED
asvs_level: 1
---

# Security Audit — Phase 04: Docker Executor & Container Network Differentiator

## Threat Verification

| Threat ID | Category | Disposition | Status | Evidence |
|-----------|----------|-------------|--------|----------|
| T-04-01 | Elevation of Privilege | accept | CLOSED | Accepted risk — see Accepted Risks log below |
| T-04-02 | Tampering | accept | CLOSED | Accepted risk — see Accepted Risks log below |
| T-04-03 | Information Disclosure | mitigate | CLOSED | `src/scheduler/docker.rs:132-134` — labels contain only `cronduit.run_id` (integer) and `cronduit.job_name` (string); no env var values or secrets appear in labels |
| T-04-04 | Information Disclosure | accept | CLOSED | Accepted risk — see Accepted Risks log below |
| T-04-05 | Denial of Service | accept | CLOSED | Accepted risk — see Accepted Risks log below |
| T-04-06 | Information Disclosure | mitigate | CLOSED | Log content stored as-is in DB (`src/scheduler/docker_log.rs`); HTML escaping delegated to UI render layer established in Phase 3 — mitigation is in the serving path, not the storage path, per plan design |
| T-04-07 | Tampering | accept | CLOSED | Accepted risk — see Accepted Risks log below |
| T-04-08 | Information Disclosure | accept | CLOSED | Accepted risk — see Accepted Risks log below |
| T-04-09 | Denial of Service | mitigate | CLOSED | `src/scheduler/docker_pull.rs:66` — bounded backoff `[1, 2, 4]` seconds (7s max total retry delay); `docker_pull.rs:103-105` — terminal errors (`unauthorized`, `manifest unknown`, `invalid reference`) return `Err` immediately, skipping all retry iterations |
| T-04-10 | Denial of Service | mitigate | CLOSED | `src/scheduler/docker_orphan.rs:59-68` — `stop_container` with `t: Some(10)` grace for running orphans; `docker_orphan.rs:71-82` — `remove_container` with `force: true` for all orphans; both calls use `.ok()` so individual failures are logged and skipped without aborting reconciliation |
| T-04-11 | Spoofing | accept | CLOSED | Accepted risk — see Accepted Risks log below |
| T-04-12 | Tampering | mitigate | CLOSED | `src/scheduler/docker_orphan.rs:121,131` — both SQLite and Postgres UPDATE queries include `AND status = 'running'` predicate, preventing orphan reconciliation from overwriting already-finalized run rows |
| T-04-13 | Denial of Service | mitigate | CLOSED | Integration tests use `testcontainers` with automatic cleanup on drop; manual containers use `force: true` removal in `reconcile_orphans`; confirmed in `tests/docker_executor.rs` and `tests/docker_container_network.rs` |

## Accepted Risks Log

The following threats were evaluated and accepted as part of the Phase 04 threat model. Each accepted risk is documented with rationale and any compensating controls.

### T-04-01 — Elevation of Privilege: Docker socket access

- **Component:** Cronduit -> Docker socket boundary
- **Risk:** Mounting the Docker socket grants root-equivalent access. Any container Cronduit creates can escape to the host.
- **Rationale:** This is inherent to Cronduit's design purpose. A cron scheduler that orchestrates Docker containers must have Docker socket access. There is no feasible v1 alternative.
- **Compensating Controls:** Default bind address is `127.0.0.1:8080` (loopback only). Startup emits a loud warning if bind is non-loopback. Documented in `THREAT_MODEL.md` and README security section. Operators are expected to control socket access through standard Docker security practices (rootless Docker, socket proxy, etc.).

### T-04-02 — Tampering: Volume mount paths

- **Component:** Config file -> Container HostConfig volumes
- **Risk:** An operator could configure arbitrary host paths as container volume mounts, affecting host filesystem.
- **Rationale:** Config is operator-authored and operator-controlled. Volume paths are not derived from user input or external sources — they are static values set by the person who deployed Cronduit.
- **Compensating Controls:** Config file is mounted read-only inside the Cronduit container. No web UI exposes volume path editing in v1.

### T-04-04 — Information Disclosure: Env vars in container

- **Component:** Config file -> Container env vars
- **Risk:** Environment variables passed to containers are visible via `docker inspect` to anyone with Docker socket access.
- **Rationale:** Docker socket access already implies the ability to inspect all containers. Cronduit does not expand this attack surface. Env var values use `SecretString` in the config layer; they become plain strings in container env (required by the Docker API).
- **Compensating Controls:** Container inspect results are not surfaced in the Cronduit web UI. Labels (which are visible in the dashboard) contain only `run_id` and `job_name` (T-04-03 is mitigated separately).

### T-04-05 — Denial of Service: Container resource exhaustion

- **Component:** Docker executor -> Host resources
- **Risk:** A misconfigured job could spawn containers that exhaust CPU, memory, or disk, degrading the host.
- **Rationale:** Resource limits are an operator-level concern in v1. Adding CPU/memory limits to `HostConfig` without per-job configuration support would require a new config schema that is out of scope.
- **Compensating Controls:** None in v1. Documented limitation. Planned for a future phase when per-job resource limit configuration is added to the TOML schema.

### T-04-07 — Tampering: Image pull from untrusted registry

- **Component:** Image registry -> Docker daemon
- **Risk:** Operator could configure a job to pull from a compromised or malicious registry.
- **Rationale:** Image selection is operator-controlled via the config file. Cronduit does not add a registry allowlist in v1 — this would require a separate policy layer beyond the project scope.
- **Compensating Controls:** None in v1. Documented in README. Operators are expected to use trusted registries and digest pinning where required.

### T-04-08 — Information Disclosure: Pre-flight reveals container existence

- **Component:** Cronduit -> Docker daemon (pre-flight queries)
- **Risk:** The `container:<name>` pre-flight check (`inspect_container`) reveals whether a named container exists and its running state.
- **Rationale:** Docker socket access already grants complete visibility into all containers on the host. The pre-flight query adds no incremental information disclosure surface beyond what any Docker socket holder already has.
- **Compensating Controls:** N/A — the threat is subsumed by T-04-01 (Docker socket EoP, accepted).

### T-04-11 — Spoofing: Fake cronduit labels on containers

- **Component:** Orphan reconciliation -> Docker container labels
- **Risk:** An attacker could create containers with `cronduit.run_id` labels to interfere with orphan reconciliation or mark legitimate DB rows as orphaned.
- **Rationale:** An attacker able to create Docker containers with arbitrary labels already has Docker socket access, which is root-equivalent (T-04-01). Label spoofing adds no meaningful new attack capability beyond what the socket access already provides.
- **Compensating Controls:** T-04-12 (mitigated) provides a SQL guard `AND status = 'running'` that limits DB damage from spoofed labels to only rows still in running state.

## Unregistered Threat Flags

None. No `## Threat Flags` section was present in any of the four SUMMARY.md files for this phase.

## Audit Trail

- **Files examined:**
  - `.planning/phases/04-docker-executor-container-network-differentiator/04-01-PLAN.md`
  - `.planning/phases/04-docker-executor-container-network-differentiator/04-02-PLAN.md`
  - `.planning/phases/04-docker-executor-container-network-differentiator/04-03-PLAN.md`
  - `.planning/phases/04-docker-executor-container-network-differentiator/04-04-PLAN.md`
  - `.planning/phases/04-docker-executor-container-network-differentiator/04-01-SUMMARY.md`
  - `.planning/phases/04-docker-executor-container-network-differentiator/04-02-SUMMARY.md`
  - `.planning/phases/04-docker-executor-container-network-differentiator/04-03-SUMMARY.md`
  - `.planning/phases/04-docker-executor-container-network-differentiator/04-04-SUMMARY.md`
  - `src/scheduler/docker.rs`
  - `src/scheduler/docker_log.rs`
  - `src/scheduler/docker_pull.rs`
  - `src/scheduler/docker_preflight.rs`
  - `src/scheduler/docker_orphan.rs`
  - `src/scheduler/run.rs`
  - `src/db/queries.rs`

- **Verification method:** Grep for declared mitigation patterns in cited source files; accepted risks documented in this file constitute their own closure evidence.
- **ASVS Level:** 1
