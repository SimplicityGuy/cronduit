---
phase: 06-live-events-metrics-retention-release-engineering
verified: 2026-04-13T00:00:00Z
status: passed
score: 5/5 wave-1 must-haves verified
overrides_applied: 1
re_verification:
  previous_status: human_needed
  previous_score: 4/5
  wave_1_plans:
    - 06-06  # GAP-1 metrics-describe + GAP-2 retention startup log
    - 06-07  # GAP-3 quickstart revert + .gitignore + compose-smoke CI
  gaps_closed:
    - "GAP-1: /metrics did not render cronduit_* families from boot (OPS-02)"
    - "GAP-2: retention_pruner emitted no startup log on cronduit.retention target (DB-08)"
    - "GAP-3: examples/cronduit.toml bind, docker-compose mount, DATABASE_URL regressions (OPS-04 subset)"
    - "GAP-3.4: quickstart has no CI smoke gate (OPS-04 infrastructure)"
  gaps_remaining:
    - "OPS-04 ports: vs expose: deviation — deferred to Phase 7 (v1.0 Cleanup)"
    - "OPS-05 stranger-under-5-minute quickstart human UAT — deferred to Phase 8 (v1.0 Final Human UAT)"
  regressions: []
requirements_satisfied:
  - UI-14  # SSE streaming (verified in original 06-VERIFICATION run)
  - OPS-02 # /metrics families now described from boot (GAP-1 closed)
  - DB-08  # Retention pruner startup log (GAP-2 closed)
requirements_deferred:
  - requirement: OPS-04
    addressed_in: Phase 7 (v1.0 Cleanup & Bookkeeping)
    reason: "ports: vs expose: deviation was an intentional quickstart-accessibility choice (Plan 04 D-12); Phase 7 Plan 01 owns the overrides block in VERIFICATION frontmatter + strengthened compose top-of-file SECURITY comment wired to THREAT_MODEL.md"
    evidence: ".planning/REQUIREMENTS.md line 125 reassigns OPS-04 to Phase 7 gap closure"
  - requirement: OPS-05
    addressed_in: Phase 8 (v1.0 Final Human UAT Validation)
    reason: "The 'stranger under 5 minutes' contract requires a human to run docker compose up on a fresh clone with Docker daemon + GHCR pull; Phase 8 owns the final human UAT pass"
    evidence: ".planning/REQUIREMENTS.md line 126 reassigns OPS-05 to Phase 8 gap closure"
overrides:
  - must_have: "example docker-compose.yml uses expose: (not ports:) for the web UI"
    reason: "Phase 6 Plan 04 D-12 explicitly chose ports: 8080:8080 for quickstart accessibility so a stranger running `docker compose up` reaches the web UI at http://localhost:8080 immediately without any additional configuration; this directly backs the OPS-05 5-minute quickstart promise in the ROADMAP. The file ships with a prominent SECURITY comment block (strengthened in Phase 7 D-02) that warns about the unauthenticated v1 UI, references THREAT_MODEL.md, and shows an exact expose: replacement snippet for production deployments behind a reverse proxy. The deviation is intentional and fully documented in-place."
    accepted_by: "SimplicityGuy"
    accepted_at: "2026-04-13T20:45:03Z"
---

# Phase 6: Live Events, Metrics, Retention & Release Engineering — Gap Closure Re-Verification

**Phase Goal:** Turn the feature-complete binary into a shippable public OSS release: SSE log tail for in-progress runs, Prometheus `/metrics` with a bounded-cardinality label set, daily retention pruner, multi-arch Docker image, complete `THREAT_MODEL.md`, and a README quickstart that takes a stranger from `git clone` to a working scheduled job in under 5 minutes.

**Verified:** 2026-04-13T00:00:00Z
**Status:** passed (wave 1 scope — two explicit deferrals tracked below)
**Re-verification:** Yes — after wave-1 gap closure (plans 06-06 and 06-07)

---

## Re-verification Scope

