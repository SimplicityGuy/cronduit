---
gsd_state_version: 1.0
milestone: v1.2
milestone_name: — Operator Integration & Insight
status: executing
stopped_at: Phase 20 context gathered
last_updated: "2026-05-01T19:49:57.174Z"
last_activity: 2026-05-01 -- Phase 20 execution started
progress:
  total_phases: 10
  completed_phases: 5
  total_plans: 42
  completed_plans: 33
  percent: 79
---

# Project State

## Project Reference

See: `.planning/PROJECT.md` (updated 2026-04-25 — v1.2 milestone kicked off)

**Core value:** One tool that both runs recurrent jobs reliably AND makes their state observable through a web UI.
**Current focus:** Phase 20 — webhook-ssrf-https-posture-retry-drain-metrics-rc-1

## Current Position

Milestone: v1.2 — Operator Integration & Insight (in progress; roadmap created 2026-04-25)
Previous milestone: v1.1 (SHIPPED 2026-04-23, tags `v1.1.0-rc.1` … `v1.1.0-rc.6`, final `v1.1.0`)
Phase: 20 (webhook-ssrf-https-posture-retry-drain-metrics-rc-1) — EXECUTING
Plan: 1 of 9
Status: Executing Phase 20
Last activity: 2026-05-01 -- Phase 20 execution started

Progress: [█████░░░░░] 50% (v1.2: 5/10 phases complete; 33/— plans complete)

## v1.2 Roadmap Summary

```mermaid
flowchart LR
    P15["P15<br/>Foundation<br/>Preamble"] --> P16["P16<br/>FCTX schema<br/>+ run.rs fix"]
    P15 --> P17["P17<br/>Docker labels<br/>(SEED-001)"]
    P15 --> P22["P22<br/>Tagging<br/>schema"]
    P16 --> P18["P18<br/>Webhook<br/>payload"]
    P17 --> P20["P20<br/>Webhook<br/>posture"]
    P18 --> P19["P19<br/>HMAC<br/>+ examples"]
    P19 --> P20
    P20 --> RC1(["rc.1"])
    RC1 --> P21["P21<br/>FCTX UI<br/>+ Exit hist"]
    P16 --> P21
    P21 --> RC2(["rc.2"])
    RC2 --> P22
    P22 --> P23["P23<br/>Tag chips"]
    P23 --> RC3(["rc.3"])
    RC3 --> P24["P24<br/>Close-out<br/>+ TM5/TM6"]
    P24 --> SHIP(["v1.2.0"])

    classDef todo fill:#1a1a3d,stroke:#7fbfff,stroke-width:1px,color:#e0e0ff
    classDef rc fill:#2a1a3d,stroke:#bf7fff,stroke-width:2px,color:#f0e0ff
    classDef ship fill:#00ff7f,stroke:#00ff7f,stroke-width:3px,color:#0a1a0a
    class P15,P16,P17,P18,P19,P20,P21,P22,P23,P24 todo
    class RC1,RC2,RC3 rc
    class SHIP ship
```

10 phases · 41 requirements · 3 rc cuts planned · strict dependency ordering: P15 before P18/P19/P20; P16 before P18+P21; P22 before P23.

## v1.1 Recap (archived)

```mermaid
flowchart LR
    P10["Phase 10<br/>Stop + Hygiene"] --> P11["Phase 11<br/>Run Numbers<br/>+ Log UX"]
    P11 --> P12["Phase 12<br/>Healthcheck<br/>+ rc.1 cut"]
    P12 --> RC1(["v1.1.0-rc.1"])
    RC1 --> P121["Phase 12.1<br/>GHCR tag hygiene<br/>(INSERTED)"]
    P121 --> P13["Phase 13<br/>Observability<br/>+ rc.2 cut"]
    P13 --> RC2(["v1.1.0-rc.2"])
    RC2 --> P14["Phase 14<br/>Bulk Toggle<br/>+ rc.3..rc.6"]
    P14 --> SHIP(["v1.1.0<br/>SHIPPED"])

    classDef done fill:#0a3d0a,stroke:#00ff7f,stroke-width:2px,color:#e0ffe0
    classDef inserted fill:#3d2a1a,stroke:#ffbf7f,stroke-width:2px,color:#ffe0c0
    classDef rc fill:#2a1a3d,stroke:#bf7fff,stroke-width:2px,color:#f0e0ff
    classDef ship fill:#00ff7f,stroke:#00ff7f,stroke-width:3px,color:#0a1a0a
    class P10,P11,P12,P13,P14,P121 done
    class RC1,RC2 rc
    class SHIP ship
```

