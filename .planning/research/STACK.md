# Stack Research — Cronduit

**Domain:** Self-hosted Docker-native cron scheduler with server-rendered web UI (Rust)
**Researched:** 2026-04-09
**Overall confidence:** HIGH (versions and critical decisions verified against crates.io and official sources; see per-row confidence)

## TL;DR (decisions the roadmap can lock)

1. **Templating:** `askama 0.15` + `askama_web 0.15` (not `maud`). Compile-time Jinja-like templates, `askama_axum` is deprecated — the new path is `askama_web` feature-flagged for axum.
2. **Cron parsing:** `croner 3.0` (not `cron`, not `saffron`). Actively maintained 2025, supports 5/6/7-field + `@hourly`-style macros + `L`/`#`/`W`, timezone-aware, and gives human-readable descriptions "for free" (useful in the UI).
3. **Config format:** **TOML stays.** Locked. `toml 1.1` + `serde` + a manual env-var interpolation pass (`${VAR}`). YAML is *actively hazardous* in Rust right now — `serde-yaml` is archived and the whole ecosystem is fragmented across several half-maintained forks.
4. **Web framework:** `axum 0.8` + `tower-http 0.6` + `axum-htmx 0.8`. HTMX over SSE/polling for live status — no SPA, no JSON API surface beyond `/health` + `/metrics`.
5. **Embedded assets:** `rust-embed 8.11` (not `include_dir`). Has a live-reload dev feature that's worth the ergonomics for Tailwind iteration.
6. **Scheduler loop:** **Hand-rolled** on `tokio` + `croner` (not `tokio-cron-scheduler`). We need custom `@random` + `random_min_gap` semantics; an off-the-shelf scheduler would be fought more than used.
7. **Logging / metrics:** `tracing` + `tracing-subscriber` (JSON) + `metrics 0.24` + `metrics-exporter-prometheus 0.18`. Standard 2025 stack.
8. **Tests:** `cargo test` + `cargo-nextest` + `testcontainers-rs 0.27` for the Docker-dependent integration layer.
9. **CI:** GitHub Actions with `dtolnay/rust-toolchain@stable`, `swatinem/rust-cache@v2`, `taiki-e/install-action@nextest`, `docker/build-push-action@v6` with QEMU for `linux/amd64,linux/arm64`.

---

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

---

## Critical Decision 1 — Templating: askama vs maud

**Recommendation: askama** (0.15) with `askama_web` for axum. **Confidence: HIGH.**

### Why askama

1. **Designer-friendly HTML files.** Templates live in `templates/*.html` as real HTML with Jinja tags. Tailwind class strings stay legible, you can open a template in a browser preview, and the Cronduit design system (terminal-green, monospace, specific class tokens) will translate from `design/showcase.html` directly.
2. **Inheritance and partials.** `{% extends "base.html" %}` + `{% block content %}` is exactly how we'll want Dashboard/Job-detail/Run-detail to share layout. Maud has `html!` macros but no real template inheritance — you'd end up inlining partials as Rust functions.
3. **HTMX partials are natural.** A `JobRowPartial` template rendering a single `<tr>` is trivial in askama; for HTMX responses we return the same partial type with a different root template. In maud you'd also do this but the Rust compiler errors when you miss a field are less template-shaped.
4. **Compile-time checking.** Both give it, but askama's is bound to the template file so errors reference the `.html` line.
5. **2025 is the right moment for askama.** Askama merged with Rinja in late 2025 → 0.15 is a fresh, consolidated line (active development, 0.15.6 released 2026-03-24). The old ecosystem split (askama vs rinja) is resolved.

### Why not maud

- Maud's "HTML in a macro" is beautiful for small snippets but strictly *more* Rust, *less* HTML. For a project where the design system lives in hand-written HTML/CSS that we want to paste in from `design/showcase.html`, maud forces a conversion step every time.
- No template inheritance story. Our layout is big enough (base + nav + job list + detail + run detail) that we want `{% extends %}`.
- Smaller ecosystem of axum examples in 2025 (askama has `askama_web` as a blessed adapter; maud has a short `Render for Markup` impl but fewer reference applications).

### Gotcha to warn the roadmap about

`askama_axum` is **deprecated** — the crate description literally reads *"Integration crates like askama_axum were removed from askama 0.13"*. Any tutorial older than late 2025 will tell you to use it; ignore them. The correct move is:

```toml
askama = "0.15"
askama_web = { version = "0.15", features = ["axum-0.8"] }
```

