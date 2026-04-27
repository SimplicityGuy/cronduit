# Phase 12: Docker Healthcheck + rc.1 Cut - Context

**Gathered:** 2026-04-17
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 12 delivers the first `v1.1.0-rc.N` tag (`v1.1.0-rc.1`) by shipping the long-standing `(unhealthy)` fix and wiring an out-of-the-box Docker healthcheck:

1. **New `cronduit health` CLI subcommand** — performs a local HTTP GET against `/health`, parses the JSON, exits 0 only when BOTH the HTTP status is 200 AND the response body's `status` field equals `"ok"`. Fails fast on connection-refused (no retries; the Docker HEALTHCHECK owns retry policy). Target URL resolved from the existing global `--bind host:port` flag (with `http://` prepended) or defaults to `http://127.0.0.1:8080` (OPS-06).
2. **Dockerfile `HEALTHCHECK`** — add `HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 CMD ["/cronduit", "health"]`. Operator `healthcheck:` stanzas in their own compose files continue to win (Dockerfile < compose override), verified by a dedicated compose-smoke CI workflow (OPS-07).
3. **OPS-08 root-cause reproduction** — the `(unhealthy)` symptom (busybox `wget --spider` misparsing axum chunked responses) is reproduced in an automated CI test (OLD `wget --spider` healthcheck → `unhealthy`; NEW `cronduit health` healthcheck → `healthy`) before the fix is declared complete.
4. **`v1.1.0-rc.1` tag cut** — patch `.github/workflows/release.yml` so pre-release tags (`v*-rc.*`) do NOT bump `:latest`, `:major`, or `:major.minor`, and DO push a rolling `:rc` tag. Maintainer cuts the tag per a new `docs/release-rc.md` runbook; existing release workflow builds multi-arch, publishes GitHub Release as prerelease.

**Out of scope (deferred to other phases):**
- Authenticated health probe / TLS readiness for `cronduit health` (v1 is localhost-only; `--bind` shape forward-compatible).
- `/healthz` "starting" state during migrations (Phase 11 D-12 blocks the listener until backfill completes; `--start-period=60s` covers the gap).
- Observability polish (timeline, sparkline, p50/p95) — Phase 13.
- Bulk enable/disable — Phase 14.
- Final `v1.1.0` GA promotion — Phase 14 close-out.

</domain>

<decisions>
## Implementation Decisions

### `cronduit health` CLI

- **D-01:** **HTTP client: `hyper 1` + `hyper-util`.** Reuses `hyper = "1"` already declared in `Cargo.toml` (currently unused directly; transitive via axum). Adds `hyper-util = { version = "0.1", features = ["client-legacy", "http1", "tokio"] }` for the client factory + `HttpConnector`. Runs inside the already-booted tokio runtime (`#[tokio::main]` in `src/main.rs`). No new rustls surface because the probe is HTTP-only (loopback). Response body read into `Bytes` then decoded via the existing `serde_json` dep. Future HTTPS extension point documented but not implemented. Zero-dep raw TCP and `reqwest` rejected (respectively: ironic — OPS-08 is a bug caused by hand-rolled HTTP parsing; overkill — ~40 transitive crates for one localhost GET).

- **D-02:** **Internal timeout: 2s connect + 3s read (5s total upper bound), enforced via `tokio::time::timeout`.** Headroom under the Dockerfile `HEALTHCHECK --timeout=5s`. Gives `cronduit health` a deterministic exit code even when invoked outside Docker (e.g., on the host during debugging). Connection-refused returns instantly either way. Aggressive 1s total rejected: can flap under heavy SSE subscriber load without meaningfully improving MTTR.

- **D-03:** **Arg shape: reuse the existing global `--bind host:port` flag.** No new subcommand-local `--url` flag in v1.1. Parser prepends `http://` to construct the target URL. Default when absent: `http://127.0.0.1:8080`. IPv6 bracketed form (`[::1]:8080`) is preserved by `url::Url::parse` / the clap global flag semantics. Consistent with `run --bind`; no second address-input shape in the CLI. Future HTTPS support lands additively via `--url` or `--tls` without breaking existing invocations.

