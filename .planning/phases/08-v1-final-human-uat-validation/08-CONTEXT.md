---
phase: 08-v1-final-human-uat-validation
gathered: 2026-04-13
status: ready_for_planning
---

# Phase 8: v1.0 Final Human UAT Validation - Context

**Gathered:** 2026-04-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Close every human-validation gap blocking v1.0 archive, plus the three functional
quickstart blockers logged in `07-UAT.md` on 2026-04-13 that prevent the walkthrough
from happening at all. Concretely:

1. **Runtime rebase** — move from `gcr.io/distroless/static-debian12:nonroot` to
   `alpine:3` so non-Docker example jobs (`command=` / `script=`) can actually execute
   and the homelab quickstart ships with a mix of job types, not just Docker.
2. **docker.sock access** — make bollard inside the container reach `/var/run/docker.sock`
   reliably on both Linux and Docker Desktop macOS via a dual-compose example set.
3. **Docker daemon pre-flight** — startup check that WARNs and exposes a gauge metric
   when the daemon is unreachable, degrading gracefully so command-only configs still boot.
4. **Expanded quickstart examples** — 3 non-container jobs (command × 2, script × 1) plus
   the existing `hello-world` docker job, all verified running on a fresh `docker compose up`
   against the new alpine runtime.
5. **Cold-start smoke test** — extend the existing `compose-smoke` CI job so it also
   asserts every quickstart job reaches `status=success` within 120 seconds of boot.
6. **Human UAT walkthrough** — user runs the binary and records pass/fail/issue in the
   existing per-phase UAT files (`03-HUMAN-UAT.md`, a new `06-HUMAN-UAT.md`, and the
   blocked entries in `07-UAT.md`) for: terminal-green theme rendering, dark/light mode
   toggle persistence, Run Now toast, ANSI log rendering, quickstart end-to-end, SSE live
   log streaming, and Plan 07-05 job-detail auto-refresh once the example jobs can stay
   running long enough to observe the transition.
7. **v1.1 backlog routing** — any issue surfaced during the walkthrough that isn't
   functional breakage gets a v1.1 backlog entry instead of a Phase 8 fix.

**Out of scope:** new features, behavior changes beyond the three logged blockers, UI
visual polish, refactors, and anything that would normally belong to a fresh phase.
Phase 8 is the last gate before v1.0 archive — scope discipline matters.

</domain>

<decisions>
## Implementation Decisions

### Runtime Rebase

- **D-01:** Rebase the cronduit runtime image from `gcr.io/distroless/static-debian12:nonroot`
  to `alpine:3`. Keeps the cronduit binary (still compiled static-musl via cargo-zigbuild)
  but gives the container a working `/bin/sh`, `date`, `sleep`, `wget`, `awk`, `sed`, and
  the rest of busybox so `command=` and `script=` jobs can execute. The Dockerfile builder
  stage (rust:1.94-slim-bookworm + cargo-zigbuild) is unchanged — only the `FROM` line of
  the runtime stage changes.

- **D-02:** Create an explicit `cronduit` system user and group inside the alpine runtime:
  `addgroup -S cronduit && adduser -S -u 1000 -G cronduit cronduit`. Cronduit runs as
  UID 1000 (not root) to keep the "cronduit does not run as root inside your homelab"
  security story intact. `/data` is pre-created with `install -d -o 1000 -g 1000 /data`
  so named-volume mounts inherit writable permissions on first mount (same pattern as
  the old `/staging-data` copy but without the multi-stage `--chown` dance).

- **D-03:** Drop the existing `--chown=65532:65532 /staging-data /data` COPY — it targets
  the defunct distroless nonroot UID and is replaced by D-02's in-runtime `install -d`.
  Do NOT leave the old copy commented out; remove it cleanly.

- **D-04:** Update the `USER` directive from `nonroot:nonroot` to `cronduit:cronduit`
  (or the numeric equivalent `1000:1000`). The `EXPOSE 8080` and `ENTRYPOINT ["/cronduit"]`
  lines are unchanged. Keep `CMD ["run", "--config", "/etc/cronduit/config.toml"]`.

