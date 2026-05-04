---
phase: 22-job-tagging-schema-validators
plan: 06
subsystem: testing

tags: [uat, human-validation, maintainer, tagging, runbook, autonomous-false]

# Dependency graph
requires:
  - phase: 22-job-tagging-schema-validators
    provides: "Three uat-tags-* maintainer recipes (uat-tags-persist, uat-tags-validators, uat-tags-webhook) — Plan 22-05 D-11"
  - phase: 22-job-tagging-schema-validators
    provides: "TAG-01..05 + D-08 schema/validator/persistence/webhook surface authored by Plans 22-01..04 and locked by the integration tests in Plan 22-05"
  - phase: 18-webhooks-mvp
    provides: "uat-webhook-mock + uat-webhook-verify recipes (referenced by Scenario 4 chain)"
  - phase: 20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1
    provides: "20-HUMAN-UAT.md structural template (closest analog: autonomous=false multi-scenario maintainer runbook with eyeball criteria + sign-off blocks)"
provides:
  - ".planning/phases/22-job-tagging-schema-validators/22-HUMAN-UAT.md — four-scenario maintainer runbook covering D-10's full validation matrix"
  - "PR-merge-gating signal: Phase 22 cannot ship until the maintainer ticks every checkbox in the runbook (in a separate /gsd-verify-work session, NOT in this planning session)"
affects: [phase-22-pr-merge, phase-23-rc3-cut, phase-24-milestone-closeout]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "autonomous: false runbook lock — plan-level frontmatter declares the maintainer (not Claude) as the sign-off authority; cited verbatim from project memory feedback_uat_user_validates.md"
    - "just-recipe-only operator steps — every scenario step is `just uat-tags-*`; no raw cargo run / docker run / curl in operator-facing lines (project memory feedback_uat_use_just_commands.md)"
    - "Mermaid-only diagrams (none in this runbook, but the pattern is preserved across the planning artifacts in this phase)"
    - "Two-commit shape for runbook plans: (a) author the runbook, (b) author the SUMMARY — separate commits keep the runbook diff readable in PR review"

key-files:
  created:
    - ".planning/phases/22-job-tagging-schema-validators/22-HUMAN-UAT.md (123 lines; 4 scenarios + Final sign-off)"
  modified: []

key-decisions:
  - "Runbook frontmatter delimiters are literal `^---$` lines (not HTML-comment placeholders) — verified by deterministic grep `grep -c '^---$'` returning exactly 2"
  - "Scenario 1 references `just uat-tags-persist`; Scenario 2 + 3 share `just uat-tags-validators` (the same recipe drives both validator-error fixtures AND the dedup WARN fixture per Plan 22-05 D-11); Scenario 4 references `just uat-tags-webhook` chained with `just uat-webhook-verify`"
  - "Eyeball criteria are SPECIFIC, not 'looks good': Scenario 1 names the exact JSON string `[\"backup\",\"prod\",\"weekly\"]`; Scenario 2 names the exact regex `^[a-z0-9][a-z0-9_-]{0,30}$`, the exact reserved list, and the exact substring/count error message shapes; Scenario 3 quotes the canonical WARN line shape from 22-CONTEXT.md L548-553; Scenario 4 names the exact substring `\"tags\":[\"backup\",\"weekly\"]` AND the two regression-shapes that would FAIL (`\"tags\":[]` and insert-order)"
  - "Both project memories (feedback_uat_user_validates.md + feedback_uat_use_just_commands.md) cited near the top of the runbook in a blockquote so the maintainer reads the lock posture before the first scenario"
  - "Cargo references appear ONLY in the Prerequisites bullets (`cargo build` / `cargo test --test v12_tags_validators`) as preflight gates, NOT as operator-facing scenario steps — explicitly permitted by the plan's <acceptance_criteria>"
  - "All four checkboxes left unticked + Maintainer name/Date blanks left empty — Claude does NOT pre-fill or self-sign per the autonomous: false lock"

patterns-established:
  - "Phase-22 UAT runbook = single self-contained file under .planning/phases/<phase>/ with deterministic frontmatter + four numbered scenarios + Final sign-off block — mirrors Phase 20's 20-HUMAN-UAT.md shape and is structurally validated by the same grep family"
  - "Recipe-call-out shape inside scenarios: each scenario's 'Steps' block leads with `just <recipe>` and follows with eyeball-criterion bullets; the recipe encapsulates all cargo/sqlite3/python3 plumbing so the operator never types those directly"

requirements-completed: [TAG-01, TAG-02, TAG-03, TAG-04, TAG-05]

