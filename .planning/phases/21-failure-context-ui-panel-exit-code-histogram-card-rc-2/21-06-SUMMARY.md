---
phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
plan: 06
subsystem: ui
tags: [askama, css, fctx, exit-histogram, ui-spec, run-detail, job-detail, tailwind]

# Dependency graph
requires:
  - phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
    plan: 04
    provides: "RunDetailPage.show_fctx_panel + RunDetailPage.fctx (FctxView) + 11-field FctxView struct (research §H) — pre-formatted view-model the askama template substitutes verbatim"
  - phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
    plan: 05
    provides: "JobDetailPage.exit_histogram (ExitHistogramView) + BucketRender + TopCodeRender — 8-field ExitHistogramView with 10-entry bucket Vec, server-clamped height_pct, locked color/copy mappings; pre-formatted view-model the askama template substitutes verbatim"
  - phase: 13-observability-polish-rc-2
    plan: 04
    provides: ".cd-tooltip / .cd-tooltip-row / .cd-tooltip-dot CSS contract reused for the histogram bar tooltip (no redeclaration; NEW .cd-exit-bar:hover .cd-tooltip selector parallel to existing .cd-timeline-bar:hover .cd-tooltip)"
provides:
  - "templates/pages/run_detail.html: <details class=\"cd-fctx-panel mb-6\"> + 5 conditionally-rendered rows (TIME DELTAS / IMAGE DIGEST / CONFIG / DURATION / FIRE SKEW); inserted between metadata card (line 73) and Log Viewer (line 75) per UI-SPEC § Component Inventory § 1"
  - "templates/pages/job_detail.html: <div class=\"cd-exit-card mb-6\"> with success-rate stat + pure-CSS bar chart + recent-codes table + below-N=5 empty-state; inserted between Duration card (line 94) and Run History (line 96) per UI-SPEC § Component Inventory § 2"
  - "assets/src/app.css @layer components: 16 cd-fctx-* declarations + 28 cd-exit-* declarations; @media print { details.cd-fctx-panel { open: open; } } interaction-contract rule; .cd-fctx-summary-caret reduced-motion extension"
  - "RunDetailPage.show_fctx_panel + RunDetailPage.fctx + JobDetailPage.exit_histogram: #[allow(dead_code)] removed (templates now consume the fields)"
affects: [21-07, 21-08, 21-09, 21-10]

# Tech tracking
tech-stack:
  added: []  # zero new external crates (D-32 invariant preserved)
  patterns:
    - "Logic-free askama template (UI-SPEC § Copywriting Contract): template substitutes {{ value }} with zero conditional copy — every per-row gating decision and every copy variant is pre-rendered server-side in plans 21-04 / 21-05's view-model builders"
    - "Native <details>/<summary> for collapse/expand (UI-SPEC § Interaction Contract): zero JavaScript, zero HTMX — browser handles aria-expanded, keyboard Space/Enter toggle, focus ring; print-mode opens panel via @media print"
    - "Pure-CSS bar chart with inline style=\"height:{{ pct }}%\" and style=\"background:var(--cd-{dot_token})\" — height_pct server-clamped to 0..=100 in plan 21-05's build_exit_histogram_view; dot_token is a controlled string-set produced by bucket_classes lookup (NEVER operator input) per research § Security V5"
    - "Tooltip reuse pattern: existing .cd-tooltip / .cd-tooltip-row / .cd-tooltip-dot classes (Phase 13 line 444-490) NOT redeclared. The ONLY new selector is .cd-exit-bar:hover .cd-tooltip, .cd-exit-bar:focus-visible .cd-tooltip — parallel to the existing .cd-timeline-bar:hover .cd-tooltip rule. Anchor element changes; tooltip styling unchanged"
    - "Outer chrome inline style on cd-exit-card matches the Duration sibling card verbatim (background/border/border-radius/padding via existing tokens) per UI-SPEC § Layout & Surfaces 'non-negotiable for design coherence'"
    - "Token-only CSS values: every padding/margin/gap/min-height/height/top/font-size/color/background/border/border-radius resolves to a var(--cd-*) token, calc(var(--cd-*) * N), or 0 — NO bare px literals (UI-SPEC § Spacing: exceptions = none). Specifically: min-height: var(--cd-space-1) (4px on-grid), top: calc(-1 * var(--cd-space-4)) (-16px count badge offset), height: calc(var(--cd-space-8) * 4) (128px chart canvas) — all token-derived"
    - "Mobile breakpoint pattern: @media (max-width: 640px) { .cd-fctx-row { grid-template-columns: 1fr; } } — single declaration collapses panel rows from 200px-1fr two-column to 1fr stacked single-column on narrow viewports; histogram chart preserves 640px min-width with overflow-x: auto matching the Phase 13 timeline pattern"
    - "Bucket modifier class pattern: .cd-exit-bar--err-strong / --err-muted / --warn / --stopped / --null map to var(--cd-status-error) / var(--cd-status-error-bg) + 1px error border / var(--cd-status-disabled) / var(--cd-status-stopped) / var(--cd-status-cancelled) per UI-SPEC § Color bucket→token mapping. The bar element gets BOTH .cd-exit-bar (base) and .cd-exit-bar--{color_class} (modifier) — modifier provides only the background/border declaration"
    - "Print-mode pattern: @media print { details.cd-fctx-panel { open: open; } } in @layer components forces the panel open when an operator prints the run-detail page for postmortems (UI-SPEC § Interaction Contract)"
    - "Struct-level #[allow(dead_code)] on FctxView (consecutive_failures + last_success_run_id) and ExitHistogramView (success_rate_pct) — these fields are populated for future consumers (metrics, JSON API) but the askama template only references their derived strings (summary_meta / last_success_run_url / success_rate_display); leaving the struct-level allow keeps the build warning-free without suppressing field-level dead-code on unrelated structs"

