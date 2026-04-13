---
phase: 08
slug: v1-final-human-uat-validation
status: draft
shadcn_initialized: false
preset: none
created: 2026-04-13
spec_kind: regression-baseline
---

# Phase 8 — UI Design Contract (Regression Baseline)

> **Phase 8 ships ZERO net-new UI.** This document codifies the visual and interaction
> contracts ALREADY implemented in v1.0 (Phases 3 and 6) so the human UAT walkthrough
> in `03-HUMAN-UAT.md`, `06-HUMAN-UAT.md`, and `07-UAT.md` has an objective yardstick.
> Every dimension below is "baseline documented — no net-new decisions" unless
> explicitly noted.
>
> **Hard rule for the planner / executor:** do NOT add any new UI element, page,
> banner, modal, toast, copy string, color token, font size, or spacing value as
> part of Phase 8. The Docker daemon pre-flight check (D-11/D-12) is **log + gauge
> only** — it MUST NOT surface in the web UI. If a UAT walkthrough finds a polish
> issue, route it to `.planning/BACKLOG.md` per Phase 8 D-26/D-27, not into a Phase 8
> code change.

---

## Design System

| Property | Value |
|----------|-------|
| Tool | none (custom design system, no shadcn — Rust + askama_web, not React) |
| Preset | not applicable |
| Component library | none — server-rendered askama 0.15 templates with `axum-0.8` feature; HTMX 2.0.x for interactivity |
| Icon library | none — Unicode glyphs only (theme toggle uses `&#9790;`); SVG favicons in `design/favicons/` |
| Font | JetBrains Mono (400, 500, 700) — sole font family for the entire UI |

**Canonical sources (read-only baseline for Phase 8):**

- `design/DESIGN_SYSTEM.md` — full token table (colors, typography, spacing, radius, components)
- `templates/base.html` — nav, theme toggle, toast container, HTMX + SSE script tags
- `templates/pages/dashboard.html`, `templates/pages/job_detail.html`, `templates/pages/run_detail.html`, `templates/pages/settings.html`
- `templates/partials/job_table.html`, `templates/partials/run_history.html`, `templates/partials/log_viewer.html`, `templates/partials/static_log_viewer.html`, `templates/partials/toast.html`
- `assets/static/app.css` — built Tailwind output (the effective rendered palette)
- `assets/static/theme.js` — dark/light toggle persistence (~10 lines)
- `assets/vendor/htmx.min.js`, `assets/vendor/htmx-ext-sse.js` — vendored, no CDN

**Phase 3 / Phase 6 origin records:**

- `.planning/phases/03-read-only-web-ui-health-endpoint/03-CONTEXT.md` § Decisions (D-01..D-17)
- `.planning/phases/06-live-events-metrics-retention-release-engineering/06-CONTEXT.md` § SSE live streaming + LIVE badge

---

## Spacing Scale

Baseline documented — no net-new decisions for Phase 8.

The implemented spacing scale is the 4px-grid tokens in `design/DESIGN_SYSTEM.md` § 4
(Spacing & Layout), exposed as CSS custom properties on `:root`:

| Token | Value | Phase 8 UAT yardstick |
|-------|-------|------------------------|
| `--cd-space-1` | 4px | Inline icon gap (theme toggle inner padding) |
| `--cd-space-2` | 8px | Compact pill / badge padding |
| `--cd-space-3` | 12px | Toast inner padding (vertical), cell padding |
| `--cd-space-4` | 16px | Default content padding, log-line gutter |
| `--cd-space-5` | 20px | Card-internal gap |
| `--cd-space-6` | 24px | Card padding (`templates/pages/run_detail.html` metadata card) |
| `--cd-space-8` | 32px | Section break |
| `--cd-space-10` | 40px | (reserved) |
| `--cd-space-12` | 48px | Page-level vertical rhythm |
| `--cd-space-16` | 64px | (reserved, hero spacing) |

**Border radius (frozen):** `--cd-radius-sm` 4px (badges), `--cd-radius-md` 8px
(buttons / inputs / cards), `--cd-radius-lg` 12px (panels), `--cd-radius-xl` 16px
(large containers).

**Exceptions:** none. Phase 8 must not introduce a 5px / 7px / 10px value.