- **D-04:** **`cronduit health` does NOT read `--config`.** No TOML parsing in the health path. Operators who bind on a non-default port either pass `--bind` in their compose healthcheck override or accept the Dockerfile default. Keeps probe startup to milliseconds and removes config-parse / config-missing as health failure modes. Auto-loading `/etc/cronduit/config.toml` (mirroring `run`) explicitly rejected: it would make the 30s-interval probe sensitive to config I/O.

- **D-05:** **Exit code contract:** `0` iff HTTP response is 200 AND body parses as JSON AND `body.status == "ok"`. `1` on connection-refused, DNS failure, timeout, non-200 status, unparseable body, or `body.status != "ok"`. A single non-zero code for all failure modes is sufficient for Docker's HEALTHCHECK semantics; detailed reason is logged to stderr at `--log-format=text` for human debugging but not distinguished numerically.

### Dockerfile HEALTHCHECK

- **D-06:** **Add `HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 CMD ["/cronduit", "health"]`** to the runtime stage of `Dockerfile` (after `USER cronduit:cronduit`, before `CMD ["run", ...]`). The 60s start-period matches Phase 11 D-12 where the HTTP listener binds only after migration backfill completes — a large `job_runs` backfill on first-boot of an upgraded deployment must not mark the container unhealthy. Operator compose `healthcheck:` stanzas still override (backward compatible; verified by D-09 smoke test).

- **D-07:** **busybox `wget` stays installed** (alpine base image ships it as part of busybox). The HEALTHCHECK no longer uses it, but jobs authored as `type = "command"` with `wget` calls in user config continue to work. No apk add / remove changes from Phase 8's alpine rebase.

### OPS-08 Root-Cause Reproduction

- **D-08:** **Automated repro test in dedicated CI workflow.** Success Criterion #4 demands reproduction before the fix is declared complete. Test procedure: (a) build the cronduit image with the OLD `HEALTHCHECK CMD ["wget", "--spider", "http://localhost:8080/health"]` baked in via a temporary Dockerfile, (b) run it, wait for start-period, assert `docker inspect --format '{{.State.Health.Status}}'` == `unhealthy`, (c) build the cronduit image with the NEW `HEALTHCHECK CMD ["/cronduit", "health"]`, (d) run it, wait for start-period, assert `healthy`. Documented repro per ROADMAP.md — "The reported `(unhealthy)` root cause is reproduced in a test environment before the fix is declared complete."