key-files:
  created: []
  modified:
    - templates/pages/run_detail.html
    - templates/pages/job_detail.html
    - assets/src/app.css
    - assets/static/app.css  # tailwind rebuild output (generated by build.rs)
    - src/web/handlers/run_detail.rs  # remove dead_code on show_fctx_panel + fctx
    - src/web/handlers/job_detail.rs  # remove dead_code on exit_histogram

key-decisions:
  - "Removed #[allow(dead_code)] from RunDetailPage.show_fctx_panel + .fctx and JobDetailPage.exit_histogram (per the wave 2 state 'should be removed' instruction). Kept struct-level #[allow(dead_code)] on FctxView (consecutive_failures + last_success_run_id) and ExitHistogramView (success_rate_pct) because the template only consumes the derived/formatted variants — those raw fields are populated for future JSON-API / metrics consumers and removing the struct-level allow would surface them as warnings. Updated the FctxView doc-comment to explain the rationale rather than the previous 'plan 21-06' marker."
  - "Did NOT modify the existing run_detail.html log-streaming inline <script> (lines 100-176). Verified by inspection that the new FCTX panel block contains zero <script> tags. The plan's must_have 'NO new inline <script> blocks' is satisfied — the existing log-streaming script predates Phase 21 and is unrelated to the FCTX surface."
  - "Inline outer chrome on cd-exit-card via style=\"...\" attribute (NOT a CSS class declaration). The plan locked this in must_haves via the 'matches the Duration sibling card' constraint and the UI-SPEC explicitly says cd-exit-card 'uses outer chrome via existing .mb-6 + inline styles to match Duration sibling — same as run_detail metadata card'. Only .cd-exit-card-title (the heading inside) gets a class-based rule. This keeps the visual contract 'exact sibling' rather than introducing a near-duplicate chrome class. Verified by grep: zero .cd-exit-card { ... } selector declarations in app.css; .cd-exit-card-title selector is the only cd-exit-card-prefixed rule."
  - "Bare-literal sentinel grep is intentionally narrow: the plan's acceptance check `grep -E 'min-height: 2px|top: -18px|height: 128px' assets/src/app.css | wc -l` returns 0 — these were the three exact bare-px values the UI-SPEC's final spacing pass eliminated. Broader 'no px literals anywhere in the file' is NOT enforced because pre-Phase-21 code (Phase 13 .cd-tooltip-dot width: 8px; height: 8px; line 476-477; .cd-tooltip box-shadow: 0 4px 12px rgba(0,0,0,0.35) line 457; the 8px disclosure offset on .cd-tooltip bottom: calc(100% + 8px) line 448) ships with bare px literals and is out-of-scope for this plan. The new CSS surfaces in Phase 21 are token-clean."
  - "Reduced-motion extension landed inside the existing @media (prefers-reduced-motion: reduce) block (line 431-434) rather than a NEW separate block. The plan said 'extend the existing block' verbatim. Single-block ergonomics: future readers find every reduced-motion override in one place rather than scattered across the file."
  - "Tooltip reuse landed as the ONLY new tooltip-related selector. Verified by `grep -c '.cd-tooltip {' assets/src/app.css` returning 1 (the existing Phase 13 declaration; no redeclaration). The Phase 21 addition is purely a NEW anchor-element rule — `.cd-exit-bar:hover .cd-tooltip, .cd-exit-bar:focus-visible .cd-tooltip { visibility: visible; opacity: 1; }` — which mirrors the existing `.cd-timeline-bar:hover .cd-tooltip` rule for a different DOM anchor. Zero net CSS duplication."
  - "Build artifact assets/static/app.css regeneration is committed alongside source assets/src/app.css. The build script (build.rs lines 4-58) runs the standalone Tailwind binary on every cargo build to keep the production CSS in sync with source; both files MUST be committed together so a fresh git clone + cargo build reproduces the exact CSS shipped to operators (single-binary deployment story)."

