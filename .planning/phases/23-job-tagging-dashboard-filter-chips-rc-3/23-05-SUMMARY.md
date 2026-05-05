---
phase: 23-job-tagging-dashboard-filter-chips-rc-3
plan: 05
subsystem: ui
tags: [askama, template, htmx, oob-swap, chip-strip, tagging, url-state, sort-header, hx-include]

# Dependency graph
requires:
  - phase: 23-job-tagging-dashboard-filter-chips-rc-3
    plan: 02
    provides: "AND-chained tags LIKE filter SQL + DashboardJob.tags field — chip clicks AND-filter against the rendered fleet"
  - phase: 23-job-tagging-dashboard-filter-chips-rc-3
    plan: 03
    provides: "axum_extra::extract::Query<DashboardParams> + handler-side fleet-tag fold + active-set intersect — view-models carry fleet_tags + active_tags ready for chip iteration"
  - phase: 23-job-tagging-dashboard-filter-chips-rc-3
    plan: 04
    provides: "cd-tag-chip-strip + cd-tag-chip + cd-tag-chip--active + cd-tag-chip--inactive CSS family — the chip strip ships visually correct from the moment the markup lands"
  - phase: 22-job-tagging-schema-validators
    provides: "TAG-04 charset regex + TAG-05 substring-collision validators — operator-supplied tag values can never break URL or HTML in chip rendering (T-23-05-01/02 mitigation: charset upstream + auto-escape downstream)"
provides:
  - "ChipView precomputation pattern (RESEARCH § Pattern 5 Option A) — Rust-side `Vec<ChipView>` carries `tag`, `is_active`, `href`, `aria_label`; single source of truth for chip `href` + `hx-get`"
  - "url::form_urlencoded::Serializer for query-string composition (no new direct dep — `url` v2.5.8 already in tree)"
  - "Chip strip block in templates/pages/dashboard.html — wrapper id=cd-tag-chip-strip, role=group, aria-label, hidden when fleet_tags.is_empty() (D-02)"
  - "OOB chip strip prefix in templates/partials/job_table.html — gated on include_oob_chip_strip flag (RESEARCH § Pattern 4); hx-swap-oob=\"true\" on WRAPPER ONLY (RESEARCH § Pitfall 2 lock)"
  - "Sort-header href + hx-get widening for all 4 sortable columns (Name / Next Fire / Status / Last Run) — append `&tag={{ t|urlencode }}` for every active tag (D-13 / RESEARCH § Pitfall 3 lock)"
  - "Hidden inputs `<input name=\"tag\">` per active tag inside the chip strip + 3s poll's hx-include widened with `[name='tag']` (D-12)"
  - "Dual-struct field placement of include_oob_chip_strip: bool driven by askama 0.15 include-scope rule — DashboardPage carries the field set false (chip strip in natural body position); JobTablePartial carries it set true (OOB prefix renders in HTMX response)"
  - "Eight V-NN integration tests GREEN (V-06 stale_tag_silent_drop, V-08 chip_strip_render, V-09 chip_active_state_class, V-10 direct_url_renders_chips_active, V-11 css_only_chip_no_inline_js, V-12 oob_response_shape, V-13 sort_header_carries_active_tags, V-14 poll_hx_include_widened)"
affects:
  - 23-06 (UAT recipes — visible chip strip end-to-end)
  - 23-07 (HUMAN UAT — maintainer-validated chip strip + AND filter + share URL)
  - 23-08 (RC3 PREFLIGHT — chip strip is the operator-visible payoff before the rc.3 tag cut)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Per-chip ChipView precomputation — handler builds `Vec<ChipView>` once per request before view-model construction; both arms of `if is_htmx` receive the same data; OOB partial response renders the SAME chip data the full-page response would (DOM/URL state guaranteed identical)"
    - "url::form_urlencoded::Serializer for query-string composition — emits application/x-www-form-urlencoded which is exactly what axum_extra::extract::Query (serde_html_form) round-trips on the way in; symmetric encode/decode without any new direct dep"
    - "askama urlencode filter for inline template composition — the four sort-header anchors call `{{ t|urlencode }}` for each active tag; askama 0.15.6 ships urlencode as a built-in (RESEARCH § Standard Stack)"
    - "OOB swap response composition — gated partial template prefix renders the OOB chip strip wrapper FIRST, then the table body SECOND, both in a single response. hx-swap-oob lives on the OUTER wrapper ONLY (RESEARCH § Pitfall 2). New pattern in this codebase; future phases reusing OOB get a precedent."
    - "Dual-struct field placement driven by askama 0.15 include-scope rule — included templates have full access to the parent context, AND ahead-of-time compilation REQUIRES every variable referenced in the included template to be reachable from the parent struct. Conclusion: any field used inside `{% include %}`-d partials must exist on every parent struct that includes the partial."
    - "Three-channel a11y signal encoding for chips (UI-SPEC § Accessibility Contract; CSS family from Plan 23-04) — class + aria-pressed + aria-label suffix; color is never the sole signal"

