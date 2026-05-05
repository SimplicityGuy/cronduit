# Deferred items — Phase 23 execution

## 23-02 — Pre-existing schema_parity Docker dependency

The `sqlite_and_postgres_schemas_match_structurally` test in `tests/schema_parity.rs`
requires a running Docker daemon (`testcontainers` Postgres). On the developer machine
(`darwin`) the Docker socket at `/var/run/docker.sock` is not present, so the test panics
during setup. This is environmental, not caused by Phase 23 changes — the same failure
reproduces on `main` before the 23-02 widening commits.

CI runs Docker, so this test passes there. No action needed in 23-02.

The two pure-logic tests in the same file (`known_types_normalize_correctly`,
`unknown_type_panics`) both pass locally and verify the TEXT-family normalization
that this phase relies on for parity.
