# Milestones

## v1.1 — Operator Quality of Life (Shipped: 2026-04-23)

**Tags:** `v1.1.0-rc.1`, `v1.1.0-rc.2`, `v1.1.0-rc.3`, `v1.1.0-rc.4`, `v1.1.0-rc.5`, `v1.1.0-rc.6`, `v1.1.0` · **Phases:** 6 (10, 11, 12, 12.1, 13, 14) · **Plans:** 52 · **Requirements shipped:** 33 / 33 v1.1

**Delivered:** A polish-and-fix milestone layered on top of v1.0.1. Operators can now stop any running job from the UI (new `stopped` status, single hard kill, three-executor coverage), see per-job run numbers (`#1`, `#2`, ...) backfilled on upgrade, navigate back to a running job and see accumulated logs before the live SSE attaches (no gap, no duplicates, no transient "error getting logs" flash), observe fleet activity on a new `/timeline` page with gantt-style bars, see per-job sparklines + success-rate badges on the dashboard, read p50/p95 duration trends on the job detail page, and bulk-enable/disable jobs via a CSRF-gated dashboard checkbox bar backed by a tri-state `jobs.enabled_override` column that survives config reloads. Ships with a working out-of-the-box `docker compose up` healthcheck (new `cronduit health` CLI + Dockerfile HEALTHCHECK), a locked six-tag GHCR contract (`:X.Y.Z`, `:X.Y`, `:X`, `:latest`, `:rc`, `:main`), and no net-new external dependencies (one `rand 0.8 → 0.9` hygiene bump; one new nullable DB column). Released iteratively as `v1.1.0-rc.1` through `v1.1.0-rc.6`, then promoted to `v1.1.0` once Phase 14 human UAT passed on rc.6.

### Key accomplishments

1. **Stop-a-running-job + hygiene preamble (Phase 10)** — New `stopped` status distinct from `cancelled`/`failed`/`timeout`, `RunControl` abstraction with `CancellationToken` + `stop_reason: Arc<AtomicU8>`, preserved `.process_group(0)` + `libc::kill(-pid, SIGKILL)` pattern across all three executors (command/script/docker), race-safe via deterministic `tokio::time::pause` test (T-V11-STOP-04, 1000-iteration lock); `rand 0.8 → 0.9` + `Cargo.toml 1.0.1 → 1.1.0` as the very first v1.1 commit.
2. **Per-job run numbers + log UX fixes (Phase 11)** — Per-job `#1, #2, …` numbering via dedicated `jobs.next_run_number` counter column incremented in a two-statement transaction (identical on SQLite + Postgres), three-file migration with 10k-row chunked backfill and INFO-level progress logs, Option A (insert-then-broadcast with `RETURNING id`) log dedupe adopted after the T-V11-LOG-02 latency benchmark, sync-insert `Run Now` fix eliminates the transient "error getting logs" flash, existing `/jobs/{job_id}/runs/{run_id}` permalinks preserved.
3. **Docker healthcheck + rc.1 cut (Phase 12)** — New `cronduit health` CLI subcommand (hyper-util client, no retry, fail-fast on connection-refused), Dockerfile `HEALTHCHECK CMD ["/cronduit", "health"]` with `--start-period=60s`, compose-smoke CI workflow exercising both shipped-compose and compose-override axes, `release.yml` D-10 rc-tag gating so rc pushes do not move `:latest`, maintainer runbook `docs/release-rc.md`, `v1.1.0-rc.1` cut + verified 2026-04-19.
4. **GHCR tag hygiene (Phase 12.1, INSERTED)** — `:latest` locked to non-rc stable tags only (retroactive + forward fix), new `:main` floating tag for bleeding-edge main-HEAD builds, one-shot `docker buildx imagetools create :latest :1.0.1` retag restored `:latest` = `:1.0.1` digest on both archs, six-tag contract documented in README, `scripts/verify-latest-retag.sh` per-platform digest comparator for maintainer + future rc-guard use.
5. **Observability polish (Phase 13, rc.2)** — New `/timeline` page: cross-job gantt-style view (24h default / 7d toggle), single SQL query bounded by `LIMIT 10000`, `EXPLAIN QUERY PLAN` green on both SQLite and Postgres; 20-run sparklines + success-rate badges on every dashboard card (`N=5` minimum, `stopped` excluded from denominator); `p50/p95` duration trends on job-detail page (`N=20` minimum, last 100 successful runs) via Rust-side `stats::percentile`; `OBS-05` CI grep guard locks no-`percentile_cont` structural parity; `v1.1.0-rc.2` cut 2026-04-21.
6. **Bulk enable/disable + rc.3..rc.6 + v1.1.0 final ship (Phase 14)** — Tri-state nullable `jobs.enabled_override` column (NULL = follow config, 0 = force disabled, 1 = force enabled), CSRF-gated `POST /api/jobs/bulk-toggle` firing `SchedulerCmd::Reload` without killing in-flight runs, settings-page "Currently overridden" audit, `upsert_job` explicitly never touches `enabled_override` (locked by T-V11-BULK-01); shipped iteratively through `rc.3` → `rc.6` clearing four UAT-surfaced bugs (dashboard `enabled_override=0` reflection, timeline bar CSS, self-polling timeline partial, `just reload` recipe); `:latest` promoted to `:1.1.0` multi-arch on 2026-04-23.

