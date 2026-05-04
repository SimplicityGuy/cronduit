# Requirements: Cronduit v1.2 — Operator Integration & Insight

**Defined:** 2026-04-25
**Milestone:** v1.2 (subsequent milestone; v1.1.0 shipped 2026-04-23)
**Core Value:** One tool that both runs recurrent jobs reliably AND makes their state observable through a web UI.

> Source documents: `.planning/PROJECT.md` § Current Milestone (locked scope), `.planning/research/SUMMARY.md` (research synthesis with two Research-Phase Corrections), `.planning/research/STACK.md`, `.planning/research/FEATURES.md`, `.planning/research/ARCHITECTURE.md`, `.planning/research/PITFALLS.md`, `.planning/seeds/SEED-001-custom-docker-labels.md` (pre-locked Docker labels design), `.planning/milestones/v1.1-REQUIREMENTS.md` (archived baseline for REQ-ID numbering continuity).

> **v1.0 + v1.1 requirements (FOUND-01..13, CONF-01..10, DB-01..14, SCHED-01..14, RAND-01..06, EXEC-01..06, DOCKER-01..10, RELOAD-01..07, UI-01..20, OPS-01..10, OBS-01..05, ERG-01..04) are all validated and archived in `.planning/milestones/v1.0-REQUIREMENTS.md` and `.planning/milestones/v1.1-REQUIREMENTS.md`.** v1.2 continues numbering per category from where v1.1 left off and introduces five new categories: `WH` (webhooks), `LBL` (Docker labels), `FCTX` (failure context), `EXIT` (exit-code histogram), `TAG` (job tagging).

## Research-Phase Corrections (LOCKED)

Two corrections surfaced during research that the requirement language inherits without re-litigation:

1. **`src/scheduler/run.rs:277` bug** — `container_id_for_finalize` is populated with `image_digest` instead of the actual container ID. The v1.2 migration wave MUST fix this by adding a proper `container_id: Option<String>` field to `DockerExecResult` and correcting the assignment. Locked in **FOUND-14** below.

2. **`job_runs.config_hash` schema gap** — `jobs.config_hash` exists per-JOB; failure-context delta needs per-RUN. Resolution: **add `job_runs.config_hash TEXT NULL` per-run column** (Option A, locked at requirements step). Locked in **FCTX-04** below.

## v1.2 Requirements

Every requirement below is a testable operator-visible behavior. Pitfall test-case identifiers (`T-V12-*` from `.planning/research/PITFALLS.md`) are referenced inline where the pitfall research surfaced a specific verification lock.

### Foundation (FOUND) — hygiene + bug fix

Continuation from v1.0/v1.1 FOUND-01..13.

- [x] **FOUND-14**: The `src/scheduler/run.rs:277` bug is fixed: `DockerExecResult` carries both `container_id: Option<String>` (from `create_container`) and `image_digest: Option<String>` (from `inspect_container().image`); `finalize_run` populates `job_runs.container_id` with the real container ID and `job_runs.image_digest` with the digest. Historical rows with `container_id = sha256:...` age out via Phase 6 retention; no data migration. `T-V12-FCTX-01`.

- [x] **FOUND-15**: `Cargo.toml` version is bumped from `1.1.0` to `1.2.0` on the first v1.2 commit. `cronduit --version` reports `1.2.0` from the very first v1.2 commit. rc tags use the semver pre-release format `v1.2.0-rc.1`, `v1.2.0-rc.2`, etc. The final ship is `v1.2.0`.

- [x] **FOUND-16**: A new `cargo-deny` CI job runs `cargo deny check` on every PR (advisories + licenses + duplicate-versions). License allowlist matches the v1.0/v1.1 licensing posture (MIT/Apache-2.0/BSD-3-Clause/ISC/Unicode-DFS-2016 + project-specific exceptions). Failures are non-blocking initially (CI status WARN, not ERROR) for the first rc; promoted to blocking before final v1.2.0.

### Webhooks (WH) — new category

Inbound: scheduler emits a `RunFinalized` event on every terminal status. Outbound: a dedicated worker dispatches webhook deliveries; the scheduler loop is never blocked.

- [ ] **WH-01**: Operators can configure a webhook URL per job (`webhook = { url = "https://...", states = ["failed", "timeout", "stopped"] }`) and in `[defaults]`. The TOML `[defaults]` + per-job override + `use_defaults = false` disable pattern matches the existing config behavior (parallels SEED-001 / Docker labels — see LBL-01..05). `T-V12-WH-01`, `T-V12-WH-02`.

