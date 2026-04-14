# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

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

### Cumulative Quality

| Milestone | Source LOC | Test LOC | Audit Verdict |
|-----------|-----------|----------|---------------|
| v1.0 | ~10,000 | ~4,000 | passed (86/86 reqs, 9/9 nyquist, 7/7 E2E flows, 0 gaps) |

### Top Lessons (Verified Across Milestones)

*Will populate as additional milestones ship and lessons recur.*

1. (v1.0) Lock big technology decisions before Phase 1 — research phase pays for itself.
2. (v1.0) Tag and `Cargo.toml` version must always match (per `feedback_tag_release_version_match.md`).
3. (v1.0) MILESTONES.md auto-extraction needs human review before commit.
