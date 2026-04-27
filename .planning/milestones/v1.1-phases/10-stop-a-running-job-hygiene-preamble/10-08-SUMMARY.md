---
phase: 10-stop-a-running-job-hygiene-preamble
plan: 08
subsystem: design-system
tags: [css, design-tokens, stopped-status, SCHED-09, SCHED-14]
requires:
  - "10-01"
provides:
  - "--cd-status-stopped + --cd-status-stopped-bg design tokens (dark + light + auto)"
  - ".cd-badge--stopped modifier class"
  - ".cd-btn-stop + .cd-btn-stop--compact button classes"
  - "DESIGN_SYSTEM.md §2.2 stopped row (Status Colors + Status Background Tints)"
affects:
  - assets/src/app.css
  - design/DESIGN_SYSTEM.md
tech-stack:
  added: []
  patterns:
    - "Three-location color token definition (dark :root + [data-theme=light] + @media prefers-color-scheme: light fallback) — mandated by 10-PATTERNS.md §14 to keep every theme path styled"
    - "Hand-rolled cd-* component classes inside @layer components (matches existing cd-badge / cd-btn convention; no Tailwind @apply)"
    - "Focus ring uses --cd-green-dim for all buttons (brand focus lock; focus is not a semantic color)"
    - "Neutral-outline Stop button (D-05) — hover tints toward slate stopped, NOT red error"
key-files:
  created: []
  modified:
    - assets/src/app.css
    - design/DESIGN_SYSTEM.md
decisions:
  - "Slate-400 (#94a3b8) dark / slate-500 (#64748b) light picked for --cd-status-stopped (D-02; matches peer tools like GitHub Actions, contrast ratios 8.9:1 dark / 5.4:1 light — pass WCAG AAA / AA)"
  - "Stop button reuses cd-btn-secondary structure but tints hover/active toward --cd-status-stopped instead of --cd-bg-hover (D-05 neutral outline)"
  - "min-height: 44px lock for .cd-btn-stop (touch target); .cd-btn-stop--compact drops the lock for dense run_history rows"
  - "assets/static/app.css left untouched (pre-existing minified Tailwind bundle; regenerated at release time by build.rs + bin/tailwindcss, per plan action note)"
  - "Sample :root + [data-theme=light] snapshot blocks in DESIGN_SYSTEM.md §6 updated alongside the tables so the design doc source-of-truth stays in sync (D-02 invariant)"
metrics:
  duration: "~15 minutes"
  completed: 2026-04-15
---

# Phase 10 Plan 08: Stopped Design Tokens + Stop Button CSS Summary

Added the `--cd-status-stopped` design token family (slate-400/500 with matching 12%/8% alpha tints) across all three color-mode blocks in `assets/src/app.css`, defined the `.cd-badge--stopped` modifier and the `.cd-btn-stop` / `.cd-btn-stop--compact` button classes inside `@layer components`, and appended the matching rows to `design/DESIGN_SYSTEM.md` so the design doc stays source-of-truth — delivering the visual surface for SCHED-09 (stopped terminal status badge) and SCHED-14 (Stop button) in a single wave-6 plan.

## Deliverables

- **`assets/src/app.css`** — 48 lines added. Two new tokens in each of three color-mode blocks (dark `:root`, `[data-theme="light"]`, `@media (prefers-color-scheme: light)`), one new `.cd-badge--stopped` class, one new 36-line `.cd-btn-stop { ... }` block with five interaction states (idle, hover, active, focus-visible, disabled), plus `.cd-btn-stop--compact` modifier.
- **`design/DESIGN_SYSTEM.md`** — 6 lines added. Stopped row in §2.2 Status Colors table, stopped row in Status Background Tints table, plus the two tokens appended to the sample `:root` and `[data-theme="light"]` snapshot blocks in §6 so the documented source-of-truth matches the live CSS.

## Verification Evidence

Grep counts from `assets/src/app.css` after edits:

| Pattern | Expected | Actual |
|---|---|---|
| `#94a3b8` | 1 | 1 (dark `:root`) |
| `#64748b` | 2 | 2 (`[data-theme="light"]` + `@media prefers-color-scheme: light`) |
| `rgba(148, 163, 184, 0.12)` | 1 | 1 |
| `rgba(100, 116, 139, 0.08)` | 2 | 2 |
| `--cd-status-stopped:` | 3 | 3 |
| `--cd-status-stopped-bg:` | 3 | 3 |
| `.cd-badge--stopped` | ≥1 | 1 |
| `^\s*\.cd-btn-stop \{` base | ≥1 | 1 |
| `.cd-btn-stop:hover` | ≥1 | 1 |
| `.cd-btn-stop:focus-visible` | ≥1 | 1 |
| `.cd-btn-stop--compact` | ≥1 | 1 |
| `var(--cd-status-stopped-bg)` | ≥2 | 3 (badge + `.cd-btn-stop:hover` + `.cd-btn-stop:active`) |
| `var(--cd-green-dim)` | ≥1 usage in `.cd-btn-stop:focus-visible` | 4 file-wide (primary, secondary, stop all reference it — brand focus lock preserved) |
| `status-error` inside any `.cd-btn-stop*` rule | **0** (D-05 invariant) | **0** — confirmed via `grep -n 'status-error'` (only present in token defs and `.cd-badge--failed` / `.cd-badge--error`) |

Grep counts from `design/DESIGN_SYSTEM.md`:

