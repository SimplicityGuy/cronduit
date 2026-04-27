# Phase 12: Docker Healthcheck + rc.1 Cut - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-17
**Phase:** 12-docker-healthcheck-rc-1-cut
**Areas discussed:** HTTP client choice, `health` arg/config surface, OPS-08 reproduction rigor, rc.1 release mechanics

---

## Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| HTTP client choice | How `cronduit health` issues the GET /health | ✓ |
| `health` arg/config surface | CLI flag and config file semantics | ✓ |
| OPS-08 reproduction rigor | How rigorously to reproduce the `(unhealthy)` root cause | ✓ |
| rc.1 release mechanics | Workflow patch + tag-cut process | ✓ |

**User's choice:** All four areas selected.

---

## HTTP Client Choice

### Which HTTP client should `cronduit health` use for the loopback GET /health?

| Option | Description | Selected |
|--------|-------------|----------|
| ureq (blocking) — Recommended | Small, blocking, rustls-native. Fits one-shot CLI perfectly. | |
| hyper 1 + hyper-util | Reuses already-declared `hyper = "1"`. Async, more setup than ureq. | ✓ |
| reqwest with rustls | Heaviest; overkill for one localhost GET. | |
| Raw TcpStream + hand-rolled HTTP | Zero deps; ironic given OPS-08 is a hand-rolled HTTP parsing bug. | |

**User's choice:** hyper 1 + hyper-util
**Notes:** Reuses already-declared dep; ok with the extra `hyper-util` feature cost to avoid adding another unrelated HTTP library.

### Should `cronduit health` apply its own client-side timeout, or rely on the outer HEALTHCHECK `--timeout=5s`?

| Option | Description | Selected |
|--------|-------------|----------|
| Internal timeout (2s connect, 3s read) — Recommended | Bounded by explicit `tokio::time::timeout`. Deterministic exit code outside Docker too. | ✓ |
| No internal timeout, rely on Docker | Simpler; hangs indefinitely when invoked by hand on the host. | |
| Aggressive timeout (1s total) | Can flap under heavy SSE load. | |

**User's choice:** Internal timeout (2s connect, 3s read)

---

## `health` Arg/Config Surface

### How should `cronduit health` accept its target address?

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse global `--bind host:port` — Recommended | Consistent with `run --bind`. Prepend `http://`. Default `127.0.0.1:8080`. | ✓ |
| Subcommand-local `--url` | New flag; more future-proof (HTTPS). Second address-input shape. | |
| Both (mutually exclusive) | Max flexibility; overkill for v1.1. | |

**User's choice:** Reuse global `--bind host:port`

### Should `cronduit health` honor `--config <path>` to pick up `[server].bind` from TOML?

| Option | Description | Selected |
|--------|-------------|----------|
| Ignore --config entirely — Recommended | Probe is fast; no TOML parse every 30s. Operators override via `--bind`. | ✓ |
| Honor --config when provided | Slightly more convenient; Dockerfile default unaffected. | |
| Auto-load /etc/cronduit/config.toml | Risky; makes probe sensitive to config I/O. | |

**User's choice:** Ignore --config entirely

---

## OPS-08 Reproduction Rigor

### How rigorously must the `(unhealthy)` root cause be reproduced before the fix is declared complete?

| Option | Description | Selected |
|--------|-------------|----------|
| Targeted automated repro test — Recommended | CI job with OLD wget → unhealthy, NEW cronduit health → healthy. Satisfies ROADMAP SC #4. | ✓ |
| Documented manual repro in a runbook | Cheaper; drift risk on busybox/alpine upgrades. | |
| Skip repro — rely on removing busybox wget | Saves CI; fails SC #4 as worded. | |

**User's choice:** Targeted automated repro test

### Where does the OPS-08 repro test + compose-smoke test (Success Criterion #3) live?

| Option | Description | Selected |
|--------|-------------|----------|
| Dedicated GHA workflow `.github/workflows/compose-smoke.yml` — Recommended | Standalone; docker daemon + compose CLI. Keeps Rust unit tests fast. | ✓ |
| Extend existing integration-tier Rust test | `testcontainers` can't cleanly drive `docker compose` overrides. | |
| Hybrid | Two artifacts; more cognitive surface. | |

**User's choice:** Dedicated GHA workflow

---

## rc.1 Release Mechanics

### The existing `release.yml` tags every pushed image as `:latest`, `:major`, and `:major.minor` unconditionally — which would bump `:latest` to `v1.1.0-rc.1`. Phase 12 must prevent that. How?

| Option | Description | Selected |
|--------|-------------|----------|
| Patch release.yml for pre-release semantics — Recommended | Add `enable=` conditions; skip `:latest` / `:major` / `:major.minor` on pre-releases; add rolling `:rc` tag. | ✓ |
| Add a separate prerelease workflow file | Two files to maintain; duplication drift risk. | |
| Manual ad-hoc fix — delete `:latest` after each rc | Brittle; footgun during the cleanup window. | |

**User's choice:** Patch release.yml for pre-release semantics

### Does Phase 12 also ship an rc-cut runbook document (e.g., `docs/release-rc.md`), or is the patched workflow sufficient?

| Option | Description | Selected |
|--------|-------------|----------|
| Ship a release-rc runbook — Recommended | Reusable for rc.2/rc.3. Covers pre-flight, tag format, post-push verification, escalation. | ✓ |
| Workflow patch only, no runbook | Works for solo maintainer; nothing to reference in rc.2/rc.3. | |
| Minimal runbook as part of CHANGELOG prep | Less discoverable than a standalone doc. | |

**User's choice:** Ship a release-rc runbook

---

## Claude's Discretion

Areas where the planner/executor has flexibility (captured in CONTEXT.md § "Claude's Discretion"):
- Exact module path for the new subcommand (`src/cli/health.rs` by parallelism).
- `hyper-util` feature set (`["client-legacy", "http1", "tokio"]` starting point; planner trims).
- Stderr log format for failure modes.
- Precise GHA `enable=` expression syntax.
- Whether compose-smoke uses `docker/setup-buildx-action` or the default daemon.
- `git-cliff` CHANGELOG heading style (defaults are fine).
- Placement of `docs/release-rc.md` (root, `docs/`, or inside `CONTRIBUTING.md`).

## Deferred Ideas

Ideas raised during discussion that were explicitly deferred (captured in CONTEXT.md § "Deferred Ideas"):
- TLS/HTTPS for `cronduit health` (forward-compatible via additive `--url` flag).
- `/healthz` "starting" state during backfill (Phase 11 D-12 already blocks the listener).
- `workflow_dispatch` shortcut for tag cuts (trust anchor stays with maintainer).
- Hand-edited GitHub Release bodies (conventional commits are authoritative).
- Retry inside `cronduit health` (Docker HEALTHCHECK retry policy sufficient).
- Config auto-discovery for the `health` subcommand.
- Per-failure-mode exit codes.
- Bumping `:latest` on rc tags.
- Extending `ci.yml` with compose smoke.
- `testcontainers`-based compose-override testing.
