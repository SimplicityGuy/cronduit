//! Phase 22 / TAG-01..05 + D-08 + WH-09: end-to-end integration tests
//! for the job-tagging schema and validators.
//!
//! Coverage:
//! - Each rejection path: charset, reserved name, empty/whitespace,
//!   substring-collision (pair + three-way), per-job count cap.
//! - Negative tests: capital input normalizes-then-passes,
//!   identical tags across jobs.
//! - Round-trip persistence: TOML → upsert → jobs.tags column →
//!   get_run_by_id → DbRunDetail.tags.
//!
//! Distinct from `v12_labels_merge.rs` family in NOT requiring docker
//! — Phase 22 is config + DB only. Tests use:
//! - `parse_and_validate(tempfile_path)` for the validator cases
//!   (mirrors the existing pattern from
//!   `tests/v12_webhook_https_required.rs` + `tests/v12_labels_interpolation.rs`)
//! - In-memory SQLite pool (`DbPool::connect("sqlite::memory:")`) for
//!   the round-trip case; postgres parity covered by the existing
//!   `schema_parity` test.
//!
//! Run: `cargo test --test v12_tags_validators -- --nocapture`

use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

use cronduit::config::{ConfigError, ParsedConfig, parse_and_validate};
use cronduit::db::DbPool;
use cronduit::db::queries::{self, PoolRef};
use sqlx::Row;
use tempfile::NamedTempFile;
use tracing::subscriber::DefaultGuard;
use tracing_subscriber::fmt::MakeWriter;

// ---- Helpers ---------------------------------------------------------------

/// Build a minimal valid `[server]` block + N `[[jobs]]` blocks. Each job
/// uses a `command` job-type so docker-only validators (LBL-04, network)
/// stay out of scope. Adapt fragment shape from
/// `tests/v12_webhook_https_required.rs::make_config_toml`.
fn config_with_jobs(jobs_blocks: &str) -> String {
    format!(
        r#"
[server]
bind = "127.0.0.1:0"
timezone = "UTC"
{}
"#,
        jobs_blocks
    )
}

/// Build a single `[[jobs]]` block with the given `name` and a tags clause.
/// `tags_inline` is the verbatim TOML line for tags (or empty string for no
/// tags). Job is a command-type — minimal-valid shape that survives the
/// non-tag validators (schedule, one-of-job-type, etc.).
fn job_block(name: &str, tags_inline: &str) -> String {
    format!(
        r#"
[[jobs]]
name = "{}"
schedule = "*/5 * * * *"
command = "true"
{}
"#,
        name, tags_inline
    )
}

/// Write a TOML body to a tempfile and run `parse_and_validate` end-to-end.
/// Returns the raw Result so callers can match on both arms.
fn parse_with_toml(toml: &str) -> Result<ParsedConfig, Vec<ConfigError>> {
    let mut tmp = NamedTempFile::new().expect("tempfile created");
    tmp.write_all(toml.as_bytes()).expect("toml written");
    tmp.flush().expect("toml flushed");
    // Hold the tempfile on the stack until parse_and_validate has read it,
    // then return the result. The drop happens after the function returns.
    let result = parse_and_validate(tmp.path());
    drop(tmp);
    result
}

// ---- Tracing capture (mirrors src/config/validate.rs::tests fixture) -------
//
// Phase 22 D-17 lock: NO `tracing-test` crate. The dedup-collapse WARN
// path is a tracing::warn! call; capturing it for a #[test] requires a
// thread-local subscriber. The fixture below is the same shape used by
// `validate.rs` unit tests (CapturedWriter + install_capturing_subscriber).
//
// Per the plan: prefer duplicating a small fixture in this integration
// file rather than making the production module's test fixtures public.

#[derive(Clone, Default)]
struct CapturedWriter {
    buf: Arc<Mutex<Vec<u8>>>,
}

impl CapturedWriter {
    fn new() -> Self {
        Self::default()
    }
    fn captured(&self) -> String {
        let v = self.buf.lock().unwrap();
        String::from_utf8_lossy(&v).into_owned()
    }
}

