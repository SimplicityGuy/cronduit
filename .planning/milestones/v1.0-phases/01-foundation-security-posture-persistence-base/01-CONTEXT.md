# Phase 1: Foundation, Security Posture & Persistence Base — Context

**Gathered:** 2026-04-09
**Status:** Ready for planning

<domain>
## Phase Boundary

A compilable Rust binary that:

1. Parses a TOML config (with `${ENV_VAR}` interpolation and `SecretString` wrapping for sensitive fields)
2. Validates that config via a dedicated `cronduit check <config>` subcommand (no DB side-effects)
3. Opens a `sqlx` pool against SQLite or PostgreSQL and runs idempotent migrations from split per-backend migration directories
4. Emits a single structured `cronduit.startup` JSON log line summarizing bind / backend / config / timezone / job counts
5. Warns loudly (at WARN level) if `[server].bind` is non-loopback
6. Wraps every secret-bearing field in `SecretString` (via the `secrecy` crate) so `Debug` never leaks plaintext
7. Passes a green CI matrix (`linux/amd64 × linux/arm64 × SQLite × Postgres`) with fmt + clippy + tests + an `openssl-sys` guard, plus a multi-arch Docker image built via `cargo-zigbuild`
8. Exposes all of the above (plus dev-loop helpers) through a `justfile`, which CI workflows are required to invoke exclusively

**Explicitly NOT in Phase 1:** the scheduler loop, any job execution backend (command/script/docker), the web UI, `@random` resolution, config reload (SIGHUP / file watch / API), SSE, `/health`, `/metrics`, orphan reconciliation, retention pruning, the full threat model. Phase 1 ships a **skeleton `THREAT_MODEL.md`** only, and the README's SECURITY section links to it.

New capabilities belong in other phases (see ROADMAP.md Phases 2–6).

</domain>

<decisions>
## Implementation Decisions

### CLI Shape

