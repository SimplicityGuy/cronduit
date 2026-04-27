---
phase: 12-docker-healthcheck-rc-1-cut
reviewed: 2026-04-17T00:00:00Z
depth: deep
files_reviewed: 8
files_reviewed_list:
  - src/cli/mod.rs
  - src/cli/health.rs
  - Cargo.toml
  - Dockerfile
  - tests/Dockerfile.ops08-old
  - tests/compose-override.yml
  - .github/workflows/compose-smoke.yml
  - .github/workflows/release.yml
findings:
  critical: 0
  high: 0
  medium: 2
  low: 3
  nit: 3
  total: 8
status: clean
---

# Phase 12: Code Review Report

**Reviewed:** 2026-04-17
**Depth:** deep (cross-file — health.rs probe wired against src/web/handlers/health.rs endpoint, Dockerfile HEALTHCHECK cross-checked against examples/cronduit.toml bind posture, compose-smoke workflow cross-checked against Dockerfile runtime stage and cronduit config defaults)
**Files Reviewed:** 8
**Status:** clean (no critical/high)

## Summary

Phase 12 ships a `cronduit health` subcommand (hyper-util probe), a Dockerfile `HEALTHCHECK` directive that targets it, a dedicated compose-smoke GHA workflow, and a patched release workflow tag set. All four pieces are internally consistent and honor the locked project constraints:

- **rustls-only / no openssl-sys:** `hyper-util` + `http-body-util` are pure-Rust; the probe uses plain `HttpConnector` (no TLS) so the constraint is trivially met. Nothing new in `Cargo.toml` pulls `openssl-sys`.
- **Async-only:** No blocking IO in `health.rs`; every syscall is `.await`ed under `tokio::time::timeout`.
- **GSD pitfall "never inline `${{ ... }}` in run:":** `compose-smoke.yml` routes `steps.ops08_old.outputs.old_status` through an `env:` block (line 194-195). `release.yml` routes `github.repository` through `REPO` (line 49-52).
- **GSD pitfall "compose wins over Dockerfile HEALTHCHECK":** verified by Assertion 2 in compose-smoke.yml.
- **`:latest` pinning invariant (D-10):** the `enable=` gate on the metadata-action tag templates is correct — `!contains(github.ref, '-')` for `:latest`/`:{major}`/`:{major}.{minor}` and `contains(github.ref, '-rc.')` for `:rc`.

Clippy-clean patterns throughout (no `unwrap()` outside tests, no `as any`, every `Result` arm is matched, tracing carries structured fields). The handful of findings below are robustness and clarity nits — none block the rc.1 cut.

## Medium

### MD-01: compose-smoke Assertion 3 `docker run` lacks `/data` mount and `DATABASE_URL` env — default SQLite path is `./cronduit.db` under CWD `/`, which UID 1000 cannot write

**File:** `.github/workflows/compose-smoke.yml:164` and `:180`
**Issue:** `docker run -d --name cronduit-ops08-old -p 18080:8080 cronduit:ops08-old` (and the symmetric NEW run at line 180) does NOT mount `/data` and does NOT set `DATABASE_URL`. The image's default CMD is `run --config /etc/cronduit/config.toml`; the embedded config omits `database_url`, so `src/config/mod.rs:62` falls back to `sqlite://./cronduit.db?mode=rwc`. The runtime stage of `Dockerfile` does not set a `WORKDIR`, so CWD is `/`. Alpine `/` is owned by root:root with mode 0755, and the image runs as `cronduit:cronduit` (UID 1000). sqlx cannot create `/cronduit.db` → cronduit exits before the listener binds → both OLD and NEW sit at `Health.Status=starting` and the NEW assertion fails at `:190` with `never reached healthy within 90s`.