# Metrics
duration: ~5min
completed: 2026-05-04
---

# Phase 22 Plan 06: Maintainer UAT Runbook Summary

**Four-scenario maintainer-validated UAT runbook for Phase 22 job tagging — autonomous: false; gates Phase 22 PR merge on the maintainer's eyeball sign-off, not on Claude's automated tests; covers TAG-01..05 + WH-09 closure.**

## Performance

- **Duration:** ~5 min (single Write call + structural grep verification + two commits)
- **Started:** 2026-05-04T20:09:00Z (approximate)
- **Completed:** 2026-05-04T20:14:09Z
- **Tasks:** 1 (write the runbook; structural greps satisfy `<verify><automated>`)
- **Files modified:** 1 (1 created, 0 edited)

## Accomplishments

- Authored `.planning/phases/22-job-tagging-schema-validators/22-HUMAN-UAT.md` with four scenarios in D-10 order:
  1. **Persistence spot-check (TAG-02)** via `just uat-tags-persist` — eyeball: `jobs.tags` column shows `["backup","prod","weekly"]` exactly.
  2. **Validator error UX walk (TAG-03 + TAG-04 + TAG-05 + D-08)** via `just uat-tags-validators` — eyeball: each of four cases (charset / reserved / substring-collision / count-cap) produces an operator-readable error that names the offending VALUE, the RULE violated, and a FIX hint.
  3. **Dedup-collapse WARN (TAG-03)** via `just uat-tags-validators` — eyeball: WARN line names ALL THREE original inputs (`["Backup", "backup ", "BACKUP"]`) plus the canonical form `["backup"]`, NOT just the canonical form alone.
  4. **End-to-end webhook backfill (WH-09)** via `just uat-tags-webhook` chained with `just uat-webhook-verify` — eyeball: delivered POST body contains substring `"tags":["backup","weekly"]` (sorted-canonical), proving WH-09 closed end-to-end.
- Cited both project memories (`feedback_uat_user_validates.md`, `feedback_uat_use_just_commands.md`) near the top in a blockquote.
- Declared `autonomous: false` in the runbook frontmatter so future automation cannot mistakenly auto-tick the boxes.
- Left every checkbox unticked + Maintainer name/Date blanks empty per the project memory lock — Claude does NOT sign UAT.

## Task Commits

Each task was committed atomically:

1. **Task 1: Write `22-HUMAN-UAT.md` runbook** — `a60c5a4` (docs)

**Plan metadata:** `<this-commit>` (docs(22-06): complete UAT runbook authoring plan)

## Files Created/Modified

- `.planning/phases/22-job-tagging-schema-validators/22-HUMAN-UAT.md` (created, 123 lines) — the four-scenario maintainer runbook
- `.planning/phases/22-job-tagging-schema-validators/22-06-SUMMARY.md` (created — this file)

## Structural Gates (all passing)

The plan's `<verify><automated>` pipeline is a single chained-bash gate. Each clause is documented below with its observed result:

| Gate | Spec | Observed |
|------|------|----------|
| File exists | `test -f 22-HUMAN-UAT.md` | OK |
| `^---$` count | exactly `2` | `2` |
| `BEGIN_FRONTMATTER` / `END_FRONTMATTER` placeholders | exactly `0` | `0` |
| `^## Scenario [1-4] ` heading count | exactly `4` | `4` |
| `^autonomous: false` line | present | OK |
| `feedback_uat_user_validates.md` citation | ≥ 1 match | OK |
| `feedback_uat_use_just_commands.md` citation | ≥ 1 match | OK |
| `just uat-tags-(persist|validators|webhook)` references | ≥ `4` | `4` |
| `Final sign-off` section | present | OK |
| `^\s*(cargo run|docker run|curl )` operator-step lines | `0` | `0` |
| `docker run` anywhere in file | `0` | `0` |

Plan-required full automated grep returned `ALL GATES PASS`.

## User-prompt-specified verification gates (also passing)

| Gate | Spec | Observed |
|------|------|----------|
| 1. File exists and is non-empty | non-empty | 123 lines |
| 2. `just uat-tags-` count | ≥ 4 | 4 |
| 3. `cargo` references outside `just uat-` lines | only in Prerequisites bullets / policy citation, not as operator-facing steps | 3 lines (1 policy citation in the leading blockquote, 2 in Prerequisites preflight bullets) — explicitly permitted by the plan's <acceptance_criteria> |
| 4. `docker run` count | 0 | 0 |
| 5. `git diff` paths under phase dir only | yes | only `.planning/phases/22-job-tagging-schema-validators/22-HUMAN-UAT.md` and `.planning/phases/22-job-tagging-schema-validators/22-06-SUMMARY.md` |

