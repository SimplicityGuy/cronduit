---
phase: 18-webhook-payload-state-filter-coalescing
plan: 06
subsystem: webhooks
tags: [uat, webhooks, justfile, examples, maintainer-validation]
requires:
  - 18-05 (HttpDispatcher fully wired into the scheduler — UAT exercises real
    deliveries against a real receiver)
  - 18-04 (HttpDispatcher with sign-then-send flow + Standard Webhooks v1 headers)
  - 18-03 (15-/16-field payload schema)
  - 18-02 (config validator — WEBHOOK_SECRET empty-string rejection used by S6)
  - 18-01 (config schema — webhook block on [defaults] + [[jobs]])
provides:
  - "4 new just recipes for Phase 18 UAT (3 uat-webhook-* + 1 api-run-now helper)"
  - "examples/webhook_mock_server.rs — local loopback receiver for hand-validation"
  - "3 webhook config variants in examples/cronduit.toml (signed/unsigned/fire_every=0)"
  - "18-HUMAN-UAT.md — 7 maintainer-validated scenarios, all unchecked"
affects:
  - "justfile (3 new uat-webhook-* recipes + 1 api-run-now cross-cutting helper)"
  - "examples/cronduit.toml (3 new webhook examples appended; existing jobs untouched)"
  - "examples/webhook_mock_server.rs (NEW)"
  - ".planning/phases/18-webhook-payload-state-filter-coalescing/18-HUMAN-UAT.md (NEW)"
tech-stack:
  added:
    - "tokio (already a workspace dep) — used by the example mock receiver"
    - "chrono (already a workspace dep) — RFC3339 timestamp on each logged request"
  patterns:
    - "recipe-calls-recipe (uat-webhook-fire body delegates to `just api-run-now`)"
    - "Connection: close response framing for reqwest keep-alive safety"
    - "header-end + Content-Length aware HTTP/1.1 reader (loop until both seen)"
key-files:
  created:
    - "examples/webhook_mock_server.rs"
    - ".planning/phases/18-webhook-payload-state-filter-coalescing/18-HUMAN-UAT.md"
  modified:
    - "justfile"
    - "examples/cronduit.toml"
decisions:
  - "Did NOT modify the existing `metrics-check` recipe per the plan's explicit DO NOT modify constraint, even though Scenario 7 expects webhook counter lines to surface (see Deferred Issues below)."
  - "Reordered checkbox layout in 18-HUMAN-UAT.md from `Maintainer-validated: [ ]` (plan verbatim text) to `[ ] Maintainer-validated` (matches plan's own acceptance grep regex AND aligns with the 17-HUMAN-UAT.md precedent)."
  - "Added `use_defaults = false` + explicit `timeout = \"5m\"` to the three new wh-example-* jobs because [defaults].labels (Watchtower) merges into every job; the LBL-04 validator (Phase 17) rejects labels on non-docker (command) jobs. This mirrors the pattern already used by echo-timestamp / http-healthcheck / disk-usage in this same file."
metrics:
  duration: "~25 min"
  completed: 2026-04-29
  tasks_completed: 2
  files_changed: 4
  loc_added: 330
  loc_removed: 1
---

# Phase 18 Plan 06: Maintainer UAT Scaffolding Summary

UAT scaffolding for Phase 18 webhook delivery — 4 new just recipes (3 `uat-webhook-*` + 1 cross-cutting `api-run-now` helper), a ~110-line cargo-runnable mock receiver behind `just uat-webhook-mock`, three documented webhook variants in `examples/cronduit.toml` (signed / unsigned / `fire_every = 0`), and a 7-scenario `18-HUMAN-UAT.md` runbook — all of it awaiting maintainer hand-validation per project memory `feedback_uat_user_validates.md`.

## What Was Built

### 1. Four new `just` recipes (justfile)

| Recipe | Group | Purpose |
|--------|-------|---------|
| `api-run-now JOB_NAME` | `api` | Cross-cutting helper — wraps `curl -sf -X POST /api/jobs/{NAME}/run-now`. The SOLE place curl appears in any new Phase 18 recipe. |
| `uat-webhook-mock` | `uat` | Starts the cargo example receiver on `127.0.0.1:9999`; logs to stdout + `/tmp/cronduit-webhook-mock.log`. |
| `uat-webhook-fire JOB_NAME` | `uat` | UAT-callable; body is `@just api-run-now {{JOB_NAME}}` — zero raw curl. |
| `uat-webhook-verify` | `uat` | `tail -n 30 /tmp/cronduit-webhook-mock.log` for maintainer hand-validation. |