**UAT acceptance criterion (regression):** Visual inspection of the dashboard, job
detail, and run detail pages shows consistent gutters that resolve to the tokens
above. If a card edge looks tighter or looser than the surrounding rhythm and it
is not within the brand tolerance, it is a v1.1 BACKLOG entry per Phase 8 D-26.

---

## Typography

Baseline documented — no net-new decisions for Phase 8.

Single font family for ALL text (headings, body, labels, code, badges) per
`design/DESIGN_SYSTEM.md` § 3:

```
'JetBrains Mono', 'Fira Code', 'Source Code Pro', 'Cascadia Code', 'Consolas', monospace
```

**Implemented sizes (CSS custom properties, used verbatim by every template):**

| Role | Token | Size | Weight | Line Height | Where to look during UAT |
|------|-------|------|--------|-------------|---------------------------|
| Caption / badge | `--cd-text-xs` | 0.65rem (10.4px) | 700 (bold), uppercase, `letter-spacing: 0.1em` | 1.5 | Status badges in dashboard table; `LIVE` badge on Run Detail |
| Secondary | `--cd-text-sm` | 0.8rem (12.8px) | 400 | 1.5 | Nav links, breadcrumb, toast body, table cell metadata |
| Body | `--cd-text-base` | 0.9rem (14.4px) | 400 | 1.6 | Form inputs, log-viewer lines, default body text |
| Emphasized | `--cd-text-md` | 1rem (16px) | 500 | 1.5 | (reserved — emphasized body) |
| Section heading | `--cd-text-lg` | 1.25rem (20px) | 700 | 1.4 | "Log Output" heading on Run Detail; "Run History" |
| Page title | `--cd-text-xl` | 1.5rem (24px) | 700, `letter-spacing: -0.02em` | 1.3 | "Run #{id}", "Job: {name}", "Dashboard" |
| Hero | `--cd-text-2xl` | 2rem (32px) | 700 | 1.2 | (reserved — not used in v1.0 web UI) |

**Frozen:** font family, the seven sizes above, the three weights (400 / 500 / 700),
and the letter-spacing values from `design/DESIGN_SYSTEM.md` § 3 ("Letter Spacing").

**UAT acceptance criterion (regression):** Open the Dashboard, Job Detail, Run Detail,
and Settings pages in the browser. Confirm:

1. JetBrains Mono renders end-to-end (no fallback monospace stack visible).
2. Page titles use `--cd-text-xl` (24px, bold, slight negative tracking).
3. Status badges render uppercase, ~10px, bold, with the `0.1em` tracking.
4. No text element appears in a non-monospace font.

If a glyph falls back to Consolas or system mono in any browser, that's a polish
nit → v1.1 BACKLOG, not a v1.0 blocker (per D-28 default).

---

## Color

Baseline documented — no net-new decisions for Phase 8.

The full token tables in `design/DESIGN_SYSTEM.md` §§ 2.1–2.6 are the source of
truth. The 60/30/10 split is encoded as:

| Role | Dark mode value | Light mode value | Usage (frozen) |
|------|-----------------|------------------|----------------|
| Dominant (60%) — page background | `--cd-bg-primary` `#050508` | `#f8f8f6` | `<body>` background; the dominant terminal-black canvas |
| Secondary (30%) — surfaces | `--cd-bg-surface` `#0a0d0b` | `#ffffff` | Cards, nav bar, dashboard table, metadata panel; `--cd-bg-surface-raised` `#0f1512` / `#f0f0ed` for table headers and elevated panels; `--cd-bg-surface-sunken` `#030405` / `#e8e8e4` for log viewers and code blocks |
| Accent (10%) — terminal green | `--cd-green` `#34d399` | `#059669` | Brand wordmark in nav, primary CTA buttons (Run Now), focus rings, "active/success" status badges, accent links, theme toggle border on hover |
| Destructive / error | `--cd-status-error` `#f87171` | `#dc2626` | Error toasts, error message panels on Run Detail, FAILED status badges, stderr left border on log lines |

**Accent reserved for (the exact list — Phase 8 must not expand this):**

