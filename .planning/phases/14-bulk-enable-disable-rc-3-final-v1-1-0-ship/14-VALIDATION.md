---
phase: 14
slug: bulk-enable-disable-rc-3-final-v1-1-0-ship
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-22
---

# Phase 14 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Seeded from 14-RESEARCH.md § Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo-nextest` 0.9.x (CI gate) + `cargo test` (local dev) |
| **Config file** | `.config/nextest.toml` (implicit CI profile via `just nextest`); `Cargo.toml` `[dev-dependencies]` holds `testcontainers 0.27.2` + `testcontainers-modules 0.15.0` |
| **Quick run command** | `cargo nextest run --test v11_bulk_toggle` (scoped to the Phase 14 integration file once Wave 0 lands it) |
| **Full suite command** | `just nextest` (= `cargo nextest run --all-features --profile ci`) |
| **Estimated runtime** | ~30s scoped / ~3-5m full |

---

## Sampling Rate

- **After every task commit:** Run `cargo nextest run --test v11_bulk_toggle --test schema_parity --test migrations_idempotent`
- **After every plan wave:** Run `just nextest`
- **Before `/gsd-verify-work`:** Full suite must be green; HUMAN-UAT.md user-checked
- **Max feedback latency:** 30s (scoped) / 300s (full)

---

## Per-Task Verification Map

