# Phase 8: v1.0 Final Human UAT Validation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-13
**Phase:** 08-v1-final-human-uat-validation
**Areas discussed:** echo-timestamp fix approach, docker.sock + pre-flight strategy, UAT file layout + v1.1 backlog policy, cold-start smoke test scope, examples expansion

---

## Gray Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| echo-timestamp fix approach | distroless:static has no /bin/date, no shell, no busybox — both `command=` and `script=` jobs are structurally broken | ✓ |
| docker.sock + pre-flight strategy | bollard can't reach /var/run/docker.sock from distroless:nonroot (UID 65532, no docker gid) | ✓ |
| UAT file layout + v1.1 backlog policy | Where Phase 8 UAT results land and the rule for 'fix in this phase vs defer to v1.1 backlog' | ✓ |
| Cold-start smoke test scope | Where the cold-start smoke test lives and what it asserts | ✓ |

**User notes on selection:** *"We need a few more examples. The docker-based examples are great. But we also need to have one or two examples that don't run in a container."*

---

## Runtime Rebase Strategy (echo-timestamp fix)

**Question:** Given the distroless:static runtime has no shell or coreutils and the user wants 1-2 working non-container examples, how should we enable command/script jobs in the quickstart runtime?

| Option | Description | Selected |
|--------|-------------|----------|
| Bake a static busybox into the runtime | Download official static busybox, symlink common applets. Preserves distroless-nonroot security posture. ~1 MB footprint. | |
| Rebase runtime on distroless/base-debian12:nonroot | Adds libc/ssl but still no shell — doesn't actually unlock command/script jobs. | |
| Rebase runtime on alpine:3 or debian:slim | Full shell + coreutils. Trades distroless security story for normal attack surface. | ✓ |
| Bake a purpose-built `cronduit-demo` helper binary | Single-purpose, narrow fix. Users writing real non-Docker jobs still hit the shell-less wall. | |

**User's choice:** Rebase runtime on alpine:3 or debian:slim
**Rationale:** The user wants real, flexible non-Docker example jobs, not a single canned demo binary. Full shell + coreutils is the cleanest path even though it walks back part of the Phase 1 distroless security posture.

---

## Alpine vs Debian Base Image

**Question:** Which base image should the runtime rebase onto?

| Option | Description | Selected |
|--------|-------------|----------|
| alpine:3 | ~5 MB, musl libc matching the cronduit binary, busybox built in | ✓ |
| debian:bookworm-slim | ~28 MB, glibc, full bash + coreutils + apt, more familiar to ops teams | |
| alpine:3 with apk-based pin-file | alpine:3 plus explicit apk pin-file for reproducibility | |

**User's choice:** alpine:3

---

## Runtime User Strategy

**Question:** Run cronduit as a dedicated non-root user in the new image, or simplify and accept root?

| Option | Description | Selected |
|--------|-------------|----------|
| Create `cronduit` user (UID/GID 1000), run as non-root | Keeps blast radius small. docker.sock still needs group_add or socket-proxy. | |
| Accept root:root for the quickstart image | Dodges docker.sock issues but contradicts Phase 6 threat model. | |
| Create `cronduit` user AND document the group_add recipe in compose | Non-root plus baked-in `group_add:` guidance. macOS users still need socket-proxy. | ✓ |

**User's choice:** Create `cronduit` user AND document the group_add recipe in compose

---

## docker.sock Access Strategy

**Question:** What should the default docker.sock access recipe in `examples/docker-compose.yml` look like?

| Option | Description | Selected |
|--------|-------------|----------|
| Numeric group_add with detected GID + Linux/macOS comments | Single compose file, `group_add: ["${DOCKER_GID:-999}"]`, README snippet for Linux GID lookup | |
| Default to tecnativa/docker-socket-proxy sidecar | Single compose file with socket-proxy as the default; most secure but more moving parts | |
| Dual examples: one minimal compose + one socket-proxy compose | Two files: `docker-compose.yml` (group_add) and `docker-compose.secure.yml` (socket-proxy) | ✓ |

**User's choice:** Dual examples (minimal + socket-proxy)
**Rationale:** Gives users a choice between simplicity and defense-in-depth without forcing either. macOS users get a first-class recipe, not a footnote.

---

## Docker Daemon Pre-flight Check

**Question:** What should the startup 'docker daemon unreachable' pre-flight do when bollard can't connect?

| Option | Description | Selected |
|--------|-------------|----------|
| WARN + set a gauge metric, never fail | Logs remediation hints, sets `cronduit_docker_reachable{}` to 0, boot continues | ✓ |
| WARN only, no metric | Simpler but harder to alert on | |
| Fail-fast IF any enabled job has type=docker | Loud, but transient daemon flaps would kill the process | |

