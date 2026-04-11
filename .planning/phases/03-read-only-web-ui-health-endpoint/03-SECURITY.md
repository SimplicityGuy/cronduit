# SECURITY.md — Phase 03: Read-Only Web UI & Health Endpoint

**Phase:** 03 — read-only-web-ui-health-endpoint
**ASVS Level:** 1
**Audit Date:** 2026-04-10
**Threats Closed:** 19/19

---

## Threat Verification

| Threat ID | Category | Disposition | Status | Evidence |
|-----------|----------|-------------|--------|----------|
| T-03-01 | Tampering | mitigate | CLOSED | `assets/vendor/htmx.min.js` exists and is checked into the repo. `templates/base.html:52` loads `/vendor/htmx.min.js` (no CDN reference anywhere). |
| T-03-02 | Information Disclosure | accept | CLOSED | Documented below in Accepted Risks. |
| T-03-03 | Tampering | accept | CLOSED | Documented below in Accepted Risks. |
| T-03-04 | Injection | mitigate | CLOSED | `src/db/queries.rs:501` uses `AND LOWER(j.name) LIKE ?1` (parameterized bind, not interpolation). Filter pattern constructed with `format!("%{}%", filter.unwrap().to_lowercase())` and bound as a query parameter. |
| T-03-05 | Denial of Service | accept | CLOSED | Documented below in Accepted Risks. |
| T-03-06 | Information Disclosure | accept | CLOSED | Documented below in Accepted Risks. |
| T-03-07 | Elevation of Privilege | mitigate | CLOSED | CSRF protection implemented in Plan 05. `src/web/handlers/api.rs` validates CSRF token before any `SchedulerCmd::RunNow` is sent. Channel itself is in-process only. |
| T-03-08 | Injection | mitigate | CLOSED | Same evidence as T-03-04. The `get_dashboard_jobs` filter parameter is bound as `?1`/`$1` in both SQLite and Postgres paths. |
| T-03-09 | Injection | mitigate | CLOSED | `src/db/queries.rs:477-487`: `order_clause` is produced by a `match (sort, order)` whitelist over a closed set of string literals. User input is never interpolated into the SQL string. |
| T-03-10 | Spoofing | accept | CLOSED | Documented below in Accepted Risks. |
| T-03-11 | XSS | mitigate | CLOSED | `src/web/ansi.rs:16`: `ansi_to_html::convert(raw)` HTML-escapes input before ANSI transformation. `templates/partials/log_viewer.html:14`: `\|safe` used only on `log.html` (crate output). `tests/xss_log_safety.rs`: CI test `script_tag_is_escaped` asserts `<script>` becomes `&lt;script&gt;`; `html_injection_inside_ansi_is_escaped` asserts `<img>` inside ANSI is escaped. |
| T-03-12 | Information Disclosure | accept | CLOSED | Documented below in Accepted Risks. |
| T-03-13 | Injection | mitigate | CLOSED | `src/web/handlers/job_detail.rs:5` and `src/web/handlers/run_detail.rs:8` use `axum::extract::Path<i64>` (and `Path<(i64,i64)>`). Non-numeric path segments return 422 before reaching handler logic. |
| T-03-14 | CSRF | mitigate | CLOSED | `src/web/csrf.rs:27-41`: `validate_csrf()` uses XOR-based constant-time comparison. `src/web/csrf.rs:63`: cookie set with `HttpOnly; SameSite=Strict`. `src/web/handlers/api.rs:38-39`: CSRF validation returns 403 on mismatch before any DB or scheduler access. `src/web/mod.rs:37`: `ensure_csrf_cookie` middleware applied to entire router. |
| T-03-15 | Denial of Service | accept | CLOSED | Documented below in Accepted Risks. |
| T-03-16 | Elevation of Privilege | mitigate | CLOSED | `src/web/handlers/api.rs:43-49`: `queries::get_job_by_id(&state.pool, job_id).await` called before `SchedulerCmd::RunNow` is sent. Returns 404 for unknown job IDs. |
| T-03-17 | Repudiation | mitigate | CLOSED | `src/scheduler/mod.rs:147`: RunNow branch calls `run_job(... "manual".to_string() ...)`, recording `trigger='manual'` in `job_runs`. |
| T-03-18 | XSS | mitigate | CLOSED | `tests/xss_log_safety.rs`: `safe_filter_only_on_ansi_output` test walks all `.html` template files at CI time and asserts `\|safe` appears only on `log.html` in `log_viewer.html`. Regression net is in place. |
| T-03-19 | Spoofing | accept | CLOSED | Documented below in Accepted Risks. |

---

## Accepted Risks Log

| Threat ID | Category | Rationale |
|-----------|----------|-----------|
| T-03-02 | Information Disclosure on static asset paths | Assets served at `/static/*` and `/vendor/*` are CSS, JS, fonts, and favicons. No secrets or sensitive data. Cache headers on public resources are standard practice. |
| T-03-03 | Tampering on build.rs | `build.rs` runs in the developer's build environment at compile time, not in production. Standard Cargo pattern; the build output (compiled binary) is what runs in production. |
| T-03-05 | Denial of Service on GET /health DB check | Health check executes a single `SELECT 1` query. No amplification risk. Pool timeout and connection limit provide back-pressure. No mitigating controls needed at ASVS Level 1. |
| T-03-06 | Information Disclosure on health response | Response reveals DB connectivity status and a static "running" scheduler status. Cronduit's security posture explicitly targets homelab operators on loopback/trusted LAN. The operator is the sole user. Documented in THREAT_MODEL.md. |
| T-03-10 | Spoofing on HTMX polling source | HTMX polling at `/partials/job-table` is a read-only GET endpoint. No state changes possible through this path. No authentication in v1 by design. Documented in THREAT_MODEL.md. |
| T-03-12 | Information Disclosure on config display | Config JSON displayed on the Job Detail page is the operator's own configuration. `SecretString` fields in the config are already rendered as `[redacted]` per Phase 1 security posture. |
| T-03-15 | Denial of Service on Run Now flooding | The `mpsc` channel has a buffer of 32. Excess sends return `SERVICE_UNAVAILABLE`. No authentication in v1 means any client on the network can attempt this, but homelab posture accepts this risk. Documented in THREAT_MODEL.md. |
| T-03-19 | Spoofing on health false positive | Health check runs a real `SELECT 1` query against the DB pool. False positives are only possible if the query succeeds but some other subsystem is broken — an acceptable residual at ASVS Level 1. |

---

## Unregistered Flags

None. The `## Threat Flags` sections in all six SUMMARY files (03-01 through 03-06) reported no new attack surface that did not map to an existing threat ID. Plan 04's SUMMARY explicitly states "None — all new endpoints are read-only GET handlers."

---

## Notes

- The `\|safe` template filter appears exactly once across all templates: `templates/partials/log_viewer.html:14` on `{{ log.html|safe }}`. This is the only sanctioned use. The CI test `safe_filter_only_on_ansi_output` in `tests/xss_log_safety.rs` enforces this constraint on every PR.
- The HTMX vendor file (`assets/vendor/htmx.min.js`) contains the string `htmx` and is loaded locally at `/vendor/htmx.min.js`. No external CDN references exist in any template.
- CSRF double-submit cookie uses 32-byte (256-bit) random token generated by `rand::RngCore::fill_bytes`. Constant-time XOR comparison prevents timing oracle attacks.
- Path parameters (`job_id`, `run_id`) use Axum's typed `Path<i64>` extractor, providing type-level injection prevention before handler code executes.
