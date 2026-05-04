# Phase 23: Job Tagging Dashboard Filter Chips — rc.3 - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-04
**Phase:** 23-job-tagging-dashboard-filter-chips-rc-3
**Areas discussed:** Chip strip layout + CSS, Distinct-tag source + order, Filter SQL + URL + HTMX, rc.3 cut + UI-SPEC routing

---

## Chip strip layout + CSS

### Q1 — Chip strip placement

| Option | Description | Selected |
|--------|-------------|----------|
| Dedicated row above name-filter | New row at top: chip strip, then existing name-filter below. Reads as "narrow by tag, then narrow by name". | ✓ |
| Inline with name-filter (same row, wrapping) | Chips share L19-36 row with name-input. Saves vertical space but cramps below ~16 chips at narrow widths. | |
| Below name-filter, above table | Name stays at L19-36; new strip slots between filter row and table at L74. Reads as "name first, then tags" — opposite of AND composition feel. | |
| You decide | Leave to planner / UI design. | |

**User's choice:** Dedicated row above name-filter
**Notes:** Matches AND-composition reading order; clear visual hierarchy.

### Q2 — Empty-state when fleet has zero tagged jobs

| Option | Description | Selected |
|--------|-------------|----------|
| Hide the chip strip entirely | Dashboard looks identical to v1.0 until any job has tags. Mirrors `cd-bulk-action-bar` hidden-until-relevant pattern. | ✓ |
| Render placeholder copy | Muted strip "No tags configured — add `tags = [...]`...". Adds chrome operators don't need. | |
| Render empty strip with `?` info-icon tooltip | Visible chrome but minimal. Heaviest — needs another tooltip primitive. | |

**User's choice:** Hide the chip strip entirely
**Notes:** Discoverability handled by docs/README; no chrome unless the feature is active.

### Q3 — Mobile / narrow-viewport behavior

| Option | Description | Selected |
|--------|-------------|----------|
| `flex-wrap` to multiple rows | `display: flex; flex-wrap: wrap; gap`. Always fully visible, no scroll. | ✓ |
| Horizontal scroll (single row) | Fixed single-row strip with `overflow-x: auto`. Compact vertically but discoverability suffers. | |
| Collapsed in `<details>` above threshold (e.g., >12 chips) | Render up to N inline; rest behind `<details>`. Doesn't auto-expand on URL-active state — special render path needed. | |

**User's choice:** `flex-wrap` to multiple rows
**Notes:** Tags-as-discovery means seeing them all is the point; vertical space cost acceptable.

### Q4 — CSS class namespace

| Option | Description | Selected |
|--------|-------------|----------|
| `cd-tag-chip` + `cd-tag-chip--active` / `--inactive` | New family explicitly named for tags. Mirrors P21 `cd-fctx-*` / `cd-exit-*` namespacing. | ✓ |
| `cd-chip` + `cd-chip--active` / `--inactive` | Generic chip primitive (could be reused for status/type filters in future). Less self-documenting; risks future drift. | |
| Extend `cd-badge` with `--filter` / `--filter-active` | Treats chips as badge variant. Badges are read-only labels (cd-badge--disabled etc.); muddies semantics. | |

**User's choice:** `cd-tag-chip` + `cd-tag-chip--active` / `--inactive`
**Notes:** Tag-namespace future-proofs for v1.3+ tag UI extensions.

---

## Distinct-tag source + ordering

### Q1 — Source of distinct fleet tags

| Option | Description | Selected |
|--------|-------------|----------|
| Rust-side fold over DashboardJob | Extend struct with `tags: Vec<String>`; handler folds via BTreeSet. Single query, no dialect divergence. | ✓ |
| SQL DISTINCT over JSON column | Sqlite `json_each` + postgres `jsonb_array_elements_text`. Two divergent dialect arms; breaks parity-friendly TEXT abstraction. | |
| Walk in-memory CronConfig.jobs at runtime | Web layer doesn't currently have config in scope; coupling creates freshness window if config reloads. | |

**User's choice:** Rust-side fold over DashboardJob
**Notes:** Mirrors P21 OBS-04 raw-fetch-then-aggregate.

### Q2 — How tags are sourced into DashboardJob

| Option | Description | Selected |
|--------|-------------|----------|
| Project j.tags into the existing SELECT | Add `j.tags AS tags_json` to both backend SELECTs; deserialize at row-mapping. Single query, identical shape across dialects. | ✓ |
| Separate query for tags, joined Rust-side | Two queries for no benefit — column lives on the same row we're already reading. | |

