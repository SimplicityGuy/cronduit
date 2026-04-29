# Phase 18: Webhook Payload + State-Filter + Coalescing - Research

**Researched:** 2026-04-29
**Domain:** Outbound HTTP delivery (Standard Webhooks v1) + TOML config validators + edge-triggered streak coalescing
**Confidence:** HIGH

## Summary

Phase 18 lights up `HttpDispatcher` against the `WebhookDispatcher` trait that Phase 15 already locked. The seam is clean: Phase 15 wired `try_send` from `finalize_run` step 7d into a bounded mpsc(1024) consumed by a single dedicated tokio worker that calls `dispatcher.deliver(&event)`. Phase 18 swaps `NoopDispatcher` for `HttpDispatcher` at the bin layer (`src/cli/run.rs:251-255`), adds the TOML `webhook = { ... }` block on `JobConfig`/`DefaultsConfig` (mirroring the locked `labels` shape from Phase 17), adds two LOAD-time validators (`check_webhook_url`, `check_webhook_block_completeness`), introduces `src/webhooks/payload.rs` with the locked 15-field v1 schema, computes the filter-matching stream position by counting consecutive matches backward from the most-recent-run cursor, signs the compact JSON body with HMAC-SHA256 over `${webhook-id}.${webhook-timestamp}.${body}`, and ships the three Standard Webhooks v1 headers (`webhook-id` ULID / `webhook-timestamp` integer Unix seconds / `webhook-signature: v1,<base64>`).

The Standard Webhooks v1 spec was fetched directly (`https://github.com/standard-webhooks/standard-webhooks/blob/main/spec/standard-webhooks.md`, **2026-04-29**) and the wire-format details are LOCKED below: literal `.` separators (full-stops), `v1,<base64>` with a single comma, standard base64 (not URL-safe), unsigned deliveries are NOT addressed in the spec — they are a cronduit extension under `webhook.unsigned = true`.

**Primary recommendation:** Use `reqwest 0.13` with `default-features = false, features = ["rustls", "json"]`, `hmac 0.13` paired with the existing `sha2 0.11`, `base64 0.22`, and `ulid 1.2`. Construct one `reqwest::Client` per `HttpDispatcher` instance (connection-pool-friendly). Serialize the payload struct **once** to `Vec<u8>`, sign that exact byte buffer, then send those bytes verbatim — never re-serialize between sign and send.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| TOML `webhook` block parse + validate | Config (load-time) | — | LBL/LOAD precedent: config errors must surface before scheduler boot |
| Filter-matching stream position computation | Backend / DB-bound Rust | — | Single SQL query against `job_runs` is cheaper + simpler than caching state in the worker; no shared state |
| Payload encoding (15-field v1 schema) | Backend (`webhooks/payload.rs`) | — | Pure data shaping; deterministic JSON; no I/O |
| HMAC-SHA256 signing | Backend (`HttpDispatcher`) | — | RustCrypto in-process; no network |
| HTTP delivery | Backend (`HttpDispatcher`, Tokio task) | — | Already the locked Phase 15 worker shape (one task, one HTTP request at a time) |
| Metrics emission | Telemetry façade (`metrics::counter!`) | — | Existing `cronduit_*` family; eager-described from boot |
| UI surface | (none) | — | Phase 18 has NO UI surface (CONTEXT D-27); Phase 21 owns FCTX panel |

## User Constraints

> Copied verbatim from `18-CONTEXT.md`. Locked decisions; planner does not relitigate.

### Locked Decisions

**Configuration (WH-01):**
- D-01: Config shape parallels SEED-001 / LBL-01..05: `webhook` block lives on `[[jobs]]` AND in `[defaults]`. Per-job override + `use_defaults = false` disable matches existing precedent. Single block (not multiple webhook configs per job) — one URL per job.
- D-02: Field schema:
  ```toml
  webhook = {
    url      = "https://hook.example.com/...",   # required
    states   = ["failed", "timeout"],            # default if omitted
    secret   = "${WEBHOOK_SECRET}",              # required UNLESS unsigned = true
    unsigned = false,                            # opt-in to skip signing (default false)
    fire_every = 1                               # 1 = first-of-stream (default); 0 = every; N = every Nth match
  }
  ```
