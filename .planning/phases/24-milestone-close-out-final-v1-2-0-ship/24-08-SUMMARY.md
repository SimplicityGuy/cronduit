---
phase: 24
plan: 08
subsystem: paperwork / maintainer-runbook
tags: [phase-24, final-ship, v1.2.0, retag, runbook, autonomous-false]
dependency_graph:
  requires:
    - "Plan 24-07 (24-HUMAN-UAT.md authored — referenced by § 1)"
    - "Plan 24-06 (24-RC4-PREFLIGHT.md authored — referenced by § 4 previous :latest digest)"
    - "Plan 24-05 (cargo-deny WARN→ERROR promotion — referenced by § 5)"
    - "Plan 24-03 (MILESTONES.md v1.2 entry — referenced by § 7 SHIPPED date finalization)"
    - "docs/release-rc.md Step 2a/2b (reused verbatim — referenced by § 3)"
    - ".github/workflows/release.yml hyphen-gate from P12 D-10 (verified by § 4)"
    - "v1.1 P14 D-16 retag-the-rc-SHA discipline (structural mirror)"
  provides:
    - "Maintainer-EXECUTES final-tag (v1.2.0) runbook"
    - "Bit-identical retag of last-passing-UAT rc.N SHA as v1.2.0"
    - "Four-tag equality verification (:1.2.0 == :1.2 == :1 == :latest) on amd64 + arm64"
    - "cargo-deny ERROR-gate verification on v1.2.0 tag CI run (FOUND-16 closure)"
    - "git-cliff cumulative release body verification (v1.1.0..v1.2.0)"
    - "Maintainer instructions to flip STATE.md + finalize MILESTONES.md SHIPPED date at § 7"
    - "Hand-off to /gsd-complete-milestone v1.2 (separate post-final-tag command per D-12)"
  affects:
    - "Post-execution: .planning/STATE.md milestone status → SHIPPED (by maintainer at § 7)"
    - "Post-execution: MILESTONES.md v1.2 entry SHIPPED date placeholder finalized (by maintainer at § 7)"
    - "Post-execution: GHCR :latest digest advances from :1.1.0 to :1.2.0 (via release.yml hyphen-gate)"
tech_stack:
  added: []
  patterns:
    - "autonomous=false maintainer-EXECUTES runbook (matches plans 24-06 / 24-07 + P21/P23 RC preflights)"
    - "Retag-the-rc-SHA bit-identical discipline (mirrors v1.1 P14 D-16)"
    - "Sign-off table with :latest invariant FLIPPED vs rc preflights (NOW must equal :1.2.0 digest)"
    - "Four-tag equality verification via docker manifest inspect (per-arch digest match)"
    - "Cross-reference footer documenting the v1.1 → v1.2 substitution"
key_files:
  created:
    - ".planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-FINAL-SHIP-PREFLIGHT.md (214 lines)"
  modified: []
decisions:
  - "Mirrored v1.1 P14 D-16 final-tag discipline verbatim with v1.1 → v1.2 substitution + :latest invariant flip"
  - "Section 7 (STATE.md flip + MILESTONES.md SHIPPED date finalization) is a MAINTAINER step inside the runbook, NOT a plan 24-08 Claude commit — placeholder removal happens at actual ship time"
  - "Section 8 references /gsd-complete-milestone v1.2 as a SEPARATE post-final-tag command per CONTEXT D-12 (NOT a P24 plan)"
  - "Used `git tag -a -s` (signed) per docs/release-rc.md Step 2a as preferred path; Step 2b (unsigned) as fallback only"
  - "Sign-off table added new rows for :1.2 and :1 four-tag equality verification (extends 21-RC2-PREFLIGHT.md sign-off shape)"
metrics:
  duration_seconds: 540
  completed: 2026-05-16
---

# Phase 24 Plan 08: Final v1.2.0 Ship Pre-Flight Summary

Authored `24-FINAL-SHIP-PREFLIGHT.md` — the 214-line maintainer-EXECUTES final-tag runbook that retags the last-passing-UAT rc.N SHA as `v1.2.0` (bit-identical image, mirroring v1.1 P14 D-16 "what was tested is what ships" discipline).

## Outcome

One file created (`.planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-FINAL-SHIP-PREFLIGHT.md`, 214 lines). One commit (`62978f2`). The runbook is ready for the maintainer to execute AFTER plan 24-07's HUMAN-UAT sign-off lands on the rc.4 (or iterated rc.5/rc.6) tag. NO source code, CI workflow, release runbook, or Cargo.toml modifications.