1. Brand wordmark `cronduit` in the nav (`templates/base.html`)
2. Primary CTA buttons — "Run Now" only (per Phase 3 D-08/D-10)
3. Focus rings on form inputs (`--cd-border-focus`)
4. Success status badges (`cd-badge--active`, `cd-badge--success`)
5. Accent links (breadcrumbs, "Refresh the page" SSE error fallback)
6. The `LIVE` badge body color — **note:** badge background uses `--cd-status-running-bg` (blue tint) per Phase 6, NOT green. Verified in `templates/pages/run_detail.html` line 68 (`cd-badge cd-badge--running`)

**Status palette (frozen, per `design/DESIGN_SYSTEM.md` § 2.2):**

| Semantic | Foreground (dark) | Background tint (dark) | Phase 8 UAT yardstick |
|----------|-------------------|------------------------|------------------------|
| Active / Success | `#34d399` | `rgba(52,211,153,0.12)` | `cd-badge--active`, dashboard "last status" badges |
| Running / In-Progress | `#60a5fa` | `rgba(96,165,250,0.12)` | `cd-badge--running`, the `LIVE` badge on Run Detail |
| Disabled / Warning | `#fbbf24` | `rgba(251,191,36,0.12)` | Disabled jobs, SSE error fallback message |
| Error / Failed | `#f87171` | `rgba(248,113,113,0.12)` | `cd-badge--error`, error message panel, error toast |

**Terminal chrome exception (frozen):** Per `design/DESIGN_SYSTEM.md` § 2.6, terminal
frames (logo backdrop, code blocks, log viewers) ALWAYS use the dark terminal
background `--cd-terminal-bg` `#0a0d0b`, even in light mode. Log viewers in the Run
Detail page use `--cd-bg-surface-sunken` (a darker variant). UAT must verify this
holds in light mode.

**UAT acceptance criterion (regression):**

1. Dashboard page background is `#050508` in dark mode and `#f8f8f6` in light mode.
2. Brand wordmark in the nav bar is the green accent in both modes (`#34d399` dark,
   `#059669` light).
3. The four status badge variants render with the correct fg/bg pair from the table
   above (use browser devtools color picker if needed).
4. The `LIVE` badge on a Run Detail page during a sustained RUNNING run renders
   in the **blue** running tint, not green.
5. Error message panel (`templates/pages/run_detail.html` lines 53-60) renders with
   the red `--cd-status-error` border.
6. Stderr lines in the log viewer have a red left border (per Phase 3 D-05).

---

## Copywriting Contract

Baseline documented — no net-new decisions for Phase 8.

All copywriting strings below are **frozen at v1.0**. The Phase 8 planner must NOT
edit these strings. If UAT surfaces a copy nit, route to `.planning/BACKLOG.md`.

| Element | Frozen copy | Source |
|---------|-------------|--------|
| Brand wordmark | `cronduit` (lowercase) | `templates/base.html` |
| Nav: Dashboard link | `Dashboard` | `templates/base.html` |
| Nav: Settings link | `Settings` | `templates/base.html` |
| Theme toggle aria-label | `Toggle dark/light mode` | `templates/base.html` |
| Primary CTA | `Run Now` | `templates/partials/job_table.html` (Phase 3 D-08) |
| Run Now toast (success) | `Run queued: {job_name}` | Phase 3 D-10, dispatched via `HX-Trigger` `showToast` event |
| Toast default fallback | `Action completed` | `templates/base.html` line 64 |
| Empty dashboard heading | `No jobs configured yet` | Phase 3 D-04 |
| Empty dashboard hint | (points to config file path from `AppState`) | Phase 3 D-04 |
| Run Detail breadcrumb | `Dashboard > {job_name} > Run #{id}` | `templates/pages/run_detail.html` |
| Run Detail page title | `Run #{id}` | `templates/pages/run_detail.html` line 17 |
| Log section heading | `Log Output` | `templates/pages/run_detail.html` lines 67, 132 |
| Live badge text | `LIVE` | `templates/pages/run_detail.html` line 68 |
| Live placeholder | `Waiting for output...` | `templates/pages/run_detail.html` line 79 |
| Empty log fallback | `No log output captured for this run.` | `templates/pages/run_detail.html` line 135 |
| SSE error fallback | `Unable to stream logs. Refresh the page to retry.` | `templates/pages/run_detail.html` lines 117-126 |
| Run Detail metadata labels | `STATUS`, `TRIGGER`, `DURATION`, `STARTED`, `ENDED`, `EXIT CODE` (uppercase, `--cd-text-xs`) | `templates/pages/run_detail.html` |
| Error block label | `ERROR` (uppercase, `--cd-status-error`) | `templates/pages/run_detail.html` lines 56-57 |

