# Phase 18: Webhook Payload + State-Filter + Coalescing — Pattern Map

**Mapped:** 2026-04-29
**Files analyzed:** 13 (4 NEW, 8 MODIFIED, 1 verify-only)
**Analogs found:** 13 / 13

> Phase 17 (LBL — Custom Docker Labels) is the load-bearing structural precedent for nearly every NEW or MODIFIED file. Phase 16 supplies the SQL/CTE precedent for the new filter-position helper. Phase 15 already locked the `WebhookDispatcher` trait + worker — Phase 18 only ADDS a new `impl` next to `NoopDispatcher`.

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| NEW `src/webhooks/payload.rs` | model + serializer | transform (struct → bytes) | `src/db/queries.rs::DbRunDetail` (struct shape) + `src/webhooks/event.rs` (chrono import + module shape) | role-match |
| NEW `src/webhooks/coalesce.rs` | DB-bound helper (filter-matching position) | request-response (single SQL fetch_one) | `src/db/queries.rs::get_failure_context` L681-744 | exact |
| MODIFY `src/webhooks/dispatcher.rs` (add `HttpDispatcher`) | service / dispatcher impl | request-response (HTTP outbound) | `src/webhooks/dispatcher.rs::NoopDispatcher` (existing trait impl) | exact |
| MODIFY `src/webhooks/mod.rs` (re-export) | barrel-export | n/a | `src/webhooks/mod.rs` L17-23 (existing pub use surface) | exact |
| MODIFY `src/config/mod.rs` (add `WebhookConfig` + fields) | model | config deserialization | `src/config/mod.rs::DefaultsConfig.labels` + `JobConfig.labels` (L86, L116) | exact |
| MODIFY `src/config/defaults.rs::apply_defaults` | service (merge logic) | transform | `apply_defaults` labels merge L166-176 (HashMap union) — **inverted to replace-on-collision** | role-match |
| MODIFY `src/config/validate.rs` (`check_webhook_*`) | validator | request-response (config → errors) | `src/config/validate.rs::check_label_reserved_namespace` L171-192, `check_labels_only_on_docker_jobs` L214-280, `check_label_size_limits` L284-321 | exact |
| MODIFY `src/config/interpolate.rs` (verify only) | n/a (no edits) | n/a | existing whole-file pass at L33-88 already handles `${WEBHOOK_SECRET}` | n/a |
| MODIFY `src/cli/run.rs` (~L250-255 dispatcher swap) | bin-layer wiring | event-driven | existing `Arc::new(NoopDispatcher)` swap site at L250-255 | exact |
| MODIFY `src/telemetry.rs` (2 new counters) | telemetry register | n/a | `setup_metrics` `cronduit_webhook_delivery_dropped_total` describe+zero L111-117 + L133 | exact |
| MODIFY `Cargo.toml` (5 new deps) | config | n/a | existing `sha2 = "0.11"` line 90; existing `hex = "0.4"` line 119 | role-match |
| NEW tests (`tests/v12_webhook_*.rs`) | integration test | event-driven | `tests/v12_webhook_queue_drop.rs` (StalledDispatcher pattern) + `tests/v12_fctx_explain.rs` (EXPLAIN PLAN pattern) + `tests/v12_labels_merge.rs` (end-to-end TOML→docker round-trip pattern) | exact (split per concern) |
| MODIFY `justfile` (3 new UAT recipes) | dev tooling | n/a | `justfile` L260-266 (`uat-fctx-bugfix-spot-check`), L380-382 (`metrics-check`) | role-match |

## Pattern Assignments

### NEW `src/webhooks/payload.rs` (model + transform)

**Analog A:** `src/webhooks/event.rs` (entire file — module/header shape)
**Analog B:** `src/db/queries.rs::DbRunDetail` L600-623 (named-field struct with all Optional metadata fields; idiom for nullable JSON fields)

**Module-header pattern** (mirror `src/webhooks/event.rs:1-9`):
```rust
//! JSON wire-format payload for webhook deliveries (Phase 18 / WH-09).
//!
//! Distinct from `src/webhooks/event.rs` (channel-message contract). This
//! module owns the 15-field v1 schema serialized to compact JSON and HMAC-
//! signed by `HttpDispatcher`. `payload_version: "v1"` is locked for the
//! entire v1.2 line — future additions are additive.
//!
//! NOTE: Field order in the struct == serialization order. `serde_derive`
//! emits fields in declaration order; downstream HMAC compares depend on
//! deterministic byte output. Pitfall B in 18-RESEARCH.md.
```

**Struct-shape pattern** (composite of `event.rs` `RunFinalized` + `queries.rs` `DbRunDetail`):
```rust
use chrono::SecondsFormat;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct WebhookPayload<'a> {
    pub payload_version: &'static str,    // "v1" — D-08
    pub event_type: &'static str,         // "run_finalized" — D-06
    pub run_id: i64,
    pub job_id: i64,
    pub job_name: &'a str,
    pub status: &'a str,
    pub exit_code: Option<i32>,
    pub started_at: String,               // RFC3339 with Z suffix (Pitfall F)
    pub finished_at: String,
    pub duration_ms: i64,
    pub streak_position: i64,             // filter-matching stream position
    pub consecutive_failures: i64,        // unified P16 count, returned as-is
    pub image_digest: Option<String>,     // null for non-docker
    pub config_hash: Option<String>,      // null for pre-v1.2 rows
    pub tags: Vec<String>,                // [] until Phase 22 — schema-stable
    pub cronduit_version: &'static str,   // env!("CARGO_PKG_VERSION") at compile
}
```