key-files:
  created: []
  modified:
    - "src/web/handlers/dashboard.rs — `pub struct ChipView` + `fn build_chip_views` helper added; both view-models widened with `chips: Vec<ChipView>` + `include_oob_chip_strip: bool`; HTMX path sets the flag true, full-page sets false"
    - "templates/pages/dashboard.html — chip strip block inserted above the existing filter row (D-01); 4 sort-header anchors widened with active-tag suffix in BOTH href and hx-get (8 attribute widenings total per D-13); 3s poll hx-include widened to `[name='tag']` (D-12)"
    - "templates/partials/job_table.html — OOB chip strip prefix prepended (gated on `include_oob_chip_strip`; hx-swap-oob=\"true\" on WRAPPER ONLY per RESEARCH § Pitfall 2)"
    - "tests/v12_tags_dashboard.rs — V-06, V-08..V-14 test bodies filled in (8 GREEN integration tests covering chip render, active state, direct-URL paste, CSS-only/no-inline-JS, OOB response shape, sort-header tag carry, poll hx-include widening, stale-tag silent-drop)"

key-decisions:
  - "URL composition via url::form_urlencoded::Serializer (NOT a new urlencoding/percent_encoding direct dep). Plan 23-03's deviation log documented that transitive deps don't resolve at the consumer crate root for direct imports — same lesson applies here. The `url` crate (v2.5.8) is already a direct dep used by src/config/validate.rs (SSRF guard) and src/db/mod.rs (DB URL parsing); its public re-export `url::form_urlencoded` is the cleanest path. Symmetric with axum_extra::Query (which uses serde_html_form internally) — what we encode here is exactly what the URL extractor decodes on the way back."
  - "askama urlencode filter (NOT a Rust-side per-sort-column ChipView extension) for the four sort-header anchors. The plan's `<action>` block flagged this risk and offered a fall-back; verification confirmed askama 0.15.6 ships urlencode + urlencode_strict as built-in filters at `askama-0.15.6/src/filters/urlencode.rs`. Single-line per-anchor template idiom is more readable than emitting four extra ChipView-like structs."
  - "Dual-struct field placement of include_oob_chip_strip — DashboardPage sets `false`; JobTablePartial sets `true`. The plan's <read_first> block in Task 1 cited the askama 0.15 include-scope rule as the load-bearing reason: `templates/pages/dashboard.html` does `{% include \"partials/job_table.html\" %}` inside `<tbody id=\"job-table-body\">`; if `DashboardPage` lacked the field, askama's ahead-of-time compilation would fail when it analyzed the partial in the dashboard.html context. Setting it `false` on `DashboardPage` skips the OOB block at render time but compiles cleanly. Plan 23-05 owns this as the single source of struct definitions for Phase 23."
  - "OOB chip strip prefix lives in templates/partials/job_table.html (Option A from PATTERNS L451-465), NOT a sibling partial. The partial is already consumed in two contexts (full-page include + HTMX response body); the gated `{% if include_oob_chip_strip %}` block adds a third behavior cleanly. Sibling-partial alternative would require two `Template` derives or an axum response composer that concatenates two render outputs — both more complex than the gated single template."
  - "Per-field `#[allow(dead_code)]` removed as each task wired its consumer (Task 2 removed allows on `DashboardPage.{fleet_tags, active_tags, chips}`; Task 3 removed allows on `JobTablePartial.*` and `DashboardPage.include_oob_chip_strip`). Mirrors the wave-window discipline established in Plan 23-03. Final state: zero `#[allow(dead_code)]` on Phase 23 view-model fields."
  - "Variable shadowing of `params.filter`/`sort`/`order`: build_chip_views takes &str borrows, so the borrows end before the view-model construction moves the params into DashboardPage. No clones needed; borrow checker happy."

patterns-established:
  - "ChipView precomputation pattern (Wave-4 chip-strip chip URL state): handler computes the post-toggle URL once per chip via url::form_urlencoded::Serializer, packages tag + is_active + href + aria_label into a ChipView, and passes Vec<ChipView> to the template. Single source of truth for href + hx-get (DRY). Future filter-pill UI (e.g., status chips, exit-code chips) can reuse this exact shape."
  - "OOB swap response composition with gated partial template prefix: when a partial is the HTMX response body for an action that needs to update both an OOB element AND a target swap region, prepend the OOB wrapper (with `hx-swap-oob=\"true\"` on the OUTER wrapper ONLY) gated on a per-context boolean flag. Order matters: OOB element first, target swap region second. This is the first OOB use in the codebase; future phases reusing OOB inherit the pattern + the regression locks (V-12 asserts hx-swap-oob count == 1 and chip_pos < alpha_pos)."
  - "Dual-struct field placement for askama 0.15 include-scope: every variable referenced in an `{% include %}`-d partial MUST exist on every parent struct that includes the partial. Future plans extending shared partials get a checklist: identify all consumer structs and add the field to each; gate the partial's render-time behavior on the field rather than splitting the template."
  - "askama urlencode filter for inline URL composition in templates: when a template needs to round-trip URL state (e.g., active-tag suffix on sort-header anchors), use the built-in `|urlencode` filter rather than emitting per-fragment Rust-side helpers. Defense-in-depth: the upstream charset regex (TAG-04) already prevents structural escape; urlencode is the second lock."

