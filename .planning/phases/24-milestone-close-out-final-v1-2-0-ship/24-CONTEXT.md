# Phase 24: Milestone Close-Out — final `v1.2.0` ship - Context

**Gathered:** 2026-05-16
**Status:** Ready for planning

<domain>
## Phase Boundary

Operational close-out for the v1.2 — Operator Integration & Insight milestone. Phase 24 has **no v1.2 REQ-IDs** (all 41 requirements covered by Phases 15–23; mirrors v1.0 Phase 9 pattern). The work is final-ship paperwork plus the rc.4 → final `v1.2.0` UAT-driven release loop, anchored in five operator-observable success criteria from `.planning/ROADMAP.md` § Phase 24:

1. **`THREAT_MODEL.md` close-out** — canonical Threat Model 5 (Webhook Outbound) replaces the v1.2 stub at L189-229; new Threat Model 6 (Operator-supplied Docker Labels) added as a peer section; STRIDE summary table gains rows T-S3 / T-T4 / T-I4 / T-D4; Changelog row for v1.2 close-out. `README.md` §Security gains link-back anchors. Pitfall 56 audit predicates T-V12-XCUT-05 / -06 / -07 all close in one diff.
2. **`.planning/REQUIREMENTS.md` Validated flip** — all 20 remaining unticked v1.2 requirements (FOUND/WH/LBL/FCTX/EXIT/TAG) flip to `[x] Validated` with the satisfying-phase reference, derived as the mechanical output of a new `.planning/milestones/v1.2-MILESTONE-AUDIT.md` (mirrors `.planning/milestones/v1.0-MILESTONE-AUDIT.md` structure). `.planning/ROADMAP.md` plan-count drift cleanup lands in the same diff (P17 'Complete', P21 11/11, P22 6/6, P24 entry ticked once rc.4 ships).
3. **`MILESTONES.md` v1.2 entry** — new top-of-file release-log entry mirroring the v1.1 and v1.0 shapes (header / one-paragraph summary / Tags row / Phases row / Requirements delivered row / Audit row pointing at `.planning/milestones/v1.2-*`). Authored in P24 plan 3; `/gsd-complete-milestone v1.2` (post-final-tag) does the eventual archive move.
4. **`:latest` promotion + cargo-deny gate promotion** — final `v1.2.0` tag advances `:latest` from `v1.1.0` to `v1.2.0` on both amd64 + arm64 via the existing `release.yml` hyphen-gate (P12 D-10); the `cargo-deny` CI job is promoted from `continue-on-error: true` (Phase 15 D-02 / FOUND-16) to blocking (`continue-on-error: false`) BEFORE the rc.4 cut, with `deny.toml` exceptions or `Cargo.lock` revs landed in the same plan if any advisory has accumulated since rc.1.
5. **Regression-smoke + new-features UAT** — full HUMAN-UAT on rc.4 covering: `docker compose up` quickstart healthy; v1.0/v1.1 dashboard surfaces (filter / sort / Run Now / Stop / bulk toggle / timeline / sparklines / settings overrides) intact; v1.2 features end-to-end (webhooks Standard-Webhooks-v1 payload + HMAC + SSRF posture + retry/drain; custom Docker labels merge + reserved-namespace validator; FCTX panel on run-detail; exit-code histogram on job-detail; tag filter chips on dashboard; tags in webhook payload).

**The work lands as ONE close-out PR** containing plans 1–5 (TM5/TM6/README§Security link, v1.2-MILESTONE-AUDIT.md + REQUIREMENTS flips + ROADMAP drift, MILESTONES.md v1.2 entry, README §Features/§Configuration updates, cargo-deny WARN→ERROR promotion). After merge, the maintainer cuts `v1.2.0-rc.4` locally per `docs/release-rc.md`, runs the full v1.2 HUMAN-UAT, then retags the rc.4 SHA as `v1.2.0` (bit-identical to rc.4). If rc.4 UAT surfaces findings, fixes land in a follow-up close-out PR + rc.5 cut + UAT; final `v1.2.0` always retags the LAST passing-UAT rc SHA. Mirrors v1.1 P14 D-16 "what was tested is what ships" discipline.

**Routing note (load-bearing):** The MILESTONE-AUDIT plan (plan 2) is the agent for `/gsd-audit-milestone v1.2` — it produces both `.planning/milestones/v1.2-MILESTONE-AUDIT.md` AND the REQUIREMENTS.md flips + ROADMAP drift cleanup. The post-final-tag `/gsd-complete-milestone v1.2` is a SEPARATE follow-up command (NOT a P24 plan) that archives `.planning/milestones/v1.2-ROADMAP.md` / `v1.2-REQUIREMENTS.md` and rewrites the main `.planning/ROADMAP.md` with milestone groupings — same pattern as v1.0 and v1.1.

**Out of scope for Phase 24** (deferred — do not creep):
- Webhook destination allow/block-list filter — explicit v1.3 candidate per `.planning/PROJECT.md` § Future Requirements; called out in the canonical TM5 Recommendations block as deferred.
- Web UI authentication — deferred to v2 per `.planning/REQUIREMENTS.md` § Out of Scope.
- Cross-run log search across retention window — explicit v1.3 candidate per `.planning/PROJECT.md` § Current Milestone (punted at v1.2 kickoff).
- Job concurrency limits / queuing — explicit v1.3 candidate per `.planning/PROJECT.md` § Current Milestone.
- Tag-based bulk operations on the dashboard bulk-action bar — v1.3 candidate per `.planning/REQUIREMENTS.md` § Out of Scope (reaffirmed P23 deferred).
- Tag autocomplete / search-as-you-type in chip strip — v1.3 candidate (P22 / P23 deferred).
- New code features — P24 is paperwork + final-ship; the only code/CI change is the `cargo-deny` WARN→ERROR promotion (FOUND-16) which is a workflow change, not source.
- `/gsd-complete-milestone v1.2` (milestone archival + ROADMAP rewrite) — runs AFTER final `v1.2.0` tag publishes; NOT a P24 plan.
- `release.yml` / `cliff.toml` / `docs/release-rc.md` modifications — reused verbatim per P12 D-10..D-12 / P20 D-30 / P21 D-22..D-26 / P23 D-15..D-16. Any maintainer-discovered runbook gap during the rc.4 cut becomes a hotfix PR before tagging.

</domain>

<decisions>
## Implementation Decisions

### Final-ship SHA strategy + rc.N policy (Gray Area 1)

