---
phase: 21
plan: 11
type: human-uat
autonomous: false
created: 2026-05-02
status: pending-maintainer-validation
---

# Phase 21 — Human UAT (Maintainer-Validated)

**Phase:** 21 — Failure-Context UI Panel + Exit-Code Histogram Card — rc.2
**Status:** **pending — maintainer-validation required.** Plan 21-11 ships this artifact unticked; the maintainer runs each scenario from a fresh terminal and flips `[ ]` → `[x]` themselves.
**Prerequisite:** Plans 21-01..21-10 merged on `phase21/ui-spec` (or successor branch) + CI matrix green. This runbook gates the `v1.2.0-rc.2` tag cut (`21-RC2-PREFLIGHT.md` reads the sign-off block below).
**Requirements covered:** FCTX-01, FCTX-02, FCTX-03, FCTX-05, FCTX-06, EXIT-01, EXIT-02, EXIT-03, EXIT-04, EXIT-05, EXIT-06.
**Locked decisions:** D-01..D-21 (implementation), D-22..D-26 (rc.2 cut), D-32 (rustls invariant).

> **Per project memory `feedback_uat_user_validates.md`:** the maintainer validates each scenario manually. Claude does NOT mark UAT passed from its own test runs — every checkbox below is operator-flipped.
>
> **Per project memory `feedback_uat_use_just_commands.md`:** every step references an existing `just` recipe. No raw `curl` / `cargo run` / `docker run` commands in scenario bodies — only inside the recipe definitions.

## Prerequisites

| Prereq | Recipe | Notes |
|--------|--------|-------|
| Phase 21 PR open against `main` | n/a | From `phase21/ui-spec` (or successor). Per project memory `feedback_no_direct_main_commits.md`. |
| Workspace builds clean | `just ci` | Full CI gate: fmt + clippy + openssl-check + nextest + schema-diff + image |
| Test suite green locally | `just nextest` | Includes `tests/v12_fctx_panel.rs`, `tests/v12_exit_histogram.rs`, extended `tests/v12_fctx_explain.rs` |
| Schema parity holds | `just schema-diff` | New `scheduled_for` column visible on both backends per D-01 / D-05 |
| rustls invariant holds | `just openssl-check` | `cargo tree -i openssl-sys` empty across native + arm64-musl + amd64-musl (D-32) |
| `Cargo.toml` at `1.2.0` | n/a | The unsuffixed in-source version per D-31 / `feedback_tag_release_version_match.md` |

## Scenarios

Each scenario lists: the goal, the recipe(s) to run, the expected output, and a checkbox the maintainer flips after observing the behavior.

### Scenario 1 — FCTX panel renders on a failed run (FCTX-01, FCTX-02, FCTX-05)

**What this proves:** FCTX-01 (panel gated to `status ∈ {failed, timeout}` per D-13 gating), FCTX-02 (5-row contract per UI-SPEC § Component Inventory), FCTX-05 (TIME DELTAS row).

- **Recipe:** `just uat-fctx-panel`
- **Steps:**
  1. Run `just uat-fctx-panel` (the recipe will reset the dev DB, prompt to start `just dev`, seed 4 consecutive failed runs against `fire-skew-demo`, and hand you the run-detail URL).
  2. In another terminal: `just dev`. Wait until the listening line appears, then return to the recipe terminal and press ENTER.
  3. Open the printed run-detail URL (`http://127.0.0.1:8080/jobs/{id}/runs/{id}`).
- **Pass criteria:**
  - [ ] Panel renders **collapsed-by-default** with summary `Failure context · 4 consecutive failures` (native `<details>` toggle per UI-SPEC § Interaction Contract)
  - [ ] Click summary → panel expands smoothly
  - [ ] **TIME DELTAS row** shows `First failure: {rel} ago • 4 consecutive failures` (no link to a prior success — none exists in the seeded data per D-13 never-succeeded path)
  - [ ] **CONFIG row** is hidden (no prior success → D-13 hides this row)
  - [ ] **DURATION row** is hidden (no successful samples → D-13 hides this row)
  - [ ] **FIRE SKEW row** shows `Scheduled: HH:MM:SS • Started: HH:MM:SS (+0 ms)` (manual-trigger inserts; `scheduled_for = start_time` per D-02)
  - [ ] **IMAGE DIGEST row** is hidden if the seeded job is non-docker; visible (with the seeded `image_digest IS NULL`-driven hide) for docker jobs
  - [ ] No console errors in browser DevTools

