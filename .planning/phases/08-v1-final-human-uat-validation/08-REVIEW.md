---
phase: 08-v1-final-human-uat-validation
reviewed: 2026-04-14T02:54:29Z
depth: standard
files_reviewed: 11
files_reviewed_list:
  - Dockerfile
  - examples/cronduit.toml
  - examples/docker-compose.yml
  - examples/docker-compose.secure.yml
  - README.md
  - .github/workflows/ci.yml
  - src/cli/run.rs
  - src/scheduler/docker_daemon.rs
  - src/scheduler/mod.rs
  - src/telemetry.rs
  - tests/docker_daemon_preflight.rs
findings:
  critical: 0
  warning: 5
  info: 6
  total: 11
status: issues_found
---

# Phase 8: Code Review Report

**Reviewed:** 2026-04-14T02:54:29Z
**Depth:** standard
**Files Reviewed:** 11
**Status:** issues_found

## Summary

Phase 8 ships three coherent, well-scoped changes: the alpine:3 runtime rebase, a four-job quickstart, and the Docker daemon preflight ping with `cronduit_docker_reachable` gauge. The Rust code is clean — no unchecked unwraps in new code paths, errors are handled or deliberately swallowed with explanatory comments, and the `OnceLock` memoization in `telemetry.rs` correctly solves the multi-test race on the global recorder.

The five warnings are all in the CI workflow and compose files. None are showstoppers for UAT, but two carry real risk under specific conditions: the `set -eu` missing from one loop and the unquoted variable in the `sed` rewrite step. The six info items are style/consistency observations.

No critical issues were found.

---

## Warnings

### WR-01: `set -eu` missing from /health polling loop in ci.yml

**File:** `.github/workflows/ci.yml:201`
**Issue:** The "Wait for /health (max 30s)" step uses `set -eu` but the `for i in $(seq 1 30)` loop runs `curl` without `-f` in the error path and the `sleep 1` call is unchecked. More importantly, if the `curl` command itself fails with a non-zero exit (e.g. DNS resolution failure before the container is up) the `||` short-circuits correctly, but the `echo "ERROR: …"` path and subsequent `docker compose … logs` call are both outside `set -eu` scope because the script explicitly exits via `exit 0` / `exit 1`. This is actually fine in structure — but the companion step at line 217 ("Assert /health body contains status:ok") runs `body=$(curl -sSf …)` where a curl failure causes `body` to be empty and `grep -q` then fails, not `curl`. The failure message at that point does not indicate the real cause (curl non-zero). Low risk but the diagnostic message is misleading on transient network hiccups.

**Fix:** Add a curl-failure guard in the assertion step:
```bash
set -eu
body=$(curl -sSf http://localhost:8080/health) || {
  echo "ERROR: /health curl failed with exit $?"
  exit 1
}
echo "health body: $body"
echo "$body" | grep -q '"status":"ok"' || {
  echo "ERROR: /health body missing status:ok"
  exit 1
}
```

---

### WR-02: Unquoted `${{ matrix.compose }}` in sed rewrite step

**File:** `.github/workflows/ci.yml:173`
**Issue:** The `sed -i` line reads the compose filename from `env.COMPOSE_FILE` (which is correctly set from `${{ matrix.compose }}` through the `env:` block), so word-splitting on the variable itself is not the risk. However, the `grep -q 'image: cronduit:ci'` verification uses a single-quoted literal that would pass even if `sed` had partially mangled the file (e.g. only one of two service blocks was rewritten). In `docker-compose.secure.yml` there are two distinct `image:` lines — one for `dockerproxy` (tecnativa) and one for `cronduit`. Only the `cronduit` service line should be rewritten, and a single `grep -q` cannot distinguish "the right line was rewritten" from "any line now reads cronduit:ci".

**Fix:** Assert the cronduit service block specifically, or count exactly one match:
```bash
count=$(grep -c 'image: cronduit:ci' "examples/${COMPOSE_FILE}" || true)
if [ "$count" -ne 1 ]; then
  echo "ERROR: expected exactly 1 'image: cronduit:ci' line, found ${count}"
  cat "examples/${COMPOSE_FILE}"
  exit 1
fi
```

---

### WR-03: Shared poll deadline allows first slow job to starve later jobs

