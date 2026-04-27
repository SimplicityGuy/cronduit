# Phase 13: Observability Polish (rc.2) - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in `13-CONTEXT.md` — this log preserves the alternatives considered.

**Date:** 2026-04-20
**Phase:** 13-observability-polish-rc-2
**Areas discussed:** Sparkline + success-rate on dashboard, Timeline page layout & interactivity, p50/p95 on job-detail page, Math conventions & sample inclusion rules

---

## Sparkline + success-rate on dashboard

### Q: Where should the sparkline + success-rate badge go in the dashboard table?

| Option | Description | Selected |
|--------|-------------|----------|
| New column left of Actions | Insert 'Recent' column between 'Last Run' and 'Actions'. Sparkline on top row, badge muted beneath. Preserves existing column meaning and sortability. Smallest diff to `job_table.html`. | ✓ |
| Replace 'Status' column with Recent+badge | Sparkline's rightmost cell already conveys last status. Collapse Status into the sparkline. Fewer columns; slight density win. | |
| Restructure table into card grid | Each job becomes a card with sparkline as the top section. More visual breathing room, but breaks column sorting, filter, and 'every 3s' polling shape. Largest diff. | |
| Add sparkline row under each job row (accordion) | Each job gets a second, lighter-weight sub-row. Keeps top row slim; doubles row count. | |

**User's choice:** New column left of Actions (Recommended)
**Notes:** Preserves existing HTMX sortability and polling shape.

---

### Q: How should each sparkline cell be rendered?

| Option | Description | Selected |
|--------|-------------|----------|
| Uniform-height status-colored bars | 20 equal-width, equal-height cells; color from `--cd-status-*`. Clean, scannable. Empty slots render transparent. Matches Buildkite's status dots. | ✓ |
| Height-encoded duration bars | Bar height scales with run duration; color still = status. More information dense but noisy; harder to scan. | |
| Two-row mini-chart | Top strip = status cells, bottom strip = duration bars. More informative; doubles vertical footprint. | |
| Letter glyph per cell (S/F/T/C/X/R) | Single colored letter per cell. Terminal-native but less scannable than solid colors. | |

**User's choice:** Uniform-height status-colored bars (Recommended)

---

### Q: What format should the success-rate badge use?

| Option | Description | Selected |
|--------|-------------|----------|
| Percent only, muted count on hover | '95%' primary; `title="19/20 non-stopped runs"`. Below N=5 renders '—'. | ✓ |
| Percent + ratio inline | '95% (19/20)'. Wider column. | |
| Ratio only | '19/20'. No rounding. Harder to scan. | |
| Percent + sample-count suffix | '95% · N=20'. Explicit sample count; jargon. | |

**User's choice:** Percent only, muted count on hover (Recommended)

---

### Q: What interactivity should the sparkline support?

| Option | Description | Selected |
|--------|-------------|----------|
| Hover tooltip per cell, no click | Native `title` on each cell: `#42 success 1m34s 2h ago`. Keyboard-focusable. No JS. | ✓ |
| Hover tooltip + click-through to run detail | Each cell is an `<a>`. 20 tiny hit targets = accidental-click risk. | |
| Static cells, no hover or click | Pure visual; loses drill-down recovery. | |
| Hover tooltip + optional click (last cell only) | Only rightmost cell clickable. Compromise. | |

**User's choice:** Hover tooltip per cell, no click (Recommended)

---

## Timeline page layout & interactivity

### Q: What is the timeline's primary layout?

| Option | Description | Selected |
|--------|-------------|----------|
| Row-per-job gantt, alphabetical | Each enabled job = one row. Bars span actual start→end. Alphabetical order; stable. Matches Airflow/Jenkins gantt. | ✓ |
| Row-per-job, ordered by most-recent-run | Recently-active jobs float to top. Rows jump between reloads — operators lose their place. | |
| Single-lane packed (flame-graph style) | All runs share one horizontal stream. Loses per-job mental model. | |
| Grid by job × time-bucket | Fixed 10-min buckets. Lossy (two runs collapse to one cell). | |

**User's choice:** Row-per-job gantt, alphabetical (Recommended)

---

### Q: How should the 24h ↔ 7d window toggle behave?

| Option | Description | Selected |
|--------|-------------|----------|
| Pill buttons + ?window= query param | Two buttons at top; active highlighted. URL reflects state; back-button + bookmark friendly. | ✓ |
| Dropdown select | Smaller; invites 1h/12h/30d scope creep. | |
| Buttons, client-side only (no URL state) | Breaks bookmarks, back button, link sharing. | |
| No toggle — always 24h | Violates OBS-01 spec lock. | |