**Adaptation notes:**
- Use `SecondsFormat::Secs` + `use_z = true` on every `chrono::DateTime` → string conversion (Pitfall F). Default `to_rfc3339()` produces `+00:00`, which would break receivers expecting `Z`.
- `tags: Vec<String> = vec![]` per D-07 — emit empty array (NOT omit). Receivers see schema-stable shape pre/post Phase 22.
- All 15 fields are named — no `BTreeMap`/`HashMap` — guaranteeing serde-derive deterministic byte output (Pitfall B mitigation).
- Two unit tests required by RESEARCH § Validation Architecture: `payload_serializes_deterministically_to_compact_json` (asserts identical bytes on repeat + no `\n`); `payload_contains_all_15_fields` (struct-level field count).

---

### NEW `src/webhooks/coalesce.rs` (DB-bound helper)

**Analog:** `src/db/queries.rs::get_failure_context` L681-744 (single fetch_one, dual SQLite/Postgres SQL strings, epoch-sentinel `'1970-01-01T00:00:00Z'`)

**Module-header pattern** (extends the FCTX module-doc voice at `queries.rs:625-679`):
```rust
//! Filter-matching stream-position helper (Phase 18 / WH-06).
//!
//! Returns the position of the current run within the consecutive-match
//! streak defined by the operator's `webhook.states` filter. Counts back
//! from the most-recent run, stopping at the first non-match OR the first
//! `success` (whichever is more recent). Mirrors the dual-SQL CTE shape
//! of `src/db/queries.rs::get_failure_context` so both queries hit the
//! same `idx_job_runs_job_id_start (job_id, start_time DESC)` index.
//!
//! Position semantics (D-15):
//! - Position 1 == this is the FIRST matching run since the previous
//!   non-match. With default `fire_every = 1`, this is the only position
//!   that triggers a delivery.
//! - Position N == there have been N consecutive matching runs since
//!   the last non-match.
```

**SQL pattern** (verbatim shape from `queries.rs:682-707`, modified for the operator-supplied filter):
```rust
let sql_sqlite = r#"
    WITH ordered AS (
        SELECT id, status, start_time
          FROM job_runs
         WHERE job_id = ?1
           AND start_time <= ?2
         ORDER BY start_time DESC
    ),
    marked AS (
        SELECT id, status, start_time,
               CASE WHEN status IN (?3, ?4, ?5, ?6, ?7, ?8) THEN 1 ELSE 0 END AS is_match
          FROM ordered
    ),
    first_break AS (
        SELECT MIN(start_time) AS break_time
          FROM marked
         WHERE is_match = 0
    )
    SELECT COUNT(*)
      FROM marked
     WHERE is_match = 1
       AND start_time > COALESCE(
             (SELECT break_time FROM first_break),
             '1970-01-01T00:00:00Z'
           )
"#;
```

**Function-signature pattern** (mirror `get_failure_context` L681 — `pool: &DbPool, job_id: i64 -> anyhow::Result<...>`):
```rust
pub async fn filter_position(
    pool: &DbPool,
    job_id: i64,
    current_start: &chrono::DateTime<chrono::Utc>,
    states: &[String],
) -> anyhow::Result<i64> { /* ... */ }
```

**Adaptation notes:**
- Use the existing 6-state closed enum (`success | failed | timeout | stopped | cancelled | error`) to pad bind placeholders to 6. Operator's actual `states` set is repeated/duplicated to fill — duplicates collapse harmlessly inside `IN (...)` (Pitfall I).
- Backend dispatch via `match pool.reader() { PoolRef::Sqlite(_) => sql_sqlite, PoolRef::Postgres(_) => sql_postgres }` — same `?N` → `$N` substitution Phase 16 used.
- Reuse the **epoch sentinel `'1970-01-01T00:00:00Z'`** — it is the project's lexicographic-comparison-safe NULL stand-in for RFC3339 TEXT timestamps. Do NOT invent a new sentinel.
- A companion EXPLAIN test (`tests/v12_webhook_filter_position_explain.rs`) MUST mirror `tests/v12_fctx_explain.rs` and assert `idx_job_runs_job_id_start` is used on both backends — Wave 0 gap.
- Dispatcher consumes the result for the coalesce decision (`fire_every` modular math); see `HttpDispatcher::deliver` step 4 below.

---

### MODIFY `src/webhooks/dispatcher.rs` (add `HttpDispatcher`)

**Analog:** `src/webhooks/dispatcher.rs::NoopDispatcher` L25-39 (existing impl; identical trait surface; same `tracing::debug!` target string `"cronduit.webhooks"`)

**Trait-impl pattern** (mirror `NoopDispatcher` L25-39 — keep same `target:`, same `Ok(())` return discipline):
```rust
// EXISTING — keep unchanged.
pub struct NoopDispatcher;
#[async_trait]
impl WebhookDispatcher for NoopDispatcher {
    async fn deliver(&self, event: &RunFinalized) -> Result<(), WebhookError> {
        tracing::debug!(
            target: "cronduit.webhooks",
            run_id = event.run_id, job_id = event.job_id,
            status = %event.status,
            "noop webhook dispatch"
        );
        Ok(())
    }
}

// NEW — same trait, new impl, owns reqwest::Client + DbPool + per-job webhook map.
pub struct HttpDispatcher {
    client: reqwest::Client,
    pool: DbPool,
    webhooks: Arc<HashMap<i64, WebhookConfig>>,
    cronduit_version: &'static str,
}

#[async_trait]
impl WebhookDispatcher for HttpDispatcher {
    async fn deliver(&self, event: &RunFinalized) -> Result<(), WebhookError> {
        // 1. Look up per-job webhook config; absent => skip silently.
        // 2. Filter by states; non-match => skip silently.
        // 3. Compute filter-position via crate::webhooks::coalesce::filter_position.
        // 4. Coalesce decision (D-16):
        //    fire_every == 0 => true; == 1 => position == 1; > 1 => position % n == 1.
        // 5. get_failure_context for consecutive_failures + last_success_*.
        // 6. get_run_by_id for image_digest + config_hash (Open Question 2 — recommended path).
        // 7. WebhookPayload::build(...) -> serde_json::to_vec ONCE -> Vec<u8>.
        // 8. ULID + Unix-seconds timestamp; HMAC-SHA256 over "id.timestamp." + body bytes.
        // 9. POST with content-type/webhook-id/webhook-timestamp + (optional) webhook-signature.
        // 10. Increment cronduit_webhook_delivery_sent_total on 2xx,
        //     cronduit_webhook_delivery_failed_total on non-2xx + network err.
        // 11. Return Ok(()) regardless — Phase 20 RetryingDispatcher upgrades error surface.
        Ok(())
    }
}
```