The original `06-VERIFICATION.md` (2026-04-12) reported `status: human_needed`, `4/5` must-haves, with one gap (`ports: vs expose:`) and two human-UAT items. The subsequent Phase 6 UAT pass (`06-UAT.md`) surfaced three more issues that the user flagged as blocking final UAT sign-off:

| Gap ID | Severity | Original UAT test | Fix plan |
| ------ | -------- | ----------------- | -------- |
| GAP-1  | MAJOR    | UAT 2 — `/metrics` missing cronduit families from boot | 06-06 |
| GAP-2  | MINOR    | UAT 7 — retention pruner silent until first 24h tick | 06-06 |
| GAP-3  | BLOCKER  | UAT 4 — working-tree regressions broke quickstart | 06-07 (revert) |
| GAP-3.4 | Infrastructure | (new) — no CI smoke gate for quickstart | 06-07 (compose-smoke) |
| GAP-4  | Policy   | `overrides:` block + strengthened compose SECURITY comment | **Deferred to Phase 7 Plan 01** |

Wave 1 executed plans `06-06` and `06-07` in parallel; this re-verification checks their must-haves against the merged `gap-closure/phase-06-wave-1` branch and confirms the explicit deferrals for OPS-04 and OPS-05.

---

## Must-Have Verification — Plan 06-06 (metrics-describe + retention-log)

### Observable Truths

| # | Truth | Status | Evidence |
| - | ----- | ------ | -------- |
| 1 | After `cargo test --test metrics_endpoint metrics_families_described_from_boot`, the test exits 0 and asserts `/metrics` renders HELP/TYPE lines for all five cronduit metric families (`cronduit_scheduler_up`, `cronduit_jobs_total`, `cronduit_runs_total`, `cronduit_run_duration_seconds`, `cronduit_run_failures_total`) before any job run or sync. | VERIFIED | `cargo test --test metrics_endpoint metrics_families_described_from_boot` → `1 passed; 0 failed; 0 ignored` in 0.20s. Body assertions cover all five families' HELP and TYPE lines directly via `telemetry::setup_metrics()` → `handle.render()` (no AppState, no axum harness). |
| 2 | After `cargo test --test retention_integration retention_pruner_emits_startup_log_on_spawn`, the test exits 0 and asserts `retention_pruner()` emits exactly one `tracing::info!` line on target `cronduit.retention` with message `retention pruner started` before the 24h interval loop. | VERIFIED | `cargo test --test retention_integration retention_pruner_emits_startup_log_on_spawn` → `1 passed; 0 failed; 0 ignored` in 0.05s. `CapturedWriter` + `WithSubscriber` pattern captures the emission within 50 ms of spawn, cancels the token, and asserts both `cronduit.retention` and `retention pruner started` are in the buffered output. |

### Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| `src/telemetry.rs` | `setup_metrics()` eagerly describes + registers all five families after `install_recorder()` | VERIFIED | Lines 82-112: five `describe_*!` calls followed by paired zero-valued `gauge!().set(0.0)` / `counter!().increment(0)` / `histogram!().record(0.0)` registrations. Code comment at line 66-81 documents the `describe_*` alone is insufficient in metrics-exporter-prometheus 0.18 (decision captured in `06-06-SUMMARY.md`). |
| `src/scheduler/retention.rs` | startup `tracing::info!` on target `cronduit.retention` before the interval loop | VERIFIED | Lines 18-25: `tracing::info!(target: "cronduit.retention", retention_secs = retention.as_secs(), "retention pruner started")` placed above `let mut interval = ...` (line 27). The original `run_prune_cycle` log line at line 49 is unchanged. |
| `tests/metrics_endpoint.rs` | Real integration test asserting all five families' HELP/TYPE from boot | VERIFIED | Lines 16-70: `metrics_families_described_from_boot` is a non-`todo!()` `#[test]`. It installs the recorder, calls `handle.render()`, asserts HELP and TYPE lines for each of the five families. Three `#[ignore]` stubs remain as forward-looking placeholders (intentional per Wave-0 Nyquist). |
| `tests/retention_integration.rs` | Real integration test capturing tracing output to assert startup log | VERIFIED | Lines 46-98: `retention_pruner_emits_startup_log_on_spawn` builds a `CapturedWriter` subscriber, attaches it to the pruner future via `WithSubscriber`, spawns the pruner, cancels after 50ms, then asserts both `cronduit.retention` and `retention pruner started` appear in the captured buffer. Uses `DbPool::connect("sqlite::memory:")`. Five `#[ignore]` stubs remain as intentional Wave-0 placeholders. |

