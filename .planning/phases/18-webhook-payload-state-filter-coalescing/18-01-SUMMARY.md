---
phase: 18-webhook-payload-state-filter-coalescing
plan: 01
subsystem: infra
tags: [webhooks, dependencies, telemetry, prometheus, reqwest, hmac, ulid, base64, wiremock, rustls]

# Dependency graph
requires:
  - phase: 15-foundation-preamble
    provides: webhook delivery worker scaffolding (mpsc(1024) + dropped_total counter); rustls-only invariant; metrics describe-from-boot pattern
provides:
  - reqwest 0.13 with the rustls feature spelling (Pitfall A) on the dep tree
  - hmac 0.13, base64 0.22, ulid 1.2 as direct runtime deps
  - wiremock 0.6 as dev-dep for HttpDispatcher integration tests
  - cronduit_webhook_delivery_sent_total + cronduit_webhook_delivery_failed_total described and zero-baselined from boot
  - metrics_families_described_from_boot test extended to regression-lock the two new counter families
  - just test-unit recipe for fast unit-only feedback (referenced by 18-VALIDATION.md sampling rate)
affects: [18-02, 18-03, 18-04, 18-05, 18-06, 19-hmac-and-examples, 20-webhook-posture]

# Tech tracking
tech-stack:
  added:
    - "reqwest 0.13.3 (rustls feature; default-features=false)"
    - "hmac 0.13"
    - "base64 0.22"
    - "ulid 1.2.1"
    - "wiremock 0.6 (dev-dep)"
  patterns:
    - "describe-from-boot for new metric families (Pitfall 3 mitigation): describe_counter! + zero-baseline counter!.increment(0) inside setup_metrics OnceLock init"
    - "Pitfall-A annotation comment on dep additions: when CONTEXT documents an outdated feature spelling, leave an inline comment in Cargo.toml so future readers do not regress"
    - "regression-lock new metric families in tests/metrics_endpoint.rs::metrics_families_described_from_boot HELP/TYPE assertions"

key-files:
  created: []
  modified:
    - "Cargo.toml — appended reqwest/hmac/base64/ulid runtime deps with Pitfall-A annotation; appended wiremock under existing [dev-dependencies]"
    - "Cargo.lock — auto-updated by cargo build"
    - "src/telemetry.rs — added 2 describe_counter! blocks + 2 zero-baseline counter!.increment(0) lines for the new sent/failed counters"
    - "tests/metrics_endpoint.rs — added 4 assertions (HELP+TYPE for sent_total and failed_total) inside the existing metrics_families_described_from_boot test"
    - "justfile — added [group('test')] just test-unit recipe (cargo test --lib --all-features)"

key-decisions:
  - "Used reqwest 0.13 spelling features=[\"rustls\", \"json\"] not the 0.12 spelling [\"rustls-tls\", \"json\"] — RESEARCH § Pitfall A; CONTEXT D-20 was written before crate-currency verification"
  - "Inline Pitfall-A annotation comment in Cargo.toml above the reqwest line so anyone editing the file later sees why `rustls-tls` is forbidden"
  - "Both new webhook delivery counters stay closed-cardinality (no labels) in Phase 18 per D-17 / WH-09; Phase 20 may add a `job` label or 4xx/5xx distinction later"
  - "test-unit recipe binds to [group('test')] — separate from [group('quality')] — so just --list groups the fast feedback loop next to existing unit-test idioms without polluting the CI quality gates"

patterns-established:
  - "Phase 18 dependency-additions block: a single comment-prefixed group at the bottom of [dependencies] (above serde_json), so any subsequent webhook plan extending the dep set has an obvious anchor"
  - "When CONTEXT.md documents a wrong-version spelling, the executor must (a) use the correct spelling AND (b) leave an inline comment in the affected source file with a citation back to the canonical source (RESEARCH.md / CLAUDE.md)"

requirements-completed: [WH-01, WH-03, WH-06, WH-09]

# Metrics
duration: 5min
completed: 2026-04-29
---

# Phase 18 Plan 01: Phase 18 Foundation Deps + Telemetry Scaffolding Summary

**Added reqwest 0.13 (rustls feature, Pitfall-A annotated), hmac 0.13, base64 0.22, ulid 1.2 as runtime deps + wiremock 0.6 dev-dep; described and zero-baselined cronduit_webhook_delivery_{sent,failed}_total in setup_metrics with regression-locked test coverage; added `just test-unit` for fast unit feedback.**

## Performance

- **Duration:** ~5 min (debug build cache warm; reqwest 0.13.3 was the only new top-level chain compile)
- **Started:** 2026-04-29 (worktree-isolated)
- **Completed:** 2026-04-29
- **Tasks:** 2
- **Files modified:** 5 (Cargo.toml, Cargo.lock, src/telemetry.rs, tests/metrics_endpoint.rs, justfile)

## Accomplishments

- All five Phase 18 crates landed in the dep tree with the **correct reqwest 0.13 `rustls` feature spelling** — `cargo tree -i openssl-sys` returns empty, rustls-only invariant holds.
- `cronduit_webhook_delivery_sent_total` and `cronduit_webhook_delivery_failed_total` now render HELP/TYPE on `/metrics` from boot — operators can scrape the families before the first webhook fires (Pitfall 3 mitigation).
- The `metrics_families_described_from_boot` integration test asserts HELP+TYPE for both new counters; any future PR that drops the describe or zero-baseline fails the test.
- `just test-unit` is wired up for the per-task fast feedback loop the rest of Phase 18 will use. 218 unit tests pass in ~1.4s on a warm cache.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Phase 18 deps to Cargo.toml** — `e791efd` (chore)
2. **Task 2: Add `just test-unit` recipe + describe + zero-baseline 2 new webhook counters** — `f3f640a` (feat)

