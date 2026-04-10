---
phase: 1
slug: foundation-security-posture-persistence-base
status: planned
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-09
updated: 2026-04-10
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Finalized by the planner on 2026-04-10 after the 8 PLAN.md files were written.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test + cargo-nextest (Rust) + assert_cmd + testcontainers-modules |
| **Config file** | `Cargo.toml` + `.config/nextest.toml` (both created in Plan 01 Task 1) |
| **Quick run command** | `just test` (wraps `cargo test --all-features`) |
| **Full suite command** | `just ci` (fmt-check → clippy → openssl-check → nextest → schema-diff → image) |
| **Estimated runtime** | `just test` ~ 60-90 s (cold), 15-30 s (warm) ; `just ci` ~ 240-480 s (cold, includes image build) |

---

## Sampling Rate

- **After every task commit:** Run the task-specific `cargo test --test <name>` OR `cargo test --lib <module>::` listed in the per-task verification map below
- **After every plan wave merge:** Run `just nextest` (full suite) plus `just schema-diff` if the wave touched migrations
- **Before `/gsd-verify-work`:** `just ci` must be green locally AND the GitHub Actions CI matrix must be green on the feature branch
- **Max feedback latency:** `just test` target < 120 s; `just ci` target < 600 s

---

## Per-Task Verification Map