**Error-enum extension pattern** (mirror existing `WebhookError` L11-17):
```rust
// EXISTING:
#[derive(Debug, Error)]
pub enum WebhookError {
    #[error("webhook dispatch failed: {0}")]
    DispatchFailed(String),
}
// Phase 18 extends with explicit variants per RESEARCH:
//   - HttpStatus(u16), Network(String), Timeout, InvalidUrl, SerializationFailed(String)
// All wrapped via map_err -> DispatchFailed for now to keep the worker_loop
// log path simple (it already pattern-matches on the broad enum L63-72).
```

**HMAC sign-once pattern** (RESEARCH § Pattern 3 — Pitfall B mitigation):
```rust
fn sign_v1(secret: &SecretString, webhook_id: &str, webhook_ts: i64, body: &[u8]) -> String {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use secrecy::ExposeSecret;
    let prefix = format!("{webhook_id}.{webhook_ts}.");
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.expose_secret().as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(prefix.as_bytes());
    mac.update(body);
    STANDARD.encode(mac.finalize().into_bytes())
}
```

**Reqwest client construction** (RESEARCH § Pattern 6):
```rust
let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(10))                  // D-18 hard-code
    .pool_idle_timeout(Some(Duration::from_secs(90)))  // keep-alive
    .user_agent(format!("cronduit/{}", env!("CARGO_PKG_VERSION")))
    .build()
    .map_err(|e| WebhookError::DispatchFailed(format!("reqwest client init: {e}")))?;
```

**Adaptation notes:**
- Per RESEARCH § Pattern 3: serialize the payload **once** to `Vec<u8>`, sign that buffer, send those exact bytes. Never `serde_json::to_string` twice.
- Use `base64::engine::general_purpose::STANDARD` (NOT `URL_SAFE_NO_PAD`) — Standard Webhooks v1 spec example contains `/` and `+` (Pitfall E).
- Use `chrono::Utc::now().timestamp()` (seconds, 10-digit), NOT `.timestamp_millis()` (Pitfall D).
- Use `dt.to_rfc3339_opts(SecondsFormat::Secs, true)` for `started_at` / `finished_at` (Pitfall F).
- Per D-05: when `cfg.unsigned == true`, OMIT the `webhook-signature` header entirely; emit `webhook-id` + `webhook-timestamp` as usual.
- Per D-19: serial within the worker task — no semaphore. The worker is already single-threaded; no need to introduce concurrency.
- Per D-17: every non-2xx + every `reqwest::Error` increments `cronduit_webhook_delivery_failed_total` and WARN-logs with `body_preview` truncated to 200 bytes. Do NOT distinguish 4xx vs 5xx (Phase 20).
- Returns `Ok(())` even on network failure — the existing `worker_loop` at `worker.rs:63-72` already logs returned `Err(...)`, but Phase 18's posture is "log + metric inside dispatcher; never propagate failure to the worker." Phase 20's `RetryingDispatcher` upgrades.

---

### MODIFY `src/webhooks/mod.rs` (re-export new types)

**Analog:** existing `pub use` block at `src/webhooks/mod.rs:21-23`

**Pattern** (extend the `pub use` block + add 2 new modules):
```rust
// EXISTING:
pub mod dispatcher;
pub mod event;
pub mod worker;
pub use dispatcher::{NoopDispatcher, WebhookDispatcher, WebhookError};
pub use event::RunFinalized;
pub use worker::{CHANNEL_CAPACITY, channel, channel_with_capacity, spawn_worker};

// NEW — Phase 18:
pub mod coalesce;     // filter-matching position helper
pub mod payload;      // 15-field v1 wire-format struct
pub use dispatcher::HttpDispatcher;
pub use payload::WebhookPayload;
```

**Adaptation notes:** No new submodules beyond `coalesce` + `payload`. `HttpDispatcher` lives inside the existing `dispatcher.rs` (next to `NoopDispatcher`) per the file-list lock; do NOT split it into its own module.

---

### MODIFY `src/config/mod.rs` (add `WebhookConfig` + fields)

**Analog A:** `DefaultsConfig.labels` field at L80-86 + `JobConfig.labels` at L110-116
**Analog B:** `DefaultsConfig.timeout` at L88-89 (Optional with humantime-serde — pattern for Optional struct field with `#[serde(default)]`)