**File:** `.github/workflows/ci.yml:267`
**Issue:** The outer loop computes `deadline=$(( $(date +%s) + BUDGET_SECS ))` once before the inner per-job polling loops. The inner loops then share this single deadline. If `echo-timestamp` (first job) takes 110s to reach `success`, only 10s remain for `http-healthcheck`, `disk-usage`, and `hello-world`. In practice the echo job should finish in under 5s, but `hello-world` (Docker pull + run) on a cold CI runner can take 30-60s. If the total budget is 120s and jobs complete sequentially rather than all-at-once, this is a latent flakiness source.

**Fix:** Compute a per-job deadline by advancing the deadline after each job completes, or poll all jobs concurrently. The minimal fix is a larger `BUDGET_SECS` (e.g. 180s) with a comment:
```bash
BUDGET_SECS=180  # 120s was tight for cold docker pulls; 180s provides headroom
```
Or move the `deadline` computation inside the per-job loop:
```bash
for name in $JOBS; do
  deadline=$(( $(date +%s) + BUDGET_SECS ))  # per-job budget
  ...
done
```

---

### WR-04: `cronduit.toml` sets `bind = "0.0.0.0:8080"` — contradicts README and SPEC defaults

**File:** `examples/cronduit.toml:17`
**Issue:** The `[server]` block explicitly sets `bind = "0.0.0.0:8080"`. The README (line 26) and the Rust startup code in `src/cli/run.rs` (line 91) both state the default is `127.0.0.1:8080` and that a non-loopback bind triggers a loud `WARN`. New operators who copy this config file as a template for production will inherit a world-facing bind with no auth warning in their logs (the WARN fires at runtime, but the file comment does not flag this). The compose file's security comment explains the trade-off, but the TOML file itself has a comment block that says `Default bind is loopback. Change ONLY if you put Cronduit behind a reverse proxy with auth` — which directly contradicts the value immediately below it.

**Fix:** Either change the value to `127.0.0.1:8080` and rely on the compose file's `ports:` binding to expose 8080, or add an inline comment on the `bind` line:
```toml
bind = "0.0.0.0:8080"  # REQUIRED for docker-compose port publish to reach the host.
                         # See the compose file SECURITY header. Never use this value
                         # outside a reverse-proxy deployment.
```
The comment block at line 12 already says "Change ONLY if…" but the actual line silently contradicts it. The inline comment makes the intent unambiguous for copy-paste.

---

### WR-05: `docker-compose.secure.yml` missing `group_add` — cronduit user cannot write to named volume

**File:** `examples/docker-compose.secure.yml:85`
**Issue:** The `cronduit` service in `docker-compose.secure.yml` does not include a `group_add` stanza. The default `docker-compose.yml` uses `group_add: ["${DOCKER_GID:-999}"]` to grant docker socket access, which is correct there. In the secure variant the socket is handled by `dockerproxy`, so no docker GID is needed — that part is intentional. However, the `cronduit-data:/data` named volume is created by the Dockerfile with `install -d -o 1000 -g 1000 /data` (UID/GID 1000). The cronduit user inside the container IS UID 1000 (set by `USER cronduit:cronduit` in the Dockerfile), so writes to `/data` should succeed. This is a consistency observation rather than a hard bug — but if the named volume was previously created by a different image (e.g. a root-owned volume from an older deployment), UID 1000 would be blocked. Worth a comment in the secure compose file noting why `group_add` is absent.

**Fix:** Add a comment to the `cronduit` service block in the secure compose file:
```yaml
cronduit:
  image: ghcr.io/simplicityguy/cronduit:latest
  # No group_add needed: docker API traffic flows through dockerproxy (no
  # direct socket mount). The cronduit user is UID 1000, matching the /data
  # volume ownership set in the Dockerfile.
```
This prevents a future maintainer from adding a `group_add` for a non-existent docker socket.

---

## Info

### IN-01: Dockerfile copies `examples/cronduit.toml` as the default config

**File:** `Dockerfile:80`
**Issue:** `COPY --from=builder /build/examples/cronduit.toml /etc/cronduit/config.toml` bakes the `0.0.0.0:8080` bind and four quickstart jobs into the image. An operator who runs `docker run ghcr.io/simplicityguy/cronduit:latest` without mounting a custom config will silently get a non-loopback bind (triggering the WARN) and quickstart jobs they didn't ask for. The `CMD` uses `--config /etc/cronduit/config.toml` which points to this baked-in file.