**User's choice:** Project j.tags into the existing SELECT
**Notes:** Mirrors P14 `enabled_override` precedent.

### Q3 — Tag ordering in the strip

| Option | Description | Selected |
|--------|-------------|----------|
| Alphabetical | Stable, predictable; matches P22 sorted-canonical JSON storage; easiest a11y. | ✓ |
| By usage frequency (most-used first) | More "useful" but order shifts as fleet evolves; muscle memory breaks. | |
| Config-declared order (preserve insert order) | P22 D-09 sorted-canonical JSON form already loses insert order at storage layer. | |

**User's choice:** Alphabetical
**Notes:** Storage form, payload form, and chip strip all match.

### Q4 — Where the distinct-tag union is computed

| Option | Description | Selected |
|--------|-------------|----------|
| In the dashboard() handler after get_dashboard_jobs() call | Keep DB layer pure; composition in handler. Mirrors P13 OBS-03 sparkline pattern. | ✓ |
| Inside get_dashboard_jobs() returning a tuple | Couples two concerns; harder to test aggregation in isolation. | |
| New dedicated query function get_distinct_fleet_tags() | Second query for data we already have in scope. | |

**User's choice:** In the dashboard() handler after get_dashboard_jobs() call
**Notes:** DB layer stays a pure read; aggregation lives where similar aggregations already do.

---

## Filter SQL + URL parsing + HTMX swap mechanics

### Q1 — Filter SQL composition

| Option | Description | Selected |
|--------|-------------|----------|
| AND-chained `tags LIKE ?N` per active tag + `tags != '[]'` when active | Parity-friendly LIKE; TAG-05 substring-collision validator gates safety. Format-string the count of LIKEs from whitelist-bound active set. | ✓ |
| `json_each` / `jsonb_array_elements_text` per backend | Two divergent dialect arms; no correctness benefit since TAG-05 prevents substring false-positives. | |
| Fetch all then filter Rust-side | Defensible at homelab scale but wastes rows when SQL form is trivial. | |

**User's choice:** AND-chained `tags LIKE ?N` per active tag + `tags != '[]'` when active
**Notes:** Inherits P22 validator-guarantees; matches existing parity-friendly pattern.

### Q2 — URL extractor for repeated `?tag=`

| Option | Description | Selected |
|--------|-------------|----------|
| `axum_extra::extract::Query<DashboardParams>` with `Vec<String>` field | Already in tree; serde_html_form supports repeated keys. Single-line change. | ✓ |
| Keep axum::Query, parse raw query string manually | Reinvents what axum_extra already does. | |
| Use serde_qs | Adds a new dep for what axum_extra does for free. | |

**User's choice:** axum_extra::extract::Query
**Notes:** Standard idiom, no new deps.

### Q3 — HTMX swap mechanics on chip toggle

| Option | Description | Selected |
|--------|-------------|----------|
| OOB swap of chip strip + targeted swap of #job-table-body | Single round-trip; both pieces refresh; no JS for toggle; 3s poll keeps working. | ✓ |
| Swap a wrapper #dashboard-content containing both | Larger swap target trips the existing 3s polling target; flicker risk. | |
| Static chips + #job-table-body swap only | Breaks the active=teal/inactive=grey toggle UX explicitly required by TAG-06. | |

**User's choice:** OOB swap of chip strip + targeted swap of #job-table-body
**Notes:** Introduces hx-swap-oob to the codebase — UI-SPEC.md should codify the contract.

### Q4 — 3s poll filter preservation

| Option | Description | Selected |
|--------|-------------|----------|
| Hidden `<input name="tag">` per active tag, included by name | Standard HTMX idiom; axum_extra deserializes form fields same as URL. | ✓ |
| hx-include via CSS selector targeting active chips with data-attribute | data-attributes don't form-encode; needs hx-vals/custom serializer. | |
| Encode active tags into polling URL via hx-vals JSON | JSON keys can't repeat; needs array serialization that's brittle. | |

**User's choice:** Hidden `<input name="tag">` per active tag
**Notes:** Hidden inputs OOB-replaced together with chip strip on toggle.

### Q5 — Sort-header href composition

| Option | Description | Selected |
|--------|-------------|----------|
| Each sort-header href must include active `?tag=` params | Required for shareable URL state to round-trip; both href and hx-get carry tags. | ✓ |
| Use hx-include on sort-headers to pull from chip-strip hidden inputs | Only works for HTMX path; href still needs params for non-HTMX. | |
| Both — hrefs carry tags AND hx-include pulls from hidden inputs | Belt-and-suspenders; most robust. | |

