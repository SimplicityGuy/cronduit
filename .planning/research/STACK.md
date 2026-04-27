# Stack Research — v1.2 "Operator Integration & Insight"

**Domain:** Rust cron scheduler (Cronduit) — subsequent milestone, stack locked
**Researched:** 2026-04-25
**Confidence:** HIGH
**Scope:** Additions / version bumps required to deliver the v1.2 feature set on top of the already-shipped v1.1.0 codebase. The full v1.0 stack was locked at `.planning/milestones/v1.0-research/STACK.md` and the v1.1 delta at `.planning/milestones/v1.1-research/STACK.md`; neither is re-evaluated here.

---

## TL;DR — decisions the roadmap can lock

| Decision | Outcome | Confidence |
|----------|---------|------------|
| **Outbound webhook HTTP client** | `reqwest = "0.12.28"` with `default-features = false` and features `["rustls-tls", "http2", "json"]` (or `"rustls-tls-webpki-roots"` for explicit root trust) | HIGH |
| **HMAC signing** | RustCrypto pair: `hmac = "0.13"` + reuse the existing `sha2 = "0.11"` pin. NOT `ring`. | HIGH |
| **Image digest extraction** | Already implemented in tree at `src/scheduler/docker.rs:240` via `Docker::inspect_container().image` (= `sha256:...` content-addressable ID). v1.2 work is purely DB persistence + UI surfacing, no bollard helper module needed. | HIGH |
| **Webhook delivery worker** | `tokio::sync::mpsc` (already in tree via `tokio = "1.52" features = ["full"]`). NOT `flume`. Bounded channel + dedicated dispatcher task. | HIGH |
| **Hygiene bumps for v1.2** | Three patch-level optional bumps (`tokio 1.52 → 1.52.1`, `bollard 0.20 → 0.20.2` if not already at .2, `testcontainers 0.27 → 0.27.3` for dev-deps). No major-version transitions recommended for v1.2. | HIGH |
| **No JS/chart library for exit-code histogram or tagging UI** | Server-rendered HTML + CSS bars + HTMX filter chips, same pattern as v1.1 sparklines and timeline. | HIGH |

**Net new runtime dependencies for v1.2: 2 crates (`reqwest`, `hmac`).** Both are needed for the webhook feature; they are introduced together by Phase 15 / 16 (whichever owns the webhook delivery skeleton). No other v1.2 feature requires a new crate.

---

## 1. Question-by-Question Findings

### 1.1 HTTP client for outbound webhooks

**Verdict: `reqwest = "0.12.28"` with `default-features = false`, features `["rustls-tls", "http2", "json"]`.**

**Why reqwest, not hyper-util:**

The repo already has `hyper-util = "0.1"` in tree from v1.1's `cronduit health` CLI (`src/cli/health.rs`). On the surface that looks like the obvious incumbent — but the health CLI is a deliberately minimal, loopback-only, no-DNS, no-TLS, single-shot probe. Using the same client for webhooks would force us to hand-roll:

- TLS via rustls + `hyper-rustls` connector wiring
- DNS resolution via `hyper-util::client::legacy::connect::dns::GaiResolver` (currently not enabled — health uses bare `HttpConnector`)
- Cookie / redirect handling (operators *will* point webhooks at services that 302 to canonical URLs)
- HTTP/2 negotiation (the CLI is HTTP/1 only)
- Retry / connection pooling
- JSON body construction (currently ad-hoc via `Empty<Bytes>` + manual builder)
- gzip/deflate response decoding for any non-200 retry path that wants to log a body excerpt

That is ~300 lines of glue we would write, test, and own — for one feature. `reqwest` is a thin opinionated wrapper *on top of* `hyper` + `hyper-util` that bundles all of it.

The repo's existing `hyper-util` client stays untouched for the health probe path — that path explicitly does NOT want any of the bells (no DNS resolution against `127.0.0.1:8080`, no TLS, no JSON, fail-fast on connection refused). Two small clients with different jobs is correct.

**Why reqwest 0.12.x and not 0.13.x:**

`reqwest 0.13.2` released 2026-02-06 — a *major* bump that primarily renames `rustls-tls` features (the new line uses just `rustls` / `default-tls`). 0.13.0 is brand new (13.5K downloads at the time of research vs. 38M+ for the 0.12.x line). For a public OSS project that values dep hygiene over bleeding-edge, **0.12.28 is the correct landing**:

- 0.12.x is the long-tail stable line everyone in the Rust HTTP ecosystem is on.
- The 0.13 rename is purely cosmetic; the API surface we use (`Client::builder()`, `client.post(url).json(body).send().await`) is unchanged.
- A future v1.3 hygiene milestone can cut over to 0.13 in isolation if desired. Do not entangle a feature ship with an HTTP-client major bump.

**TLS: rustls only — verified zero `openssl-sys` impact:**

The constraint at the project level is `cargo tree -i openssl-sys` must return empty (locked in CLAUDE.md). `reqwest`'s default `native-tls` feature pulls openssl on Linux. The fix is **`default-features = false`** + **`features = ["rustls-tls", ...]`**:

```toml
reqwest = { version = "0.12.28", default-features = false, features = [
    "rustls-tls",        # rustls instead of native-tls; zero openssl-sys
    "http2",             # was a default in 0.12; we're disabling defaults so re-add
    "json",              # serde_json body helpers
    "charset",           # was a default; cheap; keep so non-utf8 webhook responses don't panic
] }
```

