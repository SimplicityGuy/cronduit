# Phase 8: v1.0 Final Human UAT Validation - Research

**Researched:** 2026-04-13
**Domain:** Docker runtime rebase, docker.sock access, bollard pre-flight, CI smoke-testing, human UAT choreography
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions (28 total, condensed)

**Runtime rebase (D-01..D-06):**
- D-01: Rebase runtime from `gcr.io/distroless/static-debian12:nonroot` to `alpine:3`. Builder stage (rust:1.94-slim-bookworm + cargo-zigbuild) unchanged — only runtime `FROM` flips.
- D-02: Create `cronduit` system user/group at UID/GID `1000:1000` via `addgroup -S cronduit && adduser -S -u 1000 -G cronduit cronduit`. Pre-create `/data` via `install -d -o 1000 -g 1000 /data`.
- D-03: Drop `--chown=65532:65532 /staging-data /data` COPY cleanly (no commented-out residue).
- D-04: `USER cronduit:cronduit` (or `1000:1000`). `EXPOSE 8080`, `ENTRYPOINT ["/cronduit"]`, `CMD [...]` unchanged.
- D-05: `apk add --no-cache ca-certificates tzdata` in a single RUN layer, no apk cache.
- D-06: Multi-arch build (`linux/amd64 + linux/arm64`) unchanged — alpine:3 has both tags as first-class.

**docker.sock access — dual compose (D-07..D-10):**
- D-07: Ship TWO files: `examples/docker-compose.yml` (simple, numeric `group_add: ["${DOCKER_GID:-999}"]`) and NEW `examples/docker-compose.secure.yml` (tecnativa/docker-socket-proxy sidecar, `DOCKER_HOST=tcp://dockerproxy:2375`, no socket mount on cronduit).
- D-08: README mentions both; default to the simple file, call out secure/macOS recipe.
- D-09: Augment (do not replace) the existing Phase 7 SECURITY block. Plain `#`-prefixed comments — no ASCII art.
- D-10: Minimal allowlist for socket-proxy: `CONTAINERS=1, IMAGES=1, POST=1` as floor. Planner may trim to granular flags if bollard still works.

**Docker daemon pre-flight (D-11..D-14):**
- D-11: After config parse, before scheduler loop, call `Docker::ping()` once. Success → INFO + gauge=1. Failure → WARN with remediation hints + gauge=0. Boot continues regardless.
- D-12: New metric `cronduit_docker_reachable` (gauge, no labels). Lives in Phase 6 metrics family. Never removed once registered. Flipped back to 1 on next successful bollard call.
- D-13: Pre-flight fires on startup and explicit config reload only. Not per-run.
- D-14: NOT fail-fast. Cronduit still boots if daemon is unreachable — command/script jobs must still work.

**Expanded example jobs (D-15..D-17):**
- D-15: Four example jobs: `echo-timestamp` (command */1), `http-healthcheck` (command */5, `wget -q -S --spider https://example.com 2>&1 | head -10`), `disk-usage` (script */15, `du -sh /data` + `df -h /data`), `hello-world` (docker */5, `image = "hello-world:latest"`, `delete = true`).
- D-16: Preserve existing SECURITY block at top of `cronduit.toml`. Add a short paragraph after `[defaults]` explaining the four-job mix.
- D-17: Schedules fit CI budget; CI uses `POST /api/jobs/{id}/run` to force immediate execution.

**Cold-start CI smoke test (D-18..D-22):**
- D-18: Extend existing `compose-smoke` job (Phase 6 gap closure). Add new step AFTER `/health` + job-load assertions, BEFORE teardown.
- D-19: Trigger via `POST /api/jobs/{id}/run`, poll `GET /api/jobs/{id}/runs?limit=1` for `status=success` within 120s. Use `curl -sf` + `jq` (or shell grep substitute).
- D-20: On failure: dump `docker compose logs cronduit --tail=200`, `docker compose logs dockerproxy --tail=50` (secure variant), last 5 runs for each job, and `curl http://127.0.0.1:8080/metrics | grep cronduit_docker_reachable`, then exit non-zero.
- D-21: MATRIX axis: run both `docker-compose.yml` and `docker-compose.secure.yml`. Both must pass.
- D-22: Keep single compose-up/down cycle per axis. Do not split into two jobs.

**Human UAT walkthrough (D-23..D-25):**
- D-23: Land UAT results in existing per-phase files in place:
  - `03-HUMAN-UAT.md` — flip 4 pending items.
  - NEW `06-HUMAN-UAT.md` — quickstart end-to-end + SSE live-stream tests.
  - `07-UAT.md` — re-run Tests 2 + 3 in place, add `re_tested_at:` annotation.
  - NEW `08-HUMAN-UAT.md` — index only.
- D-24: Only `result: pass | issue (+severity, reported) | blocked (+blocked_by, reason)`. Follow existing `03-HUMAN-UAT.md` and `07-UAT.md` YAML frontmatter shapes verbatim.
- D-25: UAT is user-driven. Claude prepares fixtures, never marks items passed from its own runs.

**v1.1 backlog routing (D-26..D-28):**
- D-26: Functional breakage → fix in Phase 8. Visual polish / copy / dark-mode edge cases → v1.1 backlog.
- D-27: v1.1 entries land in `.planning/BACKLOG.md` (create if missing) with 999.x numbering. Each entry: title, originating UAT file + line, observed behavior, expected behavior, "why not v1.0 blocker".
- D-28: Ambiguous cases default to v1.1. Err toward shipping v1.0.

### Claude's Discretion

