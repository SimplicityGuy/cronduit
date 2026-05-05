---
phase: 23
plan: 07
title: "Phase 23 — Job Tagging Dashboard Filter Chips — Human UAT"
autonomous: false
maintainer_validated: true
created: 2026-05-04
requirements: [TAG-06, TAG-07, TAG-08]
status: pending
---

# Phase 23 — Maintainer UAT Runbook

> **`autonomous: false`** — Claude does NOT mark this UAT passed.
> Per project memory `feedback_uat_user_validates.md`: every UAT step
> requires maintainer execution + eyeball validation; Claude's automated
> tests cover the unit + integration surface (V-01..V-14 in
> `tests/v12_tags_dashboard.rs` + the V-05 / V-07 unit tests in
> `src/web/handlers/dashboard.rs::tests`), but visual rendering, mobile
> reflow, light-mode parity, keyboard navigation, and screen-reader
> narration require human judgment.
>
> Per project memory `feedback_uat_use_just_commands.md`: every scenario
> below references a `just uat-chips-*` recipe (Plan 23-06 output). NO
> ad-hoc `cargo` / `docker` / curl invocations.

## Prerequisites

- [ ] Plans 23-01 through 23-06 are merged (or applied locally on a feature branch).
- [ ] `cargo build` succeeds on the working tree.
- [ ] `cargo test --test v12_tags_dashboard` exits 0 (V-01..V-14 + V-W0).
- [ ] `cargo test --lib web::handlers::dashboard::tests` exits 0 (V-05 + V-07 unit tests).
- [ ] `just --list` shows `uat-chips-render`, `uat-chips-and-filter`, `uat-chips-share-url`.

## Scenario 1 — Chip strip render + alphabetical + empty-state hidden (TAG-06 / D-17 step 1)

**Goal:** Confirm the chip strip renders one chip per distinct fleet tag, alphabetical, and is hidden entirely when no job has tags.

**Steps:**

1. Run `just uat-chips-render` from a fresh terminal.
2. Follow the recipe's prompts (build → db-reset → write fleet TOML → run cronduit in another terminal → open the dashboard).
3. **Eyeball criterion (a):** the chip strip renders ABOVE the name-filter input (D-01); contains exactly THREE chips in alphabetical order (`backup` / `prod` / `weekly`); all chips are inactive (grey background + subtle border per UI-SPEC § Color "Inactive (default)" row).
4. **Eyeball criterion (b):** with no chip active, the `uat-chips-untagged` row IS rendered in the table (untagged jobs are visible on default load — TAG-07 only hides untagged when a tag filter is active).
5. **Eyeball criterion (c):** stop cronduit, edit the recipe's TOML to remove every `tags = [...]` line, restart cronduit, and reload the dashboard. The chip strip MUST be hidden entirely (the dashboard looks identical to v1.0 / v1.1; D-02 empty-state — `<div id="cd-tag-chip-strip" hidden>`).

**Sign-off:**

- [ ] Scenario 1 passed (chip strip renders alphabetically; empty-state hides the strip).

## Scenario 2 — AND-filter + untagged-hidden + name-filter composition (TAG-06 + TAG-07 / D-17 step 2)

**Goal:** Confirm multi-chip AND semantics + untagged-hidden when a filter is active + AND with the existing v1.0 name-filter.

**Steps:**

1. Run `just uat-chips-and-filter`.
2. Follow the recipe through:
   - (a) click `backup` → 3 rows visible, untagged hidden;
   - (b) also click `weekly` → 2 rows visible (AND across two chips);
   - (c) type `prod` in the name-filter input → 1 row visible (AND with name-filter);
   - (d) deactivate `weekly` (click it again) → 2 rows visible.

**Eyeball criteria:**

- **(a)** clicking `backup` → chip turns teal-bordered + bold + green-dim background tint (UI-SPEC § Color "Active" row — three-channel encoding). Three rows visible: `prod-backup-weekly`, `prod-backup-only`, `dev-backup-weekly`. The `untagged-noise` row is HIDDEN.
- **(b)** clicking `weekly` too → both chips show teal border + bold weight. TWO rows visible: `prod-backup-weekly`, `dev-backup-weekly`.
- **(c)** typing `prod` in the name-filter input → ONE row visible: `prod-backup-weekly`.
- **(d)** clicking `weekly` again returns it to grey/inactive. The chip-strip is now `backup` active + `weekly` inactive; `prod-backup-only` reappears (matches `prod` name-filter AND `backup` tag-filter).

**Sign-off:**

- [ ] Scenario 2 passed (AND across chips, AND with name-filter, untagged-hidden when filter active).

