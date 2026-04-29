---
id: 16-04b
phase: 16
plan: 04b
type: execute
wave: 2
depends_on: ["16-04a"]
autonomous: true
requirements_addressed: [FOUND-14, FCTX-04]
files_modified:
  - src/scheduler/run.rs
  - src/web/handlers/api.rs
  - src/db/queries.rs
  - justfile
must_haves:
  truths:
    - "All four production callers are updated: run.rs:83 insert_running_run (passes &job.config_hash), run.rs:348 finalize_run (already passes image_digest_for_finalize.as_deref() from 16-03; this plan validates compile), api.rs:82 insert_running_run (passes &job.config_hash), api.rs:131 finalize_run error fallback (passes None for image_digest)."
    - "All test-side insert_running_run callers in src/ are updated to pass the literal config_hash test value `\"testhash\"` (5 sites: 1 in src/scheduler/run.rs + 4 in src/db/queries.rs tests)."
    - "A new just recipe `uat-fctx-bugfix-spot-check` exists for the HUMAN-UAT spot check."
    - "cargo build is green; cargo test is green for the non-ignored suites; just schema-diff stays green; just clippy + fmt-check are green; just grep-no-percentile-cont stays green."
  artifacts:
    - path: "src/scheduler/run.rs"
      provides: "Production caller updates: insert_running_run at L83 (config_hash), finalize_run at L348 (already done in 16-03; this plan validates compile); test-mod insert_running_run caller update at L794 (testhash literal)"
      contains: "&job.config_hash"
    - path: "src/web/handlers/api.rs"
      provides: "Production caller updates: insert_running_run at L82 (config_hash), finalize_run error fallback at L131 (image_digest=None)"
      contains: "&job.config_hash"
    - path: "src/db/queries.rs"
      provides: "Test-mod insert_running_run caller updates at the 4 test sites identified by RESEARCH Pitfall 2"
      contains: "\"testhash\""
    - path: "justfile"
      provides: "New uat-fctx-bugfix-spot-check recipe for the HUMAN-UAT validation step"
      contains: "uat-fctx-bugfix-spot-check"
  key_links:
    - from: "src/scheduler/run.rs::run_job"
      to: "src/db/queries.rs::insert_running_run"
      via: "&job.config_hash bound from DbJob (in scope from get_job_by_id)"
      pattern: "insert_running_run.*&job.config_hash"
    - from: "src/web/handlers/api.rs::run_now"
      to: "src/db/queries.rs::insert_running_run"
      via: "&job.config_hash bound from DbJob (fetched at api.rs:66 via get_job_by_id)"
      pattern: "insert_running_run.*config_hash"
    - from: "src/web/handlers/api.rs L131 error fallback"
      to: "src/db/queries.rs::finalize_run"
      via: "passes None for image_digest (the run never started a container)"
      pattern: "finalize_run.*None"
---

<objective>
Land the four production caller updates + 5 test-mod caller updates that compose with Plan 16-04a's queries.rs signature changes to make the codebase compile cleanly. Add the just recipe needed by the HUMAN-UAT spot check. Run the full local CI gate to validate the wave-end state of the codebase.

Purpose: Plan 16-04b is the second half of the original Plan 16-04. After 16-04a updates the queries.rs signatures + struct widening, 16-04b updates every call site that consumes those signatures (production + test) and adds the operator-observable spot-check recipe. The wave-end gate (T5) is the canonical "PR 1 is mergeable" verification — `cargo build` + `cargo clippy` + `just schema-diff` + the integration tests from Plans 16-01 and 16-03 all green.

Output: 3 MODIFIED source files (`src/scheduler/run.rs`, `src/web/handlers/api.rs`, `src/db/queries.rs`) + 1 MODIFIED `justfile`. T-V12-FCTX-03 / T-V12-FCTX-04 write-site assertions are deferred to Plan 16-05's tests/v12_fctx_streak.rs (per CONTEXT.md note).

Note: Plan 16-04b was split out of the original Plan 16-04 to honor the per-plan task-count cap. 16-04a + 16-04b together cover the original 16-04 scope; both ride Wave 2 together. 16-04b strictly depends on 16-04a (the queries.rs signatures must already be applied for the call-site updates here to compile).
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/16-failure-context-schema-run-rs-277-bug-fix/16-CONTEXT.md
@.planning/phases/16-failure-context-schema-run-rs-277-bug-fix/16-RESEARCH.md
@.planning/phases/16-failure-context-schema-run-rs-277-bug-fix/16-PATTERNS.md
@CLAUDE.md
@src/scheduler/run.rs
@src/web/handlers/api.rs
@src/db/queries.rs
@justfile