- D-03: `secret` interpolates `${ENV_VAR}` via the existing whole-file textual interpolation pass (per Phase 17's CR-01 truth-up). Resolved value wrapped in `SecretString` at config-load.
- D-04: Validators (LOAD-time, cf. Phase 17 LBL pattern):
  - `webhook.url` — must parse as `url::Url`; scheme `http`/`https` only.
  - `webhook.states` — every entry must be one of `success | failed | timeout | stopped | cancelled | error`. Empty array `states = []` is rejected. Default when omitted: `["failed", "timeout"]`.
  - `webhook.secret` xor `webhook.unsigned = true` — exactly one must be present.
  - `webhook.fire_every` — must be ≥ 0 (i64). Negative rejected.
- D-05: `webhook.unsigned = true` causes the dispatcher to omit the `webhook-signature` header entirely.

**Payload Schema (WH-09):**
- D-06: 15 fields per WH-09: `payload_version` (`"v1"`), `event_type` (`"run_finalized"`), `run_id`, `job_id`, `job_name`, `status`, `exit_code`, `started_at`, `finished_at`, `duration_ms`, `streak_position`, `consecutive_failures`, `image_digest`, `config_hash`, `tags`, `cronduit_version`.
- D-07: Field formats — RFC3339 timestamps; integer `streak_position`; `null` for missing; `tags = []` until Phase 22; `env!("CARGO_PKG_VERSION")` for `cronduit_version`.
- D-08: Schema locked at `payload_version: "v1"`. Future additions additive; breaking changes require `v2` bump.

**Standard Webhooks v1 Headers (WH-03):**
- D-09: Three headers: `webhook-id` (ULID), `webhook-timestamp` (Unix seconds integer string), `webhook-signature` (`v1,<base64-of-hmac>`).
- D-10: Body is compact JSON. Signature is over raw response-body bytes BEFORE serialization to disk anywhere.
- D-11: `Content-Type: application/json`. No charset suffix.

**Coalescing × State-Filter (WH-06):**
- D-12: Coalesce within the filter-matching stream.
- D-13: `failed → timeout` with `states = ["timeout"]` → `consecutive_failures = 2` (unified P16 count); filter-matching position = 1; with default `fire_every = 1`, the timeout fires.
- D-14: P16's streak semantics stay locked; Phase 18 does NOT extend Phase 16's helper.
- D-15: Filter-matching-stream position is computed by counting (from most-recent backwards) consecutive runs whose `status ∈ states`, stopping at the first non-matching run OR the first `success`.
- D-16: Coalescing rule: `fire_every = 0` → always; `fire_every = 1` → `filter_position == 1`; `fire_every = N` (N>1) → `filter_position % N == 1`.

**HttpDispatcher Posture:**
- D-17: Single attempt per delivery. On failure: `cronduit_webhook_delivery_failed_total` ++ + WARN log. NO `webhook_deliveries` row write.
- D-18: Per-request timeout: 10 seconds, hard-coded.
- D-19: Concurrency: serial within the worker task.
- D-20: HTTP client: `reqwest` with `default-features = false, features = ["rustls-tls", "json"]`. Project lock: rustls everywhere; `cargo tree -i openssl-sys` must remain empty. **(See § Crate Currency below — reqwest 0.13 renamed `rustls-tls` to `rustls`; planner must use the 0.13 spelling.)**
- D-21: Phase 20 retry hook point: trait stays the same; Phase 20 introduces `RetryingDispatcher` wrapper.
- D-22: HMAC: `hmac` + `sha2` (RustCrypto). `webhook-id`: `ulid` (preferred) or UUID v7.

**Universal:**
- D-23: PR on feature branch; no direct commits to main. Working branch: `phase-18-webhook-payload`.
- D-24: All diagrams mermaid.
- D-25: UAT recipes use `just` commands.
- D-26: Maintainer validates UAT.
- D-27: NO UI surface in Phase 18.

### Claude's Discretion

- Migration filenames, function signatures, internal struct shapes, test file names — researcher and planner choose, following existing patterns.
- Whether `webhook-id` uses `ulid` or `uuid` v7 — researcher recommends below.
- Exact metric names beyond `cronduit_webhook_delivery_failed_total`.
- Exact validator error wording — follow Phase 17 LBL precedent.

### Deferred Ideas (OUT OF SCOPE)

- Multi-webhook per job (one URL for failures, one for successes, fan-out)
- Webhook UI (status panel, replay button)
- SSRF allow/blocklist (deferred to v1.3 per WH-08)
- HMAC algorithm-agility (SHA-384/512/Ed25519)
- Pluggable signature schemes (e.g., GitHub-style `x-hub-signature`)
- Per-attempt timeout configurability (Phase 20 if needed)
- Concurrent delivery semaphore (v1.3+ if load demands)
- `webhook_deliveries` dead-letter table (Phase 20)

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| WH-01 | Per-job + `[defaults]` `webhook` config with `use_defaults = false` disable | § Validator Sketch + § `apply_defaults` Merge Semantics |
| WH-03 | Standard Webhooks v1 headers (`webhook-id`/`webhook-timestamp`/`webhook-signature`); HMAC-SHA256 over `id.timestamp.payload`; signature `v1,<base64>` | § Standard Webhooks v1 Wire Contract (verified against spec) |
| WH-06 | Edge-triggered streak coalescing; default `fire_every = 1` (first-of-stream); operator override | § Filter-Matching Stream Position Algorithm + § `fire_every` Modular Math |
| WH-09 | Locked 15-field payload schema; `payload_version: "v1"`; future-additive | § Payload Encoder Sketch |

## Project Constraints (from CLAUDE.md)

The following directives are LOAD-BEARING and treated with the same authority as locked decisions. Plans that contradict any of these get rejected:

- **rustls everywhere; `cargo tree -i openssl-sys` must return empty.** Phase 18's `reqwest` adoption MUST use `default-features = false` and explicit rustls feature.
- **Mermaid only**; no ASCII art in any artifact (planning, README, code comments, PR body).
- **Workflow:** changes land via PR on feature branch (`phase-18-webhook-payload`). No direct commits to main.
- **Versioning:** `Cargo.toml` version = git tag. Phase 18 does NOT bump Cargo.toml — the v1.2.0-rc.1 cut is Phase 20.
- **`just` commands required for UAT steps**; never raw `cargo`/`docker`/`curl` in UAT documents.
- **UAT validated by maintainer**, never by Claude's own runs.
- **TOML for config**; no YAML.
- **askama_web 0.15 with `axum-0.8` feature**, NOT `askama_axum`. (Not relevant to Phase 18 — no UI surface.)
- **No CLI shelling out** for Docker — `bollard`. (Not directly relevant; included for reference.)

## Standard Stack

### Core (Phase 18 ADDS)

| Library | Version | Purpose | Why Standard | Confidence |
|---------|---------|---------|--------------|------------|
| **reqwest** | 0.13.3 (verified 2026-04-29) | Outbound HTTP client | Tokio-native, project's locked rustls posture, ergonomic JSON/header API. **`default-features = false, features = ["rustls", "json"]`** — see Pitfall A below for the 0.12→0.13 feature-flag rename. | HIGH |
| **hmac** | 0.13.0 (verified 2026-04-29) | Generic HMAC over a hash trait | RustCrypto idiom; pairs cleanly with `sha2 0.11` (already in Cargo.toml line 90) via `digest 0.11`. | HIGH |
| **sha2** | 0.11.0 | SHA-256 implementation | **Already a direct dep** at Cargo.toml:90. Phase 18 just adds `hmac` against it. | HIGH |
| **base64** | 0.22.1 | Standard base64 encoding | Standard Webhooks v1 explicitly uses standard alphabet (not URL-safe). Use `base64::engine::general_purpose::STANDARD` (with `=` padding). | HIGH |
| **ulid** | 1.2.1 | `webhook-id` generator | 26-char Crockford-base32, lex-sortable by time prefix, ideal for receiver-side idempotency dedup. `Ulid::new()` → `.to_string()`. | HIGH |

**Already direct deps (no version bump needed):** `tokio`, `chrono`, `serde`, `serde_json`, `tracing`, `metrics`, `secrecy`, `url`, `regex`, `once_cell`, `anyhow`, `thiserror`, `async-trait`.

### Supporting

| Library | Version | Purpose | When to Use | Confidence |
|---------|---------|---------|-------------|------------|
| **wiremock** | 0.6.5 | Mock HTTP server for integration tests | Async-native, tokio-friendly, well-suited for axum/reqwest test stacks. Used in tests for "delivery hits the right URL with the right headers" assertions. | HIGH |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `reqwest 0.13` | `reqwest 0.12.28` | 0.12 line still maintained; 0.13 is current stable as of 2025-12-30 (4 months at research date). Both work. **Pick 0.13** — fresh project additions should sit on the current line. |
| `ulid 1.2` | `uuid 1.23` with `v7` feature | Both spec-acceptable. ULID has shorter wire form (26 vs 36 chars), guaranteed lex-sortability by Crockford-base32 string compare; UUIDv7 is more widely recognized but its lex-sort property is byte-level (string-form `-`-separation breaks naive sort). **Pick `ulid`.** |
| `hmac 0.13` + `sha2 0.11` | `ring` | `ring`'s API is harder; the RustCrypto pair is the idiomatic fit + already aligned with the existing `sha2 0.11` dep. |
| `base64 0.22` | `data-encoding` | `data-encoding` is overkill; `base64::STANDARD` is the spec match exactly. |
| `wiremock 0.6` | `httpmock 0.8` | Both work. `wiremock` has more idiomatic async API; `httpmock` is simpler but blocks. **Pick `wiremock`.** No existing precedent in the repo (`grep wiremock\|httpmock src/ tests/` returned nothing). |
| `RetryingDispatcher` Phase 20 wrapper | Algebraic effects on the trait | Per D-21, composability via wrapper trait keeps the seam clean. Phase 18 stays as-is. |

**Installation (additive to existing Cargo.toml):**

```toml
[dependencies]
# (existing deps unchanged — sha2 = "0.11" already at line 90)

# Phase 18 additions
reqwest = { version = "0.13", default-features = false, features = ["rustls", "json"] }
hmac    = "0.13"
base64  = "0.22"
ulid    = "1.2"

[dev-dependencies]
# (existing dev-deps unchanged)
wiremock = "0.6"
```

### Crate Currency Verification (queried crates.io 2026-04-29)

| Crate | Latest Stable | Updated | Yanked? |
|-------|---------------|---------|---------|
| reqwest | 0.13.3 | 2026-04-27 | no |
| hmac | 0.13.0 | 2026-03-29 | no |
| sha2 | 0.11.0 | 2026-03-25 | no (already direct dep) |
| base64 | 0.22.1 | 2024-04-30 | no |
| ulid | 1.2.1 | 2025-03-17 | no |
| uuid | 1.23.1 | 2026-04-16 | no (alternative) |
| wiremock (dev) | 0.6.5 | 2025-08-24 | no |

**`openssl-sys` audit:** reqwest 0.13 with `features = ["rustls"]` pulls `hyper-rustls 0.27` + `tokio-rustls 0.26` + `rustls 0.23` + `aws-lc-rs` (Rust default crypto provider in reqwest 0.13). `aws-lc-rs` declares **zero normal deps** (verified via crates.io API on 0.13.3) — it bundles its own BoringSSL fork and does NOT pull `openssl-sys`. The existing `just openssl-check` recipe (loops native + arm64-musl + amd64-musl `cargo tree -i openssl-sys`) catches any regression.

### `reqwest` 0.12 → 0.13 Feature-Flag Rename (CRITICAL)

D-20 in CONTEXT.md says `features = ["rustls-tls", "json"]`. **That spelling is `reqwest 0.12`.** In `reqwest 0.13.3` the feature was renamed:

| reqwest 0.12 | reqwest 0.13 |
|--------------|---------------|
| `rustls-tls` | `rustls` |
| `rustls-tls-no-provider` | `rustls-no-provider` |
| `default-tls` (= native-tls + OpenSSL) | `default-tls` (= **`rustls`**) |

The 0.13 default already uses rustls (no longer OpenSSL-by-default), but the project's blanket guard — `default-features = false` — still applies for clarity and to keep the dep tree minimal. **Planner must write `features = ["rustls", "json"]`** when adding to `Cargo.toml`. The planner SHOULD note this 0.12→0.13 deviation from CONTEXT D-20 in the plan; CONTEXT was written before the version pick and is not contradicting research, just naming the prior-line feature.

## Architecture Patterns

### System Architecture Diagram

```mermaid
flowchart LR
  CFG[cronduit.toml<br/>webhook = {url,states,secret,unsigned,fire_every}] --> INTERP[interpolate.rs<br/>${WEBHOOK_SECRET} substitution]
  INTERP --> PARSE[mod.rs<br/>toml::from_str -> Config]
  PARSE --> DEFAULTS[defaults.rs::apply_defaults<br/>webhook merge]
  DEFAULTS --> VALIDATE[validate.rs::run_all_checks<br/>check_webhook_url<br/>check_webhook_block_completeness]
  VALIDATE --> SYNC[scheduler::sync<br/>config_json column]
  SYNC --> RUN[(jobs / job_runs<br/>SQLite or Postgres)]

  SCHED[scheduler::run::finalize_run<br/>step 7d try_send] -->|RunFinalized event| CHAN[(mpsc bounded 1024<br/>Phase 15 lock)]
  CHAN --> WORKER[webhooks::worker<br/>worker_loop]
  WORKER --> DISP[HttpDispatcher::deliver<br/>NEW Phase 18]

  DISP -->|read job webhook config| RUN
  DISP -->|get_failure_context job_id| RUN
  DISP -->|filter_position SQL helper| RUN
  DISP --> DECIDE{filter match<br/>AND coalesce rule}
  DECIDE -- skip --> METSKIP[no metric increment;<br/>debug log only]
  DECIDE -- fire --> ENCODE[payload.rs<br/>build 15-field payload]
  ENCODE --> SERIALIZE[serde_json::to_vec<br/>compact JSON]
  SERIALIZE --> SIGN[hmac-sha256<br/>id.timestamp.body]
  SIGN --> POST[reqwest::Client::post<br/>10s timeout]
  POST -->|2xx| METOK[cronduit_webhook_delivery_sent_total ++]
  POST -->|non-2xx<br/>or network err| METFAIL[cronduit_webhook_delivery_failed_total ++<br/>WARN log]
```

### Component Responsibilities

| Component | File | Responsibility | Phase |
|-----------|------|----------------|-------|
| `Config::JobConfig.webhook` field | `src/config/mod.rs:95-134` (extend) | TOML deserialization of the inline `webhook` block | 18 |
| `Config::DefaultsConfig.webhook` field | `src/config/mod.rs:75-92` (extend) | TOML deserialization in `[defaults]` | 18 |
| `apply_defaults` — webhook merge | `src/config/defaults.rs:108-182` (extend) | Replace-on-collision merge (single inline block, not a map) | 18 |
| `check_webhook_url` validator | `src/config/validate.rs::run_all_checks` (extend, ~L36-52) | URL parse + http/https scheme | 18 |
| `check_webhook_block_completeness` validator | `src/config/validate.rs::run_all_checks` (extend) | `secret` xor `unsigned`; `states` non-empty + valid; `fire_every >= 0` | 18 |
| `WebhookConfig` struct | NEW `src/config/mod.rs` | Holds parsed webhook block (post-merge, post-interpolate) | 18 |
| `serialize_config_json` extension | `src/scheduler/sync.rs` | Add `webhook` field to per-run `config_json` IF the dispatcher reads from there. **(Recommendation: Phase 18 dispatcher reads webhook config from a per-job in-memory map, NOT from `config_json`. See § HttpDispatcher Construction.)** | 18 |
| `WebhookPayload` struct + `Serialize` | NEW `src/webhooks/payload.rs` | 15-field payload struct with field-order = struct-order = `Serialize` derive output | 18 |
| Filter-matching stream position | NEW `src/webhooks/coalesce.rs` (or fold into dispatcher.rs) | Single SQL query against `job_runs` walking back from current run | 18 |
| `HttpDispatcher` | `src/webhooks/dispatcher.rs` (extend) | Implements `WebhookDispatcher`; owns `reqwest::Client`, secrets map, per-job webhook config map | 18 |
| `WebhookError` variants | `src/webhooks/dispatcher.rs:11-17` (extend) | Add `HttpStatus(u16)`, `Network(String)`, `Timeout`, `InvalidUrl`, `SerializationFailed(String)` | 18 |
| Metric registration | `src/telemetry.rs:107-117` (extend) | `describe_counter!` + zero-baseline for `cronduit_webhook_delivery_sent_total` and `cronduit_webhook_delivery_failed_total` | 18 |
| Bin-layer wiring | `src/cli/run.rs:250-255` (modify) | Swap `Arc::new(NoopDispatcher)` → `HttpDispatcher::new(...)` if any webhook is configured | 18 |

### Recommended Module Layout

```
src/
├── config/
│   ├── mod.rs          # extend: JobConfig.webhook + DefaultsConfig.webhook + WebhookConfig type
│   ├── defaults.rs     # extend: webhook merge (replace-on-collision)
│   └── validate.rs     # extend: check_webhook_url + check_webhook_block_completeness
├── webhooks/
│   ├── mod.rs          # extend: pub use HttpDispatcher; pub mod payload; pub mod coalesce
│   ├── dispatcher.rs   # extend: HttpDispatcher struct + impl
│   ├── event.rs        # unchanged (channel-message contract)
│   ├── worker.rs       # unchanged (Phase 15 worker_loop)
│   ├── payload.rs      # NEW: WebhookPayload struct + serialize
│   └── coalesce.rs     # NEW: filter-matching position query helper
└── telemetry.rs        # extend: 2 new counter describes + zero-baselines
```

### Pattern 1: TOML Inline Block Mirrors `labels`

**What:** A single inline TOML table on `[[jobs]]` and in `[defaults]`, controlled by `use_defaults = false` for whole-block disable.

**When to use:** Anywhere we mirror the SEED-001 / LBL-01..05 contract.

**Example** (from `src/config/mod.rs:75-92` + extension):

```rust
// Source: src/config/mod.rs:75-92 (existing labels precedent — extend the same way)
#[derive(Debug, Deserialize)]
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

    // Phase 18 / WH-01 ADDITION
    #[serde(default)]
    pub webhook: Option<WebhookConfig>,
}

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

### Pattern 2: LOAD-Time Validator with Sorted Error Output

**What:** Per-job validator runs after `apply_defaults`. Mirrors the Phase 17 `check_label_*` shape exactly.

**When to use:** Whenever an operator's TOML can violate a contract.

**Example** (sketch of `check_webhook_url` and `check_webhook_block_completeness`):

```rust
// Source: src/config/validate.rs::run_all_checks (Phase 17 LBL pattern at L36-52)
const VALID_WEBHOOK_STATES: &[&str] = &[
    "success", "failed", "timeout", "stopped", "cancelled", "error",
];

fn check_webhook_url(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let Some(wh) = &job.webhook else { return };
    match url::Url::parse(&wh.url) {
        Err(e) => {
            errors.push(ConfigError {
                file: path.into(),
                line: 0, col: 0,
                message: format!(
                    "[[jobs]] `{}`: webhook.url `{}` is not a valid URL: {}. \
                     Provide a fully-qualified URL like `https://hook.example.com/path`.",
                    job.name, wh.url, e
                ),
            });
        }
        Ok(parsed) => {
            let scheme = parsed.scheme();
            if scheme != "http" && scheme != "https" {
                errors.push(ConfigError {
                    file: path.into(),
                    line: 0, col: 0,
                    message: format!(
                        "[[jobs]] `{}`: webhook.url scheme `{}` is not supported \
                         (allowed: `http`, `https`).",
                        job.name, scheme
                    ),
                });
            }
        }
    }
}

fn check_webhook_block_completeness(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let Some(wh) = &job.webhook else { return };

    // States — non-empty AND every entry valid
    if wh.states.is_empty() {
        errors.push(ConfigError {
            file: path.into(), line: 0, col: 0,
            message: format!(
                "[[jobs]] `{}`: webhook.states is empty. Use absence of the \
                 `webhook` block to disable webhooks; `states = []` is meaningless.",
                job.name
            ),
        });
    }
    let mut invalid: Vec<&str> = wh.states.iter()
        .map(String::as_str)
        .filter(|s| !VALID_WEBHOOK_STATES.contains(s))
        .collect();
    if !invalid.is_empty() {
        invalid.sort(); // determinism for HashMap-backed inputs (Pitfall 2 mirror)
        errors.push(ConfigError {
            file: path.into(), line: 0, col: 0,
            message: format!(
                "[[jobs]] `{}`: webhook.states contains unknown values: {}. \
                 Valid values: {}.",
                job.name, invalid.join(", "), VALID_WEBHOOK_STATES.join(", ")
            ),
        });
    }

    // secret xor unsigned
    let has_secret = wh.secret.is_some();
    let unsigned = wh.unsigned;
    if has_secret == unsigned {
        // Either both or neither — both are errors with distinct messaging.
        errors.push(ConfigError {
            file: path.into(), line: 0, col: 0,
            message: if has_secret {
                format!(
                    "[[jobs]] `{}`: webhook block has both `secret` AND `unsigned = true`. \
                     Set `unsigned = true` to skip signing (omit `secret`), OR set `secret` \
                     to sign deliveries (omit `unsigned`).",
                    job.name
                )
            } else {
                format!(
                    "[[jobs]] `{}`: webhook block needs either `secret = \"${{ENV_VAR}}\"` \
                     (signed deliveries) OR `unsigned = true` (opt-in unsigned for receivers \
                     like Slack/Discord). Currently neither is set.",
                    job.name
                )
            },
        });
    }

    // fire_every non-negative
    if wh.fire_every < 0 {
        errors.push(ConfigError {
            file: path.into(), line: 0, col: 0,
            message: format!(
                "[[jobs]] `{}`: webhook.fire_every = {} is negative. \
                 Use 0 (always fire), 1 (first-of-stream — default), or N>1 (every Nth match).",
                job.name, wh.fire_every
            ),
        });
    }
}
```

### Pattern 3: HMAC Signing Sequencing — Serialize Once, Sign Buffer, Send Buffer

**What:** The single most common implementation bug in webhook signing is "serialize → sign → re-serialize for the body" — the second serialization can produce different bytes (whitespace, key reordering, escape differences) and the receiver-side HMAC computation will diverge.

**Canonical idiom:**

```rust
// CORRECT — serialize once into Vec<u8>, sign that buffer, send those exact bytes.
let body_bytes: Vec<u8> = serde_json::to_vec(&payload)?;  // compact JSON
let signing_string = format!("{webhook_id}.{webhook_timestamp}.");
let mut mac = Hmac::<Sha256>::new_from_slice(secret.expose_secret().as_bytes())
    .expect("HMAC accepts any key length");
mac.update(signing_string.as_bytes());
mac.update(&body_bytes);
let signature_bytes = mac.finalize().into_bytes();
let signature_b64 = base64::engine::general_purpose::STANDARD.encode(signature_bytes);

// Send body_bytes verbatim — DO NOT call serde_json::to_string again.
let response = client
    .post(url)
    .header("content-type", "application/json")
    .header("webhook-id", &webhook_id)
    .header("webhook-timestamp", webhook_timestamp.to_string())
    .header("webhook-signature", format!("v1,{signature_b64}"))
    .body(body_bytes)  // <-- exact same buffer the HMAC saw
    .timeout(Duration::from_secs(10))
    .send()
    .await?;
```

**Anti-pattern (DO NOT DO):**

```rust
// WRONG — second serialization may produce different bytes.
let json = serde_json::to_string(&payload)?;
let mac_input = format!("{}.{}.{}", id, ts, json);
// ... compute HMAC ...
let body = serde_json::to_string(&payload)?;  // DIFFERENT BYTES POSSIBLE
client.post(url).body(body).send().await?;
```

### Pattern 4: Filter-Matching Stream Position — Single SQL Query

**What:** Compute the position of the current run within the consecutive-match streak by a single backwards-walking SQL query that stops at the first non-match.

**When to use:** Phase 18's coalesce decision. Runs once per `RunFinalized` event that the worker processes.

**Recommendation:** Add a function to `src/db/queries.rs` (sibling to `get_failure_context`) returning an `i64` position. The query takes `(job_id, current_run_start_time, filter_states)` and counts:

```sql
-- SQLite shape (Postgres mirror with $1 / $2 / unnest binding)
WITH
  -- 1. Current and earlier runs for this job, newest first.
  ordered AS (
    SELECT id, status, start_time
      FROM job_runs
     WHERE job_id = ?1
       AND start_time <= ?2          -- include current run
     ORDER BY start_time DESC
  ),
  -- 2. Mark each row: 1 if status ∈ filter_states, 0 otherwise.
  --    Use a CASE chain because SQLite has no array-bind for IN(?...) -- the
  --    binding helper expands the filter_states slice into ?3, ?4, ?5, ...
  --    placeholders dynamically (Pitfall 7).
  marked AS (
    SELECT id, status, start_time,
           CASE WHEN status IN (?3, ?4, ?5, ?6, ?7, ?8) THEN 1 ELSE 0 END AS is_match
      FROM ordered
  ),
  -- 3. Find the first non-match's start_time (or NULL if none).
  first_break AS (
    SELECT MIN(start_time) AS break_time
      FROM marked
     WHERE is_match = 0
  )
-- 4. Count consecutive matches whose start_time > break_time (or all matches if no break).
SELECT COUNT(*)
  FROM marked
 WHERE is_match = 1
   AND start_time > COALESCE((SELECT break_time FROM first_break), '1970-01-01T00:00:00Z')
```

**Trade-off table for the algorithm choice (D-15 was researcher-decided):**

| Option | Pros | Cons | Pick? |
|--------|------|------|-------|
| **Single SQL query (recommended)** | One round-trip; portable across SQLite + Postgres; uses existing `idx_job_runs_job_id_start` index; no Rust-side state cache | Bind-parameter gymnastics for the `IN (?...)` list (need a helper to expand the filter_states slice into `?3, ?4, ...` parameters; see Pitfall 7) | **YES** |
| Read all recent runs in Rust + walk in memory | Simplest Rust code | Two queries (still pulling rows + counting); larger data transfer for jobs with deep history; same dialect-portability surface anyway | no |
| Reuse `get_failure_context` + add a separate query | DRY | `get_failure_context` doesn't apply the operator's filter; would need a second query anyway. No saving. | no |
| Cache filter-position in a per-job `Arc<RwLock<...>>` updated by the worker | Zero DB hits per delivery | Loses correctness across restart; worker becomes stateful (Phase 15 worker is intentionally stateless); reload semantics fragile | no |

**Race vs current run insertion (caller responsibility):** The dispatcher calls this AFTER `finalize_run` writes the row (the `RunFinalized` event is emitted at step 7d, AFTER the DB UPDATE at step 7). So `job_runs` already contains the current run's terminal status. The query reads from a settled state.

### Pattern 5: `apply_defaults` Webhook Merge — Replace-On-Collision

**What:** Unlike `labels` (a map, where keys merge with per-job-wins), `webhook` is a **single inline block**. Merge semantics are simpler: if per-job is `Some`, use it; if absent and `use_defaults != Some(false)`, take from defaults.

**Code-level read of `apply_defaults` (current shape):** at `src/config/defaults.rs:108-182`, the function:
1. Returns `job` unchanged if `defaults.is_none()` OR `job.use_defaults == Some(false)`.
2. Field-by-field, merges each defaults field into `job` only if `job.<field>.is_none()`.

**Webhook merge sketch (extend `apply_defaults`):**

```rust
// Source: extend src/config/defaults.rs:108-182 (after the labels merge block at L166-176)

// Webhook merge per Phase 18 D-01 / D-04:
//   * use_defaults = false      → already short-circuited at L112 (per-job stays; defaults discarded)
//   * use_defaults true / unset → if per-job webhook is Some, keep it;
//                                  if None, take defaults.webhook.
// Unlike labels (HashMap merge), webhook is a single inline block — no key merging.
// This is the simplest "fill from defaults if absent" pattern, identical to image/network/etc.
if job.webhook.is_none()
    && let Some(v) = &defaults.webhook
{
    job.webhook = Some(v.clone());
}
```

**Important:** Unlike `labels`, the `webhook` merge should be **gated on docker-or-not**? **NO.** Webhooks fire on `RunFinalized` events for ALL job types (command/script/docker). No type-gate needed.

### Pattern 6: HttpDispatcher Construction — Single `reqwest::Client` Per Process

**What:** `reqwest::Client` is internally `Arc`-cloneable and pools HTTP connections. Construct once at startup; share across all webhook deliveries.

**Standard guidance:** From reqwest docs — "The `Client` holds a connection pool internally, so it is advised that you create one and **reuse** it." Per-request construction defeats keep-alive.

**Cronduit-shape:** Phase 15 already locked one dispatcher instance for the process (`Arc<dyn WebhookDispatcher>` shared with the worker via `Arc::new(NoopDispatcher)` at `src/cli/run.rs:253`). Phase 18's `HttpDispatcher` owns the single client.

**HttpDispatcher struct sketch:**

```rust
// Source: extend src/webhooks/dispatcher.rs (after NoopDispatcher at L25-39)

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use reqwest::Client;
use secrecy::SecretString;
use crate::db::DbPool;

/// Phase 18 HttpDispatcher — implements WebhookDispatcher for real HTTP delivery.
/// Owns a single `reqwest::Client` (connection-pooled), the `DbPool` for
/// failure-context + filter-position lookups, and a per-job map of resolved
/// `WebhookConfig` keyed by `job_id`. The map is built once at startup from
/// the validated config and rebuilt on reload (see § Reload Survival in
/// Phase 20 — Phase 18 ships static initial config only).
pub struct HttpDispatcher {
    client: Client,
    pool: DbPool,
    /// Map keyed by job_id; built from the validated config at startup.
    /// `Arc` because the bin layer holds one and the worker holds one.
    webhooks: Arc<HashMap<i64, WebhookConfig>>,
    /// Project version, baked at compile time per D-07.
    cronduit_version: &'static str,
}

impl HttpDispatcher {
    pub fn new(
        pool: DbPool,
        webhooks: Arc<HashMap<i64, WebhookConfig>>,
    ) -> Result<Self, WebhookError> {
        // Single shared client; rustls TLS; 10s per-request timeout (D-18).
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            // Keep TCP connections alive across deliveries to the same host
            // (typical for an operator pointing every webhook at the same
            // PagerDuty / Slack / homelab gateway).
            .pool_idle_timeout(Some(Duration::from_secs(90)))
            .user_agent(format!("cronduit/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| WebhookError::DispatchFailed(format!("reqwest client init: {e}")))?;
        Ok(Self {
            client,
            pool,
            webhooks,
            cronduit_version: env!("CARGO_PKG_VERSION"),
        })
    }
}

#[async_trait::async_trait]
impl WebhookDispatcher for HttpDispatcher {
    async fn deliver(&self, event: &RunFinalized) -> Result<(), WebhookError> {
        // 1. Look up webhook config for this job; if absent, skip.
        let Some(cfg) = self.webhooks.get(&event.job_id) else {
            return Ok(());
        };

        // 2. Filter by states.
        if !cfg.states.iter().any(|s| s == &event.status) {
            return Ok(());
        }

        // 3. Compute filter-matching stream position.
        let filter_position = crate::webhooks::coalesce::filter_position(
            &self.pool, event.job_id, &event.started_at, &cfg.states,
        ).await
            .map_err(|e| WebhookError::DispatchFailed(format!("filter_position query: {e}")))?;

        // 4. Coalesce decision (D-16).
        let should_fire = match cfg.fire_every {
            0 => true,
            1 => filter_position == 1,
            n if n > 1 => filter_position % n == 1,
            _ => false, // negative — caught at validate, defensive return.
        };
        if !should_fire {
            tracing::debug!(
                target: "cronduit.webhooks",
                run_id = event.run_id, job_name = %event.job_name,
                filter_position, fire_every = cfg.fire_every,
                "skip delivery: coalesced"
            );
            return Ok(());
        }

        // 5. Read failure context (P16 helper).
        let fctx = crate::db::queries::get_failure_context(&self.pool, event.job_id)
            .await
            .map_err(|e| WebhookError::DispatchFailed(format!("get_failure_context: {e}")))?;

        // 6. Look up the current run's image_digest + config_hash from job_runs.
        //    Needed for payload fields. Reuse get_run_detail or add a small helper.
        //    (Plan author chooses; see Open Question 1.)
        let run_detail = crate::db::queries::get_run_detail(&self.pool, event.run_id)
            .await
            .map_err(|e| WebhookError::DispatchFailed(format!("get_run_detail: {e}")))?;

        // 7. Build payload.
        let payload = crate::webhooks::payload::WebhookPayload::build(
            event, &fctx, &run_detail, filter_position, self.cronduit_version,
        );

        // 8. Serialize ONCE into Vec<u8>.
        let body_bytes = serde_json::to_vec(&payload)
            .map_err(|e| WebhookError::DispatchFailed(format!("serialize payload: {e}")))?;

        // 9. Build headers.
        let webhook_id = ulid::Ulid::new().to_string();
        let webhook_ts = chrono::Utc::now().timestamp(); // Unix seconds (D-09)

        let mut req = self
            .client
            .post(&cfg.url)
            .header("content-type", "application/json")  // D-11
            .header("webhook-id", &webhook_id)
            .header("webhook-timestamp", webhook_ts.to_string());

        // 10. Sign IF not unsigned.
        if !cfg.unsigned {
            let secret = cfg.secret.as_ref()
                .expect("validator guarantees secret is Some when unsigned == false");
            let signature = sign_v1(secret, &webhook_id, webhook_ts, &body_bytes);
            req = req.header("webhook-signature", format!("v1,{signature}"));
        }

        // 11. Send.
        let response = req.body(body_bytes).send().await;

        // 12. Classify + record metrics.
        match response {
            Ok(resp) if resp.status().is_success() => {
                metrics::counter!("cronduit_webhook_delivery_sent_total").increment(1);
                tracing::debug!(
                    target: "cronduit.webhooks",
                    run_id = event.run_id, job_name = %event.job_name,
                    url = %cfg.url, status = resp.status().as_u16(),
                    "webhook delivered"
                );
                Ok(())
            }
            Ok(resp) => {
                metrics::counter!("cronduit_webhook_delivery_failed_total").increment(1);
                let status = resp.status().as_u16();
                let body_preview = resp.text().await.unwrap_or_default();
                let truncated: String = body_preview.chars().take(200).collect();
                tracing::warn!(
                    target: "cronduit.webhooks",
                    run_id = event.run_id, job_name = %event.job_name,
                    url = %cfg.url, status, body_preview = %truncated,
                    "webhook non-2xx"
                );
                // Per D-21: do NOT return Err here for Phase 18; the worker only logs.
                // (Phase 20's RetryingDispatcher will surface error variants.)
                Ok(())
            }
            Err(e) => {
                metrics::counter!("cronduit_webhook_delivery_failed_total").increment(1);
                tracing::warn!(
                    target: "cronduit.webhooks",
                    run_id = event.run_id, job_name = %event.job_name,
                    url = %cfg.url, error = %e,
                    "webhook network error"
                );
                Ok(())
            }
        }
    }
}

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
    let bytes = mac.finalize().into_bytes();
    STANDARD.encode(bytes)
}
```

### Pattern 7: Bin-Layer Dispatcher Swap

**What:** `src/cli/run.rs:250-255` currently spawns the worker with `NoopDispatcher`. Phase 18 swaps to `HttpDispatcher` IFF any webhook is configured.

**Current code (Phase 15):**

```rust
// src/cli/run.rs:250-255
let (webhook_tx, webhook_rx) = crate::webhooks::channel();
let webhook_worker_handle = crate::webhooks::spawn_worker(
    webhook_rx,
    std::sync::Arc::new(crate::webhooks::NoopDispatcher),
    cancel.child_token(),
);
```

**Phase 18 wire-up sketch:**

```rust
// Source: src/cli/run.rs — modify around L250-255

// Build the per-job webhook map from the validated config. None = no webhook
// for that job; falls through to dispatcher.deliver early-return.
let webhooks: std::collections::HashMap<i64, crate::config::WebhookConfig> =
    sync_result.jobs.iter()
        .filter_map(|j| {
            // sync_result.jobs are DbJob structs — webhook is on JobConfig pre-sync.
            // Plan author: if webhook is round-tripped through `config_json`, parse
            // it from `j.config_json` here; otherwise pass through cfg.jobs (the
            // pre-sync JobConfig list with webhook still on the struct). See
            // Open Question 1.
            todo!()
        })
        .collect();

let dispatcher: std::sync::Arc<dyn crate::webhooks::WebhookDispatcher> =
    if webhooks.is_empty() {
        // No webhook configured anywhere — keep NoopDispatcher. Avoids
        // building a reqwest::Client for installations that don't use webhooks.
        std::sync::Arc::new(crate::webhooks::NoopDispatcher)
    } else {
        let http = crate::webhooks::HttpDispatcher::new(
            pool.clone(),
            std::sync::Arc::new(webhooks),
        )?;
        std::sync::Arc::new(http)
    };

let (webhook_tx, webhook_rx) = crate::webhooks::channel();
let webhook_worker_handle = crate::webhooks::spawn_worker(
    webhook_rx,
    dispatcher,
    cancel.child_token(),
);
```

### Anti-Patterns to Avoid

- **Pretty-print JSON in the body.** Wastes bytes; HMAC sequencing fragile if any code path re-serializes. Always use `serde_json::to_vec` (compact).
- **Hex-encoded signature.** Spec is explicit on base64 (standard alphabet). `hex::encode(bytes)` is wrong here.
- **URL-safe base64.** `base64::engine::general_purpose::URL_SAFE_NO_PAD` produces `-` and `_` — incompatible with the spec example characters `+` and `/`. Use `STANDARD`.
- **Building a per-request `reqwest::Client`.** Defeats keep-alive; new TCP+TLS handshake per delivery; non-trivial latency for receivers behind TLS termination on slow hardware (homelab Pi). Keep one shared client.
- **`webhook-timestamp` in milliseconds.** Spec says seconds. `chrono::Utc::now().timestamp_millis()` would be wrong; use `.timestamp()`.
- **Mutating the payload after signing.** Once `body_bytes` is built, do not touch it. Send the same `Vec<u8>`.
- **Forgetting Phase 17 D-01 HashMap-determinism rule.** Any validator that builds an error string from a HashMap must `.sort()` the offending keys before formatting. Phase 18's `check_webhook_block_completeness` operates on `Vec<String>` for `states` (already deterministic order from the parser), so the sort is needed only on the invalid filter — see § Pattern 2.
- **Letting `${WEBHOOK_SECRET}` resolve to an empty string at LOAD time.** The interpolation pass currently substitutes empty for missing env vars and pushes a `MissingVar` error (`src/config/interpolate.rs:65-72`). When the secret env var is unset, that error path is the only signal — the config will not load. Phase 18 needs no extra check.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HMAC-SHA256 | a custom HMAC ipad/opad loop | `hmac::Hmac<Sha256>` (RustCrypto) | Side-channel-resistant constant-time impl; tested by RustCrypto WG |
| Base64 encoding | a Rust 2-line "encoder" | `base64 0.22` `STANDARD` engine | Padding edge cases, 6-bit table, etc. — small bug, big consequence |
| URL parsing for `webhook.url` | regex | `url::Url::parse` (already a dep) | Schemes, ports, IDN, percent-encoding |
| ULID generation | a custom epoch-millis prefix + rand | `ulid::Ulid::new()` | Crockford base32, monotonicity, lex-sort guarantees |
| RFC3339 timestamp formatting | string formatting | `chrono::DateTime::to_rfc3339_opts(SecondsFormat::Secs, true)` | Produces `Z` suffix correctly; UTC-aware |
| HTTP retry/timeout/backoff (Phase 20) | hand-coded loops | `reqwest::Client::builder().timeout(...)` + Phase 20's `RetryingDispatcher` wrapper | Per D-21 |
| Mock HTTP server for tests | a homemade tokio listener | `wiremock 0.6` | One-line mock setup; idiomatic axum/reqwest test harness |

**Key insight:** All five of "ID generation, signing, encoding, URL parsing, time formatting" are crypto/parsing surface where hand-rolled is empirically wrong more often than right. The Phase 17 LBL precedent did the same thing — used `regex::Regex` + `url::Url` instead of homegrown.

## Common Pitfalls

### Pitfall A: `reqwest 0.12` → `0.13` Feature-Flag Rename
**What goes wrong:** CONTEXT D-20 specifies `features = ["rustls-tls", "json"]`. That spelling is `reqwest 0.12`. On `reqwest 0.13` (the current line), the feature is named `rustls`, not `rustls-tls`. Naive copy-paste of D-20 produces a `cargo build` error: `feature `rustls-tls` is not declared`.
**Why it happens:** D-20 was written before crate-currency verification.
**How to avoid:** Use `features = ["rustls", "json"]` for `reqwest 0.13`. Annotate the `Cargo.toml` line with a phase-pointer comment so future readers know why this differs from D-20's literal text.
**Warning signs:** `cargo build` fails immediately on adding `reqwest`. Or worse — falls back to default features (`default-tls = rustls` in 0.13, but **`default-tls = native-tls + OpenSSL` in 0.12**). The `just openssl-check` recipe catches the worst case.

### Pitfall B: HMAC Body-Bytes Drift Between Sign and Send
**What goes wrong:** Implementations that compute `serde_json::to_string(&payload)` for the HMAC input and then call `serde_json::to_string(&payload)` again for the request body can produce different bytes if a `BTreeMap`/`HashMap`/non-deterministic source changes ordering between the two calls. Receiver-side HMAC compare fails with no diagnostic clue.
**Why it happens:** Programmer assumes "JSON serialization is deterministic for a given struct." It IS deterministic with `serde_derive` over a struct with all-named fields, but ANY map field (including future schema additions) can re-order between calls.
**How to avoid:** Serialize ONCE to `Vec<u8>`, sign that buffer, then `client.post(url).body(body_bytes)` with the SAME buffer. The 15-field schema in D-06 is all named struct fields (no maps), so today's risk is low — but the discipline future-proofs the code.
**Warning signs:** Receiver-side test reports "signature mismatch" intermittently. Add an integration test that asserts signatures match on `serde_json::to_vec(&payload)` repeated 100 times.

### Pitfall C: Pretty-Print JSON Smuggled In via `serde_json::to_string_pretty`
**What goes wrong:** Compact vs pretty produce different byte streams; pretty adds whitespace bytes that the receiver also has to reproduce when verifying. Standard Webhooks v1 implies compact (the example body has no newlines).
**How to avoid:** Use `serde_json::to_vec(&payload)` (compact by default). Never `to_string_pretty`. Add a unit test asserting body has no `\n`.

### Pitfall D: `webhook-timestamp` in Milliseconds
**What goes wrong:** `chrono::Utc::now().timestamp_millis()` returns a 13-digit number; spec says 10-digit Unix seconds. A receiver doing `time.time() - int(headers['webhook-timestamp']) > 5*60` to check freshness will see ~50 years of "skew" and reject every delivery.
**How to avoid:** `chrono::Utc::now().timestamp()` (seconds, i64). Add a unit test asserting `webhook_timestamp.to_string().len() == 10` for a current timestamp (will need updating in 2286, but that's fine).

### Pitfall E: URL-Safe Base64 Instead of Standard
**What goes wrong:** Standard Webhooks v1 spec example signature contains `/` and `+` characters — that's standard alphabet, NOT URL-safe (which uses `-` and `_`). A `base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(...)` produces a signature receivers will fail to decode (or worse — decode to wrong bytes).
**How to avoid:** Always `base64::engine::general_purpose::STANDARD.encode(...)` (with `=` padding). Add a unit test asserting signature contains no `-` or `_`.

### Pitfall F: RFC3339 Format Drift (`+00:00` vs `Z`)
**What goes wrong:** Default `chrono::DateTime::<Utc>::to_rfc3339()` produces `2026-04-29T10:43:11.123456789+00:00`. Receivers parsing into a strict ISO 8601 lib that expects `Z` suffix may reject. Worse: nanosecond precision in `started_at`/`finished_at` when the `job_runs.start_time` column is RFC3339 `Secs`-precision creates field-shape skew between the payload and DB.
**How to avoid:** Always `dt.to_rfc3339_opts(SecondsFormat::Secs, /*use_z=*/ true)`. Verified at chrono docs.rs: `use_z = true` produces `Z` when offset is UTC. Add a unit test asserting payload `started_at.ends_with('Z')` and matches `^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$`.

### Pitfall G: HashMap Iteration Non-Determinism (Phase 17 D-01 Mirror)
**What goes wrong:** Any validator that builds an error string from a HashMap or HashSet without sorting can produce different error text on repeated runs. Tests grep on substrings; flake on Linux + macOS + arm64 because hash seeds differ.
**How to avoid:** Sort offending keys before format. Phase 18's `check_webhook_block_completeness` operates on `Vec<String>` (already deterministic from TOML parse), but the `invalid` filter in `states` validation iterates a Vec and produces a Vec — order is preserved. Still defensively call `.sort()` per the LBL precedent. (See `src/config/validate.rs:181, 294, 351, 355` for the existing sort-before-format calls.)

### Pitfall H: `${WEBHOOK_SECRET}` Empty-String After Substitution
**What goes wrong:** If the env var is set but empty (`WEBHOOK_SECRET=`), the interpolation pass substitutes the empty string. The TOML still parses (`secret = ""`), validators don't object (`secret.is_some()` is true), and the dispatcher signs with an empty key — receivers reject every delivery.
**How to avoid:** Two layers of defense:
1. **At interpolation:** the existing `interpolate.rs` treats empty-string env vars same as set-to-empty (see `std::env::var` semantics — `Ok("")`). No change needed.
2. **At validation:** `check_webhook_block_completeness` checks `secret.expose_secret().is_empty()` and rejects with a clear message. (NOTE: Phase 18 should add this guard. Add a Pitfall H unit test.)

### Pitfall I: SQL Bind-Parameter Expansion for Variable-Length `IN (...)` List
**What goes wrong:** SQLite's `IN (?)` doesn't bind a slice — each `?` is a single value. The filter-position query needs `WHERE status IN (?, ?, ?, ?, ?, ?)` with one `?` per allowed state. Hard-coding 6 placeholders (one per VALID_WEBHOOK_STATES variant) works because the validator pre-pads the list, but feels fragile.
**How to avoid:** Either (a) pre-pad to a fixed 6 placeholders by repeating an entry (e.g., `("failed", "timeout", "timeout", "timeout", "timeout", "timeout")` — duplicates collapse in SQL `IN`), or (b) build the placeholder string dynamically. Option (a) is simpler and what most existing cronduit queries do. Confirm by reading `src/db/queries.rs::get_failure_context` — it hardcodes `('failed', 'timeout', 'error')` as a SQL literal because the streak-defining set is fixed. Phase 18's filter set is operator-supplied so we cannot inline; option (a) with bind padding is recommended.

### Pitfall J: Channel Back-Pressure on Slow Webhook Receivers
**What goes wrong:** Phase 15's worker is serial — one HTTP request at a time, 10s timeout each. If the receiver is at the timeout limit consistently, deliveries flow at 6/min. A noisy job firing every minute can saturate the bounded mpsc(1024) channel after ~3 hours of receiver outage.
**How to avoid (Phase 18 scope):** Confirm the bounded channel is unchanged from Phase 15 (`CHANNEL_CAPACITY = 1024` in `src/webhooks/worker.rs:21`). The `try_send` + drop-on-full path at `src/scheduler/run.rs:440-454` is the safety valve — `cronduit_webhook_delivery_dropped_total` increments and the scheduler is never blocked. Phase 18 introduces no change here.
**Phase 20 follow-up:** WH-10's drain semantics will affect this on shutdown.

### Pitfall K: HTTP Error Classification (5xx vs network vs 4xx)
**What goes wrong:** Phase 18 is single-attempt — every non-2xx and every network error increments `cronduit_webhook_delivery_failed_total` (D-17). Phase 20 will distinguish "permanent receiver-config error (4xx)" from "transient (5xx, network)" for retry decisioning.
**How to avoid in Phase 18:** Treat ALL non-`is_success()` (i.e., not 2xx) and ALL `reqwest::Error` outcomes as "failed." Don't introduce 4xx-vs-5xx logic. The metric is unlabeled per D-17 so no cardinality decisions to make. Document this as the Phase 18 posture in the SUMMARY for Phase 20's planner.

## Runtime State Inventory

> Phase 18 is a code/config-only addition. No rename, refactor, or migration. **No runtime state inventory needed.**

Specifically:
- Stored data: nothing renamed; new column NOT added to `job_runs` (Phase 20 owns `webhook_deliveries`).
- Live service config: no external service registration changes.
- OS-registered state: none.
- Secrets/env vars: NEW operator-side env var `${WEBHOOK_SECRET}` (per-job; operator-named). No cronduit-side rename. Operator sets this in the Docker compose env or shell env BEFORE starting cronduit.
- Build artifacts: no rename. New crates added (`reqwest`/`hmac`/`base64`/`ulid` + `wiremock` dev). Cargo lock will absorb them on the first build; no special handling.

## Code Examples

### Compact-JSON Serialization with Stable Field Order

```rust
// Source: src/webhooks/payload.rs (NEW)
use serde::Serialize;
use chrono::{DateTime, SecondsFormat, Utc};

#[derive(Debug, Serialize)]
pub struct WebhookPayload<'a> {
    pub payload_version: &'static str,    // "v1" — D-08
    pub event_type: &'static str,         // "run_finalized" — D-06
    pub run_id: i64,
    pub job_id: i64,
    pub job_name: &'a str,
    pub status: &'a str,
    pub exit_code: Option<i32>,
    pub started_at: String,               // RFC3339 Z-suffix
    pub finished_at: String,
    pub duration_ms: i64,
    pub streak_position: i64,             // filter-matching stream position (D-07)
    pub consecutive_failures: i64,        // unified P16 count
    pub image_digest: Option<String>,     // null for non-docker (D-07)
    pub config_hash: Option<String>,      // null for pre-v1.2 rows
    pub tags: Vec<String>,                // [] until Phase 22 (D-07)
    pub cronduit_version: &'static str,   // env!("CARGO_PKG_VERSION")
}

impl<'a> WebhookPayload<'a> {
    pub fn build(
        event: &'a crate::webhooks::RunFinalized,
        fctx: &crate::db::queries::FailureContext,
        run: &crate::db::queries::DbRunDetail,
        filter_position: i64,
        cronduit_version: &'static str,
    ) -> Self {
        let duration_ms = (event.finished_at - event.started_at).num_milliseconds();
        Self {
            payload_version: "v1",
            event_type: "run_finalized",
            run_id: event.run_id,
            job_id: event.job_id,
            job_name: &event.job_name,
            status: &event.status,
            exit_code: event.exit_code,
            started_at: event.started_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            finished_at: event.finished_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            duration_ms,
            streak_position: filter_position,
            consecutive_failures: fctx.consecutive_failures,
            image_digest: run.image_digest.clone(),
            config_hash: run.config_hash.clone(),
            tags: vec![],  // Phase 22 — TAG-01 lights up real values; schema-stable.
            cronduit_version,
        }
    }
}
```

### Verifying Payload Determinism (Unit Test Sketch)

```rust
// Source: src/webhooks/payload.rs (tests module)
#[test]
fn payload_serializes_deterministically_to_compact_json() {
    let event = RunFinalized { /* fixture */ };
    let fctx = FailureContext { /* fixture */ };
    let run = DbRunDetail { /* fixture */ };
    let payload = WebhookPayload::build(&event, &fctx, &run, 1, "1.2.0");
    let bytes_a = serde_json::to_vec(&payload).unwrap();
    let bytes_b = serde_json::to_vec(&payload).unwrap();
    assert_eq!(bytes_a, bytes_b, "two serializations must produce identical bytes");
    assert!(!bytes_a.contains(&b'\n'), "compact JSON must have no newlines (Pitfall C)");
    let s = std::str::from_utf8(&bytes_a).unwrap();
    assert!(s.contains("\"payload_version\":\"v1\""));
    assert!(s.contains("\"event_type\":\"run_finalized\""));
    // Order-stability check — fields serialize in struct-declaration order
    let pos_version = s.find("payload_version").unwrap();
    let pos_event   = s.find("event_type").unwrap();
    assert!(pos_version < pos_event, "payload_version must appear before event_type");
}
```

### Standard Webhooks v1 Wire Contract (Verified Against Spec)

```text
POST /webhook/path HTTP/1.1
Host: hook.example.com
content-type: application/json
webhook-id: 01HZAFY0V1F1BS1F2H8GV4XG3R
webhook-timestamp: 1761744191
webhook-signature: v1,K5oZfzN95Z9UVu1EsfQmfVNQhnkZ2pj9o9NDN/H/pI4=

{"payload_version":"v1","event_type":"run_finalized","run_id":42, ... }
```

**Spec citations (verified 2026-04-29 from upstream raw markdown):**
- Headers: `webhook-id`, `webhook-timestamp`, `webhook-signature` — all lowercase, hyphenated.
- Signing string: `"msg_id.timestamp.payload"` — literal `.` (full-stop) separators.
- Signature value: `"v1,<base64>"` — `v1` followed by a literal comma. Multiple signatures space-delimited (key rotation).
- Base64: standard alphabet (the spec example uses `/` and `+`); padding not explicitly addressed but the spec example IS padded with `=`. Use `STANDARD` (with padding).
- `webhook-timestamp`: integer Unix timestamp, seconds since epoch.
- `Content-Type`: not specified by spec; `application/json` is the de facto + project lock per D-11.
- **Unsigned deliveries: NOT addressed in the spec.** Cronduit's `webhook.unsigned = true` is an extension; the dispatcher omits the `webhook-signature` header entirely while still emitting `webhook-id` and `webhook-timestamp`. Document in the cronduit README as a deliberate divergence for receivers like Slack/Discord that don't HMAC-verify.

### `fire_every` Modular Math Verification

D-16 specifies:
- `fire_every = 0` → always fire
- `fire_every = 1` → `filter_position == 1`
- `fire_every = N` (N>1) → `filter_position % N == 1`

Walk-through:

| `fire_every` | `filter_position` | Calculation | Fires? |
|--------------|-------------------|-------------|--------|
| 0 | any | special-case `true` | YES |
| 1 | 1 | `1 % 1 == 0` (NOT 1) — but special-cased to `position == 1` | YES |
| 1 | 2 | special-case: `2 == 1`? false | no |
| 3 | 1 | `1 % 3 == 1` | YES |
| 3 | 2 | `2 % 3 == 2` | no |
| 3 | 3 | `3 % 3 == 0` | no |
| 3 | 4 | `4 % 3 == 1` | YES (1, 4, 7, ...) |
| 3 | 7 | `7 % 3 == 1` | YES |

**Edge case for `fire_every = 1`:** D-16 says "fire when filter_position == 1." The modular form `position % 1 == 1` does NOT hold (`x % 1 == 0` always). The dispatcher MUST special-case `fire_every == 1` to compare equality. The sketch in § Pattern 6 step 4 does this.

**Edge case for negative `fire_every`:** The validator rejects negatives at LOAD (D-04), so the dispatcher cannot see them. The defensive `_ => false` arm is documentation.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `reqwest 0.12` `rustls-tls` feature | `reqwest 0.13` `rustls` feature | 2025-12-30 (0.13.0) | Phase 18 must use `rustls` (not `rustls-tls`); see Pitfall A |
| `reqwest 0.12` default-features included `default-tls = native-tls (OpenSSL)` | `reqwest 0.13` default-tls = `rustls` | 2025-12-30 | Even default features are now openssl-free in 0.13; project still pins `default-features = false` for clarity |
| `hmac 0.12` + `sha2 0.10` | `hmac 0.13` + `sha2 0.11` | hmac 0.13.0 (2025-12-30); sha2 0.11.0 (2026-03-25) | Both are RustCrypto generation 11 — paired versions; cronduit's `sha2 = "0.11"` already direct dep so just bump hmac fresh |
| ULID monotonic gen via custom epoch | `ulid 1.2` `Ulid::new()` | stable since 1.x; no recent break | None — current approach is fine |

**Deprecated/outdated (not used in Phase 18):**
- `serde-yaml` (project doesn't use YAML)
- `tokio-cron-scheduler` (we hand-roll on tokio)
- `shiplift` (we use bollard)
- `chrono::DateTime::to_rfc3339()` without explicit options — produces `+00:00` not `Z`; Pitfall F.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust `#[test]` + `cargo nextest run --all-features --profile ci` |
| Config file | none (project-level — `nextest.toml` if present) |
| Quick run command | `cargo test -p cronduit --lib webhooks` |
| Full suite command | `just nextest` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| WH-01 | TOML `webhook` block parses correctly per-job and `[defaults]` | unit | `cargo test -p cronduit webhook_config_parses` | ❌ Wave 0 |
| WH-01 | `apply_defaults` fills `webhook` from `[defaults]` when per-job is absent | unit | `cargo test -p cronduit apply_defaults_fills_webhook_from_defaults` | ❌ Wave 0 |
| WH-01 | `use_defaults = false` discards `[defaults].webhook` | unit | `cargo test -p cronduit apply_defaults_use_defaults_false_discards_webhook` | ❌ Wave 0 |
| WH-01 | `check_webhook_url` rejects unparsable URL | unit | `cargo test -p cronduit check_webhook_url_rejects_garbage` | ❌ Wave 0 |
| WH-01 | `check_webhook_url` rejects `ftp://` etc. | unit | `cargo test -p cronduit check_webhook_url_rejects_non_http_scheme` | ❌ Wave 0 |
| WH-01 | `check_webhook_block_completeness` rejects `secret` AND `unsigned=true` | unit | `cargo test -p cronduit check_webhook_block_rejects_both_secret_and_unsigned` | ❌ Wave 0 |
| WH-01 | `check_webhook_block_completeness` rejects neither `secret` nor `unsigned=true` | unit | `cargo test -p cronduit check_webhook_block_rejects_neither_secret_nor_unsigned` | ❌ Wave 0 |
| WH-01 | `check_webhook_block_completeness` rejects `states = []` | unit | `cargo test -p cronduit check_webhook_block_rejects_empty_states` | ❌ Wave 0 |
| WH-01 | `check_webhook_block_completeness` rejects unknown state value | unit | `cargo test -p cronduit check_webhook_block_rejects_unknown_state` | ❌ Wave 0 |
| WH-01 | `check_webhook_block_completeness` rejects negative `fire_every` | unit | `cargo test -p cronduit check_webhook_block_rejects_negative_fire_every` | ❌ Wave 0 |
| WH-01 | `${WEBHOOK_SECRET}` interpolation works in `secret` field | unit | `cargo test -p cronduit webhook_secret_interpolates_env_var` | ❌ Wave 0 |
| WH-01 (Pitfall H) | empty-string secret rejected at validate | unit | `cargo test -p cronduit check_webhook_block_rejects_empty_secret` | ❌ Wave 0 |
| WH-03 | HMAC-SHA256 over `webhook-id.webhook-timestamp.body` matches a known-good fixture | unit | `cargo test -p cronduit sign_v1_matches_fixture` | ❌ Wave 0 |
| WH-03 | base64 STANDARD encoding (not URL-safe) | unit | `cargo test -p cronduit signature_uses_standard_base64_alphabet` | ❌ Wave 0 |
| WH-03 | `webhook-signature` value format is `v1,<base64>` | unit | `cargo test -p cronduit signature_value_is_v1_comma_b64` | ❌ Wave 0 |
| WH-03 | `webhook-id` is 26-char ULID | unit | `cargo test -p cronduit webhook_id_is_26char_ulid` | ❌ Wave 0 |
| WH-03 | `webhook-timestamp` is 10-digit Unix seconds | unit | `cargo test -p cronduit webhook_timestamp_is_10digit_seconds` | ❌ Wave 0 |
| WH-03 (Pitfall F) | `started_at` / `finished_at` use `Z` suffix not `+00:00` | unit | `cargo test -p cronduit timestamps_use_z_suffix` | ❌ Wave 0 |
| WH-03 | `unsigned = true` omits the `webhook-signature` header | integration | `cargo test --test v12_webhook_unsigned_omits_signature` | ❌ Wave 0 |
| WH-03 | end-to-end delivery hits a wiremock receiver with the correct headers | integration | `cargo test --test v12_webhook_delivery_e2e` | ❌ Wave 0 |
| WH-06 | filter-matching position counts back from current run, stops at first non-match | unit | `cargo test -p cronduit filter_position_basic_streak` | ❌ Wave 0 |
| WH-06 | filter-matching position stops at first `success` per D-15 | unit | `cargo test -p cronduit filter_position_stops_at_success` | ❌ Wave 0 |
| WH-06 | `failed → timeout` with `states = ["timeout"]` yields filter_position = 1 (D-13) | unit | `cargo test -p cronduit filter_position_d13_scenario` | ❌ Wave 0 |
| WH-06 | `fire_every = 0` always fires | unit | `cargo test -p cronduit fire_every_zero_always_fires` | ❌ Wave 0 |
| WH-06 | `fire_every = 1` fires only on filter_position == 1 | unit | `cargo test -p cronduit fire_every_one_first_of_stream` | ❌ Wave 0 |
| WH-06 | `fire_every = 3` fires on positions 1, 4, 7 | unit | `cargo test -p cronduit fire_every_three_modular` | ❌ Wave 0 |
| WH-06 | EXPLAIN PLAN of filter-position SQL hits `idx_job_runs_job_id_start` (SQLite + Postgres) | integration | `cargo test --test v12_webhook_filter_position_explain` | ❌ Wave 0 |
| WH-06 | success runs do NOT fire when `success ∉ states` | integration | `cargo test --test v12_webhook_state_filter_excludes_success` | ❌ Wave 0 |
| WH-09 | payload contains all 15 fields (struct-level test) | unit | `cargo test -p cronduit payload_contains_all_15_fields` | ❌ Wave 0 |
| WH-09 | `image_digest` is `null` for non-docker job, populated for docker | unit | `cargo test -p cronduit payload_image_digest_null_on_non_docker` | ❌ Wave 0 |
| WH-09 | `tags` is `[]` until Phase 22 | unit | `cargo test -p cronduit payload_tags_empty_array_until_p22` | ❌ Wave 0 |
| WH-09 (Pitfall B/C) | repeated serialization yields identical bytes; no newlines | unit | `cargo test -p cronduit payload_serializes_deterministically_to_compact_json` | ❌ Wave 0 |
| WH-09 | `cronduit_version` matches `env!("CARGO_PKG_VERSION")` | unit | `cargo test -p cronduit payload_cronduit_version_from_env_macro` | ❌ Wave 0 |
| Posture | non-2xx response increments `cronduit_webhook_delivery_failed_total` | integration | `cargo test --test v12_webhook_failed_metric` | ❌ Wave 0 |
| Posture | network error increments `cronduit_webhook_delivery_failed_total` | integration | `cargo test --test v12_webhook_network_error_metric` | ❌ Wave 0 |
| Posture | 2xx response increments `cronduit_webhook_delivery_sent_total` | integration | `cargo test --test v12_webhook_success_metric` | ❌ Wave 0 |
| Posture | metrics families described from boot via `/metrics` (HELP/TYPE lines) | integration | extend existing `tests/metrics_endpoint.rs::metrics_families_described_from_boot` | ✅ extend |
| Posture | scheduler not blocked by 60s-stalled receiver | integration (regression) | covered by existing Phase 15 `tests/v12_webhook_scheduler_unblocked.rs` | ✅ pre-existing |

### Sampling Rate

- **Per task commit:** `cargo test -p cronduit --lib webhooks` (~5s; runs all webhook unit tests)
- **Per wave merge:** `just nextest` (full suite ~3min)
- **Phase gate:** `just ci` (fmt-check + clippy + openssl-check + nextest + schema-diff + image) all green before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `tests/v12_webhook_delivery_e2e.rs` — wiremock-based round-trip; verifies headers, body bytes, signature
- [ ] `tests/v12_webhook_unsigned_omits_signature.rs` — `unsigned = true` path
- [ ] `tests/v12_webhook_filter_position_explain.rs` — SQLite + Postgres EXPLAIN PLAN asserting `idx_job_runs_job_id_start` hit (mirror `tests/v12_fctx_explain.rs`)
- [ ] `tests/v12_webhook_state_filter_excludes_success.rs` — full pipeline state-filter exclusion
- [ ] `tests/v12_webhook_failed_metric.rs`, `tests/v12_webhook_network_error_metric.rs`, `tests/v12_webhook_success_metric.rs` — metric assertion via `/metrics` text scraping
- [ ] `wiremock = "0.6"` dev-dependency added to `Cargo.toml`
- [ ] Existing `tests/metrics_endpoint.rs::metrics_families_described_from_boot` extended with HELP/TYPE asserts for the two NEW counters (`cronduit_webhook_delivery_sent_total`, `cronduit_webhook_delivery_failed_total`) — mirror Phase 15's extension for the dropped counter.
- [ ] No framework install needed — `cargo test`/`cargo nextest` already wired in `justfile`.

### Manual / UAT (just-recipe-only per project memory)

Per `feedback_uat_use_just_commands.md`, every UAT step must reference a `just` recipe. Existing recipes the planner can reuse:

- `just dev` — local cronduit run with example config
- `just metrics-check` — scrape `/metrics`
- `just check-config <PATH>` — validate a TOML file

NEW recipes the planner SHOULD add for Phase 18 UAT:

- `just uat-webhook-mock` — start a small mock HTTP receiver (e.g., a 5-line python `-m http.server` wrapper or a tiny rust binary) on `127.0.0.1:9999` that logs incoming requests + headers + body to stdout
- `just uat-webhook-fire <JOB_NAME>` — trigger a "Run Now" against the named job (exposes `POST /api/jobs/<name>/run-now`)
- `just uat-webhook-verify` — print last 5 lines of the mock receiver's log (verifies headers + body presence; maintainer hand-validates HMAC)

The HUMAN-UAT.md document for Phase 18 should reference ONLY these recipes — never raw `curl`.

## Security Domain

> `security_enforcement` is enabled (project default — CLAUDE.md security posture is explicit). Phase 18 introduces NEW threat surface: outbound HTTP from cronduit. THREAT_MODEL.md Threat Model 5 (Webhook Outbound) is owned by Phase 20 (WH-08), but Phase 18 establishes the surface.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V2 Authentication | yes (cronduit → receiver) | HMAC-SHA256 with operator-supplied per-receiver secret |
| V3 Session Management | no | each delivery is independent; no session state |
| V4 Access Control | yes | `webhook.url` is operator-controlled; SSRF accepted-risk per WH-08 (deferred to v1.3) |
| V5 Input Validation | yes | `check_webhook_url` (URL parse + scheme), `check_webhook_block_completeness` (state enum + xor) |
| V6 Cryptography | yes | RustCrypto `hmac::Hmac<Sha256>` — never hand-roll; constant-time compare on RECEIVER side (Phase 19 / WH-04 ships examples) |
| V7 Error Handling | yes | error messages do NOT include `secret` value (SecretString prevents Debug/Display leak); HTTP error body truncated to 200 bytes in WARN log |
| V8 Data Protection | yes | `webhook.secret` wrapped in `SecretString`; never logged at any level |
| V9 Communications | yes | rustls-only; HTTPS not strictly enforced in Phase 18 (Phase 20 / WH-07 adds the loopback/RFC1918 exception); `cargo tree -i openssl-sys` empty |
| V11 Business Logic | yes (anti-DoS) | bounded mpsc(1024) drops on full; serial worker; 10s per-attempt timeout |

### Known Threat Patterns for cronduit-outbound-HTTP

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Receiver-side replay | Tampering / Repudiation | `webhook-id` ULID enables receiver-side idempotency dedup; `webhook-timestamp` allows freshness check (receiver responsibility) |
| Receiver impersonates webhook | Spoofing | HMAC-SHA256 over `id.timestamp.body`; receiver verifies signature with shared secret |
| Cronduit signs with empty secret | Spoofing | Pitfall H — validator rejects empty-string secret at LOAD |
| Slow-receiver DoS on cronduit | Denial of Service | 10s per-attempt timeout; serial worker; bounded mpsc(1024) drops; `cronduit_webhook_delivery_dropped_total` metric |
| Operator-configured SSRF | Information Disclosure | **Accepted risk for v1.2 per WH-08** — documented in THREAT_MODEL.md Threat Model 5 (Phase 20 deliverable); v1.3 candidate for allow/blocklist |
| Receiver-side timing attack on HMAC compare | Tampering | **Receiver-side concern**, not cronduit's — Phase 19 receiver examples ship with `hmac.compare_digest` (Python), `hmac.Equal` (Go), `crypto.timingSafeEqual` (Node) |
| Plaintext secret leakage in logs | Information Disclosure | `SecretString` Debug/Display impl scrubs value; reqwest doesn't log auth headers; verify by grepping logs for `webhook.secret` value in tests |

## Environment Availability

> Phase 18 is purely additive Rust code/config. No new external runtime dependencies beyond crates added via `cargo`.

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Rust toolchain (stable) | build | ✓ (project lock 1.94.1+) | 1.94.1+ | — |
| `cargo` registry network | crate fetch | ✓ (CI + local) | n/a | — |
| Docker daemon | testcontainers integration tests (`#[ignore]` or feature-gated) | varies | varies | tests fall through `#[ignore]`/skip |
| HTTPS receiver for UAT | maintainer's UAT step (`just uat-webhook-mock`) | provided by NEW just recipe | n/a | — |

**No blockers.** All Phase 18 dependencies install via `cargo build`.

## Assumptions Log

> Claims tagged `[ASSUMED]` in this research that the planner / discuss-phase should confirm before locking. **Empty unless flagged below.**

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `aws-lc-rs` (reqwest 0.13's default rustls provider) does not pull `openssl-sys`. | Crate Currency, openssl audit | If wrong, `just openssl-check` fails — caught immediately in CI; recoverable by switching to `rustls-no-provider` + `rustls/ring` feature combo. **Verified via `crates.io/api/v1/crates/aws-lc-rs/dependencies` (zero normal deps).** Downgrade to `[VERIFIED]`. |
| A2 | `wiremock 0.6` is the right pick for the Phase 18 integration test mock. | Standard Stack > Alternatives | If `httpmock` proves easier in practice, planner can swap; both are async-friendly. Low risk. |
| A3 | The dispatcher reads webhook config from a per-job in-memory map handed in at construction time, NOT from the DB `config_json` round-trip. | HttpDispatcher Construction sketch | Avoids per-delivery DB read for static config. If reload semantics later require live updates (Phase 20), the map is wrapped in `Arc<RwLock<HashMap<i64, WebhookConfig>>>`. See Open Question 1. |
| A4 | Filter-position SQL query can use 6 hardcoded bind placeholders (one per VALID_WEBHOOK_STATES variant), pre-padded with the operator's actual `states` set repeated to fill empty slots. | Filter-Matching Stream Position | If sqlx/SQLite refuses duplicate-IN-arg pattern, fall back to dynamic placeholder string construction (still safe: states are validated against a closed enum). Low risk. |
| A5 | Filter-position query inherits the existing `idx_job_runs_job_id_start` index (built in Phase 16); no new index needed. | Filter-Matching Stream Position | Confirmed by `src/db/queries.rs:670-673` index name. **Verified.** |
| A6 | `serde_json::to_vec` over a struct with all named fields produces identical bytes on repeated calls. | Pitfall B mitigation | Verified by `serde` documentation + struct field-order = serialization-order. **Verified.** |

**Net assumptions needing user confirmation:** A2, A3 (planner can confirm during `/gsd-plan-phase`).

## Open Questions

1. **How does the dispatcher resolve a job's `webhook` config at delivery time?** Two viable paths:
   - **Option A (recommended):** Pass an `Arc<HashMap<i64, WebhookConfig>>` to `HttpDispatcher::new` at startup. Bin layer builds the map from the validated `Config`. Reload (SIGHUP) is Phase 20 territory; Phase 18 ships static config only.
   - **Option B:** Round-trip `webhook` through `serialize_config_json` and read it from `job_runs.config_json` at delivery time. Costs an extra DB read per delivery; brings webhook config into the parity-table at `src/config/defaults.rs:63-71`.

   **Recommendation:** Option A. Phase 18's webhook config is static (no live update). The map fits comfortably in memory (one entry per job). Avoids parity-table churn and per-delivery DB cost.

   **Implication for the planner:** The plumbing of webhook config STOPS at the bin layer; it never reaches `DockerJobConfig` / `serialize_config_json`. The Phase 17 5-layer parity invariant does NOT apply to `webhook`.

2. **Where does `DbRunDetail` come from at delivery time?** The dispatcher needs `image_digest` and `config_hash` for the payload. Phase 16 added these as columns on `job_runs`. Options:
   - Add a small helper `get_run_metadata(pool, run_id) -> (Option<String>, Option<String>)` that returns just `(image_digest, config_hash)` — minimal SQL, deterministic.
   - Reuse the existing `get_run_detail(pool, run_id)` function (returns full `DbRunDetail`) — simpler, slight over-fetch but already-tested code.

   **Recommendation:** Reuse `get_run_detail`. Over-fetch is a few extra columns at a single round-trip; not a hot path.

3. **Should empty-string secret rejection live in interpolation or in validation?** The interpolation pass already pushes a `MissingVar` error for unset env vars, but `${WEBHOOK_SECRET}=""` (set-but-empty) is a different case. Recommendation: add the empty-string check in `check_webhook_block_completeness` (Pitfall H), not in `interpolate.rs`.

4. **Should the dispatcher read webhook config from `JobConfig` (pre-sync) or `DbJob` (post-sync)?** The bin-layer build of the `HashMap<i64, WebhookConfig>` happens AFTER `sync_config_to_db` (so we have `i64` job IDs from the DB). The webhook config still lives on `JobConfig` (`cfg.jobs[i].webhook`); we map it by name → DB `id`. Plan author should choose a clean iteration pattern — `cfg.jobs.iter().zip(sync_result.jobs.iter()).filter_map(...)` works.

## Sources

### Primary (HIGH confidence)
- **Standard Webhooks v1 spec** (raw markdown, 2026-04-29) — wire format, headers, signing string, signature value format. Quoted directly into § Standard Webhooks v1 Wire Contract.
- **crates.io API queries (2026-04-29)** — verified versions: reqwest 0.13.3, hmac 0.13.0, sha2 0.11.0, base64 0.22.1, ulid 1.2.1, uuid 1.23.1, wiremock 0.6.5, httpmock 0.8.3
- **`https://crates.io/api/v1/crates/reqwest/0.13.3`** — feature flag list; confirmed `rustls` feature replaced `rustls-tls` in 0.13
- **`https://crates.io/api/v1/crates/aws-lc-rs/dependencies`** — confirmed zero normal deps (no openssl-sys)
- **`https://docs.rs/ulid/latest/ulid/struct.Ulid.html`** — `Ulid::new()` API, 26-char Crockford-base32, lex-sort guarantee
- **`https://docs.rs/chrono/latest/chrono/struct.DateTime.html`** — `to_rfc3339_opts(SecondsFormat::Secs, true)` produces `Z` suffix when offset is UTC
- **Cronduit source code** (verified live):
  - `src/webhooks/{mod,dispatcher,event,worker}.rs` — Phase 15 surface
  - `src/scheduler/run.rs:418-460` — `try_send` emit point at finalize_run step 7d
  - `src/db/queries.rs:626-720` — `FailureContext` + `get_failure_context`
  - `src/config/{mod,defaults,validate,interpolate}.rs` — Phase 17 LBL pattern
  - `src/cli/run.rs:240-289` — bin-layer worker spawn point
  - `src/telemetry.rs:107-135` — eager-describe + zero-baseline pattern
  - `Cargo.toml` — current dep versions

### Secondary (MEDIUM confidence)
- Phase 15 SUMMARY (`.planning/phases/15-foundation-preamble/15-03-SUMMARY.md`) — webhook scaffold decisions
- Phase 17 verification gap closure (`.planning/phases/17-custom-docker-labels-seed-001/17-VERIFICATION-GAP-CLOSURE.md`) — whole-file textual interpolation truth
- Phase 16 CONTEXT — streak helper contract

### Tertiary (LOW confidence)
- None. All Phase 18 claims are either spec-verified or codebase-verified.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — every version verified against crates.io 2026-04-29
- Architecture: HIGH — Phase 15 / 17 patterns directly applicable; trait seam already locked
- Wire format: HIGH — Standard Webhooks v1 spec fetched and quoted directly
- Filter-position algorithm: HIGH — single SQL query mirrors Phase 16's `get_failure_context` shape
- Pitfalls: HIGH — A through K each cite either spec, source code, or verified API behavior
- Validation architecture: HIGH — test-file naming follows `tests/v12_*.rs` convention; 30+ named tests fit cleanly into the existing `cargo nextest` flow
- Reqwest 0.13 feature rename: HIGH — verified via crates.io feature listing; explicit Pitfall A documents the deviation from CONTEXT D-20

**Research date:** 2026-04-29
**Valid until:** 2026-05-29 (30 days; reqwest/hmac/sha2 line is stable; check before promoting Phase 20 / rc.1)

## RESEARCH COMPLETE
