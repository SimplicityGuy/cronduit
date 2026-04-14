---
phase: 9
slug: ci-cd-improvements
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-14
---

# Phase 9 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
>
> **Reconstructed retroactively 2026-04-14** from Phase 9's PLAN/SUMMARY/VERIFICATION artifacts. Phase 9 was added to the roadmap 2026-04-13 as an operational-hygiene phase with `Requirements: TBD (will be enumerated at plan time)` — no v1 REQ-IDs were ever assigned. It is a DevOps/CI phase with no runtime code changes: all deliverables are `.github/workflows/*.yml`, `scripts/*`, and `docs/CI_CACHING.md`.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `actionlint` (YAML workflow syntax); `shellcheck` (shell script syntax); GitHub Actions live runners (workflow-execution verification); `scripts/update-project.sh --dry-run` (self-verifying) |
| **Config file** | `.github/workflows/ci.yml` (hosts the actionlint step); `.shellcheckrc` if present |
| **Quick run command** | `actionlint .github/workflows/cleanup-cache.yml .github/workflows/cleanup-images.yml` · `shellcheck scripts/update-project.sh` · `scripts/update-project.sh --dry-run` |
| **Full suite command** | Push to a feature branch → CI runs `ci.yml` with rust-cache / buildx GHA cache / compose-smoke; live `pull_request: closed` triggers `cleanup-cache.yml`; manual `gh workflow run cleanup-images.yml` |
| **Estimated runtime** | ~10s for local static analysis; ~5-15 min for CI observation; live workflow execution tests require a real PR |

---

## Sampling Rate

- **After every task commit:** Run `actionlint` on touched workflow files + `shellcheck` on touched scripts
- **After every plan wave:** Run `scripts/update-project.sh --dry-run` + `shellcheck scripts/update-project.sh`
- **Before `/gsd-verify-work`:** All PR CI checks green + feature-branch push exercises the new caching lanes
- **Max feedback latency:** ~10s for local static; CI observation is ~15 min

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 9-01-01 | 01 | 1 | CI-01 (cleanup-cache workflow) | — | PR-scoped concurrency group; `actions: write` permission; `set +e` delete loop tolerates missing caches | workflow-static + live-trigger | `actionlint .github/workflows/cleanup-cache.yml` + live PR open→close observation | ✅ `.github/workflows/cleanup-cache.yml` | ✅ green (static) / ⚠️ live-observation deferred to human UAT |
| 9-02-01 | 02 | 1 | CI-02 (cleanup-images workflow) | — | `packages: write` permission; `dataaxiom/ghcr-cleanup-action` pinned by SHA; `keep-n-tagged` + `older-than` retention | workflow-static + manual-dispatch | `actionlint .github/workflows/cleanup-images.yml` + `gh workflow run cleanup-images.yml` against live GHCR | ✅ `.github/workflows/cleanup-images.yml` | ✅ green (static) / ⚠️ live-observation deferred to human UAT |
| 9-03-01 | 03 | 1 | CI-03 (update-project.sh) | T-9-01 (refuses to run outside project root) | `--dry-run` exits 0 without touching tree; refuses outside project root; lands commits on a fresh feature branch (never `main`) | shell-static + self-verifying | `shellcheck scripts/update-project.sh` + `scripts/update-project.sh --dry-run` (should exit 0 + print planned updates without modifying anything) | ✅ `scripts/update-project.sh` | ✅ green |
| 9-04-01 | 04 | 1 | CI-04 (caching audit) | — | Every cargo job uses `Swatinem/rust-cache@v2`; every docker-build job uses per-arch GHA cache scopes; Tailwind binary cached | workflow-static + CI-observation | `actionlint .github/workflows/ci.yml` + second-push cache-restore log observation | ✅ `.github/workflows/ci.yml`, `docs/CI_CACHING.md` | ✅ green (static) / ⚠️ cache-restore log observation deferred to human UAT |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky/unresolved*

*Note: requirement IDs `CI-01..CI-04` are synthesized retroactively for this validation doc; they are NOT present in `.planning/REQUIREMENTS.md` because Phase 9 was added after the v1 requirement set was locked and intentionally carries no v1 REQ-IDs. See `.planning/v1.0-MILESTONE-AUDIT.md` tech-debt section for the mapping decision (backfill vs "n/a — operational hygiene phase").*

---

## Wave 0 Requirements

