# Milestones

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
