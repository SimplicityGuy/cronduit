# Phase 1: Foundation, Security Posture & Persistence Base — Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in `01-CONTEXT.md` — this log preserves the alternatives considered.

**Date:** 2026-04-09
**Phase:** 01-foundation-security-posture-persistence-base
**Areas discussed:** CLI shape, Crate layout & edition, Justfile addition (mid-discussion user scope add), Migration strategy, CI matrix + Docker image + config schema details

---

## Gray-Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| CLI shape (run/check/migrate) | Resolve CLAUDE.md ↔ REQUIREMENTS.md doc conflict; subcommand vs flag | ✓ |
| Crate layout & edition | Single crate vs workspace; edition 2021 vs 2024; sqlx feature gating | ✓ |
| Migration strategy (SQLite vs Postgres parity) | Shared vs split migration dirs; parity enforcement; config_hash/config_json placement | ✓ |
| CI matrix + Docker image + config schema details | Workflow layout; image build cadence; timezone policy; SecretString; check error format; env interp syntax; startup log; bind warning rules | ✓ |

The user selected all four. Three additional candidates were presented but not picked: startup summary log details, SecretString ergonomics, threat model scope — these were folded into the "CI matrix + Docker image + config schema details" bucket (Area 4) during discussion.

---

## Area 1: CLI Shape

### Q1 — How should the cronduit binary expose its actions?

| Option | Description | Selected |
|--------|-------------|----------|
| Subcommands | `cronduit run / check / migrate` — matches CLAUDE.md, scales to future commands, requires REQUIREMENTS.md fix | ✓ |
| Flags only | `cronduit --check <config>` — matches REQUIREMENTS.md literally; awkward long-term | |
| Default-run + subcommands | Zero-arg runs as daemon, `check` and `migrate` still subcommands | |

**User's choice:** Subcommands (recommended)
**Notes:** Resolves doc conflict between CLAUDE.md and REQUIREMENTS.md in favor of CLAUDE.md wording. REQUIREMENTS.md FOUND-03 must be rewritten by the planner before implementation.

### Q2 — Standalone `cronduit migrate` in Phase 1?

| Option | Description | Selected |
|--------|-------------|----------|
| Auto-run only | `cronduit run` auto-migrates per DB-03; no separate subcommand | ✓ |
| Both (auto + explicit) | Both paths exist; small extra surface | |
| Explicit only | Operator must run `migrate` then `run`; Kubernetes-idiomatic | |

**User's choice:** Auto-run only (recommended)
**Notes:** Keeps Phase 1 minimal. `migrate` subcommand can be added later if operator workflows demand it.

### Q3 — Precedence when CLI flag and config-file value conflict?

| Option | Description | Selected |
|--------|-------------|----------|
| CLI flag wins | Standard Unix convention; logged at INFO | ✓ |
| Config file wins | Config is source of truth; surprising | |
| Error on conflict | Safest, most annoying | |

**User's choice:** CLI flag wins (recommended)

### Q4 — `--log-format` default?

| Option | Description | Selected |
|--------|-------------|----------|
| JSON always | Matches FOUND-04; production is always inside Docker | ✓ |
| Auto-detect (tty=text, pipe=json) | Extra dep, more test surface | |
| Text default, JSON opt-in | Easy to forget for Docker | |

**User's choice:** JSON always (recommended)

---

## Area 2: Crate Layout & Edition

### Q1 — Single crate or workspace?

| Option | Description | Selected |
|--------|-------------|----------|
| Single crate | One `cronduit` crate, modules per ARCHITECTURE.md | ✓ |
| Workspace (core/cli/web) | Multi-crate; adds friction, no reuse requirement | |
| Workspace (bin + lib) | Over-engineering for v1 | |

**User's choice:** Single crate (recommended)

### Q2 — Rust edition?

| Option | Description | Selected |
|--------|-------------|----------|
| 2024 (pending smoke test) | Latest, better async ergonomics; fall back to 2021 on compile break | ✓ |
| 2021 (safe) | Battle-tested | |

**User's choice:** Edition 2024, with explicit instruction to use **latest stable Rust toolchain 1.94.1**.
**Notes:** User override: "Use the latest... 1.94.1". Planner must pin the toolchain in `rust-toolchain.toml`. Edition 2021 fallback only if bollard/sqlx/axum fail to compile under 2024 — and then it must be documented as a CONTEXT deviation, not a silent downgrade.