**User's choice:** WARN + gauge metric, never fail

---

## UAT File Layout

**Question:** How should Phase 8 record the human UAT results?

| Option | Description | Selected |
|--------|-------------|----------|
| Extend existing files in place | Flip items in `03-HUMAN-UAT.md`, create `06-HUMAN-UAT.md`, re-run blocked entries in `07-UAT.md`, short `08-HUMAN-UAT.md` index | ✓ |
| One consolidated `08-HUMAN-UAT.md` | Single file, all sections, breaks per-phase provenance | |
| Per-surface files under Phase 8 | `08-UI-UAT.md`, `08-QUICKSTART-UAT.md`, `08-AUTO-REFRESH-UAT.md` | |

**User's choice:** Extend existing files in place

---

## v1.1 Backlog vs In-Phase Fix Rule

**Question:** What's the rule for 'fix during Phase 8 vs defer to v1.1 backlog' when the walkthrough uncovers something new?

| Option | Description | Selected |
|--------|-------------|----------|
| Functional breakage = fix now; visual polish = v1.1 | Clear, easy to apply during the walkthrough | ✓ |
| Any discovered issue → backlog unless it blocks a v1.0 success-criterion | Stricter but ships fastest | |
| Whatever feels right during the session | No preset rule, user decides in the moment | |

**User's choice:** Functional breakage = fix now; visual polish = v1.1

---

## Cold-Start Smoke Test Scope

**Question:** Where should the cold-start smoke test live and what should it assert?

| Option | Description | Selected |
|--------|-------------|----------|
| Extend the existing `compose-smoke` CI job | Single CI job, single boot, reuses Phase 6 gap-closure scaffolding | ✓ |
| New shell script under `tests/smoke/` invoked from CI | Self-contained but duplicates setup | |
| Rust integration test behind `--features integration` | Matches integration-test conventions but complex orchestration | |
| Local-only shell script, not in CI | Cheapest but roadmap wording implies CI automation | |

**User's choice:** Extend the existing `compose-smoke` CI job

---

## Expanded Example Jobs

**Question:** Given the alpine:3 rebase gives us sh/date/sleep/wget/awk/sed, which example jobs should `examples/cronduit.toml` ship?

| Option | Description | Selected |
|--------|-------------|----------|
| echo-timestamp (command) — 1-min heartbeat using `date` | Preserves canonical demo, proves command execution | ✓ |
| http-healthcheck (command) — 5-min wget against example.com | Realistic uptime canary, validates DNS + egress | ✓ |
| disk-usage (script) — 15-min /data volume size snapshot | Demonstrates script path and volume read access | ✓ |
| hello-world (docker) — keep existing 5-min docker.io/hello-world | Proves docker-sock fix and docker-executor code path | ✓ |

**User's choice:** All four — shipping a full mix of command × 2, script × 1, docker × 1.

---

## Ready Gate

**Question:** Any more gray areas to explore, or ready for CONTEXT.md?

| Option | Description | Selected |
|--------|-------------|----------|
| Ready for context | Write CONTEXT.md + DISCUSSION-LOG.md now and hand off to plan-phase | ✓ |
| Explore more gray areas | Surface 2-4 additional decisions | |

**User's choice:** Ready for context

---

## Claude's Discretion

Areas the user left to the planner's judgment (documented in CONTEXT.md `<decisions>` → "Claude's Discretion"):

- Plan ordering and wave assignment across the four work streams
- Alpine package version pinning (moving-tag vs pinned versions for `ca-certificates` / `tzdata`)
- Exact wording of the pre-flight WARN log line
- Docker-socket-proxy allowlist fine-tuning beyond the D-10 minimum (`CONTAINERS=1 IMAGES=1 POST=1`)
- `jq` vs shell-only JSON parsing in the extended smoke test step
- Whether to fold `06-VERIFICATION.md` / `07-VERIFICATION.md` re-verification annotations into Phase 8 or punt to audit-milestone

## Deferred Ideas

Captured in CONTEXT.md `<deferred>`:

- Multi-arch verification of `docker-compose.secure.yml` on arm64 runners
- Full reverse-proxy (Traefik / Caddy) production example
- `cronduit_docker_ping_duration_seconds` histogram
- Migration note / chown one-liner for existing users upgrading from the distroless image (may fold into release notes, not a standalone task)
- Final `/gsd-audit-milestone` re-run (milestone-level, not a Phase 8 task)
- `06-VERIFICATION.md` / `07-VERIFICATION.md` re_verification blocks (planner decides)