- Plan ordering / wave assignment (3 plans vs 4 plans is the planner's call).
- Whether to pin alpine package versions (default: track moving tag).
- Exact WARN text for the pre-flight (<200 chars grep-friendly).
- Socket-proxy allowlist can be trimmed below the D-10 floor as long as bollard calls succeed.
- `jq` vs shell-only JSON parsing in smoke test.
- Whether to add `06-VERIFICATION.md` / `07-VERIFICATION.md` re-verification annotations.

### Deferred Ideas (OUT OF SCOPE)

- Full arm64 validation of the secure compose variant (CI runner arch only).
- Reverse-proxy + auth example in `docker-compose.secure.yml` (still `ports:`, not `expose:`).
- `cronduit_docker_ping_duration_seconds` latency histogram.
- Automated migration of existing UID 65532 named volumes to UID 1000 (document as manual step in release notes).
- Audit-milestone re-run (milestone-level, not Phase 8).
- `06-VERIFICATION.md` / `07-VERIFICATION.md` re_verification annotations (decide during planning).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| UI-05 | UI pages match `design/DESIGN_SYSTEM.md` terminal-green palette | `03-HUMAN-UAT.md` Test 1 — pending flip. Human-only validation; no code changes in Phase 8. |
| UI-06 | Dashboard lists jobs with metadata | `03-HUMAN-UAT.md` Test 2 (dark/light toggle persistence) — pending flip. Human validation only. |
| UI-09 | Run Detail page shows metadata + logs with stdout/stderr distinction, ANSI colors | `03-HUMAN-UAT.md` Test 4 (ANSI log rendering) — pending flip. Human validation only. |
| UI-12 | "Run Now" button triggers manual run | `03-HUMAN-UAT.md` Test 3 (Run Now toast) — pending flip. Human validation only. |
| OPS-05 | README quickstart enables stranger to `docker compose up` in under 5 minutes | NEW `06-HUMAN-UAT.md` — quickstart end-to-end test. Runtime rebase (D-01..D-06), dual compose (D-07..D-10), expanded examples (D-15..D-17), compose-smoke CI (D-18..D-22) together form the runnable quickstart contract that OPS-05 requires. |
| UI-14 | Run Detail log viewer streams via SSE for in-progress runs | NEW `06-HUMAN-UAT.md` — SSE live-stream test. `http-healthcheck` / `disk-usage` provide long-enough RUNNING window (DNS + TLS + HTTP is ~500ms-2s; `du` on mounted volume is workably short). May need `sleep 3` padding — see Decision 11 below. |

Additional closure: **07-UAT.md Tests 2 + 3** (Job Detail auto-refresh + polling-stop) — re-run in place after D-15's longer-running example jobs exist to unblock the Plan 07-05 browser UAT.
</phase_requirements>

## Summary

Phase 8 is a **gap-closure + human UAT phase**, not a feature phase. CONTEXT.md has 28 decisions locking every meaningful choice. Research role is to **validate the locked decisions are technically sound** and surface any executional gotchas the planner needs to hook into plans.

**Primary recommendation:** Proceed with confidence on the alpine:3 rebase, the dual-compose story, and the pre-flight check. Four real executional risks exist and need planner mitigation:

1. **`connect_with_local_defaults` does NOT consult `DOCKER_HOST`.** The current code path in `src/cli/run.rs:150` uses `Docker::connect_with_local_defaults()` which is a thin wrapper over `connect_with_unix_defaults` on Unix — it never reads `DOCKER_HOST`. For `docker-compose.secure.yml` (D-10: `DOCKER_HOST=tcp://dockerproxy:2375`) to work, we **MUST switch to `Docker::connect_with_defaults()`** which does consult the env var. This is a code change nobody mentioned in CONTEXT.md. HIGH risk if missed — the secure variant silently falls back to Unix socket and fails.

2. **Named-volume ownership is sticky on first mount, NOT re-applied on image change.** Existing users upgrading from the Phase 6/7 image (UID 65532) will keep their old volume ownership. Pre-creating `/data` with `install -d -o 1000 -g 1000` only works for **fresh** volumes. Existing volumes need a documented `chown` one-liner (CONTEXT deferred ideas mention this) — the release PR must ship a RELEASE NOTES entry or users get "permission denied on /data" on first upgrade.

3. **busybox `wget` HTTPS works on alpine only because `ca-certificates` is installed.** D-05 covers this, but the package is load-bearing for D-15's `http-healthcheck` job. If the apk install line is split or the package pinning changes, the `http-healthcheck` example will silently break on TLS handshake. Plan must include a test that exercises `wget https://` inside the CI smoke run.

4. **`cargo-zigbuild` static-musl binaries run fine on alpine** (confirmed — musl is alpine's native libc, and zigbuild produces fully static binaries with no dynamic linker dependency). No migration work needed in the builder stage. LOW risk, but worth confirming in the PR description.

The rest of CONTEXT.md is mechanically executable. The biggest time sink will be the dual-file CI matrix work (D-18..D-22) and validating the socket-proxy allowlist against every bollard call cronduit actually makes.

## Standard Stack

All locked by CLAUDE.md and the existing Cargo.toml — no new dependencies needed for Phase 8.

### Core (unchanged from Phase 6/7)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `bollard` | 0.20.2 | Docker API client | [VERIFIED: Cargo.lock] Already in use. `Docker::ping()` and `Docker::connect_with_defaults()` live here. |
| `metrics` | 0.24.x | Metrics facade | [VERIFIED: Cargo.toml] Used by existing Phase 6 `cronduit_*` family. New `cronduit_docker_reachable` gauge slots into the same facade. |
| `metrics-exporter-prometheus` | 0.18.x | `/metrics` exporter | [VERIFIED: Cargo.toml] Already wired via `src/telemetry.rs::setup_metrics()`. |
| `tracing` | 0.1.x | Structured logging | [VERIFIED: Cargo.toml] Existing target scheme `cronduit.startup` / `cronduit.docker.preflight` is the template for the new pre-flight WARN. |

### Runtime Images (the rebase)
| Image | Use | Verified |
|-------|-----|----------|
| `alpine:3` | Runtime base | [VERIFIED: Docker Hub multi-arch] Multi-arch (amd64 + arm64 + armv7 + more), ships busybox with `sh`, `date`, `wget`, `du`, `df`, `sleep`, `head`, `tail` applets. apk available for `ca-certificates` + `tzdata`. [CITED: wiki.alpinelinux.org/wiki/BusyBox, docker.com/blog/how-to-use-the-alpine-docker-official-image] |
| `tecnativa/docker-socket-proxy:latest` | Secure-variant sidecar | [CITED: github.com/Tecnativa/docker-socket-proxy README] Listens on port 2375 by default. Env-var-driven allowlist. Supports `linux/amd64` and `linux/arm64`. |
| `gcr.io/distroless/static-debian12:nonroot` | Current runtime (being replaced) | [VERIFIED: Dockerfile:53] Has no `/bin/sh`, no coreutils, no busybox — this is the root cause of the `echo-timestamp` ENOENT in 07-UAT.md. Rebase resolves it. |

**Installation (Dockerfile runtime stage, replacing lines 53-70):**
```dockerfile
# ---- runtime ----
FROM alpine:3

# Reason for the distroless walk-back: distroless has no /bin/sh or coreutils,
# which made the quickstart `examples/cronduit.toml` command/script jobs
# impossible to execute. See .planning/phases/01-*/01-CONTEXT.md D-XX
# (original distroless choice) and 08-CONTEXT.md D-01 (rebase rationale).
RUN apk add --no-cache ca-certificates tzdata \
    && addgroup -S cronduit \
    && adduser -S -u 1000 -G cronduit cronduit \
    && install -d -o 1000 -g 1000 /data

LABEL org.opencontainers.image.source="https://github.com/SimplicityGuy/cronduit"
LABEL org.opencontainers.image.description="Self-hosted Docker-native cron scheduler with a web UI"
LABEL org.opencontainers.image.licenses="MIT OR Apache-2.0"

COPY --from=builder /cronduit /cronduit
COPY --from=builder /build/examples/cronduit.toml /etc/cronduit/config.toml

EXPOSE 8080
USER cronduit:cronduit
ENTRYPOINT ["/cronduit"]
CMD ["run", "--config", "/etc/cronduit/config.toml"]
```

Note: drop the `install -d -o 65532 -g 65532 /staging-data` line in the builder stage (Dockerfile:50) and the `COPY --from=builder --chown=65532:65532 /staging-data /data` line — they are both obsolete under the alpine path.

## Decision-by-Decision Research Notes

### Decision 1: alpine:3 rebase path — SAFE

**Question:** Is `alpine:3` still the right moving tag in 2026? Any cargo-zigbuild static-musl regressions on alpine? Any `apk add ca-certificates tzdata` pitfalls?

**Findings:**
- `alpine:3` is the current moving tag pointing at Alpine 3.22+ (as of 2026-04). Multi-arch (amd64, arm64, armv6, armv7, ppc64le, riscv64, s390x, 386). [CITED: hub.docker.com/_/alpine]
- Alpine's native libc is **musl**, so cargo-zigbuild's static-musl binaries run without a dynamic linker. No `/lib/ld-musl-*.so.1` resolution issues because the binary is fully static. [CITED: drmhse.com/posts/fast-rust-docker-builds-with-zigbuild/, github.com/rust-cross/cargo-zigbuild]
- `apk add --no-cache` is the standard pattern; `--no-cache` skips the cache population (equivalent to `rm -rf /var/cache/apk/*` on older alpine) so there's no cleanup step needed.
- `ca-certificates` is required for bollard's HTTPS calls to image registries (docker.io, ghcr.io) AND for the new `http-healthcheck` example job's busybox wget TLS handshake. **Both depend on the same package.**
- `tzdata` is required for croner's timezone-aware `next_after()` (Phase 1 `[server].timezone = "UTC"` config). Without it, any non-UTC timezone falls back to UTC silently.

**Confidence:** HIGH. Alpine-on-musl + static-binary is the canonical deployment shape. The rebase is mechanical.

**Planner hook:** Single RUN layer for apk + user + install to keep layer count minimal. Add a short Dockerfile header comment explaining the walk-back from distroless (per CONTEXT.md canonical_refs).

### Decision 2: busybox wget HTTPS on alpine — WORKS with caveat

**Question:** Does `wget -q -S --spider https://example.com 2>&1 | head -10` work inside alpine:3?

**Findings:**
- Alpine's busybox wget supports HTTPS via an external `ssl_client` helper bundled in the busybox package. [CITED: gitlab.alpinelinux.org/alpine/aports/-/issues/15861, devendevour.wordpress.com/2024/11/25/fixing-tls-for-wget-in-busybox/]
- **Certificate verification requires `ca-certificates` installed.** Without it, busybox wget's HTTPS either fails on cert validation or ignores certs entirely (`CONFIG_FEATURE_WGET_HTTPS` ignores certs; alpine patched to use `ssl_client` for real verification). D-05 covers this.
- Some TLS cipher suites may not be supported depending on how busybox was compiled. `example.com` uses standard TLS 1.2/1.3 with common ciphers — tested to work on current alpine:3.
- `wget -S` dumps response headers to stderr; `--spider` makes it HEAD-only (no body download); `2>&1 | head -10` captures the headers into cronduit's log pipeline.

**Confidence:** HIGH. The TLS story on alpine is well-understood and ca-certificates is a hard prerequisite already in D-05.

**Planner hook:** The CI smoke test MUST verify the `http-healthcheck` job reaches `success` — this is the TLS + DNS + egress canary. If it fails, the apk install layer is broken.

### Decision 3: `DOCKER_HOST` env-var handling in bollard 0.20 — CRITICAL CODE CHANGE NEEDED

**Question:** Does bollard's current connection path pick up `DOCKER_HOST=tcp://dockerproxy:2375`?

**Findings:**
- `Docker::connect_with_local_defaults()` [VERIFIED: docs.rs/bollard/0.20.2 + current use at `src/cli/run.rs:150`] is "a simple wrapper over the OS-specific handlers: Unix `connect_with_unix_defaults`, Windows `connect_with_named_pipe_defaults`." It does **NOT** read `DOCKER_HOST`.
- `Docker::connect_with_defaults()` [CITED: docs.rs/bollard/0.20.2 Docker methods] is the entry point that **does** consult `DOCKER_HOST`. It inspects the env var and dispatches to unix/http/ssl as appropriate. If `DOCKER_HOST` is unset, it falls back to local defaults.
- `Docker::ping()` signature: `pub async fn ping(&self) -> Result<(), bollard::errors::Error>`. Returns unit on success, structured error on failure. The error's `Display` impl includes the underlying cause (socket path, connect error, HTTP status). [CITED: docs.rs/bollard/0.20.2/bollard/struct.Docker.html]

**Impact:** CONTEXT.md D-10 assumes "bollard reads DOCKER_HOST the same way the docker CLI does." That is only true if the code calls `connect_with_defaults()`. Today's code calls `connect_with_local_defaults()`. **The secure compose file will silently fail to pick up the proxy** — bollard will try `/var/run/docker.sock` (unmounted in the secure variant) and fail.

**Fix:** In `src/cli/run.rs:150`, change:
```rust
let docker = match bollard::Docker::connect_with_local_defaults() {
```
to:
```rust
let docker = match bollard::Docker::connect_with_defaults() {
```

This is a one-line change but **load-bearing** for the entire D-07/D-10 secure variant story. The planner MUST include this in Plan 08-0X for the pre-flight check.

**Confidence:** HIGH (signature verified via Context7-equivalent docs.rs fetch). This is the most important finding in this research.

### Decision 4: `tecnativa/docker-socket-proxy` allowlist for cronduit's bollard calls — NEEDS `CONTAINERS` + `IMAGES` + `POST`

**Question:** What is the minimum allowlist for bollard's ephemeral-container workflow (pull → create → start → wait → logs → remove)?

**Findings:**
- `tecnativa/docker-socket-proxy` starts with everything revoked except `EVENTS`, `PING`, `VERSION` by default. [CITED: github.com/Tecnativa/docker-socket-proxy README]
- Required permissions for cronduit's bollard calls:

| bollard call | HTTP endpoint | Required env |
|--------------|---------------|--------------|
| `Docker::ping()` | `GET /_ping` | `PING=1` (default enabled) |
| Pull image | `POST /images/create?fromImage=...` | `IMAGES=1` + `POST=1` |
| Create container | `POST /containers/create` | `CONTAINERS=1` + `POST=1` |
| Start container | `POST /containers/{id}/start` | `CONTAINERS=1` + `POST=1` (OR granular `ALLOW_START=1`) |
| Wait on container | `POST /containers/{id}/wait` | `CONTAINERS=1` + `POST=1` |
| Inspect container | `GET /containers/{id}/json` | `CONTAINERS=1` |
| Stream logs | `GET /containers/{id}/logs?follow=true` | `CONTAINERS=1` |
| Remove container | `DELETE /containers/{id}` | `CONTAINERS=1` + `POST=1` (DELETE is grouped under POST in the proxy's model) |
| Inspect network (`docker_preflight.rs`) | `GET /networks/{id}` | `NETWORKS=1` |

- **Minimum allowlist for the secure compose file:**
  ```yaml
  environment:
    - CONTAINERS=1
    - IMAGES=1
    - NETWORKS=1   # required for src/scheduler/docker_preflight.rs validate_named_network
    - POST=1
  ```
- D-10 says `CONTAINERS=1, IMAGES=1, POST=1` floor. **That floor is missing `NETWORKS=1`** — cronduit's existing `docker_preflight.rs::validate_named_network()` calls `docker.inspect_network()` which hits `/networks/{id}`. Without `NETWORKS=1`, any `network = "my-custom-net"` job fails pre-flight with a docker_unavailable error. The `hello-world` quickstart job uses `network = "bridge"` (builtin, no pre-flight) so the basic smoke test still passes, but users with named networks get a silent regression.
- `SECRETS=0`, `AUTH=0`, `EXEC=0`, `VOLUMES=0`, `BUILD=0`, `SYSTEM=0`, `INFO=0` stay revoked by default — good. No action needed.
- The proxy defaults are secure-by-default. Enumerating only what cronduit actually needs is correct.

**Planner hook:** Add `NETWORKS=1` to the D-10 allowlist. Document each env var with a short "why" comment inline. Recommend using granular flags (`ALLOW_START=1, ALLOW_STOP=1`) instead of broad `POST=1` if time allows — tighter blast radius. But `POST=1` is simpler and matches the "minimal for the quickstart" goal.

**Confidence:** HIGH (allowlist mapping verified against proxy README). MEDIUM on the granular-vs-broad POST recommendation — tighter is better but adds comment complexity.

### Decision 5: Docker Desktop macOS socket reality — CONFIRMS socket-proxy is the correct path

**Question:** On Docker Desktop macOS, does `group_add` + a host-mapped GID work, or is socket-proxy required?

**Findings:**
- On Docker Desktop macOS, `/var/run/docker.sock` on the host is a symlink into the Linux VM owned by `root:daemon` (or similar Unix domain) on the host. When bind-mounted into a container, it appears as `root:root` with restricted permissions (`srw-rw----`). [CITED: forums.docker.com/t/mounting-using-var-run-docker-sock-in-a-container-not-running-as-root/34390]
- Changing ownership inside the container is ineffective — "on a Mac, the ownership change happens only at the 'Hypervisor VM' Docker Desktop is creating." The host-side GID has no mapping into the Linux VM's `docker` group because there is no host-side `docker` group. `group_add: ["999"]` on macOS adds the cronduit process to a GID that has no relationship to the socket.
- **The socket-proxy approach is the community-consensus fix.** A `docker-socket-proxy` sidecar runs as root inside its own container, has legitimate access to the Unix socket, and exposes the Docker API over TCP on `dockerproxy:2375`. cronduit (non-root) connects via TCP — no socket permissions in the critical path.
- Other community options (less good):
  - `socat` bridge (same idea as socket-proxy but no allowlist, less secure).
  - Run cronduit as root (defeats the security story).
  - `fixuid` / entrypoint-based dynamic UID mapping (complex, not a homelab one-liner).

**Confidence:** HIGH. The D-07/D-08/D-10 decision to ship the dual-compose file set is correct and mandatory for macOS users.

**Planner hook:** `examples/docker-compose.yml` must have a loud comment telling macOS users to switch to the secure variant. `examples/docker-compose.secure.yml` must have a comment explaining **why** macOS needs it (VM socket ownership) — not just "it's more secure."

### Decision 6: `Docker::ping()` API in bollard 0.20 — STRAIGHTFORWARD

**Question:** Exact signature. Structured error. Async gotchas.

**Findings:**
- Signature: `pub async fn ping(&self) -> Result<(), bollard::errors::Error>`. [CITED: docs.rs/bollard/0.20.2/bollard/struct.Docker.html#method.ping]
- Hits `GET /_ping` on the daemon (always allowed by the socket-proxy's default `PING=1`).
- Error type `bollard::errors::Error` has a `Display` impl that includes the underlying cause — the pre-flight WARN line can interpolate `{error}` directly without structured destructuring.
- No async gotchas — it's a single HTTP request with a short timeout driven by `hyper`'s default connect timeout. Will NOT hang indefinitely if the socket is missing; returns an error within a handful of seconds at worst.
- The Docker daemon URI the client is talking to is NOT directly accessible via a getter on `Docker` in 0.20. Workaround: read `DOCKER_HOST` env var in the WARN interpolation (if set) or fall back to `"/var/run/docker.sock"` label. The WARN line can read:
  ```
  docker daemon unreachable at {uri}: {error}. ...
  ```
  where `{uri}` is computed at connection time from `std::env::var("DOCKER_HOST").unwrap_or_else(|_| "/var/run/docker.sock".to_string())`.

**Confidence:** HIGH.

**Planner hook:** Add a simple helper in the pre-flight module that records the computed `{uri}` at `Docker::connect_with_defaults()` time so the WARN line can reference the exact transport being used. This is the difference between "cronduit is broken" (useless log) and "cronduit could not connect to /var/run/docker.sock" (actionable).

### Decision 7: Alpine UID 1000 + named-volume ownership semantics — NEEDS MIGRATION DOCUMENTATION

**Question:** Does Docker honor `install -d -o 1000 -g 1000 /data` in the image when a fresh named volume mounts there? What about existing volumes from prior UID 65532 images?

**Findings:**
- **Fresh named volume:** Docker's "populate empty volume from image directory" behavior copies both content AND ownership/permissions. [CITED: docs.docker.com/engine/storage/volumes/ — "If you mount an _empty volume_ into a directory in the container in which files or directories exist, these files or directories are propagated (copied) into the volume by default."] The ownership propagates because Docker does an unprivileged `cp -a`-equivalent on first mount. Verified community consensus: pre-creating the directory with correct ownership in the image works for fresh volumes.
- **Existing named volume (non-empty):** Docker does **NOT** re-apply image ownership. The volume's existing ownership wins. Existing users upgrading from the distroless image (UID 65532) will have a volume where `/data` is chowned `65532:65532`. After the Phase 8 rebase, cronduit (UID 1000) cannot write to it — SQLite open fails with "unable to open database file" or "permission denied". [CITED: multiple community reports: codestudy.net, forums.docker.com/t/... volume ownership clean separation]
- **No in-process fix.** cronduit cannot run as root briefly and chown because D-02 locks the image USER to `cronduit:1000`. Even a `USER root` in a split ENTRYPOINT adds complexity and violates the "cronduit does not run as root" story.
- **The honest fix is documentation + a one-liner.** CONTEXT.md deferred ideas already identify this: `docker run --rm -v cronduit-data:/data alpine chown -R 1000:1000 /data`. This must land in:
  - The PR description for the rebase.
  - A new CHANGELOG.md / RELEASE-NOTES.md entry under the v1.0 release.
  - A prominent callout in README (at least a one-line "upgrading from 0.1.x? run this.").

**Confidence:** HIGH on behavior; MEDIUM on whether to scope the CHANGELOG work into Phase 8 or defer to the release PR. Recommendation: include in Phase 8 — it's a direct consequence of D-02 and the release flow is the natural place to surface it.

**Planner hook:** Add a task to write a `CHANGELOG.md` / `RELEASE-NOTES.md` entry for the UID migration. Include the exact chown one-liner. Reference from the README SECURITY section.

### Decision 8: Migration — no in-process rescue — DOCUMENT ONLY

**Question:** Can cronduit detect old-UID data on boot and fix it? Or must it be manual?

**Findings:**
- In-process chown from a non-root user fails with EPERM. cronduit would need to run as root, chown, then drop privileges. This requires:
  - A shell entrypoint script (breaks the "single binary + /cronduit ENTRYPOINT" story).
  - OR rust code that `setuid(0)` → chown → `setuid(1000)` (not portable; not how musl-static works cleanly).
  - OR a multi-stage entrypoint via `su-exec` / `gosu` in alpine (adds a package, adds a binary).
- **None of these are worth the complexity for a v1 homelab tool.** Document the manual step in CHANGELOG and move on.
- Alternative future option (v1.1 backlog): ship a `cronduit fixup-volume /data` subcommand that a user can run via `docker run --user 0:0 --entrypoint /cronduit <image> fixup-volume /data`. Clean but not needed for v1.0.

**Confidence:** HIGH.

**Planner hook:** Do NOT attempt in-process migration. Document manual step. Add v1.1 backlog entry for `cronduit fixup-volume` if any user reports pain.

### Decision 9: `compose-smoke` CI extension — idiomatic polling pattern

**Question:** What's the GitHub Actions idiom for polling an HTTP endpoint with a wall-clock budget? `jq` vs `grep` for JSON parsing? Matrix axis for dual compose files?

**Findings:**
- **Polling idiom:** The existing `compose-smoke` job already uses a `for i in $(seq 1 30); do curl ... && break; sleep 1; done` shell loop (ci.yml:170-182). Extend the same pattern for the new 120s job-success assertion. No external action needed.
- **`jq` availability:** `jq` is preinstalled on `ubuntu-latest` runners in 2026 (has been since 2020). [CITED: GitHub Actions runner images manifest] No apt install needed. The existing `compose-smoke` job does not currently use jq but it's available.
- **JSON parsing option:**
  ```bash
  # With jq:
  STATUS=$(curl -sSf http://localhost:8080/api/jobs/$JOB_ID/runs?limit=1 | jq -r '.[0].status')
  [ "$STATUS" = "success" ] && break

  # Without jq (simpler, less robust):
  curl -sSf http://localhost:8080/api/jobs/$JOB_ID/runs?limit=1 | grep -q '"status":"success"' && break
  ```
  Recommendation: **use jq**. It's available, more robust, and gives cleaner error messages when the response shape is wrong. D-19 already suggests jq as the default.
- **Matrix axis:** Replace the single `compose-smoke` job with a matrix-enabled version:
  ```yaml
  compose-smoke:
    strategy:
      fail-fast: false
      matrix:
        compose:
          - file: docker-compose.yml
            name: simple
          - file: docker-compose.secure.yml
            name: secure
    name: quickstart compose smoke (${{ matrix.compose.name }})
    steps:
      - uses: actions/checkout@v4
      # ...
      - name: docker compose up -d
        working-directory: examples
        run: docker compose -f ${{ matrix.compose.file }} up -d
      # ... (all subsequent steps parameterized on ${{ matrix.compose.file }})
  ```
- **Keeping boot budget sane:** D-22 says don't split into two jobs. The matrix approach accomplishes this naturally — each axis is its own runner, each does its own compose-up/down cycle in parallel, and fail-fast=false lets you see both failures if they happen.

**Confidence:** HIGH.

**Planner hook:** The new job-success polling step should:
1. Parse the job IDs from `/api/jobs` once after boot (NOT hard-code UUIDs).
2. Loop over each job, call `POST /api/jobs/{id}/run`, then poll `GET /api/jobs/{id}/runs?limit=1` at 2s intervals for 120s.
3. On any individual job timeout, dump the diagnostic bundle from D-20 and exit 1.
4. On all-pass, print a one-liner summary per job.

### Decision 10: Rust-side unit-testing the pre-flight WARN — POSSIBLE but LOW VALUE

**Question:** Can we unit-test the pre-flight WARN-and-continue behavior without a live Docker daemon?

**Findings:**
- bollard does not ship a mock client. The `Docker` struct owns its transport; there's no trait abstraction to substitute.
- Integration testing the pre-flight requires a real (or proxy-faked) daemon. Options:
  - **testcontainers** — already in use in the test stack. Can spin up a `tecnativa/docker-socket-proxy` container and point bollard at it, then `kill` the container to exercise the failure path. Heavy for a single WARN test.
  - **Isolated function test:** Extract `run_docker_preflight(docker: Result<Docker, Error>) -> DockerGaugeValue` and unit-test the two-branch behavior by passing `Ok(mock_docker)` vs `Err(synthetic_error)`. This tests the gauge side-effect via a captured-metrics handle. Works, but doesn't exercise `Docker::ping()` itself.
  - **Compile-time only:** Just cover the WARN text and gauge call via a standard `tracing-test` subscriber + `metrics::describe_gauge!` registration check at boot.
- **Recommendation:** Do NOT add a live-daemon unit test for this in Phase 8. The CI smoke test exercises the pre-flight happy path (daemon reachable) and, by virtue of running the secure variant, exercises the DOCKER_HOST codepath. The unhappy path (daemon unreachable) is adequately covered by:
  1. A Rust unit test that records the gauge registration and the tracing target string.
  2. The existing `07-UAT.md` Test 1 "boot with command/script jobs only and daemon unreachable" scenario — add this to the human UAT walkthrough as a one-line check.

**Confidence:** HIGH on "no new integration test infrastructure needed." MEDIUM on whether a mock-based unit test adds value — probably not for Phase 8's gap-closure scope.

**Planner hook:** Add ONE Rust unit test that validates:
- `metrics::describe_gauge!("cronduit_docker_reachable", ...)` is registered during `setup_metrics()` and renders in the `/metrics` output.
- The gauge value is 0 before any ping, 1 after a successful ping (simulate via direct `metrics::gauge!(...).set(1.0)`), 0 after a failed ping (direct set).

This tests the wiring without mocking bollard. The actual `Docker::ping()` path is tested via the CI smoke test (positive) and manual UAT on a machine with the daemon stopped (negative).

### Decision 11: RUNNING-state observability for Plan 07-05 UAT — LIKELY SUFFICIENT

**Question:** Do the new example jobs stay in RUNNING long enough to observe the HTMX `every 2s` polling transition (Plan 07-05 blocked test)?

**Findings:**
- `wget -q -S --spider https://example.com 2>&1 | head -10`: DNS + TLS handshake + HTTP HEAD + teardown = **500ms to 2s** typical on a homelab network (including bollard spawn overhead). On slow networks or cold DNS, can stretch to 3-5s.
- `du -sh /data` on an empty or near-empty mounted volume: **20-50ms**. Too fast.
- `du -sh /data && df -h /data`: Still <100ms.
- Plan 07-05 needs a RUNNING window > ~2s (two polling cycles) to observe the transition. The existing `echo-timestamp` (`date '+...'`) runs in ~5ms — useless for the test.
- **Recommendation:** `http-healthcheck` (wget over TLS) is the natural long-running job. In practice ~1-3s per run — enough for the operator to click through to Run Detail and see one polling cycle. **Padding with `sleep 3` inside the script would make it more reliable but is artificial** and conflicts with "realistic uptime canary" language in D-15.
- **Safer alternative:** Make `disk-usage` the observable job by including a `sleep 3` in the script body. It's already a `script =` type, so the script body is the planner's canvas:
  ```sh
  #!/bin/sh
  echo "=== disk usage check ==="
  du -sh /data 2>/dev/null || echo "/data not mounted"
  df -h /data 2>/dev/null || true
  # Small pause to make the RUNNING state observable for UI validation.
  sleep 3
  echo "=== check complete ==="
  ```
- The `sleep 3` is NOT padding for padding's sake — it's a deliberate demonstration of a longer-running script job so the UI Run Detail page has meaningful content to render. Document it in the comment.

**Confidence:** MEDIUM on whether the user will find the natural `wget` duration sufficient. HIGH on "adding `sleep 3` to `disk-usage` is the cleanest fix."

**Planner hook:** Include the `sleep 3` in the `disk-usage` script body. Add a comment explaining "demonstrates a longer-running script job so the UI Run Detail page has observable content; real monitoring scripts would omit the sleep." This satisfies Plan 07-05's Test 2 observability requirement while remaining a realistic-looking script example.

### Decision 12: Prometheus gauge mid-life registration — FOLLOWS EXISTING PHASE 6 PATTERN

**Question:** Adding `cronduit_docker_reachable` to an existing metrics family — gotchas with `metrics` 0.24 + `metrics-exporter-prometheus` 0.18 registration order?

**Findings:**
- Phase 6 GAP-1 (in `src/telemetry.rs::setup_metrics()` lines 75-121) already established the canonical pattern:
  1. `install_recorder()` once via `PrometheusBuilder`.
  2. Eagerly `describe_*!` each metric family so HELP/TYPE lines render from boot.
  3. Eagerly register each family via a zero-baseline call (`gauge!.set(0.0)`, `counter!.increment(0)`, etc.) so the family exists in the `/metrics` body even before the first observation.
- **The new gauge follows this exact pattern.** In `setup_metrics()`, add:
  ```rust
  metrics::describe_gauge!(
      "cronduit_docker_reachable",
      "1 if the Docker daemon is reachable at cronduit startup, 0 otherwise"
  );
  metrics::gauge!("cronduit_docker_reachable").set(0.0);
  ```
- Registration ORDER does not matter among the `describe_*!` calls; they just populate the metadata table. The zero-baseline `.set(0.0)` is what forces family registration.
- The pre-flight check runs AFTER `setup_metrics()` is called (see `src/cli/run.rs:130` for the call site, pre-flight happens around line 150-165). By then the gauge is registered; the pre-flight just flips the value via `metrics::gauge!("cronduit_docker_reachable").set(1.0)` or `.set(0.0)`.
- D-13's "flip back to 1 on next successful bollard call" contract is satisfied by adding one line to `docker_pull.rs` (or wherever the next bollard ping-equivalent happens). Alternatively, don't bother flipping back — the operator will re-run the pre-flight by sending SIGHUP or calling `/api/reload`.

**Confidence:** HIGH.

**Planner hook:** Copy the Phase 6 GAP-1 pattern verbatim. Add the two lines (describe + zero-baseline) to `setup_metrics()` alongside the other `cronduit_*` families. Wire the ping success/failure to the gauge from `docker_preflight.rs` (new function) or `src/cli/run.rs` around the existing `Docker::connect_with_*_defaults()` call site.

### Decision 13: YAML frontmatter shape for UAT files — CANONICAL TEMPLATE EXISTS

**Question:** Canonical shape for the new `06-HUMAN-UAT.md` + `08-HUMAN-UAT.md`. What fields carry forward?

**Findings:**
- `03-HUMAN-UAT.md` frontmatter (verified verbatim):
  ```yaml
  ---
  status: partial                    # partial | complete
  phase: 03-read-only-web-ui-health-endpoint
  source: [03-VERIFICATION.md]
  started: 2026-04-10T00:00:00Z
  updated: 2026-04-10T00:00:00Z
  ---
  ```
- `07-UAT.md` frontmatter (verified verbatim):
  ```yaml
  ---
  status: partial
  phase: 07-v1-cleanup-bookkeeping
  source:
    - 07-01-SUMMARY.md
    - 07-02-SUMMARY.md
    - 07-03-SUMMARY.md
    - 07-04-SUMMARY.md
    - 07-05-SUMMARY.md
  started: 2026-04-13T21:43:15Z
  updated: 2026-04-13T21:57:00Z
  ---
  ```
- Body sections (both files): `## Current Test`, `## Tests` (numbered `### 1. Name`), `## Summary` (total/passed/issues/pending/skipped/blocked), `## Gaps` (list of `truth:` / `status:` / `reason:` / `severity:` / `test:` / `artifacts:` / `missing:`).
- Per-test entry format (from `07-UAT.md`):
  ```markdown
  ### N. Test Name
  expected: |
    <what the tester should do>
  result: pass | issue | blocked
  # On issue:
  reported: |
    <observed symptom>
  severity: blocker | major | minor
  # On blocked:
  blocked_by: <prior-test | external-dep>
  reason: "<one-line reason>"
  ```
- **New Phase 8 additions (for re-test tracking):**
  - `re_tested_at: 2026-04-XXTHH:MM:SSZ` — when re-running a previously-failed test from `07-UAT.md` Tests 2+3, add this field to the same test entry in place. Preserves provenance per D-23.
  - The frontmatter `updated:` field is the canonical "last edit" timestamp. Bump on every in-place edit.
- **For the NEW `06-HUMAN-UAT.md`:**
  ```yaml
  ---
  status: partial
  phase: 06-live-events-metrics-retention-release-engineering
  source:
    - 06-VERIFICATION.md
    - 08-CONTEXT.md
  started: <Phase 8 UAT start>
  updated: <bumped on every edit>
  ---
  ```
  Tests: (1) Quickstart end-to-end — clone → `docker compose up` → dashboard loads → `echo-timestamp` fires within ~60s. (2) SSE live log streaming — trigger Run Now on `http-healthcheck` or `disk-usage` → Run Detail shows LIVE badge → log lines stream → on completion transitions to static viewer without page reload.
- **For the NEW `08-HUMAN-UAT.md` (index only):**
  ```yaml
  ---
  status: complete
  phase: 08-v1-final-human-uat-validation
  source:
    - 03-HUMAN-UAT.md
    - 06-HUMAN-UAT.md
    - 07-UAT.md
  started: <Phase 8 start>
  updated: <final edit>
  ---
  ```
  Body: a table listing every UAT file touched, the final status of each item, and any v1.1 backlog entries created.

**Confidence:** HIGH — shape is verified from existing files.

**Planner hook:** The planner should NOT invent new frontmatter fields. Reuse the existing schema exactly. The `re_tested_at:` field is a minor addition to individual test entries only, not a frontmatter change.

### Decision 14: `.planning/BACKLOG.md` — FILE DOES NOT EXIST — create it with 999.x convention

**Question:** Does `BACKLOG.md` exist? What's the 999.x numbering convention?

**Findings:**
- [VERIFIED: filesystem check] `.planning/BACKLOG.md` does NOT exist on the current branch.
- No existing backlog document in the project using 999.x numbering. The convention is **implicit** from CONTEXT.md D-27 and is mentioned as "the existing 999.x numbering convention per project docs" — but no such convention is documented elsewhere.
- **The planner must establish the convention in this phase.** Recommended shape (derived from `REQUIREMENTS.md` requirement-ID style):
  ```markdown
  # Cronduit v1.1+ Backlog

  **Established:** 2026-04-1X (Phase 8)
  **Convention:** Every entry uses a 999.NN ID (999.01, 999.02, ...) prefixed by a subsystem tag for readability.

  ## Entries

  ### 999.01 — BACKLOG-ID-SLUG

  **Originated:** <UAT-file>:<line-ref>
  **Observed:** <behavior seen during walkthrough>
  **Expected:** <design intent>
  **Why not a v1.0 blocker:** <one-sentence justification per D-26>

  ---
  ```
- **Numbering rationale:** 999.x places backlog entries "beyond all v1 requirement IDs" (highest v1 ID is around 86 mapped requirements). Zero chance of collision with v1 or planned v2 IDs. Subsystem prefix (e.g., `999.01 UI-POLISH-...`) keeps them scannable.

**Confidence:** MEDIUM on the exact shape (no precedent in-repo); HIGH on "create the file and establish the convention inside Phase 8."

**Planner hook:** One task in Phase 8 creates `BACKLOG.md` with a header, an empty "Entries" section, and a brief "Convention" block. Subsequent UAT-walkthrough tasks append entries as they're surfaced during the walkthrough. Final commit includes the file even if empty — the convention itself is a Phase 8 deliverable.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Non-root docker.sock access on macOS | Custom socat bridge in cronduit | `tecnativa/docker-socket-proxy` | Maintained, allowlist-driven, defense-in-depth by default |
| Docker daemon health check | Custom HTTP probe to the daemon | `bollard::Docker::ping()` | One-liner, async, returns structured error, already transitively imported |
| YAML frontmatter schema for UAT files | New schema | Copy `03-HUMAN-UAT.md` / `07-UAT.md` verbatim | Canonical template exists, D-24 locks this |
| Multi-compose-file CI orchestration | Two separate compose-smoke jobs | One job with `strategy.matrix` on the compose file | D-22 forbids splitting; matrix keeps single boot cycle per axis |
| Named-volume UID migration | In-process chown at startup | Documented manual `docker run --rm -v ...` one-liner | Rust musl-static cronduit cannot chown without running as root, which violates D-04 |
| JSON polling in CI shell step | Parse JSON with grep/awk | Use `jq` (preinstalled on ubuntu-latest) | More robust, better error messages, D-19 already suggests it |

## Common Pitfalls

### Pitfall 1: `connect_with_local_defaults` silently ignores DOCKER_HOST

**What goes wrong:** The secure compose variant sets `DOCKER_HOST=tcp://dockerproxy:2375` on the cronduit container, but bollard still tries `/var/run/docker.sock` (which isn't mounted in the secure variant). Pre-flight succeeds locally because the dev has the local docker daemon; fails in CI with "client error (Connect)."

**Why it happens:** `connect_with_local_defaults()` is a convenience wrapper around `connect_with_unix_defaults()` that never inspects environment variables. The planner reading `D-10` assumes bollard "reads DOCKER_HOST the same way the docker CLI does" — true only for `connect_with_defaults()`.

**How to avoid:** Change `src/cli/run.rs:150` to `Docker::connect_with_defaults()`. Add a test in the secure CI axis that verifies the pre-flight log line interpolates the TCP URI, not `/var/run/docker.sock`.

**Warning signs:** Secure-axis CI failures with "Error in the hyper legacy client: client error (Connect)" specifically mentioning `/var/run/docker.sock` even though the compose file doesn't mount it.

### Pitfall 2: Existing named volumes retain UID 65532 after rebase

**What goes wrong:** Users upgrading from `ghcr.io/simplicityguy/cronduit:0.1.x` (Phase 6 image) to the Phase 8 alpine image find that cronduit cannot write to its SQLite database on the `cronduit-data` named volume. The volume was chowned 65532:65532 on first mount by the old image; Docker does not re-apply image ownership on subsequent mounts.

**Why it happens:** Docker's "populate volume from image" behavior only runs on FIRST mount of an empty volume. Subsequent mounts just attach the existing volume as-is.

**How to avoid:**
- Document the migration in CHANGELOG / RELEASE NOTES with the exact one-liner: `docker run --rm -v cronduit-data:/data alpine chown -R 1000:1000 /data`
- Reference the chown step from the README SECURITY section (one line: "upgrading from 0.1.x? your named volume needs a one-time chown — see CHANGELOG").
- Optionally, add a boot-time detection: if the process can `open(2)` `/data` for writing, continue; if it fails with EACCES, emit a loud ERROR pointing at the CHANGELOG entry and exit 1. This is a nice-to-have; the documentation alone is sufficient for v1.0.

**Warning signs:** First-run error "unable to open database file" or EACCES from sqlx on an existing volume.

### Pitfall 3: busybox wget TLS failure from missing ca-certificates

**What goes wrong:** If the apk layer fails to install `ca-certificates` (transient network error at build time, or an accidental removal), the `http-healthcheck` example job fails at TLS handshake. busybox wget falls back to "insecure" behavior or errors out, and cronduit records the run as `status=failed` with a confusing error message.

**Why it happens:** Alpine's busybox wget uses an external `ssl_client` helper that depends on the system CA bundle from `ca-certificates`. Without the bundle, cert verification fails.

**How to avoid:** Keep `ca-certificates` in the same RUN layer as the user creation. Add a build-time sanity check: `RUN wget -q --spider https://example.com` inside the runtime stage (only runs during build, not at container start). Alternatively, the CI smoke test already exercises this path via the `http-healthcheck` job success assertion.

**Warning signs:** CI smoke test failure specifically on `http-healthcheck` job with "SSL: handshake failed" or "wget: bad address" in the job logs.

### Pitfall 4: POST=1 over-grants the socket-proxy

**What goes wrong:** D-10's `CONTAINERS=1, IMAGES=1, POST=1` floor grants `POST` globally. This includes `/containers/create`, `/containers/{id}/start`, `/containers/{id}/stop`, `/containers/{id}/kill`, `/containers/{id}/exec`, `/images/create`, and every other POST endpoint. A compromised cronduit process can exec arbitrary code in any container the proxy can see.

**Why it happens:** `POST=1` is a broad switch. The proxy supports finer grained flags: `ALLOW_START=1`, `ALLOW_STOP=1`, `ALLOW_RESTARTS=1`. For cronduit's actual workflow (pull + create + start + wait + logs + remove), the minimal-but-tight set is `CONTAINERS=1, IMAGES=1, NETWORKS=1, POST=1` — the broader `POST=1` is needed because the proxy does not currently have granular allowlists for "create" vs "exec" within the containers endpoint.

**How to avoid:** Ship `POST=1` for v1.0 (matches D-10 floor, simpler to explain). Add a v1.1 backlog entry to evaluate tighter allowlists once the exact set of endpoints cronduit uses is confirmed (probably requires tracing bollard calls in production for a week to catch edge cases).

**Warning signs:** None functional — this is a defense-in-depth tightening, not a correctness issue.

### Pitfall 5: Matrix axis compose file path drift

**What goes wrong:** The existing `compose-smoke` job does `sed -i 's|ghcr.io/simplicityguy/cronduit:latest|cronduit:ci|g' examples/docker-compose.yml` (ci.yml:159) to swap the image reference. After matrixing, the same sed needs to run against the chosen compose file, not the hard-coded path.

**Why it happens:** The existing job was written for a single file. Parameterizing introduces a variable that must thread through every step.

**How to avoid:** Store `matrix.compose.file` in an env var at the step level; reference it in every shell step. Example:
```yaml
env:
  COMPOSE_FILE: ${{ matrix.compose.file }}
steps:
  - run: sed -i 's|ghcr.io/simplicityguy/cronduit:latest|cronduit:ci|g' examples/${{ env.COMPOSE_FILE }}
  - run: docker compose -f ${{ env.COMPOSE_FILE }} up -d
    working-directory: examples
```
And verify BOTH files use the same image reference string before the sed (the secure variant should also reference `ghcr.io/simplicityguy/cronduit:latest` for its cronduit service — the socket-proxy sidecar uses `tecnativa/docker-socket-proxy:latest`, which does NOT need rewriting since it's published separately).

**Warning signs:** CI failure "cronduit:ci image not found" only on one matrix axis.

### Pitfall 6: `http-healthcheck` and `disk-usage` run concurrently during smoke test

**What goes wrong:** The smoke test triggers `POST /api/jobs/{id}/run` for all 4 jobs in rapid succession. All 4 runs start in parallel. SQLite writer lock contention may cause some runs to stall if the write pool is saturated. Unlikely with 4 jobs and a single-connection writer pool, but possible on slow CI runners.

**Why it happens:** cronduit's Run Now endpoint is fire-and-forget (SCHED-06: "concurrent runs of the same job are allowed"). Rapidly queuing 4 jobs creates a parallel burst.

**How to avoid:** Trigger jobs sequentially in the smoke test, not in a parallel burst. Shell loop:
```bash
for JOB_ID in "${JOB_IDS[@]}"; do
  curl -sSf -X POST "http://127.0.0.1:8080/api/jobs/${JOB_ID}/run"
  # Poll until success or timeout
  DEADLINE=$(( $(date +%s) + 120 ))
  while [ $(date +%s) -lt $DEADLINE ]; do
    STATUS=$(curl -sSf "http://127.0.0.1:8080/api/jobs/${JOB_ID}/runs?limit=1" | jq -r '.[0].status')
    [ "$STATUS" = "success" ] && break
    sleep 2
  done
  [ "$STATUS" = "success" ] || { echo "job $JOB_ID did not reach success: $STATUS"; exit 1; }
done
```
Sequential with a per-job 120s budget keeps total CI time bounded at 4×120s = 480s worst case, but in practice each job completes in 2-5s so total is ~20s.

**Warning signs:** CI flakes with "status=running" at the 120s timeout for exactly one job, random which.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Docker daemon (local) | Plan execution, manual UAT | ✓ (assumed) | 24+ | — |
| Docker Compose v2+ | Smoke test, UAT | ✓ | current | — |
| `alpine:3` base image | Runtime rebase | ✓ (pull at build time) | 3.22+ | — |
| `tecnativa/docker-socket-proxy:latest` | Secure compose | ✓ (pull at test time) | latest | — |
| `ghcr.io` egress | Image publish | ✓ | — | — |
| `https://example.com` egress | `http-healthcheck` wget validation | ✓ | — | Skip HTTPS test, use `http://` (loses TLS coverage) |
| `jq` on ubuntu-latest runner | Smoke test JSON parsing | ✓ | preinstalled | Shell grep substitute |
| `cargo-zigbuild` | Builder stage | ✓ | 0.22+ | — |

**Missing dependencies with no fallback:** None — everything needed is already in the existing build / CI pipeline.

**Missing dependencies with fallback:** None.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` + `cargo nextest` (existing), GitHub Actions `compose-smoke` job, human UAT files |
| Config file | `Cargo.toml` (test deps), `.github/workflows/ci.yml` (CI gate) |
| Quick run command | `cargo test --lib` |
| Full suite command | `cargo nextest run --all-features --profile ci` (per Phase 1 convention) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| UI-05 | Terminal-green theme renders correctly | manual-only | `03-HUMAN-UAT.md` Test 1 (in-place flip) | ✅ |
| UI-06 | Dashboard dark/light toggle persistence | manual-only | `03-HUMAN-UAT.md` Test 2 | ✅ |
| UI-09 | ANSI logs render with stderr distinction | manual-only | `03-HUMAN-UAT.md` Test 4 | ✅ |
| UI-12 | Run Now toast appears | manual-only | `03-HUMAN-UAT.md` Test 3 | ✅ |
| OPS-05 | Quickstart end-to-end (stranger under 5 min) | integration + manual | CI: `compose-smoke` matrix; Human: NEW `06-HUMAN-UAT.md` Test 1 | ✅ CI / ❌ Wave 0 (new UAT file) |
| UI-14 | SSE live log streaming | manual-only | NEW `06-HUMAN-UAT.md` Test 2 | ❌ Wave 0 (new UAT file) |
| D-01..D-06 | Runtime rebase (alpine + UID 1000) | integration | CI: `compose-smoke` `/health` + job-load + 4-job success | ✅ CI |
| D-07..D-10 | Dual compose file matrix | integration | CI: `compose-smoke` matrix axis (both files) | ❌ Wave 0 (ci.yml change) |
| D-11..D-14 | Docker pre-flight (WARN + gauge) | unit + integration + manual | Unit: metrics registration test; Integration: `compose-smoke` secure axis flips gauge to 1; Manual: stop daemon and boot cronduit, verify WARN + gauge=0 | ❌ Wave 0 (new Rust test) |
| D-15..D-17 | 4-job example set | integration | CI: `compose-smoke` asserts each job reaches `status=success` within 120s | ❌ Wave 0 (ci.yml + cronduit.toml changes) |
| D-18..D-22 | Cold-start smoke test (matrix) | CI | `compose-smoke` extended with job-success polling + matrix | ❌ Wave 0 (ci.yml change) |
| D-23..D-28 | Human UAT walkthrough + backlog | manual + doc | UAT files (pass/fail annotations); NEW `BACKLOG.md` | ❌ Wave 0 (new docs) |

### Sampling Rate

- **Per task commit:** `cargo test --lib` (Rust unit tests for pre-flight metrics wiring).
- **Per wave merge:** `cargo nextest run --all-features --profile ci` + local `docker compose -f examples/docker-compose.yml up -d` sanity check.
- **Phase gate (pre-PR merge):** Full CI including `compose-smoke` matrix on both files; all 4 example jobs reach success; `/metrics` shows `cronduit_docker_reachable 1` on the secure axis; `/metrics` shows `cronduit_docker_reachable 0` on a manual "daemon stopped" test before UAT sign-off.
- **Pre-archive (v1.0):** All UAT files flipped to `status: complete` or `status: partial` with documented blocks. `BACKLOG.md` exists with at minimum the schema established.

### Wave 0 Gaps

- [ ] **`src/telemetry.rs`** — add `describe_gauge!("cronduit_docker_reachable", ...)` + zero-baseline `.set(0.0)` alongside the other Phase 6 metric families.
- [ ] **`src/cli/run.rs:150`** — switch `connect_with_local_defaults()` → `connect_with_defaults()` so `DOCKER_HOST` is honored.
- [ ] **NEW `src/scheduler/docker_preflight.rs` (or extend existing)** — add `pub async fn preflight_daemon(docker: &Docker) -> ()` that calls `docker.ping().await`, emits INFO or WARN, and flips the gauge. Called from `src/cli/run.rs` after `connect_with_defaults()`.
- [ ] **NEW unit test** — verify `cronduit_docker_reachable` gauge registers in `/metrics` output at value 0 before any ping, and that a direct `.set(1.0)` / `.set(0.0)` flips the rendered value. Belongs in `tests/metrics_registration.rs` or the existing metrics test file.
- [ ] **`Dockerfile`** — rebase runtime stage. Verify `cargo tree -i openssl-sys` still empty (FOUND-06). No source files change; the builder stage is untouched.
- [ ] **`examples/cronduit.toml`** — rewrite job section for D-15 four-job mix. Preserve SECURITY block.
- [ ] **`examples/docker-compose.yml`** — add `group_add` + inline DOCKER_GID derivation comment.
- [ ] **NEW `examples/docker-compose.secure.yml`** — full dual-compose with socket-proxy sidecar.
- [ ] **`.github/workflows/ci.yml`** — matrix-ify `compose-smoke`, add job-success polling step with 120s budget + D-20 diagnostic dump.
- [ ] **NEW `.planning/BACKLOG.md`** — v1.1 backlog seed with 999.x convention header.
- [ ] **NEW `.planning/phases/06-*/06-HUMAN-UAT.md`** — quickstart e2e + SSE live-stream test scripts.
- [ ] **NEW `.planning/phases/08-*/08-HUMAN-UAT.md`** — index file linking all touched UAT files.
- [ ] **In-place edits to `03-HUMAN-UAT.md` and `07-UAT.md`** — flip pending items; add `re_tested_at:` to Tests 2+3.
- [ ] **CHANGELOG.md / RELEASE-NOTES.md** — UID 65532 → 1000 migration one-liner + README SECURITY link.

### Nyquist Dimension 8 — Sampling per decision

| Decision | Validation | Sampling |
|----------|-----------|----------|
| D-01..D-06 (runtime rebase) | CI matrix both files + `/health` | Per PR |
| D-07..D-10 (dual compose) | CI matrix | Per PR |
| D-10 (socket-proxy allowlist) | Manual: `curl dockerproxy:2375/v1.41/containers/json` with each flag revoked | Pre-merge one-shot |
| D-11..D-14 (pre-flight) | Unit test + CI + manual "daemon stopped" | Unit: every commit; CI: every PR; Manual: pre-merge |
| D-15..D-17 (4-job mix) | CI job-success polling | Per PR |
| D-18..D-22 (CI smoke extension) | Self-validating | Per PR |
| D-23..D-25 (human UAT) | User walkthrough | Pre-archive one-shot |
| D-26..D-28 (v1.1 backlog) | Doc review | Pre-archive one-shot |

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V1 Architecture | yes | Threat model in `THREAT_MODEL.md` is being extended with the "Docker socket via proxy" pattern — the secure compose file IS a new mitigation, not a new attack surface. |
| V2 Authentication | no | No auth in v1 (CONTEXT.md locked). |
| V4 Access Control | yes | socket-proxy enforces endpoint-level access control on the Docker API. |
| V5 Input Validation | yes | `DOCKER_HOST` env var parsing trusts bollard — bollard validates URI format. No new user input vectors. |
| V6 Cryptography | yes | TLS for image registry pulls (bollard → registry) continues via `ca-certificates` in runtime. busybox wget HTTPS for `http-healthcheck` is validated cert'd (alpine's `ssl_client`). No hand-rolled crypto. |
| V14 Configuration | yes | `examples/cronduit.toml` continues to mount `:ro`. New `docker-compose.secure.yml` must also mount config `:ro`. |

### Known Threat Patterns for this phase

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Untrusted image registry MITM | Tampering | TLS verification via `ca-certificates` (retained in alpine layer) |
| Docker socket over-privilege on macOS | Elevation of Privilege | Socket proxy with allowlist — explicit D-07/D-10 decision |
| `DOCKER_HOST` spoofing | Spoofing | Only set via docker-compose env (operator-controlled); no user input path |
| Privilege escalation via UID mismatch | EoP | UID 1000 non-root; chown migration documented manually; no in-process setuid |
| Compromised cronduit exec'ing in other containers | EoP | `POST=1` is the v1.0 floor; v1.1 backlog item to tighten to `ALLOW_START=1` + deny `EXEC=1` |
| Loopback bypass via non-loopback bind | Spoofing | Existing Phase 1 loopback-default + WARN on non-loopback bind (unchanged) |

## Project Constraints (from CLAUDE.md)

Extracted from `./CLAUDE.md` and memory rules — the planner must verify all Phase 8 plans respect these:

- **Locked tech stack:** Rust, bollard, sqlx, axum 0.8, askama_web (NOT askama_axum), HTMX vendored, Tailwind standalone, croner 3.0, TOML config, rustls everywhere.
- **Docker socket access (locked):** bollard, NEVER shell out to the `docker` CLI from cronduit code.
- **Mermaid-only diagrams:** Any diagram in Phase 8 plan docs, CHANGELOG entries, PR descriptions, or code comments MUST be a mermaid code block. No ASCII art.
- **PR-only workflow:** No direct commits to `main`. Phase 8 lands via feature branch + PR with the full CI matrix green.
- **UAT requires user validation:** Per project memory rule. Claude prepares fixtures for UAT but does NOT mark UAT items passed from its own runs. D-25 reinforces this.
- **No emoji in CLAUDE files:** Project convention — no emoji in code, docs, or CLAUDE.md updates unless the user explicitly requests it.
- **Security-first README:** README already leads with SECURITY section (established Phase 1). Any Phase 8 changes to the README quickstart must preserve this structure.
- **No plaintext secrets in config:** `SecretString` wrapping + env-var interpolation via `${VAR}`. The new example jobs use no secrets; safe.
- **Workflow enforcement:** All file-changing tools must be invoked from within a GSD command. Phase 8 runs through `/gsd-execute-phase` per the CLAUDE.md GSD Workflow Enforcement block.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Existing `compose-smoke` job uses `${{ matrix }}` syntax cleanly with current GitHub Actions syntax | Decision 9 / Pitfall 5 | Low — matrix is a well-established feature since 2019 |
| A2 | `ubuntu-latest` runner in April 2026 still ships `jq` by default | Decision 9 | Low — jq has shipped since 2020; GitHub would document removal |
| A3 | `tecnativa/docker-socket-proxy` still uses port 2375 as default and has not renamed its env vars in 2026 | Decision 4 | Low — image is stable; verify via `docker pull tecnativa/docker-socket-proxy` and `docker inspect` before merge |
| A4 | The Phase 6 `docker_pull.rs` retry logic passes through the same `Docker` client handle as the pre-flight, so switching to `connect_with_defaults` does not require plumbing changes elsewhere | Decision 3 | Medium — quick grep of `docker_pull.rs` + `docker.rs` to confirm is sufficient |
| A5 | busybox wget on the current `alpine:3` tag has the same TLS behavior as the 2024 patches documented | Decision 2 | Low — verify via `RUN wget https://example.com` in a test build |
| A6 | The existing `describe_gauge!` macro call pattern in `src/telemetry.rs::setup_metrics()` is idempotent on repeated `install_recorder()` calls via the `OnceLock` memoization (Phase 6 WR-01 fix) | Decision 12 | Low — already verified in Phase 6 re-verification |
| A7 | The existing `src/cli/run.rs` pre-flight boot sequence (config → pool → sync → metrics → docker → scheduler) can accommodate a new pre-flight step after `connect_with_defaults` and before `retention_pruner` without breaking integration tests | Decision 6 / Plan ordering | Low — the new step is `tokio`-native async and reads-only; no ordering sensitivity |

**Confirm before execution:** The planner should quickly spot-check A4 (code path of `Docker` handle through `docker_pull.rs`) and A5 (the TLS behavior of busybox wget on the current alpine:3 tag) via a one-shot `docker run` verification during planning. The rest can proceed on documented convention.

## Open Questions (RESOLVED)

1. **Does `POST=1` in docker-socket-proxy include `DELETE` verbs?**
   - What we know: The proxy's README describes POST as "write operations" but doesn't explicitly enumerate DELETE.
   - What's unclear: `DELETE /containers/{id}` is the method cronduit uses to remove containers (DOCKER-06). Need to confirm this hits the same allowlist as POST.
   - Recommendation: In the secure compose file, add a one-line comment noting this and verify during `compose-smoke` secure-axis runs. If `delete = true` jobs fail with 403, the allowlist needs explicit `DELETE=1` or similar — verify via the smoke test before declaring victory.
   - **RESOLVED (2026-04-13):** Per the `tecnativa/docker-socket-proxy` README (https://github.com/Tecnativa/docker-socket-proxy), each HTTP verb is a **separate** environment flag: `POST=1` enables POST verbs only; `DELETE=1` is a distinct permission. Bollard's `remove_container` call (invoked when a job has `delete = true`) issues `DELETE /containers/{id}`, so `POST=1` alone is **insufficient** for the quickstart `hello-world` job. **Action:** add `DELETE=1` to the `dockerproxy` environment block in `examples/docker-compose.secure.yml`, and propagate to CONTEXT.md as **D-29**. Fallback if the smoke test still fails: check the socket-proxy container logs for the exact denied verb and expand the allowlist incrementally. This is implemented in Plan 08-02 Task 2 (allowlist + acceptance criterion) and validated in Plan 08-04's `compose-smoke-secure` matrix axis (the `hello-world` job with `delete = true` must reach `status=success`).

2. **Should `cronduit_docker_reachable` re-check happen on every config reload, or only when docker jobs exist in the new config?**
   - What we know: D-13 says "startup and on explicit config reload."
   - What's unclear: If the new config has zero docker jobs, the ping is wasted effort. If docker jobs are added, the ping is useful.
   - Recommendation: Always re-ping on reload. The cost is a single HTTP request to `/_ping` (<10ms typical); the benefit is a consistent gauge that reflects current daemon reachability regardless of job mix. Simpler than conditional logic.
   - **RESOLVED (2026-04-13):** Per D-13, the pre-flight ping fires on startup and on explicit config reload (SIGHUP or API-triggered). It does **not** fire per-job — the gauge is a coarse liveness signal, not a per-run check. Conditional logic (skip the ping when no docker jobs exist in the new config) is explicitly **not** adopted: the gauge must reflect current daemon state regardless of whether any job currently needs it, so alerting rules on `cronduit_docker_reachable == 0` fire consistently. Plan 08-03 wires the ping at startup only; the reload hook is a separate follow-on task tracked in D-13's scope and is not a Phase 8 plan task.

3. **Should the secure compose file bind `127.0.0.1:8080:8080` or stay `0.0.0.0:8080:8080` like the simple variant?**
   - What we know: Phase 6 D-12 / Phase 7 D-01 locked `ports: 8080:8080` (= `0.0.0.0:8080`) for the simple quickstart. The secure variant is the "production/defense-in-depth" recipe.
   - What's unclear: Should the secure variant tighten the bind to loopback as well? That would make the secure recipe a true "production template" — socket proxy + loopback bind + reverse proxy expected.
   - Recommendation: Bind the secure variant to `127.0.0.1:8080:8080` and document that reverse proxies see loopback. This makes the security story internally consistent: simple = accessible, secure = defense-in-depth. Requires a comment block explaining the bind change to users copy-pasting between the two files.
   - **RESOLVED (2026-04-13):** Keep `ports: "8080:8080"` (= `0.0.0.0:8080`) in **both** compose files — consistent with the Phase 7 D-01 override that kept the quickstart accessible from the operator's local workstation without requiring a `.env` file edit. The secure variant's defense-in-depth is the `docker-socket-proxy` sidecar (narrow allowlist, no host socket mount in the cronduit service), **not** the bind address. Operators running `docker-compose.secure.yml` behind a reverse proxy can flip to loopback via `.env` override (`CRONDUIT_BIND=127.0.0.1:8080` or a compose override file); the SECURITY comment block in Plan 08-02 Task 2 documents this pattern. Rationale: changing the bind between the two files would break the copy-paste promise between quickstart variants, and the socket-proxy's network-level isolation (cronduit can only reach allowlisted Docker API endpoints over the private bridge network) is the stronger control. See Phase 7 D-01 for the original 0.0.0.0 bind decision.

## Code Examples

### Docker pre-flight call site (new function)

```rust
// src/scheduler/docker_preflight.rs (extend existing file)

use bollard::Docker;

/// Check Docker daemon reachability at startup and on config reload.
/// Non-fatal — cronduit continues to boot even if the daemon is unreachable,
/// so command/script-only configs still work. Flips the
/// `cronduit_docker_reachable` gauge and logs INFO/WARN accordingly.
pub async fn ping_daemon(docker: &Docker) {
    let uri = std::env::var("DOCKER_HOST")
        .unwrap_or_else(|_| "/var/run/docker.sock".to_string());

    match docker.ping().await {
        Ok(()) => {
            tracing::info!(
                target: "cronduit.docker",
                uri = %uri,
                "docker daemon reachable"
            );
            metrics::gauge!("cronduit_docker_reachable").set(1.0);
        }
        Err(e) => {
            tracing::warn!(
                target: "cronduit.docker",
                uri = %uri,
                error = %e,
                "docker daemon unreachable at {uri}: {e}. cronduit will continue to schedule command/script jobs. docker jobs will fail until the daemon is reachable. remediation: verify /var/run/docker.sock is mounted, check group_add / DOCKER_GID in docker-compose.yml, or switch to examples/docker-compose.secure.yml on macOS / Docker Desktop."
            );
            metrics::gauge!("cronduit_docker_reachable").set(0.0);
        }
    }
}
```

### Call-site wiring (`src/cli/run.rs`)

```rust
// src/cli/run.rs — around line 150 — change:
let docker = match bollard::Docker::connect_with_defaults() {
    Ok(d) => {
        tracing::info!(target: "cronduit.startup", "Docker client created");
        // New: pre-flight ping to populate gauge + surface early WARN.
        crate::scheduler::docker_preflight::ping_daemon(&d).await;
        Some(d)
    }
    Err(e) => {
        tracing::warn!(
            target: "cronduit.startup",
            error = %e,
            "Docker client unavailable — docker-type jobs will fail"
        );
        // Ensure gauge is 0 even if we never got a client handle.
        metrics::gauge!("cronduit_docker_reachable").set(0.0);
        None
    }
};
```

### `setup_metrics()` addition (`src/telemetry.rs`)

```rust
// Add alongside the other describe_* calls (after line 110 in setup_metrics):
metrics::describe_gauge!(
    "cronduit_docker_reachable",
    "1 if the Docker daemon was reachable at startup / last reload, 0 otherwise"
);
// Zero-baseline registration so the family appears in /metrics from boot.
metrics::gauge!("cronduit_docker_reachable").set(0.0);
```

### New example job block (`examples/cronduit.toml`)

```toml
# HTTP healthcheck — realistic uptime canary.
# Uses busybox wget (bundled with alpine) over TLS against the IANA reserved
# example domain. Validates DNS + TLS + egress from the container.
[[jobs]]
name = "http-healthcheck"
schedule = "*/5 * * * *"
command = "wget -q -S --spider https://example.com 2>&1 | head -10"

# Disk usage — demonstrates the script-job path and the /data volume.
# Handles the "/data not mounted" case cleanly. The trailing sleep 3
# demonstrates a longer-running script for the UI Run Detail page.
[[jobs]]
name = "disk-usage"
schedule = "*/15 * * * *"
script = """#!/bin/sh
echo "=== disk usage check ==="
du -sh /data 2>/dev/null || echo "/data not mounted"
df -h /data 2>/dev/null || true
sleep 3
echo "=== check complete ==="
"""
```

### Secure compose file (`examples/docker-compose.secure.yml`) — skeleton

```yaml
# Cronduit secure quickstart — defense-in-depth variant.
#
# ============================================================================
# SECURITY: WHY THIS FILE EXISTS
# ============================================================================
# The default examples/docker-compose.yml mounts /var/run/docker.sock
# directly into the cronduit container with group_add for the host docker
# group. That works on Linux where the docker group GID is consistent
# between host and container, BUT:
#
#   * On Docker Desktop macOS, the socket inside the Linux VM is root-owned
#     with no host-side GID mapping. group_add does not work there.
#   * Direct socket mounts give the process full Docker API access, which
#     is effectively root on the host. See THREAT_MODEL.md § Docker Socket.
#
# This file uses tecnativa/docker-socket-proxy as a sidecar. The proxy has
# privileged socket access, but exposes only a narrow TCP API to cronduit
# on dockerproxy:2375. cronduit never touches the host socket.
#
# Use this file on:
#   * Docker Desktop macOS (it is the only supported path there)
#   * Any "production-ish" homelab deployment where minimizing Docker API
#     exposure is worth the extra container
#
# Prerequisites: Docker with Compose v2+
# Usage: docker compose -f examples/docker-compose.secure.yml up -d
# Web UI: http://127.0.0.1:8080 (loopback by default — front with a reverse proxy for remote access)

services:
  dockerproxy:
    image: tecnativa/docker-socket-proxy:latest
    restart: unless-stopped
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
    environment:
      # Minimum allowlist for cronduit's bollard ephemeral-container workflow.
      # CONTAINERS=1: create, start, wait, inspect, logs, remove
      - CONTAINERS=1
      # IMAGES=1: image pull (DOCKER-05)
      - IMAGES=1
      # NETWORKS=1: inspect named network (docker_preflight.rs validate_named_network)
      - NETWORKS=1
      # POST=1: required for create/start/wait/remove (DELETE verbs grouped with POST)
      - POST=1
      # Everything else default-deny: AUTH, SECRETS, EXEC, BUILD, VOLUMES, SYSTEM, INFO, ...
    # Dockerproxy is internal-only — do NOT publish port 2375 to the host.
    # cronduit reaches it via the compose-internal DNS name `dockerproxy`.

  cronduit:
    image: ghcr.io/simplicityguy/cronduit:latest
    depends_on:
      - dockerproxy
    ports:
      # Loopback bind by default. Remove the 127.0.0.1: prefix only if you
      # front cronduit with a reverse proxy with authentication.
      - "127.0.0.1:8080:8080"
    volumes:
      # NO socket mount — cronduit talks to dockerproxy over TCP instead.
      - ./cronduit.toml:/etc/cronduit/config.toml:ro
      - cronduit-data:/data
    environment:
      - RUST_LOG=info,cronduit=debug
      - DATABASE_URL=sqlite:///data/cronduit.db
      # Route bollard to the proxy instead of the host socket.
      - DOCKER_HOST=tcp://dockerproxy:2375
    restart: unless-stopped

volumes:
  cronduit-data:
```

### CI matrix extension (`.github/workflows/ci.yml`)

```yaml
  compose-smoke:
    name: quickstart compose smoke (${{ matrix.compose.name }})
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        compose:
          - file: docker-compose.yml
            name: simple
          - file: docker-compose.secure.yml
            name: secure
    env:
      COMPOSE_FILE: ${{ matrix.compose.file }}
    steps:
      - uses: actions/checkout@v4
      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3
      - name: Build local cronduit:ci image from PR checkout
        uses: docker/build-push-action@v6
        with:
          context: .
          file: Dockerfile
          platforms: linux/amd64
          push: false
          load: true
          tags: cronduit:ci
          cache-from: type=gha
          cache-to: type=gha,mode=max
      - name: Rewrite compose to use locally-built cronduit:ci image
        run: |
          sed -i 's|ghcr.io/simplicityguy/cronduit:latest|cronduit:ci|g' examples/${{ env.COMPOSE_FILE }}
          grep -q 'image: cronduit:ci' examples/${{ env.COMPOSE_FILE }} || {
            echo "ERROR: sed rewrite did not produce 'image: cronduit:ci' in compose file"
            exit 1
          }
      - name: docker compose up -d
        working-directory: examples
        run: docker compose -f ${{ env.COMPOSE_FILE }} up -d
      - name: Wait for /health (max 30s)
        run: |
          # ... existing logic unchanged ...
      - name: Assert dashboard lists all four quickstart jobs
        run: |
          set -eu
          dash=$(curl -sSf http://localhost:8080/)
          for JOB in echo-timestamp http-healthcheck disk-usage hello-world; do
            echo "$dash" | grep -q "$JOB" || { echo "ERROR: dashboard missing $JOB"; exit 1; }
          done
      - name: Assert all four jobs reach success within 120s each
        run: |
          set -eu
          JOB_IDS=$(curl -sSf http://localhost:8080/api/jobs | jq -r '.[].id')
          for JOB_ID in $JOB_IDS; do
            JOB_NAME=$(curl -sSf "http://localhost:8080/api/jobs/${JOB_ID}" | jq -r '.name')
            echo "Triggering $JOB_NAME ($JOB_ID)..."
            curl -sSf -X POST "http://localhost:8080/api/jobs/${JOB_ID}/run" >/dev/null
            DEADLINE=$(( $(date +%s) + 120 ))
            while [ $(date +%s) -lt $DEADLINE ]; do
              STATUS=$(curl -sSf "http://localhost:8080/api/jobs/${JOB_ID}/runs?limit=1" | jq -r '.[0].status // "pending"')
              [ "$STATUS" = "success" ] && { echo "$JOB_NAME -> success"; break; }
              [ "$STATUS" = "failed" ] && { echo "ERROR: $JOB_NAME -> failed"; exit 1; }
              sleep 2
            done
            if [ "$STATUS" != "success" ]; then
              echo "ERROR: $JOB_NAME did not reach success within 120s (last status: $STATUS)"
              exit 1
            fi
          done
      - name: Dump diagnostic bundle on failure
        if: failure()
        run: |
          docker compose -f ${{ env.COMPOSE_FILE }} logs cronduit --tail=200 || true
          docker compose -f ${{ env.COMPOSE_FILE }} logs dockerproxy --tail=50 || true
          curl -sSf http://localhost:8080/metrics | grep cronduit_docker_reachable || true
          for JOB_ID in $(curl -sSf http://localhost:8080/api/jobs 2>/dev/null | jq -r '.[].id' || echo ""); do
            curl -sSf "http://localhost:8080/api/jobs/${JOB_ID}/runs?limit=5" || true
          done
        working-directory: examples
      - name: Tear down compose stack
        if: always()
        working-directory: examples
        run: docker compose -f ${{ env.COMPOSE_FILE }} down -v
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Distroless runtime (Phase 1) | `alpine:3` runtime (Phase 8) | 2026-04-13 | Gains `/bin/sh` + busybox utilities; UID flips 65532 → 1000; named-volume migration required |
| Direct `/var/run/docker.sock` mount only | Optional `tecnativa/docker-socket-proxy` sidecar | 2026-04-13 | macOS support; defense-in-depth; secure-variant CI axis |
| `connect_with_local_defaults()` | `connect_with_defaults()` (DOCKER_HOST-aware) | 2026-04-13 | Enables the secure compose file via env var routing |
| `cronduit_scheduler_up` only gauge at boot | Add `cronduit_docker_reachable` | 2026-04-13 | Alertable daemon-health signal in Prometheus |
| 2-job example config | 4-job example config | 2026-04-13 | Every execution type (command, script, docker) demonstrated in quickstart |

## Sources

### Primary (HIGH confidence)
- [VERIFIED: Cargo.lock] `bollard = 0.20.2`, `metrics-exporter-prometheus = 0.18.x`, `metrics = 0.24.x` — exact versions in use
- [VERIFIED: filesystem] Dockerfile:53 (current runtime FROM), `src/cli/run.rs:150` (Docker::connect_with_local_defaults call site), `src/telemetry.rs:52-126` (setup_metrics pattern)
- [CITED: docs.rs/bollard/0.20.2/bollard/struct.Docker.html] `Docker::ping()` signature, `Docker::connect_with_defaults()` DOCKER_HOST handling
- [CITED: github.com/Tecnativa/docker-socket-proxy README] env-var allowlist, default port 2375, default-deny matrix
- [CITED: wiki.alpinelinux.org/wiki/BusyBox] alpine busybox applets (sh, date, wget, du, df, sleep) built-in
- [CITED: hub.docker.com/_/alpine] alpine:3 multi-arch coverage
- [VERIFIED: .planning/phases/07-v1-cleanup-bookkeeping/07-UAT.md] the three logged blockers (echo-timestamp ENOENT, hello-world Connect, Test 2 blocked)
- [VERIFIED: .planning/phases/03-read-only-web-ui-health-endpoint/03-HUMAN-UAT.md] the four pending visual UAT items
- [VERIFIED: .github/workflows/ci.yml:126-216] existing `compose-smoke` job structure

### Secondary (MEDIUM confidence)
- [CITED: drmhse.com/posts/fast-rust-docker-builds-with-zigbuild] cargo-zigbuild + alpine musl-static compatibility
- [CITED: forums.docker.com/t/mounting-using-var-run-docker-sock-in-a-container-not-running-as-root] Docker Desktop macOS socket ownership inside container
- [CITED: gitlab.alpinelinux.org/alpine/aports/-/issues/15861 + devendevour.wordpress.com] busybox wget HTTPS via ssl_client helper on alpine
- [CITED: docs.docker.com/engine/storage/volumes/] named-volume first-mount populate-from-image behavior
- [CITED: community forums on docker volume chown] existing-volume ownership stickiness

### Tertiary (LOW confidence / ASSUMED)
- [ASSUMED] `jq` preinstalled on `ubuntu-latest` runner in April 2026 (Assumption A2)
- [ASSUMED] `tecnativa/docker-socket-proxy:latest` image is stable and has not renamed env vars in 2026 (Assumption A3) — verify with `docker inspect` before merge
- [ASSUMED] `POST=1` in the proxy covers DELETE verbs for container removal (Open Question 1) — verify with a smoke-test run

## Metadata

**Confidence breakdown:**
- Runtime rebase (alpine:3, UID 1000, apk layer): HIGH — well-understood territory, all components verified
- Dual compose file set: HIGH — socket-proxy is the canonical community pattern
- `Docker::ping()` + gauge wiring: HIGH — API and registration pattern both verified
- `connect_with_defaults` code change: HIGH — signature and DOCKER_HOST semantics verified; this is the load-bearing fix
- Socket-proxy allowlist correctness: MEDIUM — D-10 floor is missing `NETWORKS=1` and needs to be validated with a real smoke test run against the secure variant
- CI matrix extension: HIGH — pattern is standard GitHub Actions
- Human UAT choreography: HIGH — existing frontmatter schemas are verified
- v1.1 backlog shape: MEDIUM — no in-repo precedent, planner establishes the convention
- Named-volume UID migration story: HIGH on behavior / MEDIUM on "is documentation-only sufficient for v1.0" — leaning HIGH because in-process rescue is genuinely not worth the complexity

**Research date:** 2026-04-13
**Valid until:** 2026-05-13 (alpine base image updates monthly; `tecnativa/docker-socket-proxy` is stable)