Every task in every plan has either an automated verify command OR an explicit Wave 0 dependency. Task IDs follow `01-<plan>-<task>` where plan is 01..08.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | FOUND-01 | T-01-07 | Cargo.toml compiles cleanly on stable 1.94.1 edition 2024 | unit / build | `cargo build --workspace` | ❌ W0 | ⬜ pending |
| 01-01-02 | 01 | 1 | FOUND-02, FOUND-04 | T-01-08, T-01-09 | CLI shape, tracing init, graceful shutdown wired | build + smoke | `cargo build --workspace && cargo run --quiet -- --help` | ❌ W0 | ⬜ pending |
| 01-02-01 | 02 | 2 | CONF-01..06, CONF-08, CONF-09, CONF-10, FOUND-05 | T-01-02, T-01-04, T-01-05 | parse_and_validate compiles; unit tests cover interpolation, IANA tz, network regex, hash stability | unit | `cargo test --lib config::` | ❌ W0 | ⬜ pending |
| 01-02-02 | 02 | 2 | CONF-01..06, CONF-08, CONF-09, CONF-10, FOUND-05 | T-01-02, T-01-04, T-01-05 | 9 fixtures exist, 9 integration tests pass (happy path, collect-all, secret redaction) | integration | `cargo test --test config_parser` | ❌ W0 | ⬜ pending |
| 01-03-01 | 03 | 3 | FOUND-03 | T-01-02, T-01-05 | check subcommand wired to parse_and_validate, GCC-style printer | build + smoke | `cargo build --workspace && cargo run --quiet -- check tests/fixtures/valid-minimal.toml` | ❌ W0 | ⬜ pending |
| 01-03-02 | 03 | 3 | FOUND-03 | T-01-02, T-01-05 | Black-box tests: valid/invalid/collect-all/no-DB-IO/no-secret-leak | integration | `cargo test --test check_command` | ❌ W0 | ⬜ pending |
| 01-04-01 | 04 | 3 | DB-04, DB-06 | T-01-12 | Both initial migration files exist with identical semantic schema | file + migrate test | `test -s migrations/sqlite/20260410_000000_initial.up.sql && test -s migrations/postgres/20260410_000000_initial.up.sql` (then validated via 01-04-05) | ❌ W0 | ⬜ pending |
| 01-04-02 | 04 | 3 | DB-01, DB-02, DB-03, DB-05 | T-01-02, T-01-11 | DbPool enum compiles; migrations idempotent; credential stripping unit-tested | unit | `cargo test --lib db::` | ❌ W0 | ⬜ pending |
| 01-04-03 | 04 | 3 | FOUND-02, OPS-03, DB-01, DB-02, DB-03 | T-01-01, T-01-02 | Full boot flow compiles; loopback detection unit-tested | unit | `cargo test --lib cli::run::tests` | ❌ W0 | ⬜ pending |
| 01-04-04 | 04 | 3 | DB-05 | T-01-11 | SQLite writer pool pragmas match expectations (WAL, busy_timeout=5000, synchronous=1, FKs=1) | integration | `cargo test --test db_pool_sqlite` | ❌ W0 | ⬜ pending |
| 01-04-05 | 04 | 3 | DB-03, DB-04, DB-06 | T-01-12 | migrate() idempotent; jobs/job_runs/job_logs tables exist; config_hash column present | integration | `cargo test --test migrations_idempotent` | ❌ W0 | ⬜ pending |
| 01-04-06 | 04 | 3 | FOUND-04, OPS-03 | T-01-01 | Startup event has all D-23 fields; bind_warning=true on 0.0.0.0; credentials stripped | integration (black-box) | `cargo test --test startup_event` | ❌ W0 | ⬜ pending |
| 01-04-07 | 04 | 3 | FOUND-01 | T-01-08 | SIGTERM on cronduit run yields exit 0 within 1 s | integration (black-box, unix only) | `cargo test --test graceful_shutdown` | ❌ W0 | ⬜ pending |
| 01-05-01 | 05 | 4 | DB-04 | T-01-12 | Schema parity: SQLite vs Postgres identical tables/columns/indexes after migration | integration (testcontainers) | `cargo test --test schema_parity -- --nocapture` | ❌ W0 | ⬜ pending |
| 01-05-02 | 05 | 4 | DB-02, DB-03 | T-01-12 | DbPool connects to testcontainers Postgres; migrate idempotent | integration (testcontainers) | `cargo test --test db_pool_postgres` | ❌ W0 | ⬜ pending |
| 01-06-01 | 06 | 2 | FOUND-06, FOUND-12 | T-01-03, T-01-06 | justfile has all D-11 recipes; openssl-check pattern correct; fails on any openssl-sys presence | CLI smoke | `just --list` + `just openssl-check` + `just fmt-check` | ❌ W0 | ⬜ pending |
| 01-07-01 | 07 | 5 | FOUND-07, FOUND-08 | T-01-06, T-01-13 | ci.yml has lint/test/image jobs; 2-cell arch matrix (testcontainers covers both backends per cell); every run step calls `just`; `packages: write` scoped per-job to `image` only | YAML + grep | `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"` AND `grep -nE "^\s*run: (cargo\|docker \|rustup \|sqlx \|npm \|npx )" .github/workflows/ci.yml` (must be empty) | ❌ W0 | ⬜ pending |
| 01-07-02 | 07 | 5 | FOUND-09 | T-01-14, T-01-15 | Dockerfile multi-stage cargo-zigbuild to distroless/static nonroot | build | `test -s Dockerfile && grep -q "cargo zigbuild" Dockerfile && grep -q "nonroot:nonroot" Dockerfile` (full `just image` build is manual on buildx 0.12+) | ❌ W0 | ⬜ pending |
| 01-08-01 | 08 | 2 | FOUND-10, FOUND-11 | T-01-01, T-01-10 | README first H2 is Security; mermaid diagrams; no ASCII box drawing | grep | `head -50 README.md | grep -q '^## Security' && grep -q 'THREAT_MODEL.md' README.md` | ❌ W0 | ⬜ pending |
| 01-08-02 | 08 | 2 | FOUND-10, FOUND-11 | T-01-01, T-01-10 | THREAT_MODEL.md STRIDE skeleton covers Docker socket + loopback + no-auth-v1 | grep | `test -s THREAT_MODEL.md && grep -q 'Docker socket' THREAT_MODEL.md && grep -q 'Spoofing' THREAT_MODEL.md && grep -q 'Tampering' THREAT_MODEL.md && grep -q 'Elevation' THREAT_MODEL.md` | ❌ W0 | ⬜ pending |
| 01-08-03 | 08 | 2 | CONF-07 | — | examples/cronduit.toml parses end-to-end via `cronduit check` | integration | `RESTIC_PASSWORD=placeholder cargo run --quiet -- check examples/cronduit.toml` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

**All tasks have an automated `<automated>` verify command. Nyquist sampling continuity holds: zero gaps of 3 consecutive tasks without automated verify.**

---

## Wave 0 Requirements

Wave 0 is Plan 01 (Task 1 + Task 2). It creates the entire Rust workspace scaffold. Without it, no `cargo test` command runs at all. After Plan 01 Wave 1 lands:

