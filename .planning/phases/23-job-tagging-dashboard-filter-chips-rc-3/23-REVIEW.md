---
phase: 23-job-tagging-dashboard-filter-chips-rc-3
reviewed: 2026-05-04T00:00:00Z
depth: standard
files_reviewed: 10
files_reviewed_list:
  - README.md
  - assets/src/app.css
  - justfile
  - src/db/queries.rs
  - src/web/handlers/api.rs
  - src/web/handlers/dashboard.rs
  - templates/pages/dashboard.html
  - templates/partials/job_table.html
  - tests/dashboard_jobs_pg.rs
  - tests/v12_tags_dashboard.rs
findings:
  critical: 1
  warning: 4
  info: 4
  total: 9
status: issues_found
---

# Phase 23: Code Review Report

**Reviewed:** 2026-05-04
**Depth:** standard
**Files Reviewed:** 10
**Status:** issues_found

## Summary

Phase 23 wires the dashboard tag filter chip strip on top of the Phase 22 tag persistence layer. The load-bearing pieces (axum_extra::Query swap, two-fetch fleet-tag fold, fleet-intersect retain at the security boundary, AND-chained `tags LIKE ?` predicates with JSON-quote anchors, OOB chip strip swap with wrapper-only hx-swap-oob, sort-header tag suffix, hidden-input round-trip, CSS-only chip toggle) all line up with the planning artefacts. Integration coverage in `tests/v12_tags_dashboard.rs` is dense and exercises the documented pitfalls.

The review surfaces one BLOCKER and four WARNINGs. The BLOCKER is a SQL LIKE wildcard hole: tag charset allows `_`, which is a single-character wildcard in SQLite/Postgres LIKE. This means the `tags LIKE '%"<tag>"%'` filter can produce false-positive matches across tag pairs that the P22 substring-collision validator (`str::contains`) does not catch (`back_up` vs `back-up`, `back_up` vs `backaup`, etc.). The remaining warnings cover minor robustness gaps and test-helper invariants worth tightening before the rc.3 cut.

## Critical Issues

### CR-01: SQL LIKE wildcard `_` in tag names enables cross-tag false-positive matches

**File:** `src/db/queries.rs:907-914` (sqlite arm) and `src/db/queries.rs:976-981` (postgres arm); related: `src/config/validate.rs:611-655`
**Issue:**

The dashboard tag filter binds `format!(r#"%"{}"%"#, t)` directly into a `LIKE` predicate without escaping LIKE metacharacters. The Phase 22 tag charset (`^[a-z0-9][a-z0-9_-]{0,30}$`, `src/config/validate.rs:476`) explicitly admits `_`, which is a single-character wildcard in both SQLite and PostgreSQL `LIKE`. The Phase 22 TAG-05 substring-collision validator (`src/config/validate.rs:611`) is implemented with plain `str::contains` (line 648: `a.contains(b.as_str()) || b.contains(a.as_str())`), which is purely literal — it does NOT account for the wildcard semantics that LIKE will apply later.

Concrete failure paths admitted by the current code (all charset-legal, all pass TAG-05):

- Fleet has `back_up` and `back-up`. `?tag=back_up` produces bind value `%"back_up"%`. SQL evaluates `_` as a wildcard, so a row whose stored `tags` JSON contains `"back-up"` matches as well. The dashboard hides untagged jobs and shows the `back-up` job under the `back_up` chip filter.
- Fleet has `back_up` and `backaup`. Same hazard — `_` matches `a`. TAG-05 sees no substring relationship (`back_up` is not a literal substring of `backaup` and vice versa).
- Fleet contains `release_v1` and `releaseav1`. Same wildcard hazard.

This breaks the AND-filter contract advertised in README.md:289 ("only jobs with ALL active tags render") because the SQL silently widens the predicate. There is no integration test asserting LIKE-safety against `_`-bearing tags (`tests/v12_tags_dashboard.rs` exercises only `backup`, `weekly`, `prod`, `ghost`, none of which contain `_`).

This was not introduced by Phase 23 (the `tags LIKE` pattern lands in P22), but Phase 23 is the first phase that lets the operator drive this predicate from the URL bar — so the surface is now operator-controlled, not config-controlled, and the bug ships with the chip filter.

**Fix:**

Two viable fixes. Either is acceptable; (B) is safer because it does not depend on validator extension.

(A) Extend `check_tag_substring_collision` to treat `_` as an equivalence-class character so `back_up` and `back-up`/`backaup`/etc. are rejected at config-load with the same kind of error TAG-05 already emits. This keeps the LIKE pattern as-is.

(B) Bind a literal-LIKE pattern. Append an explicit `ESCAPE` clause and escape `_` and `\` in the bound value. Both SQLite and Postgres support `LIKE ... ESCAPE '\'`:

```rust
// Replace the bind site at queries.rs:913 (sqlite arm) and :980 (postgres arm)
fn escape_like(t: &str) -> String {
    let mut out = String::with_capacity(t.len() + 4);
    for c in t.chars() {
        if c == '\\' || c == '%' || c == '_' { out.push('\\'); }
        out.push(c);
    }
    out
}
q = q.bind(format!(r#"%"{}"%"#, escape_like(t)));
```

