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

## Current Test

[testing complete]

## Tests

### 1. Cold Start Smoke Test
expected: |
  Kill any running cronduit / docker compose stack for the project.
  Remove the SQLite DB file and any retained logs (fresh state).
  Boot the example stack from scratch:
    docker compose -f examples/docker-compose.yml up -d
  Within ~5 seconds, the container is healthy, migrations run, and
  `curl http://127.0.0.1:8080/healthz` returns a 2xx response with
  live data (not a 502/connection refused). No errors in
  `docker compose logs cronduit` at boot.
result: pass

### 2. Job Detail Run History Auto-Refresh — Running → Terminal
expected: |
  Navigate to a Job Detail page for an existing job (e.g. `/jobs/<id>`).
  Click "Run Now" 10+ times in rapid succession so multiple runs queue.
  New rows appear immediately in the Run History card as RUNNING
  (HX-Refresh from Plan 06 already worked).
  Within ~2 seconds of each underlying run completing, the RUNNING
  badge should transition to SUCCESS (or FAILED) **without manually
  reloading the page**. After all runs finish, every row should show a
  terminal status.
result: issue
reported: |
  Tried Run Now on both example jobs; both fail in ~1ms and never enter
  a sustained RUNNING state, so Plan 07-05's polling behavior cannot be
  observed:
    - hello-world: "image pull failed: transient pull error: Error in
      the hyper legacy client: client error (Connect)"
    - echo-timestamp: "failed to spawn command: No such file or
      directory (os error 2)"
severity: blocker

### 3. Job Detail Polling Stops When Idle
expected: |
  Immediately after Test 2 completes (all rows in terminal state),
  open the browser devtools Network tab and filter by "runs".
  For ~10 seconds, watch for further requests to
  `/partials/jobs/<job_id>/runs`. There should be **zero** follow-up
  polling requests — the conditional `hx-trigger="every 2s"` only
  renders when `any_running == true`, so once the last RUNNING row
  flips to terminal the wrapper re-renders without the trigger and
  HTMX stops polling on its own.
result: blocked
blocked_by: prior-test
reason: "Transitively blocked by Test 2 — cannot observe polling-stop behavior without a successful RUNNING → terminal transition window."

### 4. docker-compose.yml SECURITY Block Readable
expected: |
  Open `examples/docker-compose.yml` in an editor. The file should
  include a prominent SECURITY comment block (~30+ lines) referencing
  `THREAT_MODEL.md`, explaining why the default binds to 127.0.0.1,
  and showing an `expose:`-based override snippet for users who
  want reverse-proxy-only access. No Unicode box-drawing characters.
result: pass
note: |
  User requested tonal revision: "stranger" → "operator" on line 6.
  Applied in same session.

## Summary

total: 4
passed: 2
issues: 1
pending: 0
skipped: 0
blocked: 1

## Gaps

- truth: "examples/cronduit.toml echo-timestamp command job should run successfully when the example stack is booted fresh"
  status: failed
  reason: |
    The cronduit runtime image is gcr.io/distroless/static-debian12:nonroot
    (Dockerfile:55). Distroless has no coreutils — no /bin/date, no shell.
    src/scheduler/command.rs:199 calls Command::new("date") which returns
    ENOENT because the binary does not exist in the container filesystem.
    The bundled `echo-timestamp` example in examples/cronduit.toml is
    therefore broken out of the box for anyone following the quickstart.
  severity: blocker
  test: 2
  artifacts:
    - examples/cronduit.toml:39-42    # [[jobs]] name="echo-timestamp" command="date '+...'"
    - src/scheduler/command.rs:179-212
    - Dockerfile:55                    # distroless runtime
  missing:
    - Replace the echo-timestamp example with a command that exists in
      distroless (options: switch to a `script = "..."` shell-script job
      invoking /busybox/sh, or switch to an `image = "alpine:latest"`
      docker job, or change the runtime base to a minimal-shell image).

- truth: "examples/cronduit.toml hello-world docker job should pull and run successfully when the example stack is booted fresh"
  status: failed
  reason: |
    bollard inside the cronduit container returned "image pull failed:
    transient pull error: Error in the hyper legacy client: client error
    (Connect)". The compose file mounts /var/run/docker.sock:/var/run/docker.sock
    (examples/docker-compose.yml:47) so the mount itself is present, but
    the connect still fails. Likely root causes (need to verify on host):
      1. Host is Docker Desktop (macOS) — the socket path inside the
         Linux VM is exposed via /var/run/docker.sock on the host, but
         permissions inside the container may not match the nonroot UID.
      2. SELinux / AppArmor blocking socket access.
      3. The cronduit nonroot user (UID 65532) does not have read/write
         access to the docker.sock group inside the container.
  severity: blocker
  test: 2
  artifacts:
    - examples/cronduit.toml:44-50    # [[jobs]] name="hello-world" image="hello-world:latest"
    - examples/docker-compose.yml:46-48
    - Dockerfile:55                   # nonroot UID 65532
    - src/scheduler/docker_pull.rs:157
  missing:
    - Verify docker.sock permissions inside the cronduit container match
      the nonroot UID (65532).
    - Add a startup check that logs a clear error when bollard cannot
      connect to the Docker daemon, instead of surfacing the failure only
      on first job run.
    - Consider adding a "docker unreachable" pre-flight at startup in
      addition to the retention pruner startup log added in Phase 6.
