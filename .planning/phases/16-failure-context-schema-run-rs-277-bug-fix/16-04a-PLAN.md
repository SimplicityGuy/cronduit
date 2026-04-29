---
id: 16-04a
phase: 16
plan: 04a
type: execute
wave: 2
depends_on: ["16-01", "16-02", "16-03"]
autonomous: true
requirements_addressed: [FOUND-14, FCTX-04]
files_modified:
  - src/db/queries.rs
must_haves:
  truths:
    - "queries::finalize_run signature gains image_digest: Option<&str> as the 8th positional parameter; both SQLite and Postgres UPDATE statements bind the new column."
    - "queries::insert_running_run signature gains config_hash: &str as a new parameter; both SQLite and Postgres INSERT statements bind the new column."
    - "DbRun and DbRunDetail structs gain pub image_digest: Option<String> and pub config_hash: Option<String> fields; every SELECT site that hydrates a run row includes both columns."
  artifacts:
    - path: "src/db/queries.rs"
      provides: "finalize_run + insert_running_run signature + INSERT/UPDATE column-list extension; DbRun/DbRunDetail field-add; SELECT-site column-list extension across get_run_history (SQLite + Postgres) + get_run_by_id (SQLite + Postgres)"
      contains: "image_digest: Option<&str>"
  key_links:
    - from: "queries::finalize_run signature"
      to: "queries::DbRun.image_digest hydration"
      via: "image_digest column flows from UPDATE bind at finalize time -> SELECT-side hydration into DbRun.image_digest"
      pattern: "image_digest"
    - from: "queries::insert_running_run signature"
      to: "queries::DbRun.config_hash hydration"
      via: "config_hash column flows from INSERT bind at fire time -> SELECT-side hydration into DbRun.config_hash"
      pattern: "config_hash"
---

<objective>
Land the queries.rs signature changes that wire Plan 16-01's new schema columns and Plan 16-02/03's new struct field through the database tier (queries.rs only — production callers + recipe + gate are owned by sibling Plan 16-04b). Specifically: extend `finalize_run` with `image_digest: Option<&str>` (8th positional), extend `insert_running_run` with `config_hash: &str`, add `image_digest` and `config_hash` fields to `DbRun` and `DbRunDetail`, and update every SELECT site that hydrates a run row.

Purpose: Plan 16-04a is the single-file load-bearing seam where the schema (Plan 16-01), the struct field (Plan 16-02), and the bug fix (Plan 16-03) all converge into actual DB writes via signatures + struct widening. After this plan lands, queries.rs accepts the new arguments and exposes the new fields; Plan 16-04b then updates the four production callers + 5 test callers + adds the just recipe + runs the wave-end gate.

Output: One MODIFIED source file (`src/db/queries.rs`). T-V12-FCTX-03 / T-V12-FCTX-04 write-site assertions are still deferred to Plan 16-05's tests/v12_fctx_streak.rs (per CONTEXT.md note in `<output_files>`: "the write-site tests for FCTX-04 land in Plan 16-05 because the test infrastructure is the same as the streak tests").

Note: Plan 16-04a was split out of the original Plan 16-04 to honor the per-plan task-count cap (5 tasks ≤ blocker threshold). 16-04a + 16-04b together cover the full original 16-04 scope; both ride Wave 2 together. The compile failure noted in 16-03 still resolves within Wave 2 once both 16-04a and 16-04b land (16-04a fixes the queries.rs signature; 16-04b updates the call sites that 16-03 left referencing the new positional argument).
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
@src/db/queries.rs

<interfaces>
Current `finalize_run` signature (queries.rs L424-432):
```rust
pub async fn finalize_run(
    pool: &DbPool,
    run_id: i64,
    status: &str,
    exit_code: Option<i32>,
    start_instant: tokio::time::Instant,
    error_message: Option<&str>,
    container_id: Option<&str>,
) -> anyhow::Result<()>
```

