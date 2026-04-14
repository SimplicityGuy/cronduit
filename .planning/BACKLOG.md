# Cronduit v1.1 Backlog

**Purpose:** Seed list of items deferred from the v1.0 milestone, primarily
surfaced during Phase 8's human UAT walkthrough. This file is committed so the
v1.1 milestone kickoff has a ready-made starting point.

**Entry numbering:** Uses the `999.X` convention per project docs — 999.1,
999.2, ... — to distinguish v1.1 backlog items from v1.0 requirement IDs
(FOUND-01, UI-05, etc.).

**Triage rubric (from Phase 8 D-26 / D-28):**

- **Functional breakage → fix in Phase 8 before v1.0 archive.** Examples: a
  job fails to run, a page crashes, a toast never appears, a live log stream
  hangs, auto-refresh stops working, a docker pull errors out silently.
- **Visual polish / copy / edge cases → v1.1 backlog entry.** Examples:
  spacing or color contrast tweaks within brand tolerance, copy wording
  nitpicks, dark-mode rendering edge cases that still render, cosmetic
  alignment on narrow viewports.
- **Ambiguous → default to backlog** unless it blocks a v1.0 success
  criterion in ROADMAP.md.

## Entry Template

Copy this block for each new entry:

```
### 999.X — <short title>

- **Surfaced from:** <UAT file + section, e.g. "03-HUMAN-UAT.md § Test 2">
- **Observed:** <what the user saw>
- **Expected:** <what the user expected>
- **Why not a v1.0 blocker:** <one sentence>
- **Suggested fix:** <optional — high-level approach, not a design>
- **Target:** v1.1
```

## Entries

(No entries yet. First entries will be added during Phase 8's human UAT
walkthrough — see 08-HUMAN-UAT.md § Triage.)

---

_Created: 2026-04-13 as part of Phase 8 Plan 05._
_Owner: project maintainer._
_Next review: at v1.1 milestone kickoff._