**Destructive actions in v1.0:** none in the web UI. Run Now is additive (queues
a new run), not destructive. There is no UI surface for delete / disable / restart.
v1.0 ships read-only with the single Run Now mutation. Phase 8 introduces no new
mutations.

**Explicit Phase 8 negative contract — Docker daemon pre-flight UI:**

Per Phase 8 D-11 / D-12 / D-14, the new `cronduit_docker_reachable` gauge and the
`docker daemon unreachable` WARN log are **invisible to the web UI**. Specifically:

- NO new banner on the Dashboard
- NO new toast on page load
- NO new modal at startup
- NO new "Docker: degraded" indicator in the nav bar
- NO new column in the dashboard table
- NO new field on the Settings page
- NO new copy strings anywhere in `templates/`

The signal lives ENTIRELY in the WARN log and the `/metrics` endpoint. Operators
detect daemon-unreachable via Prometheus alerting on `cronduit_docker_reachable == 0`,
not via the UI. The planner must reject any task that proposes a UI surface for
this signal.

**UAT acceptance criterion (regression):**

1. Run Now on a configured job dispatches a toast reading exactly `Run queued: {job_name}`.
2. Toast auto-dismisses after 3000ms (success level) or persists with a close button
   (error level).
3. With Docker stopped at startup, the dashboard renders identically to a Docker-up
   startup — no new UI element appears anywhere.
4. With zero jobs configured, the Dashboard renders the "No jobs configured yet"
   empty state (Phase 3 D-04).

---

## Component States — Frozen Interaction Contracts

The six interaction contracts below are the canonical pass/fail anchors for
`03-HUMAN-UAT.md` and `06-HUMAN-UAT.md`. Each is "implemented in v1.0; Phase 8
walks through to confirm".

### IC-1 — Terminal-green theme rendering (Phase 3 UI-05, UI-06)

| Property | Frozen value |
|----------|--------------|
| Default theme on first load | `data-theme="dark"` (set in `templates/base.html` line 2) |
| Page background (dark) | `#050508` (`--cd-bg-primary`) |
| Brand wordmark color | `#34d399` (dark) / `#059669` (light) |
| Body font | JetBrains Mono 400 |
| Nav bar background | `#0a0d0b` (dark) / `#ffffff` (light) |
| Nav bar bottom border | `--cd-border-subtle` |
| Dashboard table | dense, monospace, status badges per the Color section above |

**Acceptance script for `03-HUMAN-UAT.md` Test 1 ("Terminal-green design system rendering"):**

> Open `http://127.0.0.1:8080/` in a fresh browser tab. Confirm: dark `#050508`
> background, the lowercase `cronduit` wordmark in the nav appears in the green
> accent (`#34d399`), the dashboard table renders in JetBrains Mono with bold
> uppercase column headers, status badges show in the four semantic colors per the
> table above. **Pass** if all four hold; **issue** otherwise.

### IC-2 — Dark/light toggle persistence (Phase 3 D-13)

| Property | Frozen value |
|----------|--------------|
| Toggle location | Nav bar, right side, after Settings link |
| Toggle aria-label | `Toggle dark/light mode` |
| Toggle glyph | `&#9790;` (Unicode FIRST QUARTER MOON) |
| Persistence mechanism | `localStorage.setItem('cronduit-theme', 'light' \| 'dark')` |
| localStorage key (frozen) | `cronduit-theme` |
| Applied attribute | `<html data-theme="...">` |
| Applied at | `assets/static/theme.js` runs synchronously in `<head>` (no FOUC) |
| System-preference fallback | If no localStorage value, the inline `data-theme="dark"` default applies; the system-pref `@media` query overrides only when no `data-theme="dark"` is set on `<html>` |

**Acceptance script for `03-HUMAN-UAT.md` Test 2 ("Dark/light mode toggle"):**