- **D-01:** The binary uses **clap subcommands**, not top-level flags. Phase 1 ships two subcommands: `cronduit run` (default daemon mode, loads config, runs migrations, opens web listener — but no scheduler yet) and `cronduit check <config>` (parse + validate, no DB side-effects, non-zero exit on any error). A `cronduit migrate` subcommand is **not** introduced in Phase 1; migrations run idempotently on `cronduit run` startup per DB-03. `cronduit migrate` may be added later if operator workflows demand an explicit pre-rollout step.
- **D-02:** CLI flags override corresponding config-file values when both are set. An override fires an `info!` tracing event at startup naming the field and both values (no secret-bearing fields logged). Standard Unix precedence.
- **D-03:** `--log-format` defaults to **JSON unconditionally**. `--log-format=text` is explicit opt-in for local development. No tty auto-detect (keeps test surface small; Cronduit's production is always inside Docker).
- **D-04 (action item):** REQUIREMENTS.md FOUND-03 currently reads `cronduit --check <config>`. **Planner must rewrite FOUND-03 to `cronduit check <config>`** (subcommand form) before implementing so traceability stays consistent. Success criterion #2 in ROADMAP.md is already worded as a subcommand; only the requirement itself needs the fix.

### Crate Layout & Toolchain

- **D-05:** **Single `cronduit` crate** at repo root. No workspace. Modules follow `.planning/research/ARCHITECTURE.md` §Recommended Project Structure (`src/config/`, `src/db/`, `src/cli/`, `src/telemetry.rs`, `src/web/` stubs, etc.). Workspace can be introduced later if compile times become painful; v1 has no stated library-reuse requirement.
- **D-06:** **Rust edition 2024**, toolchain pinned to the latest stable (**1.94.1** at time of context gathering). Use a `rust-toolchain.toml` file so CI and local dev stay aligned. Pre-flight smoke test during the first plan: confirm `bollard 0.20.2`, `sqlx 0.8.6`, `axum 0.8.8` all compile cleanly under edition 2024. If any blocker exists, fall back to edition 2021 and document the blocker as a known issue in CONTEXT deviations — do not silently downgrade.
- **D-07:** `sqlx` features are **always-enabled**: `runtime-tokio`, `tls-rustls`, `sqlite`, `postgres`, `chrono`, `migrate`, `macros`. A single binary supports both backends at runtime via `DATABASE_URL`. No Cargo-feature gating; no separate release artifacts per backend. The slight binary-size cost is accepted for operational simplicity.
- **D-08:** The example config lives at **`examples/cronduit.toml`** (Cargo convention). Docker packaging in Phase 6 will copy it to `/etc/cronduit/config.toml` inside the image. README quickstart references `examples/cronduit.toml` directly. The existing `docs/SPEC.md` snippet stays as-is — not the copy-paste target.

### Build Tooling — `justfile` (new Phase 1 scope, see FOUND-12 action item)

- **D-09:** A top-level `justfile` at repo root is the **single source of truth** for build, test, lint, DB, image, and dev-loop commands. Every task runnable in CI must have a matching `just` recipe.
- **D-10:** **CI workflows are strictly `just`-only.** Every step in `.github/workflows/ci.yml` calls `just <recipe>` — no raw `cargo`, `docker`, or `sqlx` invocations inline in YAML. This guarantees that devs and CI run byte-identical commands and that a green local `just ci` run predicts a green CI run. Setup step at the top of each job installs `just` via `extractions/setup-just@v2` (or an equivalent idiomatic action).
- **D-11:** Phase 1 `justfile` ships the following recipe groups:
  - **Build & artifacts:** `just build`, `just build-release`, `just clean`, `just image` (multi-arch `cargo-zigbuild` + `docker buildx`), `just tailwind` (standalone Tailwind CSS rebuild → `assets/static/app.css`)
  - **Quality gates:** `just fmt`, `just fmt-check`, `just clippy` (runs `cargo clippy --all-targets --all-features -- -D warnings`), `just test`, `just nextest`, `just openssl-check` (wraps `cargo tree -i openssl-sys`; fails on any match — FOUND-06)
  - **DB / schema:** `just db-reset` (drop + recreate local SQLite/Postgres), `just migrate`, `just sqlx-prepare` (regenerates `.sqlx/` for offline query checking), `just schema-diff` (structural parity test — see D-14)
  - **Dev loop:** `just dev` (cargo-watch + tailwind watch, text logs), `just check-config PATH` (wraps `cronduit check`), `just docker-compose-up`
  - **Meta:** `just ci` as the single recipe that chains fmt-check → clippy → openssl-check → nextest → schema-diff → image. Running `just ci` locally must produce the same exit code as the CI job.
- **D-12 (action item):** **REQUIREMENTS.md must gain a new `FOUND-12`** capturing the justfile + just-only-CI constraint, and the traceability table must add the new row. Suggested wording: *"A top-level `justfile` defines every build, test, lint, DB, image, and dev-loop command for the project; all GitHub Actions workflows invoke only `just <recipe>` targets, with no inline `cargo` or `docker` commands."* Planner adds this before the phase plan is written.

### Migrations & Schema Parity

- **D-13:** **Split migration directories** from day one: `migrations/sqlite/` and `migrations/postgres/`. Two `sqlx::migrate!` invocations, selected at runtime by a `DbPool` enum variant (`Sqlite` / `Postgres`). The initial files start byte-identical where possible; the split exists so dialect-specific syntax (BIGINT, JSONB, partial indexes, AUTOINCREMENT semantics) can be added later without disturbing the other backend. This directly addresses `.planning/research/PITFALLS.md` §Pitfall 8 (schema parity drift).
- **D-14:** **Structural parity is enforced by a Rust integration test** (`tests/schema_parity.rs`), surfaced as `just schema-diff`, running in every cell of the CI matrix. The test:
  1. Boots an in-memory SQLite and a `testcontainers`-backed Postgres
  2. Runs the corresponding migrations against each
  3. Introspects `sqlite_master` and `information_schema.columns` / `information_schema.tables` / `pg_indexes`
  4. Normalizes type names (e.g. `INTEGER` ↔ `BIGINT` where semantically equivalent) via a small whitelist
  5. Asserts identical table sets, identical column sets (name + nullable + normalized type), and identical index coverage
  Any drift fails the test with a structured diff. Whitelist entries require a justification comment.
- **D-15:** The Phase 1 initial migration already includes **`config_hash TEXT NOT NULL`** on the `jobs` table, holding a hex-encoded SHA-256 of the **normalized** (sorted-keys, stable-ordering) JSON representation of a job's config. The normalization function lives in `src/config/hash.rs` and is documented in a doc-comment that the planner MUST carry forward. Phase 1 does not yet write data to this column (no sync engine yet) — the column exists so Phase 2 does not require another migration.
- **D-16:** `config_json` is stored as **`TEXT` on both backends**. We never query inside the blob — it is purely an audit/round-trip record. Using TEXT keeps the schema-parity test trivial and sidesteps `sqlx`'s JSONB binding quirks. Documented as an intentional choice in the migration file header.

### CI Matrix & Docker Image

- **D-17:** **Single `.github/workflows/ci.yml`** with a `strategy.matrix` over `{arch: [amd64, arm64], db: [sqlite, postgres]}` (4 cells) for the test job. `fmt-check`, `clippy`, and `openssl-check` run **once** in a separate lint job (not matrixed — they are backend-agnostic). The Docker image build is its own job that `needs:` both `lint` and `test`. One consolidated status check per PR. Every job step calls `just <recipe>` (per D-10).
- **D-18:** The multi-arch Docker image (`linux/amd64` + `linux/arm64`) builds on **every PR and every push to `main`**. PR builds stay local to the runner (`docker buildx build --load`, no push). On `main` push the image is tagged `ghcr.io/<owner>/cronduit:latest` and `:sha-<short>` and pushed to GHCR. Rationale: building on every PR is the only reliable way to catch Pitfall 14 (transitive `openssl-sys` dep breaking `cargo-zigbuild` cross-compile) before merge. `tagged releases` handling is deferred to Phase 6 release engineering.

### Config Schema Details

- **D-19:** `[server].timezone` is **mandatory**. `cronduit check` fails parse and `cronduit run` refuses to start if the field is missing or not a valid IANA zone name (e.g. `"America/Los_Angeles"`, `"UTC"`). No implicit host-timezone fallback. Matches CONF-08 literally and lines up with croner's timezone handling for Phase 2 scheduler correctness.
- **D-20:** Use the **`secrecy` crate (0.10+)** for `SecretString`. No hand-roll. No `zeroize` in v1 (single-process daemon; memory-wipe guarantees are overkill and cost code-review attention we'd rather spend on the Docker executor). Secret-bearing fields (those populated from `${ENV_VAR}` interpolation and the named fields `postgres_password`, any future `api_token`, etc.) use `secrecy::SecretString` at the config-type boundary. `Debug` impls render `[REDACTED]`; `serde` wiring via the `secrecy::serde` feature.
- **D-21:** `cronduit check` **collects all errors** into a `Vec<ConfigError>` and prints them in **GCC-style `path:line:col: error: <message>`** format on stderr, then exits non-zero. Line/col come from `toml::de::Error::span()`. A single broken config file shows every broken field in one pass. JSON error output is deferred (D-26 in Deferred Ideas).
- **D-22:** Environment variable interpolation is **strict `${VAR}` only**. Missing required variables fail loudly with the full field path — e.g. `error: missing environment variable "API_KEY" required by [[jobs]]/check-ip/env/API_KEY`. No `${VAR:-default}` syntax in v1. The strict rule prevents silent typos from masking broken configs. Can be revisited in a later phase if operators ask.

### Observability & Bind Safety

- **D-23:** On startup (after config parse + DB pool + migrations all succeed), Cronduit emits **one** structured `tracing::info!` event on target `cronduit.startup` with fields: `version`, `bind` (string), `database_backend` (`"sqlite"` | `"postgres"`), `database_url` (credentials stripped — only scheme + host + dbname), `config_path`, `timezone`, `job_count`, `disabled_job_count`, `bind_warning` (bool; true when D-24 fires). One log line, greppable, addresses Pitfall 15 (zero-config surprises). Multiple granular spans still exist for component-level traces; the startup summary is strictly additive.
- **D-24:** On startup, if the **resolved** bind address (after D-02 precedence) is not `127.0.0.1` or `::1`, Cronduit emits a single loud `tracing::warn!` event explaining the no-auth-in-v1 stance and pointing at the README SECURITY section. One unified warning for every non-loopback address — RFC1918 (`192.168.x.y`, `10.x.y.z`, `172.16-31.x.y`) gets the same warning as `0.0.0.0` or a public IP, because the threat model (no auth) is identical regardless. The `bind_warning: true` field in the D-23 startup event makes this machine-detectable.

### Folded Todos

*(None — no matching pending todos existed at discussion time.)*

### Claude's Discretion

The planner / researcher may decide the following without re-asking:

- Exact sub-module boundaries within `src/config/`, `src/db/`, and `src/cli/` (follow ARCHITECTURE.md §Recommended Project Structure as a starting point; deviate with a one-line rationale in the plan)
- Specific `clap` derive attribute style (long-about from doc comments vs `#[command(...)]` literals, global vs per-subcommand flags)
- Specific `tracing-subscriber` layer ordering and filter defaults (as long as JSON-to-stdout + `RUST_LOG` env support both work)
- Whether to use `color-eyre` for human-facing `check` errors or plain `eprintln!` (both acceptable; `color-eyre` is nice but adds a dep)
- Exact `justfile` recipe bodies — as long as the recipes in D-11 exist and work
- Exact layout of the skeleton `THREAT_MODEL.md` sections (STRIDE headings, or a freeform narrative organized around the Docker-socket threat, loopback default, and no-auth-in-v1 stance — both acceptable)
- Specific GitHub Actions action versions for non-critical utilities (as long as `dtolnay/rust-toolchain`, `Swatinem/rust-cache@v2`, `docker/buildx-action@v3`, `extractions/setup-just@v2`, and `docker/build-push-action@v6` are used for the critical path — per STACK.md)
- Whether `.sqlx/` is committed to the repo (recommended YES so CI doesn't need a live DB for `query!` macro checks, but planner can document a contrary decision)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-Level Contracts

- `CLAUDE.md` — Locked tech stack, security posture, diagrams-are-mermaid rule, PR-only workflow, full `## Technology Stack` table with versions and rationale. **Authoritative** for every version pin.
- `.planning/PROJECT.md` — Vision, active requirements checklist, out-of-scope, Key Decisions table. Explicitly names the locked decisions this phase must honor.
- `.planning/REQUIREMENTS.md` — The 29 requirements allocated to Phase 1 (FOUND-01..11, CONF-01..10, DB-01..07, OPS-03). **Needs the D-04 and D-12 updates before plan execution.**
- `.planning/ROADMAP.md` §"Phase 1: Foundation, Security Posture & Persistence Base" — Phase goal, success criteria, dependencies, and pitfall mapping. Pitfall map in this section is the authoritative "this phase must address" list.

### Specification & Research

- `docs/SPEC.md` — Authoritative v1 product spec. Section headers: Core Scheduler, Job Types, Configuration, Database, Web UI, Operational, Docker Deployment, Security Considerations, Non-Goals, Suggested Crates, Example Config. Phase 1 only implements the Configuration, Database, and Operational (subset) sections.
- `.planning/research/ARCHITECTURE.md` — Component responsibilities, recommended project structure (§Recommended Project Structure), full initial SQL migration (§Database Schema), startup boot flow (§Startup Boot Flow), `AppState` pattern (§Pattern 1). **The ER diagram and initial SQL here are the source of truth for D-13/D-14/D-15/D-16.**
- `.planning/research/PITFALLS.md` — Phase 1 must address §§ 1 (Docker socket root-equivalent → README SECURITY + THREAT_MODEL skeleton), 7 (SQLite write contention → split read/write pools + WAL + busy_timeout), 8 (schema parity → D-13/D-14), 14 (cross-compile + openssl-sys → FOUND-06 + `just openssl-check`), 15 (zero-config surprises → D-23), 18 (secrets in errors → D-20 + secrecy crate), 20 (config format creep → TOML-only, no YAML/JSON/INI code paths).
- `.planning/research/STACK.md` — Version-locked crate table. **Every `Cargo.toml` dependency version comes from here or from the CLAUDE.md copy.**
- `.planning/research/FEATURES.md` — Catalog of features with phase assignment; consult when a decision touches multiple phases.
- `.planning/research/SUMMARY.md` — Research synthesis; quick-read version of the four research docs.

### Design & Brand (referenced, not yet consumed in Phase 1)

- `design/DESIGN_SYSTEM.md` — Terminal-green color tokens, monospace typography, status badges, dark/light tokens. **Not implemented in Phase 1** (no web UI yet) but the Tailwind build pipeline (`just tailwind`) must be wired so Phase 3 can consume it.
- `design/showcase.html` — HTML reference for the final brand look; Phase 3 will extract templates from this.

### Action Items for the Planner (before writing the plan)

1. Update `REQUIREMENTS.md` FOUND-03 wording from `cronduit --check <config>` to `cronduit check <config>` (subcommand form, per D-04).
2. Add a new `FOUND-12` requirement to `REQUIREMENTS.md` covering the justfile + just-only-CI constraint (per D-12).
3. Extend the `REQUIREMENTS.md` traceability table with `FOUND-12 → Phase 1 → Pending` and bump Phase 1's requirement count from 29 to 30.
4. Commit the REQUIREMENTS.md change in its own atomic commit before the plan is written — the plan should cite the amended wording.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **Brand assets in `design/`** (logos, banners, favicons, `showcase.html`) — not consumed in Phase 1 but will feed Phase 3 UI work. Phase 1 only needs the Tailwind build pipeline (`just tailwind`) wired up as scaffolding.
- **`docs/SPEC.md` example TOML** — the TOML block at the bottom can be lifted (with cleanup) into `examples/cronduit.toml`.
- **`.gitignore`** — already scoped for Rust/Docker/macOS; no additions needed for Phase 1.

### Established Patterns

*(No existing Rust source. This is a greenfield phase.)*

The **planner** should treat `.planning/research/ARCHITECTURE.md` §Recommended Project Structure as the default pattern unless a concrete reason to deviate appears.

### Integration Points

- `Cargo.toml` — does not exist; Phase 1 creates it from scratch.
- `rust-toolchain.toml` — does not exist; Phase 1 creates it pinning `1.94.1`.
- `.github/workflows/` — directory does not exist; Phase 1 creates `ci.yml`.
- `justfile` — does not exist; Phase 1 creates it.
- `migrations/sqlite/` + `migrations/postgres/` — do not exist; Phase 1 creates both.
- `README.md` — currently a 10-byte stub (`# cronduit`). Phase 1 rewrites it to lead with a SECURITY section per FOUND-10.
- `THREAT_MODEL.md` — does not exist; Phase 1 creates it as a skeleton.

</code_context>

<specifics>
## Specific Ideas

- The user explicitly introduced **`justfile` + `just`** as a Phase 1 foundation scaffold item mid-discussion and was emphatic that **CI must use `just` commands exclusively** — no raw `cargo` in workflow YAML. Downstream agents must NOT treat `just` as optional developer ergonomics — it is a mandatory part of the build surface and an enforcement constraint on CI. See D-09 / D-10 / D-11 / D-12.
- The user asked for the **latest stable Rust toolchain (1.94.1)** and **edition 2024**, explicitly overriding the "edition 2021 or 2024 if cleanly compiles" language in FOUND-01. Pin the toolchain in `rust-toolchain.toml`. Fall back to 2021 only if bollard/sqlx/axum fail to compile under 2024, and document the blocker as a CONTEXT deviation (not a silent downgrade).
- **REQUIREMENTS.md wording conflict:** FOUND-03 currently reads `cronduit --check <config>` (flag form) but CLAUDE.md and ROADMAP.md success criterion #2 use the subcommand form `cronduit check <config>`. D-01/D-04 resolve this in favor of the subcommand form; the action-items list above gives the planner instructions to update REQUIREMENTS.md before the plan is written.
- Phase 1's `cronduit run` subcommand **loads config + runs migrations + opens the axum listener** but **does not yet fire any jobs** — the scheduler loop lands in Phase 2, the job backends in Phases 2 and 4, and the UI routes in Phase 3. This means Phase 1's axum server returns empty routes (or a minimal "cronduit is running, no scheduler yet" placeholder) — the planner must either (a) ship a tiny placeholder or (b) gate the axum listener behind a `--serve-ui` flag that defaults off in Phase 1. Planner's call; call out the choice explicitly in the plan.

</specifics>

<deferred>
## Deferred Ideas

These came up during discussion but don't belong in Phase 1. Do not lose them.

- **D-25 (deferred):** Standalone `cronduit migrate` subcommand. Considered for Phase 1; rejected because `cronduit run` already runs migrations idempotently on startup (DB-03). May be added post-v1 if operator workflows demand an explicit pre-rollout migration step (e.g. Kubernetes init containers).
- **D-26 (deferred):** JSON error output for `cronduit check`. Considered; deferred because the collect-all human-readable path (D-21) is strictly the foundation and the JSON path can layer on top by serializing the same `Vec<ConfigError>`. Revisit if editor integration tooling asks for it.
- **D-27 (deferred):** `${VAR:-default}` / bash-style default-value syntax for env interpolation. Deferred because strict `${VAR}` surfaces typos immediately and the default-value convenience is a footgun for production secrets. Revisit if operator feedback shows it's actually painful.
- **D-28 (deferred):** `zeroize` crate for active memory wiping of secrets. Overkill for a single-process daemon that never writes secrets to disk. The secrecy crate's redaction in `Debug` is sufficient for v1. Revisit if threat model expands.
- **D-29 (deferred):** Backend-gated Cargo features (e.g. `--features sqlite` vs `--features postgres`) producing two slimmer binaries. Rejected as a premature optimization; always-enabled sqlx features keep the build matrix simple. Revisit only if binary size becomes a real complaint.
- **D-30 (deferred — Phase 6):** Full `THREAT_MODEL.md` content. Phase 1 ships a skeleton with STRIDE headings and TBD markers; Phase 6 release-engineering fleshes it out once the Docker executor (Phase 4) and reload flow (Phase 5) expose the full attack surface.
- **D-31 (deferred — Phase 6):** `OPS-04` (example `docker-compose.yml`) stays in Phase 6 per ROADMAP.md traceability. Phase 1 builds the multi-arch image and publishes it, but the end-to-end docker-compose quickstart lives in Phase 6.
- **D-32 (deferred — Phase 6):** Default `[server].log_retention` tuning. Phase 1 ships the column in the config struct with a `humantime_serde` type, defaulting to `"90d"`, but the retention pruner task itself is Phase 6 (DB-08).

### Reviewed Todos (not folded)

*(None — no pending todos were matched at discussion time.)*

</deferred>

---

*Phase: 01-foundation-security-posture-persistence-base*
*Context gathered: 2026-04-09*
