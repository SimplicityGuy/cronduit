<!-- GSD:project-start source:PROJECT.md -->
## Project

**Cronduit**

Cronduit is a self-hosted cron job scheduler with a web UI, built for Docker-native homelab environments. It's a single Rust binary that runs recurrent tasks — local commands, inline scripts, or ephemeral Docker containers — and gives operators a terminal-green web dashboard to see exactly what ran, when, and how it went.

**Core Value:** **One tool that both runs recurrent jobs reliably AND makes their state observable through a web UI.** If everything else is cut, the scheduler must (1) execute jobs on time with full Docker networking support (especially `--network container:<name>` for VPN setups) and (2) let the operator see pass/fail, logs, and timing from a browser.

### Constraints

- **Tech stack (locked)**: Rust backend using `bollard` for the Docker API. No CLI shelling out. No alternative languages for v1.
- **Persistence (locked)**: `sqlx` with SQLite default and PostgreSQL optional. Same logical schema, per-backend migration files where dialect requires. Separate read/write SQLite pools (WAL + busy_timeout) per pitfalls research.
- **Frontend (locked)**: Tailwind CSS + server-rendered HTML via `askama_web` 0.15 with the `axum-0.8` feature (NOT the deprecated `askama_axum` crate). HTMX-style live updates. No React/Vue/Svelte.
- **Config format (locked)**: TOML. `serde-yaml` is archived on GitHub and the YAML ecosystem is fragmented; research phase confirmed TOML is the right call.
- **Cron crate (locked)**: `croner` 3.0 — DST-aware (Vixie-cron-aligned), supports `L`/`#`/`W` modifiers, has human-readable descriptions. NOT the `cron` crate or `saffron` (abandoned 2021).
- **TLS / cross-compile (locked)**: rustls everywhere. `cargo tree -i openssl-sys` must return empty. Multi-arch (amd64 + arm64) via `cargo-zigbuild`, not QEMU emulation.
- **Deployment shape**: Single binary + Docker image. Cronduit itself runs inside Docker, mounting the host Docker socket.
- **Security posture**: No plaintext secrets in the config file; interpolate from env, wrap in a `SecretString` type. Config mounted read-only. **Default bind `127.0.0.1:8080`**; loud startup warning if bind is non-loopback. Web UI ships unauthenticated in v1 (see Out of Scope) — operators are expected to either keep Cronduit on loopback / trusted LAN or front it with their existing reverse proxy. Threat model documented in `THREAT_MODEL.md`; README leads with a security section.
- **Quality bar**: Tests + GitHub Actions CI from phase 1. Clippy + fmt gate on CI. CI matrix covers `linux/amd64 + linux/arm64 × SQLite + Postgres`. README sufficient for a stranger to self-host.
- **Design fidelity**: Web UI must match `design/DESIGN_SYSTEM.md` (Cronduit terminal-green brand), not ship in default Tailwind look.
- **Documentation**: All diagrams in any project artifact (planning docs, README, PR descriptions, code comments) must be authored as mermaid code blocks. No ASCII art diagrams.
- **Workflow**: All changes land via pull request on a feature branch. No direct commits to `main`.
<!-- GSD:project-end -->

<!-- GSD:stack-start source:research/STACK.md -->
## Technology Stack

