# Security Audit — Phase 02: Scheduler Core / Command / Script Executor

**Phase:** 02 — scheduler-core-command-script-executor
**ASVS Level:** 1
**Audit Date:** 2026-04-10
**Auditor:** gsd-secure-phase

---

## Threat Verification

| Threat ID | Category | Disposition | Status | Evidence |
|-----------|----------|-------------|--------|----------|
| T-02-01 | Tampering | mitigate | CLOSED | `src/scheduler/sync.rs:10` imports `compute_config_hash`; `src/scheduler/fire.rs:63` calls `Cron::from_str` with warn on parse error; config validated by `parse_and_validate` before sync reaches DB |
| T-02-02 | Denial of Service | mitigate | CLOSED | `src/scheduler/fire.rs:175` — `MAX_CATCHUP_WINDOW_HOURS: i64 = 24`; lines 204-213 cap scan window; test `clock_jump_limited_to_24h_window` at line 407 asserts at most 24 fires returned on 48-hour jump |
| T-02-03 | Information Disclosure | mitigate | CLOSED | `src/scheduler/sync.rs:43-74` — `serialize_config_json` builds a manual JSON map that stores `env_keys` (key names only) and never emits `SecretString` values; test `sync_config_json_excludes_secret_values` at line 327 asserts `!job.config_json.contains("super-secret-value")` |
| T-02-04 | Tampering | accept | CLOSED | Accepted: operator self-trust; `src/scheduler/command.rs:168` uses `shell_words::split` so no shell is invoked; documented in plan threat register |
| T-02-05 | Denial of Service | mitigate | CLOSED | `src/scheduler/log_pipeline.rs:14` — `DEFAULT_CHANNEL_CAPACITY: usize = 256`; line 58-60 — `buf.pop_front()` on overflow (head-drop); `MAX_LINE_BYTES: usize = 16384` at line 12; `truncate_line` at line 165 enforces 16 KB limit |
| T-02-06 | Denial of Service | mitigate | CLOSED | `src/scheduler/command.rs:150-156` — `kill_process_group` calls `libc::kill(-(pid as i32), libc::SIGKILL)`; invoked on both timeout (line 115) and shutdown/cancel (line 128); `process_group(0)` set at line 192 |
| T-02-07 | Elevation of Privilege | accept | CLOSED | Accepted: operator controls script content (self-trust); `NamedTempFile` in system temp dir; documented in plan threat register |
| T-02-08 | Information Disclosure | accept | CLOSED | Accepted: output is the operator's own command output, stored in job_logs; no additional exposure; documented in plan threat register |
| T-02-09 | Tampering | mitigate | CLOSED | `src/scheduler/run.rs:83-139` — `serde_json::from_str(&job.config_json)` returns `JobExecConfig` with `unwrap_or(JobExecConfig { command: None, script: None })` on parse failure; missing fields produce `RunStatus::Error` with message, no panic |
| T-02-10 | Denial of Service | accept | CLOSED | Accepted: no concurrency limit in v1; homelab scale; `JoinSet` tracks all in-flight tasks; documented in plan threat register |
| T-02-11 | Denial of Service | mitigate | CLOSED | `src/db/queries.rs:376-386` — `insert_log_batch` wraps inserts in a single transaction per batch; `DEFAULT_BATCH_SIZE: usize = 64` at `src/scheduler/log_pipeline.rs:17`; SQLite write pool has `busy_timeout=5000ms` from Phase 1 |
| T-02-12 | Denial of Service | mitigate | CLOSED | `src/scheduler/mod.rs:159` — `grace_deadline` bounds drain; line 192 — `join_set.abort_all()` on grace expiry; `src/shutdown.rs:17` — `std::process::exit(1)` on second signal |
| T-02-13 | Tampering | accept | CLOSED | Accepted: aborted tasks may leave job_runs in status='running'; Phase 4 SCHED-08 orphan reconciliation; documented in plan threat register and 02-04-SUMMARY.md key-decisions |
| T-02-14 | Repudiation | mitigate | CLOSED | `src/scheduler/mod.rs:211-220` — `tracing::info!` with fields `in_flight_count`, `drained_count`, `force_killed_count`, `grace_elapsed_ms` emitted as "shutdown complete"; also emitted for zero-in-flight case at lines 147-155 |

---

## Accepted Risks Log

| Threat ID | Category | Rationale | Review Trigger |
|-----------|----------|-----------|----------------|
| T-02-04 | Tampering (command injection) | Operator writes their own TOML config; already has Docker socket access; `shell_words` prevents injection from adjacent config values. No multi-tenant exposure. | Adding user-submitted job definitions from the web UI |
| T-02-07 | Elevation of Privilege (tempfile) | System temp dir with random name via `NamedTempFile`; operator controls script content. If `/tmp` is noexec the script fails with a clear error. | Operator-configurable temp dir or multi-tenant context |
| T-02-08 | Information Disclosure (log content) | Log content is from the operator's own commands and stored in operator's own DB. No secondary exposure path exists in v1. | Adding log streaming to external sinks or multi-user web access |
| T-02-10 | Denial of Service (concurrent runs) | Homelab single-operator use case; bounded only by OS process limits. JoinSet provides observability. | Adding rate-limiting requirements or multi-tenant support |
| T-02-13 | Tampering (orphan run rows) | Aborted tasks on forced shutdown may leave job_runs.status='running'. Phase 4 SCHED-08 will reconcile at next startup. No data corruption — rows are queryable but show stale status. | Phase 4 implementation of orphan reconciliation |

---

## Unregistered Flags

None. All threat flags in SUMMARY.md `## Threat Flags` sections map to registered threat IDs in the threat register (SUMMARY files for plans 01-04 contain no unregistered threat flags section).

---

## Verification Coverage

| Plan | Threats Covered | Tests Verifying Mitigations |
|------|-----------------|-----------------------------|
| 02-01 | T-02-01, T-02-02, T-02-03 | `scheduler::sync::tests::sync_config_json_excludes_secret_values`, `scheduler::fire::tests::clock_jump_limited_to_24h_window` |
| 02-02 | T-02-04, T-02-05, T-02-06, T-02-07, T-02-08 | `scheduler::log_pipeline::tests::channel_head_drop`, `scheduler::log_pipeline::tests::line_truncation_over_boundary`, `scheduler::command::tests::execute_timeout` |
| 02-03 | T-02-09, T-02-10, T-02-11 | `scheduler::run::tests::run_job_timeout_preserves_partial_logs`, `scheduler::run::tests::concurrent_runs_create_separate_rows` |
| 02-04 | T-02-12, T-02-13, T-02-14 | `scheduler::tests::shutdown_drain_completes_within_grace`, `scheduler::tests::shutdown_grace_expiry_force_kills`, `scheduler::tests::shutdown_summary_fields` |

**Total threats closed:** 14/14
