# Phase 21: Failure-Context UI Panel + Exit-Code Histogram Card — rc.2 - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-01
**Phase:** 21-failure-context-ui-panel-exit-code-histogram-card-rc-2
**Areas discussed:** FCTX-06 data source, Histogram query shape, Render-path error handling, Test + UAT shape

---

## Gray Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| FCTX-06 data source | Schema decision: scheduled_for column does NOT exist today. Add new column? Compute at render time? Defer? | ✓ |
| Histogram query shape | Single SQL with CASE-WHEN bucketing vs two queries vs Rust-side bucketing over last-100 raw rows | ✓ |
| Render-path error handling | Soft-fail vs hard-fail on DB error during render; concurrent-reload handling | ✓ |
| Test + UAT shape | Integration test files, just-recipe naming, autonomous=false UAT scope before rc.2 cut | ✓ |

**User's choice:** All four areas selected.
**Notes:** Phase 21 is unusually well-locked because UI-SPEC.md is approved and Phase 16 already shipped the schema + query helper. Discussion focused on plumbing decisions only.

---

## FCTX-06 Data Source

### Q1: How should we source the 'scheduled fire time' for the FIRE SKEW row (FCTX-06)?

| Option | Description | Selected |
|--------|-------------|----------|
| Add job_runs.scheduled_for column | One-file additive migration (TEXT NULL RFC3339, sqlite + postgres mirror). Scheduler writes it at insert_running_run; old rows stay NULL and the row hides. Matches P16 image_digest pattern. | ✓ |
| Compute at render time from cron expr | Re-evaluate the cron expression backwards from start_time using croner. No schema change. Risk: cron drift, DST edges, @random jobs. | |
| Defer FCTX-06 from this phase | Drop FIRE SKEW row from panel for v1.2; ship the other 4 P1 signals. | |

**User's choice:** Add job_runs.scheduled_for column
**Notes:** Cleanest, most accurate, future-proof. The schema add is minimal (one column), and matches the well-tested P16 pattern.

### Q2: When does the scheduler write scheduled_for, and how do @random jobs and Run-Now manual-trigger runs populate it?

| Option | Description | Selected |
|--------|-------------|----------|
| Always write at insert_running_run; trigger-aware semantics | Scheduler computes scheduled_for at fire decision. Run Now / api triggers write scheduled_for = start_time (skew = 0ms). NULL on legacy backfill rows; hide row when NULL. | ✓ |
| Only write for cron-fired runs; NULL for run-now/api | Manual triggers leave scheduled_for NULL; the row HIDES on those. | |
| Always write; @random jobs use the resolved slot time | Same as option 1 but explicit about @random slot resolution. | |

**User's choice:** Always write at insert_running_run; trigger-aware semantics
**Notes:** Single write point preserves the existing call-site simplicity. Run Now's +0ms skew is an accepted representation choice.

### Q3: How should the FIRE SKEW row render when scheduled_for is NULL (legacy v1.0/v1.1 runs that pre-date this column)?

| Option | Description | Selected |
|--------|-------------|----------|
| Hide the row entirely (NULL == row not rendered) | Cleanest scan path: on legacy runs the panel just has 4 rows (or 3 on non-docker). Mirrors IMAGE DIGEST hide-on-non-docker. | ✓ |
| Show row with muted '— (legacy run)' | Row visible but muted parenthetical. | |
| Backfill scheduled_for = start_time for old rows | Three-file migration shape. Skew always reads as 0ms on legacy runs. | |

**User's choice:** Hide the row entirely (NULL == row not rendered)
**Notes:** Matches the existing IMAGE DIGEST hide-on-non-docker behavior pattern. NULL is the intended legacy state.

### Q4: Migration shape and column type for job_runs.scheduled_for?

| Option | Description | Selected |
|--------|-------------|----------|
| One-file additive, TEXT NULL RFC3339 | Mirrors P16 image_digest + config_hash. Same project-wide convention. NO index. | ✓ |
| One-file additive, TIMESTAMP/INTEGER native | Postgres TIMESTAMPTZ + sqlite INTEGER unix-millis. Breaks the project-wide RFC3339-TEXT convention. | |
| Three-file tightening (NULL → backfill → NOT NULL) | Per P11/P14 pattern. Overkill — we want NULL on legacy rows. | |

**User's choice:** One-file additive, TEXT NULL RFC3339
**Notes:** Project-wide consistency over any local optimization.

---

## Histogram Query Shape

### Q1: How should we shape the exit-code histogram query?

| Option | Description | Selected |
|--------|-------------|----------|
| Rust-side bucketing over last-100 raw rows | Single SELECT, bucket in Rust. Mirrors v1.1 OBS-04 percentile pattern. Structurally identical on sqlite/postgres. | ✓ |
| Single SQL with CASE-WHEN bucketing + GROUP BY | Server bucketed. One round-trip, but CASE arms duplicated across dialects. | |
| Two queries (counts + per-code last-seen rows) | Doubles the index hit. | |

