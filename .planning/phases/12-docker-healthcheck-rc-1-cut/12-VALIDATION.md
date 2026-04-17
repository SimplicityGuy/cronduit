---
phase: 12
slug: docker-healthcheck-rc-1-cut
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-17
---

# Phase 12 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` + `cargo nextest` (project standard; dev-deps in `Cargo.toml`; `justfile` recipe `nextest`) |
| **Config file** | `Cargo.toml` (no separate test config); `[features].integration` for testcontainers-gated tests |
| **Quick run command** | `cargo test cli::health` (the seven `cronduit health` unit tests run in <2s) |
| **Full suite command** | `just nextest` |
| **Estimated runtime** | Quick: ~2s · Full suite: ~3-5min (matches CI standard) · compose-smoke workflow: ~4min on `ubuntu-latest` |

A separate runner is required for compose-smoke: GitHub Actions `ubuntu-latest` with bash + docker + docker compose (no Rust toolchain). Lives in NEW `.github/workflows/compose-smoke.yml` per D-09.

---

## Sampling Rate

- **After every task commit:** `cargo test cli::health` — quick feedback loop during D-14 implementation (<2s).
- **After every plan wave:** `just nextest` (full Rust suite). Compose-smoke workflow runs in parallel via PR check.
- **Before `/gsd-verify-work`:** Full suite green AND `compose-smoke.yml` workflow green on the feature branch's PR.
- **Manual UAT (post-tag-push, per `feedback_uat_user_validates.md`):** Maintainer runs runbook commands against published `:rc` tag and confirms images, manifests, release notes match expectations. CI cannot self-validate UAT for the rc.1 image.
- **Max feedback latency:** ~2s for unit tests; ~4-6min for compose-smoke (acceptable as PR check, not per-commit).

---

## Per-Task Verification Map

