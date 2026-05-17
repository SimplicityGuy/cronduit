# Phase 24: Milestone Close-Out — final `v1.2.0` ship — Pattern Map

**Mapped:** 2026-05-16
**Files analyzed:** 13 (8 paperwork/CI doc-edit files + 3 NEW maintainer runbooks + 2 derived bookkeeping files)
**Analogs found:** 13 / 13 (100%) — Phase 24 is paperwork close-out; every artifact has a direct in-tree analog locked by CONTEXT.md.
**Reading scope:** Only the read-first analogs cited below. NO Rust source changes; no `src/` edits.

> **Read-only constraint reminder:** Phase 24 mutates only paperwork (`.md`), one CI workflow toggle (`continue-on-error: true → false`), and three NEW maintainer runbooks. No `src/` files are edited. The planner must NOT spawn any plan whose action steps include `src/**.rs` edits.

---

## File Classification

| New/Modified File | Plan | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|------|-----------|----------------|---------------|
| `THREAT_MODEL.md` (TM5 rewrite L189-229 + new TM6 + STRIDE rows + Changelog) | 24-01 | threat-model-doc | request-response (audit-predicate) | `THREAT_MODEL.md:43-186` (TM1–TM4 canonical sections) + `.planning/research/PITFALLS.md:1099-1145` (Pitfall 56 STRIDE-row spec source) | **exact** (in-place rewrite within same file using sibling sections as structural template) |
| `README.md` §Security link-back (plan 24-01) | 24-01 | docs-readme | request-response | `README.md:19-33` (existing §Security pointing at `THREAT_MODEL.md`) | exact (add 2 anchor links to existing § footer) |
| `.planning/milestones/v1.2-MILESTONE-AUDIT.md` (NEW) | 24-02 | milestone-audit | batch (3-source cross-reference) | `.planning/milestones/v1.0-MILESTONE-AUDIT.md` (the ONLY existing structural precedent; v1.1 has no audit doc) | **exact** (frontmatter + 6-section body shape mirrored verbatim with v1.2 content) |
| `.planning/REQUIREMENTS.md` (20 unchecked → `[x] Validated`) | 24-02 | requirements-flip | batch (mechanical derive) | `.planning/REQUIREMENTS.md:27,29,31` (FOUND-14/15/16 already-ticked rows with `T-V12-FCTX-01` audit-predicate suffix) | exact (same tick + phase-ref + audit-predicate-suffix shape applied to 20 rows) |
| `.planning/ROADMAP.md` (P17 'Complete' / P21 11/11 / P22 6/6 / § Phases ticks) | 24-02 | roadmap-drift | batch (mechanical derive) | `.planning/ROADMAP.md` § Progress (the existing tracker table currently showing drift) | role-match (no v1.1 precedent exists in archive; flip is mechanical from audit verdict) |
| `MILESTONES.md` (new v1.2 entry at top of file) | 24-03 | release-log-entry | request-response | `MILESTONES.md:7-15` (v1.1 entry) + `MILESTONES.md:18-25` (v1.0 entry) | **exact** (header / paragraph / Tags / Phases / Requirements / Audit — same 6-row shape) |
| `README.md` (§Features / §Configuration / v1.2 'What's New' hero / MILESTONES cross-link) | 24-04 | docs-readme | request-response | `README.md:206-285` (§Labels, P17) + `README.md:287-315` (§Tag Filter Chips, P23) | exact (cumulative §Configuration subsection pattern applied for webhooks; hero block is novel but small) |
| `.github/workflows/ci.yml` (cargo-deny `continue-on-error: true → false`) | 24-05 | ci-workflow | event-driven (CI gate) | `.github/workflows/ci.yml:47-58` (the cargo-deny block itself, with embedded comment at L51-53 explicitly forecasting this Phase 24 flip) | **exact** (single-line removal; the surrounding comment block was authored in P15 with this exact flip in mind) |
| `deny.toml` (conditional, only if accumulated advisory) | 24-05 | ci-config | event-driven (CI gate) | `deny.toml` (existing file — exception entries) | role-match (planner picks `deny.toml` allowlist vs `Cargo.lock` rev per advisory remediation discretion in D-11) |
| `Cargo.lock` (conditional, only if dep rev needed) | 24-05 | ci-dep-rev | event-driven (CI gate) | n/a — mechanical `cargo update -p <crate>` output | n/a |
| `24-RC4-PREFLIGHT.md` (NEW) | 24-06 | maintainer-runbook | request-response (checklist) | `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-RC2-PREFLIGHT.md` (verbatim mirror per CONTEXT D-10) + `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-RC3-PREFLIGHT.md` (most-recent sibling, same shape) | **exact** (9-section structure, `autonomous: false`, frontmatter, sign-off table — substitute `rc.2/rc.3 → rc.4` + `P21/P23 → P24` + plan-list 01-10/01-07 → 01-08 + close-out-specific § 5 verification) |
| `24-HUMAN-UAT.md` (NEW) | 24-07 | maintainer-runbook | request-response (checklist) | `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-HUMAN-UAT.md` (six-scenario shape) + Phase 14 D-17 (v1.1 final-ship UAT shape) | exact (6-scenario shape adapted: regression-smoke + 5 v1.2 features; every step references `just uat-*` recipe per memory `feedback_uat_use_just_commands.md`) |
| `24-FINAL-SHIP-PREFLIGHT.md` (NEW) | 24-08 | maintainer-runbook | request-response (checklist) | `.planning/milestones/v1.1-phases/14-bulk-enable-disable-rc-3-final-v1-1-0-ship/14-CONTEXT.md` § D-16 (retag-the-rc-SHA bit-identical discipline) | **exact** ("v1.2.0 = retag the last-passing-rc SHA" mirrors "v1.1.0 = retag the rc.3 SHA" verbatim; verify `:latest` hyphen-gate from P12 D-10) |
| `.planning/STATE.md` (milestone status SHIPPED) | 24-08 | state-marker | event-driven | `.planning/STATE.md` (existing schema) | role-match (single status-line bump) |

---

## Pattern Assignments

### Plan 24-01 — `THREAT_MODEL.md` close-out + `README.md` §Security link-back

**Analogs:**
1. `THREAT_MODEL.md:43-186` — TM1–TM4 canonical structural pattern (the rewrite skeleton TM5/TM6 must follow exactly).
2. `.planning/research/PITFALLS.md:1099-1145` — Pitfall 56 (canonical STRIDE-row + section-spec source per D-04 HYBRID literalism).
3. `THREAT_MODEL.md:233-281` — STRIDE Summary tables (insertion sites for T-S3 / T-T4 / T-I4 / T-D4).
4. `THREAT_MODEL.md:294-300` — Changelog table (insertion site for v1.2 close-out row).
5. `README.md:19-33` — existing §Security section (link-back insertion site).

**TM1 canonical section structure** (`THREAT_MODEL.md:43-83`) — copy this skeleton for TM5 rewrite + new TM6:

```markdown
## Threat Model 1: Docker Socket

[optional mermaid diagram]

### Threat
[1 paragraph]

### Attack Vector
[1 paragraph or numbered list]

### Mitigations
- **Bold lead-in:** explanation paragraph.
- **Bold lead-in:** explanation paragraph.
- [3-5 bullets total]

### Residual Risk
[1 paragraph]

### Recommendations
- bullet
- bullet
- bullet
```

**Pitfall 56 STRIDE rows** (`.planning/research/PITFALLS.md:1127-1131`) — use this LITERAL text per D-04:

```markdown
### Spoofing (S)
| T-S3 | Attacker forges webhook payload | Mitigated by HMAC signing; out-of-scope: receiver-side verification (operator's responsibility) |

### Tampering (T)
| T-T4 | Attacker injects label collision into `cronduit.*` namespace | Mitigated by validator |

### Information Disclosure (I)
| T-I4 | Webhook URL embeds credentials in `userinfo` | Mitigated by `strip_url_credentials` (Pitfall 38) |

### Denial of Service (D)
| T-D4 | Webhook receiver outage stalls scheduler loop | Mitigated by bounded mpsc + delivery worker isolation (Pitfall 28) |
```

**Existing STRIDE row shape** for reference (`THREAT_MODEL.md:241`):

```markdown
| T-S1 | LAN attacker accesses unauthenticated web UI | Mitigated: loopback default + startup warning. Full auth deferred to v2. |
```

**Existing Changelog row shape** (`THREAT_MODEL.md:298-300`) — plan 24-01 appends to the bottom:

```markdown
| Phase 1 skeleton | 2026-04-10 | Initial STRIDE outline with Phase 1 mitigations. Phases 4-6 threats marked TBD. |
| Phase 6 complete | 2026-04-12 | Expanded with four threat models (Docker socket, untrusted client, config tamper, malicious image). Updated all STRIDE entries with Phase 2-6 mitigations. Resolved all TBD items. |
| Phase 20 stub    | 2026-05-01 | Added Threat Model 5 (Webhook Outbound) as a words-only stub satisfying WH-08. Canonical close-out (full STRIDE rows + residual-risk language for v1.3 deferred allowlist) is Phase 24's milestone close-out per ROADMAP. |
```

**Plan 24-01 adds** (revision bump at L3 + new row at end of Changelog):

```markdown
**Revision:** 2026-05-NN (Phase 24 — v1.2.0 close-out)

| Phase 24 close-out | 2026-05-NN | TM5 canonical rewrite (replaces v1.2 stub); new TM6 (Operator-supplied Docker Labels); STRIDE rows T-S3/T-T4/T-I4/T-D4 added; v1.2 milestone close. |
```

**Existing TM5 stub to be REPLACED IN-PLACE** (`THREAT_MODEL.md:189-229`) — strip these specific framings per D-03:
- L191: `> **Status:** Words-only stub for v1.2.0 (Phase 20). The canonical close-out … lands in [Phase 24 (Milestone Close-Out)]` preamble (REMOVE).
- L206: `### Mitigations (v1.2.0)` (RENAME to `### Mitigations` — drop the suffix per D-03).
- L214: `### Accepted Residual Risk` (RENAME to `### Residual Risk` — match TM1–TM4 heading exactly).
- L220-229: `### Phase 24 Close-Out` forward-pointer subsection (REMOVE entirely).

**Pitfall 56 audit-predicate language** (CONTEXT § Specifics, `THREAT_MODEL.md` audit predicates) — **USE EXACTLY**:

- `## Threat Model 5: Webhook Outbound` (drop the `(SSRF Accepted Risk)` suffix on canonical — match Pitfall 56 audit string per T-V12-XCUT-05; existing stub has the suffix because it was a stub).
- `## Threat Model 6: Operator-supplied Docker labels` (note lowercase `labels` per Pitfall 56 audit predicate verbatim; do NOT use `Labels` capitalized).

**README §Security link-back** — `README.md:31` currently reads:

```markdown
See [THREAT_MODEL.md](./THREAT_MODEL.md) for the full threat model covering Docker socket access, untrusted clients, config tampering, and malicious images.
```

Plan 24-01 widens to include TM5 + TM6 with anchor links (close audit predicate T-V12-XCUT-07):

```markdown
See [THREAT_MODEL.md](./THREAT_MODEL.md) for the full threat model covering Docker socket access, untrusted clients, config tampering, malicious images, [webhook outbound (SSRF)](./THREAT_MODEL.md#threat-model-5-webhook-outbound), and [operator-supplied Docker labels](./THREAT_MODEL.md#threat-model-6-operator-supplied-docker-labels).
```

---

### Plan 24-02 — `.planning/milestones/v1.2-MILESTONE-AUDIT.md` + REQUIREMENTS flips + ROADMAP drift

**Analog:** `.planning/milestones/v1.0-MILESTONE-AUDIT.md` (sole structural precedent; v1.1 has no audit doc — note this drift in § Tech Debt Summary per CONTEXT § Deferred).

**Audit-doc frontmatter shape** (`.planning/milestones/v1.0-MILESTONE-AUDIT.md:1-78`):

```yaml
---
milestone: v1.2
audited: 2026-05-NNTNN:NN:NNZ
status: passed | tech_debt
scores:
  requirements: 41/41 Complete
  phases: 10/10 complete on disk (NN/NN plans across Phases 15-24)
  integration: N/N cross-phase wiring paths confirmed
  flows: N/N end-to-end flows complete
  nyquist:
    compliant: 10
    partial_or_draft: 0
    missing: 0
    overall: passed
gaps:
  requirements: []
  integration: []
  flows: []
  orphans: []
tech_debt: []
nyquist:
  compliant_phases: [15, 16, 17, 18, 19, 20, 21, 22, 23, 24]
  partial_phases: []
  missing_phases: []
  overall: "passed — …"
---
```

**Body section order** (`.planning/milestones/v1.0-MILESTONE-AUDIT.md:80-235`) — mirror verbatim:

1. H1: `# v1.2 Milestone Audit` + 4-line subheader (Milestone / Audited / Prior audits / Verdict).
2. `## Score Summary` (5-row dimension table).
3. `## 1. Requirements Coverage — 3-Source Cross-Reference` (Satisfied / Partial / Unsatisfied / Orphans subsections).
4. `## 2. Phase Verifications — Status Matrix` (one row per phase 15–24).
5. `## 3. Cross-Phase Integration — Wiring Paths` (cross-phase REQ-ID lookups).
6. `## 4. End-to-End Flows` (E2E flow table).
7. `## 5. Nyquist Compliance` (per-phase nyquist_compliant lookup).
8. `## 6. Tech Debt Summary` (zero-state for clean ship; otherwise enumerated).
9. `## Verdict Routing` (`passed` or `tech_debt`).
10. `## ▶ Next Up` (`/gsd-complete-milestone v1.2` command call-out).

**Score Summary row shape** (`.planning/milestones/v1.0-MILESTONE-AUDIT.md:89-97`):

```markdown
| Dimension | Score | Status |
|---|---|---|
| Requirements | 86/86 Complete | ✅ `passed` |
| Phases | 9/9 complete on disk, 49/49 plans | ✅ `complete` |
| Integration | 9/9 wiring paths confirmed | ✅ `passed` |
| E2E Flows | 7/7 complete | ✅ `passed` |
| Nyquist compliance | 9/9 phases compliant | ✅ `passed` |
```

**REQUIREMENTS.md flip shape — pre-flip** (`.planning/REQUIREMENTS.md:37,49`):

```markdown
- [ ] **WH-01**: Operators can configure a webhook URL per job (`webhook = { url = "https://...", states = ["failed", "timeout", "stopped"] }`) … `T-V12-WH-01`, `T-V12-WH-02`.
- [ ] **WH-07**: Webhook URL validation: `https://` is required for non-loopback / non-RFC1918 destinations. … `T-V12-WH-15`, `T-V12-WH-16`.
```

**Post-flip** — mirror the already-ticked `FOUND-14` shape at L27 (NO appended phase-ref needed; the audit-predicate suffix is the evidence link, and the AUDIT.md `requirements_satisfied` block carries the per-phase reference):

```markdown
- [x] **WH-01**: Operators can configure a webhook URL per job (`webhook = { url = "https://...", states = ["failed", "timeout", "stopped"] }`) … `T-V12-WH-01`, `T-V12-WH-02`.
```

> Planner note: the existing v1.2 ticked rows (FOUND-14/15/16 at L27/29/31, WH-09 at L53, FCTX-01..07 at L79-91, EXIT-01..05 at L97-105, TAG-02 at L115, TAG-06..08 at L123-127) carry NO inline phase-ref text — only the audit-predicate suffix. Plan 24-02 preserves this shape for the 20 new flips.

**ROADMAP drift** — corrections per CONTEXT D-09:

| Tracker location | Pre-flip | Post-flip |
|---|---|---|
| § v1.2 Phase Tracker P17 status | "Gap-closure pending" | "Complete" (consume `17-VERIFICATION-GAP-CLOSURE.md:status: passed, gaps_remaining: []`) |
| § v1.2 Phase Tracker P21 plan count | 10/11 | 11/11 |
| § v1.2 Phase Tracker P22 plan count | 4/6 | 6/6 |
| § Phases P21 / P22 / P24 checkboxes | `[ ]` | `[x]` (P24 ticks AFTER rc.4 ships) |

---

### Plan 24-03 — `MILESTONES.md` v1.2 entry

**Analog:** `MILESTONES.md:7-15` (v1.1 entry — most recent) and `MILESTONES.md:18-25` (v1.0 entry).

**Verbatim v1.1 shape** (`MILESTONES.md:7-15`) to mirror for v1.2:

```markdown
## v1.1 — Operator Quality of Life — SHIPPED 2026-04-23

v1.1 is a polish-and-fix milestone on top of the shipped v1.0.1 codebase. Six phases (10, 11, 12, 12.1, 13, 14) delivered the "stop a running job" capability, per-job run numbers with fixed log UX, a Docker healthcheck that works out of the box, GHCR tag hygiene (`:latest` / `:main` semantics locked), observability polish (cross-job timeline + per-job sparklines + p50/p95 duration trends), and bulk enable/disable ergonomics with a settings-page override audit. No new external dependencies; one new nullable DB column (`jobs.enabled_override`); the scheduler core was not refactored. Released iteratively as `v1.1.0-rc.1` through `v1.1.0-rc.6`, then promoted to `v1.1.0`.

**Tags:** `v1.1.0-rc.1`, `v1.1.0-rc.2`, `v1.1.0-rc.3`, `v1.1.0-rc.4`, `v1.1.0-rc.5`, `v1.1.0-rc.6`, `v1.1.0`
**Phases:** 10 (Stop + Hygiene), 11 (Run Numbers + Log UX), 12 (Healthcheck + rc.1 cut), 12.1 (GHCR tag hygiene), 13 (Observability + rc.2 cut), 14 (Bulk Toggle + rc.3..rc.6 + final)
**Requirements delivered:** 33 across 7 categories (SCHED-09..14, DB-09..14, UI-16..20, OBS-01..05, ERG-01..04, OPS-06..10, FOUND-12..13)
**Audit:** see `.planning/milestones/v1.1-ROADMAP.md`, `.planning/milestones/v1.1-REQUIREMENTS.md`, `.planning/milestones/v1.1-MILESTONE-AUDIT.md` (archived by `/gsd-complete-milestone v1.1`)
```

**Plan 24-03 v1.2 entry skeleton** (substitution table):

| Field | v1.2 value |
|-------|-----------|
| Header | `## v1.2 — Operator Integration & Insight — SHIPPED 2026-05-NN` |
| Summary phrase | "Operator integration & insight milestone" — webhooks (outbound Standard-Webhooks-v1 + HMAC-SHA256), custom Docker labels, failure-context panel, exit-code histogram, job tagging with dashboard filter chips |
| Phases row | 15 (Foundation Preamble), 16 (Webhook Delivery Loop), 17 (Custom Docker Labels), 18 (Webhook Payload + State Filter + Coalescing), 19 (Webhook HMAC + Receiver Examples), 20 (Webhook SSRF/Retry/Drain/Metrics + rc.1), 21 (FCTX UI + Exit Histogram + rc.2), 22 (Job Tagging Schema), 23 (Tag Filter Chips + rc.3), 24 (Milestone Close-Out + rc.4 + final) |
| Tags row | `v1.2.0-rc.1`, `v1.2.0-rc.2`, `v1.2.0-rc.3`, `v1.2.0-rc.4`[, additional rc.N if iterated], `v1.2.0` |
| Requirements delivered | 41 across 6 categories (FOUND-14..16, WH-01..11, LBL-01..06, FCTX-01..07, EXIT-01..06, TAG-01..08) |
| Audit row | `see .planning/milestones/v1.2-ROADMAP.md, .planning/milestones/v1.2-REQUIREMENTS.md, .planning/milestones/v1.2-MILESTONE-AUDIT.md (archived by /gsd-complete-milestone v1.2)` |

> **Placement:** Insert at top of file (immediately after the `---` separator at L5). Pushes v1.1 to L26+ and v1.0 further down. NEVER reorder existing entries.

---

### Plan 24-04 — README §Features / §Configuration / v1.2 'What's New' hero / MILESTONES cross-link

**Analogs:**
1. `README.md:206-285` — §Labels subsection (P17 — cumulative §Configuration addition pattern; ~80 lines including mermaid + tabular merge-precedence + reserved-namespace).
2. `README.md:287-315` — §Tag Filter Chips subsection (P23 — leaner subsection: paragraph + TOML example + bullet list of behaviors + cross-link to `docs/WEBHOOKS.md`).
3. `README.md:19-33` — §Security section (existing top-anchor location for hero block insertion ABOVE per CONTEXT § Specifics "hero block timing — pre-rc.4 only").

**§Labels subsection skeleton** (`README.md:206-211` — opening) — copy for new §Webhooks subsection:

```markdown
### Labels

Cronduit attaches arbitrary Docker labels to spawned containers. Operators use this to integrate cronduit with reverse proxies (Traefik, Caddy), update tooling (Watchtower), backup tooling, and any other Docker ecosystem tool that filters or routes by container label.

Labels are configured in two places — `[defaults].labels` (inherited by every docker job) and per-job `[[jobs]].labels` (merges with or replaces the defaults). The merge precedence is:
```

**§Tag Filter Chips subsection** (`README.md:287-315` — leaner shape — closer match for plan 24-04 §Webhooks since `docs/WEBHOOKS.md` already exists with 649 lines):

```markdown
### Tag Filter Chips

Tag your jobs in TOML; the dashboard auto-renders filter chips for every distinct tag in your fleet. …

Configure tags per job in `cronduit.toml`:

```toml
[[jobs]]
name = "nightly-postgres-backup"
schedule = "0 2 * * *"
image = "postgres:16-alpine"
command = ["pg_dumpall"]
tags = ["backup", "postgres", "nightly"]
```

Tag rules (validated at config-load):
- **Charset:** …
- **Per-job cap:** …
- **Per-job only:** …

Chip strip behavior:
- **Empty state.** …
- **Untagged-hidden.** …
- **Bookmarkable URLs.** …

Tags are also delivered in webhook payloads (the `tags` field of the `run_finalized` event — see [docs/WEBHOOKS.md](./docs/WEBHOOKS.md)). …
```

**Plan 24-04 §Webhooks** — planner picks size between the §Labels depth (~80 lines) and §Tag Filter Chips leaner shape (~30 lines). Per CONTEXT § Claude's Discretion: "Can be a 2-line forward-pointer to `docs/WEBHOOKS.md` or a brief TOML example mirroring §Labels (`README.md:206`). Planner judges based on `docs/WEBHOOKS.md` shape." Given `docs/WEBHOOKS.md` is 649 lines (verified above), the README subsection should be brief (~25-40 lines: 1 intro paragraph + 1 TOML example + 3-5 behavior bullets + forward-pointer).

**§Features pointer for FCTX panel + exit-code histogram** — no analog: these are new subsections. Reference existing §Monitoring at `README.md:366` as the closest in-tree placement. Planner may add as sub-bullets within an enclosing v1.2 highlights paragraph rather than full subsections.

**Hero block** — no in-tree analog. CONTEXT § Specifics permits `<details>` collapsible block OR mermaid timeline OR single highlights paragraph. Planner's call. Insert ABOVE L19 (`## Security`).

**MILESTONES.md cross-link** — add either to §Security footer or as a new §Releases footer at end of README. The link target is `./MILESTONES.md`; the GitHub Releases page is `https://github.com/SimplicityGuy/cronduit/releases`.

---

### Plan 24-05 — `.github/workflows/ci.yml` cargo-deny `continue-on-error: true → false` (FOUND-16 / D-11)

**Analog:** `.github/workflows/ci.yml:47-58` — the cargo-deny block itself, with embedded comment explicitly forecasting this flip:

```yaml
      # Phase 15 / FOUND-16. cargo-deny supply-chain check (advisories +
      # licenses + duplicate-versions). Non-blocking on rc.1 per D-09 — the
      # step is marked continue-on-error: true so a transient advisory or
      # transitive duplicate-version finding cannot redden CI in v1.2 hands.
      # Promoted to blocking (single-line removal of continue-on-error)
      # before final v1.2.0 ships in Phase 24. Pairs with deny.toml's
      # `bans.multiple-versions = "warn"` for two-layer non-blocking (D-10).
      - uses: taiki-e/install-action@v2
        with:
          tool: cargo-deny
      - run: just deny
        continue-on-error: true
```

**Plan 24-05 single-line change** at L58:

```yaml
      - run: just deny
        continue-on-error: false   # P24 / FOUND-16 — promoted to blocking before final v1.2.0
```

Or — preferred — remove the line entirely (the comment at L47-53 explicitly describes this as "single-line removal of continue-on-error"). Update the comment block at L47-53 to past-tense:

```yaml
      # Phase 15 / FOUND-16. cargo-deny supply-chain check (advisories +
      # licenses + duplicate-versions). PROMOTED TO BLOCKING in Phase 24 per
      # the original FOUND-16 spec. Pairs with deny.toml's
      # `bans.multiple-versions = "warn"` for layered defense (D-10).
      - uses: taiki-e/install-action@v2
        with:
          tool: cargo-deny
      - run: just deny
```

**Two-branch outcome** per CONTEXT § Specifics (D-11):
- **Branch A (uneventful):** `cargo deny check` already green at WARN level → flip → done.
- **Branch B (eventful):** advisory accumulated since rc.1 → fix via `deny.toml` exception (planner picks: allowlist with documented expiry comment) OR `Cargo.lock` rev (`cargo update -p <crate>`) → re-run CI → flip → done.

**No analog for `deny.toml` edits** — the existing `deny.toml` carries the v1.0/v1.1 license + advisory + duplicate-version stance. Planner inspects `cargo deny check` output at execution time and decides between exception (timestamped expiry comment) vs dep-rev (with upstream link).

---

### Plan 24-06 — NEW `24-RC4-PREFLIGHT.md` (autonomous=false maintainer runbook)

**Analogs (both verbatim mirror targets per CONTEXT D-10):**
1. `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-RC2-PREFLIGHT.md` (~206 lines) — most-cited in CONTEXT.
2. `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-RC3-PREFLIGHT.md` (~206 lines) — most-recent sibling.

**Frontmatter shape** (`21-RC2-PREFLIGHT.md:1-9` / `23-RC3-PREFLIGHT.md:1-11`):

```yaml
---
phase: 24
plan: 06
type: rc-preflight
autonomous: false
rc_tag: v1.2.0-rc.4
created: 2026-05-NN
status: pending-maintainer-execution
---
```

**9-section structure** (`21-RC2-PREFLIGHT.md` body, `23-RC3-PREFLIGHT.md` body — identical):

1. **§ 1. Phase 24 plans merged on `main`** — per-plan PR checklist for 24-01..24-05 (5 PRs) + verification `gh pr list` commands.
2. **§ 2. CI matrix green on `main`** — 6 row checklist (4 matrix × {SQLite, Postgres} on {amd64, arm64}, compose-smoke, cargo-deny). **DIVERGENCE:** for rc.4 the cargo-deny row reads "**BLOCKING** (promoted in P24 plan 24-05 — FOUND-16 closed)" instead of "still non-blocking on rc.N — promotion to blocking is Phase 24".
3. **§ 3. rustls invariant intact** — `cargo tree -i openssl-sys` returns empty.
4. **§ 4. release.yml `:latest` gate logic intact** — verify hyphen-gate from P12 D-10 still at the expected lines; no commits touch `release.yml` / `cliff.toml` / `docs/release-rc.md`.
5. **§ 5. CLOSE-OUT-SPECIFIC verification** — replace P21's "EXIT-06 cardinality discipline" and P23's "Tags-as-Prometheus-label out-of-scope" with a v1.2-close-out audit-predicate check: TM5 + TM6 exist in `THREAT_MODEL.md`; STRIDE rows T-S3/T-T4/T-I4/T-D4 exist; README §Security links to TM5/TM6 anchors. Verifies T-V12-XCUT-05/06/07 closed.
6. **§ 6. git-cliff release-notes preview** — `git cliff --unreleased --tag v1.2.0-rc.4` shows close-out PR commits (small delta since rc.3 — likely 3-5 commits).
7. **§ 7. 24-HUMAN-UAT.md sign-off** — all six scenarios ticked + Sign-off block filled.
8. **§ 8. Tag command (maintainer runs LOCALLY)** — same git tag commands as P21/P23, message swapped: `"v1.2.0-rc.4 — milestone close-out (P24)"`.
9. **§ 9. Post-publish verification** — same as P21/P23 with the same `:latest` invariant detection assertion: digest of `:latest` == digest of `:1.1.0` (rc.4 is still hyphenated; `:latest` does NOT promote yet).

**Sign-off table shape** (`21-RC2-PREFLIGHT.md:187-196`):

```markdown
| Field | Value |
|-------|-------|
| Maintainer signature | `__________________` |
| Date (UTC) | `__________________` |
| Tag commit SHA | `__________________` |
| GHCR amd64 digest | `__________________` |
| GHCR arm64 digest | `__________________` |
| GHCR `:latest` digest (must equal `v1.1.0` digest) | `__________________` |
| GHCR `:1.1.0` digest (for comparison) | `__________________` |
| GHCR `:rc` digest (must equal `v1.2.0-rc.4` digest) | `__________________` |
```

**Out-of-scope footer** (`21-RC2-PREFLIGHT.md:158-167`) — copy verbatim with rc.4 substitution.

**Cross-reference footer** (`21-RC2-PREFLIGHT.md:203-205` and `23-RC3-PREFLIGHT.md:206-207`) — author plan 24-06's equivalent: "this runbook mirrors `21-RC2-PREFLIGHT.md` and `23-RC3-PREFLIGHT.md` with `rc.2/rc.3 → rc.4`, `P21/P23 → P24`, and the EXIT-06 / tags-cardinality verification swapped for the close-out audit-predicate verification (§ 5)."

---

### Plan 24-07 — NEW `24-HUMAN-UAT.md` (autonomous=false maintainer UAT runbook)

**Analog:** `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-HUMAN-UAT.md` (six-scenario shape, `~180 lines`).

**Frontmatter** (`23-HUMAN-UAT.md:1-10`):

```yaml
---
phase: 24
plan: 07
title: "Phase 24 — Milestone Close-Out — Human UAT"
autonomous: false
maintainer_validated: true
created: 2026-05-NN
requirements: [WH-01..11, LBL-01..06, FCTX-01..07, EXIT-01..06, TAG-01..08]   # full v1.2 regression smoke
status: pending
---
```

**6-scenario shape** (`23-HUMAN-UAT.md:36-170`) — adapted for full v1.2 regression smoke + new-feature UAT per CONTEXT plan 24-07:

| Scenario | Goal | `just` recipe target |
|----------|------|--------------------|
| **1. `docker compose up` quickstart + dashboard renders** | rc.4 image boots healthy in 90s; dashboard renders without regression vs v1.1 | `just uat-quickstart` (or compose-smoke + dashboard-open) |
| **2. v1.0/v1.1 surfaces intact** | filter / sort / Run Now / Stop / bulk toggle / timeline / sparklines / settings overrides all work | existing `just uat-*` recipes from P10/P11/P12/P13/P14 |
| **3. Webhooks end-to-end** | Standard-Webhooks-v1 payload + HMAC-SHA256 + retry/drain — fire a webhook from a docker-compose receiver fixture | `just uat-webhook-e2e` (or new recipe per planner discretion) |
| **4. Custom Docker labels + reserved-namespace error** | merge precedence + `cronduit.*` validator + size-limit at config-load | `just uat-labels-*` (P17 recipes — already exist) |
| **5. FCTX panel + exit histogram** | FCTX panel collapsed-by-default on a failed run with 5 P1 signals; exit-code histogram on job-detail with 10 buckets | `just uat-fctx-panel` + `just uat-exit-histogram` (P21 recipes — already exist) |
| **6. Tag filter chips** | chip strip + AND filter + URL state + mobile reflow + light-mode + keyboard | `just uat-chips-render` + `just uat-chips-and-filter` + `just uat-chips-share-url` (P23 recipes — already exist) |

**Per-scenario shape** (`23-HUMAN-UAT.md:36-49` for Scenario 1):

```markdown
## Scenario N — Title (REQ-IDs / D-NN step)

**Goal:** [1 sentence]

**Steps:**

1. Run `just uat-<recipe>` from a fresh terminal.
2. Follow the recipe's prompts (build → db-reset → write fleet TOML → run cronduit in another terminal → open the dashboard).
3. **Eyeball criterion (a):** [observable behavior]
4. **Eyeball criterion (b):** [observable behavior]

**Sign-off:**

- [ ] Scenario N passed (one-line summary).
```

**Final sign-off block** (`23-HUMAN-UAT.md:172-180`):

```markdown
## Final sign-off

When all six scenarios above are checked:

- [ ] **Maintainer:** I have run all six scenarios on a clean working tree against `v1.2.0-rc.4`. Each scenario produced the expected operator-observable behavior. The full v1.2 stack (webhooks, labels, FCTX, exit histogram, tags) plus v1.0/v1.1 regression surfaces work end-to-end. Phase 24 is UAT-complete and ready for the final `v1.2.0` retag.

Maintainer name: __________________
Date: __________________
```

**Per project memory `feedback_uat_use_just_commands.md`:** every scenario step MUST reference an existing `just` recipe — NO ad-hoc `cargo` / `docker` / curl invocations.

---

### Plan 24-08 — NEW `24-FINAL-SHIP-PREFLIGHT.md` (autonomous=false maintainer final-tag runbook)

**Analog:** `.planning/milestones/v1.1-phases/14-bulk-enable-disable-rc-3-final-v1-1-0-ship/14-CONTEXT.md` § D-16 (`L92`) + § D-18 (`L106`) — the v1.1.0 final-tag discipline.

**Verbatim D-16 spec** (`14-CONTEXT.md:92`):

```text
D-16: v1.1.0 = retag the rc.3 SHA (same digest). After HUMAN-UAT on rc.3 passes (D-17), maintainer runs `git tag -a v1.1.0 -m "v1.1 — Operator Quality of Life" <rc.3-SHA>` + `git push origin v1.1.0`. Because the tag points at the rc.3 commit, the image that ships as v1.1.0 is byte-identical to the image UAT validated as rc.3. Guarantees "what was tested is what ships." Rejects new-commit-between-rc.3-and-v1.1.0 (would require re-UAT because the image would differ) and workflow_dispatch-tag-cut (violates Phase 12 D-13 maintainer-key trust anchor).
```

**Plan 24-08 mirrors this for v1.2.0** — substitute `v1.1.0 → v1.2.0`, `rc.3-SHA → last-passing-rc.N-SHA`, message `"v1.1 — Operator Quality of Life" → "v1.2 — Operator Integration & Insight"`.

**Frontmatter** (mirror plan 24-06 / `21-RC2-PREFLIGHT.md:1-9`):

```yaml
---
phase: 24
plan: 08
type: final-ship-preflight
autonomous: false
final_tag: v1.2.0
created: 2026-05-NN
status: pending-maintainer-execution
---
```

**Required sections** (synthesized from `14-CONTEXT.md` D-16/D-18/D-19 + `21-RC2-PREFLIGHT.md` shape):

1. **§ 1. rc.4 UAT passed (or rc.N for the LAST passing-UAT iteration)** — `24-HUMAN-UAT.md` all six scenarios ticked; sign-off filled.
2. **§ 2. Identify rc.N SHA to retag** — `git rev-list -n 1 v1.2.0-rc.N` returns the SHA the final `v1.2.0` tag will point at.
3. **§ 3. Retag command (maintainer runs LOCALLY)** — `git tag -a -s v1.2.0 -m "v1.2 — Operator Integration & Insight" <rc.N-SHA>` + `git push origin v1.2.0`. NB: use the `-s` GPG signing flag per `docs/release-rc.md` Step 2a (or Step 2b unsigned fallback).
4. **§ 4. Post-publish verification: `:latest` hyphen-gate fires** — `release.yml` D-10 (P12) gate at L132-135 publishes `:1.2.0` + `:1.2` + `:1` + `:latest` on both amd64 + arm64 because `v1.2.0` contains NO hyphen. Verify all FOUR tags advance via `docker manifest inspect`. Verify digest of `:latest` now equals digest of `:1.2.0` (advancing from v1.1.0). Capture before/after digests.
5. **§ 5. `cargo deny check` ERROR-gated on the v1.2.0 tag's CI run** — FOUND-16 fully closed. Verify CI for the `v1.2.0` tag commit shows cargo-deny as required-and-green (no `continue-on-error`).
6. **§ 6. git-cliff cumulative release body** — `git cliff v1.1.0..v1.2.0` generates the cumulative v1.2 release body (mirrors P14 D-19 cumulative `git cliff v1.0.1..v1.1.0`). NO hand-edit post-publish per D-15 of P23.
7. **§ 7. Update `.planning/STATE.md`** — flip milestone status to SHIPPED with final tag + date.
8. **§ 8. Run `/gsd-complete-milestone v1.2`** — POST-FINAL-TAG command (per CONTEXT D-12 — explicitly NOT a P24 plan). Archives `.planning/milestones/v1.2-ROADMAP.md` + `v1.2-REQUIREMENTS.md`, rewrites main `.planning/ROADMAP.md` with milestone groupings, commits archive, runs PROJECT.md evolution review. Same pattern as v1.0 (Phase 9 close → `/gsd-complete-milestone v1.0`) and v1.1 (Phase 14 close → `/gsd-complete-milestone v1.1`).

**Sign-off table** (mirror `21-RC2-PREFLIGHT.md:187-196` with `:latest` invariant FLIPPED — `:latest` MUST equal `:1.2.0` digest, NOT `:1.1.0`):

```markdown
| Field | Value |
|-------|-------|
| Maintainer signature | `__________________` |
| Date (UTC) | `__________________` |
| Final tag commit SHA (== rc.N SHA) | `__________________` |
| GHCR amd64 digest | `__________________` |
| GHCR arm64 digest | `__________________` |
| GHCR `:latest` digest (NOW must equal `v1.2.0` digest) | `__________________` |
| GHCR `:1.2.0` digest (for comparison) | `__________________` |
| GHCR `:1.2` digest (must equal `v1.2.0` digest) | `__________________` |
| GHCR `:1` digest (must equal `v1.2.0` digest) | `__________________` |
| Previous `:latest` digest (was `:1.1.0`) | `__________________` |
```

---

## Shared Patterns

### Autonomous=false maintainer-EXECUTES discipline (Plans 24-06, 24-07, 24-08)

**Source:** `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-RC2-PREFLIGHT.md:18-20`

```markdown
> This artifact is a maintainer-validated checklist. Claude authored it; the maintainer runs through it and cuts the tag locally per D-26 + project memory `feedback_uat_user_validates.md`.
>
> **Per D-22..D-26:** reuse `docs/release-rc.md` verbatim. NO modifications to `release.yml` / `cliff.toml` / `docs/release-rc.md` (D-24). The `git-cliff` output is authoritative for the GitHub Release body (D-25); no hand-edits post-publish.
```

**Apply to:** Plans 24-06, 24-07, 24-08 — every plan with `autonomous: false` must carry this preamble adapted for P24's D-IDs.

**Verification:** plan frontmatter MUST include `autonomous: false`. The plan PLAN.md MUST include the preamble. The Claude executor MUST NOT mark UAT/preflight/final-ship passed from its own runs per memory `feedback_uat_user_validates.md`.

### Tag-version invariant (Plans 24-06, 24-08)

**Source:** `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-RC3-PREFLIGHT.md:134`

```markdown
> **Tag-version invariant (D-22 + project memory `feedback_tag_release_version_match.md`):** the tag prefix `v1.2.0` MUST match `Cargo.toml`'s `version = "1.2.0"`. The `-rc.3` is tag-only suffix. Section 9 verifies `cronduit --version` returns `cronduit 1.2.0` from the published rc.3 image.
```

**Apply to:** Plan 24-06 (rc.4 cut — `Cargo.toml = "1.2.0"` unchanged; tag-only `-rc.4` suffix per CONTEXT D-18 informational) and Plan 24-08 (final tag — both `Cargo.toml = "1.2.0"` and tag `v1.2.0` match exactly per memory `feedback_tag_release_version_match.md`).

### Atomic-commit-per-plan (all plans 24-01..24-05)

**Source:** CONTEXT § Established Patterns L257 — "Atomic-commit-per-plan — each P24 plan = one commit inside the close-out PR. Project convention."

**Apply to:** Plans 24-01..24-05 inside the SINGLE close-out PR per CONTEXT D-02. Each plan = its own commit. Plan order in the PR per CONTEXT § Specifics: 24-02 (audit) → 24-01 (TM) → 24-03 (MILESTONES) → 24-04 (README) → 24-05 (cargo-deny) → PR merge → 24-06 (rc.4 cut) → 24-07 (rc.4 UAT) → 24-08 (final retag).

### Mermaid-only diagrams (all plans)

**Source:** Project memory `feedback_diagrams_mermaid.md` — every diagram in any project artifact MUST be a mermaid code block. No ASCII art diagrams.

**Apply to:** Any diagram in any P24 artifact — PLAN.md, MILESTONE-AUDIT.md, MILESTONES.md entry, README updates, THREAT_MODEL.md additions, PR description, code comments. Existing analog: `THREAT_MODEL.md:10-28` (Assets and Trust Boundaries — mermaid flowchart) and `README.md:212-226` (§Labels merge precedence — mermaid flowchart).

### `:latest` hyphen-gate invariant (Plans 24-06, 24-08)

**Source:** `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-RC2-PREFLIGHT.md:142` and `.planning/milestones/v1.1-phases/14-.../14-CONTEXT.md:106` (P14 D-18).

**Apply to:**
- Plan 24-06: rc.4 is hyphenated (`v1.2.0-rc.4`); `:latest` MUST stay at `:1.1.0` digest. Verify via `docker manifest inspect ghcr.io/simplicityguy/cronduit:latest` digest == `:1.1.0` digest.
- Plan 24-08: final `v1.2.0` is NON-hyphenated; hyphen-gate fires; `:latest` MUST now equal `:1.2.0` digest. Verify same way (the comparison flips).

---

## No Analog Found

Every Phase 24 file has a strong analog in the codebase. The only items without direct precedent are:

| Item | Reason | Mitigation |
|------|--------|------------|
| README v1.2 'What's New' hero block | Project has never shipped a milestone-hero block at top of README | CONTEXT § Claude's Discretion allows planner-judged shape (single paragraph / `<details>` / mermaid timeline) — bounded scope |
| `.planning/REQUIREMENTS.md` Validated marker text | Existing v1.2 ticked rows (FOUND-14/15/16) carry NO appended "(Phase N — see VERIFICATION.md)" text — only the audit-predicate suffix `T-V12-FCTX-01` style ID | Mirror the existing ticked-row shape; do NOT add appended phase-ref text. The MILESTONE-AUDIT.md `requirements_satisfied` block carries the per-phase evidence |
| v1.1 MILESTONE-AUDIT.md retroactive creation | v1.1 archive lacks an audit doc; v1.0 is the SOLE structural precedent | CONTEXT § Deferred explicitly defers retroactive v1.1 audit creation; planner notes the inconsistency in plan 24-02 § Tech Debt Summary; does NOT recreate v1.1's audit retroactively |

---

## Metadata

**Analog search scope:**
- `THREAT_MODEL.md` (canonical TM1–TM4 + L189-229 stub + STRIDE summary tables + Changelog)
- `MILESTONES.md` (v1.1 + v1.0 entries)
- `.planning/milestones/v1.0-MILESTONE-AUDIT.md` (full structure)
- `.planning/milestones/v1.1-{ROADMAP,REQUIREMENTS}.md` (archive shape destination for `/gsd-complete-milestone v1.2`)
- `.planning/research/PITFALLS.md` § Pitfall 56 (audit-predicate source)
- `.planning/REQUIREMENTS.md` (tick-flip shape from FOUND-14/15/16 already-ticked rows)
- `.planning/phases/21-.../21-RC2-PREFLIGHT.md` (verbatim rc preflight mirror)
- `.planning/phases/23-.../23-RC3-PREFLIGHT.md` (most-recent sibling rc preflight)
- `.planning/phases/23-.../23-HUMAN-UAT.md` (six-scenario UAT mirror)
- `.planning/milestones/v1.1-phases/14-.../14-CONTEXT.md` D-16/D-18 (final-tag retag-the-rc-SHA discipline)
- `README.md:19-33` (§Security), `:206-285` (§Labels), `:287-315` (§Tag Filter Chips)
- `.github/workflows/ci.yml:47-58` (cargo-deny block with embedded P24-promotion forecast comment)
- `docs/release-rc.md` (rc cut runbook — reused verbatim by plan 24-06; NOT edited)
- `docs/WEBHOOKS.md` (649 lines — verified large; supports planner decision to keep README §Webhooks LEAN per CONTEXT § Claude's Discretion)

**Files scanned:** 16 in-tree analogs + 4 project-memory files (`feedback_diagrams_mermaid.md`, `feedback_no_direct_main_commits.md`, `feedback_uat_user_validates.md`, `feedback_tag_release_version_match.md`, `feedback_uat_use_just_commands.md`).

**Pattern extraction date:** 2026-05-16