**User's choice:** Rust-side bucketing over last-100 raw rows
**Notes:** Same shape as Phase 13. Matches the project's structural-parity discipline.

### Q2: Where does the bucket-categorization helper live, and what's its function shape?

| Option | Description | Selected |
|--------|-------------|----------|
| src/web/exit_buckets.rs (new module, sibling to stats.rs) | Dedicated module. Clean unit-test surface. | ✓ |
| Inline in src/web/handlers/job_detail.rs | Smaller PR; couples categorization to HTTP layer. | |
| src/web/stats.rs (extend the existing module) | Couples math + categorization. | |

**User's choice:** src/web/exit_buckets.rs (new module, sibling to stats.rs)
**Notes:** Keeps stats.rs as a pure-math module; bucket categorization is its own concern.

### Q3: How should the histogram aggregator handle EXIT-04 'stopped' vs raw exit_code = 137?

| Option | Description | Selected |
|--------|-------------|----------|
| status discriminator wins; exit_code is secondary | status='stopped' → Stopped bucket regardless of exit_code 137. status!='stopped' + exit_code=137 → 128-143 bucket. | ✓ |
| exit_code 137 always = Stopped bucket | Loses the operator-stop / external-signal distinction. | |
| Three-way: stopped status + exit_code 137 + signal range | Most pedantic but adds branches the spec doesn't require. | |

**User's choice:** status discriminator wins; exit_code is secondary
**Notes:** Matches UI-SPEC tooltip copy 'NOT a crash' + EXIT-04 spec literal. Faithful to operator intent semantics.

### Q4: How does the success-rate stat (EXIT-03) compute, and what counts as the denominator?

| Option | Description | Selected |
|--------|-------------|----------|
| success_count / sample_count (excluding stopped) | Mirrors v1.1 OBS-03 dashboard sparkline rule. Operator stop is not a failure. | ✓ |
| success_count / sample_count (full window) | Stopped runs DEPRESS the success rate. Inconsistent with OBS-03. | |
| exit_code = 0 / total exit_code samples | Skips the status field. Introduces coupling. | |

**User's choice:** success_count / (sample_count - stopped_count)
**Notes:** Already-validated convention from v1.1 sparkline.

---

## Render-Path Error Handling

### Q1: If get_failure_context() or the histogram query throws a DB error during render, how should the page degrade?

| Option | Description | Selected |
|--------|-------------|----------|
| Soft-fail: hide the new surface + tracing::warn! | FCTX panel hidden on Err; histogram shows '—' empty state. Rest of page renders normally. | ✓ |
| Hard-fail: 500 page | Loud but breaks the user's primary diagnostic surface. | |
| Mixed: FCTX soft, histogram hard | Inconsistent. | |

**User's choice:** Soft-fail: hide the new surface + tracing::warn!
**Notes:** Operators learn from logs + /metrics, not from a broken page. Consistent with v1.1 sparkline soft-fail.

### Q2: How should we handle the 'never had a successful run' edge case for FCTX rows?

| Option | Description | Selected |
|--------|-------------|----------|
| Render panel; degrade individual rows | TIME DELTAS shows 'No prior successful run', IMAGE DIGEST hides, CONFIG shows '— (no prior success)', DURATION hides, FIRE SKEW renders. | ✓ |
| Hide panel until first success exists | Loses streak signal on day-1 failures. | |
| Render panel; show single TIME DELTAS row only | Compress to one row. | |

**User's choice:** Render panel; degrade individual rows
**Notes:** Panel still gives operators the streak count even on a brand-new failing job.

### Q3: How does the panel handle a concurrent config reload that changes the job between fire-time and now?

| Option | Description | Selected |
|--------|-------------|----------|
| Compare run.config_hash to last_success.config_hash literally | Per-run snapshot comparison. The failing run's snapshot is what fired. | ✓ |
| Compare to current jobs.config_hash | Misleading on jobs reloaded between fire and render. | |
| Show both: 'this run vs last success' + 'this run vs current' | Doubles the row width. | |

**User's choice:** Compare run.config_hash to last_success.config_hash literally
**Notes:** Matches P16 D-05 semantics. No ambiguity.

### Q4: How does the histogram card behave on a brand-new job vs 1-4 runs?

| Option | Description | Selected |
|--------|-------------|----------|
| Below 5: '—' empty state with 'Need 5+ samples; have N' | UI-SPEC literal. Card always present. | ✓ |
| Below 5: hide the entire card | Loses the 'I expect this card to be here' affordance. | |
| Below 5: render histogram with 'low confidence' watermark | Misleading. | |

**User's choice:** Below 5: '—' empty state with 'Need 5+ samples; have N'
**Notes:** Honors UI-SPEC literal copy.

---

## Test + UAT Shape

### Q1: What integration test surface should ship with Phase 21?