…and widen `tag_predicates_sqlite` / `tag_predicates_postgres` to:

```rust
.map(|i| format!("AND tags LIKE ?{} ESCAPE '\\'", tag_bind_start + i))
```

Add a regression test in `tests/v12_tags_dashboard.rs`:

```rust
#[tokio::test]
async fn underscore_tag_does_not_match_dash_or_other_chars() {
    let (_app, pool) = build_test_app().await;
    seed_job_with_tags(&pool, "u", "*/5 * * * *", &["back_up"]).await;
    seed_job_with_tags(&pool, "d", "*/5 * * * *", &["back-up"]).await;
    let active = vec!["back_up".to_string()];
    let rows = queries::get_dashboard_jobs(&pool, None, "name", "asc", &active)
        .await.unwrap();
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["u"], "underscore must not match dash; got {names:?}");
}
```

## Warnings

### WR-01: Tag intersection is case-sensitive but URL casing is operator-supplied

**File:** `src/web/handlers/dashboard.rs:443-446`
**Issue:**

`active_tags.retain(|t| fleet_tags.contains(t))` is a case-sensitive comparison. Tags persisted in the DB are guaranteed lowercase (validator normalizes via `t.trim().to_lowercase()` at `src/config/validate.rs:621`), but the URL query parser does not lowercase `?tag=` values. An operator who types `?tag=Backup` instead of `?tag=backup` will see the chip silently dropped (treated as a stale tag) even though the tag is in the fleet. The user-visible failure mode is "I bookmarked a URL, opened it, and the chips do nothing."

The retain step cites the security boundary, which is correct, but the case-sensitivity is an unforced UX hazard.

**Fix:**

Lowercase `params.tags` BEFORE the retain, and ensure the retain still uses the canonicalized form. Two-line change:

```rust
let mut active_tags: Vec<String> = params.tags.iter().map(|t| t.to_lowercase()).collect();
active_tags.sort();
active_tags.dedup();
active_tags.retain(|t| fleet_tags.contains(t));
```

Add an integration test asserting `?tag=BACKUP` renders the `backup` chip active.

### WR-02: V-13 sort-header expectation will pass even when the partial-template render path drops `&tag=...` from one column

**File:** `tests/v12_tags_dashboard.rs:610-617`
**Issue:**

The assertion `sort_header_carries >= 8` uses `>=`, not `==`. Each of the four sortable columns (Name / Next Fire / Status / Last Run) is supposed to carry `&tag=backup` in BOTH `href` and `hx-get` — that is exactly 8 occurrences. If a future template edit accidentally drops one column's tag suffix while the chip strip's own URL adds two extra `&tag=` occurrences for some reason, the assertion still passes with the correct count present elsewhere on the page. The test does not actually verify per-column coverage; it only verifies total occurrences.

**Fix:**

Replace the `>=` count with explicit per-column assertions, or tighten to `==` after locking in the chip-strip URL form:

```rust
for col in ["sort=name&order=", "sort=next_run&order=", "sort=status&order=", "sort=last_run&order="] {
    let idx = body.find(col).unwrap_or_else(|| panic!("missing column anchor: {col}"));
    let scan_end = (idx + 400).min(body.len());
    let block = &body[idx..scan_end];
    assert!(block.contains("&tag=backup"),
        "column anchor `{col}` must include `&tag=backup`; window: {block}");
}
```

### WR-03: `chip.href` HTML-escapes `&` separators which is correct for `href` but worth a regression assertion for `hx-get`

**File:** `templates/pages/dashboard.html:34-43`, `templates/partials/job_table.html:23-32`
**Issue:**

`chip.href` is built by `url::form_urlencoded::Serializer` which produces e.g. `filter=&sort=name&order=asc&tag=backup&tag=weekly`. Askama auto-escapes the value to `filter=&amp;sort=name&amp;order=asc&amp;tag=backup&amp;tag=weekly` when rendered inside `href="?{{ chip.href }}"` and `hx-get="?{{ chip.href }}"`. Browsers correctly decode `&amp;` → `&` for `href`. HTMX, in current versions, also reads the raw attribute value and the browser has already decoded the entity by the time HTMX inspects the DOM, so this works. There is no integration-test assertion that the chip's `hx-get` actually fires a request with the expected query parameters (V-09 / V-10 only check class strings and aria-pressed; V-12 checks OOB shape but not chip click behavior). If a future templating change emits `&` literally into the attribute or strips the entity decode, the chip-click flow regresses with no test catching it.

**Fix:**

Either (a) add a regression test that performs an HTMX-style `GET /?{{ chip.href }}` round-trip and asserts the resulting HTML reflects the expected toggled active state, or (b) add a unit test on `build_chip_views` asserting the `href` contains the expected unescaped form (asserting the `chip.href` value, not the rendered HTML, sidesteps the askama escape).

