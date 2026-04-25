# Phase 14: Bulk Enable/Disable + rc.3 + Final v1.1.0 Ship - Context

**Gathered:** 2026-04-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 14 delivers ERG-01..04 + DB-14, then closes v1.1 with the `v1.1.0-rc.3` cut and the promotion to final `v1.1.0`:

1. **Schema: `jobs.enabled_override` tri-state column** (DB-14) — `INTEGER NULL` on SQLite, `BIGINT NULL` on Postgres. Semantics: `NULL` = follow config `enabled` flag; `0` = force disabled; `1` = force enabled (reserved; not written by the v1.1 UI). `get_enabled_jobs` filter becomes `WHERE enabled = 1 AND (enabled_override IS NULL OR enabled_override = 1)`. `upsert_job` NEVER touches this column (the critical invariant locked by T-V11-BULK-01). `disable_missing_jobs` clears the override at the same time as setting `enabled = 0` when a job leaves the config file.

2. **Dashboard bulk-select UX** (ERG-01, ERG-02) — new leftmost checkbox column on the job table, header select-all toggle, sticky-on-scroll action bar above the table with "Disable selected" + "Enable selected" + "Clear" (selection) buttons. CSRF-gated `POST /api/jobs/bulk-toggle` handler updates `enabled_override` for every selected job, fires `SchedulerCmd::Reload` to rebuild the scheduler heap, returns a verbose toast via the existing `HX-Trigger` pattern. Running jobs are NOT terminated by bulk disable — they complete naturally (ERG-02).

3. **Settings "Currently Overridden" audit surface** (ERG-03) — new full-width section below the 6-card grid on `/settings`, table with `Name | State | Clear` columns, per-row Clear button that reuses the same `POST /api/jobs/bulk-toggle` handler (with `job_ids=<n>&action=enable`).

4. **Reload invariant** (ERG-04) — SIGHUP / `POST /api/reload` / file-watch reload preserves `enabled_override` for every job still in the config; `disable_missing_jobs` clears the override for jobs that left the config. T-V11-BULK-01 locks this.

5. **`v1.1.0-rc.3` tag cut** — reuses Phase 12 release mechanics verbatim (release.yml D-10 patches, `docs/release-rc.md` runbook, manual maintainer tag cut, `git-cliff` authoritative notes, `:rc` rolling tag).

6. **Final `v1.1.0` promotion** — after HUMAN-UAT on rc.3 passes, maintainer retags the rc.3 SHA as `v1.1.0` (bit-identical image). release.yml D-10 gating implicitly advances `:latest` from `v1.0.1` to `v1.1.0` and publishes `:1.1.0` + `:1.1` + `:1` multi-arch. MILESTONES.md gets a v1.1 archive entry.

**Out of scope (deferred to v1.2 or later):**
- Force-enable via the UI (override = 1) — the schema reserves it; the UI does not write it in v1.1.
- Shift-click range selection on row checkboxes — no custom JS beyond vendored HTMX for v1.1.
- Per-row override timestamp (`enabled_override_set_at`) — adds a second column + more migration scope; revisit in v1.2 if audit demand appears.
- Terminating running jobs on bulk disable — operators who want to kill a running job use the Stop button from Phase 10 (SCHED-14).
- Bulk action toast with per-job list of names — verbose but wraps awkwardly; revisit if operators request it.
- Webhooks, queuing, email notifications — milestone scope explicitly excludes these (see PROJECT.md § Out of Scope for v1.1).
- Soak period / multi-day rc.3 canary — iterative cadence keeps the rc/final gap tight.

</domain>

<decisions>
## Implementation Decisions

### Bulk-Select UX (ERG-01, ERG-02)

- **D-01:** **Checkbox in a new leftmost column** on `templates/pages/dashboard.html` + `templates/partials/job_table.html`. `<th><input type="checkbox" id="cd-select-all"></th>` renders first in the header; `<td><input type="checkbox" class="cd-row-checkbox" value="{{ job.id }}"></td>` renders first in every row. Industry-standard table-selection pattern (Gmail, GitHub, Linear). Keeps job-name click target uncluttered; operators scan left-to-right. Rejects inline-checkbox-in-Name-cell (misclick risk between checkbox and link) and rightmost-column (unusual position).

- **D-02:** **Inline sticky-on-scroll action bar** between the filter bar and the job table. New `<div id="cd-bulk-action-bar" class="cd-bulk-bar">` element in `dashboard.html` BELOW the filter div and ABOVE the `<table>`. Shows `N selected — [Disable] [Enable] [Clear]` when at least one checkbox is ticked; renders `display: none` (or a hidden attribute) when zero. `position: sticky; top: 0; z-index: 10` keeps the action reachable as the operator scrolls through long job lists. Rejects floating-bottom-bar (CSS complexity + z-index fights on small viewports) and non-sticky inline (forces scroll-back to trigger).

- **D-03:** **Header checkbox selects all currently-rendered filtered rows only.** Clicking `#cd-select-all` toggles every `.cd-row-checkbox` in the current `<tbody>`. Respects the filter box — if the operator types "backup" into the filter, select-all selects only the visible backup jobs. Fleet-wide select is NOT possible (no "select all across filters" mode). Intermediate state on the header checkbox (`indeterminate = true`) when SOME but not all rows are ticked. Rejects no-select-all (defeats the bulk-action purpose) and shift-click-range (requires custom JS — Cronduit has no JS beyond vendored HTMX today; defer range-select to v1.2 if demanded).

- **D-04:** **Verbose toast copy with conditional second sentence.** After a successful bulk-disable: `"{N} jobs disabled. {M} currently-running jobs will complete naturally."` — the second sentence is omitted when `M == 0`. Matches ERG-02's literal REQ wording. After a successful bulk-enable (clear-override): `"{N} jobs: override cleared."` (`M` running-job note is not applicable to enable since enable doesn't kill anything). Partial-failure variant (D-12): suffix `" ({K} not found)"` when `K > 0`. Rejects terse `"{N} jobs disabled"` (hides running-jobs nuance, operators confused why spinner keeps going) and per-job-name-list (wraps awkwardly for larger selections).

