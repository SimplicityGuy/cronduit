# Phase 20 — Maintainer UAT Runbook

**Phase:** 20 — Webhook SSRF/HTTPS Posture + Retry/Drain + Metrics — rc.1
**Status:** **pending — maintainer-validation required.** Plan 20-08 ships this artifact unticked; the maintainer runs each scenario from a fresh terminal and flips `[ ]` → `[x]` themselves. Per project memory `feedback_uat_user_validates.md`, Claude does NOT mark these scenarios passed; per project memory `feedback_uat_use_just_commands.md`, every step references a `just` recipe — never raw `curl`/`cargo`/`docker`.
**Prerequisite:** Phases 15–19 merged + Phase 20 plans 01–07 merged on the working branch + CI matrix green. This runbook gates the `v1.2.0-rc.1` tag cut (Plan 20-09 reads the sign-off block below).
**Requirements covered:** WH-05 (retry chain + DLQ), WH-07 (HTTPS-required validator), WH-10 (drain on shutdown), WH-11 (metrics family). Plus D-19 (validator decision), D-26 (preserved P15 saturation counter), D-38 (rustls invariant).

## Prerequisites

| Prereq | Recipe | Notes |
|--------|--------|-------|
| Workspace builds clean | `just ci` | Full CI gate: fmt + clippy + openssl-check + nextest + schema-diff + image |
| rustls invariant holds | `just uat-webhook-rustls-check` | Wraps `just openssl-check`; `cargo tree -i openssl-sys` empty across native + arm64-musl + amd64-musl (D-38) |
| Webhook-configured job available | edit `examples/cronduit.toml` | Uncomment `wh-example-signed` (use `secret = "${WEBHOOK_SECRET}"`) and ensure its `webhook.url` points at `http://127.0.0.1:9999/<path>` so the helper mocks intercept |
| Example config validates | `WEBHOOK_SECRET=test-secret-shh just check-config examples/cronduit.toml` | Set the env var first; the validator will accept loopback `http://` per D-19 |

## Scenarios

Each scenario lists: the goal, the recipe(s) to run, the expected output, and a checkbox for the maintainer.

### Scenario 1 — HTTPS-required validator rejects public HTTP

**What this proves:** WH-07 / D-19 — the LOAD-time validator (`src/config/validate.rs::check_webhook_url`) rejects any `webhook.url` whose scheme is `http://` and whose host is NOT in the loopback / RFC1918 / `fd00::/8` allowlist.

- **Recipe:** `just uat-webhook-https-required`
- **Steps:**
  1. From a clean working tree, run `just uat-webhook-https-required`.
  2. The recipe writes a minimal `cronduit.toml` to `/tmp/cronduit-bad-webhook-XXXXXX.toml` with `webhook = { url = "http://example.com/hook" }` and runs `cargo run --quiet -- check <TMP>`.
  3. Confirm the recipe exits 0 (the recipe inverts the cronduit-check exit code: it succeeds when cronduit-check FAILS, which is the desired behavior).
  4. Confirm the recipe's last line reads `▶ PASS: 'cronduit check' rejected http://example.com with non-zero exit.`
  5. (Optional) Confirm the cronduit-check error message mentions `requires HTTPS for non-loopback / non-RFC1918` or equivalent (D-19 wording).
- **Pass criteria:** Recipe exits 0; PASS line printed; cronduit-check rejected the bad config.

[ ] Maintainer-validated

### Scenario 2 — HTTPS-required validator accepts loopback HTTP with INFO log

**What this proves:** WH-07 / D-19 (positive case) — the validator allows `http://127.0.0.1:9999/...` and logs an INFO line at boot indicating the host classification.

- **Recipe:** `just dev` (with `examples/cronduit.toml` containing a webhook job pointed at `http://127.0.0.1:9999/...`)
- **Steps:**
  1. Confirm `examples/cronduit.toml` has a webhook-configured job with `webhook.url = "http://127.0.0.1:9999/..."` (the shipped `wh-example-unsigned` job qualifies).
  2. In terminal A: `export WEBHOOK_SECRET=test-secret-shh`
  3. In terminal A: `just dev`
  4. Read the boot logs for an INFO line like `webhook URL accepted on local net` (or equivalent D-19 wording) referencing the loopback classification.
  5. Confirm the daemon does NOT exit at boot.
- **Pass criteria:** Daemon boots clean; INFO log line confirms loopback acceptance.

[ ] Maintainer-validated

### Scenario 3 — 3-attempt retry chain on a 500-returning receiver

**What this proves:** WH-05 / Success Criterion 2 — `RetryingDispatcher` over `HttpDispatcher` produces 3 attempts at t=0 / t≈30s / t≈300s with full-jitter (0.8×–1.2× per delay), and on exhaustion writes a `webhook_deliveries` row with `attempts=3, dlq_reason='http_5xx'`.

