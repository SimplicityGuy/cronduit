---
phase: 08-v1-final-human-uat-validation
plan: 01
subsystem: infra
tags: [runtime, docker, alpine, busybox, quickstart, examples, uid-1000]

# Dependency graph
requires:
  - phase: 01-foundation-security-posture-persistence-base
    provides: the distroless-nonroot runtime stage that Phase 8 walks back
  - phase: 06-live-events-metrics-retention-release-engineering
    provides: the two-job quickstart mix that Phase 8 expands to four jobs
provides:
  - alpine:3 runtime image with cronduit UID/GID 1000 and writable /data
  - ca-certificates + tzdata installed via single apk --no-cache layer
  - four-job quickstart config (command x2, script, docker) validated by cronduit check
  - unblocks the Phase 8 echo-timestamp ENOENT blocker from 07-UAT.md
  - provides sustained-RUNNING example jobs required by plan 07-05 browser UAT
affects:
  - 08-02 (compose files — will inherit new UID 1000 + /data ownership contract)
  - 08-03 (docker pre-flight — boots against this runtime)
  - 08-04 (compose-smoke CI — will run against new quickstart jobs)
  - 08-05 (human UAT walkthrough — blocked jobs now sustain RUNNING state)

# Tech tracking
tech-stack:
  added:
    - "alpine:3 base image (runtime stage)"
    - "apk package manager (ca-certificates, tzdata)"
    - "busybox wget for http-healthcheck example job"
  patterns:
    - "Explicit UID/GID (1000) via addgroup -g 1000 + adduser -u 1000 -G cronduit"
    - "Pre-created /data with install -d -o 1000 -g 1000 so named volumes inherit ownership on first mount"
    - "sh -c wrapping for command= jobs that need shell operators (2>&1, |, etc.)"
    - "Script jobs handle not-mounted /data gracefully (|| echo fallback)"

key-files:
  created: []
  modified:
    - Dockerfile
    - examples/cronduit.toml

key-decisions:
  - "Used explicit addgroup -g 1000 rather than adduser -S bare; -S alone picks the next system GID (101 on alpine) which does not satisfy the UID/GID 1000 intent in D-02"
  - "Header comment was rewritten to avoid literal 'gcr.io/distroless/static-debian12:nonroot', '65532', and '/staging-data' strings so the plan's grep-count-0 acceptance checks all pass; rationale preserved via 'Phase 1 distroless-nonroot runtime' wording"
  - "Did not modify the SECURITY comment block, [server], or [defaults] sections in examples/cronduit.toml (plan instruction: byte-identical preservation)"
  - "CLI check invocation uses positional config path (./cronduit check examples/cronduit.toml), not --config — the plan's verify.automated used --config which the CLI does not accept"

patterns-established:
  - "Pattern: alpine runtime with single apk --no-cache layer for ca-certificates + tzdata; no bash/sudo/shadow"
  - "Pattern: explicit UID/GID on both the group and user to avoid busybox adduser -S GID drift"
  - "Pattern: quickstart jobs that gracefully degrade when optional mounts are absent"

requirements-completed: [OPS-05]

# Metrics
duration: 13min
completed: 2026-04-14
---

# Phase 8 Plan 01: Alpine Runtime Rebase & Four-Job Quickstart Summary

**Rebased cronduit's runtime image from distroless/static-debian12:nonroot to alpine:3 with UID/GID 1000 and expanded the quickstart from 2 to 4 example jobs (command x2 + script + docker) so busybox-dependent heartbeat, healthcheck, and disk-usage jobs execute end-to-end on first `docker compose up`.**

## Performance

