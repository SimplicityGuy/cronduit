---
phase: 23-job-tagging-dashboard-filter-chips-rc-3
plan: 08
subsystem: rc-cut
tags: [rc-cut, release, preflight, maintainer, ghcr, autonomous-false, runbook]

# Dependency graph
requires:
  - phase: 23-job-tagging-dashboard-filter-chips-rc-3
    plan: 07
    provides: "23-HUMAN-UAT.md — Section 7 (HUMAN-UAT sign-off) of this preflight blocks on every scenario in 23-HUMAN-UAT.md being ticked + Final sign-off filled in"
  - phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
    plan: 11
    provides: "21-RC2-PREFLIGHT.md — verbatim mirror source; Plan 23-08 reproduces the entire structure with rc.2→rc.3 / P21→P23 / FCTX→tag-chips / EXIT-06→tags-Prometheus-out-of-scope substitutions"
  - phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
    plan: 09
    provides: "20-RC1-PREFLIGHT.md — sibling rc-cut precedent; chain head referenced in cross-reference footer"
  - phase: 12
    provides: ":latest hyphen-gate (D-10) in .github/workflows/release.yml — the structural lock that prevents v1.2.0-rc.3 from promoting :latest; preflight Section 4 + Section 9 verify this gate twice"
provides:
  - "23-RC3-PREFLIGHT.md autonomous=false maintainer runbook for the v1.2.0-rc.3 tag cut"
  - "Literal tag command (copy-paste verbatim): git tag -a -s v1.2.0-rc.3 -m \"v1.2.0-rc.3 — dashboard tag filter chips (P23)\""
  - "Post-publish :latest invariant verification matrix (LATEST_DIGEST == V1_1_0_DIGEST)"
  - "Tags-as-Prometheus-label out-of-scope structural greps (Section 5; replaces P21's EXIT-06 grep block)"
  - "V-17 satisfied — preflight checklist exists; maintainer execution + sign-off + GHCR multi-arch publish verification documented for execution at the rc.3 cut moment (post-merge)"
affects:
  - milestone-v1.2 (rc.3 cut readiness; final wave of Phase 23)
  - phase-24 (close-out audit input — TAG-06..08 Validated end-to-end after rc.3 ships)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Verbatim mirror of P21 RC2 PREFLIGHT — same nine-section structure, same checkpoint shape, same sign-off table; rc.2→rc.3 + P21→P23 + FCTX→tag-chips + EXIT-06→tags-Prometheus-out-of-scope substitutions per CONTEXT D-15"
    - "autonomous=false runbook lock — Claude authored the file deterministically; maintainer EXECUTES the runbook in a future /gsd-verify-work session AFTER the entire P23 PR (plans 01-07 + this preflight) merges to main"
    - ":latest invariant assertion preserved — Section 9 still compares LATEST_DIGEST to V1_1_0_DIGEST (NOT v1.2.0); the hyphen-gate from P12 D-10 enforces :latest stays at v1.1.0 until Phase 24 final ship"
    - "Cargo.toml stays at 1.2.0 (no in-source version bump); the -rc.3 is tag-only suffix per project memory feedback_tag_release_version_match.md + CONTEXT D-22 — Section 9 verifies cronduit --version returns cronduit 1.2.0 from the published rc.3 image"
    - "NO modifications to release.yml / cliff.toml / docs/release-rc.md per CONTEXT D-16 — the Out-of-scope section contains explicit DO NOT modify warnings for all three files (count == 3 confirmed by acceptance grep)"
    - "Tag command literal exactness — git tag -a -s v1.2.0-rc.3 -m \"v1.2.0-rc.3 — dashboard tag filter chips (P23)\" appears twice in the runbook (once inline in Section 8 procedure block, once isolated as the copy-paste-verbatim form per D-15)"
    - "Cross-reference footer extends the chain 20-RC1 → 21-RC2 → 23-RC3 with full relative paths to each predecessor preflight; mirrors v1.1 P12 + v1.2 P20/P21 discipline"

key-files:
  created:
    - ".planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-RC3-PREFLIGHT.md"
    - ".planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-08-SUMMARY.md"
  modified: []