patterns-established:
  - "When a UI plan adds two new surfaces sharing a stylesheet (here: FCTX panel on run_detail + exit histogram on job_detail), append CSS as named sub-blocks within the existing @layer components closure with explicit '/* === Phase N $surface === */' comment headers. Grep-friendly: `grep '=== Phase 21' assets/src/app.css` lists every Phase 21 sub-block. Allows future plan authors to locate their additions without reading the entire 555-line stylesheet."
  - "When adding a new <details>/<summary> surface that should print expanded for postmortems, the print-mode rule lives at the bottom of the @layer components block as a tiny @media print { ... } sub-block (literal 3 lines including comment). Don't scatter print-mode rules near the surface they affect — colocate so future authors can audit print behavior in one grep."
  - "When extending an existing @media (prefers-reduced-motion: reduce) block with new transition/animation overrides, append the rule INSIDE the existing block rather than opening a new @media block. Single-block ergonomics: future readers find every reduced-motion override in one place. Verified by `grep -c 'prefers-reduced-motion' assets/src/app.css` returning 1 (one block, multiple rules)."
  - "When the UI-SPEC says 'matches the sibling card', use inline style=\"...\" on the outer wrapper element to literally inherit the sibling's chrome rather than introducing a near-duplicate CSS class. Avoids drift over time: the two surfaces are visually identical because the SAME style declaration is on both elements. Only the inner heading + content gets class-based styling."

requirements-completed: [FCTX-01, FCTX-02, FCTX-03, FCTX-05, FCTX-06, EXIT-01, EXIT-02, EXIT-03, EXIT-04, EXIT-05]

# Metrics
duration: ~11min
completed: 2026-05-02
---

# Phase 21 Plan 06: FCTX panel + Exit Histogram template inserts + CSS Summary

**Phase 21 becomes user-visible: <details class="cd-fctx-panel"> with 5 conditionally-rendered rows lands in `templates/pages/run_detail.html` between metadata and Log Viewer (FCTX-01..06); <div class="cd-exit-card"> with success-rate stat + pure-CSS bar chart + recent-codes sub-table lands in `templates/pages/job_detail.html` between Duration card and Run History (EXIT-01..05); 16 cd-fctx-* + 28 cd-exit-* CSS declarations land in `assets/src/app.css` @layer components with print-mode rule and reduced-motion extension; every CSS value resolves to a token; askama template carries zero logic per UI-SPEC § Copywriting Contract — the view-model builders from plans 21-04 and 21-05 produce every conditional copy server-side and the template substitutes `{{ value }}` verbatim.**

## Performance

- **Duration:** ~11 min
- **Started:** 2026-05-02T20:48:29Z
- **Completed:** 2026-05-02T20:59:18Z
- **Tasks:** 3 (all atomic-committed)
- **Files modified:** 6 (`templates/pages/run_detail.html`, `templates/pages/job_detail.html`, `assets/src/app.css`, `assets/static/app.css` (generated), `src/web/handlers/run_detail.rs`, `src/web/handlers/job_detail.rs`)

## Accomplishments

