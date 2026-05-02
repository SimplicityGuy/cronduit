# Phase 21: Failure-Context UI Panel + Exit-Code Histogram Card — rc.2 - Context

**Gathered:** 2026-05-01
**Status:** Ready for planning

<domain>
## Phase Boundary

Two new read-only UI surfaces backed by existing schema + query helpers, plus one minimal additive schema change to land FCTX-06, plus the v1.2.0-rc.2 tag cut:

1. **Failure-Context Panel** on `templates/pages/run_detail.html` — collapsed-by-default `<details>` panel with 5 P1 signal rows; gated to `status ∈ {failed, timeout}` only.
2. **Exit-Code Histogram Card** on `templates/pages/job_detail.html` — sibling card to v1.1's Duration card; success-rate stat badge + 10-bucket histogram + recent-codes sub-table.
3. **`v1.2.0-rc.2` tag cut** at phase end via the existing `docs/release-rc.md` runbook.

The full visual + interaction contract is **already locked** in `21-UI-SPEC.md` (approved 2026-05-01) — typography, color, spacing, markup, copy, accessibility, tooltip reuse, bucket→token mapping, empty-state copy, print-mode behavior. Phase 21 implementation is plumbing-only: schema add, scheduler write site, query/aggregator helpers, handler wire-up, template inserts, integration tests, just-recipe UAT, rc.2 cut.

**In scope (Phase 21):**
- One-file additive `job_runs.scheduled_for TEXT NULL` migration (sqlite + postgres mirror) — sourced exclusively for FCTX-06 fire-skew. NO three-file tightening; NULL on legacy rows is intentional.
- Scheduler `insert_running_run` widening to write `scheduled_for` at the fire-decision moment with trigger-aware semantics (Run Now / api triggers write `scheduled_for = start_time` for skew == 0ms by definition).
- `src/web/exit_buckets.rs` new module (sibling to `stats.rs`) — `pub fn categorize(status, exit_code) -> ExitBucket` (10-variant enum) + `pub fn aggregate(rows) -> HistogramCard`. Status-discriminator-wins classifier (status='stopped' → Stopped bucket regardless of exit_code 137; status!='stopped' + exit_code=137 → 128-143 bucket).
- Handler wire-up in `src/web/handlers/run_detail.rs` (call `get_failure_context()` gated to failed/timeout; soft-fail on Err) and `src/web/handlers/job_detail.rs` (raw fetch of last-100 ALL runs + Rust-side aggregate; soft-fail on Err).
- askama template inserts on `run_detail.html` (panel between metadata card and Log Viewer per UI-SPEC layout diagram) and `job_detail.html` (card between Duration card and Run History per UI-SPEC layout diagram).
- CSS additions in `assets/src/app.css` `@layer components`: every class declared in UI-SPEC § Component Inventory (`cd-fctx-*` family + `cd-exit-*` family). NO new tokens. Reused tokens listed in UI-SPEC § Tokens Existing Reuse.
- Integration tests: `tests/v12_fctx_panel.rs` (panel render gating, all 5 rows including fire-skew, never-succeeded edge case, NULL scheduled_for hides row), `tests/v12_exit_histogram.rs` (10-bucket coverage, status/137 dual-classifier, EXIT-05 top-3 last-seen, below-N=5 empty state, success-rate excludes-stopped formula). Extend `tests/v12_fctx_explain.rs` to confirm new column doesn't change index-plan assertions.
- Three new `just` recipes: `uat-fctx-panel`, `uat-exit-histogram`, `uat-fire-skew`. Mirror P18/P19/P20 `uat-webhook-*` family naming + `recipe-calls-recipe` pattern.
- `21-HUMAN-UAT.md` with maintainer-validated scenarios (autonomous=false): panel render on real failed/timeout runs, panel hidden on success/cancelled/running/stopped, histogram bucket distribution on real exit codes, EXIT-04 stopped-vs-signal-killed distinction, never-succeeded job rendering, below-threshold empty state, fire-skew on a delayed-fire job.
- `21-RC2-PREFLIGHT.md` autonomous=false maintainer plan for the v1.2.0-rc.2 tag cut. Reuses `docs/release-rc.md` verbatim (no runbook edits per D-15 / mirroring P20 D-30).
- `cargo tree -i openssl-sys` must remain empty — Phase 21 adds zero new external crates.

**Out of scope (deferred to other phases):**
- THREAT_MODEL.md TM5 (Webhook Outbound) and TM6 (Operator-supplied Docker labels) — Phase 24 milestone close-out per ROADMAP.
- Job tagging schema + dashboard chips — Phase 22 / Phase 23.
- HTMX live re-poll of the histogram card — read-only, page-navigation-only render. Operators reload to refresh.
- HTMX live re-poll of the FCTX panel — same; render is server-side at page load.
- Dashboard re-render with FCTX summary on the job card — out of scope; FCTX is run-detail-only.
- Webhook payload extension to carry the new fire-skew signal — webhook payload schema is locked at v1.2.0 per P18 D-17; revisit in v1.3.
- Per-job exit-code Prometheus label — explicit accepted-out-of-scope per EXIT-06 (cardinality discipline).
- Trace ID / correlation ID surfaces in the panel — out of scope.
- Operator-tunable bucket boundaries — locked at 10 buckets per EXIT-02.
- Re-running a failed run from the panel ("retry button") — read-only panel; v1.3 candidate.
- Pruning `scheduled_for` from old rows — NULL is the intended legacy state; the column is additive.
- `release.yml` / `cliff.toml` / `docs/release-rc.md` modifications for v1.2-specific rc behavior — reused verbatim per D-15 (mirroring P20 D-30).