> Plan-task IDs are placeholders (`{N}-NN-NN`) — populated when plans are written. The Req → Test mapping below is authoritative; planner assigns task IDs.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 12-02-01 | 02 | 1 | OPS-06 | V11 | Exit 0 iff HTTP 200 + `body.status=="ok"` | unit | `cargo test cli::health::tests::success_exits_zero` | ❌ W0 — `src/cli/health.rs` | ⬜ pending |
| 12-02-02 | 02 | 1 | OPS-06 | V11 | Exit 1 on non-200 (e.g., 503) | unit | `cargo test cli::health::tests::non_200_exits_one` | ❌ W0 — `src/cli/health.rs` | ⬜ pending |
| 12-02-03 | 02 | 1 | OPS-06 | V11 | Exit 1 when body missing `status` field | unit | `cargo test cli::health::tests::missing_status_field_exits_one` | ❌ W0 — `src/cli/health.rs` | ⬜ pending |
| 12-02-04 | 02 | 1 | OPS-06 | — | Exit 1 fast on connect-refused | unit | `cargo test cli::health::tests::connect_refused_exits_one_fast` | ❌ W0 — `src/cli/health.rs` | ⬜ pending |
| 12-02-05 | 02 | 1 | OPS-06 | V5 | URL parses from v4 / v6-bracketed / missing-port forms | unit | `cargo test cli::health::tests::url_construction_v4`, `..._v6`, `..._missing_port` | ❌ W0 — `src/cli/health.rs` | ⬜ pending |
| 12-02-06 | 02 | 1 | OPS-06 | V12 | `cronduit health` does NOT read `--config` (D-04) | unit | `cargo test cli::health::tests::no_config_read_required` | ❌ W0 — `src/cli/health.rs` | ⬜ pending |
| 12-02-07 | 02 | 1 | OPS-06 | V11 | 5s total timeout fires deterministically | unit | `cargo test cli::health::tests::timeout_fires_after_5s` (uses `tokio::time::pause` + `advance`) | ❌ W0 — `src/cli/health.rs` | ⬜ pending |
| 12-03-01 | 03 | 2 | OPS-07 | V14 | Dockerfile HEALTHCHECK directive present and well-formed | smoke (build-time) | `docker build -t cronduit:ci . && docker inspect --format '{{json .Config.Healthcheck}}' cronduit:ci \| jq` | ❌ W0 — `.github/workflows/compose-smoke.yml` | ⬜ pending |
| 12-04-01 | 04 | 2 | OPS-07 | — | Shipped `examples/docker-compose.yml` reports `healthy` ≤90s (T-V11-HEALTH-01) | smoke (compose) | `docker compose -f examples/docker-compose.yml up -d` then poll `docker inspect --format '{{.State.Health.Status}}'` until `healthy` or 90s timeout | ❌ W0 — `.github/workflows/compose-smoke.yml` | ⬜ pending |
| 12-04-02 | 04 | 2 | OPS-07 | V14 | Operator compose `healthcheck:` overrides Dockerfile (T-V11-HEALTH-02) | smoke (compose) | `docker compose -f tests/compose-override.yml up -d` then assert `.Config.Healthcheck.Test` matches operator override, not Dockerfile | ❌ W0 — `.github/workflows/compose-smoke.yml` + `tests/compose-override.yml` | ⬜ pending |
| 12-04-03 | 04 | 2 | OPS-08 | — | OLD `wget --spider` HEALTHCHECK reproduces `unhealthy` | smoke (CI) | Build `tests/Dockerfile.ops08-old`; `docker run`; wait start-period; assert `unhealthy` | ❌ W0 — `tests/Dockerfile.ops08-old` + workflow step | ⬜ pending |
| 12-04-04 | 04 | 2 | OPS-08 | — | NEW `cronduit health` HEALTHCHECK reproduces `healthy` (the fix) | smoke (CI) | `docker run cronduit:ci`; wait 75s (start-period 60s + slack 15s); assert `healthy` | ❌ W0 — same workflow step | ⬜ pending |
| 12-04-05 | 04 | 2 | OPS-08 | — | Repro robustness: if OLD also returns healthy (root cause was different), workflow logs the divergence and still passes (cronduit health closes category regardless) | smoke (CI) | Workflow step branches on OLD result; both `(unhealthy → healthy)` and `(healthy → healthy + log)` accepted | ❌ W0 — workflow step | ⬜ pending |
| 12-05-01 | 05 | 1 | (D-10) | V14 | Pre-release tag does NOT push `:latest` / `:major` / `:major.minor` | meta-test (post-merge UAT) | `docker manifest inspect ghcr.io/simplicityguy/cronduit:latest` digest === `v1.0.1` digest after rc.1 push | manual (runbook) | ⬜ pending |
| 12-05-02 | 05 | 1 | (D-10) | — | Pre-release tag DOES push `:rc` rolling tag | meta-test | `docker manifest inspect ghcr.io/simplicityguy/cronduit:rc` digest === `v1.1.0-rc.1` digest | manual (runbook) | ⬜ pending |
| 12-06-01 | 06 | 2 | (D-11) | — | Multi-arch image pushed (amd64 + arm64) | meta-test | `docker manifest inspect -v ghcr.io/.../cronduit:v1.1.0-rc.1` shows both platforms | manual (runbook) | ⬜ pending |
| 12-06-02 | 06 | 2 | (D-12) | — | git-cliff release notes preview matches GitHub Release body | meta-test | `git cliff --unreleased -o /tmp/preview.md` then diff against published Release body | manual (runbook) | ⬜ pending |
| 12-07-01 | 07 | 3 | (D-13) | V14 | Tag is annotated (and signed if GPG configured); maintainer-cut, not workflow_dispatch | UAT (manual) | Runbook step: `git config --get user.signingkey` pre-flight + `git tag -v v1.1.0-rc.1` post-create | manual (runbook) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `src/cli/health.rs` — NEW file. Contains `pub async fn execute(cli: &Cli) -> anyhow::Result<i32>` and `#[cfg(test)] mod tests` per D-14. Covers OPS-06 unit tests (12-02-01 through 12-02-07).
- [ ] `tests/Dockerfile.ops08-old` — NEW fixture, layers OLD `wget --spider` HEALTHCHECK on top of `cronduit:ci`. Covers OPS-08 repro (12-04-03).
- [ ] `tests/compose-override.yml` — NEW fixture, defines a service with explicit `healthcheck:` test command different from the Dockerfile's. Covers T-V11-HEALTH-02 (12-04-02).
- [ ] `.github/workflows/compose-smoke.yml` — NEW workflow. Multi-step: (1) build `cronduit:ci`; (2) OPS-08 OLD-state repro; (3) OPS-08 NEW-state assert; (4) compose-override smoke; (5) shipped `examples/docker-compose.yml` healthy-by-default smoke. All in one workflow on `ubuntu-latest` per D-09. Triggers: PR + main + tags.
- [ ] `docs/release-rc.md` — NEW runbook. Pre-flight (commits merged, compose-smoke green, `git cliff --unreleased` preview), exact tag commands (with branching for GPG signing per RESEARCH §Assumption A5), post-push verification, what-if-UAT-fails escalation (ship rc.N+1, not hotfix tag).
- [ ] No new test fixtures for the `release.yml` D-10 patch — its correctness is verified by the rc.1 tag push itself (post-merge UAT, runbook). The change is mechanical (5 line edits) and well-typed by metadata-action's documented schema.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `:latest` digest unchanged after rc.1 push | OPS-07 / D-10 | Requires real GHCR registry state post-publish; cannot mock | After rc.1 tag pushed and release.yml workflow green: `docker manifest inspect ghcr.io/simplicityguy/cronduit:latest` and confirm digest === pre-existing `v1.0.1` digest |
| `:rc` rolling tag points at rc.1 | D-10 | Same as above | `docker manifest inspect ghcr.io/simplicityguy/cronduit:rc` digest === `v1.1.0-rc.1` digest |
| Multi-arch (amd64 + arm64) on `:v1.1.0-rc.1` | OPS-07 / D-11 | Requires real registry state | `docker manifest inspect -v ghcr.io/simplicityguy/cronduit:v1.1.0-rc.1` lists both platforms |
| GitHub Release body matches `git-cliff` preview | D-12 | Requires real GitHub API state post-publish | Pre-tag: `git cliff --unreleased -o /tmp/preview.md`. Post-publish: diff against `gh release view v1.1.0-rc.1 --json body --jq .body` |
| Annotated + signed (if GPG) tag | D-11 / D-13 | Maintainer-side action; requires local signing key state | Pre-cut: `git config --get user.signingkey` (pre-flight; runbook branches if absent). Post-cut: `git tag -v v1.1.0-rc.1` (verifies signature; falls back to `git cat-file tag v1.1.0-rc.1` for unsigned annotated form) |
| Operator confirms rc.1 image runs healthy in their compose stack | OPS-07 (post-publish UAT) | Real-world deploy; per `feedback_uat_user_validates.md` user must run + confirm | Maintainer (or designated UAT operator) pulls `:v1.1.0-rc.1`, runs against shipped `examples/docker-compose.yml`, confirms `docker compose ps` shows `healthy` |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify (compose-smoke covers all OPS-07/08 tasks; unit tests cover all OPS-06 tasks; only D-10/D-11/D-12/D-13 maintainer/runbook tasks are manual — those are inherently post-publish)
- [ ] Wave 0 covers all MISSING references (5 NEW files identified above)
- [ ] No watch-mode flags (`cargo test`, not `cargo watch`)
- [ ] Feedback latency: <2s for unit tests, <6min for compose-smoke (acceptable for PR check)
- [ ] `nyquist_compliant: true` set in frontmatter (after planner attaches task IDs and confirms each row has either an automated command or a documented Manual-Only justification)

**Approval:** pending