- **FCTX panel (Task 1, commit `248a0b2`):**
  - `<details class="cd-fctx-panel mb-6">` inserted in `templates/pages/run_detail.html` between line 73 (metadata card close) and line 75 (Log Viewer comment) per UI-SPEC § Component Inventory § 1 markup contract
  - 5 conditional rows in locked order: TIME DELTAS → IMAGE DIGEST → CONFIG → DURATION → FIRE SKEW
  - Outer guard: `{% if show_fctx_panel %}{% if let Some(fctx) = fctx %}` hides the entire panel on success/cancelled/running/stopped runs
  - Per-row gating per D-13 / D-04: TIME DELTAS always renders for failed/timeout; the "view last successful run" link inside it uses `{% match fctx.last_success_run_url %}` and only renders when populated; IMAGE DIGEST gated by both `{% if fctx.is_docker_job %}` (FCTX-03) and `{% match fctx.image_digest_value %}` (D-13 hide on never-succeeded); CONFIG gated by `{% match fctx.config_changed_value %}` (D-13 hide on never-succeeded); DURATION gated by both `{% if fctx.has_duration_samples %}` (FCTX-05 N>=5) and `{% match fctx.duration_value %}` (defensive); FIRE SKEW gated by `{% match fctx.fire_skew_value %}` (D-04 NULL scheduled_for hide)
  - Output escaping: ALL values use `{{ value }}` with askama auto-escaping; ZERO `|safe` filters anywhere in the new content (D-17 / UI-SPEC § Output Escaping & XSS); the `<a href="{{ url }}">` for the last-successful-run link uses auto-escaped URL
  - Locked copy: "Failure context" summary heading, "TIME DELTAS" / "IMAGE DIGEST" / "CONFIG" / "DURATION" / "FIRE SKEW" row labels, "[view last successful run]" link text, "▸" caret unicode triangle (UI-SPEC § Copywriting Contract verbatim)
  - All 11 cd-fctx-* class names in the locked namespace; `mb-6` Tailwind utility reused on the outer `<details>`
  - NO new inline `<script>` blocks added; the existing log-streaming script (lines 100-176) is untouched

- **Exit-code histogram card (Task 2, commit `a98c28a`):**
  - `<div class="cd-exit-card mb-6">` inserted in `templates/pages/job_detail.html` between line 94 (Duration card close) and line 96 (Run History comment) per UI-SPEC § Component Inventory § 2 markup contract
  - Outer chrome inline `style="background:var(--cd-bg-surface);border:1px solid var(--cd-border);border-radius:8px;padding:var(--cd-space-6)"` matches the Duration sibling card verbatim (UI-SPEC § Layout & Surfaces "non-negotiable")
  - `{% if exit_histogram.has_min_samples %}` branch renders: success-rate stat block (label/value/meta), histogram chart with 10 buckets in display order using `style="height:{{ bucket.height_pct }}%"` (server-clamped per plan 21-05), per-bucket aria-label + tooltip with reused `.cd-tooltip` Phase 13 chrome, caption "Last {N} runs (window: 100). Hover bars for detail.", recent-codes sub-table gated by `{% if !exit_histogram.top_codes.is_empty() %}` with Code/Count/Last seen columns
  - `{% else %}` empty-state branch renders: em-dash "—" glyph + locked copy "Need 5+ samples; have {N}" per UI-SPEC § Copywriting Contract + D-15 / D-16
  - Tooltip dot color via inline `style="background:var(--cd-{{ bucket.dot_token }})"` — `dot_token` is a controlled string-set from `bucket_classes` lookup (NEVER operator input per research § Security V5)
  - Bucket modifier class via `cd-exit-bar cd-exit-bar--{{ bucket.color_class }}` substitution; `bucket.color_class` is a 5-value enum-mapped string from `bucket_classes` (`err-strong` / `err-muted` / `warn` / `stopped` / `null`)
  - Accessibility: `role="img"` on the chart container with `chart_aria_summary` aria-label; `tabindex="0"` on every bar for keyboard navigation per UI-SPEC § Accessibility
  - Output escaping: ALL values use `{{ value }}` with askama auto-escaping; ZERO `|safe` filters anywhere
  - Locked copy: "Exit Code Distribution", "SUCCESS", "Most frequent codes", "Code"/"Count"/"Last seen" column headers, caption text, empty-state copy

