---
phase: 23-job-tagging-dashboard-filter-chips-rc-3
plan: 03
subsystem: web
tags: [handler, axum, axum_extra, query, distinct-fold, btreeset, tagging, wave-2, fleet-intersect, stale-tag-drop]

# Dependency graph
requires:
  - phase: 23-job-tagging-dashboard-filter-chips-rc-3
    plan: 01
    provides: tests/v12_tags_dashboard.rs scaffolds + V-05/V-07 todo!() stubs in dashboard::tests
  - phase: 23-job-tagging-dashboard-filter-chips-rc-3
    plan: 02
    provides: get_dashboard_jobs(active_tags: &[String]) — fifth parameter; AND-chained tags LIKE predicates; DashboardJob.tags: Vec<String>
provides:
  - "axum_extra::extract::Query<DashboardParams> swap (load-bearing repeated-key URL deserialization for ?tag=foo&tag=bar)"
  - "DashboardParams.tags: Vec<String> field with #[serde(default, rename = \"tag\")]"
  - "Two-fetch sequence in dashboard() handler: unfiltered fetch -> BTreeSet<String> fleet_tags fold; filtered fetch -> rendered jobs"
  - "active_tags canonicalization pipeline: sort -> dedup -> retain (intersect with fleet_tags); silent-drop of stale URL tags BEFORE SQL fires (T-23-03-01 mitigation)"
  - "Caller updated: get_dashboard_jobs receives &active_tags (replacing the &[] placeholder from Plan 23-02)"
  - "DashboardPage and JobTablePartial view-models widened with fleet_tags + active_tags fields (#[allow(dead_code)] until Plan 23-05 wires templates)"
  - "V-05 (active_tags_parsed_from_repeated_query) GREEN — repeated ?tag= keys deserialize to 2-element Vec<String>"
  - "V-07 (distinct_tag_fold_alphabetical) GREEN — BTreeSet -> Vec yields sorted-distinct fleet tags"
