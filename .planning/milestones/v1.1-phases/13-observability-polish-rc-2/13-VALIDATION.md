---
phase: 13
slug: observability-polish-rc-2
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-21
---

# Phase 13 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo + cargo-nextest (unit + integration) |
| **Config file** | `Cargo.toml`, `.config/nextest.toml` (if present) |
| **Quick run command** | `cargo nextest run --lib` |
| **Full suite command** | `cargo nextest run --all-features --profile ci` |
| **Estimated runtime** | ~60-90 seconds (unit); ~3-5 minutes (full w/ testcontainers Postgres) |

---

## Sampling Rate

- **After every task commit:** Run `cargo nextest run --lib` (unit tests for the modified crate)
- **After every plan wave:** Run `cargo nextest run --all-features --profile ci`
- **Before `/gsd-verify-work`:** Full suite must be green on both SQLite and Postgres paths
- **Max feedback latency:** 90 seconds (unit); 300 seconds (full)

---

## Per-Task Verification Map

> Populated by the planner (step 8) and filled in during execution. Each Phase 13 task must map to at least one automated command OR a Wave 0 stub.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| TBD     | TBD  | TBD  | OBS-01      | —          | N/A             | unit+integration | `cargo nextest run -E 'test(percentile)'` | ⬜ W0 | ⬜ pending |
| TBD     | TBD  | TBD  | OBS-02      | —          | N/A             | integration | `cargo nextest run -E 'test(sparkline)'` | ⬜ W0 | ⬜ pending |
| TBD     | TBD  | TBD  | OBS-03      | —          | N/A             | integration | `cargo nextest run -E 'test(timeline)'` | ⬜ W0 | ⬜ pending |
| TBD     | TBD  | TBD  | OBS-04      | —          | N/A             | integration | `cargo nextest run -E 'test(explain_query_plan)'` | ⬜ W0 | ⬜ pending |
| TBD     | TBD  | TBD  | OBS-05      | —          | Structural parity: no SQL-native percentile | CI grep + integration | `cargo nextest run --features postgres` + CI grep guard | ⬜ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/web/stats.rs` — `percentile(samples, q)` helper with inline `#[cfg(test)] mod tests` and the 5 canonical test vectors from D-21
- [ ] `tests/common/v11_fixtures.rs` — shared DB fixtures for sparkline/timeline/duration (reuse existing fixtures where possible)
- [ ] `tests/explain_query_plan.rs` — integration test asserting `idx_job_runs_start_time` appears in `EXPLAIN QUERY PLAN` output on SQLite and `EXPLAIN ANALYZE` on Postgres
- [ ] CI step (`.github/workflows/ci.yml`) — grep guard that fails the build if any SQL query in `src/db/queries.rs` contains `percentile_cont`, `percentile_disc`, or `PERCENTILE_CONT` (OBS-05 structural-parity enforcement)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Dashboard sparkline visual rendering (20 cells, status colors, `--cd-status-*` tokens) | OBS-02 | Visual fidelity across browsers / themes not automatable | Open `http://127.0.0.1:8080/` with ≥5 terminal runs across several jobs; confirm each card has 20 cells colored per status token; confirm `—` badge for <5-run jobs |
| `/timeline` gantt visual layout | OBS-03 | Visual positioning of CSS-grid bars not automatable | Open `http://127.0.0.1:8080/timeline`, confirm 24h default, click `7d` toggle, confirm disabled/hidden jobs absent, confirm color-by-status |
| `v1.1.0-rc.2` GHCR push and `:latest` immutability | OBS-05 (release gate) | Requires authenticated GHCR access and real tag | Run `docs/release-rc.md` runbook; verify `docker pull ghcr.io/OWNER/cronduit:v1.1.0-rc.2` works and `:latest` still resolves to `v1.0.1` digest |
| Operator timezone rendering | OBS-03 | Requires config mutation + server restart | Set `[server].timezone = "America/Los_Angeles"` in `cronduit.toml`, restart, confirm timeline axis labels match PT |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (percentile helper + EXPLAIN QUERY PLAN harness)
- [ ] No watch-mode flags
- [ ] Feedback latency < 90s for unit runs
- [ ] Structural parity guard (CI grep) green on both backends
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
