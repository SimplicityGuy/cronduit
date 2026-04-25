---
phase: 13
plan: 01
subsystem: web-observability-foundation
tags: [observability, foundation, percentile, css-tokens, stats, format]
requirements: [OBS-04, OBS-05]
wave: 1
depends_on: []
dependency_graph:
  requires: []
  provides:
    - "`cronduit::web::stats::percentile(samples, q)` (nearest-rank, 1-indexed) — consumed by plan 13-03 Duration card"
    - "`cronduit::web::format::format_duration_ms_floor_seconds(ms)` — consumed by plans 13-03 (Duration card), 13-04 (sparkline tooltip), 13-05 (timeline tooltip)"
    - "CSS selector family cd-sparkline-* (9 selectors) — consumed by plan 13-04"
    - "CSS selector family cd-pill-* (5 selectors) — consumed by plans 13-02 (timeline nav), 13-05 (window toggle)"
    - "CSS selector family cd-timeline-* (19 selectors + pulsing + keyframes + reduced-motion) — consumed by plan 13-05"
    - "CSS selector family cd-tooltip-* (5 selectors including ::after caret) — consumed by plan 13-05"
    - "CSS custom properties --cd-status-cancelled, --cd-status-cancelled-bg (6-status mapping completion) — consumed by plans 13-04 and 13-05"
    - "CSS custom properties --cd-timeline-bar-min-width / --cd-timeline-bar-height / --cd-timeline-row-height / --cd-timeline-label-width / --cd-timeline-axis-height — consumed by plan 13-05"
  affects: []
tech-stack:
  added: []
  patterns:
    - "Pure-Rust percentile over Vec::to_vec + sort_unstable + ceil-rank index (OBS-05 structural parity — no SQL percentile_cont)"
    - "Inline #[cfg(test)] mod tests pattern (matches src/web/format.rs shape)"
    - "Additive CSS layering inside existing @layer components block (matches cd-badge--{status} variant family pattern)"
key-files:
  created:
    - "src/web/stats.rs (83 lines: 1 pub fn + 8 #[test] cases)"
  modified:
    - "src/web/format.rs (+36 lines: 1 new pub fn + 1 new #[test])"
    - "src/web/mod.rs (+1 line: pub mod stats;)"
    - "assets/src/app.css (+225 lines, 0 deletions)"
decisions:
  - "Inline #[cfg(test)] mod tests inside src/web/stats.rs (no separate tests/v13_stats_percentile.rs) — matches src/web/format.rs analog; Research Open Question #3 resolution"
  - "Added format_duration_ms_floor_seconds as a NEW helper rather than mutating the shipped format_duration_ms — preserves Phase 11 Run History column rendering (Research A3 / Open Question #2)"
  - "Deviation from plan's literal instruction: cargo fmt reorders pub mod stats; to AFTER pub mod handlers; (true lexicographic order h < s). Fixed in follow-up style commit 1dbb627."
metrics:
  duration: "~20 minutes"
  completed: "2026-04-21"
  tasks_completed: 3
  tests_added: 9
  lines_added: 345
  commits: 4
---

# Phase 13 Plan 01: Observability Foundations Summary

One-liner: Shipped the percentile helper (nearest-rank, 1-indexed, pure-Rust per OBS-05), a floor-seconds duration formatter variant that preserves the shipped formatter, and the complete Phase 13 CSS foundation (7 new custom properties + 5 selector families + pulse keyframes) that waves 2-3 will consume verbatim.

## Scope

Plan 13-01 is the foundation wave for Phase 13 Observability Polish. It deliberately lands all shared primitives in a single isolated plan so that downstream waves (Sparkline, Duration card, Timeline page) can reference them without touching any shared file. This plan is load-bearing for plans 13-03, 13-04, and 13-05.

## Tasks Completed

### Task 1 — `percentile()` helper (OBS-04 D-19)

- **Commit:** `3fad8ff feat(13-01): add percentile helper with inline tests (OBS-04, OBS-05)`
- **Files:** `src/web/stats.rs` (new, 83 lines), `src/web/mod.rs` (+1 line)
- **Result:** `pub fn percentile(samples: &[u64], q: f64) -> Option<u64>` implemented per D-19 (nearest-rank, 1-indexed). 8 inline tests cover T-V11-DUR-01..04 plus boundary cases (empty, single, q=0, q=1, reverse-sorted parity, 100-sample distribution).
- **Verification:** `cargo nextest run --lib -E 'test(stats::tests)'` — 8 passed, 0 failed.

