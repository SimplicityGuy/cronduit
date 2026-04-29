---
phase: 16
slug: failure-context-schema-run-rs-277-bug-fix
status: verified
threats_open: 0
threats_total: 17
threats_closed: 17
asvs_level: 1
created: 2026-04-28
audited: 2026-04-28
---

# Phase 16 — Security

> Per-phase security contract: threat register, accepted risks, and audit trail.

Phase 16 is a database schema substrate + load-bearing v1.1 bug fix at `src/scheduler/run.rs:301`. No new external surface (no new HTTP routes, no new outbound calls, no new operator-facing config keys beyond what FCTX-04 / FOUND-14 / FCTX-07 explicitly prescribe). All threats land at low severity; the only `mitigate` disposition is the bug fix itself, structurally verified by the run.rs grep + HUMAN-UAT spot check on 2026-04-28.

---

## Trust Boundaries

| Boundary | Description | Data Crossing |
|----------|-------------|---------------|
| migration runner → DB | Static SQL applied by `sqlx::migrate!`; no untrusted input | DDL + parameterless backfill UPDATE |
| backfill UPDATE | Reads `jobs.config_hash` (operator-controlled config); writes `job_runs.config_hash` (operator-internal storage) | SHA-256 hex digests (not secrets) |
| bollard → DockerExecResult | `ContainerCreateResponse.id` and `inspect_container().image` come from the operator-controlled Docker daemon | Container ID (hex), image digest (`sha256:...`) |
| caller → queries::finalize_run / insert_running_run | New positional args; sqlx parameterization (`?N` SQLite, `$N` Postgres) | `Option<&str>` for image_digest; `&str` for config_hash |
| caller → get_failure_context | `job_id: i64` from internal callers (Phase 18 webhook worker, Phase 21 web handlers); already authorized at caller level | Read-only query results |
| Test infrastructure | testcontainers Postgres image + EXPLAIN-plan assertions; no production attack surface | Test-only |

---

## Threat Register

| Threat ID  | Category               | Component                                 | Disposition | Mitigation                                                                                                                                                                                                                            | Status |
|------------|------------------------|-------------------------------------------|-------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|--------|
| T-16-01-01 | Tampering              | Backfill UPDATE statement                 | accept      | No untrusted input flows into the SQL; the subquery reads `jobs.config_hash` written by the trusted config-load path. SQL is static literal in the migration file.                                                                    | closed |
| T-16-01-02 | Denial of Service      | Postgres `ACCESS EXCLUSIVE` during ALTER  | accept      | Both columns nullable (no table rewrite on PG 11+); ACCESS EXCLUSIVE held only for catalog metadata flip (microseconds). Homelab DBs <100k rows; no concurrent-write pressure at upgrade boundary.                                     | closed |
| T-16-01-03 | Denial of Service      | Bulk backfill UPDATE on very large DBs    | accept      | D-02 locks single bulk UPDATE; v1.3 introduces chunked-loop if scaling pain emerges. Homelab target <100k rows completes in <1s. RESEARCH §G.3 documents PG MVCC behavior — row locks only, no table lock.                              | closed |
| T-16-01-04 | Information Disclosure | `job_runs.config_hash` exposes config history | accept | `config_hash` is a SHA-256 hex digest of in-memory config — not reversible. Already exposed via `jobs.config_hash` (per-JOB) since v1.0; per-RUN exposure is incremental. THREAT_MODEL.md v1 documents `job_runs` as operator-visible. | closed |
| T-16-02-01 | Tampering              | DockerExecResult struct widening          | accept      | Struct field add is binary-compatible at this seam (no public API surface; consumed only inside the cronduit binary). No new attack surface.                                                                                          | closed |
| T-16-02-02 | Information Disclosure | container_id leakage to logs              | accept      | Container IDs are not secrets; already exposed via `docker ps` and existing log-streaming path. THREAT_MODEL.md v1 documents `job_runs` content as operator-visible.                                                                  | closed |
| T-16-03-01 | Information Disclosure | run.rs:301 was leaking image digests as container IDs | mitigate | **Fixed in `src/scheduler/run.rs:305-306`**: `container_id_for_finalize = docker_result.container_id` (was `.image_digest`); parallel `image_digest_for_finalize = docker_result.image_digest`. Verified by VERIFICATION.md grep + HUMAN-UAT spot check 2026-04-28 (real 64-char-hex container IDs in `job_runs.container_id` for runs 114/116/119, no `sha256:` prefix). Pre-Phase-16 rows age out via Phase 6 retention pruner (90-day default). | closed |
| T-16-03-02 | Tampering              | testcontainers integration test           | accept      | Test is `#[ignore]`-gated to local dev / CI matrix where Docker is available; matches existing `tests/docker_executor.rs` convention. No CI-time risk because the integration suite is opt-in.                                        | closed |
| T-16-04a-01 | Tampering             | New `.bind(image_digest)` / `.bind(config_hash)` sites | accept | All bind sites use sqlx parameterization (`?N` SQLite, `$N` Postgres). No string-concat into SQL; values flow through `bind()` exclusively. SQL is static literal in function body.                                                  | closed |
| T-16-04a-02 | Denial of Service     | Two columns added to SELECT projection    | accept      | image_digest + config_hash are short TEXT (sha256: 71 chars; SHA-256 hex 64 chars); negligible per-row size growth. No index changes; existing `idx_job_runs_job_id_start` covers relevant query patterns.                            | closed |
| T-16-04a-03 | Information Disclosure| `DbRun` / `DbRunDetail` widening exposes new fields to web templates | accept | Web layer not consuming new fields yet; templates that don't reference them remain unchanged. Phase 21 is the deliberate UI consumer; v1.2 release notes (Phase 24) will document the new operator-visible columns.                | closed |
| T-16-04b-01 | Tampering             | api.rs error-fallback `finalize_run` with image_digest=None | accept | Pass `None` explicitly; this caller fires when scheduler channel is closed (no docker run happened); image_digest=None is semantically correct.                                                                                   | closed |
| T-16-04b-02 | Information Disclosure| `just uat-fctx-bugfix-spot-check` recipe queries dev DB | accept | Recipe targets local dev DB; values are operator-internal (not secrets). HUMAN-UAT runs locally; no remote exposure.                                                                                                              | closed |
| T-16-05-01 | Tampering             | SQL injection via `job_id` parameter      | accept      | sqlx parameterization (`?1` / `$1`) — `bind(job_id)` where `job_id: i64` (no string concatenation). Standard pattern across queries.rs.                                                                                              | closed |
| T-16-05-02 | Denial of Service     | Unbounded streak count for very-long-failing jobs | accept | streak CTE counts via SQL `COUNT(*)` with index lookup on `idx_job_runs_job_id_start`; bounded by Phase 6 retention pruner (90-day default). Consecutive failures count is bounded by retention window.                              | closed |
| T-16-05-03 | Information Disclosure| `last_success_image_digest` / `last_success_config_hash` exposed in struct | accept | Image digests and config hashes are not secrets (see T-16-01-04 + T-16-02-02). Both are operator-internal.                                                                                                                       | closed |
| T-16-06-01 | Tampering             | EXPLAIN output format drift across SQLite/Postgres versions | accept | RESEARCH §G.4 + Assumptions A1/A2 — SQLite EXPLAIN QUERY PLAN format stable since 3.7+; Postgres FORMAT JSON Node Type stable since 9.4+. Documented textual-fallback for Postgres provides defense if Node Type semantics shift.  | closed |