Assertion 1 (shipped compose) is NOT affected because the shipped docker-compose mounts the `cronduit-data` named volume at `/data` (pre-created by the Dockerfile with UID 1000 ownership) AND sets `DATABASE_URL=sqlite:///data/cronduit.db`. Assertion 2 (compose-override) IS affected in the same way as Assertion 3 — `tests/compose-override.yml:19-21` sets `DATABASE_URL=sqlite:///data/cronduit.db` but does NOT mount a volume at `/data`, so SQLite writes to the in-container tmpfs-backed `/data` which WAS pre-created writable (line 120 in Dockerfile). That works.

Assertion 3 has neither the mount nor a usable env var. If the workflow was ever observed passing in local testing, it was likely because the local Docker daemon left `/` writable in some odd overlay-fs state; on a clean `ubuntu-latest` GHA runner this will almost certainly fail.

**Fix:** Either set `DATABASE_URL=sqlite:///data/cronduit.db` on both `docker run` invocations (the `/data` dir already exists with cronduit ownership per Dockerfile line 120), OR mount a tmpfs at `/data`:

```yaml
- name: Run OLD image and observe Health.Status (max 60s)
  id: ops08_old
  run: |
    set -eu
    docker run -d --name cronduit-ops08-old \
      -e DATABASE_URL=sqlite:///data/cronduit.db \
      -p 18080:8080 cronduit:ops08-old
    ...

- name: Run NEW image and assert healthy (max 90s)
  run: |
    set -eu
    docker run -d --name cronduit-ops08-new \
      -e DATABASE_URL=sqlite:///data/cronduit.db \
      -p 28080:8080 cronduit:ci
```

Only set this once the workflow is observed failing in CI — if GHA's runner happens to be permissive about `/` writes for UID 1000 (which shouldn't be the case but has happened on older Moby versions), the current code works and the fix is future-proofing.

### MD-02: `cronduit health` ignores `--bind` flag at HEALTHCHECK time — operator-customized bind port silently breaks the probe

**File:** `Dockerfile:134-135`
**Issue:** The HEALTHCHECK is `CMD ["/cronduit", "health"]` with NO `--bind`. The probe therefore always targets the hardcoded `127.0.0.1:8080` default (`src/cli/health.rs:28`). If an operator customizes the runtime to bind a non-default port — either via `command: ["run", "--bind", "0.0.0.0:9000"]` in compose, or by editing the mounted `cronduit.toml` to set `[server] bind = "0.0.0.0:9000"` — the HEALTHCHECK continues probing `:8080`, finds no listener, and flips the container to (unhealthy) despite the server being perfectly healthy on `:9000`.

