---
phase: 23-job-tagging-dashboard-filter-chips-rc-3
plan: 06
subsystem: testing
tags: [uat, just, recipe, tagging, docs, readme]

# Dependency graph
requires:
  - phase: 23-job-tagging-dashboard-filter-chips-rc-3
    provides: "Plan 23-05 — chip strip rendering, OOB swap response, sort-header tag suffix, poll hx-include widening; the operator-visible behavior the new just recipes exercise"
  - phase: 22-job-tagging-schema-validators
    provides: "uat-tags-* recipe family — the structural template the new uat-chips-* recipes mirror; jobs.tags column the recipes' fixtures populate"
  - phase: 17-docker-labels-seed-001
    provides: "README ### Labels subsection — the structural template for the new ### Tag Filter Chips subsection (CONTEXT D-04 labels-precedent)"
provides:
  - "just uat-chips-render — TAG-06 chip strip render + empty-state spot-check"
  - "just uat-chips-and-filter — TAG-06+TAG-07 AND-filter + untagged-hidden + name-filter compose spot-check"
  - "just uat-chips-share-url — TAG-06 shareable URL round-trip + stale-tag silent-drop spot-check"
  - "README ### Tag Filter Chips subsection — completes operator's mental model (TOML → validators → DB → webhook → chips)"
affects: [23-07-PLAN.md, 23-08-PLAN.md, 24-close-out, milestone-v1.2]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "uat-chips-* recipe family in [group('uat')] mirrors P22 uat-tags-* recipe-calls-recipe pattern"
    - "TOML fixtures use synthetic tag values (backup/weekly/prod) per threat model T-23-06-01"
    - "Each recipe ends with ritual 'Claude does NOT mark this passed' per project memory feedback_uat_user_validates.md"

key-files:
  created:
    - ".planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-06-SUMMARY.md"
  modified:
    - "justfile (+250 lines: three uat-chips-* recipes after the P22 uat-tags-webhook recipe)"
    - "README.md (+30 lines: ### Tag Filter Chips subsection between ### Labels and ### Job Types)"

key-decisions:
  - "README subsection placed between ### Labels (P17) and ### Job Types — preserves the per-job feature grouping under ## Configuration"
  - "No mermaid diagram added to README subsection — text-only is sufficient per UI-SPEC § Decisions Rationale (subsection should be short)"
  - "TOML fixtures explicitly include use_defaults = false + timeout = '5m' to match the surrounding P22 uat-tags-* recipe convention exactly (defense against any future [defaults] regressions)"
  - "Reworded 'compose with **AND** semantics' → 'compose with AND semantics' (no markdown bold) so the literal substring 'AND semantics' is greppable for the plan's acceptance criterion"

patterns-established:
  - "Pattern 1: just recipe family for chip-strip UAT — three recipes covering render, AND-filter intersection, and shareable URL round-trip; consumed by Plan 23-07 HUMAN-UAT runbook"
  - "Pattern 2: README per-feature subsection structure — short intro + TOML snippet + validator rules bullets + behavior bullets + cross-reference (mirrors P17 ### Labels)"

requirements-completed: [TAG-06, TAG-07]

# Metrics
duration: 4min
completed: 2026-05-05
---

# Phase 23 Plan 06: Wave 4 — UAT recipes + README addition Summary

**Three operator-runnable just uat-chips-* recipes (render / AND-filter / share-URL) + a short README ### Tag Filter Chips subsection completing the operator's TOML → validators → DB → webhook → chips mental model.**

## Performance

