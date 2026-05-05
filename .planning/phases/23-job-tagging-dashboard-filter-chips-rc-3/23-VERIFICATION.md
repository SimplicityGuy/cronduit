---
phase: 23-job-tagging-dashboard-filter-chips-rc-3
verified: 2026-05-04T18:00:00Z
status: human_needed
score: 10/10 must-haves verified (5 SC-roadmap + 10 phase-level truths)
overrides_applied: 0
human_verification:
  - test: "Execute 23-HUMAN-UAT.md — 6 scenarios"
    expected: "All 6 maintainer scenarios pass: chip render, AND-filter+untagged-hidden, share-URL+stale-drop, mobile reflow, light-mode parity, keyboard+screen-reader"
    why_human: "Visual brand check (terminal-green palette + bold weight + dark/light + mobile reflow), keyboard nav, screen-reader announcement of state changes — assistive-tech replay required (per project memory feedback_uat_user_validates.md)"
    artifact: ".planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-HUMAN-UAT.md"
  - test: "Execute 23-RC3-PREFLIGHT.md — rc.3 tag cut runbook"
    expected: "v1.2.0-rc.3 tag pushed; ghcr.io/SimplicityGuy/cronduit:v1.2.0-rc.3 published on amd64 + arm64; :latest still at v1.1.0; rolling :rc tag = v1.2.0-rc.3 (SC #5)"
    why_human: "Cross-system check (GitHub Releases, GHCR, multi-arch manifest); requires PR-merged-to-main precondition + maintainer-only signed git tag (D-15, project memory feedback_no_direct_main_commits.md); SC #5 cannot be verified by Claude until maintainer cuts tag"
    artifact: ".planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-RC3-PREFLIGHT.md"
warnings:
  - id: CR-01
    severity: BLOCKER
    summary: "SQL LIKE wildcard `_` in tag names enables cross-tag false-positive matches across BOTH SQLite and Postgres backends"
    detail: "src/db/queries.rs:907-914 (sqlite) and :976-981 (postgres) bind format!(r#\"%\\\"{}\\\"%\\\"\", t) without ESCAPE clause; tag charset (src/config/validate.rs:476) admits `_`; LIKE treats `_` as single-char wildcard. Concrete failure: fleet with `back_up` and `back-up` — `?tag=back_up` matches BOTH because `_` wildcards `-`. TAG-05 substring-collision validator (src/config/validate.rs:611) uses str::contains literally and cannot detect this. No regression test in tests/v12_tags_dashboard.rs covers `_`-bearing tags. This breaks the AND-filter contract advertised in README.md:289."
    flagged_by: "23-REVIEW.md (Standard depth, 2026-05-04). Pre-existing from Phase 22 LIKE-pattern landing, but Phase 23 makes it operator-controlled via the URL bar."
    fix_options:
      - "(A) Extend check_tag_substring_collision (validate.rs:611) to treat `_` as equivalence-class with `-` and any other character"
      - "(B) Bind literal-LIKE pattern with ESCAPE '\\\\' clause + escape `_` `%` `\\\\` in bound value (preferred; does not depend on validator extension)"
    impact: "AND-filter false-positives for fleets containing tags differing only by `_` placement. SC #2 is observable-correct in the common case (no `_`-only fleets) but vulnerable to silent over-matching for `_`-bearing tag pairs."
    recommendation: "Address before final v1.2.0 cut. rc.3 ship is acceptable if release notes mention the limitation OR if Phase 24 picks up the fix as a hardening item."
  - id: WR-01
    severity: WARNING
    summary: "Tag intersection in dashboard.rs:443-446 is case-sensitive but URL casing is operator-supplied"
    detail: "params.tags.retain(|t| fleet_tags.contains(t)) is case-sensitive; tags in DB are lowercased by validator but `?tag=Backup` is silently dropped as stale. UX hazard for hand-typed/case-mixed bookmark URLs."
  - id: WR-02
    severity: WARNING
    summary: "tests/v12_tags_dashboard.rs:610-617 sort_header_carries_active_tags uses `>=`, not `==`"
    detail: "Sort-header tag-suffix coverage uses `assert!(sort_header_carries >= 8, ...)`; a future template edit dropping &tag=... from one column would still pass if other markup adds extra occurrences."
  - id: WR-03
    severity: WARNING
    summary: "No regression test on the chip's hx-get URL round-trip after &amp; entity decoding"
    detail: "chip.href HTML-escapes & to &amp; in attribute serialization (correct); browsers + HTMX decode correctly today, but no test asserts this contract."
  - id: WR-04
    severity: INFO
    summary: "build_chip_views appends ?filter= even when filter is empty, padding bookmark URLs"
    detail: "Cosmetic only; bookmark URLs include filter=&sort=... Defer-only fix; rc.4 hardening candidate."