Current `insert_running_run` signature (queries.rs L368):
```rust
pub async fn insert_running_run(pool: &DbPool, job_id: i64, trigger: &str) -> anyhow::Result<i64>
```

Current `DbRun` struct (queries.rs L552-567): 10 fields, no image_digest/config_hash.
Current `DbRunDetail` struct (queries.rs L569-584): 11 fields, no image_digest/config_hash.

SELECT sites that hydrate run rows (RESEARCH §F enumeration):
- get_run_history SQLite SELECT @ L1059-L1066, hydration @ L1070-L1082
- get_run_history Postgres SELECT @ L1093-L1100, hydration @ L1104-L1115
- get_run_by_id SQLite SQL literal @ L1125-L1131, Postgres @ L1132-L1138, hydration SQLite @ L1146-L1158, Postgres @ L1165-L1177
</interfaces>
</context>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| Caller -> queries::finalize_run / insert_running_run | New parameter added; existing parameterization (sqlx .bind()) prevents SQL injection. config_hash from DbJob (config-load path); image_digest from bollard inspect_container (Docker daemon under operator control) -- neither is operator-untrusted. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-16-04a-01 | Tampering | New .bind(image_digest) and .bind(config_hash) sites | accept | All bind sites use sqlx parameterization (?N for SQLite, $N for Postgres). No string-concat into SQL; values flow through bind() exclusively. SQL is static literal in the function body. |
| T-16-04a-02 | DoS | Adding two columns to SELECT projection (no index changes) | accept | image_digest + config_hash are short TEXT (sha256: 71 chars; SHA-256 hex 64 chars); negligible per-row size growth. No index changes; existing idx_job_runs_job_id_start covers the relevant query patterns. |
| T-16-04a-03 | Information Disclosure | DbRun / DbRunDetail widening exposes new fields to web templates | accept | The web layer was not consuming these fields previously; templates that don't reference them remain unchanged. Phase 21 is the deliberate UI consumer; v1.2 release notes (Phase 24) will document the new operator-visible columns. |