## Scenario 3 — Shareable URL round-trip + stale-tag silent-drop (TAG-06 / D-17 step 3)

**Goal:** Confirm bookmarkable URL state + URL canonicalization (alphabetical) + stale-tag silent-drop on direct paste.

**Steps:**

1. Run `just uat-chips-share-url`.
2. Click two chips (any two — recipe seeds `backup` + `weekly`), copy the URL bar, paste into a fresh tab.
3. Confirm fresh-tab paint: both chips active on first paint; table filtered correctly with no flash of inactive state.
4. Test the stale-tag URL `?tag=backup&tag=ghost` per Step 6 of the recipe.

**Eyeball criteria:**

- **(a)** fresh tab loads with both chips active on first paint (no flash of inactive — server-rendered active state, not client-side hydration).
- **(b)** the URL after canonicalization is alphabetical (e.g., `/?tag=backup&tag=weekly` regardless of click order — UI-SPEC § Decisions Rationale "URL canonicalization" row).
- **(c)** reloading the page (cmd-R / F5 / ctrl-R) preserves active state.
- **(d)** stale-tag URL renders 200 OK; only the `backup` chip appears active; the `ghost` tag is silently dropped — no "ghost" chip is rendered (UI-SPEC § Decisions Rationale "Stale-tag URL handling").

**Sign-off:**

- [ ] Scenario 3 passed (shareable URL round-trips; stale tags silently dropped).

## Scenario 4 — Mobile viewport reflow (TAG-06 + UI-SPEC § Layout / D-17 mobile)

**Goal:** Confirm the chip strip wraps cleanly to multiple rows on narrow viewports (no horizontal scroll, no `<details>` collapse), and chips remain WCAG 2.2 AAA touch-target sized.

**Steps:**

1. Reuse the running cronduit instance from Scenario 2 (`just uat-chips-and-filter` setup — multi-tag fleet already loaded). If cronduit is no longer running, re-run that recipe.
2. Open the dashboard at `http://127.0.0.1:8080/`.
3. Open browser DevTools → toggle the device toolbar (mobile emulator) → set viewport to **360px × 640px** (smallest reasonable phone size).
4. With the multi-tag fleet from `uat-chips-and-filter` loaded, reload at the 360px viewport.

**Eyeball criteria:**

- Chip strip uses `flex-wrap` to flow onto multiple rows — every chip remains visible (D-03 / UI-SPEC § Layout § Mobile reflow).
- NO horizontal scroll on the chip strip (no chips clipped off-screen).
- NO `<details>` collapse / summary anywhere on the strip.
- Chips remain tap-target-sized — the rendered chip height is **≥ 44px** (WCAG 2.2 AAA). Verify by selecting a chip in DevTools → "Computed" → confirm `height` is at least 44px (math: `min-height: 40px` + `padding-block: 8px × 2 = 16px` → effective tap height ≥ 56px, well above 44px floor). Also tap each chip on the mobile emulator and confirm activation without mis-tap.
- Active chip state remains visible at 360px exactly like at desktop (border + label color + bold weight; no responsive degradation of the three-channel encoding).

**Sign-off:**

- [ ] Scenario 4 passed (chip strip reflows cleanly on mobile; touch targets remain ≥ 44px; active state visible).

## Scenario 5 — Light-mode parity (TAG-06 + UI-SPEC § Color / D-17 light mode)

**Goal:** Confirm chip rendering honors `[data-theme="light"]` automatically (UI-SPEC promised "zero new light-mode work").

**Steps:**

1. Reuse the running cronduit instance from Scenario 2 or 4 (multi-tag fleet loaded). If cronduit is no longer running, re-run `just uat-chips-and-filter`.
2. Confirm the dashboard is in dark mode (default — `<html data-theme="dark">` or no attribute).
3. Toggle to light mode via the existing theme-switch primitive (whatever button or system-preference integration the v1.0 / v1.1 dashboard uses; verify by inspecting `<html data-theme="...">` in DevTools — value should change to `light`).
4. Reload the dashboard.
5. Click one chip to put it in the active state, then eyeball the active vs inactive contrast in light mode.

**Eyeball criteria:**

- Inactive chip background is the light-mode equivalent of `--cd-bg-surface-raised` (light grey on a white-ish page background, NOT a dark surface).
- Active chip border is the light-mode equivalent of `--cd-text-accent` (deeper green `#059669` per UI-SPEC § Color, NOT the dark-mode bright `#34d399`).
- Active chip background tint (`--cd-green-dim`) is subtle in light mode — the active chip reads as "filled with a wash of green" without overwhelming the page.
- Hover and `:focus-visible` states behave consistently with light-mode tokens (the focus ring is visible against the light page).
- The three-channel active encoding (border color + label color + bold weight) remains visible in light mode — no channel collapses to invisibility.