requirements-completed: [TAG-06, TAG-08]

# Metrics
duration: ~18min
completed: 2026-05-05
---

# Phase 23 Plan 05: Wave-4 Template Chip Strip + OOB Swap + Sort-Header Widening + Poll hx-include Widening Summary

**Operators see clickable tag chips end-to-end on the dashboard. ChipView precomputation (RESEARCH § Pattern 5 Option A) drives one source of truth for chip `href` + `hx-get` via `url::form_urlencoded::Serializer`. Chip strip + OOB swap response composition + sort-header tag-suffix widening + 3s poll hx-include widening all land in three atomic commits. Eight V-NN integration tests GREEN (V-06, V-08..V-14); zero new external crates; `cargo build` + `cargo clippy --all-targets --all-features -- -D warnings` + full `tests/v12_tags_dashboard` (12/12) + `tests/dashboard_render` (2/2) + `cargo test --lib` (325/325) all GREEN.**

## Performance

- **Duration:** ~18 min (1090s)
- **Started:** 2026-05-05T02:31:07Z
- **Completed:** 2026-05-05T02:49:17Z
- **Tasks:** 3 (all `type="auto"` `tdd="true"`)
- **Files modified:** 4 (`src/web/handlers/dashboard.rs`, `templates/pages/dashboard.html`, `templates/partials/job_table.html`, `tests/v12_tags_dashboard.rs`)
- **Commits:** 3 task commits (this SUMMARY adds the metadata commit)

## Accomplishments

- **`ChipView` struct + `build_chip_views()` helper landed** in `src/web/handlers/dashboard.rs`. Each chip carries `tag`, `is_active`, `href` (post-toggle URL query string, URL-encoded), and `aria_label`. The handler precomputes `Vec<ChipView>` ONCE per request before view-model construction; both arms of the `if is_htmx` branch receive the same chip data — guaranteeing the OOB partial response renders the same canonical chip state the full-page response would.
- **`url::form_urlencoded::Serializer` is the chip-href encoder** (no new external crate added — `url` v2.5.8 is already a direct dep used by `src/config/validate.rs` SSRF guard and `src/db/mod.rs` DB URL parsing). It emits `application/x-www-form-urlencoded` which is exactly what `axum_extra::extract::Query` (backed by `serde_html_form`) round-trips on the way back: symmetric encode/decode.
- **Both view-models carry `include_oob_chip_strip: bool`** (askama 0.15 include-scope rule lock). `DashboardPage` sets it `false` (chip strip rendered in natural body position by `dashboard.html`); `JobTablePartial` sets it `true` (OOB prefix rendered by `partials/job_table.html` for the HTMX path). This single source of struct definitions for Phase 23 was the load-bearing decision flagged in Task 1's `<read_first>` block.
- **Chip strip block inserted in `templates/pages/dashboard.html`** above the existing filter row (D-01). Wrapper carries `id="cd-tag-chip-strip"`, `class="cd-tag-chip-strip"`, `role="group"`, `aria-label="Filter jobs by tag"`, and the HTML5 `hidden` attribute when `fleet_tags.is_empty()` (D-02 — mirrors `cd-bulk-action-bar`). The chip iteration is `{% for chip in chips %}` against `Vec<ChipView>` from Task 1 — DRY between `href` (anchor's `?{{ chip.href }}`) and `hx-get` (HTMX target).
- **All four sortable column anchors widened** (D-13 / RESEARCH § Pitfall 3) — Name, Next Fire, Status, Last Run. Each anchor's `href` AND `hx-get` got the `{% for t in active_tags %}&tag={{ t|urlencode }}{% endfor %}` suffix appended. Eight total attribute widenings (4 columns × 2 attributes); askama's built-in `|urlencode` filter handles defense-in-depth URL encoding (TAG-04 charset regex prevents structural escape upstream).
- **3s table-body poll's `hx-include` widened** from `[name='filter'],[name='sort'],[name='order']` to `[name='filter'],[name='sort'],[name='order'],[name='tag']` (D-12). Hidden `<input type="hidden" name="tag" value="X">` rendered for each active tag inside the chip strip — polling now preserves the active filter set without a JS hand-off.
- **OOB chip strip prefix added to `templates/partials/job_table.html`** (Option A from PATTERNS — gated single-template approach). The OOB block is gated on `{% if include_oob_chip_strip %}`; `hx-swap-oob="true"` lives on the OUTER wrapper ONLY (RESEARCH § Pitfall 2 — putting it on each chip `<a>` silently fails because chips lack unique IDs); OOB element appears BEFORE the table body in the response (RESEARCH § Pitfall 5 — HTMX 2.0 requires OOB elements at the top so the swap engine processes them before the target swap). V-12 regression-locks both invariants.
- **All 8 V-NN tests for Plan 23-05's scope GREEN end-to-end:** V-06 `stale_tag_silent_drop` (handler-side intersect + template that doesn't render unknown chips = silent drop end-to-end), V-08 `chip_strip_render` (alphabetical chip render + untagged-job row still visible on default load), V-09 `chip_active_state_class` (active class + aria-pressed correctness), V-10 `direct_url_renders_chips_active` (bookmarkable URL state — `?tag=backup&tag=weekly` paints both chips active on first load), V-11 `css_only_chip_no_inline_js` (no `onclick`, no `<script>` in chip strip block — TAG-08 lock), V-12 `oob_response_shape` (HTMX response carries OOB wrapper + table body + correct order + count == 1), V-13 `sort_header_carries_active_tags` (Name sort header round-trips active tag through href + hx-get; ≥ 8 occurrences total across all four columns), V-14 `poll_hx_include_widened` (hidden inputs render + tbody hx-include extends — not replaces — the existing selector list).
- **Compile + clippy + regression gates GREEN.** `cargo build --quiet` exits 0; `cargo clippy --all-targets --all-features -- -D warnings` (the project CI gate per `CLAUDE.md`) exits 0; `cargo test --test v12_tags_dashboard` 12/12 GREEN; `cargo test --test dashboard_render` 2/2 GREEN; `cargo test --lib` 325/325 GREEN.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add ChipView precompute + include_oob_chip_strip flag in handler; widen view-models** — `9732c6b` (feat)
2. **Task 2: Insert chip strip into templates/pages/dashboard.html + widen sort-header href + widen poll hx-include** — `604edae` (feat)
3. **Task 3: Add OOB chip strip prefix to templates/partials/job_table.html (gated on include_oob_chip_strip flag); fill V-06 + V-12** — `9a7ef18` (feat)