### Override Semantics + Symmetry (ERG-04, T-V11-BULK-01)

- **D-05:** **"Enable selected" clears the override (sets `enabled_override = NULL`).** The v1.1 UI NEVER sets `enabled_override = 1`. Tri-state value #3 (force-enable) is reserved in the schema for future use but not exposed. Mental model: "Disable = override off; Enable = stop overriding." Symmetric with `disable_missing_jobs`, which also clears the override when a job leaves the config. Rejects force-enable (creates sticky `override = 1` rows operators lose track of) and two-button-set (extra cognitive cost without homelab-tool payoff).

- **D-06:** **A bulk-enable on a config-disabled job is a silent no-op; toast reports what was done, not the end state.** When the operator bulk-enables a job whose config has `enabled = false` (row has `enabled = 0, enabled_override IS NULL` or `enabled_override = 0`), the handler clears the override to NULL. The row stays disabled because config still says `enabled = 0`. Toast says `"{N} jobs: override cleared"` — NOT `"{N} jobs enabled"`. Honors the "config is source of truth" principle locked in v1.0. Operator's mental contract: config decides the base state, override only *disables* on top of it. Rejects force-override-to-1 behavior (contradicts config; breaks the single-source invariant) and error-on-enable-of-disabled (feels adversarial for a bulk flow).

- **D-07:** **Bulk operations are idempotent in intent; mixed-state selections apply uniformly.** `Disable selected` sets `enabled_override = 0` on every selected row regardless of prior state (no SELECT-before-UPDATE). `Enable selected` sets `enabled_override = NULL` on every selected row regardless of prior state. Toast reports selection count, not per-row transition delta: `"{N} jobs disabled"` not `"{N} newly disabled, {M} already disabled"`. Matches the idempotent-action pattern already used by Run Now, Stop, and reroll (none of them gate on current state). Rejects transition-diff reporting (extra query cost + clutter) and reject-if-duplicate (adversarial for a bulk flow).

- **D-08:** **Both buttons always shown when `N > 0`.** Action bar renders `[Disable selected] [Enable selected] [Clear]` unconditionally once the first checkbox is ticked. Server-side partial has no knowledge of per-row override state at render time; keeping both buttons unconditional avoids an extra SELECT-by-id-list on every selection change. Idempotency (D-07) guarantees the buttons are always safe to click — bulk-enable on all-unoverridden rows is a harmless no-op. Rejects conditional hide (requires HTMX-driven action-bar refresh per selection change; complex) and disable-only-bulk-enable-per-row (operators need to un-stick multiple jobs in one gesture, especially after a "test-mode" maintenance window).

### Settings Audit Surface (ERG-03)

- **D-09:** **"Currently Overridden" as a new full-width section below the 6-card grid on `/settings`.** Insert a new `<section>` block after the closing `</div>` of the existing 2-col grid in `templates/pages/settings.html` (line 71). Shape: `<h2 style="font-size:var(--cd-text-lg);font-weight:700;margin-top:var(--cd-space-8)">Currently Overridden</h2>` + description line + `<table>` styled consistently with the dashboard table. Doesn't disturb the status-card grid; full-width table accommodates the name + state + action columns. Rejects 7th-grid-card (too cramped for a list), separate `/settings/overrides` sub-page (extra routing + click cost; ERG-03 wants discoverability).

- **D-10:** **Columns: `Name | Override State | Clear`.** Three columns:
  - `Name` — job name, links to `/jobs/{id}`; same style as dashboard job-name cells.
  - `Override State` — badge: `<span class="cd-badge cd-badge--disabled">DISABLED</span>` when `enabled_override = 0`; `<span class="cd-badge cd-badge--forced">FORCED ON</span>` when `enabled_override = 1` (reserved; not writeable by v1.1 UI but renderable if someone `UPDATE`s the DB directly — defensive rendering). Reuses existing `--cd-status-disabled` token for the DISABLED badge; FORCED ON reuses `--cd-status-running` (blue) for visual distinction since both are "abnormal" states.
  - `Clear` — single inline button `<button class="cd-btn-secondary text-sm py-1 px-3">Clear</button>` inside a `<form hx-post="/api/jobs/bulk-toggle" hx-swap="none">` with hidden inputs `csrf_token`, `action=enable`, `job_ids={{ job.id }}`. Per-row POST to the same bulk handler; `action=enable` + single `job_ids` clears the override. One-click un-stick addresses ERG-03's "operator forgets for months" failure mode.
- **D-10a:** **Empty state: hide the section entirely when no job has an active override.** Server-side: if `overridden_jobs.is_empty()`, skip the `<section>` block. Avoids "no overrides to display" placeholder noise for the common case. The section appears only when it's actionable.
- **D-10b:** **Ordering: alphabetical by job name.** Stable ordering that doesn't shuffle when an override is added or cleared elsewhere. Matches the timeline's D-07 (Phase 13) alphabetical decision for the same reason.

### Bulk-Toggle API Contract (ERG-01, ERG-02)

- **D-11:** **`POST /api/jobs/bulk-toggle` accepts form-urlencoded body.** Shape: `csrf_token=<token>&action=disable&job_ids=1&job_ids=2&job_ids=3`. Handler is `#[axum::debug_handler] pub async fn bulk_toggle(State<AppState>, CookieJar, Form<BulkToggleForm>) -> Response` with `BulkToggleForm { csrf_token: String, action: String, job_ids: Vec<i64> }` — axum's `Form` extractor handles repeated `job_ids` keys natively into `Vec<i64>` via `serde_urlencoded` (confirmed via context7 on `axum 0.8`). Matches every other mutation handler (`run_now`, `stop_run`, `reroll`, `reload` are all `Form<T>`); HTMX sends this shape natively from a `<form>` element; CSRF cookie+form-field pattern is the project-standard mitigation (reused from `api.rs:32-42`). Rejects JSON body (diverges from the existing CSRF pattern; needs `hx-ext='json-enc'` or custom JS) and two-endpoints-per-action (duplicates CSRF/handler skeleton for no semantic gain — `action` field is 8 bytes).

