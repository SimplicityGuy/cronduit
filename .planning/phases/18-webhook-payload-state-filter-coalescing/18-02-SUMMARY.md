---
phase: 18-webhook-payload-state-filter-coalescing
plan: 02
subsystem: config
tags: [webhook, validation, secrecy, toml, serde, url, hmac]

# Dependency graph
requires:
  - phase: 17-operator-labels
    provides: "apply_defaults merge structure (image/network replace pattern), validate.rs sort-before-format determinism (Pitfall G), error message naming convention (offending field + value + remediation)"
  - phase: 18-webhook-payload-state-filter-coalescing/01
    provides: "Phase 18 planning artifacts (CONTEXT D-01..D-05 / D-16, RESEARCH Pattern 1+2+5, PATTERNS struct shapes, threat model T-18-04..09)"
provides:
  - "WebhookConfig struct (5 fields: url, states, secret as Option<SecretString>, unsigned, fire_every) with serde defaults"
  - "JobConfig.webhook + DefaultsConfig.webhook fields (Option<WebhookConfig>, #[serde(default)])"
  - "apply_defaults webhook merge — replace-on-collision, no type-gate, mirrors image/network pattern (NOT labels HashMap union)"
  - "VALID_WEBHOOK_STATES const enumerating canonical RunFinalized status values"
  - "check_webhook_url validator: rejects unparsable URLs and non-http/https schemes"
  - "check_webhook_block_completeness validator: enforces non-empty/valid states, secret xor unsigned, non-negative fire_every, non-empty resolved secret (Pitfall H)"
  - "Both validators wired into run_all_checks per-job loop"
  - "19 new unit tests across the 3 modified files"
affects:
  - "18-03 (`cronduit check` UAT — exercises these validators end-to-end)"
  - "18-04 (dispatcher — reads WebhookConfig.url/states/secret/unsigned/fire_every)"
  - "18-05 (bin layer wire-up — builds Arc<HashMap<i64, WebhookConfig>> from validated JobConfig.webhook)"
  - "18-06 (filter/coalescing — reads fire_every and the canonical states list)"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Bin-layer config bypass: a TOML config field (webhook) intentionally NOT propagated through DockerJobConfig / serialize_config_json / compute_config_hash — the 5-layer parity invariant from Phase 17 LBL applies to docker-execution surface only. Webhook config flows to the dispatcher via a bin-layer per-job HashMap built at startup, not via config_json round-trips."
    - "Single-block replace-on-collision merge: `webhook` is a single inline TOML block; `apply_defaults` mirrors image/network/volumes/timeout/delete per-field replace, NOT the labels HashMap union. Per-job webhook fully discards defaults webhook on collision (operators wanting partial override must spell out every field)."
    - "secret xor unsigned signing-intent gate: distinguishes accidental empty config (neither) from ambiguous double-spec (both); each branch produces a remediation message naming both legal endpoints."

key-files:
  created: []
  modified:
    - "src/config/mod.rs (WebhookConfig struct, JobConfig.webhook, DefaultsConfig.webhook, default_webhook_states, default_fire_every helpers, 4 parse tests)"
    - "src/config/defaults.rs (apply_defaults webhook merge block, 4 merge tests, mechanical webhook: None additions to existing fixtures)"
    - "src/config/validate.rs (VALID_WEBHOOK_STATES const, check_webhook_url + check_webhook_block_completeness fns, run_all_checks wiring, 11 tests)"
    - "src/config/hash.rs (mechanical webhook: None additions to fixtures only — no hash logic change)"
    - "src/scheduler/sync.rs (mechanical webhook: None additions to fixtures only — no serialize logic change)"
    - "tests/scheduler_integration.rs (mechanical webhook: None addition to test fixture)"