- **Recipes:** `just uat-webhook-mock-500` (terminal A) + `just dev` (terminal B) + `just uat-webhook-retry <JOB>` (terminal C) + `just uat-webhook-dlq-query` (terminal C, after the chain completes)
- **Steps:**
  1. Confirm a webhook-configured job named e.g. `wh-example-signed` exists in `examples/cronduit.toml` with `webhook.url = "http://127.0.0.1:9999/signed"` and a `command` that fails (`command = "false"` is sufficient — the webhook fires on the `failed` state).
  2. In terminal A: `just uat-webhook-mock-500` (Python stdlib mock returning 500 for ALL POSTs; logs to `/tmp/cronduit-webhook-mock-500.log`).
  3. In terminal B: `export WEBHOOK_SECRET=test-secret-shh && just dev`.
  4. In terminal C: `just uat-webhook-retry wh-example-signed` (calls `just uat-webhook-fire` and prints the wait-and-verify guidance).
  5. Wait ≈ **6 minutes** (the locked schedule is t=0 + ~30s jittered + ~300s jittered, plus reqwest's 10s per-attempt cap).
  6. In terminal C: `tail -n 30 /tmp/cronduit-webhook-mock-500.log`.
  7. In terminal C: `just uat-webhook-dlq-query`.
- **Pass criteria:**
  - The mock log shows **3** `POST /<path> 500 bytes=<N>` lines (one per attempt) with timestamps roughly matching the locked schedule (t=0, t≈30s, t≈300s — full-jitter expected).
  - `uat-webhook-dlq-query` prints at least one row with `attempts=3` and `dlq_reason='http_5xx'`.
  - cronduit's terminal-B log shows a final WARN-level line indicating the chain exhausted.

[ ] Maintainer-validated

### Scenario 4 — Drain on shutdown — in-flight HTTP not cancelled, queued events dropped at expiry

**What this proves:** WH-10 / Success Criterion 3 / D-15 / D-18 — on SIGTERM, the webhook worker enters drain mode for `webhook_drain_grace` (default 30s); already-pulled in-flight HTTP requests run to reqwest's 10s per-attempt cap (NOT cancelled mid-flight); sleeping retries cancel via the cancel token and write a `dlq_reason='shutdown_drain'` row; events still queued at budget expiry are drained-and-dropped, incrementing `cronduit_webhook_deliveries_total{status="dropped"}` per event.

- **Recipes:** `just uat-webhook-mock-slow` (terminal A) + `just dev` (terminal B) + `just uat-webhook-fire <JOB>` (terminal C) + Ctrl-C in terminal B + `just uat-webhook-dlq-query` (terminal C, after) + `just uat-webhook-metrics-check` (terminal C, after) + `just uat-webhook-drain` (terminal D — prints the 4-step procedure for reference)
- **Steps:**
  1. (Optional, for the procedural printout): in terminal D: `just uat-webhook-drain`.
  2. In terminal A: `just uat-webhook-mock-slow` (Python stdlib mock returning 200 after a 5s sleep; logs to `/tmp/cronduit-webhook-mock-slow.log`).
  3. In terminal B: `export WEBHOOK_SECRET=test-secret-shh && just dev` (with a webhook-configured job pointed at `http://127.0.0.1:9999/...`).
  4. In terminal C: `just uat-webhook-fire <JOB_NAME>` to trigger an immediate run.
  5. Within ≈ 1s of step 4, send Ctrl-C (SIGINT) to terminal B. Wall-clock the shutdown duration (or prefix `just dev` with `time` and read the `real` value at exit).
  6. After cronduit exits in terminal B: in terminal C, run `just uat-webhook-dlq-query` and `just uat-webhook-metrics-check`.
- **Pass criteria:**
  - Terminal B logs `webhook worker entering drain mode (budget: 30s)` (or equivalent D-15 wording).
  - Terminal A logs `SENT 200 ... bytes=<N>` for the in-flight POST (in-flight HTTP NOT cancelled).
  - Total terminal-B shutdown time is **≤ webhook_drain_grace + 10s** (≈ 40s for the default 30s drain). With a single in-flight request and an empty queue this often shows as ≈ 5–10s; the worst-case ceiling is the documented 40s (per docs/WEBHOOKS.md § Drain on shutdown).
  - If any events were still queued at budget expiry: `uat-webhook-metrics-check` shows `cronduit_webhook_deliveries_total{status="dropped"}` incremented; `uat-webhook-dlq-query` shows at least one row with `dlq_reason='shutdown_drain'`. (If only the single fired event was in flight, both may be zero — that's correct: the in-flight POST completed within the budget so no drop occurred.)

[ ] Maintainer-validated

### Scenario 5 — `/metrics` surface — labeled family + per-job seed at boot

**What this proves:** WH-11 / Success Criterion 4 / D-22 / D-23 / D-24 / D-25 / D-26 — the labeled `cronduit_webhook_deliveries_total{job, status}` family is eagerly described and zero-baselined at boot for every webhook-configured job; the duration histogram, queue depth gauge, and the preserved P15 `_dropped_total` saturation counter are all surfaced.

- **Recipe:** `just uat-webhook-metrics-check`
- **Steps:**
  1. With `just dev` running (terminal B from scenarios above) and a webhook-configured job named e.g. `wh-example-signed` in the config…
  2. In terminal C: `just uat-webhook-metrics-check`.
- **Pass criteria:** Recipe exits 0; stdout includes (at minimum):
  - `# HELP cronduit_webhook_deliveries_total ...` and `# TYPE cronduit_webhook_deliveries_total counter`
  - Three pre-seeded zero series for the configured job: `cronduit_webhook_deliveries_total{job="<name>",status="success"} 0`, `..."failed"} 0`, `..."dropped"} 0` (per-job zero-baseline at boot, D-23)
  - `# HELP cronduit_webhook_delivery_duration_seconds ...` and `# TYPE cronduit_webhook_delivery_duration_seconds histogram` plus at least one `_bucket{le="..."}` line
  - `# HELP cronduit_webhook_queue_depth ...` and `# TYPE cronduit_webhook_queue_depth gauge`
  - The P15 `cronduit_webhook_delivery_dropped_total` counter still present (preserved per D-26 — operators' v1.1 dashboards rely on it)

[ ] Maintainer-validated

### Scenario 6 — rustls invariant intact (no openssl-sys in dep tree)

**What this proves:** D-38 — Phase 20 added zero new TLS-touching crates; `cargo tree -i openssl-sys` returns empty across native + linux/amd64-musl + linux/arm64-musl.

- **Recipe:** `just uat-webhook-rustls-check`
- **Steps:**
  1. Run `just uat-webhook-rustls-check` (delegates to `just openssl-check`).
  2. Confirm the recipe exits 0.
  3. Confirm stdout contains `OK: no openssl-sys in dep tree (native + arm64-musl + amd64-musl)`.
- **Pass criteria:** Recipe exits 0; OK line printed.

[ ] Maintainer-validated

### Scenario 7 — `docs/WEBHOOKS.md` reads coherently

**What this proves:** D-27 / Plan 20-07 — the operator hub doc was extended with 6 new sections covering retry/Retry-After/DLQ/drain/HTTPS posture/metrics; mermaid diagrams render; the breaking-change migration note for the metrics family is unambiguous; the TM5 forward-pointer is in place.

- **Recipe:** None — visual review of the rendered Markdown after the PR is open (or local preview).
- **Steps:**
  1. Open `docs/WEBHOOKS.md` (in a markdown viewer that renders mermaid — e.g., GitHub PR Files-changed tab, VS Code preview, or any GFM-aware reader).
  2. Confirm 6 new sections appear after the existing P19 sections: `Retry schedule`, `Retry-After header handling`, `Dead-letter table (\`webhook_deliveries\`)`, `Drain on shutdown`, `HTTPS / SSRF posture`, `Metrics family (\`cronduit_webhook_*\`)`.
  3. Confirm 2 new mermaid diagrams render (3-attempt retry chain `flowchart TD`; SIGTERM drain `sequenceDiagram`).
  4. Confirm the breaking-change migration note for the metrics family is visible: P18's flat `_sent_total` / `_failed_total` are replaced by the labeled `cronduit_webhook_deliveries_total{job, status}` family; the P15 `_dropped_total` counter is **preserved** (D-26 — distinct semantic event).
  5. Confirm the TM5 forward-pointer link `[Threat Model 5 ... Phase 24](../THREAT_MODEL.md)` appears in `## HTTPS / SSRF posture`.
  6. Confirm NO ASCII-art diagrams anywhere in the new sections (no `┌`, `└`, `├`, `┤`, `─` box-drawing characters).
- **Pass criteria:** All 6 sections present; both new mermaid diagrams render; breaking-change note unambiguous; TM5 forward-pointer present; no ASCII art.

[ ] Maintainer-validated

## Sign-off

All 7 scenarios above must be ticked `[x]` by the maintainer before the `v1.2.0-rc.1` tag is cut (Plan 20-09 prerequisite per its frontmatter `must_haves.truths`).

| Field | Value |
|-------|-------|
| Maintainer | _________________________ |
| Date (UTC) | _________________________ |
| Comment / context | _________________________ |

After all boxes are ticked and the sign-off table is filled in:
- The maintainer comments on the rc.1 PR (or merges Plan 20-09's PR) signaling UAT passed.
- Plan 20-09 reads this file to gate the actual `git tag -a -s v1.2.0-rc.1 ...` invocation per `docs/release-rc.md` (D-13 / D-29).
- Post-tag, `.planning/STATE.md` and `.planning/ROADMAP.md` reflect Phase 20 → SHIPPED at rc.1 (orchestrator owns those writes per project workflow).

Cross-reference: every scenario above has automated regression coverage in the Phase 20 integration test files (`tests/v12_webhook_retry.rs`, `tests/v12_webhook_drain.rs`, `tests/v12_webhook_dlq.rs`, `tests/v12_webhook_https_required.rs` — created in Plans 20-02 through 20-05). The UAT scenarios re-prove the same behaviors against a real receiver — operator-side Python mocks, real network latency, real `${WEBHOOK_SECRET}` substitution, real wall-clock drain timing. The tests guard against regressions; this runbook guards against drift between the implementation and the operator's experience.