### Key Links

| From | To | Via | Status |
| ---- | -- | --- | ------ |
| `src/telemetry.rs::setup_metrics` | `metrics-exporter-prometheus` render output | `describe_*!` + paired zero-valued registration macro calls after `install_recorder()` | WIRED — line-level match on pattern `describe_gauge!\(\s*"cronduit_jobs_total"` and `gauge!("cronduit_jobs_total").set(0.0)` |
| `src/scheduler/retention.rs::retention_pruner` | tracing subscriber on target `cronduit.retention` | `tracing::info!` macro called before `let mut interval = ...` | WIRED — literal `"retention pruner started"` present at line 24, above the interval init at line 27 |
| `tests/retention_integration.rs` | `cronduit::scheduler::retention::retention_pruner` | Direct library call with in-memory sqlite pool + future-attached subscriber | WIRED — `use cronduit::scheduler::retention::retention_pruner` at line 18 and direct invocation at line 71 |
| `tests/metrics_endpoint.rs` | `cronduit::telemetry::setup_metrics` | Direct library call returning `PrometheusHandle` whose `render()` is asserted | WIRED — `use cronduit::telemetry` at line 6 and `telemetry::setup_metrics()` at line 21 |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| Integration test for metrics describe-from-boot passes | `cargo test --test metrics_endpoint metrics_families_described_from_boot` | `1 passed; 0 failed` in 0.20s | PASS |
| Integration test for retention startup log passes | `cargo test --test retention_integration retention_pruner_emits_startup_log_on_spawn` | `1 passed; 0 failed` in 0.05s | PASS |

---

## Must-Have Verification — Plan 06-07 (quickstart revert + compose-smoke)

### Observable Truths

| # | Truth | Status | Evidence |
| - | ----- | ------ | -------- |
| 3 | After `docker compose -f examples/docker-compose.yml up -d` on a fresh checkout, `curl -sSf http://localhost:8080/health` returns 200 within 30s and logs show `job_count=2` (echo-timestamp + hello-world). | VERIFIED (static checks) | `examples/cronduit.toml` line 16 has active `bind = "0.0.0.0:8080"` (no `#` prefix); `examples/docker-compose.yml` mounts `./cronduit.toml:/etc/cronduit/config.toml:ro` (line 18) and sets `DATABASE_URL=sqlite:///data/cronduit.db` in the environment (line 22); both quickstart jobs (`echo-timestamp` `*/1 * * * *` and `hello-world` `*/5 * * * *`) are present in `examples/cronduit.toml` lines 37-51. `git diff 0c9ceb4..HEAD -- examples/cronduit.toml examples/docker-compose.yml` is empty — files are byte-identical to the shipped commit. Full end-to-end container run requires Docker + is exercised by the new `compose-smoke` CI job on every PR. |
| 4 | `examples/cronduit-uat.toml` does not exist and `.gitignore` contains a pattern preventing it from being re-committed. | VERIFIED | `test -f examples/cronduit-uat.toml` → ABSENT. `git check-ignore -v examples/cronduit-uat.toml` → `.gitignore:93:examples/*-uat.toml	examples/cronduit-uat.toml` (pattern matches). Line 93 of `.gitignore` contains `examples/*-uat.toml` with a preceding explanatory comment block referencing the GAP-3 regression. |
| 5 | `.github/workflows/ci.yml` contains a `compose-smoke` job that builds `cronduit:ci` locally, rewrites the compose image in-place in the runner workspace, brings up the stack, polls `/health`, and asserts both quickstart jobs load. | VERIFIED | `.github/workflows/ci.yml` lines 110-200: new `compose-smoke` job contains `docker/setup-buildx-action@v3`, `docker/build-push-action@v6` with `load: true` / `push: false` / `tags: cronduit:ci`, a `sed -i 's\|ghcr.io/simplicityguy/cronduit:latest\|cronduit:ci\|g' examples/docker-compose.yml` rewrite (line 143), `docker compose -f docker-compose.yml up -d` (line 151), a 30s curl-loop on `/health` (lines 153-166), a `status:ok` assertion (lines 168-176), dashboard assertions for `echo-timestamp` and `hello-world` (lines 178-191), a `if: failure()` log dump (lines 193-195), and an `if: always()` tear-down (lines 197-200). |

### Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| `examples/cronduit.toml` | active (non-commented) `bind = "0.0.0.0:8080"` under `[server]`, both quickstart jobs | VERIFIED | Line 16 `bind = "0.0.0.0:8080"` active; `grep -c '^#bind' examples/cronduit.toml` = 0; byte-identical to `0c9ceb4:examples/cronduit.toml`. Contains both `[[jobs]]` blocks for `echo-timestamp` (line 37-40) and `hello-world` (line 47-51). |
| `examples/docker-compose.yml` | `./cronduit.toml` mount, `DATABASE_URL` env, still references `ghcr.io/simplicityguy/cronduit:latest` | VERIFIED | Line 13 `image: ghcr.io/simplicityguy/cronduit:latest`; line 18 `- ./cronduit.toml:/etc/cronduit/config.toml:ro`; line 22 `- DATABASE_URL=sqlite:///data/cronduit.db`; byte-identical to `0c9ceb4:examples/docker-compose.yml`. `grep -c 'cronduit-uat.toml'` = 0. Top-of-file SECURITY comment block (lines 1-9) preserved exactly for Phase 7 Plan 01 to strengthen. |
| `.gitignore` | pattern matching `examples/*-uat.toml` | VERIFIED | Line 93 `examples/*-uat.toml` in the Cronduit section (lines 79-93); preceded by an explanatory comment block at lines 89-92 documenting the GAP-3 regression. `git check-ignore` confirms matching works. |
| `.github/workflows/ci.yml` | `compose-smoke` job that builds locally, rewrites compose, asserts `/health` + jobs | VERIFIED | Job block at lines 110-200 with all required steps in order; no `needs:` dependency so the job runs in parallel with `lint` / `test` / `image`; `push: false, load: true, tags: cronduit:ci` ensures PR code is exercised, not stale `:latest`. |

### Key Links

| From | To | Via | Status |
| ---- | -- | --- | ------ |
| `examples/docker-compose.yml` | `examples/cronduit.toml` | read-only bind mount `./cronduit.toml:/etc/cronduit/config.toml:ro` | WIRED |
| `.github/workflows/ci.yml::compose-smoke` | `examples/docker-compose.yml` | `docker buildx build` → `sed` rewrite → `docker compose up -d` → `curl /health` | WIRED (all five pipeline steps present in declared order) |
| `.github/workflows/ci.yml::compose-smoke` | PR source code | `docker/build-push-action@v6` with `context: .` + `load: true` producing `cronduit:ci` locally, then sed rewrite in runner workspace | WIRED — lines 124-134 show the build + line 143 shows the sed rewrite |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
| -------- | ------- | ------ | ------ |
| Examples match shipped state (commit `0c9ceb4`) | `git diff 0c9ceb4..HEAD -- examples/cronduit.toml examples/docker-compose.yml` | empty diff | PASS |
| Scratch UAT file absent from working tree | `test -f examples/cronduit-uat.toml` | ABSENT | PASS |
| Scratch UAT file pattern gitignored | `git check-ignore -v examples/cronduit-uat.toml` | `.gitignore:93:examples/*-uat.toml` | PASS |
| Active bind in quickstart config | `grep -c '^bind = "0.0.0.0:8080"$' examples/cronduit.toml` | 1 | PASS |
| Compose mount points at shipped config | `grep -c './cronduit.toml:/etc/cronduit/config.toml:ro' examples/docker-compose.yml` | 1 | PASS |
| Compose env carries DATABASE_URL | `grep -c 'DATABASE_URL=sqlite:///data/cronduit.db' examples/docker-compose.yml` | 1 | PASS |
| compose-smoke job header + cronduit:ci tag + sed rewrite present | `grep -c 'compose-smoke' && grep -c 'cronduit:ci' && grep -c "sed -i 's|ghcr.io" .github/workflows/ci.yml` | all positive | PASS |
| End-to-end `docker compose up` + `/health` poll | CI `compose-smoke` job (not run locally — requires Docker daemon) | gated in CI | DEFERRED to CI green |