### Task 2 — `format_duration_ms_floor_seconds()` helper

- **Commit:** `810bbba feat(13-01): add format_duration_ms_floor_seconds helper (A3, Open Q#2)`
- **Files:** `src/web/format.rs` (+36 lines)
- **Result:** New helper emits `"42s"` (integer seconds) in sub-minute range; shipped `format_duration_ms` still emits `"42.0s"` verbatim. Phase 11 Run History column output unchanged — regression canary green.
- **Verification:** `cargo nextest run --lib -E 'test(format::tests)'` — 2 passed (original + new).

### Task 3 — Phase 13 CSS tokens + selectors

- **Commit:** `5c7ac13 feat(13-01): add Phase 13 CSS tokens and selectors for sparkline, timeline, pill, tooltip`
- **Files:** `assets/src/app.css` (+225 lines, 0 deletions)
- **Result:** 7 new CSS custom properties declared (2 color + 5 layout-scalar), 5 Phase 13 selector families added inside `@layer components` (Sparkline, Pill, Timeline, Pulse, Tooltip, Empty). No shipped selector modified.
- **Verification:** `cargo build --lib` green; all acceptance-criteria greps pass.

### Extra — cargo fmt alignment

- **Commit:** `1dbb627 style(13-01): reorder pub mod declarations per cargo fmt`
- **Rationale:** Plan said "alphabetically between `format` and `handlers`" but cargo fmt enforces strict lexicographic order (`handlers` < `stats`). Fixed to match fmt; no semantic change.

## Files Touched

| File                    | Status   | Lines (delta) | Purpose                                                                |
|-------------------------|----------|---------------|------------------------------------------------------------------------|
| `src/web/stats.rs`      | Created  | +83           | Pure-Rust `percentile()` helper + 8 inline tests (OBS-04, OBS-05)      |
| `src/web/format.rs`     | Modified | +36           | New `format_duration_ms_floor_seconds()` helper + regression test      |
| `src/web/mod.rs`        | Modified | +1            | `pub mod stats;` declaration (alphabetically after `handlers`)         |
| `assets/src/app.css`    | Modified | +225          | 7 new tokens + 5 selector families (sparkline/pill/timeline/pulse/tooltip) |

Total: 1 file created, 3 files modified. +345 lines, 0 deletions.

## Test Counts

| Test group                      | Before | After | Delta |
|---------------------------------|--------|-------|-------|
| `web::stats::tests::*`          | 0      | 8     | +8    |
| `web::format::tests::*`         | 1      | 2     | +1    |
| **Total new tests**             |        |       | **+9**|

All tests pass on SQLite in-memory. Tests are pure-compute / formatting only — no database, no integration harness, no container fixtures.

```
cargo nextest run --lib -E 'test(stats::tests) + test(format::tests)'
     Summary [0.016s] 10 tests run: 10 passed, 184 skipped
```

## CSS Selectors Added (verbatim list for next-plan executors)

### Sparkline family (plan 13-04 consumer)
- `.cd-sparkline` — grid container (20 × 6px columns, 14px height)
- `.cd-sparkline-cell` — base cell style (14px height, radius-sm)
- `.cd-sparkline-cell--success` — green (shipped token `--cd-status-active`)
- `.cd-sparkline-cell--failed` — red (shipped `--cd-status-error`)
- `.cd-sparkline-cell--timeout` — amber (shipped `--cd-status-disabled`)
- `.cd-sparkline-cell--cancelled` — grey-green (NEW `--cd-status-cancelled`)
- `.cd-sparkline-cell--stopped` — slate (shipped `--cd-status-stopped`)
- `.cd-sparkline-cell--empty` — transparent border-only placeholder
- `.cd-sparkline-badge` — muted numeric badge (95% / —)

### Pill toggle family (plans 13-02, 13-05)
- `.cd-pill-group` — inline-flex container
- `.cd-pill` — anchor-styled pill button
- `.cd-pill:hover` — hover state
- `.cd-pill--active` — active variant (text-accent, weight 700)
- `.cd-pill:focus-visible` — keyboard focus ring (project `--cd-green-dim` convention)