> Click the moon glyph in the nav bar. Page should switch from dark to light (or
> vice versa) within one frame. Reload the page (Cmd-R / Ctrl-R). Theme should
> persist. Open browser devtools → Application → Local Storage → `http://127.0.0.1:8080`
> → confirm a `cronduit-theme` key with value `light` or `dark`. **Pass** if all
> three hold; **issue** otherwise.

### IC-3 — Run Now toast (Phase 3 D-10)

| Property | Frozen value |
|----------|--------------|
| Trigger | `POST /api/jobs/{id}/run` returns 200 with `HX-Trigger: showToast` (custom event with `{message, level, duration}` payload) |
| DOM container | `<div id="toast-container">` in `templates/base.html` (fixed top-4 right-4, z-50) |
| Default duration | 3000ms (success), persists with close button (error) |
| Success copy | `Run queued: {job_name}` |
| Success styling | bg `--cd-status-active-bg`, fg `--cd-status-active`, border `1px solid --cd-status-active`, radius `--cd-radius-md`, padding `--cd-space-3 --cd-space-4`, `--cd-text-sm` |
| Error styling | bg `--cd-status-error-bg`, fg `--cd-status-error`, border `1px solid --cd-status-error`, with explicit `×` close button |
| ARIA | `role="status"`, `aria-live="polite"` |
| Auto-dismiss animation | `opacity: 0` over 300ms then `remove()` |

**Acceptance script for `03-HUMAN-UAT.md` Test 3 ("Run Now toast notification"):**

> Open the dashboard. Click the Run Now button on a row whose underlying job is
> known to run successfully on the alpine runtime (post-D-01 rebase): pick
> `echo-timestamp` (it runs `date '+%Y-%m-%d %H:%M:%S -- Cronduit is running!'`).
> A toast should appear in the top-right corner reading exactly
> `Run queued: echo-timestamp`. The toast should remain visible for ~3 seconds
> then fade out over ~300ms. **Pass** if both message and timing hold; **issue**
> otherwise.

### IC-4 — ANSI log rendering with stdout/stderr distinction (Phase 3 D-05, D-06, UI-09)

| Property | Frozen value |
|----------|--------------|
| Rendering location | Server-side in Rust (Phase 3 D-06) — log lines arrive at the browser as already-sanitized HTML spans |
| ANSI parser scope | SGR color codes only; everything else HTML-escaped (UI-10) |
| stdout styling | Default text color, no left border |
| stderr styling | Subtle left border in `--cd-status-error` per `design/DESIGN_SYSTEM.md` § 2.2; lines remain in temporal order interleaved with stdout |
| Container background | `--cd-bg-surface-sunken` (`#030405` dark / `#e8e8e4` light) |
| Container max-height | 600px when running (with overflow scroll); auto when static |
| Font | JetBrains Mono `--cd-text-base` |
| Pagination | Most recent 500 lines first; "Load older lines" button via HTMX `hx-get` (Phase 3 D-07) |

**Acceptance script for `03-HUMAN-UAT.md` Test 4 ("ANSI log rendering in Run Detail"):**

