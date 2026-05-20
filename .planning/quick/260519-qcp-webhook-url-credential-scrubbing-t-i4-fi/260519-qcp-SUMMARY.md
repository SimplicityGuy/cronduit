---
phase: quick-260519-qcp
plan: 01
subsystem: security
tags: [webhooks, t-i4, credential-scrubbing, threat-model, url, sqlx, tracing]

# Dependency graph
requires:
  - phase: Phase 20 (Webhook posture)
    provides: webhook delivery worker, HttpDispatcher, RetryingDispatcher, webhook_deliveries DLQ table
  - phase: Phase 1 (Foundation)
    provides: strip_db_credentials precedent in src/db/mod.rs
provides:
  - strip_url_credentials helper (src/db/mod.rs) — userinfo stripping for webhook URLs, passthrough-safe
  - userinfo scrubbing applied at every webhook URL sink (tracing spans + reqwest error + DLQ persist)
  - examples/cronduit.toml header comments corrected to list all 8 active jobs
affects: [THREAT_MODEL T-I4, webhooks, v1.2.1 patch release]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Credential-stripping via url::Url round-trip (set_password(None) + set_username(\"\")), mirroring strip_db_credentials"
    - "Passthrough-on-parse-failure variant for fields that carry non-URL text (reqwest errors, plain messages)"

key-files:
  created: []
  modified:
    - src/db/mod.rs
    - src/webhooks/dispatcher.rs
    - src/webhooks/retry.rs
    - tests/v12_webhook_dlq.rs
    - examples/cronduit.toml

key-decisions:
  - "strip_url_credentials lives ALONGSIDE strip_db_credentials in src/db/mod.rs — url::Url already imported there (line 24), it is the project's single credential-stripping precedent, and it is already pub-reachable as crate::db::strip_url_credentials from src/webhooks/. A separate util module would add a file + import for zero benefit."
  - "Deliberate divergence from strip_db_credentials: on parse failure return the input UNCHANGED (not \"<unparseable>\"). A webhook URL field must pass non-URL / userinfo-free inputs through safely (reqwest error strings, plain messages); only an embedded user:pass@host URL token is rewritten."
  - "Primary defense is scrubbing at the dispatcher boundary (reqwest error → Network construction); the write_dlq url scrub and the Network last_error re-scrub are idempotent defense-in-depth."

patterns-established:
  - "Pattern: scrub at the construction boundary AND at the persist boundary — idempotent helper makes the second pass harmless belt-and-suspenders."

requirements-completed: [T-I4]

# Metrics
duration: ~25min
completed: 2026-05-19
---

# Quick Task 260519-qcp: Webhook URL Credential Scrubbing (T-I4) Summary

**Closed the last unmitigated Information-Disclosure gap in the v1.2 STRIDE register: webhook URL `userinfo` (`https://user:pass@host`) can no longer reach `webhook_deliveries.url`, `webhook_deliveries.last_error`, or any webhook tracing span — a new `strip_url_credentials` helper now scrubs every enumerated sink.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-05-20T02:00Z (approx)
- **Completed:** 2026-05-20T02:18Z (approx)
- **Tasks:** 3 completed
- **Files modified:** 5 (4 source/test + 1 example config)

## Accomplishments

- Added `pub fn strip_url_credentials(url: &str) -> String` in `src/db/mod.rs` mirroring `strip_db_credentials`, with 7 unit tests (TDD: RED → GREEN). Adds **no new imports** (reuses `use url::Url;` at line 24) and **no new crates** (stack remains locked per CLAUDE.md).
- Applied scrubbing at **every** webhook URL sink the plan enumerated, verified against live code (line numbers matched exactly): the three dispatcher tracing spans, the reqwest error string (which Display can echo the full request URL incl. userinfo), the `WebhookError::Network` construction, the DLQ `url` persist, and a defense-in-depth re-scrub of the Network `last_error`.
- Corrected both `examples/cronduit.toml` header comment blocks (claimed "six"/"Six" jobs) to accurately list all 8 active `[[jobs]]` — comment-only, no config touched.

## Task Commits

Each task was committed atomically (code changes only):

1. **Task 1: Add strip_url_credentials helper + unit tests** — `7e33ff5` (feat) — TDD RED (compile-fail) → GREEN single commit per the quick-task flow.
2. **Task 2: Scrub at all webhook sinks** — `fb93c50` (fix) — dispatcher + retry + the one regression-test update the scrub necessitated.
3. **Task 3: Correct examples/cronduit.toml header to 8 jobs** — `f3b041b` (docs) — comment-only.

_Plan metadata (SUMMARY.md / STATE.md / PLAN.md) and `deferred-items.md` are handled by the orchestrator's docs commit, not committed here per task constraints._

## Files Created/Modified

- `src/db/mod.rs` — added `strip_url_credentials` (line 375) + 7 unit tests (`strip_url_creds_*`). Helper home justification recorded in key-decisions.
- `src/webhooks/dispatcher.rs` — scrub `cfg.url` in all three tracing spans + scrub the reqwest error string before logging and before wrapping in `WebhookError::Network`.
- `src/webhooks/retry.rs` — scrub `cfg.url` before it populates `WebhookDlqRow.url` in `write_dlq`; defense-in-depth re-scrub of the `Network` `last_error` before `truncate_error`.
- `tests/v12_webhook_dlq.rs` — `dlq_url_matches_configured_url` now asserts against `strip_url_credentials(configured_url)` (the scrub normalizes the URL via `Url` round-trip, appending the empty-path trailing slash).
- `examples/cronduit.toml` — both header comment blocks now enumerate 8 jobs (added `wh-example-unsigned` and `fire-skew-demo`).

## Exact Sinks Scrubbed (file:line, verified against live code)

| Sink | File:Line | What was scrubbed |
|------|-----------|-------------------|
| "webhook delivered" debug span | `src/webhooks/dispatcher.rs:287` (bind), emitted `url = %safe_url` | `cfg.url` |
| "webhook non-2xx" warn span | `src/webhooks/dispatcher.rs:310` (bind), emitted `url = %safe_url` | `cfg.url` |
| "webhook network error" warn span — url | `src/webhooks/dispatcher.rs:346` (bind), emitted `url = %safe_url` | `cfg.url` |
| "webhook network error" warn span — error | `src/webhooks/dispatcher.rs:347` (`err_str`), emitted `error = %err_str` | `format!("{e}")` (reqwest Display can echo full URL) |
| `WebhookError::Network` construction | `src/webhooks/dispatcher.rs` — `Err(WebhookError::Network(err_str))` | the same scrubbed `err_str` (flows to `webhook_deliveries.last_error`) |
| DLQ `url` persist | `src/webhooks/retry.rs:270` (`write_dlq`, `Some(cfg)` arm) | `cfg.url` before `WebhookDlqRow.url` |
| DLQ `last_error` (Network arm) | `src/webhooks/retry.rs:400` | `msg` before `truncate_error` (idempotent defense-in-depth) |

## Helper-placement justification

`strip_url_credentials` lives alongside `strip_db_credentials` in `src/db/mod.rs` because: (1) `url::Url` is already imported there (line 24); (2) it is the project's single existing credential-stripping precedent; (3) it is already a `pub fn` reachable from `src/webhooks/` as `crate::db::strip_url_credentials`. A separate util module would add a file and an import for zero benefit.

## Confirmation: no web / template sink exists in v1.2

Re-confirmed the plan's CONFIRMED NON-SINKS — no scrubbing was added to web handlers or templates because none surface webhook URL/last_error in v1.2:

- `grep -rni "webhook" src/web/` → no view-model renders webhook url/last_error.
- `grep -rni "webhook|deliveries|dlq|last_error" templates/` → no askama template renders them.
- Prometheus webhook metrics use the `job` label only (no url) — already safe (T-20-05).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Test follow-through] Updated `dlq_url_matches_configured_url` expectation for the new scrub**
- **Found during:** Task 2 (`just nextest`).
- **Issue:** The pre-existing test asserted byte-equality between the stored DLQ url and the raw mock-server URI (`http://127.0.0.1:PORT`). The mandated `write_dlq` scrub round-trips the URL through `url::Url`, which normalizes a userinfo-free URL by appending the empty-path trailing slash (`http://127.0.0.1:PORT/`), so the raw-equality assertion failed.
- **Fix:** Assert against `strip_url_credentials(configured_url)` — the test still proves the url is persisted, non-empty, and matches the configured target, now correctly accounting for the T-I4 scrub the plan requires.
- **Files modified:** `tests/v12_webhook_dlq.rs`
- **Commit:** `fb93c50`

### Out-of-scope discoveries (logged, NOT fixed)

13 Postgres / Docker-socket integration tests fail under `just nextest` on this machine because `/var/run/docker.sock` is absent (every one panics with `SocketNotFoundError("/var/run/docker.sock")` at testcontainer setup). These are pre-existing, environment-driven, and entirely outside the webhook scrubbing surface. Logged to `deferred-items.md`; no action taken (scope boundary). They pass in CI (which provisions Postgres + Docker).

## Verification (verbatim results)

| Command | Result |
|---------|--------|
| `just test-unit` | **PASS** — `test result: ok. 334 passed; 0 failed; 1 ignored` (incl. 7 new `strip_url_creds_*` tests) |
| `just nextest` | **PASS for all in-scope tests** — `623 tests run: 610 passed, 13 failed, 29 skipped`. All 13 failures are pre-existing Postgres/Docker-socket env failures (`*_pg`, `*_postgres`, `schema_parity`, `db_pool_postgres`, `docker_orphan_guard postgres_tests`, `v11_runnum_migration migration_03_postgres_not_null`, `v13_timeline_explain explain_uses_index_postgres`), each `SocketNotFoundError("/var/run/docker.sock")`. Every webhook test (unit + `v12_webhook_*` integration, incl. the updated `dlq_url_matches_configured_url`) passes. |
| `just check-config examples/cronduit.toml` | **PASS** — `ok: examples/cronduit.toml`, exit 0; `grep -c '^\[\[jobs\]\]'` returns 8 |
| `just fmt-check` (`cargo fmt --all -- --check`) | **PASS** — exit 0, no diff |
| `just fmt` (`cargo fmt --all`) | **PASS** — no-op, no source files reformatted |
| `just clippy` (`cargo clippy --all-targets --all-features -- -D warnings`) | **PASS** — `Finished` with `-D warnings`; no warnings/errors emitted |

## Constraint compliance

- No version bump — `Cargo.toml` remains `1.2.0` (handled separately by orchestrator).
- No new external crates (stack locked per CLAUDE.md).
- No metrics / signing / payload / control-flow changes — pure scrub insertions.
- Docs artifacts (SUMMARY.md, STATE.md, PLAN.md, deferred-items.md) NOT committed in task commits — left for the orchestrator's docs commit.
- ROADMAP.md NOT touched.
- Branch: `release/v1.2.1`, sequential (no worktree), commits made directly on the branch.

## Self-Check: PASSED

- All modified files exist: `src/db/mod.rs`, `src/webhooks/dispatcher.rs`, `src/webhooks/retry.rs`, `tests/v12_webhook_dlq.rs`, `examples/cronduit.toml`, and this SUMMARY.
- All three task commits exist in git: `7e33ff5`, `fb93c50`, `f3b041b`.
- `pub fn strip_url_credentials` present in `src/db/mod.rs`.
- Sink scrubbing present: dispatcher.rs (5 references incl. helper calls), retry.rs (2 references).