<interfaces>
After Plan 16-04a, queries.rs has:
- `finalize_run(... container_id: Option<&str>, image_digest: Option<&str>)` — 8th positional parameter added.
- `insert_running_run(... trigger: &str, config_hash: &str)` — 4th parameter added.
- `DbRun` and `DbRunDetail` carry `image_digest: Option<String>` and `config_hash: Option<String>` fields.
- All 4 SELECT-site arms hydrate the new fields.

Production callers needing update (RESEARCH §B verified):
- run.rs:83 — `insert_running_run(&pool, job.id, &trigger).await` — `DbJob.config_hash` is in scope.
- run.rs:348-356 — `finalize_run(...)` — Plan 16-03 has already added the new positional argument in this PR (this plan only validates compile).
- api.rs:82 — `queries::insert_running_run(&state.pool, job_id, "manual").await` — `job: DbJob` fetched at L66.
- api.rs:131-140 — `queries::finalize_run(...)` error fallback — passes `None` (run never started a container).

Test callers (RESEARCH Pitfall 2):
- src/scheduler/run.rs:794 — `run_job_with_existing_run_id_skips_insert` test pre-inserts a row.
- src/db/queries.rs tests at L1833, L1874, L1923, L1983.
</interfaces>
</context>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Production caller -> queries::finalize_run / insert_running_run | New positional argument plumbed through trusted paths only: config_hash from DbJob (config-load path); image_digest from bollard inspect_container or None at error-fallback. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-16-04b-01 | Tampering | api.rs:131 error-fallback finalize_run with image_digest=None | accept | Pass None explicitly; this caller fires when scheduler channel is closed (no docker run happened); image_digest=None is semantically correct. |
| T-16-04b-02 | Information Disclosure | justfile recipe queries dev SQLite DB and prints container_id + image_digest | accept | Recipe targets the local dev `cronduit.db`; values are operator-internal (not secrets). HUMAN-UAT runs locally; no remote exposure. |

Severity: low. The wiring updates do not introduce new attack surface beyond what was assessed in 16-04a.
</threat_model>

<tasks>

<task id="16-04b-T1" type="auto">
  <name>Task 1: Update production caller src/scheduler/run.rs:83 (insert_running_run, scheduler-driven path)</name>
  <files>src/scheduler/run.rs</files>
  <read_first>
    - src/scheduler/run.rs lines 75-90 (the run_job entry; verify L83 is the insert_running_run call)
    - 16-RESEARCH.md section B "insert_running_run caller location for config_hash plumbing" (CRITICAL DRIFT correction -- fire.rs is NOT a caller; run.rs:83 IS)
    - 16-PATTERNS.md section "db/queries.rs::insert_running_run signature change (Plan 16-04) -> Production callers needing update"
  </read_first>
  <action>
At line 83 of src/scheduler/run.rs, locate the call:
```rust
let run_id = match insert_running_run(&pool, job.id, &trigger).await {
```

Update to pass `&job.config_hash` as the new last argument:
```rust
let run_id = match insert_running_run(&pool, job.id, &trigger, &job.config_hash).await {
```

`DbJob.config_hash: String` is in scope at this site (the `job: DbJob` parameter; `DbJob` is defined in queries.rs and was already populated at config-load time via `compute_config_hash` -- RESEARCH §B confirmed). No recomputation needed; pass the existing value by reference.
  </action>
  <verify>
    <automated>grep -q 'insert_running_run(&pool, job.id, &trigger, &job.config_hash)' src/scheduler/run.rs</automated>
  </verify>
  <acceptance_criteria>
    - grep -q 'insert_running_run(&pool, job.id, &trigger, &job.config_hash)' src/scheduler/run.rs returns 0.
    - The call no longer matches the old 3-argument shape `insert_running_run(&pool, job.id, &trigger)\b` (use `grep -E 'insert_running_run\([^)]*&trigger\)\.await' src/scheduler/run.rs` and ensure 0 matches in production code).
  </acceptance_criteria>
  <done>run.rs:83 caller updated to pass &job.config_hash.</done>
