---
phase: 24-milestone-close-out-final-v1-2-0-ship
plan: 01
subsystem: threat-model + readme-security-overview
tags: [paperwork, threat-model, security, close-out, audit-predicate]
dependency_graph:
  requires:
    - "Phase 15 webhook delivery worker isolation (bounded mpsc + dedicated worker)"
    - "Phase 17 Docker labels validators (reserved-namespace + type-gate + size limits)"
    - "Phase 18 Standard Webhooks v1 payload + state filter + edge-triggered coalescing"
    - "Phase 19 HMAC-SHA256 signing + base64 signature header + receiver examples"
    - "Phase 20 HTTPS-required validator + strip_url_credentials + full-jitter retry + 30s drain + webhook_deliveries dead-letter + cronduit_webhook_* metrics"
    - "Pitfall 56 (CRITICAL) — canonical STRIDE-row + section-spec source"
  provides:
    - "Canonical Threat Model 5 (Webhook Outbound) in THREAT_MODEL.md"
    - "New Threat Model 6 (Operator-supplied Docker labels) in THREAT_MODEL.md"
    - "Four new STRIDE summary-table rows: T-S3 / T-T4 / T-I4 / T-D4"
    - "Changelog row for v1.2 close-out + Revision bump"
    - "README §Security link-back anchors to TM5 + TM6"
    - "Closes Pitfall 56 audit predicates T-V12-XCUT-05 / T-V12-XCUT-06 / T-V12-XCUT-07"
  affects:
    - "Downstream plan 24-04 README updates (must not reintroduce the prior single-sentence §Security pointer)"
    - "Plan 24-06 RC4 PREFLIGHT § 5 close-out audit-predicate verification (TM5/TM6 anchors + STRIDE rows + README link-back)"
tech_stack:
  added: []
  patterns:
    - "Doc-only — replace-in-place rewrite + sibling peer-section addition + table-row inserts"
    - "Anchor-derived markdown links (GitHub-Markdown H2-derived: lowercase, dash-separated, punctuation stripped)"
key_files:
  created: []
  modified:
    - "THREAT_MODEL.md"
    - "README.md"
decisions:
  - "Followed D-03 (REPLACE-IN-PLACE): rewrote THREAT_MODEL.md TM5 stub at L189-229 area as canonical TM5 matching TM1-TM4 structure exactly; no stub framing, no Phase 24 Close-Out forward-pointer subsection, no (v1.2.0) suffix on Mitigations heading, no `Accepted` prefix on Residual Risk."
  - "Followed D-04 (HYBRID literalism): used verbatim Pitfall 56 text for the four STRIDE summary-table rows (T-S3/T-T4/T-I4/T-D4); used Pitfall 56's 5-section skeleton (Threat / Attack Vector / Mitigations / Residual Risk / Recommendations) for TM5 and TM6; authored the narrative bodies fresh to ground in v1.2.0 shipped reality with phase-by-phase mitigation citations."
  - "Followed D-05 (peer TM6): TM6 lands as a standalone peer section AFTER TM5; TM5 Recommendations cross-links to TM6; TM6 cross-links to TM3 (Config Tamper) since the config-tamper mitigation stack underpins TM6."
  - "Followed D-06 (single-plan bundling): TM doc edits AND README §Security link-back land in the same plan (separate atomic commits per task); single coherent close-out diff covers Pitfall 56 audit predicates T-V12-XCUT-05/06/07."
  - "Pitfall 56 audit-predicate literalism (D-13 from CONTEXT § Specifics): TM6 heading uses lowercase `labels` (`Operator-supplied Docker labels`) — NOT capitalized `Labels` — to exactly match the T-V12-XCUT-05 audit string."
  - "TM5 section title drops the `(SSRF Accepted Risk)` suffix the stub carried — matches the T-V12-XCUT-05 audit predicate `Threat Model 5: Webhook Outbound` exactly."
  - "Revision bumped to `2026-05-17 (Phase 24 — v1.2.0 close-out)` (UTC day-of-month at commit time)."
  - "No mermaid diagrams added for TM5/TM6 — matches the closer stubless precedent of TM3/TM4 which are also diagram-light; TM5/TM6 narrative is sufficient. Mermaid would be additive noise here."