affects:
  - 23-04 (handler-side V-06 stale-tag silent-drop test — depends on this plan's intersect step)
  - 23-05 (template inserts consume DashboardPage/JobTablePartial.fleet_tags + active_tags + the OOB swap response)
  - 23-06..23-08 (HUMAN UAT + RC3 PREFLIGHT — observe end-to-end behavior the plan wires)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "axum_extra::extract::Query swap — the canonical 2025/26 path for accepting Vec<T> from repeated form-encoded query keys; serde_html_form is the underlying deserializer"
    - "Query::try_from_uri unit test pattern — exercises the same serde_html_form path as from_request_parts without the FromRequestParts<S> state-type dance"
    - "Two-fetch fold-vs-render sequence — handler issues an UNFILTERED read for the aggregation (chip strip / fleet view) and a FILTERED read for the table body; documented trade-off vs single-fetch (chips disappear when only-job is filtered out)"
    - "BTreeSet<String> -> Vec<String> alphabetical-distinct fold — handler-side aggregation pattern mirroring P13 OBS-03 sparkline hydration (D-08 explicit)"
    - "Sort -> dedup -> retain canonicalization pipeline — order matters: sort first (canonicalize), dedup second (collapse `?tag=foo&tag=foo`), intersect third (drop stale). Output is alphabetical-canonical per UI-SPEC § URL canonicalization"
    - "View-model widening with #[allow(dead_code)] forward-link — fields land before the template plan that consumes them; askama 0.15 tolerates unused struct fields, but the lint allow keeps clippy `-D warnings` green during the wave-window"

key-files:
  created: []
  modified:
    - src/web/handlers/dashboard.rs

key-decisions:
  - "V-05 test shape uses Query::try_from_uri (axum-extra public API), not serde_html_form::from_str directly — serde_html_form is a transitive dep, not a direct dev-dep, so a direct import would fail to resolve at the test-crate root. try_from_uri exercises the same serde_html_form path as from_request_parts and is the cleanest path with no extra dep churn."
  - "Two-fetch sequence (unfiltered + filtered) chosen over single-fetch — fleet_tags MUST reflect the unfiltered fleet so chips render every tag, not only tags surviving the active AND-filter. RESEARCH § Open Question 1 / PATTERNS L296-318 documents the trade-off; cost is sub-millisecond at homelab scale (<200 jobs typical)."
  - "Sort + dedup + intersect ORDER preserved per CONTEXT D-07 + UI-SPEC § URL canonicalization — sort first to canonicalize, dedup second to collapse repeat-keys, retain third to drop stale. Reordering would either skip canonicalization (canonicalize-after-intersect would fail to canonicalize stale-only inputs to empty) or cost performance (retain-first would scan over un-deduped repeats)."
  - "View-model fields gain #[allow(dead_code)] per-field rather than a struct-level #[allow] — keeps the lint scope minimal and lets Plan 23-05 remove the per-field attribute as it wires each template reference. askama 0.15 compile-tolerates unused struct fields."
  - "Both view-models (DashboardPage AND JobTablePartial) carry fleet_tags + active_tags — D-11 OOB swap calls for the partial response to render the chip strip alongside the table body, so the partial template needs both pieces."

patterns-established:
  - "Wave-2 handler-aggregation pattern: handler issues TWO database reads when an aggregation must reflect the unfiltered domain alongside a filtered render. Sub-millisecond at homelab scale; the alternative (compute aggregate from filtered set) breaks UX when the filter narrows past the aggregate's natural width."
  - "axum_extra::Query unit test via try_from_uri — pattern future plans can copy when they add Vec<T>-bearing query params and need to assert deserialization without standing up a router."
  - "Sort -> dedup -> retain pipeline as the canonicalization-and-security idiom for operator-controlled URL list params. The retain step is the security boundary; the sort+dedup is the canonicalization."

requirements-completed: [TAG-06]

# Metrics
duration: ~10min
completed: 2026-05-05
---

# Phase 23 Plan 03: Wave-2 Handler-Side URL Extraction + Active-Tag Intersection Summary

**`axum_extra::extract::Query` swap unlocks repeated-key URL parsing; handler issues a two-fetch sequence (unfiltered fold + filtered render), folds `Vec<DashboardJob>` into an alphabetical-distinct `fleet_tags` set, intersects `params.tags` against the fleet to silent-drop stale URL tags BEFORE SQL fires, passes `&active_tags` into `get_dashboard_jobs`, and widens both view-models for Wave-2 templates. V-05 + V-07 GREEN, V-03 still GREEN.**

## Performance

- **Duration:** ~9.5 min (566s)
- **Started:** 2026-05-05T02:09:24Z
- **Completed:** 2026-05-05T02:18:50Z
- **Tasks:** 2 (both `type="auto"` `tdd="true"`)
- **Files modified:** 1
- **Commits:** 2

## Accomplishments

- **Import swap landed (the load-bearing technical pivot for TAG-06).** `dashboard.rs:5` rewritten from `use axum::extract::{Query, State};` to `use axum::extract::State;` + `use axum_extra::extract::Query;` (with a citation comment pointing at RESEARCH § Pitfall 1 — the EXACT failure mode TAG-06 forbids). The extractor binding site (`Query(params): Query<DashboardParams>` at the original line 242) remained textually unchanged; only the import resolves to the axum_extra version now. The pre-existing path-qualified `axum_extra::extract::CookieJar` use is untouched.
- **`DashboardParams.tags: Vec<String>` field added** with `#[serde(default, rename = "tag")]`. URL form is `?tag=backup&tag=weekly` (singular key); Rust field name is plural per Rust idiom. The doc-comment is a load-bearing security note that names the silent-drop intersection requirement on the consumer side.
- **Two-fetch sequence in `dashboard()` handler** (RESEARCH § Pattern 3 / D-08). The first call (`unfiltered_jobs`) uses `(None, ..., &[])` — no name-filter, no tag-filter — and is consumed only by the fleet-tag fold. The second call (`jobs`) is the filtered render set, drives `to_view`, sparkline hydration, and view-model construction. Variable shadowing is intentional: `jobs` keeps its existing semantics in the rest of the handler.
- **Fleet-tag fold** via `BTreeSet<String> -> Vec<String>` chain at the handler level. Pattern mirrors RESEARCH § Pattern 3 verbatim; `BTreeSet` provides alphabetical sort + dedup at insert time, the `into_iter().collect()` lands a sorted distinct `Vec<String>`. Empty-tag jobs contribute nothing (nothing is yielded by `flat_map`).
- **Active-tag canonicalization pipeline:** `params.tags.clone()` -> `.sort()` -> `.dedup()` -> `.retain(|t| fleet_tags.contains(t))`. The retain step is the security boundary (T-23-03-01 mitigation per the plan's threat register) — operator-supplied URL tags not in the fleet are silently dropped BEFORE the SQL second-fetch fires. Sort + dedup canonicalize per UI-SPEC § URL canonicalization (so `/?tag=zebra&tag=alpha` and `/?tag=alpha&tag=zebra` produce the same shareable URL).
- **`get_dashboard_jobs` second call passes `&active_tags`** — replacing the `&[]` placeholder Plan 23-02 left at the production handler call site. This is the wire-up the SQL widening from 23-02 was waiting on.
- **`DashboardPage` widened** at `dashboard.rs:64-87` with `#[allow(dead_code)] fleet_tags: Vec<String>` + `#[allow(dead_code)] active_tags: Vec<String>`. Doc-comments forward-reference Plan 23-05 (template wiring).
- **`JobTablePartial` widened** with the same two fields (D-11 OOB swap requires the partial response to carry both: chip strip OOB-swap into `#cd-tag-chip-strip` + the table body target swap).
- **Both view-model construction sites updated** in the `is_htmx` if/else branches — the partial branch passes the fields by move (no clone needed because the alternative branch is mutually exclusive in `if/else`). Same on the page branch.
- **V-05 (`active_tags_parsed_from_repeated_query`) GREEN** — `Query::try_from_uri` against `?tag=backup&tag=weekly` yields `params.tags == vec!["backup", "weekly"]` (length 2). The test exercises the same `serde_html_form` path the extractor takes in a real request.
- **V-07 (`distinct_tag_fold_alphabetical`) GREEN** — hand-built `[["weekly","backup"], ["backup","prod"], []]` folds via the same BTreeSet pattern to `["backup", "prod", "weekly"]`. Asserts the alphabetical-distinct invariant (duplicates collapse, empty-tag jobs contribute nothing, order is alphabetical).
- **V-03 regression (`no_filter_shows_all_jobs`)** STILL GREEN — default load (no `?tag=` in URL) produces an empty `params.tags`, the canonicalization pipeline produces an empty `active_tags`, and the SQL emits no `AND tags LIKE` predicates and no `tags != '[]'` clause (gated on `!active_tags.is_empty()`). Pre-P23 dashboard semantics preserved on the default-load path.
- **Compile + clippy + regression gates GREEN.** `cargo build --quiet`, `cargo test --lib --no-run`, `cargo test --test v12_tags_dashboard --no-run`, `cargo test --test dashboard_render` (2/2 passing), and `cargo clippy --all-targets --all-features -- -D warnings` (the project's CI gate) all exit 0.

## Task Commits

Each task was committed atomically:

1. **Task 1: Swap `axum::Query` for `axum_extra::Query` + add `DashboardParams.tags` field + fill V-05** — `d8b9994` (feat)
2. **Task 2: Two-fetch fleet-tag fold + active-tag intersect + view-model widening + V-07** — `4553db7` (feat)

## Files Created/Modified

- `src/web/handlers/dashboard.rs` (MODIFIED only)
  - Imports: `axum::extract::{Query, State}` -> `axum::extract::State` + `axum_extra::extract::Query` (with a TAG-06 / RESEARCH Pitfall 1 citation block); `std::collections::HashMap` widened to `std::collections::{BTreeSet, HashMap}`.
  - `DashboardParams` (now lines 23-50): `tags: Vec<String>` field added with `#[serde(default, rename = "tag")]` and a load-bearing security doc-comment.
  - `DashboardPage` (now lines 64-87): `fleet_tags` + `active_tags` `Vec<String>` fields appended with `#[allow(dead_code)]` (forward-link to Plan 23-05).
  - `JobTablePartial` (now lines 89-103): same two fields under the same `#[allow(dead_code)]` posture; D-11 OOB swap rationale in the doc-comment.
  - `dashboard()` body (now lines 282-435): two `get_dashboard_jobs` call sites (unfiltered fetch for fold + filtered fetch for render); inline `BTreeSet<String>` fleet-tag fold; sort/dedup/retain canonicalization pipeline. Variable shadowing of `jobs` is intentional (the unfiltered binding exists only for the fold and goes out of scope; the filtered binding keeps the existing role for sparkline hydration + view-model construction).
  - `is_htmx` if/else branch arms: each constructor receives `fleet_tags` + `active_tags` (passed by move — `if/else` arms are mutually exclusive).
  - V-05 test body: `Query::try_from_uri` exercise against `?tag=backup&tag=weekly`; asserts `params.tags == vec!["backup", "weekly"]`.
  - V-07 test body: hand-built `[mk_job, mk_job, mk_job]` array; runs the same `BTreeSet -> Vec` fold; asserts the alphabetical-distinct invariant.

## Validation

| V-row | Test fn | Status | Where exercised |
|---|---|---|---|
| V-05 | `active_tags_parsed_from_repeated_query` | GREEN | `cargo test --lib web::handlers::dashboard::tests::active_tags_parsed_from_repeated_query` (1 passed) |
| V-07 | `distinct_tag_fold_alphabetical` | GREEN | `cargo test --lib web::handlers::dashboard::tests::distinct_tag_fold_alphabetical` (1 passed) |
| V-03 (regression) | `no_filter_shows_all_jobs` | GREEN | `cargo test --test v12_tags_dashboard no_filter_shows_all_jobs` (1 passed) — pre-P23 default-load semantics preserved |
| V-01..V-04 (Wave-1 sweep) | per Plan 23-02 | GREEN | All four still 1/1 passing — no Wave-1 regression |

```text
cargo test --lib web::handlers::dashboard::tests::active_tags_parsed_from_repeated_query  →  1 passed
cargo test --lib web::handlers::dashboard::tests::distinct_tag_fold_alphabetical          →  1 passed
cargo test --test v12_tags_dashboard no_filter_shows_all_jobs                              →  1 passed
cargo test --test v12_tags_dashboard and_filter_two_tags                                   →  1 passed
cargo test --test v12_tags_dashboard untagged_hidden_when_filter_active                    →  1 passed
cargo test --test v12_tags_dashboard and_with_name_filter                                  →  1 passed
cargo test --test dashboard_render                                                         →  2 passed
```

The remaining 8 v12_tags_dashboard tests (`stale_tag_silent_drop`, `chip_strip_render`, `chip_active_state_class`, `direct_url_renders_chips_active`, `css_only_chip_no_inline_js`, `oob_response_shape`, `sort_header_carries_active_tags`, `poll_hx_include_widened`) remain at `todo!()` per Wave-0 scaffold — they are owned by Plans 23-04 and 23-05 (template + integration wiring), explicitly out of scope for 23-03.

## Compile + lint gates

- `cargo build --quiet` exits 0
- `cargo test --lib --no-run` exits 0
- `cargo test --test v12_tags_dashboard --no-run` exits 0
- `cargo test --test dashboard_render` 2/2 GREEN
- `cargo clippy --all-targets --all-features -- -D warnings` exits 0 (CI gate)

## Security gate (T-23-03-01..T-23-03-05)

- **T-23-03-01 (Tampering — stale URL tag reaches SQL):** `active_tags.retain(|t| fleet_tags.contains(t))` is the explicit boundary. Stale URL tags drop BEFORE the second `get_dashboard_jobs` call fires. Acceptance grep `grep -q 'active_tags.retain' src/web/handlers/dashboard.rs` returns 1.
- **T-23-03-02 (DoS — unbounded `?tag=` flood):** Accepted in the threat register; effective bound is `min(len(params.tags), len(fleet_tags))` after intersect, and `fleet_tags` is bounded by the P22 16-tags-per-job cap × fleet size. No code action required.
- **T-23-03-03 (Repudiation — extractor silently drops repeats):** V-05 is the regression assertion. Test passes; `axum::Query` would fail it.
- **T-23-03-04 (Information disclosure — error echoes unknown tag):** Silent drop. No error path constructed; chip strip and table render normally with the surviving tags.
- **T-23-03-05 (Logic violation — fleet_tags reflects filtered subset):** Mitigated by the two-fetch sequence. Acceptance `grep -c 'queries::get_dashboard_jobs(' src/web/handlers/dashboard.rs` returns 2.

No HIGH severity threats. No new threat surface introduced beyond the plan's register.

## Decisions Made

- **V-05 test uses `Query::try_from_uri` (axum-extra public API), not `serde_html_form::from_str` directly.** First-pass implementation tried the simpler `serde_html_form::from_str("tag=backup&tag=weekly")` form per the plan's "Alternative simpler test shape" suggestion, but the test crate failed to compile: `serde_html_form` is a transitive dep of `axum-extra`, not a direct dev-dep, so the import path didn't resolve. `Query::try_from_uri` is a public API on `axum_extra::extract::Query` (verified at `axum-extra-0.12.6/src/extract/query.rs:121-126`) that calls `serde_html_form::from_str` internally — same code path as `from_request_parts`, no extra dep needed. This is the cleaner choice and avoids the axum trait-state dance the plan flagged as "finicky".
- **V-05 test approach decided WITHOUT adding `serde_html_form` as a direct dev-dependency.** The alternative was to add `serde_html_form = "0.2"` to `[dev-dependencies]` in `Cargo.toml`. Rejected — it would (a) introduce a direct dep that floats with axum-extra's transitive choice (version drift risk) and (b) violate the plan's `must_haves.truths.D-23` invariant ("`axum_extra` is already in tree (no new direct deps added by this plan; `serde_html_form` is a transitive dep of `axum_extra`)"). The `try_from_uri` path satisfies the same load-bearing assertion without changing Cargo.toml.
- **`#[allow(dead_code)]` applied per-field rather than struct-level.** Forward-link to Plan 23-05 — when the templates land and reference `fleet_tags`/`active_tags`, Plan 23-05 can remove each per-field allow without touching the whole struct.
- **View-model fields passed by MOVE to whichever `if/else` arm runs (no clone).** The plan suggested `fleet_tags.clone()` + `active_tags.clone()` at both arms, citing the alternative-arm requirement. But `if/else` arms are mutually exclusive; only one runs per request. Passing by move is correct and saves two `Vec<String>` clones per request. (Cost is sub-microsecond per clone, but the cleaner shape is preferred.)
- **`use std::collections::BTreeSet` consolidated into the existing HashMap import line** (`std::collections::{BTreeSet, HashMap}`) rather than added as a separate line. Smaller diff; both come from the same crate path.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] V-05 test failed to compile with the plan's "Alternative simpler" test shape**

- **Found during:** Task 1 verification (`cargo test --lib web::handlers::dashboard::tests::active_tags_parsed_from_repeated_query`)
- **Issue:** The plan's `<action>` block at line 250-265 offered an alternative test shape using `serde_html_form::from_str("tag=backup&tag=weekly")` directly. Implemented that path first; `cargo test` failed with `error[E0433]: failed to resolve: use of unresolved module or unlinked crate `serde_html_form``. Root cause: `serde_html_form` is a transitive dep of `axum-extra` (visible in `Cargo.lock` line 3265), but it is NOT a direct dev-dep of cronduit, so the test crate's symbol resolution doesn't see it. The plan's expectation ("`serde_html_form` is a transitive dep of `axum-extra` so it's available in the test crate") was incorrect — transitive deps reachable via crate-internal use paths do not become importable at the consumer crate root.
- **Fix:** Switched to `axum_extra::extract::Query::try_from_uri(&uri)` — a public API on the `Query` extractor that calls `serde_html_form::from_str` internally (verified in axum-extra-0.12.6/src/extract/query.rs:121-126). This exercises the SAME code path as `from_request_parts` against an `axum::http::Uri`, asserts the same load-bearing property (Vec<String> receives both occurrences of repeated `?tag=` keys), and adds no new dependency to Cargo.toml. The plan's "Critical caveat" block flagged this risk: "If the `from_request_parts` shape is finicky, fall back to ..." — implementation chose `try_from_uri` as the cleanest path among the offered shapes.
- **Files modified:** `src/web/handlers/dashboard.rs` (V-05 test body only; pre-commit; landed in the same Task 1 commit `d8b9994`).
- **Verification:** `cargo test --lib web::handlers::dashboard::tests::active_tags_parsed_from_repeated_query` → 1 passed; `cargo build --quiet` exits 0.
- **Committed in:** `d8b9994` (Task 1 commit).

**2. [Rule 1 — Bug / lint] `clippy::useless_vec` warning on the V-07 test fixture**

- **Found during:** Task 2 final verification (`cargo clippy --all-targets --all-features -- -D warnings` — the project's CI gate per CLAUDE.md "Quality bar: Clippy + fmt gate on CI").
- **Issue:** `let jobs = vec![mk_job(...), mk_job(...), mk_job(...)]` triggered `clippy::useless_vec` because the binding is iterated by reference only (no `Vec`-specific operation needed). The plan's V-07 action block at lines 480-498 used `vec![...]` literally, but the project's `cargo clippy --all-targets --all-features -- -D warnings` gate would fail on this in CI.
- **Fix:** Changed `vec![...]` to `[...]` (an owned array). Iteration over `&[T]` is identical to iteration over `&Vec<T>` for the `flat_map` chain. Test still passes; clippy clean.
- **Files modified:** `src/web/handlers/dashboard.rs` (V-07 fixture; pre-commit; landed in the same Task 2 commit `4553db7`).
- **Verification:** `cargo clippy --all-targets --all-features -- -D warnings` exits 0; V-07 test still 1 passed.
- **Committed in:** `4553db7` (Task 2 commit).

**3. [Rule 2 — Missing critical / forward-link hygiene] `#[allow(dead_code)]` on the new view-model fields**

- **Found during:** Task 2 (during view-model widening; predicted clippy warnings on unread struct fields).
- **Issue:** The plan widens `DashboardPage` and `JobTablePartial` with `fleet_tags` and `active_tags` BEFORE Plan 23-05 wires the templates. With `cargo clippy ... -- -D warnings` as the CI gate, the unread fields would generate `dead_code` warnings (the templates don't reference them yet, and Rust's lint sees the fields as never-read). The plan's `<action>` block at line 422 acknowledged this: "askama may compile fine ... OR may emit 'unused field' warnings (clippy `dead_code`); add `#[allow(dead_code)]` per-field if needed and Plan 23-05 will remove them." Implementation took the per-field allow path proactively rather than waiting for the warning to surface.
- **Fix:** Applied `#[allow(dead_code)]` per-field on `DashboardPage.fleet_tags`, `DashboardPage.active_tags`, `JobTablePartial.fleet_tags`, `JobTablePartial.active_tags`. Doc-comments forward-link Plan 23-05. Plan 23-05 will remove each `#[allow(dead_code)]` as it wires the corresponding template reference.
- **Files modified:** `src/web/handlers/dashboard.rs` (template structs; landed in Task 2 commit).
- **Verification:** `cargo clippy --all-targets --all-features -- -D warnings` exits 0; downstream Plan 23-05 contract preserved.
- **Committed in:** `4553db7` (Task 2 commit).

---

**Total deviations:** 3 auto-fixed (1 blocking compile-fail on the plan's alt test shape, 1 clippy bug-fix, 1 missing-critical lint hygiene for the wave-window).
**Impact on plan:** All deviations preserve the substantive contract — V-05 + V-07 GREEN with the same load-bearing assertions, view-models carry both new fields, no new direct deps added, clippy `-D warnings` gate green. No scope creep. The `try_from_uri` swap on V-05 is a strictly cleaner test shape than the plan's "alternative simpler" form (no Cargo.toml change, exercises the actual extractor's code path).

## Issues Encountered

None — all three deviations were caught and fixed within their owning tasks before commit.

## TDD Gate Compliance

Plan frontmatter is `type: execute` and individual tasks are marked `tdd="true"`. The Wave-0 plan (23-01) provided the failing-test surface (V-05 + V-07 in `todo!()` state). This plan filled in the implementation, flipping V-05 + V-07 from `todo!()` panic to GREEN. Both task commits are `feat(...)` per project convention because the new code is feature implementation; Plan 23-01 owned the `test(...)` commits. Cross-plan picture: RED (Plan 23-01 — `todo!()` stubs) → GREEN (Plan 23-03 — implementation lands).

## User Setup Required

None.

## Next Phase Readiness

- **Plan 23-04 (handler-side V-06 stale-tag silent-drop integration test) is unblocked.** This plan landed the silent-drop intersect step (`active_tags.retain`) the V-06 test needs to assert against — V-06 will exercise `?tag=backup&tag=ghost` against a fleet that has only `backup`, expecting `ghost` to disappear and the `backup`-tagged job(s) to render normally.
- **Plan 23-05 (template inserts + CSS chip primitive + OOB swap) is unblocked.** `DashboardPage` and `JobTablePartial` carry `fleet_tags` and `active_tags`; Plan 23-05 wires the templates against these fields and removes each `#[allow(dead_code)]` as it lands.
- **Wave-2 gate (this plan + 23-02) per the plan's `<verification>` block:** V-01..V-05 + V-07 GREEN, V-W2 stubs (V-06, V-08..V-14) still on `todo!()` — owned by 23-04..23-05 per Plan 23-01's cross-reference table.
- **No blockers introduced.** No new direct deps; no Cargo.toml changes; no schema changes.

## Self-Check: PASSED

- `src/web/handlers/dashboard.rs` exists on disk and contains:
  - `use axum_extra::extract::Query;` (1 hit)
  - 0 hits of the old combined import `use axum::extract::{Query, State}`
  - `rename = "tag"` (1 hit)
  - `pub tags: Vec<String>` (1 hit — `DashboardParams.tags`)
  - `BTreeSet<String>` (4 hits — `use std::collections::{BTreeSet, HashMap};`, `BTreeSet -> Vec` fold, V-07 test imports + fold)
  - `flat_map` (3 hits — handler fold + V-07 test fold + a pre-existing P13 sparkline use)
  - `active_tags.retain` (1 hit — the security boundary)
  - `active_tags.sort` (1 hit), `active_tags.dedup` (1 hit)
  - `fleet_tags: Vec<String>` (4 hits — 2 struct-field declarations + 2 V-07 doc-comment + handler local mentions)
  - `active_tags: Vec<String>` (3 hits — 2 struct-field declarations + 1 handler `let mut`)
  - 2 calls to `queries::get_dashboard_jobs(` (unfiltered + filtered)
  - 1 hit of `&active_tags` (the second/render call site)
  - 0 occurrences of `todo!` inside the V-05 / V-07 fn bodies (verified via `grep -A2 'fn active_tags_parsed_from_repeated_query' ... | grep -c 'todo!'` → 0)
- `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-03-SUMMARY.md` will exist on disk after this commit.
- Commit `d8b9994` (Task 1) found in `git log --oneline --all`.
- Commit `4553db7` (Task 2) found in `git log --oneline --all`.

---

*Phase: 23-job-tagging-dashboard-filter-chips-rc-3*
*Completed: 2026-05-05*