impl Write for CapturedWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let mut v = self.buf.lock().unwrap();
        v.extend_from_slice(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for CapturedWriter {
    type Writer = CapturedWriter;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

fn install_capturing_subscriber() -> (CapturedWriter, DefaultGuard) {
    let writer = CapturedWriter::new();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(writer.clone())
        .with_max_level(tracing::Level::WARN)
        .with_ansi(false)
        .without_time()
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    (writer, guard)
}

// ---- Tests: TAG-04 charset + reserved + empty (Plan 02) --------------------

/// TAG-04: charset reject. `"MyTag!"` post-normalizes to `"mytag!"`,
/// which fails the `^[a-z0-9][a-z0-9_-]{0,30}$` charset due to `!`.
#[test]
fn tag_charset_rejected_for_uppercase_with_special_char() {
    let toml = config_with_jobs(&job_block("j1", r#"tags = ["MyTag!"]"#));
    let errors = parse_with_toml(&toml).expect_err("charset reject expected");
    assert!(
        errors
            .iter()
            .any(|e| e.message.contains("fail charset") && e.message.contains("`mytag!`")),
        "expected charset error mentioning normalized form `mytag!`; got {errors:?}"
    );
}

/// TAG-04: reserved-name reject. `"cronduit"`, `"system"`, `"internal"`,
/// and uppercase `"Cronduit"` (post-normalized) are all rejected.
#[test]
fn tag_reserved_name_rejected() {
    for reserved in ["cronduit", "system", "internal", "Cronduit"] {
        let toml = config_with_jobs(&job_block(
            "j-reserved",
            &format!(r#"tags = ["{}"]"#, reserved),
        ));
        let errors =
            parse_with_toml(&toml).unwrap_err_or_else_panic("reserved-name reject expected");
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("reserved tag names")),
            "expected reserved-tag error for `{reserved}`; got {errors:?}"
        );
    }
}

/// TAG-03 D-04 step 2: capital input normalizes to lowercase before
/// charset evaluation. `"Backup"` → `"backup"` → passes charset.
#[test]
fn tag_capital_input_normalizes_then_passes_charset() {
    let toml = config_with_jobs(&job_block("j-cap", r#"tags = ["Backup"]"#));
    let parsed = parse_with_toml(&toml).expect("Backup must normalize and pass charset");
    let job = parsed
        .config
        .jobs
        .iter()
        .find(|j| j.name == "j-cap")
        .expect("job present");
    // The raw form survives in JobConfig.tags — normalization is applied at
    // validate-time only. The validator's job is to ensure the post-normalized
    // form is acceptable.
    assert_eq!(job.tags, vec!["Backup".to_string()]);
}

/// TAG-04 Pitfall 6: empty string AND whitespace-only entries are both rejected.
#[test]
fn tag_empty_string_rejected() {
    // Case A: empty literal.
    let toml_a = config_with_jobs(&job_block("j-empty", r#"tags = [""]"#));
    let errors_a = parse_with_toml(&toml_a).expect_err("empty-tag reject expected");
    assert!(
        errors_a
            .iter()
            .any(|e| e.message.contains("empty or whitespace-only")),
        "expected empty-tag error for empty literal; got {errors_a:?}"
    );

    // Case B: whitespace-only.
    let toml_b = config_with_jobs(&job_block("j-ws", r#"tags = ["   "]"#));
    let errors_b = parse_with_toml(&toml_b).expect_err("whitespace-only reject expected");
    assert!(
        errors_b
            .iter()
            .any(|e| e.message.contains("empty or whitespace-only")),
        "expected empty-tag error for whitespace-only; got {errors_b:?}"
    );
}

// ---- Tests: TAG-05 substring-collision (Plan 02) ---------------------------

/// TAG-05 D-03: pair substring-collision. Two jobs, one with `back`, one
/// with `backup` → exactly ONE substring-collision ConfigError. Other
/// validators must NOT fire (both tags are individually valid).
#[test]
fn tag_substring_collision_pair_one_error_per_pair() {
    let toml = config_with_jobs(&format!(
        "{}{}",
        job_block("job-back", r#"tags = ["back"]"#),
        job_block("job-backup", r#"tags = ["backup"]"#)
    ));
    let errors = parse_with_toml(&toml).expect_err("substring collision expected");
    let collision_errors: Vec<&ConfigError> = errors
        .iter()
        .filter(|e| e.message.to_lowercase().contains("substring"))
        .collect();
    assert_eq!(
        collision_errors.len(),
        1,
        "exactly one substring-collision error expected; got {} ({:?})",
        collision_errors.len(),
        errors
    );
    // The error should reference both tag values + at least one job name.
    let msg = &collision_errors[0].message;
    assert!(
        msg.contains("back") && msg.contains("backup"),
        "collision error should name both tags; got `{msg}`"
    );
    assert!(
        msg.contains("job-back") || msg.contains("job-backup"),
        "collision error should name at least one job; got `{msg}`"
    );
}

/// TAG-05 D-03: three-way substring-collision. Tags `bac`, `back`,
/// `backup` produce three pairs ({bac,back}, {bac,backup}, {back,backup})
/// → exactly THREE substring-collision ConfigErrors.
#[test]
fn tag_substring_collision_three_way_three_pairs() {
    let toml = config_with_jobs(&format!(
        "{}{}{}",
        job_block("j-bac", r#"tags = ["bac"]"#),
        job_block("j-back", r#"tags = ["back"]"#),
        job_block("j-backup", r#"tags = ["backup"]"#)
    ));
    let errors = parse_with_toml(&toml).expect_err("three-way collision expected");
    let collision_errors: Vec<&ConfigError> = errors
        .iter()
        .filter(|e| e.message.to_lowercase().contains("substring"))
        .collect();
    assert_eq!(
        collision_errors.len(),
        3,
        "exactly three substring-collision errors expected; got {} ({:?})",
        collision_errors.len(),
        errors
    );
}

/// TAG-05: identical tags across jobs are NOT a collision. Two jobs both
/// tagged `backup` produce ZERO substring-collision errors (sharing tags
/// is the intended cross-job grouping mechanism).
#[test]
fn tag_identical_across_jobs_no_error() {
    let toml = config_with_jobs(&format!(
        "{}{}",
        job_block("job-a", r#"tags = ["backup"]"#),
        job_block("job-b", r#"tags = ["backup"]"#)
    ));
    parse_with_toml(&toml).expect("identical tags must NOT trigger collision");
}

// ---- Tests: TAG count cap (Plan 02 D-08) -----------------------------------

/// TAG D-08: per-job count cap. 17 unique tags → ONE count-cap error
/// mentioning both 17 (actual) and 16 (max).
#[test]
fn tag_count_cap_17_rejected() {
    // Build a 17-tag list with non-substring-colliding values: t01..t09 and
    // u10..u17. Plain numeric prefixes would create substring collisions
    // (t1 ⊂ t10), so pad to two digits AND use a different leading char
    // for the second decade so no value is a substring of another.
    let tags: Vec<String> = (1..=9)
        .map(|i| format!(r#""t{:02}""#, i))
        .chain((10..=17).map(|i| format!(r#""u{:02}""#, i)))
        .collect();
    let tag_line = format!("tags = [{}]", tags.join(", "));
    let toml = config_with_jobs(&job_block("j-count-17", &tag_line));
    let errors = parse_with_toml(&toml).expect_err("count-cap reject expected");
    let cap_errors: Vec<&ConfigError> = errors
        .iter()
        .filter(|e| e.message.contains("17") && e.message.contains("max is 16"))
        .collect();
    assert_eq!(
        cap_errors.len(),
        1,
        "exactly one count-cap error expected; got {} ({:?})",
        cap_errors.len(),
        errors
    );
}

// ---- Tests: TAG-03 dedup-collapse WARN (Plan 02) ---------------------------

/// TAG-03 D-04: `["Backup", "backup ", "BACKUP"]` all collapse to the
/// canonical `"backup"`. ZERO ConfigErrors emitted; a tracing::warn!
/// line is emitted naming the originals + canonical form.
#[test]
fn tag_dedup_collapse_does_not_error() {
    let (writer, _guard) = install_capturing_subscriber();
    let toml = config_with_jobs(&job_block(
        "nightly-backup",
        r#"tags = ["Backup", "backup ", "BACKUP"]"#,
    ));
    let parsed = parse_with_toml(&toml).expect("dedup-collapse must not produce errors");
    assert_eq!(parsed.config.jobs.len(), 1);

    // Spot-assert that the WARN line was emitted (lock-step with Plan 02
    // unit tests; integration depth is intentionally light here per the
    // plan's "leaves WARN-line assertion to unit tests" caveat).
    let captured = writer.captured();
    assert!(
        captured.contains("nightly-backup"),
        "WARN should name the job; captured=`{captured}`"
    );
    assert!(
        captured.contains("backup"),
        "WARN should name the canonical form; captured=`{captured}`"
    );
}

// ---- Test: TAG-01 + TAG-02 + WH-09 round-trip (Plan 03) --------------------

/// TAG-01 + TAG-02 + D-09: full TOML → upsert → jobs.tags column →
/// get_run_by_id → DbRunDetail.tags round-trip.
///
/// Asserts (a) the column stores the sorted-canonical JSON form
/// `["backup","weekly"]` (operator wrote `["weekly","backup"]`), and
/// (b) `get_run_by_id` deserializes that column back to
/// `vec!["backup", "weekly"]` on `DbRunDetail.tags`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn tags_persisted_to_db_and_round_trip_via_get_run_by_id() {
    // Step 1: parse a TOML with `tags = ["weekly", "backup"]` to confirm
    // the validator passes the operator-order input. `parse_and_validate`
    // does NOT mutate the JobConfig.tags ordering — sorted-canonical is
    // applied at the upsert site (`src/scheduler/sync.rs`). For this
    // integration test we don't go through sync.rs; we exercise the
    // upsert_job contract documented at `src/db/queries.rs:62-66`:
    // "tags_json -- sorted-canonical JSON-serialized Vec<String>".
    let toml = config_with_jobs(&job_block(
        "round-trip-job",
        r#"tags = ["weekly", "backup"]"#,
    ));
    let parsed = parse_with_toml(&toml).expect("config parses");
    let job = parsed
        .config
        .jobs
        .iter()
        .find(|j| j.name == "round-trip-job")
        .expect("job present");

    // Step 2: produce the sorted-canonical tags_json the production sync
    // path writes. Operator order is ["weekly","backup"]; canonical is
    // sorted+deduped → ["backup","weekly"].
    let mut canonical: Vec<String> = job.tags.iter().map(|t| t.trim().to_lowercase()).collect();
    canonical.sort();
    canonical.dedup();
    let tags_json = serde_json::to_string(&canonical).expect("serialize tags_json");
    assert_eq!(
        tags_json, r#"["backup","weekly"]"#,
        "sorted-canonical form must reorder `weekly,backup` to `backup,weekly`"
    );

    // Step 3: open in-memory SQLite pool and run migrations.
    let pool = DbPool::connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    pool.migrate().await.expect("apply migrations");

    // Step 4: upsert with the canonical tags_json.
    let job_id = queries::upsert_job(
        &pool,
        "round-trip-job",
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        "{}",
        "test-config-hash",
        300,
        &tags_json,
    )
    .await
    .expect("upsert_job ok");

    // Step 5: confirm the raw column value is sorted-canonical JSON.
    let raw_tags: String = match pool.reader() {
        PoolRef::Sqlite(p) => sqlx::query("SELECT tags FROM jobs WHERE id = ?1")
            .bind(job_id)
            .fetch_one(p)
            .await
            .expect("select tags column"),
        PoolRef::Postgres(_) => panic!("test is sqlite-only"),
    }
    .get("tags");
    assert_eq!(
        raw_tags, r#"["backup","weekly"]"#,
        "jobs.tags column must store sorted-canonical JSON"
    );

    // Step 6: insert a job_runs row so get_run_by_id can join it.
    let run_id = queries::insert_running_run(&pool, job_id, "manual", "test-config-hash", None)
        .await
        .expect("insert running run");

    // Step 7: get_run_by_id surfaces tags via the JOIN on jobs.tags.
    let detail = queries::get_run_by_id(&pool, run_id)
        .await
        .expect("get_run_by_id ok")
        .expect("run found");
    assert_eq!(
        detail.tags,
        vec!["backup".to_string(), "weekly".to_string()],
        "DbRunDetail.tags must round-trip sorted-canonical from jobs.tags"
    );
    assert_eq!(detail.job_name, "round-trip-job");
    assert_eq!(detail.job_id, job_id);

    // Sanity: empty-tags path. Upsert a second job with `'[]'` and confirm
    // get_run_by_id returns Vec::new() — locks the column-default behavior.
    let empty_id = queries::upsert_job(
        &pool,
        "no-tags-job",
        "*/5 * * * *",
        "*/5 * * * *",
        "command",
        "{}",
        "test-config-hash-2",
        300,
        "[]",
    )
    .await
    .expect("upsert_job (empty tags)");
    let empty_run =
        queries::insert_running_run(&pool, empty_id, "manual", "test-config-hash-2", None)
            .await
            .expect("insert running run (empty)");
    let empty_detail = queries::get_run_by_id(&pool, empty_run)
        .await
        .expect("get_run_by_id (empty)")
        .expect("run found (empty)");
    assert_eq!(
        empty_detail.tags,
        Vec::<String>::new(),
        "empty tags column must round-trip to Vec::new()"
    );
}

// ---- Tiny helper trait for nicer expect-err with custom panic message ------

trait ResultExt<T, E: std::fmt::Debug> {
    fn unwrap_err_or_else_panic(self, msg: &str) -> E;
}

impl<T: std::fmt::Debug, E: std::fmt::Debug> ResultExt<T, E> for Result<T, E> {
    fn unwrap_err_or_else_panic(self, msg: &str) -> E {
        match self {
            Ok(v) => panic!("{msg} (got Ok({v:?}))"),
            Err(e) => e,
        }
    }
}

// Path used across helpers (currently only `config_with_jobs` references it
// via tempfile name — silenced as unused now but kept to clarify the path
// type the harness operates on).
#[allow(dead_code)]
fn _path_marker() -> &'static Path {
    Path::new("phase-22-test")
}