### Q3 — `sqlx` feature gating?

| Option | Description | Selected |
|--------|-------------|----------|
| Both always enabled | Single binary, `DATABASE_URL`-driven backend choice | ✓ |
| Cargo features toggle backends | Two binaries, smaller each, more CI cells | |

**User's choice:** Both always enabled (recommended)

### Q4 — Example config location?

| Option | Description | Selected |
|--------|-------------|----------|
| `examples/cronduit.toml` | Cargo convention | ✓ |
| `docs/example-config.toml` | Groups with SPEC.md; non-standard | |
| Inline in README only | No standalone file | |

**User's choice:** `examples/cronduit.toml` (recommended)

---

## Scope Addition (Mid-Discussion): `justfile` + `just`

User interrupted the area-by-area loop with: *"It's foundation scaffold. I want to make sure we use a justfile and just commands for each part of the build, test, clean. Also, just commands must be used from the CI environment."*

### Follow-up Q1 — CI strictness?

| Option | Description | Selected |
|--------|-------------|----------|
| Strict — CI only calls `just <recipe>` | Single source of truth; dev+CI byte-identical | ✓ |
| Mostly `just`, raw cargo for trivial | Less discipline, more YAML drift | |
| Dev-facing only | CI stays on raw cargo | |

**User's choice:** Strict — CI only calls `just <recipe>` (recommended)

### Follow-up Q2 — Recipe groups for Phase 1?

| Option | Description | Selected |
|--------|-------------|----------|
| Build & artifacts | `just build`, `build-release`, `clean`, `image`, `tailwind` | ✓ |
| Quality gates | `just fmt`, `fmt-check`, `clippy`, `test`, `nextest`, `openssl-check` | ✓ |
| DB / schema | `just db-reset`, `migrate`, `sqlx-prepare`, `schema-diff` | ✓ |
| Dev loop helpers | `just dev`, `check-config PATH`, `docker-compose-up` | ✓ |

**User's choice:** All four groups (full set)
**Notes:** This scope addition creates a new Phase 1 requirement — the planner must add `FOUND-12` to REQUIREMENTS.md and update traceability before writing the plan.

---

## Area 3: Migration Strategy (SQLite vs Postgres Parity)

### Q1 — Migration directory layout?

| Option | Description | Selected |
|--------|-------------|----------|
| Split dirs from day one | `migrations/sqlite/` + `migrations/postgres/`, selected per DbPool variant | ✓ |
| Shared single dir | Works until first dialect divergence | |
| Shared + conditional SQL | Unmaintainable | |

**User's choice:** Split dirs from day one (recommended)

### Q2 — Schema parity enforcement?

| Option | Description | Selected |
|--------|-------------|----------|
| Structural parity test | Rust integration test introspecting both catalogs, exposed as `just schema-diff` | ✓ |
| Checksum + manual review | Relies on discipline | |
| Cross-backend query test | Catches drift via query failures, weaker | |

**User's choice:** Structural parity test (recommended)

### Q3 — `config_hash` column placement?

| Option | Description | Selected |
|--------|-------------|----------|
| TEXT column, SHA-256 hex | Portable, human-inspectable | ✓ |
| BLOB column, raw SHA-256 | 32 bytes, not human-inspectable | |
| Defer until Phase 2 | Forces an extra migration | |

**User's choice:** TEXT column, SHA-256 hex (recommended)

### Q4 — `config_json` storage type?

| Option | Description | Selected |
|--------|-------------|----------|
| TEXT on both | Opaque audit blob, trivial parity | ✓ |
| TEXT on SQLite, JSONB on Postgres | Breaks shared-schema fiction | |

**User's choice:** TEXT on both (recommended)

---

## Area 4a: CI Matrix & Secrets & Timezone

### Q1 — CI workflow layout?

| Option | Description | Selected |
|--------|-------------|----------|
| One workflow, matrix strategy | `ci.yml` with `strategy.matrix` | ✓ |
| Split workflows | `lint.yml`, `test.yml`, `image.yml` | |
| One workflow, sequential jobs | Verbose, slower, explicit | |

**User's choice:** One workflow, matrix strategy (recommended)

### Q2 — Docker image build cadence?