- **D-09:** **Test lives in new `.github/workflows/compose-smoke.yml`.** Dedicated GitHub Actions workflow runs (1) the OPS-08 before/after repro (D-08), (2) a compose-override smoke test that layers a custom `healthcheck:` stanza in a test compose file and asserts compose wins over Dockerfile (Success Criterion #3). Runs on PR + main + release tags. Standalone so unit tests stay fast; not gated behind the `integration` feature because it exercises `docker` / `docker compose` directly, not `testcontainers`. The existing `ci.yml` is NOT extended — compose smoke is a separate concern with its own runner setup.

### rc.1 Release Mechanics

- **D-10:** **Patch `.github/workflows/release.yml` for pre-release tag semantics.** Required patches via `docker/metadata-action` tag conditions:
  - `type=semver,pattern={{version}}` — keep (always on; pushes `:1.1.0-rc.1` for pre-releases and `:1.1.0` for final).
  - `type=semver,pattern={{major}}.{{minor}}` — add `enable=${{ !contains(github.ref, '-') }}` (skip on pre-release).
  - `type=semver,pattern={{major}}` — add `enable=${{ !contains(github.ref, '-') }}` (skip on pre-release).
  - `type=raw,value=latest` — add `enable=${{ !contains(github.ref, '-') }}` (skip on pre-release).
  - `type=raw,value=rc` — NEW; add `enable=${{ contains(github.ref, '-rc.') }}` (pushes rolling `:rc` tag for any `*-rc.*` tag).
  Existing `prerelease: ${{ contains(steps.version.outputs.version, '-') }}` on `softprops/action-gh-release` already routes GitHub Release correctly — no change.

- **D-11:** **Ship `docs/release-rc.md` runbook.** Reusable by rc.2 (Phase 13) and rc.3 (Phase 14). Covers: (a) pre-flight checklist (all scoped PRs merged to main, compose-smoke green on main, CHANGELOG draft verified via `git cliff --unreleased`), (b) exact `git tag -a v1.1.0-rc.N -m "..."` command and signing expectations (carry forward project feedback: full semver `vX.Y.Z-rc.N`, annotated tag, signed if GPG is configured), (c) post-push verification — GHCR manifest inspection (`docker manifest inspect ghcr.io/.../cronduit:v1.1.0-rc.1` shows amd64+arm64), `:rc` rolling tag digest matches, `:latest` digest still pinned to `v1.0.1`, (d) what-if-UAT-fails escalation (ship rc.N+1, not a hotfix tag). Points readers at `.planning/ROADMAP.md` § rc cut points for the schedule.

- **D-12:** **Release-notes content for `v1.1.0-rc.1`.** `git-cliff` already generates the changelog from conventional commits (`cliff.toml` config). Release body content aggregates Phases 10+11+12 work. No manual curation pass in v1.1; `git-cliff` output is authoritative. If Phase 10/11 commit messages don't render well as release notes, that's a conventional-commit discipline problem, not an rc-cut problem — file a hotfix PR before tagging. Do NOT hand-edit the GitHub Release body after publish.

- **D-13:** **Tag cut is a maintainer action, not a workflow_dispatch.** The runbook in D-11 documents `git tag -a v1.1.0-rc.1 -m "Phase 10/11/12 bug-fix block"` run locally by the maintainer, followed by `git push origin v1.1.0-rc.1`. This is deliberately manual — the tag is the canonical release attestation. A `workflow_dispatch` shortcut that creates the tag in GitHub UI is rejected because it moves the trust anchor from the maintainer's signing key into GH Actions' runner identity.

### Testing

- **D-14:** **`cronduit health` unit tests live in `src/cli/health.rs` next to the handler** (following the existing `src/cli/check.rs` / `src/cli/run.rs` pattern). Covers: (a) successful 200 + `status=ok` body → exit 0, (b) non-200 response → exit 1, (c) body without `status` field → exit 1, (d) connect-refused → exit 1 fast, (e) URL construction from `host:port` forms (v4, v6 bracketed, missing port fallback). No testcontainers for this tier; a tokio test HTTP server stub is sufficient.

- **D-15:** **Integration coverage on all four existing CI runners** (SQLite × {amd64, arm64}, Postgres × {amd64, arm64}) remains green. The `cronduit health` handler does not touch the database; no DB-backend parity test needed.

### Claude's Discretion

- Exact module name / path for the new subcommand (`src/cli/health.rs` recommended by parallelism with `check.rs` and `run.rs`).
- Whether `hyper-util` feature set is `["client-legacy", "http1", "tokio"]` or more minimal — planner trims after a local `cargo tree` check.
- Log format for failure messages on stderr — follow existing `tracing` conventions; `cronduit health` can be spartan (one line per failure mode).
- The precise GHA `enable=${{ ... }}` expression syntax (`!contains(github.ref, '-')` vs regex on tag name) — pick whichever renders most readable in `docker/metadata-action`'s tag templates.
- Whether the compose-smoke workflow uses `docker/setup-buildx-action` or the runner's default docker daemon — whichever yields cleaner caching against `cache-from: type=gha,scope=cronduit-release` in the release workflow.
- CHANGELOG heading for the rc (e.g., `## [1.1.0-rc.1] - 2026-04-XX`) — `git-cliff` default is fine; don't over-customize `cliff.toml`.
- Whether `docs/release-rc.md` lives at the repo root, under `docs/`, or is folded into an existing `CONTRIBUTING.md` section — small doc; pick whichever fits the repo's existing doc-placement pattern.

### Folded Todos

None — no pending todos matched Phase 12 (cross-reference surface returned no actionable matches in `.planning/STATE.md § Pending Todos`).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (researcher, planner, executor) MUST read these before planning or implementing.**

### Phase 12 scope and requirements
- `.planning/ROADMAP.md` § "Phase 12: Docker Healthcheck + rc.1 Cut" — phase goal, depends-on, success criteria, locked design decisions (lines 167–186).
- `.planning/ROADMAP.md` § "Strict Dependency Order" item #5 — Docker healthcheck is independent; slots into rc.1 as its own small phase.
- `.planning/REQUIREMENTS.md` § OPS-06..OPS-08 — `cronduit health` subcommand contract, HEALTHCHECK flags, root-cause reproduction commitment.
- `.planning/REQUIREMENTS.md` § Traceability — `T-V11-HEALTH-01`, `T-V11-HEALTH-02` test ids map to OPS-07 success criteria (compose-override + healthy-on-boot).
- `.planning/PROJECT.md` § Current Milestone — iterative rc strategy, `:latest` pinning policy, semver pre-release notation (`vX.Y.Z-rc.N`).

### Carried decisions from earlier phases
- `.planning/phases/11-per-job-run-numbers-log-ux-fixes/11-CONTEXT.md` § D-12 — HTTP listener binds AFTER migration backfill completes; drives the `--start-period=60s` choice.
- `.planning/phases/10-stop-a-running-job-hygiene-preamble/10-CONTEXT.md` § D-12 — `Cargo.toml` version already bumped to `1.1.0` (no re-bump in Phase 12).
- `.planning/STATE.md` § Accumulated Context — iterative rc cadence, `:latest` stays at `v1.0.1` until final `v1.1.0`, full-semver tag format (`v1.1.0-rc.1`).

### Project-level constraints
- `/Users/Robert/Code/public/cronduit/CLAUDE.md` § "Constraints" — rustls-only (no openssl); `cargo tree -i openssl-sys` must return empty. TOML config (not relevant to Phase 12 but locks no-YAML ethos). Mermaid-only diagrams. All changes via PR on feature branch — no direct commits to `main`.
- `.planning/REQUIREMENTS.md` § Security posture — v1 UI is unauthenticated; Phase 12 adds no new attack surface (health probe is read-only, localhost-only by default).
- Auto-memory `feedback_tag_release_version_match.md` — tag must equal `Cargo.toml` version; full semver `v1.1.0-rc.1` preferred. Consumed by D-11 runbook.
- Auto-memory `feedback_no_direct_main_commits.md` — Phase 12 work lands via a feature branch + PR like Phases 10/11.
- Auto-memory `feedback_diagrams_mermaid.md` — any diagram in `docs/release-rc.md` or `compose-smoke.yml` comments must be mermaid, not ASCII.

### Source files the phase touches
- `src/cli/mod.rs` — add `Health` variant to `Command` enum (L34); add dispatch arm in `dispatch()` (L50); potentially clarify `--bind` doc-comment to cover `health` use.
- `src/cli/health.rs` — NEW file, ~80 LOC. `pub async fn execute(cli: &Cli) -> anyhow::Result<i32>`. Constructs URL from `cli.bind` or default, performs GET via `hyper-util::client::legacy::Client`, parses body, returns 0 or 1.
- `src/cli/run.rs`, `src/cli/check.rs` — reference pattern (structure, error handling, exit-code semantics) for the new `health.rs` module.
- `src/main.rs` — no change; already boots tokio and dispatches via `cli::dispatch`.
- `src/web/handlers/health.rs` — reference only. Phase 12 does NOT change the `/health` handler; the CLI just consumes its existing JSON contract (`{status, db, scheduler}`).
- `Cargo.toml` — add `hyper-util = { version = "0.1", features = ["client-legacy", "http1", "tokio"] }`; possibly tighten the existing `hyper = { version = "1", default-features = false }` with `["client"]` or `["http1"]` features depending on what `hyper-util` needs.
- `Dockerfile` — add `HEALTHCHECK` directive in the runtime stage (after L127 `USER cronduit:cronduit`, before L129 `ENTRYPOINT`).
- `.github/workflows/release.yml` — patch `docker/metadata-action` tag templates per D-10 (lines 111–115).
- `.github/workflows/compose-smoke.yml` — NEW. Multi-job workflow: OPS-08 before/after repro + compose-override smoke test.
- `docs/release-rc.md` — NEW. Runbook per D-11.
- `examples/docker-compose.yml` — reference only. The quickstart compose file is the target of Success Criterion #1; Phase 12 does NOT need to edit it (the Dockerfile HEALTHCHECK is picked up automatically). If the compose file currently has a `healthcheck:` override stanza, Phase 12 verifies it either overrides cleanly (compose wins) or is absent (Dockerfile wins).
- `.planning/REQUIREMENTS.md` — flip OPS-06/07/08 checkboxes from `[ ]` to `[x]` as the close-out commit of Phase 12.

### External references
- `docker/metadata-action` tag-condition docs — https://github.com/docker/metadata-action#tags-input (for D-10 `enable=` expressions).
- `hyper 1.x` client migration guide — https://hyper.rs/guides/1/client/basic/ (shows the hyper-util + HttpConnector pairing).
- Alpine 3 busybox wget source (for D-08 historical context; not a canonical ref for implementation).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`GET /health` handler** (`src/web/handlers/health.rs`) — already returns `{"status": "ok"|"degraded", "db": ..., "scheduler": ...}` with HTTP 200 or 503. Phase 12 does NOT change this; `cronduit health` consumes the existing contract unchanged.
- **Global `--bind` flag** (`src/cli/mod.rs:24-26`) — already typed as `Option<String>`. Reused by D-03 without schema change.
- **`dispatch()` pattern** (`src/cli/mod.rs:50-55`) — match on `Command` enum and call subcommand's `execute()`. Phase 12 adds one arm.
- **`anyhow::Result<i32>` exit-code contract** — `check::execute` and `run::execute` already return exit codes via this shape. `health::execute` follows.
- **`serde_json` already in deps** (`Cargo.toml` L84) — drives D-05 body parsing with no new dep.
- **`hyper = "1"`** (`Cargo.toml` L27) — already declared. D-01 adds `hyper-util` alongside.
- **`softprops/action-gh-release` `prerelease: ${{ contains(...) }}` logic** (`release.yml` L163) — already routes rc tags to GitHub Release prerelease. No change.
- **`docker/metadata-action@v5`** (`release.yml` L85) — already in use with four tag templates; D-10 adds `enable` conditions + one new raw template.

### Established Patterns
- **`#[tokio::main]` wraps every CLI subcommand** (`src/main.rs:5`) — `cronduit health` runs inside the same runtime as `run`. Adds no bootstrap.
- **Tracing-based stderr logging** (`telemetry::init` in `src/main.rs:13`) — `cronduit health` uses the same telemetry init; failure messages go through `tracing::error!` at text log-format for human readability.
- **Clap derive with `#[arg(long, global = true)]`** for flags like `--bind` (`src/cli/mod.rs:24-26`) — `cronduit health` reuses the global flag without re-declaring.
- **`cliff.toml` + `git-cliff-action`** for CHANGELOG (`release.yml:64-71`) — D-12 relies on this; no changes to `cliff.toml`.
- **Multi-arch cross-compile via `cargo-zigbuild`** (`Dockerfile:15-77`) — healthcheck is a compiled-in subcommand, so no extra cross-compile surface; just works on both arches.
- **GHA `enable=${{ !contains(github.ref, '-') }}` pattern** — idiomatic for pre-release gating in `docker/metadata-action`; consistent with other Rust OSS projects' release workflows.

### Integration Points
- **Cargo.toml dependencies** — `hyper-util` slots under the existing `# HTTP / web placeholder` comment group (L24).
- **`src/cli/mod.rs` Command enum** — single-variant addition; no trait surface change.
- **Dockerfile runtime stage** — the HEALTHCHECK line placement is between `USER` (L127) and `ENTRYPOINT` (L129); it's a one-line addition.
- **`.github/workflows/release.yml` metadata-action `tags:` block** (L111–115) — five edits (four `enable` additions + one new `type=raw,value=rc` entry); everything else in the workflow unchanged.
- **`.github/workflows/compose-smoke.yml`** — new top-level workflow; runs alongside (not inside) `ci.yml`. Triggers: `push` on main, `pull_request`, and optionally `workflow_call` for reuse in `release.yml`.

</code_context>

<specifics>
## Specific Ideas

- **ROADMAP wording to follow literally:** "No retries (the Docker healthcheck has its own retry policy)." — D-01 lands this as "one request, fast exit, no client-side retry loop."
- **ROADMAP wording to follow literally:** "Compose stanzas still override (compose wins over Dockerfile — backward compatible)." — D-09 lands this as a dedicated CI smoke test.
- **ROADMAP wording to follow literally:** "Reads the bind from `--bind` or defaults to `http://127.0.0.1:8080`." — D-03 preserves this exactly; no new URL flag surface.
- **Symmetry with Phase 10/11:** Phase 10's "silence is success" (D-07) and Phase 11's "HTTP listener binds after backfill" (D-12) both carry forward. `--start-period=60s` is the single timing boundary the operator sees; it accommodates both the backfill and the first read-only query on the `/health` handler.
- **Auto-memory specific:** `feedback_tag_release_version_match.md` says tag and release version must match; full semver preferred. D-11 runbook explicitly calls out `v1.1.0-rc.1` not `v1.1.0-rc1`.

</specifics>

<deferred>
## Deferred Ideas

- **TLS/HTTPS for `cronduit health`** — v1.1 is localhost-HTTP only. Future `--url https://...` or `--tls` flag additive; no breaking change needed when introduced.
- **`/healthz` with a "starting" state during backfill** — Phase 11 D-12 defers this; Phase 12 `--start-period=60s` covers the operator-visible window.
- **`workflow_dispatch` shortcut for tag cuts** — Rejected (D-13). Trust anchor stays with the maintainer's signing key, not the runner identity.
- **Hand-edited GitHub Release bodies** — Rejected (D-12). Conventional-commit discipline is the authoritative source of release notes.
- **Retry inside `cronduit health`** — Rejected by ROADMAP lock and D-01; Docker HEALTHCHECK retry policy is sufficient.
- **Config-file auto-discovery for the `health` subcommand** — Rejected (D-04). Probes must stay config-read free.
- **Per-failure-mode exit codes (e.g., 1=connect-refused, 2=bad-body, 3=timeout)** — Rejected (D-05). Docker HEALTHCHECK treats all non-zero the same; detail stays in stderr logs.
- **Bumping `:latest` on rc tags** — Rejected (D-10). Breaks the PROJECT.md commitment that `:latest` stays at `v1.0.1` through rcs.
- **Extending `ci.yml` to include compose-smoke** — Rejected (D-09). Compose smoke has distinct runner requirements (docker daemon + buildx + compose CLI) that don't belong in the fast unit-test tier.
- **`testcontainers`-based OPS-08 repro** — Rejected (D-09 Hybrid option). `testcontainers` talks to containers directly, not compose; can't cleanly express the compose-override semantic.

</deferred>

---

*Phase: 12-docker-healthcheck-rc-1-cut*
*Context gathered: 2026-04-17*
