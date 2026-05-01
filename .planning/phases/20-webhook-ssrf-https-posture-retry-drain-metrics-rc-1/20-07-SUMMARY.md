---
phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
plan: 07
subsystem: docs
tags: [webhooks, docs, operator-doc, threat-model-stub, mermaid]
requires: [20-02, 20-04, 20-05]
provides: [operator-facing-webhook-posture-doc-extension]
affects: [docs/WEBHOOKS.md]
tech-stack:
  added: []
  patterns: [mermaid-only-diagrams, append-only-doc-extension, toc-coherence]
key-files:
  created: []
  modified:
    - docs/WEBHOOKS.md
decisions:
  - D-27 section ordering applied verbatim (retry → Retry-After → DLQ → drain → HTTPS/SSRF → metrics).
  - TOC updated to enumerate the 6 new sections (Rule 1 doc-coherence — existing TOC explicitly numbered 1..10).
  - TM5 forward-pointer wording locked verbatim with "full close-out in Phase 24" per security_constraint + ROADMAP scope.
  - Two mermaid diagrams added (3-attempt retry chain flowchart + SIGTERM drain sequence). Zero ASCII-art.
metrics:
  duration_minutes: 5
  completed: 2026-05-01T21:51:24Z
  task_count: 1
  file_count: 1
  commit_count: 1
---

# Phase 20 Plan 07: Operator-Doc Extension (`docs/WEBHOOKS.md`) Summary

Extended the Phase 19 operator hub doc with 6 Phase 20 sections covering retry schedule, Retry-After handling, the new DLQ table, drain-on-shutdown semantics, HTTPS/SSRF posture, and the labeled metrics family — all in mermaid where diagrams are needed and with operator-actionable SQL/PromQL examples.

## What changed

**File modified:** `docs/WEBHOOKS.md`
- Pre-state: 290 lines, 10 P19 sections (Overview through Loopback Rust mock)
- Post-state: **649 lines**, **16 sections**, **5 mermaid blocks** (3 from P19 + 2 new)

## New sections (in append order)

| # | Section heading | Line range | Notes |
|---|-----------------|------------|-------|
| 1 | `## Retry schedule` | 300..358 | Locked schedule table, classification table, mermaid flowchart of 3-attempt chain with cancel/drain branches |
| 2 | `## Retry-After header handling` | 360..395 | Cap math + worked table (slot 0/1/2 → cap 36s/360s/360s), receiver guidance |
| 3 | `## Dead-letter table (`webhook_deliveries`)` | 397..466 | Schema, dlq_reason enum, 3 operator SQL queries (sqlite + postgres syntax), retention note |
| 4 | `## Drain on shutdown` | 468..531 | Worst-case ceiling = `webhook_drain_grace + 10s`, mermaid sequence diagram, dropped-vs-DLQ-recorded breakdown |
| 5 | `## HTTPS / SSRF posture` | 533..582 | Allowed http:// destinations table, accepted residual risk (hostname → public IP), TM5 forward-pointer |
| 6 | `## Metrics family (`cronduit_webhook_*`)` | 584..649 | Surface table, breaking-change migration (P18 → P20), dropped-counter semantic split (P15 vs P20), histogram buckets, label-naming guidance |

Plus: Table of contents at lines 21..26 extended to enumerate items 11..16 (TOC coherence).

## Mermaid diagram inventory

| # | Section | Diagram type | Purpose |
|---|---------|--------------|---------|
| 1 | Retry schedule | `flowchart TD` | 3-attempt retry chain showing 2xx success, 4xx-other permanent, 408/429/5xx/network/timeout transient, cancel-fired drain, exhaust paths |
| 2 | Drain on shutdown | `sequenceDiagram` | SIGTERM → scheduler → webhook worker → dispatcher → DB sequence with drain budget elapsed |

