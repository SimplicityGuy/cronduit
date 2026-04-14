---
status: complete
phase: 01-foundation-security-posture-persistence-base
source: [01-01-SUMMARY.md, 01-02-SUMMARY.md, 01-03-SUMMARY.md, 01-04-SUMMARY.md, 01-05-SUMMARY.md, 01-06-SUMMARY.md, 01-07-SUMMARY.md, 01-08-SUMMARY.md, 01-09-SUMMARY.md]
started: 2026-04-10T01:30:00Z
updated: 2026-04-10T02:00:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Cold Start Smoke Test
expected: Kill any running cronduit process. Run `cargo build --workspace` from a clean state. Then run `cargo run --quiet -- --help`. The binary should compile without errors and print CLI help showing `check` and `run` subcommands.
result: pass

### 2. Config Validation — Valid Config
expected: Run `RESTIC_PASSWORD=test cargo run --quiet -- check examples/cronduit.toml`. Should print `ok: examples/cronduit.toml` and exit 0. The check command validates TOML parsing, env-var interpolation, timezone, cron expressions, and network modes — all without touching a database.
result: pass

### 3. Config Validation — Invalid Cron Expression
expected: Run `cargo run --quiet -- check tests/fixtures/invalid-schedule.toml`. Should exit non-zero and print an error containing "invalid cron expression" to stderr with a GCC-style `path:line:col: error:` format.
result: pass

### 4. Config Validation — Missing Env Var
expected: Run `cargo run --quiet -- check tests/fixtures/invalid-missing-env-var.toml` (without setting the referenced env var). Should exit non-zero with an error about the missing environment variable, mentioning the variable NAME but never its value.
result: pass

### 5. Config Validation — Collect-All Errors
expected: Run `cargo run --quiet -- check tests/fixtures/invalid-multiple.toml`. Should exit non-zero and display TWO OR MORE distinct error messages on stderr (not just the first error). This confirms the collect-all-errors design.
result: pass

### 6. Database Migration — SQLite
expected: Run `RESTIC_PASSWORD=test cargo run --quiet -- run --config examples/cronduit.toml --database-url sqlite:///tmp/cronduit-uat-test.db`. The process should start, emit a JSON startup log line containing `cronduit.startup`, then stay running. Send SIGTERM (Ctrl+C) and it should exit cleanly with exit code 0. Verify `/tmp/cronduit-uat-test.db` was created.
result: pass

### 7. Non-Loopback Bind Warning
expected: Create a temp config with `bind = "0.0.0.0:9999"` and run cronduit. The JSON startup log should contain `bind_warning: true` and a WARN-level log line about the non-loopback bind explaining the no-auth-in-v1 stance. Default bind should be `127.0.0.1:8080` if not specified.
result: pass

### 8. SecretString Redaction
expected: Run `cargo test --test config_parser valid_with_secrets -- --nocapture`. The test should pass, confirming that `format!("{:?}", config)` never contains the actual secret value — it shows `[REDACTED]` instead.
result: pass

### 9. Justfile Recipes
expected: Run `just --list`. Should show recipes including: `test`, `ci`, `fmt-check`, `clippy`, `openssl-check`, `nextest`, `schema-diff`, `image`, `dev`, `check-config`. Run `just fmt-check` — should exit 0 (no formatting diffs).
result: pass

### 10. OpenSSL-Free Dependency Tree
expected: Run `just openssl-check` (or `cargo tree -i openssl-sys`). Should produce NO output and exit 0, confirming zero `openssl-sys` in the entire dependency graph. This enforces the rustls-only constraint.
result: pass

### 11. README Security-First Layout
expected: Open `README.md`. The FIRST `## ` heading should be `## Security`. It should mention Docker socket risk, default loopback bind, and unauthenticated web UI in v1. `THREAT_MODEL.md` should exist and contain STRIDE headings (Spoofing, Tampering, Repudiation, Information Disclosure, Denial of Service, Elevation of Privilege).
result: pass

### 12. Mermaid-Only Diagrams
expected: Run `grep -rn '┌\|┐\|└\|┘\|─\|│\|├\|┤\|╔\|╗\|╚\|╝\|═\|║' README.md THREAT_MODEL.md` — should find NO matches (no ASCII box-drawing characters). Run `grep -c 'mermaid' README.md THREAT_MODEL.md` — should find at least 1 mermaid block in each file.
result: pass

## Summary

total: 12
passed: 12
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps

[none]
