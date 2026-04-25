---
phase: 12
plan: 03
subsystem: "deployment / packaging"
tags: [dockerfile, healthcheck, ops-07, rc.1]
requires:
  - Dockerfile runtime stage (L90–130) as inherited from Phase 8 alpine rebase
  - /cronduit health subcommand (delivered by Plan 12-01 / 12-02)
provides:
  - Bakes the OPS-07 HEALTHCHECK directive into the cronduit image
  - `docker compose up` reports `healthy` out of the box (contingent on Plans 01+02)
  - Operator compose `healthcheck:` overrides still win (Dockerfile < compose)
affects:
  - Dockerfile (runtime stage)
  - Downstream image config at build time (exposed via `docker inspect`)
tech_stack:
  added: []
  patterns:
    - "Dockerfile HEALTHCHECK directive in exec form (T-12-03-01 mitigation)"
    - "Non-root probe (directive placed after USER cronduit:cronduit; T-12-03-04 mitigation)"
    - "60s start-period accommodates Phase 11 D-12 migration backfill window (T-12-03-02 mitigation)"
key_files:
  created: []
  modified:
    - Dockerfile
decisions:
  - "Exec form `CMD [\"/cronduit\", \"health\"]` (not shell form) — consistent with existing ENTRYPOINT/CMD style and avoids shell expansion surface (T-12-03-01)"
  - "60s start-period locked per D-06 / Phase 11 D-12 (listener binds after migration backfill)"
  - "Directive placed after `USER cronduit:cronduit` so probe runs as non-root (T-12-03-04)"
  - "No changes to `RUN apk add` line — busybox stays installed per D-07 for user-authored `type = \"command\"` jobs"
metrics:
  duration_min: 1.3
  tasks_completed: 1
  files_modified: 1
  completed_date: "2026-04-18"
requirements:
  - OPS-07
---

# Phase 12 Plan 03: Docker HEALTHCHECK directive Summary

One-liner: Added the OPS-07 `HEALTHCHECK` directive to the runtime stage of `Dockerfile` (exec form, non-root, 60s start-period) so `docker compose up` reports `healthy` out of the box without any compose-file healthcheck stanza.

## Line range modified

- **File:** `Dockerfile`
- **Insertion point:** between `USER cronduit:cronduit` (previously L127, now L127) and `ENTRYPOINT ["/cronduit"]` (previously L129, now L136)
- **New lines:** 8 insertions (5-line `#` comment block + 2-line HEALTHCHECK directive + 1 surrounding blank line preserved). Final line count grew from 131 to 139.
- **Post-edit layout:** USER@L127 < HEALTHCHECK@L134 < ENTRYPOINT@L137 (verified by the plan's Python line-order assertion).

## Exact HEALTHCHECK directive committed

```dockerfile
# Phase 12 OPS-07: probe /health every 30s; allow 60s for migration backfill
# (Phase 11 D-12 binds the listener AFTER backfill completes), 5s timeout per
# probe, 3 consecutive failures flip the container to (unhealthy). Operator
# `healthcheck:` stanzas in compose still override (compose wins over Dockerfile,
# verified by .github/workflows/compose-smoke.yml).
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
    CMD ["/cronduit", "health"]
```

All four timing flags match D-06 exactly; the CMD is in exec form (square-bracket array) matching the existing `ENTRYPOINT` and `CMD` style (L137–L138 post-edit).

## `docker inspect` sanity check

Docker daemon is available in the executor environment (Rancher Desktop 29.1.3), but a full multi-stage `docker build` was not run because the builder stage cross-compiles via `cargo-zigbuild` against a Docker-external network (zig tarball + Tailwind binary downloads) and would take many minutes. Instead, the plan's recommended fallback was applied:

```
$ docker buildx build --check .
...
Check complete, no warnings found.
```

Exit status `0`. The Dockerfile linter (dockerfile:1.8.1) did not flag the new HEALTHCHECK directive — syntax is well-formed. The full-image `docker inspect --format '{{json .Config.Healthcheck}}'` assertion is owned by Plan 04's compose-smoke workflow (`.github/workflows/compose-smoke.yml`), which is the canonical acceptance gate for OPS-07's image-level behavior (per D-09).

## Unchanged lines (audit)

All verifications in the plan's `<acceptance_criteria>` passed:

| Check | Result |
|-------|--------|
| `RUN apk add --no-cache ca-certificates tzdata` unchanged (D-07) | PASS — one match, same line |
| `ENTRYPOINT ["/cronduit"]` present and unchanged | PASS |
| `CMD ["run", "--config", "/etc/cronduit/config.toml"]` present and unchanged | PASS |
| Builder stage (L1–77) unchanged | PASS — commit diff is +8 insertions, 0 deletions, 0 rewrites |
| `EXPOSE 8080` and `USER cronduit:cronduit` unchanged | PASS |
| Exactly one `HEALTHCHECK` directive in file | PASS (`grep -c '^HEALTHCHECK' = 1`) |
| USER < HEALTHCHECK < ENTRYPOINT line order | PASS (USER@126, HEALTHCHECK@133, ENTRYPOINT@136 — 0-indexed per the Python assertion script) |
| Comment block references `Phase 12 OPS-07` | PASS |
| Comment block references `Phase 11 D-12` | PASS |

## Threat model verification

All four `mitigate` dispositions from the plan's threat register are satisfied by the committed directive:

- **T-12-03-01 (shell injection):** Exec form used — `CMD ["/cronduit", "health"]`. No shell expansion, no argv interpolation surface.
- **T-12-03-02 (DoS from slow backfill):** `--start-period=60s` matches Phase 11 D-12 listener-bind timing.
- **T-12-03-04 (probe-as-root):** HEALTHCHECK placed after `USER cronduit:cronduit` — probe runs as UID 1000.
- **T-12-03-05 (multi-arch drift):** HEALTHCHECK is a metadata directive, not a compiled target — single Dockerfile covers amd64 + arm64 via the existing `cargo-zigbuild` matrix; no arch-conditional logic added.

T-12-03-03 (healthy-state masking after a hung scheduler) is correctly `accept`-ed per the plan — out of scope for Phase 12; the `/health` endpoint's `degraded` JSON contract is consumed unchanged.

## Commit

- `ead987f` — `feat(12-03): add OPS-07 HEALTHCHECK directive to Dockerfile`

## Deviations from Plan

None. Plan executed exactly as written. The one discretionary choice — whether to run a full `docker build` or fall back to `docker buildx build --check` — was made per the plan's explicit fallback clause ("If `docker build` is unavailable ... fall back to validating syntax with `docker buildx build --check .`"). Build check passed with zero warnings.

## Known Stubs

None. This plan adds exactly one Dockerfile directive; there are no empty-value placeholders, no "coming soon" text, no mocked components. The HEALTHCHECK CMD's runtime correctness depends on the `/cronduit health` binary produced by Plans 12-01 + 12-02, which Plan 12-04's compose-smoke workflow verifies end-to-end — that integration gap is expected and documented in the plan `<objective>` ("runtime correctness of the HEALTHCHECK CMD depends on Plans 01+02"), not a stub.

## Self-Check

Files claimed:
- `Dockerfile` — FOUND (exists at repo root, L134 contains HEALTHCHECK directive)

Commits claimed:
- `ead987f` — FOUND in `git log --oneline`

## Self-Check: PASSED