- **D-01:** **Final-ship SHA strategy: cut rc.4 from the close-out PR merge SHA → run full HUMAN-UAT on rc.4 → retag the rc.4 SHA as final `v1.2.0` (bit-identical image).** Mirrors v1.1 P14 D-16 "what was tested is what ships" discipline. Final tag does NOT point at the rc.3 SHA (rc.3 lacks the v1.2 close-out docs in repo: TM5/TM6, MILESTONES.md v1.2 entry, REQUIREMENTS flips, README updates, cargo-deny promotion). If rc.4 UAT surfaces findings, fixes land in a follow-up close-out PR; cut rc.5 from that merge SHA; retag the LAST passing-UAT rc SHA as `v1.2.0`. The v1.2 GHCR container image is unaffected by the .md doc landings (Dockerfile COPYs only binary + assets) BUT the source repo state at the tagged SHA matters for attestation, future archaeology, and the `cargo-deny` promotion (which IS a CI workflow change at the tagged SHA).

  **Rejected:** retag rc.3 SHA as v1.2.0 (rc.3 lacks the v1.2 close-out docs + cargo-deny promotion; the source repo at v1.2.0 would not match the published artifacts). **Rejected:** retag the close-out PR merge SHA without an rc.4 cut + UAT (skips the regression-validation gate that rc.4 provides; risks shipping unvalidated bytes even if the binary is plausibly identical). **Rejected:** per-plan rc cuts (rc.4 docs / rc.5 cargo-deny / rc.6 …) — heaviest cadence with no benefit since plans 1–5 land in one PR by D-02.

- **D-02:** **Docs batching: ONE close-out PR contains all v1.2 paperwork (plans 1–5) → ONE rc cut from that PR's merge SHA → ONE full HUMAN-UAT.** Plans 1–5 are TM5/TM6+README§Security, v1.2-MILESTONE-AUDIT + REQUIREMENTS flips + ROADMAP drift, MILESTONES.md v1.2 entry, README §Features/§Configuration updates, and cargo-deny WARN→ERROR promotion. Atomic-commit-per-plan still holds (each plan is its own commit inside the PR per project convention). If rc.4 UAT fails on a finding, the fix is a NEW close-out PR + rc.5 cut + UAT.

  **Rejected:** one rc per close-out PR with multiple PRs accumulating across plans (lacks atomicity at the rc-tag layer; "what was tested" diverges across rc tags). **Rejected:** one rc per plan (highest cadence: rc.4..rc.8+; no defensible benefit over single-PR-single-rc since the plans are docs-only and inter-dependent for the audit narrative). **Rejected:** zero rc cuts (just retag rc.3) — fails the bit-identical-to-UAT invariant per D-01 because the source repo at v1.2.0 would diverge.

### Threat-model authoring (Gray Area 2)

- **D-03:** **TM5 transformation: REPLACE-IN-PLACE rewrite of `THREAT_MODEL.md:189-229`.** Remove the `> Status: Words-only stub for v1.2.0 (Phase 20). The canonical close-out … lands in Phase 24 (Milestone Close-Out)` preamble, the `### Phase 24 Close-Out` forward-pointer subsection (L220-229), and the "Until that lands, this stub is the holding signal" footer. The section body becomes the canonical TM5 entry matching the TM1–TM4 structure exactly (`### Threat / ### Attack Vector / ### Mitigations / ### Residual Risk / ### Recommendations` — no `(v1.2.0)` suffix on the Mitigations header). Single coherent doc state; no archival noise; no "stub vs canonical" reader confusion.

  **Rejected:** expand-the-stub additively (leaves the "stub" framing in the canonical doc; readers wonder why TM5 has scaffolding TM1–TM4 don't). **Rejected:** version-the-doc with stub kept archival (creates two TM5 sections; SEO-hostile internal anchors; explicit confusion vector).

- **D-04:** **Spec literalism: HYBRID — literal Pitfall 56 text for STRIDE rows + section structure; fresh prose for narrative bodies.** Use Pitfall 56 (`.planning/research/PITFALLS.md:1099-1145`) **literal** text for the four STRIDE table row additions (T-S3 Spoofing: "Attacker forges webhook payload" → "Mitigated by HMAC signing; out-of-scope: receiver-side verification (operator's responsibility)"; T-T4 Tampering: "Attacker injects label collision into `cronduit.*` namespace" → "Mitigated by validator"; T-I4 Information Disclosure: "Webhook URL embeds credentials in `userinfo`" → "Mitigated by `strip_url_credentials` (Pitfall 38)"; T-D4 DoS: "Webhook receiver outage stalls scheduler loop" → "Mitigated by bounded mpsc + delivery worker isolation (Pitfall 28)"). Use Pitfall 56's structural list (Threat / Attack Vector / Mitigations / Residual Risk / Recommendations) as the section skeleton for TM5 and TM6. Author the narrative bodies **fresh** to ground in v1.2.0 shipped reality:
  - **TM5 Mitigations** narrative cites Phase 15 (`src/webhooks/{mod,worker}.rs` bounded `mpsc(1024)` + dedicated worker — scheduler isolation), Phase 18 (Standard Webhooks v1 payload + edge-triggered coalescing), Phase 19 (HMAC-SHA256 + base64 signature header + constant-time receiver examples), Phase 20 (HTTPS-required validator + full-jitter retry + 30s drain + `webhook_deliveries` dead-letter + `cronduit_webhook_*` metrics).
  - **TM6 Mitigations** narrative cites Phase 17 (`cronduit.*` reserved-namespace validator + type-gated `docker`-only validator + size-limit DoS surface).
  - **TM5 Residual Risk** explicitly retains Pitfall 56's "any URL the cronduit container can reach is reachable" + "operator must use network controls" framing.
  - **TM5 Recommendations** explicitly defers the destination allow/block-list filter to v1.3 (the existing stub already says this; preserve it in the canonical form).

  **Rejected:** verbatim-Pitfall-56 prose (reads like an internal checklist rather than an external-audience threat model; misses v1.2-shipped-code grounding). **Rejected:** all-fresh prose ignoring Pitfall 56 STRIDE row text (risks coverage drift; Pitfall 56 is the audit predicate source).

- **D-05:** **TM6 anchor: standalone `## Threat Model 6: Operator-supplied Docker Labels` peer section AFTER TM5.** Matches the TM1–TM5 structural pattern. Independent `### Threat / ### Attack Vector / ### Mitigations / ### Residual Risk / ### Recommendations` body. TM5's Recommendations block cross-links to TM6 ("See also: [Threat Model 6: Operator-supplied Docker Labels](#threat-model-6-operator-supplied-docker-labels)"). STRIDE row T-T4 in the Tampering summary table references TM6.

  **Rejected:** TM6 as a subsection of TM3 (Config Tamper) — label-namespace clobber has a discrete validator mitigation story (Phase 17 `check_labels_reserved` + `check_labels_only_on_docker_jobs`) that warrants peer-section visibility; embedding in TM3 obscures the standalone mitigation. **Rejected:** TM6 as an inline-mention within TM2 (Untrusted Client) — same loss of mitigation-story visibility; muddier doc map.

- **D-06:** **Bundling: SINGLE plan touches `THREAT_MODEL.md` (TM5 rewrite, TM6 new section, STRIDE rows T-S3/T-T4/T-I4/T-D4 in the summary table, Changelog row for v1.2 close-out with revision bump) AND `README.md` §Security link-back to `#threat-model-5-webhook-outbound` + `#threat-model-6-operator-supplied-docker-labels` anchors.** Atomic-commit-per-plan still satisfied; one coherent threat-model close-out diff. Pitfall 56 audit predicates T-V12-XCUT-05 (TM5 + TM6 sections exist) + T-V12-XCUT-06 (STRIDE rows T-S3/T-T4/T-I4/T-D4 exist) + T-V12-XCUT-07 (README links to TM5/TM6) all close in one PR commit.

  **Rejected:** two-plan split (TM doc edits + README link) — artificial seam; the README link IS the threat-model close-out from an audit-predicate perspective. **Rejected:** three-plan split (TM5+TM6 / STRIDE rows / README link) — over-fragmented; STRIDE rows are 4 lines in an existing table, not a discrete deliverable.

### REQUIREMENTS.md flip mechanics + ROADMAP drift cleanup (Gray Area 3)

- **D-07:** **REQUIREMENTS flip mechanism: BY-PHASE grouping mirror of v1.0 MILESTONE-AUDIT pattern.** Produce `.planning/milestones/v1.2-MILESTONE-AUDIT.md` mirroring `.planning/milestones/v1.0-MILESTONE-AUDIT.md` structure exactly: § Score Summary / § 1. Requirements Coverage — 3-Source Cross-Reference (Satisfied / Partial / Unsatisfied / Orphans subsections) / § 2. Phase Verifications — Status Matrix / § 3. Cross-Phase Integration — Wiring Paths / § 4. End-to-End Flows / § 5. Nyquist Compliance / § 6. Tech Debt Summary / § Verdict Routing. The 20 remaining REQUIREMENTS.md tick flips become the audit's **mechanical output** — each tick gets `(Phase N — see VERIFICATION.md)` reference text mirroring the existing FOUND-14..16 ticks. Produced via `/gsd-audit-milestone v1.2` invoked as part of plan 2; the spawned `gsd-integration-checker` (per the workflow's `available_agent_types`) writes both files.

  **Rejected:** bulk single-commit flip without an audit doc (loses the 3-source cross-reference + Nyquist coverage + tech-debt rollup that future maintainers expect; v1.0/v1.1 set the audit-doc precedent). **Rejected:** per-category flip (FOUND/WH/LBL/FCTX/EXIT/TAG) in six separate commits (artificial fragmentation; the audit doc already groups by category internally — six commits add noise without auditability gain).

