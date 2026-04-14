---
phase: 01-foundation-security-posture-persistence-base
fixed_at: 2026-04-11T12:00:00Z
review_path: .planning/phases/01-foundation-security-posture-persistence-base/01-REVIEW.md
iteration: 2
findings_in_scope: 2
fixed: 2
skipped: 0
status: all_fixed
---

# Phase 01: Code Review Fix Report

**Fixed at:** 2026-04-11T12:00:00Z
**Source review:** .planning/phases/01-foundation-security-posture-persistence-base/01-REVIEW.md
**Iteration:** 2

**Summary:**
- Findings in scope: 2
- Fixed: 2
- Skipped: 0

## Fixed Issues

### WR-01: CLI `--database-url` flag holds credentials as plain `String` in memory

**Files modified:** `src/cli/run.rs`
**Commit:** e7b5a3d
**Applied fix:** Changed `resolved_db_url` from `String` to `SecretString` in `run.rs`. The CLI flag value is now wrapped via `SecretString::from(flag.clone())`, and both the `DbPool::connect` call and the startup log use `.expose_secret()` to access the underlying value. This ensures credential bytes are zeroed on drop regardless of whether the database URL comes from the CLI flag or config file, making both code paths consistent.

### WR-02: Default Docker image will fail to start when `RESTIC_PASSWORD` is unset

**Files modified:** `examples/cronduit.toml`
**Commit:** c7ebf79
**Applied fix:** Commented out the `[jobs.env]` section and the `RESTIC_PASSWORD = "${RESTIC_PASSWORD}"` line in the example config. The nightly-backup job definition is preserved as a useful example of Docker container jobs, but the env block that would cause a hard parse-time failure when `RESTIC_PASSWORD` is unset is now a commented-out example with instructions for operators. The config remains valid TOML and all three example jobs parse without requiring external environment variables.

---

_Fixed: 2026-04-11T12:00:00Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 2_
