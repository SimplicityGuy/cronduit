# Cronduit Quickstart

This guide walks you from `git clone` to a running scheduled job in under ten minutes. It assumes you already have a host that runs Docker and a terminal — if you need a deeper reference for individual fields, jump to [`CONFIG.md`](./CONFIG.md). If you want the architectural picture, read [`SPEC.md`](./SPEC.md). If you need to understand the trust model before deploying, read [`../THREAT_MODEL.md`](../THREAT_MODEL.md).

> **Read [`../THREAT_MODEL.md`](../THREAT_MODEL.md) first if this machine is reachable from anywhere other than localhost.** Cronduit mounts the Docker socket and ships unauthenticated in v1 — those are deliberate trade-offs for homelab ergonomics, not oversights. Every deployment beyond loopback belongs behind a reverse proxy with authentication.

## Table of contents

1. [Prerequisites](#prerequisites)
2. [Install Cronduit](#install-cronduit)
3. [Pick a compose variant](#pick-a-compose-variant)
4. [Start Cronduit](#start-cronduit)
5. [Open the web UI](#open-the-web-ui)
6. [Walk through the four example jobs](#walk-through-the-four-example-jobs)
7. [Trigger a run manually](#trigger-a-run-manually)
8. [Add your own job](#add-your-own-job)
9. [Reload the config](#reload-the-config)
10. [Validate before deploying](#validate-before-deploying)
11. [Common pitfalls](#common-pitfalls)
12. [Next steps](#next-steps)

## Prerequisites

You need:

- **Docker** — any recent release (20.10+). Cronduit ships as a multi-arch image for `linux/amd64` and `linux/arm64`.
- **Docker Compose** — the v2 CLI plugin (`docker compose`, not the legacy `docker-compose` binary).
- **Access to `/var/run/docker.sock`** on the host, or the willingness to run the [secure compose file](../examples/docker-compose.secure.yml) which proxies the socket through a narrow allowlist instead.
- **A terminal** and `git`.

Cronduit does **not** require a database server, a language runtime, a package manager, or any Node/Python tooling. The default deployment uses embedded SQLite with WAL journaling. If you prefer PostgreSQL, see [CONFIG.md § database_url](./CONFIG.md#serverdatabase_url).

## Install Cronduit

```bash
git clone https://github.com/SimplicityGuy/cronduit
cd cronduit
```

The repo ships two example compose files and a working `cronduit.toml` in `examples/`. You do not need to write any configuration to see Cronduit run — the defaults are enough to reach a working dashboard in under two minutes.

## Pick a compose variant

Cronduit ships two example compose files. Read this section before starting — the right variant depends on which host you're on.

| Compose file | When to use it | Docker socket access |
|---|---|---|
| [`examples/docker-compose.yml`](../examples/docker-compose.yml) | Linux with a native Docker daemon, or Rancher Desktop on macOS. | Direct mount of `/var/run/docker.sock`. The cronduit process joins the host's `docker` group via `group_add`, so you must set `DOCKER_GID` to match your host. |
| [`examples/docker-compose.secure.yml`](../examples/docker-compose.secure.yml) | macOS with Docker Desktop, or any deployment where you want defense-in-depth. | A `docker-socket-proxy` sidecar runs as root and mediates API access through a narrow allowlist. Cronduit itself never touches the socket directly. No `DOCKER_GID` alignment needed. |

If you are unsure, start with `docker-compose.secure.yml`. The socket-proxy adds ~5 MB of memory overhead and removes an entire class of escalation risk.

### Deriving `DOCKER_GID` (only needed for `docker-compose.yml`)

Cronduit runs as UID 1000 inside the container. To let the non-root cronduit user read `/var/run/docker.sock`, the default compose file joins the cronduit process to the host's docker group. The `DOCKER_GID` variable must match your host's docker group GID.

**Linux:**

```bash
export DOCKER_GID=$(stat -c %g /var/run/docker.sock)
```

**Rancher Desktop on macOS:**

```bash
export DOCKER_GID=102
```

The Rancher Desktop lima VM's docker socket is owned by `root:102` regardless of host user — 102 is not a typo.

**Docker Desktop on macOS:** the socket GID inside the Docker Desktop VM is not stable across releases. Use `docker-compose.secure.yml` instead — it does not require GID alignment.

You can also set `DOCKER_GID` in a `.env` file next to the compose file:

```bash
# .env
DOCKER_GID=998
```

## Start Cronduit

With the compose variant picked, start Cronduit in the background:

```bash
# Default path — Linux or Rancher Desktop
docker compose -f examples/docker-compose.yml up -d

# Secure path — Docker Desktop, or you want the socket proxy regardless
docker compose -f examples/docker-compose.secure.yml up -d
```

Expected output ends with two or three services reporting `Started`. Check that Cronduit is healthy:

```bash
docker compose -f examples/docker-compose.yml ps
```

The `cronduit` service should show `Up (healthy)` after ~10 seconds. The healthcheck calls `GET /health`, which returns `ok` once the scheduler has synced the config and the database pool is alive.

If the healthcheck is `unhealthy`, jump to [Common pitfalls](#common-pitfalls) — nine times out of ten it's either a `DOCKER_GID` mismatch or a stale volume from a previous run.

## Open the web UI

```bash
open http://localhost:8080
```

(`open` is macOS; use `xdg-open` on Linux or paste the URL into your browser.)

You should see the Cronduit dashboard in its terminal-green design. The dashboard shows a list of jobs with columns for name, type, schedule (raw + human-readable), next run time, last run status, and a "Run Now" button.

If the dashboard loads but is unstyled (plain HTML, no colors), your Cronduit image was built without running `just tailwind` during the build step. Rebuild from source or pull a fresh image from GHCR.

## Walk through the four example jobs

The shipped `examples/cronduit.toml` contains four jobs. Each one demonstrates a different execution path so you can see every feature Cronduit supports without reading the spec first.

### 1. `echo-timestamp` — command job, runs every minute

```toml
[[jobs]]
name = "echo-timestamp"
schedule = "*/1 * * * *"
command = "date '+%Y-%m-%d %H:%M:%S -- Cronduit is running!'"
```

- **Type:** command. The string is tokenized via `shell-words` and executed with `tokio::process::Command`. **No shell is invoked**, so `$VAR` expansion and shell metacharacters are not interpreted.
- **Why it exists:** this is the instant heartbeat. Within ~60 seconds of starting Cronduit you should see it flip from "pending" to a green "success" badge — the fastest possible signal that your install works.
- **Watch:** click the job name to open its detail page, then click a run to see its stdout/stderr stream.

### 2. `http-healthcheck` — command job, realistic uptime canary

```toml
[[jobs]]
name = "http-healthcheck"
schedule = "*/5 * * * *"
command = "sh -c \"wget -q -S --spider https://www.google.com 2>&1 | head -10\""
```

- **Type:** command. Uses `sh -c` explicitly because the job needs `2>&1` redirection and a pipe, which are shell features — the default command path does not invoke a shell. If you want shell features, you must wrap the command in `sh -c "..."`.
- **What it tests:** DNS + egress + TLS from inside the alpine runtime image. The `wget --spider` variant does a HEAD request and prints response headers without downloading the body.
- **Adapt for yourself:** swap the URL for your own service's health endpoint.

### 3. `disk-usage` — script job

```toml
[[jobs]]
name = "disk-usage"
schedule = "*/15 * * * *"
script = """#!/bin/sh
du -sh /data 2>/dev/null || echo "/data not mounted"
df -h /data 2>/dev/null || true
"""
```

- **Type:** script. The inline string is written to a tempfile with its shebang and executable mode, executed, then unlinked. Useful when you need a few lines of shell without writing a dedicated file.
- **Demonstrates:** the `/data` named volume and the `script` path. If you run the default compose file, `/data` is a named volume mounted inside the Cronduit container — a common pattern for backup targets and shared state.

### 4. `hello-world` — docker job demonstrating `[defaults]` merge + `cmd` override

```toml
[[jobs]]
name = "hello-world"
schedule = "*/5 * * * *"
cmd = ["echo", "Hello from cronduit defaults!"]
```

This is the most feature-dense example — three distinct concepts live in this one block.

**What's missing from the block, and why:**

- `image` — inherited from `[defaults].image = "alpine:latest"`.
- `network` — inherited from `[defaults].network = "bridge"`.
- `delete` — inherited from `[defaults].delete = true`.
- `timeout` — inherited from `[defaults].timeout = "5m"`.

The `[defaults]` section (lines 29-34 of `examples/cronduit.toml`) provides shared defaults that every job inherits unless it overrides them per-job or opts out with `use_defaults = false`. Moving shared fields into `[defaults]` is the right pattern once you have more than a handful of jobs — it keeps per-job blocks minimal and one-line spec edits to `[defaults]` propagate everywhere.

**Why `cmd` is on the job and NOT in `[defaults]`:** `cmd` is a per-job-only field (see [CONFIG.md § cmd](./CONFIG.md#jobscmd)). It overrides the Docker image's baked-in `CMD`. Without `cmd`, `alpine:latest` has no default entrypoint and would exit immediately with no output — so the `cmd = ["echo", "..."]` line is load-bearing here. This is exactly what `docker run alpine echo "..."` does from the command line.

**Docker socket requirement:** this job requires the compose file to have mounted the Docker socket (either directly in `docker-compose.yml` or through the proxy sidecar in `docker-compose.secure.yml`). Every 5 minutes it pulls `alpine:latest`, runs `echo`, captures the output into the run logs, and removes the container.

## Trigger a run manually

Every job has a "Run Now" button on its detail page. Clicking it sends a `RunNow` command into the scheduler's main `select!` loop, queueing the job for immediate execution without waiting for its next scheduled fire.

Try it on `echo-timestamp` — you should see a new run appear in the run history within a second or two, then transition through `running` → `success`. Click the run to see the stdout.

The same page has a log viewer that subscribes to a server-sent events (SSE) stream for in-progress runs. When a run completes, the SSE connection closes and the page becomes a static log view.

## Add your own job

Edit `examples/cronduit.toml` (or better: copy it to a new path and mount that instead). Add a block at the bottom:

```toml
[[jobs]]
name = "my-first-job"
schedule = "*/2 * * * *"
command = "echo hello from my own job"
```

Save the file. By default Cronduit watches the config file and reloads within a few seconds of a change (see [`CONFIG.md § watch_config`](./CONFIG.md#serverwatch_config)). You should see `my-first-job` appear in the dashboard without restarting the container.

### A more realistic command job

```toml
[[jobs]]
name = "nightly-db-backup"
schedule = "15 3 * * *"
command = "sh -c 'pg_dump -h db.internal mydb | gzip > /data/backup-$(date -u +%Y%m%d).sql.gz'"
timeout = "30m"
```

Note the `sh -c` wrapper — without it, `$()` subshell interpolation and the pipe would not work.

### A docker job with `container:<name>` routing

Cronduit's marquee Docker feature is `network = "container:<name>"`, which routes all traffic for a job through another container's network namespace. Pair this with a VPN sidecar to run scheduled backups behind a VPN, or with a proxy sidecar to sanitize egress.

```toml
[[jobs]]
name = "restic-via-vpn"
schedule = "0 3 * * *"
image = "restic/restic:0.17.0"
network = "container:vpn"        # join the "vpn" container's network namespace
volumes = ["/data:/data:ro", "/backup:/backup"]
cmd = ["backup", "/data", "--repo", "/backup/restic"]
timeout = "2h"

[jobs.env]
RESTIC_PASSWORD = "${RESTIC_PASSWORD}"    # interpolated from the host env at parse time
```

The VPN sidecar must already be running when Cronduit spawns the job. Cronduit runs a network preflight check (see [`SPEC.md § Job Execution`](./SPEC.md#docker-container-jobs)) and fails with `reason=network_target_unavailable` if the target container is missing at spawn time.

## Reload the config

Three ways to reload the config, all converging on the same `do_reload` path inside Cronduit:

1. **File watcher** (default). If `[server].watch_config = true`, Cronduit watches the config file for changes and reloads automatically within a debounce window (~1 second).
2. **HTTP endpoint.** `curl -X POST http://localhost:8080/api/reload`. The dashboard Settings page has a "Reload now" button that hits this endpoint.
3. **SIGHUP.** `docker compose kill -s SIGHUP cronduit` sends a SIGHUP to the cronduit process, which triggers a reload.

All three paths produce identical semantics: the config file is re-parsed, re-validated, env vars are re-interpolated, `[defaults]` are re-merged, the DB is reconciled (jobs added, updated, or disabled), and the scheduler's in-memory state flips atomically.

**Key invariant:** the config file is the source of truth. Any job present in the DB but NOT in the reloaded config is set to `enabled = 0` — it will not fire again until you add it back. This is deliberate: an operator should always be able to look at the config file and know exactly which jobs are scheduled.

## Validate before deploying

Cronduit ships a `check` subcommand that parses a config file and runs every validation check without touching the database or starting the scheduler. Use it in CI and before every reload:

```bash
docker compose exec cronduit /cronduit check /etc/cronduit/config.toml
```

Or, from outside the container:

```bash
docker run --rm -v $(pwd)/examples/cronduit.toml:/etc/cronduit/config.toml:ro \
  ghcr.io/simplicityguy/cronduit:latest check /etc/cronduit/config.toml
```

Exit code 0 means the config is valid. Exit code 1 means at least one validation error — the errors print to stderr with file + line + column when available.

Validation covers TOML syntax, env var resolution, cron syntax (including `@random` and Quartz extended modifiers), timezone (`IANA` zone names only), bind address, job name uniqueness, network mode syntax, and the "exactly one of `command`/`script`/`image`" rule per job.

## Common pitfalls

### `cronduit` healthcheck is unhealthy, logs show `permission denied` on `/var/run/docker.sock`

Your `DOCKER_GID` does not match the host's docker group. Run `stat -c %g /var/run/docker.sock` on Linux or `echo 102` on Rancher Desktop macOS, export the value, and `docker compose up -d` again. Or switch to `docker-compose.secure.yml` which does not require GID alignment.

### Jobs have a red "failed" badge with `reason=image_pull_failed`

Cronduit's alpine runtime needs egress to a Docker registry to pull images for Docker jobs. Check the container can reach the registry — for GHCR images, `nslookup ghcr.io` from inside the container should resolve. If egress is firewalled, either preload the images with `docker pull` on the host (the Cronduit container shares the host's image cache when using the default compose) or mirror the registry internally.

### Cron schedule fires at the wrong hour

Check `[server].timezone`. Cronduit does **not** default to the host timezone — you must set `timezone = "America/Los_Angeles"` or `timezone = "UTC"` explicitly. All cron expressions are evaluated in that timezone, with DST handled correctly by `croner` (DST "spring forward" and "fall back" edge cases are both covered).

### Startup log shows `bind_warning: true`

You set `[server].bind` to a non-loopback address (e.g., `0.0.0.0:8080`). Cronduit emits a loud `WARN` at startup to make sure you did it on purpose. Put Cronduit behind a reverse proxy with authentication if the host is reachable from anywhere other than the local machine — see the Security section in the README and the full threat model in `THREAT_MODEL.md`.

### A new job I added isn't showing up after saving the config

Three possibilities:

1. The file watcher is disabled (`[server].watch_config = false`). Use `POST /api/reload` or SIGHUP.
2. The config has a syntax error. Run `cronduit check` on it — validation fail stops the reload from being applied.
3. You edited a copy of `cronduit.toml` that isn't the one mounted into the container. Verify with `docker compose exec cronduit cat /etc/cronduit/config.toml`.

### `hello-world` job passes validation but a run exits with no output

You edited `[defaults].image` or the `hello-world` block and ended up with a container that has no `CMD`. The alpine runtime image has no default `CMD` — without `cmd = [...]` on the job, or an image with a baked-in entrypoint, the container starts and immediately exits with exit code 0 and empty stdout. Set `cmd = ["your", "command"]` or pick a different image. See [CONFIG.md § cmd](./CONFIG.md#jobscmd) for details.

### I set a field in `[defaults]` but jobs don't inherit it

Check that the field is one of the defaults-eligible fields: `image`, `network`, `volumes`, `delete`, `timeout`, `random_min_gap`. Other fields (`cmd`, `container_name`, `command`, `script`, `env`) are **per-job only** and will not merge from `[defaults]`. See [CONFIG.md § defaults](./CONFIG.md#defaults) for the complete list.

Also check that the job does not have `use_defaults = false` — that opts the job out of the entire `[defaults]` section.

## Next steps

- **[CONFIG.md](./CONFIG.md)** — reference for every field in `cronduit.toml`: what it means, what values are valid, what happens when you leave it out, and which fields are mergeable via `[defaults]`.
- **[SPEC.md](./SPEC.md)** — architectural spec: persistence, observability, reload semantics, Docker lifecycle, failure reasons, and the full v1.0 feature list.
- **[../THREAT_MODEL.md](../THREAT_MODEL.md)** — threat model. Read before exposing Cronduit beyond loopback.
- **[../examples/cronduit.toml](../examples/cronduit.toml)** — the working example this guide walked through. Copy it, edit it, mount it.
- **[../examples/docker-compose.yml](../examples/docker-compose.yml)** and **[../examples/docker-compose.secure.yml](../examples/docker-compose.secure.yml)** — the two compose variants with security trade-offs documented in the file headers.