metrics:
  duration: "~5 minutes (323 seconds)"
  completed: "2026-05-17"
---

# Phase 24 Plan 01: Threat Model Close-Out — Canonical TM5 + New TM6 + STRIDE Rows + README §Security Link-Back Summary

**One-liner:** Replace-in-place rewrite of the THREAT_MODEL.md TM5 stub at L189-229 area as canonical Threat Model 5 (Webhook Outbound), add new peer Threat Model 6 (Operator-supplied Docker labels), insert four STRIDE summary-table rows (T-S3 / T-T4 / T-I4 / T-D4) verbatim from Pitfall 56, append a Phase 24 close-out Changelog row + Revision bump, and widen the README §Security pointer to include link-back anchors to both new TM sections — closing Pitfall 56 audit predicates T-V12-XCUT-05/06/07 in two atomic commits inside the close-out PR.

## What Was Built

### Task 1 — `THREAT_MODEL.md` close-out (commit `00a3cb4`)

Edits confined to:

| Edit site | Before | After | Line range (post-edit) |
|---|---|---|---|
| Revision line | `**Revision:** 2026-04-12 (Phase 6 -- complete)` | `**Revision:** 2026-05-17 (Phase 24 — v1.2.0 close-out)` | L3 |
| TM5 section | L189-229 stub (`(SSRF Accepted Risk)` suffix + `Words-only stub` blockquote preamble + `### Mitigations (v1.2.0)` heading + `### Accepted Residual Risk` heading + `### Phase 24 Close-Out` forward-pointer subsection + "stub is the holding signal" footer) | Canonical TM5 matching TM1-TM4 structure exactly (`## Threat Model 5: Webhook Outbound` → Threat / Attack Vector / Mitigations / Residual Risk / Recommendations with cross-link to TM6) | L189-225 |
| TM6 section (NEW) | n/a | `## Threat Model 6: Operator-supplied Docker labels` peer section (Threat / Attack Vector / Mitigations / Residual Risk / Recommendations) — lowercase `labels` per T-V12-XCUT-05 | L227-259 |
| STRIDE Spoofing table row insertion | L240-242 (T-S1/T-S2 only) | + T-S3 (Pitfall 56 verbatim) | L271 (insertion site) |
| STRIDE Tampering table row insertion | L247-250 (T-T1/T-T2/T-T3) | + T-T4 (Pitfall 56 verbatim) | L280 (insertion site) |
| STRIDE Information Disclosure table row insertion | L261-264 (T-I1/T-I2/T-I3) | + T-I4 (Pitfall 56 verbatim) | L295 (insertion site) |
| STRIDE Denial of Service table row insertion | L269-272 (T-D1/T-D2/T-D3) | + T-D4 (Pitfall 56 verbatim) | L304 (insertion site) |
| Changelog table append | `Phase 20 stub` row last | + `Phase 24 close-out` row at bottom | L329 (last data row) |

TM5 Mitigations narrative cites the shipped implementation phase-by-phase:
- **Phase 15** `src/webhooks/{mod,worker}.rs` bounded `tokio::sync::mpsc::channel(1024)` + dedicated delivery worker; scheduler emits via `try_send`, NEVER awaits.
- **Phase 18** Standard Webhooks v1 payload (`payload_version: "v1"`) + state filter + edge-triggered coalescing (`streak_position == 1` default).
- **Phase 19** HMAC-SHA256 signing with per-job env-sourced `SecretString` + base64 signature header + receiver examples under `docs/webhooks/receivers/` (Python/Go/Node).
- **Phase 20** `src/config/validate.rs::check_webhook_url` HTTPS-required validator + `strip_url_credentials` (Pitfall 38) + full-jitter retry (`t=0 / t=30s / t=300s` with `rand 0.8-1.2×`) + 30s graceful drain + `webhook_deliveries` dead-letter table + `cronduit_webhook_{success,failed,retried,dropped}_total` Prometheus counters.
- **Loopback-bound default + reverse-proxy fronting** (carryover from Threat Model 2 cross-link).