**Sign-off:**

- [ ] Scenario 5 passed (light mode renders chip strip with correct token mapping; no hardcoded dark-mode colors leak through).

## Scenario 6 — Keyboard navigation + screen-reader narration (TAG-06 + UI-SPEC § Accessibility / D-17 keyboard + screen-reader)

**Goal:** Confirm WCAG 2.2 AAA touch + keyboard + assistive-technology contracts: `:focus-visible` ring, `aria-pressed` true/false, three-channel active state encoding, ≥ 44px touch target, full keyboard reachability.

**Steps:**

1. Reuse the running cronduit instance from any of Scenarios 2 / 4 / 5 (multi-tag fleet loaded). If cronduit is no longer running, re-run `just uat-chips-and-filter`.
2. Focus the browser tab.
3. Press Tab repeatedly to navigate from the URL bar onto the dashboard's interactive elements; confirm the chip anchors are reachable in document order.
4. Tab onto a chip; press Enter (and separately, press Space). Both MUST trigger the chip toggle (URL changes; chip active state updates via OOB swap; table body refreshes).
5. Activate a screen reader (VoiceOver on macOS via cmd-F5; NVDA on Windows; Orca on Linux). Navigate to the chip strip; listen.

**Eyeball / ear criteria:**

- **(a) Tab order:** the chip strip is reachable BEFORE the name-filter input (matches D-01 visual order — chips filter the corpus, name-filter narrows within).
- **(b) `:focus-visible` ring:** when a chip receives keyboard focus, a 2px green ring renders around it (`box-shadow: 0 0 0 2px var(--cd-green-dim)` per UI-SPEC § Color "Inactive + `:focus-visible`" / "Active + `:focus-visible`" rows). The ring is visible in BOTH dark and light mode.
- **(c) Enter and Space activate:** both keys toggle the chip (UI-SPEC § Interaction Contract row "Keyboard: Enter / Space"). The URL changes; the chip's active state visually flips; the table body re-renders to match.
- **(d) Screen reader: group label.** Entering the chip strip, the screen reader announces something equivalent to "Filter jobs by tag, group" (matches `<div role="group" aria-label="Filter jobs by tag">` in the markup).
- **(e) Screen reader: chip aria-label.** Each chip is announced with the full sentence form per UI-SPEC § Copywriting Contract: e.g., "Tag filter: backup (inactive — click to add)" or "Tag filter: backup (active — click to remove)" depending on state.
- **(f) Screen reader: aria-pressed.** Active chips announce "pressed" (or platform-equivalent) and inactive chips announce "not pressed" — verify by toggling a chip and listening to the state change announcement. The `aria-pressed="true"` / `aria-pressed="false"` attribute drives this.
- **(g) Three-channel active state encoding:** with the screen reader silenced, confirm by sight alone that an active chip is distinguishable from an inactive chip via THREE independent channels — **border color** (teal `--cd-text-accent` vs subtle `--cd-border-subtle`), **label color** (`--cd-text-accent` vs `--cd-text-secondary`), AND **font weight** (700 bold vs 400 regular). Color-vision-deficient operators must read state via the bolder weight + border darkness alone.
- **(h) No focus traps:** Tab from the last chip moves focus into the name-filter input (and onward to the table); Shift-Tab from the first chip moves focus back to the page chrome above. No element traps keyboard focus.
- **(i) Touch target ≥ 44px:** focus a chip, then in DevTools → "Computed" → confirm the rendered chip height is at least 44px. Combined `min-height: 40px` + `padding-block: 8px × 2 = 16px` produces an effective height ≥ 56px (well above the WCAG 2.2 AAA 44px floor).

**Sign-off:**

- [ ] Scenario 6 passed (keyboard navigation, screen-reader narration including aria-pressed, focus rings, three-channel active encoding, and touch targets all conform to UI-SPEC § Accessibility Contract).

## Final sign-off

When all six scenarios above are checked:

- [ ] **Maintainer:** I have run all six scenarios on a clean working tree against a feature branch with Plans 23-01 through 23-06 applied. Each scenario produced the expected operator-observable behavior. Mobile reflow, light-mode parity, keyboard navigation including Enter/Space activation, screen-reader narration with `aria-pressed` state, and three-channel active state encoding (border + label color + bold weight) are all correct. Touch targets are ≥ 44px (WCAG 2.2 AAA). Phase 23 is UAT-complete and ready for the rc.3 cut.

Maintainer name: ________
Date: ________
