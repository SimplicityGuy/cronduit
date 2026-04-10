---
phase: 01-foundation-security-posture-persistence-base
plan: 01
subsystem: scaffold
tags: [rust, cargo, toolchain, cli, tracing, axum, shutdown]
dependency_graph:
  requires: []
  provides: [cargo-workspace, cli-skeleton, telemetry, graceful-shutdown, web-placeholder, config-stub, db-stub]
  affects: [01-02, 01-03, 01-04, 01-05, 01-06, 01-07, 01-08]
tech_stack:
  added: [tokio-1.51, axum-0.8.8, tower-http-0.6.8, sqlx-0.8.6, clap-4.6, tracing-0.1.44, tracing-subscriber-0.3.23, anyhow-1.0.102, thiserror-2.0.18, secrecy-0.10.3, chrono-0.4.44, chrono-tz-0.10.4, serde-1.0.228, toml-1.1.2, humantime-2.3.0, humantime-serde-1.1.1, sha2-0.10, serde_json-1, url-2, regex-1, once_cell-1, tokio-util-0.7.18, hyper-1]
  patterns: [clap-derive-subcommands, tracing-subscriber-json-format, cancellation-token-shutdown, axum-graceful-shutdown]
key_files:
  created: [Cargo.toml, Cargo.lock, rust-toolchain.toml, .cargo/config.toml, .config/nextest.toml, src/main.rs, src/lib.rs, src/cli/mod.rs, src/cli/check.rs, src/cli/run.rs, src/telemetry.rs, src/shutdown.rs, src/web/mod.rs, src/config/mod.rs, src/db/mod.rs, assets/src/app.css]
  modified: [.gitignore]
decisions:
  - "Edition 2024 with rustc 1.94.1 pinned via rust-toolchain.toml"
  - "sqlx with tls-rustls only; zero openssl-sys in dependency graph"
  - "JSON log format as default per D-03; text opt-in via --log-format=text"
  - "Minimal src/main.rs and src/lib.rs stubs created in Task 1 for build verification (plan Task 2 action replaced them)"
metrics:
  duration: 552s
  completed: 2026-04-10T04:25:09Z
  tasks_completed: 2
  tasks_total: 2
  files_created: 16
  files_modified: 1
---

# Phase 01 Plan 01: Rust Workspace Scaffold Summary

Compiling cronduit binary with clap CLI skeleton, tracing JSON/text subscriber, SIGINT/SIGTERM graceful shutdown, minimal axum placeholder, and stub modules for config and db -- all Phase 1 deps version-pinned per STACK.md with zero openssl-sys in the graph.

## What Was Done

### Task 1: Cargo.toml, toolchain, and build configuration

Created the Rust project manifest with all Phase 1 dependencies version-pinned to match STACK.md and CLAUDE.md specifications. Key points:

- `Cargo.toml`: edition 2024, rust-version 1.94.1, all deps from the plan verbatim
- `rust-toolchain.toml`: channel 1.94.1, musl targets for cross-compile, rustfmt + clippy components
- `.cargo/config.toml`: `SQLX_OFFLINE=true` for offline query checking
- `.config/nextest.toml`: CI profile with fail-fast=false, exponential retry, JUnit output
- `.gitignore`: extended with `*.db`, `*.db-wal`, `*.db-shm`, `bin/tailwindcss`, `.sqlx/tmp/`
- `Cargo.lock` generated via initial build

**sqlx** uses `tls-rustls` feature (never `tls-native-tls`). `cargo tree -i openssl-sys` confirms the package is absent from the entire dependency graph.

### Task 2: Source module skeleton

Created 11 source files implementing the full CLI dispatch pipeline:

- `src/main.rs` -- binary entrypoint: parses clap args, inits telemetry, dispatches
- `src/lib.rs` -- library root re-exporting all 6 modules
- `src/cli/mod.rs` -- clap Parser with `Run` and `Check` subcommands, global flags
- `src/cli/check.rs` -- stub returning exit code 2 with "not yet implemented" message
- `src/cli/run.rs` -- stub wiring bind address, AppState, CancellationToken, and web::serve
- `src/telemetry.rs` -- tracing-subscriber init with JSON (default) and Text format variants
- `src/shutdown.rs` -- installs SIGINT and SIGTERM handlers via CancellationToken
- `src/web/mod.rs` -- minimal axum Router with TraceLayer and graceful shutdown
- `src/config/mod.rs` -- stub Config struct (Plan 02 replaces)
- `src/db/mod.rs` -- stub DbPool struct (Plan 04 replaces)
- `assets/src/app.css` -- empty Tailwind entrypoint

## Verification Results

| Check | Result |
|-------|--------|
| `cargo build --workspace` | exit 0 |
| `cargo run -- --help` shows `run` and `check` | PASS |
| `cargo run -- --help` shows `--config`, `--bind`, `--database-url`, `--log-format` | PASS |
| `cargo run -- check /dev/null` exits with code 2 | PASS |
| `cargo run -- check /dev/null` stderr contains "not yet implemented" | PASS |
| `cargo run -- run --bind 127.0.0.1:0 --log-format json` emits JSON with "listening" | PASS |
| SIGTERM causes clean shutdown with exit 0 | PASS |
| `cargo tree -i openssl-sys` returns empty (exit 101) | PASS |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Minimal source stubs needed for Task 1 build verification**
- **Found during:** Task 1
- **Issue:** Task 1's acceptance criteria requires `cargo build --workspace` to exit 0, but the plan creates source files only in Task 2. Without `src/main.rs` and `src/lib.rs`, the build fails.
- **Fix:** Created minimal placeholder `src/main.rs` (one-line `fn main()`) and `src/lib.rs` (empty doc comment) in Task 1 to satisfy the build. Task 2 then replaced both files with the full content.
- **Files modified:** `src/main.rs`, `src/lib.rs` (temporary, replaced in Task 2)
- **Commit:** 48a9664

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | 48a9664 | Cargo.toml, toolchain, build config, .gitignore |
| 2 | be5d669 | Full src/ module skeleton with CLI, telemetry, shutdown, web |

## Known Stubs

| File | Line | Description | Resolving Plan |
|------|------|-------------|----------------|
| `src/cli/check.rs` | 7 | Returns exit 2 with "not yet implemented" | Plan 03 |
| `src/cli/run.rs` | 8 | Stub boot flow (no config parse, no DB, no migrate) | Plan 04 |
| `src/config/mod.rs` | 4 | Empty Config struct, no parsing | Plan 02 |
| `src/db/mod.rs` | 4 | Empty DbPool struct, no pool construction | Plan 04 |
| `assets/src/app.css` | 1 | Empty Tailwind file, no directives | Phase 3 |

All stubs are intentional scaffolding; downstream plans replace them without modifying `src/main.rs` or `src/lib.rs`.

## Self-Check: PASSED

- All 16 created files verified present on disk
- Both commit hashes (48a9664, be5d669) found in git log