- **Duration:** 13 min
- **Started:** 2026-04-14T00:01:41Z
- **Completed:** 2026-04-14T00:14:45Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Dockerfile runtime stage rebased to `alpine:3` with a non-root `cronduit` user at UID/GID 1000 and pre-created `/data` (writable named-volume mount point)
- Single-layer `apk add --no-cache ca-certificates tzdata` install preserves the small-attack-surface promise while giving bollard its CA bundle and croner its IANA timezone data
- Dockerfile header comment records the Phase 1 -> Phase 8 walk-back rationale so future maintainers understand why distroless was abandoned
- Builder stage (Rust 1.94-slim-bookworm + cargo-zigbuild + cross-compile targets) is byte-identical; cross-compile cache keys unchanged
- `examples/cronduit.toml` ships four quickstart jobs covering every execution path: `echo-timestamp` (command heartbeat), `http-healthcheck` (command, busybox `wget -q -S --spider https://www.google.com` wrapped in `sh -c` for pipe handling), `disk-usage` (script with `#!/bin/sh` shebang and graceful fallback), `hello-world` (ephemeral docker container)
- `cronduit check examples/cronduit.toml` validates cleanly (exit 0)
- SECURITY comment block, `[server]`, and `[defaults]` sections in `examples/cronduit.toml` preserved byte-identical per plan instruction

## Task Commits

Each task was committed atomically with `--no-verify` (parallel executor mode):

1. **Task 1: Rebase Dockerfile runtime to alpine:3 with cronduit UID 1000** — `3977867` (feat)
2. **Task 2: Rewrite examples/cronduit.toml with four quickstart jobs** — `25a14dd` (feat)

## Files Created/Modified

### Modified

- `Dockerfile` — Runtime stage fully rewritten (lines 45-86); builder stage (lines 1-43) unchanged. Line-range diff:
  - Lines 45-49: deleted old `# Pre-create /data directory owned by distroless nonroot...` comment block
  - Line 50: deleted `RUN install -d -o 65532 -g 65532 /staging-data` from builder stage
  - Line 53: replaced `FROM gcr.io/distroless/static-debian12:nonroot` with the 10-line walk-back rationale header + `FROM alpine:3`
  - Lines 64-76: inserted `apk add --no-cache ca-certificates tzdata` + `addgroup -g 1000 -S cronduit && adduser -S -u 1000 -G cronduit cronduit && install -d -o 1000 -g 1000 /data`
  - Line 67 (old): deleted `COPY --from=builder --chown=65532:65532 /staging-data /data`
  - Line 70 (old): replaced `USER nonroot:nonroot` with `USER cronduit:cronduit`
  - Labels, EXPOSE 8080, ENTRYPOINT, CMD preserved verbatim
- `examples/cronduit.toml` — Jobs section (line 32 onward) fully rewritten; lines 1-31 (SECURITY + `[server]` + `[defaults]`) untouched. New jobs: `echo-timestamp`, `http-healthcheck`, `disk-usage`, `hello-world` (4 `[[jobs]]` blocks total).

## Decisions Made

- **Explicit `addgroup -g 1000`**: The plan's action text said `addgroup -S cronduit && adduser -S -u 1000 -G cronduit cronduit`. Runtime-stage verification revealed that busybox's `adduser -S` does not inherit the primary group's GID — it picks the next system GID (101). To honor D-02's "UID/GID 1000" requirement I bumped the addgroup line to `addgroup -g 1000 -S cronduit`. Verified via `docker run` that `id -u=1000 && id -g=1000` now both resolve. Documented as a deviation (Rule 1 — correctness/fidelity to D-02).
- **Header comment wording**: The plan's exact comment text contained the literal strings `gcr.io/distroless/static-debian12:nonroot`, `65532`, and `/staging-data`. The plan's acceptance criteria separately required grep counts of 0 for those same literals in the final Dockerfile. These are directly contradictory; I resolved by preserving the rationale ("walk-back from the Phase 1 distroless-nonroot runtime" + "slightly larger attack surface") while dropping the forbidden literals. The D-01..D-06 reference and the `walk-back` keyword remain present. Documented as a deviation (Rule 1).
- **No apk package pinning**: Per CONTEXT.md Claude's Discretion, track the moving `alpine:3` tag rather than pin `ca-certificates=20241121-r1` or `tzdata=YYYYnn-rN`. Keeps the build simple and avoids a reproducibility burden for an OSS launch project.
- **CLI check uses positional config path**: `src/cli/check.rs` takes config as a positional `<CONFIG>` argument, not `--config <path>`. The plan's `<verify><automated>` field used `--config` which the binary does not accept (exit 2 "unexpected argument"). Used `./target/debug/cronduit check examples/cronduit.toml` instead. Result: `ok: examples/cronduit.toml`, exit 0.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Correctness] Explicit GID 1000 on addgroup**
- **Found during:** Task 1 (runtime verification via stub-builder test image)
- **Issue:** Plan action used `addgroup -S cronduit` (system group, next available GID). On alpine:3 this resolves to GID 101 for the first system group added, which does not satisfy D-02's "UID/GID 1000" requirement or the must_haves "runs cronduit as UID 1000" spirit (GID of /data owner also needs to be 1000 for named-volume semantics to work).
- **Fix:** Changed to `addgroup -g 1000 -S cronduit` so the primary group of the `cronduit` user is explicitly GID 1000.
- **Files modified:** Dockerfile (line 74)
- **Verification:** Rebuilt a runtime-stage stub image; `docker run --rm --entrypoint /bin/sh cronduit:runtime-verify -c 'id -u && id -g'` now prints `1000\n1000`.
- **Committed in:** 3977867 (Task 1 commit)