key-decisions:
  - "WebhookConfig.secret is Option<SecretString> not Option<String> — Debug/Display scrub the value, T-18-07 Information Disclosure mitigation."
  - "Webhook merge applies to ALL job types (no type-gate on `is_non_docker`) — webhooks fire on RunFinalized for command/script/docker alike per RESEARCH § Pattern 5 adaptation note."
  - "Webhook config STOPS at the bin layer — does NOT enter DockerJobConfig / serialize_config_json / compute_config_hash. The 5-layer parity invariant from Phase 17 LBL applies to docker-execution surface only (RESEARCH Open Q 1)."
  - "VALID_WEBHOOK_STATES = ['success', 'failed', 'timeout', 'stopped', 'cancelled', 'error'] — canonical RunFinalized status values from src/scheduler/run.rs:315-322. Operators filter on this exact set; unknown values rejected at LOAD with a sorted offending list (Pitfall G)."
  - "Pitfall H guard: `secret.expose_secret().is_empty()` rejected at LOAD time. interpolate.rs catches unset env vars (MissingVar) but `${WEBHOOK_SECRET}=\"\"` (set-but-empty) would silently sign HMACs with an empty key — distinct LOAD-time failure path."
  - "secret xor unsigned: both-set produces 'unsigned=true skips signing OR secret signs deliveries' remediation; neither-set produces 'set secret OR set unsigned=true' remediation. Each branch names both legal endpoints to avoid forcing a debug round-trip."

patterns-established:
  - "Inline-block replace-on-collision merge: when a config field is a single inline block (not a HashMap), use the same shape as `image`/`network` — `if job.field.is_none() && let Some(v) = &defaults.field { job.field = Some(v.clone()); }`. The labels HashMap union pattern (lines 154-176 of defaults.rs) is the WRONG analog for inline blocks."
  - "5-field validator function: combine independent assertions (D-01 aggregation) in a single `check_*_block_completeness` fn that pushes one ConfigError per violation. Sort offending lists before format (Pitfall G — HashMap iteration is non-deterministic in Rust)."
  - "SecretString-aware error formatting: error messages reference `secret.is_empty()` boolean ONLY — never `secret.expose_secret()`. Doing so would defeat the V8 Data Protection scrubbing the SecretString wrapper provides."

requirements-completed: [WH-01]

# Metrics
duration: 22min
completed: 2026-04-29
---

# Phase 18 Plan 02: Webhook config struct + apply_defaults merge + LOAD-time validators Summary

**WebhookConfig (5 fields, secret as Option<SecretString>) on JobConfig and DefaultsConfig, replace-on-collision apply_defaults merge with no type-gate, plus check_webhook_url + check_webhook_block_completeness LOAD-time validators wired into run_all_checks**

## Performance

- **Duration:** 22 min
- **Started:** 2026-04-29T20:59:35Z
- **Completed:** 2026-04-29T21:22:04Z
- **Tasks:** 3
- **Files modified:** 6 (3 substantive + 3 mechanical fixture updates)

## Accomplishments

- WebhookConfig struct with all 5 fields + serde defaults (`states` defaults to `["failed", "timeout"]`; `fire_every` defaults to `1`); secret wrapped in `SecretString` for Debug/Display scrubbing.
- JobConfig.webhook + DefaultsConfig.webhook fields with `Option<WebhookConfig>`, `#[serde(default)]` on each.
- apply_defaults webhook merge — no type-gate, replace-on-collision (mirrors image/network NOT labels HashMap union); honors `use_defaults = false` short-circuit.
- VALID_WEBHOOK_STATES const + check_webhook_url + check_webhook_block_completeness validators rejecting every D-04 violation including Pitfall H (empty resolved secret).
- 19 new unit tests (4 parse + 4 merge + 11 validator) passing; lib test count rose from 218 → 237.
- Webhook config flows through the bin layer only — DockerJobConfig / serialize_config_json / compute_config_hash all unchanged, preserving the Phase 17 LBL 5-layer parity invariant for docker-execution surface.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add WebhookConfig struct + JobConfig.webhook + DefaultsConfig.webhook fields with serde defaults** — `7f60798` (feat)
2. **Task 2: Extend apply_defaults with webhook merge (replace-on-collision)** — `a1c0b24` (feat)
3. **Task 3: Add check_webhook_url + check_webhook_block_completeness validators wired into run_all_checks** — `e4ef91a` (feat)

**Plan deferred-items log:** `d8a669e` (docs)

