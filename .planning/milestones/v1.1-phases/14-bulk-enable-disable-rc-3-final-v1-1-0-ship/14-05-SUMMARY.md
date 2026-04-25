---
phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship
plan: 05
subsystem: ui
tags: [askama, htmx, tailwind, css, dashboard, bulk-toggle, accessibility]

# Dependency graph
requires:
  - phase: 14
    provides: "Plan 04 wave-2 POST /api/jobs/bulk-toggle handler (consumes name=\"job_ids\" repeated keys via axum-extra Form)"
  - phase: 14
    provides: "Plan 02 wave-1 DashboardJobView shape (untouched here; v1.1 dashboard does NOT surface override state per UI-SPEC Open Q 1)"
provides:
  - "Dashboard bulk-select chrome: leftmost checkbox column with stable per-row id surviving the 3s tbody poll"
  - "Sticky `.cd-bulk-bar` placed as a SIBLING of the `.overflow-x-auto` table wrapper (position: sticky requires non-clipping ancestor)"
  - "Inline JS helpers (~30 LOC) for indeterminate state, bar visibility, select-all, clear, and afterSwap re-sync"
  - "Six new CSS selectors purely additive to `@layer components`; zero new design tokens; zero existing selectors modified"
affects: [14-06-settings-page, 14-07-htmx-integration, 14-09-uat]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "axum-extra Form repeated keys (name=\"job_ids\" emits job_ids=1&job_ids=2)"
    - "hx-preserve=\"true\" + stable id=cd-row-cb-{job.id} survives tbody innerHTML swap"
    - "Sticky bar as SIBLING (not child) of horizontally-scrolling wrapper"
    - "axum-htmx hx-include selector harvesting checked checkboxes + hidden CSRF input"

key-files:
  created: []
  modified:
    - templates/partials/job_table.html
    - templates/pages/dashboard.html
    - assets/src/app.css

key-decisions:
  - "v1.1 does NOT surface enabled_override on the dashboard (UI-SPEC Open Q 1); DashboardJobView remains untouched in this plan"
  - "Bulk bar uses individual button hx-post + hx-vals (not a wrapping <form>) — matches axum-htmx idiom and CONTEXT D-02"
  - "name=\"job_ids\" without bracket notation; serde_html_form / axum_extra::Form parses repeated keys into Vec<i64>"

patterns-established:
  - "Bulk-select pattern: per-row checkbox in partial + sticky action bar in parent page + hx-include selector harvesting"
  - "Inline JS helper namespace: __cdBulkOnRowChange / __cdBulkUpdateBar / __cdBulkUpdateIndeterminate / __cdBulkSelectAll / __cdBulkClearSelection"
  - "afterSwap event listener gated on e.detail.target.id === 'job-table-body' for poll-resilient state sync"

requirements-completed: [ERG-01, ERG-02]

# Metrics
duration: 5m 37s
completed: 2026-04-22
---

# Phase 14 Plan 05: Dashboard Bulk-Select UI Summary

**Sticky bulk-action bar + leftmost checkbox column on the jobs dashboard, with hx-preserve guarding selection state across the 3s HTMX poll and inline JS wiring indeterminate state.**

## Performance

- **Duration:** 5m 37s (337s)
- **Started:** 2026-04-22T22:29:08Z
- **Completed:** 2026-04-22T22:34:45Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- Per-row select checkbox added to `templates/partials/job_table.html` with stable `id="cd-row-cb-{{ job.id }}"` + `hx-preserve="true"` (Landmine §3 mitigation: poll wipes selection without these)
- Select-all `<th>` + sticky `.cd-bulk-bar` rendered into `templates/pages/dashboard.html` as a SIBLING of `.overflow-x-auto` (Landmine §`Sticky`: sticky requires non-clipping ancestor; verified bar at line 42, wrapper at line 71)
- Three action buttons (`Disable selected`, `Enable selected`, `Clear`) wired via `hx-post="/api/jobs/bulk-toggle"` + `hx-include=".cd-row-checkbox:checked, [name='csrf_token']"` to consume Plan 04's wave-2 handler
- Inline JS (~50 LOC, vanilla DOM): 5 helper functions + `htmx:afterSwap` listener gated on the polled `<tbody>` id, re-syncs indeterminate state and bar visibility every 3s
- Six new CSS selectors appended to `@layer components` in `assets/src/app.css`, all reusing existing `--cd-*` tokens; zero new tokens, zero existing selectors touched

## Task Commits

Each task was committed atomically with `--no-verify` (parallel-execution fast path):

