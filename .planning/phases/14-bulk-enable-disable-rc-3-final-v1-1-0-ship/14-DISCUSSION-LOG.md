# Phase 14: Bulk Enable/Disable + rc.3 + Final v1.1.0 Ship - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-21
**Phase:** 14-bulk-enable-disable-rc-3-final-v1-1-0-ship
**Areas discussed:** Bulk-select UX, Override semantics + symmetry, Settings audit surface + API shape, Final v1.1.0 ship mechanics

---

## Gray Area Selection

| Option                                    | Description                                                                                                                                       | Selected |
|-------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------|----------|
| Bulk-select UX                            | Checkbox placement/shape, action-bar position, select-all behavior, toast copy                                                                    | ✓        |
| Override semantics + symmetry             | Enable=clear vs force; mixed-state handling; button visibility; config-disabled bulk-enable behavior                                              | ✓        |
| Settings audit surface + API shape        | "Currently Overridden" section design + bulk-toggle API contract                                                                                  | ✓        |
| Final v1.1.0 ship mechanics               | rc.3 → v1.1.0 promotion, UAT definition, :latest advancement, release-notes strategy                                                              | ✓        |

**User's choice:** All 4 areas selected for discussion.

---

## Bulk-Select UX

### Q1: Checkbox column placement

| Option                                 | Description                                                                                                                                          | Selected |
|----------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------|----------|
| New leftmost column (Recommended)      | Insert `<th><input type='checkbox'></th>` as first column, before Name. Industry-standard table-selection pattern (Gmail, GitHub, Linear).           | ✓        |
| Inside Name cell                       | Checkbox inline inside Name `<td>`. Saves a column but mixes selection with navigation — misclick risk.                                              |          |
| Rightmost column (before Actions)      | Unusual position; most users scan left-to-right and expect selection at the start.                                                                   |          |

### Q2: Action-bar placement

| Option                                            | Description                                                                                                                                   | Selected |
|---------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------|----------|
| Inline above table (sticky on scroll) (Recommended) | New div between filter bar and table; shows 'N selected' when N>0. Sticks to top on scroll so action stays reachable.                       | ✓        |
| Floating bottom bar on selection                  | Fixed-position bar slides up from viewport bottom. Modern (Linear, Notion) but adds CSS complexity and fights small-viewport scroll UX.       |          |
| Inline above table (non-sticky)                   | Same as option 1 but scrolls away. Lower cost; operators may need to scroll back up after selecting jobs far down.                            |          |

### Q3: Select-all behavior

| Option                                                | Description                                                                                                                                    | Selected |
|-------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------|----------|
| Header checkbox — visible-filtered rows (Recommended) | Toggles selection for all jobs currently rendered. Clear mental model; no hidden selections across paginated state.                            | ✓        |
| No select-all                                         | Row checkboxes only. Safer but tedious for fleet-wide maintenance.                                                                             |          |
| Header + shift-click range on rows                    | Power-user ergonomics, but requires inline JS — codebase has zero custom JS beyond vendored HTMX.                                              |          |

### Q4: Toast copy

| Option                                                                                           | Description                                                                                                     | Selected |
|--------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------|----------|
| Verbose: '3 jobs disabled. 2 currently-running jobs will complete naturally.' (Recommended)      | Literal REQ wording from ERG-02. Conditional second sentence — omitted when running_count=0.                    | ✓        |
| Terse: '3 jobs disabled'                                                                         | Shorter but hides the running-jobs nuance; operators may think the toast is buggy.                              |          |
| Verbose with per-job list: '3 jobs disabled: backup-db, logs-rotate, metrics-ping'               | Maximum transparency. Wraps awkwardly for larger selections; becomes modal.                                     |          |

---

## Override Semantics + Symmetry

### Q1: "Enable selected" action

| Option                                             | Description                                                                                                                               | Selected |
|----------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------|----------|
| Clear override — set to NULL (Recommended)         | 'Enable' means 'stop overriding; let config decide.' Simplest mental model; symmetric with disable_missing_jobs.                           | ✓        |
| Force enable — set to 1                            | Gives bulk-enable teeth (always-on). Creates sticky override=1 rows; operator loses visibility on config-driven vs forced-on.             |          |
| Two buttons: Clear override + Force enable         | Max power, max confusion. Homelab operators probably won't use Force enable; extra button is cognitive cost without payoff.               |          |

### Q2: Bulk-enable on config-disabled job