_Note: Each task was a single GREEN-after-RED commit. Tests authored before implementation per `tdd="true"` task tags; RED was confirmed via compile errors / runtime panics before implementing GREEN._

## Files Created/Modified

- `src/config/mod.rs` — `WebhookConfig` struct (5 fields including `secret: Option<SecretString>`); `JobConfig.webhook` + `DefaultsConfig.webhook` fields; `default_webhook_states()` + `default_fire_every()` helpers; 4 parse tests in `mod tests` (newly added).
- `src/config/defaults.rs` — `apply_defaults` extended with the webhook merge block (lines after the labels merge, before the random_min_gap NOTE); 4 webhook merge tests appended to existing test module; mechanical `webhook: None` additions to `empty_job()`, `full_defaults()`, and 9 in-test `JobConfig`/`DefaultsConfig` literals.
- `src/config/validate.rs` — `VALID_WEBHOOK_STATES` const after `LABEL_KEY_RE`; `check_webhook_url` + `check_webhook_block_completeness` fns before `check_duplicate_job_names`; both wired into `run_all_checks` per-job loop after `check_label_key_chars`; 11 unit tests appended including `make_webhook_job` helper; mechanical `webhook: None` addition to `stub_job`.
- `src/config/hash.rs` — Mechanical `webhook: None` additions to `mk_job()`, `mk_docker_job()`, and 5 `DefaultsConfig` literals in `hash_stable_across_defaults_merge`. No hash logic change — webhook is excluded from `compute_config_hash` by design (config_json bypass).
- `src/scheduler/sync.rs` — Mechanical `webhook: None` additions to the test-helper `JobConfig` fixture (line 262) and the inline `Config { jobs: vec![JobConfig {...}] }` (line 432). No serialize logic change.
- `tests/scheduler_integration.rs` — Mechanical `webhook: None` addition to the test-helper fixture.
- `.planning/phases/18-webhook-payload-state-filter-coalescing/deferred-items.md` — Created. Logs a pre-existing flaky env-var test in `tests/v12_labels_interpolation.rs` (passes single-threaded; concurrent `unsafe std::env::set/remove_var` between two sibling tests). Out of scope per scope-boundary rule (changes were purely additive on `webhook`, do not touch the labels code path).

## Decisions Made

- **Inline-block merge analog: image/network/volumes/timeout/delete (replace), NOT labels (HashMap union).** `webhook` is a single inline TOML block; collision semantics for replace are the only sane choice — partial-field-merge would mean operators silently inherit a secret from defaults while overriding the URL, which violates the principle of least surprise and would create T-18-05 / T-18-06 ambiguity at the validator boundary.
- **No type-gate on apply_defaults webhook merge.** Unlike LBL-04 which gates `labels` on docker jobs, webhooks fire on RunFinalized for command/script/docker alike. Adding a gate here would silently drop defaults webhooks for command jobs, masking the intended behavior. The dispatcher reads webhook config from a bin-layer per-job HashMap; it never touches DockerJobConfig.
- **5-layer parity exemption for webhook (RESEARCH Open Q 1).** The 5-layer parity invariant from Phase 17 LBL applies ONLY to docker-execution surface. Webhook config does NOT enter `DockerJobConfig` / `serialize_config_json` / `compute_config_hash`. This is documented inline on `JobConfig.webhook` (the field-level doc comment names all four layers it does NOT propagate to).
- **Pitfall H empty-secret guard at LOAD time.** `interpolate.rs` catches unset env vars via MissingVar, but `${WEBHOOK_SECRET}=""` (set-but-empty) is a distinct silent failure that would sign HMACs with an empty key. `check_webhook_block_completeness` rejects it via `secret.expose_secret().is_empty()` — the only call to `expose_secret()` in the validator, used for a boolean check (no value leak).
- **Two distinct error messages for the secret/unsigned xor branch.** Both-set ("set unsigned=true to skip signing OR set secret to sign deliveries") and neither-set ("set secret OR set unsigned=true") produce different remediation text — operators see the exact fix without re-reading the docs.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Mechanical `webhook: None` additions to existing fixtures**
- **Found during:** Task 1 (immediately after adding `JobConfig.webhook` / `DefaultsConfig.webhook` fields)
- **Issue:** The struct field additions broke compilation of every existing test fixture that constructed `JobConfig { ... }` or `DefaultsConfig { ... }` literals. The plan called this out for `defaults.rs`, but the same fix was needed in `hash.rs` (6 fixtures), `validate.rs` (1 fixture), `sync.rs` (2 fixtures), and `tests/scheduler_integration.rs` (1 fixture). All edits are pure mechanical `webhook: None` additions — no logic change, just keeping existing tests compiling.
- **Fix:** Added `webhook: None` to every existing `JobConfig { ... }` and `DefaultsConfig { ... }` literal across the workspace. Confirmed by `cargo build --workspace` exit-0 and the existing 218 lib tests still passing (now 237 with the 19 new tests).
- **Files modified:** src/config/hash.rs, src/config/validate.rs (stub_job only), src/scheduler/sync.rs, tests/scheduler_integration.rs
- **Verification:** `cargo test --lib --all-features` → 237 passed, 0 failed. No existing test broken.
- **Committed in:** 7f60798 (Task 1 commit, alongside the new struct + parse tests)

