# Cronduit Configuration Reference

Cronduit is configured via a **single TOML file**. That file is the source of truth — any job present in the database but not in the reloaded config is disabled. There is no layered config, no include-file machinery, and no environment-only config path. This is deliberate: an operator should be able to look at the file and know exactly which jobs are scheduled, without needing to cross-reference three locations.

This document is the complete reference for every field the config file accepts. For a step-by-step walkthrough that ends with a running dashboard, start with [`QUICKSTART.md`](./QUICKSTART.md). For the architectural picture, read [`SPEC.md`](./SPEC.md).

## Table of contents

1. [File structure](#file-structure)
2. [`[server]` section](#server-section)
3. [`[defaults]` section](#defaults-section)
4. [`[[jobs]]` blocks](#jobs-blocks)
5. [Cron syntax](#cron-syntax)
6. [Environment variable interpolation](#environment-variable-interpolation)
7. [Validation](#validation)
8. [Hot reload](#hot-reload)
9. [Patterns and recipes](#patterns-and-recipes)

## File structure

A complete Cronduit config has three top-level sections and one or more `[[jobs]]` array-of-tables entries:

```toml
[server]        # REQUIRED — bind address, timezone, log retention, etc.
# ...

[defaults]      # OPTIONAL — shared fields inherited by [[jobs]] blocks
# ...

[[jobs]]        # one or more job blocks
# ...

[[jobs]]
# ...
```

TOML arrays-of-tables (`[[jobs]]`) preserve the order in which they appear in the file, but Cronduit does not rely on order — jobs are identified by the `name` field, which must be unique.

A minimal working config that passes `cronduit check`:

```toml
[server]
bind = "127.0.0.1:8080"
timezone = "UTC"

[[jobs]]
name = "heartbeat"
schedule = "*/5 * * * *"
command = "echo alive"
```

That's the smallest valid config. No `[defaults]`, no env vars, no secrets, just one command job. Everything else is additive.

## `[server]` section

The `[server]` section is required. It configures the HTTP server, the scheduler's timezone, persistence, and reload behavior.

### `[server].bind`

- **Default:** `"127.0.0.1:8080"`
- **Type:** socket-address string, parsed by Rust's `std::net::SocketAddr::from_str`
- **Example:** `bind = "0.0.0.0:8080"`

The TCP address and port the web UI and HTTP API listen on. Defaults to loopback so a fresh install is never exposed outside the local machine.

If you set this to any non-loopback address, Cronduit emits a **loud `WARN`-level log line at startup** with `bind_warning: true` in the structured startup event. The warning is deliberate — the v1 web UI ships unauthenticated, so any non-loopback bind must be fronted by a reverse proxy with auth (Traefik, Caddy, nginx). See [`../THREAT_MODEL.md`](../THREAT_MODEL.md).

### `[server].timezone`

- **Default:** none — **this field is mandatory**
- **Type:** IANA timezone name
- **Example:** `timezone = "America/Los_Angeles"`, `timezone = "UTC"`, `timezone = "Europe/Berlin"`

All cron expressions in the config are evaluated in this timezone. Cronduit has **no implicit host-timezone fallback** — you must set this explicitly. The mandatory field is documented as decision `D-19` in the project's decision log.

`croner`, the cron parser Cronduit uses, is DST-aware. "Spring forward" and "fall back" edge cases are handled correctly: jobs scheduled for the skipped hour either run once (at the next valid time) or not at all, depending on whether the schedule matches the destination hour; jobs scheduled during the doubled hour run exactly once, not twice.

Invalid timezone names (`America/Los_Angles`, typo of `Angeles`) are caught by `cronduit check` with a clear error.

### `[server].log_retention`

- **Default:** `"90d"`
- **Type:** [humantime](https://docs.rs/humantime/) duration string
- **Example:** `log_retention = "30d"`, `log_retention = "12h"`, `log_retention = "1y"`

How long to keep rows in the `job_logs` table. A daily pruner deletes rows older than this in batches of 1000 with a 100ms sleep between batches to avoid stalling concurrent writes. After a large prune, SQLite gets a `WAL_CHECKPOINT(TRUNCATE)` to reclaim WAL growth.

The pruner emits `tracing` INFO events on every cycle so you can confirm it's running. It also runs once at startup so a freshly-started Cronduit reclaims old rows immediately instead of waiting 24 hours.

### `[server].shutdown_grace`

- **Default:** `"30s"`
- **Type:** humantime duration string
- **Example:** `shutdown_grace = "2m"`, `shutdown_grace = "10s"`

How long to wait for in-flight jobs to finish during graceful shutdown (SIGINT, SIGTERM, or a clean exit). After the grace period expires, Cronduit sends SIGTERM to any remaining container jobs (waiting 10s for each), then terminates.

Set this longer than your longest-running job's expected duration if you want graceful shutdowns to never truncate real work. The HTTP server closes immediately on first signal (decision `D-17`) so the reverse proxy can stop routing to Cronduit, while the scheduler continues draining in-flight jobs in the background.

### `[server].watch_config`

- **Default:** `true`
- **Type:** boolean
- **Example:** `watch_config = false`

Enables the debounced file watcher that reloads the config automatically when the file changes on disk. Set to `false` to disable the watcher — you can still trigger reloads via `POST /api/reload` or SIGHUP. See [Hot reload](#hot-reload) for the complete list of reload paths.

### `[server].database_url`

- **Default:** reads from environment variable `DATABASE_URL`, then falls back to `sqlite://./cronduit.db?mode=rwc`
- **Type:** string (wrapped in `SecretString` — never logged)
- **Example:** `database_url = "sqlite:///data/cronduit.db"`, `database_url = "postgres://cronduit:secret@db.internal/cronduit"`

The database connection URL. Both SQLite and PostgreSQL are supported with the same logical schema.

**SQLite** (default): zero-config, single-file, separate read/write connection pools with WAL journaling and `busy_timeout` to avoid writer-contention collapse under concurrent log writes. Cronduit auto-creates the tables on first run via embedded `sqlx` migrations.

**PostgreSQL** (optional): for shared infrastructure or homelabs that already run Postgres. Same schema; per-backend migration files where dialect differs. Tested in CI for structural parity.

You should **not** put passwords in plaintext here — use env var interpolation: `database_url = "postgres://cronduit:${DB_PASSWORD}@db.internal/cronduit"`. Cronduit resolves `${VAR}` references at parse time and wraps the resolved value in `SecretString`.

## `[defaults]` section

The `[defaults]` section is optional. It provides shared field values that apply to every `[[jobs]]` block unless the job overrides the field or opts out entirely with `use_defaults = false`.

### Which fields are defaults-eligible?

**Only these six fields** can be set under `[defaults]`:

| Field | Scope | Notes |
|---|---|---|
| `image` | docker jobs only | Default Docker image for jobs that don't set their own. |
| `network` | docker jobs only | Default network mode. |
| `volumes` | docker jobs only | Default volume mounts. Arrays replace — per-job `volumes` does not concatenate with the defaults entry. |
| `delete` | docker jobs only | Default "remove container after drain" flag. |
| `timeout` | all job types | Default per-job timeout. |
| `random_min_gap` | **global only** | Minimum gap between `@random`-scheduled jobs on the same day. This one is **not merged per-job** — it's a global scheduler knob consumed directly by the `@random` resolver. See below. |

**No other field is defaults-eligible.** Specifically, these fields are per-job only and will **not** merge from `[defaults]` even if you write them there (the TOML parser will silently accept the unknown key, but `apply_defaults` will not use it):

- `name` — every job must have a unique name.
- `schedule` — every job has its own cron schedule.
- `command`, `script`, `image` — these determine job type; each job declares exactly one.
- `cmd` — docker CMD override, per-job only (see [CONFIG.md § cmd](#jobscmd) below for why).
- `container_name` — container names must be unique, so defaulting would produce collisions.
- `env` — environment variables are job-specific and secret-sensitive.

### How merging works

For each `[[jobs]]` block, Cronduit runs `apply_defaults(job, defaults)` exactly once during config parse, before validation:

1. If `[defaults]` is absent, the job is returned unchanged.
2. If the job has `use_defaults = false`, the job is returned unchanged.
3. Otherwise, for each defaults-eligible field:
   - If the job does **not** set the field, the value from `[defaults]` is copied in.
   - If the job **does** set the field, the job's value wins — `[defaults]` is ignored for this field on this job.
4. For command and script jobs specifically, the docker-only fields (`image`, `network`, `volumes`, `delete`) are **not** merged regardless of what `[defaults]` contains — a command job with `image = "alpine"` from defaults would fail the "exactly one of command/script/image" check. `timeout` still merges into every job type.

Once `apply_defaults` has run, every downstream consumer (validator, DB sync, `config_hash`, executor) reads the already-merged job and never re-consults `[defaults]` for per-job fields. This means the `config_hash` is stable whether you put a field on the job or in `[defaults]` — moving `image = "alpine"` from the job block to `[defaults]` and removing it from the job produces an identical hash, so Cronduit treats it as unchanged and does not churn the DB.

### `[defaults].image`

Default Docker image for container jobs that don't set their own `image`. Only takes effect on docker jobs (jobs that set `image`, either directly or via this default). Command and script jobs never merge this field even if `[defaults]` sets it.

```toml
[defaults]
image = "alpine:3.20"
```

### `[defaults].network`

Default Docker network mode for container jobs. Valid values are the same as the per-job `network` field: `bridge`, `host`, `none`, `container:<name>`, or any named Docker network. See [`[[jobs]].network`](#jobsnetwork) for the full syntax.

```toml
[defaults]
network = "container:vpn"     # route every docker job through the VPN sidecar by default
```

### `[defaults].volumes`

Default volume mount list. The per-job field, when set, **replaces** the defaults entry — arrays do not concatenate.

```toml
[defaults]
volumes = ["/mnt/nas:/data:ro"]
```

If you want a common mount on every job **plus** job-specific mounts, set them all per-job and don't use `[defaults].volumes`. Array concatenation would be nicer ergonomically but would break the "per-job wins, always" rule that keeps the merge semantics easy to reason about.

### `[defaults].delete`

Controls whether Cronduit removes the container after the run completes.

- `delete = true` (the default behavior when the field is unset) — Cronduit explicitly removes the container after `wait_container` drains and the log pipeline closes. This is NOT bollard's `auto_remove`; Cronduit always sets `auto_remove=false` to avoid the moby#8441 race that can truncate exit codes, then explicitly removes the container after all state is captured. See [`SPEC.md § Job Execution`](./SPEC.md#docker-container-jobs).
- `delete = false` — Cronduit preserves the container after the run ends so an operator can inspect it with `docker logs <id>` or `docker inspect <id>` for post-mortem debugging. Cronduit emits an INFO-level log line on every preserved run with the container ID, the job name, and the cleanup command so the container can be found later.

```toml
[defaults]
delete = true
```

**Operator responsibility when `delete = false`:** preserved containers accumulate forever. Cronduit does NOT reap them on restart — orphan reconciliation only touches rows still marked `running` in the database, and a preserved-but-exited container has a final `success`/`failed`/`timeout` status, so it is invisible to reconciliation. Prune preserved containers yourself with `docker container prune` (clears all stopped containers on the host) or a targeted loop using the `cronduit.job_name=<name>` label that every Cronduit-spawned container carries. Consider scheduling this as a Cronduit docker job of its own.

### `[defaults].timeout`

Default per-job timeout. Applies to every job type (command, script, docker). The job-level `timeout` field overrides it.

```toml
[defaults]
timeout = "5m"
```

When a run exceeds its timeout, Cronduit kills the process (for command/script jobs) or stops the container with a 10s SIGTERM grace (for docker jobs) and records the run with `status=timeout` and `failure_reason=timeout`.

### `[defaults].random_min_gap`

**Global knob, NOT merged per-job.** Minimum gap between `@random`-scheduled jobs on the same day, enforced by the `@random` resolver. Defaults to `0s` if unset.

```toml
[defaults]
random_min_gap = "90m"
```

If you have ten jobs with `schedule = "@random * * * *"` and `random_min_gap = "90m"`, the resolver picks ten distinct times on day 1 such that consecutive jobs are at least 90 minutes apart. If the slot is infeasible (too many jobs, too large a gap), the resolver emits a `WARN`-level log with `random_min_gap is infeasible; relaxing gap for overflow jobs` and picks the tightest feasible schedule.

`random_min_gap` is not a per-job field — there is no `[[jobs]].random_min_gap`. It's a property of the scheduler's `@random` resolver as a whole.

## `[[jobs]]` blocks

Each `[[jobs]]` block defines one scheduled job. The block's **type** is inferred from which of `command`, `script`, or `image` is set — exactly one must be set, or `cronduit check` fails. The other fields are shared across all three job types.

### Common fields (all job types)

#### `[[jobs]].name`

- **Required.** Unique identifier for the job across the whole config.
- **Type:** string.
- **Example:** `name = "nightly-backup"`

Used as the primary key in the database and as the subject of every log/metric/UI reference to the job. Duplicate names cause a parse error (with the line number of the second occurrence).

#### `[[jobs]].schedule`

- **Required.**
- **Type:** cron expression string (see [Cron syntax](#cron-syntax)).
- **Example:** `schedule = "*/5 * * * *"`, `schedule = "0 3 L * *"`, `schedule = "@hourly"`, `schedule = "@random * * * *"`

The cron schedule. Cronduit uses `croner` 3.0, which supports 5-field standard cron, 6-field with seconds, Quartz extended modifiers (`L`, `#`, `W`), named macros (`@hourly`, `@daily`, etc.), and the Cronduit-specific `@random` token.

All schedules are evaluated in `[server].timezone`.

#### `[[jobs]].timeout`

- **Optional.** Falls back to `[defaults].timeout`, then to unlimited if neither is set.
- **Type:** humantime duration string.
- **Example:** `timeout = "30s"`, `timeout = "2h"`

Per-job timeout. When the run exceeds this duration, Cronduit kills the process (command/script) or stops the container with SIGTERM grace (docker) and records `status=timeout`, `failure_reason=timeout`.

Setting `timeout = "0s"` is **not** a way to say "unlimited" — it means "time out immediately". To express "unlimited", omit the field and omit it from `[defaults]`.

#### `[[jobs]].use_defaults`

- **Optional.** Default: `true` (i.e., `[defaults]` applies if present).
- **Type:** boolean.
- **Example:** `use_defaults = false`

Set to `false` to opt this job out of the entire `[defaults]` section. No field will be merged in. Useful when a specific job needs to be fully self-contained for clarity or isolation.

### Command jobs

A command job runs a local shell command inside the Cronduit container. The command string is tokenized via the `shell-words` crate and executed directly with `tokio::process::Command`. **No shell is invoked** — `$VAR` expansion, pipes, redirects, globs, and backticks are not interpreted.

```toml
[[jobs]]
name = "http-healthcheck"
schedule = "*/5 * * * *"
command = "curl -sf https://example.com/health"
```

#### `[[jobs]].command`

- **Type:** string, tokenized with `shell-words` rules.
- **Behavior:** the first token is the executable; remaining tokens are argv.

If you need shell features (pipes, redirects, variable expansion, subshells), wrap the command in `sh -c "..."`:

```toml
[[jobs]]
name = "rotate-logs"
schedule = "0 2 * * *"
command = "sh -c 'find /var/log -name \"*.log\" -mtime +7 -delete'"
```

The shell wrapper trade-off: you get shell features, but you also accept all of `sh`'s quoting and escaping complexity. For simple arg lists, the default (no-shell) path is safer and more predictable.

### Script jobs

A script job writes an inline script to a tempfile with the script's shebang line, marks it executable, runs it, then unlinks. Useful when you need several lines of shell without writing a dedicated file.

```toml
[[jobs]]
name = "backup-index"
schedule = "0 * * * *"
script = """#!/bin/sh
set -eu
echo "building backup index at $(date -u +%FT%TZ)"
find /data -type f -mtime -1 | wc -l
"""
```

#### `[[jobs]].script`

- **Type:** multi-line string. The first line **must** be a shebang (`#!/bin/sh`, `#!/usr/bin/env python3`, etc.).
- **Behavior:** the string is written verbatim to a tempfile, `chmod +x`'d, executed, and unlinked when the run completes. The interpreter is picked by the shebang and must exist inside the Cronduit runtime image (alpine ships `sh`, `ash`, and a minimal set of tools; for Python or Ruby you'll need to pick a docker job with a language-specific image instead).

Per-job timeout applies to the script interpreter process. Killing the interpreter also kills any child processes it spawned via process groups.

### Docker container jobs

A docker job spawns an ephemeral container via `bollard` 0.20, streams its stdout/stderr into the log pipeline, waits for it to exit, extracts its image digest, and removes it (if `delete = true`).

```toml
[[jobs]]
name = "nightly-backup"
schedule = "15 3 * * *"
image = "restic/restic:0.17.0"
network = "container:vpn"
volumes = ["/data:/data:ro", "/backup:/backup"]
container_name = "nightly-backup"
cmd = ["backup", "/data", "--repo", "/backup/restic"]
timeout = "2h"
delete = true

[jobs.env]
RESTIC_PASSWORD = "${RESTIC_PASSWORD}"
```

#### `[[jobs]].image`

- **Type:** string, Docker image reference (`repo:tag` or `repo@sha256:...`).
- **Required for docker jobs** (either directly or via `[defaults].image`).
- **Example:** `image = "restic/restic:0.17.0"`

The image Cronduit spawns the container from. If the image is missing locally, Cronduit pulls it with 3-attempt exponential backoff. Image-pull failures are classified as terminal (`reason=image_pull_failed`) vs. transient and retried appropriately.

Pin tags for reproducibility. `:latest` is accepted but not recommended for production — prefer specific tags or digests.

#### `[[jobs]].network`

- **Type:** string. One of:
  - `bridge` — the default Docker bridge network
  - `host` — shares the host network namespace
  - `none` — no network
  - `container:<name>` — shares another container's network namespace **(the marquee feature)**
  - `<named-network>` — any Docker network you created with `docker network create`
- **Optional.** Falls back to `[defaults].network`, then to the Docker default (bridge) if neither is set.
- **Example:** `network = "container:vpn"`

The `container:<name>` mode is validated before spawn — if the target container is missing, Cronduit fails the run with `reason=network_target_unavailable` instead of letting bollard produce a confusing error. This is critical for VPN routing patterns, where a job that silently falls back to the default bridge when the VPN is down could leak egress to the wrong network.

#### `[[jobs]].volumes`

- **Type:** array of strings. Each string follows Docker's volume-spec syntax: `source:destination[:flags]`.
- **Optional.** Falls back to `[defaults].volumes`. Per-job value replaces the defaults entry — no concatenation.
- **Example:** `volumes = ["/mnt/data:/data", "/backup:/backup:ro"]`

Volume binds are passed verbatim to bollard. Both host-path binds (`/mnt/data:/data`) and named volumes (`mydata:/data`) work — but for named volumes, the volume must already exist in the Docker daemon (e.g., created by your compose file or a `docker volume create`).

#### `[[jobs]].container_name`

- **Type:** string.
- **Optional.** No default. **Not defaults-eligible** (container names must be unique).
- **Example:** `container_name = "nightly-backup"`

When set, Cronduit spawns the container with the given name. Useful when another container needs to reference this one by name (e.g., `network = "container:nightly-backup"` from a different job). Without `container_name`, Docker auto-generates a random name.

Setting `container_name` on multiple jobs that might run concurrently will cause a conflict — the second container fails to spawn. Pick a unique name or omit the field.

#### `[[jobs]].cmd`

- **Type:** array of strings.
- **Optional.** **Per-job only — NOT merged from `[defaults]`.**
- **Example:** `cmd = ["backup", "/data", "--repo", "/backup/restic"]`

Overrides the Docker image's baked-in `CMD`. When set, the vec is passed verbatim to the container at start time via bollard's `ContainerCreateBody.cmd`. When unset, the container runs with whatever `CMD` the image defines.

**Important:** some images have no default `CMD` at all (for example, `alpine:latest` has no `CMD`, only an entrypoint that defaults to `/bin/sh`). A docker job using such an image with no `cmd` will start the entrypoint, immediately exit with exit code 0, and produce no output. If that's not what you want, always set `cmd`.

The semantics match `docker run IMAGE CMD...` on the command line: the `cmd` array is the argv for the image's entrypoint, not a shell command. For shell features (pipes, redirects, glob expansion), wrap in `sh -c`:

```toml
cmd = ["sh", "-c", "pg_dump mydb | gzip > /backup/db.sql.gz"]
```

**Why `cmd` is not in `[defaults]`:** CLI args are almost always job-specific. Every docker job that uses `cmd` has its own arg list; defaulting it would encourage operators to copy-paste the same arg vector into every job's block, which is error-prone and hides per-job intent. If you genuinely need the same `cmd` on multiple jobs, make them share a base image with the `cmd` baked in via `CMD` in the Dockerfile, or use a short shell helper script.

#### `[[jobs]].delete`

- **Type:** boolean.
- **Optional.** Falls back to `[defaults].delete`, then to implicit removal (equivalent to `true`).
- **Example:** `delete = true`, `delete = false`

When `true` (or unset), Cronduit explicitly removes the container after the run completes and logs are drained. When `false`, the container is preserved so an operator can `docker logs <id>` or `docker inspect <id>` for post-mortem debugging — at the cost of owning the cleanup themselves. See [`[defaults].delete`](#defaultsdelete) for the full semantics, including the operator-responsibility note on preserved containers accumulating.

#### `[jobs.env]` — environment variables

- **Type:** inline table of string → string.
- **Optional.**
- **Example:**

```toml
[[jobs]]
name = "nightly-backup"
schedule = "15 3 * * *"
image = "restic/restic:0.17.0"
cmd = ["backup", "/data"]

[jobs.env]
RESTIC_REPOSITORY = "/backup/restic"
RESTIC_PASSWORD = "${RESTIC_PASSWORD}"
AWS_ACCESS_KEY_ID = "${AWS_ACCESS_KEY_ID}"
AWS_SECRET_ACCESS_KEY = "${AWS_SECRET_ACCESS_KEY}"
```

Environment variables passed to the docker container at start time. Values support env var interpolation via `${VAR}` — Cronduit resolves the reference at parse time by reading the host process's environment, then wraps the resolved value in `secrecy::SecretString` so it never appears in `Debug` output, log lines, metric labels, or the `config_json` stored in the database.

**env is NOT merged from `[defaults]`.** Each job's env block is its own. The `env` field is also never hashed into `config_hash` — the column stores `env_keys` (just the key names) instead, so changes to secret values do not trigger phantom "config changed" reloads, and the values never appear in any audit log.

`[jobs.env]` is currently only read for docker jobs. Command and script jobs inherit the Cronduit process's environment (with `PATH`, `HOME`, and a few other basics). A future release may add per-job env for command/script too — track issues on the repo if you need this.

## Cron syntax

Cronduit uses `croner` 3.0 for cron parsing. Supported syntax:

### 5-field standard cron

```
┌───────────── minute (0-59)
│ ┌───────────── hour (0-23)
│ │ ┌───────────── day-of-month (1-31)
│ │ │ ┌───────────── month (1-12 or JAN-DEC)
│ │ │ │ ┌───────────── day-of-week (0-6 or SUN-SAT, 0 and 7 both mean Sunday)
│ │ │ │ │
* * * * *
```

Examples:

| Expression | Meaning |
|---|---|
| `*/5 * * * *` | Every 5 minutes |
| `0 * * * *` | Every hour at :00 |
| `30 2 * * *` | 2:30 AM every day |
| `15 3 * * 0` | 3:15 AM every Sunday |
| `0 */6 * * *` | Every 6 hours |
| `0,30 9-17 * * 1-5` | On the hour and half-hour, 9 AM-5 PM, Monday-Friday |

### 6-field with seconds

Add a seconds column at the front:

```
*/30 * * * * *
```

Runs every 30 seconds. The scheduler resolution is 1 second, so sub-second schedules are not supported.

### Named macros

- `@hourly` = `0 * * * *`
- `@daily` = `0 0 * * *`
- `@midnight` = `0 0 * * *`
- `@weekly` = `0 0 * * 0`
- `@monthly` = `0 0 1 * *`
- `@yearly` = `0 0 1 1 *`
- `@annually` = `0 0 1 1 *`

`@reboot` is **not** supported. Cronduit has explicit startup hooks (via the `@random` resolver and the config sync pass on startup); if you need "run at boot", use those instead.

### Quartz extended modifiers

`croner` supports three Quartz extensions that aren't in POSIX cron:

- **`L`** in the day-of-month field means "last day of month". `0 3 L * *` is "3 AM on the last day of every month".
- **`L`** in the day-of-week field (prefixed by a weekday number) means "last <weekday> of month". `0 3 ? * 7L` is "3 AM on the last Saturday of every month" (Quartz uses `?` for "no specific value" and numbers `1-7` with `1=Sunday`; croner accepts both POSIX and Quartz syntax).
- **`#`** in the day-of-week field means "the Nth <weekday> of month". `0 3 ? * 2#1` is "3 AM on the first Monday of every month".
- **`W`** in the day-of-month field means "nearest weekday". `0 3 15W * *` is "3 AM on the weekday closest to the 15th of every month".

These are the killer features for real-world backup and maintenance schedules — "last weekday of month", "first Monday", etc. — that POSIX cron cannot express.

### `@random` — Cronduit extension

Any field in a 5-field cron expression can be set to `@random`, which Cronduit resolves at startup using a slot-based algorithm. The resolver honors `[defaults].random_min_gap` between consecutive jobs on the same day.

```toml
[[jobs]]
name = "snapshot"
schedule = "@random * * * *"
command = "snapshot-cli --target /data"
```

Resolved values are persisted in the DB's `resolved_schedule` column and re-rolled on the next daily boundary. Changing `random_min_gap` does **not** re-roll an existing day's assignments — it only affects the next day.

Use `@random` when you have many jobs that should spread out over a day without you having to pick distinct minutes by hand.

## Environment variable interpolation

Cronduit interpolates `${VAR}` references from the host process's environment at parse time.

```toml
[jobs.env]
DB_PASSWORD = "${POSTGRES_PASSWORD}"
API_KEY = "${STRIPE_SECRET_KEY}"
```

Rules:

- **Only `${VAR}` syntax is supported.** `$VAR` and `${VAR:-default}` are not. If you need a default, set the variable unconditionally in your compose file's `environment:` block.
- **Missing variables fail loudly.** If `${STRIPE_SECRET_KEY}` is not set in the environment, `cronduit check` fails with a line-precise error and Cronduit refuses to start. Silent empty-string substitution is never what you want for credentials.
- **Every interpolated value is wrapped in `SecretString`.** The `secrecy` crate suppresses the value from `Debug` output, `Display`, and standard logger formatters, so you will not see credentials in log lines, metric labels, or the DB's `config_json`.
- **Interpolation happens once at parse time**, not on every run. Changing the host env without reloading the config does not update the interpolated values. Use `POST /api/reload` or restart Cronduit after rotating a secret.
- **Only string fields are interpolated.** `timeout = "${TIMEOUT}"` works; `port = ${PORT}` (without quotes) does not, because `${...}` must be inside a TOML string.

Field-by-field behavior:

- `[server].database_url` — interpolated, wrapped in SecretString.
- `[jobs.env]` values — interpolated, wrapped in SecretString.
- `[[jobs]].command`, `[[jobs]].script`, `[[jobs]].cmd` — NOT interpolated. These fields are **not** run through the env resolver because it is easy to accidentally shell-inject: `command = "curl ${URL}"` would substitute user-controlled text into a shell command at parse time. Instead, pass the value as an env var to the job and reference it inside the command string (where it is obvious to the reader that the value is runtime, not parse-time).

If you need to make a command's behavior depend on the environment, use shell expansion:

```toml
command = "sh -c 'curl -sf \"$HEALTHCHECK_URL\"'"

[jobs.env]
HEALTHCHECK_URL = "${HEALTHCHECK_URL}"
```

Here the outer `${HEALTHCHECK_URL}` is interpolated by Cronduit at parse time into `[jobs.env]`, then the inner `$HEALTHCHECK_URL` is expanded by `sh -c` at runtime when the command runs. Two-step interpolation keeps parse-time config separate from runtime shell expansion.

## Validation

Run `cronduit check <path-to-config>` to validate a config file without starting the scheduler:

```bash
docker compose exec cronduit /cronduit check /etc/cronduit/config.toml
```

Or outside the container:

```bash
docker run --rm \
  -v $(pwd)/cronduit.toml:/etc/cronduit/config.toml:ro \
  -e POSTGRES_PASSWORD=... \
  ghcr.io/simplicityguy/cronduit:latest check /etc/cronduit/config.toml
```

Exit code 0 means the config is valid. Exit code 1 means at least one validation error.

Validation covers:

- **TOML syntax.** Parser errors include line number and column.
- **Required fields.** `[server].timezone`, `[[jobs]].name`, `[[jobs]].schedule`, and exactly one of `command`/`script`/`image` per job.
- **Job name uniqueness.** Duplicate names across `[[jobs]]` blocks.
- **Timezone validity.** IANA zone names only.
- **Bind address.** Must parse as a `SocketAddr`.
- **Cron syntax.** Every `schedule` is parsed through `croner` — invalid expressions fail with croner's error message plus the job name for context.
- **Network mode syntax.** `bridge`/`host`/`none`/`container:<name>`/`<named>`; whitespace and empty strings are rejected.
- **Env var resolution.** Every `${VAR}` in an interpolated string must resolve.
- **Default merging correctness.** `cronduit check` runs `apply_defaults` before validation, so error messages reflect the merged view — if a docker job relies on `[defaults].image`, validation sees `image` as set even though the job block omits it.

Validation is **not fail-fast** — Cronduit collects every error it can find and prints all of them in one pass so you can fix multiple mistakes without running `check` repeatedly. This is decision `D-21` in the project's decision log.

## Hot reload

Three reload paths, all converging on the same `do_reload` codepath:

### 1. File watcher (automatic)

When `[server].watch_config = true` (the default), Cronduit uses `notify` 8.2 to watch the config file for changes. On a save, it waits ~1 second (debounce), then re-reads, re-validates, re-interpolates env vars, re-merges `[defaults]`, and syncs the DB.

The watcher detects both atomic saves (editor writes to `.tmp` then renames) and in-place edits. It gracefully handles the file being temporarily absent (some editors delete-then-create) by retrying.

### 2. HTTP endpoint

```bash
curl -X POST http://localhost:8080/api/reload
```

Returns `{"status":"reloaded","jobs_synced":N}` on success, or `{"status":"error","errors":[...]}` with the same structured error list `cronduit check` produces on validation failure. The Settings page in the web UI has a "Reload now" button that hits this endpoint.

### 3. SIGHUP

```bash
docker compose kill -s SIGHUP cronduit
```

Sends a SIGHUP to the cronduit process. Cronduit catches it and triggers the same reload path.

### Semantics (all three paths)

- **The config file is the source of truth.** Any job present in the DB but NOT in the reloaded config is set to `enabled = 0`. It will not fire again until you add it back.
- **Validation failure is atomic.** If the new config fails to parse or validate, the in-memory state and DB are untouched and Cronduit keeps running with the previous config. Reload errors are surfaced in logs (and in the HTTP response for the `POST /api/reload` path).
- **In-flight jobs keep running.** A reload does not cancel in-flight jobs. New runs start with the new config once they fire.
- **Scheduler timezone is fixed across reloads.** Changing `[server].timezone` and reloading logs a warning and keeps the original timezone in effect — changing timezones at runtime would re-fire every job's "next run" calculation and potentially skip or double-schedule boundary jobs. Restart Cronduit to change the timezone.
- **Database URL is fixed across reloads.** Same reason — changing the DB URL at runtime would break in-flight connections. Restart to change the DB backend.

## Patterns and recipes

### Minimal healthcheck job

```toml
[[jobs]]
name = "healthcheck"
schedule = "*/5 * * * *"
command = "curl -sf https://example.com/health"
timeout = "30s"
```

### Backup with compression and rotation

```toml
[[jobs]]
name = "nightly-db-backup"
schedule = "15 3 * * *"
command = "sh -c 'pg_dump -h db.internal mydb | gzip > /data/backup-$(date -u +%Y%m%d).sql.gz && find /data -name \"backup-*.sql.gz\" -mtime +14 -delete'"
timeout = "1h"
```

### Docker job routed through a VPN sidecar

```toml
[defaults]
image = "alpine:3.20"
network = "container:vpn"         # every docker job goes through the VPN by default
delete = true

[[jobs]]
name = "vpn-check"
schedule = "*/15 * * * *"
cmd = ["sh", "-c", "wget -qO- https://ifconfig.me && echo"]

[[jobs]]
name = "restic-backup"
schedule = "0 2 * * *"
image = "restic/restic:0.17.0"    # overrides the defaults image
volumes = ["/data:/data:ro", "/backup:/backup"]
cmd = ["backup", "/data", "--repo", "/backup/restic"]
timeout = "2h"

[jobs.env]
RESTIC_PASSWORD = "${RESTIC_PASSWORD}"
```

Both jobs inherit `network = "container:vpn"` from `[defaults]`. If the VPN sidecar is down, both jobs fail fast with `reason=network_target_unavailable` instead of silently falling back to the bridge network.

### Staggered `@random` jobs

```toml
[defaults]
image = "alpine:3.20"
random_min_gap = "30m"

[[jobs]]
name = "snapshot-1"
schedule = "@random * * * *"
cmd = ["sh", "-c", "echo snap1"]

[[jobs]]
name = "snapshot-2"
schedule = "@random * * * *"
cmd = ["sh", "-c", "echo snap2"]

[[jobs]]
name = "snapshot-3"
schedule = "@random * * * *"
cmd = ["sh", "-c", "echo snap3"]
```

Each day, the resolver picks three distinct minute values at least 30 minutes apart and persists them to the DB. Restarting Cronduit does not re-roll the current day's values — they persist across restarts and only re-roll on the next daily boundary.

### Last weekday of month backup (Quartz extension)

```toml
[[jobs]]
name = "month-end-close"
schedule = "0 23 LW * *"
command = "sh -c 'run-month-end-close.sh'"
timeout = "2h"
```

`LW` in the day-of-month field means "last weekday of month" — on a month that ends on Sunday, the job fires on the preceding Friday. POSIX cron cannot express this without a three-job workaround.

### A job that only runs on business days

```toml
[[jobs]]
name = "business-hours-check"
schedule = "0 9-17 * * 1-5"       # every hour, 9 AM - 5 PM, Monday through Friday
command = "curl -sf https://example.com/api/status"
```

### Opting one job out of `[defaults]`

```toml
[defaults]
image = "alpine:3.20"
network = "container:vpn"
timeout = "5m"

[[jobs]]
name = "quick-script"
schedule = "*/5 * * * *"
script = """#!/bin/sh
echo 'this runs inside Cronduit, not a container'
"""
use_defaults = false             # do NOT inherit the timeout — use the Cronduit default (unlimited)
```

Script jobs already ignore `image`/`network`/`delete`/`volumes`, so opting out is only useful when you want to specifically ignore `[defaults].timeout`. A rare but valid use case.

---

For questions this document does not answer, read [`SPEC.md`](./SPEC.md) (architecture), [`../THREAT_MODEL.md`](../THREAT_MODEL.md) (security posture), or the working example at [`../examples/cronduit.toml`](../examples/cronduit.toml).