```rust
#[test]
fn build_chip_views_post_toggle_active_remove() {
    let chips = build_chip_views(&["backup".into(), "weekly".into()],
                                 &["backup".into()], "", "name", "asc");
    let backup = chips.iter().find(|c| c.tag == "backup").unwrap();
    assert!(backup.is_active);
    assert!(!backup.href.contains("tag=backup"),
        "active chip must drop its own tag from post-toggle URL; got {}", backup.href);
    let weekly = chips.iter().find(|c| c.tag == "weekly").unwrap();
    assert!(!weekly.is_active);
    assert!(weekly.href.contains("tag=backup&tag=weekly"),
        "inactive chip must add itself to canonical URL; got {}", weekly.href);
}
```

### WR-04: Dashboard renders `?filter=` even when filter is empty, padding URLs and bookmarks unnecessarily

**File:** `src/web/handlers/dashboard.rs:190-200`, `templates/pages/dashboard.html:123,135,145,155`
**Issue:**

`build_chip_views` unconditionally calls `ser.append_pair("filter", filter)` even when `filter` is the empty string. The resulting URLs always include `filter=` (e.g., `/?filter=&sort=name&order=asc&tag=backup`). The sort-header anchors in `templates/pages/dashboard.html` follow the same pattern with `?filter={{ filter }}&sort=...`. Functionally correct (axum_extra::Query treats `filter=` as `filter=""` which `is_empty()` rejects in the handler), but bookmark URLs become longer and visually noisier than they need to be — and importantly, two visits with the same active-tag set don't share the canonical form between the chip strip's `chip.href` (no leading `?`) and the sort-header anchors (literal `?filter=...`).

**Fix:**

Skip the empty-filter pair in the chip view builder and gate the sort-header anchors. Defer-only fix if rc.3 is shipping; rc.4 hardening candidate.

```rust
if !filter.is_empty() { ser.append_pair("filter", filter); }
```

## Info

### IN-01: `untagged_clause` interpolation is safe but uses naked string interpolation into SQL

**File:** `src/db/queries.rs:865-869, 881, 894, 953, 966`
**Issue:**

`untagged_clause` is a `&'static str` chosen from a closed set of literal values (`"AND tags != '[]'"` or `""`). It is then interpolated directly into the SQL via `format!`. Functionally safe (no operator input flows into the value), but the codebase generally avoids string-interpolating SQL fragments. The pattern is documented and the value is closed-set, but a future reader could mistake it for a place where user input might land.

**Fix:**

Rename the local to a more clearly-static identifier (e.g., `UNTAGGED_HIDDEN_CLAUSE` / `EMPTY_CLAUSE` constants chosen by `if`) and reference the constant in the `format!` call. Optional cleanup; current code is correct.

### IN-02: `tag_predicates_sqlite` and `tag_predicates_postgres` differ only by placeholder syntax — opportunity to dedup

**File:** `src/db/queries.rs:853-860`
**Issue:**

The two `String` builds are structurally identical except for `?N` vs `$N`. A single helper that takes a `placeholder_fmt: fn(usize) -> String` would remove duplication and keep the two backend arms in lockstep. Pre-existing code style elsewhere in this file follows the same per-backend duplication pattern, so this is a project-wide opportunity, not a Phase 23-specific defect.

**Fix:** Optional refactor; non-blocking.

### IN-03: `chip.aria_label` em-dash is hand-formatted; consider lifting to a constant for i18n / a11y review

**File:** `src/web/handlers/dashboard.rs:202-206`
**Issue:**

The aria-label string is inlined with U+2014 em dash and parenthetical state. The two label variants ("active — click to remove" / "inactive — click to add") are minor copywriting that should sit alongside the rest of the UI-SPEC § Copywriting Contract for a single-source review pass. No correctness defect.

**Fix:** Optional — extract to module-level `const` strings or move to a future `copy.rs` module if the project has plans to centralize UI copy.

### IN-04: `unfiltered_jobs` discards database errors with `unwrap_or_default()`, hiding fleet-tag fold failures

**File:** `src/web/handlers/dashboard.rs:416-424, 466-474`
**Issue:**

Both `get_dashboard_jobs` calls in the handler use `unwrap_or_default()`. If the first (unfiltered) fetch errors transiently (e.g., DB connection blip), the chip strip silently disappears (empty `fleet_tags`) and the second fetch proceeds with `active_tags` already retained-down to empty. Operator sees an unfiltered table and no chips, with no error in the UI. The error IS logged inside `queries::get_dashboard_jobs`-callers via tracing elsewhere in the codebase, but here in this handler the `unwrap_or_default()` is bare — no `tracing::warn!` accompanies the silent default.

This matches the existing pattern in this handler (the second fetch was already `unwrap_or_default()` pre-Phase-23), so it is not a Phase 23 regression. Worth noting for rc.4 hardening if the silent-fall-through behavior is ever observed in production.

**Fix:** Add `tracing::warn!(target: "cronduit.web", ...)` adjacent to the `unwrap_or_default()` to surface DB transient errors in logs without changing user-visible behavior.

---

_Reviewed: 2026-05-04_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