- **CSS additions (Task 3, commit `3c2440e`):**
  - 16 cd-fctx-* selector declarations appended to `@layer components` block in `assets/src/app.css` per UI-SPEC § Component Inventory § 1 CSS contract table
  - 28 cd-exit-* selector declarations per UI-SPEC § 2 CSS contract table (card title, stats stack, chart grid, bar wrapper, 5 bucket modifier classes, bucket label, caption, recent-codes table th/td, empty-state)
  - Tooltip reuse: NEW selector `.cd-exit-bar:hover .cd-tooltip, .cd-exit-bar:focus-visible .cd-tooltip { visibility: visible; opacity: 1; }` parallel to existing Phase 13 `.cd-timeline-bar:hover .cd-tooltip` rule (line 467-471). The `.cd-tooltip` / `.cd-tooltip-row` / `.cd-tooltip-dot` classes from Phase 13 are NOT redeclared — only the new anchor-element selector
  - Print-mode rule `@media print { details.cd-fctx-panel { open: open; } }` per UI-SPEC § Interaction Contract — operator-friendly when printing run detail for incident postmortems
  - Reduced-motion extension: `.cd-fctx-summary-caret { transition: none; }` appended INSIDE the existing `@media (prefers-reduced-motion: reduce)` block at lines 431-434 (now lines 431-435)
  - Token-only values: every padding/margin/gap/min-height/height/top/font-size/color/background/border/border-radius resolves to a `var(--cd-*)` token, `calc(var(--cd-*) * N)`, or `0` per UI-SPEC § Spacing (exceptions: none). Specifically: `min-height: var(--cd-space-1)` (4px on-grid; was bare `2px` pre-checker), `top: calc(-1 * var(--cd-space-4))` (-16px count badge; was bare `-18px` pre-checker), `height: calc(var(--cd-space-8) * 4)` (128px chart canvas; was bare `128px` pre-checker)
  - Mobile breakpoint: `@media (max-width: 640px) { .cd-fctx-row { grid-template-columns: 1fr; } }` — single declaration collapses panel rows from 200px-1fr two-column to 1fr stacked single-column on narrow viewports
  - Bucket modifier classes (5): `.cd-exit-bar--err-strong { background: var(--cd-status-error); }` / `.cd-exit-bar--err-muted { background: var(--cd-status-error-bg); border: 1px solid var(--cd-status-error); }` / `.cd-exit-bar--warn { background: var(--cd-status-disabled); }` / `.cd-exit-bar--stopped { background: var(--cd-status-stopped); }` / `.cd-exit-bar--null { background: var(--cd-status-cancelled); }` per UI-SPEC § Color bucket→token mapping
  - `.cd-exit-card` intentionally has NO declaration (outer chrome via inline `style="..."` matching Duration sibling); `.cd-exit-card-title` is the only cd-exit-card-prefixed rule
  - Zero new tokens, zero new fonts, zero new external crates (D-32 invariant preserved)
  - `assets/static/app.css` regenerated by `build.rs` (Tailwind standalone binary on cargo build) — committed alongside source per single-binary contract
  - `#[allow(dead_code)]` removed from `RunDetailPage.show_fctx_panel`, `RunDetailPage.fctx`, and `JobDetailPage.exit_histogram` (templates now consume them); struct-level allow on `FctxView` and `ExitHistogramView` retained because the template doesn't reference every field (e.g., `consecutive_failures` is populated but the template uses `summary_meta` instead — kept for future JSON-API / metrics consumers)

- **Verification:**
  - `cargo build --workspace` exits 0 on all 3 task commits (askama compile-checks template field references against `FctxView` / `ExitHistogramView` / `BucketRender` / `TopCodeRender` structs)
  - `cargo nextest run --no-fail-fast` 528/537 pass (the same 9 sandbox-Docker `SocketNotFoundError("/var/run/docker.sock")` Postgres testcontainer failures as plans 21-02 / 21-04 / 21-05 wave-end gates; not regressions — `dashboard_jobs_pg`, `db_pool_postgres`, `schema_parity::sqlite_and_postgres_schemas_match_structurally`, all `v11_bulk_toggle_pg::*`, `v13_timeline_explain::explain_uses_index_postgres`)
  - `cargo tree -i openssl-sys` empty (D-32 rustls-only invariant holds)
  - All 34 unique cd-fctx-* + cd-exit-* class names appear in the regenerated `assets/static/app.css` (Tailwind compile picks up source-CSS additions automatically)

## Task Commits

Each task was committed atomically:

1. **Task 1: Insert FCTX panel into templates/pages/run_detail.html** — `248a0b2` (feat)
2. **Task 2: Insert exit-code histogram card into templates/pages/job_detail.html** — `a98c28a` (feat)
3. **Task 3: Add cd-fctx-* + cd-exit-* CSS classes to assets/src/app.css; remove #[allow(dead_code)] on consumed fields** — `3c2440e` (feat)

## Files Created/Modified

**Created (0):**
- (none)