- [x] `actionlint` — GitHub Actions YAML linter (already wired into `ci.yml` as a CI step; can be invoked locally if installed)
- [x] `shellcheck` — shell script linter (standard local-install tool; CI job invokes it on touched scripts)
- [x] `gh` CLI — GitHub CLI (already used throughout the repo for PR / workflow / cache operations)
- [x] No new test framework or fixtures needed — this is a DevOps phase with no Rust test surface

*Existing tooling covers Phase 9 validation.*

---

## Manual-Only Verifications

Per `09-VERIFICATION.md` and `09-HUMAN-UAT.md`, three workflow-execution observations require live GitHub Actions runners — they cannot be reproduced in static analysis or on a developer laptop:

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `cleanup-cache.yml` fires on `pull_request: closed` and deletes the PR's caches | CI-01 | Requires a real GitHub `pull_request: closed` event — no local simulation | Open a draft PR against main, close it, then run `gh run list --workflow=cleanup-cache.yml` — confirm a run appears, log shows `Fetching list of cache keys for refs/pull/<N>/merge`, run exits 0 |
| `cleanup-images.yml` prunes real GHCR image revisions | CI-02 | Requires a live GHCR package with published images | `gh workflow run cleanup-images.yml` against the real `ghcr.io/<owner>/cronduit` package, then `gh run list --workflow=cleanup-images.yml` — confirm run succeeds, log shows retention policy summary (`keep-n-tagged:2`, `older-than:30days`) applied |
| `Swatinem/rust-cache@v2` cache restores on second PR push | CI-04 | Cache hit/miss behavior only observable on GitHub-hosted runners with live GHA cache infrastructure | Open a PR, let CI run to completion, push a second commit, confirm CI stays green and rust-cache step logs `Cache restored from key:...` on lint and test jobs; compose-smoke logs `importing cache manifest` from `cronduit-ci-smoke` scope |

These items are tracked in `09-HUMAN-UAT.md` as `result: pending` — they will flip to `pass` as the project accumulates normal PR activity. They do not block v1.0 archive.

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or are listed in the manual-only table above
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No `cargo watch` / watch-mode flags in plans
- [x] Feedback latency < 60s for local static analysis
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved (retroactive audit 2026-04-14 — reconstructed from SUMMARY artifacts per State B of gsd-validate-phase)

---

## Validation Audit 2026-04-14

| Metric | Count |
|--------|-------|
| Gaps found | 1 (no VALIDATION.md existed for this phase — State B reconstruction) |
| Resolved | 1 (document created from PLAN / SUMMARY / VERIFICATION artifacts) |
| Escalated | 0 |

**Audit method:** Phase 9 was added to the roadmap 2026-04-13 after the v1 requirement set was locked. It shipped 2026-04-13 with `09-VERIFICATION.md` status `human_needed` at score 8/9, where the "human_needed" items are three live GitHub Actions runner observations that fundamentally cannot be reproduced in static analysis (see Manual-Only Verifications above). This VALIDATION.md was missing entirely — reconstructed from `09-0[1-4]-PLAN.md`, `09-0[1-4]-SUMMARY.md`, `09-VERIFICATION.md`, and the current `.github/workflows/` + `scripts/` + `docs/CI_CACHING.md` on disk.

**Key evidence:**
- `.github/workflows/cleanup-cache.yml` — PR-scoped concurrency group, `actions: write` permission, tolerant delete loop
- `.github/workflows/cleanup-images.yml` — monthly schedule + workflow_dispatch, `packages: write`, SHA-pinned `dataaxiom/ghcr-cleanup-action`
- `scripts/update-project.sh` — executable, supports `--dry-run`, `--major`, `--skip-tests`, `--no-backup`, `--help`; refuses to run outside project root; lands on a feature branch (never main)
- `.github/workflows/ci.yml` — `Swatinem/rust-cache@v2` on every cargo job; per-arch `type=gha,scope=...` for Docker buildx; Tailwind binary cache step
- `docs/CI_CACHING.md` — documents every cache and its eviction behavior (added in Plan 04)
- `.planning/phases/09-ci-cd-improvements/09-VERIFICATION.md` — `human_needed` with 3 explicitly-documented live-runner items

**Requirements note:** Phase 9 has no v1 REQ-IDs. The synthetic `CI-01..CI-04` identifiers used in the Per-Task Verification Map above are local to this validation doc and are not present in `.planning/REQUIREMENTS.md`. This is flagged in `.planning/v1.0-MILESTONE-AUDIT.md` tech-debt section as a bookkeeping decision: either backfill `CI-*` as a new requirement category or explicitly mark Phase 9 as "n/a — operational hygiene phase" in the traceability table.
