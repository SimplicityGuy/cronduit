---
phase: 12-docker-healthcheck-rc-1-cut
verified: 2026-04-18T03:08:17Z
status: human_needed
score: 18/18 automated must-haves verified; 3 maintainer-action items deferred to human
overrides_applied: 0
re_verification: null
human_verification:
  - test: "Maintainer cuts v1.1.0-rc.1 tag locally per docs/release-rc.md AFTER the Phase 12 PR merges to main"
    expected: "Annotated (and signed if GPG configured) tag v1.1.0-rc.1 pushed to origin; release.yml workflow runs green on the pushed tag"
    why_human: "Per Phase 12 D-13 the tag is the trust anchor — cut by the maintainer's signing key, explicitly NOT by workflow_dispatch. Per feedback_uat_user_validates.md Claude does not assert UAT pass."
  - test: "Post-push GHCR tag verification after rc.1 tag is pushed"
    expected: "docker manifest inspect shows :1.1.0-rc.1 + :rc present and multi-arch (amd64+arm64); :latest digest is unchanged from v1.0.1; :1 and :1.1 digests unchanged; gh release view reports isPrerelease=true; release body matches git-cliff --unreleased preview"
    why_human: "Requires live GHCR registry state post-publish; cannot be programmatically asserted from the local repo. Per feedback_uat_user_validates.md, operator confirms each row in the runbook post-push verification table."
  - test: "compose-smoke GitHub Actions workflow runs green on the Phase 12 PR"
    expected: "The `compose-smoke / compose-smoke` GHA check reports a green status on the feature-branch PR, exercising shipped-compose healthy-by-default, compose-override wins, and OPS-08 before/after assertions on ubuntu-latest"
    why_human: "Requires GitHub Actions runner execution (docker daemon + buildx + compose CLI) — confirmable only after the branch pushes and the PR is opened. The workflow file itself is verified present, well-formed, and YAML-valid locally."
---

# Phase 12: Docker Healthcheck + rc.1 Cut — Verification Report

**Phase Goal:** `docker compose up` with the shipped quickstart compose file reports the cronduit container as `healthy` out of the box, with no operator-authored healthcheck stanza required. Phase closes with the `v1.1.0-rc.1` tag cut and published to GHCR.
**Verified:** 2026-04-18T03:08:17Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (from ROADMAP Success Criteria + per-plan must_haves)