> Trigger an `echo-timestamp` run via Run Now. Open its Run Detail page after the
> run completes. Confirm: log line shows the timestamp string. (Optional: configure
> a job that emits ANSI escape sequences — Phase 8 does NOT add such a job, so this
> sub-test is opportunistic.) For stderr distinction: trigger `http-healthcheck`
> (`wget` with `2>&1` mixes streams; the `2>&1` redirect collapses stderr into
> stdout, so this UAT may need a synthetic stderr-emitting test job from the
> operator's own config). If the operator does not have a synthetic stderr job,
> mark the stderr sub-criterion as `result: pass note: "no stderr-emitting example
> job in v1.0 quickstart; visual inspection only"`. **Pass** if (a) ANSI parsing
> renders to colored spans without raw escape codes leaking and (b) the log
> container background and font match the table above.

### IC-5 — SSE LIVE badge → static viewer transition (Phase 6 UI-14, D-11/D-12 in 06-CONTEXT)

| Property | Frozen value |
|----------|--------------|
| Trigger condition | `is_running == true` on the Run Detail handler (run row in `status='running'`) |
| Badge placement | Inline next to the `Log Output` section heading |
| Badge styling | `cd-badge cd-badge--running` (blue running tint, NOT green); text content `LIVE`; uppercase; `--cd-text-xs`; bold |
| Badge ARIA | `aria-label="Live streaming"` |
| Stream endpoint | `/events/runs/{run_id}/logs` |
| HTMX SSE wiring | `hx-ext="sse"`, `sse-connect=...`, `sse-swap="log_line"`, `hx-swap="beforeend"` (append-only) |
| Container ARIA | `role="log"`, `aria-live="polite"` |
| Auto-scroll | Anchored to bottom; user scroll-up pauses auto-scroll until they scroll back within 50px of the bottom |
| Placeholder | `Waiting for output...` (centered, secondary text color) — removed on first appended line |
| Completion transition | Server emits SSE `run_complete` event → JS calls `htmx.ajax('GET', '/partials/runs/{run_id}/logs', {target:'#log-container', swap:'outerHTML'})` → entire LIVE container is replaced by the static viewer (`templates/partials/static_log_viewer.html` rendered into a `#log-container` `<div>` with no LIVE badge) |
| SSE error fallback | Replace container contents with disabled-color message + "Refresh the page" link in accent color |

**Acceptance script for the new `06-HUMAN-UAT.md` "SSE live log streaming" test:**

> Trigger a long-running job (after the alpine rebase, `http-healthcheck` `wget`
> over the network typically sustains a RUNNING state for ~3-15 seconds; in CI the
> smoke test uses Run Now + poll-for-success). Open the Run Detail page for that
> run BEFORE it completes. Confirm:
>
> 1. The `LIVE` badge appears immediately next to the `Log Output` heading, in the
>    blue running tint.
> 2. The log container shows `Waiting for output...` initially, then begins
>    appending lines as they stream in. New lines append at the bottom; the
>    container auto-scrolls to keep the most recent line visible.
> 3. When the run completes, the entire log container is replaced by the static
>    viewer — the `LIVE` badge disappears, and the same log content is now
>    rendered from the database (no SSE connection, no auto-scroll). No browser
>    refresh required.
> 4. If the SSE connection drops mid-run, the container shows
>    `Unable to stream logs. Refresh the page to retry.` in the disabled tint
>    with the "Refresh the page" link in the accent color.
>
> **Pass** if all four hold; **issue** otherwise.

### IC-6 — Job Detail run-history conditional auto-refresh (Plan 07-05, re-UAT)

| Property | Frozen value |
|----------|--------------|
| Wrapper element | The Run History card on the Job Detail page (`templates/partials/run_history.html`) |
| Polling trigger | `hx-trigger="every 2s"` — applied conditionally by the server based on `any_running` (true if any row in the rendered partial is in `status='running'`) |
| Polling endpoint | `/partials/jobs/{job_id}/runs` |
| Stop condition | When the server-rendered partial sees `any_running == false`, it re-renders the wrapper WITHOUT the `hx-trigger` attribute → HTMX implicitly stops polling on the next swap |
| RUNNING → terminal transition cadence | Within ~2 seconds of underlying run completion (one full poll interval) |
| Polling cadence intent | Matches the smoke-test polling cadence (2s) so what CI asserts mirrors what the user sees |
| Network tab evidence | After all rows reach terminal state, devtools Network filtered by "runs" should show ZERO follow-up requests for at least 10 seconds |

**Acceptance script for `07-UAT.md` Test 2 (re-run, currently `result: issue` blocker):**

> Once Phase 8 plans land (alpine rebase + the four example jobs in D-15), navigate
> to the Job Detail page for `http-healthcheck` (or `disk-usage`, both sustain a
> ~5-15 second RUNNING window naturally). Click Run Now 10 or more times in rapid
> succession. Confirm:
>
> 1. New rows appear in the Run History card immediately as RUNNING (HX-Refresh
>    from Plan 06 — already shipped).
> 2. Within ~2 seconds of each underlying run completing, the row's RUNNING badge
>    transitions to SUCCESS (or FAILED) without a manual page reload.
> 3. After all runs reach terminal state, open devtools → Network, filter by
>    "runs", and watch for 10 seconds. Zero follow-up requests should appear (the
>    conditional `hx-trigger` is gone after the last RUNNING row flips).
>
> **Flip `result: issue` → `result: pass`** if all three hold. If any sub-criterion
> fails, leave as `issue` and route to v1.1 BACKLOG only if the failure is cosmetic
> (per D-26 — functional breakage of polling is a Phase 8 fix).

---

## Out-of-Scope Negative Contracts (Phase 8 must NOT add)

The list below exists because Phase 8 is small, the temptation to "while we're
already in here, polish X" is real, and the project memory rule "UAT requires
user validation" means Claude cannot self-justify polish edits. The planner
must reject every task that introduces any of these:

1. New page, new route, new template file under `templates/pages/`
2. New partial under `templates/partials/`
3. New CSS custom property (no `--cd-*` additions to `assets/src/app.css`)
4. New Tailwind config change (no new colors, sizes, or classes scanned beyond
   what `templates/**/*.html` already uses)
5. New copy string in any template
6. New icon, glyph, or SVG
7. New JS file beyond `theme.js`
8. New `HX-Trigger` event type or new toast level
9. UI surface for `cronduit_docker_reachable` (the gauge is metrics-only)
10. UI surface for `cronduit_retention_*` metrics (also metrics-only)
11. Dark-mode visual tweaks (every dark-mode value is frozen in `design/DESIGN_SYSTEM.md` § 2)
12. Light-mode visual tweaks
13. Accessibility upgrades beyond what Phase 3/6 already shipped (route polish to v1.1)
14. New favicon variant or banner asset

If a UAT walkthrough surfaces any of the above as a desire, it goes into
`.planning/BACKLOG.md` per Phase 8 D-27 with the originating UAT file + line and
a "why this isn't a v1.0 blocker" sentence.

---

## Registry Safety

Not applicable — Cronduit does not use shadcn or any third-party component
registry. The entire UI is hand-written askama templates + a small custom CSS
token file built by the standalone Tailwind binary. No third-party UI code
enters the binary.

| Registry | Blocks Used | Safety Gate |
|----------|-------------|-------------|
| n/a | n/a | not applicable — no registry in use |

---

## Phase 8 Planner Instructions Summary

1. **Treat this document as read-only baseline.** Do not write tasks that mutate
   any frozen value above.
2. **The Docker daemon pre-flight (D-11) is log + gauge only.** Reject any task
   that proposes a UI banner / modal / toast / nav indicator for it.
3. **The four new example jobs in D-15 are config-file-only** (`examples/cronduit.toml`).
   They do NOT require new UI work — the dashboard already renders any job from
   any config.
4. **The dual compose files in D-07 are YAML-only.** They do NOT require UI work.
5. **The compose-smoke CI extension in D-18 is shell + YAML only.** It does NOT
   require UI work.
6. **The human UAT walkthrough uses this document as the yardstick.** Each `IC-N`
   section above maps to a test in `03-HUMAN-UAT.md`, `06-HUMAN-UAT.md`, or
   `07-UAT.md`. The user reads the acceptance script and types `pass` / `issue` /
   `blocked`. Per project memory rule, Claude does NOT self-mark.
7. **Polish issues route to v1.1 BACKLOG.** Functional breakage routes to a
   Phase 8 fix. Default to BACKLOG when ambiguous (D-28).

---

## Checker Sign-Off

This is a regression-baseline UI-SPEC, not a net-new design. The checker should
verify that each dimension cites the canonical source (`design/DESIGN_SYSTEM.md`,
`templates/...`, `assets/static/theme.js`, or a Phase 3/6 context decision) rather
than introducing a new value. The checker MUST flag any cell in the tables above
that contains a value not traceable to one of those sources.

- [ ] Dimension 1 Copywriting: PASS (frozen — every string traced to a template file or Phase 3/6 decision)
- [ ] Dimension 2 Visuals: PASS (frozen — every component contract traced to a template file)
- [ ] Dimension 3 Color: PASS (frozen — every value traced to `design/DESIGN_SYSTEM.md` § 2)
- [ ] Dimension 4 Typography: PASS (frozen — every value traced to `design/DESIGN_SYSTEM.md` § 3)
- [ ] Dimension 5 Spacing: PASS (frozen — every value traced to `design/DESIGN_SYSTEM.md` § 4)
- [ ] Dimension 6 Registry Safety: PASS (not applicable — no third-party registry)

**Approval:** pending (checker to upgrade to `approved YYYY-MM-DD` after verifying
the citation chain above)
