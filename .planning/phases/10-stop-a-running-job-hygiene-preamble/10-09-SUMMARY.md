---
phase: 10-stop-a-running-job-hygiene-preamble
plan: 09
subsystem: web-ui
tags: [templates, htmx, csrf, stop-button, SCHED-09, SCHED-14]
requires:
  - "10-07"
  - "10-08"
provides:
  - "Stop button on run_detail page header (Surface A)"
  - "Per-row compact Stop button in run_history partial (Surface B)"
  - "csrf_token threaded through RunDetailPage and RunHistoryPartial template contexts"
affects:
  - templates/pages/run_detail.html
  - templates/partials/run_history.html
  - src/web/handlers/run_detail.rs
  - src/web/handlers/job_detail.rs
tech-stack:
  added: []
  patterns:
    - "HTMX hx-post + hx-swap=none + hx-disabled-elt=this for Stop form submission (UI-SPEC HTMX Interaction Contract)"
    - "Hidden csrf_token form field paired with cronduit_csrf cookie (double-submit CSRF, validated by stop_run handler from plan 10-07)"
    - "Per-row Actions column with empty th label + width:1% shrink-to-content (Surface B canonical layout)"
    - "Gate Stop button rendering behind is_running / run.status == running so terminal rows render empty cells (no button churn)"
    - "csrf_token threaded through every 2s poll of /partials/jobs/{job_id}/runs so browsers always hold a fresh token paired with the cookie (CookieJar extracted in job_runs_partial handler)"
key-files:
  created: []
  modified:
    - templates/pages/run_detail.html
    - templates/partials/run_history.html
    - src/web/handlers/run_detail.rs
    - src/web/handlers/job_detail.rs
decisions:
  - "CookieJar added to run_detail handler signature so csrf::get_token_from_cookies can populate RunDetailPage.csrf_token on the full-page render path (HTMX log-viewer partial path does not render the Stop form, so no token needed there)"
  - "CookieJar added to job_runs_partial handler so every 2s poll re-emits csrf_token into the RunHistoryPartial context — tokens are never cached client-side"
  - "RunHistoryPartial.csrf_token not required when {% include %} is used from JobDetailPage (askama include inherits parent scope which already has csrf_token), but adding it to the struct is harmless and future-proofs standalone partial rendering"
  - "No confirmation dialog, no keyboard shortcut, no icon glyph — single-word 'Stop' text label per UI-SPEC Copywriting Contract"
metrics:
  duration: "~20 minutes"
  completed: 2026-04-15
---

# Phase 10 Plan 09: Stop Button Template Wiring Summary

## One-liner

Two Stop button surfaces (run-detail header + run-history per-row) now POST to `/api/runs/{run_id}/stop` with CSRF-protected HTMX forms, gated by `is_running` / `run.status == "running"`, styled with the `cd-btn-stop` classes from plan 10-08, pending a human visual checkpoint.

## What changed

### Task 1 — templates/pages/run_detail.html header action slot

Added an `is_running`-gated `<form>` inside the existing `flex items-center justify-between` header div, positioned to the right of the `Run #N` h1:

```html
{% if is_running %}
<form hx-post="/api/runs/{{ run.id }}/stop"
      hx-swap="none"
      hx-disabled-elt="this"
      style="display:inline">
  <input type="hidden" name="csrf_token" value="{{ csrf_token }}">
  <button type="submit" class="cd-btn-stop" aria-label="Stop run #{{ run.id }}">Stop</button>
</form>
{% endif %}
```

Handler change (`src/web/handlers/run_detail.rs`):
- Added `use crate::web::csrf;`
- Added `csrf_token: String` field to `RunDetailPage` struct
- Added `cookies: axum_extra::extract::CookieJar` to `run_detail` handler signature
- Populated `csrf_token` via `csrf::get_token_from_cookies(&cookies)` on the full-page render path only (HTMX log-viewer partial does not render the Stop form)

Commit: `5de0a2d feat(10-09): add Stop button to run_detail header action slot`

### Task 2 — templates/partials/run_history.html 6th column

Appended an empty-label header th (`width:1%` shrink-to-content) and a per-row td that renders the compact Stop form when `run.status == "running"`, otherwise renders an empty cell:

```html
<th class="text-right py-2 px-4" style="...;width:1%"></th>
...
<td class="py-2 px-4" style="text-align:right">
  {% if run.status == "running" %}
  <form hx-post="/api/runs/{{ run.id }}/stop" hx-swap="none" hx-disabled-elt="this" style="display:inline">
    <input type="hidden" name="csrf_token" value="{{ csrf_token }}">
    <button type="submit" class="cd-btn-stop cd-btn-stop--compact" aria-label="Stop run #{{ run.id }}">Stop</button>
  </form>
  {% endif %}
</td>
```

Handler changes (`src/web/handlers/job_detail.rs`):
- Added `csrf_token: String` field to `RunHistoryPartial` struct
- `job_detail` HTMX partial render path now pulls token from existing `cookies` param
- `job_runs_partial` handler signature grew a `cookies: axum_extra::extract::CookieJar` extractor so every 2s poll response emits a fresh `csrf_token` into the re-rendered table

Commit: `6e11326 feat(10-09): add Stop column to run_history.html partial`