| Option                                                                         | Description                                                                                                                        | Selected |
|--------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------|----------|
| Silent no-op; toast shows 'N jobs: override cleared' (Recommended)             | Config is source-of-truth; clearing override lets config win. Toast reports what was done, not end state.                           | ✓        |
| Force enable that specific job (override=1)                                    | Explicit operator intent. But quietly contradicts config — surprises the next operator reading the config file.                     |          |
| Reject with error toast                                                        | Feels too formal for a homelab tool; operators can check config themselves.                                                        |          |

### Q3: Mixed-state selection

| Option                                                                                             | Description                                                                                                | Selected |
|----------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------|----------|
| Idempotent: all 5 get override=0; toast says '5 jobs disabled' (Recommended)                       | Simplest semantics. Matches Run Now/Stop/reroll (don't gate on current state).                             | ✓        |
| Transition-diff: '2 newly disabled, 3 already disabled'                                            | Accurate reporting but extra query cost + clutter.                                                         |          |
| Reject if any selected is already overridden-disabled                                              | Forces operator precision. Feels adversarial for a bulk-action UX.                                         |          |

### Q4: Button visibility when no overrides

| Option                                                                            | Description                                                                                                                | Selected |
|-----------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------|----------|
| Always show both buttons when N>0 (Recommended)                                   | Clicking Enable on non-overridden jobs is idempotent no-op. Simpler template; no conditional logic.                         | ✓        |
| Conditional: hide 'Enable' when no selected is overridden                         | Button appears only when needed. Requires server-side check of every selected row's override state — complex.              |          |
| Only 'Disable selected'; single-job enable is per-row-only                        | Bulk is one-way. Still covers main use case (bulk-disable test jobs during maintenance).                                   |          |

---

## Settings Audit Surface + API Shape

### Q1: "Currently Overridden" placement

| Option                                                                     | Description                                                                                                   | Selected |
|----------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------|----------|
| New full-width section below the 6-card grid (Recommended)                 | Full-width table accommodates columns; doesn't disturb card grid. Familiar 'cards above, detail below' shape. | ✓        |
| New card inside the grid                                                   | Too cramped for a job list; would force truncation + 'see all' link.                                          |          |
| Separate /settings/overrides sub-page                                      | Keeps settings clean but adds click cost. REQ lock ERG-03 wants discoverability.                              |          |

### Q2: Audit columns + actions

| Option                                                                                   | Description                                                                                                            | Selected |
|------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------|----------|
| Name + override state + 'Clear' inline button (Recommended)                              | Three columns, per-row Clear button. One-click un-stick addresses 'operator forgets for months' failure mode.          | ✓        |
| Name + override state only                                                               | Read-only audit; operator must go to dashboard and use bulk action. Worse ergonomics.                                  |          |
| Name + state + 'since' timestamp + Clear                                                 | Richer audit but adds `enabled_override_set_at` column + migration complexity.                                         |          |

### Q3: API request body shape

| Option                                                                                                        | Description                                                                                                           | Selected |
|---------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------|----------|
| Form-urlencoded: csrf_token + action + repeated job_ids (Recommended)                                          | Matches every other mutation handler (run_now, stop_run, reroll, reload). HTMX sends natively.                        | ✓        |
| JSON body                                                                                                     | Structured but diverges from existing pattern; needs hx-ext='json-enc' or custom JS.                                  |          |
| Separate endpoints per action                                                                                 | `POST /api/jobs/bulk-disable` + `POST /api/jobs/bulk-enable`. Duplicates CSRF/handler skeleton for no semantic gain.   |          |

### Q4: Partial failure handling

| Option                                                                                                           | Description                                                                                                   | Selected |
|------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------|----------|
| Apply to valid IDs; return 200 with toast '2 jobs disabled (1 not found)' (Recommended)                          | Best-effort. Stale dashboard rows (race) don't break the action. Toast surfaces mismatch.                      | ✓        |
| All-or-nothing 400 on any invalid                                                                                | Strict validation. In practice a race scenario produces frustrating failure. REQ-01 doesn't mandate strictness. |          |
| Apply to valid; 207 Multi-Status JSON per-id                                                                     | Max fidelity. Overkill; needs new error-rendering UI.                                                          |          |

---

## Final v1.1.0 Ship Mechanics

### Q1: Tag promotion mechanic

| Option                                                                                   | Description                                                                                                       | Selected |
|------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|----------|
| Retag the rc.3 SHA as v1.1.0 (same digest) (Recommended)                                 | Image shipped as v1.1.0 is byte-identical to UAT-validated rc.3. Guarantees 'what was tested is what ships.'      | ✓        |
| New commit on main bumping MILESTONES.md/README, then tag v1.1.0                         | Breaks bit-identical-image guarantee; UAT validates rc.3, v1.1.0 is a new build.                                  |          |
| workflow_dispatch tag cut                                                                | Phase 12 D-13 explicitly rejected (trust-anchor stance). Rejected again for consistency.                          |          |

### Q2: UAT definition

| Option                                                                                       | Description                                                                                                   | Selected |
|----------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------|----------|
| Written HUMAN-UAT.md checklist with 5–8 concrete steps using `just` recipes (Recommended)    | Same pattern as Phase 8 + Phase 12. Honors feedback_uat_user_validates + feedback_uat_use_just_commands.       | ✓        |
| Multi-day soak (72h) against operator's fleet                                                | Highest confidence but freezes milestone for a week for low incremental benefit in a homelab tool.             |          |
| Automated-only: compose-smoke + integration tests green                                      | Breaks feedback_uat_user_validates memory ('UAT requires user validation'). Rejected.                          |          |

### Q3: :latest advancement

| Option                                                         | Description                                                                                                   | Selected |
|----------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------|----------|
| Implicit via release.yml D-10 gating (Recommended)             | Pushing non-rc tag auto-advances :latest. Close-out runs verify-latest-retag.sh for post-push confirmation.   | ✓        |
| Explicit maintainer retag via docker buildx imagetools create  | Matches Phase 12.1 pattern, but Phase 12.1 was fixing pre-existing divergence — redundant here.               |          |
| Advance :latest in separate hotfix after UAT                   | Decouples UAT from release; adds complexity; violates 'tag = ship' principle.                                 |          |

### Q4: Release notes structure

| Option                                                                                  | Description                                                                                                             | Selected |
|-----------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------|----------|
| rc.3 = Phase 14 delta only; v1.1.0 = cumulative v1.0.1→v1.1.0 (Recommended)             | Two audiences: rc.3 readers watching milestone land; v1.1.0 readers adopting stable. Both via `git-cliff`.              | ✓        |
| rc.3 + v1.1.0 identical (both cumulative)                                               | Simpler but rc.3 notes become massive; rc.3 should highlight what's NEW since rc.2.                                     |          |
| Hand-edit v1.1.0 with human 'what's new' intro + auto-generated diff                    | Best UX for adopters but violates Phase 12 D-12 (git-cliff authoritative; no hand-editing).                             |          |

---

## Wrap-up Q&A

### Q: Ready for CONTEXT.md, or explore more gray areas?

| Option                           | Description                                                                                                                         | Selected |
|----------------------------------|-------------------------------------------------------------------------------------------------------------------------------------|----------|
| I'm ready for context            | 16 decisions captured across 4 areas. Proceed.                                                                                       | ✓        |
| Explore more gray areas          | Migration shape, HTMX poll + selection state, THREAT_MODEL.md wording, Cargo.toml version bump — all candidates but lower-value.     |          |

---

## Claude's Discretion

Items the user delegated to Claude/planner judgement:

- Selection state preservation across the dashboard's 3s HTMX poll (recommendation: `hx-preserve="true"` on row checkboxes).
- Exact CSS tokens for the sticky action bar (`position: sticky; top: var(--cd-space-2); z-index: 10; background: var(--cd-bg-surface-raised); ...`).
- Whether to wrap the action bar in a single `<form>` or use per-button `hx-vals` + `hx-include`.
- Indeterminate-state UI for the header checkbox (CSS-only or tiny inline script).
- FORCED ON badge color token (reuse `--cd-status-running` or add new `--cd-status-forced`).
- Order of action-bar buttons (`[Disable] [Enable] [Clear]` vs other arrangement).
- Toast wording when `job_ids` is empty (defensive edge).
- Whether bulk_toggle needs a dedicated Prometheus counter (probably not).
- `cliff.toml` section grouping for ERG-* commits in cumulative v1.1.0 notes.
- Single-migration-per-backend shape (NO 3-step dance — column is nullable and needs no backfill).
- `THREAT_MODEL.md` exact wording (one-line bullet consistent with Phase 10's Stop note).
- `MILESTONES.md` v1.1 archive entry prose (follows existing v1.0 shape).
- `README.md` "Current State" paragraph wording update.

## Deferred Ideas

Captured in CONTEXT.md `<deferred>` section. Highlights: force-enable-via-UI (reserved schema value; not exposed in v1.1), shift-click range selection (no custom JS in v1.1), per-row override timestamp (v1.2), transition-diff toast reporting, 207 Multi-Status response shape.