Severity: low. SQL parameterization is robust; no new attack surface beyond the schema columns themselves (already covered by 16-01's threat model).
</threat_model>

<tasks>

<task id="16-04a-T1" type="auto">
  <name>Task 1: Extend finalize_run signature with image_digest: Option<&str>; update both backend UPDATE statements</name>
  <files>src/db/queries.rs</files>
  <read_first>
    - src/db/queries.rs lines 423-468 (the entire finalize_run function body; both SQLite arm L437-L450 and Postgres arm L451-L466)
    - 16-PATTERNS.md section "db/queries.rs::finalize_run signature change (Plan 16-04)" (verbatim before/after diff)
    - 16-RESEARCH.md section "Code Examples -> finalize_run signature extension" (the verified VERIFIED-tagged signature template)
  </read_first>
  <action>
Modify `pub async fn finalize_run` in src/db/queries.rs starting at L424.

Step 1 -- update the signature: append `image_digest: Option<&str>` after `container_id: Option<&str>`:

```rust
pub async fn finalize_run(
    pool: &DbPool,
    run_id: i64,
    status: &str,
    exit_code: Option<i32>,
    start_instant: tokio::time::Instant,
    error_message: Option<&str>,
    container_id: Option<&str>,
    image_digest: Option<&str>,   // Phase 16 FOUND-14
) -> anyhow::Result<()>
```

Step 2 -- update the SQLite UPDATE (L439-L440). Before:
```rust
"UPDATE job_runs SET status = ?1, exit_code = ?2, end_time = ?3, duration_ms = ?4, error_message = ?5, container_id = ?6 WHERE id = ?7"
```
After (add image_digest = ?7; bump WHERE to ?8):
```rust
"UPDATE job_runs SET status = ?1, exit_code = ?2, end_time = ?3, duration_ms = ?4, error_message = ?5, container_id = ?6, image_digest = ?7 WHERE id = ?8"
```

Step 3 -- update the SQLite bind chain (L442-L449). Insert `.bind(image_digest)` BETWEEN `.bind(container_id)` and `.bind(run_id)`:

```rust
.bind(status)
.bind(exit_code)
.bind(&now)
.bind(duration_ms)
.bind(error_message)
.bind(container_id)
.bind(image_digest)        // Phase 16 FOUND-14: NEW bind, position ?7
.bind(run_id)
```

Step 4 -- mirror the same change in the Postgres arm. UPDATE statement at L453-L454:
```rust
"UPDATE job_runs SET status = $1, exit_code = $2, end_time = $3, duration_ms = $4, error_message = $5, container_id = $6, image_digest = $7 WHERE id = $8"
```

Postgres bind chain: insert `.bind(image_digest)` between `.bind(container_id)` and `.bind(run_id)` (same shape as SQLite).

Step 5 -- update the doc comment above the function (currently L423: "/// Finalize a job run by updating its status, exit_code, end_time, duration_ms, error_message, and container_id."). Add `, image_digest` to the field list. Final:

```rust
/// Finalize a job run by updating its status, exit_code, end_time, duration_ms, error_message, container_id, and image_digest.
/// Phase 16 FOUND-14: image_digest captured from `inspect_container` post-start; NULL for command/script jobs.
```
  </action>
  <verify>
    <automated>grep -q 'image_digest: Option<&str>' src/db/queries.rs &amp;&amp; grep -q 'image_digest = ?7' src/db/queries.rs &amp;&amp; grep -q 'image_digest = \$7' src/db/queries.rs &amp;&amp; grep -c '\.bind(image_digest)' src/db/queries.rs | xargs -I{} test {} -ge 2</automated>
  </verify>
  <acceptance_criteria>
    - grep -q 'image_digest: Option<&str>' src/db/queries.rs returns 0 (signature updated).
    - grep -q 'image_digest = ?7' src/db/queries.rs returns 0 (SQLite UPDATE statement updated).
    - grep -q 'image_digest = \$7' src/db/queries.rs returns 0 (Postgres UPDATE statement updated).
    - grep -c '\.bind(image_digest)' src/db/queries.rs returns at least 2 (both backend arms have the new bind).
    - The WHERE clause placeholder in finalize_run is now ?8 (SQLite) / $8 (Postgres) -- verify with grep 'WHERE id = ?8' and 'WHERE id = $8'.
  </acceptance_criteria>
  <done>finalize_run signature, SQLite UPDATE, Postgres UPDATE, and bind chains all updated. Doc comment references Phase 16 FOUND-14.</done>
</task>

<task id="16-04a-T2" type="auto">
  <name>Task 2: Extend insert_running_run signature with config_hash: &str; update both backend INSERT statements</name>
  <files>src/db/queries.rs</files>
  <read_first>
    - src/db/queries.rs lines 368-421 (the entire insert_running_run function body; both backend arms)
    - 16-PATTERNS.md section "db/queries.rs::insert_running_run signature change (Plan 16-04)" (the verbatim diff)
    - 16-CONTEXT.md FCTX-04 (rationale: config_hash captured at fire time so reload-mid-fire reflects the run's actual config)
  </read_first>
  <action>
Modify `pub async fn insert_running_run` in src/db/queries.rs at L368.

Step 1 -- update the signature: append `config_hash: &str`:

```rust
pub async fn insert_running_run(pool: &DbPool, job_id: i64, trigger: &str, config_hash: &str) -> anyhow::Result<i64>
```

Step 2 -- update the SQLite INSERT statement (currently around L380). Before:
```rust
"INSERT INTO job_runs (job_id, status, trigger, start_time, job_run_number) VALUES (?1, 'running', ?2, ?3, ?4) RETURNING id"
```
After (add config_hash to column list and ?5 to VALUES):
```rust
"INSERT INTO job_runs (job_id, status, trigger, start_time, job_run_number, config_hash) VALUES (?1, 'running', ?2, ?3, ?4, ?5) RETURNING id"
```

Step 3 -- update the SQLite bind chain. Add `.bind(config_hash)` AFTER `.bind(reserved)`:

```rust
.bind(job_id)
.bind(trigger)
.bind(&now)
.bind(reserved)
.bind(config_hash)   // Phase 16 FCTX-04: NEW bind, position ?5
```

Step 4 -- mirror the same in the Postgres arm. INSERT statement column list and VALUES list extended; bind chain gets `.bind(config_hash)` after the `reserved` bind.

Step 5 -- update doc comment to mention config_hash + FCTX-04 reference.
  </action>
  <verify>
    <automated>grep -q 'config_hash: &str' src/db/queries.rs &amp;&amp; grep -q 'job_run_number, config_hash' src/db/queries.rs &amp;&amp; grep -c '\.bind(config_hash)' src/db/queries.rs | xargs -I{} test {} -ge 2</automated>
  </verify>
  <acceptance_criteria>
    - grep -q 'config_hash: &str' src/db/queries.rs returns 0 (signature updated).
    - grep -q 'job_run_number, config_hash' src/db/queries.rs returns 0 (column list extended on at least one INSERT statement).
    - grep -c '\.bind(config_hash)' src/db/queries.rs returns at least 2 (both backends bind the new value -- this counts insert_running_run sites; later tasks add more bind sites for SELECT hydration is via .get not .bind, so 2 is the minimum).
    - The function doc comment references Phase 16 FCTX-04.
  </acceptance_criteria>
  <done>insert_running_run signature, SQLite + Postgres INSERT statements, and bind chains all updated.</done>
</task>

<task id="16-04a-T3" type="auto">
  <name>Task 3: Add image_digest and config_hash fields to DbRun and DbRunDetail structs</name>
  <files>src/db/queries.rs</files>
  <read_first>
    - src/db/queries.rs lines 552-584 (both struct definitions)
    - 16-PATTERNS.md section "DbRun / DbRunDetail field-add (Plan 16-04)" (concrete diff)
  </read_first>
  <action>
Append two fields to `DbRun` (around L567, after `error_message: Option<String>`):

```rust
pub struct DbRun {
    // ... existing fields ...
    pub error_message: Option<String>,
    /// Phase 16 FOUND-14: image digest from post-start `inspect_container`. NULL for
    /// command/script jobs (no image), pre-v1.2 docker rows (capture site landed in v1.2).
    pub image_digest: Option<String>,
    /// Phase 16 FCTX-04: per-run config_hash captured at fire time by
    /// `insert_running_run`. NULL for pre-v1.2 rows whose backfill found no matching
    /// `jobs.config_hash`. See migration `*_000007_config_hash_backfill.up.sql` for
    /// the BACKFILL_CUTOFF_RFC3339 marker (D-03).
    pub config_hash: Option<String>,
}
```

Apply the same two-field append to `DbRunDetail` (around L584, after `error_message: Option<String>`). Doc comments are identical to DbRun's.
  </action>
  <verify>
    <automated>grep -A 30 'pub struct DbRun {' src/db/queries.rs | grep -q 'pub image_digest: Option<String>' &amp;&amp; grep -A 30 'pub struct DbRun {' src/db/queries.rs | grep -q 'pub config_hash: Option<String>' &amp;&amp; grep -A 30 'pub struct DbRunDetail {' src/db/queries.rs | grep -q 'pub image_digest: Option<String>' &amp;&amp; grep -A 30 'pub struct DbRunDetail {' src/db/queries.rs | grep -q 'pub config_hash: Option<String>'</automated>
  </verify>
  <acceptance_criteria>
    - DbRun struct contains `pub image_digest: Option<String>` and `pub config_hash: Option<String>` fields.
    - DbRunDetail struct contains `pub image_digest: Option<String>` and `pub config_hash: Option<String>` fields.
    - Both fields on each struct carry doc comments referencing Phase 16 FOUND-14 / FCTX-04.
  </acceptance_criteria>
  <done>Both structs widened with two new Option<String> fields and Phase 16 doc comments.</done>
</task>

<task id="16-04a-T4" type="auto">
  <name>Task 4: Update get_run_history SELECT statements + hydration on both backends</name>
  <files>src/db/queries.rs</files>
  <read_first>
    - src/db/queries.rs lines 1051-1115 (full get_run_history function: SQLite arm L1059-L1082, Postgres arm L1093-L1115)
    - 16-PATTERNS.md section "get_run_history and get_run_by_id SELECT-list extension (Plan 16-04)" (line-by-line action table + concrete excerpt)
    - 16-RESEARCH.md section F (exhaustive enumeration of SELECT sites)
  </read_first>
  <action>
Step 1 -- SQLite SELECT (L1059-L1066). Append `, image_digest, config_hash` to the column list:

Before:
```rust
"SELECT id, job_id, job_run_number, status, trigger, start_time, end_time, duration_ms, exit_code, error_message FROM job_runs WHERE job_id = ?1 ORDER BY start_time DESC LIMIT ?2 OFFSET ?3"
```
After:
```rust
"SELECT id, job_id, job_run_number, status, trigger, start_time, end_time, duration_ms, exit_code, error_message, image_digest, config_hash FROM job_runs WHERE job_id = ?1 ORDER BY start_time DESC LIMIT ?2 OFFSET ?3"
```

Step 2 -- SQLite hydration (L1070-L1082). Append two `.get(...)` calls:
```rust
.map(|r| DbRun {
    id: r.get("id"),
    job_id: r.get("job_id"),
    job_run_number: r.get("job_run_number"),
    status: r.get("status"),
    trigger: r.get("trigger"),
    start_time: r.get("start_time"),
    end_time: r.get("end_time"),
    duration_ms: r.get("duration_ms"),
    exit_code: r.get("exit_code"),
    error_message: r.get("error_message"),
    image_digest: r.get("image_digest"),   // Phase 16 FOUND-14
    config_hash: r.get("config_hash"),     // Phase 16 FCTX-04
})
```

Step 3 -- Postgres SELECT (L1093-L1100). Same column list extension as SQLite (no `r.` prefix needed; this is the unprefixed projection).

Step 4 -- Postgres hydration (L1104-L1115). Same two `.get(...)` lines appended.
  </action>
  <verify>
    <automated>grep -c 'image_digest, config_hash FROM job_runs' src/db/queries.rs | xargs -I{} test {} -ge 2 &amp;&amp; grep -c 'image_digest: r.get("image_digest")' src/db/queries.rs | xargs -I{} test {} -ge 2 &amp;&amp; grep -c 'config_hash: r.get("config_hash")' src/db/queries.rs | xargs -I{} test {} -ge 2</automated>
  </verify>
  <acceptance_criteria>
    - SQLite get_run_history SELECT contains `, image_digest, config_hash` in the column list.
    - Postgres get_run_history SELECT contains the same.
    - Both backend hydration blocks contain `image_digest: r.get("image_digest")` and `config_hash: r.get("config_hash")` lines.
  </acceptance_criteria>
  <done>get_run_history SELECTs and hydrations updated on both backends.</done>
</task>

<task id="16-04a-T5" type="auto">
  <name>Task 5: Update get_run_by_id SELECT literals + hydration on both backends</name>
  <files>src/db/queries.rs</files>
  <read_first>
    - src/db/queries.rs lines 1124-1180 (full get_run_by_id function; sql_sqlite + sql_postgres raw-string locals + match arms)
    - 16-PATTERNS.md section "get_run_history and get_run_by_id SELECT-list extension (Plan 16-04)" (concrete excerpt)
  </read_first>
  <action>
Step 1 -- sql_sqlite raw-string literal (L1125-L1131). Append `, r.image_digest, r.config_hash` to the column list:

Before:
```rust
let sql_sqlite = r#"
    SELECT r.id, r.job_id, r.job_run_number, j.name AS job_name, r.status, r.trigger,
           r.start_time, r.end_time, r.duration_ms, r.exit_code, r.error_message
    FROM job_runs r
    JOIN jobs j ON j.id = r.job_id
    WHERE r.id = ?1
"#;
```
After:
```rust
let sql_sqlite = r#"
    SELECT r.id, r.job_id, r.job_run_number, j.name AS job_name, r.status, r.trigger,
           r.start_time, r.end_time, r.duration_ms, r.exit_code, r.error_message,
           r.image_digest, r.config_hash
    FROM job_runs r
    JOIN jobs j ON j.id = r.job_id
    WHERE r.id = ?1
"#;
```

Note the `r.` prefix because the SELECT JOINs `job_runs r` with `jobs j` -- without the prefix the parser would error on ambiguity (only an issue if `jobs` ever gets `image_digest`/`config_hash` columns, which it does not in v1.2).

Step 2 -- sql_postgres raw-string literal (L1132-L1138). Same column list extension; placeholder is `$1` not `?1`.

Step 3 -- SQLite hydration (L1146-L1158). Append two `r.get(...)` calls inside the `Some(DbRunDetail { ... })` block.

Step 4 -- Postgres hydration (L1165-L1177). Same two appended.
  </action>
  <verify>
    <automated>grep -A 8 'let sql_sqlite' src/db/queries.rs | grep -q 'r.image_digest, r.config_hash' &amp;&amp; grep -A 8 'let sql_postgres' src/db/queries.rs | grep -q 'r.image_digest, r.config_hash' &amp;&amp; grep -c 'image_digest: r.get("image_digest")' src/db/queries.rs | xargs -I{} test {} -ge 4</automated>
  </verify>
  <acceptance_criteria>
    - sql_sqlite raw-string literal in get_run_by_id contains `r.image_digest, r.config_hash`.
    - sql_postgres raw-string literal contains the same.
    - Both backend hydration blocks now contain the two new `r.get(...)` lines (combined with Task 4, total `image_digest: r.get("image_digest")` occurrences in queries.rs is at least 4: 2 from get_run_history + 2 from get_run_by_id).
  </acceptance_criteria>
  <done>get_run_by_id SQL literals and hydrations updated on both backends.</done>
</task>

</tasks>

<verification_criteria>
- finalize_run signature has 8 parameters; both backend UPDATE statements bind image_digest at position ?7/$7.
- insert_running_run signature has 4 parameters; both backend INSERT statements include config_hash in the column+VALUES list.
- DbRun and DbRunDetail each have two new Option<String> fields with Phase 16 doc comments.
- All 4 SELECT-site arms (get_run_history SQLite/Postgres + get_run_by_id SQLite/Postgres) include image_digest + config_hash in projection AND hydration.
- Compile failure (missing call-site updates) is EXPECTED at the close of this plan; Plan 16-04b in the same Wave 2 batch resolves it before the wave-end gate runs.
</verification_criteria>

<success_criteria>
After Plan 16-04a lands (alongside its sibling Plan 16-04b in PR 1):
1. queries.rs accepts the new positional parameters and exposes the new struct fields.
2. The schema-substrate (16-01) + struct (16-02) + bug fix (16-03) + DB-tier wiring (16-04a) form a coherent commit set.
3. Build greens once 16-04b updates the four production callers + 5 test-mod callers + adds the just recipe.
</success_criteria>

<output>
After completion, create `.planning/phases/16-failure-context-schema-run-rs-277-bug-fix/16-04a-SUMMARY.md` documenting:
- The signature changes to finalize_run and insert_running_run (before/after diffs).
- The DbRun / DbRunDetail field additions.
- The 4 SELECT-site updates (line numbers + brief diff narration).
- Note that compile is expected to fail at this plan's close until 16-04b lands; that's the intended Wave 2 batch behavior.
- Cross-reference to Plan 16-04b which lands the call-site updates and runs the wave-end gate.
</output>