</task>

<task id="16-04b-T2" type="auto">
  <name>Task 2: Update production caller src/web/handlers/api.rs:82 (insert_running_run, Run Now path) and api.rs:131 (finalize_run error fallback)</name>
  <files>src/web/handlers/api.rs</files>
  <read_first>
    - src/web/handlers/api.rs lines 60-148 (the run_now handler -- both call sites)
    - 16-RESEARCH.md Pitfall 1 (the second finalize_run caller at L131)
    - 16-PATTERNS.md section "src/web/handlers/api.rs:131-140 (MODIFY) -- error fallback" (verbatim diff)
  </read_first>
  <action>
Step 1 -- update the insert_running_run call at api.rs:82. The handler fetches `job: DbJob` at L66 via `get_job_by_id`; that local is in scope. Update from:

```rust
queries::insert_running_run(&state.pool, job_id, "manual").await
```

To:

```rust
queries::insert_running_run(&state.pool, job_id, "manual", &job.config_hash).await
```

Step 2 -- update the finalize_run error fallback at api.rs:131-140. Currently:

```rust
let _ = queries::finalize_run(
    &state.pool,
    run_id,
    "error",
    None,
    tokio::time::Instant::now(),
    Some("scheduler shutting down"),
    None,
)
.await;
```

Append `None` as the new last positional argument (the run never started a container, so image_digest is rightly None):

```rust
let _ = queries::finalize_run(
    &state.pool,
    run_id,
    "error",
    None,
    tokio::time::Instant::now(),
    Some("scheduler shutting down"),
    None,
    None,   // Phase 16 FOUND-14: image_digest -- error fallback never started a container
)
.await;
```
  </action>
  <verify>
    <automated>grep -q 'insert_running_run(&state.pool, job_id, "manual", &job.config_hash)' src/web/handlers/api.rs &amp;&amp; grep -A 12 'queries::finalize_run' src/web/handlers/api.rs | grep -q 'Phase 16 FOUND-14'</automated>
  </verify>
  <acceptance_criteria>
    - grep -q 'insert_running_run(&state.pool, job_id, "manual", &job.config_hash)' src/web/handlers/api.rs returns 0.
    - The api.rs error-fallback finalize_run invocation has TWO `None` arguments at the end (the original container_id None plus the new image_digest None).
    - The new image_digest None argument carries a `// Phase 16 FOUND-14` trailing comment.
  </acceptance_criteria>
  <done>Both api.rs callers (insert_running_run at L82, finalize_run at L131) updated.</done>
</task>

<task id="16-04b-T3" type="auto">
  <name>Task 3: Update test-side callers of insert_running_run in src/</name>
  <files>src/scheduler/run.rs, src/db/queries.rs</files>
  <read_first>
    - 16-RESEARCH.md Pitfall 2 (test caller locations: src/scheduler/run.rs:794 + src/db/queries.rs L1833, L1874, L1923, L1983)
    - 16-RESEARCH.md "Open Questions (RESOLVED)" Q3 (resolution: literal "testhash" matches existing convention from queries.rs:579 upsert_job test fixture pattern)
    - src/scheduler/run.rs lines 790-800 (verify the test call site)
    - src/db/queries.rs lines 1820-1990 (verify the four test sites in mod tests)
  </read_first>
  <action>
For every test-call of `insert_running_run` in `src/` (under `mod tests` blocks), append `"testhash"` as the new last argument. Per RESEARCH Open Questions (RESOLVED) Q3, use the literal string `"testhash"` to match the existing convention from queries.rs:579 (the `upsert_job` test fixture pattern).

Files + sites:
- src/scheduler/run.rs:794 -- one site
- src/db/queries.rs:1833, 1874, 1923, 1983 -- four sites

Search-and-update pattern: every `insert_running_run(<pool_arg>, <job_id_arg>, <trigger_arg>).await` becomes `insert_running_run(<pool_arg>, <job_id_arg>, <trigger_arg>, "testhash").await`.