**Zero ASCII-art** — verified by grep for `+--`, `|...|...+` patterns in the appended content (none found). Project memory `feedback_diagrams_mermaid.md` honored.

## TM5 forward-pointer (verbatim — Phase 24 reads this in advance)

> Phase 20 ships the operational mitigations above. The full threat model
> discussion (including the operator-with-UI-access widening of the SSRF blast
> radius, the loopback-bound default mitigation, and the reverse-proxy/auth
> deployment posture) lands in
> **[Threat Model 5 — Webhook Outbound, full close-out in Phase 24](../THREAT_MODEL.md)**.
> Phase 20 is intentionally a forward-pointer; the canonical TM5 entry is part of
> the v1.2.0 milestone close-out.

Link target: `../THREAT_MODEL.md` (relative path from `docs/WEBHOOKS.md`).
Anchor text contains the literal string `Threat Model 5` (1 match in file — verified).

## Acceptance criteria results

| Criterion | Required | Actual | Result |
|-----------|----------|--------|--------|
| File exists | yes | yes | PASS |
| Line count | ≥ 600 | 649 | PASS |
| `## Retry schedule` count | 1 | 1 | PASS |
| `## Retry-After header handling` count | 1 | 1 | PASS |
| `## Dead-letter table` count | 1 | 1 | PASS |
| `## Drain on shutdown` count | 1 | 1 | PASS |
| `## HTTPS / SSRF posture` count | 1 | 1 | PASS |
| `## Metrics family` count | 1 | 1 | PASS |
| ` ```mermaid ` count | ≥ 2 | 5 | PASS |
| `Threat Model 5` count | ≥ 1 | 1 | PASS |
| `webhook_drain_grace + 10s` count | ≥ 1 | 1 | PASS |
| `cronduit_webhook_delivery_sent_total` count | ≥ 1 | 1 | PASS |
| `cronduit_webhook_deliveries_total` count | ≥ 3 | 5 | PASS |
| `fd00::/8` count | ≥ 1 | 2 | PASS |
| ASCII-art diagrams in new sections | 0 | 0 | PASS |
| `../THREAT_MODEL.md` link | ≥ 1 | 1 | PASS |

All 16 acceptance criteria passing.

## Deviations from Plan

### Auto-fixed issues

**1. [Rule 1 — Doc Coherence] Updated Table of Contents to include the 6 new sections**
- **Found during:** Task 1 (immediately after appending content)
- **Issue:** The plan said "APPEND (do NOT replace) 6 new sections at the end" but did not mention updating the existing TOC. The existing TOC at lines 11..22 explicitly enumerates sections 1..10 by number; leaving it stale would mean operators landing on the doc see a TOC that disagrees with the document body — a Rule 1 doc-coherence defect.
- **Fix:** Added items 11..16 to the TOC, mirroring the section headings in append order. The "do NOT replace" instruction was honored — only added; nothing changed in lines 1..22 prior content other than the addition.
- **Files modified:** `docs/WEBHOOKS.md` (TOC list)
- **Commit:** `124a9e6` (same as Task 1 — single atomic commit)

No other deviations. The 6 sections were appended verbatim from the plan's `<action>` block.

## Authentication gates

None. This was a docs-only plan; no external services touched.

## Commits

| Hash | Type | Description |
|------|------|-------------|
| `124a9e6` | docs | docs(20-07): extend WEBHOOKS.md with 6 Phase 20 operator sections |

## Self-Check

Verified:
- File exists: `docs/WEBHOOKS.md` — FOUND
- Commit `124a9e6` — FOUND in `git log`
- All 6 new section headings present at expected line numbers (300, 360, 397, 468, 533, 584).
- 5 ` ```mermaid ` blocks (3 inherited from P19 + 2 new — verified by tail-grep).
- TM5 forward-pointer link `../THREAT_MODEL.md` resolves at the project root (`THREAT_MODEL.md` exists; the link is operator-navigation and works from `docs/`).

## Self-Check: PASSED
