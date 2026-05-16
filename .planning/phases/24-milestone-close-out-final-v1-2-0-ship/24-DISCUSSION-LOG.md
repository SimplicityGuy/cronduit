# Phase 24: Milestone Close-Out — final `v1.2.0` ship - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-16
**Phase:** 24-milestone-close-out-final-v1-2-0-ship
**Areas discussed:** Final-ship SHA + rc.N policy, Threat-model authoring (TM5/TM6 + STRIDE), REQUIREMENTS flip + ROADMAP drift cleanup, Plan structure + cargo-deny + audit/complete flow + README scope

---

## Final-ship SHA + rc.N policy (Gray Area 1)

### Sub-question 1: Final-ship SHA strategy for v1.2.0?

| Option | Description | Selected |
|--------|-------------|----------|
| Retag rc.3 SHA (mirror v1.1 P14 D-16) | Final `git tag v1.2.0 <rc.3-SHA>` against the SHA UAT validated (`dbc4b28`). Docs land on main during P24 but tag points at rc.3 commit. Bit-identical image to rc.3. | |
| Retag close-out merge SHA | Close-out PR merges to main; final v1.2.0 tag points at the merge commit. Fresh build, fresh attestation. Image bytes identical if docs-only, but new digest in image manifest. | |
| Cut rc.4 with docs, UAT it, retag rc.4 | Treat docs landings as material; cut rc.4 after docs land, run light rc.4 UAT, then retag rc.4 SHA as v1.2.0. Heaviest process; most conservative. | ✓ |

**User's choice:** Cut rc.4 with docs, UAT it, retag rc.4.
**Notes:** Conservative path. The rc.4 SHA carries the v1.2 close-out docs + cargo-deny promotion in repo source; bit-identical image to what UAT validated. Did NOT pick "retag rc.3" because rc.3 lacks the v1.2 close-out docs (TM5/TM6/MILESTONES/REQUIREMENTS flips/README/cargo-deny) and the source repo at v1.2.0 would not match the published artifacts. Did NOT pick "retag close-out merge SHA without rc cut" because it skips the regression-validation gate that rc.4 provides.

### Sub-question 2: Policy — do P24 docs-only fixes warrant a new rc cut?

| Option | Description | Selected |
|--------|-------------|----------|
| No — docs-only PRs land on main without an rc cut | P24 docs land between rc.3 and final v1.2.0 with NO rc.4 cut. | |
| Yes — every docs batch triggers a fresh rc | Each P24 docs PR cuts an rc.N + gets a light UAT. | ✓ |
| Only if code changes | Docs land freely; rc cut only on cargo-deny advisory fix or other non-docs change. | |

**User's choice:** Yes — every docs batch triggers a fresh rc.
**Notes:** Conservative. Combined with sub-question 1 = "Cut rc.4 with docs" this implies frequent rc cuts during P24 close-out. The next clarifier sub-question pinned what "batch" means.

### Sub-question 3: What is the 'batch' boundary for an rc cut during P24?

| Option | Description | Selected |
|--------|-------------|----------|
| One rc per close-out PR (multiple plans per PR) | Plans grouped into close-out PRs; each merged close-out PR triggers an rc cut + light UAT. Likely 2–3 rcs total. | |
| One rc per plan (atomic-commit-per-plan → atomic-rc-per-plan) | Every plan = one PR = one rc cut + one light UAT. Highest cadence (rc.4..rc.8+). | |
| Single big close-out PR → single rc.4 → retag as v1.2.0 | All P24 docs land in one mega-PR; cut rc.4; full UAT on rc.4; retag rc.4 SHA as v1.2.0. | ✓ |

**User's choice:** Single big close-out PR → single rc.4 → retag as v1.2.0.
**Notes:** Effectively "one batch = one rc." Cleanest mid-ground given the prior answers. If rc.4 UAT fails on a finding, fix in a NEW close-out PR + rc.5 cut + UAT; final v1.2.0 always retags the LAST passing-UAT rc SHA. Locked as CONTEXT.md D-01 + D-02.

---

## Threat-model authoring (TM5/TM6 + STRIDE) (Gray Area 2)

### Sub-question 1: How should TM5 (currently a v1.2 stub) be transformed into the canonical entry?

| Option | Description | Selected |
|--------|-------------|----------|
| Replace-in-place: rewrite TM5 stub into canonical entry | Edit `THREAT_MODEL.md:189-229` in place. Remove stub preamble, forward-pointer subsection, holding-signal footer. Body becomes canonical TM5 matching TM1–TM4 structure. Single coherent doc state. | ✓ |
| Expand the stub: keep header, append canonical subsections | Lighter touch: rewrite 'Status:' line; strip forward-pointer; leave existing v1.2 mitigations; append new subsections (Recommendations, STRIDE rows, TM6 cross-link). Additive diff. | |
| Version the doc: add revision block, keep stub archival | Add `## Document Revision` block at top declaring 'v1.2.0 close-out'. Existing stub stays archival; new canonical TM5 lives at separate anchor. Heavier doc; risks two-TM5 confusion. | |