deferred:
  - truth: "Operator pushing the v1.2.0-rc.3 tag sees the GHCR image published on both architectures"
    addressed_in: "23-RC3-PREFLIGHT.md (Plan 23-08, autonomous=false, maintainer-only)"
    evidence: "Per CONTEXT D-15 the rc.3 cut is the maintainer's responsibility post-PR-merge; runbook is authored and gates the SC #5 verification"
---

# Phase 23: Job Tagging Dashboard Filter Chips — rc.3 — Verification Report

**Phase Goal:** Operators get CSS-only filter chips on the dashboard with AND semantics across selected tags, untagged-hidden when filter active, shareable URL state — then cut `v1.2.0-rc.3`.

**Verified:** 2026-05-04T18:00:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

The phase delivered a complete, working dashboard filter chip implementation. All automated test surface (12 integration tests + 2 unit tests) passes locally. Code review (`23-REVIEW.md`) found one BLOCKER and four warnings — the BLOCKER is a pre-existing LIKE-wildcard hole exposed (not introduced) by Phase 23 making the predicate operator-controlled. The remaining work to declare the phase goal achieved is human-only: 23-HUMAN-UAT.md execution + 23-RC3-PREFLIGHT.md tag cut. Both runbooks are authored, complete, and `autonomous: false` per project policy (`feedback_uat_user_validates.md`).

### Observable Truths (ROADMAP Success Criteria)

| #   | Truth (ROADMAP SC)                                                                                                                                                                              | Status                                  | Evidence                                                                                                                                                                                                                                                |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Operator viewing the dashboard sees filter chips for every distinct tag in the current fleet; clicking a chip toggles state (active = teal-bordered + bold; inactive = grey)                     | VERIFIED (code) / NEEDS HUMAN (visual)  | Code: `dashboard.rs:454-460` build_chip_views, `dashboard.html:28-49` chip strip; `app.css:610-618` cd-tag-chip-* family with `.cd-tag-chip--active { font-weight: 700; border-color: var(--cd-text-accent) }` and `.cd-tag-chip--inactive { ... grey }`. Tests `chip_strip_render` + `chip_active_state_class` PASS. Visual brand match needs HUMAN-UAT Scenario 1+5 |
| 2   | AND semantics across multiple chips; composes with v1.0 name-filter via AND                                                                                                                     | VERIFIED                                | `queries.rs:853-860` AND-chained `tags LIKE` predicates per active tag + name-filter `LOWER(j.name) LIKE ?1` AND-joined. Tests `and_filter_two_tags`, `and_with_name_filter` PASS. WARNING: CR-01 LIKE-wildcard `_` hole — see overrides/warnings.       |
| 3   | Untagged jobs HIDDEN when any tag filter is active                                                                                                                                              | VERIFIED                                | `queries.rs:865-869` `untagged_clause = if !active_tags.is_empty() { "AND tags != '[]'" } else { "" }` — gated on active set. Test `untagged_hidden_when_filter_active` PASS; `no_filter_shows_all_jobs` PASS (no-regression on default load).            |
| 4   | Shareable URL `/?tag=backup&tag=weekly`; chips render active on first paint (bookmarkable)                                                                                                      | VERIFIED                                | `dashboard.rs:12,398` axum_extra::Query; `:49` `#[serde(default, rename = "tag")] pub tags: Vec<String>`. Tests `active_tags_parsed_from_repeated_query` (V-05 unit) + `direct_url_renders_chips_active` (V-10 integration) PASS.                          |
| 5   | Operator pushing `v1.2.0-rc.3` tag sees GHCR image on both architectures                                                                                                                        | DEFERRED (autonomous: false)            | `23-RC3-PREFLIGHT.md` authored at 204 lines; mirrors P21 RC2-PREFLIGHT structure verbatim per D-15. Maintainer-only execution post-PR-merge to main. NOT verifiable by Claude until tag cut.                                                            |

### Phase-Level Must-Haves (extracted from CONTEXT.md decision lock)