- **D-12:** **Best-effort on invalid IDs: apply to valid, return 200 with a `(K not found)` toast suffix.** Handler flow: (1) validate CSRF → 403 on mismatch, (2) validate `action ∈ {"disable", "enable"}` → 400 on garbage, (3) dedupe `job_ids` to avoid UPDATE noise, (4) execute `UPDATE jobs SET enabled_override = $1 WHERE id = ANY($2)` (or SQLite-parameterized equivalent), (5) `rows_affected` vs `job_ids.len()` gives the `(not_found)` count, (6) fire `SchedulerCmd::Reload`, (7) render toast partial with the derived copy. Stale dashboard rows (operator deleted a job from config, dashboard still shows it, operator bulk-disables) don't break the request — the dominant race case in practice. Rejects all-or-nothing 400 (stale-row operator frustration) and 207 Multi-Status (overkill for a homelab tool; would need a new error-rendering UI).

- **D-12a:** **Dedupe `job_ids` before the UPDATE.** `let ids: Vec<i64> = form.job_ids.into_iter().collect::<std::collections::BTreeSet<_>>().into_iter().collect();`. HTMX sends whatever is on the page; multiple ticked checkboxes with the same `value` (shouldn't happen but) would cause duplicate UPDATEs. Cheap, defensive.

- **D-12b:** **Reply is an empty 200 with an `HX-Trigger: {"toast": {...}}` header; the row rerender happens via the existing HTMX 3s poll refresh of `#job-table-body`.** Mirrors `stop_run` (`api.rs:428-445`): no response body, just a toast trigger, next poll cycle picks up the new `enabled_override` state (if dashboard rendering surfaces it in any way). Alternative `HX-Refresh: true` (full page reload) is NOT used — jarring for the operator mid-interaction. Selection state preservation across the 3s poll is a separate concern (see D-17 discretion).

### Schema Migration (DB-14)

- **D-13:** **Single migration file per backend adding the nullable column.** Files: `migrations/sqlite/20260422_000004_enabled_override_add.up.sql` + `migrations/postgres/20260422_000004_enabled_override_add.up.sql`. SQLite body: `ALTER TABLE jobs ADD COLUMN enabled_override INTEGER NULL;`. Postgres body: `ALTER TABLE jobs ADD COLUMN enabled_override BIGINT NULL;`. No backfill (NULL is the correct initial state for every existing row); no 3-step dance like Phase 11's `job_run_number` migration (that was forced by NOT-NULL + backfill). Idempotent re-run safety handled by `sqlx`'s migration tracking (same mechanism as every prior migration). Naming: `enabled_override_add` is consistent with the Phase 11 `job_run_number_add` convention.

- **D-13a:** **No index on `enabled_override`.** The `jobs` table is tiny (≤ few hundred rows for a homelab fleet); a full scan on `WHERE enabled = 1 AND (enabled_override IS NULL OR enabled_override = 1)` is sub-millisecond. Adding an index creates write-side overhead on every `upsert_job` + `disable_missing_jobs` without meaningful read-side benefit.

### rc.3 Release Mechanics

- **D-14:** **Reuse Phase 12 release artifacts verbatim for rc.3 — no new release-engineering work.** Follow `docs/release-rc.md` runbook (D-11 from Phase 12) for the `git tag -a v1.1.0-rc.3 -m "Phase 14 — bulk enable/disable"` + `git push origin v1.1.0-rc.3` sequence. release.yml D-10 gating already publishes `:v1.1.0-rc.3` + `:rc` rolling tag; does NOT move `:latest`. `git-cliff --unreleased` generates the rc.3 release notes from the Phase 14 commits since rc.2. No workflow edits; no runbook edits.

- **D-15:** **rc.3 release notes: Phase 14 delta only.** `git-cliff --unreleased` naturally produces notes from the last tag (rc.2) to HEAD — that's the Phase 14 work. rc.3 readers are "watching the milestone land" and want to know what's NEW since rc.2 (bulk toggle), not a re-statement of Phase 10/11/12/12.1/13 work they already saw in prior rc notes. No hand-editing; same policy as Phase 12 D-12 and Phase 13 D-23.

### Final v1.1.0 Promotion

- **D-16:** **v1.1.0 = retag the rc.3 SHA (same digest).** After HUMAN-UAT on rc.3 passes (D-17), maintainer runs `git tag -a v1.1.0 -m "v1.1 — Operator Quality of Life" <rc.3-SHA>` + `git push origin v1.1.0`. Because the tag points at the rc.3 commit, the image that ships as `v1.1.0` is byte-identical to the image UAT validated as rc.3. Guarantees "what was tested is what ships." Rejects new-commit-between-rc.3-and-v1.1.0 (would require re-UAT because the image would differ) and workflow_dispatch-tag-cut (violates Phase 12 D-13 maintainer-key trust anchor).

- **D-17:** **HUMAN-UAT.md checklist with concrete steps referencing `just` recipes.** Phase 14 close-out plan includes a `HUMAN-UAT.md` with 6–8 steps covering the bulk-toggle end-to-end flow. Draft shape (planner refines):
  1. `just compose-up-rc3` → container reports healthy within 90s.
  2. Open dashboard → verify new leftmost checkbox column visible on every job row; header select-all works; action bar appears only when a checkbox is ticked.
  3. Bulk-disable 3 jobs while at least 1 is running → toast reads `"3 jobs disabled. 1 currently-running job will complete naturally."`; the running job finishes to its normal terminal status (not `stopped`); the other 2 stop firing.
  4. `just reload` (SIGHUP) → bulk-disabled jobs stay disabled; dashboard reflects this.
  5. Navigate to `/settings` → "Currently Overridden" section lists all 3 bulk-disabled jobs with a Clear button each.
  6. Click Clear on one → toast `"1 jobs: override cleared"`; job returns to the dashboard active state within one poll cycle.
  7. Remove one of the still-disabled jobs from `config.toml` + `just reload` → `enabled_override` clears at the same time as `enabled = 0`; settings section drops that job from the overridden list; re-adding the job to config and reloading produces a fresh enabled job (no stale override).
  8. Fresh browser tab `/metrics` → `cronduit_scheduler_up == 1`; no `cronduit_runs_total{...,status="stopped"}` hits attributable to bulk disable.

   User runs each step and checks it off; Claude does NOT mark UAT passed from automation alone (honors `feedback_uat_user_validates.md`). Every command is a `just` recipe per `feedback_uat_use_just_commands.md` — no ad-hoc `cargo`/`docker`/`curl` URLs in UAT steps. Exact step count + wording is the planner's call within this shape.

- **D-18:** **`:latest` advances implicitly via release.yml D-10 gating; verify post-push.** Because Phase 12 D-10 gates the `type=raw,value=latest` tag on `!contains(github.ref, '-')`, pushing a non-rc tag (`v1.1.0`) automatically publishes `:latest` at the v1.1.0 digest. Phase 14 close-out runs `scripts/verify-latest-retag.sh` (from Phase 12.1) POST-push to confirm `:latest` digest == `:1.1.0` digest on both amd64 and arm64. No manual `docker buildx imagetools create` retag command (that was Phase 12.1's one-shot correction for a pre-existing divergence; no divergence to fix here). Simultaneously, release.yml publishes `:1.1.0` + `:1.1` + `:1` via the same metadata-action matrix already in place.

- **D-19:** **Final v1.1.0 release notes: cumulative `git-cliff v1.0.1..v1.1.0`.** Unlike rc.3 (which is delta-only from rc.2), the v1.1.0 release body aggregates ALL v1.1 work — Phases 10, 11, 12, 12.1, 13, 14. Audience is adopters on the stable channel who skipped every rc; they want one canonical "what changed since v1.0.1" list. `git-cliff v1.0.1..v1.1.0` generates this; no hand-editing (D-12 Phase 12 policy). If sections cluster awkwardly (say, all bug fixes lumped together instead of grouped by milestone theme), that's a conventional-commit discipline issue from the Phase 10–14 commits, not a release-notes problem.

- **D-20:** **MILESTONES.md v1.1 archive entry follows the v1.0 pattern.** After v1.1.0 ships, close-out commit appends a v1.1 entry to `MILESTONES.md` matching the shape of the v1.0 entry there. Includes: milestone title, ship date, brief summary (one paragraph), pointers to `.planning/milestones/v1.1-ROADMAP.md` + `.planning/milestones/v1.1-REQUIREMENTS.md` + `.planning/milestones/v1.1-MILESTONE-AUDIT.md` (those files are created by `/gsd-complete-milestone` at archive time, not by this phase). README.md gets a one-line bump noting v1.1.0 is current stable; the "Current State" paragraph updates.

- **D-21:** **`THREAT_MODEL.md` gets a one-line bulk-toggle note consistent with Phase 10's Stop-button wording.** Add a bullet under the existing "Authentication posture" or "Web UI" section (wherever Phase 10's SCHED-14 Stop note went): `"POST /api/jobs/bulk-toggle widens the blast radius for anyone with UI access: they can disable every configured job in one request. Same mitigation as the Stop button from Phase 10 — keep Cronduit on loopback / trusted LAN or front it with a reverse-proxy-authenticated path."` Exact wording is the planner's call; the invariant is: one line, no design work, explicit enumeration.

### Folded Todos

None — `.planning/STATE.md § Pending Todos` lists only already-completed carryover items. The quick task `260421-nn3` (Postgres `j.enabled = true` BIGINT bug) was fixed on 2026-04-22 and is in `.planning/quick/` — not a Phase 14 scope item.

### Claude's Discretion

- **Selection state across the 3s HTMX poll** — the existing dashboard polls `/partials/job-table` every 3s. A naive implementation wipes the operator's row checkboxes mid-interaction. Planner picks one of: (a) `hx-preserve="true"` on every row checkbox (HTMX 2.0.4 supports it), (b) move the checkboxes OUTSIDE the polled `<tbody>` (e.g., as an overlay row), (c) pause the poll (`hx-trigger="every 3s, ..."`) while any checkbox is ticked. Recommendation: (a) — smallest template diff, standard HTMX idiom.
- **Exact CSS of the sticky action bar** — probably `position: sticky; top: var(--cd-space-2); z-index: 10; background: var(--cd-bg-surface-raised); border: 1px solid var(--cd-border); border-radius: 8px; padding: var(--cd-space-3) var(--cd-space-4)` — planner locks the exact token set in the implementation plan.
- **Whether to wrap the action bar in a single `<form>`** or fire HTMX `hx-post` directly on each button via per-button `hx-vals` — either works; planner picks the idiom that keeps the template closer to the existing `api/jobs/{id}/run` and `api/runs/{id}/stop` patterns.
- **Indeterminate-state UI for the header checkbox** — CSS-only with `:checked`/`:indeterminate` selectors is enough; planner decides whether to add a tiny inline `<script>` to toggle `.indeterminate` or just omit the indeterminate state and accept that the header checkbox shows as unchecked when a partial selection exists.
- **Color of the FORCED ON badge in the settings overridden list** — `--cd-status-running` (blue) or a new token `--cd-status-forced` — planner decides. Defensive rendering only since v1.1 UI can't produce FORCED ON rows.
- **Order of buttons in the bulk action bar** — `[Disable] [Enable] [Clear]` (recommended) vs `[Clear] [Enable] [Disable]` (destructive-rightmost convention). Minor call; planner picks.
- **Toast variant when `job_ids` is empty** (operator managed to POST with zero IDs somehow — shouldn't happen through the UI, but) — 400 with `"no jobs selected"` OR silently 200 + `"0 jobs changed"` toast. Planner picks.
- **Whether `bulk_toggle` needs a dedicated prometheus counter** — probably not for v1.1; `SchedulerCmd::Reload` already increments the existing reload counter. Planner confirms no new metric is needed.
- **Exact `cliff.toml` section grouping for ERG-* commits in v1.1.0 cumulative notes** — `git-cliff` default grouping is fine; don't over-customize (same stance as Phase 13 D-23).
- **Release-notes bullet for v1.1.0 headline feature list** — if the cumulative `git-cliff` output needs a cosmetic tweak (e.g., reordering so bulk toggle isn't buried), do it in `cliff.toml` once, not via hand-edit of the GitHub Release body.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (researcher, planner, executor) MUST read these before planning or implementing.**

### Phase 14 scope and requirements
- `.planning/ROADMAP.md` § "Phase 14: Bulk Enable/Disable + rc.3 + Final v1.1.0 Ship" — phase goal, depends-on, locked design decisions from ARCHITECTURE.md §3.7 (Option b), success criteria.
- `.planning/ROADMAP.md` § "Strict Dependency Order" item #4 — Bulk toggle lands last because it touches `sync_config_to_db`, the highest-regression-risk path in the scheduler.
- `.planning/ROADMAP.md` § "rc cut points" — `v1.1.0-rc.3` ◀ (tag cut after Phase 14 implementation), `v1.1.0` promoted after UAT. `:latest` advances from `v1.0.1` to `v1.1.0` on this tag.
- `.planning/REQUIREMENTS.md` § DB-14 — `enabled_override` tri-state column semantics; `upsert_job` does NOT touch it; `disable_missing_jobs` clears it; `T-V11-BULK-01` test lock.
- `.planning/REQUIREMENTS.md` § ERG-01..04 — multi-select dashboard UX, CSRF-gated bulk-toggle endpoint, running-jobs-not-terminated, settings audit surface, reload invariant.
- `.planning/REQUIREMENTS.md` § Traceability — `T-V11-BULK-01` covers the upsert-does-not-touch-override + reload-preserves-override invariants.
- `.planning/PROJECT.md` § "v1.1 — Operator Quality of Life" — iterative rc strategy, `:latest` pinning policy until final v1.1.0, semver pre-release notation (`vX.Y.Z-rc.N`), bulk-disable design resolved (no re-discussion).
- `.planning/PROJECT.md` § "Requirements > Active > Ergonomics" — "Operator can multi-select jobs from the dashboard and bulk enable/disable them".

### Carried decisions from earlier phases (MUST honor)
- `.planning/phases/10-stop-a-running-job-hygiene-preamble/10-CONTEXT.md` § D-08 — `--cd-status-*` tokens including `--cd-status-disabled`, `cd-badge--*` styles. Phase 14 reuses unchanged for the override-state badges in the settings audit table.
- `.planning/phases/10-stop-a-running-job-hygiene-preamble/10-CONTEXT.md` § D-12 — `Cargo.toml` version already at `1.1.0` (from FOUND-13 first commit). Phase 14 does NOT re-bump; the rc.3 tag matches `1.1.0`, and the final `v1.1.0` matches the same base.
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-CONTEXT.md` § three-step migration pattern — Phase 14 does NOT need the 3-step dance because `enabled_override` is nullable and needs no backfill; D-13 justifies the single-step deviation.
- `.planning/phases/12-docker-healthcheck-rc-1-cut/12-CONTEXT.md` § D-10 — release.yml `docker/metadata-action` tag-condition patches (pre-release gating for `:latest`, `:1.1`, `:1`; unconditional for `:{{version}}` and new `:rc` rolling). Phase 14 makes NO changes to this file; D-18 explicitly relies on this gating for the implicit `:latest` advancement.
- `.planning/phases/12-docker-healthcheck-rc-1-cut/12-CONTEXT.md` § D-11 — `docs/release-rc.md` maintainer runbook. Phase 14 follows it verbatim for both the rc.3 cut and the v1.1.0 promotion.
- `.planning/phases/12-docker-healthcheck-rc-1-cut/12-CONTEXT.md` § D-12 — `git-cliff` authoritative release notes, no hand-editing. Phase 14 D-15 (rc.3 delta) and D-19 (v1.1.0 cumulative) both honor this policy.
- `.planning/phases/12-docker-healthcheck-rc-1-cut/12-CONTEXT.md` § D-13 — manual-maintainer-tag-cut policy (not `workflow_dispatch`). Phase 14 D-16 honors this for the v1.1.0 promotion.
- `.planning/phases/12.1-ghcr-tag-hygiene/` (all planning artifacts in this directory) — `:latest` pinning policy + `:main` floating tag; `scripts/verify-latest-retag.sh` per-platform digest-diff tool Phase 14 D-18 uses for post-push verification.
- `.planning/phases/13-observability-polish-rc-2/13-CONTEXT.md` § D-22, D-23 — rc.2 release mechanics reuse pattern. Phase 14 mirrors exactly for rc.3.
- `.planning/phases/13-observability-polish-rc-2/13-CONTEXT.md` § D-07 — alphabetical ordering for stable row placement. Phase 14 D-10b applies the same principle to the settings overridden list.
- `.planning/STATE.md` § Accumulated Context — "Bulk-disable design resolved — `jobs.enabled_override` nullable tri-state" is locked; no re-discussion of the schema shape.

### Project-level constraints
- `/Users/Robert/Code/public/cronduit/CLAUDE.md` § "Constraints" — tech stack lock (`sqlx`, `askama_web 0.15` with `axum-0.8` feature, TOML config, terminal-green design system), rustls everywhere, mermaid-only diagrams, PR-only landing (no direct commits to main), full-semver tag format.
- `design/DESIGN_SYSTEM.md` § "Status Colors" — `--cd-status-disabled` + `--cd-status-running` tokens reused for the audit-table badges. NO new status tokens added in Phase 14.
- `design/DESIGN_SYSTEM.md` § "Components" — `cd-btn-primary`, `cd-btn-secondary`, `cd-badge` families reused for the action-bar buttons + override-state badges + per-row Clear button. Phase 14 MAY add `.cd-bulk-bar` and `.cd-row-checkbox` selectors for the new chrome.
- `THREAT_MODEL.md` — security posture. Phase 14 D-21 adds a one-line bullet about bulk-toggle's blast radius under UI access; same pattern as the Phase 10 Stop-button note.
- Auto-memory `feedback_diagrams_mermaid.md` — any diagram in Phase 14 PLAN.md, release notes, MILESTONES.md entry, or commit messages is mermaid, not ASCII.
- Auto-memory `feedback_no_direct_main_commits.md` — Phase 14 work lands via a feature branch (`gsd/phase-14-bulk-enable-disable` or similar) + PR.
- Auto-memory `feedback_uat_user_validates.md` — Phase 14 HUMAN-UAT.md steps require user validation; Claude does NOT mark UAT passed from automated runs.
- Auto-memory `feedback_uat_use_just_commands.md` — every step in `HUMAN-UAT.md` references an existing `just` recipe, not ad-hoc `cargo`/`docker`/`curl`.
- Auto-memory `feedback_tag_release_version_match.md` — `v1.1.0-rc.3` matches `Cargo.toml` base `1.1.0`; final `v1.1.0` also matches. Full semver form (`v1.1.0-rc.3`, not `v1.1.0-rc3`).

### Code integration points (verified against v1.0.1 + Phase 10/11/12/12.1/13 diffs)
- `src/db/queries.rs` — extend `DbJob` struct with `enabled_override: Option<i64>`; extend `SqliteDbJobRow` + `PgDbJobRow` with the same field; modify `get_enabled_jobs` to include the field AND update the SQL filter to `WHERE enabled = 1 AND (enabled_override IS NULL OR enabled_override = 1)`; modify `disable_missing_jobs` to include `enabled_override = NULL` in the SET clause when disabling jobs not in active_names; add `pub async fn bulk_set_override(pool: &DbPool, job_ids: &[i64], new_override: Option<i64>) -> anyhow::Result<u64>` for the bulk handler; add `pub async fn get_overridden_jobs(pool: &DbPool) -> anyhow::Result<Vec<DbJob>>` for the settings audit view.
- `src/db/queries.rs` line 57-125 — `upsert_job` stays EXACTLY as-is. NO `enabled_override` in the INSERT columns; NO `enabled_override` in the `ON CONFLICT DO UPDATE SET` clause. T-V11-BULK-01 asserts this invariant.
- `src/web/handlers/api.rs` — add `pub async fn bulk_toggle(State<AppState>, CookieJar, Form<BulkToggleForm>) -> Response` handler following the `stop_run`/`run_now`/`reroll` shape (CSRF check first, then DB op, then `SchedulerCmd::Reload` via the existing `mpsc` channel, then HX-Trigger toast). Reuse `crate::web::csrf::validate_csrf` (already imported at line 19).
- `src/web/handlers/settings.rs` — extend the view model with `overridden_jobs: Vec<OverriddenJobView>`; hydrate from `queries::get_overridden_jobs()`; pass to the template.
- `src/web/handlers/dashboard.rs` — add `enabled_override: Option<i64>` to `DashboardJobView`; carry through the `to_view()` pipeline (row source is `DbJob` which now has the field per above). No visible behavior change on the dashboard itself (the checkbox doesn't reflect override state), but downstream queries may use it if the planner wants to visually flag overridden rows (discretionary; not required by ERG-01).
- `src/web/mod.rs` — add `.route("/api/jobs/bulk-toggle", post(api::bulk_toggle))` to the router. One line.
- `src/scheduler/mod.rs` or wherever `SchedulerCmd` is defined — NO new variant needed; Reload already exists and is what D-11 fires. Confirm via grep pre-implementation.
- `templates/pages/dashboard.html` — add `<th>` for the leftmost checkbox column (select-all checkbox); add `<div class="cd-bulk-bar">` action bar between the filter-bar `<div>` (closes line 36) and the table `<div class="overflow-x-auto">` (line 39); bar contains hidden `<input name="csrf_token">`, a `<form hx-post="/api/jobs/bulk-toggle">` with action/job_ids hidden inputs populated by inline JS OR (preferred) the action-bar buttons use `hx-post` + `hx-vals` + `hx-include=".cd-row-checkbox:checked"` to collect IDs from the table body at submit time.
- `templates/partials/job_table.html` — add `<td><input type="checkbox" class="cd-row-checkbox" name="job_ids" value="{{ job.id }}"{% if preserve_selection %} hx-preserve="true"{% endif %}></td>` as the first `<td>` of every row.
- `templates/pages/settings.html` — add new `<section>` below line 71 containing `<h2>Currently Overridden</h2>` + description + table with `Name | Override State | Clear` columns iterating `{% for job in overridden_jobs %}`. Renders empty (section-hidden) when `overridden_jobs.is_empty()` per D-10a.
- `assets/static/app.css` — new selectors: `.cd-bulk-bar`, `.cd-row-checkbox`, `#cd-select-all` (styling/positioning), `.cd-badge--forced` (FORCED ON defensive rendering). `position: sticky` CSS for the action bar. NO global-selector changes.
- `migrations/sqlite/20260422_000004_enabled_override_add.up.sql` — NEW. `ALTER TABLE jobs ADD COLUMN enabled_override INTEGER NULL;`
- `migrations/postgres/20260422_000004_enabled_override_add.up.sql` — NEW. `ALTER TABLE jobs ADD COLUMN enabled_override BIGINT NULL;`
- `.planning/REQUIREMENTS.md` — flip ERG-01..04 + DB-14 checkboxes from `[ ]` to `[x]` as part of the Phase 14 close-out commit.
- `THREAT_MODEL.md` — append the D-21 one-line bulk-toggle blast-radius bullet.
- `docs/release-rc.md` — NO edits (Phase 12 artifact reused as-is for rc.3 AND v1.1.0 promotion).
- `scripts/verify-latest-retag.sh` — NO edits (Phase 12.1 artifact reused for D-18 post-push verification).
- `MILESTONES.md` — v1.1 archive entry added after final promotion per D-20. Exact shape follows the existing v1.0 entry.
- `README.md` — "Current State" paragraph update per D-20 (v1.1.0 becomes current stable).

### Test coverage
- `tests/` — new integration tests covering T-V11-BULK-01 (the upsert-does-not-touch-override invariant). Shape: seed job, set `enabled_override = 0` via the bulk handler, call `upsert_job` with modified config for that job, assert `enabled_override` is still `0`.
- `tests/` — reload-invariant test: seed job with `enabled_override = 0`, call `sync_config_to_db` with the job still in the config, assert `enabled_override` is still `0` AND assert `disable_missing_jobs` clears the override when the job is removed from active names.
- `tests/` — dashboard filter test: seed 3 jobs (2 with override = 0, 1 with override = NULL), call `get_enabled_jobs`, assert only the NULL-override job is returned.
- `tests/` — bulk handler integration: CSRF pass/fail, valid `action`, partial-invalid `job_ids` → 200 + toast with `(K not found)` suffix. Follows the `stop_run` integration-test pattern.
- Postgres parity tests (using `testcontainers-modules::postgres`) for every SQLite test in the Phase 14 scope. Structural-parity constraint honored same as prior phases.

### External references
- **context7 verified (2026-04-21):** `axum 0.8` `Form<T>` extractor deserializes `application/x-www-form-urlencoded` bodies with repeated keys (`job_ids=1&job_ids=2`) into `Vec<i64>` via `serde_urlencoded`. No extra crate needed. (`https://docs.rs/axum/0.8/axum/extract/struct.Form.html`)
- `docker/metadata-action` tag-condition docs — https://github.com/docker/metadata-action#tags-input (already relied on for D-18).
- `git-cliff` range syntax — `git cliff v1.0.1..v1.1.0` for D-19 cumulative notes; `git cliff --unreleased` for D-15 delta notes.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **CSRF cookie+form-field pattern** (`src/web/handlers/api.rs:32-42` in `run_now`; also `stop_run`, `reroll`, `reload`) — four existing mutation handlers already use the exact shape Phase 14 needs. `bulk_toggle` copies the skeleton verbatim; only the DB op differs.
- **`SchedulerCmd::Reload` + `state.scheduler_tx`** (used by `reload` handler) — Phase 14's handler dispatches Reload after the DB update to rebuild the scheduler heap; no new scheduler-loop arm required.
- **`HX-Trigger` toast pattern** (`stop_run` at `api.rs:428-445` shows the "normal path — toast + HX-Refresh" template) — Phase 14 uses the same shape for success toasts.
- **`cd-badge` family + `--cd-status-*` tokens** (`design/DESIGN_SYSTEM.md § Components` + `app.css`) — reused for DISABLED/FORCED ON badges in the settings audit table. Zero new status tokens.
- **`DbJob` + `DashboardJobView` pipeline** (`src/db/queries.rs:40-52` + `src/web/handlers/dashboard.rs`) — straightforward extension: one new nullable field, flows through existing hydration.
- **Idempotent migration discipline** (every existing migration under `migrations/{sqlite,postgres}/`) — `sqlx` migration tracking ensures re-run safety; no manual idempotency code needed in the single ALTER TABLE statement.
- **HTMX 3s table-body poll** (`templates/pages/dashboard.html:91-95`) — Phase 14 piggybacks. Selection state preservation via `hx-preserve="true"` on each `.cd-row-checkbox` (recommended Claude's Discretion pick).
- **`get_job_by_name` + `get_overridden_jobs`-like read-only query shape** (`src/db/queries.rs:194-215`) — template for the new `get_overridden_jobs` read-side query.

### Established Patterns
- **Reader pool for read queries, writer pool for mutations** (project CLAUDE.md + `DbPool::reader()` / `DbPool::writer()`) — Phase 14's new `get_overridden_jobs` uses reader; `bulk_set_override` + ALTER TABLE migration use writer.
- **SQLite uses `?N` placeholders; Postgres uses `$N`** (`src/db/queries.rs` end-to-end) — `bulk_set_override` follows the same split. SQLite needs a generated placeholder list for `WHERE id IN (?1, ?2, ...)`; Postgres uses `ANY($1)` with an `i64` array bind — same split already in use in `disable_missing_jobs` (lines 139-148 vs 160-167).
- **Form handlers return `impl IntoResponse`** with `(StatusCode, "...")` on validation failure — `bulk_toggle` follows the same shape.
- **Templates extend `base.html`; no new base template** — settings.html already extends; Phase 14 adds a section within the existing block.
- **askama compile-time template type safety** — `DashboardJobView` + `SettingsPageView` structs are the source-of-truth contracts between handler and template; Phase 14 extends both.
- **`feedback_uat_use_just_commands` + existing `justfile`** — every HUMAN-UAT.md step references a `just` recipe; if a needed recipe doesn't exist yet (e.g., `just compose-up-rc3`), the planner adds it in the implementation plan.

### Integration Points
- **`src/web/mod.rs` router** — single `.route("/api/jobs/bulk-toggle", post(api::bulk_toggle))` addition.
- **`src/web/handlers/mod.rs`** — no new module (bulk_toggle lives in `api.rs` alongside the other mutation handlers).
- **`src/db/queries.rs`** — three new functions (`bulk_set_override`, `get_overridden_jobs`, and the modification to `get_enabled_jobs` + `disable_missing_jobs`). One modification to `DbJob` struct shape.
- **`src/web/handlers/dashboard.rs` `to_view()`** — add `enabled_override` field to `DashboardJobView` (carried but not necessarily rendered; optional flag for the planner).
- **`src/web/handlers/settings.rs`** — extend view model with `overridden_jobs`; hydrate via `get_overridden_jobs`.
- **`templates/pages/dashboard.html`** — new `<th>` checkbox column, new `<div class="cd-bulk-bar">` action bar.
- **`templates/partials/job_table.html`** — new leading `<td>` with row checkbox.
- **`templates/pages/settings.html`** — new `<section>` after the 6-card grid.
- **`assets/static/app.css`** — extend with `.cd-bulk-bar`, `.cd-row-checkbox`, `#cd-select-all`, `.cd-badge--forced`. No global style changes.
- **`migrations/sqlite/` + `migrations/postgres/`** — one new migration file per backend.
- **`THREAT_MODEL.md`** — one-line bullet append.
- **`MILESTONES.md`** + **`README.md`** — v1.1 archive entry + current-stable note (final-promotion commit only).

</code_context>

<specifics>
## Specific Ideas

- **Literal REQ wording honored (ERG-01):** "multi-select of jobs via checkboxes and a 'Disable selected' / 'Enable selected' action bar" → D-01 (checkbox column) + D-02 (action bar inline sticky) + D-08 (both buttons always visible).
- **Literal REQ wording honored (ERG-02):** "Bulk disable does NOT terminate running jobs — running instances complete naturally. The success toast communicates this explicitly" → D-04 (verbose toast with conditional "will complete naturally" sentence).
- **Literal REQ wording honored (ERG-03):** "The settings page shows a 'Currently overridden' section listing every job whose `enabled_override` is non-null" → D-09 + D-10 + D-10a (empty-state hides section).
- **Literal REQ wording honored (ERG-04):** "A reload (SIGHUP / API / file-watch) does NOT reset `enabled_override`. A job that is present in the config file AND has `enabled_override = 0` stays disabled. A job that is absent from the config (e.g. removed by the operator) has its `enabled_override` cleared at the same time as `enabled` is set to 0" → D-05 (enable=clear-to-NULL symmetric with disable_missing_jobs behavior) + migration + modified `disable_missing_jobs`.
- **Literal DB-14 invariant honored:** "`upsert_job` does NOT touch this column in its `ON CONFLICT DO UPDATE` SET clause" → D-13 explicitly states the query at lines 57-125 of `queries.rs` stays as-is; T-V11-BULK-01 asserts this.
- **Symmetry with Phase 10/11/12/13:** All prior v1.1 phases kept design changes additive. Phase 14 continues: one new column (nullable), new selectors (`.cd-bulk-bar`, `.cd-row-checkbox`), one new handler — NO renamed selectors, NO deleted columns, NO removed tokens.
- **Symmetry with Phase 12:** Release mechanics fully reused. D-14 explicitly "no new release-engineering work"; rc.3 AND v1.1.0 both follow `docs/release-rc.md` verbatim. Same stance as Phase 13 D-22.
- **Schema consistency:** `enabled_override INTEGER NULL` on SQLite + `enabled_override BIGINT NULL` on Postgres mirrors the existing `enabled` column type split (SQLite stores booleans as INTEGER; Postgres stores them as BIGINT via `i32`/`i64` deserialization in the FromRow impls at `queries.rs:217-280`-ish). Same dialect pair pattern locked in Phase 11's `job_run_number` column.
- **Auto-memory specific (`feedback_uat_use_just_commands`):** D-17 HUMAN-UAT.md shape explicitly requires `just` recipes for every step — e.g., `just compose-up-rc3`, `just reload`. If a recipe doesn't exist, planner adds it to the justfile.
- **Auto-memory specific (`feedback_uat_user_validates`):** D-17 + Success Criterion #5 both explicitly require user validation; Claude does NOT mark UAT passed from compose-smoke or integration-test green alone.
- **Auto-memory specific (`feedback_tag_release_version_match`):** `v1.1.0-rc.3` matches `Cargo.toml = "1.1.0"`; final `v1.1.0` also matches. Full semver form (`v1.1.0-rc.3`, not `v1.1.0-rc3`) per Phase 10/11/12/13 precedent.
- **Auto-memory specific (`feedback_diagrams_mermaid`):** Any diagram in Phase 14 artifacts (PLAN.md, release notes, MILESTONES.md entry) is mermaid; no ASCII art.

</specifics>

<deferred>
## Deferred Ideas

- **Force-enable via the UI (override=1)** — schema supports it, UI does not write it in v1.1. Revisit in v1.2 if a use case appears (e.g., "operator wants a job ON even when `enabled = false` in config"). D-05 rejected two-button "Clear override / Force enable" split.
- **Shift-click range selection on row checkboxes** — requires custom JS beyond vendored HTMX. Power-user ergonomic; not load-bearing. Revisit in v1.2 alongside any other JS-dependent UX polish.
- **Per-row override timestamp (`enabled_override_set_at` column)** — adds a second column to the jobs table + more migration surface. D-10 rejected for the v1.1 settings audit surface. Revisit in v1.2 if audit demand appears.
- **Terminating running jobs on bulk disable** — explicitly rejected by REQ-02. Operators who want to kill a running job use the Phase 10 Stop button per SCHED-14.
- **Bulk action with per-job name list in the toast** — wraps awkwardly for larger selections; REQ-lock wording calls for counts only. Revisit if operators request it.
- **Per-row inline override state indicator on the dashboard** (e.g., dim the row, or show an "overridden" micro-badge) — not required by ERG-01..04. Operators see override state on the settings page. Discretionary; planner may add a subtle visual cue if trivially cheap.
- **Bulk operation on job config file (edit config to disable)** — explicitly rejected by the entire "config is read-only source of truth" design from v1.0. Revisit only if the overall config-source model changes (it won't in v1.1 or v1.2).
- **Transition-diff reporting in toast ("2 newly disabled, 3 already disabled")** — extra query cost + clutter. Simpler idempotent behavior (D-07) is consistent with Run Now / Stop / reroll.
- **207 Multi-Status response for partial-failure bulk requests** — overkill for a homelab tool. Best-effort 200 + suffix (D-12) is sufficient.
- **Soak period / multi-day rc.3 canary before v1.1.0 promotion** — D-17 rejected. Iterative cadence keeps the milestone moving; HUMAN-UAT.md is the signoff gate.
- **New commit between rc.3 and v1.1.0 bumping MILESTONES.md** — D-16 rejected (breaks bit-identical-image guarantee). MILESTONES.md update lands on the v1.1.0-tagged commit OR as a follow-up commit on main AFTER the tag is pushed.
- **Hand-edited v1.1.0 release notes** — D-19 rejected (violates Phase 12 D-12 `git-cliff` authoritative policy). Use `cliff.toml` for any structural changes, not hand-edits.
- **`workflow_dispatch`-driven tag cut for v1.1.0** — D-16 rejected (Phase 12 D-13 trust-anchor stance).
- **Separate `/settings/overrides` sub-page** — D-09 rejected (extra click cost, ERG-03 discoverability prefers in-place visibility).
- **Conditional action-bar button visibility based on selected-row override states** — D-08 rejected (requires HTMX action-bar refresh per selection change; complex). Idempotency (D-07) makes both-visible safe.
- **Email / webhook notification on bulk-toggle action** — out of v1.1 scope entirely (deferred to v1.2 per PROJECT.md § Future Requirements).

</deferred>

---

*Phase: 14-bulk-enable-disable-rc-3-final-v1-1-0-ship*
*Context gathered: 2026-04-21*
