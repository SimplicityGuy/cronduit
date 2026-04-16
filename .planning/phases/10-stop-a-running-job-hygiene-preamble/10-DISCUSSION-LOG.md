# Phase 10: Stop-a-Running-Job + Hygiene Preamble - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-15
**Phase:** 10-stop-a-running-job-hygiene-preamble
**Areas discussed:** Map structure, `stopped` status color, Stop button placement, Stop feedback UX

---

## Gray Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| Map structure | Merge `active_runs` with `running_handles` into one `RunEntry`, or keep as two maps. Research flagged this as an open question for phase planning. | ✓ |
| `stopped` status color | Which design-system token the new `stopped` state uses: amber (disabled), red (error), new neutral token, or something else. Phase 13 depends on this. | ✓ |
| Stop button placement | Where Stop lives on the run-detail page and how it renders in the per-row run-history partial. | ✓ |
| Stop feedback UX | Toast wording, optimistic badge update vs server-driven refresh, race-case communication. | ✓ |

**User's choice:** All four areas selected for discussion.

---

## Area 1 — Map structure

**Question:** How should the scheduler hold per-run state for Stop?

| Option | Description | Selected |
|--------|-------------|----------|
| Keep separate | Add `running_handles: HashMap<i64, RunControl>` alongside existing `active_runs`. Minimal diff, SSE hot path unchanged, smaller blast radius in the highest-risk phase. | |
| Merge into `RunEntry` | Single `HashMap<i64, RunEntry { broadcast_tx, control }>`. Cleaner long-term but touches every `active_runs` call site and enlarges the Phase 10 diff. | ✓ |

**User's choice:** Merge into `RunEntry`.
**Notes:** User accepted the larger diff in exchange for a single authoritative per-run record, atomic insert/remove on run boundaries, and elimination of drift risk between two maps. Claude's recommendation was "keep separate" on blast-radius grounds; user overrode in favor of long-term cleanliness. Race tests T-V11-STOP-04..06 must cover the merged lifecycle.

---

## Area 2 — `stopped` status color

**Question:** Which color token for the new `stopped` status?

| Option | Description | Selected |
|--------|-------------|----------|
| New neutral `stopped` token | Add `--cd-status-stopped` (slate/gray) + bg pair + `.cd-badge--stopped`. "Operator interrupt" is its own category. Matches GHA-style. | ✓ |
| Reuse `disabled` (amber) | Treats operator-stop visually as "paused by human." No new token. Conflicts with Phase 14's bulk disable-toggle which also uses amber. | |
| Reuse `error` (red) | Maximum visual contrast from success. Misleading — `stopped` runs are excluded from failure metrics by design. | |

**User's choice:** New neutral `stopped` token (Claude's recommendation).
**Notes:** Planner to pick specific hex in a neutral slate/gray family that harmonizes with the terminal-green brand and clears the existing badge contrast ratio bar. Update `design/DESIGN_SYSTEM.md` Status Colors table in the same commit.

---

## Area 3 — Stop button placement & affordance

### Q3a: Where on the run-detail page?

| Option | Description | Selected |
|--------|-------------|----------|
| Top-right page action | Place it in the currently-empty right side of the `Run #N` header row. Reads as a deliberate page-level command. | ✓ |
| Inline next to status badge | Place it immediately next to the running + LIVE badges inside the metadata card. Tighter coupling; busier card header. | |

**User's choice:** Top-right page action (Claude's recommendation).

### Q3b: Row button style in the run-history partial?

| Option | Description | Selected |
|--------|-------------|----------|
| Text button: "Stop" | Compact text button. Scannable, accessible by default. | ✓ |
| Icon-only (with aria-label) | Minimal row width. Requires title + aria-label. Risks being missed. | |
| Icon + text | Most obvious affordance. Widest; tightens other row columns. | |

**User's choice:** Text button "Stop" (Claude's recommendation).

### Q3c: Button visual weight?

| Option | Description | Selected |
|--------|-------------|----------|
| Neutral outline | Plain outline button, text color inherits. Non-alarming before click; matches peer tools. | ✓ |
| Tinted with `--cd-status-stopped` | Pre-colors button with the new neutral token. | |
| Tinted with `--cd-status-error` (red) | Maximum urgency. Contradicts "stopped is not a failure" semantics. | |

**User's choice:** Neutral outline (Claude's recommendation).
**Notes:** Hover can tint toward `--cd-status-stopped` — do not escalate to red on hover.

---

## Area 4 — Stop feedback UX

### Q4a: Success toast wording?

| Option | Description | Selected |
|--------|-------------|----------|
| `"Stopped: <job name>"` | Symmetric with Run Now's `"Run queued: <job name>"`. Short, honest, name-anchored. | ✓ |
| `"Run #{id} stopped"` | Unambiguous run ID but drops the job name operators scan for. | |
| `"Stopping run #{id}..."` | Acknowledges the request but vague about outcome. | |

**User's choice:** `"Stopped: <job name>"` (Claude's recommendation).

### Q4b: Race-case messaging (Stop arrives after natural finish)?

| Option | Description | Selected |
|--------|-------------|----------|
| Silent refresh | No toast. Page reloads; operator sees the real natural terminal status. | ✓ |
| Info toast: "Run already completed" | Explicit; educates the operator. Slight noise for a rare condition. | |
| Warning toast: "Couldn't stop" | Treats the no-op as a failure. Misleading. | |

**User's choice:** Silent refresh (Claude's recommendation).
**Notes:** User explicitly adopted "silence is success" — do not later add an info toast "for completeness."

### Q4c: Optimistic badge swap?

| Option | Description | Selected |
|--------|-------------|----------|
| Wait for refresh (server-driven) | Same pattern as Run Now. Race-case safe with no rollback path needed. | ✓ |
| Optimistic swap | Snappier perceived feedback. Requires rollback path when race case fires. | |

**User's choice:** Wait for refresh (Claude's recommendation).

---

## Final confirmation

**Question:** Ready to write CONTEXT.md, or explore more gray areas?
**User's choice:** I'm ready for context.

---

## Claude's Discretion

- Specific hex values for `--cd-status-stopped` and `--cd-status-stopped-bg` — planner picks.
- Whether `Cargo.toml` and `rand` bumps are one plan or two (commit granularity).
- Exact position of the `SchedulerCmd::Stop` match arm in the `tokio::select!` loop.
- Whether the race-case detection happens in the web handler (DB read) or in the scheduler (oneshot reply).
- Icon glyph choice (if any) for the Stop button.
- Keyboard shortcut affordance — not requested; planner has discretion.

## Deferred Ideas

- SIGTERM-to-SIGKILL escalation + `stop_grace_period` — v1.2.
- Authentication gating on Stop — v2 auth phase.
- Webhook/chain notification on stop — v1.2 webhooks will include `stopped` as a transition.
- Stop-all / bulk-stop from the dashboard — not requested; would layer onto Phase 14 if needed.
- Optimistic UI with rollback — rejected in D-08; revisit as a cross-app HTMX pattern change if ever needed.
