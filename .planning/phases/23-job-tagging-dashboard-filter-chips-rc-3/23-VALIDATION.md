---
phase: 23
slug: job-tagging-dashboard-filter-chips-rc-3
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-04
---

# Phase 23 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: `23-RESEARCH.md` § Validation Architecture (HIGH confidence, anchored to in-tree code).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (CI uses `cargo nextest`) |
| **Config file** | None at root — sqlx + standard cargo testing; per-test `tests/*.rs` files |
| **Quick run command** | `cargo test --test v12_tags_dashboard` |
| **Full suite command** | `just check` (lint + clippy + fmt + test) |
| **Estimated runtime** | ~30s quick / ~3 min full (matrix `linux/{amd64,arm64} × {SQLite, Postgres}` runs in CI) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --test v12_tags_dashboard --no-run` (compile gate; no DB needed)
- **After every plan wave:** Run `just check` (full lint + clippy + fmt + tests including SQLite + Postgres matrix)
- **Before `/gsd-verify-work`:** Full CI matrix green on `main` (`linux/{amd64,arm64} × {SQLite, Postgres}`)
- **Before `23-RC3-PREFLIGHT.md` Section 2:** Full suite green
- **Max feedback latency:** ~30 seconds for compile-gate signal per commit

---

## Per-Task Verification Map

> Source: `23-RESEARCH.md` § Phase Requirements → Test Map. Task IDs assigned by planner; this matrix is the requirement-side contract that PLAN tasks will reference.

| # | Plan area | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---|-----------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| V-01 | DB / SQL | 1 | TAG-07 | T-V12-TAG-07 | AND-tag SQL filters with parameterized binds; LIKE clause uses `?N`/`$N` only | integration (SQL) | `cargo test --test v12_tags_dashboard and_filter_two_tags` | ❌ W0 | ⬜ pending |
| V-02 | DB / SQL | 1 | TAG-07 | T-V12-TAG-08 | Untagged jobs hidden when active set non-empty (`tags != '[]'` clause gated) | integration (SQL) | `cargo test --test v12_tags_dashboard untagged_hidden_when_filter_active` | ❌ W0 | ⬜ pending |
| V-03 | DB / SQL | 1 | TAG-07 | T-V12-TAG-09 | No filter → all jobs (tagged + untagged) shown (no regression on default load) | integration (SQL) | `cargo test --test v12_tags_dashboard no_filter_shows_all_jobs` | ❌ W0 | ⬜ pending |
| V-04 | DB / SQL | 1 | TAG-07 | — | `?filter=foo&tag=backup` composes name LIKE foo AND has tag backup | integration (SQL) | `cargo test --test v12_tags_dashboard and_with_name_filter` | ❌ W0 | ⬜ pending |
| V-05 | Handler | 2 | TAG-06 | — | `?tag=backup&tag=weekly` deserializes to `Vec<String>` length 2 via `axum_extra::Query` | unit (handler) | `cargo test --lib web::handlers::dashboard::tests::active_tags_parsed_from_repeated_query` | ❌ W0 | ⬜ pending |
| V-06 | Handler | 2 | TAG-06 | T-V12-TAG-10 | Stale tag (not in `fleet_tags`) silent-dropped at handler; no 500, no error echo | integration | `cargo test --test v12_tags_dashboard stale_tag_silent_drop` | ❌ W0 | ⬜ pending |
| V-07 | Handler | 2 | TAG-06 | — | Distinct-tag fold from `Vec<DashboardJobView>` produces sorted union for chip strip | unit (handler) | `cargo test --lib web::handlers::dashboard::tests::distinct_tag_fold_alphabetical` | ❌ W0 | ⬜ pending |
| V-08 | Template | 3 | TAG-06 | — | Dashboard renders one chip per distinct fleet tag, alphabetical, hidden when empty | integration (HTML shape) | `cargo test --test v12_tags_dashboard chip_strip_render` | ❌ W0 | ⬜ pending |
| V-09 | Template | 3 | TAG-06 | — | Active chip has `cd-tag-chip--active` + `aria-pressed="true"`; inactive has `cd-tag-chip--inactive` | integration (HTML shape) | `cargo test --test v12_tags_dashboard chip_active_state_class` | ❌ W0 | ⬜ pending |
| V-10 | Template | 3 | TAG-06 | — | Direct GET `/?tag=backup&tag=weekly` renders chips active on first paint (bookmarkable) | integration | `cargo test --test v12_tags_dashboard direct_url_renders_chips_active` | ❌ W0 | ⬜ pending |
| V-11 | Template | 3 | TAG-08 | — | Chip element has correct CSS classes; no inline JS introduced | integration (HTML shape) | `cargo test --test v12_tags_dashboard css_only_chip_no_inline_js` | ❌ W0 | ⬜ pending |
| V-12 | Template | 3 | TAG-08 | — | HTMX response renders BOTH `#cd-tag-chip-strip[hx-swap-oob="true"]` AND `#job-table-body` content | integration (response shape) | `cargo test --test v12_tags_dashboard oob_response_shape` | ❌ W0 | ⬜ pending |
| V-13 | Template | 3 | TAG-08 | T-V12-TAG-11 | Sort-header `href` AND `hx-get` both contain `&tag=...` for every active tag | integration (HTML shape) | `cargo test --test v12_tags_dashboard sort_header_carries_active_tags` | ❌ W0 | ⬜ pending |
| V-14 | Template | 3 | TAG-08 | — | Hidden `<input name="tag">` rendered for each active tag; poll `hx-include` lists `[name='tag']` (3s poll preserves filter) | integration (HTML shape) | `cargo test --test v12_tags_dashboard poll_hx_include_widened` | ❌ W0 | ⬜ pending |
| V-15 | UAT | 4 | TAG-06 | — | Maintainer eyeballs chip color + bold weight in dark + light mode + mobile reflow | manual-only (UAT) | `just uat-chips-render` | ❌ W0 | ⬜ pending |
| V-16 | UAT | 4 | TAG-06+07 | — | Maintainer toggles two chips, copies URL, pastes into new tab → identical state | manual-only (UAT) | `just uat-chips-and-filter` + `just uat-chips-share-url` | ❌ W0 | ⬜ pending |
| V-17 | Release | 5 | rc.3 | — | `ghcr.io/SimplicityGuy/cronduit:v1.2.0-rc.3` exists on amd64 + arm64; `:latest` still at `v1.1.0` | manual-only (post-publish) | `gh release view v1.2.0-rc.3 --json isPrerelease --jq .isPrerelease` + `docker manifest inspect ghcr.io/SimplicityGuy/cronduit:v1.2.0-rc.3` | Captured in `23-RC3-PREFLIGHT.md` | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