## Files Created/Modified

- `src/web/handlers/dashboard.rs` (MODIFIED only)
  - New `pub struct ChipView` + `fn build_chip_views(...)` near the top of the file (between view-model structs and the existing `to_view` helper).
  - `DashboardPage` widened with `chips: Vec<ChipView>` + `include_oob_chip_strip: bool` (latter `#[allow(dead_code)]` until Task 3 wires the partial that consumes it via the `{% include %}`'d block).
  - `JobTablePartial` widened with `chips: Vec<ChipView>` + `include_oob_chip_strip: bool`.
  - `dashboard()` handler builds chips ONCE per request (after the active-tag canonicalization pipeline, before the second `get_dashboard_jobs` call); both `if/else` arms construct view-models with the same `chips` data.
  - `is_htmx` arm: `include_oob_chip_strip: true`. Else arm: `include_oob_chip_strip: false`.
- `templates/pages/dashboard.html` (MODIFIED only)
  - Chip strip block inserted above the existing filter row (line ~19; D-01).
  - 4 sort-header anchors widened with `{% for t in active_tags %}&tag={{ t|urlencode }}{% endfor %}` in BOTH `href` AND `hx-get` (D-13).
  - 3s poll `hx-include` widened to add `[name='tag']` (D-12).
- `templates/partials/job_table.html` (MODIFIED only)
  - OOB chip strip block prepended at the top of the file, gated on `{% if include_oob_chip_strip %}`. Chip wrapper carries `hx-swap-oob="true"` (RESEARCH § Pattern 4); chip anchors do NOT carry OOB (RESEARCH § Pitfall 2 lock); hidden `<input name="tag">` siblings are inside the OOB wrapper too.
- `tests/v12_tags_dashboard.rs` (MODIFIED only)
  - V-06, V-08, V-09, V-10, V-11, V-12, V-13, V-14 test bodies filled in (replacing the Wave-0 `todo!()` stubs from Plan 23-01). 8 GREEN integration tests.

## Validation

| V-row | Test fn | Status | Where exercised |
|-------|---------|--------|-----------------|
| V-06 | `stale_tag_silent_drop` | GREEN | `cargo test --test v12_tags_dashboard stale_tag_silent_drop` |
| V-08 | `chip_strip_render` | GREEN | `cargo test --test v12_tags_dashboard chip_strip_render` |
| V-09 | `chip_active_state_class` | GREEN | `cargo test --test v12_tags_dashboard chip_active_state_class` |
| V-10 | `direct_url_renders_chips_active` | GREEN | `cargo test --test v12_tags_dashboard direct_url_renders_chips_active` |
| V-11 | `css_only_chip_no_inline_js` | GREEN | `cargo test --test v12_tags_dashboard css_only_chip_no_inline_js` |
| V-12 | `oob_response_shape` | GREEN | `cargo test --test v12_tags_dashboard oob_response_shape` |
| V-13 | `sort_header_carries_active_tags` | GREEN | `cargo test --test v12_tags_dashboard sort_header_carries_active_tags` |
| V-14 | `poll_hx_include_widened` | GREEN | `cargo test --test v12_tags_dashboard poll_hx_include_widened` |
| V-01..V-04 | Plan 23-02 SQL tests | GREEN (regression) | `cargo test --test v12_tags_dashboard {and_filter_two_tags,untagged_hidden_when_filter_active,no_filter_shows_all_jobs,and_with_name_filter}` |
| V-05, V-07 | Plan 23-03 handler unit tests | GREEN (regression) | `cargo test --lib web::handlers::dashboard::tests` |
| Dashboard render | tests/dashboard_render.rs | 2/2 GREEN (regression) | `cargo test --test dashboard_render` |
| Lib tests | All `#[cfg(test)] mod tests` | 325/325 GREEN (regression — 1 pre-existing ignored) | `cargo test --lib` |

```text
cargo test --test v12_tags_dashboard            →  12 passed (V-01..V-14 minus V-05/V-07 which are lib-side)
cargo test --test dashboard_render              →  2 passed
cargo test --lib                                →  325 passed (1 pre-existing ignored)
cargo build --quiet                             →  exit 0
cargo clippy --all-targets --all-features -- -D warnings  →  exit 0 (CI gate per CLAUDE.md)
```

## Compile + lint gates

- `cargo build --quiet` exits 0
- `cargo clippy --all-targets --all-features -- -D warnings` exits 0 (CI gate per `CLAUDE.md` Quality Bar)
- `cargo test --lib --no-run --quiet` exits 0
- `cargo test --test v12_tags_dashboard --no-run --quiet` exits 0
- `cargo fmt --check` not explicitly run; the source edits use the existing file's tab/space conventions verbatim.

## Security gate (T-23-05-01..T-23-05-07)

- **T-23-05-01 (Tampering — XSS via tag content in HTML):** Mitigated. Chip text is `{{ chip.tag }}` (askama 0.15 default auto-escape; never `{{ chip.tag|safe }}`). Defense-in-depth: P22 TAG-04 charset regex `^[a-z0-9][a-z0-9_-]{0,30}$` rejects `<`, `>`, `&`, `'`, `"` at config-load. V-11 `css_only_chip_no_inline_js` regression-locks the no-inline-JS posture (no `onclick=`, no `<script>` near chip strip).
- **T-23-05-02 (Tampering — XSS via tag content in URL):** Mitigated. Chip `href` URL composition uses `url::form_urlencoded::Serializer` (Rust-side); sort-header anchor widening uses askama's `|urlencode` filter (template-side). Charset regex makes structural escape impossible upstream; URL encoding is defense-in-depth.
- **T-23-05-03 (Tampering — OOB on each chip vs wrapper):** Mitigated. RESEARCH § Pitfall 2 explicit failure mode. V-12 `oob_response_shape` regression-locks `body.matches("hx-swap-oob").count() == 1` — exactly one OOB attribute, on the wrapper only.
- **T-23-05-04 (Logic violation — sort-header drops active tags):** Mitigated. RESEARCH § Pitfall 3 explicit failure mode. V-13 `sort_header_carries_active_tags` regression-locks: ≥ 8 occurrences of `&tag=backup` in body (4 columns × 2 attrs each) PLUS the Name sort anchor block specifically must include `&tag=backup` in its href.
- **T-23-05-05 (Logic violation — poll drops active tags):** Mitigated. Hidden `<input name="tag">` inputs render inside the chip strip; tbody `hx-include` widened to extend (not replace) the existing selector list. V-14 `poll_hx_include_widened` regression-locks both: hidden input present + tbody `hx-include` contains all four `[name='X']` selectors.
- **T-23-05-06 (Information disclosure):** Accepted. Tag values echoed in `aria-label` are operator-controlled metadata; if a tag is configured with sensitive content, that's an operator decision (documented in CONTEXT). No code action required.
- **T-23-05-07 (Logic violation — duplicate chip strip on full-page render):** Mitigated. The `{% if include_oob_chip_strip %}` gate in `partials/job_table.html` + handler always sets the flag `false` on the non-HTMX path = the OOB block is skipped on full-page renders. Full-page chip strip renders ONLY in its natural body position via `dashboard.html`. Verified via V-08 (chip strip renders exactly once on default load) + V-12 (OOB renders only when HX-Request header is set).

No HIGH severity threats. The XSS surfaces (T-23-05-01, T-23-05-02) are dual-locked (askama auto-escape + P22 charset regex). The HTMX-specific failure modes (T-23-05-03, T-23-05-05, T-23-05-07) have explicit V-NN regression tests. The sort-header bookmarkability promise (T-23-05-04) is locked to V-13.

## Decisions Made

- **URL encoding via `url::form_urlencoded::Serializer` (NOT a new direct dep on `urlencoding` or `percent_encoding`).** Plan 23-03's deviation log documented that transitive deps don't resolve at the consumer crate root for direct imports; same lesson applies. The `url` crate (v2.5.8) is already a direct dep used by `src/config/validate.rs` (SSRF guard) and `src/db/mod.rs` (DB URL parsing). Its public re-export `url::form_urlencoded` is the cleanest path. Symmetric with `axum_extra::Query` (which uses `serde_html_form` internally on the way IN) — what we encode here is exactly what the URL extractor decodes on the way back. Zero new external crates added (per `D-23` invariant).
- **askama `|urlencode` filter for the four sort-header anchors (NOT a Rust-side per-sort-column ChipView extension).** Verification: askama 0.15.6 ships `urlencode` and `urlencode_strict` as built-in filters at `askama-0.15.6/src/filters/urlencode.rs`. Single-line per-anchor template idiom is more readable than emitting four extra ChipView-like structs Rust-side. Plan's `<action>` block flagged the fall-back; verification confirmed the simple path works.
- **OOB block lives in `templates/partials/job_table.html` (Option A from PATTERNS L451-465), gated on `include_oob_chip_strip`.** The partial is already consumed in two contexts (full-page include + HTMX response body); the gated `{% if %}` block adds a third behavior cleanly. Sibling-partial alternative (e.g., `templates/partials/oob_chip_strip.html`) would require either two `Template` derives on `JobTablePartial` or an axum response composer that concatenates two render outputs — both more complex than the gated single template.
- **Per-field `#[allow(dead_code)]` removed as each task wired its consumer.** Task 1 added the allows on every new field; Task 2 removed allows on `DashboardPage.{fleet_tags, active_tags, chips}` (consumed by chip strip block); Task 3 removed allows on `JobTablePartial.{fleet_tags, active_tags, chips, include_oob_chip_strip}` and `DashboardPage.include_oob_chip_strip` (latter is consumed via the askama include-scope rule by the partial template). Final state: zero `#[allow(dead_code)]` on Phase 23 view-model fields. Mirrors Plan 23-03's wave-window discipline.
- **Variable shadowing of `params.filter`/`sort`/`order` is fine for the borrow checker.** `build_chip_views()` takes `&str` borrows; the borrows end after the call returns. The view-model construction then moves `params.filter` etc. into `DashboardPage` — no clones needed. Sub-microsecond cost would be tolerable either way at homelab scale, but the cleaner shape is preferred.
- **V-13 `sort_header_carries_active_tags` initially failed because the test's "Name" string finder hit a non-sort-header occurrence first.** Switched to the unique substring `sort=name&order=` (which appears only in the Name sort anchor's href + hx-get) to scope the search window. Test then GREEN. Documented in Deviations § 1.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] V-13 test's `body.find("Name")` matched a non-sort-header occurrence**