**Suggestion:** Consider baking in a minimal "safe default" config (loopback bind, no jobs) or documenting prominently in README that the default image ships with `0.0.0.0` and the quickstart jobs. The compose files do override this with a volume mount (`:ro`), so compose users are unaffected.

---

### IN-02: `telemetry.rs` records a zero-valued histogram observation at boot

**File:** `src/telemetry.rs:124`
**Issue:** `metrics::histogram!("cronduit_run_duration_seconds").record(0.0)` forces registration but also permanently contributes a `0.0` second observation to the histogram's `_sum` and `_count`. This means `/metrics` will always show `cronduit_run_duration_seconds_sum 0` and `cronduit_run_duration_seconds_count 1` even before any job has run, which can confuse alerting rules that compute average duration via `rate(sum)/rate(count)`.

**Suggestion:** Check whether `metrics-exporter-prometheus` 0.18 offers a registration path that does not require a sample (e.g. `describe_histogram!` alone without `.record()`). If not, add a code comment explaining why the zero observation is present so operators are not confused by the phantom count-1 reading.

---

### IN-03: CI workflow `compose-smoke` job has no `needs:` dependency on `image`

**File:** `.github/workflows/ci.yml:126`
**Issue:** The `compose-smoke` job builds its own local image via `docker/build-push-action` independently of the `image` job. The `compose-smoke` job does not declare `needs: [lint, test]`, meaning it can run in parallel with (or before) `lint` and `test`. If a lint gate fails, `compose-smoke` may still run and succeed, giving a misleading green signal on the compose axis. The `image` job correctly declares `needs: [lint, test]`.

**Suggestion:** Add `needs: [lint, test]` to `compose-smoke` for consistency with the `image` job's gating:
```yaml
compose-smoke:
  name: quickstart compose smoke (${{ matrix.compose }})
  runs-on: ubuntu-latest
  needs: [lint, test]
```

---

### IN-04: `docker-compose.secure.yml` uses `tecnativa/docker-socket-proxy:latest` — unpinned tag

**File:** `examples/docker-compose.secure.yml:62`
**Issue:** The socket-proxy image uses `:latest` which is not a reproducible tag. If tecnativa ships a breaking change or a supply-chain-compromised image, operators who `docker compose pull` will silently get the new version. The `dockerproxy` container runs as root with access to the host Docker socket, making this a high-value supply-chain target.

**Suggestion:** Pin to a specific digest or tag (e.g. `tecnativa/docker-socket-proxy:0.7.0`) and document the pinning rationale. Acknowledge the tradeoff in a comment (pinned = reproducible but misses security patches unless manually updated).

---

### IN-05: `README.md` `cronduit check` example uses `cronduit check` but the command shown in the `just` recipe list is `check-config`

**File:** `README.md:307`
**Issue:** The "Validate Config" section shows `just check-config examples/cronduit.toml`, but the Troubleshooting section at line 306 shows `docker run … cronduit check /etc/cronduit/config.toml`. The binary subcommand is `check` and the `just` recipe is `check-config`. This is not incorrect, but the two names appear on adjacent pages without explaining the relationship, which may confuse new operators.

**Suggestion:** Add a one-line note: "The `just check-config` recipe wraps `cronduit check`; you can also invoke the subcommand directly as shown above."

---

### IN-06: `tests/docker_daemon_preflight.rs` contains no test for a live Docker ping success path

**File:** `tests/docker_daemon_preflight.rs:17`
**Issue:** The integration test covers the gauge lifecycle via `update_reachable_gauge` and `preflight_ping(None)`, but there is no test for `preflight_ping(Some(&live_docker))` — the happy path where `docker.ping()` returns `Ok(())` and the gauge flips to 1. The happy path is covered only in production code. If the `Ok(_)` branch in `docker_daemon.rs:53` were accidentally removed or broken, the existing test suite would not catch it.

**Suggestion:** Add a `#[cfg(feature = "integration")]` gated test that constructs a bollard client pointed at the real Docker daemon and asserts the gauge reaches 1 after `preflight_ping(Some(&client))`. This mirrors the existing pattern in `tests/docker_daemon_preflight.rs` and the testcontainers integration tier described in `CLAUDE.md`.

---

_Reviewed: 2026-04-14T02:54:29Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