TM5 Residual Risk preserves Pitfall 56's "any URL the cronduit container can reach is reachable from the webhook worker; the operator is responsible for network controls (firewall / network namespaces / outbound deny-list at the gateway layer)" framing verbatim. Recommendations defer destination allow/block-list filter to v1.3 (PROJECT.md § Future Requirements) with cross-link to TM6.

TM6 Mitigations narrative cites Phase 17:
- `src/config/validate.rs::check_labels_reserved` — config-load rejection of any `cronduit.*` reserved-namespace key.
- `src/config/validate.rs::check_labels_only_on_docker_jobs` — type-gate rejection of `[[jobs]].labels` on non-`docker` jobs.
- Per-key + per-value byte-size cap (Phase 17 DoS-surface limit).
- Merge precedence: `use_defaults = false` → replace; default → merge with per-job-wins on collision.
- Read-only config mount + standard Unix permissions (cross-link to TM3).

TM6 Residual Risk: operator-controlled label values flow into the Docker daemon and become visible to downstream tooling (Traefik / Watchtower / log shippers / Prometheus container-discovery); Cronduit does not sanitize for those downstream surfaces. TM3's residual risk inherits.

TM6 Recommendations: read-only mount, audit `[defaults].labels` + per-job labels with the same care as image changes, scope downstream label-filter rules narrowly, defer per-label / total-bytes / cardinality size-limit refinements to v1.3, clobber recovery is operator-removes-TOML-key + restart.

### Task 2 — `README.md` §Security link-back (commit `312a472`)

Single sentence edit at the §Security paragraph (`README.md:31`):

**Before:**
```
See [THREAT_MODEL.md](./THREAT_MODEL.md) for the full threat model covering Docker socket access, untrusted clients, config tampering, and malicious images.
```

**After:**
```
See [THREAT_MODEL.md](./THREAT_MODEL.md) for the full threat model covering Docker socket access, untrusted clients, config tampering, malicious images, [webhook outbound (SSRF)](./THREAT_MODEL.md#threat-model-5-webhook-outbound), and [operator-supplied Docker labels](./THREAT_MODEL.md#threat-model-6-operator-supplied-docker-labels).
```