### Validated milestone gates

- **Requirements coverage:** 33/33 Complete across 7 categories (SCHED-09..14, DB-09..14, UI-16..20, OBS-01..05, ERG-01..04, OPS-06..10, FOUND-12..13). Full REQ-ID → phase traceability in `.planning/milestones/v1.1-REQUIREMENTS.md`.
- **rc-by-rc UAT:** rc.1 (Phase 12 close-out, 2026-04-19), rc.2 (Phase 13 close-out, 2026-04-21), rc.3..rc.6 (Phase 14 UAT-driven fix loop, 2026-04-22 → 2026-04-23). Final `v1.1.0` UAT signed off by maintainer on rc.6 commit.
- **GHCR invariants at final ship:** `:latest` digest advanced from `:1.0.1` to `:1.1.0`; `:1.1.0` == `:1.1` == `:1` == `:latest` on both amd64 and arm64 (D-18 four-tag equality verified).
- **CI:** compose-smoke workflow continuously regression-tests both `docker-compose.yml` and `docker-compose.secure.yml` axes throughout the milestone; `release.yml` D-10 rc-tag gating verified on every rc push; OBS-05 CI grep guard locked in.

### Known deferred items at close

6 items acknowledged at close (see STATE.md § Deferred Items). All are audit-tool false positives — the underlying work is complete and validated, only the file shapes do not match the audit heuristics. No real deferred work.

### Archives

- `.planning/milestones/v1.1-ROADMAP.md` — full phase details for all 6 phases (including inserted 12.1)
- `.planning/milestones/v1.1-REQUIREMENTS.md` — all 33 requirements with traceability
- `.planning/milestones/v1.1-phases/` — raw execution history for every phase (if archived at close; otherwise still at `.planning/phases/`)

---

## v1.0 — Docker-Native Cron Scheduler (Shipped: 2026-04-14)

**Tags:** `v1.0.0`, `v1.0.1` (latest) · **Phases:** 9 · **Plans:** 49 · **Requirements shipped:** 86 / 86 v1

**Delivered:** A self-hosted, single-binary Rust cron scheduler with a terminal-green HTMX web UI, full Docker-API job execution (including `--network container:<name>` for VPN-bound jobs), `@random` schedule resolution, hot config reload, Prometheus metrics, SSE live log tail, multi-arch release engineering, and a documented threat model — all gated by a green CI matrix on `linux/{amd64,arm64} × {SQLite, Postgres}` from Phase 1.

### Key accomplishments

