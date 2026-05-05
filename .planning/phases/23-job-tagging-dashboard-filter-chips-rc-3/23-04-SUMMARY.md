---
phase: 23-job-tagging-dashboard-filter-chips-rc-3
plan: 04
subsystem: ui
tags: [css, design-system, tagging, components, a11y, tailwind, focus-ring, reduced-motion, print]

# Dependency graph
requires:
  - phase: 23-02
    provides: "DashboardJob.tags field + AND-chained tags LIKE filter SQL — V-01..V-04 GREEN; chips render against a real query surface"
  - phase: 23-03
    provides: "axum_extra::Query<DashboardParams> + handler-side fleet-tag fold + active-set intersect — V-05/V-07 GREEN; the view-model fields cd-tag-chip-* will consume by name in 23-05"
  - phase: 21
    provides: "cd-fctx-* / cd-exit-* @layer components namespacing precedent + existing @media (prefers-reduced-motion: reduce) and @media print blocks (extended in this plan)"
  - phase: 22
    provides: "TAG-04 charset regex + TAG-05 substring-collision validators — guarantee operator-supplied tag values can never break CSS or HTML in chip rendering (T-23-04-01 disposition: accept)"
provides:
  - "cd-tag-chip-strip + cd-tag-chip + cd-tag-chip--active + cd-tag-chip--inactive component family in @layer components"
  - "Triple-channel a11y active-state encoding (--cd-green-dim background + --cd-text-accent border + --cd-text-accent label color + font-weight: 700) — color-vision-deficient operators read state via weight + border darkness alone"
  - ":focus-visible rule emitting box-shadow: 0 0 0 2px var(--cd-green-dim) on both chip variants — matches Phase 13/14/21 focus-ring pattern"
  - "min-height: 40px + padding: var(--cd-space-2) var(--cd-space-3) — WCAG 2.2 AAA touch target ≥ 44px"
  - "@media (prefers-reduced-motion: reduce) extended single-line — .cd-tag-chip { transition: none; }"
  - "@media print extended single-line — .cd-tag-chip-strip { display: none; }"
affects: [23-05, 23-06, 23-07, 23-08]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Mirror P21 namespacing precedent — new tag-chip family lives in @layer components alongside cd-fctx-* and cd-exit-* (UI-SPEC § Component Inventory § CSS contract)"
    - "Single-line append discipline for shared @media blocks — never duplicate @media (prefers-reduced-motion: reduce) or @media print; extend the existing block in place"
    - "Three-channel a11y signal encoding — color is never the sole signal; weight + border + color compose"
    - "Pill-radius idiom for interactive filter chips (literal 9999px) vs square-corner cd-badge for read-only labels — distinct primitives, distinct radii"
    - "Documented inline literal fallbacks when a UI-SPEC-claimed token is absent from :root — avoids a stealth new-token declaration while surfacing the gap for follow-up"

key-files:
  created: []
  modified:
    - "assets/src/app.css — 11 new selector rules + 1 header comment + 1 fallback note in @layer components (between Phase 21 cd-exit-* and the print @media block); single-line append to reduced-motion + single-line append to print"
    - "assets/static/app.css — Tailwind standalone build regenerated this bundle automatically via build.rs (rust-embed picks it up at next compile)"

key-decisions:
  - "Use literal 40px and 9999px inline (not declare new --cd-* tokens) when UI-SPEC-claimed --cd-space-10 and --cd-radius-full are absent from :root — preserves the ZERO new tokens contract and surfaces the gap for follow-up token canonicalization"
  - "Place the new block AFTER the P21 cd-exit-* family and BEFORE the @media print block, inside the same @layer components { } parent — keeps the file in component-namespace order"
  - "Active state encodes across THREE channels (border, label color, font-weight) per UI-SPEC § Accessibility Contract — closes the only meaningful threat in the STRIDE register (T-23-04-05 a11y)"

patterns-established:
  - "Tag-chip CSS contract: every value resolves to an existing --cd-* token OR an inline literal whose absent-token gap is documented adjacent — ZERO stealth token declarations"
  - "Touch-target math via min-height: 40px + padding-block: var(--cd-space-2) — composes to ≥ 44px effective height per WCAG 2.2 AAA without reaching for new tokens"

requirements-completed: [TAG-06, TAG-08]

# Metrics
duration: ~14 min
completed: 2026-05-05
---

# Phase 23 Plan 04: Tag Filter Chip CSS Primitive Summary

