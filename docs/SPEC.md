# cronduit: cron, modernized for the container era

## Overview

Build a self-hosted cron job scheduler with a web UI, designed for Docker-native homelab environments. The core problem it solves: existing schedulers (ofelia, Cronicle, xyOps, docker-crontab) either lack proper Docker networking support (`--network container:X`), require complex bootstrapping, don't support config-file-driven job definitions, or don't have a web UI.

## Requirements

### Core Scheduler

- **Rust-based** backend for performance and reliability
- **Cron scheduling** with standard 5-field cron expressions
- **Random field support**: any cron field can be set to `@random` (randomized on startup, persisted until next restart or re-randomize)
- **Job spacing**: when using random schedules, guarantee minimum configurable gaps between jobs on the same day (e.g., 90 minutes apart)
- **Idempotent startup**: first run creates all required database tables; subsequent runs are safe no-ops

### Job Types

Jobs can execute in three modes:

1. **Command**: run a local shell command/script
2. **Script**: run an inline script (shell, python, etc.)
3. **Docker container**: spawn a container via the Docker API (not CLI)
   - Support ALL Docker network modes: `bridge`, `host`, `none`, `container:<name>`, named networks
   - Support container naming (custom `container_name` per job)
   - Support volume mounts
   - Support environment variables
   - Auto-remove containers after completion (`--rm`)
   - Pull image if not present

### Configuration

Jobs are defined in a **config file** (support TOML, YAML, JSON, or INI — pick one as primary, consider supporting multiple). The config file is the source of truth; the scheduler syncs state from config on startup.

**Common/default settings** that apply to all jobs unless overridden:

```toml
[defaults]
image = "curlimages/curl:latest"
network = "container:vpn"
volumes = ["/mnt/data:/data"]
delete = true
timeout = "1h"
```

**Per-job definition** (only requires name, schedule, and the job-specific field):

```toml
[[jobs]]
name = "check-ip"
schedule = "@random"
command = "https://ipinfo.io"

[[jobs]]
name = "custom-job"
schedule = "0 2 * * 1"
image = "my-custom-image:latest"
network = "bridge"
command = "my-command --flag"
volumes = ["/data:/data", "/config:/config:ro"]
container_name = "weekly-custom-job"
env = { API_KEY = "xxx", DEBUG = "true" }
```

**Sync behavior on startup:**
- Create jobs that are in config but not in DB
- Update jobs whose config has changed (schedule, image, command, etc.)
- Disable/remove jobs that are in DB but no longer in config
- Preserve job history and run logs for removed jobs

### Database

- Support **SQLite** (default, zero-config) and **PostgreSQL** (for shared infrastructure)
- Auto-create tables on first run (migrations built-in)
- Schema stores: job definitions, run history, run logs

**Tables (conceptual):**
- `jobs` — name, schedule, resolved_schedule, config hash, enabled, created_at, updated_at
- `job_runs` — job_id, status (running/success/failed/timeout), start_time, end_time, exit_code, duration_ms
- `job_logs` — run_id, stream (stdout/stderr), timestamp, line

### Web UI

- **Tailwind CSS** for styling
- Served by the Rust backend (embedded static assets)
- No JavaScript framework required — server-rendered HTML with HTMX or minimal JS for live updates

**Pages/views:**
- **Dashboard**: overview of all jobs, next run time, last run status (success/fail badge), last run time
- **Job detail**: job config, full run history table (start, end, duration, exit code, status)
- **Run detail**: stdout/stderr logs, metadata (image, container ID, network, exit code, duration)
- **Settings/status**: scheduler uptime, DB connection status, config file path, next reload time

**Features:**
- Manual trigger ("Run Now") button per job
- Live/auto-refresh for running jobs
- Filter/search jobs by name
- Sort by name, last run, next run, status

### Operational

- **Graceful shutdown**: wait for running jobs to complete (with configurable timeout)
- **Config reload**: support SIGHUP or API endpoint to reload config without restart
- **Logging**: structured JSON logs to stdout (for Docker log collection)
- **Health endpoint**: `GET /health` returning scheduler status
- **Metrics endpoint**: `GET /metrics` with Prometheus-compatible metrics (jobs_total, runs_total, run_duration_seconds, failures_total)

### Docker Deployment

The scheduler itself runs as a Docker container:

```yaml
cronduit:
  image: cronduit:latest
  container_name: cronduit
  volumes:
    - /var/run/docker.sock:/var/run/docker.sock
    - ./cronduit.toml:/etc/cronduit/config.toml:ro
    - cronduit_data:/data  # SQLite DB location
  environment:
    - DATABASE_URL=sqlite:///data/cronduit.db
    # Or for PostgreSQL:
    # - DATABASE_URL=postgres://user:pass@postgres:5432/cronduit
  expose:
    - 8080
  healthcheck:
    test: ["CMD", "curl", "-sf", "http://localhost:8080/health"]
    interval: 30s
    timeout: 10s
    retries: 3
```

### Security Considerations

- Docker socket access is required (read-write for container management)
- Web UI should support optional basic auth or token auth
- No secrets in config file — support environment variable references (`${ENV_VAR}`)
- Config file mounted read-only

## Non-Goals (v1)

- Multi-node / distributed scheduling
- User management / role-based access
- Workflow DAGs / job dependencies
- Email/webhook notifications (can be added later)
- Job queuing / concurrency limits (can be added later)

## Suggested Rust Crates

- `tokio` — async runtime
- `cron` or `saffron` — cron expression parsing
- `bollard` — Docker API client (native Rust, no CLI dependency)
- `sqlx` — async database (supports SQLite + PostgreSQL)
- `axum` — web framework
- `askama` or `maud` — HTML templating
- `tailwindcss` — build-time CSS (via standalone CLI)
- `rust-embed` — embed static assets in binary
- `tracing` — structured logging
- `clap` — CLI argument parsing
- `serde` + `toml` — config parsing

## Example Config (Full)

```toml
[server]
bind = "0.0.0.0:8080"
database_url = "sqlite:///data/cronduit.db"
config_reload_interval = "5m"

[defaults]
image = "curlimages/curl:latest"
network = "container:vpn"
volumes = ["/mnt/data:/data"]
delete = true
timeout = "2h"
random_min_gap = "90m"

[[jobs]]
name = "check-ip-hourly"
schedule = "@hourly"
command = "https://ipinfo.io"

[[jobs]]
name = "check-ip-daily"
schedule = "@random"
command = "-s https://ipinfo.io/json"

[[jobs]]
use_defaults = false   # Do not use the defaults
name = "weekly-backup"
schedule = "0 3 * * 0"
type = "command"
command = "/usr/local/bin/backup.sh"

[[jobs]]
# Defaults are overridden, use_defaults = false is optional
name = "custom-container"
schedule = "30 */6 * * *"
image = "my-app:latest"
network = "bridge"
command = "python process.py"
container_name = "my-app-processor"
volumes = ["/data/input:/input:ro", "/data/output:/output"]
env = { MODE = "production" }
timeout = "30m"
```