[ ] Maintainer-validated

### Scenario 2 — Panel hidden on success / cancelled / running / stopped (FCTX-01 negative)

**What this proves:** FCTX-01 gating per D-13 — the panel is gated to `status ∈ {failed, timeout}` and is invisible on every other status.

- **Recipe:** Reuse the seeded data from Scenario 1; flip status via `just db-reset`-friendly inline sqlite3 (the only place in this UAT where raw sqlite3 is acceptable, since this scenario specifically exercises the status discriminator).
- **Steps:**
  1. After Scenario 1, run:
     ```bash
     sqlite3 cronduit.dev.db "UPDATE job_runs SET status='success' WHERE id=(SELECT MAX(id) FROM job_runs);"
     ```
  2. Reload the run-detail page for that run.
  3. Repeat for `status='cancelled'`, `status='running'`, `status='stopped'`, reloading after each.
- **Pass criteria:**
  - [ ] No `cd-fctx-panel` element rendered for `status='success'`
  - [ ] No `cd-fctx-panel` element rendered for `status='cancelled'`
  - [ ] No `cd-fctx-panel` element rendered for `status='running'`
  - [ ] No `cd-fctx-panel` element rendered for `status='stopped'`
  - [ ] Page renders normally (no broken layout, no template errors, no console errors) in all 4 cases

[ ] Maintainer-validated

### Scenario 3 — Image-digest row hidden on non-docker job (FCTX-03)