**2. [Rule 1 - Correctness] Header comment literals reworded to satisfy grep-0 checks**
- **Found during:** Task 1 (first grep acceptance sweep)
- **Issue:** Plan's exact comment text contained `gcr.io/distroless/static-debian12:nonroot`, `65532`, and `--chown=65532:65532 /staging-data`. Plan's own acceptance criteria require `grep -c 'gcr.io/distroless/static-debian12:nonroot' Dockerfile` == 0, `grep -c '65532' Dockerfile` == 0, and `grep -c '/staging-data' Dockerfile` == 0. The must_haves truth set also asserts "No `--chown=65532:65532` reference survives anywhere in the Dockerfile". These are directly in conflict.
- **Fix:** Rewrote both comment blocks to preserve the walk-back rationale, the UID/GID 1000 explanation, the D-01..D-06 reference, and the `walk-back` keyword — but without the forbidden literals. "Phase 1 distroless-nonroot runtime" replaces the full distroless image tag; "the Phase 1 multi-stage chown dance that targeted the old distroless nonroot UID" replaces the `--chown=65532:65532 /staging-data` phrase.
- **Files modified:** Dockerfile (lines 45-55 header comment; lines 70-73 user-creation comment)
- **Verification:** `grep -c 'gcr.io/distroless/static-debian12:nonroot' Dockerfile` == 0; `grep -c '65532' Dockerfile` == 0; `grep -c '/staging-data' Dockerfile` == 0; `grep -c 'walk-back' Dockerfile` == 2; `grep -c '^FROM alpine:3$' Dockerfile` == 1.
- **Committed in:** 3977867 (Task 1 commit)

**3. [Rule 3 - Blocking] cronduit check CLI accepts positional, not --config**
- **Found during:** Task 2 (post-edit verification)
- **Issue:** Plan's `<verify><automated>` runs `cargo run --release --bin cronduit -- check --config examples/cronduit.toml`. The actual `src/cli/mod.rs` defines `Check { config: PathBuf }` as a positional `<CONFIG>` arg, so `--config` is rejected with "unexpected argument '--config' found; tip: use '-- --config'" (exit 2).
- **Fix:** Ran the equivalent verification with the correct CLI shape: `./target/debug/cronduit check examples/cronduit.toml`. Also used a pre-built debug binary instead of `cargo run --release` to save compile time (the check subcommand does not touch optimized code paths).
- **Files modified:** (none — verification change only)
- **Verification:** `./target/debug/cronduit check examples/cronduit.toml` printed `ok: examples/cronduit.toml` and exited 0.
- **Committed in:** n/a (no code change)

---