| Option | Description | Selected |
|--------|-------------|----------|
| Every PR + push to main | PR builds local-only, main push publishes to GHCR | ✓ |
| main push only | Faster PRs, cross-compile untested until merge | |
| Tag push only | Slowest feedback | |

**User's choice:** Every PR + push to main (recommended)

### Q3 — `[server].timezone` mandatory or default UTC?

| Option | Description | Selected |
|--------|-------------|----------|
| Mandatory | Forces explicit decision; matches CONF-08 | ✓ |
| Default UTC, WARN on missing | More forgiving; conflicts with CONF-08 | |
| Default host TZ | Explicitly forbidden | |

**User's choice:** Mandatory (recommended)

### Q4 — `secrecy` crate or hand-roll?

| Option | Description | Selected |
|--------|-------------|----------|
| `secrecy` crate | Purpose-built, redaction + serde integration | ✓ |
| Hand-roll newtype | Reinvents redaction, no upside | |
| `secrecy` + `zeroize` | Memory wiping, overkill for v1 | |

**User's choice:** `secrecy` crate (recommended)

---

## Area 4b: Config Errors / Env Interp / Startup Log / Bind Warning

### Q1 — `cronduit check` error reporting?

| Option | Description | Selected |
|--------|-------------|----------|
| Collect-all, human-readable | GCC-style `path:line:col: error: msg` | ✓ |
| Fail-fast, first error only | Worse UX | |
| Collect-all, JSON output | Layerable later | |

**User's choice:** Collect-all, human-readable (recommended)

### Q2 — Env-var interpolation syntax?

| Option | Description | Selected |
|--------|-------------|----------|
| Strict `${VAR}` only | Loud fail on missing; matches CONF-02 | ✓ |
| Bash-style `${VAR:-default}` | Footgun for typos | |
| Explicit `env.<NAME>` function | Ugly | |

**User's choice:** Strict `${VAR}` only (recommended)

### Q3 — Startup summary log shape?

| Option | Description | Selected |
|--------|-------------|----------|
| Single `cronduit.startup` JSON event | Greppable, addresses Pitfall 15 | ✓ |
| Multiple tracing events | Harder to grep | |
| Human ASCII banner | Violates FOUND-04 | |

**User's choice:** Single `cronduit.startup` JSON event (recommended)

### Q4 — Non-loopback bind warning rules?

| Option | Description | Selected |
|--------|-------------|----------|
| WARN on any non-127.0.0.1/::1 | Matches OPS-03 literally | ✓ |
| WARN on public, INFO on RFC1918 | Threat is the same on hostile LAN | |
| Error + refuse to start | Maximally annoying | |

**User's choice:** WARN on any non-loopback (recommended)

---

## Claude's Discretion

Items where the user delegated the decision to the planner/researcher:

- Exact sub-module boundaries within `src/config/`, `src/db/`, `src/cli/` (follow ARCHITECTURE.md §Recommended Project Structure)
- `clap` derive attribute style
- `tracing-subscriber` layer ordering and filter defaults
- Whether to use `color-eyre` for `check` human output
- Exact `justfile` recipe bodies (as long as the named recipes in D-11 exist)
- Exact layout of the skeleton `THREAT_MODEL.md`
- Specific GitHub Actions action versions for non-critical utilities
- Whether `.sqlx/` is committed (recommendation: yes)
- Phase 1 axum route shape (empty placeholder vs `--serve-ui` flag gating)

## Deferred Ideas

- Standalone `cronduit migrate` subcommand (D-25 in CONTEXT)
- JSON error output for `cronduit check` (D-26)
- `${VAR:-default}` env interpolation syntax (D-27)
- `zeroize` memory wiping (D-28)
- Backend-gated Cargo features (D-29)
- Full `THREAT_MODEL.md` content (D-30 — Phase 6)
- Example `docker-compose.yml` (D-31 — Phase 6, per OPS-04)
- Default `[server].log_retention` tuning and the pruner itself (D-32 — Phase 6, per DB-08)

## Scope Creep Attempted / Redirected

None during this discussion. The user's mid-discussion `justfile` addition was **not** scope creep — it is a foundation-scaffold constraint that belongs in Phase 1 and was folded in as D-09..D-12 with the corresponding FOUND-12 action item.