**What this proves:** FCTX-03 — the IMAGE DIGEST row hides on jobs whose `type ≠ docker` per UI-SPEC § Component Inventory row table (mirrored by FIRE SKEW's NULL-hides-row behavior per D-04).

- **Recipe:** `just uat-fctx-panel` (reuse the panel walk-through against a `command`-type job).
- **Steps:**
  1. Edit `examples/cronduit.toml` to add a `[[jobs]]` block of `type = "command"` (e.g., `command = "false"` to guarantee a failed exit).
  2. Reload `just dev`.
  3. Wait for / trigger a failed run on the new command job (or seed a row matching the recipe's pattern but pointing at the new job's `job_id`).
  4. Open the run-detail page for that run.
- **Pass criteria:**
  - [ ] FCTX panel renders (status=failed)
  - [ ] **IMAGE DIGEST row is ABSENT** from the panel body (the row is hidden, not rendered with a `—` placeholder)

[ ] Maintainer-validated

### Scenario 4 — Histogram card on job-detail page (EXIT-01, EXIT-02, EXIT-03, EXIT-04, EXIT-05)

**What this proves:** EXIT-01 (card sibling to Duration card), EXIT-02 (10-bucket layout), EXIT-03 (success-rate stat), EXIT-04 (status discriminator wins — D-08), EXIT-05 (top-3 codes with last-seen — D-10).

- **Recipe:** `just uat-exit-histogram`
- **Steps:**
  1. Run `just uat-exit-histogram` (the recipe resets the DB, prompts to start `just dev`, seeds 10 mixed-status/exit-code runs against `fire-skew-demo`, and hands you the job-detail URL).
  2. In another terminal: `just dev`. Wait for the listening line, return to the recipe terminal, press ENTER.
  3. Open the printed job-detail URL (`http://127.0.0.1:8080/jobs/{id}`).
- **Pass criteria:**
  - [ ] **"Exit Code Distribution" card** is sibling to the Duration card (between Duration and Run History per UI-SPEC layout diagram)
  - [ ] **SUCCESS stat:** ~56 % (5 success / (10 − 1 stopped) = 5/9 per D-09 success-rate-excludes-stopped formula)
  - [ ] **10 bucket columns** visible with the locked short labels: `1`, `2`, `3-9`, `10-126`, `127`, `128-143`, `144-254`, `255`, `none`, `stopped`
  - [ ] **Bucket1** column has count 2 with `cd-exit-bar--err-strong` styling
  - [ ] **Bucket127** column has count 1 with `cd-exit-bar--warn` (yellow) styling
  - [ ] **Bucket128to143** column has count 1 with `cd-exit-bar--warn` (yellow) styling — **proves EXIT-04 dual-classifier** (a status='failed' + exit_code=137 row landed HERE, NOT in BucketStopped, per D-08)
  - [ ] **BucketStopped** column has count 1 with `cd-exit-bar--stopped` (slate-grey) styling — the status='stopped' + exit_code=137 row landed HERE per D-08
  - [ ] **Recent codes table** shows entries for `1` (count 2), `137` (count 1), `127` (count 1) per D-10 top-3 with last-seen
  - [ ] Hover BucketStopped bar → tooltip reads **"NOT a crash"** per UI-SPEC § Copywriting Contract

[ ] Maintainer-validated

### Scenario 5 — Below-threshold empty state (D-15, D-16)

**What this proves:** D-15 / D-16 — the histogram card is always rendered (outer chrome + heading) but shows an em-dash empty state with `Need 5+ samples; have {N}` copy when sample_count < 5.

- **Recipe:** `just db-reset` then `just dev` (no seeding — exercise the zero-runs path).
- **Steps:**
  1. Run `just db-reset` (per D-19 recipe-calls-recipe pattern, this is the same recipe `uat-exit-histogram` calls internally).
  2. In another terminal: `just dev`.
  3. Open `/jobs/{id}` for any seeded job that has zero runs (or any newly created job).
- **Pass criteria:**
  - [ ] Card renders with heading **"Exit Code Distribution"**
  - [ ] Empty state: `—` em-dash + the locked copy **"Need 5+ samples; have 0"**
  - [ ] No bars rendered, no recent-codes table, no SUCCESS stat
  - [ ] Card outer border + heading visible (so operators don't wonder where the card went per D-15 reasoning)

[ ] Maintainer-validated

### Scenario 6 — Fire-skew row renders on a slow-start container (FCTX-06)

**What this proves:** FCTX-06 / D-01 / D-02 / D-04 — the FIRE SKEW row computes `start_time − scheduled_for` and renders the delta in milliseconds; on a container that sleeps before completing, the skew is visibly positive.

- **Recipe:** `just uat-fire-skew`
- **Steps:**
  1. Run `just uat-fire-skew` (the recipe verifies `examples/cronduit.toml` contains the `fire-skew-demo` job from plan 21-10 task 1, prompts to start `just dev`, waits for a `* * * * *` tick, and hands you the run-detail URL).
  2. In another terminal: `just dev`. Wait for the listening line and the next minute boundary, then return and press ENTER.
  3. Wait at least 90 seconds (so a tick fires AND the container completes).
  4. Open the printed run-detail URL.
- **Pass criteria:**
  - [ ] **FIRE SKEW row** reads approximately `Scheduled: HH:MM:00 • Started: HH:MM:30 (+30000 ms)` — the slow-start container delays its `start_time` by ~30 s relative to `scheduled_for` per the recipe-seeded `sleep 30 && echo done` command
  - [ ] Skew is **positive** (`start_time > scheduled_for`) and within expected docker-startup tolerance (±5 s of 30 000 ms)
  - [ ] Note: `fire-skew-demo` exits 0 by default (so the FCTX panel itself is hidden — Scenario 6 verifies the FIRE SKEW row's data source. To see the row inside the panel, edit `command` to `sleep 30 && exit 1` and re-run; the recipe documents this fallback.)

[ ] Maintainer-validated

### Scenario 7 — Accessibility umbrella (mobile / light / print / keyboard) — research §G

**What this proves:** UI-SPEC § Accessibility Contract — the 4-phase a11y umbrella per research § Discretion Resolutions §G (single recipe, single browser session, 4 sub-phases). D-20 locks the umbrella shape (vs split recipes).

- **Recipe:** `just uat-fctx-a11y`
- **Steps:**
  1. Have a failed run available (run `just uat-fctx-panel` first if needed).
  2. Run `just uat-fctx-a11y` and walk the 4 phases interactively. The recipe pauses for ENTER between phases.
- **Pass criteria** (mark each as you complete that phase):
  - [ ] **Phase 1 — Mobile (<640 px):** rows STACK 1-column inside the FCTX panel; histogram chart is HORIZONTALLY SCROLLABLE on `/jobs/{id}` (per UI-SPEC § Responsive Contract)
  - [ ] **Phase 2 — Light mode:** panel + card render with light tokens (grey-on-white, no broken contrast); the existing `[data-theme="light"]` block in `assets/src/app.css` covers the new classes
  - [ ] **Phase 3 — Print:** FCTX panel renders **OPEN by default** in print preview (per `@media print { details { open: open } }` in UI-SPEC § Print contract)
  - [ ] **Phase 4 — Keyboard-only:** focus rings visible on the `<summary>`, the "view last successful run" link, and each histogram bar; **Space/Enter** toggles the `<details>` panel; tooltip appears on bar focus (not just on hover)

[ ] Maintainer-validated

### Scenario 8 — EXIT-06 cardinality discipline (out-of-scope verification)

**What this proves:** EXIT-06 (accepted-out-of-scope per CONTEXT § Out of scope and research § Phase Requirements EXIT-06) — Phase 21 did NOT add a per-job exit-code Prometheus label. The cardinality discipline holds.

- **Recipe:** None (this is a static-analysis grep + a runtime `/metrics` scrape spot check).
- **Steps:**
  1. Run the canonical greps:
     ```bash
     grep -rn 'exit_code' src/metrics.rs                # MUST return empty (no metrics.rs file or no match)
     grep -rn 'cronduit_runs_total.*exit_code' src/    # MUST return empty
     grep -rn 'exit_code' src/web/handlers/metrics.rs   # MUST return empty (canonical handler path)
     ```
  2. With `just dev` running, scrape `/metrics`:
     ```bash
     curl -s http://127.0.0.1:8080/metrics | grep -E '^cronduit_runs_total'
     ```
- **Pass criteria:**
  - [ ] All three greps return empty (no `exit_code` label registered anywhere in the metrics source path)
  - [ ] `/metrics` scrape shows `cronduit_runs_total{job, status}` with NO `exit_code` label dimension
  - [ ] No new per-job exit-code Prometheus label was added (the histogram card is the operator-facing surface; cardinality is preserved per EXIT-06 reasoning)

[ ] Maintainer-validated

## Out-of-band spot check — `cargo tree -i openssl-sys` (D-32)

**What this proves:** D-32 — the rustls-everywhere invariant holds. Phase 21 added zero new external crates (the histogram is pure-CSS bars + askama; the panel is native `<details>`; no JS bundle, no SVG, no canvas).

```bash
cargo tree -i openssl-sys
```

- [ ] Output is empty (or `error: package ID specification \`openssl-sys\` did not match any packages`) — confirming no transitive dependency on openssl-sys was introduced

## Sign-off

All 8 scenarios above plus the rustls invariant spot check must be ticked `[x]` by the maintainer before the `v1.2.0-rc.2` tag is cut (`21-RC2-PREFLIGHT.md` reads this sign-off block as a pre-flight gate per its frontmatter `must_haves`).

| Field | Value |
|-------|-------|
| Maintainer signature | `__________________` |
| Date (UTC) | `__________________` |
| Comment / context | `__________________` |

After all boxes are ticked and the sign-off table is filled in:

- The maintainer comments on the rc.2 PR (or merges Plan 21-11) signaling UAT passed.
- `21-RC2-PREFLIGHT.md` reads this file's sign-off to gate the actual `git tag -a -s v1.2.0-rc.2 ...` invocation per `docs/release-rc.md` (D-22..D-26).
- Post-tag, `.planning/STATE.md` and `.planning/ROADMAP.md` reflect Phase 21 → SHIPPED at rc.2 (orchestrator owns those writes per project workflow).

---

**Cross-reference:** every scenario above has automated regression coverage in the Phase 21 integration test files (`tests/v12_fctx_panel.rs`, `tests/v12_exit_histogram.rs`, extended `tests/v12_fctx_explain.rs` — created in plans 21-07..21-09). The UAT scenarios re-prove the same behaviors against a real browser — operator-side rendering, real CSS tokens applied, real `<details>` toggle, real keyboard-driven focus order. The tests guard against regressions; this runbook guards against drift between the implementation and the operator's experience.
