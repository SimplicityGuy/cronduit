# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v1.1 — Operator Quality of Life

**Shipped:** 2026-04-23 (tag `v1.1.0`; rc tags `v1.1.0-rc.1` … `v1.1.0-rc.6`)
**Phases:** 6 (10, 11, 12, 12.1 inserted, 13, 14) | **Plans:** 52 | **Calendar timeline:** 9 days (2026-04-15 → 2026-04-23)
**LOC delta:** +~500 Rust src (14,535 total in `src/`); substantial new integration test coverage
**Requirements:** 33 / 33 v1.1 Complete | **Outcome:** all goals met; UAT-driven rc loop rc.3 → rc.6 absorbed four in-cycle fixes before the stable tag

### What Was Built

- A "Stop" button and `stopped` terminal status wired through all three executors (command/script/docker) via a new `RunControl` abstraction (`CancellationToken` + `stop_reason: Arc<AtomicU8>`), preserving the shipped `.process_group(0)` + `libc::kill(-pid, SIGKILL)` pattern and locking the `mark_run_orphaned` `WHERE status = 'running'` guard in place with dedicated tests.
- Per-job run numbers (`#1, #2, …`) via a dedicated `jobs.next_run_number` counter column, delivered as a three-file migration (add nullable → chunked 10k-row backfill → NOT NULL) that works identically on SQLite and Postgres; global `job_runs.id` remains the canonical URL key so permalinks survive.
- An end-to-end log UX fix: insert-then-broadcast with `RETURNING id` gives every line a monotonic id; the run-detail handler backfills from DB on page load then attaches SSE with id-based client-side dedupe, eliminating both the ordering glitch and the transient "error getting logs" flash.
- A `cronduit health` CLI subcommand plus Dockerfile `HEALTHCHECK` that makes `docker compose up` report `healthy` out of the box — removing the busybox `wget --spider` dependency from the healthcheck path entirely.
- A locked six-tag GHCR contract (`:X.Y.Z`, `:X.Y`, `:X`, `:latest`, `:rc`, `:main`) with `release.yml` D-10 gating so rc tags never move `:latest`, a new `:main` floating-tag workflow, a maintainer `docs/release-rc.md` runbook, and `scripts/verify-latest-retag.sh` for per-platform digest verification.
- A new `/timeline` cross-job gantt page (single SQL query bounded by `LIMIT 10000`, EXPLAIN-verified on both backends), 20-run sparklines + success-rate badges on every dashboard card (N=5 minimum, `stopped` excluded from denominator), and p50/p95 duration trends on job-detail (N=20 minimum, last 100 successful runs) computed in Rust via `stats::percentile` — with a CI grep guard locking OBS-05 structural parity (no `percentile_cont`).
- A CSRF-gated bulk enable/disable UX backed by a tri-state nullable `jobs.enabled_override` column (NULL = follow config, 0 = force disabled, 1 = force enabled); `upsert_job` never touches it; `disable_missing_jobs` clears it. Settings page adds a "Currently overridden" audit section. Bulk disable does NOT terminate running jobs.
- Hygiene: `rand 0.8 → 0.9` across four call sites; `Cargo.toml 1.0.1 → 1.1.0` as the very first v1.1 commit; Tailwind v3 → v4 migration landed as a dedicated dep-refresh PR at the start of the milestone.

### What Worked

