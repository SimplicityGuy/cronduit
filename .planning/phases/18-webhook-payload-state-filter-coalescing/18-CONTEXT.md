# Phase 18: Webhook Payload + State-Filter + Coalescing - Context

**Gathered:** 2026-04-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Operators can configure per-job (and `[defaults]`) webhook URLs with a state-filter list and coalescing override. On terminal `RunFinalized` events, the dispatcher fires Standard Webhooks v1-conformant deliveries (signed with HMAC-SHA256 over `webhook-id.webhook-timestamp.payload`) carrying the locked v1.2.0 payload schema. Coalescing is edge-triggered against the operator's filter-matching stream — by default deliveries fire only on the FIRST run that matches the filter since a non-matching run.

**In scope (Phase 18):**
- TOML config: `[[jobs]] webhook = { url, states, secret, unsigned, fire_every }` and `[defaults] webhook = { ... }` with `use_defaults = false` disable
- LOAD-time validators for the `webhook` block (URL well-formed, `states` valid, `secret` xor `unsigned`)
- `src/webhooks/payload.rs` — JSON wire-format encoder for the locked v1 schema (15 fields)
- `src/webhooks/dispatcher.rs::HttpDispatcher` — implements `WebhookDispatcher`, builds payload from `RunFinalized` + `get_failure_context`, signs, sends one HTTP attempt
- Standard Webhooks v1 headers (`webhook-id` ULID, `webhook-timestamp` Unix-epoch-seconds, `webhook-signature: v1,<base64>`)
- Coalescing: filter-matching stream position computed Rust-side; fire when position == 1, or every N for `fire_every = N`, or always for `fire_every = 0`
- Receivers: `Phase 19 (WH-04)` ships Python/Go/Node receiver examples — NOT in scope here

**Out of scope (deferred to other phases):**
- HMAC algorithm-agility / multi-secret rotation cronduit-side — locked at SHA-256 only (WH-04, Phase 19)
- Receiver examples (Python/Go/Node + constant-time compare guidance) — Phase 19 (WH-04)
- 3-attempt retry with full-jitter exponential backoff — Phase 20 (WH-05)
- `webhook_deliveries` dead-letter table — Phase 20 (WH-05)
- HTTPS-required URL validation for non-loopback/non-RFC1918 — Phase 20 (WH-07)
- 30-second drain on shutdown — Phase 20 (depends on Phase 18 surface; lands as posture)
- SSRF allow/block-list filter — explicit accepted-risk per WH-08; deferred to v1.3
- THREAT_MODEL.md Threat Model 5 (Webhook Outbound) — Phase 20 (WH-08)
- Per-attempt timeout configurability — Phase 20 if needed (Phase 18 hard-codes 10s)
- Concurrent delivery semaphore — Phase 20+ if load demands; Phase 18 stays serial inside the worker task

</domain>

<decisions>
## Implementation Decisions

### Configuration (WH-01)
- **D-01:** Config shape parallels SEED-001 / LBL-01..05: `webhook` block lives on `[[jobs]]` AND in `[defaults]`. Per-job override + `use_defaults = false` disable matches existing precedent. Single block (not multiple webhook configs per job) — one URL per job.
- **D-02:** Field schema:
  ```toml
  webhook = {
    url      = "https://hook.example.com/...",   # required
    states   = ["failed", "timeout"],            # default if omitted
    secret   = "${WEBHOOK_SECRET}",              # required UNLESS unsigned = true
    unsigned = false,                            # opt-in to skip signing (default false)
    fire_every = 1                               # 1 = first-of-stream (default); 0 = every; N = every Nth match
  }
  ```