| Option | Description | Selected |
|--------|-------------|----------|
| tests/v12_fctx_panel.rs + tests/v12_exit_histogram.rs + extend v12_fctx_explain.rs | Two new files mirroring P16/P17/P20 layout. | ✓ |
| Single tests/v12_p21_ui.rs covering both surfaces | Harder to grep. | |
| Per-feature: panel_rows.rs + bucket_helper.rs + render_gating.rs | Three smaller files. More boilerplate. | |

**User's choice:** tests/v12_fctx_panel.rs + tests/v12_exit_histogram.rs + extend v12_fctx_explain.rs
**Notes:** Mirrors the established v1.2 pattern.

### Q2: What just-recipe UAT surface for Phase 21?

| Option | Description | Selected |
|--------|-------------|----------|
| uat-fctx-panel + uat-exit-histogram + uat-fire-skew | Three recipes mirroring P18/P19/P20 family pattern. | ✓ |
| Single uat-p21-ui covering all three | Harder to debug regressions. | |
| uat-fctx-panel + uat-exit-histogram only (skip uat-fire-skew) | Two recipes total. | |

**User's choice:** uat-fctx-panel + uat-exit-histogram + uat-fire-skew
**Notes:** Honors project memory `feedback_uat_use_just_commands.md`. Recipe-calls-recipe pattern.

### Q3: rc.2 release-engineering: reuse Phase 20's discipline (D-28..D-31) verbatim?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes: identical to rc.1 — maintainer-local tag, no release.yml/cliff.toml/runbook edits | Same mechanics as P20. | ✓ |
| Yes, plus expand release notes content with new sections | Breaks v1.1 P12 D-12 'git-cliff is authoritative'. | |
| Defer rc.2 cut to Phase 22 wrap | Conflicts with ROADMAP Phase 21 success criterion #5. | |

**User's choice:** Yes: identical to rc.1 — maintainer-local tag, no release.yml/cliff.toml/runbook edits
**Notes:** Reuses the working pattern.

### Q4: Phase 21 autonomous mode?

| Option | Description | Selected |
|--------|-------------|----------|
| autonomous=true through verification; maintainer-only for rc.2 tag | Final wave is autonomous=false 21-RC2-PREFLIGHT.md. Mirrors P20 D-29/D-30. | ✓ |
| autonomous=false on UAT recipe waves too | More checkpoints but slower. | |
| autonomous=false on the migration wave | One extra checkpoint before migration. | |

**User's choice:** autonomous=true through verification; maintainer-only for rc.2 tag
**Notes:** Plans run autonomously through code/test/verification; maintainer drives UAT + tag cut.

---

## Claude's Discretion

User accepted Claude's recommendation in every area. Areas left to Claude's discretion (per CONTEXT.md § Claude's Discretion):

- Exact migration filename + timestamp prefix (continuing the existing `2026XXXX_NNNNNNN_*` sequence after `20260502_000008`).
- Internal struct names (`HistogramCard` is the public name; field names planner picks).
- The `aggregate` row-tuple input shape vs a dedicated `RawRunRow` struct.
- Whether `categorize` returns `Option<ExitBucket>` vs an `ExitBucket::Success` variant.
- Exact `tracing::warn!` field shape for soft-fail logs.
- The `uat-fire-skew` artificial-delay technique (sidecar lock vs feature-flag sleep vs slow-start container).
- Whether the 21-HUMAN-UAT.md mobile/light/print/keyboard scenarios are split into individual recipes or rolled into a single `uat-fctx-a11y` umbrella.
- Wave structure (linear vs parallel) — migration must land first; helper module + handler wire-up + templates can run in parallel; tests + UAT recipes wave-end; human-UAT + rc.2 preflight final wave.
- Whether to extend `RunDetailContext` in-place or wrap it in a new sub-struct.

## Deferred Ideas

- Re-running a failed run from the panel ("retry button") — v1.3 candidate.
- HTMX live-poll of the FCTX panel — v1.3 if usage data warrants.
- HTMX live-poll of the histogram card — same.
- Dashboard re-render with FCTX summary on the job card — out-of-scope; v1.3 polish.
- Per-run trace ID / correlation ID surfaces in the panel.
- Webhook payload extension to carry the new fire-skew signal — webhook payload is locked at v1.2.0 per P18 D-17.
- Per-job exit-code Prometheus label — explicit accepted-out-of-scope per EXIT-06 (cardinality discipline).
- Operator-tunable bucket boundaries — locked at 10 buckets per EXIT-02.
- Larger histogram window than last-100 — v1.3.
- Trace-style fire-skew chart (sparkline of fire-skew over time) — out-of-scope.
- "Last seen for ALL codes" sub-table — UI-SPEC EXIT-05 locks top-3.
- Tooltip on the success-rate stat — out-of-scope.
- Webhook delivery panel on run-detail — v1.3 candidate per P20 deferred-list.
- THREAT_MODEL.md TM5 (Webhook Outbound) full close-out — Phase 24.
- `release.yml` / `cliff.toml` / `release-rc.md` modifications — reused verbatim per D-24.