**User's choice:** Replace-in-place.
**Notes:** Single coherent doc state; no "stub vs canonical" reader confusion. Locked as CONTEXT.md D-03.

### Sub-question 2: How literal should TM5/TM6 follow the Pitfall 56 spec text?

| Option | Description | Selected |
|--------|-------------|----------|
| Literal: use Pitfall 56's bullet text near-verbatim | Pitfall 56 lines 1115–1125 list explicit Threat/Attack Vector/Mitigations/Residual/Recommendation bullets. Treat as source spec; expand to TM1–TM4 prose but preserve substantive points + bullet order. | |
| Author's prose: spec is a checklist, prose is fresh | Read Pitfall 56 as coverage checklist; write prose afresh in P24 to match shipped reality. Risks drifting from spec but produces more natural text. | |
| Hybrid: literal for STRIDE rows + structure, prose for Threat/Mitigation narrative | Use Pitfall 56 literal text for STRIDE table rows (T-S3, T-T4, T-I4, T-D4) and threat-model section headings/structure. Author Threat / Attack Vector / Mitigations / Residual / Recommendations narrative bodies fresh to match v1.2.0 shipped reality. | ✓ |

**User's choice:** Hybrid.
**Notes:** Best of both — Pitfall 56 STRIDE rows are the audit-predicate source text (literal preserves T-V12-XCUT-06); narrative bodies ground in v1.2 shipped reality (Phase 15 worker isolation, P19 HMAC, P20 SSRF/HTTPS for TM5; P17 reserved-namespace + type-gate + size-limit for TM6). Locked as CONTEXT.md D-04.

### Sub-question 3: Bundle STRIDE row additions + README link-back with TM5/TM6 edits, or split into separate plans?

| Option | Description | Selected |
|--------|-------------|----------|
| Bundle: TM5 + TM6 + STRIDE rows + README link-back all in ONE plan | Single plan touches THREAT_MODEL.md AND README.md §Security link-back. Pitfall 56 audit predicates T-V12-XCUT-05/06/07 close in one commit. | ✓ |
| Two plans: TM doc edits + README link separately | Plan A: THREAT_MODEL.md. Plan B: README.md §Security link-back. Per-file separation. | |
| Three plans: TM5+TM6 / STRIDE rows / README link | Highest granularity: Plan A (prose), B (STRIDE), C (README). Easier per-plan review but artificial seams. | |

**User's choice:** Bundle.
**Notes:** Atomic-commit-per-plan still satisfied; one coherent threat-model close-out diff. Locked as CONTEXT.md D-06.

### Sub-question 4: How should TM6 (operator-supplied Docker labels) anchor in the doc?

| Option | Description | Selected |
|--------|-------------|----------|
| TM6 as a peer section after TM5 | Standalone `## Threat Model 6: Operator-supplied Docker Labels` section after TM5; matches TM1–TM5 pattern. Independent Threat/Attack Vector/Mitigations/Residual/Recommendations. TM5 cross-links to it. STRIDE row T-T4 in Tampering table. | ✓ |
| TM6 as a subsection of TM3 (Config Tamper) | Treat label-namespace clobber as a Config Tamper sub-threat; add as `### Threat Model 3.1`. Reduces section count but obscures the label-validator mitigation story. | |

**User's choice:** Peer section after TM5.
**Notes:** Matches TM1–TM5 structural pattern; gives the discrete label-validator mitigation story peer-section visibility. Locked as CONTEXT.md D-05.

---

## REQUIREMENTS flip + ROADMAP drift cleanup (Gray Area 3)

### Sub-question 1: REQUIREMENTS.md 'Validated' flip mechanics for the 20 remaining unticked items?

| Option | Description | Selected |
|--------|-------------|----------|
| By-phase grouping mirror of v1.0 MILESTONE-AUDIT pattern | Build `.planning/milestones/v1.2-MILESTONE-AUDIT.md` mirroring v1.0-MILESTONE-AUDIT.md structure. REQUIREMENTS.md tick changes become audit's mechanical output. Produced via `/gsd-audit-milestone v1.2`. | ✓ |
| Bulk single-commit flip at end of P24 | One commit ticks all 20 remaining with phase refs. Simplest. Risk: if rc.4 UAT surfaces finding invalidating a previously-ticked req, this commit needs amend/revert. No separate audit doc. | |
| Per-category flip (FOUND/WH/LBL/FCTX/EXIT/TAG) | Six separate commits, one per category. Auditable per-category. Heaviest. | |