**`cd-tag-chip-*` family added to `@layer components` with triple-channel a11y active state, WCAG 2.2 AAA touch targets, and single-line extensions to the existing reduced-motion + print @media blocks — zero new design tokens, zero hex literals, build green.**

## Performance

- **Duration:** ~14 min
- **Started:** 2026-05-05T02:12:30Z (approx — read context + plan, then edits)
- **Completed:** 2026-05-05T02:26:15Z
- **Tasks:** 1
- **Files modified:** 2 (`assets/src/app.css` source + `assets/static/app.css` Tailwind-generated bundle)

## Accomplishments

- Added the `cd-tag-chip-strip`, `cd-tag-chip-strip[hidden]`, `cd-tag-chip`, `cd-tag-chip--inactive`, `cd-tag-chip--inactive:hover`, `cd-tag-chip--inactive:focus-visible`, `cd-tag-chip--active`, `cd-tag-chip--active:hover`, `cd-tag-chip--active:focus-visible` rules to `@layer components` per UI-SPEC § Component Inventory § CSS contract — 9 selector rules behind 4 logical class names.
- Triple-channel a11y active-state encoding: `--cd-green-dim` background + `--cd-text-accent` border + `--cd-text-accent` label + `font-weight: 700`. Closes T-23-04-05 (a11y; color-vision-deficient operators read state via weight + border darkness alone).
- `:focus-visible` rule emits `box-shadow: 0 0 0 2px var(--cd-green-dim)` on both variants — matches the Phase 13/14/21 focus-ring pattern verbatim. Inactive variant adds the hover-state recolor on focus (background/border/color) for high-contrast keyboard navigation.
- Hover differentiation: inactive chip darkens to `--cd-bg-hover` / `--cd-border` / `--cd-text-primary` (NO green on hover; hover is interactivity signal, not active signal); active chip uses `filter: brightness(1.1)` (sibling to `.cd-exit-bar:hover`).
- Touch target ≥ 44px via `min-height: 40px` + `padding: var(--cd-space-2) var(--cd-space-3)` (8px vertical / 12px horizontal). 40 + 8 + 8 = 56px effective tap height; horizontal padding 24px + label width naturally ≥ 44px wide. WCAG 2.2 AAA satisfied.
- `@media (prefers-reduced-motion: reduce)` block extended single-line with `.cd-tag-chip { transition: none; }` — NO duplicate block (`grep -c '@media (prefers-reduced-motion: reduce)' assets/src/app.css` returns 1).
- `@media print` block extended single-line with `.cd-tag-chip-strip { display: none; }` — NO duplicate block (`grep -c '@media print' assets/src/app.css` returns 1).
- Tailwind standalone build (via `build.rs`) regenerated `assets/static/app.css` automatically; chip classes present in the embedded bundle (`grep -c 'cd-tag-chip' assets/static/app.css` returns 1+ matches).

## Task Commits

Each task was committed atomically:

1. **Task 1: Add cd-tag-chip-* family to @layer components + extend reduced-motion + print blocks** — `e8ef4a9` (feat)

## Files Created/Modified

- `assets/src/app.css` — 11 new selector rules + 1 header comment + 1 fallback note inside `@layer components` (between the Phase 21 `cd-exit-*` block and the `@media print` block); reduced-motion `@media` block at line 431-435 single-line extended; print `@media` block (now at end of `@layer components`) single-line extended.
- `assets/static/app.css` — Tailwind standalone regenerated this bundle on `cargo build` via `build.rs`. `rust-embed` picks it up automatically at next compile (debug builds read from disk per `debug-embed = false`).

## Decisions Made