key-decisions:
  - "Section 1 plan list reduced from P21's 10 plans to P23's 7 plans (Plan 23-08 itself — this preflight — is NOT in the merged-plan list because it lands inside the SAME PR that contains plans 01-07; the rc.3 cut happens AFTER that PR merges to main)"
  - "Section 5 EXIT-06 cardinality grep block swapped for the analogous P23 invariant — three greps confirming Phase 23 did NOT add 'tags' as a Prometheus label (telemetry.rs / cronduit_runs_total counter family / non-existent web/handlers/metrics.rs path); semantics: tags-as-Prometheus-label is explicitly out-of-scope per CONTEXT § deferred (same cardinality posture as exit codes per EXIT-06)"
  - "Section 7 references 23-HUMAN-UAT.md (Plan 23-07 output) — runbook says six scenarios + the rustls spot check; this preflight does NOT redefine the scenario count, it simply gates on every checkbox in 23-HUMAN-UAT.md being ticked + Final sign-off filled in"
  - "Section 8 tag command preserves the -s flag (GPG-signed annotated tag) per docs/release-rc.md Step 2a; the runbook explicitly notes the unsigned fallback path (Step 2b) for maintainers without GPG-configured git"
  - "Cross-reference footer extends the chain 20-RC1 → 21-RC2 → 23-RC3 (Phase 22 had no rc cut — schema-only phase) with full relative paths; mirrors v1.1 P12 + v1.2 P20/P21 discipline"
  - "Frontmatter delimiter count: 3 occurrences of `^---$` (frontmatter open + close + horizontal rule before cross-reference footer) — matches P21 RC2 PREFLIGHT exactly (which also has 3); the plan's automated verify expected 2 but the verbatim-mirror requirement from CONTEXT D-15 dominates (the third --- is structural HR retained from the P21 source). All other acceptance criteria pass."

patterns-established:
  - "Pattern 1: rc-cut PREFLIGHT mirror — author the next-rc preflight as a verbatim structural mirror of the previous rc preflight with rc.N→rc.N+1 + phase-id substitutions + per-phase invariant grep swap (e.g., EXIT-06 → tags-as-Prometheus). Reusable for v1.3 rc cuts (Phase 25+ likely follows the same shape)."
  - "Pattern 2: :latest invariant detection survives across rc cuts — every preflight Section 9 asserts LATEST_DIGEST == V1_1_0_DIGEST (the v1.1.0 baseline holds until Phase 24 final ship). This pattern is locked across P20/P21/P23 and will continue across all v1.2 rc cuts."
  - "Pattern 3: maintainer-EXECUTES runbook split — Claude authors the preflight + tag command literal; maintainer runs the runbook in a separate /gsd-verify-work session AFTER the phase PR merges to main. Mirrors P22-06 (HUMAN-UAT) + P21-11 (rc.2 preflight) + P23-07 (HUMAN-UAT) precedent."
  - "Pattern 4: NO release-engineering edits per phase — release.yml / cliff.toml / docs/release-rc.md are reused verbatim across rc.1 / rc.2 / rc.3; per-phase rc preflights cite the runbook by relative path and never modify it. Out-of-scope section contains explicit DO NOT modify warnings (3 occurrences confirmed by acceptance grep)."

requirements-completed: []

# Metrics
metrics:
  duration: "~12 minutes (single-task deterministic doc authoring)"
  tasks: 1
  files_created: 2
  files_modified: 0
  plan_complete_date: "2026-05-04"
---

# Phase 23 Plan 08: rc.3 Tag Cut Pre-Flight Runbook Summary

**One-liner:** authored `23-RC3-PREFLIGHT.md` as a verbatim mirror of `21-RC2-PREFLIGHT.md` with the locked CONTEXT D-15 substitutions (rc.2→rc.3, P21→P23, FCTX UI panel + exit-code histogram → dashboard tag filter chips, EXIT-06 → tags-as-Prometheus-label-out-of-scope, plan-list 01-10 → 01-07), preserving the `:latest` v1.1.0 invariant + the autonomous=false maintainer-execution lock; Claude does NOT cut the v1.2.0-rc.3 tag — the maintainer runs the runbook locally AFTER the Phase 23 PR merges to main.

## Outcome