The recipe-calls-recipe pattern (`uat-webhook-fire` → `api-run-now`) keeps the UAT-callable surface free of raw shell per **D-25** / project memory `feedback_uat_use_just_commands.md`.

Pre-existing recipes (`dev`, `check-config`, `metrics-check`, `health`, `ci`, `openssl-check`) are reused by the UAT runbook unchanged.

### 2. `examples/webhook_mock_server.rs` (NEW, ~110 lines)

A standalone tokio binary listening on `127.0.0.1:9999`. Cargo's default example discovery picks it up from `examples/*.rs`; no `Cargo.toml` change needed. Verified built clean: `cargo build --example webhook_mock_server` → `target/debug/examples/webhook_mock_server` produced.

Key design points:

- **`Connection: close` response framing.** Forces request-per-connection on the reqwest dispatcher side — reqwest cannot reuse a stale TCP stream between deliveries. The plan's threat T-18-35 explicitly calls this out.
- **Loop-based reader.** Reads until headers + Content-Length body are received OR the client closes. Includes a 1 MiB safety cap to prevent unbounded memory growth on a misbehaving client.
- **Logs each request twice** — stderr (visible in `just uat-webhook-mock` terminal) AND `/tmp/cronduit-webhook-mock.log` (consumed by `just uat-webhook-verify`).
- **Loopback-only.** Comment block flags this as NOT a production HTTP/1.1 implementation — for local maintainer UAT validation only.

### 3. `examples/cronduit.toml` — 3 new webhook variants

Appended after the existing 6-job set (no existing job modified):

| Job | Variant | Demonstrates |
|-----|---------|--------------|
| `wh-example-signed` | Signed delivery, default state filter (failed/timeout), default coalescing (fire_every = 1) | WH-01, WH-03, WH-09, default behavior |
| `wh-example-unsigned` | `unsigned = true`, custom states list `[failed, stopped]` | D-05 (cronduit extension to Standard Webhooks v1) |
| `wh-example-fire-every-zero` | Narrow filter `[timeout]`, `fire_every = 0` (always fire, legacy mode) | WH-06 / D-16 |

`use_defaults = false` + explicit `timeout = "5m"` is set on all three because `[defaults].labels` (Watchtower) is inherited by every job; the LBL-04 validator from Phase 17 rejects labels on non-docker jobs. This mirrors the pre-existing `echo-timestamp` / `http-healthcheck` / `disk-usage` pattern in the same file.

`${WEBHOOK_SECRET}` interpolation appears 3 times (`wh-example-signed`, `wh-example-fire-every-zero`, plus a header comment); `unsigned = true` appears once.

Validation: `WEBHOOK_SECRET=test-secret-shh just check-config examples/cronduit.toml` → `ok: examples/cronduit.toml`.

### 4. `18-HUMAN-UAT.md` — 7 maintainer-validated scenarios

| # | Scenario | Proves |
|---|----------|--------|
| 1 | Signed delivery with default state filter | WH-01 / WH-03 / WH-09 (3 headers + 16-field payload) |
| 2 | Unsigned delivery omits webhook-signature header | WH-03 / D-05 |
| 3 | Default coalescing (fire_every = 1) | WH-06 / D-12 / D-15 / D-16 |
| 4 | State filter excludes success | WH-01 / D-04 |
| 5 | `fire_every = 0` legacy mode | WH-06 / D-16 |
| 6 | `${WEBHOOK_SECRET}` env-var interpolation (unset / empty / set) | WH-01 / D-03 / Pitfall H |
| 7 | Metrics families (`cronduit_webhook_delivery_*`) | Phase 15/18 telemetry |

Every step references only `just` recipes (`uat-webhook-mock`, `uat-webhook-fire`, `uat-webhook-verify`, `api-run-now`, `dev`, `metrics-check`, `check-config`, `ci`, `openssl-check`). 25 total `just <recipe>` references across the document; well above the plan's `>= 12` threshold.

All 7 boxes are `[ ] Maintainer-validated` (verified: 7 unchecked, 0 pre-checked). The header carries a verbatim warning that Claude does NOT mark UAT passed — checkboxes are explicitly maintainer-only.

## UAT Status — Awaiting Maintainer Validation

**Per project memory `feedback_uat_user_validates.md` (D-26):** Claude has built and committed the UAT artifacts but has NOT run the scenarios end-to-end. The 7 checkboxes in `18-HUMAN-UAT.md` are all `[ ]` and remain so until the maintainer (Robert) runs each scenario locally and flips them to `[x]`.

