---
phase: 01-foundation-security-posture-persistence-base
plan: 03
subsystem: cli
tags: [config-validation, assert-cmd, gcc-errors, security]

# Dependency graph
requires:
  - phase: 01-foundation-security-posture-persistence-base
    plan: 01
    provides: "CLI dispatch scaffold with check stub"
  - phase: 01-foundation-security-posture-persistence-base
    plan: 02
    provides: "parse_and_validate pipeline, ConfigError with GCC Display, test fixtures"
provides:
  - "Working `cronduit check <config>` subcommand with collect-all GCC-style errors"
  - "6 black-box integration tests proving FOUND-03 (no DB I/O, no secret leaks)"
affects: [04-schema-migration, 05-ci]

# Tech tracking
tech-stack:
  added: []
  patterns: ["assert_cmd black-box CLI testing", "GCC-style error output format"]

key-files:
  created: [tests/check_command.rs]
  modified: [src/cli/check.rs]

key-decisions:
  - "Used crate::config path (not cronduit::config) since cli module is inside the lib crate"

patterns-established:
  - "CLI subcommand testing via assert_cmd::Command::cargo_bin for black-box verification"
  - "GCC-style error format: path:line:col: error: message"

requirements-completed: [FOUND-03]

# Metrics
duration: 3min
completed: 2026-04-10
---

# Phase 01 Plan 03: cronduit check Subcommand Summary

**Wired `cronduit check` to parse_and_validate with GCC-style collect-all errors, proven by 6 black-box tests covering valid/invalid/no-DB/no-secret-leak**

## Performance

- **Duration:** 3 min
- **Started:** 2026-04-10T04:40:49Z
- **Completed:** 2026-04-10T04:44:07Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Replaced Plan 01 check.rs stub with full parse_and_validate dispatch
- GCC-style error output on stderr with collect-all behavior (D-21)
- 6 passing black-box tests proving FOUND-03 end-to-end
- Verified: no DB files created, no secret values leaked, correct exit codes

## Task Commits

Each task was committed atomically:

1. **Task 1: Replace check.rs stub with parse_and_validate wiring** - `3b202bc` (feat)
2. **Task 2: Create black-box assert_cmd test suite** - `1c43c9e` (test)

## Files Created/Modified
- `src/cli/check.rs` - Full parse_and_validate dispatch with GCC-style error printer (replaced stub)
- `tests/check_command.rs` - 6 black-box integration tests via assert_cmd

## Observed Behavior

**Valid config:**
```
$ cronduit check tests/fixtures/valid-minimal.toml
ok: tests/fixtures/valid-minimal.toml
(exit 0)
```

**Invalid config (multiple errors):**
```
$ cronduit check tests/fixtures/invalid-multiple.toml
tests/fixtures/invalid-multiple.toml: error: not a valid IANA timezone: `Not/A/Real_Zone` (see [server].timezone)
tests/fixtures/invalid-multiple.toml:10:1: error: duplicate job name `dup` (first declared at tests/fixtures/invalid-multiple.toml:5)

2 error(s)
(exit 1)
```

## Test Results

6 tests, all passing:
- `check_valid_minimal_exits_zero` - exit 0, stderr contains "ok:"
- `check_missing_timezone_reports_error` - exit 1, stderr contains "error:" and timezone reference
- `check_collects_all_errors` - exit 1, >= 2 error lines, "N error(s)" summary
- `check_nonexistent_file_reports_cannot_read` - exit 1, "cannot read file"
- `check_does_not_open_db` - no .db files created in temp dir (T-01-05 mitigation)
- `check_does_not_leak_secret_value` - distinctive env var value never appears in output (T-01-02 mitigation)

## Decisions Made
- Used `crate::config` import path instead of `cronduit::config` since the cli module lives inside the library crate

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `cronduit check` is fully functional and tested
- Ready for Plan 04 (schema/migration) which is independent
- The parse_and_validate pipeline shared by `check` and future `run` is proven end-to-end

---
*Phase: 01-foundation-security-posture-persistence-base*
*Completed: 2026-04-10*