- **D-08:** **Flip timing: IN the close-out PR (lands on main BEFORE rc.4 cut).** REQUIREMENTS.md flips + `v1.2-MILESTONE-AUDIT.md` land alongside TM5/TM6/MILESTONES.md/README updates in plans 1–5 of the close-out PR. rc.4 SHA therefore carries the audit + flipped requirements in repo. If rc.4 UAT surfaces a finding that invalidates a previously-ticked requirement, the audit gets revised in the follow-up close-out PR for rc.5 (the bookkeeping stays consistent with the LAST passing-UAT rc per D-01). Conservative; matches D-02's "single big close-out PR" decision; matches v1.0/v1.1 archival precedent of "audit doc shipped with the milestone, not after."

  **Rejected:** post-final-tag flip (the v1.2.0 image ships with un-flipped repo state; main drifts from the shipped artifact during the bookkeeping window; future archaeologists clicking `git log v1.2.0..main` see "validate v1.2 reqs" as post-ship paperwork rather than part of the ship).

- **D-09:** **ROADMAP drift cleanup: SAME close-out PR as REQUIREMENTS flip + MILESTONE-AUDIT (plan 2).** The audit plan bumps `.planning/ROADMAP.md` § v1.2 Phase Tracker (P17 'Complete' — clear the 'Gap-closure pending' status since `17-VERIFICATION-GAP-CLOSURE.md` shows `status: passed, gaps_remaining: []`; P21 plan count 10/11 → 11/11; P22 plan count 4/6 → 6/6) AND ticks `[ ]` → `[x]` on the P21/P22/P24 entries in § Phases once their phase artifacts are clean. All tracker hygiene in one diff with the REQUIREMENTS flip.

  **Rejected:** separate cleanup plan (ROADMAP drift is the same shape of work as REQUIREMENTS ticks — both are tracker mechanical output of the audit). **Rejected:** defer to `/gsd-complete-milestone v1.2` (that command archives the milestone to `.planning/milestones/v1.2-ROADMAP.md` and rewrites the main ROADMAP with milestone groupings — by then the drift is in the archive, not fixed; readers of the in-flight `.planning/ROADMAP.md` between rc.4 cut and final-tag would see wrong counts).

### Plan structure + cargo-deny + audit/complete flow + README scope (Gray Area 4)

- **D-10:** **Plan structure: 8-plan close-out — 5 docs/CI plans landing in the close-out PR, 3 autonomous=false maintainer-EXECUTES plans for the rc.4 cut + UAT + final tag.** Plan inventory:
  - **Plan 24-01** — Threat model close-out: `THREAT_MODEL.md` (TM5 in-place rewrite + new TM6 + STRIDE rows T-S3/T-T4/T-I4/T-D4 + Changelog) + `README.md` §Security link-back to new anchors (per D-03..D-06).
  - **Plan 24-02** — Milestone audit: invoke `/gsd-audit-milestone v1.2`; produce `.planning/milestones/v1.2-MILESTONE-AUDIT.md` mirroring v1.0 audit shape; mechanical output flips REQUIREMENTS.md ticks for the 20 remaining items + cleans ROADMAP plan-count drift (P17/P21/P22) per D-07..D-09.
  - **Plan 24-03** — `MILESTONES.md` v1.2 entry: new top-of-file release-log entry mirroring v1.1 and v1.0 shapes (header / one-paragraph summary citing five v1.2 features / Tags row `v1.2.0-rc.1` … `v1.2.0` / Phases row 15–24 / Requirements delivered row 41 across 6 categories / Audit row pointing at `.planning/milestones/v1.2-*`).
  - **Plan 24-04** — README updates: §Features pointer block for v1.2 (FCTX panel + exit-code histogram + webhook overview link to `docs/WEBHOOKS.md` + MILESTONES cross-link), v1.2 'What's New' hero block at top of README (per D-13).
  - **Plan 24-05** — cargo-deny WARN→ERROR promotion (FOUND-16): `.github/workflows/ci.yml` flips `continue-on-error: true` → `false` on the `cargo-deny` job; if any advisory has accumulated since rc.1, fix via `deny.toml` exception OR `Cargo.lock` rev within this plan; ensures rc.4 cut is gated by passing `cargo deny check` (per D-11).
  - **Plan 24-06 (autonomous=false)** — `24-RC4-PREFLIGHT.md`: maintainer-EXECUTES runbook mirroring `21-RC2-PREFLIGHT.md` / `23-RC3-PREFLIGHT.md` verbatim. rc.4 → rc.3 / P21 → P24 / FCTX→close-out / plan-list 01-10 → 01-08 substitutions. Cargo.toml unchanged at `1.2.0` (tag-only `-rc.4` suffix). NO `release.yml` / `cliff.toml` / `docs/release-rc.md` edits.
  - **Plan 24-07 (autonomous=false)** — `24-HUMAN-UAT.md`: maintainer runs full v1.2 HUMAN-UAT on the rc.4 image covering (a) `docker compose up` quickstart healthy in 90s + dashboard renders without regression, (b) v1.0/v1.1 surfaces intact (filter / sort / Run Now / Stop / bulk toggle / timeline / sparklines / settings overrides), (c) all five v1.2 features end-to-end (webhooks Standard-Webhooks-v1 + HMAC + retry; custom Docker labels merge + reserved-namespace error; FCTX panel collapsed-by-default on a failed run with the 5 P1 signals; exit-code histogram on job-detail with 10 buckets; tag filter chips on dashboard with AND filter + URL state). Every step references an existing `just` recipe per project memory `feedback_uat_use_just_commands.md`.
  - **Plan 24-08 (autonomous=false)** — `24-FINAL-SHIP-PREFLIGHT.md`: maintainer-EXECUTES final-tag runbook. After rc.4 UAT passes (or rc.N if iterated), retag the last-passing-rc SHA as `v1.2.0` per `docs/release-rc.md` Step 2a/2b. Verify `release.yml` publishes `:1.2.0` + `:1.2` + `:1` + `:latest` on both amd64 + arm64 (the hyphen-gate from P12 D-10 implicitly advances `:latest` from `:1.1.0` since `v1.2.0` contains no hyphen). Verify `git-cliff --tag v1.2.0` output ships as the GitHub Release body (D-15 of P23 inherits). Verify `cargo deny check` is the ERROR gate on the `v1.2.0` tag's CI run (FOUND-16 fully closed). Update `.planning/STATE.md` milestone status to SHIPPED. NB: `/gsd-complete-milestone v1.2` is a SEPARATE post-final-tag command (NOT plan 24-08).

  **Rejected:** Fewer plans (2–3 mega-plans collapsing docs work) — blurs atomic-commit boundary even within one PR; review burden is unchanged; loses per-plan VERIFICATION traceability. **Rejected:** 10+ plans (per-section splits) — over-fragmented for what is logically one close-out pass; signal-to-noise drops; planner overhead exceeds review benefit.

