# Deferred / Out-of-Scope Items — 260519-qcp

These items were discovered during execution but are OUT OF SCOPE for this quick
task (T-I4 webhook URL credential scrubbing). They are pre-existing and unrelated
to the changes made here.

## Postgres / Docker-socket integration test failures (environment, not code)

`just nextest` reports 13 failing tests, ALL of which are Postgres-backed or
Docker-socket-backed integration tests that require a running Docker daemon /
Postgres testcontainer. On this machine `/var/run/docker.sock` is not present, so
every one fails at setup with:

```
start postgres: Client(Init(SocketNotFoundError("/var/run/docker.sock")))
```

Confirmed pre-existing (verified against a single targeted run before the Task-2
edits) and entirely outside the webhook scrubbing surface:

- `cronduit::db_pool_postgres db_pool_connects_and_migrates_against_postgres`
- `cronduit::docker_orphan_guard postgres_tests::pg_mark_orphan_running_to_error`
- `cronduit::docker_orphan_guard postgres_tests::pg_mark_orphan_skips_stopped`
- `cronduit::docker_orphan_guard postgres_tests::pg_mark_orphan_skips_all_terminal_statuses`
- `cronduit::dashboard_jobs_pg get_dashboard_jobs_postgres_smoke`
- `cronduit::schema_parity sqlite_and_postgres_schemas_match_structurally`
- `cronduit::v11_bulk_toggle_pg disable_missing_clears_override_pg`
- `cronduit::v11_bulk_toggle_pg bulk_set_override_pg`
- `cronduit::v11_bulk_toggle_pg upsert_invariant_pg`
- `cronduit::v11_bulk_toggle_pg get_overridden_jobs_alphabetical_pg`
- `cronduit::v11_bulk_toggle_pg dashboard_filter_pg`
- `cronduit::v11_runnum_migration migration_03_postgres_not_null`
- `cronduit::v13_timeline_explain explain_uses_index_postgres`

These pass in CI (which provisions Postgres + Docker) and on any dev box with a
Docker daemon running. No action taken — not in scope for T-I4.
