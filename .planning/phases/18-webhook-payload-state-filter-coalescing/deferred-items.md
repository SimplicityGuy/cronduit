# Deferred Items — Phase 18

Pre-existing issues observed during plan execution that are out of scope for the current plan.

## From Plan 18-02 execution

### Pre-existing flaky test: `tests/v12_labels_interpolation.rs`

Two tests share env-var `TEAM` and use `unsafe { std::env::remove_var/set_var }`:
- `lbl_05_key_position_interpolation_env_set_resolves_to_literal_when_pattern_matches`
- `lbl_05_key_position_interpolation_env_unset_caught_by_strict_chars`

When run concurrently (default `cargo test`), one test's env-var mutation leaks into the
other's body, producing intermittent failures. Passes deterministically with `--test-threads=1`.

**Status:** Pre-existing on `main`. Not caused by Plan 18-02 changes (changes were
purely additive: a new `webhook` field on `JobConfig`/`DefaultsConfig` and validators
that don't touch the labels code path). Out of scope per scope-boundary rule.

**Suggested follow-up:** Either serialize the two tests with a shared `Mutex<()>`, or
move both into the same `#[tokio::test]` body with no concurrent peers. File a separate
fix issue on the next available phase boundary.
