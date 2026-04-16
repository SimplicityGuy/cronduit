# Phase 11: Per-Job Run Numbers + Log UX Fixes - Pattern Map

**Mapped:** 2026-04-16
**Files analyzed:** 20 (new + modified)
**Analogs found:** 19 / 20 (one "first-of-its-kind" pattern flagged)

Planner-facing guidance: every new file in Phase 11 has a concrete analog already in this repo. The only greenfield pattern is the **three-file migration shape** (no prior multi-file migration exists — phase 10 went schema-less). The SQL content itself follows `migrations/sqlite/20260410_000000_initial.up.sql` verbatim for header comments, `IF NOT EXISTS`, and the pairing-with-postgres contract.

---

## File Classification

| New/Modified File | New? | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|------|-----------|----------------|---------------|
| `migrations/sqlite/20260416_000001_job_run_number_add.up.sql` | NEW | migration (schema) | DDL batch | `migrations/sqlite/20260410_000000_initial.up.sql` | header/style exact; structure greenfield |
| `migrations/sqlite/20260416_000002_job_run_number_backfill.up.sql` | NEW | migration (marker) | no-op placeholder | `migrations/sqlite/20260410_000000_initial.up.sql` | header/style exact; body greenfield |
| `migrations/sqlite/20260416_000003_job_run_number_not_null.up.sql` | NEW | migration (schema tighten) | DDL batch (12-step SQLite rewrite) | `migrations/sqlite/20260410_000000_initial.up.sql` | header/style exact; rewrite body greenfield |
| `migrations/postgres/20260416_000001_job_run_number_add.up.sql` | NEW | migration (schema) | DDL batch | `migrations/postgres/20260410_000000_initial.up.sql` | header/style exact |
| `migrations/postgres/20260416_000002_job_run_number_backfill.up.sql` | NEW | migration (marker) | no-op placeholder | `migrations/postgres/20260410_000000_initial.up.sql` | header/style exact |
| `migrations/postgres/20260416_000003_job_run_number_not_null.up.sql` | NEW | migration (schema tighten) | DDL (ALTER COLUMN ... SET NOT NULL) | `migrations/postgres/20260410_000000_initial.up.sql` | header/style exact |
| `src/db/migrate_backfill.rs` | NEW | utility / orchestrator | chunked UPDATE loop w/ progress log | `src/scheduler/retention.rs` | role-match (chunked DB batch + log) |
| `src/db/mod.rs` (`DbPool::migrate`) | CHANGED | model / pool | call-post-migrate | self (existing fn L97-108) | exact self-extension |
| `src/db/queries.rs::insert_log_batch` | CHANGED | model | batch INSERT + RETURNING id | `src/db/queries.rs::insert_running_run` (L286-313) | exact — same RETURNING id shape to copy |
| `src/db/queries.rs::insert_running_run` | CHANGED | model | two-statement tx (counter + insert) | `src/db/queries.rs::insert_running_run` (L286-313 body) + `src/db/queries.rs::insert_log_batch` (L374-405 tx pattern) | exact — same `begin() ... commit()` tx shape |
| `src/db/queries.rs::DbRun`, `DbRunDetail`, `get_run_history`, `get_run_by_id` | CHANGED | model | SELECT + view struct | `src/db/queries.rs::DbRun` (L428-438) | exact self-extension (add one field + select column) |
| `src/scheduler/log_pipeline.rs::LogLine` | CHANGED | model (DTO) | struct field add | self (L20-29) | exact self-extension |
| `src/scheduler/run.rs::log_writer_task` | CHANGED | service (async task) | broadcast + DB write | self (L344-374) | exact self-extension |
| `src/scheduler/run.rs::run_job_with_existing_run_id` (new) | NEW | service (async task) | per-run lifecycle | `src/scheduler/run.rs::run_job` (L65-316) | exact — sibling that skips step 1 insert |
| `src/scheduler/cmd.rs` (new `RunNowWithRunId` variant) | CHANGED | model (enum) | command enum variant add | `src/scheduler/cmd.rs::SchedulerCmd::Stop` (L33-37, L65-74) | exact — same enum variant shape |
| `src/scheduler/mod.rs` (select loop arm) | CHANGED | controller (scheduler loop) | arm on mpsc cmd | self — existing `RunNow` arm (L187-211) + `Stop` arm (L323-361) | exact — copy existing arm structure |
| `src/web/handlers/api.rs::run_now` | CHANGED | controller (axum handler) | sync insert → dispatch cmd → HX-Refresh | self (L26-80) + `stop_run` (L329-418) for response-shape | exact self-extension |
| `src/web/handlers/sse.rs::format_log_line_html` / SSE stream | CHANGED | controller (axum handler) | SSE frame emit | self (L41-69, L93-113) | exact self-extension |
| `src/web/handlers/run_detail.rs::run_detail` / `fetch_logs` | CHANGED | controller (axum handler) | DB fetch → template render | self (L99-196) + `static_log_partial` (L219-234) | exact self-extension |
| `templates/pages/run_detail.html` (title, breadcrumb, header, SSE script, `data-max-id`) | CHANGED | template (page) | display + HTMX wiring | self (L1-155) + Phase 10 D-03 Stop-button precedent (L18-26) | exact self-extension |
| `templates/partials/run_history.html` (new `#N` cell + tr title) | CHANGED | template (partial) | table row | self (L17-62); for the compact-button-lives-in-rightmost-cell precedent see same file L50-59 (Phase 10 D-04) | exact self-extension |
| `templates/partials/static_log_viewer.html` (`data-max-id` attr) | CHANGED | template (partial) | inline data attr | self (L9) | exact self-extension |
| `src/cli/run.rs` (post-migrate NULL-count assertion) | CHANGED | controller (bin entry) | assertion before spawn | self — existing `pool.migrate().await?` at L62-63 + scheduler spawn at L220-230 | exact self-extension |
| `tests/v11_run_number_backfill.rs` (or named per plan) | NEW | test (integration) | SQLite + Postgres migration validation | `tests/migrations_idempotent.rs`, `tests/schema_parity.rs`, `tests/db_pool_postgres.rs` | exact — same testcontainers + migrate() shape |
| `tests/v11_log_id_broadcast.rs` (or named per plan) | NEW | test (integration) | RETURNING id + broadcast assertion | `tests/stop_executors.rs` (full executor path) + `src/db/queries.rs` tests L1512-1517 (insert-order proof) | exact |
| `tests/v11_run_now_race.rs` (or named per plan) | NEW | test (integration) | axum handler → insert → dispatch → refresh | `tests/api_run_now.rs` (L1-157) | exact — same handler test shape |
| `tests/v11_sse_dedupe.rs` (or named per plan) | NEW | test (integration) | SSE stream + id emit + terminal event | `tests/sse_streaming.rs` scaffold + `tests/stop_executors.rs` for run_job driving | role-match (SSE file is ignore-gated placeholder; use as structure + supplement with `stop_executors.rs` driver pattern) |