1. **Task 1: Per-row select checkbox in job_table.html** — `1330a9f` (feat)
2. **Task 2: Select-all `<th>`, sticky bar, inline JS in dashboard.html** — `ce33a7a` (feat)
3. **Task 3: Six additive CSS selectors in app.css** — `77c1172` (feat)

**Plan metadata commit:** _to be added when committing this SUMMARY.md_

## Files Modified

- `templates/partials/job_table.html` (+10 lines) — leading `<td>` with `.cd-row-checkbox` (id, name, value, aria-label, hx-preserve, onclick)
- `templates/pages/dashboard.html` (+91 lines) — hidden CSRF input, sticky `.cd-bulk-bar` with three buttons, select-all `<th>`, inline `<script>` block with 5 helpers + afterSwap listener
- `assets/src/app.css` (+55 lines) — six selectors (`.cd-row-checkbox`, `.cd-row-checkbox:focus-visible`, `.cd-bulk-bar`, `.cd-bulk-bar[hidden]`, `.cd-bulk-bar-count`, `.cd-bulk-bar-count strong`, `.cd-btn-secondary.cd-btn-disable-hint:hover/:active`, `.cd-badge--forced`)

## HTMX Interaction Flow

```mermaid
flowchart TD
    Start[Operator clicks row checkbox] --> RowChange[onclick=__cdBulkOnRowChange]
    RowChange --> UpdateBar[__cdBulkUpdateBar:<br/>count checked rows<br/>show/hide bar]
    RowChange --> UpdateInd[__cdBulkUpdateIndeterminate:<br/>set select-all to<br/>0 / mixed / all]
    UpdateBar --> BarVisible{N > 0?}
    BarVisible -- yes --> ShowBar[bar.removeAttribute hidden]
    BarVisible -- no --> HideBar[bar.setAttribute hidden]
    ShowBar --> ClickAction[Operator clicks<br/>Disable / Enable selected]
    ClickAction --> HxInclude[hx-include harvests<br/>.cd-row-checkbox:checked<br/>+ csrf_token]
    HxInclude --> Post["POST /api/jobs/bulk-toggle<br/>job_ids=1&amp;job_ids=2&amp;...&amp;action=disable&amp;csrf_token=..."]
    Post --> Handler[Plan 04 handler:<br/>axum-extra Form parses<br/>repeated job_ids -> Vec&lt;i64&gt;]
    Handler --> Trigger[200 + HX-Trigger:<br/>cdBulkToggleResult]
    Trigger --> AfterReq[hx-on::after-request<br/>__cdBulkUpdateBar]
    Poll[Every 3s:<br/>tbody hx-get/swap=innerHTML] --> AfterSwap[htmx:afterSwap event]
    AfterSwap --> GateId{e.detail.target.id ==<br/>'job-table-body'?}
    GateId -- yes --> ReSync[__cdBulkUpdateBar +<br/>__cdBulkUpdateIndeterminate]
    GateId -- no --> Ignore[ignore]
    Poll -.->|hx-preserve=true on each row| Preserved[row checkbox state<br/>preserved across swap]
```

## Sibling Assertion (load-bearing — Landmine `Sticky`)

`.cd-bulk-bar` MUST appear before — and at the same depth as — `.overflow-x-auto`. Confirmed:

```
$ grep -nE 'cd-bulk-bar|overflow-x-auto' templates/pages/dashboard.html | head -5
42:<div id="cd-bulk-action-bar" class="cd-bulk-bar" hidden>
43:  <span class="cd-bulk-bar-count"><strong id="cd-bulk-count">0</strong> selected</span>
71:<div class="overflow-x-auto">
```

Bar opens at line 42, closes at line 68; overflow wrapper opens at line 71. Sibling, not descendant. Sticky positioning will not be horizontally clipped on narrow viewports.

## CSS Diff Summary

| Metric                          | Value |
| ------------------------------- | ----- |
| New selectors added             | 9 (8 distinct rule blocks + `.cd-badge--forced`) |
| Existing selectors modified     | 0     |
| New design tokens introduced    | 0     |
| Existing tokens reused          | 15 (all from `:root` / `[data-theme=light]` blocks at L24-92) |
| Lines added                     | 55    |
| Lines removed (excluding diff header) | 0 |

Tokens used (all pre-existing): `--cd-bg-surface-raised`, `--cd-border`, `--cd-border-focus`, `--cd-radius-md`, `--cd-space-2`, `--cd-space-3`, `--cd-space-4`, `--cd-status-disabled`, `--cd-status-disabled-bg`, `--cd-status-running`, `--cd-status-running-bg`, `--cd-text-accent`, `--cd-text-base`, `--cd-text-primary`, `--cd-text-secondary`.