- **D-11:** **cargo-deny WARN→ERROR promotion (FOUND-16): own plan 24-05, lands EARLY in the close-out PR ahead of the maintainer-EXECUTES plans 24-06..08.** Surfaces any accumulated advisory / license / duplicate-version issues since rc.1 (cargo-deny has been WARN-only since Phase 15 D-02) with time to fix `deny.toml` or `Cargo.lock` in the same close-out PR. The fix lives in a focused commit-per-plan diff. Plans 24-06..08 then run against a known-green CI gate.

  **Rejected:** fold into 24-RC4-PREFLIGHT (plan 24-06) — last-minute-bind risk; if cargo-deny surfaces an advisory at preflight time, rc.4 cut stalls and the whole close-out PR re-opens. **Rejected:** pre-stage cargo-deny promotion as a `/gsd-quick` before P24 plan-phase — decouples CI hygiene from the close-out audit narrative; the audit doc (plan 24-02) wants to record cargo-deny WARN→ERROR as a v1.2 close-out act, not as a side-PR.

- **D-12:** **`/gsd-audit-milestone` + `/gsd-complete-milestone` integration: both inline-in-P24 for the audit; complete-milestone is a SEPARATE post-final-tag command.**
  - **`/gsd-audit-milestone v1.2`** IS plan 24-02. The agent (per the workflow's `available_agent_types`: `gsd-integration-checker`) produces `.planning/milestones/v1.2-MILESTONE-AUDIT.md` + mechanical REQUIREMENTS.md flips + ROADMAP drift cleanup. Inline because the audit IS the v1.2 close-out's source of truth for the tick flips (D-07); decoupling would create a "what audit did we use?" lineage question.
  - **`/gsd-complete-milestone v1.2`** runs AFTER the final `v1.2.0` tag publishes. It archives `.planning/milestones/v1.2-ROADMAP.md` + `v1.2-REQUIREMENTS.md`, rewrites the main `.planning/ROADMAP.md` with milestone groupings (mirrors v1.0 + v1.1 archive moves), commits the archive, then runs the PROJECT.md evolution review. NOT a P24 plan — same pattern as v1.0 (Phase 9 close → `/gsd-complete-milestone v1.0`) and v1.1 (Phase 14 close → `/gsd-complete-milestone v1.1`). The `24-FINAL-SHIP-PREFLIGHT.md` (plan 24-08) instructs the maintainer to run `/gsd-complete-milestone v1.2` AFTER the v1.2.0 tag verification step.

  **Rejected:** pre-stage `/gsd-audit-milestone v1.2` before P24 plan-phase — the audit doc would land in a separate PR ahead of the close-out PR; muddies the "single big close-out PR" narrative locked in D-02; risks the audit being stale by the time rc.4 cuts. **Rejected:** hand-author `v1.2-MILESTONE-AUDIT.md` without invoking `/gsd-audit-milestone` — bypasses the integration checker that verifies cross-phase wiring + E2E flows + Nyquist coverage; v1.0/v1.1 set the precedent of using the audit pipeline.

- **D-13:** **README updates scope (plan 24-04): ALL FOUR additions.**
  1. **§Features pointer for FCTX panel + exit-code histogram** — brief subsections (or bullets in an existing §Features list, depending on README's current shape) so operators discover the new v1.2 run-detail / job-detail visualizations.
  2. **§Configuration webhook overview + forward-reference to `docs/WEBHOOKS.md`** — operator discoverability of the v1.2 webhook configuration shape (per-job `webhook = { url, secret_env, states, fire_every }` + `[defaults]` override pattern). Mirrors P17's §Configuration Labels subsection (`README.md:206`) and P23's §Configuration Tag Filter Chips subsection (`README.md:287`) patterns.
  3. **v1.2 'What's New' hero block at top of README** — small block (above §Security at L19) listing the five v1.2 features (webhooks / labels / failure context / exit histogram / tags) with anchor links into the relevant §Configuration / §Features subsections. Same shape as a typical OSS-project README hero-list.
  4. **MILESTONES.md cross-link** — from §Security or a new §Releases footer, link to `MILESTONES.md` and the GitHub Releases page. Operators reading the README discover version history easily; the v1.2 MILESTONES entry from plan 24-03 becomes naturally surfaceable.

  Each addition is a small targeted edit, not a README rewrite. Plan 24-04 produces ONE README diff containing all four. Mirrors the cumulative §Configuration subsection additions across v1.2 (P17 Labels, P23 Tag Filter Chips) — same edit shape, applied here for the v1.2 features that didn't get their own §Configuration block during their phase.

  **Rejected:** subset of additions (operator-discoverability of webhooks is the most-asked v1.2 feature; FCTX + exit histogram need pointers since they're discovered only by clicking into run/job detail; the hero block + MILESTONES cross-link are tiny). **Rejected:** README rewrite (out of scope; v1.2 is additive on top of v1.0/v1.1 README).

### Universal project constraints (carried forward)

> The decisions below are **[informational]** — repo-wide process constraints honored by absence. They are not phase-implementation tasks. Project-memory file references in parentheses.

- **D-14 [informational]:** All Phase 24 changes land via PR on a feature branch. No direct commits to `main`. (`feedback_no_direct_main_commits.md`.)
- **D-15 [informational]:** All diagrams in any Phase 24 artifact (PLAN, MILESTONE-AUDIT, MILESTONES entry, README, THREAT_MODEL, PR description, code comments) are mermaid code blocks. No ASCII art. (`feedback_diagrams_mermaid.md`.)
- **D-16 [informational]:** UAT recipes reference existing/new `just` commands per D-10 plan 24-07; no ad-hoc `cargo` / `docker` / curl-URL invocations. (`feedback_uat_use_just_commands.md`.)
- **D-17 [informational]:** Maintainer validates UAT — Claude does NOT mark UAT passed from its own runs. Plans 24-06, 24-07, 24-08 are `autonomous=false`. (`feedback_uat_user_validates.md`.)
- **D-18 [informational]:** Tag and version match — `Cargo.toml` stays at `1.2.0` through rc.4 cut and final tag (matches v1.2.0 prefix). The `-rc.N` is tag-only. (`feedback_tag_release_version_match.md`.)
- **D-19 [informational]:** `cargo tree -i openssl-sys` must remain empty. Phase 24 adds zero new external crates (cargo-deny promotion is a workflow-level change, not a Rust dependency add). No new TLS / cross-compile surface.

### Claude's Discretion

The planner picks freely on each of the following — none of these were locked during the gray-area selection:

- **Plan 24-02 audit-doc detail level.** v1.0-MILESTONE-AUDIT.md is dense (script summary, 3-source cross-reference, Nyquist compliance, integration paths, tech-debt rollup). Plan 24-02 may invoke `/gsd-audit-milestone v1.2` to produce a full audit OR may scope the audit to "match v1.0 shape exactly with v1.2-specific content" — the audit pipeline's spawned `gsd-integration-checker` produces the structural skeleton; planner judges depth.
- **MILESTONES.md v1.2 entry length.** v1.0 and v1.1 entries are 1 paragraph + Tags row + Phases row + Requirements delivered row + Audit row. Plan 24-03 mirrors. The summary-paragraph shape may include / omit the five-feature itemization depending on planner's style read.
- **README hero block format.** Plan 24-04 hero block can be a single 'v1.2 highlights' paragraph with inline anchor links, a `<details>` collapsible block with feature bullets, or a small mermaid timeline showing milestones. UI-SPEC.md is NOT needed (no visual contract). Planner judges.
- **Plan 24-04 §Configuration webhook subsection depth.** Can be a 2-line forward-pointer to `docs/WEBHOOKS.md` or a brief TOML example mirroring §Labels (`README.md:206`). Planner judges based on `docs/WEBHOOKS.md` shape (which exists per P20 D-30 — verify size before deciding redundancy).
- **Plan 24-04 §Features anchor structure.** Whether FCTX panel + exit histogram get their own §Features subsections, get bullet-listed under an existing §Features section, or get inline anchors under §Configuration subsections — planner judges by current README structure.
- **`cargo-deny` advisory remediation in plan 24-05.** If any advisory has accumulated since rc.1, planner picks between `deny.toml` allowlist exception (with a documented expiry / re-evaluate-date comment) vs `Cargo.lock` rev (with the upstream dependency bump). The choice depends on which is less risky for the v1.2 final ship. Both shapes are precedented by the v1.0/v1.1 dependency hygiene posture.
- **Plan 24-06/07/08 sub-section format.** PREFLIGHT-shape mirrors P21 D-22..D-26 / P23 D-15..D-16 verbatim; HUMAN-UAT shape mirrors v1.1 P14 D-17 / v1.2 P23 D-17 (six scenarios). Planner may consolidate plans 24-07 + 24-08 into a single autonomous=false ship-doc if the substantive ground covered is small.
- **Whether v1.2 needs a `.planning/milestones/v1.2-MILESTONE-AUDIT.md` BEFORE the close-out PR or as PART of it.** D-08 locks "lands in the close-out PR" which is correct; planner may choose to commit the audit doc FIRST inside the PR (before TM5/TM6/README/cargo-deny) to anchor the rest of the close-out narrative.
- **Whether `/gsd-complete-milestone v1.2` is documented in plan 24-08 as an explicit numbered step** vs noted in passing in the trailing prose. Same shape either way; cosmetic.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level

- `.planning/PROJECT.md` § Current Milestone (v1.2 scope — five features: webhooks, custom Docker labels, failure context, exit-code histogram, job tagging; iterative `v1.2.0-rc.N` cadence; `:latest` stays at `v1.1.0` until final `v1.2.0`) and § Constraints (locked tech stack: `sqlx`, TOML, askama+askama_web axum-0.8 feature, rustls invariant, single-binary + Docker image, `127.0.0.1:8080` loopback default, threat model in `THREAT_MODEL.md`, README leads with security section).
- `.planning/REQUIREMENTS.md` § all 41 v1.2 requirements (FOUND-14..16, WH-01..11, LBL-01..06, FCTX-01..07, EXIT-01..06, TAG-01..08) — Phase 24's audit (plan 24-02) ticks the 20 remaining unticked items to `[x] Validated` with phase-reference text. § Research-Phase Corrections (LOCKED) — TM5 / TM6 / Pitfall 56 audit predicates flow through the audit. § Out of Scope — language P24 should preserve in TM5 Recommendations (v1.3 destination filter) and TM6 Recommendations (size limits + reserved-namespace clobber recovery).
- `.planning/ROADMAP.md` § Phase 24: Milestone Close-Out — final `v1.2.0` ship (L326-341) — five operator-observable success criteria + n/a requirements + plans TBD. § Progress (L343-365) — plan-count drift table P24 ticks (P17 'Gap-closure pending' → Complete, P21 10/11 → 11/11, P22 4/6 → 6/6). § v1.2 Build Order (L366-401) — mermaid showing rc.3 → P24 → SHIP.
- `.planning/STATE.md` — current phase state; updated at plan 24-08 to mark v1.2 SHIPPED with final tag + date.

### Threat-model authoritative

- `THREAT_MODEL.md` — current state with TM1–TM4 canonical + TM5 v1.2 stub at L189-229 (carries the "Phase 24 Close-Out" forward pointer at L220-229 that plan 24-01 retires) + STRIDE Summary at L233-281 + Out-of-Band Trust Assumptions at L284-290 + Changelog at L294-300 (P24 adds a row for v1.2 close-out).
- `.planning/research/PITFALLS.md` § Pitfall 56 (L1099-1145) — **CANONICAL spec source** for TM5 / TM6 / STRIDE row additions. Audit predicates T-V12-XCUT-05 (`THREAT_MODEL.md` contains "Threat Model 5: Webhook Outbound" and "Threat Model 6: Operator-supplied Docker labels" sections), T-V12-XCUT-06 (STRIDE table contains rows T-S3, T-T4, T-I4, T-D4), T-V12-XCUT-07 (README links to the new threat-model sections from the security overview). Plan 24-01 uses Pitfall 56 LITERAL text for STRIDE rows + section structure per D-04.
- `.planning/research/PITFALLS.md` § Pitfall 31 / 32 / 38 / 39 / 28 / 30 — referenced inline by Pitfall 56 (SSRF, HTTPS posture, `strip_url_credentials`, reserved-namespace clobber, bounded mpsc + worker isolation, HMAC verification). The TM5/TM6 narrative bodies cite these by reference where Pitfall 56 already did.

### Milestone audit precedent (LOAD-BEARING for plan 24-02)

- `.planning/milestones/v1.0-MILESTONE-AUDIT.md` — **structural template for `v1.2-MILESTONE-AUDIT.md`**. Sections: § Score Summary / § 1. Requirements Coverage — 3-Source Cross-Reference / § 2. Phase Verifications — Status Matrix / § 3. Cross-Phase Integration — Wiring Paths / § 4. End-to-End Flows / § 5. Nyquist Compliance / § 6. Tech Debt Summary / § Verdict Routing / § Next Up. Plan 24-02 mirrors this shape with v1.2-specific content.
- (No `v1.1-MILESTONE-AUDIT.md` exists per `ls .planning/milestones/` — v1.1 archive currently lacks the audit doc, so v1.0 is the only structural precedent. Planner may note this drift but should NOT recreate v1.1's audit retroactively in P24.)

### Milestone release-log precedent (LOAD-BEARING for plan 24-03)

- `MILESTONES.md` L7-25 (v1.1 entry) and L27-end (v1.0 entry) — **shape template for the v1.2 entry**. Each entry has: H2 header with version + dash + name + ` — SHIPPED YYYY-MM-DD`, one-paragraph summary, **Tags:** row listing every rc + final tag, **Phases:** row listing phase numbers + names, **Requirements delivered:** row with category breakdown, **Audit:** row pointing at `.planning/milestones/v[X.Y]-*` archive files.
- `.planning/milestones/v1.1-ROADMAP.md` — archive shape produced by `/gsd-complete-milestone v1.1`. NOT a P24 deliverable but the destination `/gsd-complete-milestone v1.2` will write to.

### Phase 17 precedent (closest TM6 mitigation analog)

- `.planning/phases/17-custom-docker-labels-seed-001/17-CONTEXT.md` — Docker labels feature decisions (reserved-namespace, type-gate, size limits). Plan 24-01 TM6 narrative cites these.
- `.planning/phases/17-custom-docker-labels-seed-001/17-VERIFICATION-GAP-CLOSURE.md` — `status: passed, gaps_remaining: []`. Plan 24-02 audit-doc treats P17 as COMPLETE despite ROADMAP table currently showing 'Gap-closure pending'.
- `src/config/validate.rs` `check_labels_reserved` + `check_labels_only_on_docker_jobs` — Rust validators TM6 Mitigations narrative cites.

### Phase 15 + 18 + 19 + 20 precedent (TM5 mitigation analog)

- `.planning/phases/15-foundation-preamble/15-CONTEXT.md` — webhook delivery worker isolation (bounded `mpsc(1024)` + dedicated worker; scheduler `try_send` never blocking). TM5 Mitigations cites.
- `.planning/phases/18-webhook-payload-state-filter-coalescing/18-CONTEXT.md` — Standard Webhooks v1 payload + state-filter + edge-triggered coalescing. TM5 Mitigations cites.
- `.planning/phases/19-webhook-hmac-signing-receiver-examples/19-CONTEXT.md` — HMAC-SHA256 + base64 signature header + constant-time receiver examples (Python/Go/Node). TM5 Mitigations cites.
- `.planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-CONTEXT.md` — HTTPS-required validator + full-jitter retry + 30s drain + `webhook_deliveries` dead-letter + `cronduit_webhook_*` metrics. TM5 Mitigations cites.
- `src/webhooks/{mod,worker,dispatcher,payload,signing}.rs` — implementation paths the TM5 narrative grounds in.
- `docs/WEBHOOKS.md` — operator-facing webhook docs from Phase 20 D-30. Plan 24-04 README §Configuration forward-references this; verify content/shape before deciding redundancy.

### Phase 14 precedent (CLOSEST close-out analog — v1.1 final ship)

- `.planning/milestones/v1.1-phases/14-bulk-enable-disable-rc-3-final-v1-1-0-ship/14-CONTEXT.md` § D-16 (v1.1.0 = retag rc.3 SHA, bit-identical image) and D-17 (HUMAN-UAT.md checklist with `just` recipes). **Plan 24-08 mirrors D-16 verbatim**; plan 24-07 mirrors D-17's six-step shape adapted for v1.2 features.

### Phase 21 + 23 precedent (rc.N PREFLIGHT shape)

- `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-RC2-PREFLIGHT.md` — autonomous=false maintainer rc preflight runbook. **Plan 24-06 (`24-RC4-PREFLIGHT.md`) mirrors verbatim with rc.4 / P24 / close-out substitutions.**
- `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-RC3-PREFLIGHT.md` — sibling rc preflight. Same shape.
- `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-HUMAN-UAT.md` — autonomous=false maintainer UAT runbook with six scenarios. Plan 24-07 mirrors structurally; scenarios cover full v1.2 regression smoke + new-feature UAT.

### Release runbook (UNCHANGED — reused verbatim)

- `docs/release-rc.md` — the rc cut runbook. Plan 24-06 reuses verbatim per D-10. NO edits in this phase.
- `.github/workflows/release.yml` — `:latest` hyphen-gate from P12 D-10. Implicitly advances `:latest` from `:1.1.0` → `:1.2.0` on the final non-hyphenated `v1.2.0` tag push. Unchanged.
- `.github/workflows/ci.yml` § `cargo-deny` job — Plan 24-05 flips `continue-on-error: true` → `false`. Otherwise unchanged.
- `cliff.toml` — `git-cliff` config. Authoritative source for release body. Unchanged.
- `deny.toml` — Plan 24-05 MAY edit if any advisory has accumulated since rc.1 needs an exception.

### Source files the phase touches

- `THREAT_MODEL.md` — TM5 in-place rewrite (L189-229) + new TM6 section (peer after TM5) + STRIDE rows T-S3/T-T4/T-I4/T-D4 in the L237-281 summary tables + Changelog row at L295 onward (per plan 24-01).
- `README.md` — §Security link-back to TM5/TM6 anchors (plan 24-01) + §Features / §Configuration / v1.2 'What's New' hero block / MILESTONES cross-link (plan 24-04).
- `MILESTONES.md` — new v1.2 entry at top of file (plan 24-03).
- `.planning/REQUIREMENTS.md` — 20 remaining unticked items flipped to `[x] Validated` with phase references (plan 24-02 mechanical output).
- `.planning/ROADMAP.md` — § v1.2 Phase Tracker plan-count corrections P17/P21/P22 + § Phases ticks `[ ]` → `[x]` for P21/P22/P24 (plan 24-02 mechanical output).
- `.planning/milestones/v1.2-MILESTONE-AUDIT.md` — NEW file produced by plan 24-02 via `/gsd-audit-milestone v1.2`.
- `.github/workflows/ci.yml` — `cargo-deny` job `continue-on-error` flip (plan 24-05).
- `deny.toml` (conditional) — exceptions for any accumulated advisories (plan 24-05).
- `Cargo.lock` (conditional) — dep rev for any advisory remediations (plan 24-05).
- `.planning/STATE.md` — milestone status SHIPPED at plan 24-08.
- NEW: `.planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-RC4-PREFLIGHT.md` (plan 24-06).
- NEW: `.planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-HUMAN-UAT.md` (plan 24-07).
- NEW: `.planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-FINAL-SHIP-PREFLIGHT.md` (plan 24-08).

### NEW files (full list)

- `.planning/milestones/v1.2-MILESTONE-AUDIT.md` — plan 24-02.
- `.planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-{01..08}-PLAN.md` + corresponding `-SUMMARY.md` artifacts.
- `.planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-RC4-PREFLIGHT.md` — plan 24-06.
- `.planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-HUMAN-UAT.md` — plan 24-07.
- `.planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-FINAL-SHIP-PREFLIGHT.md` — plan 24-08.

### Cross-reference

- After plan 24-08 publishes the final `v1.2.0` tag and the GHCR images verify, run `/gsd-complete-milestone v1.2` as a SEPARATE post-final-tag command per D-12. It archives `.planning/milestones/v1.2-ROADMAP.md` + `v1.2-REQUIREMENTS.md` and rewrites the main `.planning/ROADMAP.md` with v1.2 grouped under "Milestones."

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets (paperwork shape, not Rust)

- **`THREAT_MODEL.md` TM1–TM4 structure** (`THREAT_MODEL.md:43-186`) — the canonical structural template plan 24-01 follows for the TM5 rewrite + new TM6. Each TM1–TM4 has exactly `### Threat / ### Attack Vector / ### Mitigations / ### Residual Risk / ### Recommendations`. TM5 currently inherits this; plan 24-01 preserves it after stripping the stub framing.
- **`THREAT_MODEL.md` STRIDE Summary tables** (L237-281) — direct insertion site for the four new rows (T-S3 in Spoofing table at L240-242, T-T4 in Tampering at L247-250, T-I4 in Information Disclosure at L261-264, T-D4 in DoS at L269-272). Existing rows have the shape `| T-X1 | <threat> | <mitigation status> |` — plan 24-01 follows.
- **`THREAT_MODEL.md` Changelog table** (L296-300) — existing rows: "Phase 1 skeleton" / "Phase 6 complete" / "Phase 20 stub". Plan 24-01 adds: `| Phase 24 close-out | 2026-05-NN | TM5 canonical rewrite (replaces v1.2 stub); new TM6 (Operator-supplied Docker Labels); STRIDE rows T-S3/T-T4/T-I4/T-D4 added; v1.2 milestone close. |`.
- **`MILESTONES.md` v1.1 + v1.0 entries** (`MILESTONES.md:7-25` and L27-end) — direct shape template for plan 24-03's v1.2 entry. Header / paragraph / Tags / Phases / Requirements delivered / Audit row.
- **`.planning/milestones/v1.0-MILESTONE-AUDIT.md` § Score Summary onward** — structural template for `v1.2-MILESTONE-AUDIT.md` produced by plan 24-02 via `/gsd-audit-milestone v1.2`.
- **`.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-RC2-PREFLIGHT.md`** — verbatim mirror target for plan 24-06's `24-RC4-PREFLIGHT.md`.
- **`.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-HUMAN-UAT.md`** — structural mirror target for plan 24-07's `24-HUMAN-UAT.md`.
- **`.planning/milestones/v1.1-phases/14-.../14-CONTEXT.md` D-16** — direct mirror for plan 24-08's final-tag retag-the-rc-SHA discipline.
- **`docs/release-rc.md`** — rc cut runbook reused verbatim by plan 24-06 (`docs/release-rc.md` Step 2a/2b for signed/unsigned).
- **`.github/workflows/release.yml`** — `:latest` hyphen-gate (P12 D-10) implicitly advances `:latest` from `:1.1.0` → `:1.2.0` on the v1.2.0 tag push. NO edit needed.
- **`.github/workflows/ci.yml`** — `cargo-deny` job currently with `continue-on-error: true` per Phase 15 D-02. Plan 24-05 flips to `false`.

### Established Patterns

- **Maintainer-cuts-tag-locally trust anchor** (Phase 12 D-13) — every rc and final tag is cut on the maintainer's machine, NOT via `workflow_dispatch`. Plans 24-06 and 24-08 honor this.
- **`git-cliff --unreleased` authoritative release body** (Phase 12 D-12) — no hand-edit post-publish. Plan 24-08 verifies this.
- **`:latest` advances only on non-hyphenated tags** (Phase 12 D-10) — implicit; nothing to do in P24 except confirm post-publish.
- **Atomic-commit-per-plan** — each P24 plan = one commit inside the close-out PR. Project convention.
- **`autonomous=false` for maintainer-EXECUTES plans** — plans 24-06/07/08 carry the flag; Claude does NOT execute the rc tag command, the UAT, or the final retag. Mirrors v1.1 P14, P20, P21, P23 precedent.
- **README §Configuration subsections per feature** — P17 added §Labels (L206), P23 added §Tag Filter Chips (L287). Plan 24-04 may follow this shape for webhooks (§Webhooks) if the planner judges `docs/WEBHOOKS.md` insufficient as the single discoverability surface.
- **Milestone archive happens AFTER the milestone ships** — v1.0 and v1.1 both followed this; v1.2 mirrors via `/gsd-complete-milestone v1.2` as a separate post-final-tag command per D-12.

### Integration Points

- **Plan 24-01 → README link-back anchors** — the new `#threat-model-5-webhook-outbound` and `#threat-model-6-operator-supplied-docker-labels` anchors are GitHub-Markdown-derived from the H2 headings; plan 24-04 may also reference them in §Security if §Security isn't fully covered by plan 24-01.
- **Plan 24-02 → REQUIREMENTS.md + ROADMAP.md** — the audit doc IS the source of truth; tick flips and table corrections are derived. If the audit surfaces a "Partial" or "Unsatisfied" item, that requirement does NOT get ticked, and the audit doc records the gap with remediation plan.
- **Plan 24-03 → MILESTONES.md** — additive at top of file. Pushes v1.1 entry to L29+ and v1.0 entry further down.
- **Plan 24-05 → CI gate behavior change** — once `continue-on-error: false` lands, every PR commit (including subsequent rc.4 close-out PR commits) must pass `cargo deny check`. If an advisory surfaces between rc.4 cut and final-tag, the fix is a HOTFIX PR per the project-wide PR-only policy (D-14).
- **Plan 24-06 (rc.4 cut) → CI artifacts** — `release.yml` publishes `:v1.2.0-rc.4` + `:rc` rolling tag. Verifies image-publish flow before the final tag relies on it.
- **Plan 24-08 (final v1.2.0 retag) → GHCR multi-tag publish** — the hyphen-gate fires once; `:1.2.0` + `:1.2` + `:1` + `:latest` all publish on both amd64 + arm64. ROADMAP success criterion #4 verifies the four-tag equality.

</code_context>

<specifics>
## Specific Ideas

- **The audit doc is the load-bearing artifact of plan 24-02.** REQUIREMENTS flips + ROADMAP corrections are mechanical derivations. If `/gsd-audit-milestone v1.2` finds a gap (Partial / Unsatisfied / Orphan), the close-out PR cannot land plan 24-02's flips for that requirement; the audit doc records the gap with remediation, and either (a) the gap is closed in a follow-up plan inside P24 if remediation is small, or (b) v1.2 ships with the gap noted as deferred-to-v1.3 in `.planning/PROJECT.md` § Future Requirements. Plan 24-02's PLAN.md should call out this branching explicitly.

- **TM5/TM6 narrative depth calibration.** TM1–TM4 average ~30-40 lines per section (excluding boilerplate). Plan 24-01's TM5 + TM6 should match — neither thinner (loses parity with TM1–TM4) nor thicker (signals over-importance). The existing TM5 stub is ~40 lines; the canonical rewrite should land at ~50-60 lines (gains Recommendations + cross-link to TM6). TM6 should be ~30-40 lines.

- **Pitfall 56 audit predicate language is the test rubric.** `T-V12-XCUT-05` says `THREAT_MODEL.md` contains "Threat Model 5: Webhook Outbound" and "Threat Model 6: Operator-supplied Docker labels" sections (with that capitalization). Plan 24-01 MUST use those exact section titles. Drift to "Threat Model 6: Operator Docker Labels" (without "supplied") fails the audit predicate.

- **`v1.2.0-rc.4` is the FIRST P24 rc cut.** Despite the project-wide "rc.N+1 means real findings happened" convention, here rc.4 is a clean docs+CI cut after rc.3 passed UAT. The rc.4 release notes (`git-cliff --unreleased --tag v1.2.0-rc.4`) will be SMALL — likely 3-5 commits since rc.3 (the close-out PR's plan commits). That's correct; rc.4 exists to gate final-tag publication on a clean CI + UAT pass, not to introduce a substantive code delta.

- **The cargo-deny WARN→ERROR promotion may be uneventful or eventful.** Plan 24-05 PLAN.md should write a two-branch outcome:
  - Branch A (uneventful): `cargo deny check` is already green at WARN level → flip `continue-on-error` → done.
  - Branch B (eventful): one or more advisories accumulated since rc.1 → fix via `deny.toml` exception or `Cargo.lock` rev → re-run CI → flip `continue-on-error` → done.
  Plan 24-05's effort is bounded by Branch B's worst case. Estimate small (most deps have been quiet); but the plan must be ready for it.

- **`/gsd-complete-milestone v1.2` is a follow-up command, not a P24 plan.** v1.1 followed this pattern (Phase 14 ended with v1.1.0 tag; `/gsd-complete-milestone v1.1` ran afterward). Plan 24-08's `24-FINAL-SHIP-PREFLIGHT.md` lists `/gsd-complete-milestone v1.2` as the FINAL step the maintainer runs after verifying `:latest` advanced. NOT included in the P24 plan count.

- **README hero block timing — pre-rc.4 only.** The 'v1.2 What's New' hero block (plan 24-04) lands in the close-out PR BEFORE rc.4 cuts; rc.4 image's README has the hero block. If the planner instead defers the hero block to a post-final-tag commit, the v1.2.0 tag points at a SHA whose README still says "this project ships Cronduit v1.1+" — an avoidable inconsistency. Lock: hero block in the close-out PR.

- **The MILESTONE-AUDIT plan order matters narratively.** Plan 24-02 (audit) should commit FIRST in the close-out PR so it can be cited by plans 24-03 (MILESTONES entry summary cites the audit "passed" verdict) and 24-04 (README hero block summary mirrors the audit's score-summary lead). Plan order in the close-out PR: 24-02 → 24-01 → 24-03 → 24-04 → 24-05 → (PR merge) → 24-06 (rc.4 cut) → 24-07 (rc.4 UAT) → 24-08 (final retag). Atomic-commit-per-plan still holds.

- **rc.4 UAT scope vs rc.3 UAT scope.** rc.3 UAT was tag-filter-chips-specific (P23 was the rc.3 feature). rc.4 UAT must be FULL v1.2 regression — every feature shipped in P15–23 (webhooks end-to-end, custom labels, FCTX panel, exit histogram, tag chips) PLUS the v1.0/v1.1 surfaces (filter / sort / Run Now / Stop / bulk toggle / timeline / sparklines / settings overrides / healthcheck). The first time the full v1.2 stack gets a single-session smoke test is plan 24-07.

</specifics>

<deferred>
## Deferred Ideas

- **Webhook destination allow/block-list filter** — v1.3 candidate per `.planning/PROJECT.md` § Future Requirements; called out explicitly in canonical TM5 Recommendations.
- **Web UI authentication** — deferred to v2 per `.planning/REQUIREMENTS.md` § Out of Scope; `THREAT_MODEL.md` TM2 (Untrusted Client) holds the line in v1.2.
- **Cross-run log search across retention window** — v1.3 candidate per `.planning/PROJECT.md` § Current Milestone (punted at v1.2 kickoff).
- **Job concurrency limits / queuing** — v1.3 candidate per `.planning/PROJECT.md` § Current Milestone.
- **Tag-based bulk operations on dashboard bulk-action bar** — v1.3 candidate per `.planning/REQUIREMENTS.md` § Out of Scope; reaffirmed P23 deferred.
- **Tag autocomplete / search-as-you-type in chip strip** — v1.3 candidate; P22 + P23 deferred.
- **Per-tag job count badge on chips** — UI-SPEC decision deferred to v1.3 polish.
- **Tag chips on `/jobs/{id}` job detail page** — Phase 23 was dashboard-only per TAG-06; v1.3+ extension.
- **Tags / labels as Prometheus labels** — explicit out-of-scope (cardinality discipline); same posture as exit codes per EXIT-06.
- **Tag-based webhook routing keys** — WH-09 carries tags in payload but never AS a routing key; v1.3+.
- **Browser-based playwright HTMX smoke tests** — adds new test infrastructure not in tree; v1.3 candidate.
- **`v1.1-MILESTONE-AUDIT.md` retroactive creation** — v1.1 currently lacks the audit doc that v1.0 has and v1.2 will have. Planner may note the inconsistency in plan 24-02's audit doc § Tech Debt Summary; retroactive creation is OUT of scope for P24.
- **`/gsd-complete-milestone v1.2` workflow** — separate post-final-tag command per D-12; archives `.planning/milestones/v1.2-ROADMAP.md` + `v1.2-REQUIREMENTS.md`, rewrites main `.planning/ROADMAP.md` with milestone groupings.
- **PROJECT.md evolution review** — happens INSIDE `/gsd-complete-milestone v1.2` per the workflow's `<archival_behavior>`. Not a P24 plan.
- **New v1.3 milestone kickoff** — `/gsd-complete-milestone v1.2` offers to create the next milestone inline. Acceptance / deferral is the maintainer's call AFTER v1.2 ships.
- **`docs/release-rc.md` / `release.yml` / `cliff.toml` modifications** — not in this phase; reused verbatim per D-10 plan 24-06 and per P12 D-10..D-12 / P20 D-30 / P21 D-22..D-26 / P23 D-15..D-16.

</deferred>

---

*Phase: 24-milestone-close-out-final-v1-2-0-ship*
*Context gathered: 2026-05-16*