### Timeline family (plan 13-05 consumer)
- `.cd-timeline` — outer container (min-width 640px, overflow-x auto)
- `.cd-timeline-axis` — top axis strip (height from `--cd-timeline-axis-height`)
- `.cd-timeline-tick` — positioned tick label
- `.cd-timeline-row` — grid row (`--cd-timeline-label-width` | 1fr, height `--cd-timeline-row-height`)
- `.cd-timeline-row:last-child` — border override
- `.cd-timeline-row-label` — job-name anchor (text-accent, ellipsis)
- `.cd-timeline-row-label:hover` — underline on hover
- `.cd-timeline-row-stripe` — positioned lane (sunken background)
- `.cd-timeline-bar` — positioned bar anchor (min-width from token, position relative)
- `.cd-timeline-bar:hover` — brightness filter
- `.cd-timeline-bar:focus-visible` — keyboard focus ring
- `.cd-timeline-bar--success` | `--failed` | `--timeout` | `--cancelled` | `--stopped` | `--running` — 6 status colorways

### Pulse animation (D-11)
- `@keyframes cd-pulse` — opacity 1 → 0.7 → 1 cycle
- `.cd-timeline-bar--pulsing` — 2s ease-in-out infinite animation
- `@media (prefers-reduced-motion: reduce) .cd-timeline-bar--pulsing` — override to static opacity 1

### Rich tooltip (D-09)
- `.cd-tooltip` — positioned popover (visibility-hidden default)
- `.cd-timeline-bar:hover .cd-tooltip`, `.cd-timeline-bar:focus-visible .cd-tooltip` — show state
- `.cd-tooltip-row` — row display
- `.cd-tooltip-row + .cd-tooltip-row` — sibling spacing
- `.cd-tooltip-dot` — 8px status indicator circle
- `.cd-tooltip::after` — caret (6px transparent border + border-top `--cd-border`)

### Empty-window block (D-14)
- `.cd-timeline-empty` — centered message block (padding `--cd-space-8`)
- `.cd-timeline-empty p + p` — sibling paragraph spacing

### CSS Custom Properties Added

Dark (`:root`):
- `--cd-status-cancelled: #7a8f80`
- `--cd-status-cancelled-bg: rgba(122, 143, 128, 0.12)`
- `--cd-timeline-bar-min-width: 2px`
- `--cd-timeline-bar-height: 20px`
- `--cd-timeline-row-height: 32px`
- `--cd-timeline-label-width: 200px`
- `--cd-timeline-axis-height: 24px`

Light (`[data-theme="light"]` AND `@media (prefers-color-scheme: light) :root:not([data-theme])`):
- `--cd-status-cancelled: #5a6b60`
- `--cd-status-cancelled-bg: rgba(90, 107, 96, 0.08)`

(Layout-scalar tokens intentionally declared only in `:root` — theme-invariant per plan.)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Module declaration order did not satisfy cargo fmt**

- **Found during:** Task 3 verification (`cargo fmt --check`)
- **Issue:** Plan 13-01 Task 1 instructed placing `pub mod stats;` "alphabetically between `pub mod format;` and `pub mod handlers;`". Literally followed, this produced:
  ```rust
  pub mod format;
  pub mod stats;
  pub mod handlers;
  ```
  But cargo fmt enforces strict lexicographic order where `handlers` < `stats`, so the intended alphabetical ordering is:
  ```rust
  pub mod format;
  pub mod handlers;
  pub mod stats;
  ```
  The plan's instruction conflicted with its own acceptance criterion (`cargo fmt --check` passing is implied by CLAUDE.md "clippy + fmt gate on CI").
- **Fix:** Ran `cargo fmt` to reorder. Wired as a separate `style(13-01): reorder pub mod declarations per cargo fmt` commit to keep Task 1's semantic commit clean.
- **Files modified:** `src/web/mod.rs` (1 line moved)
- **Commit:** `1dbb627`

### No other deviations

- Percentile helper implementation matches the verbatim code block supplied in plan 13-01 Task 1 action section.
- Formatter helper implementation matches the verbatim code block supplied in plan 13-01 Task 2 action section.
- CSS additions match the UI-SPEC-referenced token list and selector family layout verbatim; every hex literal, every new token name, every new selector name is as specified.
- No auth gates hit (this plan has no authentication surface — all changes are pure internal helpers or static CSS).

## Design Fidelity Check

