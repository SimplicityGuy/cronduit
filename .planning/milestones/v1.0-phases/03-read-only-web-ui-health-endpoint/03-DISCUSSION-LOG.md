# Phase 3: Read-Only Web UI & Health Endpoint - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-10
**Phase:** 03-read-only-web-ui-health-endpoint
**Areas discussed:** Dashboard layout, Log viewer & ANSI, Run Now mechanics, Template & asset pipeline

---

## Dashboard Layout

### Job list presentation

| Option | Description | Selected |
|--------|-------------|----------|
| Dense table | Single-row-per-job HTML table with columns: name, schedule, resolved schedule, next fire, last status badge, last run time. Terminal aesthetic. | ✓ |
| Card grid | One card per job. More visual, lower density. | |
| Hybrid | Table on desktop, cards on mobile. | |

**User's choice:** Dense table
**Notes:** Matches the terminal aesthetic and UI-06 spec literally.

### Filter/sort controls

| Option | Description | Selected |
|--------|-------------|----------|
| Inline controls | Filter text input + clickable column headers. Query params in URL. HTMX-friendly. | ✓ |
| Sidebar filters | Dedicated sidebar with status checkboxes + sort dropdown. | |
| You decide | Claude picks. | |

**User's choice:** Inline controls
**Notes:** None.

### Empty state

| Option | Description | Selected |
|--------|-------------|----------|
| Helpful onboarding | Centered "No jobs configured" with config file path hint. | ✓ |
| Empty table | Table headers with no rows. | |
| You decide | Claude picks. | |

**User's choice:** Helpful onboarding
**Notes:** None.

### Next fire countdown

| Option | Description | Selected |
|--------|-------------|----------|
| Server-side only | Relative time computed on each 3s poll. No extra JS. | ✓ |
| Client-side JS countdown | data-attribute with UTC timestamp, JS ticks every second. | |

**User's choice:** Server-side only
**Notes:** 3s granularity sufficient for cron jobs.

---

## Log Viewer & ANSI

### Stdout/stderr distinction

| Option | Description | Selected |
|--------|-------------|----------|
| Interleaved with color | Single chronological view, stderr lines tinted with error background. | ✓ |
| Tab split | Two tabs: stdout and stderr. Loses temporal context. | |
| Side by side | Two columns. Requires wide viewport. | |

**User's choice:** Interleaved with color
**Notes:** Preserves timeline of what actually happened.

### ANSI parsing approach

| Option | Description | Selected |
|--------|-------------|----------|
| Server-side Rust crate | Parse SGR in Rust, emit sanitized <span> tags. No client-side JS. | ✓ |
| Strip ANSI, plain text | Remove all sequences. Simpler but loses color context. | |
| You decide | Claude picks. | |

**User's choice:** Server-side Rust crate
**Notes:** None.

### Long output handling

| Option | Description | Selected |
|--------|-------------|----------|
| Paginated | Most recent 500 lines, "Load older" button with HTMX offset. | ✓ |
| Full render | All lines. Potentially slow for large output. | |
| You decide | Claude picks. | |

**User's choice:** Paginated
**Notes:** None.

---

## Run Now Mechanics

### Communication with scheduler

| Option | Description | Selected |
|--------|-------------|----------|
| Scheduler command channel | tokio::sync::mpsc channel, SchedulerCmd::RunNow sent from web handler. | ✓ |
| Direct DB + spawn | Web handler directly inserts run row and spawns. Bypasses scheduler. | |
| You decide | Claude picks. | |

**User's choice:** Scheduler command channel
**Notes:** Satisfies UI-12's "through the scheduler" requirement. SchedulerCmd enum designed for extensibility (Phase 5 adds Reload).

### UI feedback after click

| Option | Description | Selected |
|--------|-------------|----------|
| HTMX toast + row update | POST returns HX-Trigger for toast. Next 3s poll shows new run. | ✓ |
| Redirect to run detail | POST redirects to new run's detail page. | |
| Inline status swap | HTMX swaps button cell with "Running..." indicator. | |

**User's choice:** HTMX toast + row update
**Notes:** None.

### CSRF strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Double-submit cookie | Random token in HttpOnly cookie + hidden form field. Server validates match. | ✓ |
| SameSite cookie only | Rely on SameSite=Strict alone. Less defense-in-depth. | |
| You decide | Claude picks. | |

**User's choice:** Double-submit cookie
**Notes:** No server-side session store needed. Fits stateless single-binary model.

---

## Template & Asset Pipeline

### Template inheritance structure

| Option | Description | Selected |
|--------|-------------|----------|
| Base + page templates | base.html with {% block content %}, pages extend it, partials/ for HTMX fragments. | ✓ |
| Flat templates | Each page self-contained with duplicated head/nav/footer. | |
| You decide | Claude picks. | |

**User's choice:** Base + page templates
**Notes:** Template structure: base.html, dashboard.html, job_detail.html, run_detail.html, settings.html, partials/{job_table, run_history, log_viewer}.html

### Dark/light mode

| Option | Description | Selected |
|--------|-------------|----------|
| Dark only for v1 | Ship dark mode only. Halves CSS surface. | |
| Both dark and light | Implement both with toggle. DESIGN_SYSTEM.md has both token sets. | ✓ |
| Dark default + light via media query | Auto-switch via media query, no toggle. | |

**User's choice:** Both dark and light
**Notes:** Deviated from recommended dark-only. Full token table from DESIGN_SYSTEM.md must be implemented.

### Theme toggle mechanism

| Option | Description | Selected |
|--------|-------------|----------|
| System preference + manual toggle | Default follows prefers-color-scheme. Toggle in nav overrides, persists in localStorage. ~10 lines JS. | ✓ |
| System preference only | No toggle, OS settings only. Zero extra JS. | |
| You decide | Claude picks. | |

**User's choice:** System preference + manual toggle
**Notes:** data-theme attribute on <html>, CSS custom properties switch on it.

### Tailwind integration

| Option | Description | Selected |
|--------|-------------|----------|
| just tailwind + build.rs | just recipe runs standalone binary, build.rs ensures CSS is current. just dev runs --watch. | ✓ |
| build.rs only | Tailwind runs inside build.rs automatically. Couples CSS to Rust compile. | |
| You decide | Claude picks. | |

**User's choice:** just tailwind + build.rs
**Notes:** None.

### Navigation structure

| Option | Description | Selected |
|--------|-------------|----------|
| Dashboard + Settings | Two nav items. Job/run detail are drill-down pages. | ✓ |
| Dashboard + Jobs + Settings | Three nav items. Jobs separate from Dashboard. | |
| You decide | Claude picks. | |

**User's choice:** Dashboard + Settings
**Notes:** Theme toggle button also in nav bar.

---

## Claude's Discretion

- ANSI parsing crate choice
- Toast notification implementation details
- Run history pagination page size
- Log viewer lines per page (500 starting point)
- Status badge exact design
- build.rs Tailwind integration details
- AppState field extensions
- Health endpoint additional fields
- Template file naming conventions
- CSS custom property naming

## Deferred Ideas

None — discussion stayed within phase scope.