- **D1 (token gap surfacing):** UI-SPEC § Tokens — Existing Reuse Verified claims `--cd-space-10` (= 40px) and `--cd-radius-full` (= 9999px) are declared in `:root`. Verified by grep: NEITHER is declared. The `:root` block declares `--cd-space-1, 2, 3, 4, 6, 8, 12, 16` (no 10) and `--cd-radius-sm, md` (no full). Per the plan's documented fallback procedure for `--cd-space-10`, I substituted the literal `40px` inline with an adjacent comment surfacing the gap. Extended the same pattern to `--cd-radius-full` → literal `9999px`. ZERO new token declarations introduced; ZERO change to the design system at the token layer. Both gaps surfaced for follow-up canonicalization (see § Known Stubs / Token Gaps below).
- **D2 (insertion point):** New block placed AFTER the Phase 21 `cd-exit-*` block (at the end of the component primitives, just before the `@media print` block at the bottom of `@layer components`). Keeps the file in component-namespace order: `cd-fctx-*` → `cd-exit-*` → `cd-tag-chip-*`, then the two cross-cutting `@media` blocks (reduced-motion sits earlier in `@layer components` next to the timeline pulse keyframes; print is the final stanza inside the layer).
- **D3 (active state encoded across three channels):** Per UI-SPEC § Accessibility Contract, color is never the sole signal. Triple-channel encoding (border + label color + weight) makes the active state legible to color-vision-deficient operators via the bolder weight and darker border alone. Closes T-23-04-05 (a11y) in the threat register.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking / Token gap] UI-SPEC-claimed `--cd-radius-full` token is absent from `:root`**
- **Found during:** Task 1 (pre-edit verification grep `grep -n -- '--cd-radius-full' assets/src/app.css` returned no match; only `--cd-radius-sm` and `--cd-radius-md` are declared)
- **Issue:** UI-SPEC § Tokens — Existing Reuse Verified (line 340) explicitly claims `--cd-radius-full` is "already declared in `app.css` and consumed nowhere yet." That claim is false: the `:root` declaration block at lines 89-91 declares only `--cd-radius-sm: 4px` and `--cd-radius-md: 8px`. The plan's "ZERO new tokens" hard contract forbids declaring a new `--cd-radius-full`. The plan's documented fallback procedure (lines 175-177) explicitly addresses ONLY the `--cd-space-10` case (literal `40px` with adjacent comment); it does not address `--cd-radius-full`.
- **Fix:** Mirrored the plan's `--cd-space-10` fallback procedure for `--cd-radius-full`: used the literal `9999px` (the documented value of the missing token per UI-SPEC line 340) inline with an adjacent comment surfacing the gap. ZERO stealth new-token declarations; pill-radius semantics preserved exactly per UI-SPEC. Single combined fallback comment placed immediately above the new chip block: `/* Phase 23 — tokens --cd-space-10 and --cd-radius-full absent from :root; literal 40px (on 4px grid) and 9999px used inline awaiting token addition. ZERO new token declarations introduced; values match UI-SPEC § Tokens locked vocabulary. */`
- **Files modified:** `assets/src/app.css`
- **Verification:** `grep -E '^\+\s*--cd-' git diff` returns 0 (no new token declarations); `grep '9999px\|40px' assets/src/app.css` matches the chip rules with the documented adjacent comment; `cargo build --quiet` exits 0 (Tailwind accepts the literals); test compile gates green.
- **Committed in:** `e8ef4a9`

**2. [Rule 3 - Blocking / Token gap] UI-SPEC-claimed `--cd-space-10` token is absent from `:root`**
- **Found during:** Task 1 (pre-edit verification grep `grep -n -- '--cd-space-10' assets/src/app.css` returned no match; the `:root` spacing scale declares 1, 2, 3, 4, 6, 8, 12, 16 — no 10)
- **Issue:** UI-SPEC line 341 claims `--cd-space-10` exists "in the project scale at app.css." Verification: false. The plan anticipated this exact gap (lines 175-177) and pre-authorized the fallback: "fall back to `min-height: 40px` (one-time literal, documented inline as `Phase 23 — token --cd-space-10 absent; literal 40px on 4px grid awaiting token addition`) and surface the gap in the SUMMARY for follow-up."
- **Fix:** Applied the plan's pre-authorized fallback. Used `min-height: 40px` literal in `.cd-tag-chip` rule with the adjacent comment (combined with the `--cd-radius-full` fallback note above for a single deviation comment). Effective touch target unchanged (≥ 44px when combined with 8px vertical padding).
- **Files modified:** `assets/src/app.css`
- **Verification:** Same checks as deviation 1.
- **Committed in:** `e8ef4a9`

---

**Total deviations:** 2 auto-fixed (both Rule 3 — blocking token-gap fallbacks, both substituting literals for UI-SPEC-claimed-but-absent tokens)
**Impact on plan:** Both fallbacks were pre-authorized in spirit by the plan (line 177 explicitly addresses `--cd-space-10`; the same procedure trivially extends to `--cd-radius-full`). Both preserve the "ZERO new tokens" hard contract. Both surface the underlying gap for follow-up canonicalization without blocking Wave-3 progress. No scope creep.

## Issues Encountered

None during execution. The two token-gap fallbacks above were anticipated by the plan and handled per its documented procedure.