- [x] **WH-02**: A new module `src/webhooks/mod.rs` owns a dedicated tokio task that consumes `RunFinalized` events from a bounded `tokio::sync::mpsc::channel(1024)`. The scheduler emits via `try_send` (NEVER `await tx.send()`); on full queue, the event is dropped with a warn-level log AND a `cronduit_webhook_delivery_dropped_total` counter increment. The scheduler loop is never blocked by outbound HTTP. `T-V12-WH-03`, `T-V12-WH-04`.

- [ ] **WH-03**: Webhook payloads adhere to the [Standard Webhooks v1 spec](https://github.com/standard-webhooks/standard-webhooks/blob/main/spec/standard-webhooks.md): three required headers (`webhook-id`, `webhook-timestamp`, `webhook-signature`); HMAC-SHA256 over `webhook-id.webhook-timestamp.payload` (as raw bytes); signature header value `v1,<base64-of-hmac>`. Payloads include a `payload_version: "v1"` field at the root for forward-compatibility. `T-V12-WH-05`, `T-V12-WH-06`, `T-V12-WH-07`.

- [ ] **WH-04**: HMAC algorithm is **SHA-256 only** in v1.2 (no algorithm-agility / multi-secret rotation cronduit-side; rotation lives on the receiver via dual-secret verify). Cronduit ships receiver examples in Python, Go, and Node demonstrating constant-time HMAC compare (NOT `==` on the hex-decoded bytes — timing-attack defense). `T-V12-WH-08`, `T-V12-WH-09`.

- [ ] **WH-05**: Webhook delivery retry is 3 attempts at t=0, t=30s, t=300s with full-jitter exponential backoff (each attempt's delay multiplied by `rand::random::<f64>() * 0.4 + 0.8` to spread thundering-herd retries). After the third attempt, the delivery is dropped with a counter increment AND an entry in a new `webhook_deliveries` table (one-file migration, no backfill). `T-V12-WH-10`, `T-V12-WH-11`, `T-V12-WH-12`.

- [ ] **WH-06**: Webhook flooding from a 1/min failing job is mitigated by **edge-triggered streak coalescing**: by default, deliveries fire only on `streak_position == 1` (the FIRST failure in a new streak), not every subsequent failure. Operators can override per-job with `webhook.fire_every = N` to fire every N failures, or `webhook.fire_every = 0` to keep the original per-failure behavior. Default is `1` (first-of-streak only). `T-V12-WH-13`, `T-V12-WH-14`.

- [ ] **WH-07**: Webhook URL validation: `https://` is required for non-loopback / non-RFC1918 destinations. `http://` is permitted only for `127.0.0.0/8`, `::1`, `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, and `fd00::/8` (loopback + RFC1918 + ULA). Cronduit emits a startup WARN if any configured webhook URL targets a non-local public destination over `http://`. `T-V12-WH-15`, `T-V12-WH-16`.

- [ ] **WH-08**: SSRF is documented as accepted risk (no allow/block-list filter in v1.2 — half-built filter is worse than no filter; deferred to v1.3 as an explicit feature). `THREAT_MODEL.md` gains Threat Model 5 (Webhook Outbound) enumerating: operator-with-UI-access can configure a webhook URL pointing at any internal service; cronduit is loopback-bound by default; reverse-proxy fronting with auth is the v1.2 mitigation. `T-V12-WH-17`.

- [ ] **WH-09**: Webhook payload schema includes (at minimum): `payload_version: "v1"`, `event_type: "run_finalized"`, `run_id`, `job_id`, `job_name`, `status`, `exit_code`, `started_at`, `finished_at`, `duration_ms`, `streak_position`, `consecutive_failures`, `image_digest` (docker jobs only), `config_hash`, `tags`, plus a `cronduit_version` field. The schema is locked at v1.2.0; future additions are additive (new fields only); breaking changes require a `payload_version: "v2"` bump. `T-V12-WH-18`, `T-V12-WH-19`.

- [ ] **WH-10**: Webhook delivery survives scheduler reload (SIGHUP / `POST /api/reload` / file-watch). In-flight deliveries are NOT cancelled; new deliveries continue to be queued. On graceful shutdown (SIGTERM), the worker drains the queue with a configurable `webhook_drain_grace = "30s"` deadline (default), then drops remaining queued deliveries with a counter increment. `T-V12-WH-20`, `T-V12-WH-21`.

- [ ] **WH-11**: A new `cronduit_webhook_*` Prometheus metric family is added to `/metrics`: `cronduit_webhook_deliveries_total{job, status}` (`status` ∈ `{success, failed, dropped}`), `cronduit_webhook_delivery_duration_seconds{job}` (histogram), `cronduit_webhook_queue_depth` (gauge). Bounded cardinality — `status` is a closed enum; `job` is bounded by configured-job-count. Eagerly described at boot like the v1.0 metrics. `T-V12-WH-22`.

### Custom Docker Labels (LBL) — new category, SEED-001 pre-locked

Pre-locked design: `.planning/seeds/SEED-001-custom-docker-labels.md`. Three decisions inherited verbatim (merge semantics, reserved namespace, type-gating).

- [ ] **LBL-01**: A new `labels: Option<HashMap<String, String>>` field is added to `DefaultsConfig` and `JobConfig`. TOML keys may contain dots (`com.centurylinklabs.watchtower.enable = "false"`). Operator-defined labels are merged with cronduit-internal labels at container-create time, populating `bollard::Config::labels`.

- [ ] **LBL-02**: Merge semantics — `use_defaults = false` → per-job labels REPLACE defaults entirely (whole-section escape hatch consistent with rest of config). `use_defaults = true` or unset → defaults map ∪ per-job map; **on key collision, per-job key wins** (standard override semantics, parallels every other field). `T-V12-LBL-01`, `T-V12-LBL-02`.

- [ ] **LBL-03**: Reserved-namespace validator — operator labels under `cronduit.*` MUST fail config validation at LOAD time (not runtime). The `cronduit.*` prefix is reserved for cronduit-internal labels (currently `cronduit.run_id`, `cronduit.job_name`; reserved for future additions like `cronduit.job_run_number`, `cronduit.image_digest`). Validator emits a GCC-style error pointing at the offending key. `T-V12-LBL-03`, `T-V12-LBL-04`, `T-V12-LBL-05`.

- [ ] **LBL-04**: Type-gated validator — setting `labels` on a `type = "command"` or `type = "script"` job is a config-validation error at load time (commands and scripts have no container; the labels would be silently dropped). Mirrors the v1.0.1 `cmd`-on-non-docker validator at `src/config/validate.rs:89`. `T-V12-LBL-06`, `T-V12-LBL-07`.

- [ ] **LBL-05**: `${ENV_VAR}` interpolation works in label VALUES (free from v1.0's config string-pre-parse interpolation; verified at requirements time). Operators can write `labels = { "deployment.id" = "${DEPLOYMENT_ID}" }` and the value is interpolated at config-load. Keys are NOT interpolated (interpolated keys would be hostile to validation). `T-V12-LBL-08`.

- [ ] **LBL-06**: Label size limits enforced at config-load: each label value ≤ 4 KB (Docker convention); total label-set size per job ≤ 32 KB (sum of all key+value byte lengths). Larger values fail validation with a clear error message. `T-V12-LBL-09`, `T-V12-LBL-10`.

### Failure Context on Run Detail (FCTX) — new category

5 signals on a single inline panel on `run_detail.html` for failed runs. P1 set locked at requirements step (5 signals, not 3).

- [x] **FCTX-01**: A new failure-context panel renders inline on the run-detail page when `status ∈ {failed, timeout}` (not `success`, `cancelled`, `running`, or `stopped`). The panel is collapsed by default; clicking expands it. The 5 signals below render as labeled rows. `T-V12-FCTX-02`.

- [x] **FCTX-02**: **Time-based deltas** — first-failure timestamp ("first failure since last success: 2 hours ago"), consecutive-failure streak ("4 consecutive failures"), link to last successful run ("last successful run: 3 hours ago [view]"). Sourced from a new `get_failure_context(job_id) -> FailureContext` query function in `src/db/queries.rs`. `T-V12-FCTX-03`, `T-V12-FCTX-04`.

- [x] **FCTX-03**: **Image-digest delta** (docker jobs only) — "image digest changed since last success: sha256:abc...→ sha256:def..." with truncation to 12 hex chars per side. Requires `job_runs.image_digest` column populated correctly (FOUND-14). Non-docker jobs hide this row. `T-V12-FCTX-05`, `T-V12-FCTX-06`.

- [x] **FCTX-04**: **Config-hash delta** — "config changed since last success" boolean ("Yes" if `current_run.config_hash != last_successful_run.config_hash`, "No" otherwise). Requires new `job_runs.config_hash TEXT NULL` per-run column added in v1.2 migration wave (Research-Phase Correction 2 / Option A). Conservative backfill from current `jobs.config_hash` for old rows; written from `insert_running_run` at fire time. `T-V12-FCTX-07`, `T-V12-FCTX-08`, `T-V12-FCTX-09`.

- [x] **FCTX-05**: **Duration-vs-p50 deviation** — "duration was 12.3s; typical p50 is 4.2s (3× longer than usual)" computed as `current.duration_ms / p50_of_last_100_successful_runs`. Below 5 sample threshold (no p50 available), this row is suppressed. Reuses `src/web/stats.rs::percentile()` from Phase 13. `T-V12-FCTX-10`.

- [x] **FCTX-06**: **Scheduler-fire-time vs run-start-time skew** — "scheduled fire: 14:30:00; actual start: 14:30:23 (+23s)" computed from `scheduled_for` (already in `job_runs`) and `started_at`. Highlights scheduler back-pressure or executor slow-start situations. `T-V12-FCTX-11`.

- [x] **FCTX-07**: The `get_failure_context(job_id)` query returns a single struct populated from a single SQL query (NOT 5 separate queries). The query is verified via `EXPLAIN QUERY PLAN` on both SQLite and Postgres to use indexed access on `job_runs.job_id` + `start_time`. `T-V12-FCTX-12`.

### Per-Job Exit-Code Histogram (EXIT) — new category

Job-detail page card; parallels v1.1 OBS-04 (p50/p95) pattern. Bucketing strategy locked.

- [x] **EXIT-01**: A new exit-code-distribution card renders on the job-detail page, sibling to the v1.1 p50/p95 duration card. Sample window: last 100 ALL runs (not just successful — exit codes are most useful for diagnosing failures, divergent from OBS-04's last-100-successful window). Below `N=5` sample threshold, the card renders "—" instead of an empty histogram. `T-V12-EXIT-01`, `T-V12-EXIT-02`.

- [x] **EXIT-02**: Exit-code bucketing — 10 fixed buckets to prevent cardinality explosion: `0` (success), `1` (general error), `2` (shell builtin misuse), `3-9` (custom range), `10-126` (custom range), `127` (command not found), `128-143` (signal-killed: SIGINT=130, SIGKILL=137, SIGSEGV=139, SIGTERM=143), `144-254` (custom range), `255` (exit out of range), `null` (no exit code recorded — for `timeout` and `stopped` runs). Named exit codes (127, 137, 139, 143, etc.) render with a small label tooltip. `T-V12-EXIT-03`.

- [x] **EXIT-03**: The exit-code `0` (success) bar is rendered as a SEPARATE STAT (success rate badge sibling to the histogram), NOT as a bar within the histogram chart itself. This prevents the giant `0` bar from dominating the chart and obscuring the failure distribution. Matches the v1.1 sparkline + success-rate badge pattern. `T-V12-EXIT-04`.

- [x] **EXIT-04**: `stopped` runs (which exit 137 from cronduit's SIGKILL) are rendered as a DISTINCT visual bucket separate from the `128-143` signal-killed bucket. Otherwise the histogram would lie about crash rate (operator-stops would inflate "signal-killed" counts). The `stopped` bucket uses the `--cd-status-stopped` color from v1.1's design tokens. `T-V12-EXIT-05`, `T-V12-EXIT-06`.

- [x] **EXIT-05**: The card displays "last seen for each code" alongside the count (e.g., "1: 12 occurrences (last: 2h ago)") for the top-3 most-frequent codes. Cronitor-style touch worth adding cheaply; small additional column in the underlying query. `T-V12-EXIT-07`.

- [ ] **EXIT-06**: Exit codes are NOT exposed as a Prometheus label on existing `cronduit_runs_total` family or any new family — preserves v1.0 cardinality discipline (i32 exit codes would be unbounded label cardinality). Operators who want exit-code metrics can scrape the histogram via the dashboard or build their own pipeline. `T-V12-EXIT-08`.

### Job Tagging / Grouping (TAG) — new category

UI-only filter chips on the dashboard. NO effect on webhooks (WH-09 includes tags in the payload but never AS a routing key), search, or metrics labels.

- [ ] **TAG-01**: A new `tags: Vec<String>` field is added to `JobConfig` (TOML: `tags = ["backup", "weekly"]`). NOT added to `DefaultsConfig` (per-job only — the `[defaults] + per-job + use_defaults = false` override pattern does NOT apply to tags). `T-V12-TAG-01`.

- [x] **TAG-02**: Tags are persisted to a new `jobs.tags TEXT NOT NULL DEFAULT '[]'` column (JSON-serialized array; structurally parity-friendly across SQLite + Postgres without JSONB ops). Migration is a single-file additive nullable→default '[]' (NOT the three-file tightening pattern; old jobs get default `'[]'` automatically on column add). `T-V12-TAG-02`.

- [ ] **TAG-03**: Tag normalization at config-load: lowercase + trim. `["Backup", "backup ", "BACKUP"]` collapse to `["backup"]`. Operators get a config-load WARN (not error) when normalization would collapse multiple tags to the same canonical form — flags the deduplication so operators notice. `T-V12-TAG-03`, `T-V12-TAG-04`.

- [ ] **TAG-04**: Tag charset validator at config-load: `^[a-z0-9][a-z0-9_-]{0,30}$` (ASCII lowercase + digits + underscore + dash; max 31 chars total; must start with alphanumeric). REJECT (don't silently mutate) on invalid input — operator gets a clear error pointing at the offending tag. Reserved tag names (`cronduit`, `system`, `internal`) are rejected. `T-V12-TAG-05`, `T-V12-TAG-06`.

- [ ] **TAG-05**: Tag substring-collision check at config-load: rejects fleets where one tag is a substring of another (e.g., `back` and `backup` cannot both exist). The SQL filter `tags LIKE '%"' || ?tag || '"%'` is structurally parity-friendly across SQLite + Postgres but vulnerable to substring false-positives without this validator. `T-V12-TAG-07`.

- [ ] **TAG-06**: The dashboard renders filter chips for every distinct tag in the current fleet. Clicking a chip toggles its filter state (active = teal-bordered + bold; inactive = grey). Multiple active chips compose with **AND semantics** (job must have ALL active tags to render). URL state via repeated `?tag=` params (e.g., `/?tag=backup&tag=weekly`); shareable, bookmarkable. `T-V12-TAG-08`, `T-V12-TAG-09`.

- [ ] **TAG-07**: When ANY tag filter is active, untagged jobs are hidden from the dashboard (least-surprise behavior for an active filter). The tag-filter composes with the existing v1.0 name-filter via **AND** (job must match BOTH name-filter AND tag-filter). `T-V12-TAG-10`.

- [ ] **TAG-08**: Tag dashboard rendering uses CSS-only chip components (no JS framework, no canvas); HTMX swaps the dashboard partial on chip toggle (matches v1.0 dashboard polling architecture). `T-V12-TAG-11`.

## Future Requirements

Deferred to a future milestone; NOT in v1.2 scope. Duplicated from `.planning/PROJECT.md` § Future Requirements for traceability.

### v1.3 — Search + concurrency + ergonomics deepening (tentative)
- Cross-run log search across retention window — engine choice (naive LIKE vs SQLite FTS5 / Postgres tsvector) deferred from v1.2 kickoff for usage-data-driven decision
- Job concurrency limits and queuing (deep scheduler-core change; affects the `tokio::select!` loop + persistence + fairness)
- Snooze a job for a duration without editing the config; auto-re-enable
- Run history filters (status, date range, exit code) and sortable columns
- Webhook persistence to disk across restart — best-effort delivery in v1.2; durable queue is a v1.3 candidate
- Webhook SSRF allow/block-list filter (half-built filter is worse than no filter; deferred until usage-data informs the right shape)

### v1.4 — UX polish (tentative)
- Job duplicate-as-snippet (UI emits a TOML block to paste into the config)
- Fuzzy job search (`back` → `backup-postgres`)

## Out of Scope

Explicit boundaries; NOT in v1.2 or v1.3.

- **Web UI authentication** — deferred to v2. v1.x still assumes loopback / trusted LAN / reverse-proxy fronting. The new webhook-config surface widens the operator-access blast radius (operator-with-UI-access can configure outbound HTTP at any URL); `THREAT_MODEL.md` Threat Model 5 documents this explicitly. No design work in v1.2.
- **Multi-node / distributed scheduling** — single-node only.
- **User management / RBAC** — single-operator tool.
- **Workflow DAGs / job dependencies** — jobs are independent.
- **Email notifications** — operators can layer email on top of webhooks via bridges.
- **Ad-hoc one-shot runs not defined in the config** — config remains source of truth.
- **Importer for existing ofelia configs** — users rewrite.
- **SPA / React frontend** — server-rendered HTML only.
- **Webhook payload templating with Jinja-style placeholders** — receivers shape their own messages from the locked schema; templating is too much surface for v1.2.
- **Cronduit-side dual HMAC signing for secret rotation** — multi-secret window stays on receiver side.
- **HMAC algorithm agility (configurable algorithm)** — SHA-256 only in v1.2; matches Standard Webhooks spec.
- **Webhook UI configuration (forms in dashboard)** — config remains TOML; no UI-side webhook editing in v1.2.
- **Webhook delivery dead-letter inspector UI** — `webhook_deliveries` table is queryable via SQL and Prometheus metrics; dedicated dashboard panel deferred to v1.3.
- **`[defaults].tags`** — tags are per-job only; no defaults pattern (would create the substring-collision detection problem on every config-load).
- **Tags as Prometheus label** — unbounded cardinality risk; explicitly out (TAG-related metrics are NOT planned).
- **Tag UI bulk operations (bulk enable/disable BY TAG)** — possible v1.3 candidate; v1.2 tags are filter-only.
- **Image-digest delta for non-docker jobs** — command and script jobs have no image; the row is hidden on the failure-context panel for these job types.
- **Config-hash delta proxied via `jobs.updated_at`** (Option B from research) — Option A locked at requirements step (per-run column).

## Traceability

| REQ-ID    | Phase | Status  |
| --------- | ----- | ------- |
| FOUND-14  | 16    | Complete |
| FOUND-15  | 15    | Complete |
| FOUND-16  | 15    | Complete |
| WH-01     | 18    | Pending |
| WH-02     | 15    | Complete |
| WH-03     | 18    | Pending |
| WH-04     | 19    | Pending |
| WH-05     | 20    | Pending |
| WH-06     | 18    | Pending |
| WH-07     | 20    | Pending |
| WH-08     | 20    | Pending |
| WH-09     | 18    | Pending |
| WH-10     | 20    | Pending |
| WH-11     | 20    | Pending |
| LBL-01    | 17    | Complete |
| LBL-02    | 17    | Complete |
| LBL-03    | 17    | Complete |
| LBL-04    | 17    | Complete |
| LBL-05    | 17    | Complete |
| LBL-06    | 17    | Complete |
| FCTX-01   | 21    | Complete |
| FCTX-02   | 21    | Complete |
| FCTX-03   | 21    | Complete |
| FCTX-04   | 16    | Complete |
| FCTX-05   | 21    | Complete |
| FCTX-06   | 21    | Complete |
| FCTX-07   | 16    | Complete |
| EXIT-01   | 21    | Complete |
| EXIT-02   | 21    | Complete |
| EXIT-03   | 21    | Complete |
| EXIT-04   | 21    | Complete |
| EXIT-05   | 21    | Complete |
| EXIT-06   | 21    | Pending |
| TAG-01    | 22    | Pending |
| TAG-02    | 22    | Complete |
| TAG-03    | 22    | Pending |
| TAG-04    | 22    | Pending |
| TAG-05    | 22    | Pending |
| TAG-06    | 23    | Pending |
| TAG-07    | 23    | Pending |
| TAG-08    | 23    | Pending |

**Total:** 41 requirements across 6 categories (3 FOUND + 11 WH + 6 LBL + 7 FCTX + 6 EXIT + 8 TAG). Phase mapping populated by `/gsd-roadmapper` 2026-04-25; v1.2 roadmap covers Phases 15-24 with three rc cuts embedded (rc.1 in P20, rc.2 in P21, rc.3 in P23, final v1.2.0 in P24).

---

*Defined: 2026-04-25 — milestone kickoff, after the 4-dimension research pass with two Research-Phase Corrections surfaced (run.rs:277 bug, job_runs.config_hash schema gap). Open questions resolved at requirements step: Option A for config_hash (per-run column), DB column for tags, cargo-deny v1.2 preamble, 5 P1 failure-context signals (time + image + config + duration-vs-p50 + scheduler-fire-skew), HMAC SHA-256 only, tag filter AND semantics, ${ENV_VAR} interpolation enabled in label values, stopped runs distinct in exit-code histogram, exit-code 0 as separate stat. Traceability populated by `/gsd-roadmapper` next.*