| #   | Truth                                                                                                  | Status     | Evidence                                                                                                                                                                                                                                                                                                                              |
| --- | ------------------------------------------------------------------------------------------------------ | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| MH-1 | `axum_extra::extract::Query` swap for repeated `?tag=` deserialize (TAG-06 lock; D-10)                  | VERIFIED   | `src/web/handlers/dashboard.rs:12` `use axum_extra::extract::Query;` (with comment block citing TAG-06 + Pitfall 1); `:398` `Query(params): Query<DashboardParams>` extractor wired. V-05 unit test asserts repeated-key deserialize.                                                                                                  |
| MH-2 | AND-chained `tags LIKE` filter SQL with active_tags param (TAG-07; D-09)                                | VERIFIED   | `queries.rs:825-830` signature widened with `active_tags: &[String]` 5th param; `:853-860` per-backend predicate strings; `:907-914` (sqlite) + `:976-981` (postgres) bind sites use `format!(r#"%"{}"%"#, t)` JSON-quote-anchored pattern.                                                                                            |
| MH-3 | `tags != '[]'` clause gated on `!active_tags.is_empty()` (TAG-07 untagged-hidden)                        | VERIFIED   | `queries.rs:865-869` exact match; gated unconditionally — RESEARCH § Pitfall 7 hazard avoided. Test `untagged_hidden_when_filter_active` confirms only active-set non-empty hides untagged.                                                                                                                                            |
| MH-4 | Fleet-tag intersection BEFORE SQL composition (security boundary — stale URL silent-drop)                | VERIFIED   | `dashboard.rs:443-446` `active_tags.retain(\|t\| fleet_tags.contains(t))` AFTER sort+dedup, BEFORE second `get_dashboard_jobs(active_tags=&active_tags)` call at `:466`. Test `stale_tag_silent_drop` PASS. WR-01 case-sensitivity warning noted.                                                                                       |
| MH-5 | `cd-tag-chip-*` CSS family with three-channel active state, focus-visible, touch target (TAG-08; D-04)  | VERIFIED   | `assets/src/app.css:610-618` cd-tag-chip-strip + cd-tag-chip + --active + --inactive + :hover + :focus-visible variants; `:434` reduced-motion override; `:623` print mode hides chip strip. Three channels: color (green vs grey), weight (700 vs 400), border. min-height: 40px = WCAG 2.2 AAA touch target. |
| MH-6 | Chip strip in dashboard.html + HTMX OOB swap on toggle + sort headers + 3s poll preserve tags           | VERIFIED   | `dashboard.html:28-49` chip strip (full-page); `partials/job_table.html:1-39` OOB chip strip block gated on `include_oob_chip_strip`; `dashboard.html:123,135,145,155` 4 sortable columns include `{% for t in active_tags %}&tag={{ t\|urlencode }}{% endfor %}` in BOTH href and hx-get; `:173` 3s poll `hx-include` adds `[name='tag']`. Tests `oob_response_shape`, `sort_header_carries_active_tags`, `poll_hx_include_widened` PASS. |
| MH-7 | `just uat-chips-*` recipes (3 mirroring P22 pattern; D-17)                                              | VERIFIED   | `justfile:1439, 1524, 1605` — `uat-chips-render`, `uat-chips-and-filter`, `uat-chips-share-url` — all three recipes verified via `just -l \| grep uat-chips`. Each recipe references TAG-06/TAG-07 in description + mirrors uat-tags-* recipe-calls-recipe pattern.                                                                       |
| MH-8 | README `### Tag Filter Chips` subsection (D-04 labels-precedent template)                               | VERIFIED   | `README.md:287-315` complete subsection: chip strip behavior, charset, per-job cap, untagged-hidden, bookmarkable URL contract, stale-tag silent-drop, webhook payload + Prometheus exclusion cross-reference. Mirrors P17 D-04 labels-precedent shape.                                                                                |
| MH-9 | `23-HUMAN-UAT.md` with 6 scenarios (3 recipe-driven + 3 eyeball-only; autonomous: false)                | VERIFIED   | 179 lines; frontmatter `autonomous: false`, `requirements: [TAG-06, TAG-07, TAG-08]`. 6 scenarios: render, AND-filter+untagged-hidden, share-URL+stale-drop, mobile reflow, light-mode, keyboard+screen-reader. First three cite `just uat-chips-*` recipes verbatim per D-20.                                                          |
| MH-10 | `23-RC3-PREFLIGHT.md` as verbatim mirror of 21-RC2-PREFLIGHT.md (rc.2→rc.3 + P21→P23 substitutions; D-15) | VERIFIED   | 204 lines; frontmatter `autonomous: false`, `rc_tag: v1.2.0-rc.3`. Section structure mirrors P21 RC2-PREFLIGHT verbatim with substitutions. References `docs/release-rc.md` REUSED VERBATIM (D-16 lock — no edits to release.yml/cliff.toml/release-rc.md).                                                                              |