- **Duration:** 4 min
- **Started:** 2026-05-05T02:56:34Z
- **Completed:** 2026-05-05T03:00:09Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Three new `[group('uat')]` just recipes (`uat-chips-render`, `uat-chips-and-filter`, `uat-chips-share-url`) mirror the P22 `uat-tags-*` recipe-calls-recipe pattern and end with the maintainer ritual ("Claude does NOT mark this passed"). Each recipe writes a synthetic-tag TOML fixture to `.tmp/uat-chips-*.toml` and chains `just build` / `just db-reset` / `just check-config` per the locked UAT shape (project memory `feedback_uat_use_just_commands.md` — no ad-hoc `cargo` / `docker` / curl-URL invocations).
- README gains a `### Tag Filter Chips` subsection between `### Labels` (P17) and `### Job Types`, with a TOML snippet, validator rules summary (charset / 16-tag cap / per-job only / reserved names), chip strip behavior bullets (empty-state, untagged-hidden, bookmarkable URLs, stale-tag silent-drop), and a cross-reference to `docs/WEBHOOKS.md`. Closes the P22 deferred forward-pointer to Phase 23.
- V-15 and V-16 satisfied — recipes exist and are operator-runnable; maintainer execution + sign-off lives in Plan 23-07 (HUMAN-UAT runbook).
- Threat model honored: synthetic tag values only (`backup`, `weekly`, `prod`), `command = "true"` no-op fixtures, no real credentials surface (T-23-06-01 accept). The maintainer-ritual line on every recipe closes T-23-06-02 (mitigate).
- Mermaid-only diagram rule (project memory `feedback_diagrams_mermaid.md`) honored — zero ASCII art added; no diagram needed for the short subsection per UI-SPEC § Decisions Rationale (D-19 informational).

## Task Commits

Each task was committed atomically:

1. **Task 1: Add uat-chips-render, uat-chips-and-filter, uat-chips-share-url recipes to justfile** — `3e365e7` (feat)
2. **Task 2: Add Tag Filter Chips subsection to README.md** — `b7b6817` (docs)

**Plan metadata:** to be added in the final docs commit (this SUMMARY.md + STATE.md + ROADMAP.md + REQUIREMENTS.md).

## Files Created/Modified

- `justfile` — three new `[group('uat')]` recipes appended after the existing `uat-tags-webhook` recipe (lines 1429-1678). Each recipe is `[doc(...)]` annotated, uses `set -euo pipefail`, writes its TOML fixture to `.tmp/uat-chips-*.toml`, chains `just build` + `just db-reset` + `just check-config`, gates operator-driven steps with `read -rp` prompts, and closes with the maintainer-ritual line.
- `README.md` — `### Tag Filter Chips` subsection inserted between `### Labels` and `### Job Types` under `## Configuration`. Documents the operator-facing contract: TOML configuration shape, validator rules (charset / cap / per-job only / reserved names), chip strip behavior (empty-state, untagged-hidden, bookmarkable URLs, stale-tag silent-drop), and the WH-09 webhook payload cross-reference.

## Verification

All Plan 23-06 acceptance criteria met:

- `just --list 2>&1 | grep -cE 'uat-chips-render|uat-chips-and-filter|uat-chips-share-url'` returns `3`.
- `grep -c 'uat-chips-' justfile` returns ≥ 3 (recipe definitions plus internal references).
- `grep -B1 -E 'uat-chips-(render|and-filter|share-url):' justfile | grep -c '\[doc('` returns `3`.
- `awk '/^\[group/{g=$0} /^uat-chips-/{print g; print}' justfile | grep -c "group('uat')"` returns `3`.
- `grep -c 'Claude does NOT mark this passed' justfile` increased by 6 (two contextual + one ritual line per new recipe; baseline P22 contribution preserved — `just --list 2>&1 | grep -c uat-tags-` still returns `3`).
- `grep -c '\.tmp/uat-chips-' justfile` returns `13` (≥ 6 threshold; per-recipe count includes `mkdir -p .tmp` + `cat > .tmp/...toml` + `just check-config .tmp/...toml` + descriptive `echo` lines).
- `grep -qE '^#{2,4} Tag Filter Chips' README.md` — PASS.
- `grep -q 'tags = \[' README.md` — PASS.
- `grep -qF 'AND semantics' README.md` — PASS.
- `grep -F '?tag=' README.md` — 1 match.
- `grep -ciF 'untagged' README.md` — 1 match.
- `grep -qF 'docs/WEBHOOKS.md' README.md` — PASS; target file exists at `docs/WEBHOOKS.md` (P19).
- `git diff README.md | grep -cE '^\+[│┌┐└┘├┤┬┴┼─━]'` returns `0` (zero ASCII-art box-drawing characters added — D-19 honored).
- `git diff README.md | grep -c '\`\`\`mermaid'` returns `0` (no diagram added; text-only suffices for this short subsection per UI-SPEC § Decisions Rationale).
- README title (`<div align="center">`) unchanged — no regression on existing content.