Per UI-SPEC Live-CSS Reality Check: zero token additions, zero existing tokens redefined.

## Threat Mitigations Applied

| Threat ID | Mitigation Status |
| --------- | ----------------- |
| T-14-05-01 (hx-preserve silent fail without stable id) | DONE — `id="cd-row-cb-{{ job.id }}"` on every per-row checkbox |
| T-14-05-02 (sticky bar clipped inside overflow wrapper) | DONE — bar inserted at line 42, wrapper at line 71; sibling structure |
| T-14-05-03 (CSRF missing from bulk POST) | DONE — hidden `<input name="csrf_token">` outside polled tbody; bar buttons use `hx-include=".cd-row-checkbox:checked, [name='csrf_token']"` |
| T-14-05-04 (info disclosure via overrides) | ACCEPT — v1.1 UI is unauth by design; no new exposure |
| T-14-05-05 (XSS via job name in aria-label) | DONE — `{{ job.name }}` is HTML-escaped by askama default; no `\| safe` filter used |

## Decisions Made

- **Plan-prescribed inline-JS signature for select-all is `__cdBulkSelectAll(on)`** (boolean), wired as `onclick="__cdBulkSelectAll(this.checked)"` per Plan Task 2 verbatim markup. UI-SPEC `<th>` snippet at L249 used `__cdBulkSelectAll(this)` (the older "pass element" signature); the plan supersedes the UI-SPEC here, and this implementation follows the plan. Both signatures are functionally equivalent; adopting the plan's choice for consistency with the rest of the helpers using primitive args.
- **CSS appended at the end of `@layer components`, immediately before its closing brace at L493.** The plan said "after `cd-btn-stop--compact` ends ~L284 inside the same `@layer components`" — interpreted as "anywhere inside that layer, additive". Appending at the bottom keeps the layer's existing ordering intact (sparkline, pill, timeline blocks remain contiguous).
- **`hx-on::after-request` syntax used verbatim** per the plan; this is the HTMX 2.0+ namespaced-event listener syntax (double-colon), correct for the project's vendored HTMX.

## Deviations from Plan

None — plan executed exactly as written. All three tasks landed verbatim per their `<action>` blocks. Acceptance criteria for all three tasks all passed on first attempt. `cargo build --quiet` and `cargo clippy --quiet -- -D warnings` both green after the final commit.

## Issues Encountered

None. The PreToolUse "READ-BEFORE-EDIT REMINDER" hooks fired three times (once per file) but were cosmetic — each file had been read earlier in the session, the edits had already succeeded by the time the reminder appeared, and verification commands confirmed the writes landed correctly.

## Self-Check

Verifying all claims before returning to orchestrator:

```
$ [ -f templates/partials/job_table.html ] && echo "FOUND"
FOUND
$ [ -f templates/pages/dashboard.html ] && echo "FOUND"
FOUND
$ [ -f assets/src/app.css ] && echo "FOUND"
FOUND
$ [ -f .planning/phases/14-bulk-enable-disable-rc-3-final-v1-1-0-ship/14-05-SUMMARY.md ] && echo "FOUND"
FOUND  (this very file)
$ git log --oneline | grep -E '1330a9f|ce33a7a|77c1172' | wc -l
3
```

Three task commits land in `git log`. Three modified files exist on disk. Build green, clippy green.

## Self-Check: PASSED

## Screenshot / Visual Verification

Deferred to **Plan 09 HUMAN-UAT**. Manual smoke test plan (per the plan's `<verification>` block):
- `just dev` → visit dashboard → check at least one row → verify bulk bar appears
- Scroll the dashboard → verify bulk bar sticks to viewport top (not clipped horizontally)
- HTMX 3s poll → verify checked state survives (`hx-preserve` working)
- Select-all → verify indeterminate state updates correctly when partial selection

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Wave 4 (Plan 06 settings page):** can reuse `.cd-badge--forced` and the `cd-btn-secondary` action pattern landed here
- **Wave 4+ (Plan 07 HTMX integration toast):** the bar's `hx-on::after-request="__cdBulkUpdateBar()"` is intentionally minimal — the toast wiring will extend the bar's `hx-on` clauses to also dispatch the trigger event from the response header
- **Wave 5+ (Plan 09 HUMAN-UAT):** four discrete UAT items above; all are visual and require operator confirmation per `feedback_uat_user_validates` memory

---
*Phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship*
*Plan: 05*
*Completed: 2026-04-22*
