# Phase 22: Job Tagging Schema + Validators - Pattern Map

**Mapped:** 2026-05-04
**Files analyzed:** 11 (8 modified + 3 new)
**Analogs found:** 11 / 11 (every site has an in-tree template — phase is mechanically additive)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `src/config/mod.rs` (modify L114-165, `JobConfig`) | model (config schema) | TOML→struct deserialize | `pub labels: Option<HashMap<...>>` field at `mod.rs:135` | exact |
| `src/config/validate.rs` (add 3 fns + register in `run_all_checks` L43-66) | validator (config) | request-response (config-load → `Vec<ConfigError>`) | `check_label_reserved_namespace` at `validate.rs:185`; `check_label_size_limits` at `validate.rs:298`; `check_duplicate_job_names` at `validate.rs:612` | exact |
| `src/config/hash.rs` (comment-only edit at L16, around L47-50) | model (deterministic ser) | transform (`JobConfig` → SHA-256 hex) | `// DO NOT include env` comment at `hash.rs:50` | exact |
| `src/db/queries.rs` (widen `upsert_job` L62-130) | service (DB write) | CRUD upsert (parity-pair SQLite/Postgres) | existing `upsert_job` itself; `image_digest` parity-pair pattern across same function | exact (self-extension) |
| `src/db/queries.rs` (extend `DbRunDetail` struct L622-648) | model (DB row) | CRUD read | existing `DbRunDetail` fields (`image_digest`, `config_hash`, `scheduled_for`) | exact (self-extension) |
| `src/db/queries.rs` (`get_run_by_id` L1390-1454) | service (DB read) | request-response (row-map JOIN) | `image_digest`/`config_hash`/`scheduled_for` row-map at L1426-1428, L1448-1450 | exact (self-extension) |
| `src/webhooks/payload.rs` (one-line replace L88; rename test L235) | controller-adjacent (payload builder + test) | transform (`DbRunDetail` → `WebhookPayload` JSON) | `image_digest: run.image_digest.clone()` at `payload.rs:86`; `payload_image_digest_null_when_none` at `payload.rs:224` | exact (sibling field on same struct) |
| `migrations/sqlite/20260504_000010_jobs_tags_add.up.sql` (NEW) | migration (DDL) | schema additive | `migrations/sqlite/20260427_000005_image_digest_add.up.sql` | exact |
| `migrations/postgres/20260504_000010_jobs_tags_add.up.sql` (NEW) | migration (DDL) | schema additive | `migrations/postgres/20260427_000005_image_digest_add.up.sql` | exact |
| `tests/v12_tags_validators.rs` (NEW) | test (integration) | end-to-end TOML→DB | `tests/v12_labels_merge.rs` (and `v12_labels_*` family) | exact (file naming family + harness shape) |
| `examples/cronduit.toml` (add `tags = [...]` line) | config (demo) | static asset | existing `labels = {...}` line in same file (Phase 17 demo job) | role-match |

## Pattern Assignments

### `src/config/mod.rs` — `JobConfig.tags` field add (modify, model)

**Analog:** `pub labels: Option<HashMap<String, String>>` at `src/config/mod.rs:135`.

**Field declaration template** (verbatim from `mod.rs:129-135`):
```rust
/// Operator-defined Docker labels attached to spawned containers.
/// Per LBL-01..06 / SEED-001. Merged with cronduit-internal labels
/// at container-create time. `cronduit.*` namespace reserved (LBL-03).
/// Type-gated to docker jobs only (LBL-04). Per-value 4 KB / per-set
/// 32 KB byte-length limits (LBL-06).
#[serde(default)]
pub labels: Option<std::collections::HashMap<String, String>>,
```