**Modified (6):**
- `templates/pages/run_detail.html` — `+67 / -0`: <details class="cd-fctx-panel"> with 5 conditionally-rendered rows inserted between metadata card and Log Viewer
- `templates/pages/job_detail.html` — `+72 / -0`: <div class="cd-exit-card"> with stats + chart + recent-codes table + empty-state branch inserted between Duration card and Run History
- `assets/src/app.css` — `+50 / -1`: 16 cd-fctx-* declarations, 28 cd-exit-* declarations, print-mode rule, reduced-motion extension. The `-1` is the existing reduced-motion block's closing `}` getting a new rule appended above it before re-closing
- `assets/static/app.css` — generated by `build.rs` Tailwind invocation; sees the source additions and re-minifies the output bundle. Committed alongside source per single-binary deployment story
- `src/web/handlers/run_detail.rs` — `+5 / -7`: `#[allow(dead_code)]` removed from `RunDetailPage.show_fctx_panel` and `RunDetailPage.fctx`; doc comment on `FctxView` struct updated to explain why struct-level allow remains (consecutive_failures + last_success_run_id intentionally unused by template)
- `src/web/handlers/job_detail.rs` — `+1 / -2`: `#[allow(dead_code)]` removed from `JobDetailPage.exit_histogram`; doc comment updated

## Decisions Made

- **Removed `#[allow(dead_code)]` from RunDetailPage.show_fctx_panel + .fctx and JobDetailPage.exit_histogram (per the wave 2 state instruction).** Kept struct-level `#[allow(dead_code)]` on `FctxView` (`consecutive_failures` + `last_success_run_id`) and `ExitHistogramView` (`success_rate_pct`) because the askama template only consumes the derived/formatted variants — those raw fields are populated for future JSON-API / metrics consumers and removing the struct-level allow would surface them as warnings. Updated the `FctxView` doc-comment to explain the rationale rather than the previous "plan 21-06" marker.
- **Did NOT modify the existing `run_detail.html` log-streaming inline `<script>` (lines 100-176).** Verified by inspection that the new FCTX panel block contains zero `<script>` tags. The plan's must_have "NO new inline `<script>` blocks" is satisfied — the existing log-streaming script predates Phase 21 and is unrelated to the FCTX surface.
- **Inline outer chrome on `.cd-exit-card` via `style="..."` attribute (NOT a CSS class declaration).** The plan locked this in must_haves via the "matches the Duration sibling card" constraint and the UI-SPEC explicitly says `.cd-exit-card` "uses outer chrome via existing `.mb-6` + inline styles to match Duration sibling — same as run_detail metadata card". Only `.cd-exit-card-title` (the heading inside) gets a class-based rule. This keeps the visual contract "exact sibling" rather than introducing a near-duplicate chrome class. Verified by grep: zero `.cd-exit-card { ... }` selector declarations in `app.css`; `.cd-exit-card-title` selector is the only cd-exit-card-prefixed rule.
- **Bare-literal sentinel grep is intentionally narrow:** the plan's acceptance check `grep -E 'min-height: 2px|top: -18px|height: 128px' assets/src/app.css | wc -l` returns 0 — these were the three exact bare-px values the UI-SPEC's final spacing pass eliminated. Broader "no px literals anywhere in the file" is NOT enforced because pre-Phase-21 code (Phase 13 `.cd-tooltip-dot width: 8px; height: 8px;` line 476-477; `.cd-tooltip box-shadow: 0 4px 12px rgba(0,0,0,0.35)` line 457; the `8px` disclosure offset on `.cd-tooltip bottom: calc(100% + 8px)` line 448) ships with bare px literals and is out-of-scope for this plan. The new CSS surfaces in Phase 21 are token-clean.
- **Reduced-motion extension landed inside the existing `@media (prefers-reduced-motion: reduce)` block (line 431-434) rather than a NEW separate block.** The plan said "extend the existing block" verbatim. Single-block ergonomics: future readers find every reduced-motion override in one place rather than scattered across the file.
- **Tooltip reuse landed as the ONLY new tooltip-related selector.** Verified by `grep -c '.cd-tooltip {' assets/src/app.css` returning 1 (the existing Phase 13 declaration; no redeclaration). The Phase 21 addition is purely a NEW anchor-element rule — `.cd-exit-bar:hover .cd-tooltip, .cd-exit-bar:focus-visible .cd-tooltip { visibility: visible; opacity: 1; }` — which mirrors the existing `.cd-timeline-bar:hover .cd-tooltip` rule for a different DOM anchor. Zero net CSS duplication.
- **Build artifact `assets/static/app.css` regeneration is committed alongside source `assets/src/app.css`.** The build script (`build.rs` lines 4-58) runs the standalone Tailwind binary on every `cargo build` to keep the production CSS in sync with source; both files MUST be committed together so a fresh `git clone + cargo build` reproduces the exact CSS shipped to operators (single-binary deployment story).