**Field-add pattern** (mirror the labels addition exactly — extend BOTH `DefaultsConfig` AND `JobConfig`):
```rust
// EXISTING in DefaultsConfig at L75-92:
pub struct DefaultsConfig {
    pub image: Option<String>,
    pub network: Option<String>,
    pub volumes: Option<Vec<String>>,
    #[serde(default)]
    pub labels: Option<HashMap<String, String>>,
    pub delete: Option<bool>,
    #[serde(default, with = "humantime_serde::option")]
    pub timeout: Option<Duration>,
    #[serde(default, with = "humantime_serde::option")]
    pub random_min_gap: Option<Duration>,
    // NEW — Phase 18 / WH-01:
    #[serde(default)]
    pub webhook: Option<WebhookConfig>,
}

// EXISTING in JobConfig at L94-134 — add `webhook` after `labels`:
pub struct JobConfig {
    // ...existing fields kept verbatim...
    #[serde(default)]
    pub labels: Option<HashMap<String, String>>,
    // NEW — Phase 18 / WH-01:
    #[serde(default)]
    pub webhook: Option<WebhookConfig>,
    // ...remaining fields unchanged...
}

// NEW struct (place near the bottom of mod.rs, after Config/JobConfig):
#[derive(Debug, Deserialize, Clone)]
pub struct WebhookConfig {
    pub url: String,                       // required (validated by check_webhook_url)
    #[serde(default = "default_webhook_states")]
    pub states: Vec<String>,               // default ["failed", "timeout"]
    #[serde(default)]
    pub secret: Option<SecretString>,      // None when unsigned = true
    #[serde(default)]
    pub unsigned: bool,                    // default false
    #[serde(default = "default_fire_every")]
    pub fire_every: i64,                   // default 1 (first-of-stream)
}

fn default_webhook_states() -> Vec<String> {
    vec!["failed".to_string(), "timeout".to_string()]
}
fn default_fire_every() -> i64 { 1 }
```