**User's choice:** Pill buttons + ?window= query param (Recommended)

---

### Q: What belongs in a bar's hover tooltip?

| Option | Description | Selected |
|--------|-------------|----------|
| Native title attribute: full context | Browser-native tooltip; a11y-friendly, keyboard-focusable, touch-device long-press. Minimum-surface. | |
| Rich inline HTML tooltip via CSS :hover | Styled popover with border, shadow, status-colored dot. ~50 LOC CSS; no touch support. | ✓ |
| No tooltip — click is the drill-down | Bars react nothing on hover; must click to find out. | |
| Minimal title: status + duration only | `title='success 1m34s'`. Loses `#N` and start-time. | |

**User's choice:** Rich inline HTML tooltip via CSS :hover
**Notes:** Planner instructed to also emit a fallback `title="..."` attribute on every bar for a11y / touch-device long-press support.

---

### Q: Pick the behavior bundle (click target + refresh + nav + empty state):

| Option | Description | Selected |
|--------|-------------|----------|
| Click→run; poll 30s; nav link; in-grid empty | Bars link to run detail. 30s HTMX poll. 'Timeline' added to base.html nav. Empty: axis + centered message. | ✓ |
| Click→job; static (no poll); nav link; in-grid empty | Bar click → job detail. Static page; operator refreshes manually. Lower DB load. | |
| No click; poll 10s; dashboard-link-only; full-page empty | Bars not clickable. Nav entry omitted; discoverable via 'View timeline' button on dashboard. | |
| Click→run; manual Refresh button; nav link; in-grid empty | Explicit Refresh button; no auto-poll. Lowest DB burn. Grafana-like. | |

**User's choice:** Click→run; poll 30s; nav link; in-grid empty message (Recommended)

---

## p50/p95 on job-detail page

### Q: Where should p50/p95 live on the job-detail page?

| Option | Description | Selected |
|--------|-------------|----------|
| New 'Duration' card between Configuration and Run History | Own card matching Configuration shape; room for sample-count subtitle. | ✓ |
| Two chips in the page header next to 'Run Now' | Visible on load; tight on space; competes with Run Now button. | |
| Add a row inside the Configuration card | Minimal diff; conceptually mixes static config with derived stats. | |
| As a header row inside Run History | Ties metric to data. Below the fold on long pages; mixed with pagination. | |

**User's choice:** New 'Duration' card between Configuration and Run History (Recommended)

---

### Q: What format should the two numbers use?

| Option | Description | Selected |
|--------|-------------|----------|
| Labeled chips: 'p50 1m 34s' + 'p95 2m 12s' | Human-readable; matches `run_history.html` Duration column format. | ✓ |
| Raw seconds: 'p50 94s' + 'p95 132s' | More precise; diverges from Duration column on same page. | |
| Colon clock: 'p50 0:01:34' + 'p95 0:02:12' | HH:MM:SS. Novel for this app. | |
| Delta vs p50: 'p50 1m 34s / p95 +38s' | Highlights variance; forces mental math for absolute p95. | |

**User's choice:** Labeled chips: 'p50 1m 34s' + 'p95 2m 12s' (Recommended)

---

### Q: How should sub-threshold state render (N < 20)?

| Option | Description | Selected |
|--------|-------------|----------|
| Dash with tooltip: 'p50 — title=...' | Em-dash in both slots. `title` explains: 'need 20 successful runs, currently have 12'. Card stays visible. | ✓ |
| Hide the Duration card entirely | Silent; delayed feature discoverability. | |
| Show partial: 'p50 850ms† (preview, N=12)' | Dagger + warning. Violates OBS-04 'render as "—" instead of meaningless numbers'. | |
| Show error banner: 'Insufficient data' | Alarmist for a non-problem. | |

**User's choice:** Dash with tooltip (Recommended)

---

### Q: Should the card show the actual sample count used?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, muted subtitle: 'last 87 successful runs' | `--cd-text-secondary` subtitle under values. Same treatment as 'Resolved to ...' in Configuration card. | ✓ |
| Yes, inline: 'p50 1m 34s (n=87)' | More compact; repeats 'n=87' twice; jargon. | |
| Only in tooltip, not visible on page | Hides real information behind a hover. | |
| Don't show sample count anywhere | Simplest; opaque about data window. | |

**User's choice:** Yes, muted subtitle (Recommended)

---