**Watch out for** — `default-features = false` drops `default-tls`, `system-proxy`, `http2`, `h2`, `charset`, and `mime`. Of those, `http2` and `charset` are worth re-adding (HTTP/2 because some webhook endpoints negotiate to it cleanly; charset because operators *will* hit a server that returns `text/plain; charset=iso-8859-1` for an error page and we don't want to crash the dispatcher). Skip `system-proxy` — homelab webhook endpoints are reachable directly. Skip `gzip`/`brotli`/`deflate` — webhook *requests* are tiny JSON; we don't need to gzip outbound and we don't need to decode the response body (we only inspect status codes for retry classification).

**HTTP/1.1 + HTTP/2 outbound:** confirmed. `reqwest 0.12` supports both; with `http2` feature the client negotiates HTTP/2 via ALPN automatically when the server supports it. Verified: docs.rs/reqwest/0.12.28 ClientBuilder — `http2_prior_knowledge()` requires the feature flag, but transparent ALPN-negotiated h2 is on by default once the feature is enabled.

**Configurable timeouts:** `reqwest::ClientBuilder::timeout(Duration)` (whole-request budget) + `connect_timeout(Duration)` (connect-only) are exactly what the webhook spec calls for. Default the whole-request timeout to 10s, connect to 5s, expose both as `[defaults] webhook_timeout` / `webhook_connect_timeout` in the config schema.

**Why not ureq:**

`ureq 3.3.0` (released 2026-03-21) is a small, RustCrypto-friendly HTTP client — and **definitively the wrong choice for cronduit**. ureq is *blocking I/O* by design: "It uses blocking I/O instead of async I/O, because that keeps the API simple and keeps dependencies to a minimum." A blocking webhook client called from inside a `tokio` task would either (a) block one of the `tokio::runtime::Runtime` worker threads, throttling the scheduler, or (b) require wrapping each call in `tokio::task::spawn_blocking`, which adds a thread per concurrent webhook delivery. Neither is acceptable for a service that may be sending hundreds of webhooks per hour on a busy fleet.

ureq has rustls support out of the box (it is the *default* TLS backend), no openssl, and a smaller dep tree than reqwest — those are real wins. None of them outweigh the blocking-runtime mismatch.

**Why not a hyper-util upgrade path:**

You could reach the same destination by adding `hyper-rustls` to the existing `hyper-util` client. We'd be reinventing reqwest. The 12-15 transitive deps reqwest pulls (when configured with `default-features = false` + the four features above) are all already in the tree via tokio + hyper + tower-http + sqlx + bollard. The marginal compile-time cost is ~5-10 seconds; the marginal binary-size cost is ~200KB after LTO; both are well within the project's tolerance.

**Confidence: HIGH.** Verified versions on crates.io 2026-04-25; verified rustls feature flags against docs.rs/reqwest/0.12.28; confirmed `default-features = false` is the documented escape from openssl-sys.

---

### 1.2 HMAC signing for webhooks

**Verdict: RustCrypto pair — `hmac = "0.13"` + reuse existing `sha2 = "0.11"`.**

**Why RustCrypto, not ring:**

| Property | `hmac` 0.13 + `sha2` 0.11 | `ring` 0.17 |
|----------|---------------------------|-------------|
| Pure Rust | Yes | No (large C/asm component for AES, ChaCha, etc.) |
| Last release | 2026-03-29 (`hmac`) / 2026-03-25 (`sha2`) | 2025-03-11 |
| Pulls into rustls already? | Yes — `sha2` is a transitive dep of rustls → already in `Cargo.lock` | Yes — but as a *transitive* dep we do not call directly |
| HMAC-SHA256 ergonomics | `Hmac::<Sha256>::new_from_slice(key)?.update(body).finalize()` | `ring::hmac::sign(&Key::new(HMAC_SHA256, key), body)` |
| Compile-time | Negligible — both crates already compiled | Significant — ring rebuilds C/asm primitives whenever the rustc target triple changes |
| Maintenance | RustCrypto org, very active, used by `rustls`/`webpki` ecosystem | Brian Smith solo; 0.17.14 in March 2025, no patch in 13 months |
| Cross-compile cleanliness | Pure Rust → identical build on amd64 + arm64 + musl | C component → has caused arm64-musl headaches historically (resolved, but a friction surface) |

`sha2 = "0.11"` is **already pinned in `Cargo.toml` line 85** (used today for config-hash computation in v1.0). The `hmac` 0.13 release on 2026-03-29 is API-compatible with `sha2 = "0.11"` (RustCrypto coordinates `hmac` and `sha2` releases). One new line in `Cargo.toml` — `hmac = "0.13"` — and we get HMAC-SHA256 for free.

**Watch out for** — RustCrypto crates use the `digest` trait machinery, so the call site is:

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
    .expect("HMAC accepts any key length");
mac.update(payload_bytes);
let signature = mac.finalize().into_bytes();
let header = format!("sha256={}", hex::encode(signature));
```

The `hex` crate is already in tree (line 114 of `Cargo.toml`) for CSRF tokens. The signature header format `sha256=<hex>` matches the GitHub webhook convention exactly — operators piping cronduit alerts into existing GitHub-style verifiers will get parity.

**`ring` would also work** — it's not wrong, just heavier and less aligned with the project's already-rustls-only crypto posture. The dep-tree audit and arm64-musl build cleanliness arguments tip the call to RustCrypto.

**Confidence: HIGH.** Verified `hmac` 0.13.0 release on crates.io 2026-04-25 (released 2026-03-29 with 368M+ historical downloads, RustCrypto-maintained). Confirmed `sha2 = "0.11"` is pinned in current `Cargo.toml` and would be reused.

---

### 1.3 Image digest extraction (bollard 0.20)

**Verdict: NO NEW CODE NEEDED for the bollard call. The digest is already captured at run-start in v1.1; v1.2 work is purely DB persistence + UI surfacing.**

**Verified via direct source read** (`src/scheduler/docker.rs:240`):

```rust
// Extract image digest via inspect (DOCKER-09).
let image_digest = match docker.inspect_container(&container_id, None).await {
    Ok(info) => info.image.unwrap_or_default(),
    Err(e) => {
        tracing::warn!(...);
        String::new()
    }
};
```

This already runs after `docker.start_container()` succeeds, capturing the resolved image digest of the *actual* image the container was created from. The value flows through `DockerExecResult::image_digest: Option<String>` (line 67) and into the executor return path at `src/scheduler/run.rs:277`. Today it is only used for diagnostic logging and is **discarded after the run completes — the schema has no column to persist it.**

**bollard 0.20.2 API confirmed** (docs.rs):

- `Docker::inspect_container(name, options) -> Result<ContainerInspectResponse, Error>`
- `ContainerInspectResponse.image: Option<String>` — documented as: *"The ID (digest) of the image that this container was created from."*
- Format: a content-addressable digest like `sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.

This is *exactly* what cronduit needs for the failure-context image-digest delta — comparing the previous run's digest to the current run's digest tells the operator whether the image was repulled / updated between runs. The fact that the value is already extracted and merely thrown away makes this a near-trivial v1.2 addition.

**`RepoDigests` from `inspect_image` is the wrong field for this use case:**

`Docker::inspect_image(name).repo_digests` returns the registry-canonical digests (`registry.example.com/myimage@sha256:...`), which is more verbose, sometimes empty for locally-built images, and changes meaning between registries. The container-level `info.image` is the actual ID of the resolved image at start time, which is what "this run vs that run, did the image change?" needs. Stay on `inspect_container().image`.

**The v1.2 work for this feature:**

1. **Schema migration** (three-file pattern, parallels v1.1 `job_run_number`):
   - `2026MMDD_NNNNNN_image_digest_add.up.sql` — add nullable `job_runs.image_digest TEXT`.
   - No backfill migration — existing rows simply leave the column NULL (older runs can't synthesize a digest from history; UI gracefully degrades to "—" for those).
   - **No NOT NULL flip** — command/script jobs never have an image digest; the column is correctly nullable forever.

2. **Persistence wiring** in `src/scheduler/run.rs` finalize path — pass `docker_result.image_digest` through to the `INSERT INTO job_runs ... RETURNING id` (or rather, the `UPDATE job_runs SET ... WHERE id = ?` at finalize), parallel to the existing `exit_code` plumb-through.

3. **UI surfacing** in `src/web/handlers/run_detail.rs` (or wherever the run-detail handler lives) — query the previous successful run for the same job, compare digests, render a small badge if the digest changed since the last success.

**No new bollard helper module.** The existing call site is the right place. **No new crate.** No new dependency on `bollard` features.

**Confidence: HIGH.** Verified via direct source read of `src/scheduler/docker.rs:240`; verified the bollard 0.20.2 `ContainerInspectResponse.image` field semantics on docs.rs; confirmed no `image_digest` column in the current schema (`migrations/sqlite/` and `migrations/postgres/` enumerated, none mention image_digest).

---

### 1.4 Webhook delivery worker — channel choice

**Verdict: `tokio::sync::mpsc` (already in tree). Bounded buffer + dedicated dispatcher task. NO new dependency.**

**Why tokio mpsc and not flume:**

`flume = "0.12.0"` (released 2025-12-08) is a strong general-purpose channel crate — sometimes faster than `std::sync::mpsc` for high-contention scenarios, supports both blocking and async receivers, allows multiple consumers. None of those properties matter for the webhook delivery worker:

- **Producer side:** the scheduler emits at most one webhook per job state transition. At a homelab's pace (dozens to low hundreds of state transitions per hour fleet-wide), there is no contention to optimize for.
- **Consumer side:** one dispatcher task is the right shape. Multiple consumers would require coordinating retries (you don't want two dispatcher tasks both retrying the same delivery on different schedules) — single-consumer is the correct design.
- **Async boundary:** the producer is async (scheduler), the consumer is async (dispatcher loop calling `reqwest`), so we want an async-native channel. `tokio::sync::mpsc` is async-native; `flume` adds a second channel runtime alongside tokio's.

`tokio::sync::mpsc` is **already enabled** by `tokio = { version = "1.52", features = ["full"] }` (the `sync` feature is in `full`). No `Cargo.toml` edit. No new transitive deps.

**Channel sizing — recommended pattern:**

```rust
// At startup (in src/lib.rs or src/main.rs), bounded for backpressure:
let (webhook_tx, webhook_rx) = tokio::sync::mpsc::channel::<WebhookDelivery>(256);
//                                                                          ^^^^
// Bounded so a misconfigured webhook URL that hangs every request can't
// inflate the queue without bound. 256 is large enough that normal homelab
// burst traffic (a flock of timers firing at :00) won't block the scheduler;
// small enough that a sustained backpressure event surfaces as a metrics blip
// (cronduit_webhook_queue_depth) and a "webhook backpressure" warn log,
// not silent OOM.

// Producer (scheduler, after run finalize):
if let Err(e) = webhook_tx.try_send(delivery) {
    metrics::counter!("cronduit_webhook_dropped_total").increment(1);
    tracing::warn!(target: "cronduit.webhook", "webhook dispatch queue full; dropping");
}

// Consumer (dispatcher task spawned at startup):
let dispatcher = tokio::spawn(async move {
    while let Some(delivery) = webhook_rx.recv().await {
        // Retry loop with exponential backoff (3 attempts).
        for attempt in 0..3 {
            match send_webhook(&client, &delivery).await {
                Ok(_) => { metrics::counter!(...).increment(1); break; }
                Err(_) if attempt < 2 => {
                    tokio::time::sleep(backoff(attempt)).await;
                    continue;
                }
                Err(e) => {
                    tracing::error!(...); break;
                }
            }
        }
    }
});
```

**Watch out for** — the producer should use `try_send` (not `send().await`). A blocking `send` would propagate webhook backpressure into the scheduler's run-finalize hot path; a `try_send` failure is a clean "drop and metric" signal that surfaces the operational problem without ever blocking job execution. The bounded buffer + try_send pattern is the same shape v1.0 already uses for the per-run `mpsc::channel(LOG_CHANNEL_CAPACITY)` log pipeline (see `src/scheduler/log_pipeline.rs`) — the project has internalized this pattern.

**Why not a one-task-per-delivery `tokio::spawn` model:**

You could skip the channel entirely and do `tokio::spawn(deliver_webhook(...))` from the run-finalize site. That works, but:

- Concurrency is unbounded (operator misconfigures a slow webhook → tasks pile up).
- No central place to read queue depth as a metric.
- Retry/backoff lives inside each spawned task with no coordinator visibility.
- Graceful shutdown gets harder — the existing `CancellationToken` cascade can't reach detached spawns easily; would need a `JoinSet`.

The mpsc + single-dispatcher model gets you bounded concurrency, observable queue depth, central retry policy, and a single graceful-shutdown handle. ~30 extra lines of code for substantially better operational properties. Recommended.

**Multiple dispatchers in v1.3+ if needed:** if a future milestone shows the single dispatcher becoming a bottleneck (unlikely at homelab scale), the upgrade path is a `JoinSet` of N dispatchers all `recv()`ing from the same `Receiver` (mpsc supports this via `Mutex<Receiver>` — or the `mpmc` flavor in `tokio::sync::mpmc` if it's stabilized by then). Not a v1.2 concern.

**Confidence: HIGH.** Pattern matches the existing log pipeline shape; tokio mpsc is already enabled; no new crate needed.

---

### 1.5 Hygiene bumps for v1.2

**Audit cross-referenced 2026-04-25 against `Cargo.toml`:**

| Crate | Pinned | Current | Drift | Recommendation |
|-------|--------|---------|-------|----------------|
| `tokio` | 1.52 | 1.52.1 (2026-04-16) | Patch | **Optional** — bump to 1.52.1 or `"1.52"` (which resolves to 1.52.1 anyway given semver caret). Trivial. |
| `axum` | 0.8.9 | 0.8.9 | None | Already current. |
| `tower-http` | 0.6.8 | 0.6.8 | None | Current. |
| `bollard` | 0.20 | 0.20.2 | Patch (caret already matches) | `"0.20"` resolves to 0.20.2. No change needed. |
| `sqlx` | 0.8.6 | 0.8.6 | None | Current. (0.9.0-alpha exists; do not adopt.) |
| `askama` / `askama_web` | 0.15 | 0.15.6 | Patch (caret matches) | Resolves to 0.15.6. No change needed. |
| `croner` | 3.0 | 3.0.1 | Patch | Resolves to 3.0.1. No change needed. |
| `serde` | 1.0.228 | 1.x | None | Current. |
| `chrono` | 0.4.44 | 0.4.44 | None | Current. |
| `chrono-tz` | 0.10.4 | 0.10.4 | None | Current. |
| `metrics` | 0.24 | 0.24.x | None | Current. |
| `metrics-exporter-prometheus` | 0.18 | 0.18.x | None | Current. |
| `rust-embed` | 8.11 | 8.11 | None | Current. |
| `rand` | 0.10 | 0.10.1 | Patch (caret matches) | Resolves to 0.10.1. No change needed. (Note: v1.1 research recommended `rand 0.9` as the conservative landing; the actual v1.1 ship took it all the way to 0.10. Either is fine; the codebase is on 0.10 today.) |
| `secrecy` | 0.10.3 | 0.10.3 | None | Current. |
| `notify` | 8.2 | 8.2.0 | None (9.0.0-rc.3 exists; do not adopt rc) | Current. |
| `hyper` / `hyper-util` | 1 / 0.1 | 1.x / 0.1.20 | None (caret matches) | Current. |
| `clap` | 4.6 | 4.6.x | None | Current. |
| `tracing` / `tracing-subscriber` | 0.1.44 / 0.3.23 | current | None | Current. |
| `regex` | 1 | 1.x | None | Current. |
| `serde_json` | 1 | 1.x | None | Current. |
| `axum-htmx` | 0.8 | 0.8.x | None | Current. |
| `axum-extra` | 0.12 | 0.12.x | None | Current. |
| `ansi-to-html` | 0.2 | 0.2.x | None | Current. |
| `async-stream` | 0.3 | 0.3.x | None | Current. |
| `mime_guess` | 2 | 2.x | None | Current. |
| `tempfile` | 3 | 3.x | None | Current. |
| `libc` | 0.2 | 0.2.x | None | Current. |
| `shell-words` | 1.1 | 1.1.x | None | Current. |
| `humantime` / `humantime-serde` | 2.3.0 / 1.1.1 | current | None | Current. |
| **`testcontainers` (dev)** | 0.27.3 | 0.27.3 | None | Current. |
| **`testcontainers-modules` (dev)** | 0.15.0 | 0.15.x | None | Current. |
| `assert_cmd` (dev) | 2 | 2.x | None | Current. |
| `predicates` (dev) | 3 | 3.x | None | Current. |

**Verdict: zero hygiene bumps are required for v1.2.** Caret semver already absorbs every patch release. The audit table is the *evidence* that the v1.1.0 lockfile is in good shape on entry to v1.2.

**Two NEW pins** introduced by this milestone (both tied to feature work, not hygiene):

```toml
# Outbound HTTP client for webhooks (NEW for v1.2)
reqwest = { version = "0.12.28", default-features = false, features = [
    "rustls-tls",
    "http2",
    "json",
    "charset",
] }

# HMAC-SHA256 for webhook signing (NEW for v1.2)
hmac = "0.13"
# (sha2 already pinned at line 85; reused, not re-declared)
```

**Watch out for major-version transitions on the horizon (do NOT bake into v1.2):**

| Crate | Now | Next major | Why defer |
|-------|-----|-----------|-----------|
| `reqwest` | 0.12.28 | 0.13.x | Renamed feature flags; would require touching the brand-new webhook code immediately after landing it. Defer to a later hygiene pass. |
| `notify` | 8.2 | 9.0.0-rc.3 | Still RC. Wait for stable. |
| `sqlx` | 0.8.6 | 0.9.x | 0.9.0-alpha.1 exists; alpha → unstable for an OSS release. Wait for stable. |
| `secrecy` | 0.10.3 | 0.11.x (none yet) | Last release 2024-10. No reason to touch. |
| `bollard` | 0.20.2 | 0.21.x (none yet) | Currently the bleeding edge. |
| `rand` | 0.10.1 | 0.11.x (none yet) | Just bumped in v1.1. |

**`cargo-deny` / `cargo-audit` integration:** v1.1 research called this out as a "polish-milestone-appropriate" addition. v1.1 didn't ship it. v1.2 is *not* a polish milestone, but the same CI hygiene argument still applies — and adding two new direct deps (`reqwest` + `hmac`) is exactly the moment to lock in supply-chain gating. **Recommendation: roadmapper to consider folding `cargo-deny` into the v1.2 hygiene preamble alongside `Cargo.toml` `version` bump from `1.1.0` → `1.2.0`.**

**`Cargo.toml` `version` bump:** parallel to v1.1's first commit pattern. Phase 15 (the first v1.2 phase) should include a single commit that bumps `version = "1.1.0"` → `version = "1.2.0"`, updates the README badge / Docker label / CHANGELOG draft. At first rc cut, tag `v1.2.0-rc.1` and publish GHCR `:1.2.0-rc.1` + rolling `:rc`.

**Confidence: HIGH** on the audit table (versions verified against crates.io API 2026-04-25); HIGH on the "no major-version transitions during v1.2" recommendation.

---

### 1.6 What's NOT in scope (per the question's explicit exclusions)

- **Tagging UI:** server-rendered HTML + HTMX filter chips. Same pattern as the v1.1 dashboard sort/filter. No JS framework, no chart library, no new crate.
- **Exit-code histogram rendering:** server-rendered HTML using `<div>` bars with `style="width: {{ pct }}%; background: var(--cd-status-...)"`. Same pattern as v1.1 sparklines (handcrafted SVG `<polyline>`) and the v1.1 `/timeline` page (handcrafted SVG `<rect>`). **No `plotters`, no `charming`, no chart crate of any kind.** The terminal-green design system already provides the visual vocabulary; bars are a `format!()` away.

---

## 2. Recommended Stack Additions

### Core Technologies — **no changes**

All locked v1.0 + v1.1 core technologies remain current and correct.

### Supporting Libraries — **two new pins**

| Library | Version | Purpose | When to Use | Status in v1.2 |
|---------|---------|---------|-------------|----------------|
| **`reqwest`** | `0.12.28` | Outbound HTTP client for webhook delivery | The single webhook dispatcher task. Configured rustls-only (zero `openssl-sys` impact), HTTP/1.1 + HTTP/2 outbound, JSON body, 10s default timeout. | **NEW for v1.2** — introduced by Phase 15 (or whichever phase lands the webhook delivery skeleton) |
| **`hmac`** | `0.13` | HMAC-SHA256 signing of webhook payloads | Per-job optional signing key; signs request body, emits `X-Cronduit-Signature: sha256=<hex>` header. Reuses already-pinned `sha2 = "0.11"`. | **NEW for v1.2** — introduced alongside `reqwest` |

### Development Tools — **optional addition**

| Tool | Purpose | Notes | Status |
|------|---------|-------|--------|
| `cargo-deny` (optional) | Supply-chain gate (licenses + advisories + duplicate check) | v1.1 research already flagged this as appropriate; v1.2 introduces two new direct deps which is a natural moment to lock it in. Gate as a separate non-blocking CI job. | **Optional addition to v1.2 CI scope** — roadmapper decision, low-risk |

## 3. Recommended Version Bumps

**Zero hygiene bumps required.** The v1.1.0 lockfile is healthy on entry to v1.2. Caret semver already absorbs every patch release published since v1.1.0 shipped.

| Crate | From | To | Reason | Risk | Milestone |
|-------|------|----|--------|------|-----------|
| `Cargo.toml` `version` | `1.1.0` | `1.2.0` | Match target milestone on all non-tag commits (project rule) | None | v1.2 Phase 15 |

That's it. Nothing else.

## 4. Installation

`Cargo.toml` edits for v1.2:

```toml
# ===== Add to [dependencies] =====

# Outbound HTTP client for webhooks (Phase 15+)
# - rustls-only (zero openssl-sys; verified by `cargo tree -i openssl-sys`)
# - HTTP/1.1 + HTTP/2 outbound via ALPN
# - default-features=false drops native-tls + system-proxy + gzip/brotli (we don't need them)
# - charset re-added so non-utf8 webhook responses don't panic the dispatcher
reqwest = { version = "0.12.28", default-features = false, features = [
    "rustls-tls",
    "http2",
    "json",
    "charset",
] }

# HMAC-SHA256 webhook signing (Phase 15+)
# - RustCrypto pair; reuses existing sha2 = "0.11" pin (no second declaration needed)
# - hex crate for encoding already in tree at line 114 for CSRF tokens
hmac = "0.13"
```

Verification commands to run after the Phase 15 plan that adds these:

```bash
cargo tree -i openssl-sys                    # MUST return empty
cargo tree -i rustls                         # rustls should now appear (transitive via reqwest)
cargo tree -i hyper                          # Should show hyper 1.x (matches existing tree)
cargo deny check                             # If cargo-deny is wired into CI
just lint                                    # Existing CI gate
just check-no-openssl                        # Existing constraint guard (CLAUDE.md)
```

## 5. Alternatives Considered

| Recommended | Alternative | When Alternative Makes Sense |
|-------------|-------------|------------------------------|
| `reqwest 0.12` with rustls features | `hyper-util` + `hyper-rustls` + hand-rolled glue | Never — we'd be reinventing reqwest poorly. Existing `hyper-util` for the health probe stays untouched (different requirements). |
| `reqwest 0.12.28` | `reqwest 0.13.2` | Eventually — wait for the 0.13 line to mature past its first quarter. The flag rename is cosmetic; zero functional benefit for v1.2. |
| `reqwest 0.12` (async) | `ureq 3.3` (blocking) | Never inside cronduit — blocking I/O in a tokio task throttles the scheduler. ureq is the right pick for a CLI tool that is itself blocking. |
| `hmac 0.13` + `sha2 0.11` (RustCrypto) | `ring 0.17` | If the project ever adds elliptic-curve crypto (X25519, P-256 signing) — `ring` is best-in-class there and already a transitive dep via `webpki`. For pure HMAC-SHA256, RustCrypto is the cleaner pick because `sha2` is already in tree. |
| `tokio::sync::mpsc` bounded | `flume = "0.12"` | If we needed multi-consumer fanout or a sync-and-async mixed channel. We don't. |
| `tokio::sync::mpsc` bounded | One `tokio::spawn` per delivery | If the operator's expected webhook volume were >100/sec sustained. At homelab scale, the bounded mpsc + single dispatcher is the correct design. |
| `inspect_container().image` for digest | `inspect_image(name).repo_digests` | When the operator needs the *registry-canonical* digest for a CI/CD attestation use case. Not the case here — for "did the image change between runs?" the container-level ID is the right value. |
| Three-file migration for `image_digest` (add nullable; no NOT NULL flip) | Single migration NOT NULL with default `''` | For a column that *correctly* has no value for command/script jobs forever, NOT NULL is wrong. Keep nullable. |
| Server-rendered exit-code histogram (HTML + CSS bars) | `plotters` / `charming` / any chart crate | Never — same constraint as v1.1 sparklines / `/timeline`. No JS, no WASM, no heavy SVG framework. |

## 6. What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `reqwest` with default features | Pulls `native-tls` which pulls `openssl-sys` on Linux — violates the locked rustls-only constraint | `default-features = false` + `["rustls-tls", "http2", "json", "charset"]` |
| `reqwest 0.13.x` | Brand new (Feb 2026); flag rename churn for zero functional benefit; entangles a feature ship with an HTTP-client major bump | `reqwest 0.12.28` for v1.2; revisit 0.13 in a later isolated hygiene pass |
| `ureq` (any version) | Blocking I/O — wrong shape for a tokio scheduler context | `reqwest 0.12` async |
| `ring` for HMAC-SHA256 | Heavier than RustCrypto; brings a C/asm component for primitives we don't use; not aligned with the project's pure-Rust + rustls posture | `hmac = "0.13"` + reuse `sha2 = "0.11"` |
| `flume` for the webhook queue | Adds a second channel runtime alongside tokio's; gains nothing at homelab scale | `tokio::sync::mpsc::channel(256)` (already in tree) |
| `tokio::spawn` per webhook delivery | Unbounded concurrency, no observable queue depth, awkward graceful shutdown | mpsc + single dispatcher task |
| `inspect_image(name).repo_digests` for the failure-context digest delta | Returns registry-canonical digests (verbose, sometimes empty for local builds, varies between registries) | The already-captured `inspect_container().image` value at `src/scheduler/docker.rs:240` |
| Adding a bollard 0.21+ pre-release | bollard 0.20.2 is current stable; nothing in v1.2 needs newer | Stay on `bollard = "0.20"` (caret resolves to 0.20.2) |
| `notify 9.0.0-rc.3` | Still RC; no functional benefit for v1.2 work | Stay on `notify = "8.2"` |
| `sqlx 0.9.0-alpha.1` | Alpha during a feature-ship milestone | Stay on `sqlx = "0.8.6"` |
| Any chart / plotting crate for the exit-code histogram or tag UI | Heavy dep tree + violates no-JS / no-WASM / no-CDN constraints | Server-rendered HTML + CSS bars (same pattern as v1.1 sparklines / timeline) |
| Plaintext webhook secrets in the config file | Violates the existing no-plaintext-secrets posture (`SecretString` wrapper in v1.0) | Wrap webhook signing keys in `SecretString` and require env-var interpolation: `signing_key = "${WEBHOOK_HMAC_SECRET}"` |
| Loading `reqwest::Client` per-delivery | Bypasses connection pooling; expensive | Build the `Client` once at startup, pass `Arc<Client>` into the dispatcher task (reqwest's `Client` is already cheaply cloneable — internally `Arc`-wrapped) |

## 7. Stack Patterns by Variant

**Webhook dispatcher (Phase 15-ish):**

```rust
// At startup (src/web/state.rs or src/lib.rs):
let webhook_client = reqwest::Client::builder()
    .timeout(Duration::from_secs(10))            // whole-request budget
    .connect_timeout(Duration::from_secs(5))     // connect-only budget
    .user_agent("cronduit/1.2")
    .build()
    .expect("reqwest client construction");

let (webhook_tx, webhook_rx) = tokio::sync::mpsc::channel(256);

let dispatcher_client = webhook_client.clone();  // Arc-cheap clone
let dispatcher = tokio::spawn(webhook_dispatcher_loop(
    dispatcher_client,
    webhook_rx,
    shutdown_token.clone(),
));

// Producer (src/scheduler/run.rs finalize path):
if should_emit_webhook(&job, &final_status) {
    let delivery = WebhookDelivery {
        url: webhook_url,
        body: serde_json::to_vec(&payload)?,
        hmac_secret: hmac_secret_opt,
        attempts_remaining: 3,
    };
    let _ = webhook_tx.try_send(delivery); // try_send, never blocking the run loop
}
```

**HMAC signing (src/web/webhook.rs or similar):**

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;

fn sign_payload(secret: &str, body: &[u8]) -> String {
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(body);
    let signature = mac.finalize().into_bytes();
    format!("sha256={}", hex::encode(signature))
}
```

**Image-digest persistence (Phase 16-ish):**

```rust
// src/scheduler/run.rs finalize, around line 314 (status update):
sqlx::query!(
    "UPDATE job_runs
     SET completed_at = ?,
         status = ?,
         exit_code = ?,
         image_digest = ?,
         error_message = ?
     WHERE id = ?",
    completed_at,
    status_str,
    exec_result.exit_code,
    docker_result.image_digest,    // NEW for v1.2 — was discarded in v1.1
    exec_result.error_message,
    run_id,
)
.execute(&db)
.await?;
```

**Exit-code histogram + tag-chip rendering** — pure askama templates with `<div>` bars and inline CSS variables. Same shape as the v1.1 sparkline partial and the bulk-toggle filter chip pattern. Zero new deps.

## 8. Version Compatibility

No new cross-crate compatibility constraints introduced by v1.2 beyond what v1.0 + v1.1 already documented.

**One note on the new pins:**

- `reqwest 0.12.28` requires `tokio 1.x` (already pinned at 1.52) and `hyper 1.x` (already pinned). No version conflicts.
- `reqwest 0.12.x` with `rustls-tls` feature pulls `rustls 0.23.x`, `webpki-roots 0.26.x`, `tokio-rustls 0.26.x` — all transitively, all rustls-aligned with `sqlx`'s `tls-rustls` feature. **Important:** verify `cargo tree -i rustls` shows a single `rustls` major version after the addition; if two majors appear (0.21 + 0.23), that's a duplicate-dep concern worth resolving with `cargo update` and possibly an explicit `[patch.crates-io]` entry for any lagging transitive consumer. Expected outcome based on the current lockfile shape: single `rustls 0.23.x` post-add.
- `hmac 0.13` requires `sha2 = "0.11"` or compatible — already pinned; no conflict.

**`cargo tree -i openssl-sys` after the add must remain empty** — locked invariant. The `default-features = false` + `rustls-tls` configuration on reqwest is the contract that preserves this. **CI guard `just check-no-openssl` (already in tree) is the regression lock.**

## 9. Confidence Assessment

| Area | Confidence | Basis |
|------|------------|-------|
| `reqwest 0.12.28` rustls-only over hyper-util upgrade or ureq | HIGH | Verified versions on crates.io 2026-04-25; verified rustls feature flags on docs.rs/reqwest/0.12.28; hyper-util upgrade quantified at ~300 LoC of reinvention; ureq disqualified by blocking-I/O incompatibility with tokio runtime |
| `hmac 0.13` over `ring 0.17` | HIGH | Verified `hmac` 0.13.0 on crates.io 2026-04-25 (368M+ downloads, RustCrypto-maintained, last release 2026-03-29); confirmed `sha2 = "0.11"` already pinned and reusable; `ring` argument turns on cross-compile cleanliness which has historical evidence at this project (CLAUDE.md openssl-sys constraint) |
| `inspect_container().image` already gives sha256:... digest | HIGH | Direct source-read of `src/scheduler/docker.rs:240` confirms the call is implemented and the value is captured; docs.rs/bollard/0.20.2 confirms `ContainerInspectResponse.image` is documented as "the ID (digest) of the image that this container was created from"; current schema (`migrations/sqlite/`, `migrations/postgres/`) confirmed to lack an `image_digest` column |
| `tokio::sync::mpsc` over `flume` | HIGH | Producer/consumer cardinality matches single-dispatcher model; flume's properties (multi-consumer, sync-async mix) are not used; tokio mpsc is already enabled by `["full"]`; mirrors the existing log-pipeline pattern at `src/scheduler/log_pipeline.rs` |
| Zero hygiene bumps required for v1.2 | HIGH | Crates.io API verification of all 30+ pinned crates as of 2026-04-25; caret semver already absorbs every patch release; no major-version transitions where the new majors are mature enough for an OSS feature ship |
| No new chart / plotting crate for exit-code histogram or tag UI | HIGH | Same constraint argument as v1.1 sparklines / timeline; the v1.1 STACK.md already enumerated and rejected the plotting crate ecosystem |
| `Cargo.toml` `version` bump 1.1.0 → 1.2.0 on first commit | HIGH | Follows the locked "tag = Cargo.toml version at tag time" rule (CLAUDE.md memory); precedent set by v1.1's first-commit pattern |
| Webhook secrets via `SecretString` + env-var interpolation | HIGH | Existing v1.0 posture; webhook signing keys are exactly the kind of credential `SecretString` exists for |

## 10. Open Questions (for roadmapper / phase planning)

1. **`cargo-deny` scope** — v1.2 hygiene preamble or defer to v1.3? Recommended for v1.2 given the introduction of two new direct deps (`reqwest`, `hmac`) is a natural pivot moment for supply-chain gating. Roadmapper decision.
2. **`reqwest` connection pooling tuning** — defaults are reasonable for homelab scale; only worth touching if profiling shows pool contention. Out of scope for stack research.
3. **HMAC algorithm flexibility** — fixed at SHA-256 to match the GitHub webhook convention, or configurable to SHA-512 / SHA-1 (HMAC-SHA1 is still the default for some legacy receivers like Stripe pre-2019)? Recommend SHA-256 only for v1.2 — extend later if a real operator request surfaces. **Pattern lock candidate** for the requirements step.
4. **Webhook dispatcher backpressure metric name** — `cronduit_webhook_queue_depth` (gauge) + `cronduit_webhook_dropped_total` (counter) suggested. Roadmapper to align with the existing metrics naming convention from v1.0 Phase 6.
5. **Image-digest column type on Postgres** — `TEXT` is sufficient; `CHAR(71)` (the length of `sha256:` + 64 hex) would be over-tight given Docker can theoretically emit other algorithms in the future. Recommendation: `TEXT NULL` on both backends. **Pattern lock candidate.**
6. **Tagging persistence model** — tags live on the in-memory job (TOML-driven, ephemeral) or in a DB table (`job_tags`)? For UI-only filtering with no metrics-cardinality impact (per PROJECT.md), the simpler path is in-memory only; no schema migration needed. Roadmapper decision but the in-memory path is the lighter shape and matches the "tags don't affect webhooks/search/metrics" constraint.

None of these open questions block research handoff to the requirements / roadmapper steps.

## Sources

- **Cronduit `Cargo.toml`** (v1.1.0, read 2026-04-25) — current dep pins; 30+ crates verified
- **Cronduit `src/scheduler/docker.rs`** (read 2026-04-25, lines 140-251) — confirmed `inspect_container().image` digest extraction is already implemented; confirmed label-build site for SEED-001 plumb-through
- **Cronduit `src/scheduler/run.rs`** (read 2026-04-25, lines 264-299) — confirmed `image_digest` flows through executor return path; not currently persisted
- **Cronduit `src/cli/health.rs`** (read 2026-04-25) — confirmed existing hyper-util client is purpose-built for loopback health probe; not a candidate for webhook reuse
- **Cronduit `migrations/sqlite/` + `migrations/postgres/`** (enumerated 2026-04-25) — confirmed no `image_digest` column exists; v1.2 must add it
- **`.planning/seeds/SEED-001-custom-docker-labels.md`** — confirms `Config::labels` plumb-through is already designed and pre-locked at seed time
- **`.planning/PROJECT.md`** — v1.2 milestone scope, constraints, release strategy
- **`.planning/milestones/v1.1-research/STACK.md`** — v1.1 stack delta baseline (referenced for shape, not re-litigated)
- **`.planning/milestones/v1.0-research/STACK.md`** — locked v1.0 stack baseline (not re-researched)
- **crates.io API 2026-04-25** — latest versions for `reqwest` (0.12.28 stable / 0.13.2 latest major), `hmac` (0.13.0), `sha2` (0.11.0), `ring` (0.17.14), `ureq` (3.3.0), `flume` (0.12.0), `tokio` (1.52.1), `axum` (0.8.9), `bollard` (0.20.2), `sqlx` (0.8.6), `askama` (0.15.6), `croner` (3.0.1), `rand` (0.10.1), `secrecy` (0.10.3), `notify` (8.2.0), `hyper-util` (0.1.20), `testcontainers` (0.27.3) — HIGH confidence
- **docs.rs/reqwest/0.12.28** — verified ClientBuilder API: `timeout()`, `connect_timeout()`, `read_timeout()`, `use_rustls_tls()`, HTTP/2 default-on; verified rustls feature flag set: `rustls-tls`, `rustls-tls-webpki-roots`, `rustls-tls-native-roots`, `rustls-tls-manual-roots`
- **docs.rs/bollard/0.20.2** — verified `Docker::inspect_container` signature; verified `ContainerInspectResponse.image: Option<String>` documented as "The ID (digest) of the image that this container was created from"; verified `ImageInspect.repo_digests` semantics differ from container-level image ID
- **docs.rs/ureq/3.3.0** — verified ureq 3.x is blocking I/O by design; "uses blocking I/O instead of async I/O, because that keeps the API simple and keeps dependencies to a minimum" — disqualifying for tokio context
- **rustsec.org advisories** (cross-referenced 2026-04-25) — no 2026 advisories on cronduit's direct dep set; no 2026 advisories on `reqwest 0.12.x` or `hmac 0.13.x`
- **GitHub webhook signing convention** (community knowledge) — `X-Hub-Signature-256: sha256=<hex>` header format; matched by the recommended `format!("sha256={}", hex::encode(...))` pattern in §1.2

---
*Stack research for: Cronduit v1.2 "Operator Integration & Insight"*
*Researched: 2026-04-25*