Tip: use `grep -n 'insert_running_run' src/scheduler/run.rs src/db/queries.rs` to enumerate every site, then verify each is updated. Production sites (run.rs:83, api.rs:82) were updated in T1/T2 -- they pass `&job.config_hash`, NOT `"testhash"`. Only test-mod sites get `"testhash"`.
  </action>
  <verify>
    <automated>grep -c 'insert_running_run.*"testhash"' src/db/queries.rs src/scheduler/run.rs | awk -F: '{s+=$2} END {exit !(s>=5)}' &amp;&amp; ! grep -E 'insert_running_run\([^)]*"manual"\)\.await' src/web/handlers/api.rs &amp;&amp; ! grep -E 'insert_running_run\([^)]*&trigger\)\.await' src/scheduler/run.rs</automated>
  </verify>
  <acceptance_criteria>
    - At least 5 occurrences of `insert_running_run(...,"testhash")` across src/scheduler/run.rs and src/db/queries.rs (1 in run.rs + 4 in queries.rs).
    - Zero remaining old-shape calls (no `insert_running_run(<pool>, <id>, <trigger>)` with only 3 arguments anywhere in src/).
  </acceptance_criteria>
  <done>All 5 test-mod insert_running_run sites updated with "testhash" literal.</done>
</task>