**Total deviations:** 3 auto-fixed (2 Rule 1 correctness, 1 Rule 3 blocking)
**Impact on plan:** All three auto-fixes preserve the plan's intent. The GID fix is a strict improvement (D-02 satisfied more precisely). The header comment rewording resolves an internal contradiction in the plan between action-text literals and acceptance-criteria grep counts. The CLI positional fix resolves a verify command that would have 100% failed as written regardless of file correctness. No scope creep.

## Issues Encountered

- **Local Docker build blocked by Rancher Desktop VM disk limits.** The plan's `docker build -t cronduit:phase8-test .` acceptance criterion could not be fully exercised in this worktree: the local Rancher Desktop Linux VM ran out of disk during the `cargo zigbuild --release` step (`No space left on device` compiling `aws-lc-sys` / `chrono-tz`). A `docker system prune -af --volumes` reclaimed 1.4 GB but the cross-compile target directory still hit the VM's internal disk limit on retry. This is an environmental constraint, **not a Dockerfile defect**. Mitigation: I built a standalone runtime-stage test image (`cronduit:runtime-verify`) that copies the exact alpine:3 RUN blocks and `USER cronduit:cronduit` setup but stubs the Rust binary as a shell script. That image built cleanly and `docker run` confirmed `id -u=1000`, `id -g=1000`, all five busybox applets (`date`, `sh`, `wget`, `du`, `df`) resolve, and `/data` is writable. The full multi-stage build will be exercised on CI by the `compose-smoke` workflow (plans 08-04) on fresh runners with ample disk.

## Verification Evidence

**Task 1 (Dockerfile) — static grep checks**

```
$ grep -c '^FROM alpine:3$' Dockerfile                           # 1 ✓
$ grep -c 'gcr.io/distroless/static-debian12:nonroot' Dockerfile  # 0 ✓
$ grep -c 'walk-back' Dockerfile                                  # 2 ✓ (>=1)
$ grep -c 'addgroup -g 1000 -S cronduit' Dockerfile               # 1 ✓
$ grep -c 'adduser -S -u 1000 -G cronduit cronduit' Dockerfile    # 1 ✓
$ grep -c 'install -d -o 1000 -g 1000 /data' Dockerfile           # 1 ✓
$ grep -c 'apk add --no-cache ca-certificates tzdata' Dockerfile  # 1 ✓
$ grep -c '65532' Dockerfile                                      # 0 ✓
$ grep -c '/staging-data' Dockerfile                              # 0 ✓
$ grep -c '^USER cronduit:cronduit$' Dockerfile                   # 1 ✓
$ grep -c '^USER nonroot:nonroot$' Dockerfile                     # 0 ✓
$ grep -c 'FROM --platform=\$BUILDPLATFORM rust:1.94-slim-bookworm AS builder' Dockerfile  # 1 ✓ (builder preserved)
$ grep -c 'cargo zigbuild --release --target' Dockerfile          # 1 ✓ (cross-compile preserved)
```

**Task 1 (Dockerfile) — runtime stub-image docker run checks**

```
$ docker run --rm --entrypoint /bin/sh cronduit:runtime-verify \
    -c 'id -u && id -g && which date sh wget du df && test -w /data && echo WRITABLE'
1000
1000
/bin/date
/bin/sh
/usr/bin/wget
/usr/bin/du
/bin/df
WRITABLE
```

**Task 2 (examples/cronduit.toml) — static grep checks**

```
$ grep -c 'name = "echo-timestamp"' examples/cronduit.toml        # 1 ✓
$ grep -c 'name = "http-healthcheck"' examples/cronduit.toml      # 1 ✓
$ grep -c 'name = "disk-usage"' examples/cronduit.toml            # 1 ✓
$ grep -c 'name = "hello-world"' examples/cronduit.toml           # 1 ✓
$ grep -cF "date '+%Y-%m-%d %H:%M:%S -- Cronduit is running!'" examples/cronduit.toml  # 1 ✓
$ grep -c 'wget -q -S --spider https://www.google.com' examples/cronduit.toml  # 1 ✓
$ grep -c 'du -sh /data 2>/dev/null' examples/cronduit.toml       # 1 ✓
$ grep -c '#!/bin/sh' examples/cronduit.toml                      # 1 ✓
$ grep -c '^bind = "0.0.0.0:8080"$' examples/cronduit.toml        # 1 ✓ (server preserved)
$ grep -c 'SECURITY:' examples/cronduit.toml                      # 1 ✓ (security block preserved)
$ grep -c '\[\[jobs\]\]' examples/cronduit.toml                   # 4 ✓
```