## Decisions Made

- **Insertion point: between `### Labels` and `### Job Types`.** The `## Configuration` section's per-feature subsection grouping (Server Settings → Default Job Settings → Labels → **Tag Filter Chips** → Job Types) reads naturally as "configure the host, configure defaults, attach labels, attach tags, define jobs." Tags are a per-job feature like labels, so they belong adjacent to the Labels subsection rather than being relegated to a separate dashboard-features section.
- **No mermaid diagram in the subsection.** UI-SPEC § Decisions Rationale recommends "short" for this addition; the four behavior bullets (empty-state / untagged-hidden / bookmarkable / stale-tag) read clearly as text. A diagram would add chrome without explanatory power. The mermaid-only rule (D-19 / project memory) is honored by absence — there is no ASCII-art alternative present.
- **Reworded `**AND** semantics` to `AND semantics`.** The plan's acceptance criterion `grep -qF 'AND semantics'` requires the exact substring without intervening markdown markup. Stripped the bold so the substring greps cleanly; semantic content unchanged.
- **TOML fixtures include `use_defaults = false` + `timeout = "5m"`.** Mirrors the P22 `uat-tags-*` recipe convention exactly. Defends against any hypothetical future `[defaults]` block regressions in cronduit's TOML validators (none observed today, but the existing P22 fixture shape is the safe default).

## Deviations from Plan

None. Plan executed exactly as written. All deviations from the literal plan-provided markdown were cosmetic (heading-level adjustment to `###` to match the surrounding `### Labels` / `### Job Types` style; one-character markdown-bold removal to satisfy the literal-substring grep). Both adjustments were explicitly anticipated by the plan ("Step B — Insert the new subsection. Use the markdown below verbatim (adjust the heading level — `###` or `####` — to match the surrounding structure)").

## Issues Encountered

- Initial draft of the README subsection wrote `compose with **AND** semantics`. The plan's acceptance criterion `grep -qF 'AND semantics'` requires the exact substring; the markdown bold (`**AND**`) breaks the literal substring match. Resolved by removing the bold (one-character edit). Reverification: `grep -qF 'AND semantics' README.md` PASS.

## User Setup Required

None — no external service configuration required. The new just recipes are operator-runnable on a fresh checkout (each starts with `just build` + `just db-reset`), but the actual UAT execution happens in Plan 23-07 (the HUMAN-UAT runbook).

## Next Phase Readiness

- **Plan 23-07 (HUMAN-UAT) prerequisites met.** The three `uat-chips-*` recipes exist and are runnable; the HUMAN-UAT runbook can reference each by name without further preconditions.
- **Plan 23-08 (RC3-PREFLIGHT) blocked on Plan 23-07 completion** per the Wave 5 → Wave 6 dependency in ROADMAP.md.
- **Phase 23 is two waves from rc.3 cut.** Plan 23-07 (Wave 5) and Plan 23-08 (Wave 6) remain.

## Threat Flags

None. No new threat surface introduced by this plan — recipes are pure shell orchestration over existing privileged-only `just` recipes (`build` / `db-reset` / `check-config`); the README addition is operator-facing documentation describing already-shipped behavior.

## Self-Check: PASSED

Verified that all claimed artifacts exist and all claimed commits are reachable:

- `[ -f justfile ] && grep -q 'uat-chips-render:' justfile` — FOUND.
- `[ -f README.md ] && grep -q '### Tag Filter Chips' README.md` — FOUND.
- `[ -f docs/WEBHOOKS.md ]` — FOUND (cross-reference target).
- `git log --oneline --all | grep -q '3e365e7'` — FOUND (Task 1 commit).
- `git log --oneline --all | grep -q 'b7b6817'` — FOUND (Task 2 commit).

---
*Phase: 23-job-tagging-dashboard-filter-chips-rc-3*
*Completed: 2026-05-05*