<task id="16-04b-T4" type="auto">
  <name>Task 4: Add justfile recipe `uat-fctx-bugfix-spot-check` for the HUMAN-UAT entry</name>
  <files>justfile</files>
  <read_first>
    - justfile (read all -- look at existing test/dev recipes for shape)
    - 16-RESEARCH.md section "H. just recipe inventory relevant to P16" (existing recipes)
    - 16-CONTEXT.md "Whether 16-HUMAN-UAT.md is needed" (Claude's discretion: yes, one spot check entry)
    - CLAUDE.md / MEMORY.md feedback_uat_use_just_commands (every UAT step MUST reference an existing just recipe)
  </read_first>
  <action>
Add a new recipe `uat-fctx-bugfix-spot-check` to the justfile. This recipe produces the operator-observable validation for FOUND-14 Success Criterion 1: query the most recent docker run from the dev SQLite DB and print the container_id; the maintainer visually verifies it does NOT start with `sha256:`.

Add the recipe near the existing `db-reset` and `migrate` recipes:

```makefile
# Phase 16 FOUND-14 spot check: print the container_id of the most recent
# job_runs row from the dev SQLite DB. The maintainer verifies this is a
# real Docker container ID (12-char hex prefix) and NOT a sha256:... digest
# (which would indicate the v1.0/v1.1 bug regressed).
uat-fctx-bugfix-spot-check:
    @echo "Phase 16 / FOUND-14 spot check"
    @echo "Most recent job_run container_id (must NOT start with 'sha256:'):"
    @sqlite3 cronduit.db "SELECT id, job_id, status, container_id, image_digest FROM job_runs ORDER BY id DESC LIMIT 1;"
    @echo ""
    @echo "Expected: container_id is a 64-char hex Docker container ID (or NULL for non-docker runs)."
    @echo "FAIL if: container_id starts with 'sha256:' (would indicate the bug regressed)."
```

The recipe assumes `cronduit.db` exists in the working dir from a prior `just dev` run -- this matches the existing convention (see `just db-reset` which also targets `cronduit.db`).
  </action>
  <verify>
    <automated>grep -q '^uat-fctx-bugfix-spot-check:' justfile &amp;&amp; just --list 2>&amp;1 | grep -q 'uat-fctx-bugfix-spot-check'</automated>
  </verify>
  <acceptance_criteria>
    - justfile contains a recipe named `uat-fctx-bugfix-spot-check`.
    - `just --list` shows the new recipe in its output.
    - The recipe header comment references Phase 16 + FOUND-14.
    - The recipe queries `job_runs` for the most recent row and prints both `container_id` and `image_digest`.
  </acceptance_criteria>
  <done>Justfile recipe added for the HUMAN-UAT spot check; available via `just uat-fctx-bugfix-spot-check`.</done>
</task>

<task id="16-04b-T5" type="auto">
  <name>Task 5: Wave-end gate -- verify full build + test suite is green</name>
  <files></files>
  <read_first>
    - 16-RESEARCH.md section "Validation Architecture -> Per task commit" (cargo test for individual files)
    - 16-RESEARCH.md section "H. just recipe inventory" (just nextest, just clippy, just fmt-check, just schema-diff, just grep-no-percentile-cont)
    - justfile (verify recipes exist as expected)
  </read_first>
  <action>
Run the full local CI gate to confirm Plans 16-01..16-04a..16-04b compose into a green codebase. Order matters -- run from cheapest to most expensive:

1. `cargo build` -- if this fails, the most likely cause is a missed test-mod caller from T3 or a missed SELECT site from 16-04a. Use `cargo build 2>&1 | grep error | head -20` to triage.
2. `just fmt-check` -- formatting compliance (CI gate).
3. `just clippy` -- lint compliance (CI gate; -D warnings).
4. `cargo test --lib` -- unit tests pass (most lib tests do not touch DB, so they should pass quickly).
5. `cargo test --test v12_fctx_config_hash_backfill` -- Plan 16-01's test (already validated there, but re-running confirms no regression).
6. `cargo test --test v12_run_rs_277_bug_fix command_run_leaves_image_digest_null` -- Plan 16-03's non-ignored test (must compile and pass after the queries.rs signature change in 16-04a + the call-site updates here).
7. `just schema-diff` -- structural parity invariant (Plan 16-01's test must stay green).
8. `just grep-no-percentile-cont` -- D-15 compliance gate.
9. `just nextest` -- the full CI suite (final gate before phase verification).

If `just clippy` fails, the most likely cause is a `dead_code` warning on the new struct fields (DbRun.image_digest, DbRun.config_hash) if the web layer is not yet consuming them. Mitigation: the existing template renderers in src/web/templates/run_detail.askama may still need to reference these fields for clippy to be happy; if so, consume them with a placeholder pattern (e.g., format!("{:?}", run.image_digest) in a debug log). RESEARCH §C.2 confirms templates compile cleanly; this is a defensive note.
  </action>
  <verify>
    <automated>cargo build &amp;&amp; just fmt-check &amp;&amp; just clippy &amp;&amp; cargo test --lib &amp;&amp; just schema-diff &amp;&amp; just grep-no-percentile-cont</automated>
  </verify>
  <acceptance_criteria>
    - `cargo build` exits 0.
    - `just fmt-check` exits 0.
    - `just clippy` exits 0 (no warnings under -D warnings).
    - `cargo test --lib` exits 0.
    - `just schema-diff` exits 0 (Plan 16-01's parity test stays green).
    - `just grep-no-percentile-cont` exits 0 (D-15 compliance).
    - `cargo test --test v12_fctx_config_hash_backfill` exits 0.
    - `cargo test --test v12_run_rs_277_bug_fix command_run_leaves_image_digest_null` exits 0 (the non-ignored test from Plan 16-03 now compiles and passes).
  </acceptance_criteria>
  <done>All compile/lint/format/parity gates green; PR 1 (Plans 16-01..16-04b) is mergeable.</done>
</task>

</tasks>

<verification_criteria>
- 4 production callers updated: run.rs:83, run.rs:348-356 (Plan 16-03 already did this; verify compile), api.rs:82, api.rs:131-140.
- 5 test-mod callers updated in src/.
- justfile has the new uat-fctx-bugfix-spot-check recipe.
- Full local CI gate (build + clippy + fmt-check + lib tests + schema-diff + grep-no-percentile-cont + integration tests from 16-01/16-03) is green.
</verification_criteria>

<success_criteria>
After Plan 16-04b lands (completing PR 1 -- Plans 16-01 through 16-04b):
1. v1.2 docker runs persist real Docker container IDs to job_runs.container_id (FOUND-14 Success Criterion 1 -- operator-observable via `just uat-fctx-bugfix-spot-check`).
2. v1.2 runs persist per-run config_hash from the in-memory Config at fire time (FCTX-04 Success Criterion 2 -- distinct hashes across reload-mid-fire scenarios; tested in Plan 16-05).
3. v1.2 docker runs persist sha256: image digests to job_runs.image_digest (FOUND-14 second observable).
4. The schema-parity invariant remains green; the migration-idempotency invariant remains green.
5. PR 1 is reviewable as one coherent commit set: schema substrate + bug fix + signature transition + call-site wiring.
</success_criteria>

<output>
After completion, create `.planning/phases/16-failure-context-schema-run-rs-277-bug-fix/16-04b-SUMMARY.md` documenting:
- The 4 production caller updates and the 5 test-mod caller updates.
- The new `uat-fctx-bugfix-spot-check` justfile recipe.
- Confirmation that the full local CI gate is green.
- Cross-reference to PR 1 review readiness (Plans 16-01..16-04b cohesive).
</output>