**Phase 22 minimal delta:**
- Add `tags: Vec<String>` (NOT `Option<Vec<String>>` — empty Vec is the natural "no tags" form, matches `serde(default)` for omitted-in-TOML, matches the JSON column's `NOT NULL DEFAULT '[]'`).
- Doc-comment cites TAG-01..05 + WH-09 (NOT LBL-* — different requirement family) and explicitly notes "per-job only — NOT on `[defaults]`" (D-01 of REQUIREMENTS).
- **Field placement** (per CONTEXT § canonical_refs lines 382-384): after `cmd: Option<Vec<String>>` (`mod.rs:152`) and before `webhook: Option<WebhookConfig>` (`mod.rs:163`) — groups organizational metadata together, keeps `webhook` last for the existing 5-layer-parity comment to read naturally.
- `#[serde(default)]` — makes `tags = [...]` optional in TOML; omitted → `Vec::new()`.

---

### `src/config/validate.rs` — TAG charset + reserved validator (NEW fn)

**Analog:** `check_label_reserved_namespace` at `validate.rs:185-206`.

**Imports + Lazy<Regex> idiom** (verbatim from `validate.rs:1-20`):
```rust
use super::{Config, ConfigError, JobConfig};
use croner::Cron;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
// ...
static LABEL_KEY_RE: Lazy<Regex> = Lazy::new(|| {
    // Strict ASCII: leading char alphanumeric or underscore; subsequent chars
    // alphanumeric, dot, hyphen, or underscore. Per CONTEXT D-02; mirrors the
    // once_cell idiom at validate.rs:10-13 and interpolate.rs:23-24.
    Regex::new(r"^[a-zA-Z0-9_][a-zA-Z0-9._-]*$").unwrap()
});
```

**Reserved-list constant template** (verbatim from `validate.rs:22-31`):
```rust
/// Canonical RunFinalized status values per src/scheduler/run.rs:315-322.
/// Used by `check_webhook_block_completeness` for the operator's `webhook.states` filter.
const VALID_WEBHOOK_STATES: &[&str] = &[
    "success",
    "failed",
    "timeout",
    "stopped",
    "cancelled",
    "error",
];
```

**Core validator pattern** (verbatim from `validate.rs:185-206`):
```rust
/// LBL-03: reject operator labels under the reserved `cronduit.*` namespace.
/// The cronduit.* prefix is reserved for cronduit-internal labels (currently
/// cronduit.run_id, cronduit.job_name; consumed by docker_orphan reconciliation
/// at src/scheduler/docker_orphan.rs:31). Sorting the offending-key list is
/// CRITICAL — HashMap iteration is non-deterministic (see RESEARCH Pitfall 2).
fn check_label_reserved_namespace(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let Some(labels) = &job.labels else { return };
    let mut offending: Vec<&str> = labels
        .keys()
        .filter(|k| k.starts_with("cronduit."))
        .map(String::as_str)
        .collect();
    if offending.is_empty() {
        return;
    }
    offending.sort(); // determinism — HashMap iter order is random
    errors.push(ConfigError {
        file: path.into(),
        line: 0,
        col: 0,
        message: format!(
            "[[jobs]] `{}`: labels under reserved namespace `cronduit.*` are not allowed: {}. Remove these keys; the cronduit.* prefix is reserved for cronduit-internal labels.",
            job.name,
            offending.join(", ")
        ),
    });
}
```

**Phase 22 minimal delta** (`check_tag_charset_and_reserved`):
- Static regex: `static TAG_CHARSET_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-z0-9][a-z0-9_-]{0,30}$").unwrap());` — different anchored pattern (lowercase post-normalization; length 1-31).
- Reserved-list constant: `const RESERVED_TAGS: &[&str] = &["cronduit", "system", "internal"];` — slice of 3 names (not a prefix; finite list per CONTEXT specifics § lines 543-547).
- Body order per D-04: (1) `if job.tags.is_empty() { return; }`; (2) build `normalized: Vec<String>` via `.iter().map(|t| t.trim().to_lowercase())`; (3) collect three offender categories — `bad_charset` (filter `!TAG_CHARSET_RE.is_match`), `empty_after_trim` (filter `t.trim().is_empty()` on raw inputs — Pitfall 6 in RESEARCH), `reserved_hits` (filter `RESERVED_TAGS.contains(&t.as_str())`); (4) `.sort()` each (Pitfall 2 — determinism); (5) emit one `ConfigError` per category that fired with the operator-readable message (preserve `[[jobs]] '<name>': ...` prefix shape).
- Charset operates on the **post-normalization** form (D-04 step 2 lock — `"Backup"` lowercases to `"backup"` and passes; capital-letter input does NOT produce a charset error).

---

### `src/config/validate.rs` — TAG count cap validator (NEW fn)

**Analog:** `check_label_size_limits` at `validate.rs:298-335`.

**Core count-cap pattern** (verbatim from `validate.rs:298-320`):
```rust
/// LBL-06: enforce per-value (4 KB) and per-set (32 KB) byte-length limits.
/// Two independent checks may both fire for one job (per D-01 aggregation).
fn check_label_size_limits(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let Some(labels) = &job.labels else { return };

    // Per-value check
    let mut oversized_keys: Vec<&str> = labels
        .iter()
        .filter(|(_, v)| v.len() > MAX_LABEL_VALUE_BYTES)
        .map(|(k, _)| k.as_str())
        .collect();
    if !oversized_keys.is_empty() {
        oversized_keys.sort();
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: label values exceed 4 KB limit: {}. Each label value must be ≤ {} bytes.",
                job.name,
                oversized_keys.join(", "),
                MAX_LABEL_VALUE_BYTES
            ),
        });
    }
    // ...
}
```

**Phase 22 minimal delta** (`check_tag_count_per_job`):
- Constant `const MAX_TAGS_PER_JOB: usize = 16;` (D-08 lock).
- Operates on the **post-dedup** count (D-04 step 4 lock): re-run normalize+dedup inside the function OR (cleaner) call the shared `normalize_tags` helper from RESEARCH § Code Examples L718-727.
- Single error per job (no enumeration of which tags are over the cap — operator already wrote them all):
  ```rust
  errors.push(ConfigError {
      file: path.into(),
      line: 0, col: 0,
      message: format!(
          "[[jobs]] `{}`: has {} tags; max is {}. Remove tags or split into multiple jobs.",
          job.name, count, MAX_TAGS_PER_JOB,
      ),
  });
  ```

---

### `src/config/validate.rs` — TAG fleet-level substring-collision (NEW fn)

**Analog:** `check_duplicate_job_names` at `validate.rs:612-654` (the only existing fleet-level validator).

**Fleet-level signature pattern** (verbatim from `validate.rs:612-654`):
```rust
fn check_duplicate_job_names(
    jobs: &[JobConfig],
    path: &Path,
    raw: &str,
    errors: &mut Vec<ConfigError>,
) {
    // Find line numbers by scanning raw source for `name = "..."` matches in order.
    let mut first_seen: HashMap<&str, usize> = HashMap::new();
    let lines: Vec<&str> = raw.lines().collect();
    // ...
    for job in jobs {
        let hits: Vec<usize> = occurrences.iter()
            .filter(|(n, _)| n == &job.name)
            .map(|(_, ln)| *ln)
            .collect();
        if hits.len() > 1 && !first_seen.contains_key(job.name.as_str()) {
            first_seen.insert(&job.name, hits[0]);
            for &dup_line in hits.iter().skip(1) {
                errors.push(ConfigError {
                    file: path.into(),
                    line: dup_line,
                    col: 1,
                    message: format!(
                        "duplicate job name `{}` (first declared at {}:{})",
                        job.name, path.display(), hits[0]
                    ),
                });
            }
        }
    }
}
```

**Phase 22 minimal delta** (`check_tag_substring_collision`):
- Signature: `fn check_tag_substring_collision(jobs: &[JobConfig], path: &Path, errors: &mut Vec<ConfigError>)` — drops `raw: &str` because tag collisions don't have an in-source-line locus (the message names tag values + jobs, not file lines; `line: 0, col: 0` per D-03).
- Body per RESEARCH § Section 4 algorithm (lines 405-462): build `HashMap<String, Vec<String>>` (tag → using-jobs) over post-normalization, post-dedup tags; collect sorted unique tag list; double-loop with `i < j`; for each pair where `a.contains(b) || b.contains(a)`, emit one `ConfigError` with the message shape from D-03:
  ```
  tag 'back' (used by 'cleanup-temp') is a substring of 'backup' (used by 'nightly-backup'); rename or remove one to avoid SQL substring false-positives at filter time.
  ```
- Helper `preview_jobs(&[String]) -> String` for the `'a', 'b', 'c' (+N more)` cap at 3 (RESEARCH L455-461).
- Plain `str::contains` — NOT regex (CONTEXT specifics § L530).

---

### `src/config/validate.rs` — `run_all_checks` registration site (modify L43-66)

**Analog (verbatim from `validate.rs:43-66`):**
```rust
/// Run every post-parse check; push errors into `errors`. Never fail-fast.
pub fn run_all_checks(cfg: &Config, path: &Path, raw: &str, errors: &mut Vec<ConfigError>) {
    check_timezone(&cfg.server.timezone, path, errors);
    check_bind(&cfg.server.bind, path, errors);
    check_duplicate_job_names(&cfg.jobs, path, raw, errors);
    for job in &cfg.jobs {
        check_one_of_job_type(job, path, errors);
        check_cmd_only_on_docker_jobs(job, path, errors);
        check_network_mode(job, path, errors);
        check_schedule(job, path, errors);
        // Phase 17 / SEED-001 — operator labels (LBL-03, LBL-04, LBL-06, D-02)
        check_label_reserved_namespace(job, path, errors);
        check_labels_only_on_docker_jobs(
            job,
            cfg.defaults.as_ref().and_then(|d| d.labels.as_ref()),
            path,
            errors,
        );
        check_label_size_limits(job, path, errors);
        check_label_key_chars(job, path, errors);
        // Phase 18 / WH-01 — webhook validators.
        check_webhook_url(job, path, errors);
        check_webhook_block_completeness(job, path, errors);
    }
}
```

**Phase 22 minimal delta:**
- Inside the per-job loop (after `check_webhook_block_completeness`), append two calls in D-04 order:
  ```rust
  // Phase 22 / TAG-* — D-04 order: normalize → reject → dedup → count cap.
  check_tag_charset_and_reserved(job, path, errors); // TAG-03 + TAG-04 + empty/whitespace
  check_tag_count_per_job(job, path, errors);        // D-08 — post-dedup count
  ```
- After the per-job loop (sibling of `check_duplicate_job_names`), append the fleet-level call:
  ```rust
  // Phase 22 / TAG-05 — fleet-level substring-collision pass (D-03).
  check_tag_substring_collision(&cfg.jobs, path, errors);
  ```
- Note: TAG-03 dedup-WARN is `tracing::warn!` (NOT `errors.push`) — folded into `check_tag_charset_and_reserved` per RESEARCH Open Q 3 (planner discretion). If extracted, name `check_tag_dedup_warn` and call between charset and count-cap.

---

### `src/config/hash.rs` — comment-only edit (modify L47-50)

**Analog:** existing `// DO NOT include env` comment at `hash.rs:50`.

**Existing pattern (verbatim from `hash.rs:44-50`):**
```rust
    if let Some(c) = &job.cmd {
        map.insert("cmd", serde_json::json!(c));
    }
    if let Some(l) = &job.labels {
        map.insert("labels", serde_json::json!(l));
    }
    // DO NOT include `env` -- its values are SecretString and must not be hashed/logged.
```

**Phase 22 minimal delta (D-01 lock):**
- Add a comment line at the field-list site mirroring the existing `env` exclusion:
  ```rust
  // DO NOT include `tags` (Phase 22 / D-01) -- tags are organizational
  // metadata, not docker-execution input. A tag-only edit must NOT
  // trigger "config changed since last success" in the FCTX panel.
  ```
- Placement: immediately after the existing `env` comment (or grouped with it), before the `serde_json::to_vec` call. Order is cosmetic; the function MUST NOT call `map.insert("tags", ...)`.
- **Tests/regression:** RESEARCH Pitfall 2 (validator order) does NOT apply here, but the existing `hash_is_stable` test at `hash.rs:88` should be left untouched. Add a new test asserting `compute_config_hash(job_with_tags_X) == compute_config_hash(job_with_tags_Y)` for two tag sets where everything else is equal — locks D-01 as a regression.

---

### `src/db/queries.rs` — `upsert_job` widening (modify L62-130)

**Analog:** existing `upsert_job` itself (self-extension across the SQLite/Postgres parity-pair).

**Current pattern (verbatim from `queries.rs:62-130`):**
```rust
#[allow(clippy::too_many_arguments)]
pub async fn upsert_job(
    pool: &DbPool,
    name: &str,
    schedule: &str,
    resolved_schedule: &str,
    job_type: &str,
    config_json: &str,
    config_hash: &str,
    timeout_secs: i64,
) -> anyhow::Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();
    match pool.writer() {
        PoolRef::Sqlite(p) => {
            let row = sqlx::query(
                r#"INSERT INTO jobs (name, schedule, resolved_schedule, job_type, config_json, config_hash, enabled, timeout_secs, created_at, updated_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?8)
                   ON CONFLICT(name) DO UPDATE SET
                       schedule = excluded.schedule,
                       resolved_schedule = excluded.resolved_schedule,
                       job_type = excluded.job_type,
                       config_json = excluded.config_json,
                       config_hash = excluded.config_hash,
                       enabled = 1,
                       timeout_secs = excluded.timeout_secs,
                       updated_at = excluded.updated_at
                   RETURNING id"#,
            )
            .bind(name).bind(schedule).bind(resolved_schedule).bind(job_type)
            .bind(config_json).bind(config_hash).bind(timeout_secs).bind(&now)
            .fetch_one(p).await?;
            Ok(row.get::<i64, _>("id"))
        }
        PoolRef::Postgres(p) => {
            // identical with $N placeholders + EXCLUDED instead of excluded
        }
    }
}
```

**Phase 22 minimal delta:**
- Signature: append `tags_json: &str,` argument after `timeout_secs`. (Already `#[allow(clippy::too_many_arguments)]` — no new lint suppressions needed.)
- SQLite SQL: add `tags` to INSERT column list, add `?9` to VALUES, shift `created_at/updated_at` from `?8, ?8` → `?9, ?9` (no — re-number: `?8` for `tags`, then `?9, ?9` for now); add `tags = excluded.tags,` to ON CONFLICT UPDATE SET.
- Postgres SQL: same with `$N`/`EXCLUDED`.
- Both branches widen identically — only differ in `?N` vs `$N` and `excluded`/`EXCLUDED` capitalization (existing pattern already in place).
- Caller-side update: `src/scheduler/sync.rs:177, 192` — both call sites pass an additional `&tags_json` arg; tags_json built right before via `serde_json::to_string(&normalize_tags(&job.tags))` (see RESEARCH § Code Examples sorted-canonical helper).

---

### `src/db/queries.rs` — `DbRunDetail.tags` field add (modify L622-648)

**Analog:** existing `image_digest`, `config_hash`, `scheduled_for` fields on `DbRunDetail` itself (self-extension).

**Existing pattern (verbatim from `queries.rs:621-648`):**
```rust
/// A row from job_runs with the associated job name (for run detail page).
#[derive(Debug, Clone)]
pub struct DbRunDetail {
    pub id: i64,
    pub job_id: i64,
    /// Per-job sequential run number (Phase 11 DB-11). Mirrors `DbRun::job_run_number`.
    pub job_run_number: i64,
    pub job_name: String,
    pub status: String,
    pub trigger: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub duration_ms: Option<i64>,
    pub exit_code: Option<i32>,
    pub error_message: Option<String>,
    /// Phase 16 FOUND-14: image digest from post-start `inspect_container`. NULL for
    /// command/script jobs (no image), pre-v1.2 docker rows (capture site landed in v1.2).
    pub image_digest: Option<String>,
    /// Phase 16 FCTX-04: per-run config_hash captured at fire time by
    /// `insert_running_run`. NULL for pre-v1.2 rows whose backfill found no matching
    /// `jobs.config_hash`. See migration `*_000007_config_hash_backfill.up.sql` for
    /// the BACKFILL_CUTOFF_RFC3339 marker (D-03).
    pub config_hash: Option<String>,
    /// Phase 21 FCTX-06: fire-decision-time RFC3339 timestamp captured by
    /// `insert_running_run` (D-02). NULL on pre-v1.2 rows that landed before
    /// migration `*_000009_scheduled_for_add.up.sql` (D-04 — no backfill).
    pub scheduled_for: Option<String>,
}
```

**Phase 22 minimal delta:**
- Append after `scheduled_for`:
  ```rust
  /// Phase 22 TAG-01 / WH-09: tags from the joined `jobs.tags` JSON column,
  /// deserialized to Vec<String> at the row-mapping site. Empty Vec when the
  /// job has no tags (column is NOT NULL DEFAULT '[]'); never None — schema
  /// guarantees a value. Sorted-canonical order per the upsert path's
  /// normalize_tags helper.
  pub tags: Vec<String>,
  ```
- Note: NOT `Option<Vec<String>>` — the column is `NOT NULL DEFAULT '[]'`, so the read site always produces a `Vec<String>` (possibly empty). Distinguishes from `image_digest`/`config_hash` which are nullable.

---

### `src/db/queries.rs` — `get_run_by_id` row-map widening (modify L1390-1454)

**Analog:** existing SELECT + row-map for `image_digest` / `config_hash` / `scheduled_for` in the same function.

**Current SELECT + row-map pattern (verbatim from `queries.rs:1391-1453`):**
```rust
let sql_sqlite = r#"
    SELECT r.id, r.job_id, r.job_run_number, j.name AS job_name, r.status, r.trigger,
           r.start_time, r.end_time, r.duration_ms, r.exit_code, r.error_message,
           r.image_digest, r.config_hash, r.scheduled_for
    FROM job_runs r
    JOIN jobs j ON j.id = r.job_id
    WHERE r.id = ?1
"#;
// ... Postgres mirror with $1 ...
match pool.reader() {
    PoolRef::Sqlite(p) => {
        let row = sqlx::query(sql_sqlite).bind(run_id).fetch_optional(p).await?;
        Ok(row.map(|r| DbRunDetail {
            id: r.get("id"),
            // ... existing fields ...
            image_digest: r.get("image_digest"), // Phase 16 FOUND-14
            config_hash: r.get("config_hash"),   // Phase 16 FCTX-04
            scheduled_for: r.get("scheduled_for"), // Phase 21 FCTX-06
        }))
    }
    // Postgres arm — identical with $1 placeholder
}
```

**Phase 22 minimal delta:**
- Both SQL strings: project `j.tags AS tags_json` from the existing `JOIN jobs j` (no new JOIN — already in place):
  ```sql
  SELECT r.id, r.job_id, r.job_run_number, j.name AS job_name, r.status, r.trigger,
         r.start_time, r.end_time, r.duration_ms, r.exit_code, r.error_message,
         r.image_digest, r.config_hash, r.scheduled_for,
         j.tags AS tags_json
  FROM job_runs r
  JOIN jobs j ON j.id = r.job_id
  WHERE r.id = ?1
  ```
- Both row-map arms: append after `scheduled_for`:
  ```rust
  tags: {
      let s: String = r.get("tags_json");
      serde_json::from_str(&s).unwrap_or_default() // Phase 22 TAG-01: forgiving on corrupt JSON
  },
  ```
- Both arms widen identically (only differ in `?1` vs `$1`, already in place).
- **Edit symmetry checkpoint:** SQLite arm is at L1414, Postgres at L1436 (RESEARCH § Section 5 verified). Both must change in the same commit.

---

### `src/webhooks/payload.rs` — one-line replace at L88

**Analog:** existing `image_digest: run.image_digest.clone(),` at `payload.rs:86` (sibling field on the same struct, same `run: &DbRunDetail` reference, same `.clone()` ownership pattern).

**Existing pattern (verbatim from `payload.rs:84-91`):**
```rust
            streak_position: filter_position,
            consecutive_failures: fctx.consecutive_failures,
            image_digest: run.image_digest.clone(),
            config_hash: run.config_hash.clone(),
            tags: vec![],
            cronduit_version,
        }
    }
}
```

**Phase 22 minimal delta (D-05 / D-07):**
- Replace `tags: vec![],` with `tags: run.tags.clone(),` (one-line edit).
- Update the field's doc-comment at `payload.rs:51-52` to remove the "Empty `[]` until Phase 22 lights up real values" line. New comment per Phase 22:
  ```rust
  /// Real values from `jobs.tags` column via `DbRunDetail.tags` (Phase 22
  /// WH-09 / D-05). Sorted-canonical order. Always emitted (never omitted)
  /// for schema stability; receivers can index without `KeyError`.
  pub tags: Vec<String>,
  ```

---

### `src/webhooks/payload.rs::tests` — test rename + rewrite at L235-242

**Analog:** existing `payload_image_digest_null_when_none` test at `payload.rs:223-232` (same fixture-build → serialize-to-string → assert-substring shape).

**Existing pattern (verbatim from `payload.rs:223-232`):**
```rust
#[test]
fn payload_image_digest_null_when_none() {
    let event = fixture_event();
    let fctx = fixture_fctx();
    let run = fixture_run_detail(None, None);
    let p = WebhookPayload::build(&event, &fctx, &run, 1, "1.2.0");
    let s = serde_json::to_string(&p).unwrap();
    assert!(s.contains("\"image_digest\":null"));
    assert!(s.contains("\"config_hash\":null"));
}
```

**Existing fixture pattern (verbatim from `payload.rs:120-140`):**
```rust
fn fixture_run_detail(
    image_digest: Option<String>,
    config_hash: Option<String>,
) -> DbRunDetail {
    DbRunDetail {
        id: 42,
        job_id: 7,
        job_run_number: 12,
        job_name: "backup-nightly".to_string(),
        status: "failed".to_string(),
        trigger: "scheduled".to_string(),
        start_time: "2026-04-29T10:43:11Z".to_string(),
        end_time: Some("2026-04-29T10:43:12Z".to_string()),
        duration_ms: Some(1000),
        exit_code: Some(1),
        error_message: None,
        image_digest,
        config_hash,
        scheduled_for: None, // Phase 21 FCTX-06: test fixture
    }
}
```

**Phase 22 minimal delta (D-06.5):**
- **Fixture widening:** Add `tags: vec![]` to the `DbRunDetail { ... }` literal (after `scheduled_for`). Existing 2-arg signature `fixture_run_detail(image_digest, config_hash)` defaults tags to empty so the seven other call sites (`payload.rs:147, 155, 165, 195, 213, 228, 249, 273`) need no change. Add a new helper for the new test:
  ```rust
  fn fixture_run_detail_with_tags(
      image_digest: Option<String>,
      config_hash: Option<String>,
      tags: Vec<String>,
  ) -> DbRunDetail {
      let mut r = fixture_run_detail(image_digest, config_hash);
      r.tags = tags;
      r
  }
  ```
- **Delete** the old test:
  ```rust
  #[test]
  fn payload_tags_empty_array_until_p22() {
      let event = fixture_event();
      let fctx = fixture_fctx();
      let run = fixture_run_detail(None, None);
      let p = WebhookPayload::build(&event, &fctx, &run, 1, "1.2.0");
      let s = serde_json::to_string(&p).unwrap();
      assert!(s.contains("\"tags\":[]"));
  }
  ```
- **Replace with** (mirrors `payload_image_digest_null_when_none` shape):
  ```rust
  #[test]
  fn payload_tags_carries_real_values() {
      // Phase 22 WH-09 / D-06.5: the placeholder is gone; receivers see
      // real tag values from the jobs.tags column. Sorted-canonical order
      // verified — operator-written ["weekly", "backup"] becomes
      // ["backup", "weekly"] in the wire payload.
      let event = fixture_event();
      let fctx = fixture_fctx();
      let run = fixture_run_detail_with_tags(
          None, None, vec!["backup".to_string(), "weekly".to_string()],
      );
      let p = WebhookPayload::build(&event, &fctx, &run, 1, "1.2.0");
      let s = serde_json::to_string(&p).unwrap();
      assert!(
          s.contains(r#""tags":["backup","weekly"]"#),
          "tags must round-trip into payload: {s}"
      );
  }
  ```

---

### `migrations/sqlite/20260504_000010_jobs_tags_add.up.sql` (NEW)

**Analog:** `migrations/sqlite/20260427_000005_image_digest_add.up.sql` (Phase 16 image_digest column add).

**Verbatim template (`migrations/sqlite/20260427_000005_image_digest_add.up.sql`):**
```sql
-- Phase 16: job_runs.image_digest per-run column (FOUND-14, FCTX-04).
--
-- Nullable TEXT, FOREVER (D-01): docker jobs populate this from
-- post-start `inspect_container().image` at finalize time; command and
-- script jobs legitimately have no image and leave the column NULL;
-- pre-v1.2 docker rows also stay NULL forever (D-04 — no backfill).
--
-- Pairs with migrations/postgres/20260427_000005_image_digest_add.up.sql.
-- Any structural change MUST land in both files in the same PR;
-- tests/schema_parity.rs::normalize_type collapses TEXT-family types to
-- TEXT, so this column passes parity with zero test edits (RESEARCH §E).
--
-- Idempotency: sqlx _sqlx_migrations tracking. SQLite ALTER TABLE ADD
-- COLUMN does NOT support a conditional-existence guard clause
-- (RESEARCH Pitfall 3 — Postgres pair uses one; SQLite cannot).
-- Re-runs are guarded by sqlx's migration ledger.

ALTER TABLE job_runs ADD COLUMN image_digest TEXT;
```

**Phase 22 minimal delta:**
- Header swap: `Phase 16: job_runs.image_digest` → `Phase 22: jobs.tags JSON column (TAG-01, TAG-02)`.
- Nullable note swap: "Nullable TEXT, FOREVER (D-01)" → "TEXT NOT NULL DEFAULT '[]', FOREVER (TAG-02)" + rationale: "operators may attach normalized organizational tags to any job in cronduit.toml; existing pre-Phase-22 rows are auto-defaulted to '[]' on column add. Old rows never need backfill — empty-array is a valid in-domain value."
- Pair-comment swap: `migrations/postgres/20260504_000010_jobs_tags_add.up.sql`.
- DDL line: `ALTER TABLE jobs ADD COLUMN tags TEXT NOT NULL DEFAULT '[]';` — different table (`jobs` not `job_runs`), different column type (`TEXT NOT NULL DEFAULT '[]'` not nullable `TEXT`).

---

### `migrations/postgres/20260504_000010_jobs_tags_add.up.sql` (NEW)

**Analog:** `migrations/postgres/20260427_000005_image_digest_add.up.sql`.

**Verbatim template:**
```sql
-- Phase 16: job_runs.image_digest per-run column (FOUND-14, FCTX-04).
--
-- Nullable TEXT, FOREVER (D-01): ...
--
-- Pairs with migrations/sqlite/20260427_000005_image_digest_add.up.sql.
-- ...
-- Idempotency: Postgres `IF NOT EXISTS` provides re-run safety even if
-- sqlx's _sqlx_migrations ledger is somehow out of sync.

ALTER TABLE job_runs ADD COLUMN IF NOT EXISTS image_digest TEXT;
```

**Phase 22 minimal delta:**
- Same header/rationale swaps as the SQLite pair.
- DDL line: `ALTER TABLE jobs ADD COLUMN IF NOT EXISTS tags TEXT NOT NULL DEFAULT '[]';` — preserves the `IF NOT EXISTS` Postgres-only guard clause (RESEARCH Pitfall 5).

---

### `tests/v12_tags_validators.rs` (NEW)

**Analog:** `tests/v12_labels_merge.rs` (closest in family — same v1.2 generation, same `parse_and_validate` end-to-end harness, same atomic-test-per-scenario shape).

**Imports + harness pattern (verbatim from `v12_labels_merge.rs:1-30`):**
```rust
//! Phase 17 / LBL-01 / LBL-02: defaults+per-job labels merge round-trip
//! end-to-end through parse_and_validate -> apply_defaults -> serialize ->
//! execute_docker -> bollard -> docker daemon -> inspect_container.
//!
//! Run: `cargo test --test v12_labels_merge -- --ignored --nocapture --test-threads=1`
//!
//! IMPORTANT: --test-threads=1 (project-wide convention for docker tests).

use cronduit::config::parse_and_validate;
use std::io::Write;
```

**TOML-fixture pattern (verbatim from `v12_labels_merge.rs:38-66`):**
```rust
let toml_text = format!(
    r#"
[server]
bind = "127.0.0.1:0"
timezone = "UTC"

[defaults]
image = "alpine:latest"

[[jobs]]
name = "labels-merge-job"
schedule = "*/5 * * * *"
image = "alpine:latest"
labels = {{ "traefik.enable" = "true" }}
"#
);

let mut tmp = tempfile::NamedTempFile::new().expect("tempfile created");
tmp.write_all(toml_text.as_bytes()).expect("toml written");
let parsed = parse_and_validate(tmp.path()).expect("config parses + validates");
```

**Phase 22 minimal delta:**
- Header doc-comment cites Phase 22 / TAG-01..05 + WH-09; runs *without* `--ignored` (no docker dependency — pure config-load + DB tests).
- **NO docker dependency** — distinct from `v12_labels_merge.rs` which spawns containers. This file's tests are unit-test-flavored integration: `parse_and_validate(tmp_path)` → assert error count + message contents. The DB round-trip tests use a sqlx in-memory SQLite pool; for Postgres parity, gate behind `#[ignore]` or `cfg(feature = "integration")` per existing convention.
- One test per rejection path (T-V12-TAG-01..07 verification anchors per RESEARCH § Phase Requirements):
  - `tag_charset_rejected_for_uppercase_with_special_char` — `tags = ["MyTag!"]` produces a charset error after lowercase normalization (`!` is not in `[a-z0-9_-]`).
  - `tag_reserved_name_rejected` — `tags = ["cronduit"]`, `tags = ["system"]`, `tags = ["internal"]` each produce a reserved-name error.
  - `tag_capital_input_normalizes_then_passes_charset` — `tags = ["Backup"]` produces ZERO errors and the post-normalization tag list is `["backup"]` (RESEARCH Pitfall 2 lock).
  - `tag_dedup_collapse_emits_warn_no_error` — `tags = ["Backup", "backup ", "BACKUP"]` produces zero errors but logs a `tracing::warn!` (use `tracing-subscriber` test fixture or assert via `parsed.warnings` if a warning channel exists).
  - `tag_substring_collision_pair_one_error_per_pair` — two jobs with `["back"]` and `["backup"]` produce exactly ONE `ConfigError`.
  - `tag_substring_collision_three_way_three_pairs` — three tags `bac`/`back`/`backup` produce THREE errors (RESEARCH § Section 4 edge case).
  - `tag_count_cap_19_rejected` — `tags = ["t1","t2",...,"t19"]` produces one count-cap error mentioning 19 and 16.
  - `tag_empty_string_rejected` — `tags = [""]` and `tags = ["   "]` rejected (RESEARCH Pitfall 6).
- One round-trip test: `tags_persisted_to_db_and_round_trip_via_get_run_by_id` — TOML `tags = ["weekly", "backup"]` → `upsert_job` writes `'["backup","weekly"]'` (sorted-canonical) → `get_run_by_id` returns `vec!["backup", "weekly"]`.
- Test naming family: `tests/v12_tags_validators.rs` matches the existing `v12_<feature>_<scenario>.rs` convention (e.g., `v12_labels_merge.rs`, `v12_labels_interpolation.rs`).

---

### `examples/cronduit.toml` — demo line add

**Analog:** the existing `labels = { ... }` line on the demo job (Phase 17 added it).

**Phase 22 minimal delta:**
- Add `tags = ["demo", "hello"]` to ONE existing demo job (RESEARCH Open Q 2 recommendation — one not two; two creates substring-collision risk on a file operators copy-paste-modify).
- Place near the existing `labels = ...` line so operators see organizational metadata grouped together.
- No comment needed — the README subsection (planner discretion) explains the syntax.

---

## Shared Patterns

### Lazy<Regex> static for validator regexes
**Source:** `src/config/validate.rs:10-20`, `src/config/interpolate.rs:34`.
**Apply to:** `check_tag_charset_and_reserved` (TAG_CHARSET_RE).
```rust
static LABEL_KEY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-zA-Z0-9_][a-zA-Z0-9._-]*$").unwrap()
});
```
Compile-once-on-first-call; zero binary-load cost; thread-safe via `once_cell::sync::Lazy`. Project-blessed idiom (zero new crates per D-17).

### ConfigError aggregation (no fail-fast)
**Source:** `src/config/validate.rs` — every existing validator pushes into `&mut Vec<ConfigError>`.
**Apply to:** all three new tag validators.
- Signature pattern: `fn check_*(job_or_jobs, path: &Path, errors: &mut Vec<ConfigError>)`.
- `errors.push(ConfigError { file: path.into(), line: 0, col: 0, message: format!("[[jobs]] `{name}`: ...", ...) })` — line/col 0 for non-source-located errors.
- **CRITICAL:** Always `Vec::sort()` before `.join(", ")` in error messages. HashMap iteration is non-deterministic (RESEARCH Pitfall 2 / Pitfall 3 — already cited at `validate.rs:184, 195, 308, 369`).

### Operator-readable error message shape
**Source:** every existing `check_label_*` and `check_webhook_*` validator.
**Apply to:** all three new tag validators.
- Prefix: `[[jobs]] '<job_name>': ` for per-job errors.
- Body: state the rule violated + name the offending values + state the fix.
- Example: `"[[jobs]] 'nightly-backup': has 19 tags; max is 16. Remove tags or split into multiple jobs."`
- Fleet-level errors (substring collision) drop the `[[jobs]] '<name>':` prefix and instead name BOTH tags + jobs in the message body (D-03 lock).

### sqlx parity-pair widening (SQLite + Postgres in lockstep)
**Source:** `src/db/queries.rs:62-130` (`upsert_job`), `src/db/queries.rs:1390-1454` (`get_run_by_id`).
**Apply to:** `upsert_job` widening AND `get_run_by_id` row-map.
- Both branches widen identically; only differ in `?N` vs `$N` placeholder syntax and `excluded`/`EXCLUDED` capitalization.
- The `match pool.writer() { PoolRef::Sqlite(p) => ..., PoolRef::Postgres(p) => ... }` (or `pool.reader()` for SELECT) shape is the existing pattern; add the new column/binding/SELECT projection in BOTH arms.
- **Edit-pair invariant:** any DDL in `migrations/sqlite/*` MUST land in `migrations/postgres/*` in the same commit. `tests/schema_parity.rs::normalize_type` enforces this for column types via runtime parity check (TEXT-family normalization absorbs `tags TEXT NOT NULL DEFAULT '[]'` automatically — no schema_parity edit needed).

### serde derive on `Vec<String>` for TOML/JSON round-trip
**Source:** native serde behavior; `Vec<String>` is the natural shape for both TOML arrays of strings (`tags = ["a","b"]`) and `serde_json::to_string` output (`["a","b"]`).
**Apply to:** `JobConfig.tags: Vec<String>` field add.
- `#[serde(default)]` → omitted-in-TOML produces `Vec::new()`.
- `serde_json::to_string(&sorted_vec)` → `["a","b"]` canonical compact form (no whitespace, no escapes for charset `[a-z0-9_-]`).
- Round-trip via `serde_json::from_str::<Vec<String>>` is symmetric (RESEARCH § Section 1).

### Sorted-canonical helper (recommended — RESEARCH Open Q 1)
**Source:** new helper recommended in RESEARCH § Code Examples L718-727.
**Apply to:** the upsert_job bind site (sort-then-`to_string` just before binding) AND the substring-collision pass (consistency).
```rust
pub fn normalize_tags(raw: &[String]) -> Vec<String> {
    let mut v: Vec<String> = raw.iter()
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    v.sort();
    v.dedup();
    v
}
```
Placement is planner discretion: `src/config/mod.rs` (alongside `JobConfig`) or new `src/config/tags.rs` sibling module.

---

## No Analog Found

None. Every Phase 22 site has a direct in-tree analog:

| Site | Status |
|------|--------|
| All validators | `check_label_*` family at `validate.rs:185, 298, 612` is the template. |
| Migration | Phase 16 `image_digest_add` migration is the template. |
| `upsert_job` widening | The function itself is the template (self-extension across SQLite/Postgres parity-pair). |
| `DbRunDetail.tags` + row-map | Sibling fields `image_digest`/`config_hash`/`scheduled_for` are the template. |
| Webhook payload one-liner | Sibling field `image_digest: run.image_digest.clone()` at `payload.rs:86` is the template. |
| Test file | `v12_labels_merge.rs` family is the naming + harness template. |

**Phase 22 is mechanically additive with zero structural novelty.** Every primitive the planner needs has an in-tree precedent within ~5 files.

---

## Metadata

**Analog search scope:**
- `src/config/{mod,validate,interpolate,hash}.rs` — all validator + serde + hash analogs.
- `src/db/queries.rs` — DB write/read analogs.
- `src/webhooks/payload.rs` — payload builder + tests analogs.
- `migrations/{sqlite,postgres}/` — migration templates.
- `tests/v12_*.rs` — test-file naming + harness family.

**Files scanned:** 9 source files + 2 migration files + 1 test file = 12 in-tree analogs read; ranges non-overlapping per the no-re-read constraint.

**Pattern extraction date:** 2026-05-04
