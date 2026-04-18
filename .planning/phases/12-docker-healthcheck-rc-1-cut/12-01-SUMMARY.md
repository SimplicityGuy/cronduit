---
phase: 12
plan: 01
subsystem: cli
tags: [cli, dependencies, healthcheck, skeleton, phase-12, ops-06]
dependency_graph:
  requires:
    - "src/cli/mod.rs — existing Command enum + dispatch()"
    - "src/main.rs — exit-code unwrapping via cli::dispatch"
    - "hyper 1 (already in Cargo.toml as transitive)"
  provides:
    - "`cronduit health` subcommand reachable through clap"
    - "src/cli/health.rs with canonical `execute(&Cli) -> anyhow::Result<i32>` signature"
    - "hyper-util 0.1 + http-body-util 0.1 declared as direct deps"
  affects:
    - "Plan 12-02 (implements the probe body + 7 unit tests)"
    - "Plan 12-03 (Dockerfile `HEALTHCHECK CMD [\"/cronduit\", \"health\"]`)"
tech_stack:
  added:
    - "hyper-util 0.1.20 (direct; client-legacy + http1 + tokio features)"
    - "http-body-util 0.1.3 (direct; default features)"
  patterns:
    - "Clap subcommand wiring: pub mod + enum variant with doc + dispatch arm"
    - "Exit-code contract via anyhow::Result<i32> (matches check.rs, run.rs)"
key_files:
  created:
    - "src/cli/health.rs"
  modified:
    - "Cargo.toml"
    - "Cargo.lock"
    - "src/cli/mod.rs"
decisions:
  - "Honored D-01 verbatim: features `[client-legacy, http1, tokio]` — no TLS feature; rustls invariant preserved."
  - "Honored D-03: `Health` variant carries no subcommand-local args; reuses global `--bind`."
  - "Honored D-04: the skeleton does not touch `cli.config`; health path is config-free."
  - "Honored D-05 surface contract: `pub async fn execute(_cli: &Cli) -> anyhow::Result<i32>` ready for Plan 12-02 to fill in."
  - "Split Cargo.lock update into its own commit (Task 1 follow-up) to keep the dep-declaration commit minimal and give the lockfile change standalone traceability."
metrics:
  duration: "5m 45s"
  completed: "2026-04-18T00:41:35Z"
  tasks_completed: 3
  files_created: 1
  files_modified: 3
  lines_added: 38
  commits: 4
---

# Phase 12 Plan 01: CLI Skeleton + Dep Declaration Summary

Wires the `cronduit health` clap subcommand through to a placeholder `execute(&Cli) -> anyhow::Result<i32>` handler and declares `hyper-util` + `http-body-util` as direct dependencies, establishing the file-and-symbol contracts that Plan 12-02 (probe implementation + 7 unit tests) and Plan 12-03 (`Dockerfile HEALTHCHECK`) land against — all without adding a TLS surface or breaking the green build.

## What Shipped

Three tasks executed exactly as planned, no deviations:

| Task | Summary                                                                                   | Commit    |
| ---- | ----------------------------------------------------------------------------------------- | --------- |
| 1    | Declare `hyper-util = { version = "0.1", features = ["client-legacy", "http1", "tokio"] }` and `http-body-util = "0.1"` in the `# HTTP / web placeholder` group of `Cargo.toml`. | `c87dac9` |
| 2    | Add `pub mod health;` (alphabetical), `Health` variant with `///` doc comment to the `Command` enum, and `Command::Health => health::execute(&cli).await` dispatch arm in `src/cli/mod.rs`. | `4cf2705` |
| 3    | Create `src/cli/health.rs` (28 lines) with module doc citing D-01..D-05 and the canonical skeleton body `Ok(0)` so the binary builds end-to-end. | `91b6150` |
| 1b   | Commit `Cargo.lock` update (root package section lists the two new direct deps). | `069290b` |

## Files Changed

### Created

- **`src/cli/health.rs`** (28 lines)
  - Module docstring cross-references D-01..D-05 so the Plan 12-02 planner can resume without re-reading CONTEXT.md.
  - `use crate::cli::Cli;` import so the signature compiles.
  - `pub async fn execute(_cli: &Cli) -> anyhow::Result<i32>` — returns `Ok(0)` placeholder. Underscore prefix silences the unused-variable warning until Plan 12-02 wires the probe.

### Modified

- **`Cargo.toml`** (+2 lines inside the `# HTTP / web placeholder` group)

  ```toml
  hyper-util = { version = "0.1", features = ["client-legacy", "http1", "tokio"] }
  http-body-util = "0.1"
  ```

- **`src/cli/mod.rs`** (+6 lines, three coordinated additive edits)
  - `pub mod health;` between `pub mod check;` and `pub mod run;`.
  - `Health` variant with a three-line `///` doc comment, placed after `Check`.
  - `Command::Health => health::execute(&cli).await,` dispatch arm.
  - `Cli` struct, `Run`, `Check`, `LogFormatArg` unchanged.