---

## Pattern Assignments

### `migrations/sqlite/20260416_000001_job_run_number_add.up.sql` (migration, DDL)

**Analog:** `migrations/sqlite/20260410_000000_initial.up.sql`

**Header-comment pattern** (analog L1-14) — copy verbatim shape; include the "pairs with migrations/postgres/..." lock AND the `tests/schema_parity.rs must remain green` line:

```sql
-- Phase 11: per-job run numbering (DB-09, DB-10, DB-11) — SQLite.
--
-- Pairs with migrations/postgres/20260416_000001_job_run_number_add.up.sql.
-- Any structural change MUST land in both files in the same PR,
-- and tests/schema_parity.rs (Plan 05) MUST remain green.
--
-- Design notes:
--   * jobs.next_run_number: NOT NULL DEFAULT 1 at creation — no backfill needed.
--   * job_runs.job_run_number: nullable initially (file 2 backfills, file 3
--     tightens to NOT NULL). Required for in-place upgrades per DB-10.
```

**DDL shape** (analog L16-28 `CREATE TABLE IF NOT EXISTS`, L30 `CREATE INDEX IF NOT EXISTS`): Phase 11 uses `ALTER TABLE` (no CREATE); reuse the `IF NOT EXISTS`-style idempotency guard where SQLite supports it and otherwise guard via migration tracker (sqlx's `_sqlx_migrations` does this automatically — only one run of each file is applied).

---

### `migrations/postgres/20260416_000001_job_run_number_add.up.sql` (migration, DDL)

**Analog:** `migrations/postgres/20260410_000000_initial.up.sql`

**Header-comment pattern** (analog L1-5) — copy verbatim shape:

```sql
-- Phase 11: per-job run numbering (DB-09, DB-10, DB-11) — PostgreSQL.
--
-- Pairs with migrations/sqlite/20260416_000001_job_run_number_add.up.sql. Keep in sync.
-- tests/schema_parity.rs MUST remain green — both backends add identical column
-- shape/names/indexes (INT64 normalization covers BIGINT ↔ INTEGER per schema_parity).
```

**Type-parity rule** (analog L15-17 comment): PostgreSQL uses `BIGINT` where SQLite uses `INTEGER` — schema_parity normalizes both to `INT64` (see `tests/schema_parity.rs::normalize_type` L41: `"INTEGER" | "BIGINT" | "BIGSERIAL" | "INT8" => "INT64"`). Planner MUST use `BIGINT` (not `INTEGER`) on Postgres so sqlx decodes to `i64` matching SQLite.

---

### `src/db/migrate_backfill.rs` (utility, chunked UPDATE loop)

**Analog:** `src/scheduler/retention.rs`

**Imports pattern** (analog L1-8):

```rust
//! Phase 11 per-job run-number backfill (DB-09..DB-12).
//!
//! Loops 10k-row batches of `UPDATE job_runs SET job_run_number = ...`
//! with INFO progress logging (D-13). Fail-fast on error (D-14). Called
//! from DbPool::migrate() AFTER sqlx::migrate! applies file 1 and BEFORE
//! file 3 is applied (so job_run_number can still be NULL during the loop).

use crate::db::{DbPool, queries};
```

**Chunked-batch loop pattern** (analog L57-91 `delete_old_logs_batch` loop) — copy the `loop { ... }` shape:

```rust
let mut total_done: i64 = 0;
let mut batch_num: u64 = 0;
let start = tokio::time::Instant::now();
loop {
    match queries::backfill_job_run_number_batch(pool, BATCH_SIZE).await {
        Ok(rows) => {
            total_done += rows;
            batch_num += 1;
            // D-13: per-batch INFO log shape
            tracing::info!(
                target: "cronduit.migrate",
                batch = batch_num,
                rows_done = total_done,
                rows_total = total,
                pct = 100.0 * (total_done as f64) / (total.max(1) as f64),
                elapsed_ms = start.elapsed().as_millis() as u64,
                "job_run_number backfill"
            );
            if rows == 0 { break; }          // done or un-progressable
        }
        Err(e) => {
            // D-14: fail-fast, no retry — crash the process so the container
            // orchestrator restarts and the backfill resumes idempotently.
            return Err(anyhow::anyhow!(
                "backfill failed at batch={batch_num} done={total_done}: {e}"
            ));
        }
    }
}
```

**Log-target idiom** (analog L21-25, L49-53, L149-154): use `target: "cronduit.migrate"` per D-13 wording (analog uses `cronduit.retention` for its domain).

**Constants placement** (analog L10-12): `const BATCH_SIZE: i64 = 10_000;` at module top, not inline.

---

### `src/db/queries.rs::insert_log_batch` (model, batch INSERT + RETURNING id)

**Analog for RETURNING id:** `src/db/queries.rs::insert_running_run` (L286-313)
**Analog for tx shape:** `src/db/queries.rs::insert_log_batch` itself (L374-405)

**Copy-verbatim pattern — RETURNING id + fetch_one + row.get::<i64>** (analog L291-299 and L302-310):

```rust
// SQLite branch — uses ?1..?4 placeholders
let row = sqlx::query(
    "INSERT INTO job_logs (run_id, stream, ts, line) VALUES (?1, ?2, ?3, ?4) RETURNING id",
)
.bind(run_id)
.bind(stream)
.bind(ts)
.bind(line)
.fetch_one(&mut *tx)  // keep transaction — do NOT use pool directly
.await?;
let id: i64 = row.get::<i64, _>("id");
ids.push(id);

// Postgres branch — identical shape, $1..$4 placeholders
```

**Tx shape to preserve** (analog L374-405) — D-03 locks one `begin() ... commit()` per batch, NOT per line:

```rust
let mut tx = p.begin().await?;
for (stream, ts, line) in lines {
    // per-line INSERT ... RETURNING id ... fetch_one(&mut *tx)
    ids.push(row.get::<i64, _>("id"));
}
tx.commit().await?;
```

**Ordering invariant** (cited at 11-RESEARCH §1 and locked by the existing `src/db/queries.rs` test at L1512-1517): per-line INSERT inside a tx on both backends preserves input order for `INTEGER PRIMARY KEY` (SQLite rowid) / `BIGSERIAL` (Postgres). Planner MUST NOT reorder the returned ids — zip 1:1 with the input `lines` slice.

---

### `src/db/queries.rs::insert_running_run` (model, two-statement tx — counter + insert)

**Analog for tx shape:** `src/db/queries.rs::insert_log_batch` (L374-405)
**Analog for single INSERT RETURNING:** self (L286-313) — to be modified

**New shape** — two-statement tx, per DB-11 (no MAX+1 subquery):

```rust
// SQLite branch
let mut tx = p.begin().await?;
// Step 1: read-and-increment the counter atomically inside the tx.
let row = sqlx::query(
    "UPDATE jobs SET next_run_number = next_run_number + 1 \
     WHERE id = ?1 RETURNING next_run_number - 1 AS reserved"
)
.bind(job_id)
.fetch_one(&mut *tx).await?;
let reserved: i64 = row.get::<i64, _>("reserved");

// Step 2: insert the job_runs row using the reserved number.
let row = sqlx::query(
    "INSERT INTO job_runs (job_id, status, trigger, start_time, job_run_number) \
     VALUES (?1, 'running', ?2, ?3, ?4) RETURNING id"
)
.bind(job_id).bind(trigger).bind(&now).bind(reserved)
.fetch_one(&mut *tx).await?;
let id: i64 = row.get::<i64, _>("id");
tx.commit().await?;
```

Mirror for Postgres with `$1..$4`. Same guarantee as `insert_log_batch`: one fsync per run, counter write and run insert atomic.

---

### `src/scheduler/log_pipeline.rs::LogLine` (DTO, struct field add)

**Analog:** self (L20-29)

**Existing struct to extend:**

```rust
#[derive(Debug, Clone)]
pub struct LogLine {
    pub stream: String,
    pub ts: String,
    pub line: String,
    // D-01 / UI-20: populated by insert_log_batch AFTER RETURNING id lands.
    // None for pre-broadcast / transient lines (e.g. the [truncated N lines]
    // marker at L97-101 constructed before persistence).
    pub id: Option<i64>,
}
```

Update `make_log_line` at L178-184 and the `[truncated N lines]` marker at L97-101 to initialize `id: None`.

---

### `src/scheduler/run.rs::log_writer_task` (service, async task — broadcast after persist)

**Analog:** self (L344-374)

**Existing shape** (L350-373) — the pattern already drains a batch and then both broadcasts and inserts. Phase 11 inverts the order: **insert first (collecting ids), then broadcast** with the ids attached.

Key lines to adjust:
- L358-360 (current pre-broadcast loop): delete — moves below the insert.
- L361-364 (tuple build): keep unchanged.
- L365-372 (insert_log_batch match): change `?` / error-log arm to handle the new `Result<Vec<i64>>` return; on `Ok(ids)`, zip with the owned `batch: Vec<LogLine>` and broadcast `LogLine { id: Some(i), ..line }`.

```rust
// Phase 11 D-01 / Option A: broadcast AFTER persist so LogLine.id carries
// the job_logs.id that just landed in the DB.
let tuples: Vec<(String, String, String)> = batch
    .iter()
    .map(|l| (l.stream.clone(), l.ts.clone(), l.line.clone()))
    .collect();
match insert_log_batch(&pool, run_id, &tuples).await {
    Ok(ids) => {
        // Zip in input order — SQLite INTEGER PRIMARY KEY + Postgres BIGSERIAL
        // preserve insert order inside a single tx (see queries.rs:1512-1517 test).
        for (line, id) in batch.into_iter().zip(ids.into_iter()) {
            let _ = broadcast_tx.send(LogLine { id: Some(id), ..line });
        }
    }
    Err(e) => {
        tracing::error!(
            target: "cronduit.log_writer",
            run_id, error = %e,
            "failed to insert log batch"
        );
        // Broadcast nothing — subscribers never see unpersisted lines (D-01 lock).
    }
}
```

---

### `src/scheduler/cmd.rs` (enum — `RunNowWithRunId` variant)

**Analog:** `SchedulerCmd::Stop` (L33-37) + `StopResult` (L65-74)

**Existing variant to mirror:**

```rust
/// Dispatch a manual run whose job_runs row has ALREADY been inserted on
/// the API handler thread (UI-19 fix — eliminates the run-detail race).
/// The scheduler spawns run::run_job_with_existing_run_id(..) which skips
/// the step-1 insert and reuses every other stage of the lifecycle.
RunNowWithRunId { job_id: i64, run_id: i64 },
```

Keep the existing `RunNow { job_id }` variant — internal call-sites (if any) continue to work, and the enum-exhaustive arm in `scheduler/mod.rs` need only add one new arm.

**Doc-comment tone** (analog L23-32 Stop variant): explain the *why* not the *what*. Reference the UI-19 fix and the paired handler site.

---

### `src/scheduler/mod.rs` (select loop arm)

**Analog (arm structure):** existing `RunNow` arm at L187-211 + `Stop` arm at L323-361

**Pattern to copy (arm body shape)** from L187-211:

```rust
Some(cmd::SchedulerCmd::RunNowWithRunId { job_id, run_id }) => {
    if let Some(job) = self.jobs.get(&job_id) {
        let child_cancel = self.cancel.child_token();
        join_set.spawn(run::run_job_with_existing_run_id(
            self.pool.clone(),
            self.docker.clone(),
            job.clone(),
            run_id,
            child_cancel,
            self.active_runs.clone(),
        ));
        tracing::info!(
            target: "cronduit.scheduler",
            job_id, run_id, job_name = %job.name,
            "manual run with pre-inserted run_id dispatched"
        );
    } else {
        // Unknown job_id but row already exists — mark it errored to avoid
        // a zombie running row.
        tracing::warn!(
            target: "cronduit.scheduler",
            job_id, run_id,
            "RunNowWithRunId for unknown job — finalizing run as error"
        );
        // Planner: use finalize_run(..., "error", ...) here; see queries.rs L316.
    }
}
```

**Coalesce-drain parity** (analog L229-283): the reload-coalesce drain loop handles every `SchedulerCmd` variant to avoid dropping commands. Planner MUST add a `RunNowWithRunId` arm inside `while let Ok(queued) = self.cmd_rx.try_recv()` (analog L230-283) so a reload coinciding with a Run Now doesn't lose the new variant.

---

### `src/web/handlers/api.rs::run_now` (controller — sync insert → dispatch)

**Analog:** self (L26-80) + `stop_run` (L329-418) for response-shape

**Imports to add (already partially present):**
```rust
use crate::db::queries;                    // already L16
use crate::scheduler::cmd::SchedulerCmd;   // already L17 (extend usage)
```

**CSRF pattern** (analog L32-40) — copy verbatim:

```rust
let cookie_token = cookies
    .get(csrf::CSRF_COOKIE_NAME)
    .map(|c| c.value().to_string())
    .unwrap_or_default();
if !csrf::validate_csrf(&cookie_token, &form.csrf_token) {
    return (StatusCode::FORBIDDEN, "CSRF token mismatch").into_response();
}
```

**Job-existence lookup** (analog L43-47) — copy verbatim.

**NEW — sync insert before dispatch (UI-19 core fix):**

```rust
// Phase 11 UI-19: insert the running row on the handler thread so the
// navigation that follows HX-Refresh: true finds an existing row. Prior
// behavior left the insert to the scheduler loop which created a
// sub-second race window where the run-detail handler 404'd or rendered
// a half-state, firing the spurious htmx:sseError flash.
let run_id = match queries::insert_running_run(&state.pool, job_id, "manual").await {
    Ok(id) => id,
    Err(err) => {
        tracing::error!(target: "cronduit.web", error = %err, job_id, "run_now: insert failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
    }
};
```

**Cmd dispatch** (analog L50) — replace `RunNow { job_id }` with `RunNowWithRunId { job_id, run_id }`.

**Response pattern** (analog L62-72) — copy verbatim (toast + HX-Refresh header):

```rust
let event = HxEvent::new_with_data(
    "showToast",
    json!({"message": format!("Run queued: {}", job.name), "level": "info"}),
).expect("toast event serialization");
let mut headers = axum::http::HeaderMap::new();
headers.insert("HX-Refresh", "true".parse().unwrap());
(HxResponseTrigger::normal([event]), headers, StatusCode::OK).into_response()
```

---

### `src/web/handlers/sse.rs::format_log_line_html` / `sse_logs` (controller — SSE frame emit)

**Analog:** self (L41-69 stream body, L97-113 format_log_line_html)

**Existing stream shape** (L41-69):

```rust
// Current — no id:
yield Ok(Event::default().event("log_line").data(html));

// Phase 11 D-09: add .id() when LogLine.id is Some so the client's
// `event.lastEventId` can be compared to data-max-id.
let mut event = Event::default().event("log_line").data(html);
if let Some(id) = line.id {
    event = event.id(id.to_string());
}
yield Ok(event);
```

**Terminal `run_finished` event** (D-10) — emit BEFORE closing the stream. The current `RecvError::Closed` arm at L57-60 yields `run_complete` and breaks. Add the terminal frame either:
- **Option A (researcher recommendation):** detect a sentinel `LogLine { stream == "__terminal__", ... }` sent by `finalize_run` before `drop(broadcast_tx)` and yield a distinct event:
  ```rust
  if line.stream == "__terminal__" {
      yield Ok(Event::default().event("run_finished").data(format!("{{\"run_id\":{}}}", line.line)));
      break;
  }
  ```
- **Option B:** keep `run_complete` as-is and rely on the SSE handler closing the stream naturally — the existing client at `run_detail.html:117` already listens for `sse:run_complete`. See UI-SPEC § Claude's Discretion #3: planner picks but must keep server and client in lockstep.

**Keep the existing `Lagged` and `Closed` arms unchanged** (L50-60) — they remain the fallback for network-drop and natural end.

---

### `src/web/handlers/run_detail.rs::run_detail` / `fetch_logs` (controller — DB fetch → template)

**Analog:** self (L99-196 run_detail handler, L99-134 fetch_logs)

**Existing pattern** (L150): `fetch_logs` already returns `(logs, total_logs, has_older, next_offset)`. Phase 11 extends the tuple with `last_log_id: i64`:

```rust
async fn fetch_logs(
    pool: &crate::db::DbPool,
    run_id: i64,
    offset: i64,
) -> (Vec<LogLineView>, i64, bool, i64, i64) {  // added last_log_id
    // existing body...
    let last_log_id = log_result.items.last().map(|l| l.id).unwrap_or(0);
    // ...
    (logs, total, has_older, next_offset, last_log_id)
}
```

**LogLineView extension** (L83-88) — add `pub id: i64`:

```rust
pub struct LogLineView {
    pub id: i64,    // NEW: source for data-max-id; currently id isn't exposed
    pub stream: String,
    pub is_stderr: bool,
    pub ts: String,
    pub html: String,
}
```

**Running-branch backfill** (L152-160, L183-195): today the `is_htmx` branch renders `LogViewerPartial`; for the non-HTMX running-run path (full page), Phase 11 keeps the existing `is_running` branch (L179-193) but now the template already has `logs` available — only the `last_log_id` (new field on `RunDetailPage`) needs passing.

**Template-field additions** (L35-44, L46-53, L55-63): every struct that ends up in `templates/.../*log_viewer.html` plus the `RunDetailPage` itself gains `pub last_log_id: i64`.

---

### `templates/pages/run_detail.html` (template — header + breadcrumb + SSE wiring)

**Analog:** self; existing layout is already the correct shape.

**Change 1 — title/breadcrumb/header** (L2, L12, L17) — swap `{{ run.id }}` for `{{ run.job_run_number }}` at the three display sites. Per UI-SPEC § Component Inventory:

```html
<!-- L2  -->  {% block title %}Run #{{ run.job_run_number }} - Cronduit{% endblock %}
<!-- L12 -->  <span class="text-(--cd-text-primary)">Run #{{ run.job_run_number }}</span>
<!-- L17 -->  <h1 style="font-size:var(--cd-text-xl);font-weight:700;letter-spacing:-0.02em">
                Run #{{ run.job_run_number }}
                <span style="font-size:var(--cd-text-base);font-weight:400;color:var(--cd-text-secondary);margin-left:var(--cd-space-2)">
                  (id {{ run.id }})
                </span>
              </h1>
```

**Interaction lock with Phase 10 D-03** (L18-26): the Stop button stays in the right-side slot — do NOT restructure the flex container at L16. UI-SPEC § Layout & Spacing Specifics confirms the `Run #42 (id 1234)` + Stop fit at 1280+px viewports.

**Change 2 — `data-max-id` attr on `#log-lines`** (L79-90) — add `data-max-id="{{ last_log_id }}"` to the `<div id="log-lines">`:

```html
<div id="log-lines"
     role="log"
     aria-live="polite"
     data-max-id="{{ last_log_id }}"            {# NEW — D-08/D-09 #}
     hx-ext="sse"
     sse-connect="/events/runs/{{ run_id }}/logs"
     sse-swap="log_line"
     hx-swap="beforeend"
     style="...">
```

**Change 3 — inline backfill** (L87-89 current placeholder-only) — when running AND `total_logs > 0`, render `log_viewer.html` like the terminal branch at L147-151:

```html
{% if total_logs > 0 %}
  {% include "partials/log_viewer.html" %}
{% else %}
  <div id="log-placeholder" style="...">Waiting for output...</div>
{% endif %}
```

**Change 4 — client dedupe script** (extend inline script at L92-138) — add a handler that drops events whose `lastEventId <= dataset.maxId` and updates the dataset after successful appends. Keep ≤ 15 LOC per CONTEXT.md Discretion; remain inline in run_detail.html.

```javascript
// D-09: id-based dedupe across backfill/SSE boundary.
logLines.addEventListener('htmx:sseMessage', function(evt) {
  var incoming = parseInt(evt.detail?.event?.lastEventId || '0', 10);
  var max = parseInt(logLines.dataset.maxId || '0', 10);
  if (incoming && incoming <= max) {
    evt.preventDefault();    // drop duplicate
    return;
  }
  if (incoming > max) logLines.dataset.maxId = String(incoming);
});
```

**Change 5 — `run_finished` listener** (add alongside existing `sse:run_complete` at L117-122). Either rename or add:

```javascript
logLines.addEventListener('sse:run_finished', function(e) {
  htmx.ajax('GET', '/partials/runs/{{ run_id }}/logs', {
    target: '#log-container', swap: 'outerHTML'
  });
});
```

---

### `templates/partials/run_history.html` (template — new `#N` cell + tr title)

**Analog:** self

**Change 1 — new `<th>` at L21 (leftmost column).** Match the Phase-10 right-most empty `<th>` at L26 pattern (`width:1%`):

```html
<th class="text-left py-2 px-4" style="font-size:var(--cd-text-xs);font-weight:700;text-transform:uppercase;letter-spacing:0.1em;color:var(--cd-text-secondary);width:1%"></th>
```

**Change 2 — new `<td>` at L32 (first cell), plus `title=` on `<tr>` at L31:**

```html
<tr class="hover:bg-(--cd-bg-hover) border-b border-(--cd-border-subtle)"
    title="global id: {{ run.id }}">                               {# D-04 #}
  <td class="py-2 px-4" style="font-size:var(--cd-text-base);color:var(--cd-text-primary);white-space:nowrap">
    #{{ run.job_run_number }}
  </td>
  <!-- existing Status, Trigger, Started, Duration, Exit Code, Stop cells follow unchanged -->
```

**Phase-10 interaction lock** (analog L50-59): the compact Stop cell stays rightmost. Phase 11 adds only one new leftmost column — no other column order or contents changes.

---

### `templates/partials/static_log_viewer.html` (template — inline data attr)

**Analog:** self (L9)

**Current:**
```html
<div id="log-lines" style="background:var(--cd-bg-surface-sunken);padding:var(--cd-space-4);border-radius:8px;overflow-x:auto">
```

**Change:**
```html
<div id="log-lines" data-max-id="{{ last_log_id }}" style="...">
```

`last_log_id` is already passed to `StaticLogViewerPartial` per the controller-side change above.

---

### `src/cli/run.rs` (bin entry — post-migrate assertion)

**Analog:** self — existing migrate call at L62-63 + scheduler spawn at L220-230

**Insertion point:** between L63 (`pool.migrate().await?;`) and L66 (`let tz: chrono_tz::Tz = ...`).

```rust
// 4. Open DB pool and run migrations (idempotent per DB-03).
let pool = DbPool::connect(resolved_db_url.expose_secret()).await?;
pool.migrate().await?;

// Phase 11 D-15: lock the post-migration invariant. In production this
// can never fire (D-12 binds the listener only after migrate + D-14
// fails fast on any backfill error); in tests it guards against a
// regression that lets the scheduler spawn against unbackfilled rows.
{
    let null_count = queries::count_job_runs_with_null_run_number(&pool).await?;
    if null_count > 0 {
        anyhow::bail!(
            "migration assertion failed: {null_count} job_runs row(s) still have NULL \
             job_run_number after migrate(). Restart is recoverable (file 2 is \
             idempotent — WHERE job_run_number IS NULL)."
        );
    }
}
```

Planner note: `count_job_runs_with_null_run_number` is a new `queries.rs` helper — 10 LOC each backend, `SELECT COUNT(*) FROM job_runs WHERE job_run_number IS NULL`.

---

### `tests/v11_*.rs` integration tests

**Analogs by data-flow:**

| Test concern | Analog file | What to copy |
|---|---|---|
| Migration idempotency + schema presence (SQLite) | `tests/migrations_idempotent.rs` (L1-49) | Entire file structure: `DbPool::connect("sqlite::memory:").await.unwrap()` → `pool.migrate().await` ×2 → `PRAGMA table_info` assertion for new columns. |
| Migration parity (SQLite vs Postgres) | `tests/schema_parity.rs` (L215-259) | Testcontainers Postgres start + `sqlx::migrate!("./migrations/postgres")` + introspect both backends + `diff_report` — both migrations must produce schema parity including new `job_runs.job_run_number BIGINT NOT NULL` + `jobs.next_run_number BIGINT NOT NULL`. |
| Postgres migration smoke | `tests/db_pool_postgres.rs` (L1-20) | Entire file verbatim shape; change assertions to check new columns exist after migrate. |
| `insert_log_batch` returns ids in input order | existing test at `src/db/queries.rs:1512-1517` | Insert 10 lines, assert returned ids are monotonic AND `SELECT id, line FROM job_logs ORDER BY id` returns inputs in order. |
| Run-Now race fix (handler inserts before dispatch) | `tests/api_run_now.rs` (L1-157) | Copy the `build_test_app()` + `oneshot(POST /api/jobs/{id}/run)` pattern; add: after the 200 response, `SELECT COUNT(*) FROM job_runs WHERE job_id = ?` must be 1 AND the `SchedulerCmd` dispatched MUST be the new `RunNowWithRunId` variant with matching `run_id`. |
| Scheduler Stop-arm-style driver for end-to-end broadcast | `tests/stop_executors.rs` (L67-98 `spawn_stop_arm_driver`) | Copy the pattern: seed DB → spawn `run_job` → drive via mpsc cmd channel → assert state via `pool.reader()` direct SQL. Use this shape for log-broadcast-with-id tests. |
| SSE stream wiring (currently stubs) | `tests/sse_streaming.rs` (L1-48) | Scaffolds exist but are `#[ignore]`. Planner implements them for Phase 11: build `AppState` with active_runs, subscribe to a broadcast, send a `LogLine { id: Some(42), ... }`, assert the SSE response body contains `id: 42` followed by `event: log_line`. |
| Metrics / test-binary isolation | `tests/metrics_stopped.rs` (L1-98) | File-level doc-comment idiom that explains why each `tests/*.rs` compiles to its own binary (fresh OnceLock state) — useful if Phase 11 adds a backfill counter (deferred). |

**Test ID convention** (all `stop_executors.rs` headers): Phase 11 uses `T-V11-RUNNUM-*`, `T-V11-LOG-*`, `T-V11-BACK-*` per REQUIREMENTS.md Traceability.

---

## Shared Patterns

### Per-backend SQL dispatch via `PoolRef` match

**Source:** `src/db/queries.rs::insert_running_run` (L289-312) and every other query function in the file.

**Apply to:** Every new/modified function in `queries.rs` (D-01 insert_log_batch, D-07/D-11 counter increment, D-15 NULL-count helper, D-07 backfill_job_run_number_batch).

**Pattern:**

```rust
match pool.writer() {                 // .reader() for reads
    PoolRef::Sqlite(p) => {
        // ?1..?N placeholders, &mut *tx or &pool directly
    }
    PoolRef::Postgres(p) => {
        // $1..$N placeholders, otherwise identical
    }
}
```

**Do NOT** try to unify via `Any` driver — the project explicitly uses per-backend branches so the type system catches dialect drift (see 11-RESEARCH §1 Architectural responsibility map).

### Structured tracing log target

**Source:** `src/scheduler/retention.rs` (L21-25, L49-53, L149-154), `src/web/handlers/api.rs` (L52-57, L164-171), `src/scheduler/run.rs` (L80-98, L303-309), `src/scheduler/mod.rs` (L154-159, L198-203).

**Apply to:** `src/db/migrate_backfill.rs` (D-13 per-batch log), `src/web/handlers/api.rs::run_now` new INFO on insert, `src/scheduler/mod.rs` new `RunNowWithRunId` arm, `src/cli/run.rs` assertion failure panic.

**Pattern:**

```rust
tracing::info!(
    target: "cronduit.<domain>",    // e.g. "cronduit.migrate", "cronduit.web", "cronduit.scheduler", "cronduit.run"
    key1 = value1,                  // typed fields — NOT format args
    key2 = %displayed,              // %/= for Display/Debug
    "human-readable summary"        // no interpolation — put data in fields
);
```

**Target names already established** (grep-verified):
- `cronduit.startup`, `cronduit.web`, `cronduit.scheduler`, `cronduit.run`, `cronduit.log_writer`, `cronduit.retention`, `cronduit.reload`

Phase 11 introduces `cronduit.migrate` (D-13 explicit).

### CSRF validation prelude

**Source:** `src/web/handlers/api.rs::run_now` (L32-40), `reload` (L90-98), `reroll` (L204-213), `stop_run` (L336-343).

**Apply to:** Any new POST handler (Phase 11 adds no new POST handlers — `run_now` remains the only mutated one — but the locked CSRF prelude is preserved verbatim).

### HX-Refresh + toast response

**Source:** `src/web/handlers/api.rs::run_now` (L62-72) and `stop_run` (L384-393).

**Apply to:** `run_now` Phase-11 rewrite — D-11 explicitly preserves `HX-Refresh: true` for Run Now and Stop. The live→static log transition does NOT use this pattern (D-11 lock).

### Existing broadcast-channel lifecycle (DO NOT CHANGE)

**Source:** `src/scheduler/run.rs` (L100-121 create + register, L299-301 drop after `finalize_run`) and `src/web/handlers/sse.rs` (L34-39 subscribe site).

**Invariant** (locked by `scheduler/mod.rs::RunEntry` doc-comment L46-57): broadcast_tx refcount arithmetic — executor inserts ONE clone at L101, drops ITS clone at L301 after `active_runs.remove(&run_id)`, leaving zero refs → SSE subscribers get `RecvError::Closed`. Phase 11 MUST preserve this exactly; the terminal `run_finished` event fires BEFORE the drop, not after.

### Template `{% include %}` composition

**Source:** `templates/partials/static_log_viewer.html` (L10) and `templates/pages/run_detail.html` terminal branch (L149).

**Apply to:** Phase 11 D-08 — the running-run branch at `run_detail.html:79-90` gains an `{% include "partials/log_viewer.html" %}` to render the initial backfill. Reuse the existing partial; do NOT fork.

---

## No Analog Found

| File/Pattern | Why | Mitigation |
|---|---|---|
| **Three-file migration shape** (add → backfill → NOT-NULL) — `migrations/{sqlite,postgres}/20260416_000001..3_*.up.sql` | No prior multi-file migration in this repo. `migrations/*/20260410_000000_initial.up.sql` is the only existing migration per backend. | Planner establishes the shape as "first of its kind." The SQL *within* each file follows the existing style exactly (header comments from analog L1-14; `IF NOT EXISTS` + column conventions). The orchestration across the three files lives in `src/db/migrate_backfill.rs` (Rust-side) because sqlx::migrate! supports only static SQL — file 2 is a marker per 11-RESEARCH §2. |

Everything else has a concrete analog already in the codebase.

---

## Metadata

**Analog search scope:** `src/`, `migrations/`, `templates/`, `tests/`, `.planning/phases/11-*/`
**Files scanned:** 42 (via targeted Read + Grep on the 11-RESEARCH § Component Responsibilities row list)
**Key analogs grep-verified:**
- `insert_running_run` RETURNING id pattern — `src/db/queries.rs:286-313` (exact)
- `insert_log_batch` tx + per-line INSERT — `src/db/queries.rs:365-408`
- `SchedulerCmd::Stop` variant shape + `StopResult` enum — `src/scheduler/cmd.rs:33-74`
- `RunNow` arm body — `src/scheduler/mod.rs:187-211`
- `Stop` arm body + coalesce drain — `src/scheduler/mod.rs:229-283, 323-361`
- `retention_pruner` chunked batch + progress log — `src/scheduler/retention.rs:44-155`
- `run_now` handler body — `src/web/handlers/api.rs:26-80`
- `stop_run` handler body — `src/web/handlers/api.rs:329-418`
- `sse_logs` stream + `format_log_line_html` — `src/web/handlers/sse.rs:30-113`
- `run_detail` + `fetch_logs` — `src/web/handlers/run_detail.rs:99-234`
- `LogLine` DTO — `src/scheduler/log_pipeline.rs:22-29`
- `log_writer_task` broadcast + insert — `src/scheduler/run.rs:344-374`
- `run_history.html` Phase-10 compact Stop column — `templates/partials/run_history.html:50-59`
- `run_detail.html` Phase-10 Stop button in header right slot — `templates/pages/run_detail.html:18-26`
- `static_log_viewer.html` `#log-lines` site — `templates/partials/static_log_viewer.html:9`
- `tests/stop_executors.rs::spawn_stop_arm_driver` — analog for scheduler-cmd-arm drivers — `tests/stop_executors.rs:67-98`
- `tests/api_run_now.rs` — analog for axum handler tests — `tests/api_run_now.rs:1-157`
- `tests/migrations_idempotent.rs` + `tests/schema_parity.rs` + `tests/db_pool_postgres.rs` — migration test analogs

**Pattern extraction date:** 2026-04-16