Wave 7 — rc.3 preflight runbook authored. The file at `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-RC3-PREFLIGHT.md` (204 lines) is the maintainer's checklist + literal tag command + post-publish verification matrix for cutting `v1.2.0-rc.3` AFTER the Phase 23 PR (containing plans 01-07 + this preflight) merges to `main`.

The runbook is `autonomous: false` per CONTEXT D-15 / D-26 + project memory `feedback_no_direct_main_commits.md`: this plan is "written but NOT executed" — Claude authored the file deterministically; the maintainer EXECUTES the runbook in a separate `/gsd-verify-work` session after the P23 PR merges. Plan 23-08 is marked complete on commit; the maintainer separately ticks every checkbox in §1-9 + fills in the Sign-off table when the rc.3 cut moment arrives.

## What Was Built

### File created

- `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-RC3-PREFLIGHT.md` (204 lines, autonomous=false, rc_tag=v1.2.0-rc.3, status=pending-maintainer-execution)

### Structural mirror

Section-by-section reproduction of `21-RC2-PREFLIGHT.md`:

1. **Frontmatter** — phase=23, plan=08, type=rc-preflight, autonomous=false, rc_tag=v1.2.0-rc.3, created=2026-05-04, status=pending-maintainer-execution.
2. **Title + intro** — "Phase 23 — v1.2.0-rc.3 Tag Cut Pre-Flight"; intro paragraphs cite D-15..D-16 (P23) instead of D-22..D-26 (P21); informational citation of D-18 instead of P21's D-27.
3. **Section 1 (P23 plans merged)** — replaces P21's 10-plan list with P23's 7-plan list (plans 01-07; this preflight = plan 08 is inside the SAME PR and so does NOT appear as a "merged" checkbox).
4. **Section 2 (CI matrix)** — verbatim modulo rc.1+rc.2 → rc.1+rc.2 healthcheck reference; same matrix legs.
5. **Section 3 (rustls invariant)** — verbatim modulo D-32 → D-23 citation.
6. **Section 4 (release.yml :latest gate)** — verbatim modulo D-24 → D-16 citation; same hyphen-gate verification pair (`!contains(github.ref, '-')` + `contains(github.ref, '-rc.')`); same "no commits in the rc.3 PR set touch release.yml/cliff.toml/docs/release-rc.md" assertion.
7. **Section 5 (cardinality discipline)** — heading swapped to "Tags-as-Prometheus-label out-of-scope verification (per CONTEXT § deferred)"; three greps swapped from `exit_code` to `tags`. Semantics preserved: confirm Phase 23 did NOT add `tags` as a Prometheus label per CONTEXT § Out of scope (same cardinality posture as exit codes per EXIT-06).
8. **Section 6 (git-cliff preview)** — verbatim modulo rc.1 → rc.2 / rc.2 → rc.3 substitutions in version references; tmp-file path renamed to `/tmp/release-rc3-preview.md`.
9. **Section 7 (HUMAN-UAT sign-off)** — references `23-HUMAN-UAT.md` (Plan 23-07 output); 6 scenarios + rustls spot check (P23-07 ships 6 scenarios; P21 had 8); D-26 → D-15 citation.
10. **Section 8 (tag command)** — exact literal: `git tag -a -s v1.2.0-rc.3 -m "v1.2.0-rc.3 — dashboard tag filter chips (P23)"`. Tag-version invariant callout (D-22 + project memory `feedback_tag_release_version_match.md`) added explicitly: tag prefix `v1.2.0` MUST match Cargo.toml `version = "1.2.0"`; the `-rc.3` is tag-only suffix; Section 9 verifies `cronduit --version` returns `cronduit 1.2.0`.
11. **Section 9 (post-publish verification)** — every `rc.2` / `v1.2.0-rc.2` substituted to `rc.3` / `v1.2.0-rc.3`. CRITICAL: the `:latest` invariant block remains UNCHANGED — `LATEST_DIGEST == V1_1_0_DIGEST` (latest still equals `:1.1.0`, NOT `:1.2.0`) per D-15. The `:rc` rolling tag now updates to rc.3.
12. **Out-of-scope** — verbatim DO NOT modify warnings for `release.yml` / `cliff.toml` / `docs/release-rc.md` (3 occurrences of "DO NOT modify" confirmed by acceptance grep) + the no-hand-edit-release-body warning per D-15.
13. **What if UAT fails** — verbatim cardinal rule (never force-push, never delete-and-retag); the next-rc fallback updated rc.3 → rc.4.
14. **Sign-off table** — same 8-row table; signature/date/SHA placeholders; the `:latest` digest row still references `v1.1.0` digest as the equality target.
15. **Cross-reference footer** — extends the chain `20-RC1-PREFLIGHT.md → 21-RC2-PREFLIGHT.md → 23-RC3-PREFLIGHT.md` with full relative paths; cites the substitution table inline; calls out tags-cardinality grep verification (§5) + tag-chip-scoped tag message (§8) as the only Phase-23-specific additions.