All 6 phases complete, 6 rc cuts (`rc.1` → `rc.6`), final `v1.1.0` shipped 2026-04-23. Full v1.1 archive: `.planning/milestones/v1.1-ROADMAP.md`, `.planning/milestones/v1.1-REQUIREMENTS.md`.

## v1.0 Recap (archived)

```mermaid
flowchart LR
    P1[Phase 1<br/>Foundation] --> P2[Phase 2<br/>Scheduler]
    P2 --> P3[Phase 3<br/>Web UI]
    P3 --> P4[Phase 4<br/>Docker Executor]
    P4 --> P5[Phase 5<br/>Reload + random]
    P5 --> P6[Phase 6<br/>Release Engineering]
    P6 --> P7[Phase 7<br/>Cleanup]
    P7 --> P8[Phase 8<br/>Human UAT]
    P8 --> P9[Phase 9<br/>CI/CD]
    P9 --> SHIP([v1.0.1 SHIPPED])

    classDef done fill:#0a3d0a,stroke:#00ff7f,stroke-width:2px,color:#e0ffe0
    classDef ship fill:#00ff7f,stroke:#00ff7f,stroke-width:3px,color:#0a1a0a
    class P1,P2,P3,P4,P5,P6,P7,P8,P9 done
    class SHIP ship
```

Full v1.0 archive: `.planning/milestones/v1.0-ROADMAP.md`, `.planning/milestones/v1.0-REQUIREMENTS.md`, `.planning/milestones/v1.0-MILESTONE-AUDIT.md`.

## Accumulated Context

### Decisions

All v1.0 and v1.1 decisions live in `.planning/PROJECT.md` § Key Decisions.

**v1.2 decisions inherited from research/requirements (LOCKED):**

- Webhook delivery worker shape: NEW module `src/webhooks/mod.rs` with bounded `tokio::sync::mpsc::channel(1024)` + dedicated worker task. Scheduler emits via `try_send` (NEVER await tx.send).
- HMAC algorithm: SHA-256 ONLY (Standard Webhooks v1 spec). No algorithm-agility.
- Webhook retry: 3 attempts at t=0, t=30s, t=300s with full-jitter (rand 0.8-1.2× multiplier).
- Webhook coalescing: edge-triggered, default fires only on `streak_position == 1`; configurable per-job via `fire_every`.
- Webhook URL validation: HTTPS required for non-loopback/non-RFC1918; HTTP only for local destinations (with WARN).
- Webhook payload: Standard Webhooks v1 spec headers; `payload_version: "v1"` field.
- Webhook reload survival + 30s drain on shutdown.
- Docker labels merge: `use_defaults=false` → replace; otherwise merge with per-job-wins on collision.
- Reserved namespace: `cronduit.*` (validator at config-load).
- Type-gated: labels only on `type="docker"` jobs.
- run.rs:277 bug fix: add `container_id` field to `DockerExecResult`; fix the assignment.
- Failure-context: 5 P1 signals (time + image + config + duration-vs-p50 + scheduler-fire-skew).
- `job_runs.config_hash`: per-run column added (Option A; not `jobs.updated_at` proxy).
- Exit-code: 10-bucket strategy; success as separate stat; stopped distinct from 128-143; last-100-ALL window; NOT exposed as Prometheus label.
- Tagging: `jobs.tags` TEXT JSON column; per-job only (not in `[defaults]`); lowercase+trim normalization; charset regex; substring-collision check; AND filter semantics; untagged-hidden when filter active; URL state via repeated `?tag=`.
- cargo-deny: v1.2 preamble (non-blocking initially; promoted to blocking before final v1.2.0).