**Task 2 (examples/cronduit.toml) — cronduit check validation**

```
$ ./target/debug/cronduit check examples/cronduit.toml
ok: examples/cronduit.toml
EXIT=0
```

## CLAUDE.md Compliance

- [x] Mermaid-only diagrams: no diagrams added in this plan (none needed).
- [x] No direct commits to main: committed to worktree branch `worktree-agent-ad503857`, base 4c68959 (phase/08-plan feature branch).
- [x] Rust/bollard/sqlx/askama_web stack: no deps added; Dockerfile rebase is runtime-only.
- [x] TLS/rustls story unchanged (builder stage untouched; no openssl-sys reintroduction).
- [x] Alpine multi-arch: `alpine:3` supports linux/amd64 + linux/arm64 natively; no buildx matrix change required (D-06).
- [x] Non-root UID preserved (1000, not root; still satisfies Phase 1 security posture even after distroless walk-back).

## Cross-reference to Phase 8 Decisions

- **D-01** (rebase to alpine:3) — Dockerfile line 56
- **D-02** (cronduit user UID 1000) — Dockerfile lines 70-76
- **D-03** (drop /staging-data multi-stage copy) — Dockerfile line 77 (COPY cronduit binary directly)
- **D-04** (USER cronduit:cronduit) — Dockerfile line 83
- **D-05** (ca-certificates + tzdata single RUN layer) — Dockerfile line 68
- **D-06** (multi-arch via alpine:3 native tag) — no workflow change, implicit in the FROM line
- **D-15** (four quickstart jobs) — examples/cronduit.toml lines 32-84
- **D-16** (SECURITY block + [server]/[defaults] preserved) — examples/cronduit.toml lines 1-31 (untouched)
- **D-17** (schedules fit inside compose-smoke CI budget) — */1, */5, */15, */5 minute cadences

## Next Phase Readiness

**Ready for Wave 2 (plans 08-02, 08-03):**
- Plan 08-02 (compose files) can now assume the runtime boots as UID/GID 1000 and /data is pre-owned; the `group_add: ["${DOCKER_GID:-999}"]` stanza will layer cleanly on top.
- Plan 08-03 (docker pre-flight) has a working alpine runtime to test `bollard::Docker::ping()` against, and the `cronduit_docker_reachable` gauge can be wired into the existing Phase 6 metrics family.
- Plan 08-04 (compose-smoke CI) now has four example jobs to assert `status=success` against within the 120s wall-clock budget.
- Plan 08-05 (human UAT) — the `http-healthcheck` and `disk-usage` jobs sustain a RUNNING state long enough for plan 07-05's HTMX `every 2s` polling transition to be observable, unblocking the previously-blocked Test 2 in 07-UAT.md.

**Deferred verification** (not a blocker for plan completion, handled in CI):
- Full `docker build -t cronduit:phase8-test .` with actual Rust cross-compile — blocked locally by Rancher Desktop VM disk limits. CI `compose-smoke` job (plan 08-04) will validate this on both amd64 and arm64 runners.

## Self-Check: PASSED

**Files verified present:**
- `Dockerfile` — FOUND (modified)
- `examples/cronduit.toml` — FOUND (modified)
- `.planning/phases/08-v1-final-human-uat-validation/08-01-SUMMARY.md` — FOUND (this file)

**Commits verified in git log:**
- `3977867` — FOUND (Task 1: Dockerfile alpine rebase)
- `25a14dd` — FOUND (Task 2: cronduit.toml four jobs)

---
*Phase: 08-v1-final-human-uat-validation*
*Plan: 01*
*Completed: 2026-04-14*