**Score:** 10/10 phase-level must-haves verified + 4/5 ROADMAP SCs verified (SC #5 deferred to maintainer per autonomous=false runbook, NOT a gap)

### Required Artifacts

| Artifact                                                                | Expected                                                  | Status     | Details                                                                                                                                                                                              |
| ----------------------------------------------------------------------- | --------------------------------------------------------- | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/web/handlers/dashboard.rs`                                         | axum_extra::Query swap; ChipView; build_chip_views; fold; intersect | VERIFIED   | 700+ lines; explicit comments cite TAG-06, RESEARCH Pitfalls 1, 4, 7; build_chip_views URL-encodes via url::form_urlencoded::Serializer.                                                              |
| `src/db/queries.rs`                                                     | DashboardJob.tags field; get_dashboard_jobs(&[String]) signature; AND-chained LIKE | VERIFIED   | `:610` `pub tags: Vec<String>` field; `:825-830` 5-param signature; `:853-1008` per-backend AND-chain + JSON-quote anchored bind values + JSON deserialize at row-mapping site for both arms.       |
| `assets/src/app.css`                                                    | cd-tag-chip-* family in @layer components; reduced-motion + print extensions | VERIFIED   | `:610-618` + `:434` + `:623` complete; ZERO new tokens introduced (literal 40px and 9999px noted with awaiting-token-addition comment).                                                                |
| `templates/pages/dashboard.html`                                        | Chip strip above filter row; sort-header href widening; 3s poll hx-include | VERIFIED   | `:28-49` chip strip block above filter at L51; `:123,135,145,155` 4 sortable columns each include the active-tag suffix in href + hx-get; `:173` `hx-include="[name='filter'],[name='sort'],[name='order'],[name='tag']"` widened.                                |
| `templates/partials/job_table.html`                                     | OOB chip strip block gated on include_oob_chip_strip      | VERIFIED   | `:1-39` exact OOB block — `hx-swap-oob="true"` on outer wrapper only (Pitfall 2 honored); rendered FIRST then table rows (Pitfall 5 ordering); skipped on full-page render.                          |
| `tests/v12_tags_dashboard.rs`                                           | Integration test; 12 #[tokio::test] covering V-01..V-04, V-06, V-08..V-14 | VERIFIED   | 711 lines; 12 tests all PASS. Function names match VALIDATION.md V-NN test command suffixes.                                                                                                          |
| `src/web/handlers/dashboard.rs::tests`                                  | Unit tests for V-05 + V-07                                | VERIFIED   | `:646` `active_tags_parsed_from_repeated_query` (V-05); `:676` `distinct_tag_fold_alphabetical` (V-07); both PASS.                                                                                    |
| `justfile`                                                              | uat-chips-render, uat-chips-and-filter, uat-chips-share-url | VERIFIED   | All three recipes present at `:1439`, `:1524`, `:1605`; descriptions reference TAG-06/TAG-07.                                                                                                          |
| `README.md`                                                             | ### Tag Filter Chips subsection                           | VERIFIED   | `:287-315` complete subsection with TOML config example + behavior bullets + cross-references.                                                                                                         |
| `.planning/phases/23-.../23-HUMAN-UAT.md`                               | autonomous=false; 6 scenarios                              | VERIFIED   | 179 lines; 6 scenarios; autonomous: false.                                                                                                                                                             |
| `.planning/phases/23-.../23-RC3-PREFLIGHT.md`                           | autonomous=false; mirrors P21 RC2-PREFLIGHT               | VERIFIED   | 204 lines; structure mirrors P21 verbatim with substitutions; D-15/D-16 locks honored.                                                                                                                 |

### Key Link Verification

| From                                  | To                                  | Via                                                              | Status     | Details                                                                                                                                                                                                                                          |
| ------------------------------------- | ----------------------------------- | ---------------------------------------------------------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `?tag=...&tag=...` URL                | `DashboardParams.tags: Vec<String>` | axum_extra::extract::Query → serde_html_form                     | WIRED      | `dashboard.rs:12,49,398`. V-05 unit test confirms.                                                                                                                                                                                                |
| `params.tags`                         | `active_tags` (canonicalized)        | sort + dedup + retain(\|t\| fleet_tags.contains(t))               | WIRED      | `dashboard.rs:443-446`. Stale-tag silent-drop test confirms. Case-sensitivity warning (WR-01) but not blocking.                                                                                                                                  |
| `active_tags`                         | SQL bind values                     | `q.bind(format!(r#"%"{}"%"#, t))` per active tag                 | WIRED      | `queries.rs:907-914` (sqlite), `:976-981` (postgres). LIKE-wildcard `_` hazard (CR-01) — see warnings.                                                                                                                                            |
| `unfiltered_jobs.iter().flat_map(\|j\| j.tags)` | `fleet_tags: Vec<String>`         | BTreeSet<String> → Vec<String>                                   | WIRED      | `dashboard.rs:430-435` two-fetch pattern (D-08); first fetch unfiltered, second fetch active-tag filtered.                                                                                                                                       |
| `fleet_tags + active_tags`            | `Vec<ChipView>`                     | build_chip_views                                                 | WIRED      | `dashboard.rs:159-216,454-460`. Each chip carries post-toggle URL (active = remove self; inactive = add self + canonicalize).                                                                                                                     |
| `chips`                               | Chip strip HTML (full-page + OOB)   | `dashboard.html:33-44` + `partials/job_table.html:22-33`         | WIRED      | Both renders consume the same `chips` vector (single source of truth). HTMX path emits OOB-wrapped strip; full-page path emits inline strip with the partial's OOB block gated `false`.                                                          |
| Chip click                            | Dashboard partial response         | `<a hx-get="?{{ chip.href }}" hx-target="#job-table-body" hx-swap="innerHTML" hx-push-url="true">` | WIRED      | OOB block in partial response replaces `#cd-tag-chip-strip` (state) + target swap of `#job-table-body` (rows). Test `oob_response_shape` PASS.                                                                                                   |
| Sort header                           | Chip-aware navigation               | `{% for t in active_tags %}&tag={{ t\|urlencode }}{% endfor %}` in href + hx-get | WIRED      | All 4 sortable columns; 8 occurrences in `dashboard.html`. WR-02 warning: tests use `>=8` rather than per-column assertions.                                                                                                                      |
| 3s poll                               | Tag preservation                    | `hx-include="[name='filter'],[name='sort'],[name='order'],[name='tag']"` | WIRED      | `dashboard.html:173`. Hidden `<input name="tag">` siblings inside chip strip per active tag (`:46-48`). Test `poll_hx_include_widened` PASS.                                                                                                     |

### Data-Flow Trace (Level 4)

| Artifact                              | Data Variable      | Source                                                              | Produces Real Data | Status                                                                              |
| ------------------------------------- | ------------------ | ------------------------------------------------------------------- | ------------------ | ----------------------------------------------------------------------------------- |
| `DashboardPage.fleet_tags`            | fleet_tags         | `BTreeSet<String>` fold over real `unfiltered_jobs.iter().flat_map(\|j\| j.tags)` | YES                | FLOWING — sourced from real `get_dashboard_jobs` query that selects `j.tags AS tags_json` from real `jobs` table. |
| `DashboardPage.active_tags`           | active_tags        | `params.tags` → sort+dedup+intersect with fleet_tags                | YES                | FLOWING — operator-supplied URL params filtered through real fleet intersect.       |
| `DashboardPage.chips`                 | chips              | `build_chip_views(&fleet_tags, &active_tags, ...)`                  | YES                | FLOWING — reuses real fleet + active sets; returns ChipView per real fleet tag.     |
| `DashboardPage.jobs`                  | jobs (filtered)    | `get_dashboard_jobs(filter, sort, order, &active_tags)`             | YES                | FLOWING — real DB query with AND-chained tag predicates.                            |
| `chip.href`                           | url query string   | `url::form_urlencoded::Serializer` real-encodes filter+sort+order+next_set | YES                | FLOWING — real URL-encoded round-trip; test `direct_url_renders_chips_active` confirms. |

### Behavioral Spot-Checks

| Behavior                                                                       | Command                                                                              | Result                                                              | Status |
| ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------ | ------------------------------------------------------------------- | ------ |
| `?tag=backup&tag=weekly` deserializes via axum_extra into Vec<String> length 2 | `cargo test --lib web::handlers::dashboard::tests::active_tags_parsed_from_repeated_query` | 1 passed                                                            | PASS   |
| Distinct-tag fold yields alphabetical Vec<String> from heterogeneous DashboardJob set | `cargo test --lib web::handlers::dashboard::tests::distinct_tag_fold_alphabetical`     | 1 passed                                                            | PASS   |
| 12 integration tests covering V-01..V-04, V-06, V-08..V-14                      | `cargo test --test v12_tags_dashboard`                                                | 12 passed; 0 failed; 0 ignored                                      | PASS   |
| Compile-gate green for both test surfaces                                       | `cargo test --test v12_tags_dashboard --no-run` + `cargo test --lib --no-run`         | Both Finished in 0.55s/23.12s; 0 errors                             | PASS   |
| Three uat-chips-* recipes registered in just                                    | `just -l \| grep uat-chips`                                                            | 3 lines: uat-chips-render, uat-chips-and-filter, uat-chips-share-url | PASS   |

### Requirements Coverage

| Requirement | Source Plan(s)        | Description                                                                                                         | Status      | Evidence                                                                                                                                                |
| ----------- | --------------------- | ------------------------------------------------------------------------------------------------------------------- | ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| TAG-06      | 23-01..23-08          | Dashboard renders filter chips per fleet tag; AND semantics; URL state via repeated `?tag=`                          | SATISFIED   | MH-1, MH-4, MH-6, SC #1, SC #2, SC #4 all VERIFIED. Caveat WR-01 (case-sensitivity).                                                                    |
| TAG-07      | 23-01, 23-02, 23-06   | Untagged jobs hidden when ANY tag filter active; AND with v1.0 name-filter                                          | SATISFIED   | MH-3 VERIFIED. Tests `untagged_hidden_when_filter_active`, `and_with_name_filter` PASS. SC #2, SC #3 VERIFIED.                                          |
| TAG-08      | 23-04, 23-05, 23-07   | CSS-only chip components (no JS); HTMX swaps on toggle (matches v1.0 dashboard polling architecture)                 | SATISFIED   | MH-5, MH-6 VERIFIED. Test `css_only_chip_no_inline_js` PASS confirms no inline JS in chip strip. Three-channel a11y encoding (color + weight + border). |

**Orphan check:** No additional TAG-XX requirements mapped to Phase 23 in REQUIREMENTS.md beyond TAG-06..08 (table at L210-212 confirms). All three requirements have full plan coverage.

### Anti-Patterns Found

| File                                  | Line(s)        | Pattern                              | Severity   | Impact                                                                                                                            |
| ------------------------------------- | -------------- | ------------------------------------ | ---------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `src/db/queries.rs`                   | 907-914, 976-981 | `LIKE` without `ESCAPE` clause; `_` in tag charset wildcards in SQL | BLOCKER (CR-01) | False-positive cross-tag matches for `_`-bearing tag pairs (e.g., `back_up` vs `back-up`). Breaks AND-filter contract for affected fleets. Pre-existing from P22. |
| `src/web/handlers/dashboard.rs`       | 443-446        | Case-sensitive retain on operator-supplied URL params | WARNING (WR-01) | UX hazard for hand-typed/case-mixed bookmark URLs (`?tag=Backup` silently dropped).                                                |
| `tests/v12_tags_dashboard.rs`         | 610-617        | `>=` not `==` on sort-header tag-suffix coverage      | WARNING (WR-02) | Future template edits dropping `&tag=` from one column not caught.                                                                  |
| `src/web/handlers/dashboard.rs`       | 416-424, 466-474 | `unwrap_or_default()` on get_dashboard_jobs without tracing::warn | INFO (IN-04)    | DB transient error silently fall-throughs to empty fleet tags + chips disappear; pre-existing pattern, not a P23 regression.       |
| `src/db/queries.rs`                   | 865-869, 881, 894 | Static-string SQL fragment via `format!` (untagged_clause) | INFO (IN-01)    | Functionally safe (closed-set static literal); reader confusion only.                                                              |

### Human Verification Required

#### 1. Execute 23-HUMAN-UAT.md — 6 scenarios

**Test:** Run through all 6 scenarios in `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-HUMAN-UAT.md`:

1. Chip strip render + alphabetical + empty-state hidden (`just uat-chips-render`)
2. AND-filter + untagged-hidden + name-filter composition (`just uat-chips-and-filter`)
3. Shareable URL round-trip + stale-tag silent-drop (`just uat-chips-share-url`)
4. Mobile viewport reflow (eyeball at <640px width)
5. Light-mode parity (eyeball with `prefers-color-scheme: light`)
6. Keyboard navigation + screen-reader narration of active state changes

**Expected:** All 6 scenarios pass per the runbook acceptance criteria. Maintainer ticks each scenario in the runbook and fills in Final Sign-Off.

**Why human:** Visual brand check (terminal-green palette + bold weight + dark/light + mobile reflow) requires human eyeballs. Keyboard nav + screen-reader announcement of chip-state changes requires assistive-tech replay. Per project memory `feedback_uat_user_validates.md` — Claude does NOT mark UAT passed from its own runs.

#### 2. Execute 23-RC3-PREFLIGHT.md — rc.3 tag cut runbook

**Test:** Run through `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-RC3-PREFLIGHT.md` post-PR-merge to `main`:

1. Phase 23 plans merged on `main`
2. Green CI on merge commit
3. Green compose-smoke
4. `git cliff --unreleased --tag v1.2.0-rc.3` preview clean
5. HUMAN-UAT all 6 scenarios ticked
6. `git tag -a -s v1.2.0-rc.3 -m "v1.2.0-rc.3 — dashboard tag filter chips (P23)"`
7. `git push origin v1.2.0-rc.3`
8. `gh release view v1.2.0-rc.3 --json isPrerelease --jq .isPrerelease` → `true`
9. `docker manifest inspect ghcr.io/SimplicityGuy/cronduit:v1.2.0-rc.3` → amd64 + arm64 digests
10. `:latest` still resolves to `v1.1.0` (P12 D-10 hyphen-gate)
11. Rolling `:rc` resolves to `v1.2.0-rc.3`

**Expected:** SC #5 ("Operator pushing the `v1.2.0-rc.3` tag sees the GHCR image published at `ghcr.io/SimplicityGuy/cronduit:v1.2.0-rc.3` on both architectures") becomes observable.

**Why human:** Cross-system check (GitHub Releases, GHCR, multi-arch manifest) requires post-push verification with maintainer-only signed git tag (D-15). PR-merged-to-main precondition not yet met (current branch: `phase23/discuss`). Per project memory `feedback_no_direct_main_commits.md` — only the maintainer cuts the tag.

### Gaps Summary

**No blocking gaps for goal achievement at the code level.** All 10 phase-level must-haves and 4 of 5 ROADMAP success criteria are VERIFIED through code inspection + automated test execution. SC #5 is intentionally deferred to the maintainer per the autonomous=false runbook.

**One BLOCKER from code review (CR-01) is real and load-bearing.** The LIKE-wildcard `_` hole admits cross-tag false-positive matches for fleets containing tags like `back_up` and `back-up`. The hole was inherited from Phase 22's tag charset + LIKE pattern decision; Phase 23 is the first phase to expose it via operator-controlled URL inputs. The fix is a small change (5-15 lines) in either the validator or the query layer:

- **Option A** (recommended by reviewer): Bind a literal-LIKE pattern with `ESCAPE '\\'` clause + escape `_` `%` `\\` in bound value. Independent of validator.
- **Option B**: Extend `check_tag_substring_collision` to treat `_` as equivalence-class with `-` and any other character. Keeps LIKE pattern as-is.

**Recommendation:** Address CR-01 in either rc.4 (if maintainer chooses to defer rc.3 ship) or as a Phase 24 hardening item. The phase goal is observably achieved for the common-case fleet (no `_` in tag names); the hazard is silent over-matching only for `_`-bearing tag pairs.

**Status routing:** `human_needed` (not `gaps_found`). Human verification required for HUMAN-UAT execution + RC3-PREFLIGHT tag cut. The CR-01 BLOCKER is surfaced as a `warnings:` entry in frontmatter for maintainer decision (not a `gaps:` entry, because the phase goal is otherwise achieved and the issue is a known pre-existing inheritance from Phase 22).

---

_Verified: 2026-05-04T18:00:00Z_
_Verifier: Claude (gsd-verifier)_