---

## Requirements Coverage

| Requirement | Description | Status | Evidence |
| ----------- | ----------- | ------ | -------- |
| UI-14 | Run Detail page log viewer streams new lines via SSE for in-progress runs | SATISFIED | Verified in the original `06-VERIFICATION.md` run (Truth 1, all artifacts + key links WIRED). No changes in wave 1. |
| OPS-02 | `GET /metrics` exposes Prometheus metrics for all five cronduit families with bounded-cardinality labels | SATISFIED (gap closed) | `src/telemetry.rs::setup_metrics` now describes + registers all five families at boot; `tests/metrics_endpoint.rs::metrics_families_described_from_boot` passes (0.20s). `FailureReason` closed enum was already verified in the original run. |
| DB-08 | Daily retention pruner deletes `job_runs`/`job_logs` older than `[server].log_retention` in batched transactions | SATISFIED (gap closed) | `src/scheduler/retention.rs::retention_pruner` now emits startup log on target `cronduit.retention`; `tests/retention_integration.rs::retention_pruner_emits_startup_log_on_spawn` passes (0.05s). Batching / FK ordering / WAL checkpoint / spawn wiring all previously verified. |
| OPS-04 | Example `docker-compose.yml` with Docker socket, read-only config mount, named volume for SQLite | DEFERRED | `.planning/REQUIREMENTS.md` line 125 reassigns OPS-04 to Phase 7 gap closure. `ports: vs expose:` deviation is an intentional quickstart-accessibility choice (Plan 04 D-12); Phase 7 Plan 01 owns the `overrides:` frontmatter block plus the strengthened compose top-of-file SECURITY comment wired to `THREAT_MODEL.md`. |
| OPS-05 | README quickstart: stranger clones, runs `docker compose up`, schedules a working job in under 5 minutes | DEFERRED | `.planning/REQUIREMENTS.md` line 126 reassigns OPS-05 to Phase 8 gap closure. Validation requires a live human running the quickstart on a fresh clone with Docker daemon + GHCR pull; Phase 8 owns the final human UAT pass. Wave 1 adds the `compose-smoke` CI gate so regressions to the quickstart contract are caught programmatically on every PR. |

---

## Anti-Patterns Scan

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| `tests/metrics_endpoint.rs` | 84, 93, 105 | `todo!()` (3 cases) | Info | Intentional Wave-0 Nyquist placeholders, all `#[ignore]`d. The one test promised by plan 06-06 (`metrics_families_described_from_boot`) is now real. |
| `tests/retention_integration.rs` | 110, 119, 128, 137, 146 | `todo!()` (5 cases) | Info | Intentional Wave-0 Nyquist placeholders, all `#[ignore]`d. The one test promised by plan 06-06 (`retention_pruner_emits_startup_log_on_spawn`) is now real. |
| `tests/sse_streaming.rs` | — | `todo!()` stubs | Info | Unchanged from original verification; intentional Nyquist placeholders. |