Then `#[derive(Template, WebTemplate)]` gives you `IntoResponse` for free.

### Alternatives considered

- **maud 0.27** — best-in-class for "HTML DSL inside Rust" projects. Not right here.
- **tera** — runtime templates; we lose compile-time safety and startup checks, no reason to pick it.
- **minijinja** — Rinja's runtime cousin. Also not right: we want compile-time checked templates in a single binary.

---

## Critical Decision 2 — Cron parsing: croner vs cron vs saffron

**Recommendation: `croner 3.0`.** **Confidence: HIGH.**

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

Croner is the only crate that gets us all of:
- The extended Quartz-ish modifiers homelab users will ask for (`0 0 L * *` = "last day of month")
- Human-readable strings we can show in the UI (`"Every hour at minute 0"` next to the raw expression — big UX win)
- A timezone-aware `next_after` so `@daily` means *midnight in the operator's TZ*, not UTC-00:00

### Why not `cron` (the obvious pick)

- No `L`/`#`/`W` support. Homelab backup scripts routinely want "last Sunday of the month" (`0 3 ? * 7L` in Quartz). With `cron` we'd either ship a limited subset or parse ourselves.
- No human descriptions; we'd pull `cron_descriptor` separately and keep two parsers in sync.

### Why not `saffron`

Unmaintained since 2021. Dead. Do not touch.

### `@random` — we implement it ourselves

None of these crates handle `@random`. That's fine — the design is:

1. Config parse substitutes `@random` with a generated expression before handing to `croner`.
2. Randomization honors `random_min_gap` by rejecting candidates that'd collide on the same day.
3. Persist the *resolved* expression in `jobs.resolved_schedule` so restarts without re-randomize stay deterministic.

This is a feature of the Cronduit config layer, not the cron crate.

### Scheduler loop — *not* `tokio-cron-scheduler`

We also evaluated `tokio-cron-scheduler 0.15`. Rejected:

- It wraps `cron` (no `L`/`#`/`W`).
- It has its own persistence layer we don't want.
- We need fine control over `@random` resolution + `random_min_gap` + per-job timeouts + graceful shutdown. The simple hand-rolled loop (`tokio::select!` on `next_tick`, `shutdown.cancelled()`, and a `JoinSet` of running jobs) is ~150 lines and we own the semantics. Cheaper than fighting a library.

---

## Critical Decision 3 — Config format: TOML vs YAML (vs INI/JSON)

**Recommendation: TOML. Keep it locked. Confidence: HIGH.**

This section is deliberately detailed because the user unlocked the decision.

### YAML in Rust is actively broken in 2025/26

This is the decisive factor and it's under-appreciated:

- **`serde-yaml` (dtolnay) is archived.** GitHub archive flag = true. Last release 2024-03, description: *"+deprecated"*. This was *the* YAML crate for Rust for ~5 years. It's done.
- The ecosystem has fragmented into `serde_yml`, `serde_norway`, `serde-yaml-bw`, and others. None have the trust/adoption of the archived original. `serde_yml` had its own governance drama in 2024 and recent downloads have stalled.
- For a public OSS project where dependency hygiene matters, pinning to any of these is a maintenance liability — every user who runs `cargo tree` will see the broken state.

**TOML, by contrast:**

- `toml` crate hit **1.0 in 2025** and tracks TOML spec 1.1.0. Released 1.1.2 on 2026-04-01 (updated *this week* of the research date). Maintained by the toml-rs org, same folks behind `cargo`'s TOML handling.
- `serde` integration is first-class. `#[derive(Deserialize)]` on your config struct → `toml::from_str(&s)?` → done.
- Error messages include line numbers and the offending key path.

### Hand-written ergonomics for our config shape

The v1 config has `[defaults]`, `[[jobs]]` arrays of tables, and per-job `env = { KEY = "VAL" }` inline tables. Side-by-side:

**TOML (recommended):**
```toml
[defaults]
image = "curlimages/curl:latest"
network = "container:vpn"
volumes = ["/mnt/data:/data"]
timeout = "2h"
random_min_gap = "90m"

[[jobs]]
name = "check-ip"
schedule = "@random"
command = "https://ipinfo.io"

[[jobs]]
name = "weekly-backup"
schedule = "0 3 * * 0"
type = "command"
command = "/usr/local/bin/backup.sh"
use_defaults = false
```