- **D-03:** `secret` interpolates `${ENV_VAR}` via the existing whole-file textual interpolation pass (per Phase 17's CR-01 truth-up). The resolved value is wrapped in `SecretString` at config-load to prevent accidental Debug/Display leakage. Match the existing `command`/`image` env-var pattern.
- **D-04:** Validators (LOAD-time, cf. Phase 17 LBL pattern):
  - `webhook.url` — must parse as `url::Url`; scheme `http`/`https` only. (Strict HTTPS-for-public-hosts is Phase 20 / WH-07; Phase 18 only enforces parseability.)
  - `webhook.states` — every entry must be one of `success | failed | timeout | stopped | cancelled | error` (the canonical `RunFinalized.status` set). Unknown values rejected at LOAD with the offending value + valid list. **Empty array `states = []` is rejected** ("use absence of `webhook` block to disable; `states = []` is meaningless"). Default when omitted: `["failed", "timeout"]`.
  - `webhook.secret` xor `webhook.unsigned = true` — exactly one must be present. If `webhook` block exists but neither, reject at LOAD with a remediation message naming both fixes.
  - `webhook.fire_every` — must be ≥ 0 (i64). 0/1/N all valid; negative rejected.
- **D-05:** `webhook.unsigned = true` causes the dispatcher to omit the `webhook-signature` header entirely (Standard Webhooks v1 conventional behavior). Operator's explicit opt-in for receivers like Slack/Discord that don't verify.

### Payload Schema (WH-09)
- **D-06:** Locked 15 fields per WH-09 emitted on every delivery:
  `payload_version` (`"v1"`), `event_type` (`"run_finalized"`), `run_id`, `job_id`, `job_name`, `status`, `exit_code`, `started_at`, `finished_at`, `duration_ms`, `streak_position`, `consecutive_failures`, `image_digest`, `config_hash`, `tags`, `cronduit_version`.
- **D-07:** Field formats:
  - **Timestamps** (`started_at`, `finished_at`): RFC3339 strings (`"2026-04-29T10:43:11Z"`). Project convention; matches `job_runs` columns.
  - **`streak_position`**: integer (1, 2, 3, …). Reflects position within the **filter-matching stream** (D-12), not the unified P16 streak.
  - **`consecutive_failures`**: integer (the unified P16 count, returned as-is from `get_failure_context`). Diverges from `streak_position` on purpose — visibility into the broader failure picture even when the operator's filter is narrow.
  - **`image_digest` on non-docker jobs**: `null` (always emit the field for schema stability — receivers can `payload["image_digest"]` without `KeyError`).
  - **`tags`**: empty array `[]` until Phase 22 lights up real values. Schema-stable; no breaking change at the Phase 22 cutover.
  - **`cronduit_version`**: `env!("CARGO_PKG_VERSION")` at compile time. Aligns with project memory `feedback_tag_release_version_match.md` (tag = Cargo.toml version).
  - **Optional fields generally**: `null` for missing rather than omitted. Receivers get a stable schema.
- **D-08:** Schema is locked at `payload_version: "v1"` for the whole v1.2 line. Future additions are additive (new optional fields only); breaking changes require `payload_version: "v2"` bump (a future phase, not this one).

### Standard Webhooks v1 Headers (WH-03)
- **D-09:** Three headers on every delivery:
  - `webhook-id`: ULID (using the `ulid` crate or rolling our own from `rand` — researcher to confirm; either works). One per delivery; receivers use this for idempotency.
  - `webhook-timestamp`: Unix epoch SECONDS (integer string). Per Standard Webhooks v1 spec.
  - `webhook-signature`: `v1,<base64-of-hmac>` where HMAC-SHA256 is computed over `${webhook-id}.${webhook-timestamp}.${raw-body-bytes}`. Omitted entirely when `webhook.unsigned = true`.
- **D-10:** Body is **compact JSON** (no pretty-print). Signature is over raw response-body bytes BEFORE serialization to disk anywhere. Pretty-print would add whitespace bytes the receiver also has to reproduce — error-prone.
- **D-11:** `Content-Type: application/json`. No charset suffix needed (JSON is UTF-8 by spec).

### Coalescing × State-Filter Interaction (WH-06)
- **D-12:** **Coalesce within the filter-matching stream.** The operator's `states = [...]` filter defines what they care about; coalescing applies to that stream. The dispatcher computes "position within the filter-matching stream since the last non-matching run" Rust-side using P16's `get_failure_context` as the data source plus a small wrapper for filter-matching position.
- **D-13:** Concretely: `failed → timeout` with `states = ["timeout"]`. `consecutive_failures = 2` (unified P16 count, emitted as-is in payload). The timeout's filter-matching-stream position is 1 (first match in the filter stream since the previous non-match — which was the `failed` run, since `failed` ∉ states). With default `fire_every = 1`, the timeout fires.
- **D-14:** P16's streak semantics stay locked: `consecutive_failures` counts `failed | timeout | error` since last `success`. `cancelled` and `stopped` are neutral for the **unified** streak (don't extend, don't reset) — see `src/db/queries.rs:662`. Phase 18 does NOT extend Phase 16's helper. The filter-matching position computation is Phase 18's wrapper.
- **D-15:** Filter-matching-stream position is computed by counting (from most-recent backwards) consecutive runs whose `status ∈ states`, stopping at the first non-matching run OR the first `success`. Implementation strategy is researcher-decided; the fields exposed to the payload are pinned by D-07.
- **D-16:** Fire decision: IF `current_run.status ∈ states` AND coalescing-rule(filter_position, fire_every) → fire. Coalescing rule:
  - `fire_every = 0` → always fire (legacy per-failure)
  - `fire_every = 1` → fire when filter_position == 1 (first-of-stream)
  - `fire_every = N` (N>1) → fire when `filter_position % N == 1` (first, then every Nth: 1, N+1, 2N+1, ...)

### HttpDispatcher Posture (Phase 18 only)
- **D-17:** Single attempt per delivery. On HTTP failure (non-2xx, timeout, connection refused): increment `cronduit_webhook_delivery_failed_total` counter (NEW metric — extends the existing `cronduit_webhook_delivery_dropped_total` from Phase 15) + WARN log naming `job_name`, `url`, status/error, truncated body (first 200 bytes). NO `webhook_deliveries` row write — that table is Phase 20's deliverable.
- **D-18:** Per-request timeout: 10 seconds, hard-coded. Configurability deferred to Phase 20 if operator demand emerges.
- **D-19:** Concurrency: serial within the worker task — one HTTP request at a time. WH-02 already locks the worker as a single dedicated tokio task; this constrains Phase 18 to that shape. If load demands change in v1.3+, a semaphore can be added without touching the trait.
- **D-20:** HTTP client: `reqwest` with `default-features = false, features = ["rustls-tls", "json"]`. Project lock: rustls everywhere; `cargo tree -i openssl-sys` must remain empty.
- **D-21:** Phase 20 retry hook point: trait stays the same; Phase 20 introduces `RetryingDispatcher` that wraps `HttpDispatcher` and implements the same trait. Composability without trait expansion.
- **D-22:** HMAC implementation: `hmac` + `sha2` crates (RustCrypto, rustls-compatible). `webhook-id` generation: `ulid` crate (preferred; lexicographically sortable + 26-char compact base32) — researcher confirms availability, falls back to UUID v7 if blocked.

### Universal Project Constraints (carried forward)

> The decisions below are **[informational]** — repo-wide process constraints honored by absence (no UI files modified, mermaid-only diagrams, PR-only branch state, maintainer-validated UAT). They are not phase-implementation tasks and do not need to be cited in any single plan's `must_haves`. Plan 06 still gates D-25 / D-26 explicitly because they shape that plan's UAT structure.

- **D-23:** [informational] All changes land via PR on a feature branch. No direct commits to main. `phase-18-webhook-payload` is the working branch name.
- **D-24:** [informational] Diagrams in any Phase 18 artifact (PLAN, SUMMARY, README, code comments) are mermaid. No ASCII art.
- **D-25:** UAT recipes use existing `just` commands. Per project memory `feedback_uat_use_just_commands.md` — every UAT step references a `just` recipe, not ad-hoc cargo/docker/curl.
- **D-26:** [informational] Maintainer validates UAT — Claude does NOT mark UAT passed from its own runs (project memory `feedback_uat_user_validates.md`).
- **D-27:** [informational] UI: Phase 18 has NO UI surface. Webhook config visibility on the dashboard is Phase 21 (FCTX UI panel) territory if at all. ROADMAP marks Phase 18 `UI hint: no`.

### Claude's Discretion
- Migration filenames, function signatures, internal struct shapes, and test file names — researcher and planner choose, following existing patterns (`src/webhooks/{mod,dispatcher,event,worker}.rs`, `src/config/{mod,defaults,validate,interpolate,hash}.rs`).
- Whether the `webhook-id` generator uses `ulid` or `uuid` v7 — both are spec-acceptable. Researcher checks crate currency.
- Exact metric names beyond the new `cronduit_webhook_delivery_failed_total` — follow existing `cronduit_*` family conventions.
- Exact validator error wording — follow Phase 17 LBL precedent (name the offending field, name the fix).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level locks
- `.planning/PROJECT.md` — core value, locked v1.2 decisions (webhook worker shape, HMAC SHA-256, retry shape, coalescing default, URL validation, payload schema, reload survival, drain)
- `.planning/REQUIREMENTS.md` § Webhook Outbound (`WH-01`, `WH-03`, `WH-04`, `WH-05`, `WH-06`, `WH-07`, `WH-08`, `WH-09`) — Phase 18 is `WH-01`, `WH-03`, `WH-06`, `WH-09`; the others scope adjacent phases and ARE referenced in Out-of-Scope
- `.planning/STATE.md` § Accumulated Context > Decisions — v1.2 webhook decisions inherited from research/requirements
- `.planning/ROADMAP.md` § Phase 18 — goal + 4 success criteria
- `./CLAUDE.md` — project conventions, locked tech stack, mermaid-only, PR-only workflow

### Standard Webhooks v1 spec
- `https://github.com/standard-webhooks/standard-webhooks/blob/main/spec/standard-webhooks.md` — wire format spec; `webhook-id`/`webhook-timestamp`/`webhook-signature` headers; `v1,<base64>` signature format; HMAC over `id.timestamp.payload`. **Read this directly; do not paraphrase.**

### Phase 15 (foundation; what's already built)
- `.planning/phases/15-foundation-preamble/15-CONTEXT.md` — webhook worker scaffolding decisions
- `.planning/phases/15-foundation-preamble/15-04-SUMMARY.md` *(or whichever 15-XX-SUMMARY.md covers WH-02)* — what was built: bounded mpsc(1024), worker task, NoopDispatcher, drop counter
- `src/webhooks/mod.rs` — module entry; `pub use` surface
- `src/webhooks/dispatcher.rs` — `WebhookDispatcher` trait + `NoopDispatcher` (Phase 18 ADDS `HttpDispatcher`); `WebhookError` enum
- `src/webhooks/event.rs` — `RunFinalized` channel-message contract (NOT the wire-format payload)
- `src/webhooks/worker.rs` — `worker_loop`, `spawn_worker`, `channel`, `CHANNEL_CAPACITY = 1024`

### Phase 16 (failure-context schema + streak helper)
- `.planning/phases/16-failure-context-schema-run-rs-277-bug-fix/16-CONTEXT.md` — locked decisions; D-06 explains `streak_position` is caller-side derived
- `.planning/phases/16-failure-context-schema-run-rs-277-bug-fix/16-VERIFICATION.md` — pinned helper contract
- `src/db/queries.rs:626-720` — `FailureContext` struct + `get_failure_context(pool, job_id)` async fn; CTEs joined `LEFT JOIN`; epoch-sentinel `'1970-01-01T00:00:00Z'`; SINGLE fetch_one per call
- `src/scheduler/run.rs:315-322` — `status_str` mapping; canonical `RunFinalized.status` values

### Phase 17 (the LBL precedent for config + validator patterns)
- `.planning/phases/17-custom-docker-labels-seed-001/17-CONTEXT.md` — D-04 README structure; D-06 PR-only; D-07 mermaid only; D-08 just-recipe UAT
- `.planning/phases/17-custom-docker-labels-seed-001/17-VERIFICATION-GAP-CLOSURE.md` — CR-01/CR-02 close-out; whole-file textual interpolation truth (relevant for `${WEBHOOK_SECRET}` in `webhook.secret`)
- `src/config/mod.rs:80-130` — `JobConfig`/`DefaultsConfig`/`DockerJobConfig` shapes; `labels` + `use_defaults` fields are the structural template for `webhook` + `use_defaults`
- `src/config/defaults.rs::apply_defaults` — merge semantics; LBL-02 precedent for `webhook` defaults merge (likely: replace-on-collision since `webhook` is a single inline block, not a map; researcher confirms)
- `src/config/validate.rs::check_label_*` — validator pattern for LOAD-time errors; `ConfigError { line: 0, col: 0 }` shape (D-01 in Phase 17 CONTEXT)
- `src/config/interpolate.rs::interpolate` — whole-file textual env-var pass (the `${WEBHOOK_SECRET}` mechanism)

### Existing Cronduit infra to reuse
- `src/scheduler/run.rs` — `finalize_run` is where the scheduler emits the `RunFinalized` event via `try_send` (already wired in Phase 15)
- `src/db/queries.rs::insert_running_run` — for understanding how `config_hash` reaches `job_runs` (Phase 16 added the per-RUN column)
- `Cargo.toml` — `tokio` (`features = ["full"]`), `chrono`, `tracing`, `serde`, `metrics`, `metrics-exporter-prometheus` already present; Phase 18 ADDS `reqwest` (rustls-tls + json features), `hmac`, `sha2`, `base64`, `ulid` (or `uuid` with v7 feature)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`WebhookDispatcher` trait** (`src/webhooks/dispatcher.rs`): Phase 18's `HttpDispatcher` implements this. NoopDispatcher stays as a test/dry-run alternative.
- **`RunFinalized` event** (`src/webhooks/event.rs`): the channel message. Already carries `run_id`, `job_id`, `job_name`, `status`, `exit_code`, `started_at`, `finished_at`. Phase 18 reads `job_id` from this event to call `get_failure_context(pool, job_id)` for the rest of the payload fields.
- **`get_failure_context(pool, job_id)`** (`src/db/queries.rs:681`): single-query helper returning `FailureContext { consecutive_failures, last_success_run_id, last_success_image_digest, last_success_config_hash }`. Provides the "broader failure" data; the filter-matching position is Phase 18's wrapper.
- **`SecretString` type** (already in project per CLAUDE.md security posture): wraps `webhook.secret` to prevent accidental Debug/Display leakage.
- **`metrics::counter!`** macros: `cronduit_webhook_delivery_dropped_total` already exists from Phase 15. Phase 18 adds `cronduit_webhook_delivery_failed_total` (HTTP failure) and possibly `cronduit_webhook_delivery_sent_total` (success).
- **Config interpolation pipeline** (`src/config/interpolate.rs`): handles `${WEBHOOK_SECRET}` whole-file substitution before TOML parses.

### Established Patterns
- **`[defaults]` + per-job override + `use_defaults = false`**: locked LBL-precedent shape. Phase 18 mirrors it for `webhook`.
- **LOAD-time validators with `ConfigError { line: 0, col: 0 }`**: Phase 17 D-01 — sort offending keys before format (RESEARCH Pitfall 2 for HashMap iteration determinism).
- **One-file migration** (precedent: Phase 16's `job_runs.config_hash` add): if Phase 18 needs any DB column for state (probably not — `webhook_deliveries` table is Phase 20), the pattern is paired SQLite + Postgres migrations under `migrations/` with `IF NOT EXISTS` on Postgres.
- **`#[ignore]` integration tests** for Docker/HTTP path tests; non-`#[ignore]` for parse-pipeline tests (Phase 17 precedent).
- **Embedded HTMX + askama_web 0.15** for any UI — but Phase 18 has NO UI surface.

### Integration Points
- **Where the dispatcher gets wired**: `src/main.rs` (or wherever the worker is currently spawned with `NoopDispatcher`) — swap to `HttpDispatcher::new(pool, http_client)` when any `webhook` block is configured. If NO webhook is configured anywhere, keep `NoopDispatcher` to avoid spinning the HTTP client.
- **Where the dispatcher reads streak data**: at delivery time, the dispatcher calls `get_failure_context(pool, event.job_id).await`. P16's helper is async and returns `anyhow::Result<FailureContext>`.
- **Where the filter-matching position is computed**: Rust-side wrapper inside `dispatcher.rs` (or a sibling `coalesce.rs`) that takes the unified streak count + the operator's `states` filter + recent-runs query and computes the filter-matching position.
- **Where validators hook in**: `src/config/validate.rs::run_all_checks` per-job loop. Two new check fns: `check_webhook_url`, `check_webhook_block_completeness` (mirrors LBL-03/04/06 fan-out pattern).

</code_context>

<specifics>
## Specific Ideas

- **HMAC base64 encoding**: the `base64` crate's `STANDARD` engine (RFC 4648 §4, with `=` padding). Standard Webhooks spec is explicit on padded base64.
- **`webhook-id` generation**: ULID preferred. `ulid::Ulid::new()` returns a 26-char base32 string. Receivers can use `webhook-id` for idempotency dedup.
- **`webhook-timestamp` clock source**: `chrono::Utc::now().timestamp()` (seconds, not millis). The header is integer seconds per Standard Webhooks v1.
- **Unsigned-delivery wire shape**: when `webhook.unsigned = true`, omit the `webhook-signature` header entirely; emit `webhook-id` and `webhook-timestamp` as usual. Operators using receivers like Slack/Discord still get idempotency + timestamp data.
- **Validator error format example** (mirroring Phase 17 LBL):
  ```
  [[jobs]] `backup-nightly`: `webhook.states = ["fialed"]` contains unknown value `fialed`. Valid values: success, failed, timeout, stopped, cancelled, error.
  ```
- **Coalescing + `fire_every = 0` escape hatch**: an operator who sets `fire_every = 0` AND a narrow filter (`states = ["timeout"]`) gets every timeout delivery without coalescing. Useful when timeouts indicate distinct incidents. Document this in the README.

</specifics>

<deferred>
## Deferred Ideas

- **Multi-webhook per job** (e.g., one URL for failures, one URL for successes, fan-out): NOT in scope. Phase 18 ships single-`webhook`-block per job. Future feature if operator demand surfaces.
- **Webhook UI** (status, last-delivery timestamps, replay/retry button on dashboard): NOT in scope. Phase 21 owns FCTX UI panel; webhooks could light up there if Phase 21's discuss decides so. Capture as roadmap candidate.
- **Allowlist/blocklist URL filter** (SSRF defense beyond loopback/RFC1918): explicit accepted-risk per WH-08; deferred to v1.3.
- **Algorithm-agility for HMAC** (SHA-256 → SHA-384 / SHA-512 / Ed25519 negotiation): WH-04 locked at SHA-256-only for v1.2; revisit for v1.3+ if receiver ecosystem demands it.
- **Pluggable signature schemes** (e.g., GitHub-style `x-hub-signature` instead of `webhook-signature`): NOT in scope. Standard Webhooks v1 is the locked wire format.
- **Per-attempt timeout configuration**: deferred to Phase 20 if needed; Phase 18 hard-codes 10s.
- **Concurrent delivery semaphore** (N parallel HTTPs from the worker): deferred to v1.3+ if load demands; Phase 18 is serial within the single worker task per WH-02.
- **`webhook_deliveries` dead-letter table**: Phase 20 (WH-05).

</deferred>

---

*Phase: 18-webhook-payload-state-filter-coalescing*
*Context gathered: 2026-04-29*