## Runbook Structure (8 sections + sign-off + out-of-scope + cross-reference)

| § | Title | Maintainer action |
|---|-------|------------------|
| 1 | rc.N UAT passed | Verify all 6 scenarios + Final sign-off filled in `24-HUMAN-UAT.md` |
| 2 | Identify rc.N SHA to retag | `RC_SHA=$(git rev-list -n 1 v1.2.0-rc.N)` |
| 3 | Retag command (LOCAL) | `git tag -a -s v1.2.0 -m "v1.2 — Operator Integration & Insight" "$RC_SHA"` + push |
| 4 | `:latest` hyphen-gate + four-tag equality | `docker manifest inspect` for `:1.2.0` / `:1.2` / `:1` / `:latest` on both arches |
| 5 | cargo-deny ERROR-gate verification | `gh run view` confirms `just deny` was a required step (no `continue-on-error: true`) |
| 6 | git-cliff cumulative release body | `git cliff v1.1.0..v1.2.0` shipped verbatim as the GitHub Release body |
| 7 | STATE.md flip + MILESTONES.md SHIPPED date | Maintainer flips milestone status + replaces `SHIPPED YYYY-MM-DD` placeholder |
| 8 | Run `/gsd-complete-milestone v1.2` | Separate post-final-tag command per CONTEXT D-12 — archives milestone artifacts |

## The retag-the-rc-SHA discipline (mirrored from P14 D-16)

> `v1.2.0` MUST retag the LAST PASSING-UAT rc.N SHA (rc.4 if UAT passed first time; rc.5 / rc.6 / etc. if iterated). Bit-identical image — what the maintainer UAT-validated is what ships. NO new commits between rc.N and v1.2.0.

If rc.4 UAT surfaces a finding, fixes land in a follow-up close-out PR → rc.5 cut → UAT → the LAST passing rc.N SHA gets retagged as `v1.2.0`. Substitution from v1.1:

- `v1.1.0` → `v1.2.0`
- `rc.3` → `last-passing-UAT rc.N` (rc.4 if clean first time; rc.5+ if iterated)
- Tag message `"v1.1 — Operator Quality of Life"` → `"v1.2 — Operator Integration & Insight"`
- `:latest` invariant flipped: `:latest` now MUST equal `:1.2.0` digest (rather than staying at `:1.1.0`)

## Four-tag equality verification commands (§ 4)

The runbook's § 4 verification loop:

```bash
for tag in 1.2.0 1.2 1 latest; do
  echo "== :$tag =="
  docker manifest inspect ghcr.io/simplicityguy/cronduit:$tag | jq '.manifests[] | { arch: .platform.architecture, digest: .digest }'
done
```

Plus a before/after `:latest` digest comparison to confirm advancement from `:1.1.0` → `:1.2.0`:

```bash
PREV_LATEST_DIGEST="<from plan 24-06 § Sign-off — the :1.1.0 digest>"
NEW_LATEST_DIGEST=$(docker manifest inspect ghcr.io/simplicityguy/cronduit:latest | jq -r '.manifests[0].digest')
test "$PREV_LATEST_DIGEST" != "$NEW_LATEST_DIGEST" && echo "ADVANCED" || echo "STUCK — investigate"
```

The release.yml hyphen-gate from P12 D-10 (verified by inspection at `.github/workflows/release.yml:132-134`) fires implicitly because `v1.2.0` contains no hyphen — `:1.2.0` + `:1.2` + `:1` + `:latest` all publish on both amd64 + arm64 in one tag-push event. Satisfies ROADMAP Phase 24 success criterion #4.

## `:latest` invariant FLIP vs plan 24-06