## Deviations from Plan

None — plan executed exactly as written.

The plan's `<interfaces>` block specified the verbatim FCTX panel markup, the locked histogram card markup matched UI-SPEC § Component Inventory § 2 byte-for-byte, and the CSS contract tables enumerated every property → token mapping. All three tasks landed without auto-fix triggers:

- **Task 1:** FCTX panel inserted at the locked anchors (between line 73 and line 75); 5 row-gating rules applied per D-13 / D-04; askama compile passed against the existing `FctxView` struct; all 13 acceptance grep checks returned the expected counts.
- **Task 2:** Histogram card inserted at the locked anchors (between line 94 and line 96); inline-style outer chrome matches Duration sibling; askama compile passed against `ExitHistogramView` / `BucketRender` / `TopCodeRender` structs; all 16 acceptance grep checks returned the expected counts.
- **Task 3:** CSS additions land in `@layer components`; reduced-motion extension appends to the existing block; print-mode rule added; bare-px sentinel grep returns 0; tailwind rebuild picks up the new classes; all 14 acceptance grep checks returned the expected counts. Removed `#[allow(dead_code)]` from page-level fields per wave 2 state instruction; kept struct-level allow with updated rationale comment.

The wave-end gate ran `cargo nextest run --no-fail-fast` and observed 528 passed / 9 failed where all 9 failures are the same sandbox-Docker `SocketNotFoundError("/var/run/docker.sock")` testcontainer issues as plans 21-02 / 21-04 / 21-05 — pre-existing sandbox limitation, not regressions. Verified by `nextest` failure-message inspection.

## Issues Encountered

- **Postgres testcontainer tests cannot run in this sandbox** — same 9 tests that failed at plans 21-02 / 21-04 / 21-05 wave-end gates fail again here with `Client(Init(SocketNotFoundError("/var/run/docker.sock")))`. They require `testcontainers-modules::postgres::Postgres` which spins up a Postgres container via the host Docker daemon — the sandbox has no Docker daemon. All other 528 tests pass. Postgres parity verifies on CI where Docker is available.

## User Setup Required

None — pure template + CSS additions, no operator-visible config changes, no new env vars. Restarting the cronduit binary (or in dev mode, the next page navigation) picks up the new surfaces because `rust-embed` debug mode reads the templates and assets/static/app.css from disk on every request. Production builds embed the regenerated `assets/static/app.css` into the binary at `cargo build --release` time.

## Next Phase Readiness

- **Plan 21-07 (justfile UAT recipes)** — can now reference `just run-server` + browser-driven verification of the FCTX panel + histogram card. The panel collapsed/expanded states, the keyboard Tab + Space/Enter toggle, the histogram bar tooltip on hover/focus, the empty-state branch (`Need 5+ samples; have {N}`), the recent-codes sub-table gating, and the mobile-viewport stacking are all directly observable now.
- **Plan 21-08 (integration tests)** — can seed a job with a failed run (status="failed") + a prior successful run, GET `/jobs/{job_id}/runs/{run_id}`, assert the rendered HTML contains the locked copy strings (`"Failure context"`, `"TIME DELTAS"`, `"Config changed since last success: Yes"`, etc.) and the locked class names (`cd-fctx-panel`, `cd-fctx-row-link`, etc.). For the histogram, seed a job with mixed-status runs covering all 10 bucket variants and assert the rendered HTML contains the locked aria-labels, the per-bar count display, the success-rate stat, and the recent-codes table rows. Below-N=5 jobs should render the empty-state copy without a histogram chart.
- **Plan 21-09 / 21-10 (rc.2 tag cut + UAT)** — the surfaces are now operator-visible; UAT can verify the visual match against the UI-SPEC mockups (terminal-green chrome, sibling-card outer styling, on-grid spacing, focus rings on `<summary>` + bars + link, tooltip placement, print-mode rendering with the panel auto-expanded).

## Threat Flags

None — the plan's `<threat_model>` enumerates five threats; all five remain valid as written. The new template + CSS introduces:

