# Phase 10 Deferred Items

Items discovered during plan execution that are out-of-scope for the current plan and deferred to a later plan or phase.

## Pre-existing fmt drift in src/scheduler/command.rs

**Discovered during:** Plan 10-04 (Wave 4)
**State at base commit f821cd9:** `cargo fmt --check` reports two formatting violations in `src/scheduler/command.rs` at L348 and L385 — two `execute_command(...)` test calls in `mod tests` are broken across multiple lines and rustfmt wants them collapsed onto a single line.

**Verification:**
```bash
git checkout f821cd9 -- src/scheduler/command.rs
cargo fmt -p cronduit -- --check src/scheduler/command.rs
# exits 1 with the diff above
```

**Why deferred:** Plan 10-04's `files_modified` list explicitly does NOT include `src/scheduler/command.rs`. Per executor scope-boundary rules, only fixes directly caused by the current plan's changes are in scope; pre-existing CI-gate failures in unrelated files are deferred.

**Likely cause:** Plan 10-03 (the spike) added the two `stop_operator_yields_stopped` and `shutdown_cancel_yields_shutdown` tests but committed them without running `cargo fmt` first.

**Recommended owner:** A trivial follow-up plan or hygiene commit in any subsequent Phase 10 wave can run `cargo fmt -p cronduit` and commit the 18-line diff. This will unblock CI for the entire phase. Plan 10-05 or 10-10 are good candidates.