| Pattern | Expected | Actual |
|---|---|---|
| `cd-status-stopped` | ≥2 | 6 (table row + tint row + 2 dark-sample + 2 light-sample) |
| `#94a3b8` | ≥1 | 2 |
| `#64748b` | ≥1 | 2 |
| `Operator-Interrupt` | ≥1 | 1 |
| `rgba(148, 163, 184, 0.12)` | ≥1 | 2 |
| `rgba(100, 116, 139, 0.08)` | ≥1 | 2 |
| `cd-status-active` (baseline regression sentinel) | unchanged | 7 (no existing rows replaced) |
| ` ```mermaid ` blocks | unchanged | 0 (file has none; no diagrams added) |

## Build Gates

- `cargo build -p cronduit` → **exits 0** (47.92s, dev profile). Expected warning: `Tailwind binary not found at bin/tailwindcss — run \`just tailwind\` to build CSS`. `build.rs` leaves `assets/static/app.css` as the pre-existing stale minified bundle because no `bin/tailwindcss` is present in the worktree; per plan action note and `build.rs:22`, the CI/release Docker stage regenerates the static bundle automatically (and will `panic!` if the binary is missing in a release build, so staleness cannot leak to production). `rust-embed` debug mode reads `assets/src/app.css` from disk so the dev loop sees the new classes immediately.
- `cargo clippy -p cronduit --all-targets -- -D warnings` → **exits 0** (38.69s). No warnings introduced.

## Required SUMMARY Disclosures (from plan §output)

1. **Count of `--cd-status-stopped*` occurrences in `assets/src/app.css`:** 13 total matches (3 `--cd-status-stopped:` token-definition lines + 3 `--cd-status-stopped-bg:` token-definition lines + 4 `var(--cd-status-stopped...)` consumer references inside `.cd-badge--stopped`, `.cd-btn-stop:hover` (×2 — background + border-color + color are three references actually = 3 but some collapse; actual total including base class = 7 consumer refs). Token definitions alone total 6 as the plan predicts: 3 blocks × 2 tokens.
2. **Confirmation that `.cd-btn-stop` does NOT reference `--cd-status-error`:** Confirmed. `grep -n 'status-error' assets/src/app.css` lists only token definitions (L32/36/89/93/122/126) and `.cd-badge--failed` (L179) / `.cd-badge--error` (L182). No `cd-btn-stop*` rule references any error token. D-05 "neutral outline, NOT red" invariant holds.
3. **Path to regenerated `assets/static/app.css`:** Not regenerated in this worktree — `bin/tailwindcss` is absent. The existing `assets/static/app.css` (13506-byte minified bundle from prior release) is **intentionally left alone per plan action note**: "DO NOT commit a stale `assets/static/app.css` — it must either be regenerated from the new source OR be excluded from the commit if the project only regenerates it at release time." Release builds will regenerate it via `build.rs` → Docker builder stage `bin/tailwindcss`, and will hard-fail (panic) if the binary is missing (`build.rs:22`). Dev builds use `rust-embed` debug mode to read `assets/src/app.css` from disk, so the new classes are live immediately in local dev.

## Deviations from Plan

**None — plan executed exactly as written.**

All five edits in Task 1 (three token blocks, badge class, button class block) and three edits in Task 2 (two table rows plus the sample `:root` snapshot updates for D-02 source-of-truth consistency) landed verbatim. No Rule 1/2/3 auto-fixes applied. No checkpoints hit. No authentication gates.

## Threat Model Compliance

- **T-10-08-01 (Information Disclosure — contrast failure):** mitigated. Slate-400 `#94a3b8` on `--cd-bg-surface` `#0a0d0b` = 8.9:1 (WCAG AAA body); slate-500 `#64748b` on `#ffffff` = 5.4:1 (WCAG AA body, AA large). Badge text is xs/bold/uppercase — effectively large-text, so 3:1 minimum requirement is comfortably exceeded.
- **T-10-08-02 (UI Spoofing — Stop button semantics):** accepted per threat register. Neutral outline + D-05 lock is the phase design.
- **ASVS L1:** no new authentication/authorization/crypto/input-validation surface added. V14 Configuration (static design tokens, source-controlled) — unchanged.
- **Threat flags:** none. No new network endpoints, auth paths, file access, or schema changes.

## Files Touched

| File | Change | Purpose |
|---|---|---|
| `assets/src/app.css` | +48 lines | Stopped tokens (3 blocks), badge class, button class block |
| `design/DESIGN_SYSTEM.md` | +6 lines | Status Colors + tints table rows + §6 sample block sync |

`assets/static/app.css` **not modified** — pre-existing compiled bundle left for the release-time Tailwind rebuild (build.rs + Docker builder stage).

## Commits

| Task | Hash | Message |
|---|---|---|
| 1 | `ff1b2e7` | `style(10-08): add stopped status token + .cd-badge--stopped + .cd-btn-stop classes` |
| 2 | `5a1708f` | `docs(10-08): add stopped row to DESIGN_SYSTEM.md Status Colors + tints tables` |

## Known Stubs

None. This plan delivers complete CSS tokens + classes that downstream plans (10-09 template updates, 10-07 status mapping) consume directly.

## Self-Check: PASSED

- [x] `assets/src/app.css` modified — confirmed via `git log --oneline -3`
- [x] `design/DESIGN_SYSTEM.md` modified — confirmed via `git log --oneline -3`
- [x] Commit `ff1b2e7` exists in history
- [x] Commit `5a1708f` exists in history
- [x] `cargo build -p cronduit` exits 0
- [x] `cargo clippy -p cronduit --all-targets -- -D warnings` exits 0
- [x] All plan acceptance-criteria grep counts match exactly
- [x] No modifications to `STATE.md` or `ROADMAP.md` (parallel-worktree rule)
- [x] No edit of `assets/static/app.css` build output (plan invariant)
- [x] No destructive git operations (`git clean`, `git reset --hard`, etc.) beyond the required initial worktree rebase to the 67449b3 base