---

**Total deviations:** 1 auto-fixed (Rule 3 — blocking compile error from struct field addition)
**Impact on plan:** Mechanical-only; no design/scope change. The plan's Task 2 action did call this out for `defaults.rs` but did not enumerate `hash.rs`/`sync.rs`/`tests/`; the fix is pure compile-error remediation.

## Issues Encountered

- **Pre-existing flaky integration test (out of scope).** `tests/v12_labels_interpolation.rs` contains two `#[tokio::test]` functions that share env-var `TEAM` via `unsafe { std::env::remove_var/set_var }`. Concurrent execution lets one test's mutation leak into the other's body, producing intermittent failures under `cargo test` (passes deterministically with `--test-threads=1`). This is pre-existing on `main` — Plan 18-02 changes are purely additive (a new `webhook` field; a new validator that does not touch the labels code path) and cannot have caused it. Logged in `.planning/phases/18-webhook-payload-state-filter-coalescing/deferred-items.md` for a follow-up serialization fix.

## User Setup Required

None — no external service configuration required. Validators run at `cronduit check` / `cronduit run` startup; operators see errors before the daemon attempts to deliver.

## Next Phase Readiness

- **WH-01 surface ready for downstream consumers.**
  - Plan 18-04 (dispatcher) can read `WebhookConfig.{url, states, secret, unsigned, fire_every}` directly.
  - Plan 18-05 (bin layer) can build `Arc<HashMap<i64, WebhookConfig>>` from `cfg.jobs[i].webhook` (already merged with defaults; already validated).
  - Plan 18-06 (filter/coalescing) can rely on `fire_every >= 0` and `states ⊆ VALID_WEBHOOK_STATES` invariants — no defensive re-validation needed downstream.
- **No blockers.** All 237 lib tests pass; the 19 new unit tests cover every D-04 branch + Pitfall G ordering + Pitfall H empty-secret + signed/unsigned acceptance paths.

---
*Phase: 18-webhook-payload-state-filter-coalescing*
*Completed: 2026-04-29*

## Self-Check: PASSED

Files claimed:
- src/config/mod.rs — FOUND (modified, contains WebhookConfig struct, helpers, 4 tests)
- src/config/defaults.rs — FOUND (modified, contains webhook merge block + 4 tests)
- src/config/validate.rs — FOUND (modified, contains VALID_WEBHOOK_STATES, 2 fns, run_all_checks wiring, 11 tests)
- src/config/hash.rs — FOUND (modified, mechanical webhook: None only)
- src/scheduler/sync.rs — FOUND (modified, mechanical webhook: None only)
- tests/scheduler_integration.rs — FOUND (modified, mechanical webhook: None only)
- .planning/phases/18-webhook-payload-state-filter-coalescing/deferred-items.md — FOUND (created)

Commits claimed:
- 7f60798 — FOUND
- a1c0b24 — FOUND
- e4ef91a — FOUND
- d8a669e — FOUND
