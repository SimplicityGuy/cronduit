---
phase: 01-foundation-security-posture-persistence-base
plan: 02
subsystem: config
tags: [toml, parsing, validation, interpolation, security, secrets]
dependency_graph:
  requires: [01-01]
  provides: [parse_and_validate, ConfigError, SecretString-wiring, config-fixtures]
  affects: [01-03, 01-04]
tech_stack:
  added: [secrecy, chrono-tz, sha2, regex, once_cell, serde_json, humantime-serde]
  patterns: [collect-all-errors, strict-env-interpolation, GCC-style-error-format]
key_files:
  created:
    - src/config/errors.rs
    - src/config/interpolate.rs
    - src/config/validate.rs
    - src/config/hash.rs
    - tests/config_parser.rs
    - tests/fixtures/valid-minimal.toml
    - tests/fixtures/valid-everything.toml
    - tests/fixtures/valid-with-secrets.toml
    - tests/fixtures/invalid-missing-timezone.toml
    - tests/fixtures/invalid-duplicate-job.toml
    - tests/fixtures/invalid-missing-env-var.toml
    - tests/fixtures/invalid-multiple-job-types.toml
    - tests/fixtures/invalid-bad-network.toml
    - tests/fixtures/invalid-multiple.toml
  modified:
    - src/config/mod.rs
decisions:
  - "SecretString wraps database_url and all job env values; Debug renders [REDACTED]"
  - "Env interpolation is strict: only ${VAR} syntax, ${VAR:-default} explicitly rejected"
  - "Semantic validation errors use line:0 col:0 (toml crate lacks per-key spans); duplicate names get real line numbers via raw-text scan"
metrics:
  duration: 411s
  completed: 2026-04-10T04:36:43Z
  tasks_completed: 2
  tasks_total: 2
  test_count: 20
  files_created: 14
  files_modified: 1
---

# Phase 01 Plan 02: TOML Config Parsing Pipeline Summary

Full TOML config parsing pipeline with strict ${VAR} env interpolation, SecretString redaction on all sensitive fields, collect-all validation, and SHA-256 config hashing for future sync detection.

## What Was Built

### Config Module (`src/config/`)

- **mod.rs**: Config struct tree (`Config`, `ServerConfig`, `DefaultsConfig`, `JobConfig`, `ParsedConfig`), `parse_and_validate` entrypoint that collects all errors (D-21). Mandatory `timezone` field (D-19). Default bind `127.0.0.1:8080`.
- **errors.rs**: `ConfigError` type with GCC-style `Display` (`file:line:col: error: message`). `byte_offset_to_line_col` helper for mapping toml spans back to source positions.
- **interpolate.rs**: Strict `${VAR}` env expansion. Missing vars produce `InterpolationError` with byte range. `${VAR:-default}` syntax explicitly rejected with `DefaultSyntaxForbidden`.
- **validate.rs**: Post-parse validators: IANA timezone via `chrono_tz::Tz`, socket address parsing for bind, one-of job type (command/script/image), network mode regex, duplicate job name detection with real line numbers from raw source scan.
- **hash.rs**: `compute_config_hash` produces a stable 64-char SHA-256 hex digest of normalized (sorted-keys BTreeMap) JSON. Excludes `env` (SecretString values must never be hashed/logged). Ready for D-15 config_hash column in Phase 2.

### Test Fixtures (`tests/fixtures/`)

9 TOML fixtures covering every positive and negative case:
- 3 valid: minimal, everything (all fields), with-secrets (env interpolation)
- 6 invalid: missing-timezone, duplicate-job, missing-env-var, multiple-job-types, bad-network, multiple-errors (two independent problems)

### Integration Tests (`tests/config_parser.rs`)

9 integration tests proving:
- Happy path parsing with defaults populated
- SecretString redaction (Debug output does not contain "hunter2")
- Mandatory timezone enforcement
- Duplicate job names with real line numbers
- Missing env var error with variable name in message
- One-of job type validation (CONF-05)
- Network mode regex validation
- Collect-all behavior (>= 2 errors from invalid-multiple.toml)

## Test Results

- **Unit tests**: 11 passing (`cargo test --lib config::`)
- **Integration tests**: 9 passing (`cargo test --test config_parser`)
- **Total**: 20 tests

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | ab2be18 | feat(01-02): implement config parse_and_validate pipeline |
| 2 | abc946b | test(01-02): add 9 TOML fixtures and config_parser integration tests |

## Deviations from Plan

None -- plan executed exactly as written.

## Self-Check: PASSED

All 15 files verified present. Both commit hashes (ab2be18, abc946b) found in git log.