## Decisions Made

- **Verbatim transcription of plan-specified runbook content.** The plan's `<action>` block contained the exact runbook body to write; deviation from that wording would invite drift between the plan's structural greps and the file. The runbook is the canonical artifact; the plan was the spec.
- **No mermaid diagram added.** The plan did not request one; the four scenarios are linear (recipe → expected output → eyeball criterion) and a diagram would not aid the maintainer. Project memory `feedback_diagrams_mermaid.md` only mandates the format WHEN diagrams are used, not that diagrams must exist.
- **All sign-off blanks left empty.** The plan's `<action>` is explicit: "Claude does NOT tick the boxes; Claude does NOT mark Plan 06 complete by running through the scenarios itself." The runbook ships with `[ ]` checkboxes and blank `Maintainer name: ________` / `Date: ________` lines.

## Deviations from Plan

None — plan executed exactly as written. The runbook content matches the plan's `<action>` block verbatim; structural greps all pass on the first write; no auto-fixes were necessary.

## Issues Encountered

None.

## User Setup Required

None — this plan ships only a markdown runbook; no env vars, no external services, no DB migrations. The maintainer will run the runbook in a separate `/gsd-verify-work` session against an existing Plan 22-05 working tree.

## Maintainer Sign-off Pointer

The runbook's "Final sign-off" section (at `.planning/phases/22-job-tagging-schema-validators/22-HUMAN-UAT.md` lines 119–124) is the gating block:

> - [ ] **Maintainer:** I have run all four scenarios on a clean working tree against a feature branch with Plans 01–05 applied. Each scenario produced the expected operator-readable output. WH-09 is closed end-to-end. Phase 22 is UAT-complete and ready to merge.
>
> Maintainer name: ________
> Date: ________

Phase 22 PR merge blocks until the maintainer ticks all five boxes (one per scenario + the final aggregate) and fills name + date.

## Cross-references

- **Plan 22-05 SUMMARY** (the recipes the runbook uses): `.planning/phases/22-job-tagging-schema-validators/22-05-SUMMARY.md`
- **Plan 22-05 PLAN** (full recipe spec): `.planning/phases/22-job-tagging-schema-validators/22-05-PLAN.md`
- **Phase 20 HUMAN-UAT** (closest structural analog): `.planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-HUMAN-UAT.md`
- **Phase 22 CONTEXT D-10** (the four-scenario lock this runbook discharges): `.planning/phases/22-job-tagging-schema-validators/22-CONTEXT.md`
- **Phase 22 VALIDATION § "Manual-Only Verifications"**: `.planning/phases/22-job-tagging-schema-validators/22-VALIDATION.md` lines 78-83
- **Project memories cited in the runbook:**
  - `feedback_uat_user_validates.md` (only the maintainer signs UAT)
  - `feedback_uat_use_just_commands.md` (every UAT step is a `just` recipe)

## Plan-level posture: written but NOT executed

This plan's `autonomous: false` declaration carries a specific semantic:

- **Claude's responsibility (this session):** Write the runbook deterministically; pass the structural greps; commit on the feature branch. **Done.**
- **Maintainer's responsibility (a separate `/gsd-verify-work` session, NOT this one):** Run the four scenarios on a clean working tree; eyeball each output against the criterion; tick the checkboxes; sign + date the Final sign-off block.
- **Phase 22 PR merge gate:** the maintainer's signed runbook is the merge signal — NOT this SUMMARY's existence, NOT the green CI build, NOT Claude's verification of structural greps.

The runbook is ready for `/gsd-verify-work` maintainer sign-off.

## Next Phase Readiness

- **Phase 22 PR:** ready to open. PR description must mention that `22-HUMAN-UAT.md` requires maintainer execution + sign-off before merge (this is part of the PR's manual review step, NOT something Claude can self-assert).
- **Phase 23 (rc.3 cut):** unblocked once Phase 22 merges. Phase 22 closes WH-09 end-to-end (the last open milestone-v1.2 webhook requirement).
- **No blockers.**

## Self-Check: PASSED

- File `.planning/phases/22-job-tagging-schema-validators/22-HUMAN-UAT.md` exists (verified `test -f` returned 0).
- Commit `a60c5a4` exists in `git log --oneline` (verified).
- All structural gates above pass.

---
*Phase: 22-job-tagging-schema-validators*
*Completed: 2026-05-04*