- **D-05:** The runtime must still include `ca-certificates` (alpine's `apk add --no-cache
  ca-certificates tzdata`) so bollard's HTTPS calls to image registries and cronduit's
  timezone handling continue to work. Chain the `apk add` into a single RUN layer and do
  not leave an apk cache behind.

- **D-06:** Regenerate the multi-arch build path so the GitHub Actions workflow still
  publishes `linux/amd64 + linux/arm64` images. Alpine has both arches as first-class
  tags (`alpine:3`), so no `docker buildx` matrix change is needed — only the runtime
  `FROM` line flips.

### docker.sock Access (Dual Compose Examples)

- **D-07:** Ship TWO compose files under `examples/`:
  - `examples/docker-compose.yml` — the simple quickstart with a numeric `group_add`
    stanza. Default: `group_add: ["${DOCKER_GID:-999}"]`. A SECURITY comment block at
    the top documents how to derive `DOCKER_GID` on Linux (`stat -c %g /var/run/docker.sock`)
    and warns that Docker Desktop macOS needs the socket-proxy file instead because the
    socket inside the Linux VM is root-owned with no host-side GID mapping.
  - `examples/docker-compose.secure.yml` — adds `tecnativa/docker-socket-proxy` as a
    sidecar, points cronduit at `DOCKER_HOST=tcp://dockerproxy:2375` via environment,
    and drops the socket mount from the cronduit service entirely. This is the "same
    thing, defense-in-depth" example and is what macOS users are pointed to.

- **D-08:** README quickstart (or docs link) must mention both files: default to
  `docker-compose.yml` for new users, call out `docker-compose.secure.yml` as the
  production / macOS / "I want the threat model honored" recipe. Two-file layout lets
  users copy whichever matches their constraints without editing YAML.

- **D-09:** Keep the existing SECURITY block at the top of `docker-compose.yml` (written
  in Phase 7) but insert the `group_add` explanation + `DOCKER_GID` derivation snippet
  inline. Do not turn it into a table or ASCII diagram — plain `#`-prefixed lines only
  (per the mermaid-only diagram rule).

- **D-10:** In `docker-compose.secure.yml`, use `tecnativa/docker-socket-proxy:latest`
  with a minimal allowlist (`CONTAINERS=1`, `IMAGES=1`, `POST=1` for container create,
  everything else default-deny). Comment lines explaining each env var. Point cronduit
  at it via `DOCKER_HOST=tcp://dockerproxy:2375` — bollard reads DOCKER_HOST the same way
  the docker CLI does.

### Docker Daemon Pre-flight Check

- **D-11:** At startup (after config parse, before the scheduler loop begins) cronduit
  calls `bollard::Docker::ping()` once. If it succeeds, log an INFO line ("docker daemon
  reachable at {uri}") and set the new gauge `cronduit_docker_reachable{}` to `1`. If it
  fails, log a WARN line with the remediation hints (`check /var/run/docker.sock mount`,
  `check group_add or DOCKER_GID env`, `on macOS Docker Desktop consider docker-compose.secure.yml`)
  and set the gauge to `0`. Cronduit continues to boot either way — command/script configs
  must still work when Docker is unavailable.

- **D-12:** New metric: `cronduit_docker_reachable` (gauge, no labels). Lives in the
  existing Phase 6 metrics family. Flip back to `1` on the first successful bollard call
  during subsequent operation (e.g., next docker job launch or next config reload that
  triggers a fresh ping). Never remove the gauge once registered — Prometheus expects
  stable metric families.

- **D-13:** Pre-flight fires only on startup and on explicit config reload (SIGHUP or
  API-triggered). Do not re-ping every job run — that's O(N jobs × daemon latency) noise.
  The gauge value is updated opportunistically by any successful or failing docker-job
  launch in between reloads.

- **D-14:** Pre-flight is NOT fail-fast. Even if the config has docker jobs and the
  daemon is unreachable, cronduit still boots. The operator sees the WARN line, the
  gauge flips to `0`, and Prometheus can alert. Rationale: transient daemon flaps at
  startup should not kill the process; the scheduler loop handles per-run docker failures
  with the existing `image_pull_failed` / `unknown` failure reasons from Phase 6 D-05.

### Expanded Example Jobs (`examples/cronduit.toml`)

- **D-15:** `examples/cronduit.toml` ships FOUR example jobs after Phase 8, covering
  every execution type:
  1. `echo-timestamp` — **command**, `*/1 * * * *`, runs `date '+%Y-%m-%d %H:%M:%S -- Cronduit is running!'`.
     Instant heartbeat; proves command execution end-to-end on the alpine runtime.
  2. `http-healthcheck` — **command**, `*/5 * * * *`, runs `wget -q -S --spider https://example.com 2>&1 | head -10`.
     Realistic uptime canary; validates DNS + egress from the container; uses busybox wget
     (already present in alpine). `2>&1 | head -10` captures the response headers into
     cronduit's stdout log.
  3. `disk-usage` — **script**, `*/15 * * * *`, shebang `#!/bin/sh`, body runs
     `du -sh /data 2>/dev/null || echo "/data not mounted"; df -h /data 2>/dev/null || true`.
     Demonstrates the script-job path and the `/data` volume; useful as a retention
     sanity check the operator can grep for in the dashboard log view.
  4. `hello-world` — **docker**, `*/5 * * * *`, `image = "hello-world:latest"`, `delete = true`.
     Keeps the existing docker-executor demo. Works end-to-end once D-07 lands.

- **D-16:** The existing SECURITY + usage comment block at the top of
  `examples/cronduit.toml` is preserved. Add a short paragraph after the `[defaults]`
  block explaining the new job mix ("four quickstart jobs covering command, script,
  and Docker execution"). Do not reorganize the existing `[server]` / `[defaults]`
  structure.

- **D-17:** Every new example job's schedule must fit inside the compose-smoke CI
  budget: `echo-timestamp` runs every 1 minute so it's observable in ~60s of smoke
  testing; `http-healthcheck`, `disk-usage`, and `hello-world` run at 5/15/5-minute
  intervals. The cold-start smoke test (D-18) uses `run_now` or direct job-ID triggering
  to force every job to execute once during the CI window rather than waiting for the
  schedule.

### Cold-Start Smoke Test (CI)

- **D-18:** Extend the existing `compose-smoke` GitHub Actions job (added in Phase 6
  gap closure) with a new test step that asserts every example job reaches a terminal
  `success` state within 120 seconds of container boot. The new step runs AFTER the
  existing `/health` + job-load assertions and BEFORE teardown.

- **D-19:** Trigger strategy: use cronduit's existing Run Now API (`POST /api/jobs/{id}/run`)
  to force each of the 4 jobs to execute immediately, then poll the run history endpoint
  (`GET /api/jobs/{id}/runs?limit=1`) until the latest run shows `status=success` OR the
  120-second wall-clock budget expires. Use `curl -sf` + `jq` to keep the step shell-only;
  add `jq` to the existing `compose-smoke` apt install line if it's not already there.

- **D-20:** On failure (any job not success within 120s), the step must dump:
  - `docker compose logs cronduit --tail=200`
  - `docker compose logs dockerproxy --tail=50` (when the secure variant runs)
  - The last 5 entries from `/api/jobs/{id}/runs` for each job
  - `curl http://127.0.0.1:8080/metrics | grep cronduit_docker_reachable`
  Then exit non-zero. This gives enough context to diagnose flakes without re-running
  locally.

- **D-21:** Run the smoke test against BOTH compose files — one CI matrix axis covers
  `compose: [docker-compose.yml, docker-compose.secure.yml]`. Both must pass for the
  PR to merge. Guarantees the dual-file story stays real and not documentation-only.
  The secure variant runs `docker-socket-proxy`, so the CI runner's docker.sock permissions
  are isolated from cronduit itself — simpler to reason about in hosted runners.

- **D-22:** Do NOT move the existing `/health` + job-load assertions out of `compose-smoke`.
  Add the new job-success assertions inside the same job so we keep a single compose-up /
  compose-down cycle per matrix axis. Two separate jobs would double the boot time budget.

### Human UAT Walkthrough Layout

- **D-23:** UAT results land in the existing per-phase files, in place, to preserve
  provenance. No consolidated Phase 8 UAT file. Specifically:
  - `.planning/phases/03-read-only-web-ui-health-endpoint/03-HUMAN-UAT.md` — flip the
    four pending items (terminal-green theme, dark/light toggle persistence, Run Now
    toast, ANSI log rendering with stderr distinction) to `pass` / `issue`.
  - **New:** `.planning/phases/06-live-events-metrics-retention-release-engineering/06-HUMAN-UAT.md`
    — capture the user-run quickstart end-to-end test (clone → `docker compose up` →
    dashboard loads → first `echo-timestamp` run fires within ~1 minute) AND the SSE
    live log streaming test (trigger a long-running job → Run Detail shows LIVE badge →
    log lines stream in real time → on completion transitions to the static viewer).
  - `.planning/phases/07-v1-cleanup-bookkeeping/07-UAT.md` — re-run the currently
    blocked Test 2 (job-detail auto-refresh with 10+ Run Now clicks) now that D-15's
    longer-running example jobs exist (http-healthcheck + disk-usage can both sustain
    a ~5-15 second RUNNING state). Flip `result: blocker` to `pass` or `issue`.
  - **New short summary:** `.planning/phases/08-v1-final-human-uat-validation/08-HUMAN-UAT.md`
    acts as an index only. Lists every per-phase UAT file touched in Phase 8, the
    final status of each item, and any follow-up fixes / v1.1 backlog items.

- **D-24:** Every UAT entry must include one of: `result: pass`, `result: issue`
  (with `severity` and `reported` fields), or `result: blocked` (with `blocked_by`
  and `reason`). The existing `03-HUMAN-UAT.md` and `07-UAT.md` YAML frontmatter shapes
  are the canonical template — follow them exactly. Do not invent new status strings.

- **D-25:** UAT is user-driven. Per project memory rule ("UAT requires user validation"),
  Claude does NOT mark any UAT item passed from its own test runs. Claude prepares
  fixtures (docker-compose up, example jobs running, browser ready) and surfaces the
  test scripts / acceptance criteria; the user clicks through and types in the result.

### v1.1 Backlog Routing

- **D-26:** The triage rule during the walkthrough: **functional breakage → fix in
  Phase 8; visual polish / copy / dark-mode edge cases → v1.1 backlog entry**. Examples:
  - Fix in Phase 8: job fails to run, page crashes, toast never appears, live log
    stream hangs, auto-refresh stops working, docker pull errors out silently.
  - Defer to v1.1: spacing or color contrast tweaks within brand tolerance, copy
    wording nitpicks, dark-mode rendering edge cases that still render, cosmetic
    alignment on narrow viewports.

- **D-27:** v1.1 backlog entries land in `.planning/BACKLOG.md` (create if missing)
  using the existing 999.x numbering convention per project docs. Every entry must
  include: a short title, the originating UAT file + line, the observed behavior,
  the expected behavior, and a "why this isn't a v1.0 blocker" sentence. The file is
  committed alongside the UAT updates so the v1.1 milestone kickoff has a ready-made
  seed list.

- **D-28:** If a surfaced issue is ambiguous (functional OR cosmetic, depending on
  viewpoint), default to v1.1 unless blocking a v1.0 success criterion. Err toward
  shipping v1.0. The milestone audit already accepted Phase 6/7 as complete — Phase 8
  is closing the last door, not reopening all the others.

### Socket-proxy allowlist (resolves RESEARCH Q1)

- **D-29:** The `docker-socket-proxy` environment allowlist in
  `examples/docker-compose.secure.yml` must include **`DELETE=1`** in addition to
  `CONTAINERS=1`, `IMAGES=1`, and `POST=1` (D-10 baseline). Rationale: bollard's
  `remove_container` call (fired when a job has `delete = true`, e.g. the `hello-world`
  quickstart job) issues `DELETE /containers/{id}`. Per the `tecnativa/docker-socket-proxy`
  README, HTTP verbs are separate permission flags — `POST=1` enables POST verbs only
  and does **not** imply DELETE. Without this flag the Phase 8 `compose-smoke-secure`
  CI axis fails at the `hello-world` step, blocking the Wave 3 human UAT walkthrough.
  Source: https://github.com/Tecnativa/docker-socket-proxy (env var reference).
  Resolves 08-RESEARCH.md Open Question Q1.

### Claude's Discretion

- **Plan ordering / wave assignment.** The four work streams (runtime rebase, compose
  files, pre-flight check, cold-start smoke test, UAT walkthrough) have a natural
  dependency: compose + pre-flight + examples need the runtime rebase to land first,
  smoke test depends on all three, UAT walkthrough depends on everything passing CI.
  The planner can decide whether to ship as 3 plans (rebase + support / smoke-test /
  human-UAT) or 4 plans (one per bullet).

- **Alpine package pinning.** D-05 says "include ca-certificates and tzdata via apk
  add". Whether to pin to explicit versions (e.g., `ca-certificates=20241121-r1`) or
  track the alpine:3 moving tag is the planner's call. Default to moving-tag for
  simplicity unless there's a reproducibility argument for pinning.

- **Exact wording of the WARN line for the pre-flight check.** D-11 lists required
  remediation hints but the exact text is the planner's choice. Keep it under 200
  chars so structured log consumers can grep it.

- **Exact allowlist for docker-socket-proxy.** D-10 specifies `CONTAINERS=1, IMAGES=1,
  POST=1` as the minimum. The planner can trim further (e.g., scope POST to container
  create only) as long as cronduit's actual bollard calls still succeed.

- **jq vs shell-only polling in the smoke test.** D-19 suggests `jq`. If the existing
  compose-smoke job already uses a different JSON parser or if adding jq bloats the
  runner setup meaningfully, the planner can substitute a shell-only `grep '"status":"success"'`
  check.

- **Whether to wait for `06-VERIFICATION.md` / `07-VERIFICATION.md` updates.** Phase 8
  is a human UAT phase, not a verification re-run. If flipping the "human_needed" status
  on those verification files is needed for audit-milestone to record v1.0 as complete,
  the planner can fold a small annotation into the final plan. Otherwise, leave them.

### Folded Todos

None — `todo match-phase 8` returned 0 matches.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (researcher, planner, executor) MUST read these before planning or implementing.**

### Phase 8 Source-of-Truth Documents

- `.planning/ROADMAP.md` § "Phase 8: v1.0 Final Human UAT Validation" — phase boundary,
  the 7 success criteria, and the three logged blockers from `07-UAT.md` (echo-timestamp
  ENOENT, hello-world docker.sock Connect, Plan 07-05 blocked-by-prior-test).
- `.planning/REQUIREMENTS.md` — UI-05, UI-06, UI-09, UI-12 (Phase 3 visual validation),
  OPS-05, UI-14 (Phase 6 quickstart + SSE). Check current Status column for each.

### Known Blocker Sources (The Things This Phase Must Close)

- `.planning/phases/07-v1-cleanup-bookkeeping/07-UAT.md` — the canonical record of the
  three blockers. Tests 2 + 3 are the ones this phase unblocks. `gaps:` section has
  root-cause analysis and suggested fixes for both echo-timestamp and hello-world.
- `.planning/phases/03-read-only-web-ui-health-endpoint/03-HUMAN-UAT.md` — the four
  pending visual UAT items this phase flips to `pass` / `issue`.
- `.planning/phases/06-live-events-metrics-retention-release-engineering/06-VERIFICATION.md`
  § "human_verification" — the OPS-05 + UI-14 items this phase closes via the new
  `06-HUMAN-UAT.md`.

### Prior-Phase Context (Locked Decisions That Must Be Respected)

- `.planning/phases/06-live-events-metrics-retention-release-engineering/06-CONTEXT.md`
  § Decisions — D-11 (example job mix: 1 command + 1 docker in quickstart), D-12
  (`ports: 8080:8080` in quickstart compose), D-13 (README structure). Phase 8 expands
  D-11 to four jobs but must not contradict the intent.
- `.planning/phases/07-v1-cleanup-bookkeeping/07-CONTEXT.md` § Decisions — D-01 + D-02
  (OPS-04 override via 06-VERIFICATION.md overrides block + strengthened compose SECURITY
  comment). The `group_add` language in D-07/D-09 of Phase 8 extends D-02 — do not
  delete the existing SECURITY block, augment it.
- `.planning/phases/01-foundation-security-posture-persistence-base/01-CONTEXT.md`
  § Decisions — the original distroless / nonroot / loopback-by-default security
  posture. Phase 8's rebase to alpine:3 is a conscious walk-back. Document the rationale
  in the final Dockerfile header comment so future-us remembers why.

### Code Files Touched

- `Dockerfile` — runtime stage `FROM` (line 53), apk setup, user creation, `/data`
  ownership (lines 45-50 + 67), `USER` directive (line 70). Builder stage unchanged.
- `examples/cronduit.toml` — rewrite the job section to ship the four example jobs
  in D-15. Keep the existing SECURITY block and `[server]` / `[defaults]` layout.
- `examples/docker-compose.yml` — add `group_add: ["${DOCKER_GID:-999}"]` + update
  the SECURITY block with Linux GID derivation instructions. Keep `ports: 8080:8080`
  (Phase 7 D-01 override still valid).
- **NEW:** `examples/docker-compose.secure.yml` — full dual-compose file with
  `tecnativa/docker-socket-proxy` sidecar.
- `src/scheduler/docker_pull.rs` OR a new `src/scheduler/docker_preflight.rs` — home
  for the `Docker::ping()` call and the `cronduit_docker_reachable` gauge wiring.
  Planner decides which file.
- `src/metrics.rs` (or wherever the Phase 6 metrics are defined) — add the
  `cronduit_docker_reachable` gauge to the existing family.
- `.github/workflows/*.yml` — the `compose-smoke` job definition. Extend the existing
  step; do not add a new job.
- **NEW:** `.planning/BACKLOG.md` — v1.1 backlog seed file (create if missing).
- **NEW:** `.planning/phases/06-live-events-metrics-retention-release-engineering/06-HUMAN-UAT.md`
  — new file for the quickstart + SSE UAT results.
- **NEW:** `.planning/phases/08-v1-final-human-uat-validation/08-HUMAN-UAT.md`
  — Phase 8 index file.

### Project-Level Docs

- `CLAUDE.md` (project root) — locked stack constraints (axum 0.8, askama_web, HTMX
  vendored, mermaid-only diagrams, PR-only workflow). All code changes in Phase 8
  land via feature branch + PR.
- `THREAT_MODEL.md` (if present on `main` after Phase 6) — referenced from the
  SECURITY block in `docker-compose.yml` and from the docker-socket-proxy comment
  in `docker-compose.secure.yml`.

### Research / Precedent

- `tecnativa/docker-socket-proxy` README — env var reference for `CONTAINERS`,
  `IMAGES`, `POST`, etc. Planner consults this to finalize the allowlist in D-10.
- Alpine 3 changelog / `apk add` docs — reference for confirming busybox applets
  (`wget`, `du`, `df`) are present by default and no extra packages are needed beyond
  `ca-certificates` + `tzdata`.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`compose-smoke` CI job** (Phase 6 gap closure) — already builds the cronduit image
  from the PR, rewrites `examples/docker-compose.yml` for the runner workspace, boots
  it, and asserts `/health` + job load. D-18/D-21 extend this job rather than creating
  a new one; the entire orchestration scaffolding is reusable.
- **`POST /api/jobs/{id}/run` (Run Now)** — already wired end-to-end (Phase 3 + 6).
  Smoke test uses it to force immediate execution rather than waiting for the cron
  schedule.
- **`GET /api/jobs/{id}/runs`** — returns run history with status, exit_code, duration.
  Smoke test polls this endpoint; no new API surface needed.
- **`cronduit_run_failures_total{reason=...}`** — the existing Phase 6 D-05 closed
  enum (`image_pull_failed`, `network_target_unavailable`, `timeout`, `exit_nonzero`,
  `abandoned`, `unknown`). Phase 8 must NOT invent new reason strings; the docker
  pre-flight failures surface via `unknown` or through the new `cronduit_docker_reachable`
  gauge, not through this counter.
- **`Docker::ping()` from bollard 0.20** — the exact call the pre-flight check uses.
  Already imported transitively via `src/scheduler/docker_*.rs`.
- **tempfile + unix permissions pattern** in `src/scheduler/script.rs` — used by
  script-type jobs. `disk-usage` (D-15) exercises this path on the alpine runtime;
  the tempfile write / exec / cleanup mechanics stay identical, the shebang just
  needs to resolve (`#!/bin/sh` works on alpine, did not on distroless).
- **`tests/reload_*.rs` harness pattern** — shared scaffold for axum-based integration
  tests. If the planner chooses to add a Rust-side unit test for the pre-flight gauge
  wiring, this is the template.

### Established Patterns

- **Per-phase `*-HUMAN-UAT.md` files use YAML frontmatter** — `status: pending|partial|complete`,
  `phase`, `source[]`, `started`, `updated`. Test entries use `expected:`, `result:`,
  and on issue: `reported:`, `severity:`. Follow `03-HUMAN-UAT.md` and `07-UAT.md`
  verbatim for the new `06-HUMAN-UAT.md` and `08-HUMAN-UAT.md`.
- **Per-phase `*-VERIFICATION.md` overrides block** (Phase 7 D-01) — existing schema
  at `06-VERIFICATION.md` lines 137-143. If Phase 8 needs to record the runtime rebase
  as a deliberate walk-back from the Phase 1 distroless decision, the same `overrides:`
  block shape applies.
- **Dockerfile multi-stage builder + runtime pattern** — Phase 1 established
  `rust:1.94-slim-bookworm` builder → minimal runtime. Phase 8 keeps the builder stage
  identical (only the runtime `FROM` changes) so cross-compile and cargo caches are
  unaffected.
- **Structured `tracing` logging at INFO / WARN / ERROR** with `target:` fields —
  the pre-flight check must match existing log style (e.g., `target: "cronduit.scheduler"`
  or `"cronduit.docker"`).
- **No direct commits to `main`** — every Phase 8 change lands via feature branch + PR.
  Per `CLAUDE.md` and user feedback memory.
- **Diagrams must be mermaid** — if the planner adds any diagram to plan docs or code
  comments, it must be a mermaid code block. No ASCII art.

### Integration Points

- **Runtime image ↔ `examples/cronduit.toml`** — the alpine rebase (D-01) is what
  makes the new example jobs executable. The two decisions are one commit.
- **`cronduit_docker_reachable` gauge ↔ Phase 6 metrics family** — the new gauge
  registers alongside `cronduit_runs_total`, `cronduit_run_duration_seconds`,
  `cronduit_run_failures_total`, `cronduit_active_subscribers`, and the eagerly-described
  families from Phase 6 GAP-1 closure. Must be described in the same startup log line.
- **`compose-smoke` CI job ↔ both compose files** — the dual-file matrix (D-21) means
  the CI workflow needs a new matrix axis. Existing `compose-smoke` is single-file;
  refactor it to parameterize on `compose` rather than hard-coding `docker-compose.yml`.
- **Pre-flight WARN + gauge ↔ alerting story** — operators can write a Prometheus rule
  like `cronduit_docker_reachable == 0` and fire an alert. The alpine runtime rebase
  + dual compose files + pre-flight ping together form the "docker is optional but
  observable" contract.
- **`07-UAT.md` Test 2 ↔ the new `http-healthcheck` / `disk-usage` example jobs** —
  Test 2 needs a job that stays in RUNNING state long enough to observe the HTMX
  `every 2s` polling transition. `http-healthcheck` (wget over the network) and
  `disk-usage` (`du -sh` on a data volume) both satisfy this naturally without any
  artificial `sleep`.

</code_context>

<specifics>
## Specific Ideas

- **Alpine runtime user numeric UID.** Use UID/GID `1000:1000` for the `cronduit`
  user. Matches the conventional "first non-system user" on Linux and plays nicely
  with bind-mounted volumes from host users who are also UID 1000.

- **DOCKER_GID default fallback.** In `docker-compose.yml`, use `${DOCKER_GID:-999}`.
  On Debian/Ubuntu the docker group is usually GID 998 or 999; the former is safe-ish
  as a default but users should override via `.env` or the environment. The SECURITY
  comment must show `stat -c %g /var/run/docker.sock` as the canonical lookup.

- **docker-socket-proxy minimal allowlist.** The comment block in
  `docker-compose.secure.yml` should explicitly list why each permission is granted:
  `CONTAINERS=1` (create/start/wait/remove), `IMAGES=1` (pull), `POST=1` (container
  create is a POST). No `NETWORKS`, no `VOLUMES`, no `EXEC`, no `INFO` beyond minimum.
  A curious operator reading the file should understand the threat boundary.

- **The echo-timestamp command text.** `date '+%Y-%m-%d %H:%M:%S -- Cronduit is running!'`
  — preserve the exact current string from `examples/cronduit.toml` line 40 so the UAT
  "heartbeat" screenshots stay recognizable between Phase 6 and Phase 8.

- **http-healthcheck target URL.** Use `https://example.com` — the IANA reserved example
  domain, stable, minimal, no ethical issue with automated probing. Not `google.com`,
  not a Cronduit-hosted endpoint.

- **disk-usage script body.** Must handle the "/data not mounted" case cleanly so local
  dev runs without the compose volume don't silently fail. `du -sh /data 2>/dev/null ||
  echo "/data not mounted"` gives a meaningful log line either way.

- **Pre-flight WARN text template.**
  `docker daemon unreachable at {uri}: {error}. cronduit will continue to schedule command/script jobs.
  docker jobs will fail until the daemon is reachable. remediation: verify /var/run/docker.sock is
  mounted, check group_add / DOCKER_GID in docker-compose.yml, or switch to examples/docker-compose.secure.yml
  on macOS / Docker Desktop.`
  Single line, under ~280 chars so it stays grep-friendly in log collectors.

- **Smoke test polling cadence.** 2-second poll interval, 120-second budget = 60 iterations
  max per job. Matches the HTMX `every 2s` cadence used in the UI polling, so what CI
  asserts is the same behavior a user sees.

- **Naming: `cronduit_docker_reachable`.** Gauge name follows the existing Phase 6
  naming convention (`cronduit_*`). Singular `reachable`, not `is_reachable` or
  `reachability_status`, to match Prometheus idioms.

- **Reusing `07-UAT.md` vs creating `07-HUMAN-UAT.md`.** Do NOT create a new file —
  `07-UAT.md` already has Test 2 / Test 3 in the canonical shape with `result: issue`
  / `result: blocked`. Phase 8 edits those rows in place and adds a one-line
  `re_tested_at: 2026-04-13T..Z` annotation so the audit trail shows the retry.

</specifics>

<deferred>
## Deferred Ideas

- **Multi-arch verification of socket-proxy.** `tecnativa/docker-socket-proxy` supports
  `linux/amd64` and `linux/arm64`, but Phase 8's smoke test only exercises whichever
  arch the CI runner uses. Full arm64 validation of the secure compose file is a v1.1
  nice-to-have if anyone reports issues.

- **Full `docker-compose.secure.yml` + reverse-proxy example.** The `docker-compose.secure.yml`
  file uses socket-proxy but still exposes cronduit on `ports: 8080:8080`. A true
  production example would also front it with Traefik or Caddy + basic auth. Out of
  scope for v1.0 — Phase 6 D-12 already decided the quickstart stays `ports:` and auth
  is a v2 feature.

- **`cronduit_docker_reachable` with a latency histogram.** Just the gauge for v1.0;
  a `cronduit_docker_ping_duration_seconds` histogram would be nicer for tracking
  daemon slowness but adds a metric family late in the cycle. v1.1.

- **Migration note for existing users moving off distroless.** Operators running
  `ghcr.io/simplicityguy/cronduit:0.1.0` (Phase 6 tag) and upgrading to the Phase 8
  alpine build will inherit the UID change (65532 → 1000). Their existing
  `cronduit-data` named volume was chown'd to 65532 on first mount. A RELEASE-NOTES.md
  or CHANGELOG entry needs to call this out with the `docker run --user 0:0 alpine
  chown -R 1000:1000 /data` one-liner. The entry itself belongs in the release PR's
  description + a CHANGELOG.md update — handle inside the phase since it's a direct
  consequence of D-02, but if the planner needs to cut scope, punt to v1.1 and flag
  in the release notes only.

- **Audit-milestone v1.0 re-run.** After Phase 8 closes, the final step before archive
  is re-running `/gsd-audit-milestone` to confirm all 86 requirements are SATISFIED.
  That's a milestone-level action, not a Phase 8 task.

- **v1.1 backlog entries surfaced during the walkthrough.** By definition these live
  in `.planning/BACKLOG.md` and are not deferred — they become part of the v1.1 milestone
  scope. This section exists only to hold ideas that are NOT backlog-worthy (e.g.,
  "maybe we should have a completely different CI strategy").

- **`06-VERIFICATION.md` / `07-VERIFICATION.md` re_verification annotations.** If
  audit-milestone requires these files to flip from `human_needed` to `code_complete,
  human_verified`, the planner can add a small annotation task. Otherwise the per-phase
  UAT files are the canonical record and the verification files stay as-is. Decide
  during planning, not now.

### Reviewed Todos (not folded)

None — `todo match-phase 8` returned 0 matches.

</deferred>

---

*Phase: 08-v1-final-human-uat-validation*
*Context gathered: 2026-04-13*