## TL;DR (decisions the roadmap can lock)
## Recommended Stack
### Core Technologies
| Technology | Version | Purpose | Why Recommended | Confidence |
|------------|---------|---------|-----------------|------------|
| **rustc** (stable) | 1.85+ | Compiler | Edition 2024 available; locked stable (no nightly features needed) | HIGH |
| **tokio** | 1.51 | Async runtime | De facto Rust async runtime; `bollard`/`sqlx`/`axum` all require it. Use `features = ["full"]` for v1, tighten before release. | HIGH |
| **axum** | 0.8.8 | HTTP server | Tokio-native, tower-based, lean API, first-class `tower-http` middleware. 0.8 is the current stable (breaking from 0.7 around `#[debug_handler]` + extractor ergonomics). | HIGH |
| **tower-http** | 0.6.8 | HTTP middleware | `TraceLayer`, `CompressionLayer`, `CorsLayer`, `ServeDir` fallback — idiomatic for axum. | HIGH |
| **bollard** | 0.20.2 | Docker API client | Locked by decision. 0.20 (March 2026) is current, async, covers every network mode we need (`container:<name>`, `host`, named networks). Maintained by `fussybeaver`. | HIGH |
| **sqlx** | 0.8.6 | Async DB | Locked by decision. Supports SQLite + Postgres from the same query surface; offline query checking via `sqlx prepare` works in CI. 0.8.x is the current line. | HIGH |
| **askama** | 0.15.6 | HTML templating | See "Templating: askama vs maud" below — compile-time type-safe Jinja templates, designer-friendly HTML files. 0.15 is post-Rinja-merge. | HIGH |
| **askama_web** | 0.15.2 | axum adapter for askama | `askama_axum` is **deprecated** (last version is literally titled `0.5.0+deprecated`). `askama_web` with the `axum-0.8` feature is the officially blessed replacement. | HIGH |
| **croner** | 3.0.1 | Cron expression parsing | See "Cron parsing" below — actively maintained, feature-rich, 5/6/7-field + macros + timezones + human descriptions. | HIGH |
| **serde** | 1.0.228 | Ser/de traits | Universal. | HIGH |
| **toml** | 1.1.2 (spec 1.1.0) | Config parsing | Locked after evaluation (see "Config format"). Toml-rs hit 1.0 in 2025, tracks TOML spec 1.1. | HIGH |
| **clap** | 4.6 | CLI arg parsing | With `derive` feature. Subcommands for `cronduit run` / `cronduit check <config>` / `cronduit migrate`. | HIGH |
| **tracing** | 0.1.44 | Structured logging | De facto standard; works with async, spans per job run. | HIGH |
| **tracing-subscriber** | 0.3.23 | Log formatting | With `env-filter` and `json` features for Docker stdout collection. | HIGH |
| **metrics** | 0.24.3 | Metrics facade | Decoupled facade (like `log`/`tracing`) — keep instrumentation code independent of the exporter. | HIGH |
| **metrics-exporter-prometheus** | 0.18.1 | `/metrics` exporter | Official exporter for the `metrics` facade; mounts as an axum route easily. | HIGH |
| **rust-embed** | 8.11.0 | Embedded static assets | Single binary goal. With `debug-embed = false` (default) it reads from disk during `cargo run` → Tailwind edit-refresh loop works without rebuilds. | HIGH |
| **chrono** | 0.4.44 | Timestamps & timezones | `croner` + `sqlx` both integrate cleanly with `chrono`. `time 0.3` is the other option but `chrono` is more common in scheduling code and `croner` examples use it. | HIGH |
### Supporting Libraries
| Library | Version | Purpose | When to Use | Confidence |
|---------|---------|---------|-------------|------------|
| **anyhow** | 1.0.102 | Error aggregation (app layer) | Top-level main, config loading, anywhere context-chaining helps debugging | HIGH |
| **thiserror** | 2.0.18 | Error derive (lib layer) | For the scheduler core + job executor crates if we split them out | HIGH |
| **axum-htmx** | 0.8.1 | HTMX request/response helpers | `HxRequest` extractor for partial rendering, `HxTrigger`/`HxRedirect` responders for "Run Now" | MEDIUM (small crate but well-scoped and current) |
| **humantime** | 2.3.0 | Parse `"90m"`, `"2h"` | `random_min_gap`, `timeout`, `config_reload_interval` in the TOML config | HIGH |
| **humantime-serde** | 1.1.1 | Serde adapter for humantime | `#[serde(with = "humantime_serde")]` on duration fields | HIGH |
| **notify** | 8.2.0 | File-watch config reload | Optional — combine with SIGHUP; lets edits to `cronduit.toml` trigger a reload without SIGHUP. `notify` is the standard cross-platform watcher. | MEDIUM (nice-to-have; can defer to Phase 2) |
| **tokio-util** | 0.7.18 | Cancellation tokens | Graceful shutdown: `CancellationToken` cascades to running job futures. | HIGH |
| **axum-extra** | 0.12.5 | Useful extractors | `Query<T>` with `serde_qs`, typed headers. Optional. | MEDIUM |
| **uuid** | 1.x | Run IDs | Store `job_runs.id` as ULID/UUID for external references. | HIGH |
| **rand** | 0.8/0.9 | `@random` schedule picker | For randomized cron field evaluation. | HIGH |
| **shell-words** | 1.x | Command tokenization | When a user writes `command = "curl -sf https://x.y"`, we need argv splitting (don't invoke a shell for `type = "command"`). | HIGH |
### Development / Quality Tools
| Tool | Purpose | Notes | Confidence |
|------|---------|-------|------------|
| **cargo fmt** | Formatter | Required CI gate. `rustfmt.toml` with `edition = "2024"`. | HIGH |
| **cargo clippy** | Linter | CI: `cargo clippy --all-targets --all-features -- -D warnings`. | HIGH |
| **cargo-nextest** | Faster test runner | 2-3x faster than `cargo test`, better output. Install on CI via `taiki-e/install-action`. | HIGH |
| **cargo-deny** | Supply-chain gate | License + advisory + duplicate checks. Matters for an OSS release. | MEDIUM |
| **sqlx-cli** | Offline query prepare | Run `cargo sqlx prepare` pre-commit so CI doesn't need a live DB for `query!` macro checks. | HIGH |
| **testcontainers** | 0.27.2 | Docker-backed integration tests | Spin up real `alpine`/`postgres` containers inside tests. Only run on the integration tier (feature gate + CI job). | HIGH |
| **testcontainers-modules** | 0.15.0 | Prebuilt Postgres module | Saves writing `GenericImage` setup for the `sqlx`-Postgres test suite. | HIGH |
| **tailwindcss (standalone binary)** | 3.4.x | CSS build | The **standalone** Tailwind binary (no Node) fits the single-binary philosophy. Wire it through a `build.rs` or a dedicated `make css` step that writes to `assets/static/app.css`. | HIGH |
| **htmx** | 2.0.x (CDN or vendored) | Live updates | Vendor into `assets/vendor/htmx.min.js` and embed via `rust-embed`. Don't load from a CDN — breaks the single-binary promise. | HIGH |
## Critical Decision 1 — Templating: askama vs maud
### Why askama
### Why not maud
- Maud's "HTML in a macro" is beautiful for small snippets but strictly *more* Rust, *less* HTML. For a project where the design system lives in hand-written HTML/CSS that we want to paste in from `design/showcase.html`, maud forces a conversion step every time.
- No template inheritance story. Our layout is big enough (base + nav + job list + detail + run detail) that we want `{% extends %}`.
- Smaller ecosystem of axum examples in 2025 (askama has `askama_web` as a blessed adapter; maud has a short `Render for Markup` impl but fewer reference applications).
### Gotcha to warn the roadmap about
### Alternatives considered
- **maud 0.27** — best-in-class for "HTML DSL inside Rust" projects. Not right here.
- **tera** — runtime templates; we lose compile-time safety and startup checks, no reason to pick it.
- **minijinja** — Rinja's runtime cousin. Also not right: we want compile-time checked templates in a single binary.
## Critical Decision 2 — Cron parsing: croner vs cron vs saffron
### Why croner
| Feature | croner 3.0 | cron 0.16 | saffron 0.1 |
|---------|------------|-----------|-------------|
| 5-field (POSIX/Vixie) | Yes | Yes | Yes |
| 6-field (seconds) | Yes | Yes | No |
| `@hourly` `@daily` macros | Yes | Yes | Partial |
| `L` / `#` / `W` modifiers | **Yes** | No | No |
| Timezone support (chrono-tz) | **Yes** | Via chrono | No |
| Human-readable descriptions | **Yes** | No | No |
| Actively maintained 2025/26 | **Yes** (2026-04-08) | Yes (2026-03-25) | **No** (last release 2021) |
| `next_after(DateTime)` API | Yes | Yes | Yes |
- The extended Quartz-ish modifiers homelab users will ask for (`0 0 L * *` = "last day of month")
- Human-readable strings we can show in the UI (`"Every hour at minute 0"` next to the raw expression — big UX win)
- A timezone-aware `next_after` so `@daily` means *midnight in the operator's TZ*, not UTC-00:00
### Why not `cron` (the obvious pick)
- No `L`/`#`/`W` support. Homelab backup scripts routinely want "last Sunday of the month" (`0 3 ? * 7L` in Quartz). With `cron` we'd either ship a limited subset or parse ourselves.
- No human descriptions; we'd pull `cron_descriptor` separately and keep two parsers in sync.
### Why not `saffron`
### `@random` — we implement it ourselves
### Scheduler loop — *not* `tokio-cron-scheduler`
- It wraps `cron` (no `L`/`#`/`W`).
- It has its own persistence layer we don't want.
- We need fine control over `@random` resolution + `random_min_gap` + per-job timeouts + graceful shutdown. The simple hand-rolled loop (`tokio::select!` on `next_tick`, `shutdown.cancelled()`, and a `JoinSet` of running jobs) is ~150 lines and we own the semantics. Cheaper than fighting a library.
## Critical Decision 3 — Config format: TOML vs YAML (vs INI/JSON)
### YAML in Rust is actively broken in 2025/26
- **`serde-yaml` (dtolnay) is archived.** GitHub archive flag = true. Last release 2024-03, description: *"+deprecated"*. This was *the* YAML crate for Rust for ~5 years. It's done.
- The ecosystem has fragmented into `serde_yml`, `serde_norway`, `serde-yaml-bw`, and others. None have the trust/adoption of the archived original. `serde_yml` had its own governance drama in 2024 and recent downloads have stalled.
- For a public OSS project where dependency hygiene matters, pinning to any of these is a maintenance liability — every user who runs `cargo tree` will see the broken state.
- `toml` crate hit **1.0 in 2025** and tracks TOML spec 1.1.0. Released 1.1.2 on 2026-04-01 (updated *this week* of the research date). Maintained by the toml-rs org, same folks behind `cargo`'s TOML handling.
- `serde` integration is first-class. `#[derive(Deserialize)]` on your config struct → `toml::from_str(&s)?` → done.
- Error messages include line numbers and the offending key path.
### Hand-written ergonomics for our config shape
### INI and JSON
- **INI** — insufficient. No arrays of tables, no nesting, no way to express `volumes = ["a", "b"]` or `env = { KEY = "v" }`. Rejected outright.
- **JSON** — hostile to hand-writing (no comments, trailing commas, quoted keys). Good as an *export* format but not primary. Rejected.
### Env-var interpolation (`${ENV_VAR}`)
- `figment` (Rocket's config lib) is powerful but heavy for our one-file use case, and it ties us into its provider model.
- The `config` crate supports layering, but we explicitly want **one file is the source of truth** (per the spec's sync behavior). Layering would muddy the "disable jobs not in the file" rule.
- A 30-line regex pass + `toml::from_str` is auditable and does exactly what we need.
### Defaults handling
#[derive(Deserialize)]
### Decision: TOML. Lock it.
## HTMX Integration Approach
### 1. Polling for dashboard freshness (simplest, ship first)
### 2. Server-Sent Events for live run logs (Phase 2+)
### 3. "Run Now" + toast via `HX-Trigger` response header
### Alternatives considered & rejected
- **Datastar** (`datastar` crate) — newer HTMX competitor. Interesting but low adoption (13k downloads), not worth pioneering on an OSS launch project.
- **Leptos / Dioxus** — full SSR frameworks. Overkill and violates the "no SPA" constraint.
- **Raw JS fetch + innerHTML** — you'll reinvent HTMX badly.
## Observability Stack
### Logging
### Metrics
- Decouples instrumentation from export format (future-proof if we ever emit OpenTelemetry).
- Macros (`counter!`, `histogram!`, `gauge!`) are lightweight.
- `metrics-exporter-prometheus` renders `/metrics` with the correct text format.
- `cronduit_jobs_total` (gauge) — number of configured jobs
- `cronduit_runs_total{job, status}` (counter) — `status ∈ {success,failed,timeout,cancelled}`
- `cronduit_run_duration_seconds{job}` (histogram)
- `cronduit_failures_total{job}` (counter; redundant with runs_total{status="failed"} but explicit for alerting)
- `cronduit_scheduler_up` (gauge, constant `1` — standard liveness)
### Rejected
- **`prometheus` (the `tikv/rust-prometheus` crate)** — solid but uses its own registry model. Ties instrumentation to the exporter, which I want to avoid.
- **`prometheus-client`** — better than `prometheus` for OpenMetrics but we still prefer the `metrics` facade for the ecosystem benefit.
## Test Stack
### Layer 1 — Unit tests (`cargo test`)
### Layer 2 — Integration tests (`cargo test --features integration`)
- **Postgres tests:** `testcontainers-modules::postgres::Postgres` → run full schema migrations → `sqlx::query!` tests against real DB. SQLite tests use an in-memory DB, no container needed.
- **Bollard tests:** use `testcontainers-rs` to confirm we can spawn/inspect/remove a container through the exact code path the scheduler uses. Test `network_mode = "none"` and `network_mode = "host"` — the critical ones for the feature promise.
- **`container:<name>` test:** start a sidecar container (e.g., a plain `alpine sleep`), then spawn a second container with `network_mode = "container:<name>"`, confirm it joins the namespace. This is the marquee feature — we *must* test it.
### Layer 3 — `cargo nextest`
- uses: taiki-e/install-action@nextest
- run: cargo nextest run --all-features --profile ci
### End-to-end
## CI Stack (GitHub Actions, idiomatic 2025/26)
### Workflow outline
### Standard action versions (verified currency)
| Action | Version | Notes |
|--------|---------|-------|
| `actions/checkout` | v4 | v5 exists on some runners; v4 is safe default. |
| `dtolnay/rust-toolchain` | `stable` | Tag, not version. The idiomatic Rust toolchain installer in 2025. |
| `Swatinem/rust-cache` | v2 | Caches target/ and registry; essential. |
| `taiki-e/install-action` | (no version) | Use `with: tool: nextest` pattern; maintained. |
| `docker/setup-qemu-action` | v3 | Required for arm64 on amd64 runners. |
| `docker/setup-buildx-action` | v3 | |
| `docker/build-push-action` | v6 | GHA cache backend `type=gha` is the right choice. |
| `docker/login-action` | v3 | |
### Multi-arch Docker build — the approach
# Stage 1: planner (compute cargo-chef recipe for dep caching) — optional but recommended
# Use zigbuild for cross target builds
### Release tagging
## Embedded Assets: rust-embed vs include_dir
- **rust-embed** has a debug mode where assets are read from disk live, so Tailwind edits + template tweaks show on refresh without `cargo build`. This is a 10x faster inner loop during UI work.
- **include_dir** is simpler (pure `const`) but embeds everything even in debug mode, forcing a rebuild on every CSS change.
### Asset layout
## Installation (Cargo.toml sketch)
# Runtime
# Web
# Templates
# Docker
# Database
# Cron
# Config
# CLI
# Observability
# Errors
# Utilities
## Alternatives Considered (Summary)
| Decision | Recommended | Alternative | When Alternative Makes Sense |
|----------|-------------|-------------|------------------------------|
| Templating | askama 0.15 | maud 0.27 | If you'd rather write HTML-in-Rust macros and don't need template inheritance. Not our case. |
| Cron parser | croner 3.0 | cron 0.16 | If you only need basic 5-field cron and want minimum deps. Loses `L`/`#`/`W` and human descriptions. |
| Cron parser | croner 3.0 | saffron 0.1 | **Never** — abandoned 2021. |
| Config format | TOML | YAML | Never for Rust in 2026 (archived serde-yaml). |
| Config format | TOML | JSON | If config were machine-generated, not hand-written. |
| Scheduler loop | hand-rolled on tokio | tokio-cron-scheduler 0.15 | If we didn't need `@random` + `random_min_gap`. Not our case. |
| Config layering | plain `toml::from_str` + regex env expand | figment / config crates | If we had multi-file layered config (dev/prod/local). We explicitly don't — one file is source of truth. |
| Embedded assets | rust-embed 8.11 | include_dir 0.7 | If you never want live-reload and want the smallest dep. |
| HTTP middleware | tower-http 0.6 | hand-roll middleware | Never — tower-http is the de facto standard. |
| Metrics | metrics facade + prom exporter | prometheus crate directly | If you want a single-binary-registry model and will never change export format. |
| Logs | tracing + tracing-subscriber | log + env_logger | Never for async-heavy apps like this. |
| Errors (app) | anyhow | eyre / color-eyre | color-eyre is nice for CLI output if you want colorful panics; worth for v2. |
| Docker client | bollard | shiplift | shiplift is unmaintained. Locked to bollard anyway. |
| Test runner | cargo-nextest | cargo test | cargo test is fine; nextest is better. |
| CI Docker | buildx + cargo-zigbuild | QEMU + native cargo | If build times don't matter. They do for OSS CI credits. |
## What NOT to Use
| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `serde-yaml` (dtolnay) | **Archived**. Announced end-of-life. | Don't use YAML — use TOML. |
| `serde_yml`, `serde_norway`, `serde-yaml-bw` | Ecosystem fragmentation, none are universally trusted. | TOML. |
| `askama_axum` | Deprecated; last version is literally `0.5.0+deprecated`. | `askama_web` with `axum-0.8` feature. |
| `saffron` | Unmaintained since 2021. | `croner 3.0`. |
| `tokio-cron-scheduler` | Wraps `cron` (no extended modifiers) + its own persistence. Not a fit for `@random`. | Hand-rolled loop on `tokio` + `croner`. |
| `shiplift` | Unmaintained Docker client. | `bollard` (locked). |
| `log` + `env_logger` for a tokio app | Doesn't carry span context across await points. | `tracing` + `tracing-subscriber`. |
| `prometheus` crate directly | Couples instrumentation to exporter. | `metrics` facade + `metrics-exporter-prometheus`. |
| `docker` CLI shelling from Rust | Why we're building this project in the first place. | `bollard` (locked). |
| Tailwind via Node/npm | Adds Node to the build toolchain, kills the single-binary story. | **Standalone Tailwind binary** (official, static). |
| Loading HTMX from a CDN | Requires internet at first page load; breaks offline/airgap homelabs. | Vendor `htmx.min.js` into `assets/vendor/` and embed. |
| `docker buildx` + QEMU without zigbuild for arm64 | CI times explode (~20m for arm64 emulated). | `cargo-zigbuild` cross-compile on amd64 runner. |
| YAML for **any** cron config | Bare cron expressions parse as YAML sequences. Footgun-as-a-service. | TOML. |
| `figment` / `config` crates | Over-engineered for one-file source-of-truth; fights our sync-from-file semantics. | Plain `toml::from_str` + a tiny env-expand pass. |
## Version Compatibility Notes
| Constraint | Compatible Line | Notes |
|------------|-----------------|-------|
| `axum 0.8.x` | `tower-http 0.6.x`, `axum-htmx 0.8.x`, `askama_web` with `axum-0.8` feature | axum 0.8 bumped its `http` crate dep; pin tower-http to 0.6 (not 0.5). |
| `sqlx 0.8.x` | `tokio 1.x`, `chrono 0.4.x` | Use `runtime-tokio` + `tls-rustls` features; the `tls-native-tls` path pulls OpenSSL. |
| `bollard 0.20.x` | `tokio 1.x`, `hyper 1.x` | 0.20 moved to hyper 1; don't mix with old middleware expecting hyper 0.14. |
| `askama 0.15` | `askama_web 0.15` | **Must** match major.minor. `askama_axum` is defunct. |
| `rust-embed 8.x` | Rust 1.75+ | `debug-embed` feature toggles disk reads in debug builds. |
| `testcontainers 0.27` | `testcontainers-modules 0.15` | These pairs track each other; always bump together. |
| `metrics 0.24` | `metrics-exporter-prometheus 0.18` | The exporter lags the facade by a version; always verify. |
| `croner 3.x` | `chrono 0.4.x` (optional feature) | Enable the `chrono` integration feature for `DateTime`-aware `next_after`. |
## Stack Patterns by Variant
- SQLite, no migrations runner, embedded HTMX, no auth, `/metrics` open on LAN.
- One binary, single docker-compose, docker socket mount.
- Swap `DATABASE_URL` to Postgres — schema is the same.
- Put cronduit behind Traefik/Caddy for TLS + basic auth.
- Scrape `/metrics` from Prometheus; no code change.
- Still a single instance (no HA in v1/v2).
- `rust-embed` with `debug-embed = false` (default) reads from disk in debug builds → edit + refresh.
- Run Tailwind CLI in `--watch` mode in a second terminal writing to `assets/static/app.css`.
- Don't need `cargo-watch` for template-only changes thanks to disk reads.
## Confidence Assessment
| Area | Confidence | Basis |
|------|------------|-------|
| Crate versions | HIGH | All verified against crates.io API on 2026-04-09 (research date). |
| askama over maud | HIGH | Verified askama 0.15 / `askama_web` as current path; `askama_axum` deprecation explicit in crate description. |
| croner over cron | HIGH | Feature matrix is observable in both repos; croner repo last push 2026-04-08. |
| TOML over YAML | HIGH | `serde-yaml` GitHub archive flag = true (verified via API). Alternative crates confirmed fragmented. |
| Hand-rolled scheduler | MEDIUM-HIGH | Architectural judgment based on `@random` requirements; roadmap should sanity-check the executor loop in Phase 1 spike. |
| HTMX integration pattern | HIGH | Standard idiomatic 2025 pattern; `axum-htmx` 0.8.1 verified current. |
| CI pipeline shape | HIGH | Standard 2025/26 Rust-on-GHA idioms; action versions verified. |
| cargo-zigbuild for multi-arch | MEDIUM | Well-established technique, but worth verifying against current `rust:alpine` base image during Phase 1. |
| Metrics facade choice | HIGH | `metrics` facade is the idiomatic modern Rust approach. |
## Sources
- **crates.io API** (2026-04-09) — version verification for all listed crates. Queried:
- **GitHub API** (2026-04-09) — repository health & archive status:
- **askama README** (github.com/askama-rs/askama) — merge of askama ↔ rinja history, recommended usage.
- **croner-rust README** (via WebFetch) — feature matrix for cron syntax support (5/6/7-field, `L`/`#`/`W`, macros, timezones, descriptions).
- **`askama_axum` crate description** on crates.io — verbatim: *"Integration crates like `askama_axum` were removed from askama 0.13."*
- **`serde_yaml` crate description** on crates.io — version `0.9.34+deprecated`.
- **Cronduit SPEC.md** and **PROJECT.md** — context for locked decisions (Rust, bollard, sqlx, Tailwind, SSR), soft-locked TOML, the `@random` + `random_min_gap` requirement, the `container:<name>` network mode promise that drives the bollard + integration-test decisions.
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

Conventions not yet established. Will populate as patterns emerge during development.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

Architecture not yet mapped. Follow existing patterns found in the codebase.
<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->
## Project Skills

No project skills found. Add skills to any of: `.claude/skills/`, `.agents/skills/`, `.cursor/skills/`, or `.github/skills/` with a `SKILL.md` index file.
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->
## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->



<!-- GSD:profile-start -->
## Developer Profile

> Profile not yet configured. Run `/gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