</domain>

<decisions>
## Implementation Decisions

### FCTX-06 Fire-Skew Data Source (Gray Area 1; FCTX-06)
- **D-01:** **Add `job_runs.scheduled_for TEXT NULL` column.** One-file additive migration per backend (`migrations/{sqlite,postgres}/2026XXXX_NNNNNNN_scheduled_for_add.up.sql`). RFC3339 string convention to match `start_time` / `end_time` (project-wide). NO index — skew is read on a single-row select; no fleet-level filter consumes the column.
- **D-02:** Scheduler write point: `insert_running_run` is widened to accept `scheduled_for: Option<DateTime>` and persists it. The scheduler computes the value at fire-decision time (the croner `next_after` value, or for `@random` jobs the resolved-slot time). Run Now and api-triggered runs write `scheduled_for = start_time` (skew = 0ms by definition).
- **D-03:** Trigger-aware semantics live at the SCHEDULER call site, not in the DB layer or handler. Single write point in `insert_running_run`; the scheduler picks the right value before calling. Researcher confirms whether this also requires widening the wave-3-write-site downstream of `insert_running_run` per the existing P16 `finalize_run` widening pattern.
- **D-04:** **Legacy NULL handling: hide the FIRE SKEW row entirely** when `scheduled_for IS NULL` (i.e., on v1.0/v1.1 runs that pre-date the column). Mirrors the IMAGE DIGEST row's hide-on-non-docker behavior per UI-SPEC § Component Inventory row table. NO backfill — NULL is the intended legacy state.
- **D-05:** Migration shape: **one-file additive only** (NOT three-file tightening). The column stays nullable forever — `@random` slot resolution + Run Now both produce a valid value, but legacy rows must remain expressible. Mirrors P16 image_digest + config_hash `_add` migration shape exactly. Researcher picks the timestamp prefix continuing the existing `2026XXXX_NNNNNNN_*` sequence.

### Exit-Code Histogram Query Shape (Gray Area 2; EXIT-01..EXIT-05)
- **D-06:** **Rust-side bucketing over last-100 raw rows.** Single SELECT (`SELECT id, status, exit_code, end_time FROM job_runs WHERE job_id=? ORDER BY start_time DESC LIMIT 100`); aggregation happens in Rust. Mirrors the v1.1 OBS-04 percentile pattern exactly (raw-fetch-plus-Rust-math); structurally identical on sqlite/postgres without dialect-specific CASE arms; reuses the existing `idx_job_runs_job_id_start` index. NO server-side bucketing.
- **D-07:** **`src/web/exit_buckets.rs` new module** sibling to `stats.rs`. Public surface:
  - `pub enum ExitBucket { Bucket1, Bucket2, Bucket3to9, Bucket10to126, Bucket127, Bucket128to143, Bucket144to254, Bucket255, BucketNull, BucketStopped }` — 10 variants matching UI-SPEC § Color bucket→token table. NOTE: `0` (success) is NOT a bucket variant — it's a separate stat per EXIT-03.
  - `pub fn categorize(status: &str, exit_code: Option<i32>) -> Option<ExitBucket>` — returns `None` when status='success' (the success path; routed to the EXIT-03 success-rate stat instead).
  - `pub fn aggregate(rows: &[(&str, Option<i32>, Option<&str>)]) -> HistogramCard` — bucket counts, top-3 codes with last-seen, success-rate stat, sample_count, has_min_samples flag.