The sign-off table in `24-RC4-PREFLIGHT.md` (plan 24-06) asserts `:latest` digest MUST equal `:1.1.0` digest (because `v1.2.0-rc.4` is hyphenated and the gate skips). The sign-off table in this plan (`24-FINAL-SHIP-PREFLIGHT.md`) flips that invariant: `:latest` MUST NOW equal `:1.2.0` digest. To make the flip explicit and auditable, the sign-off table records both the new `:latest` digest AND the previous `:latest` digest (was `:1.1.0`) as separate rows. New rows for `:1.2` and `:1` were added (vs the rc preflight's shape) to capture all three rolling tags participating in the four-tag equality assertion.

## Separation between plan 24-08 and `/gsd-complete-milestone v1.2`

Per CONTEXT D-12, plan 24-08 ends at "v1.2.0 tag published + verified." The follow-up `/gsd-complete-milestone v1.2` command (a SEPARATE invocation from a fresh Claude session, NOT a P24 plan) is responsible for:

1. Archiving `.planning/milestones/v1.2-ROADMAP.md` and `v1.2-REQUIREMENTS.md` (snapshots of the current ROADMAP v1.2 zone + REQUIREMENTS).
2. Rewriting the main `.planning/ROADMAP.md` with milestone groupings (mirrors v1.0 + v1.1 archive moves).
3. Committing the archive.
4. Running the PROJECT.md evolution review.
5. Offering to create the next milestone (v1.3) inline (maintainer decides accept or defer).

Plan 24-08's § 8 surfaces `/gsd-complete-milestone v1.2` as the maintainer's final step but does NOT execute it. The discipline matches v1.0 (Phase 9 close → `/gsd-complete-milestone v1.0`) and v1.1 (Phase 14 close → `/gsd-complete-milestone v1.1`).

## STATE.md / MILESTONES.md scope clarification

The plan's frontmatter lists `.planning/STATE.md` and `MILESTONES.md` in `files_modified` with `may_change: true`. These are **maintainer-driven** edits executed at runbook § 7 (NOT Claude commits in plan 24-08). Specifically:

- **STATE.md:** Maintainer flips `milestone: v1.2` `status: planning` → `status: shipped`, sets `last_updated` to NOW, bumps `progress.completed_phases` to 10 and `progress.percent` to 100, updates Current Position to `Phase 24 / Plan 24-08 (SHIPPED) / Status: Milestone v1.2 SHIPPED`.
- **MILESTONES.md:** Maintainer replaces the existing `SHIPPED YYYY-MM-DD` placeholder in the v1.2 entry's H2 header (`## v1.2 — Operator Integration & Insight — SHIPPED YYYY-MM-DD`, authored by plan 24-03) with the actual ship date.

Both edits land as a single `chore(24): finalize v1.2 ship — STATE.md + MILESTONES.md ship date` commit per the runbook's § 7 instructions. Plan 24-08 itself contains ONE commit (the runbook).

## Deviations from Plan

None — plan executed exactly as written. The plan's `<action>` block specified the runbook content verbatim (frontmatter + preamble + 8 numbered sections + sign-off + out-of-scope + cross-reference). The author followed it section-by-section.

## Verification

Automated gate (from the plan's `<verify><automated>` block) returned `PASS` — all 8 sections present with correct titles + keywords, frontmatter has `final_tag: v1.2.0` + `autonomous: false`, signed `git tag -a -s` command present, `/gsd-complete-milestone v1.2` cross-reference present, `:latest` invariant flip language present, "Previous `:latest` digest (was `:1.1.0`)" row present, file length 214 lines (≥ 120 required).

Pitfall-56-style audit predicates do not apply here (this is the final-tag runbook, not the threat-model artifact — TM5/TM6 predicates close in plan 24-01). Plan 24-08's predicates are:

- 8-section runbook structure complete: ✅
- Retag-the-rc-SHA discipline preserved verbatim from P14 D-16: ✅
- `:latest` four-tag equality verification on both arches: ✅
- cargo-deny ERROR-gate verification (FOUND-16 closure): ✅
- git-cliff cumulative release body (v1.1.0..v1.2.0): ✅
- STATE.md + MILESTONES.md SHIPPED date instructions in § 7: ✅
- `/gsd-complete-milestone v1.2` separation in § 8: ✅
- NO release.yml / cliff.toml / docs/release-rc.md / Cargo.toml edits: ✅

## Known Stubs

None. The runbook is fully populated. The `<rc.N-SHA>` placeholder in § 2-3 and the digest blanks in the sign-off table are INTENTIONAL — they are runtime values the maintainer captures during execution, not authoring stubs.

## Self-Check: PASSED

- File present: `/Users/Robert/Code/public/cronduit/.claude/worktrees/agent-a5ee3fafe0913d2df/.planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-FINAL-SHIP-PREFLIGHT.md` ✅
- Commit present: `62978f2` ✅
- Plan's automated verify gate: `PASS` (all assertions met) ✅
- No accidental writes to main repo (initial Write misroute moved + main repo confirmed clean) ✅
