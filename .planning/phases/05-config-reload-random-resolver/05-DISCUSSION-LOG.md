# Phase 5: Config Reload & `@random` Resolver - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-11
**Phase:** 05-config-reload-random-resolver
**Areas discussed:** Reload feedback UX, @random re-roll cadence, @random UI presentation, Reload concurrency

---

## Reload feedback UX

| Option | Description | Selected |
|--------|-------------|----------|
| Toast + settings update | HTMX toast notification with diff summary + Settings page shows last reload timestamp. Toast auto-dismisses after 5s. | ✓ |
| Settings page only | No transient notification — operator checks Settings page for reload history. | |
| Persistent status banner | Banner at top of all pages showing last reload status. | |

**User's choice:** Toast + settings update
**Notes:** None

---

| Option | Description | Selected |
|--------|-------------|----------|
| Error toast + log | Red toast with parse error summary, persists until dismissed. Full error in structured log. Settings page shows last failed attempt. | ✓ |
| Error toast only | Red toast with error summary, auto-dismiss after 10s. Details only in logs. | |
| Non-dismissing banner | Persistent red banner at top of all pages until next successful reload. | |

**User's choice:** Error toast + log
**Notes:** None

---

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, full diff | Response includes status, added, updated, disabled, unchanged counts. | ✓ |
| Minimal ack | Response is just status + optional error message. | |

**User's choice:** Yes, full diff
**Notes:** None

---

## @random re-roll cadence

| Option | Description | Selected |
|--------|-------------|----------|
| Daily at midnight in operator TZ | Background task fires at midnight, re-resolves all @random jobs. Fresh random spread each day. | |
| Only at sync time | Re-roll only on startup or config reload. Once resolved, stays fixed until next restart/reload. | ✓ |
| Configurable interval | Add [server].random_reroll_interval for operator control. | |

**User's choice:** Only at sync time
**Notes:** None

---

| Option | Description | Selected |
|--------|-------------|----------|
| Preserve | If config_hash unchanged, keep existing resolved_schedule from DB. | ✓ |
| Always re-roll on reload | Every reload generates fresh random values even for unchanged schedules. | |

**User's choice:** Preserve
**Notes:** None

---

| Option | Description | Selected |
|--------|-------------|----------|
| API endpoint | POST /api/jobs/{id}/reroll — clears and re-resolves. | |
| Not in v1 | Re-randomization only on schedule field change. | |
| UI button + API | Both API endpoint and "Re-roll" button on Job Detail page. | ✓ |

**User's choice:** UI button + API
**Notes:** None

---

## @random UI presentation

| Option | Description | Selected |
|--------|-------------|----------|
| Inline label | "Schedule: @random 14 * * * (resolved to 14 17 * * *)" — raw and resolved on same line. | ✓ |
| Two separate rows | Raw schedule and resolved schedule on separate lines. | |
| Tooltip on hover | Show resolved only, raw in tooltip on hover. | |

**User's choice:** Inline label
**Notes:** None

---

| Option | Description | Selected |
|--------|-------------|----------|
| Badge pill | Small "@random" badge/pill next to schedule column in terminal-green accent. | ✓ |
| Icon indicator | Dice or shuffle icon next to schedule. | |
| No distinction | Dashboard shows only resolved schedule, detail page reveals @random. | |

**User's choice:** Badge pill
**Notes:** None

---

## Reload concurrency

| Option | Description | Selected |
|--------|-------------|----------|
| Coalesce via debounce | All triggers through same channel. 500ms debounce for file watcher. SIGHUP/API immediate but coalesce if reload in-flight. | ✓ |
| Serialize with lock | Mutex around reload logic. Additional triggers queue sequentially. | |
| Reject if busy | Return 409 Conflict if reload already in progress. | |

**User's choice:** Coalesce via debounce
**Notes:** None

---

| Option | Description | Selected |
|--------|-------------|----------|
| Enabled by default | File watcher on unless [server].watch_config = false. Logs startup message. | ✓ |
| Opt-in | [server].watch_config = true required to enable. | |

**User's choice:** Enabled by default
**Notes:** None

---

## Claude's Discretion

- Debounce implementation details (tokio::time::sleep vs notify built-in)
- Internal locking mechanism for reload serialization
- Slot algorithm implementation for random_min_gap enforcement
- Re-roll button placement and styling
- Toast animation and positioning

## Deferred Ideas

None — discussion stayed within phase scope.