- **Found during:** Task 2 verification (`cargo test --test v12_tags_dashboard sort_header_carries_active_tags`)
- **Issue:** Initial test body used `let name_anchor_idx = body.find("Name").expect("Name header text");` to scope the assertion window for the Name sort anchor. `body.find` returns the FIRST occurrence — and the first "Name" in the response was somewhere upstream of the sort header (likely in a CSS class name or label, given the failure output showed text from the bulk-action-bar's "Clear" button). The window scanned the wrong region of the body and the assertion `name_block.contains("&tag=backup")` failed.
- **Fix:** Switched the scoping to the unique substring `sort=name&order=` (which appears ONLY in the Name sort anchor's `href` and `hx-get` attributes after the widening). Scan window: `[name_sort_idx .. min(name_sort_idx + 300, body.len())]`. The assertion now correctly checks only the Name sort header's href content.
- **Files modified:** `tests/v12_tags_dashboard.rs` (V-13 test body only; pre-commit; landed in the same Task 2 commit `604edae`).
- **Verification:** `cargo test --test v12_tags_dashboard sort_header_carries_active_tags` exits 0 after the fix. Other 7 V-NN tests in Task 2's scope (V-08..V-12, V-14) were unaffected.
- **Committed in:** `604edae` (Task 2 commit).

**2. [Rule 3 — Blocking / lint] doc-comment formatted as a list-item-without-indentation**

- **Found during:** Task 1 verification (`cargo clippy --all-targets --all-features -- -D warnings` — the project CI gate per `CLAUDE.md` Quality Bar).
- **Issue:** The `build_chip_views` doc-comment used an em-dash (`—`) at the start of a continuation line (`/// Each chip's `href` is the post-toggle URL —`), which clippy's `doc_lazy_continuation` lint flags as a list-item without indentation. Clippy `-D warnings` would fail CI on this.
- **Fix:** Reformatted the doc-comment with a paragraph break before the continuation, splitting the em-dash continuation into a new sentence after a blank `///`-line.
- **Files modified:** `src/web/handlers/dashboard.rs` (doc-comment only; pre-commit; landed in Task 1 commit).
- **Verification:** `cargo clippy --all-targets --all-features -- -D warnings` exits 0 after the fix.
- **Committed in:** `9732c6b` (Task 1 commit).

**3. [Rule 2 — Missing critical / lint hygiene] Per-field `#[allow(dead_code)]` on Task 1's new fields**

- **Found during:** Task 1 final verification (`cargo clippy --all-targets --all-features -- -D warnings`).
- **Issue:** Task 1 widens both view-models with new fields (`chips`, `include_oob_chip_strip`) BEFORE Task 2 wires the templates that consume them. With clippy `-D warnings` as the CI gate, the unread fields would generate `dead_code` errors. Plan 23-03's deviation log established the per-field `#[allow(dead_code)]` pattern for exactly this wave-window situation.
- **Fix:** Applied per-field `#[allow(dead_code)]` to all 8 new fields (4 per struct: `fleet_tags` + `active_tags` were already wired in 23-03; `chips` + `include_oob_chip_strip` are new in 23-05 Task 1). Task 2 removed the allows on `DashboardPage.{fleet_tags, active_tags, chips}` as the chip strip block landed (templates consumed them). Task 3 removed the allows on `JobTablePartial.*` and `DashboardPage.include_oob_chip_strip` as the OOB partial block landed.
- **Files modified:** `src/web/handlers/dashboard.rs` (Task 1 added the allows; Tasks 2 and 3 progressively removed them).
- **Verification:** `cargo clippy --all-targets --all-features -- -D warnings` exits 0 after each task. Final state: ZERO `#[allow(dead_code)]` on Phase 23 view-model fields.
- **Committed in:** `9732c6b` (added) → `604edae` (partial removal) → `9a7ef18` (final removal).

---

**Total deviations:** 3 auto-fixed (1 bug-fix on a test scoping issue, 1 blocking clippy doc lint, 1 wave-window lint hygiene with progressive removal across tasks).
**Impact on plan:** All deviations are localized fixes that preserve the plan's substantive contract. V-13 still asserts the same load-bearing property (sort-header anchor href/hx-get carry the active-tag suffix) — just with a more reliable substring marker. The doc-lint fix is purely cosmetic. The dead-code progression matches Plan 23-03's wave-window pattern exactly. No scope creep, no new external crates, no architectural shifts.

## Issues Encountered

- **Pre-existing `tests/schema_parity.rs::sqlite_and_postgres_schemas_match_structurally` requires Docker daemon** (already documented in `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/deferred-items.md` from Plan 23-02 execution). The local development environment (`darwin`) does not have a running Docker daemon at `/var/run/docker.sock`, so the test panics during setup. CI runs Docker, so this test passes there. NOT caused by Plan 23-05 changes — verified by stashing the working tree and re-running the test (still fails). Out of scope per Rule (only fix issues directly caused by current task's changes).

## Threat Flags

None — no new security-relevant surface introduced beyond the threats explicitly addressed in the plan's STRIDE register (T-23-05-01..T-23-05-07).

## TDD Gate Compliance

Plan frontmatter is `type: execute` and individual tasks are marked `tdd="true"`. The Wave-0 plan (23-01) provided the failing-test surface (V-06, V-08..V-14 in `todo!()` state — RED gate). This plan filled in the implementation, flipping all 8 V-NN tests from `todo!()` panic to GREEN (GREEN gate). Both task commits are `feat(...)` per project convention because the new code is feature implementation; Plan 23-01 owned the `test(...)` commits for the test scaffolds.

Cross-plan picture for the V-NN family touched by this plan:
- **RED:** Plan 23-01 (`todo!()` stubs) for V-06, V-08..V-14
- **GREEN:** Plan 23-05 Task 2 (V-08..V-11, V-13, V-14) + Task 3 (V-06, V-12)

No REFACTOR commits this plan — the implementation landed clean on the first GREEN, and clippy + dashboard_render regression sweeps are all green.

## User Setup Required

None. No external service configuration required.

## Next Phase Readiness

- **Plan 23-06 (UAT recipes — `uat-chips-render`, `uat-chips-and-filter`, `uat-chips-share-url`)** UNBLOCKED. The chip strip renders end-to-end; the AND-filter SQL fires via the same handler path; the OOB swap pattern works for HTMX clicks. The three UAT recipes will exercise the visible chip strip on a live cronduit instance per the P22 `uat-tags-*` recipe-calls-recipe pattern.
- **Plan 23-07 (`23-HUMAN-UAT.md` autonomous=false maintainer plan)** UNBLOCKED. Maintainer-validated UAT scenarios cover mobile viewport (chip strip wraps via `flex-wrap` from Plan 23-04 CSS), light-mode rendering, keyboard navigation (Tab onto chips; Enter/Space toggles), screen-reader narration of active state (triple-channel encoding from Plan 23-04 CSS + `aria-pressed` + `aria-label` from Plan 23-05 markup), and end-to-end with the v1.0 name-filter combined with active chips.
- **Plan 23-08 (`23-RC3-PREFLIGHT.md` final wave)** the chip strip is the operator-visible payoff for v1.2.0-rc.3; with Plan 23-05 landed, the rc.3 cut maintainer plan can include the chip strip in the release-body summary.
- **No blockers introduced.** Zero new external crates; zero Cargo.toml changes; zero schema changes; zero `release.yml` / `cliff.toml` / `docs/release-rc.md` changes per D-15 / D-16.
- **Wave 4 gate per plan's `<verification>` block:** V-06, V-08..V-14 GREEN end-to-end. V-07 still GREEN (Plan 23-03 regression preserved). `cargo test --lib`, `cargo test --test schema_parity` (the 2 logic tests; the Docker one is environmental and pre-existing), `cargo test --test dashboard_render`, and `cargo test --test v12_tags_dashboard` all exit 0.

## RESEARCH § Pitfall regression locks

| Pitfall | What it forbids | This plan's regression test |
|---------|-----------------|------------------------------|
| Pitfall 1 | `axum::Query` silently drops repeated `?tag=` keys | V-05 (Plan 23-03 — preserved GREEN; this plan does not regress it) |
| Pitfall 2 | `hx-swap-oob="true"` on each chip `<a>` (chips lack unique IDs) | V-12 `body.matches("hx-swap-oob").count() == 1` |
| Pitfall 3 | Sort-click silently de-filters because anchor `href` omits `&tag=...` | V-13 ≥ 8 occurrences of `&tag=backup` in body (4 columns × 2 attrs) + Name-block specific check |
| Pitfall 4 | Stale URL tag reaches SQL (e.g., bookmarked `?tag=foo` after `tags=["foo"]` removed from a job) | V-06 `stale_tag_silent_drop` (no `ghost` chip rendered + `alpha` row still present) |
| Pitfall 5 | OOB element appears AFTER target swap region in HTMX response | V-12 `chip_pos < alpha_pos` |

## Cross-reference to RESEARCH § Patterns

- **Pattern 4 (OOB swap response composition):** wrapper div carries `hx-swap-oob="true"` on `#cd-tag-chip-strip`, individual chips DO NOT carry OOB. Response body order: chip strip first, table body second. Both invariants regression-locked by V-12.
- **Pattern 5 Option A (precomputed `Vec<ChipView>` Rust-side):** the recommended shape — handler computes chips once, template iterates. DRY between `href` + `hx-get`. New pattern in this codebase; future filter-pill UI inherits the shape.

---

## Self-Check: PASSED

- `.planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-05-SUMMARY.md` exists on disk after this commit.
- Commit `9732c6b` (Task 1) found in `git log --oneline -5`.
- Commit `604edae` (Task 2) found in `git log --oneline -5`.
- Commit `9a7ef18` (Task 3) found in `git log --oneline -5`.
- `src/web/handlers/dashboard.rs` contains:
  - `pub struct ChipView` (1 hit)
  - `fn build_chip_views` (1 hit)
  - `form_urlencoded::Serializer` (1 hit)
  - `chips: Vec<ChipView>` (2 hits — DashboardPage + JobTablePartial)
  - `include_oob_chip_strip: bool` (2 field declarations — DashboardPage + JobTablePartial)
  - `include_oob_chip_strip: true` (1 hit — HTMX path)
  - `include_oob_chip_strip: false` (1 hit — full-page path)
  - 0 `#[allow(dead_code)]` on Phase 23 view-model fields
- `templates/pages/dashboard.html` contains:
  - `id="cd-tag-chip-strip"` (1 hit)
  - `role="group"` (1 hit on chip strip wrapper)
  - `<input type="hidden" name="tag"` (1 hit)
  - `for t in active_tags` (8 hits — 4 sort columns × 2 attrs)
  - `[name='tag']` (1 hit — poll hx-include)
- `templates/partials/job_table.html` contains:
  - `{% if include_oob_chip_strip %}` (1 hit)
  - `hx-swap-oob` (1 hit — on outer wrapper only)
  - `id="cd-tag-chip-strip"` (1 hit — only inside the OOB block)
- `tests/v12_tags_dashboard.rs` test bodies for V-06, V-08, V-09, V-10, V-11, V-12, V-13, V-14 contain ZERO `todo!()` calls (the `todo!()` mentions in the file are in comments + the `_wave0_compile_anchor` doc, not inside test fn bodies).
- `cargo test --test v12_tags_dashboard` 12/12 GREEN.
- `cargo test --test dashboard_render` 2/2 GREEN.
- `cargo test --lib` 325/325 GREEN (1 pre-existing ignored).
- `cargo build --quiet` exits 0.
- `cargo clippy --all-targets --all-features -- -D warnings` exits 0.
- HEAD on per-feature branch `phase23/discuss` (NOT `main`) — commit safety honored.

---

*Phase: 23-job-tagging-dashboard-filter-chips-rc-3*
*Plan: 05*
*Completed: 2026-05-05*