- **`Cargo.lock`** (+2 lines — root package entries for `http-body-util` and `hyper-util`)

## Verification Command Outputs

```text
$ cargo check
... Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.54s

$ cargo build
... Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.17s

$ cargo run -- health
(exits 0 — placeholder)

$ cargo run -- --help
...
Commands:
  run     Run the cronduit daemon (loads config, migrates DB, serves web UI)
  check   Validate a config file without touching the database
  health  Probe the local /health endpoint and exit 0 if status="ok". Intended as a Dockerfile HEALTHCHECK target. ...
  help    Print this message or the help of the given subcommand(s)

$ cargo run -- health --help
(exits 0 — clap renders usage)

$ cargo tree -i hyper-util
hyper-util v0.1.20
├── axum v0.8.9
│   └── ...

$ cargo tree -i http-body-util
http-body-util v0.1.3
├── axum v0.8.9
│   └── ...

$ cargo tree -i openssl-sys
error: package ID specification `openssl-sys` did not match any packages
(rustls-only invariant preserved — CLAUDE.md § Constraints)

$ cargo clippy --all-targets --all-features -- -D warnings
... Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.06s
(no warnings surfaced)
```

## Green-Build Confirmation for Plan 12-02

Plan 12-02 can begin immediately from this commit:

- `src/cli/health.rs::execute` signature matches the contract in `<interfaces>`.
- `hyper-util::client::legacy::{Client, connect::HttpConnector}` and `hyper_util::rt::TokioExecutor` are resolvable (`cargo tree` confirms 0.1.20).
- `http_body_util::{BodyExt, Empty}` are resolvable (`cargo tree` confirms 0.1.3).
- `cli::Cli` is already imported; `_cli.bind` will be the input to the URL builder.
- `serde_json` (already in `Cargo.toml` L84) provides the body-parse path — no additional deps needed.

## Deviations from Plan

None — this was mechanical wiring. All three task acceptance criteria passed on first verification.

### Minor execution notes (not deviations)

1. **Cargo.lock split into its own commit.** The plan's Task 1 did not explicitly name `Cargo.lock`, but `cargo check` after the Cargo.toml edit correctly updated the lockfile's root-package section (the two new direct deps are listed alongside the previously-transitive versions). To keep the dep-declaration commit minimal and to give the lockfile change standalone traceability for a reviewer, it was split into its own `chore(12-01)` follow-up commit (`069290b`). Both commits cite `12-01` so `git log --grep "12-01"` surfaces them together.

2. **Acceptance criterion arithmetic.** Task 2's acceptance criterion says `grep -c '#\[arg(long, global = true)\]' src/cli/mod.rs` returns `3` — the actual count is `2` (two lines use the exact token without `short = 'c'` or `default_value = "json"`). The count is unchanged from the pre-edit state (verified via `git show HEAD~3:src/cli/mod.rs`), so the spirit of the criterion ("Cli struct unchanged") is satisfied. The plan's `3` appears to be an off-by-one in the verification expression — noted here so downstream plans don't re-propagate the arithmetic.

## Threat Model Disposition

All four threats from the plan's `<threat_model>` section are mitigated as specified:

| Threat ID  | Category    | Mitigation                                                                                                     |
| ---------- | ----------- | -------------------------------------------------------------------------------------------------------------- |
| T-12-01-01 | Tampering   | Both deps pinned to `"0.1"`; already transitive via axum + bollard — no new top-level supply-chain surface.    |
| T-12-01-02 | Info-disc.  | `cargo tree -i openssl-sys` returns "did not match any packages"; `Cargo.toml` does not contain `hyper-rustls`. |
| T-12-01-03 | EoP         | Skeleton returns `Ok(0)` unconditionally — no privileged operation. Probe logic (Plan 12-02) owns its own threat model. |
| T-12-01-04 | DoS (clap)  | `cargo run -- health` and `cargo run -- health --help` both exit 0; clap derive succeeded.                     |

## Known Stubs

- **`src/cli/health.rs::execute`** — intentional skeleton. Returns `Ok(0)` unconditionally; does not open a socket, does not parse a URL, does not check the body. This is by design per the plan's `<objective>` ("body returns `Ok(0)` placeholder so the binary builds and Plan 12-02 can fill in the implementation") and the threat register entry T-12-01-03. Plan 12-02 (already scoped in `.planning/phases/12-docker-healthcheck-rc-1-cut/12-02-PLAN.md`) replaces the body with the real hyper-util probe + 7 unit tests.

No unintended stubs or hardcoded placeholders introduced.

## Self-Check: PASSED

- `src/cli/health.rs` — FOUND
- `src/cli/mod.rs` — FOUND (modified)
- `Cargo.toml` — FOUND (modified)
- `Cargo.lock` — FOUND (modified)
- Commit `c87dac9` (chore: declare deps) — FOUND
- Commit `4cf2705` (feat: wire Command::Health) — FOUND
- Commit `91b6150` (feat: add health.rs skeleton) — FOUND
- Commit `069290b` (chore: lockfile update) — FOUND