### Open questions

None at roadmap level. Phase-plan-level open questions surface during `/gsd-discuss-phase`.

### Pending Todos

- Run `/gsd-discuss-phase 15` to start the first v1.2 phase (Foundation Preamble: Cargo bump + cargo-deny + webhook worker scaffolding).

### Blockers/Concerns

None.

Three Phase 9 UAT items from v1.0 are accepted as deferred to natural post-merge validation per the v1.0 audit verdict — see `.planning/milestones/v1.0-MILESTONE-AUDIT.md` § `deferred_post_merge_observation`. They are NOT blockers for v1.2.

## Deferred Items

Items acknowledged and deferred at v1.1 milestone close on 2026-04-24. All six surfaced by the pre-close open-artifact audit are **false positives** — the underlying work shipped and was validated, but the audit tool's heuristics do not recognize the completion markers in these file shapes. Recorded here for traceability.

| Category | Item | Reason flagged | Actual status |
|----------|------|----------------|---------------|
| uat | 13/HUMAN-UAT.md | audit tool read "0 pending scenarios" as incomplete | Complete — maintainer runbook for rc.2 tag cut; rc.2 cut + verified 2026-04-21 (commits `7e43c1c`, `344263c`) |
| uat | 14/14-08-UAT-RESULTS.md | audit tool read "0 pending scenarios" as incomplete | Complete — documents rc.3 UAT FAIL; rc.4/5/6/final resolved all findings (commits `c4b8267`, `7c5f6dd`, `a49898e`, final `v1.1.0` at `a49898e`) |
| uat | 14/14-HUMAN-UAT.md | audit tool read "0 pending scenarios" as incomplete | Complete — all validation boxes ticked by maintainer at v1.1.0 sign-off (per `14-09-SUMMARY.md` Prerequisites table) |
| verification | 12/12-VERIFICATION.md | front-matter `status: human_needed` | Complete — all three human-needed items (rc.1 tag cut, GHCR post-push verification, compose-smoke green on PR) closed 2026-04-19 |
| quick_task | 260414-gbf-fix-defaults-merge-bug-issue-20-defaults | state file missing under `.planning/quick/` | Complete — archived with v1.0 milestone; recorded in `.planning/milestones/v1.0-MILESTONE-AUDIT.md` |
| quick_task | 260421-nn3-fix-get-dashboard-jobs-postgres-j-enable | state file missing under `.planning/quick/` | Complete — landed in commits `07d81bb`, `7cb1a10`, `7917502`, `3b92a45` (PR #37); logged in Quick Tasks Completed table above |

### Quick Tasks Completed

| ID | Date | Description | Commits | Reference |
|----|------|-------------|---------|-----------|
| 260421-nn3 | 2026-04-22 | Fix `get_dashboard_jobs` Postgres `j.enabled = true` BIGINT bug (queries.rs lines 615 + 628) + add Postgres regression test `tests/dashboard_jobs_pg.rs` mirroring v13_timeline_explain harness. Closes the deferred item logged in Phase 13 plan 06. | `07d81bb`, `7cb1a10`, `7917502` | `.planning/quick/260421-nn3-fix-get-dashboard-jobs-postgres-j-enable/` |

v1.0 quick task `260414-gbf` is archived in `.planning/milestones/v1.0-MILESTONE-AUDIT.md`.

## Session Continuity

Last session: 2026-05-01T17:26:16.264Z
Stopped at: Phase 20 context gathered
Resume command: `/gsd-discuss-phase 20` for Webhook SSRF/HTTPS posture + Retry/Drain + Metrics — rc.1

**Planned Phase:** 20 — Webhook SSRF/HTTPS Posture + Retry/Drain + Metrics — rc.1 (HTTPS-required for non-loopback/non-RFC1918, SSRF guards, retry schedule t=0/30s/300s with full-jitter, 30s drain on shutdown, Prometheus metrics)