## Files Created/Modified

- **Cargo.toml** — appended five Phase 18 crates: `reqwest = { version = "0.13", default-features = false, features = ["rustls", "json"] }`, `hmac = "0.13"`, `base64 = "0.22"`, `ulid = "1.2"` under `[dependencies]`; `wiremock = "0.6"` under existing `[dev-dependencies]`. Inline Pitfall-A annotation explains why the 0.12 `rustls-tls` spelling is forbidden.
- **Cargo.lock** — auto-updated by `cargo build --workspace`. Concrete versions: reqwest 0.13.3, hmac 0.13.0, base64 (transitive), ulid 1.2.1, wiremock 0.6.5.
- **src/telemetry.rs** — added two `metrics::describe_counter!(...)` blocks for `cronduit_webhook_delivery_sent_total` and `cronduit_webhook_delivery_failed_total` immediately after the existing `dropped_total` describe; added two `metrics::counter!("...").increment(0);` zero-baselines next to the existing `dropped_total` zero-baseline. Both stay closed-cardinality (no labels) per D-17 / WH-09.
- **tests/metrics_endpoint.rs** — extended `metrics_families_described_from_boot` with four new assertions (HELP + TYPE for each new counter family). Existing assertions and ignored stubs preserved verbatim.
- **justfile** — added `[group('test')] [doc('Run unit tests only — fast feedback (cargo test --lib)')] test-unit:` recipe immediately above the existing `test:` recipe.

## Decisions Made

- **Used reqwest 0.13 `rustls` spelling, not the 0.12 `rustls-tls` spelling that CONTEXT D-20 documents.** RESEARCH § "reqwest 0.12 → 0.13 Feature-Flag Rename" + Pitfall A name this rename explicitly; the 0.12 spelling does not compile under 0.13.x. The Cargo.toml inline comment cites both sources so the reasoning is durable.
- **Both new counters stay closed-cardinality in Phase 18.** D-17 / WH-09 caps webhook metric labels at zero for Phase 18; Phase 20 may add a `job` label or 4xx/5xx differentiation when the retrying dispatcher lands. Describe text on each counter calls this out so anyone reading `/metrics` sees the v1.2 forward-roadmap inline.
- **`just test-unit` binds to `[group('test')]`, distinct from `[group('quality')]`.** Existing `nextest` and `test` recipes are CI quality gates; `test-unit` is per-task feedback. Different groups keep `just --list` readable.

## Deviations from Plan

None — plan executed exactly as written.

The single semi-deviation worth flagging is **not** a real deviation: the plan's acceptance criterion `grep -c 'rustls-tls' Cargo.toml` returns 0 conflicts with the requirement to add the Pitfall-A annotation comment, which inevitably mentions the wrong-spelling string `rustls-tls` to explain what NOT to do. The annotation was the explicit ask in Task 1's `<action>` block (and the must_haves `key_links.from->to` pattern), so the comment-line occurrence is intentional. Verified via `grep -E '^[^#].*rustls-tls' Cargo.toml` → no non-comment matches; the only `rustls-tls` lives in line 94 (the annotation explaining Pitfall A). Both intents satisfied.

## Issues Encountered

None. Build was clean on the first compile after dep addition; the only surprise was a stale "Tailwind binary not found" cargo build warning emitted by the project's `build.rs` — pre-existing, unrelated to this plan, ignorable for non-CSS work.

## User Setup Required

None — no external services configured.

## Self-Check: PASSED

**Files exist:**
- `Cargo.toml` — FOUND (modified)
- `Cargo.lock` — FOUND (modified)
- `src/telemetry.rs` — FOUND (modified)
- `tests/metrics_endpoint.rs` — FOUND (modified)
- `justfile` — FOUND (modified)

**Commits exist:**
- `e791efd` — FOUND (Task 1: Phase 18 deps)
- `f3f640a` — FOUND (Task 2: telemetry + just test-unit + test extension)

**Verification commands:**
- `just test-unit` — exit 0, 218 unit tests pass
- `cargo test --test metrics_endpoint metrics_families_described_from_boot` — exit 0, 1 passed
- `cargo build --workspace` — exit 0
- `cargo tree -i openssl-sys` — exit 101 with "did not match any packages" (rustls-only invariant: openssl-sys absent)
- `grep -c 'cronduit_webhook_delivery_sent_total' src/telemetry.rs` → 2 (1 describe + 1 zero-baseline)
- `grep -c 'cronduit_webhook_delivery_failed_total' src/telemetry.rs` → 2
- `grep -c 'cronduit_webhook_delivery_sent_total' tests/metrics_endpoint.rs` → 2 (HELP + TYPE)
- `grep -c 'cronduit_webhook_delivery_failed_total' tests/metrics_endpoint.rs` → 2

## Next Phase Readiness

- All Wave 0 prerequisites for Phase 18 are now on disk: every subsequent plan in the phase can `use reqwest::Client`, `use hmac::Mac`, `use base64::engine::general_purpose::STANDARD`, `use ulid::Ulid`, and `use wiremock::*` (in tests) without further dep work.
- The two new webhook delivery counters are observable from boot — Plans 18-02..18-06 can `metrics::counter!("cronduit_webhook_delivery_sent_total").increment(1)` and `metrics::counter!("cronduit_webhook_delivery_failed_total").increment(1)` directly, with HELP/TYPE already wired.
- `just test-unit` is the agreed per-task feedback command for the rest of Phase 18 (per 18-VALIDATION.md sampling rate).

No blockers.

---
*Phase: 18-webhook-payload-state-filter-coalescing*
*Completed: 2026-04-29*