**Wave assignments are advisory** — the planner sets the authoritative wave numbers based on dependency analysis. The above grouping reflects the natural dependency order (DB → handler → template → UAT → release).

---

## Wave 0 Requirements

- [ ] `tests/v12_tags_dashboard.rs` — covers TAG-06..08 (integration; mirrors `tests/v12_tags_validators.rs` and `tests/dashboard_render.rs` harnesses)
- [ ] `src/web/handlers/dashboard.rs::tests` — extend with handler-side fold + active-set parsing tests (unit)
- [ ] `justfile` — three new recipes (`uat-chips-render`, `uat-chips-and-filter`, `uat-chips-share-url`) per CONTEXT D-17
- [ ] `23-HUMAN-UAT.md` — `autonomous: false` maintainer plan (mobile viewport, light mode, keyboard nav, screen-reader, end-to-end with name filter)
- [ ] `23-RC3-PREFLIGHT.md` — `autonomous: false` maintainer plan (mirrors `21-RC2-PREFLIGHT.md` verbatim modulo rc.2→rc.3 + P21→P23 substitutions)

*All test infrastructure exists; gap is the new test files and `just` recipes only — no framework install needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Chip color/visual rendering across themes + viewport | TAG-06 | Visual brand check (terminal-green palette + bold weight + dark/light + mobile reflow) requires human eyeballs | `just uat-chips-render` — operator runs the recipe, screenshots dashboard at desktop + mobile breakpoints, both color schemes |
| End-to-end AND-filter + share-URL round-trip | TAG-06 + TAG-07 | UX assertion ("identical state when URL pasted into a new tab") requires browser-level round-trip | `just uat-chips-and-filter` + `just uat-chips-share-url` |
| Keyboard nav + screen-reader announcement of chip-state changes | TAG-06 | A11y assertion requires assistive-tech replay | Captured in `23-HUMAN-UAT.md` |
| GHCR image publish + `:latest` non-promotion on rc tag | rc.3 release | Cross-system check (GitHub Releases, GHCR, multi-arch manifest) requires post-push verification | `gh release view v1.2.0-rc.3 --json isPrerelease` + `docker manifest inspect …:v1.2.0-rc.3` (in `23-RC3-PREFLIGHT.md` Section 2) |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (5 items above)
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s for compile gate; < 3 min for full suite
- [ ] `nyquist_compliant: true` set in frontmatter (after planner wires every task to a row in this matrix)

**Approval:** pending