| Req / Invariant | Behavior | Test Type | Automated Command | File Exists | Status |
|-----------------|----------|-----------|-------------------|-------------|--------|
| **DB-14** | Migration adds `enabled_override` nullable column (SQLite INTEGER NULL / Postgres BIGINT NULL) | migration | `cargo nextest run --test migrations_idempotent` | ✅ existing — add column-presence assertion | ⬜ pending |
| **DB-14** | Schema parity: SQLite INTEGER + Postgres BIGINT both normalize to INT64 | schema-parity | `cargo nextest run --test schema_parity` (alias `just schema-diff`) | ✅ existing — must stay green after column add | ⬜ pending |
| **T-V11-BULK-01** | `upsert_job` does NOT touch `enabled_override` in INSERT columns or ON CONFLICT SET clause | unit (in-memory SQLite + testcontainers-postgres) | `cargo nextest run --test v11_bulk_toggle::upsert_invariant` | ❌ Wave 0 | ⬜ pending |
| **T-V11-BULK-01** | Reload preserves `enabled_override` for every job still in config | integration | `cargo nextest run --test v11_bulk_toggle::reload_invariant` | ❌ Wave 0 | ⬜ pending |
| **ERG-04** | `disable_missing_jobs` sets `enabled = 0` AND `enabled_override = NULL` together for jobs removed from config | integration | `cargo nextest run --test v11_bulk_toggle::disable_missing_clears_override` | ❌ Wave 0 | ⬜ pending |
| **DB-14** | `get_enabled_jobs` filter becomes `enabled = 1 AND (enabled_override IS NULL OR enabled_override = 1)`; override=0 excludes | unit | `cargo nextest run --test v11_bulk_toggle::dashboard_filter` | ❌ Wave 0 | ⬜ pending |
| **ERG-01** | `bulk_toggle` handler: CSRF pass → 200 + HX-Trigger; CSRF fail → 403 | integration (axum test server) | `cargo nextest run --test v11_bulk_toggle::handler_csrf` | ❌ Wave 0 | ⬜ pending |
| **ERG-01** | `bulk_toggle` handler: `action=disable` sets override=0 for all `job_ids` | integration | `cargo nextest run --test v11_bulk_toggle::handler_disable` | ❌ Wave 0 | ⬜ pending |
| **ERG-01** | `bulk_toggle` handler: `action=enable` sets override=NULL for all `job_ids` | integration | `cargo nextest run --test v11_bulk_toggle::handler_enable` | ❌ Wave 0 | ⬜ pending |
| **D-12** | Partial-invalid IDs: handler applies to valid, returns 200, toast carries `(K not found)` suffix | integration | `cargo nextest run --test v11_bulk_toggle::handler_partial_invalid` | ❌ Wave 0 | ⬜ pending |
| **UI-SPEC primary-count** | Partial-invalid toast primary count == `rows_affected` (not `selection_size`); locks `"2 jobs disabled. (1 not found)"` exact-string for selection=[1,2,9999] | integration (exact-string on HX-Trigger) | `cargo nextest run --test v11_bulk_toggle::handler_partial_invalid_toast_uses_rows_affected` | ❌ Wave 0 | ⬜ pending |
| **D-12a** | Dedupe `job_ids` before UPDATE (duplicate IDs do NOT cause duplicate UPDATEs) | unit | `cargo nextest run --test v11_bulk_toggle::handler_dedupes_ids` | ❌ Wave 0 | ⬜ pending |
| **Claude's Discretion** | Empty `job_ids` → 400 + error toast | integration | `cargo nextest run --test v11_bulk_toggle::handler_rejects_empty` | ❌ Wave 0 | ⬜ pending |
| **ERG-01** | `bulk_toggle` dispatches `SchedulerCmd::Reload` AFTER DB commit (heap-rebuild order) | integration (asserts mpsc message + DB state) | `cargo nextest run --test v11_bulk_toggle::handler_fires_reload_after_update` | ❌ Wave 0 | ⬜ pending |
| **ERG-03** | `get_overridden_jobs` returns all jobs with `enabled_override IS NOT NULL`, alphabetical by name | unit | `cargo nextest run --test v11_bulk_toggle::get_overridden_jobs_alphabetical` | ❌ Wave 0 | ⬜ pending |
| **ERG-03** | Settings page renders "Currently Overridden" section only when list non-empty | integration (render test) | `cargo nextest run --test v11_bulk_toggle::settings_empty_state_hides_section` | ❌ Wave 0 | ⬜ pending |
| **ERG-02** | Bulk disable on running job does NOT terminate the run (runs complete naturally) | manual-UAT | HUMAN-UAT.md step 3 per D-17 | covered by UAT, not automated | ⬜ pending |
| **Postgres parity** | Every SQLite test above has a Postgres counterpart using `testcontainers-modules::postgres::Postgres` | integration | `cargo nextest run --test v11_bulk_toggle_pg` OR cfg-feature gate in `v11_bulk_toggle.rs` (precedent: `tests/dashboard_jobs_pg.rs`) | ❌ Wave 0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/v11_bulk_toggle.rs` — new file covering T-V11-BULK-01 invariants + ERG-01..04 handler/query behaviors (15+ test cases enumerated above)
- [ ] `tests/v11_bulk_toggle_pg.rs` — OR cfg-feature-gated Postgres parity tests within the same file (precedent: `tests/dashboard_jobs_pg.rs`)
- [ ] `tests/schema_parity.rs` — verify the new `enabled_override` column is picked up automatically via schema introspection (normalized INT64); no code change expected, but explicit re-run required
- [ ] `tests/migrations_idempotent.rs` — add assertion that the new migration runs cleanly on a DB at the prior migration head AND runs idempotently on re-invocation

*Testcontainers postgres image already pinned at `postgres:16-alpine` — no new pin needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Running-job completion on bulk disable | ERG-02 | Requires real scheduler + Docker + clock-time wait to observe that a running job terminates at its own natural completion point, not on bulk-disable write | HUMAN-UAT.md step 3: `just compose-up-rc3` → start a long-running job → bulk-disable it plus 2 others → observe toast "3 jobs disabled. 1 currently-running job will complete naturally." → watch the running job finish to terminal status (NOT `stopped`) |
| End-to-end dashboard bulk-select UX | ERG-01 | Requires a human to exercise checkbox clicks, header select-all, sticky-bar scroll behavior, HX-Trigger toast render | HUMAN-UAT.md steps 2, 3 |
| Settings "Currently Overridden" audit section | ERG-03 | Requires human to navigate, verify empty-state hide, click per-row Clear button and confirm toast + list refresh | HUMAN-UAT.md steps 5, 6 |
| Reload invariant round-trip | ERG-04 | Requires human-driven SIGHUP or `just reload` on a running container with live DB state | HUMAN-UAT.md steps 4, 7 |
| rc.3 tag + `:rc` rolling + multi-arch manifest | v1.1.0-rc.3 ship | `git-cliff --unreleased` output + `docker manifest inspect` cross-check; not automated on main CI | HUMAN-UAT.md + `docs/release-rc.md` runbook |
| v1.1.0 promotion = bit-identical image | D-16 | Maintainer-only tag-retag of the rc.3 SHA; `scripts/verify-latest-retag.sh` confirms post-push | Close-out step; invoked only after HUMAN-UAT.md steps 1-8 are user-checked |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s for scoped run / < 300s full suite
- [ ] `nyquist_compliant: true` set in frontmatter after Wave 0 lands

**Approval:** pending