- [x] `Cargo.toml` + `rust-toolchain.toml` + `.cargo/config.toml` + `.config/nextest.toml` (Plan 01 Task 1)
- [x] `src/main.rs` + `src/lib.rs` + `src/cli/{mod,check,run}.rs` + `src/telemetry.rs` + `src/shutdown.rs` + `src/web/mod.rs` + stubs for `src/config/mod.rs` + `src/db/mod.rs` (Plan 01 Task 2)

Subsequent Wave 0-equivalent scaffolding items created in later plans (required by their own tasks):

- `justfile` (Plan 06 Task 1) — required before `just openssl-check` can run
- `tests/fixtures/*.toml` (Plan 02 Task 2) — required before any assert_cmd test
- `tests/schema_parity.rs` (Plan 05 Task 1) — required before `just schema-diff`
- `migrations/sqlite/20260410_000000_initial.up.sql` + `migrations/postgres/20260410_000000_initial.up.sql` (Plan 04 Task 1) — required before `sqlx::migrate!` macro compiles in `src/db/mod.rs`
- `.github/workflows/ci.yml` (Plan 07 Task 1) — required before Success Criterion #4 is verifiable end-to-end
- `examples/cronduit.toml` (Plan 08 Task 3) — required before Dockerfile build context is valid

**Wave 0 status:** all items pending until Plan 01 (and subsequently 02, 04, 05, 06, 07, 08) land. The `wave_0_complete` frontmatter flag flips to `true` only when every item above exists on disk.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `cronduit` binary runs under Docker on a real host with the socket mounted | OPS-03 | Requires a Docker daemon + host filesystem — not reproducible inside a CI job without Docker-in-Docker complications | 1. `just image` to build the multi-arch image (requires buildx 0.12+). 2. `docker run --rm -v /var/run/docker.sock:/var/run/docker.sock -v $(pwd)/examples/cronduit.toml:/etc/cronduit/config.toml:ro cronduit:dev cronduit check /etc/cronduit/config.toml` must exit 0. 3. `docker run ... cronduit run --config /etc/cronduit/config.toml` must emit the `cronduit.startup` JSON log line within 5 s and keep running until SIGTERM. |
| Loud non-loopback bind warning is visible and readable to a human operator | FOUND-05 / D-24 / T-01-01 | The warning text must be scannable in a terminal, not just present in the log stream | Set `[server].bind = "0.0.0.0:8080"` in a test config. Run `cronduit run --config that.toml --log-format=text` in a terminal. Confirm the WARN line is visible in the scroll-back AND that `bind_warning: true` appears in the `cronduit.startup` JSON event (switch to `--log-format=json` to verify). |
| README SECURITY section is intelligible to a stranger self-hosting for the first time | FOUND-10 / FOUND-11 / T-01-01 | Documentation clarity is subjective | Ask a reviewer who has never seen Cronduit to read the README top-to-bottom and answer: "Would I run this on my LAN? Would I put it behind a reverse proxy? Do I understand the Docker-socket risk?" Reviewer comment recorded in PR. |
| Multi-arch Docker image loads both amd64 and arm64 layers on a buildx 0.12+ runner | FOUND-09 / T-01-15 | Requires buildx + docker-in-docker setup; automated fully in CI via `just image`, but the first local run on a new dev machine should be verified manually | `just image` then `docker buildx inspect cronduit:dev` must show both `linux/amd64` and `linux/arm64` manifests. |

---


### Footnote on DB-07 (schema-only in Phase 1)

Phase 1 provides the `jobs.enabled` column only. The runtime behavior — "removed jobs are marked `enabled=0` rather than deleted" — requires the config reload sync engine which is implemented in **Phase 5**. The Phase 1 verifier MUST NOT assert the soft-delete behavior; task `01-04-05` only asserts that the column exists (via the schema parity + migrations idempotency tests). DB-07 remains listed in Plan 04's `requirements` frontmatter because the schema provision is a real Phase 1 contribution — Phase 5 depends on the column existing — but the runtime soft-delete is out of scope for this phase.

---
## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (Plan 01 scaffolds the test surface; later plans create their own test fixtures/harnesses)
- [x] No watch-mode flags (no `cargo watch`, no `--watch`, no long-lived test processes in CI)
- [x] Feedback latency < 120 s for `just test`; < 600 s for `just ci`
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** planner-approved 2026-04-10. `wave_0_complete` flips to `true` during execution once the scaffold files listed above exist on disk.