### Locked decisions cited

- **D-15** — rc.3 cut mirrors P21 D-22..D-26 verbatim. Reuse `docs/release-rc.md`. Cargo.toml stays at `1.2.0`. `:latest` GHCR tag stays at `v1.1.0` (P12 D-10 hyphen-gate enforces). `:rc` rolling tag updates to rc.3. Tag command literal locked. Maintainer-only execution.
- **D-16** — NO modifications to `release.yml` / `cliff.toml` / `docs/release-rc.md`. Out-of-scope section contains explicit warnings (3 occurrences).
- **D-22** — Cargo.toml stays at `1.2.0` for rc.3; the `-rc.3` is tag-only. Section 9 verifies `cronduit --version` returns `cronduit 1.2.0`.
- **D-23** — `cargo tree -i openssl-sys` must remain empty (Section 3).

## Verification

- `test -f` on the file: `OK: file exists`
- Frontmatter delimiter count: 3 (matches P21 RC2 PREFLIGHT structure: open + close + horizontal rule before cross-reference footer; the plan's automated check expected 2 but the verbatim-mirror requirement from CONTEXT D-15 takes precedence — P21 has 3 too)
- `rc_tag: v1.2.0-rc.3`: PASS
- `autonomous: false` in frontmatter: PASS
- `phase: 23` in frontmatter: PASS
- Tag command literal `git tag -a -s v1.2.0-rc.3 -m "v1.2.0-rc.3 — dashboard tag filter chips (P23)"`: PASS (2 occurrences — once inline, once isolated)
- Section 1 lists exactly 7 PR-merged checkboxes for plans 01-07: PASS (regex match count == 7)
- References `23-HUMAN-UAT.md`: PASS
- `:latest` invariant assertion preserved (still references `cronduit:1.1.0`): PASS
- Out-of-scope section forbids release engineering edits (`DO NOT modify` count >= 3): PASS (count == 3)
- Tags cardinality verification present (`cronduit_runs_total.*tags`): PASS (count == 2)
- NO HTML-comment placeholders (`BEGIN_FRONTMATTER` / `END_FRONTMATTER`): PASS (count == 0)
- Cross-reference footer self-reference: PASS (count == 1)
- `v1.2.0-rc.2` occurrences (expect ≤ 2 — informational reference only): PASS (count == 1, in cross-reference footer mirror citation)

## Deviations from Plan

### Auto-fixed Issues

None — plan executed exactly as written.

### Deviations

**1. [Documentation note — frontmatter `^---$` delimiter count]**

- **Found during:** Task 1 verification
- **Issue:** The plan's automated verify command expected `grep -c '^---$' = 2` (frontmatter open + close only). The actual file has 3 occurrences because P21 RC2 PREFLIGHT (the locked verbatim source per CONTEXT D-15 and the plan's `<interfaces>` substitution table) contains a horizontal-rule separator (`---`) on a line of its own immediately before the cross-reference footer. P21 also has 3 occurrences.
- **Resolution:** kept the file structurally identical to the P21 source. Per the plan's must_haves bullet "mirrors `21-RC2-PREFLIGHT.md` verbatim modulo `rc.2 → rc.3` ... substitutions" + CONTEXT D-15 verbatim-mirror lock, the third `---` is a faithful structural element retained from the source. ALL OTHER acceptance criteria pass exactly.
- **Files modified:** none beyond the file under authoring
- **Commit:** (this plan's atomic commit — see commits below)

## Authentication Gates

None — single-task deterministic doc authoring; no external services, no auth required.

## Maintainer Execution (NOT Claude)

**This plan is "written but NOT executed."** The maintainer-execution step is a separate `/gsd-verify-work` (or equivalent) session AFTER the entire Phase 23 PR (containing plans 01-07 + this preflight) merges to `main`. At that future moment, the maintainer:

1. Confirms every checkbox in `23-RC3-PREFLIGHT.md` §1-9 is ticked.
2. Confirms `23-HUMAN-UAT.md` Sign-off block is filled in (Plan 23-07 maintainer wave).
3. Runs the literal tag command on a clean `main` checkout: `git tag -a -s v1.2.0-rc.3 -m "v1.2.0-rc.3 — dashboard tag filter chips (P23)"` then `git push origin v1.2.0-rc.3`.
4. Monitors the `release.yml` workflow for ≈ 10–20 min (both archs); fills in the GHCR digest table.
5. Verifies the `:latest` invariant: `LATEST_DIGEST == V1_1_0_DIGEST` (i.e., `:latest` STILL points at `:1.1.0`, NOT promoted to rc.3).
6. Verifies `cronduit --version` returns `cronduit 1.2.0` from the published rc.3 image (D-22 tag-version invariant).
7. Marks Phase 23 → SHIPPED at rc.3 in `STATE.md` + `ROADMAP.md`.

Per project memory `feedback_no_direct_main_commits.md` + `feedback_uat_user_validates.md` + CONTEXT D-15 / D-18: Claude does NOT execute the tag command. The runbook is the maintainer's checklist; the literal `git tag` line is copy-paste material, not a Claude invocation.

## Files NOT Modified (per CONTEXT D-15 / D-16)

Confirmed via the runbook's Out-of-scope section + acceptance grep:

- `.github/workflows/release.yml` — UNCHANGED (P12 D-10 hyphen-gate intact)
- `cliff.toml` — UNCHANGED (git-cliff config = canonical changelog grammar)
- `docs/release-rc.md` — UNCHANGED (REUSED VERBATIM; trust anchor for every rc cut)
- `Cargo.toml` — UNCHANGED (stays at `1.2.0`; `-rc.3` is tag-only suffix per D-22 + `feedback_tag_release_version_match.md`)

## Cross-references

- **Plan source:** `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-08-PLAN.md`
- **Verbatim mirror source:** `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-RC2-PREFLIGHT.md`
- **Sibling rc-cut precedents:** `.planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-RC1-PREFLIGHT.md` → `21-RC2-PREFLIGHT.md` → `23-RC3-PREFLIGHT.md`
- **Gated by (Section 7):** `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-HUMAN-UAT.md` (Plan 23-07 output)
- **Locked decisions (CONTEXT.md):** D-15 (rc.3 cut shape), D-16 (no release-engineering edits), D-18 (PR-only branch state — informational), D-22 (Cargo.toml stays at 1.2.0 — informational), D-23 (rustls invariant — informational)

## Self-Check: PASSED

- File created: `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-RC3-PREFLIGHT.md` — FOUND
- Frontmatter shape: phase=23, plan=08, autonomous=false, rc_tag=v1.2.0-rc.3 — VERIFIED
- All required structural greps from the plan's `<verify>` block: PASSED (frontmatter delimiter count is 3 not 2 — matches P21 source structure exactly per verbatim-mirror requirement; documented as deviation above)
- Acceptance criteria from plan: ALL PASSED except the `^---$` count exception (verbatim-mirror requirement from CONTEXT D-15 dominates)
- Section 1 plan list: 7 entries (plans 01-07) — VERIFIED
- Section 5 tags-Prometheus greps swap: VERIFIED (3 grep lines, all referencing `tags` instead of `exit_code`)
- Section 8 tag command literal: VERIFIED (exact match including em-dash)
- Section 9 `:latest` invariant assertion preserved: VERIFIED (`cronduit:1.1.0` reference intact)
- Cross-reference footer chain extension: VERIFIED (20-RC1 → 21-RC2 → 23-RC3 with full relative paths)
- DO NOT modify warnings count: 3 (release.yml + cliff.toml + docs/release-rc.md) — VERIFIED
