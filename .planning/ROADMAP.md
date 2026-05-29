# Roadmap: Cronduit

## Milestones

- ✅ **v1.0 — Docker-Native Cron Scheduler** — Phases 1–9 (shipped 2026-04-14, tags `v1.0.0` + `v1.0.1`) — see [`milestones/v1.0-ROADMAP.md`](milestones/v1.0-ROADMAP.md) and [`MILESTONES.md`](MILESTONES.md)
- ✅ **v1.1 — Operator Quality of Life** — Phases 10–14 + 12.1 inserted (shipped 2026-04-23, tags `v1.1.0-rc.1`…`v1.1.0-rc.6`, final `v1.1.0`) — see [`milestones/v1.1-ROADMAP.md`](milestones/v1.1-ROADMAP.md) and [`MILESTONES.md`](MILESTONES.md)
- ✅ **v1.2 — Operator Integration & Insight** — Phases 15–24 (shipped 2026-05-19, tags `v1.2.0-rc.1`…`v1.2.0-rc.5`, final `v1.2.0`, patch `v1.2.1`) — see [`milestones/v1.2-ROADMAP.md`](milestones/v1.2-ROADMAP.md) and [`MILESTONES.md`](MILESTONES.md)

## Phases

<details>
<summary>✅ v1.0 Docker-Native Cron Scheduler (Phases 1–9) — SHIPPED 2026-04-14</summary>

- [x] Phase 1: Foundation, Security Posture & Persistence Base (9/9 plans) — 2026-04-10
- [x] Phase 2: Scheduler Core & Command/Script Executor (4/4 plans) — 2026-04-10
- [x] Phase 3: Read-Only Web UI & Health Endpoint (6/6 plans) — 2026-04-11
- [x] Phase 4: Docker Executor & `container:<name>` Differentiator (4/4 plans) — 2026-04-11
- [x] Phase 5: Config Reload & `@random` Resolver (5/5 plans) — 2026-04-12
- [x] Phase 6: Live Events, Metrics, Retention & Release Engineering (7/7 plans) — 2026-04-13
- [x] Phase 7: v1.0 Cleanup & Bookkeeping (5/5 plans) — 2026-04-13
- [x] Phase 8: v1.0 Final Human UAT Validation (5/5 plans) — 2026-04-14
- [x] Phase 9: CI/CD Improvements (4/4 plans) — 2026-04-14

**Total:** 49 plans across 9 phases · 86/86 v1.0 requirements Complete · audit verdict `passed`

</details>

<details>
<summary>✅ v1.1 Operator Quality of Life (Phases 10–14 + 12.1) — SHIPPED 2026-04-23</summary>

- [x] Phase 10: Stop-a-Running-Job + Hygiene Preamble (10/10 plans) — 2026-04-15
- [x] Phase 11: Per-Job Run Numbers + Log UX Fixes (15/15 plans + 1 pre-wave spike) — 2026-04-17
- [x] Phase 12: Docker Healthcheck + rc.1 Cut (7/7 plans) — 2026-04-18; `v1.1.0-rc.1` cut 2026-04-19
- [x] Phase 12.1: GHCR Tag Hygiene _(INSERTED)_ (4/4 plans) — 2026-04-20
- [x] Phase 13: Observability Polish — rc.2 (6/6 plans) — 2026-04-21; `v1.1.0-rc.2` cut 2026-04-21
- [x] Phase 14: Bulk Enable/Disable + rc.3..rc.6 + v1.1.0 final ship (9/9 plans) — 2026-04-23

**Total:** 52 plans across 6 phases · 33/33 v1.1 requirements Complete · six rc tags (`rc.1`…`rc.6`) + final `v1.1.0` · `:latest` promoted from `:1.0.1` to `:1.1.0` on both archs

</details>

<details>
<summary>✅ v1.2 Operator Integration & Insight (Phases 15–24) — SHIPPED 2026-05-19</summary>

- [x] Phase 15: Foundation Preamble (5/5 plans) — 2026-04-26
- [x] Phase 16: Failure-Context Schema + run.rs Bug Fix (7/7 plans) — 2026-04-28
- [x] Phase 17: Custom Docker Labels — SEED-001 (9/9 plans: 6 core + 3 gap closure) — 2026-04-29
- [x] Phase 18: Webhook Payload + State-Filter + Coalescing (6/6 plans) — 2026-04-29
- [x] Phase 19: Webhook HMAC Signing + Receiver Examples (6/6 plans) — 2026-04-30
- [x] Phase 20: Webhook SSRF/HTTPS + Retry/Drain + Metrics — rc.1 (12/12 plans) — 2026-05-01; `v1.2.0-rc.1` cut
- [x] Phase 21: Failure-Context UI Panel + Exit-Code Histogram Card — rc.2 (11/11 plans) — 2026-05-03; `v1.2.0-rc.2` cut
- [x] Phase 22: Job Tagging Schema + Validators (6/6 plans) — 2026-05-04
- [x] Phase 23: Job Tagging Dashboard Filter Chips — rc.3 (8/8 plans) — 2026-05-05; `v1.2.0-rc.3` cut
- [x] Phase 24: Milestone Close-Out — final `v1.2.0` ship (8/8 plans) — 2026-05-17

**Total:** 78 plans across 10 phases · 41/41 v1.2 requirements Validated · audit verdict `passed` · rc tags `rc.1`…`rc.5` then last UAT-passing SHA retagged `v1.2.0` (2026-05-19) + patch `v1.2.1` · `:latest` promoted from `:1.1.0` to `:1.2.0` on both archs

</details>

## Progress

| Milestone | Phases | Plans | Status | Shipped |
| --------- | ------ | ----- | ------ | ------- |
| v1.0 | 1–9 | 49/49 | ✅ Complete | 2026-04-14 |
| v1.1 | 10–14 (+ 12.1) | 52/52 | ✅ Complete | 2026-04-23 |
| v1.2 | 15–24 | 78/78 | ✅ Complete | 2026-05-19 |

---

*v1.0 archived 2026-04-14 via `/gsd-complete-milestone`. Full historical roadmap, requirements, audit, and execution history preserved under `.planning/milestones/v1.0-*`.*

*v1.1 archived 2026-04-24 via `/gsd-complete-milestone`. Full roadmap and requirements preserved under `.planning/milestones/v1.1-*`. Phase execution history archived to `.planning/milestones/v1.1-phases/`.*

*v1.2 archived 2026-05-29 via `/gsd-complete-milestone`. Full roadmap (phase details + build-order mermaid), requirements, and audit preserved under `.planning/milestones/v1.2-*`. Phase execution history retained under `.planning/phases/` (use `/gsd-cleanup` to archive retroactively).*
