---
phase: 20
slug: webhook-ssrf-https-posture-retry-drain-metrics-rc-1
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-01
---

# Phase 20 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source of truth: `20-RESEARCH.md` § 11 (Validation Architecture).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` + `cargo nextest` (preferred); `tokio::test` async; `wiremock 0.6` HTTP mocks; `tokio::time::pause` / `advance` deterministic clocks |
| **Config file** | `Cargo.toml` `[features.integration]` + dev-deps (already wired) |
| **Quick run command** | `just test-unit` (existing — `cargo nextest run --lib`) |
| **Full suite command** | `cargo nextest run --all-features` |
| **Estimated runtime** | ~90s lib, ~6 min full suite (matrix `linux/{amd64,arm64} × {SQLite, Postgres}`) |

---

## Sampling Rate

- **After every task commit:** Run `just test-unit`
- **After every plan wave:** Run `cargo nextest run --all-features`
- **Before `/gsd-verify-work`:** Full suite green + CI matrix (4 legs) green + `cargo tree -i openssl-sys` empty + `git cliff --unreleased --tag v1.2.0-rc.1` reviewed
- **Max feedback latency:** ~90s (unit), ~6 min (full)

---

## Per-Task Verification Map

| Req ID | Behavior | Test Type | Automated Command | File Exists | Status |
|--------|----------|-----------|-------------------|-------------|--------|
| WH-05 | 3-attempt retry chain at t=0/30s/300s with full-jitter backoff | integration | `cargo nextest run --test v12_webhook_retry` | ❌ W0 | ⬜ pending |
| WH-05 | Classification table (200/408/429/4xx/5xx/network/timeout) | integration | `cargo nextest run --test v12_webhook_retry_classification` | ❌ W0 | ⬜ pending |
| WH-05 | DLQ row written on retry exhaustion + 4xx-permanent + shutdown_drain | integration | `cargo nextest run --test v12_webhook_dlq` | ❌ W0 | ⬜ pending |
| WH-05 | `Retry-After` honored within cap (`next_attempt+1 × 1.2`) | integration | `cargo nextest run --test v12_webhook_retry_after` | ❌ W0 | ⬜ pending |
| WH-07 | LOAD-time HTTPS-required validator (allowlist 127/8, ::1, 10/8, 172.16/12, 192.168/16, fd00::/8) | integration | `cargo nextest run --test v12_webhook_https_required` | ❌ W0 | ⬜ pending |
| WH-07 | Validator unit tests for IP classification (incl. IPv6 ULA via `is_unique_local()`) | unit | `cargo nextest run --lib config::validate::tests` | ❌ W0 | ⬜ pending |
| WH-08 | THREAT_MODEL.md TM5 forward-pointer stub from `docs/WEBHOOKS.md` | manual | `grep -q 'TM5' docs/WEBHOOKS.md` | manual | ⬜ pending |
| WH-10 | 30s drain budget enforced; in-flight HTTP NOT cancelled mid-flight | integration | `cargo nextest run --test v12_webhook_drain` | ❌ W0 | ⬜ pending |
| WH-10 | Drained-and-dropped counter increments per remaining queued event | integration | (covered in `v12_webhook_drain`) | ❌ W0 | ⬜ pending |
| WH-11 | `cronduit_webhook_deliveries_total{job, status}` family eagerly described + zero-baselined | integration | `cargo nextest run --test v12_webhook_metrics_family` | ❌ W0 | ⬜ pending |
| WH-11 | `cronduit_webhook_delivery_duration_seconds{job}` histogram with operator-tuned buckets | integration | (covered in `v12_webhook_metrics_family`) | ❌ W0 | ⬜ pending |
| WH-11 | `cronduit_webhook_queue_depth` gauge sampled at `rx.recv()` boundary | integration | (covered in `v12_webhook_metrics_family`) | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

Per-task IDs (`{N}-PP-TT`) populate from PLAN.md frontmatter once plans exist.

---

## Wave 0 Requirements

- [ ] `tests/v12_webhook_retry.rs`
- [ ] `tests/v12_webhook_retry_classification.rs`
- [ ] `tests/v12_webhook_retry_after.rs`
- [ ] `tests/v12_webhook_drain.rs`
- [ ] `tests/v12_webhook_dlq.rs`
- [ ] `tests/v12_webhook_https_required.rs`
- [ ] `tests/v12_webhook_metrics_family.rs`
- [ ] `migrations/sqlite/20260502_000008_webhook_deliveries_add.up.sql`
- [ ] `migrations/postgres/20260502_000008_webhook_deliveries_add.up.sql`
- [ ] `src/webhooks/retry.rs` (new file — `dispatcher.rs` already at 535 lines)
- [ ] `docs/WEBHOOKS.md` extension (6 new sections per D-27)
- [ ] `justfile` UAT recipes (`uat-webhook-retry`, `uat-webhook-drain`, `uat-webhook-dlq-query`, `uat-webhook-https-required`)

(No framework install needed — `cargo nextest`, `wiremock`, `tokio test-util`, `testcontainers` already in `Cargo.toml`.)

---

## Instrumentation Points

Every invariant asserts visibility through THREE channels (project's standard observability surface):

| Invariant | Counter | DLQ row | Log line |
|-----------|---------|---------|----------|
| Successful delivery | `_deliveries_total{job=X, status="success"}` += 1 | none | DEBUG `"webhook delivered"` |
| Failed delivery (4xx-permanent) | `_deliveries_total{job=X, status="failed"}` += 1 | `dlq_reason='http_4xx'`, `attempts=1` | WARN `"webhook non-2xx"` |
| Failed delivery (5xx-exhausted) | `_deliveries_total{job=X, status="failed"}` += 1 | `dlq_reason='http_5xx'`, `attempts=3` | WARN per attempt + WARN final |
| Drain-on-shutdown (mid-chain) | `_deliveries_total{job=X, status="failed"}` += 1 | `dlq_reason='shutdown_drain'`, `attempts=N` | INFO `"entering drain mode"` + WARN per cancelled chain |
| Drain-on-shutdown (queued) | `_deliveries_total{job=X, status="dropped"}` += 1 | none (post-channel only) | WARN `"drained event dropped"` |
| Channel saturation (P15 unchanged) | `cronduit_webhook_delivery_dropped_total` (UNLABELED) | none | WARN `"channel saturated — event dropped"` |

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `v1.2.0-rc.1` GHCR multi-arch publish | Success criterion 5 | Maintainer-local terminal per `docs/release-rc.md` D-13; not CI | Run pre-flight checklist; `git tag -a -s v1.2.0-rc.1 -m "v1.2.0-rc.1 — webhook block (P15..P20)"`; `git push origin v1.2.0-rc.1`; verify `ghcr.io/SimplicityGuy/cronduit:v1.2.0-rc.1` published amd64+arm64; `:latest` still at `v1.1.0` |
| `cargo tree -i openssl-sys` empty | D-38 (rustls invariant) | Build-graph property, asserted at CI level | `cargo tree -i openssl-sys` returns empty in CI |
| Operator UAT recipes | D-34 / project memory `feedback_uat_use_just_commands.md` | Maintainer validates UAT — Claude does NOT mark UAT passed (D-35) | Run `just uat-webhook-retry`, `uat-webhook-drain`, `uat-webhook-dlq-query`, `uat-webhook-https-required`; record results in `20-HUMAN-UAT.md` |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 90s (unit) / 6 min (full)
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