**For the maintainer — recipes to run, in order:**

1. **`just ci`** — full CI gate as a sanity check before starting UAT.
2. **`just openssl-check`** — confirms the rustls invariant.
3. **`WEBHOOK_SECRET=test-secret-shh just check-config examples/cronduit.toml`** — validates the extended config parses with all three webhook variants.
4. **`just uat-webhook-mock`** (terminal A, leave running) — starts the loopback receiver.
5. **`just dev`** (terminal B, with `WEBHOOK_SECRET` exported) — starts cronduit against `examples/cronduit.toml`.
6. **`just uat-webhook-fire wh-example-signed`** (terminal C) — Scenario 1.
7. **`just uat-webhook-verify`** (terminal C) — inspect the mock log.

Then proceed scenario-by-scenario per `18-HUMAN-UAT.md`.

**Each recipe should produce:**

| Recipe | Expected output |
|--------|-----------------|
| `just ci` | exit 0 |
| `just openssl-check` | `OK: no openssl-sys in dep tree (...)` exit 0 |
| `just check-config examples/cronduit.toml` | `ok: examples/cronduit.toml` exit 0 |
| `just uat-webhook-mock` | `[webhook-mock] listening on http://127.0.0.1:9999/  (log: /tmp/cronduit-webhook-mock.log)` then runs |
| `just uat-webhook-fire <JOB>` | `▶ UAT: ...` then `OK: triggered run-now for <JOB>` |
| `just uat-webhook-verify` | last 30 lines of `/tmp/cronduit-webhook-mock.log` |
| `just metrics-check` | `cronduit_scheduler_up 1` + `cronduit_runs_total{...}` lines |

## Deferred Issues

### 1. Scenario 7 may not surface webhook counter lines via `just metrics-check`

The plan's `<what-built>` block for Scenario 7 says:

> Expected: stdout/grep includes:
> - `# HELP cronduit_webhook_delivery_dropped_total ...`
> - `# HELP cronduit_webhook_delivery_sent_total ...`
> - `# HELP cronduit_webhook_delivery_failed_total ...`

…but the existing `metrics-check` recipe greps for `'^cronduit_scheduler_up\b|^cronduit_runs_total\b'`, which (a) excludes `# HELP` / `# TYPE` lines (those start with `#`, not `cronduit_`) and (b) doesn't include the `cronduit_webhook_*` family.

The plan explicitly says **DO NOT modify any existing recipe**, so the `metrics-check` recipe was left untouched. As a result, Scenario 7 as written will only show scheduler liveness + run counts. The webhook families ARE eagerly described at boot (verified via grep of `src/telemetry.rs` lines 112/123/129) and would surface from a raw `curl /metrics` call — but raw curl in UAT is forbidden per project memory.

**Recommendation for the maintainer:** Either (a) accept Scenario 7 as a "scheduler liveness sanity check" and visually trust that the eager descriptions exist (verifiable by inspection of `src/telemetry.rs` boot code), or (b) file a follow-up plan to widen `metrics-check` to also accept the `cronduit_webhook_delivery_*` family. Option (b) is a one-line regex change; not done here because the plan locked the existing recipe.

### 2. Plan's raw-curl check returns 3 (pre-existing recipes)

The plan's `<acceptance_criteria>` includes:

```
grep -nE '^\s*curl ' justfile | grep -v 'api-run-now' | wc -l
```