## Math conventions & sample inclusion rules

### Q: Which percentile algorithm should `stats.rs::percentile(samples, q)` use?

| Option | Description | Selected |
|--------|-------------|----------|
| Nearest-rank, 1-indexed | `samples[ceil(q*n)-1]`. Always returns an observed sample. Matches Prometheus semantics. | ✓ |
| Linear interpolation (type-7) | Numpy/R default. Synthetic values; confuses cross-checks against run_history. | |
| Lower rank: `samples[floor(q*n)]` | Biased low. | |
| Upper rank: `samples[min(ceil(q*n), n-1)]` | Biased high. | |

**User's choice:** Nearest-rank, 1-indexed (Recommended)

---

### Q: What counts as a 'successful run' for the p50/p95 input set?

| Option | Description | Selected |
|--------|-------------|----------|
| Strict: status = 'success' only | Matches REQ OBS-04 literal wording. Excludes failed, timeout, cancelled, stopped. | ✓ |
| Lenient: all terminal runs with duration_ms | Mixes populations; skews p95 toward timeout value. | |
| Success + stopped | Stopped = 'however long before operator clicked Stop'. Unrepresentative. | |
| Success + failed | Mixes two populations. Violates REQ 'successful runs' wording. | |

**User's choice:** Strict: status = 'success' only (Recommended)

---

### Q: Sparkline sample rules — 20 cells and denominator

| Option | Description | Selected |
|--------|-------------|----------|
| 20 cells = last 20 terminal runs (incl. stopped); denominator excludes stopped | Matches REQ lock. Stopped visible (grey) but not counted. | ✓ |
| 20 cells = last 20 non-stopped; stopped skipped entirely | Stops invisible. Operator loses visibility into manual interrupts. | |
| 20 cells = last 20 of ANY status (incl. running); denominator strict | Running cells mutate between polls. | |
| 20 cells = last 20 successful + separate failure counter | All-green bar; defeats the sparkline's storytelling. | |

**User's choice:** 20 cells = last 20 terminal runs (incl. stopped); denominator excludes stopped (Recommended)

---

### Q: For the Timeline page, which statuses are bars, and how do running runs render?

| Option | Description | Selected |
|--------|-------------|----------|
| All terminal + running; running bar extends to 'now' with pulse | Running bars span start_time → server now; `@keyframes pulse` @ 2s cycle. Redrawn on 30s poll. | ✓ |
| Terminal only; running runs hidden | Incomplete during active work. | |
| All terminal + running; running as thin placeholder | Loses elapsed-time signal. | |
| All terminal + running; running extends to window edge | Visually misleading about elapsed time. | |

**User's choice:** All terminal + running; running bar extends to 'now' with pulse (Recommended)

---

## Claude's Discretion

Items where the user deferred to the planner, or where implementation choices have no user-facing impact:

- Exact SQL shape for the sparkline query (window function vs LATERAL/subquery).
- `stats.rs::percentile` signature: `&[u64]` vs `&mut Vec<u64>`.
- Exact HTML shape for timeline bar anchors (`<a>` wrapping `<div>` vs styled `<a>` directly).
- Pulse animation cadence for running bars (1.5s / 2s / 3s).
- Whether to emit a vertical "now" indicator line spanning timeline rows.
- "Recent" column header casing ("RECENT" vs "Recent").
- Tooltip above-vs-below-bar positioning.
- `hx-trigger` visibility-aware poll pause.
- `cliff.toml` section grouping for OBS-* commits in the rc.2 release notes.

## Deferred Ideas

Ideas noted during discussion for future phases or v1.2+:

- Timeline SSE live updates (v1.2 observability).
- Additional timeline windows (1h, 12h, 30d) — scope creep.
- Drill-down click from sparkline cells — accidental-click risk.
- Height-encoded duration in sparkline — p50/p95 card already surfaces it.
- SQL-native percentile on Postgres — permanently rejected (structural parity).
- Linear-interpolation percentile — synthetic values confuse cross-checks.
- Lenient p50/p95 sample definitions — population mixing.
- Timeline row reordering by activity — destabilizing.
- Vertical "now" line — Claude's discretion.
- Auto-refresh pause on hidden tab — Claude's discretion.
- Prometheus histogram percentile via metrics facade — out of scope.
- Export timeline data as CSV/JSON — consistent with v1.0 'no export' stance.
- Hand-edited release notes for rc.2 — `git-cliff` authoritative.
- `workflow_dispatch` tag cut shortcut — inherited Phase 12 D-13 rejection.