**Adaptation notes:**
- Imports already in scope: `secrecy::SecretString` (L12), `serde::Deserialize` (L13). No new imports needed.
- `secret: Option<SecretString>` — note that `SecretString` already implements `Deserialize` (project's `secrecy` feature `serde` — `Cargo.toml:74`). The interpolation pass at `interpolate.rs::interpolate` substitutes `${WEBHOOK_SECRET}` BEFORE TOML parsing, so the resolved value flows into `SecretString` automatically (RESEARCH § Pattern 3 / D-03).
- **Phase 17 5-layer parity invariant DOES NOT apply to `webhook`** (Open Question 1 / Recommendation A). The webhook config STOPS at the bin layer; it never reaches `DockerJobConfig` / `serialize_config_json` / `compute_config_hash`. Document this in the module-doc-comment of `defaults.rs` parity table.
- Class diagram in `defaults.rs` module-doc (lines ~24-58) does NOT need to be updated — it documents the docker-executor surface, which `webhook` does not touch.

---

### MODIFY `src/config/defaults.rs::apply_defaults` (webhook merge)

**Analog A:** existing labels merge at L166-176 (HashMap union — per-job-wins on collision)
**Analog B:** existing `image`/`network`/`volumes`/`timeout`/`delete` per-field merges at L125-153 (single-value replace pattern — `if job.<field>.is_none() && let Some(v) = &defaults.<field> { job.<field> = Some(v.clone()); }`)

**Pattern (extend `apply_defaults` after the labels block):**
```rust
// Webhook merge per Phase 18 D-01 / D-04:
//   * use_defaults = false      → already short-circuited at L112-114
//                                  (per-job stays; defaults discarded entirely).
//   * use_defaults true / unset → if per-job webhook is Some, keep it;
//                                  if None, take defaults.webhook clone.
// Unlike `labels` (HashMap merge with key-level union), `webhook` is a
// SINGLE inline block — replace-on-collision semantics (per-job-wins as a
// whole, not per-field). This matches image/network/volumes/timeout/delete.
//
// No is_non_docker gate: webhooks fire on RunFinalized for ALL job types
// (command/script/docker). The validator owns type-gating concerns; the
// merge is universal.
if job.webhook.is_none()
    && let Some(v) = &defaults.webhook
{
    job.webhook = Some(v.clone());
}
```

**Adaptation notes — the inversion from labels:**
- `labels` (analog A) merges KEYS — defaults map is the base, per-job map overlays per-key, individual keys can be inherited or overridden separately.
- `webhook` is a SINGLE block — there is no "merge URL but inherit secret" semantics. Per-job either fully replaces defaults or doesn't. The closest analog is **NOT** the labels merge; it is the `image`/`network` per-field replace at L125-130. That is the shape Phase 18 should mirror.
- **No type-gate.** Unlike LBL-04 (which gated labels on docker jobs), webhooks apply universally. Update the module-doc parity table at `defaults.rs:63-71` only if you add a row for `webhook`; otherwise leave it (since webhook does not flow into `DockerJobConfig`).
- New unit tests required (Phase 17 LBL-style):
  - `apply_defaults_fills_webhook_from_defaults`
  - `apply_defaults_use_defaults_false_discards_webhook`
  - `apply_defaults_per_job_webhook_overrides_defaults_entirely` (sanity for replace-not-merge)

---

### MODIFY `src/config/validate.rs` (add `check_webhook_*`)

**Analog A:** `check_label_reserved_namespace` L171-192 (Vec collect → sort → format pattern)
**Analog B:** `check_labels_only_on_docker_jobs` L214-280 (multi-branch error formatting + descriptive remediation)
**Analog C:** `check_label_size_limits` L284-321 (multiple independent assertions in one function — D-01 aggregation)
**Analog D:** `check_one_of_job_type` L76-90 (simplest error-shape: single ConfigError with descriptive remediation)

**Pattern: register checks** (extend `run_all_checks` per-job loop at L36-51):
```rust
// EXISTING in run_all_checks per-job loop:
for job in &cfg.jobs {
    check_one_of_job_type(job, path, errors);
    check_cmd_only_on_docker_jobs(job, path, errors);
    check_network_mode(job, path, errors);
    check_schedule(job, path, errors);
    check_label_reserved_namespace(job, path, errors);
    check_labels_only_on_docker_jobs(/* ... */);
    check_label_size_limits(job, path, errors);
    check_label_key_chars(job, path, errors);
    // NEW — Phase 18 / WH-01:
    check_webhook_url(job, path, errors);
    check_webhook_block_completeness(job, path, errors);
}
```

**Pattern: ConfigError shape** (verbatim from `check_label_reserved_namespace` L181-191 — `line: 0, col: 0` is the LBL precedent for post-parse semantic errors):
```rust
errors.push(ConfigError {
    file: path.into(),
    line: 0,
    col: 0,
    message: format!(
        "[[jobs]] `{}`: <NAME-OFFENDING-FIELD-AND-REMEDIATION>",
        job.name
    ),
});
```

**Pattern: `check_webhook_url`** (RESEARCH § Pattern 2 — full sketch is reproduced in 18-RESEARCH.md L327-356):
```rust
fn check_webhook_url(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let Some(wh) = &job.webhook else { return };
    match url::Url::parse(&wh.url) {
        Err(e) => { /* ConfigError: not a valid URL */ }
        Ok(parsed) => {
            let scheme = parsed.scheme();
            if scheme != "http" && scheme != "https" {
                /* ConfigError: scheme not supported */
            }
        }
    }
}
```

**Pattern: `check_webhook_block_completeness`** (RESEARCH § Pattern 2 — full sketch at 18-RESEARCH.md L358-424):
```rust
const VALID_WEBHOOK_STATES: &[&str] =
    &["success", "failed", "timeout", "stopped", "cancelled", "error"];

fn check_webhook_block_completeness(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let Some(wh) = &job.webhook else { return };

    // 1. states non-empty
    // 2. every state ∈ VALID_WEBHOOK_STATES (sort offending list per Pitfall G)
    // 3. secret xor unsigned (both errors are distinct messages — branch on state)
    // 4. fire_every >= 0
    // 5. (Pitfall H) — if secret.is_some(), assert secret.expose_secret().is_empty() == false
}
```

**Adaptation notes:**
- Use `url::Url::parse` (already a project dep at `Cargo.toml:92`) — do NOT regex-validate URLs (RESEARCH § Don't Hand-Roll).
- Per Pitfall G, sort the `invalid` Vec before format — even though `Vec<String>` from TOML parse is order-stable, the convention is enforced project-wide. See `validate.rs:181, 294, 351, 355`.
- Per Pitfall H, ADD the empty-secret check inside `check_webhook_block_completeness` (NOT in `interpolate.rs`). Test name: `check_webhook_block_rejects_empty_secret`.
- Validator error wording: name the offending field, include the offending value, name the fix (Phase 17 LBL precedent — see `check_labels_only_on_docker_jobs` L250-275 for the dual-branch "name the actual fix" example).
- Test coverage required (mirror the labels test set at `validate.rs::tests` L584-908): one test per assertion in `check_webhook_block_completeness` (rejects unknown state / rejects empty states / rejects both secret AND unsigned / rejects neither / rejects negative fire_every / rejects empty secret).

---

### MODIFY `src/config/interpolate.rs` (verify only — no edits)

**Analog:** existing `interpolate` function L33-88 (whole-file textual `${VAR}` substitution; runs BEFORE TOML parse)

**Confirmation pattern:**
```rust
// L33-34 — VAR_RE pattern allows ASCII uppercase + underscore + digits:
static VAR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)\}").unwrap());
```

**Adaptation notes:**
- `${WEBHOOK_SECRET}` matches the `[A-Z_][A-Z0-9_]*` regex — no edits needed to `interpolate.rs`.
- Empty-string env var (`WEBHOOK_SECRET=`) substitutes empty (Pitfall H) — caught at validate-layer, not here.
- Unset env var produces `MissingVar` error at L65-72 — config will not load. Phase 18 gets free coverage for the `${WEBHOOK_SECRET}` unset case.
- ONE new unit test recommended (mirroring the existing labels-interpolation test pattern at `tests/v12_labels_interpolation.rs`): `webhook_secret_interpolates_env_var` — confirms the value flows through into the parsed `Option<SecretString>`.

---

### MODIFY `src/cli/run.rs` (~L250-255 dispatcher swap)

**Analog:** existing dispatcher wire-up at `src/cli/run.rs:250-255`

**Pattern** (from RESEARCH § Pattern 7):
```rust
// EXISTING (Phase 15) at L250-255:
let (webhook_tx, webhook_rx) = crate::webhooks::channel();
let webhook_worker_handle = crate::webhooks::spawn_worker(
    webhook_rx,
    std::sync::Arc::new(crate::webhooks::NoopDispatcher),
    cancel.child_token(),
);

// NEW (Phase 18) — build webhook map; conditionally swap dispatcher:
let webhooks: HashMap<i64, crate::config::WebhookConfig> = cfg.jobs.iter()
    .zip(sync_result.jobs.iter())
    .filter_map(|(job_cfg, db_job)| {
        job_cfg.webhook.as_ref().map(|wh| (db_job.id, wh.clone()))
    })
    .collect();

let dispatcher: Arc<dyn crate::webhooks::WebhookDispatcher> =
    if webhooks.is_empty() {
        Arc::new(crate::webhooks::NoopDispatcher)
    } else {
        Arc::new(crate::webhooks::HttpDispatcher::new(
            pool.clone(),
            Arc::new(webhooks),
        )?)
    };

let (webhook_tx, webhook_rx) = crate::webhooks::channel();
let webhook_worker_handle = crate::webhooks::spawn_worker(
    webhook_rx,
    dispatcher,
    cancel.child_token(),
);
```

**Adaptation notes:**
- `cfg.jobs` and `sync_result.jobs` are zipped by index because `apply_defaults` preserved their order in `parse_and_validate` (`mod.rs:206-209`). Per Open Question 4, this is the clean iteration pattern — confirm by reading the zipped types' invariants (DbJob.id is the post-sync database ID; JobConfig.webhook is the post-defaults config block).
- Per Open Question 1 / Recommendation A: the dispatcher reads from this `HashMap` at construction time. NO round-trip through `config_json`. NO live updates in Phase 18 (Phase 20 owns reload semantics).
- Keep `NoopDispatcher` for the case where ZERO webhooks are configured — avoids spinning a `reqwest::Client` for installations that don't use webhooks.
- The `?` operator on `HttpDispatcher::new(...)?` requires the surrounding fn to return a type compatible with `WebhookError`. Confirm `cli::run`'s return type at the call site supports `WebhookError -> SomeError` conversion (or `.map_err(anyhow::Error::from)`).

---

### MODIFY `src/telemetry.rs` (2 new counters)

**Analog:** existing `cronduit_webhook_delivery_dropped_total` describe + zero-baseline at `telemetry.rs:111-117` + `:133`

**Pattern** (mirror exactly — describe with full HELP string, then zero-baseline below):
```rust
// EXISTING at L111-117 (describe):
metrics::describe_counter!(
    "cronduit_webhook_delivery_dropped_total",
    "Total webhook events dropped because the bounded delivery channel was \
     saturated. Closed-cardinality (no labels in P15). Increments correlate \
     with WARN-level events on the cronduit.webhooks tracing target. The \
     full cronduit_webhook_* family lands in P20 / WH-11."
);

// NEW (Phase 18):
metrics::describe_counter!(
    "cronduit_webhook_delivery_sent_total",
    "Total successful webhook deliveries (HTTP 2xx). Closed-cardinality in \
     Phase 18 (no labels). Phase 20 may add a `job` label when load patterns \
     warrant it."
);
metrics::describe_counter!(
    "cronduit_webhook_delivery_failed_total",
    "Total failed webhook deliveries (non-2xx response or network error). \
     Closed-cardinality in Phase 18. Phase 20 distinguishes 4xx (permanent) \
     vs 5xx/network (transient) for retry decisioning."
);

// EXISTING at L133 (zero-baseline):
metrics::counter!("cronduit_webhook_delivery_dropped_total").increment(0);

// NEW (Phase 18):
metrics::counter!("cronduit_webhook_delivery_sent_total").increment(0);
metrics::counter!("cronduit_webhook_delivery_failed_total").increment(0);
```

**Adaptation notes:**
- Both new counters MUST be both described AND zero-baselined. Phase 15 / Pitfall 3 documented the consequence of describe-without-baseline: HELP/TYPE lines exist but the family disappears from `/metrics` until first observation, breaking Prometheus alerts that reference series existence.
- `tests/metrics_endpoint.rs::metrics_families_described_from_boot` (existing test L17-80) MUST be extended with HELP/TYPE asserts for both new counters — mirror the existing dropped-counter assertions at L72-79.

---

### MODIFY `Cargo.toml` (5 new deps)

**Analog A:** existing `sha2 = "0.11"` at line 90 (single-line dep, no features)
**Analog B:** existing `sqlx = { ..., default-features = false, features = [...] }` at lines 32-40 (full default-features-off feature pinning)

**Pattern** (additive — append to `[dependencies]` and `[dev-dependencies]`):
```toml
# Phase 18 / WH-01..03 / WH-09 — webhook outbound
reqwest = { version = "0.13", default-features = false, features = ["rustls", "json"] }
hmac = "0.13"
base64 = "0.22"
ulid = "1.2"
# (sha2 = "0.11" already at line 90 — no change)

# Phase 18 dev-deps:
[dev-dependencies]
wiremock = "0.6"
```

**Adaptation notes (CRITICAL — Pitfall A):**
- The CONTEXT file (D-20) says `features = ["rustls-tls", "json"]` — that spelling is **reqwest 0.12 only**. In reqwest 0.13 the feature was renamed to `rustls` (no `-tls` suffix). Plan author must use `rustls`, NOT `rustls-tls`. The plan MUST note this deviation from CONTEXT D-20 with a phase-pointer comment in `Cargo.toml`.
- `default-features = false` is mandatory per project lock — keeps the dep tree minimal even though reqwest 0.13's defaults are now rustls-by-default (vs OpenSSL in 0.12).
- `just openssl-check` must remain green (`cargo tree -i openssl-sys` empty). RESEARCH verified `aws-lc-rs` (reqwest 0.13's default rustls provider) declares zero normal deps and bundles its own BoringSSL fork.
- Reuse existing `sha2 = "0.11"` direct dep at line 90 — paired generation with `hmac 0.13` per RustCrypto WG release schedule.

---

### NEW tests (`tests/v12_webhook_*.rs`)

**Analog A:** `tests/v12_webhook_queue_drop.rs` (existing Phase 15 webhook test — `setup_metrics()` + custom dispatcher fixture + counter delta-assert pattern)
**Analog B:** `tests/v12_fctx_explain.rs` (Phase 16 EXPLAIN PLAN harness — `idx_job_runs_job_id_start` index-hit assertion on both SQLite + Postgres)
**Analog C:** `tests/v12_labels_merge.rs` L1-60 (Phase 17 end-to-end TOML→executor round-trip — exercises the full parse/interpolate/apply_defaults/validate path)

**File-naming convention:** `tests/v12_webhook_<concern>.rs` — matches the existing `tests/v12_*.rs` family for v1.2 phase tests. RESEARCH § Wave 0 Gaps lists 6 new test files:

1. `tests/v12_webhook_delivery_e2e.rs` — wiremock round-trip; verifies 3 headers + body bytes + signature
2. `tests/v12_webhook_unsigned_omits_signature.rs` — `unsigned = true` path
3. `tests/v12_webhook_filter_position_explain.rs` — SQLite + Postgres EXPLAIN PLAN (mirror `tests/v12_fctx_explain.rs`)
4. `tests/v12_webhook_state_filter_excludes_success.rs` — full pipeline state-filter exclusion
5. `tests/v12_webhook_failed_metric.rs` — `cronduit_webhook_delivery_failed_total` increment on non-2xx
6. `tests/v12_webhook_success_metric.rs` — `cronduit_webhook_delivery_sent_total` increment on 2xx
7. `tests/v12_webhook_network_error_metric.rs` — failed counter increments on network error

**Pattern: setup_metrics + delta-assert** (verbatim from `tests/v12_webhook_queue_drop.rs:71-78`):
```rust
// Eagerly register the metric family (Pitfall 3 prevention from
// plan 15-03's src/telemetry.rs additions).
let handle = setup_metrics();
// Capture the baseline counter — setup_metrics() is idempotent across tests
// (it uses OnceLock); other tests in this binary may have already incremented.
// Assert on the DELTA, not the absolute value.
let baseline = read_counter(&handle.render(), "cronduit_webhook_delivery_sent_total");
// ... do thing ...
let final_value = read_counter(&handle.render(), "cronduit_webhook_delivery_sent_total");
assert_eq!(final_value - baseline, 1.0, "expected exactly 1 delivery");
```

**Pattern: EXPLAIN PLAN dual-backend** (verbatim from `tests/v12_fctx_explain.rs:46-80`):
```rust
use cronduit::db::queries::{self, PoolRef};
use cronduit::db::{DbBackend, DbPool};
use sqlx::Row;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;

const FILTER_POS_SQL_SQLITE: &str = r#"<verbatim CTE from coalesce.rs>"#;
const FILTER_POS_SQL_POSTGRES: &str = r#"<same CTE with $N>"#;

#[tokio::test]
async fn filter_position_query_uses_idx_job_runs_job_id_start_sqlite() {
    // ... seed >100 rows + ANALYZE + EXPLAIN QUERY PLAN
    // ... assert "idx_job_runs_job_id_start" in plan output
    // ... assert NO bare "SCAN job_runs"
}
#[tokio::test]
async fn filter_position_query_uses_idx_job_runs_job_id_start_postgres() {
    // ... testcontainer seed 10000 rows + ANALYZE + EXPLAIN
    // ... assert "Index Scan using idx_job_runs_job_id_start"
}
```

**Pattern: end-to-end TOML round-trip** (mirror `tests/v12_labels_merge.rs:30-58`):
```rust
let toml_text = r#"
[server]
bind = "127.0.0.1:0"
timezone = "UTC"

[[jobs]]
name = "webhook-test"
schedule = "*/5 * * * *"
command = "echo hi"
webhook = { url = "http://127.0.0.1:0/", states = ["failed"], unsigned = true }
"#;
// Write to tempfile; parse_and_validate(); assert post-validate state.
```

**Adaptation notes:**
- All 6 new tests are non-`#[ignore]` (no Docker required for HTTP wiremock tests; postgres EXPLAIN test uses testcontainers and follows existing `tests/v12_fctx_explain.rs` shape).
- Wiremock setup pattern is documented in 18-RESEARCH.md § Don't Hand-Roll (Standard Stack table); add `wiremock = "0.6"` to `[dev-dependencies]` BEFORE writing tests.
- Per project memory `feedback_uat_use_just_commands.md`: integration tests are NOT UAT — they don't need `just` recipes. `cargo test --test v12_webhook_<name>` is the production runner.
- Test names in `cargo test` filter format: `cargo test -p cronduit --lib webhooks` runs all webhook unit tests; `cargo test --test v12_webhook` runs all integration tests prefixed `v12_webhook_`. RESEARCH § Validation Architecture pre-pinned 30+ test names — planner MUST use those exact names.

---

### MODIFY `justfile` (3 new UAT recipes)

**Analog A:** existing `uat-fctx-bugfix-spot-check` recipe at `justfile:260-266` (Phase 16 spot-check style — uses `sqlite3` + descriptive output)
**Analog B:** existing `metrics-check` recipe at `justfile:380-382` (curl + grep — minimal output for HUMAN-UAT)
**Analog C:** existing `dev` recipe at `justfile:272-275` (`cargo run -- run --config examples/cronduit.toml`)

**Pattern** (mirror `uat-fctx-bugfix-spot-check` shape — `[group(...)]` + `[doc(...)]` + descriptive `@echo` lines):
```just
# Phase 18 / WH-01 — webhook UAT recipes (per project memory feedback_uat_use_just_commands.md).
# Every UAT step in HUMAN-UAT-18.md MUST reference one of these recipes; never raw curl.

[group('uat')]
[doc('Phase 18 — start a small mock HTTP receiver on 127.0.0.1:9999 logging requests')]
uat-webhook-mock:
    # Implementation choice: tiny rust binary OR python -m http.server wrapper.
    # Researcher recommends: a 30-line examples/webhook_mock_server.rs that prints
    # request method, path, all headers, and body to stdout.
    @echo "Starting webhook mock receiver on http://127.0.0.1:9999/"
    @cargo run --example webhook_mock_server

[group('uat')]
[doc('Phase 18 — trigger Run Now against a webhook-configured job to fire one delivery')]
uat-webhook-fire JOB_NAME:
    @echo "Firing 'Run Now' for job: {{JOB_NAME}}"
    @curl -sf -X POST "http://127.0.0.1:8080/api/jobs/{{JOB_NAME}}/run-now"

[group('uat')]
[doc('Phase 18 — print last 5 lines of mock receiver log; maintainer hand-validates HMAC')]
uat-webhook-verify:
    @echo "Last 5 lines from mock receiver:"
    @tail -n 5 /tmp/cronduit-webhook-mock.log
```

**Adaptation notes:**
- Project memory `feedback_uat_use_just_commands.md` is LOAD-BEARING (CONTEXT D-25) — every step in HUMAN-UAT-18.md MUST reference one of these recipes, NEVER raw `curl`/`cargo`/`docker`.
- Project memory `feedback_uat_user_validates.md` (CONTEXT D-26) — Claude does NOT mark UAT passed. Recipes facilitate maintainer's hand-validation, not Claude's auto-runs.
- Mock receiver implementation is researcher-chosen (Open per CONTEXT D-25). The 30-line `examples/webhook_mock_server.rs` approach keeps the toolchain Rust-only and avoids adding python/node to the project.
- Three recipes match RESEARCH § Manual / UAT requirements (just-recipe-only).

## Shared Patterns

### Cross-cutting: ConfigError shape
**Source:** `src/config/errors.rs` — exact struct + `byte_offset_to_line_col` re-export at `src/config/mod.rs:18`
**Apply to:** All new validators in `validate.rs`
```rust
errors.push(ConfigError {
    file: path.into(),
    line: 0,        // post-parse semantic check — no source line per LBL precedent
    col: 0,
    message: format!("[[jobs]] `{}`: <field> <issue>. <remediation>.", job.name),
});
```

### Cross-cutting: HashMap-determinism (Pitfall G / Phase 17 D-01)
**Source:** `src/config/validate.rs:181, 294, 351, 355` — sort BEFORE format
**Apply to:** Any validator that builds an error string from a `HashMap`/`HashSet`/`Vec` of operator-supplied values
```rust
let mut offending: Vec<&str> = ...;
offending.sort();   // determinism — required even for Vec<String> by project convention
errors.push(ConfigError { ... message: format!("..., {}", offending.join(", ")) });
```

### Cross-cutting: Tracing target string
**Source:** `src/webhooks/dispatcher.rs:31` + `src/webhooks/worker.rs:65, 79, 89`
**Apply to:** All new tracing macros in `dispatcher.rs::HttpDispatcher` and `coalesce.rs`
```rust
tracing::debug!(target: "cronduit.webhooks", run_id = event.run_id, ...);
tracing::warn!(target: "cronduit.webhooks", url = %cfg.url, error = %e, "...");
```

### Cross-cutting: Async-trait + Send+Sync trait objects
**Source:** `src/webhooks/dispatcher.rs:19-22`
**Apply to:** `HttpDispatcher` impl
```rust
#[async_trait::async_trait]
impl WebhookDispatcher for HttpDispatcher {
    async fn deliver(&self, event: &RunFinalized) -> Result<(), WebhookError> { ... }
}
```

### Cross-cutting: Telemetry describe + zero-baseline (Pitfall 3)
**Source:** `src/telemetry.rs:111-117` (describe) + `:133` (zero-baseline)
**Apply to:** Both new counters
```rust
metrics::describe_counter!("cronduit_webhook_delivery_<NAME>_total", "<HELP>");
// ... below in same setup block:
metrics::counter!("cronduit_webhook_delivery_<NAME>_total").increment(0);
```

### Cross-cutting: Dual-backend SQL with epoch sentinel
**Source:** `src/db/queries.rs:682-744` (`get_failure_context`)
**Apply to:** `coalesce.rs::filter_position`
- Two parallel `&str` constants (`sql_sqlite` with `?N`, `sql_postgres` with `$N`).
- Backend dispatch via `match pool.reader() { PoolRef::Sqlite(_) => ..., PoolRef::Postgres(_) => ... }`.
- Epoch sentinel `'1970-01-01T00:00:00Z'` for NULL-safe lexicographic timestamp comparison.

### Cross-cutting: SecretString never leaks (D-08 / V8 Data Protection)
**Source:** `src/config/mod.rs:12, 41, 108` — `SecretString` is the project's plaintext-secret wrapper
**Apply to:** `WebhookConfig.secret`, dispatcher logs, validator error messages
```rust
// Never:
tracing::warn!(secret = %cfg.secret.as_ref().unwrap().expose_secret(), "...");
// Always: omit the field, OR use a placeholder, OR rely on SecretString's
// scrubbed Debug/Display impls.
let secret_for_hmac = secret.expose_secret().as_bytes();  // expose only at call site
```

## No Analog Found

(none) — every Phase 18 file has a strong existing analog in the codebase.

## Metadata

**Analog search scope:**
- `src/webhooks/` (4 files: mod, dispatcher, event, worker) — all read in full
- `src/config/` (4 files: mod, defaults, validate, interpolate) — read in targeted ranges
- `src/db/queries.rs` — read at lines 595-744 + signature index (grep)
- `src/cli/run.rs` — read at lines 230-300 (dispatcher wire-up)
- `src/telemetry.rs` — read at lines 90-168 (describe + zero-baseline pattern)
- `Cargo.toml` — read in full (157 lines)
- `tests/` — listed all 60+ files; read 4 in detail (`v12_webhook_queue_drop.rs`, `v12_fctx_streak.rs`, `v12_fctx_explain.rs`, `metrics_endpoint.rs`, `v12_labels_merge.rs`)
- `justfile` — read at lines 255-285, 370-385

**Files scanned:** 17 source files + 5 test files + 2 config files (24 total)

**Pattern extraction date:** 2026-04-29

**Confidence:** HIGH — every analog cited has been read directly; line numbers verified against current `main` (commit ancestry includes phase 17 LBL merge).