## Verification

- `cargo build -p cronduit` exits 0 (askama compile-time template context check passes)
- `cargo clippy -p cronduit --all-targets -- -D warnings` exits 0
- `cargo test -p cronduit web::` — all web suites still green (0 failures across sse_streaming, startup_event, stop_handler, stop_race, xss_log_safety)
- All Task 1 acceptance criteria met: cd-btn-stop=1, is_running=2, hx-post=1, /api/runs/=1, hx-swap="none"=1, hx-disabled-elt="this"=1, csrf_token=1, aria-label="Stop run #"=1, forbidden copy=0
- All Task 2 acceptance criteria met: cd-btn-stop cd-btn-stop--compact=1, hx-post=1, /api/runs/=1, run.status == "running"=1, width:1%=1, csrf_token=1, aria-label="Stop run #"=1, forbidden copy=0

## Deviations from Plan

### Rule 2 — Auto-added CookieJar extractor to run_detail handler

- **Found during:** Task 1
- **Issue:** Plan Task 1 action said "Open src/web/handlers/run_detail.rs and look for the template-context struct … Add `csrf_token: String` if absent." The struct was absent from csrf context. The existing `run_detail` handler had no `CookieJar` extractor, so simply adding the struct field would not compile (no token source).
- **Fix:** Added `cookies: axum_extra::extract::CookieJar` to `run_detail` handler signature and threaded `csrf::get_token_from_cookies(&cookies)` through the full-page render branch. Matches the exact pattern already used by `job_detail.rs` (lines 137 + 209 pre-change).
- **Files modified:** src/web/handlers/run_detail.rs
- **Commit:** 5de0a2d

### Rule 2 — Auto-added CookieJar extractor to job_runs_partial handler

- **Found during:** Task 2
- **Issue:** Plan Task 2 action said to confirm csrf_token in the partial's context. The `job_runs_partial` handler (which serves the 2s HTMX poll response — `/partials/jobs/{job_id}/runs`) had no CookieJar extractor. Without this, the polled partial would render with the same (possibly stale) csrf_token from the initial page load, or worse, fall through to `generate_csrf_token()` and emit a token that no browser cookie holds.
- **Fix:** Added `cookies: axum_extra::extract::CookieJar` to `job_runs_partial` signature and threaded the token into `RunHistoryPartial.csrf_token`. Every 2s poll response now carries a fresh `csrf_token` that matches the browser's `cronduit_csrf` cookie.
- **Files modified:** src/web/handlers/job_detail.rs
- **Commit:** 6e11326

### Note on `{% include %}` scope inheritance

- `templates/pages/job_detail.html` renders `run_history.html` via `{% include %}`, which in askama inherits the parent template's scope. `JobDetailPage` already carries `csrf_token`, so the existing page-level render would have worked without the new struct field. Adding the field to `RunHistoryPartial` is still correct (and required) for the standalone HTMX polled path — this is not a deviation, just a documentation note.

## Known Stubs

None.

## Threat Flags

None. The threat model's T-10-09-01 (CSRF on stop form) is mitigated exactly as planned by the double-submit pattern: cookie token + form token compared in constant-time by the plan-10-07 `stop_run` handler. T-10-09-02 (XSS via template interpolation) is mitigated by askama's auto-escape (run.id is i64; csrf_token is hex-string sanitized by the generator). T-10-09-03 (clickjacking) accepted per v1 trusted-LAN posture.

## Tasks Completed

| Task | Name                                                                 | Commit  | Files                                                                             |
| ---- | -------------------------------------------------------------------- | ------- | --------------------------------------------------------------------------------- |
| 1    | Add Stop button to run_detail.html header action slot                | 5de0a2d | templates/pages/run_detail.html, src/web/handlers/run_detail.rs                   |
| 2    | Add Stop column to run_history.html partial                          | 6e11326 | templates/partials/run_history.html, src/web/handlers/job_detail.rs               |

## Pending Human Verification (Task 3)

Task 3 is a `type="checkpoint:human-verify"` gate and this plan is marked `autonomous: false`. The executor has committed all automated work and is returning a structured checkpoint message to the orchestrator. The user must perform the 9 visual checks per 10-09-PLAN.md `<how-to-verify>` (Surface A hover + focus, Surface B compact rendering, normal stop path, silent-refresh race case, dashboard cascade, light-mode slate fallback, version 1.1.0) and reply with "approved" or describe deviations.

Per project memory `feedback_uat_user_validates.md`: "UAT requires user validation; never mark UAT passed from Claude's own test runs." This checkpoint closes the template half of SCHED-09 and SCHED-14 only after the user signs off.

## Self-Check: PASSED

Files modified exist:
- FOUND: templates/pages/run_detail.html
- FOUND: templates/partials/run_history.html
- FOUND: src/web/handlers/run_detail.rs
- FOUND: src/web/handlers/job_detail.rs

Commits exist:
- FOUND: 5de0a2d (Task 1)
- FOUND: 6e11326 (Task 2)

Build clean: `cargo build -p cronduit` exits 0
Clippy clean: `cargo clippy -p cronduit --all-targets -- -D warnings` exits 0
Web tests green: `cargo test -p cronduit web::` all-passed
