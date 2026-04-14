---
phase: 07-v1-cleanup-bookkeeping
gathered: 2026-04-12
status: ready_for_planning
---

# Phase 7: v1.0 Cleanup & Bookkeeping - Context

**Gathered:** 2026-04-12
**Status:** Ready for planning

<domain>
## Phase Boundary

Close 4 mechanical gaps surfaced by `/gsd-audit-milestone` so v1.0 archives cleanly:

1. Resolve the `ports:` vs `expose:` deviation in `examples/docker-compose.yml` (OPS-04)
2. Bulk-update `REQUIREMENTS.md` traceability so the master table reflects what actually shipped (per-phase `0X-VERIFICATION.md` reports as the source of truth)
3. Refresh the stale `05-VERIFICATION.md` (Phase 5 code gaps were closed in PR #9, master report still says `gaps_found`)
4. Ship the minor settings-page Reload Config card auto-refresh fix (filed in `05-UAT.md`, fix already in `main` via PR #9 — needs documentation + regression test)

**Pure bookkeeping.** No new features, no behavioral changes beyond what is already on `main`. If any discussion or planning surfaces a "while we're here" idea that adds capability, it goes to deferred.

**Phase 8 owns** human/visual UAT (browser walkthroughs, visual checkpoints). Anything that requires a human looking at a screen does NOT belong in Phase 7.

</domain>

<decisions>
## Implementation Decisions

### OPS-04 — `ports:` vs `expose:` Deviation

- **D-01:** Accept the D-12 deviation via `overrides:` block in `06-VERIFICATION.md` frontmatter. Keep `ports: 8080:8080` in `examples/docker-compose.yml` so a stranger running `docker compose up` reaches `localhost:8080` immediately. The override block must include `must_have`, `reason` (cite Phase 6 D-12 + the 5-minute quickstart promise of OPS-05), `accepted_by`, and `accepted_at` fields per the `overrides:` schema already documented in 06-VERIFICATION.md lines 137-143.
- **D-02:** Strengthen the existing 3-line comment at the top of `examples/docker-compose.yml` into a clearer SECURITY block. The new block must:
  - State loudly that `ports: 8080:8080` publishes the unauthenticated v1 web UI on the host network
  - Reference `THREAT_MODEL.md` (or the README SECURITY section if `THREAT_MODEL.md` does not exist on `main`)
  - Show the exact `expose:` replacement snippet for production deployments behind a reverse proxy
  - Preserve the existing usage / web UI URL lines below

### REQUIREMENTS.md Traceability Update

- **D-03:** **Strict cross-check, not bulk trust.** For each of the ~81 currently-Pending v1 REQ-IDs, the planner/executor must grep the matching `0X-VERIFICATION.md` for that exact REQ-ID and confirm it appears in a row with `SATISFIED` status before flipping the master table to `Complete`. PARTIAL or FAILED entries do NOT get flipped.
- **D-04:** **Add an evidence column to the traceability table.** The current 3-column shape (`Requirement | Phase | Status`) becomes 4-column: `Requirement | Phase | Status | Evidence`, where Evidence is the relative path to the per-phase verification file (e.g., `01-VERIFICATION.md`) that documents satisfaction. This makes future audits trivial and the master table self-defending.
- **D-05:** **PARTIAL items stay Pending with footnotes.** Any REQ-ID whose matching verification entry is `PARTIAL` (or `FAILED` for any reason other than already-closed-in-code-since-verification) keeps its `Pending` checkbox in the master table and gets a footnote pointing to the open issue. We do NOT promote partials based on "Phase 8 will handle the human UAT." Honest accounting > optimistic accounting.
- **D-06:** **OPS-04 specifically:** After D-01 + D-02 land and the override is recorded in 06-VERIFICATION.md, OPS-04 in the master table flips to `Complete` and cites both `06-VERIFICATION.md` (override entry) and `examples/docker-compose.yml` (strengthened comment). Without those changes landing first, OPS-04 stays Pending.
- **D-07:** **OPS-05 stays Pending.** Phase 8 owns OPS-05 (human UAT for the 5-minute quickstart promise). The Phase 7 update does not touch OPS-05.
- **D-08:** **Update the Coverage summary block** at the bottom of REQUIREMENTS.md (lines 267-282 area) so the prose, the 2026-04-12 note, and the per-phase distribution counts reflect the new state after the flip. Anything claiming "81 unflipped" must be updated to the post-flip number.

### 05-VERIFICATION.md Refresh

- **D-09:** **In-place `re_verification:` annotation.** Edit `05-VERIFICATION.md` directly. Do NOT regenerate via `/gsd-verify-work 5` — regeneration would either lose the original audit trail or still report `gaps_found` because the human visual checkpoint is a Phase-8 concern.
- **D-10:** **Annotation structure** — add a `re_verification:` block to the existing frontmatter (after the `human_verification:` block, before the closing `---`). The block must record:
  - `re_verified_at: 2026-04-12T<time>Z`
  - `re_verifier: Claude (Phase 7)`
  - `gap_resolutions:` list with one entry per original gap, each citing the closing commit/PR and the file:line where the fix lives:
    - Gap 1 (do_reroll stub): closed by PR #9 — `src/scheduler/reload.rs:170-172` now calls `crate::scheduler::random::resolve_schedule(&job.schedule, None, &mut rng)`
    - Gap 2 (unchanged count hardcoded): closed by PR #9 — `src/scheduler/reload.rs:88` now reads `unchanged: sync_result.unchanged`
    - Gap 3 (visual checkpoint): deferred to Phase 8 (human-only)
  - `status_change:` from `gaps_found` to `code_complete, human_needed`
- **D-11:** **Status field update** — the top-level `status:` field in 05-VERIFICATION.md frontmatter changes from `gaps_found` to `code_complete, human_needed`. The two-part status mirrors the 06-VERIFICATION.md `status: human_needed` pattern and makes it unambiguous that no code work remains while still flagging that Phase 8 must close the visual gate.
- **D-12 (note):** D-09's annotation is the source of truth for "what shipped after the audit." The original gap rows in the frontmatter stay where they are — they are now historical facts about the audit, not open work items.

### HX-Refresh Fix Documentation + Regression Test

- **D-13:** **Code is already in `main`.** The fix landed in PR #9: `src/web/handlers/api.rs:175-177` adds `headers.insert("HX-Refresh", "true".parse().unwrap())` to the reload response. No code change to `api.rs` is needed in Phase 7.
- **D-14:** **Add a regression test in a new file: `tests/reload_api.rs`.** The test must spin up an axum app (use the existing test harness pattern from `tests/reload_sighup.rs` / `tests/reload_inflight.rs` if available, otherwise build minimally) and assert that `POST /api/reload` with a valid CSRF token returns a response whose headers contain `HX-Refresh: true`. The test should NOT depend on docker, container networking, or any external system — it is a pure HTTP-handler test.
- **D-15:** **Cite the fix in the 05-VERIFICATION.md re_verification annotation** as a fourth gap_resolution entry: HX-Refresh on /api/reload — closed by PR #9, `src/web/handlers/api.rs:175-177`, regression covered by `tests/reload_api.rs::reload_response_includes_hx_refresh` (or whatever the planner names the test). This ties the UAT-filed bug to its closure.
- **D-16:** **Browser UAT for the auto-refresh behavior** is NOT in Phase 7 scope. If Phase 8's human UAT script needs to confirm the auto-refresh visually, that's a Phase 8 task. Phase 7 ships the code (already there) + automated test + documentation only.

### Claude's Discretion

- **Plan ordering / wave assignment.** The 4 work items have one real dependency: D-06 (flip OPS-04 in REQUIREMENTS.md) cannot land until D-01 + D-02 land. Otherwise everything is independent — the planner can choose whether to ship as 1 plan, 2 plans, or 4 plans, and how to wave them.
- **Test harness shape for `tests/reload_api.rs`.** The planner/executor decides whether to use `axum::test`, `tower::ServiceExt::oneshot`, an in-memory SQLite, or a full integration setup. Match whatever pattern is already idiomatic in the existing `tests/reload_*.rs` files.
- **Exact wording of strengthened docker-compose.yml comment** — the planner picks the language as long as it satisfies the must-haves in D-02.
- **Exact ISO timestamp** for `re_verified_at` in 05-VERIFICATION.md — use the time of the actual edit.
- **Whether to capture Phase 7's own work in a `07-VERIFICATION.md`** at the end of the phase. If GSD's standard flow auto-creates one, accept it. If not, the planner can decide whether bookkeeping work needs its own verification report or whether the audit-milestone re-run will serve.

### Folded Todos

None — `todo match-phase 7` returned 0 matches.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (researcher, planner, executor) MUST read these before planning or implementing.**

### Phase 7 Source-of-Truth Documents

- `.planning/ROADMAP.md` § "Phase 7: v1.0 Cleanup & Bookkeeping" — phase boundary, success criteria, and the explicit OPS-04 "either/or" wording the override path leans on
- `.planning/REQUIREMENTS.md` § "Traceability" (lines 178-282) — the master table being updated; current Coverage block including the 2026-04-12 audit note (line 271)

### Per-Phase Verification Reports (evidence sources for D-03 cross-check)

- `.planning/phases/01-foundation-security-posture-persistence-base/01-VERIFICATION.md` — FOUND, CONF, DB-01..07, OPS-03 evidence
- `.planning/phases/02-scheduler-core-command-script-executor/02-VERIFICATION.md` — SCHED-01..07, EXEC-01..06 evidence
- `.planning/phases/03-read-only-web-ui-health-endpoint/03-VERIFICATION.md` — UI-01..13, UI-15, OPS-01 evidence
- `.planning/phases/04-docker-executor-container-network-differentiator/04-VERIFICATION.md` — DOCKER-01..10, SCHED-08 evidence
- `.planning/phases/05-config-reload-random-resolver/05-VERIFICATION.md` — RAND-01..06, RELOAD-01..07 evidence; **also the file being edited under D-09..D-12**
- `.planning/phases/06-live-events-metrics-retention-release-engineering/06-VERIFICATION.md` — DB-08, OPS-02, UI-14 evidence; **also the file being edited under D-01** (overrides: block); contains the override schema example at lines 137-143

### Closing PR Reference

- PR #9 (squash-merged as commit `8b69cb8` on `main`): contains the `do_reroll()` random integration, the `unchanged: sync_result.unchanged` wiring, and the `HX-Refresh` reload header. Full message via `git show 8b69cb8`.

### UAT Source

- `.planning/phases/05-config-reload-random-resolver/05-UAT.md` § "5. Settings Page Shows Reload Card" — the original user report ("after clicking reload, the page doesn't refresh automatically. but if i refresh it, the data is there.") that drove the HX-Refresh fix

### Code Files Touched / Read

- `examples/docker-compose.yml` — file being edited under D-02; current state has `ports: 8080:8080` and a 3-line comment
- `src/web/handlers/api.rs` (lines 78-189) — `pub async fn reload()`; the `HX-Refresh` header insertion is at line 177; file is read-only for Phase 7
- `src/scheduler/reload.rs` (lines 88, 170-172, 116-180) — `do_reload()` and `do_reroll()`; file is read-only for Phase 7 (the fixes are already in)
- `templates/pages/settings.html` (line 8) — Reload Config form; read-only for Phase 7
- `tests/reload_sighup.rs`, `tests/reload_inflight.rs`, `tests/reload_random_stability.rs`, `tests/reload_file_watch.rs` — existing reload test files; read for harness patterns to mimic in the new `tests/reload_api.rs`

### Project-Level Docs

- `CLAUDE.md` (project root) — locked stack constraints (axum 0.8 + askama_web + HTMX vendored, no CDN), default loopback bind, mermaid-only diagrams, PR-only workflow (no direct commits to `main`)
- `THREAT_MODEL.md` (if present at repo root) — to cite from the strengthened docker-compose.yml comment under D-02. If absent, cite the README SECURITY section instead.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`overrides:` schema in 06-VERIFICATION.md frontmatter** — already documented at lines 137-143 with the exact 4-field shape (`must_have`, `reason`, `accepted_by`, `accepted_at`). D-01 just instantiates this schema; nothing new to design.
- **`re_verification:` annotation pattern** — not yet used in the project but the frontmatter is YAML, so a `re_verification:` block parses cleanly alongside existing `gaps:` and `human_verification:` blocks. No tooling change needed.
- **Existing `tests/reload_*.rs` integration test files** — the new `tests/reload_api.rs` should follow whatever harness setup these already use (axum app builder, in-memory SQLite, CSRF cookie/token pattern).
- **`HxResponseTrigger::normal([event])` + `HeaderMap` response shape** in `src/web/handlers/api.rs:179-184` — read-only reference for understanding what the test should assert against; do NOT modify.

### Established Patterns

- **Per-phase VERIFICATION.md is YAML frontmatter + Markdown body.** Frontmatter holds machine-readable status fields; body holds the human-readable narrative tables. Editing frontmatter (D-09, D-11) is structurally safe.
- **OPS-04 + OPS-05 already get special handling** in REQUIREMENTS.md line 264-265: their Phase column reads `Phase 6 → Phase 7 (gap closure)` / `Phase 6 → Phase 8 (gap closure)`. After D-06 lands, OPS-04's row should keep that history-preserving format and just flip Status to Complete, citing 06-VERIFICATION.md (override) + examples/docker-compose.yml (comment).
- **CSRF tokens for /api/reload** are bound to the cookie `csrf::CSRF_COOKIE_NAME` and validated via `csrf::validate_csrf(...)`. The new test in D-14 must construct a valid token pair or it will get a 403 instead of the HX-Refresh response.
- **No direct commits to main** — every change in Phase 7 lands via a feature branch + PR. (Per `CLAUDE.md` and the user's persistent feedback memory.)

### Integration Points

- **`REQUIREMENTS.md` ↔ per-phase `0X-VERIFICATION.md` files** — the cross-check loop in D-03. Each verification file already has a "Requirements Coverage" table that the executor can grep for `<REQ-ID>` + `SATISFIED` to confirm before flipping.
- **`06-VERIFICATION.md` ↔ `examples/docker-compose.yml`** — the override block in 06 references the file's intentional ports: choice; the file's strengthened comment (D-02) should reference the override decision back. Bidirectional doc link.
- **`tests/reload_api.rs` ↔ `src/web/handlers/api.rs::reload`** — the new test exercises the existing handler. No changes to the handler.

</code_context>

<specifics>
## Specific Ideas

- The 06-VERIFICATION.md `overrides:` example block (lines 137-143) literally shows the field names and shape we should use for D-01. Copy that structure verbatim, fill in `accepted_by` with the user's GitHub handle (`SimplicityGuy` per the recent commits author) or actual name, and `accepted_at` with the ISO timestamp at edit time.
- The `re_verification:` block under D-10 is new — no precedent in the codebase. The planner should pick a YAML shape that's grep-friendly: top-level `re_verification:` key, nested `re_verified_at`, `re_verifier`, `gap_resolutions:` (list), `status_change:` (object with `from`/`to`).
- For the new `tests/reload_api.rs`, name the test function `reload_response_includes_hx_refresh_header` so its purpose is unambiguous. If multiple tests end up in the file, also add `reload_response_includes_toast_event` and `reload_csrf_required_returns_403` so the file isn't a one-test orphan.
- The strengthened docker-compose.yml comment under D-02 should NOT use ASCII art, tables, or boxes — keep it plain `#`-prefixed lines. The repo's diagrams-must-be-mermaid rule (per memory and CLAUDE.md) means even in a comment we should not introduce ASCII art.

</specifics>

<deferred>
## Deferred Ideas

- **Browser UAT for the reload card auto-refresh behavior** — belongs in Phase 8 (human UAT). Phase 7 ships the test + docs only.
- **Visual checkpoint signoff for 05-VERIFICATION.md gap 3** — belongs in Phase 8 (human-only).
- **OPS-05 (5-minute stranger quickstart UAT)** — already on Phase 8.
- **Auto-generation of the REQUIREMENTS.md traceability table from per-phase verification files** — could be a script (`tools/sync-traceability.sh` or similar) for v1.1+. Out of scope for Phase 7; manual update is fine for the one-time bookkeeping flip.
- **`07-VERIFICATION.md` content** — if GSD's standard plan-phase flow generates it automatically, accept whatever shape it takes. If not, leaving Phase 7 without its own verification file is acceptable since the audit-milestone re-run after Phase 8 will catch any drift.
- **THREAT_MODEL.md creation** — if the file does not yet exist on `main`, create it as a separate phase/task. For Phase 7, the strengthened docker-compose.yml comment cites whichever security doc actually exists (README SECURITY section as fallback).

</deferred>

---

*Phase: 07-v1-cleanup-bookkeeping*
*Context gathered: 2026-04-12*