1. **Foundation, security, and persistence (Phase 1)** — TOML config with env-var interpolation and `SecretString` wrapping, `cronduit check` validator, dual SQLite (read/write WAL pools) and PostgreSQL backends with structural parity tests, and a green CI matrix on `linux/{amd64,arm64} × {SQLite, Postgres}` via `cargo-zigbuild` (no QEMU, no `openssl-sys` in the dep tree).
2. **Hand-rolled scheduler core (Phase 2)** — `tokio::select!` loop firing on `croner` 3.0 expressions in the configured timezone, command and inline-script executors with `shell-words` argv splitting, head-drop bounded log channel with 16 KB line truncation, and a double-signal SIGINT/SIGTERM drain state machine.
3. **Read-only HTMX web UI + `/health` (Phase 3)** — Tailwind (standalone binary, no Node) Cronduit terminal-green design system, vendored HTMX 2.0.4 + JetBrains Mono, dashboard with parameterized filter/sort, job/run/log drill-downs with ANSI rendering, working "Run Now" button via `mpsc` channel into the scheduler, and dark/light token toggle.
4. **Docker executor — the headline differentiator (Phase 4)** — `bollard` 0.20 lifecycle with `auto_remove=false` + explicit `wait_container`/post-drain remove (avoids moby#8441), 10s SIGTERM grace, `cronduit.run_id=<id>` labels for orphan reconciliation, image pull with 3-attempt exponential backoff, and full network-mode coverage (`bridge`, `host`, `none`, `container:<name>`, named) with a `testcontainers` integration test of the marquee `container:<name>` path.
5. **Config reload + `@random` resolver (Phase 5)** — Slot-based `@random` algorithm with `random_min_gap` enforcement and daily re-roll, hot reload via SIGHUP / `POST /api/reload` / debounced file-watch (500 ms), `do_reload`/`do_reroll` rebuild the fire heap without cancelling in-flight container runs, and 7 integration tests lock in survival of in-flight jobs across reload.
6. **Live events, metrics, retention & release engineering (Phase 6)** — SSE log fan-out for in-progress runs with HTMX live→static transition, Prometheus `/metrics` with six eagerly-described families (`cronduit_scheduler_up`, `cronduit_jobs_total`, `cronduit_runs_total{job,status}`, `cronduit_run_duration_seconds{job}`, `cronduit_run_failures_total{job,reason}`, `cronduit_docker_reachable`) on bounded-cardinality labels, daily retention pruner with batched deletes + WAL checkpoint, multi-arch GHCR release workflow, and a SECURITY-first README + `THREAT_MODEL.md`.
7. **Cleanup, bookkeeping & UAT regressions (Phase 7)** — Resolved the docker-compose `ports:` vs `expose:` deviation (OPS-04), bulk-flipped REQUIREMENTS.md traceability to reflect what actually shipped, refreshed stale Phase 5 verification, landed the settings-page reload-card auto-refresh fix, and added an HTMX-polled job-detail run-history partial that stops polling once all runs are terminal (closing the Phase 6 UAT Test 4 "rows frozen at RUNNING" bug).
8. **Final human UAT walkthrough (Phase 8)** — User-driven end-to-end walkthrough rebased the runtime image from distroless to `alpine:3` (UID/GID 1000), expanded the quickstart to 4 example jobs (2 commands + script + Docker), fixed two mid-walkthrough Docker-on-macOS blockers in-session (Rancher Desktop `DOCKER_GID`, docker-socket path parametrization), and closed Phase 8 with the operator's verbal approval recorded across `08-HUMAN-UAT.md` (8/8 passed) and `08-05-SUMMARY.md`.
9. **CI/CD operational hygiene (Phase 9)** — Added `cleanup-cache.yml` and `cleanup-images.yml` workflows, restored `rust-cache` for second-push speedups, fixed a Dockerfile rust-image regex that had silently never matched, and locked the v1.0 release-engineering decision so future audits do not re-litigate it.

### Validated milestone gates

- **Audit:** `v1.0-MILESTONE-AUDIT.md` verdict `passed` (2026-04-14): 86/86 requirements Complete, 9/9 phases nyquist-compliant, 9/9 cross-phase wiring paths confirmed, 7/7 E2E flows complete, 0 gaps, 0 outstanding tech debt.
- **Compose-smoke CI:** Continuous regression coverage via Run Now API per-job assertion on both `docker-compose.yml` and `docker-compose.secure.yml` axes.
- **Operator walkthrough:** Phase 8 walkthrough confirmed quickstart fidelity end-to-end on Docker-on-macOS (the hardest target for the Docker socket path).

### Known post-merge observations (NOT BLOCKERS)

Three Phase 9 UAT items are accepted as deferred to natural post-merge validation per the audit verdict:

- `cleanup-cache.yml` fires on `pull_request:closed` — self-validates when this branch's PR closes.
- `cleanup-images.yml` dispatches against live GHCR — requires published image; monthly cron `0 0 15 * *` provides natural validation, manual dispatch available post-merge.
- `rust-cache` restore on second PR push — self-validates on any PR's second commit.

### Archives

- `.planning/milestones/v1.0-ROADMAP.md` — full phase details for all 9 phases
- `.planning/milestones/v1.0-REQUIREMENTS.md` — all 86 requirements with traceability
- `.planning/milestones/v1.0-MILESTONE-AUDIT.md` — passed-verdict audit report
- `.planning/milestones/v1.0-phases/` — raw execution history (PLAN/SUMMARY/VALIDATION/UAT/CONTEXT for every phase)

---
