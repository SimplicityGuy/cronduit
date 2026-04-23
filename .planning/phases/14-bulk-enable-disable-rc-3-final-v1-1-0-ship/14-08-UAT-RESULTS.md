# Phase 14 UAT Results — `v1.1.0-rc.3` FAILED

**UAT run date:** 2026-04-22
**rc tag tested:** `v1.1.0-rc.3` (`e36dc26ea83518c39af8bec9aa5a26d87bb91922` — Phase 14 merge commit)
**GHCR digest tested:** `ghcr.io/simplicityguy/cronduit:1.1.0-rc.3` → amd64 `sha256:0a33f3b06fd0f7d1da7b215305e660cbce43fe24878b8d8c11aef7341799c4a5`
**Walked-through steps:** Pre-UAT checklist (PASS), Step 1 (PASS), Step 2 (**FAIL** — stopped per doc's own "If any checkbox in any step fails, STOP" rule)
**UAT outcome:** **FAILED** — rc.3 cannot promote to `v1.1.0`; `v1.1.0-rc.4` will be cut from `main` after fixes land.

> This document follows the pointer in `14-HUMAN-UAT.md` line 20: *"Document what you observed in `.planning/phases/14-bulk-enable-disable-rc-3-final-v1-1-0-ship/14-08-UAT-RESULTS.md`"*.

---

## Pre-UAT Checklist — PASS

| # | Item | Result |
|---|---|---|
| 1 | `v1.1.0-rc.3` tag on `origin` | ✅ `refs/tags/v1.1.0-rc.3` → `e36dc26` |
| 2 | rc.3 multi-arch on GHCR | ✅ amd64 + arm64 present |
| 3 | `:rc` digest == `:1.1.0-rc.3` digest | ✅ identical on both archs |
| 4 | `:latest` unchanged from v1.0.1 | ✅ `:latest` digest matches `:1.0.1` digest on both archs (D-10 gating held) |
| 5 | All 4 `just` recipes parse (`compose-up-rc3`, `reload`, `health`, `metrics-check`) | ✅ |
| 6 | `CRONDUIT_IMAGE` env override in `examples/docker-compose.yml` | ✅ line 72 |
| 7 | No existing cronduit container on UAT host | ✅ empty `docker ps` |
| 8 | ≥3 jobs in `examples/cronduit.toml` | ✅ 5 shipped jobs; a 6th `sleep-30-uat` command job was appended locally to satisfy Step 3's ~30s-runner requirement. **This addition is UAT scaffolding only — NOT part of the rc.4 PR.** |

Tag-cut procedure: unsigned annotated (Step 2b of `docs/release-rc.md`; GPG signing key not configured). `git-cliff --unreleased --tag v1.1.0-rc.3` preview reviewed and approved before tag push. `release.yml` completed green; all GHCR invariants verified post-push.

---

## Step 1 — Compose up pinned to rc.3 — PASS

`just compose-up-rc3` pulled `:1.1.0-rc.3`, container reported `(healthy)` within 90 s, `just health` printed `healthy`. Image digest matched the rc.3 manifest.

All three Step-1 validation boxes tick.

---

## Step 2 — Dashboard bulk-select chrome — FAIL (3 gaps)

### Gap 1 — Bulk `Disable selected`, `Enable selected`, and `Clear` all fail (BLOCKER)

**Observed:**
- Row selection works (checkboxes tick, visual state correct).
- Sticky action bar appears on first ticked row with correct `{N} selected` count.
- Clicking `Disable selected` → no visible effect; selected jobs remain enabled.
- Clicking `Clear` → no visible effect; row checkboxes stay checked, action bar stays up.
- `Enable selected` also believed affected (same wiring as `Disable`).

**Impact:** Core Phase 14 ERG-01 / ERG-02 feature is non-functional. UAT cannot proceed past Step 2.

**Severity:** BLOCKER.

**Root cause hypothesis:** Because `Clear` is pure inline-JS (`onclick="__cdBulkClearSelection()"` at `templates/pages/dashboard.html:65`) yet still fails, this is not merely a server-side CSRF or routing issue. Either the `__cdBulk*` helper functions are not being defined at page load, or the inline `onclick` / `hx-post` attributes aren't being bound. Empirical investigation with Playwright MCP (browser console + network tab) is the next step before patching — code inspection alone is inconclusive.

**Relevant files:**
- `templates/pages/dashboard.html:42-68` (action bar markup, HTMX attrs)
- `templates/pages/dashboard.html:143-191` (inline `<script>` defining `__cdBulk*` helpers)
- `templates/partials/job_table.html:4-11` (row checkbox with `onclick="__cdBulkOnRowChange()"` + `hx-preserve="true"`)
- `src/web/handlers/api.rs:517-620` (bulk_toggle handler; looks correct in isolation)
- `src/web/mod.rs:82` (route registration `/api/jobs/bulk-toggle`)

### Gap 2 — Timeline page self-polls its own page URL, nesting full HTML inside `#timeline-body` (MAJOR)

**Observed:**
- Initial page load renders correctly.
- Every 30 s the 30 s poll fires; a second copy of the top nav + "Timeline" heading + "24h/7d" pills appears BELOW the original nav and INSIDE the timeline area. After several minutes, layered copies of the "now" indicator stack vertically into a column of green running-bars (Image 7); the container bloats horizontally and vertically, producing unwanted scrolling (Image 8).
- The "green line across the row divider" seen earlier and the "empty row below each job" are lower-order symptoms of the same nesting.

**Impact:** Timeline page is unusable after the first poll. Regresses Phase 10/11 timeline behavior.

**Severity:** MAJOR.

**Root cause (confirmed by code reading):** `templates/pages/timeline.html:26-30` configures:
```html
<div id="timeline-body"
     hx-get="/timeline"           ← returns the FULL page (extends base.html)
     hx-trigger="every 30s"
     hx-swap="outerHTML"          ← replaces the div with the entire response
     hx-include="[name='window']">
  {% include "partials/timeline_body.html" %}
</div>
```
`/timeline` returns the full page HTML; `hx-swap="outerHTML"` replaces `#timeline-body` with the entire response. The next poll's response itself contains a `#timeline-body` div which ALSO polls `/timeline`, producing infinite nesting at 30-second intervals.

Compare the dashboard (`templates/pages/dashboard.html:132-138`) which correctly polls the dedicated partial route `/partials/job-table` with `hx-swap="innerHTML"` — the pattern already exists in the codebase; the timeline just doesn't follow it.

**Fix direction:** Add a `/partials/timeline` route returning only `timeline_body.html`; change the poll to `hx-get="/partials/timeline"` with `hx-swap="innerHTML"`. (Alternate: branch the existing `/timeline` handler on the `HX-Request` header and return the partial when set. Either works; the dedicated-route form matches the dashboard's existing pattern.)

### Gap 3 — `Select row` header text visible on dashboard (COSMETIC)

**Observed:** The dashboard job-table header renders the literal text `Select row` above the select-all checkbox in the first column (Image 3). The designed behavior is screen-reader-only: the checkbox already carries `aria-label="Select all visible jobs"` for accessibility.

**Impact:** Cosmetic drift from Phase 14 UI-SPEC. Would need to ship as-is to `v1.1.0` unless fixed.

**Severity:** COSMETIC.

**Root cause:** `templates/pages/dashboard.html:77` contains `<span class="sr-only">Select row</span>`. Either the `sr-only` Tailwind utility isn't present in the compiled CSS (content-glob miss in the Tailwind build), or the class is being overridden.

**Fix direction:** Remove the `<span>` outright. The checkbox's own `aria-label="Select all visible jobs"` (`dashboard.html:81`) already covers the a11y requirement, so the span is redundant. This avoids depending on whatever broke the `sr-only` utility in the first place, and is the minimal change.

---

## Remediation plan

1. Feature branch: `fix/phase-14-uat-rc4-blockers` (off `main` at `e36dc26`).
2. Atomic commits — one per gap — for clean history.
3. PR → `main`; CI must stay green on both SQLite + Postgres and on amd64 + arm64.
4. After merge, cut `v1.1.0-rc.4` off the merge commit per `docs/release-rc.md` (unsigned annotated path).
5. Re-run this entire HUMAN-UAT from Step 1 against rc.4 — per the doc's own "cut a `v1.1.0-rc.4` tag, and re-run this entire HUMAN-UAT against rc.4" directive.

---

*This file supersedes the validation state of `14-HUMAN-UAT.md` for rc.3. A new rc.4 UAT run will reuse the same `14-HUMAN-UAT.md` — this RESULTS file is the rc.3-specific historical record.*