**User's choice:** Each sort-header href must include active `?tag=` params
**Notes:** Mechanical composition — planner may extract askama macro for readability (Claude's discretion noted).

---

## rc.3 cut + UI-SPEC routing

### Q1 — UI-SPEC.md routing

| Option | Description | Selected |
|--------|-------------|----------|
| Run `/gsd-ui-phase 23` after this CONTEXT lands | Mirrors P21 discipline; locks visual contract before planning. | ✓ |
| Inline visual contract in CONTEXT.md / PLAN.md | Visual decisions get re-litigated during implementation. | |
| Defer UI-SPEC to PLAN-time | Highest re-litigation risk; pattern explicitly retired after v1.1. | |

**User's choice:** Run `/gsd-ui-phase 23` after this CONTEXT lands
**Notes:** Roadmap labels Phase 23 as "UI hint: yes"; pattern matches.

### Q2 — rc.3 cut shape

| Option | Description | Selected |
|--------|-------------|----------|
| Mirror P21 D-22..D-26 verbatim | Reuse `docs/release-rc.md`; Cargo.toml stays 1.2.0; autonomous=false 23-RC3-PREFLIGHT.md final wave; no release.yml/cliff.toml edits. | ✓ |
| Different rc cut shape | Free-text describe what should change vs P21. | |

**User's choice:** Mirror P21 D-22..D-26 verbatim
**Notes:** Same hyphen-gate, same git-cliff authoritative body, same maintainer-cuts-locally posture.

### Q3 — Test + UAT shape

| Option | Description | Selected |
|--------|-------------|----------|
| Mirror P22 D-09..D-11 shape | New `tests/v12_tags_dashboard.rs` + extend handler tests + 3 new `just` recipes following recipe-calls-recipe + `23-HUMAN-UAT.md`. | ✓ |
| Heavier — add browser-based test (playwright) | New test infrastructure not in tree; v1.3 candidate. | |
| Lighter — just extend existing dashboard handler tests | SQL composition + URL extractor + OOB swap mechanics need integration coverage; reject. | |

**User's choice:** Mirror P22 D-09..D-11 shape
**Notes:** Recipe-calls-recipe pattern keeps UAT discoverable + repeatable.

---

## Claude's Discretion

The user explicitly delegated to Claude / planner on these (also captured in CONTEXT.md `<decisions>` § Claude's Discretion):

- Plan count and grouping (suggested 6-plan split; planner may collapse or expand).
- Per-tag job count badge on chips (`backup (3)` shape) — UI-SPEC owns.
- Chip label rendering shape (`backup` vs `# backup` vs CSS-decorated).
- Disabled-job tag inclusion in fleet-tag union (default: mirror what's rendered = enabled jobs only).
- Active-tag URL canonicalization (recommendation: sort alphabetically before push-url).
- Stale-tag handling (recommendation: silently drop unknown tag from active set).
- Sort-header href template idiom (inline `{% for %}` vs extracted askama macro).
- Template file split (chip strip as inline vs sibling partial `templates/partials/chip_strip.html`).
- README addition (Phase 22 deferred subsection on tag filtering — this phase is the natural landing site).
- `cd-tag-chip-*` CSS placement in `assets/src/app.css` `@layer components` (P21 precedent).

## Deferred Ideas

Captured in CONTEXT.md `<deferred>`:

- Tag autocomplete / search-as-you-type (v1.3).
- Tag-based bulk operations (v1.3 per REQUIREMENTS.md Out of Scope).
- Tag chips on `/jobs/{id}` (job detail) page (future phase).
- Tags as Prometheus label (out-of-scope; cardinality discipline).
- Tag-based webhook routing keys (P18 D-17 lock).
- Per-tag job count badge on chips (Claude's discretion / UI-SPEC call).
- Browser-based playwright smoke test for HTMX chip clicks (v1.3 at most).
- Sort-header href askama macro (v1.3 readability cleanup if planner inlines).
- Active-set URL canonicalization beyond sort (Claude's discretion / planner picks).
- Stale-tag handling beyond silent-drop (over-engineered for v1.2).
- README configuration subsection on tag filtering (Claude's discretion; deferred to P24 if not landed here).
- Tag display on disabled jobs (out of scope; dashboard hides disabled jobs).
- `docs/release-rc.md` modifications (reused verbatim per D-15/D-16).
