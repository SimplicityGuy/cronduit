# Security Audit — Phase 05: config-reload-random-resolver

**Audited:** 2026-04-12
**ASVS Level:** 1
**Threats Closed:** 15/15
**Threats Open:** 0/15

---

## Threat Verification

| Threat ID | Category | Disposition | Status | Evidence |
|-----------|----------|-------------|--------|----------|
| T-05-01 | Tampering | mitigate | CLOSED | `src/scheduler/random.rs:57-66` — `resolve_schedule()` validates field count is exactly 5; malformed input logged as WARN and returned unchanged |
| T-05-02 | Denial of Service | mitigate | CLOSED | `src/scheduler/random.rs:27-30,74,223` — `MAX_RESOLVE_RETRIES=10`, `MAX_SLOT_RETRIES=100` constants cap all retry loops; `resolve_random_schedules_batch()` includes feasibility pre-check at line 182 that relaxes infeasible gap to `1440 / num_jobs` |
| T-05-03 | Information Disclosure | accept | CLOSED | Accepted risk: @random resolved values are intentionally visible in UI per RAND-06; no secret data is involved in resolution |
| T-05-04 | Tampering | mitigate | CLOSED | `src/scheduler/reload.rs:36-57` — `do_reload()` calls `config::parse_and_validate()` first; parse failure returns `ReloadResult{status: Error}` without any DB mutation (RELOAD-04) |
| T-05-05 | Denial of Service | mitigate | CLOSED | `src/scheduler/mod.rs:196-240` — reload coalescing via `try_recv()` drain loop; concurrent SIGHUP signals are coalesced into at most one additional reload, not N separate reloads |
| T-05-06 | Denial of Service | mitigate | CLOSED | `src/scheduler/reload.rs:287` — 500ms debounce (`Duration::from_millis(500)`) in the file watcher select loop; only one `SchedulerCmd::Reload` is sent per debounce window |
| T-05-07 | Tampering | mitigate | CLOSED | `src/scheduler/reload.rs:257,259,279-283` — watcher watches parent directory with `RecursiveMode::NonRecursive`; events are filtered by exact `config_filename` match before debounce is armed |
| T-05-08 | Tampering | mitigate | CLOSED | `src/web/handlers/api.rs:88-93` — `reload()` validates CSRF via `csrf::validate_csrf(&cookie_token, &form.csrf_token)` before processing; returns 403 FORBIDDEN on mismatch |
| T-05-09 | Tampering | mitigate | CLOSED | `src/web/handlers/api.rs:199-204` — `reroll()` validates CSRF token before sending command; also verifies job existence via `get_job_by_id()` before dispatching |
| T-05-10 | Denial of Service | mitigate | CLOSED | `src/scheduler/mod.rs:196-240` — reload coalescing in scheduler loop drains queued Reload commands after each reload run; rapid API requests are funneled through the same mpsc channel and coalesced |
| T-05-11 | Repudiation | mitigate | CLOSED | `src/web/handlers/api.rs:160-167` — `reload()` logs at info level with `status`, `added`, `updated`, `disabled` fields; `api.rs:240-246` — `reroll()` logs at info level with `job_id`, `job_name`, `status` |
| T-05-12 | Information Disclosure | accept | CLOSED | Accepted risk: reload state (timestamp, summary) in settings page is intentionally operator-visible; no secrets are stored in `ReloadState` |
| T-05-13 | Tampering | mitigate | CLOSED | `templates/pages/job_detail.html` — Re-roll form includes `csrf_token` hidden field; `src/web/handlers/api.rs:199-204` — validated by `reroll()` handler |
| T-05-14 | Tampering | mitigate | CLOSED | `templates/pages/settings.html` — Reload Config form includes `csrf_token` hidden field; `src/web/handlers/api.rs:88-93` — validated by `reload()` handler |
| T-05-15 | Information Disclosure | accept | CLOSED | Accepted risk: integration test fixtures use no real secrets; tempfiles are cleaned up by `tempfile::NamedTempFile` and `tempfile::TempDir` drop impls |

---

## Accepted Risks Log

| Threat ID | Rationale |
|-----------|-----------|
| T-05-03 | @random resolved schedules are a design feature (RAND-06) requiring UI visibility; they contain no sensitive data — only cron timing values derived from field ranges (minute 0-59, hour 0-23, etc.) |
| T-05-12 | Last reload timestamp and diff summary are operator-facing status information; the settings page is behind the same unauthenticated UI as all other pages (v1 auth posture documented in THREAT_MODEL.md) |
| T-05-15 | Test fixture TOML configs contain only synthetic job definitions; `tempfile` crate guarantees cleanup on drop; no production secrets flow into test code |

---

## Unregistered Threat Flags

None. All threat flags from SUMMARY.md `## Threat Flags` sections map to registered threat IDs.

---

## Notes

**T-05-07 path traversal scope:** The file watcher filters by exact filename match (`config_filename`), not a path prefix. An adversary with write access to the config parent directory could place files with other names without triggering a reload, but could also overwrite the config file directly. This is within the accepted local-filesystem trust boundary for a self-hosted tool.

**T-05-05 / T-05-10 coalescing implementation:** The `try_recv()` drain loop in `src/scheduler/mod.rs:197` runs after each Reload branch completes. Non-Reload commands (RunNow, Reroll) encountered during the drain are handled inline. This ensures at most one additional coalesced reload, bounding the worst-case reload amplification factor to 2x regardless of SIGHUP rate or API request concurrency.