- **D-08:** **Status discriminator wins; exit_code is secondary.** `categorize` rules (locked):
  - `status == "success"` → `None` (handled by success-rate stat per EXIT-03)
  - `status == "stopped"` → `BucketStopped` (cronduit's SIGKILL path; tooltip says "NOT a crash" per UI-SPEC). Regardless of exit_code (which IS 137 for cronduit-issued stops).
  - `exit_code IS NULL` (no terminal exit code recorded) → `BucketNull` (covers timeout/cancelled-without-code edge cases)
  - `exit_code == Some(1)` → `Bucket1`; `Some(2)` → `Bucket2`; `Some(127)` → `Bucket127`; `Some(255)` → `Bucket255`
  - `exit_code ∈ 3..=9` → `Bucket3to9`; `10..=126` → `Bucket10to126`; `128..=143` → `Bucket128to143` (this is where externally signal-killed runs land — status='failed' + exit_code=137 from outside-cronduit signal goes here, NOT into BucketStopped); `144..=254` → `Bucket144to254`
- **D-09:** **EXIT-03 success-rate formula:** `success_count / (sample_count - stopped_count)`. Numerator = runs with status='success'. Denominator excludes stopped (operator stop is not a failure; mirrors v1.1 OBS-03 dashboard sparkline rule). When denominator is 0 (everything was stopped), render `—` for the stat.
- **D-10:** **EXIT-05 top-3 codes with last-seen** are computed Rust-side from the same raw row buffer. `aggregate` returns `Vec<TopCode>` sorted by count DESC, length ≤ 3. Uses the raw `exit_code` value (e.g., `137`, `127`, `143`) keyed in a `HashMap<i32, (count, latest_end_time)>`; renders per UI-SPEC § Copywriting Contract recent-codes table.
- **D-11:** **N=5 sample threshold** counts the full last-100 raw row buffer length. `has_min_samples = sample_count >= 5`. Below threshold the card renders the locked `—` empty state per UI-SPEC § Copywriting Contract.

### Render-Path Error Handling (Gray Area 3)
- **D-12:** **Soft-fail with `tracing::warn!`.** Both `get_failure_context()` and the histogram aggregate query soft-fail on DB error: the surface is omitted from render (FCTX panel hidden entirely; histogram card shows the locked `—` empty state with a "Need 5+ samples; have N" copy substituted to a degraded "—" presentation). Each emits a warn-level structured log with `run_id` / `job_id` / error context. Rest of the page renders normally. Operators learn from logs + `/metrics`, not from a broken page.
  Consistent with the v1.1 sparkline soft-fail in OBS-03 (the dashboard sparkline silently no-ops when its query fails).
- **D-13:** **Never-succeeded job rendering** (the `last_success_*` fields are NULL because the job has never produced a success):
  - Panel renders if status ∈ {failed, timeout} (gating unchanged per FCTX-01).
  - TIME DELTAS row: renders with copy `First failure: {ts} ago • {N} consecutive failures • No prior successful run` (locked in UI-SPEC § Copywriting Contract).
  - IMAGE DIGEST row (docker only): hides — no baseline to compare against.
  - CONFIG row: hides — `last_success.config_hash` is NULL; the comparison is undefined.
  - DURATION row: hides — `percentile()` returns None on empty input; UI-SPEC FCTX-05 row already specifies suppression below 5 successful samples.
  - FIRE SKEW row: renders if `scheduled_for IS NOT NULL` (independent of success history).
- **D-14:** **Config-hash compare semantics:** literal `run.config_hash != last_success.config_hash` per P16 D-05. Both are per-run snapshots; no comparison to the current effective config. If the operator reloaded mid-flight, the failing run's snapshot is what fired — that's the relevant comparison.
- **D-15:** **Below-N=5 empty state** for the histogram: render outer chrome (heading + outer border) + the locked `—` empty state with `Need 5+ samples; have {N}` copy. NO success-rate stat, NO histogram bars, NO recent-codes table. Card is always present so operators don't wonder where it went.
- **D-16:** **Brand-new job** (zero runs): same as below-N=5 — the empty state copy substitutes `Need 5+ samples; have 0`. Locked.
- **D-17:** Both surfaces use askama auto-escaping per UI-SPEC § Output Escaping & XSS. NO `|safe` filters. NO new inline `<script>` blocks (per UI-SPEC).

### Test + UAT Shape (Gray Area 4)
- **D-18:** **Two new integration test files + extend the P16 explain test.**
  - `tests/v12_fctx_panel.rs` covers: panel render gating (renders on failed/timeout, hides on success/cancelled/running/stopped), all 5 rows including fire-skew with NULL/non-NULL `scheduled_for`, never-succeeded edge case (degraded-rows behavior per D-13), config_hash literal compare per D-14, soft-fail on simulated DB error.
  - `tests/v12_exit_histogram.rs` covers: 10-bucket categorize coverage (one row per bucket, including all corner exit codes 0/1/2/3/9/10/126/127/128/137/143/144/254/255/null), status='stopped'+exit_code=137 → BucketStopped, status='failed'+exit_code=137 → Bucket128to143 (EXIT-04 dual-classifier), EXIT-05 top-3 last-seen, success-rate excludes-stopped formula per D-09, below-N=5 empty state per D-15, sample_count==0.
  - Extend `tests/v12_fctx_explain.rs` (from P16) to assert the new `scheduled_for` column doesn't shift the existing `idx_job_runs_job_id_start` index plans on either backend.
- **D-19:** **Three new `just` recipes** mirroring the P18/P19/P20 family pattern (`uat-webhook-*`, `uat-fctx-bugfix-*`):
  - `uat-fctx-panel` — seed N failing runs against a docker job and a non-docker job → start cronduit → walk to `/jobs/{id}/runs/{id}` for each → confirm panel collapsed-by-default → expand and confirm 5 rows including image-digest hide-on-non-docker.
  - `uat-exit-histogram` — seed mixed-exit runs (success, code-1, code-127, code-137 stopped, code-137 external-kill, timeout, null) → walk to `/jobs/{id}` → confirm 10-bucket distribution + success-rate badge + recent-codes table + tooltip on hover.
  - `uat-fire-skew` — artificially delay a fire (e.g., a `* * * * *` job whose receiver is stalled or whose scheduler is backpressured) → confirm FIRE SKEW row renders with non-zero skew. Test plan picks the artificial-delay technique (researcher decides between a sidecar that holds open a lock vs a slow-start container).
  Recipes follow the `recipe-calls-recipe` pattern (P18 D-25 precedent — each `uat-*` recipe orchestrates seed → run → walk → assert via existing recipes like `dev-build`, `dev-run`, `seed-fixture-runs`).
- **D-20:** **`21-HUMAN-UAT.md` autonomous=false** maintainer plan covering the scenarios above plus mobile viewport (panel rows stack 1-column below 640px; histogram horizontally scrolls below 640px), light-mode rendering, print-mode (`@media print` opens the panel), keyboard-only navigation (Tab + Space/Enter on summary; Tab onto bars). Each step references an existing `just` recipe per project memory `feedback_uat_use_just_commands.md`.
- **D-21:** **No new top-level structural CI changes** — the existing `linux/{amd64,arm64} × {SQLite, Postgres}` matrix covers Phase 21. The new tests run inside the existing test job. No new feature flag. No new lint gate (existing `grep-no-percentile-cont` from v1.1 OBS-05 still applies — Phase 21 doesn't reach for `percentile_cont`).

### rc.2 Tag Cut (Release Engineering)
- **D-22:** **Reuse `docs/release-rc.md` verbatim** — identical to P20 D-28..D-31. Cargo.toml stays at `1.2.0` (P15 already set this). `:latest` GHCR tag stays at `v1.1.0` (the `release.yml` patch from P12 D-10 enforces this on tags containing `-`). The rolling `:rc` tag updates to `v1.2.0-rc.2` on push.
- **D-23:** Tag command: `git tag -a -s v1.2.0-rc.2 -m "v1.2.0-rc.2 — FCTX UI panel + exit-code histogram (P21)"`. Pre-flight checklist: P21 PR merged to `main` + green CI + green compose-smoke + `git cliff --unreleased --tag v1.2.0-rc.2` preview clean.
- **D-24:** Phase 21 does NOT modify `release.yml`, `cliff.toml`, or `docs/release-rc.md`. Any maintainer-discovered runbook gap during the rc.2 cut becomes a hotfix PR before tagging (mirrors v1.1 P12 + v1.2 P20 discipline).
- **D-25:** GitHub Release notes: `git-cliff` output is authoritative (per v1.1 P12 D-12). Phase 21 does NOT hand-edit the release body post-publish.
- **D-26:** Final wave is the autonomous=false `21-RC2-PREFLIGHT.md` — maintainer runs the human UAT scenarios from D-20 + cuts the v1.2.0-rc.2 tag locally per `docs/release-rc.md`. Plans 21-01..21-NN run autonomously through verification; rc.2 cut is maintainer-only. Mirrors P20 D-29.

### Universal Project Constraints (carried forward)

> The decisions below are **[informational]** — repo-wide process constraints honored by absence (mermaid-only diagrams, PR-only branch state, maintainer-validated UAT, just-recipe UAT). They are not phase-implementation tasks.

- **D-27:** [informational] All changes land via PR on a feature branch. No direct commits to `main`. Working branch: continues from the existing `phase21/ui-spec` branch the UI-SPEC was authored on (or planner picks a successor; the UI-SPEC commit history is already on the right branch).
- **D-28:** [informational] Diagrams in any Phase 21 artifact (PLAN, SUMMARY, README, code comments, mermaid blocks in PR description) are mermaid. No ASCII art. (Carries forward project memory `feedback_diagrams_mermaid.md`.)
- **D-29:** UAT recipes reference existing `just` commands. New recipes per D-19 follow `recipe-calls-recipe` (project memory `feedback_uat_use_just_commands.md`).
- **D-30:** [informational] Maintainer validates UAT — Claude does NOT mark UAT passed from its own runs (project memory `feedback_uat_user_validates.md`).
- **D-31:** [informational] Tag and version match — `Cargo.toml` is at `1.2.0`; the rc tag is `v1.2.0-rc.2`. Per project memory `feedback_tag_release_version_match.md` the in-source version stays unsuffixed; `-rc.2` is the tag-only suffix.
- **D-32:** [informational] Cronduit-side rustls invariant unchanged — `cargo tree -i openssl-sys` must remain empty. Phase 21 adds zero new external crates. The histogram uses pure-CSS bars + askama; the panel uses native `<details>`. No JS bundle, no SVG, no canvas.
- **D-33:** [informational] UI-SPEC.md is authoritative for visuals. Any visual deviation discovered during planning/implementation is a UI-SPEC.md amendment first, then code follows. Class names are the UI-SPEC contract (`cd-fctx-*` and `cd-exit-*` namespaces).

### Claude's Discretion
- Exact migration filename + timestamp prefix (Phase 21 starts a new sequence after `20260502_000008`).
- Internal struct names — `HistogramCard` is the public name for the aggregate output; field names planner picks.
- The `aggregate` row-tuple input shape `&[(&str, Option<i32>, Option<&str>)]` vs a dedicated `RawRunRow` struct — researcher decides based on `queries.rs` shape and existing tuple-vs-struct precedent.
- Whether `categorize` returns `Option<ExitBucket>` (skip success path) or `ExitBucket` with a `Success` variant the aggregator filters out. Both work; the former is slightly cleaner for the caller. Planner picks.
- The exact `tracing::warn!` field shape on the soft-fail path (`%error` vs `error.message`) — planner aligns with existing telemetry.rs patterns.
- The `uat-fire-skew` artificial-delay technique (sidecar container holding a lock vs slow-start container vs scheduler-loop sleep injected via test feature flag). Researcher picks the cleanest one that doesn't require new test infrastructure.
- Whether the `21-HUMAN-UAT.md` mobile/light/print/keyboard scenarios are split into individual `uat-fctx-mobile`, `uat-fctx-light`, `uat-fctx-print`, `uat-fctx-keyboard` recipes or rolled into a single `uat-fctx-a11y` umbrella. Planner picks.
- The exact wave structure (linear vs parallel waves) — researcher decides; the migration must land first (Wave 1), the helper module + handler wire-up + templates can run in parallel (Wave 2), tests + UAT recipes wave-end (Wave 3), human-UAT + rc.2 preflight final wave (Wave 4 autonomous=false).
- Whether to extend `src/web/handlers/run_detail.rs` `RunDetailContext` struct in-place or wrap it in a new `FailureContextSection` sub-struct exposed via askama template. Planner picks based on `RunDetailContext` size.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level locks
- `.planning/PROJECT.md` — core value, locked v1.2 decisions, Tech Stack constraints (rustls everywhere, mermaid diagrams, PR-only workflow, just-recipe UAT, single-binary, server-rendered HTML)
- `.planning/REQUIREMENTS.md` § Failure Context (FCTX) — `FCTX-01`, `FCTX-02`, `FCTX-03`, `FCTX-05`, `FCTX-06` are Phase 21's FCTX requirements; `FCTX-04` and `FCTX-07` already shipped in P16
- `.planning/REQUIREMENTS.md` § Per-Job Exit-Code Histogram (EXIT) — `EXIT-01`, `EXIT-02`, `EXIT-03`, `EXIT-04`, `EXIT-05`, `EXIT-06` (all Phase 21)
- `.planning/STATE.md` § Accumulated Context — v1.2 milestone state, prior phase decisions, working-branch context
- `.planning/ROADMAP.md` § Phase 21 — goal + 5 success criteria + dependency on Phase 16 + rc.2 cut commitment
- `./CLAUDE.md` — project conventions, locked tech stack, mermaid-only, PR-only workflow, GSD enforcement

### Phase 21 design contract (locked at UI-SPEC step — MUST NOT be re-derived)
- `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-UI-SPEC.md` — **AUTHORITATIVE** for visuals: typography (4 active sizes), color (60/30/10 split + bucket→token mapping table), spacing (4px-grid token-derived), markup contracts for both surfaces (`<details>` + pure-CSS bars), copy lines, accessibility, tooltip reuse, bucket short-labels, empty-state copy, print-mode behavior. Approved 2026-05-01 by gsd-ui-checker (6/6 dimensions PASS). Class names are the contract (`cd-fctx-*`, `cd-exit-*` namespaces).

### Phase 16 (FCTX schema + query helper — already shipped)
- `.planning/phases/16-failure-context-schema-run-rs-277-bug-fix/16-CONTEXT.md` — D-05 config_hash compare semantics; D-04 image_digest backfill posture; FailureContext struct shape; query helper landing site
- `src/db/queries.rs` — `get_failure_context(pool, job_id) -> FailureContext` (lines ~681-750); `FailureContext` struct (lines ~636-657) with `streak_position`, `consecutive_failures`, `last_success_run_id`, `last_success_image_digest`, `last_success_config_hash`. **Phase 21 consumes this verbatim** in the run_detail handler. NO modifications to the helper or struct.
- `migrations/sqlite/20260427_000005_image_digest_add.up.sql` + postgres mirror — one-file additive precedent for `scheduled_for_add` (D-05).
- `migrations/sqlite/20260428_000006_config_hash_add.up.sql` + postgres mirror — one-file additive precedent.
- `tests/v12_fctx_explain.rs` — Phase 21 EXTENDS to assert the new `scheduled_for` column doesn't shift `idx_job_runs_job_id_start` index plans (D-18).

### Phase 13 (P50/P95 percentile helper + tooltip CSS — already shipped)
- `src/web/stats.rs::percentile(samples, q) -> Option<u64>` (lines 13-22) — Phase 21 reuses verbatim for FCTX-05 duration deviation. The N≥20 minimum from OBS-04 does NOT apply to FCTX — UI-SPEC FCTX-05 specifies N≥5 (suppress below 5 successful runs).
- `assets/src/app.css` — `.cd-tooltip`, `.cd-tooltip-row`, `.cd-tooltip-dot` classes (Phase 13 timeline). Phase 21 reuses verbatim per UI-SPEC § Component Inventory tooltip-reuse note.
- `src/web/handlers/job_detail.rs::DurationCardContext` — sibling-card precedent for the Phase 21 ExitHistogramCardContext shape and the soft-fail call-site pattern.

### v1.1 (Stop status + status colors + sparkline soft-fail — already shipped)
- `src/scheduler/run.rs` (Phase 10 stop semantics) — `status='stopped'` is distinct from `cancelled`/`failed`/`timeout` AND emits exit_code=137 from cronduit's SIGKILL. Phase 21's status-discriminator-wins classifier (D-08) depends on this distinction.
- `assets/src/app.css` — `--cd-status-stopped`, `--cd-status-stopped-bg` color tokens (v1.1 Phase 10).
- `src/web/handlers/dashboard.rs` — sparkline soft-fail pattern (Phase 13 OBS-03). Phase 21's D-12 soft-fail mirrors this.

### Phase 12 / 12.1 (rc-cut release-engineering precedent — v1.1)
- `.planning/milestones/v1.1-phases/12-docker-healthcheck-rc-1-cut/12-CONTEXT.md` — D-10/D-11/D-12/D-13 rc-cut decisions (release.yml `:latest` gating, runbook structure, git-cliff authoritative, maintainer-not-workflow_dispatch trust anchor)
- `docs/release-rc.md` — runbook itself; reused verbatim (D-22). NO modifications in Phase 21.
- `.github/workflows/release.yml` — `:latest` gated to skip on tags containing `-` (per P12 D-10); Phase 21 does NOT modify this file (D-24).

### Phase 20 (rc.1 precedent for rc.X discipline — already shipped)
- `.planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-CONTEXT.md` — D-28/D-29/D-30/D-31 rc.1 cut decisions; Phase 21 D-22..D-26 mirror these one-to-one (substitute rc.1 → rc.2, P20 → P21).
- `.planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-RC1-PREFLIGHT.md` — preflight document precedent for `21-RC2-PREFLIGHT.md`.

### Existing Cronduit infra to reuse
- `Cargo.toml` — `tokio` (full), `chrono`, `tracing`, `serde`, `sqlx` (sqlite + postgres + rustls features), `askama_web 0.15` (axum-0.8 feature). Phase 21 adds ZERO new external crates. `cargo tree -i openssl-sys` must remain empty (D-32).
- `templates/pages/run_detail.html` — Phase 21 inserts the failure-context panel between the metadata card and the Log Viewer block per UI-SPEC § Layout & Surfaces diagram. The existing inline `<script>` for log streaming is NOT extended (UI-SPEC § Output Escaping & XSS).
- `templates/pages/job_detail.html` — Phase 21 inserts the exit-code-histogram card between the existing Duration card and the Run History block per UI-SPEC § Layout & Surfaces diagram. Sibling card chrome matches Duration card exactly.
- `src/web/handlers/run_detail.rs` — Phase 21 wires `get_failure_context()` call here, gated to `status ∈ {failed, timeout}` (FCTX-01). Soft-fail per D-12.
- `src/web/handlers/job_detail.rs` — Phase 21 wires the histogram aggregate here. Already houses the Duration card percentile call site — Phase 21's wire-up sits adjacent.
- `src/web/mod.rs` — Phase 21 adds `pub mod exit_buckets;` next to the existing `pub mod stats;`.
- `src/scheduler/run.rs` — Phase 21 widens the `insert_running_run` call site(s) with the `scheduled_for` value computed at fire-decision time per D-02/D-03.
- `src/db/queries.rs::insert_running_run` — Phase 21 widens this signature (current shape: `pub async fn insert_running_run(pool, job_id, status, trigger, start_time, job_run_number, config_hash) -> Result<RunId>`) to accept `scheduled_for: Option<DateTime>`. Mirrors the P16 widening pattern (image_digest + config_hash both flowed in via signature changes per P16 D-04).
- `migrations/{sqlite,postgres}/` — one-file additive migration pattern. Phase 21 ships ONE pair of files for `job_runs.scheduled_for`.
- `justfile` — existing `uat-*` family for recipe-naming consistency. Phase 21 adds `uat-fctx-panel`, `uat-exit-histogram`, `uat-fire-skew` (D-19).
- `tests/v12_fctx_*.rs` — Phase 16 integration test precedent. Phase 21 adds `tests/v12_fctx_panel.rs` + `tests/v12_exit_histogram.rs` (D-18).
- `assets/src/app.css` — Phase 21 ADDS the `cd-fctx-*` and `cd-exit-*` class declarations into the existing `@layer components` block per UI-SPEC § Component Inventory CSS contract tables. NO new tokens. NO new font weights.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`get_failure_context()`** (`src/db/queries.rs:681`): Phase 16's single-query CTE'd helper returning `FailureContext { streak_position, consecutive_failures, last_success_run_id, last_success_image_digest, last_success_config_hash }`. Phase 21 consumes verbatim. No modification needed; the helper already returns the data the panel needs (TIME DELTAS row + IMAGE DIGEST row + CONFIG row).
- **`stats::percentile(samples, q)`** (`src/web/stats.rs:13`): Phase 13's `Option<u64>`-returning percentile. Phase 21 reuses for FCTX-05 duration deviation: `let p50 = stats::percentile(&durations_of_last_100_successful, 0.5)`. UI-SPEC FCTX-05 row spec already mandates suppression below 5 successful samples; the helper already returns `None` on empty input.
- **`.cd-tooltip` / `.cd-tooltip-row` / `.cd-tooltip-dot`** (Phase 13 timeline; declared in `assets/src/app.css`): Phase 21 reuses verbatim. UI-SPEC § Component Inventory locks the `.cd-exit-bar:hover .cd-tooltip` rule as the only addition needed (different anchor from Phase 13's `.cd-timeline-bar:hover`).
- **`--cd-status-stopped` color token** (v1.1 Phase 10; declared in `assets/src/app.css`): Phase 21 reuses for the BucketStopped histogram bar per UI-SPEC § Color and EXIT-04 spec literal.
- **`format_duration_ms_floor_seconds()`** (used in `src/web/handlers/job_detail.rs:259-260`): Phase 21 reuses for the FCTX-05 DURATION row value rendering ("12.3s; typical p50 is 4.2s (2.9× longer than usual)").
- **askama auto-escaping**: every user-controlled value (run.exit_code, image_digest, config_hash, error_message, scheduled_for) flows through default escape. UI-SPEC § Output Escaping & XSS already locked: NO `|safe` filters anywhere in Phase 21 templates.

### Established Patterns
- **Sibling-card chrome on job-detail**: `background: var(--cd-bg-surface); border: 1px solid var(--cd-border); border-radius: 8px; padding: var(--cd-space-6); margin-bottom: var(--cd-space-6);` — locked in UI-SPEC § Layout & Surfaces. The Duration card and the new ExitCard share this chrome verbatim.
- **One-file additive migration** (P16 image_digest_add + config_hash_add): Phase 21 mirrors the shape exactly for `scheduled_for_add` (D-05). Migration runner picks up the new file at first start post-merge.
- **Soft-fail with tracing::warn!**: dashboard sparkline pattern (Phase 13 OBS-03) — the surface returns the empty/degraded state on Err and emits a structured warn. Phase 21 D-12 mirrors this for both new surfaces.
- **CTE'd single-query**: P16's `get_failure_context` two-CTE shape — Phase 21 doesn't add any new query of this shape (the histogram uses a flat `SELECT ... LIMIT 100` per D-06).
- **rc.X tag cut maintainer-local**: P12 + P20 precedent. Phase 21 D-22..D-26 reuse verbatim.

### Integration Points
- **Where the `scheduled_for` write hooks in**: `src/scheduler/run.rs` at the call to `insert_running_run` — the scheduler computes the value (croner `next_after` for cron-fired, slot-time for `@random`, `start_time` for Run Now / api triggers) and passes it through. The write happens inside `insert_running_run` itself; out-of-band UPDATE is rejected per D-03.
- **Where the FCTX panel wire-up lives**: `src/web/handlers/run_detail.rs` — call `get_failure_context(pool, run.job_id).await` gated to `run.status ∈ {failed, timeout}` (FCTX-01). Soft-fail per D-12. Pass the result + the run record + a docker-detection bool + the scheduled_for value into the askama template context.
- **Where the histogram wire-up lives**: `src/web/handlers/job_detail.rs` — fetch last-100 ALL runs (single SELECT) → call `exit_buckets::aggregate(rows)` → pass `HistogramCard` into the askama template context. Soft-fail per D-12.
- **Where `pub mod exit_buckets;` lands**: `src/web/mod.rs` next to `pub mod stats;`.
- **Where the `cd-fctx-*` and `cd-exit-*` CSS classes live**: `assets/src/app.css` `@layer components` block. Single PR for all class additions per UI-SPEC § Component Inventory CSS contract tables.
- **Where the askama template inserts go**: `templates/pages/run_detail.html` (panel between metadata card and `#log` block) and `templates/pages/job_detail.html` (card between Duration card and Run History block) per UI-SPEC § Layout & Surfaces diagrams.

</code_context>

<specifics>
## Specific Ideas

- **`scheduled_for` write must never block the scheduler loop.** It's a value computed before the await on the executor; the existing `insert_running_run` is already sync-after-compute, so adding one column doesn't change the latency budget.
- **Run Now writes `scheduled_for = start_time`** (skew = 0ms). UI-SPEC § Copywriting Contract fire-skew row reads `Scheduled: 14:30:00 • Started: 14:30:00 (+0 ms)` on Run Now triggers — that's the intended copy. Operators see "manually triggered = no skew" by inference. The semantically-different "manual run, no schedule applies" rendering is a v1.3 polish candidate; v1.2 ships the +0ms representation.
- **`@random` jobs use the resolved-slot time** as `scheduled_for`. The slot-resolution time is the operator-facing meaning of "when did the slot say this should fire" — operators inspecting an `@random` job's fire skew see the gap between the slot decision and the actual start, which is the load signal they care about (vs. randomization vs reschedule drift).
- **EXIT-05 top-3 last-seen never goes stale beyond 100 runs** because the aggregator only sees the last-100 raw rows. A code that fired 99 runs ago is at the edge; a code that fired 101 runs ago is invisible. This is the intended window — UI-SPEC copy says "Last {N} runs (window: 100)".
- **The FCTX panel's TIME DELTAS row is the ONLY row that links out** ("[view last successful run]"). That link uses `--cd-text-accent`; per UI-SPEC § Color § Accent reserved-for #1.
- **Histogram bars use `style="height:{pct}%"` inline.** Server-clamped to 0..100. The 4px min-height (per UI-SPEC § Spacing — `var(--cd-space-1)`) keeps zero/near-zero buckets visible.
- **Both surfaces are server-rendered at page load.** No HTMX polling for either. Operators reload the page to see updated state. The histogram is read-only in v1.2 — no interaction beyond hover/focus tooltips.
- **Print mode opens the panel** via `@media print { details { open: open } }` per UI-SPEC § Interaction Contract. Operators printing a run-detail page for incident postmortems get the panel inline without manual expand.
- **The fire-skew artificial-delay technique for `uat-fire-skew`** should be the cleanest one — researcher decides between (a) a sidecar container holding a global lock so the scheduler waits, (b) an `async sleep` injected via a test feature flag in the scheduler loop, (c) a slow-start container that spends 30s in `pull` before fire. Option (c) is closest to operator reality (image-pull delay) and doesn't require new test infra; Option (a) is most deterministic; Option (b) is fastest to write but adds production code surface that only test mode uses.
- **The 21-HUMAN-UAT.md mobile/light/print/keyboard scenarios** can be a single `uat-fctx-a11y` umbrella recipe OR four split recipes. Single umbrella is cleaner for the maintainer's eyes; split is easier to grep when one mode regresses. Planner picks.

</specifics>

<deferred>
## Deferred Ideas

- **Re-running a failed run from the panel** (a "retry" or "re-fire" button): explicit out-of-scope. Read-only panel in v1.2; v1.3 candidate.
- **HTMX live-poll of the FCTX panel** (auto-refresh while a follow-up run is in progress): out-of-scope. Operators reload to refresh. Revisit if usage data shows operators staring at the panel during reschedule windows.
- **HTMX live-poll of the histogram card**: same as above.
- **Dashboard re-render with FCTX summary on the job card** ("3 consecutive failures" badge on the dashboard): out-of-scope. FCTX is run-detail-only in v1.2. Revisit after operators report whether they want a dashboard-level early-warning.
- **Per-run trace ID / correlation ID in the panel**: out-of-scope. Cronduit doesn't surface trace IDs today; adding the data path is its own phase.
- **Webhook payload extension to carry the fire-skew signal**: out-of-scope. Webhook payload schema is locked at v1.2.0 per P18 D-17. Revisit in v1.3.
- **Per-job exit-code Prometheus label**: explicit accepted-out-of-scope per EXIT-06 (cardinality discipline). Operators wanting exit-code metrics scrape the histogram via the dashboard or build their own pipeline.
- **Operator-tunable bucket boundaries**: locked at 10 buckets per EXIT-02. v1.3 candidate if operators report misclassification on real exit-code patterns.
- **Larger histogram window than last-100**: locked at 100 per EXIT-01. v1.3 candidate; would need a new "window selector" UI affordance.
- **Trace-style fire-skew chart** (a sparkline of fire-skew over time): out-of-scope. Single-value row in v1.2.
- **Pruning `scheduled_for` from old rows or backfilling it**: NULL is the intended legacy state. No backfill (D-04). No tightening (D-05).
- **"Last seen for ALL codes" sub-table** (instead of top-3): UI-SPEC EXIT-05 locks top-3. Revisit if operators want a fuller table.
- **Tooltip on the success-rate stat**: out-of-scope. The stat is a simple badge per UI-SPEC.
- **Webhook delivery panel on run-detail** (the inverse of FCTX — outbound delivery context): explicit deferred per P20 deferred-list (v1.3 candidate).
- **THREAT_MODEL.md TM5 (Webhook Outbound) full close-out**: Phase 24 owns this per ROADMAP.
- **`release.yml` / `cliff.toml` / `release-rc.md` modifications** for v1.2-specific rc behavior: reused verbatim per D-24. Any maintainer-discovered runbook gap during the rc.2 cut is a hotfix PR before tagging.

</deferred>

---

*Phase: 21-failure-context-ui-panel-exit-code-histogram-card-rc-2*
*Context gathered: 2026-05-01*