**YAML (rejected):**
```yaml
defaults:
  image: "curlimages/curl:latest"
  network: "container:vpn"
  volumes:
    - "/mnt/data:/data"
  timeout: 2h  # ← ambiguous! string? duration? YAML 1.1 sexagesimal?
  random_min_gap: 90m

jobs:
  - name: check-ip
    schedule: "@random"  # ← MUST quote; otherwise YAML parses as null-ish
    command: "https://ipinfo.io"
  - name: weekly-backup
    schedule: "0 3 * * 0"  # ← MUST quote; unquoted parses as list
    type: command
    command: /usr/local/bin/backup.sh
    use_defaults: false
```

Problems YAML introduces that TOML doesn't:

1. **Cron expressions *must* be quoted in YAML** — unquoted `0 3 * * 0` is parsed as a sequence. This is the single biggest footgun for a cron config format.
2. **`@random` must be quoted** — leading `@` is reserved syntax in YAML.
3. **`2h` / `90m` duration strings are risky** — YAML 1.1 parsed bare numbers with colons as sexagesimal. YAML 1.2 cleans this up but `serde-yml` behavior varies.
4. **The Norway problem** — bare `no` (e.g., a tag field value) deserializes as `false` in YAML 1.1. Real bug that shipped in production elsewhere.
5. **Indentation sensitivity** — a mis-indented per-job field silently attaches to the wrong parent. TOML's `[[jobs]]` markers make table boundaries explicit.

### INI and JSON

- **INI** — insufficient. No arrays of tables, no nesting, no way to express `volumes = ["a", "b"]` or `env = { KEY = "v" }`. Rejected outright.
- **JSON** — hostile to hand-writing (no comments, trailing commas, quoted keys). Good as an *export* format but not primary. Rejected.

### Env-var interpolation (`${ENV_VAR}`)

Implement this as a **string pre-processing pass before `toml::from_str`**, not with a config framework:

```rust
fn expand_env(raw: &str) -> Result<String, ConfigError> {
    // regex: \$\{([A-Z_][A-Z0-9_]*)\}
    // fail hard on missing vars (better than silent empty strings)
    // optionally support ${VAR:-default}
}

let raw = std::fs::read_to_string(&path)?;
let expanded = expand_env(&raw)?;
let config: Config = toml::from_str(&expanded)?;
```

Reasons we skip `figment` / `config` crates:

- `figment` (Rocket's config lib) is powerful but heavy for our one-file use case, and it ties us into its provider model.
- The `config` crate supports layering, but we explicitly want **one file is the source of truth** (per the spec's sync behavior). Layering would muddy the "disable jobs not in the file" rule.
- A 30-line regex pass + `toml::from_str` is auditable and does exactly what we need.

`shellexpand` is a tempting off-the-shelf solution but it's designed for shell-style expansion (`~` home dirs, `$VAR` unbraced, etc.) — more than we want. We want strictly `${VAR}` with a clear error on missing vars.

### Defaults handling

Implement defaults merge in Rust code, not in the deserializer:

```rust
#[derive(Deserialize)]
struct Config {
    defaults: Option<JobDefaults>,
    #[serde(default)]
    jobs: Vec<RawJob>,
}

impl Config {
    fn resolve(self) -> Vec<ResolvedJob> {
        let defaults = self.defaults.unwrap_or_default();
        self.jobs.into_iter().map(|j| {
            if j.use_defaults.unwrap_or(true) {
                j.merge_with(&defaults)
            } else {
                j.into_resolved()
            }
        }).collect()
    }
}
```

This keeps the TOML shape verbatim and makes the merge rules *testable* independently of parsing.

### Decision: TOML. Lock it.

---

## HTMX Integration Approach

**Pattern: HTMX + server-sent partials, no client-side framework, no JSON API beyond `/health` + `/metrics`.**

Three concrete techniques to use:

### 1. Polling for dashboard freshness (simplest, ship first)

```html
<div hx-get="/ui/dashboard/jobs-table"
     hx-trigger="every 5s"
     hx-swap="outerHTML">
  <!-- server-rendered table; askama partial -->
</div>
```

Server handler returns the `JobsTablePartial` template (no layout). Use `axum_htmx::HxRequest` to distinguish full-page vs partial requests from the same route if you want one endpoint; otherwise give partials their own `/ui/...` paths. I recommend **separate `/ui/...` partial routes** — clearer in logs, trivial to cache-bust.

### 2. Server-Sent Events for live run logs (Phase 2+)

While a job is running, stream stdout/stderr to the run-detail page:

```html
<div hx-ext="sse"
     sse-connect="/ui/runs/{id}/stream"
     sse-swap="log-line"
     hx-swap="beforeend">
</div>
```

Implement via `axum::response::sse::Sse` + a `tokio::sync::broadcast` channel from the executor. No new dep.

### 3. "Run Now" + toast via `HX-Trigger` response header

```rust
use axum_htmx::HxResponseTrigger;

async fn run_now(Path(id): Path<i64>, State(app): State<AppState>) -> impl IntoResponse {
    app.scheduler.trigger(id).await?;
    (
        HxResponseTrigger::normal(["jobTriggered"]),
        StatusCode::NO_CONTENT,
    )
}
```

Client-side JS listens for `jobTriggered` to flash a toast. No full framework needed.

**`axum-htmx` 0.8.1** is low-risk and handles these patterns idiomatically. Alternative: inline header handling (trivially a few lines of Rust), which is fine too if you'd rather not take the dep.

### Alternatives considered & rejected

- **Datastar** (`datastar` crate) — newer HTMX competitor. Interesting but low adoption (13k downloads), not worth pioneering on an OSS launch project.
- **Leptos / Dioxus** — full SSR frameworks. Overkill and violates the "no SPA" constraint.
- **Raw JS fetch + innerHTML** — you'll reinvent HTMX badly.

---

## Observability Stack

### Logging

```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
```

```rust
tracing_subscriber::fmt()
    .json()
    .with_env_filter(EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,cronduit=debug")))
    .with_current_span(true)
    .init();
```

One span per job run (`span!(Level::INFO, "job_run", job=%name, run_id=%id)`) — gives you correlated log output by run in any log aggregator.

### Metrics

**Use the `metrics` facade, not `prometheus` directly.** Reasons:

- Decouples instrumentation from export format (future-proof if we ever emit OpenTelemetry).
- Macros (`counter!`, `histogram!`, `gauge!`) are lightweight.
- `metrics-exporter-prometheus` renders `/metrics` with the correct text format.

```toml
metrics = "0.24"
metrics-exporter-prometheus = { version = "0.18", features = ["http-listener"] }
```

Or mount the recorder as an axum route so we don't need a second listener:

```rust
let recorder = PrometheusBuilder::new().install_recorder()?;
// in router:
.route("/metrics", get(move || async move { recorder.render() }))
```

Metrics to emit from day one (matches SPEC):

- `cronduit_jobs_total` (gauge) — number of configured jobs
- `cronduit_runs_total{job, status}` (counter) — `status ∈ {success,failed,timeout,cancelled}`
- `cronduit_run_duration_seconds{job}` (histogram)
- `cronduit_failures_total{job}` (counter; redundant with runs_total{status="failed"} but explicit for alerting)
- `cronduit_scheduler_up` (gauge, constant `1` — standard liveness)

### Rejected

- **`prometheus` (the `tikv/rust-prometheus` crate)** — solid but uses its own registry model. Ties instrumentation to the exporter, which I want to avoid.
- **`prometheus-client`** — better than `prometheus` for OpenMetrics but we still prefer the `metrics` facade for the ecosystem benefit.

---

## Test Stack

### Layer 1 — Unit tests (`cargo test`)

Pure scheduler logic, cron parsing, config resolution, defaults merging, `@random` generation with fixed seed. Must run without Docker, without network. Target: ~80% of scheduler core covered here.

### Layer 2 — Integration tests (`cargo test --features integration`)

Feature-gated. Spin up **real containers** via `testcontainers 0.27`:

- **Postgres tests:** `testcontainers-modules::postgres::Postgres` → run full schema migrations → `sqlx::query!` tests against real DB. SQLite tests use an in-memory DB, no container needed.
- **Bollard tests:** use `testcontainers-rs` to confirm we can spawn/inspect/remove a container through the exact code path the scheduler uses. Test `network_mode = "none"` and `network_mode = "host"` — the critical ones for the feature promise.
- **`container:<name>` test:** start a sidecar container (e.g., a plain `alpine sleep`), then spawn a second container with `network_mode = "container:<name>"`, confirm it joins the namespace. This is the marquee feature — we *must* test it.

These need `/var/run/docker.sock`. On CI (GitHub Actions), the `ubuntu-latest` runner has Docker pre-installed. On macOS CI (not recommended for this project) it does not.

### Layer 3 — `cargo nextest`

Install in CI for faster + prettier output:

```yaml
- uses: taiki-e/install-action@nextest
- run: cargo nextest run --all-features --profile ci
```

A `nextest.toml` with a `[profile.ci]` that retries flaky Docker-dependent tests once is worth setting up early.

### End-to-end

**Skip for v1.** Don't build a Playwright/browser harness yet — the return is not worth the CI complexity. Instead, write **smoke tests** that: boot cronduit in a subprocess, POST a "run now", scrape the HTML with `scraper` or `select.rs`, and assert the run appears. This catches 80% of e2e bugs at 10% of the cost.

---

## CI Stack (GitHub Actions, idiomatic 2025/26)

### Workflow outline

```yaml
name: CI
on:
  push: { branches: [main] }
  pull_request: {}

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: "-D warnings"

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: rustfmt, clippy }
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all -- --check
      - run: cargo clippy --all-targets --all-features

  test:
    runs-on: ubuntu-latest
    needs: check
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - uses: taiki-e/install-action@nextest
      - run: cargo nextest run --all-features

  docker:
    runs-on: ubuntu-latest
    needs: test
    permissions:
      contents: read
      packages: write  # for ghcr.io
    steps:
      - uses: actions/checkout@v4
      - uses: docker/setup-qemu-action@v3
      - uses: docker/setup-buildx-action@v3
      - uses: docker/login-action@v3
        if: github.event_name == 'push'
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - uses: docker/build-push-action@v6
        with:
          context: .
          platforms: linux/amd64,linux/arm64
          push: ${{ github.event_name == 'push' }}
          tags: ghcr.io/${{ github.repository }}:${{ github.sha }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

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

Use `cross` or `cargo-zigbuild` to **cross-compile natively** inside the builder stage, *not* QEMU-emulated `cargo build` (which is ~10x slower on GHA runners). Pattern:

```dockerfile
# Stage 1: planner (compute cargo-chef recipe for dep caching) — optional but recommended
FROM rust:1.85-alpine AS chef
RUN cargo install cargo-chef cargo-zigbuild
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG TARGETPLATFORM
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
# Use zigbuild for cross target builds
RUN cargo zigbuild --release --target x86_64-unknown-linux-musl --target aarch64-unknown-linux-musl \
  && mkdir -p /out \
  && cp target/x86_64-unknown-linux-musl/release/cronduit /out/cronduit-amd64 \
  && cp target/aarch64-unknown-linux-musl/release/cronduit /out/cronduit-arm64

FROM alpine:3.20 AS runtime
ARG TARGETARCH
COPY --from=builder /out/cronduit-${TARGETARCH} /usr/local/bin/cronduit
ENTRYPOINT ["/usr/local/bin/cronduit"]
```

This builds both arches *once* on the amd64 runner, which is dramatically faster than QEMU-emulating the arm64 build. `cargo-chef` caches the dependency layer so incremental pushes rebuild in seconds.

### Release tagging

On tag push (`v*`), also push `:latest` and `:X.Y.Z`. Use `docker/metadata-action@v5` to generate tag sets.

---

## Embedded Assets: rust-embed vs include_dir

**Recommendation: `rust-embed 8.11`. Confidence: HIGH.**

- **rust-embed** has a debug mode where assets are read from disk live, so Tailwind edits + template tweaks show on refresh without `cargo build`. This is a 10x faster inner loop during UI work.
- **include_dir** is simpler (pure `const`) but embeds everything even in debug mode, forcing a rebuild on every CSS change.

Both work; rust-embed's dev ergonomics are worth the tiny extra complexity.

### Asset layout

```
assets/
  static/
    app.css           # Tailwind output (generated)
    favicon.svg       # from design/favicons/
    logo.svg          # from design/logos/
  vendor/
    htmx.min.js       # pinned htmx 2.x
  templates/          # askama templates live here? NO — askama wants them at templates/
```

Keep `templates/` at the crate root (askama default) separate from `assets/` (rust-embed). They have different embedding mechanisms.

---

## Installation (Cargo.toml sketch)

```toml
[package]
name = "cronduit"
version = "0.1.0"
edition = "2024"
rust-version = "1.85"

[dependencies]
# Runtime
tokio = { version = "1.51", features = ["full"] }
tokio-util = "0.7"

# Web
axum = "0.8"
axum-htmx = "0.8"
tower-http = { version = "0.6", features = ["trace", "compression-gzip", "fs"] }

# Templates
askama = "0.15"
askama_web = { version = "0.15", features = ["axum-0.8"] }

# Docker
bollard = "0.20"

# Database
sqlx = { version = "0.8", features = ["runtime-tokio", "tls-rustls", "sqlite", "postgres", "macros", "chrono", "uuid", "migrate"] }

# Cron
croner = "3.0"

# Config
serde = { version = "1", features = ["derive"] }
toml = "1.1"
humantime = "2"
humantime-serde = "1"

# CLI
clap = { version = "4", features = ["derive", "env"] }

# Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
metrics = "0.24"
metrics-exporter-prometheus = "0.18"

# Errors
anyhow = "1"
thiserror = "2"

# Utilities
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v7", "serde"] }
rand = "0.8"
shell-words = "1"
rust-embed = { version = "8", features = ["debug-embed"] }  # debug-embed=false for release build profile

[dev-dependencies]
testcontainers = "0.27"
testcontainers-modules = { version = "0.15", features = ["postgres"] }
tokio = { version = "1", features = ["test-util"] }
tempfile = "3"
pretty_assertions = "1"
scraper = "0.20"  # HTML assertion helper for smoke tests

[features]
default = []
integration = []  # gate docker/postgres-requiring tests
```

---

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

---

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

---

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

---

## Stack Patterns by Variant

**If the deployment is homelab single-operator (the v1 default):**
- SQLite, no migrations runner, embedded HTMX, no auth, `/metrics` open on LAN.
- One binary, single docker-compose, docker socket mount.

**If the deployment is shared infra (v1.1 hypothesis):**
- Swap `DATABASE_URL` to Postgres — schema is the same.
- Put cronduit behind Traefik/Caddy for TLS + basic auth.
- Scrape `/metrics` from Prometheus; no code change.
- Still a single instance (no HA in v1/v2).

**If you want live-reload during Tailwind UI development:**
- `rust-embed` with `debug-embed = false` (default) reads from disk in debug builds → edit + refresh.
- Run Tailwind CLI in `--watch` mode in a second terminal writing to `assets/static/app.css`.
- Don't need `cargo-watch` for template-only changes thanks to disk reads.

---

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

---

## Sources

- **crates.io API** (2026-04-09) — version verification for all listed crates. Queried:
  - `tokio`, `axum`, `sqlx`, `bollard`, `askama`, `askama_web`, `askama_axum`, `maud`
  - `cron`, `saffron`, `croner`, `tokio-cron-scheduler`, `job_scheduler_ng`, `clokwerk`, `apalis`
  - `tracing`, `tracing-subscriber`, `metrics`, `metrics-exporter-prometheus`, `prometheus`, `prometheus-client`
  - `rust-embed`, `include_dir`
  - `serde`, `toml`, `serde_yaml`, `serde_yml`, `serde_norway`, `serde-yaml-bw`, `figment`, `config`
  - `tower-http`, `axum-htmx`, `axum-extra`, `datastar`
  - `testcontainers`, `testcontainers-modules`, `cargo-nextest`
  - `humantime`, `humantime-serde`, `notify`, `tokio-util`, `chrono`, `time`, `anyhow`, `thiserror`, `shellexpand`
- **GitHub API** (2026-04-09) — repository health & archive status:
  - `dtolnay/serde-yaml` — `archived: true` (the decisive datapoint for the YAML call)
  - `askama-rs/askama` — active, 1040 stars, last push 2026-04-05
  - `lambda-fairy/maud` — active, 2551 stars, last push 2026-04-06
  - `hexagon/croner-rust` — active, last push 2026-04-08
  - `zslayton/cron` — active, last push 2026-03-25
  - `fussybeaver/bollard` — active, 1253 stars, last push 2026-04-06
  - `robertwayne/axum-htmx` — active, 263 stars, last push 2026-01-20
- **askama README** (github.com/askama-rs/askama) — merge of askama ↔ rinja history, recommended usage.
- **croner-rust README** (via WebFetch) — feature matrix for cron syntax support (5/6/7-field, `L`/`#`/`W`, macros, timezones, descriptions).
- **`askama_axum` crate description** on crates.io — verbatim: *"Integration crates like `askama_axum` were removed from askama 0.13."*
- **`serde_yaml` crate description** on crates.io — version `0.9.34+deprecated`.
- **Cronduit SPEC.md** and **PROJECT.md** — context for locked decisions (Rust, bollard, sqlx, Tailwind, SSR), soft-locked TOML, the `@random` + `random_min_gap` requirement, the `container:<name>` network mode promise that drives the bollard + integration-test decisions.

---

*Stack research for: self-hosted Docker-native cron scheduler with server-rendered web UI*
*Researched: 2026-04-09*