No blocking anti-patterns. The surviving `todo!()` stubs are all gated behind `#[ignore]` so `cargo test` never panics on them.

---

## Deferred Items (Addressed in Later Milestone Phases)

Two items remain outside wave-1 scope; both are explicitly tracked in `.planning/REQUIREMENTS.md` and in the v1.0 milestone roadmap.

| # | Item | Addressed In | Evidence |
| - | ---- | ------------ | -------- |
| 1 | OPS-04 `ports: vs expose:` deviation + strengthened compose SECURITY comment wired to `THREAT_MODEL.md` | Phase 7 (v1.0 Cleanup & Bookkeeping), Plan 07-01 | `.planning/REQUIREMENTS.md` line 125; `06-07-SUMMARY.md` "Out of Scope" section confirms Plan 07-01 owns D-01 (frontmatter `overrides:` block) and D-02 (strengthened comment). The committed compose top-of-file comment was intentionally preserved byte-identical to `HEAD` to prevent merge conflicts. |
| 2 | OPS-05 "stranger under 5 minutes" human UAT validation | Phase 8 (v1.0 Final Human UAT Validation) | `.planning/REQUIREMENTS.md` line 126 reassigns to Phase 8. Phase 8 owns `06-UAT.md` Test 4 final sign-off. The compose-smoke CI job ships in wave 1 as the automatable guard; the human contract remains Phase 8's scope. |

**These deferrals do not affect wave-1 status.** The explicit handoffs are documented in REQUIREMENTS.md, the v1.0 milestone roadmap, and this frontmatter.

---

## Human Verification Required

None for wave-1 scope. The two human-verification items carried over from the original `06-VERIFICATION.md` (SSE live-log streaming + stranger quickstart end-to-end) are now:

- SSE live-log streaming → tracked under Phase 8 final human UAT (already verified programmatically in original run; the human test validates visual/real-time behavior).
- Stranger quickstart → DEFERRED to Phase 8 as OPS-05 (see Deferred Items above).

The `compose-smoke` CI job added in plan 06-07 converts the previously-human quickstart test into an automated PR-gated check that exercises the PR's code (not stale `:latest`), so any regression to the committed `examples/cronduit.toml` / `examples/docker-compose.yml` will fail CI before it reaches main.

---

## Summary

Wave 1 gap closure is complete. All 5 must-haves across plans 06-06 and 06-07 verify cleanly:

- **Plan 06-06** (GAP-1 + GAP-2): `src/telemetry.rs` eagerly describes + registers all five cronduit metric families at boot; `src/scheduler/retention.rs::retention_pruner` emits a boot-time `tracing::info!` on target `cronduit.retention` before the interval loop; both behaviors are gated by real runtime integration tests that replaced prior `todo!()` stubs. Both tests pass locally (0.20s + 0.05s).
- **Plan 06-07** (GAP-3 + GAP-3.4): `examples/cronduit.toml` and `examples/docker-compose.yml` are byte-identical to the shipped `0c9ceb4` state; `examples/cronduit-uat.toml` is absent and gitignored via `examples/*-uat.toml`; `.github/workflows/ci.yml` has a new `compose-smoke` job that builds `cronduit:ci` locally from the PR checkout, rewrites the compose image in the runner workspace, asserts `/health` returns `status:ok`, and asserts both quickstart jobs load on the dashboard. The committed compose file still points at `ghcr.io/simplicityguy/cronduit:latest` for end-user quickstart.

Two items remain deferred by explicit roadmap reassignment (OPS-04 → Phase 7, OPS-05 → Phase 8). These are not gaps — they are known milestone handoffs.

**Status: passed** (wave-1 scope; deferrals tracked in frontmatter and REQUIREMENTS.md).

---

_Re-verified: 2026-04-13T00:00:00Z_
_Verifier: Claude (gsd-verifier)_
_Branch: gap-closure/phase-06-wave-1_