**User's choice:** By-phase grouping mirror of v1.0 MILESTONE-AUDIT pattern.
**Notes:** Inline `/gsd-audit-milestone v1.2` invocation produces both `v1.2-MILESTONE-AUDIT.md` AND the mechanical REQUIREMENTS.md flips. Locked as CONTEXT.md D-07.

### Sub-question 2: Timing for the REQUIREMENTS flip + MILESTONE-AUDIT?

| Option | Description | Selected |
|--------|-------------|----------|
| In the close-out PR (lands on main before rc.4 cut) | REQUIREMENTS flips + audit doc land in the close-out PR alongside TM5/TM6/MILESTONES/README. rc.4 SHA carries the audit + flipped requirements in repo. If UAT fails, audit gets revised in follow-up PR for rc.5. | ✓ |
| Post-final-tag (after v1.2.0 ships) | Audit + flip happen AFTER v1.2.0 retags rc.4 SHA. v1.2.0 image ships without flipped requirements; bookkeeping catches up post-tag. | |

**User's choice:** In the close-out PR (before rc.4 cut).
**Notes:** Conservative; matches D-02's single-big-PR decision and v1.0/v1.1 archival precedent. Locked as CONTEXT.md D-08.

### Sub-question 3: ROADMAP plan-count drift cleanup (P17 gap-closure status, P21 10/11→11/11, P22 4/6→6/6)?

| Option | Description | Selected |
|--------|-------------|----------|
| Same close-out PR as REQUIREMENTS flip + MILESTONE-AUDIT | Bookkeeping pass also bumps ROADMAP plan counts. All tracker hygiene in one diff. | ✓ |
| Separate cleanup plan inside P24 | Own plan for ROADMAP table hygiene only. | |
| Defer to /gsd-complete-milestone v1.2 | ROADMAP edits land via the milestone-completion workflow that archives to `.planning/milestones/v1.2-ROADMAP.md`. | |

**User's choice:** Same close-out PR.
**Notes:** ROADMAP drift is the same shape of work as REQUIREMENTS ticks (mechanical output of the audit). Both happen in plan 24-02. Locked as CONTEXT.md D-09.

---

## Plan structure + cargo-deny + audit/complete flow + README scope (Gray Area 4)

### Sub-question 1: Plan structure for P24?

| Option | Description | Selected |
|--------|-------------|----------|
| 8-plan close-out (5 docs + 3 autonomous=false maintainer plans) | 1) THREAT_MODEL+README§Security 2) MILESTONE-AUDIT + REQUIREMENTS + ROADMAP 3) MILESTONES.md v1.2 entry 4) README updates 5) cargo-deny WARN→ERROR 6) 24-RC4-PREFLIGHT (autonomous=false) 7) 24-HUMAN-UAT (autonomous=false) 8) 24-FINAL-SHIP-PREFLIGHT (autonomous=false). Mirrors P14/P21/P23 shape. | ✓ |
| Fewer plans (collapse 1–5 into 2–3 mega-plans) | Merge docs plans into thematic mega-plans. Reduces plan count to 5–6. Atomic-commit boundary blurs. | |
| 10+ plans (split aggressively per task) | Separate plans per THREAT_MODEL section, per ROADMAP edit, per README subsection. Maximum auditability but heavy artifact. | |

**User's choice:** 8-plan close-out.
**Notes:** Locked as CONTEXT.md D-10. Plans 1–5 land in close-out PR; 6–8 are maintainer-EXECUTES (autonomous=false) per project memory `feedback_uat_user_validates.md`.

### Sub-question 2: cargo-deny WARN→ERROR promotion timing (FOUND-16) in the close-out PR?

| Option | Description | Selected |
|--------|-------------|----------|
| Own plan early in P24 (plan 5 in 8-plan shape) | Land cargo-deny promotion as own plan early. Surfaces advisory/license issues with time to fix in same close-out PR (deny.toml exception or Cargo.lock rev). Code-change risk in focused diff. | ✓ |
| Folded into the 24-RC4-PREFLIGHT step | Promote as last commit before maintainer cuts rc.4. If advisories block, rc.4 cut stalls. Last-minute-bind risk. | |
| Pre-stage: /gsd-quick before P24 plan-phase | Bypass P24's close-out PR for cargo-deny. Decouples CI change from docs close-out PR. | |