This returns **3** rather than 0 because the pre-existing `tailwind` (line 58), `health` (line 427), and `metrics-check` (line 434) recipes invoke curl directly. These pre-date Phase 18 and are themselves wrapper recipes (the curl is wrapped in a `just` recipe — exactly the pattern the plan's intent endorses for `api-run-now`).

Plan **intent** is satisfied: zero raw curl appears in the new UAT-callable recipes (`uat-webhook-mock`, `uat-webhook-fire`, `uat-webhook-verify`). Verified via:

```
awk '/^uat-webhook-/{flag=1; next} /^[a-zA-Z]/{flag=0} flag' justfile | grep -c 'curl '
# → 0
```

The acceptance regex was overly strict — it didn't account for already-existing wrapper recipes that themselves invoke curl exactly once. No action taken.

## Deviations from Plan

### Deviation 1 — `[ ] Maintainer-validated` checkbox order (Rule 3, blocking)

The plan's `<what-built>` block uses `Maintainer-validated: [ ]` (label-then-bracket) per scenario, but the same plan's `<acceptance_criteria>` requires:

```
grep -c '\[ \] Maintainer-validated' ... returns at least 7
```

The acceptance grep is bracket-then-label. The two are incompatible as written.

**Resolution:** Reordered all 7 scenario checkboxes to `[ ] Maintainer-validated` (bracket-first). This:

1. Matches the plan's own acceptance regex (verified: 7 unchecked, 0 pre-checked).
2. Matches the established `17-HUMAN-UAT.md` precedent (uses bracket-first checkbox lines).
3. Stays semantically equivalent — the maintainer still flips `[ ]` → `[x]`.

### Deviation 2 — `use_defaults = false` + `timeout` on the new wh-example-* jobs (Rule 2, scope-blocking)

The plan's `<action>` for Sub-step B did not include `use_defaults = false` on the three webhook examples. But `examples/cronduit.toml` already carries `[defaults].labels = { "com.centurylinklabs.watchtower.enable" = "false" }` from Phase 17 / SEED-001 — that label inherits into every command-type job, and the LBL-04 validator (Phase 17) rejects labels on non-docker jobs.

Without `use_defaults = false` the three new command-type jobs would fail validation immediately on `just check-config`. The pre-existing command/script jobs (`echo-timestamp`, `http-healthcheck`, `disk-usage`) already use this pattern and document the rationale in their comment block.

**Resolution:** Added `use_defaults = false` + explicit `timeout = "5m"` to all three new webhook examples, mirroring the existing pattern in the same file. Documented in the inline comment header of the new section. Validated: `WEBHOOK_SECRET=test-secret-shh just check-config examples/cronduit.toml` exits 0.

### Auth gates encountered

None. UAT artifacts are pure scaffolding — no external auth needed during build/commit. The maintainer's UAT run will exercise `${WEBHOOK_SECRET}` interpolation per Scenario 6, but that's the maintainer's local shell, not a Claude-handled gate.

## Verification Performed

- `cargo build --example webhook_mock_server` → exit 0; binary at `target/debug/examples/webhook_mock_server` (2.9 MB)
- `WEBHOOK_SECRET=test-secret-shh just check-config examples/cronduit.toml` → `ok: examples/cronduit.toml` exit 0
- `just --list | grep -E '(uat-webhook|api-run-now)'` → 4 recipes registered (`api-run-now`, `uat-webhook-fire`, `uat-webhook-mock`, `uat-webhook-verify`)
- `grep -c 'webhook = {' examples/cronduit.toml` → 3
- `grep -F '${WEBHOOK_SECRET}' examples/cronduit.toml | wc -l` → 3 (>= 2 required)
- `grep -F 'unsigned = true' examples/cronduit.toml` → 1
- `grep -c '\[ \] Maintainer-validated' .planning/phases/18-webhook-payload-state-filter-coalescing/18-HUMAN-UAT.md` → 7
- `grep -c '\[x\] Maintainer-validated' .planning/phases/18-webhook-payload-state-filter-coalescing/18-HUMAN-UAT.md` → 0
- `grep -E -c 'just (uat-webhook-mock|uat-webhook-fire|uat-webhook-verify|api-run-now|dev|metrics-check|check-config|ci|openssl-check)' .../18-HUMAN-UAT.md` → 25 (>= 12 required)
- `grep -c 'curl ' inside uat-webhook-* recipe bodies` → 0
- `grep -F 'Connection: close' examples/webhook_mock_server.rs` → 3 (header in static response + comment + comment)
- `grep -F 'NOT a production-grade HTTP/1.1' examples/webhook_mock_server.rs` → 1

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | d494f1d | feat(18-06): add Phase 18 UAT scaffolding (recipes + mock receiver + config) |
| 2 | 5dcd304 | docs(18-06): author 18-HUMAN-UAT.md (7 maintainer-validated scenarios, all unchecked) |

## Self-Check: PASSED

- [x] `examples/webhook_mock_server.rs` exists and compiles
- [x] `examples/cronduit.toml` carries 3 webhook variants
- [x] 4 new just recipes are registered (3 uat-webhook-* + 1 api-run-now)
- [x] `uat-webhook-fire` body delegates to `just api-run-now {{JOB_NAME}}` (zero raw curl)
- [x] `18-HUMAN-UAT.md` exists with 7 unchecked scenarios
- [x] Every scenario references only `just` recipes (no raw curl/cargo/docker on the UAT-callable surface)
- [x] All claimed commits exist in git log (`d494f1d`, `5dcd304`)
- [x] No modifications to STATE.md, ROADMAP.md, or any other shared orchestrator artifact
- [x] No modifications to sibling-owned files (`src/cli/run.rs`, `tests/v12_webhook_*.rs` are sibling 18-05's)