- **Research before kickoff, again.** The v1.1 research pass (`research/SUMMARY.md` with four Architecture corrections from `PITFALLS.md`) surfaced the `kill_on_drop(true)` anti-pattern and the `mark_run_orphaned` guard **before** implementation — both corrections flowed straight into the requirement language (SCHED-12, SCHED-13) and were locked in by regression tests, not discovered mid-stream.
- **Phase ordering derived from template dependencies, not feature size.** Phase 10 (Stop) and Phase 11 (run numbers) landed first because Phase 13 (observability) needed their status tokens and per-job numbers in templates before sparkline/timeline rendering. The roadmap's `research/SUMMARY.md § Architecture Integration Map` made this explicit and prevented template rewrites.
- **Inserting Phase 12.1 mid-milestone.** Discovered `:latest` divergence during Phase 12 post-push verification; inserted Phase 12.1 via `/gsd-insert-phase` rather than deferring the fix, and locked in both the retroactive retag and the forward prevention (`:main` floating tag + D-10 gating) before rc.2 shipped into a healthy tag ecosystem.
- **UAT-driven rc loop.** Phase 14 shipped as rc.3 and failed UAT at Step 2. Rather than hotpatching directly to `main`, rc.4 → rc.5 → rc.6 cut fix PRs (#39, #40, #41) with each UAT pass finding real operator-visible bugs (dashboard reflection, timeline bar CSS, self-polling partials, `just reload` recipe). All fixes landed in-cycle; `v1.1.0` tag promotes cleanly off rc.6.
- **D-13 maintainer-action tag cuts.** rc.1 through `v1.1.0` all landed via maintainer-signed tags. No `workflow_dispatch` trust-anchor compromise; no drift between tag and code.
- **Test discipline held under load.** New test files (`tests/stop_executors.rs`, `tests/process_group_kill.rs`, `tests/v11_bulk_toggle.rs` + `_pg.rs`, `tests/v13_timeline_explain.rs`, `tests/dashboard_jobs_pg.rs`) landed alongside implementation, not as follow-ups. Postgres parity tests caught a real bug (`j.enabled = true` BIGINT coercion in `get_dashboard_jobs`, quick task `260421-nn3`) that SQLite-only testing would have missed.
- **Benchmarks gate design choices.** T-V11-LOG-02 benchmark confirmed Option A (insert-then-broadcast with `RETURNING id`) was cheap enough for log dedupe before Phase 11's implementation plan was written. No retroactive re-design.
- **Structural parity CI gate.** OBS-05 `just grep-no-percentile-cont` locks SQL identical across backends forward — a future contributor can't regress the decision without editing CI.

### What Was Inefficient

- **Phase 14 UAT surfaced four rc cuts.** rc.3 → rc.6 was technically working as designed (UAT catches what tests miss) but each cut burned a multi-arch release-yml run. For Phase 14-like surface-touching features, we should consider a pre-rc maintainer smoke-test against a just-built `:main` image before tagging `rc.N`.
- **`bfea2e8` Plan 09 Task 3 created a root `MILESTONES.md`.** The Plan 09 description explicitly notes it "originally assumed `MILESTONES.md` already had a v1.0 entry to mirror" — meaning the v1.0 close-out never created the top-level file, only the in-`.planning` one. Split-file ownership (root vs `.planning/`) is now locked by precedent: root `MILESTONES.md` is the external release log, `.planning/MILESTONES.md` is the internal archive index.
- **Audit-tool false positives on close.** Pre-close audit flagged 3 UAT files, 1 verification file, and 2 quick-task state files as "incomplete" — all were complete but in formats the audit heuristics don't recognize. Wasted operator attention at close time. Fix forward: either update the heuristics or adopt file-format conventions the tool recognizes (explicit `status: complete` front-matter on UAT files; quick-task state files kept as placeholder rather than deleted on completion).
- **Plan counts displayed as `?` in ROADMAP.md until each phase was decomposed.** The roadmap's Progress table had entries like `10. Stop… 0/10 | 10/10 | Complete` where the first column was stale (never updated from the kickoff `0/?`) and the second was the actual count. Merge-of-progress with plan-decomposition created two truths. Fix forward: roadmap's plan-count column should be updated by `/gsd-plan-phase` at decomposition time, not left as `?`.
- **Phase 12 split-commit pattern carried to Phase 14 Plan 09.** Phase 12 Plan 07 executed REQUIREMENTS checkbox flips + traceability flips atomically (Truth #1 + Truth #2). Phase 14 Plan 09 split them across two commits (`e919fc7` requirements, `bfea2e8` MILESTONES/README). The split worked, but the atomic pattern from Phase 12 is tidier — re-codify as a convention.

### Patterns Established

- **Decimal-numbered inserted phases as first-class citizens.** Phase 12.1 got its own directory (`12.1-ghcr-tag-hygiene/`), its own UAT, its own close-out — not a sub-task of Phase 12. This is the pattern for "I discovered this during Phase N's post-push verification; must land before Phase N+1".
- **Three-file tightening migrations.** The Phase 11 add-nullable → backfill → add-NOT-NULL pattern was reused by Phase 14's `enabled_override` migration. Partial-failure recovery is in the shape, not the code.
- **Maintainer-action checkpoints in human-UAT docs.** Every rc tag cut has a `HUMAN-UAT.md` runbook that the maintainer executes locally. Claude never invokes `git tag` or `git push`; Claude only verifies the preconditions and documents the procedure. Locked by `feedback_uat_user_validates.md`.
- **`just`-recipe-anchored UAT steps.** Every UAT step in `14-HUMAN-UAT.md` references a `just` recipe (per `feedback_uat_use_just_commands.md`). The recipes are the contract; "run `cargo test`" or "docker-compose up -d" never appear in UAT docs.
- **Six-tag GHCR contract, documented negative-space.** README's "What's NOT published" list is load-bearing. Future contributors proposing `:edge`/`:nightly`/`:dev` tags have to justify the addition against the contract.
- **Structural-parity CI gates.** OBS-05's `grep-no-percentile-cont` recipe is the model for locking backend-specific antipatterns out of the code. Cheap (`grep`), durable (CI-enforced), and self-explanatory in review.
- **Rc cuts as polish checkpoints, not just release artifacts.** rc.1 closed the bug-fix block, rc.2 closed observability, rc.3→rc.6 absorbed Phase 14 UAT fixes. Each rc is a checkpoint against the prior rc, not a fresh diff against main.

### Key Lessons

1. **Research corrections are worth more than feature research.** Four research-phase corrections (`kill_on_drop`, orphan guard, log-id option selection, and a fourth) all saved multi-day rework. Budget for "what are we about to do wrong?" explicitly in the research phase.
2. **UAT-driven rc loops catch the right class of bugs.** rc.4-6 caught behaviors that unit and integration tests never would have (dashboard reflection, timeline CSS, operator-facing recipe ergonomics). The rc cadence is the test harness.
3. **Phase ordering should derive from architecture integration maps, not feature priority.** Template dependencies dictated Phase 10 and 11 order, not "what's most important". The integration map is the scheduling primitive.
4. **Inserting a phase mid-milestone is fine when the insertion is load-bearing for a later phase.** Phase 12.1 was a must-land-before-rc.2 insertion, not scope creep. The decimal number signals this.
5. **Close-out audit heuristics are lossy.** Six false positives at `/gsd-complete-milestone` close means the tool's format expectations are out of sync with our file conventions. Either match the conventions or update the heuristics — both are valid, but the gap creates toil every time.
6. **REQUIREMENTS.md flips belong in the closing plan's commit, atomically with traceability updates.** Phase 12 got this right; Phase 14 split it. Treat the full rollup (body checkboxes + traceability table + footer) as a single unit of meaning.
7. **Cargo.toml bump on the first milestone commit.** FOUND-13 ensured `cronduit --version` reported `1.1.0` from the very first v1.1 commit. Carries forward to v1.2 — bump on the first commit.

### Cost Observations

- **Profile:** quality (`opus` throughout; fast mode for some batched edit work)
- **Calendar timeline:** 9 days (2026-04-15 → 2026-04-23)
- **Phases added mid-milestone:** 1 of 6 (Phase 12.1 inserted during Phase 12 post-push verification)
- **rc cuts:** 6 (`rc.1` → `rc.6`) before the stable `v1.1.0` tag
- **PRs merged to main:** ~14 (`#24` v1.1 kickoff → `#41` rc.6 fix), all via feature branches, zero direct commits to main
- **Quick tasks completed in-milestone:** 1 (`260421-nn3` Postgres BIGINT fix, PR #37)
- **Notable:** A 9-day polish milestone shipping 52 plans and 33 requirements across 6 phases with 6 rc cuts and zero main-branch hotfixes is a useful datapoint for GSD-orchestrated iterative maintenance cycles on a shipped v1.

---

## Milestone: v1.0 — Docker-Native Cron Scheduler

**Shipped:** 2026-04-14 (tag `v1.0.0`)
**Phases:** 9 | **Plans:** 49 | **Calendar timeline:** 6 days (2026-04-08 → 2026-04-14)
**LOC:** ~14,000 Rust (~10k src, ~4k tests)
**Requirements:** 86 / 86 v1 Complete | **Audit verdict:** `passed`

### What Was Built

- A single-binary Rust cron scheduler that executes commands, scripts, and ephemeral Docker containers via `bollard` 0.20 (no docker CLI shell-out), with full coverage of every Docker network mode including the marquee `network = "container:<name>"` for VPN-bound jobs.
- A terminal-green HTMX web UI (`askama_web` 0.15 + Tailwind via standalone binary, no Node) with dashboard, job/run/log drill-down, "Run Now", SSE log tail for in-progress runs, and a settings/reload page.
- Hot config reload via SIGHUP / `POST /api/reload` / debounced file-watch — without cancelling in-flight container runs, integration-tested.
- A slot-based `@random` cron resolver with `random_min_gap` enforcement and daily re-roll.
- Prometheus `/metrics` with six eagerly-described families on bounded-cardinality labels, daily retention pruner, multi-arch (amd64+arm64) GHCR release via `cargo-zigbuild` (no QEMU).
- A SECURITY-first README, `THREAT_MODEL.md`, and two reference compose files (convenience + defense-in-depth socket-proxy variant), all exercised by a `compose-smoke` CI job.

### What Worked

- **Lock all big technology decisions before Phase 1.** The PROJECT.md "Key Decisions" table was populated *before* code existed (Rust + bollard + sqlx + askama_web + croner + TOML + tokio scheduler + Tailwind standalone + rust-embed). Zero rewrites of foundational choices across all 9 phases.
- **Research phase paid for itself within Phase 1.** The pre-implementation `research/` notes flagged `serde-yaml` archival, `saffron` abandonment, `askama_axum` deprecation, and the SQLite read/write pool requirement — all decisions that would have caused multi-day rework if discovered mid-implementation.
- **Test discipline from Phase 1.** Every phase shipped with both unit and integration tests on green CI. The `linux/{amd64,arm64} × {SQLite, Postgres}` matrix from day one caught schema-parity drift the first time it happened, instead of at "polish phase".
- **`testcontainers` for the marquee `container:<name>` path.** This was the riskiest feature in the spec, and an actual integration test that joins one container's network namespace is the only real proof that it works. Worth the test-runtime cost.
- **`auto_remove=false` + explicit post-drain remove for Docker jobs.** Discovering moby#8441 in Phase 4 and locking the workaround into the Key Decisions table prevented Phase 6/7/8 from accidentally re-introducing the race.
- **Just-only invocation rule (FOUND-12 / D-10).** Every CI step is a `run: just <recipe>` — no inline `cargo`/`docker`/`rustup` commands. This means "what runs in CI" and "what I run on my laptop" are byte-identical and there's exactly one place to look when something diverges.
- **Mandatory user UAT after Phases 7 and 9.** The Phase 8 walkthrough caught two Docker-on-macOS blockers (Rancher Desktop `DOCKER_GID` mismatch, docker-socket path parametrization) that no automated test would have surfaced. They were fixed in-session and merged before v1.0 archival.
- **Audit-driven gap closure.** `/gsd-audit-milestone` surfaced bookkeeping debt (OPS-04, OPS-05, stale Phase 5 verification) that would have shipped silently — Phase 7 (cleanup) and Phase 8 (UAT) closed those gaps cleanly.

### What Was Inefficient

- **MILESTONES.md auto-generation pulls noisy section headers as "accomplishments".** The CLI extracted strings like `Commit:`, `Root cause:`, `1. [Rule 3 - Blocking]` from SUMMARY.md files because it grepped for one-liner patterns without filtering for prose. Hand-curation was required at archival time. Fix forward: improve `summary-extract` heuristics or have `gsd-tools milestone complete` emit a draft for human review rather than committing the noisy version directly.
- **Roadmap counts went stale when Phase 8 and Phase 9 were inserted mid-milestone.** `gsd-tools roadmap analyze` returned `phase_count: 7` because the roadmap text was never updated when the new phases were added — only their phase directories existed. The archive workflow then picked up the stale 7/40 numbers and had to be hand-corrected to 9/49. Fix forward: keep ROADMAP.md and disk-state in lockstep at phase-add time, or have the analyzer reconcile both.
- **Dynamically added phases (8, 9) lacked v1 REQ-IDs.** Phase 9 in particular is operational hygiene with no v1 mapping; the audit had to make a "n/a — operational hygiene phase" judgment call. Future milestones should either backfill REQ-IDs at phase-add time or document the carve-out up front.
- **Tag/code version mismatch was almost shipped.** `Cargo.toml` was still at `0.1.0` until the milestone-completion pass — the project would have tagged `v1.0.0` while reporting `cronduit 0.1.0` from `--version`. Caught and fixed at archival, with a permanent feedback memory written so it's surfaced in every future release.
- **README and SPEC drifted from code.** The README's metric families table listed only 4 of the 6 actually exposed; the SPEC was the original 2026-04-09 pre-implementation doc and had never been updated. Both were caught and rewritten during milestone archival rather than at the moment of drift.

### Patterns Established

- **Single-binary discipline.** No CDN dependencies (HTMX vendored), no Node toolchain (Tailwind standalone binary), no QEMU (`cargo-zigbuild`), no `openssl-sys` in the dep tree. The CI `openssl-check` recipe enforces the last one mechanically.
- **`metrics` facade over direct exporter.** Decouples instrumentation from export format; lets us swap to OpenTelemetry later without touching call sites.
- **Closed-enum labels in Prometheus.** `failure_reason` and `status` are bounded enums known at compile time, so the exporter cannot grow these label sets at runtime.
- **Threat model lives in code review.** `THREAT_MODEL.md` is referenced from the README's first section and from the example compose files' security headers — every operator path leads back to it.
- **`gsd-validate-phase` retroactive Nyquist sweep.** Phases 1–9 were all flipped to `nyquist_compliant: true` via a single sweep at the end of v1.0 — not a per-phase gate. This worked because the audit caught coverage gaps before they hardened, but for v1.1+ we should consider whether per-phase compliance gating is worth the workflow cost.
- **PR-only main branch + every change branched.** No direct commits to main throughout the entire 9-phase v1.0 cycle. Even retroactive bookkeeping like the Nyquist sweep landed via PR #17. Zero merge conflicts.

### Key Lessons

1. **"Lock decisions before code" is a Pareto-optimal use of research time.** ~1 day of upfront research saved an unknowable but clearly multi-day cost in mid-stream rework.
2. **Tag/release version drift is a release-engineering trap.** Always bump `Cargo.toml` first, regenerate `Cargo.lock`, verify `cronduit --version` reports the new value, *then* tag. This is now codified in `feedback_tag_release_version_match.md`.
3. **Phases that get added late don't get audited the same way.** Phases 8 and 9 (added 2026-04-12 and 2026-04-13 respectively) needed an explicit reconciliation pass with the original 86-requirement traceability table. Future late-added phases should either rebuild the full traceability or be explicitly carved out as "n/a — operational hygiene" at insertion time.
4. **User UAT catches what automation can't.** The Phase 8 walkthrough found two macOS-host environmental blockers no automated test would have surfaced. UAT is not optional for a v1.0 ship.
5. **Auto-extracted accomplishment lists need human review before they ship to MILESTONES.md.** The pattern-matching heuristics that extract one-liners from SUMMARY.md files pull in noise. Treat the CLI's output as a draft, not a finished document.
6. **`gsd-tools roadmap analyze` is only as fresh as ROADMAP.md.** When a phase is added by directory creation alone, the analyzer returns stale counts. Either keep the doc in sync or change the analyzer to reconcile.
7. **The single-binary constraint is load-bearing for the homelab use case.** Every adopter constraint we honored (vendored HTMX, standalone Tailwind, embedded assets via `rust-embed`, no Node) compounded into a quickstart that worked end-to-end on first `docker compose up` during the Phase 8 walkthrough.

### Cost Observations

- **Profile:** quality (`opus` for both planner and executor across all 9 phases)
- **Calendar timeline:** 6 days (2026-04-08 → 2026-04-14)
- **Plans-per-day average:** ~8 plans/day across the v1.0 window
- **Phases added mid-milestone:** 2 of 9 (Phase 8 and Phase 9 — both gap closure / operational hygiene, neither in the original Phase 1–7 roadmap)
- **PRs merged to main:** 9 (#9 → #17), all phases via PR, zero direct commits to main
- **Notable:** 49 plans / 9 phases / 86 requirements / ~14k Rust LOC in 6 calendar days using a quality-tier profile and a strict "land via PR" workflow is a useful upper bound on what GSD-orchestrated v1 development can accomplish for a single-binary scheduler shape.

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Phases | Plans | Key Change |
|-----------|--------|-------|------------|
| v1.0 | 9 | 49 | Initial milestone — established the GSD workflow on this project |
| v1.1 | 6 (incl. 12.1 inserted) | 52 | First polish-cycle milestone; UAT-driven rc loop (rc.3 → rc.6) as a test harness; first mid-milestone inserted decimal phase (12.1) |

### Cumulative Quality

| Milestone | Source LOC | Test LOC | Outcome |
|-----------|-----------|----------|---------|
| v1.0 | ~10,000 | ~4,000 | passed (86/86 reqs, 9/9 nyquist, 7/7 E2E flows, 0 gaps) |
| v1.1 | ~14,500 (`src/`) | substantial new integration coverage (stop, bulk toggle, timeline explain, timeline timezone, dashboard jobs pg, process group kill, metrics stopped) | 33/33 reqs Complete, `v1.1.0` tagged 2026-04-23 after rc.6 UAT pass |

### Top Lessons (Verified Across Milestones)

1. (v1.0, v1.1) Lock big technology decisions before the first phase — research phase pays for itself. v1.1's research corrections (e.g. preserve `.process_group(0)`; lock `mark_run_orphaned` guard) saved multi-day rework.
2. (v1.0) Tag and `Cargo.toml` version must always match (per `feedback_tag_release_version_match.md`). v1.1 extended this to "bump `Cargo.toml` on the first milestone commit" (FOUND-13 pattern).
3. (v1.0) MILESTONES.md auto-extraction needs human review before commit. (v1.1) Close-out audit heuristics are lossy — six false positives at v1.1 close.
4. (v1.1) UAT-driven rc loops catch operator-visible bugs that tests miss. Budget for rc.3 → rc.N+ on surface-touching phases.
5. (v1.1) REQUIREMENTS.md checkbox + traceability flips belong atomically in the closing plan's commit (Phase 12 pattern beats Phase 14 split-commit pattern).
6. (v1.0, v1.1) PR-only main branch + every change on a feature branch holds up across 15 phases — zero main-branch hotfixes, zero merge conflicts.
7. (v1.1) Decimal-numbered mid-milestone phase insertions are first-class citizens when they're load-bearing for a later phase (Phase 12.1 as prerequisite for rc.2).