**User's choice:** Own plan early (plan 24-05).
**Notes:** Decouples potential code-change risk (deny.toml or Cargo.lock fix) from the rc.4 preflight runbook. Locked as CONTEXT.md D-11.

### Sub-question 3: /gsd-audit-milestone + /gsd-complete-milestone integration with P24?

| Option | Description | Selected |
|--------|-------------|----------|
| Both inline in P24: audit-milestone IS plan 2, complete-milestone is post-final-tag | /gsd-audit-milestone v1.2 produces audit + REQUIREMENTS flips inline in plan 24-02. /gsd-complete-milestone runs AFTER final v1.2.0 tag publishes — archives v1.2-ROADMAP.md / v1.2-REQUIREMENTS.md, rewrites main ROADMAP.md. Mirrors v1.0/v1.1 archival precedent. | ✓ |
| Pre-stage audit: run /gsd-audit-milestone v1.2 BEFORE P24 plan-phase | Audit produces doc before P24 begins; plan 2 lands the agent's output + matching flips. | |
| Skip /gsd-audit-milestone; hand-author v1.2-MILESTONE-AUDIT.md as P24 plan 2 | Plan 2 directly authors audit following v1.0 template without invoking skill. /gsd-complete-milestone still runs post-tag. | |

**User's choice:** Both inline in P24 (audit = plan 24-02; complete-milestone = post-final-tag follow-up).
**Notes:** Locked as CONTEXT.md D-12. Plan 24-08 (FINAL-SHIP-PREFLIGHT) instructs maintainer to run `/gsd-complete-milestone v1.2` as the FINAL step after verifying `:latest` advanced.

### Sub-question 4: README updates scope (multiSelect — beyond §Security link-back already in plan 24-01)?

| Option | Description | Selected |
|--------|-------------|----------|
| Webhook overview / forward-reference to docs/WEBHOOKS.md | Add README §Configuration webhook subsection or forward-pointer. | ✓ |
| Failure-context + exit-code histogram pointers | Brief README mention of new v1.2 run-detail FCTX panel + job-detail exit-code histogram. | ✓ |
| v1.2 'What's New' hero block at top of README | Small block above §Security listing five v1.2 features with anchor links. | ✓ |
| MILESTONES.md cross-link from README | Link to MILESTONES.md and GitHub Releases from §Security or a new §Releases footer. | ✓ |

**User's choice:** All four.
**Notes:** Each addition is a small targeted edit; plan 24-04 produces ONE README diff containing all four. Mirrors cumulative §Configuration subsection additions across v1.2 (P17 Labels, P23 Tag Filter Chips). Locked as CONTEXT.md D-13.

---

## Claude's Discretion

The user explicitly left these to planner judgment (see CONTEXT.md § Claude's Discretion):

- Plan 24-02 audit-doc detail level (full `/gsd-audit-milestone v1.2` invocation depth vs. tighter scope matching v1.0 shape).
- MILESTONES.md v1.2 entry length (whether five-feature itemization expands or stays one-paragraph).
- README hero block format (paragraph vs `<details>` collapsible vs mermaid timeline).
- Plan 24-04 §Configuration webhook subsection depth (forward-pointer vs brief TOML example).
- Plan 24-04 §Features anchor structure (own subsections vs bullets vs inline anchors).
- cargo-deny advisory remediation choice in plan 24-05 (deny.toml exception vs Cargo.lock rev) — depends on what surfaces in Branch B.
- Plan 24-06/07/08 sub-section format details.
- Whether v1.2-MILESTONE-AUDIT.md is committed FIRST in close-out PR or after plan 24-01.
- Whether `/gsd-complete-milestone v1.2` appears as a numbered step in plan 24-08 vs trailing prose.

## Deferred Ideas

Captured in CONTEXT.md `<deferred>` section. Highlights:

- Webhook destination allow/block-list filter — v1.3 candidate.
- Web UI authentication — v2 deferral.
- Cross-run log search / job concurrency / queuing — v1.3 candidates.
- Tag-based bulk operations / tag autocomplete / tag count badge / tag chips on job-detail — v1.3 candidates.
- Browser-based playwright HTMX smoke tests — v1.3 candidate.
- `v1.1-MILESTONE-AUDIT.md` retroactive creation — v1.1 lacks the audit doc that v1.0 has; planner may note in plan 24-02's audit § Tech Debt Summary. OUT of scope for P24 retroactive fix.
- `/gsd-complete-milestone v1.2` workflow — separate post-final-tag command per D-12.
- PROJECT.md evolution review + v1.3 milestone kickoff — both happen INSIDE `/gsd-complete-milestone v1.2`.
- `docs/release-rc.md` / `release.yml` / `cliff.toml` modifications — reused verbatim per D-10.