## Known Stubs / Token Gaps

NOT stubs in the conventional sense — these are token-layer drift between UI-SPEC and the actual `:root` declaration block, surfaced here for a later token canonicalization pass:

| Token | UI-SPEC claim (line ref) | Actual `:root` state | Phase 23-04 fallback |
|-------|--------------------------|----------------------|----------------------|
| `--cd-space-10` | Line 341: "in the project scale at app.css" | ABSENT (scale jumps 8 → 12) | Literal `40px` with adjacent fallback comment |
| `--cd-radius-full` | Line 340: "already declared in `app.css` and consumed nowhere yet" | ABSENT (only `sm` = 4px and `md` = 8px declared) | Literal `9999px` with adjacent fallback comment |

**Recommended follow-up (out-of-scope for Plan 23-04):** add the two missing token declarations to `:root` (and the matching `[data-theme="light"]` block if light-theme handling differs) in a future plan, then replace the two literals with `var(--cd-space-10)` and `var(--cd-radius-full)`. Candidate landing spot: a polish plan inside Phase 23 (e.g., 23-08 close-out or a follow-on token-canonicalization micro-plan), or a quick task. The literals on the 4px-grid with documented intent are safe to ship to rc.3 — operators see no visual difference, and the design system contract holds at the visual layer.

## User Setup Required

None — no external service configuration required. Pure CSS extension.

## Next Phase Readiness

- **Plan 23-05 (template chip strip insert)** — UNBLOCKED. The CSS classes exist by name in `assets/src/app.css` and in the regenerated `assets/static/app.css` bundle. Plan 23-05 references them verbatim from `templates/pages/dashboard.html` per the planned chip strip markup contract.
- **V-11 (`css_only_chip_no_inline_js`)** — currently in `todo!()` state (V-test scaffolding from Plan 23-01); will go GREEN once Plan 23-05 lands and the test asserts the chip class names appear in the rendered HTML.
- **Plan 23-06 (UAT recipes)** — blocking dep is 23-05, not 23-04. Visual confirmation by maintainer happens via `just uat-chips-render` once the chips render end-to-end.
- **rc.3 cut readiness** — chip CSS layer is feature-complete for v1.2.0-rc.3. No release-engineering touchpoints in this plan; `release.yml` / `cliff.toml` / `docs/release-rc.md` unchanged per D-15 / D-16.

---

## Self-Check: PASSED

Verified:

- `assets/src/app.css` exists and contains the new `cd-tag-chip-*` family — `grep -c 'cd-tag-chip' assets/src/app.css` returns 9+ matches (multiple selectors per logical class)
- `assets/static/app.css` regenerated and contains chip classes — `grep -c 'cd-tag-chip' assets/static/app.css` returns 1+
- Commit `e8ef4a9` exists in `git log --oneline` on `phase23/discuss`
- All four primary classes present (`cd-tag-chip-strip`, `cd-tag-chip` `{`, `cd-tag-chip--active`, `cd-tag-chip--inactive`)
- Active rule contains `var(--cd-text-accent)` (twice — border + color), `var(--cd-green-dim)` (background), `font-weight: 700` (triple-channel encoding present)
- Inactive rule contains `var(--cd-bg-surface-raised)`, `var(--cd-border-subtle)`, `var(--cd-text-secondary)`, `font-weight: 400`
- Focus-visible rule contains `box-shadow: 0 0 0 2px var(--cd-green-dim)` for both variants
- `cd-tag-chip-strip[hidden]` hidden-attribute rule present
- Reduced-motion EXTENDED (1 block, not 2) — append confirmed
- Print EXTENDED (1 block, not 2) — append confirmed
- ZERO new `--cd-*` token declarations — `git diff assets/src/app.css | grep -E '^\+\s*--cd-' | grep -v 'var(--cd-'` returns 0
- ZERO hex literals in chip rules — `git diff assets/src/app.css | grep -E '^\+.*\.cd-tag-chip.*#[0-9a-fA-F]{3,8}'` returns 0
- `cargo build --quiet` exits 0
- `cargo test --lib --no-run --quiet` exits 0
- `cargo test --test v12_tags_dashboard --no-run --quiet` exits 0
- HEAD on per-feature branch `phase23/discuss` (NOT `main`) — commit safety honored

---

*Phase: 23-job-tagging-dashboard-filter-chips-rc-3*
*Plan: 04*
*Completed: 2026-05-05*