*Status: open · closed*
*Disposition: mitigate (implementation required) · accept (documented risk) · transfer (third-party)*

---

## Accepted Risks Log

| Risk ID    | Threat Ref  | Rationale                                                                                                                                                                                                                                              | Accepted By | Date       |
|------------|-------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-------------|------------|
| AR-16-01   | T-16-01-01..04 | Schema migration risks (DoS / Info Disclosure on operator-internal data) accepted in line with v1 THREAT_MODEL.md posture: `job_runs` is operator-visible by design; per-RUN exposure of config_hash is incremental, not new.                       | Robert      | 2026-04-28 |
| AR-16-02   | T-16-02-01..02 | Internal struct widening + container_id log exposure accepted: container IDs are not secrets and were already discoverable via `docker ps`.                                                                                                            | Robert      | 2026-04-28 |
| AR-16-03   | T-16-03-02   | testcontainers integration test gated `#[ignore]`; matches the project-wide convention for Docker-dependent tests. Standard CI does not run them; they exist for local dev + the dedicated Docker-tier CI job.                                          | Robert      | 2026-04-28 |
| AR-16-04   | T-16-04a-01..03 | sqlx parameterized binds + projection-only SELECT widening accepted: SQL is static literal; `bind()` covers untrusted-input prevention; row-size growth is sub-1KB.                                                                                  | Robert      | 2026-04-28 |
| AR-16-05   | T-16-04b-01..02 | api.rs error-fallback caller passes `image_digest=None` semantically (no docker run happened); just recipe is dev-only and operator-internal.                                                                                                         | Robert      | 2026-04-28 |
| AR-16-06   | T-16-05-01..03 | get_failure_context query: SQL injection prevented by sqlx bind; DoS bounded by retention; new struct fields are operator-internal (not secrets).                                                                                                     | Robert      | 2026-04-28 |
| AR-16-07   | T-16-06-01   | EXPLAIN-plan format stability accepted on SQLite 3.7+ / PG 9.4+ baselines; documented textual-fallback provides defense in depth.                                                                                                                      | Robert      | 2026-04-28 |

*Accepted risks do not resurface in future audit runs.*

---

## Mitigation Verification — T-16-03-01

The single `mitigate` disposition (T-16-03-01 — the load-bearing v1.1 bug at `run.rs:301`) was verified through three independent signals:

1. **Code grep (VERIFICATION.md):** `src/scheduler/run.rs:305-306` shows
   `container_id_for_finalize = docker_result.container_id.clone()` (was `.image_digest`)
   `image_digest_for_finalize = docker_result.image_digest.clone()` (parallel local).

2. **HUMAN-UAT spot check (2026-04-28):** Maintainer ran `sqlite3 cronduit.db "SELECT ... WHERE j.job_type = 'docker' ORDER BY jr.id DESC LIMIT 3"` and observed three consecutive `spot-check-docker` runs (id=114/116/119) with real 64-char-hex container IDs (no `sha256:` prefix) and `sha256:5b10f432...` image digests.

3. **Standard CI regression-lock (post-fix WR-02):** `tests/v12_run_rs_277_bug_fix.rs::wr02_finalize_args_wiring_locks_found14_against_silent_regression` (added in commit `d49ac60`) exercises the wiring without a Docker daemon, so a future swap re-introducing the bug would fail standard `cargo test`.

---

## Security Audit Trail

| Audit Date | Threats Total | Closed | Open | Run By                                |
|------------|---------------|--------|------|---------------------------------------|
| 2026-04-28 | 17            | 17     | 0    | /gsd-secure-phase 16 (orchestrator)   |

---

## Sign-Off

- [x] All threats have a disposition (16 accept / 1 mitigate)
- [x] Accepted risks documented in Accepted Risks Log (AR-16-01 through AR-16-07)
- [x] `threats_open: 0` confirmed
- [x] `status: verified` set in frontmatter
- [x] T-16-03-01 mitigation verified by code grep + HUMAN-UAT + post-fix CI regression test