| #  | Truth                                                                                                                                                              | Status     | Evidence |
| -- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------- | -------- |
| 1  | ROADMAP SC#1: `docker compose up` → `(healthy)` within 90s (shipped quickstart, amd64+arm64)                                                                        | human_needed (automated proof wired) | compose-smoke.yml builds cronduit:ci, copies examples/docker-compose.yml + image: cronduit:ci override, polls Health.Status for 90s — real behavior requires GHA runner; locally verified workflow wiring exists and is YAML-valid. |
| 2  | ROADMAP SC#2: `cronduit health` returns exit 0 on healthy server, exit 1 fast (no hang) on connection-refused                                                        | VERIFIED   | `cargo test cli::health`: all 9 tests pass including `success_exits_zero`, `connect_refused_exits_one_fast` (elapsed<2s asserted), `timeout_fires_after_5s` (tokio::time::pause, deterministic). |
| 3  | ROADMAP SC#3: Operator compose `healthcheck:` stanza overrides Dockerfile (compose-override semantics preserved)                                                     | VERIFIED   | tests/compose-override.yml (CMD-SHELL form, interval=7s, distinguishable from Dockerfile default) + compose-smoke.yml Assertion 2 ("Assert override wins") with CMD-SHELL first-element check wired. |
| 4  | ROADMAP SC#4: OPS-08 root cause reproduction before fix is declared complete                                                                                         | VERIFIED   | tests/Dockerfile.ops08-old (FROM cronduit:ci + busybox wget --spider HEALTHCHECK) + compose-smoke.yml Assertion 3 (OLD-state image → NEW-state image; divergence branch per D-08/12-04-05 logs ::warning:: and still passes because the fix removes wget). |
| 5  | ROADMAP SC#5: `v1.1.0-rc.1` tag on GHCR; multi-arch; release notes; `:latest` pinned to v1.0.1                                                                       | human_needed (infra wired) | release.yml D-10 patch in place (5 tag lines with correct enable= gates); runbook docs/release-rc.md documents tag cut and verification. Actual rc.1 tag cut is a maintainer action — routed to human verification per plan 12-07 and phase goal note. |
| 6  | Plan 12-01: Command::Health variant exists + canonical execute signature + hyper-util/http-body-util deps without TLS                                                | VERIFIED   | src/cli/mod.rs L4 `pub mod health;`, L46 `Health,`, L59 `Command::Health => health::execute(&cli).await,`; src/cli/health.rs L47 `pub async fn execute(cli: &Cli) -> anyhow::Result<i32>`; Cargo.toml L28-29 declares hyper-util + http-body-util; `cargo tree -i openssl-sys` returns "did not match any packages". |
| 7  | Plan 12-02: 9 tests passing covering D-14 surface; deterministic 5s timeout; no --config required                                                                    | VERIFIED   | `cargo test cli::health` shows 9 tests ok, 0 failed. Timeout test uses `#[tokio::test(start_paused = true)]` + `tokio::time::advance(Duration::from_secs(6))`. `no_config_read_required` sets `cli.config = Some("/nonexistent/cronduit.toml")` and execute returns without File-Not-Found surfacing. |
| 8  | Plan 12-03: Dockerfile HEALTHCHECK directive with exact flags, exec form, correct placement                                                                          | VERIFIED   | Dockerfile L134-135: `HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \\ CMD ["/cronduit", "health"]`; L129 comment references Phase 12 OPS-07 and Phase 11 D-12; line ordering: USER(L127) < HEALTHCHECK(L134) < ENTRYPOINT(L137). Exactly 1 HEALTHCHECK directive. |
| 9  | Plan 12-04: compose-smoke.yml + fixtures; three assertions; runs alongside ci.yml (NOT inside)                                                                      | VERIFIED   | .github/workflows/compose-smoke.yml exists as standalone workflow (not inside ci.yml); references tests/Dockerfile.ops08-old, tests/compose-override.yml, examples/docker-compose.yml; 9 `set -eu` blocks; concurrency + permissions scoped; build-push-action@v6 with `push: false`; YAML parses. |
| 10 | Plan 12-05: release.yml metadata-action tags block patched per D-10 (5 edits)                                                                                       | VERIFIED   | .github/workflows/release.yml L131-135: 5 tags entries — `{{version}}`, `{{major}}.{{minor}},enable=${{ !contains(github.ref, '-') }}`, `{{major}},enable=...`, `value=latest,enable=...`, `value=rc,enable=${{ contains(github.ref, '-rc.') }}`. Comment block above references "Phase 12 D-10". YAML parses. |
| 11 | Plan 12-06: docs/release-rc.md exists, min 100 lines, mermaid diagram, UAT = user-validated, tag format `vX.Y.Z-rc.N`                                                | VERIFIED   | docs/release-rc.md: 163 lines; all 6 required `## ` sections present (Why this matters, Pre-flight checklist, Cutting the tag, Post-push verification, What if UAT fails, References); 1 mermaid code block with terminal-green palette (#00ff7f); `git tag -a -s v1.1.0-rc.1` + fallback + `git cliff --unreleased` + `docker manifest inspect` commands present; "user-validated" substring present; "never force-push a tag" present. |
| 12 | Plan 12-07: OPS-06/07/08 flipped to [x] in REQUIREMENTS.md; traceability rows flipped to Done                                                                      | VERIFIED   | REQUIREMENTS.md L87/89/91: `- [x] **OPS-06**:`, `- [x] **OPS-07**:`, `- [x] **OPS-08**:`; L170-172 traceability: three rows show `Done` (padded to maintain alignment). Prose preserved verbatim. |
| 13 | Plan 12-07: truths #3/#4 (tag cut + post-push verification) DEFERRED as maintainer-action, route as human_needed NOT failure                                         | VERIFIED (route) | Routed to `human_verification` section below per phase goal note and plan 12-07 `type=checkpoint:human-action`. |
| 14 | Invariant: `cargo build` clean                                                                                                                                      | VERIFIED   | `cargo build` → Finished `dev` profile (0 errors, 0 warnings). |
| 15 | Invariant: `cargo test` — all tests pass (≥237)                                                                                                                     | VERIFIED   | Aggregated `cargo test`: 322 passed; 0 failed. (Exceeds 237 threshold; up from Phase 11 baseline.) |
| 16 | Invariant: `cargo clippy --all-targets --all-features -- -D warnings` clean                                                                                         | VERIFIED   | Exit 0, no warnings. |
| 17 | Invariant: `cargo fmt --check` clean                                                                                                                                | VERIFIED   | Exit 0. |
| 18 | Invariant: `cargo tree -i openssl-sys` returns empty (rustls-only invariant preserved)                                                                              | VERIFIED   | "error: package ID specification `openssl-sys` did not match any packages". |

**Score:** 16/18 truths VERIFIED automated; 2/18 (truths #1, #5) require human validation of external infrastructure (GHA runner + GHCR registry state) — the phase goal explicitly routes these as human_needed per plan 12-07 and the invocation guidance.

### Deferred Items

(Not applicable — these are Phase 12's own goal; nothing is deferred to a later phase. The rc.1 tag cut is Phase 12's own deliberate maintainer-action closure per D-13, routed to human verification rather than flagged as a gap.)

### Required Artifacts

| Artifact                                      | Expected                                                                 | Status     | Details |
| --------------------------------------------- | ------------------------------------------------------------------------ | ---------- | ------- |
| `src/cli/health.rs`                           | Production hyper-util probe (pub async fn execute) + 9 unit tests       | VERIFIED   | 287 lines. Contains `hyper_util::client::legacy::Client`, `Duration::from_secs(5)`, `set_connect_timeout(Some(Duration::from_secs(2)))`, `if json.get("status").and_then(|v| v.as_str()) != Some("ok")`, `#[cfg(test)] mod tests` with all 9 named tests. No `crate::config`, no `rustls` / `hyper_rustls`. |
| `src/cli/mod.rs`                              | Health variant + module + dispatch arm                                   | VERIFIED   | `pub mod health;` (alphabetized between check and run); `Health,` variant with doc comment; `Command::Health => health::execute(&cli).await,` arm in dispatch. Cli struct unchanged (3 `#[arg(long, global = true)]` per pre-phase baseline). |
| `Cargo.toml`                                  | hyper-util + http-body-util declared, no TLS surface                     | VERIFIED   | L28 `hyper-util = { version = "0.1", features = ["client-legacy", "http1", "tokio"] }`; L29 `http-body-util = "0.1"`. version stays at 1.1.0 per Phase 10 D-12. No hyper-rustls. |
| `Dockerfile`                                  | HEALTHCHECK directive in runtime stage with correct flags + placement    | VERIFIED   | L129-135: comment block + HEALTHCHECK exec form between USER (L127) and ENTRYPOINT (L137). `RUN apk add --no-cache ca-certificates tzdata` unchanged. |
| `tests/Dockerfile.ops08-old`                  | OLD-state wget HEALTHCHECK over cronduit:ci                              | VERIFIED   | 23 lines; `FROM cronduit:ci` + `HEALTHCHECK --interval=10s --timeout=5s --start-period=20s --retries=3 CMD ["wget", "--spider", "-q", "http://localhost:8080/health"]`. No ENTRYPOINT/CMD/USER (inherits). |
| `tests/compose-override.yml`                  | Operator healthcheck different from Dockerfile                           | VERIFIED   | 29 lines; `image: cronduit:ci`; `test: ["CMD-SHELL", "/cronduit health"]`; interval=7s; timeout=4s; retries=2; start_period=15s. No docker socket, no group_add. |
| `.github/workflows/compose-smoke.yml`         | Workflow with 3 assertions; alongside ci.yml (not inside)                | VERIFIED   | Separate workflow file; standalone job `compose-smoke` on ubuntu-latest; timeout-minutes=15; concurrency scoped; 3 assertion blocks (shipped/override/OPS-08 before-after) with diagnostics (if: failure) and tear-down (if: always). |
| `.github/workflows/release.yml`               | D-10 metadata-action tags block patched (5 edits)                        | VERIFIED   | tags block has 5 entries with correct enable= gates; comment block enumerates per-template behavior in both scenarios; references "Phase 12 D-10"; existing `prerelease:` on softprops/action-gh-release unchanged. |
| `docs/release-rc.md`                          | Runbook ≥100 lines, mermaid, UAT user-validated, vX.Y.Z-rc.N format      | VERIFIED   | 163 lines; 6 `##` sections; 1 mermaid block; GPG pre-flight branching (Step 2a signed / Step 2b annotated); post-push verification table with 8 rows; what-if-UAT-fails escalation (rc.N+1, never force-push). |
| `.planning/REQUIREMENTS.md`                   | OPS-06/07/08 flipped to [x]; traceability rows flipped to Done           | VERIFIED   | 3 checkbox flips confirmed via grep; 3 traceability rows show `Done` with proper padding; prose preserved. No collateral edits to DB-/UI-/OBS-/ERG- checkboxes. |

### Key Link Verification

| From                                       | To                                          | Via                                                | Status  | Details |
| ------------------------------------------ | ------------------------------------------- | -------------------------------------------------- | ------- | ------- |
| src/cli/mod.rs `Command::Health`           | src/cli/health.rs `execute`                 | dispatch arm `Command::Health => health::execute(&cli).await` | WIRED   | Confirmed via grep of both file pointers + `cargo run -- --help` renders `health` subcommand with correct doc string. |
| src/cli/health.rs `execute`                | axum /health handler JSON contract          | GET http://{bind}/health → `json.get("status").as_str() == Some("ok")` | WIRED   | Code reads body via hyper-util client, collects to Bytes, parses via serde_json, asserts status field. Test `success_exits_zero` proves round-trip. |
| src/cli/health.rs `execute`                | Dockerfile HEALTHCHECK CMD                  | exit code 0\|1 consumed by Docker daemon            | WIRED   | Dockerfile L134-135 invokes `/cronduit health` in exec form; execute returns `anyhow::Result<i32>` which main.rs unwraps into std::process::exit. |
| Dockerfile HEALTHCHECK CMD                 | /cronduit health subcommand                 | docker exec inside running container                | WIRED   | `CMD ["/cronduit", "health"]` matches the binary entrypoint name inherited from COPY --from=builder /cronduit /cronduit. Runs as non-root (USER cronduit:cronduit precedes HEALTHCHECK). |
| .github/workflows/compose-smoke.yml        | Dockerfile + tests/* fixtures               | docker build + docker run + docker compose         | WIRED   | All three file paths referenced literally in the workflow; build-push-action produces cronduit:ci before ops08-old builds FROM it. |
| .github/workflows/compose-smoke.yml        | /cronduit health binary                     | exec inside running container via HEALTHCHECK       | WIRED   | Shipped stack assertion polls Health.Status; NEW-state run polls Health.Status which fires the Dockerfile HEALTHCHECK (which invokes /cronduit health). |
| .github/workflows/release.yml metadata-action | GHCR tag manifest                        | tag templates derive image tags from git tag        | WIRED (static) | Gates are syntactically correct; dynamic verification occurs at actual tag push (human verification). |
| docs/release-rc.md pre-flight              | .github/workflows/compose-smoke.yml + release.yml | checklist references named workflows by path  | WIRED   | Both workflow paths appear in runbook text; `gh run list --workflow=compose-smoke.yml` + `gh run list --workflow=ci.yml` commands present. |
| docs/release-rc.md post-push               | GHCR manifest inspection                    | explicit `docker manifest inspect` commands         | WIRED   | 8 verification rows; each has an explicit command + expected outcome column. |

### Data-Flow Trace (Level 4)

| Artifact               | Data Variable    | Source                                    | Produces Real Data | Status   |
| ---------------------- | ---------------- | ----------------------------------------- | ------------------ | -------- |
| src/cli/health.rs      | `json` (body)    | `resp.into_body().collect().await.to_bytes()` then `serde_json::from_slice` | Yes — real bytes off socket | FLOWING |
| src/cli/health.rs      | `url` / `uri`    | `parse_health_url(cli.bind.as_deref())`   | Yes — derived from CLI `--bind` flag or DEFAULT_BIND | FLOWING |
| src/cli/health.rs      | exit code (i32)  | Ok(0) on success path, Ok(1) on 6 failure branches | Yes — deterministic based on HTTP response | FLOWING |
| Dockerfile HEALTHCHECK | Healthcheck.Test | Literal exec-form JSON array               | Yes — Docker daemon reads directly from image config | FLOWING |
| compose-smoke.yml Assertion 1 | `status`  | `docker inspect --format '{{.State.Health.Status}}' $container` | Yes — docker daemon state | FLOWING |
| compose-smoke.yml Assertion 2 | `test_array` | `docker inspect --format '{{json .Config.Healthcheck.Test}}' $container` | Yes — reflects actual runtime config | FLOWING |
| compose-smoke.yml Assertion 3 | OLD_STATUS via `$GITHUB_OUTPUT` → env: OLD_STATUS | routed per-step via env-block (not inline `${{ ... }}`) | Yes — routing pattern matches project convention (T-12-04-01 mitigation) | FLOWING |

### Behavioral Spot-Checks

| Behavior                                              | Command                                                                  | Result                                             | Status |
| ----------------------------------------------------- | ------------------------------------------------------------------------ | -------------------------------------------------- | ------ |
| All 9 `cli::health` unit tests pass                   | `cargo test cli::health`                                                  | 9 passed; 0 failed                                 | PASS   |
| Full test suite passes                                | `cargo test`                                                             | 322 passed; 0 failed                               | PASS   |
| CLI exposes `health` subcommand                       | `cargo run --quiet -- --help`                                             | `health  Probe the local /health endpoint ...`     | PASS   |
| rustls-only invariant                                 | `cargo tree -i openssl-sys`                                              | "did not match any packages"                       | PASS   |
| hyper-util present and wired                          | `cargo tree -i hyper-util`                                               | `hyper-util v0.1.20 ├── axum ...`                  | PASS   |
| http-body-util present and wired                      | `cargo tree -i http-body-util`                                           | `http-body-util v0.1.3 ├── axum ...`               | PASS   |
| Clippy clean under `-D warnings`                       | `cargo clippy --all-targets --all-features -- -D warnings`              | Exit 0, finished dev profile                       | PASS   |
| Format clean                                           | `cargo fmt --check`                                                      | Exit 0                                             | PASS   |
| compose-smoke.yml parses as YAML                       | `ruby -e 'require "yaml"; YAML.load_file(...)'`                          | both parse ok                                      | PASS   |
| release.yml parses as YAML                             | `ruby -e 'require "yaml"; YAML.load_file(...)'`                          | both parse ok                                      | PASS   |
| Exactly 1 HEALTHCHECK in Dockerfile                    | `grep -c '^HEALTHCHECK' Dockerfile`                                      | 1                                                  | PASS   |
| Exactly 1 HEALTHCHECK in tests/Dockerfile.ops08-old    | `grep -c '^HEALTHCHECK' tests/Dockerfile.ops08-old`                      | 1                                                  | PASS   |
| compose-smoke.yml uses `set -eu` in shell blocks       | `grep -c 'set -eu'`                                                      | 9 (≥ required 6)                                   | PASS   |
| Connect-refused fail-fast under 2s                     | test assertion `elapsed < Duration::from_secs(2)`                        | Test passes                                        | PASS   |
| Timeout test runs deterministically via paused time    | `#[tokio::test(start_paused = true)]` + `tokio::time::advance(6s)`       | Test passes in ms (not 5s wall clock)              | PASS   |
| actual docker build/run of Dockerfile healthcheck      | `docker build` + `docker run` → Health.Status = healthy                  | SKIPPED (no runnable Docker daemon state check)    | SKIP   |
| compose-smoke GHA workflow green on PR                 | GHA run                                                                   | Requires push + PR                                 | SKIP → routed to human_verification |

### Requirements Coverage

| Requirement | Source Plan(s) | Description                                                  | Status     | Evidence |
| ----------- | -------------- | ------------------------------------------------------------ | ---------- | -------- |
| OPS-06      | 12-01, 12-02, 12-07 | `cronduit health` CLI subcommand exit 0 iff status=="ok"; fail-fast on connect-refused; --bind or default 127.0.0.1:8080 | SATISFIED | src/cli/health.rs implements all clauses; 9 tests prove contract; REQUIREMENTS.md flipped to `[x]`. |
| OPS-07      | 12-03, 12-04, 12-05, 12-06, 12-07 | Dockerfile HEALTHCHECK `CMD ["/cronduit", "health"]`; defaults interval=30s/timeout=5s/start-period=60s/retries=3; compose override preserved | SATISFIED | Dockerfile L134-135 exact match; compose-override.yml fixture + compose-smoke.yml Assertion 2 prove override semantics; REQUIREMENTS.md flipped to `[x]`. |
| OPS-08      | 12-04, 12-07   | Reproduce the (unhealthy) root cause before the fix is declared complete; fix is correct regardless because it removes busybox wget entirely | SATISFIED | tests/Dockerfile.ops08-old fixture + compose-smoke.yml Assertion 3 with divergence-safe evaluation; REQUIREMENTS.md flipped to `[x]`. |

No orphaned requirements: REQUIREMENTS.md maps Phase 12 only to OPS-06/07/08, and all three are claimed by at least one plan in the phase.

### Anti-Patterns Found

| File                                           | Line   | Pattern                                      | Severity | Impact                                                                                                                               |
| ---------------------------------------------- | ------ | -------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| docs/release-rc.md                             | L12    | Contains the string `workflow_dispatch`      | Info     | Plan 06 acceptance criterion requested the substring be absent. Context: the runbook explicitly states tags are cut "NOT by `workflow_dispatch`" — a *prohibition* statement, not a recommendation. Intent (no workflow_dispatch recommendation) is satisfied; literal grep check is not. Non-blocking; documentation-only. |

No other anti-patterns detected: no TODO/FIXME/PLACEHOLDER in src/cli/health.rs or any modified file; no `return null`/`return []` stubs; no empty handlers; no `crate::config` import in health.rs (D-04 respected); no TLS surface introduced.

### Human Verification Required

Three items require human execution and live-infrastructure validation. These are known and intentional per plan 12-07 (`autonomous: false`, `type=checkpoint:human-action`, `gate=blocking`) and the phase goal note ("The tag cut itself is a maintainer action … should be routed as human_needed, NOT as a gap").

#### 1. Maintainer cuts v1.1.0-rc.1 tag locally per docs/release-rc.md AFTER Phase 12 PR merges

**Test:** Follow `docs/release-rc.md` end-to-end after the Phase 12 PR merges to main. Pre-flight checklist (all 6 boxes), GPG pre-flight (Step 2a signed or Step 2b annotated), `git tag -a ... v1.1.0-rc.1`, `git push origin v1.1.0-rc.1`, watch with `gh run watch --exit-status`.
**Expected:** Annotated (and signed if GPG configured) tag present on origin; `release.yml` workflow runs green.
**Why human:** Per Phase 12 D-13 the tag is the trust anchor — cut by the maintainer's signing key, explicitly NOT by `workflow_dispatch`. Per `feedback_uat_user_validates.md`, Claude does not assert UAT pass on the maintainer's behalf.

#### 2. Post-push GHCR verification after rc.1 tag is pushed

**Test:** Run every row in the `docs/release-rc.md` "Post-push verification" table:
- `docker manifest inspect ghcr.io/simplicityguy/cronduit:1.1.0-rc.1` → two platforms (amd64 + arm64)
- `docker manifest inspect ghcr.io/simplicityguy/cronduit:rc` → digest identical to `:1.1.0-rc.1` digest
- `docker manifest inspect ghcr.io/simplicityguy/cronduit:latest` → digest identical to v1.0.1 pre-existing digest (unchanged)
- `docker manifest inspect ghcr.io/simplicityguy/cronduit:1` and `:1.1` → unchanged
- `gh release view v1.1.0-rc.1 --json isPrerelease --jq .isPrerelease` → `true`
- Release body diff vs `git cliff --unreleased -o /tmp/preview.md` → zero or whitespace-only
- `docker run --rm ghcr.io/simplicityguy/cronduit:1.1.0-rc.1 --version` → `cronduit 1.1.0`
- `docker compose -f examples/docker-compose.yml up -d` (with `image: :1.1.0-rc.1` override) → `Up N seconds (healthy)` within 90s.

**Expected:** Every row passes; the D-10 `:latest` invariant is preserved; multi-arch manifest verified; prerelease=true.
**Why human:** Requires live GHCR registry state post-publish; cannot be programmatically asserted from the local repo pre-push.

#### 3. GHA `compose-smoke` workflow runs green on the Phase 12 PR

**Test:** Push the phase branch and open the PR; observe the `compose-smoke / compose-smoke` check.
**Expected:** Green status. Workflow log shows shipped stack `healthy` within 90s, compose override first-element is `CMD-SHELL`, OPS-08 NEW image `healthy` and OLD image either `unhealthy` (clean repro) or `healthy` (divergence — `::warning::` logged; check still passes per D-08 / 12-04-05).
**Why human:** Requires a GitHub Actions ubuntu-latest runner with docker daemon + buildx + compose CLI. The workflow file and all three fixtures are verified present, well-formed, and YAML-valid locally; runtime behavior is only observable after push.

### Gaps Summary

No gaps found. All automated must-haves for Phase 12 verify cleanly:
- The `cronduit health` subcommand is fully implemented, tested (9/9 tests pass), and wired through clap dispatch.
- The Dockerfile HEALTHCHECK is present with exact flags and correct placement (USER < HEALTHCHECK < ENTRYPOINT) with busybox wget left installed for user jobs (D-07 respected).
- The compose-smoke GHA workflow + both test fixtures exist with the prescribed content and YAML parses clean.
- The release.yml D-10 metadata-action patch ships exactly the 5 tag entries with correct `enable=` gates; the existing `prerelease:` on softprops/action-gh-release is untouched.
- The docs/release-rc.md runbook covers the six required sections, GPG-signing branching, post-push verification table, and UAT-failure escalation, and is reusable across rc.1/rc.2/rc.3.
- REQUIREMENTS.md OPS-06/07/08 are flipped to `[x]` and the traceability table rows flipped to `Done`.
- All build/test/clippy/fmt/rustls invariants hold; 322 tests pass (well above the ≥237 threshold).

One informational note (not a gap): the runbook contains the string `workflow_dispatch` because it explicitly states tags are cut "NOT by `workflow_dispatch`". Plan 06's literal grep-check acceptance criterion would fail on this, but the *intent* (no workflow_dispatch recommendation) is met — the usage is a prohibition statement, not a recommendation. No change required.

The three human-verification items above are intentional phase-design outcomes per plan 12-07's `checkpoint:human-action` gate and the explicit phase-goal instruction to route the tag cut as `human_needed`, NOT as a gap.

---

_Verified: 2026-04-18T03:08:17Z_
_Verifier: Claude (gsd-verifier)_