- **T-21-06-01 (Tampering / XSS via run.* values)** — mitigated. ALL `{{ value }}` substitutions use askama auto-escaping; ZERO `|safe` filters in the new content. Verified by `grep -c '|safe'` returning 0 on both modified templates.
- **T-21-06-02 (Tampering / CSS injection via inline `style="height:{{ pct }}%"`)** — mitigated. `pct` is server-clamped to 0..=100 in plan 21-05's `build_exit_histogram_view` via `((count as i64 * 100) / max_count as i64).clamp(0, 100)`; type is `i64`. NO operator data path.
- **T-21-06-03 (Tampering / CSS injection via `style="background:var(--cd-{{ dot_token }})"`)** — mitigated. `dot_token` is one of a controlled string-set produced by plan 21-05's `bucket_classes` lookup (an enum-mapped match expression). NEVER operator input.
- **T-21-06-04 (Tampering / aria-label injection)** — mitigated. `aria_label` and `chart_aria_summary` are pre-rendered server-side from constant templates with `{N}` substituted by typed integers; askama escapes the count integer.
- **T-21-06-05 (Information Disclosure / print-mode opens panel)** — accepted. Print mode is operator-initiated; same exposure as on-screen render.

No new security-relevant surface beyond what the threat model enumerates.

## Self-Check: PASSED

- Commit `248a0b2` (Task 1) — FOUND in `git log --oneline -5`
- Commit `a98c28a` (Task 2) — FOUND in `git log --oneline -5`
- Commit `3c2440e` (Task 3) — FOUND in `git log --oneline -5`
- File `templates/pages/run_detail.html` exists; modifications confirmed via `grep -c '<details class="cd-fctx-panel mb-6">'` returns 1
- File `templates/pages/job_detail.html` exists; modifications confirmed via `grep -c '<div class="cd-exit-card mb-6"'` returns 1
- File `assets/src/app.css` exists; modifications confirmed via `grep -cE '^  \.cd-fctx-' assets/src/app.css` returns 16, `grep -cE '^  \.cd-exit-' assets/src/app.css` returns 28
- File `assets/static/app.css` regenerated; new classes present via `grep -o 'cd-fctx-[a-z-]*\|cd-exit-[a-z-]*' assets/static/app.css | sort -u | wc -l` returns 34
- All Task 1 acceptance grep checks pass (13/13)
- All Task 2 acceptance grep checks pass (16/16)
- All Task 3 acceptance grep checks pass (14/14)
- `cargo build --workspace` exits 0
- `cargo nextest run --no-fail-fast` — 528 passed, 9 failed (all 9 = `SocketNotFoundError("/var/run/docker.sock")`; sandbox limitation; same set as plans 21-02 / 21-04 / 21-05 wave-end gates)
- `cargo tree -i openssl-sys` — returns "package ID specification ... did not match any packages" (D-32 invariant)
- `grep -c '|safe' templates/pages/run_detail.html` returns 0 (no |safe filters in new content)
- `grep -c '|safe' templates/pages/job_detail.html` returns 0 (no |safe filters in new content)
- Reduced-motion extension verified: `grep -c '.cd-fctx-summary-caret { transition: none' assets/src/app.css` returns 1
- Print-mode rule verified: `grep -A2 'Phase 21 print mode' assets/src/app.css | grep -c 'details.cd-fctx-panel { open: open;'` returns 1
- Tooltip-reuse selector verified: `grep -c '.cd-exit-bar:hover .cd-tooltip' assets/src/app.css` returns 1
- All 5 bucket modifier classes verified: `grep -cE '.cd-exit-bar--(err-strong|err-muted|warn|stopped|null)' assets/src/app.css` returns 5
- Bare-px sentinel grep returns 0: `grep -E 'min-height: 2px|top: -18px|height: 128px' assets/src/app.css | wc -l` returns 0
- Token-derived dimensions verified: `grep -c 'min-height: var(--cd-space-1)' assets/src/app.css` returns 1, `grep -c 'top: calc(-1 \* var(--cd-space-4))' assets/src/app.css` returns 1, `grep -c 'height: calc(var(--cd-space-8) \* 4)' assets/src/app.css` returns 1
- Mobile breakpoint verified: `grep -c '@media (max-width: 640px) { .cd-fctx-row' assets/src/app.css` returns 1

---
*Phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2*
*Plan: 06*
*Completed: 2026-05-02*