- All new CSS uses either shipped `--cd-*` tokens, newly-declared `--cd-status-cancelled*` / `--cd-timeline-*` tokens, or sub-4px literals that fall within UI-SPEC's enumerated allowed sub-4px exceptions (6px sparkline column, 14px sparkline height, 2px bar min-width, 20px bar height, 32px row height, 200px label width, 24px axis height, 6px tooltip caret, 8px tooltip offset, 2px focus outline, 8px tooltip dot, 36px badge min-width).
- Focus-visible rings on `.cd-pill` and `.cd-timeline-bar` use the project-standard 3-line pattern (`outline: none; box-shadow: 0 0 0 2px var(--cd-green-dim); border-color: var(--cd-border-focus)`) seen on every existing interactive element (`cd-btn-primary`, `cd-btn-secondary`, `cd-btn-stop`).
- Cancelled color `#7a8f80` (dark) matches the shipped `--cd-text-secondary` token exactly — maintains the "grey-green muted" family chroma without introducing a novel hue.
- `@media (prefers-reduced-motion: reduce)` override on the pulse animation satisfies accessibility per WCAG 2.1 SC 2.3.3.

## Threat Model Coverage

Plan 13-01's threat register (4 rows) had no `mitigate` dispositions:
- T-13-01-01 Tampering on `percentile()` — accepted (pure fn, no external input path)
- T-13-01-02 DoS on sort cost — accepted (N bounded ≤100 by consumer)
- T-13-01-03 Info disclosure via CSS tokens — accepted (public by design)
- T-13-01-04 Repudiation — n/a (no audit-relevant action)

No mitigation code required. Consumer plans (13-03 in particular) are responsible for enforcing the N<20 threshold BEFORE calling `percentile()` — this is explicit in D-21 and documented in the module-level doc comment of `src/web/stats.rs`.

## Verification Commands Run

```bash
# Unit tests (Task 1 + Task 2)
cargo nextest run --lib -E 'test(stats::tests)'        # 8 passed
cargo nextest run --lib -E 'test(format::tests)'       # 2 passed
cargo nextest run --lib -E 'test(stats::tests) + test(format::tests)'  # 10 passed

# Build + lint
cargo build --lib                                      # success
cargo clippy --lib -- -D warnings                      # success
cargo fmt --check                                      # success (after 1dbb627)

# CSS acceptance criteria greps (Task 3)
grep -c 'cd-sparkline-cell--' assets/src/app.css       # 6 (4 status + cancelled + stopped + empty = 6)
grep -c 'cd-timeline-bar--' assets/src/app.css         # 8 (6 status + pulsing + reduced-motion override)
grep -q '@keyframes cd-pulse' assets/src/app.css       # present
grep -q '.cd-tooltip::after' assets/src/app.css        # present
grep -q '.cd-timeline-empty' assets/src/app.css        # present
```

## Self-Check: PASSED

All claimed artifacts verified present on disk and in git history:

- `src/web/stats.rs` — FOUND (83 lines, 8 tests)
- `src/web/format.rs` — FOUND (70 lines, +36 delta, 2 tests)
- `src/web/mod.rs` — FOUND (contains `pub mod stats;` after `pub mod handlers;`)
- `assets/src/app.css` — FOUND (+225 lines, 0 deletions vs base `cfacd3d`)

All commit hashes verified present in git history:

- `3fad8ff` — FOUND
- `810bbba` — FOUND
- `1dbb627` — FOUND
- `5c7ac13` — FOUND

## Known Stubs

None. Every change in this plan is either a fully-wired helper (stats, format) or additive CSS that downstream plans will reference by name. The helpers are not yet consumed by any view code (that is the work of plans 13-03, 13-04, 13-05) — but they are fully implemented, tested, and callable. No placeholder values, no hardcoded empty data flows to any UI.

## Handoff to Next Plans

- **Plan 13-02** (Timeline nav + query types): may reference `.cd-pill`/`.cd-pill--active`/`.cd-pill-group` for the nav pill, and `.cd-timeline-empty` for the empty-window block.
- **Plan 13-03** (Duration card on job_detail): MUST enforce the N<20 threshold before calling `cronduit::web::stats::percentile()`. MUST use `cronduit::web::format::format_duration_ms_floor_seconds()` — NOT the shipped `format_duration_ms` — for the p50/p95 chip values.
- **Plan 13-04** (Dashboard sparkline + success-rate badge): references `.cd-sparkline*` selectors verbatim. Uses `format_duration_ms_floor_seconds` for the cell `title` attribute.
- **Plan 13-05** (Timeline page): references `.cd-timeline*`, `.cd-tooltip*`, `@keyframes cd-pulse`, the new layout-scalar custom properties, and the new `--cd-status-cancelled*` color tokens. Uses `format_duration_ms_floor_seconds` for tooltip duration text.