Anchors derived from the TM5/TM6 H2 headings authored in Task 1 (lowercase, dash-separated, punctuation stripped — GitHub-Markdown's standard anchor derivation).

## Pitfall 56 Audit-Predicate Closures

| Predicate | Requirement | Closed by | Verified |
|---|---|---|---|
| **T-V12-XCUT-05** | `THREAT_MODEL.md` contains "Threat Model 5: Webhook Outbound" AND "Threat Model 6: Operator-supplied Docker labels" sections | Task 1 (commit `00a3cb4`) | `grep -c "^## Threat Model 5: Webhook Outbound$" THREAT_MODEL.md` returns `1`; `grep -c "^## Threat Model 6: Operator-supplied Docker labels$" THREAT_MODEL.md` returns `1` |
| **T-V12-XCUT-06** | STRIDE table contains rows T-S3, T-T4, T-I4, T-D4 | Task 1 (commit `00a3cb4`) | `grep -q "| T-S3 |"`, `grep -q "| T-T4 |"`, `grep -q "| T-I4 |"`, `grep -q "| T-D4 |"` all return success |
| **T-V12-XCUT-07** | README links to the new threat-model sections from the security overview | Task 2 (commit `312a472`) | `grep -q "#threat-model-5-webhook-outbound" README.md` AND `grep -q "#threat-model-6-operator-supplied-docker-labels" README.md` both return success |

## Files Created/Modified

**Modified (2 files, both via in-place edits — no new files created in this plan):**
- `THREAT_MODEL.md` — 1 file changed, 59 insertions(+), 26 deletions(-) (commit `00a3cb4`)
- `README.md` — 1 file changed, 1 insertion(+), 1 deletion(-) (commit `312a472`)

## Decisions Made

1. **D-03 REPLACE-IN-PLACE** — rewrote TM5 stub in place (no archival "stub vs canonical" duplication). Stripped all four stub framings: `(SSRF Accepted Risk)` heading suffix, `Words-only stub` blockquote preamble, `### Mitigations (v1.2.0)` suffix, `### Accepted Residual Risk` heading; removed entire `### Phase 24 Close-Out` forward-pointer subsection and "stub is the holding signal" footer. Section now matches TM1-TM4 structure exactly.
2. **D-04 HYBRID literalism** — used Pitfall 56 verbatim text for the four STRIDE summary-table rows; used Pitfall 56's 5-section skeleton for TM5 + TM6; authored narrative bodies fresh grounded in v1.2.0 shipped reality with phase-by-phase mitigation citations (Phase 15 / 17 / 18 / 19 / 20).
3. **D-05 TM6 anchor** — TM6 lands as a standalone peer section AFTER TM5 (not as a subsection of TM2 or TM3). Cross-links: TM5 Recommendations → TM6; TM6 Mitigations + Residual Risk → TM3 (Config Tamper).
4. **D-06 single-plan bundling** — TM doc edits AND README §Security link-back in the same plan (two atomic commits within the plan). Single coherent threat-model close-out diff.
5. **TM6 lowercase `labels`** — heading uses `Operator-supplied Docker labels` (NOT capitalized `Labels`) to exactly match Pitfall 56 audit predicate T-V12-XCUT-05's literal string. Capitalized `Labels` would FAIL the predicate.
6. **TM5 title without `(SSRF Accepted Risk)` suffix** — matches Pitfall 56 audit predicate T-V12-XCUT-05's literal section title. The stub had the suffix because it was a stub.
7. **Revision date `2026-05-17`** — UTC day-of-month at commit time per the PATTERNS § Plan 24-01 instruction to substitute `NN` with actual day.
8. **No mermaid diagrams for TM5/TM6** — matches the closer stubless precedent of TM3/TM4 which are also diagram-light. Narrative is sufficient.

## Deviations from Plan

None — plan executed exactly as written. Both tasks ran clean, the automated grep verification gate passed on first execution for each task, no Rule 1/2/3 auto-fixes needed, no Rule 4 architectural questions surfaced. Diff is confined to:

- `THREAT_MODEL.md` Revision line (L3), TM5 rewrite range (L189-225), new TM6 section (L227-259), four STRIDE summary-table rows (L271, L280, L295, L304), one Changelog row (L329).
- `README.md` §Security paragraph single sentence (L31).

TM1-TM4 sections, Assets and Trust Boundaries section, STRIDE Repudiation + Elevation of Privilege tables, Out-of-Band Trust Assumptions section, and pre-existing Changelog rows are all preserved verbatim.

## Authentication Gates

None — pure doc-edit plan, no external service interactions, no auth flows touched.

## Verification

| Gate | Result |
|---|---|
| Task 1 automated grep gate (12 conditions, all required) | PASS on first run |
| Task 2 automated grep gate (2 anchor substrings required) | PASS on first run |
| Overall plan: `grep -c "## Threat Model" THREAT_MODEL.md >= 6` | PASS (returned `6`) |
| Overall plan: `grep "Phase 24 close-out" THREAT_MODEL.md` matches in Changelog | PASS |
| Overall plan: `grep -E "threat-model-(5-webhook-outbound|6-operator-supplied-docker-labels)" README.md` matches both anchors | PASS |
| Post-commit deletion check (Task 1) | No deletions |
| Post-commit deletion check (Task 2) | No deletions |

## Self-Check: PASSED

**Files verified (both modifications committed to worktree branch `worktree-agent-a4f11ba359f11bf00`):**
- `THREAT_MODEL.md` — FOUND, contains canonical `## Threat Model 5: Webhook Outbound`, `## Threat Model 6: Operator-supplied Docker labels`, all four STRIDE rows (T-S3/T-T4/T-I4/T-D4), Phase 24 close-out Changelog row, and Revision bump to `2026-05-17 (Phase 24 — v1.2.0 close-out)`.
- `README.md` — FOUND, §Security sentence includes both `#threat-model-5-webhook-outbound` and `#threat-model-6-operator-supplied-docker-labels` anchor substrings.

**Commits verified (`git log --oneline -3`):**
- FOUND: `00a3cb4` — `docs(24-01): threat model close-out — canonical TM5 + new TM6 + STRIDE rows + Changelog`
- FOUND: `312a472` — `docs(24-01): widen README §Security link-back to TM5 + TM6 anchors`

**No missing items.**