This is a second-order correctness gap (today's quickstart pins `8080`), but it's a footgun for the documented "operator can override" story and will surface the first time someone runs cronduit on a non-8080 port.

**Fix:** One of:
1. **Accept a `CRONDUIT_BIND` env var in `src/cli/health.rs`** (belt-and-suspenders) so operators can do `environment: - CRONDUIT_BIND=0.0.0.0:9000` in compose and the HEALTHCHECK picks it up without a Dockerfile change.
2. **Document the limitation in `docs/release-rc.md` or `README.md`** under a "Healthcheck assumes port 8080" callout, with an operator-supplied `healthcheck:` override as the workaround (compose wins over Dockerfile per Assertion 2, which is already tested).
3. **Pass `--bind` via the Dockerfile CMD wrapping** (not viable — HEALTHCHECK can't see the server's CMD args at runtime).

Option 2 is the lowest-risk fix for rc.1; option 1 is the proper follow-up for rc.2+.

## Low

### LO-01: `health.rs` sets `Host` header explicitly even though `hyper-util` would derive it from the URI authority — redundant and risks drifting from the URI if the two disagree

**File:** `src/cli/health.rs:67-70`
**Issue:**
```rust
let req = match Request::builder()
    .uri(uri)
    .header(hyper::header::HOST, bind)
    .body(Empty::<Bytes>::new())
```
`hyper_util::client::legacy::Client::request` auto-derives the `Host` header from the URI authority when the header is absent. Setting it explicitly to `bind` is functionally identical today (`bind` == URI authority by construction of `parse_health_url`), but if a future change makes the URL and `Host` derive from different sources, they can drift silently.

**Fix:** Drop the explicit `.header(Host, bind)` — let hyper derive it. Alternatively, keep it but add a code comment pinning the invariant ("Host MUST equal the URI authority; both derive from `bind`"). Trivial.

### LO-02: `parse_health_url` accepts any string as `bind` — path segments, queries, and spaces are not rejected

**File:** `src/cli/health.rs:37-40`
**Issue:** `parse_health_url(Some("127.0.0.1:8080/admin?x=y"))` returns `"http://127.0.0.1:8080/admin?x=y/health"`. The subsequent `uri.parse()` at `:50` will either accept this (producing a probe against `/admin?x=y/health`, wrong endpoint) or reject it (fallback to exit 1). Since `--bind` is a CLI flag controlled by the operator (not an attacker surface), this is a correctness/debuggability issue, not a security one.

Note: `cronduit run` rejects such values via `SocketAddr::from_str` at `src/cli/run.rs:59`, so the two subcommands have inconsistent tolerance for bad `--bind` values.

**Fix:** Validate `bind` in `health.rs` the same way `run.rs` does — parse it as `SocketAddr` first, reject with exit 1 + tracing::error! if invalid. Keeps the two subcommands in sync and produces a clearer error message than a downstream URI parse failure.

```rust
use std::net::SocketAddr;
use std::str::FromStr;

// Validate bind early — mirrors src/cli/run.rs:59.
let _: SocketAddr = match SocketAddr::from_str(bind) {
    Ok(a) => a,
    Err(e) => {
        tracing::error!(target: "cronduit.health", bind = %bind, error = %e, "invalid --bind");
        return Ok(1);
    }
};
```

### LO-03: `release.yml` uses unquoted `$TAG` in `echo ... | cut`

**File:** `.github/workflows/release.yml:61-62`
**Issue:**
```bash
echo "major=$(echo $TAG | cut -d. -f1)" >> "$GITHUB_OUTPUT"
echo "minor=$(echo $TAG | cut -d. -f1-2)" >> "$GITHUB_OUTPUT"
```
`$TAG` is unquoted. Because `TAG` derives from `${GITHUB_REF#refs/tags/v}` (server-controlled, semver-shaped), this is not a security issue — but shellcheck would flag it as SC2086 and it clashes with the quoted-everywhere style used elsewhere in the workflow. Also: `steps.version.outputs.major` / `.minor` do not appear to be consumed anywhere in the rest of the workflow (metadata-action derives its own `{{major}}`/`{{major}}.{{minor}}` from the ref), so these two lines are dead code unless a downstream step is about to use them.

**Fix:** Either delete the two unused outputs, or quote:

```bash
echo "major=$(echo "$TAG" | cut -d. -f1)" >> "$GITHUB_OUTPUT"
echo "minor=$(echo "$TAG" | cut -d. -f1-2)" >> "$GITHUB_OUTPUT"
```

## Nit

### NI-01: `Cargo.toml` has `tokio = "1.52"` in `[dependencies]` and `tokio = "1.51"` in `[dev-dependencies]`

**File:** `Cargo.toml:21` and `Cargo.toml:142`
**Issue:** The dev-dependency override specifies an OLDER version (1.51) than the main dep (1.52). Cargo's resolver will pick 1.52 (the higher of the two semver-compatible constraints), so this is functionally a no-op, but the dev-deps override only exists to add the `test-util` feature — the version string should match the main dep to avoid confusion on future bumps.

**Fix:** Bump dev-deps to `1.52`, or drop the version string entirely (cargo resolves from the main dep):

```toml
[dev-dependencies]
tokio = { features = ["full", "test-util"] }
```
(Note: cargo still requires the version field — use `tokio = { version = "1.52", features = ["full", "test-util"] }`.)

### NI-02: `health.rs` `TIMEOUT` is a `Duration::from_secs(5)` constant but the tracing log field is the literal `5`

**File:** `src/cli/health.rs:91`
**Issue:**
```rust
tracing::error!(target: "cronduit.health", timeout_secs = 5, "request timed out");
```
The `5` is a magic number duplicated from `TIMEOUT`. If `TIMEOUT` is ever changed, this stays 5 and lies.

**Fix:**
```rust
tracing::error!(
    target: "cronduit.health",
    timeout_secs = TIMEOUT.as_secs(),
    "request timed out",
);
```

### NI-03: `docs/release-rc.md` post-push verification table includes eight checks but some are trivially redundant with others

**File:** `docs/release-rc.md:122-131`
**Issue:** Checks `:1` and `:1.1` are unchanged, `:latest` unchanged, and `manifest digest IDENTICAL` all encode the same D-10 invariant (`:latest` gated on hyphen-free ref). A first-time rc cutter reading the checklist could reasonably skip `:1` / `:1.1` after verifying `:latest`, because all three are driven by the same `enable=` clauses. Not incorrect — just dense.

**Fix:** Consider collapsing into a single "Verify no stable tags moved" step or adding a sentence in the note at the bottom: "Checks 3 and 4 are driven by the same D-10 invariant — failing one will fail both. If you are short on time and check 3 passes, check 4 is expected to pass as well." Documentation nit only.

## Notes (not findings)

These were checked and are fine — recorded here as review context, not issues:

- **No unwrap() in non-test code in `health.rs`.** Every `Result` is matched; the only `.unwrap()` calls are in the `#[cfg(test)] mod tests` block.
- **`tokio::time::timeout(TIMEOUT, ...)` + `connector.set_connect_timeout(Duration::from_secs(2))` compose correctly.** Outer 5 s is the absolute upper bound; inner 2 s connect is the fast-fail for connect-refused/DNS. Test `connect_refused_exits_one_fast` asserts `elapsed < 2s` which is the right shape.
- **`src/cli/mod.rs` `Health` variant takes no config path argument and correctly inherits the global `--bind` / `--log-format`/`--config` flags via `clap(global = true)`.** The `no_config_read_required` test proves `--config` is accepted but unused.
- **`rustls-only` constraint preserved.** `hyper-util`/`http-body-util`/`hyper 1` have no TLS features enabled; probe uses plain `HttpConnector`. No `openssl-sys` in `Cargo.toml`.
- **`tests/Dockerfile.ops08-old` correctly inherits USER/ENTRYPOINT/CMD from `cronduit:ci`** — only the `HEALTHCHECK` line changes, which is exactly what the OPS-08 before/after test is meant to isolate.
- **Dockerfile `HEALTHCHECK --start-period=60s`** is correctly calibrated for the Phase 11 D-12 listener-after-backfill behavior (server binds only after orphan reconcile completes). `tests/Dockerfile.ops08-old`'s `--start-period=20s` is appropriate for the fixture's faster verdict.
- **Compose-override fixture uses `CMD-SHELL` form** — this is the single-byte marker Assertion 2 asserts on, and it's intentionally different from the Dockerfile's `CMD` (exec) form. Correct.
- **`docker/metadata-action@v5` tag templates honor D-10:** `:latest`/`:{major}`/`:{major}.{minor}` are all gated with `enable=${{ !contains(github.ref, '-') }}`, which skips on any pre-release tag. `:rc` is gated with `enable=${{ contains(github.ref, '-rc.') }}`. Both expressions are correct for the dot-separated semver convention the runbook requires.
- **`release.yml` GITHUB_REF handling** (`TAG="${GITHUB_REF#refs/tags/v}"`) is safe — `GITHUB_REF` is server-controlled for tag pushes.
- **Two pinned actions (`orhun/git-cliff-action` and `softprops/action-gh-release`) use SHA + version comment** — good supply-chain hygiene. The rest are major-version tags (`@v3`/`@v4`/`@v5`/`@v6`), consistent with the rest of the project's workflow style.
- **docs/release-rc.md mermaid diagram** uses the canonical terminal-green palette from `design/DESIGN_SYSTEM.md` / `docs/CI_CACHING.md`. Compliant with the CLAUDE.md "all diagrams must be mermaid" rule.

---

_Reviewed: 2026-04-17_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: deep_
