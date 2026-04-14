---
phase: 09-ci-cd-improvements
verified: 2026-04-13T00:00:00Z
status: human_needed
score: 8/9 must-haves verified
overrides_applied: 0
re_verification: null
human_verification:
  - test: "Open a draft PR against main, then close it. Run: gh run list --workflow=cleanup-cache.yml"
    expected: "Workflow run appears in the list; run log shows 'Fetching list of cache keys for refs/pull/<N>/merge'; run exits 0 even if zero caches are found."
    why_human: "cleanup-cache.yml triggers on pull_request:closed — cannot simulate without a live GitHub event."
  - test: "gh workflow run cleanup-images.yml, then gh run list --workflow=cleanup-images.yml"
    expected: "Run succeeds (exit 0); log shows retention policy summary (keep-n-tagged:2, older-than:30days) applied against ghcr.io/<owner>/cronduit."
    why_human: "Requires a live GHCR package with at least one published image; cleanup-images.yml must be triggered on the real GitHub-hosted runner."
  - test: "Open a PR and let CI run to completion; check second push hits cache restore on lint, test, and compose-smoke jobs."
    expected: "CI stays green; Swatinem/rust-cache@v2 step logs 'Cache restored from key:...' on lint and test jobs; compose-smoke logs 'importing cache manifest' from the cronduit-ci-smoke scope; image job does NOT log a cache restore (expected per FOUND-12 deliberate gap)."
    why_human: "Caching behaviour only observable on GitHub-hosted runners with live GHA cache infrastructure."
---

# Phase 9: CI/CD Improvements Verification Report

**Phase Goal:** Bring Cronduit's CI/CD hygiene up to a level that matches a long-lived OSS project: PR caches stop accumulating, old GHCR image revisions get pruned, dependency upgrades are a one-command operation, and every existing workflow exploits the standard caching lanes (cargo registry/index/target, Docker buildx layers, Tailwind binary). Reference implementations come from the SimplicityGuy/discogsography repo, adapted for Cronduit's Rust-only + single-image shape.

**Verified:** 2026-04-13
**Status:** human_needed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `.github/workflows/cleanup-cache.yml` exists, triggers on `pull_request: closed`, deletes that PR's caches via `gh cache delete`, has `actions: write` permission, and is concurrency-grouped per PR | ✓ VERIFIED | File at `.github/workflows/cleanup-cache.yml` (52 lines). Verified: `on.pull_request.types: [closed]`, `concurrency.group: cleanup-cache-${{ github.event.pull_request.number }}`, job-level `permissions.actions: write`, top-level `permissions.contents: read`, `timeout-minutes: 10`, `set +e` delete loop via `gh cache delete`. |
| 2 | `.github/workflows/cleanup-images.yml` exists, runs on `workflow_dispatch` and a monthly schedule, has `packages: write`, uses SHA-pinned `dataaxiom/ghcr-cleanup-action`, keeps last N tagged images, and deletes old images | ✓ VERIFIED | File at `.github/workflows/cleanup-images.yml` (44 lines). Verified: dual triggers (`workflow_dispatch` + `cron: "0 0 15 * *"`), `packages: write`, action pinned to `cd0cdb900b5dbf3a6f2cc869f0dbb0b8211f50c4 # v1.0.16`, `keep-n-tagged: 2`, `older-than: 30 days`, `delete-untagged: true`, `delete-partial-images: true`, `package: cronduit`. No matrix. |
| 3 | `scripts/update-project.sh` exists, is executable, supports `--dry-run / --major / --skip-tests / --no-backup / --help`, refuses to run outside the project root, delegates to `just` recipes, creates a feature branch, never commits to `main` | ✓ VERIFIED | File at `scripts/update-project.sh` (613 lines, executable). `bash -n` passes. `--help` lists all 5 flags and no `--python`. Project-root guard checks `Cargo.toml + justfile + Dockerfile + (src/main.rs or crates/)`. Main-branch refusal confirmed in source (lines 114–116). Delegates `just update-cargo`, `just openssl-check`, `just update-hooks`. Ecosystem order: cargo → dockerfile → tailwind → gha-pins → pre-commit → tests → summary. |
| 4 | Every cargo-running job in `ci.yml` uses `Swatinem/rust-cache@v2` | ✓ VERIFIED | Two occurrences in `ci.yml`: one in the `lint` job (line 33) and one in the `test` matrix job (line 63–65, keyed by `${{ matrix.arch }}`). Both cargo-running jobs are covered. |
| 5 | Every `docker/build-push-action` step in `ci.yml` and `release.yml` has unique-scoped `cache-from` + `cache-to` | ✓ VERIFIED | `ci.yml` `compose-smoke`: `cache-from: type=gha,scope=cronduit-ci-smoke` and `cache-to: type=gha,mode=max,scope=cronduit-ci-smoke`. `release.yml` `release`: `cache-from: type=gha,scope=cronduit-release` and `cache-to: type=gha,mode=max,scope=cronduit-release`. The PR-path `image` job has no GHA cache — this is a documented deliberate gap (FOUND-12 / D-10 invariant preservation). |
| 6 | Every new or edited workflow sets `permissions` and `timeout-minutes` on every job | ✓ VERIFIED | `ci.yml`: lint `timeout-minutes: 15`, test `timeout-minutes: 30`, image `timeout-minutes: 45`, compose-smoke `timeout-minutes: 20`. All four jobs have job-level `permissions:`. `release.yml`: `timeout-minutes: 60`. `cleanup-cache.yml`: `timeout-minutes: 10`. `cleanup-images.yml`: `timeout-minutes: 30`. |
| 7 | `docs/CI_CACHING.md` exists with cache inventory, mermaid diagram, "Deliberate cache gaps" section, and single-scope multi-arch release rationale | ✓ VERIFIED | File at `docs/CI_CACHING.md` (164 lines). Contains ```` ```mermaid ```` flowchart at line 96. Section `## Deliberate cache gaps` at line 46. Section `### Why one scope for the multi-arch release` at line 34. Cache inventory table present with all 9 rows including the deliberate gap row. |
| 8 | `scripts/update-project.sh --dry-run` exits 0 and makes no file mutations | ✓ VERIFIED | SUMMARY.md dry-run transcript confirms exit 0, "Dry-run complete. No files were modified.", `git status --porcelain` clean after run. The dry-run exercises all 5 update functions and produces correct discovery output (rust:1.94→1.94.1 Dockerfile diff found; Tailwind v3.4.17→v3.4.19 diff found; GHA pin already at latest). |
| 9 | The new workflows (`cleanup-cache.yml`, `cleanup-images.yml`) fire correctly on live GitHub runners and the caching audit report correctly shows cache hits on the second CI push | ? HUMAN NEEDED | These truths are structurally verified (code exists, is wired, is syntactically and semantically correct) but require live GitHub runner execution to confirm actual cache-hit behaviour and event triggering. See Human Verification Required section. |

**Score:** 8/9 truths verified (1 human-needed)

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `.github/workflows/cleanup-cache.yml` | PR cache cleanup on close | ✓ VERIFIED | 52 lines. Contains all required structural elements per 09-01-PLAN.md. Valid YAML confirmed by executor. |
| `.github/workflows/cleanup-images.yml` | Monthly GHCR pruning for ghcr.io/<owner>/cronduit | ✓ VERIFIED | 44 lines. Single flat job, SHA-pinned action. SHA `cd0cdb900b5dbf3a6f2cc869f0dbb0b8211f50c4 # v1.0.16` re-verified by executor against live GitHub API before commit. |
| `scripts/update-project.sh` | One-command dependency updater | ✓ VERIFIED | 613 lines (plan required ≥ 400). Executable. Passes `bash -n`. `--help` exits 0. `--dry-run` exits 0. |
| `justfile` | `update-cargo` recipe (minimum) | ✓ VERIFIED | Both `update-cargo:` and `update-hooks:` recipes present. No existing recipes modified. |
| `.gitignore` | `backups/` exclusion | ✓ VERIFIED | Line 91: `backups/` present under the Cronduit section, with explanatory comment. |
| `docs/CI_CACHING.md` | Authoritative cache topology doc | ✓ VERIFIED | 164 lines (plan required ≥ 60). 1 mermaid diagram. Complete inventory table. Deliberate gaps section. |
| `.github/workflows/ci.yml` | CI workflow with filled caching gaps + timeouts + permissions | ✓ VERIFIED | `timeout-minutes:` on all 4 jobs. Job-level `permissions:` on all 4 jobs. `cache-from`/`cache-to` scopes corrected on compose-smoke. `Swatinem/rust-cache@v2` on lint + test. FOUND-12 / D-10 preserved: `- run: just image` unchanged. |
| `.github/workflows/release.yml` | Release workflow with `cronduit-release` cache scope + `timeout-minutes` | ✓ VERIFIED | `timeout-minutes: 60` added. `cache-from: type=gha,scope=cronduit-release` and `cache-to: type=gha,mode=max,scope=cronduit-release` replacing unscoped `type=gha` entries. |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `pull_request: closed` event | `gh cache delete` | `gh cache list --ref refs/pull/$PR/merge --json id --jq .[].id` loop | ✓ WIRED | `cleanup-cache.yml` step passes `BRANCH: refs/pull/${{ github.event.pull_request.number }}/merge` via `env:` and loops `gh cache delete "$cacheKey"` inside `set +e`. |
| `schedule cron "0 0 15 * *"` OR `workflow_dispatch` | `ghcr.io/<owner>/cronduit` package | `dataaxiom/ghcr-cleanup-action@cd0cdb900b5dbf3a6f2cc869f0dbb0b8211f50c4` with `packages: write` token | ✓ WIRED | `cleanup-images.yml` wires both triggers to the single `cleanup` job with `package: cronduit` and `owner: ${{ github.repository_owner }}`. |
| `scripts/update-project.sh` | `just update-cargo` | `update_cargo_deps()` function | ✓ WIRED | Lines 202–209 call `just update-cargo` (and `just openssl-check` for Pitfall 14 guard). Dry-run output confirms "[DRY RUN] Would run: just update-cargo". |
| `scripts/update-project.sh` | `.github/workflows/*.yml` SHA pins | `gh api repos/<o>/<r>/releases/latest` + `git/refs/tags/<tag>` SHA resolution + sed rewrite | ✓ WIRED | `update_gha_pins()` function (lines ~380–490) scans for `uses: <owner>/<repo>@[a-f0-9]{40} # v...` pattern, resolves current release SHA via API, rewrites in place. Bash 3.2 portability fix applied (parallel simple arrays instead of `local -A`). |
| CI jobs running `just nextest` / `just clippy` | cargo target + `~/.cargo` registry cache | `Swatinem/rust-cache@v2` keyed by arch matrix | ✓ WIRED | `ci.yml` lint job line 33, test job lines 63–65 with `key: ${{ matrix.arch }}`. |
| `docker/build-push-action@v6` (compose-smoke, release) | GHA type=gha scoped caches | `cache-from: type=gha,scope=<name>` + `cache-to: type=gha,mode=max,scope=<name>` | ✓ WIRED | `ci.yml` compose-smoke: `scope=cronduit-ci-smoke`. `release.yml` release: `scope=cronduit-release`. |

---

## Data-Flow Trace (Level 4)

Not applicable. Phase 9 delivers CI/CD workflow files, a shell script, and documentation — no runtime components rendering dynamic data.

---

## Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| `cleanup-cache.yml` triggers ONLY on `pull_request: closed` (not push/schedule) | `grep -Ec '(on push:|schedule:)' .github/workflows/cleanup-cache.yml` | 0 | ✓ PASS |
| `cleanup-images.yml` has no matrix strategy | `grep -c 'strategy:' .github/workflows/cleanup-images.yml` | 0 | ✓ PASS |
| `cleanup-images.yml` action is SHA-pinned with semver comment | `grep -Ec 'dataaxiom/ghcr-cleanup-action@[a-f0-9]{40} # v[0-9]+' .github/workflows/cleanup-images.yml` | 1 | ✓ PASS |
| `scripts/update-project.sh` passes bash syntax check | `bash -n scripts/update-project.sh` | exit 0 | ✓ PASS |
| `scripts/update-project.sh --help` exits 0 | `./scripts/update-project.sh --help` | exit 0, all 5 flags listed | ✓ PASS |
| `ci.yml` FOUND-12 invariant preserved | `grep -nE '^\s*- run:' .github/workflows/ci.yml \| grep -vE 'just '` | Empty (except pre-existing violations — see Anti-Patterns) | ✓ PASS (Phase 9 changes only — pre-existing violations are pre-existing) |
| `ci.yml` `Swatinem/rust-cache@v2` coverage | `grep -c 'Swatinem/rust-cache@v2' .github/workflows/ci.yml` | 2 | ✓ PASS |
| `ci.yml` all jobs have `timeout-minutes:` | `grep -c 'timeout-minutes:' .github/workflows/ci.yml` | 4 | ✓ PASS |
| `release.yml` has `timeout-minutes:` | `grep -c 'timeout-minutes:' .github/workflows/release.yml` | 1 | ✓ PASS |
| `docs/CI_CACHING.md` has mermaid diagram | `grep -c '^\`\`\`mermaid' docs/CI_CACHING.md` | 1 | ✓ PASS |
| `docs/CI_CACHING.md` has deliberate cache gaps section | `grep -c '## Deliberate cache gaps' docs/CI_CACHING.md` | 1 | ✓ PASS |

---

## Requirements Coverage

Requirements for Phase 9 were listed as "TBD (DevOps phase)" in REQUIREMENTS.md. No formal REQ-IDs were assigned. Coverage is assessed against the five ROADMAP Success Criteria directly (see Observable Truths table above).

---

## Anti-Patterns Found

| File | Lines | Pattern | Severity | Impact |
|------|-------|---------|----------|--------|
| `.github/workflows/ci.yml` | 81–88 | Pre-existing: raw `docker pull` + `docker tag` commands in `run:` step ("Pre-pull testcontainers images via mirror.gcr.io") — violates FOUND-12 / D-10 | ⚠️ Warning | FOUND-12 invariant breach is **pre-existing** (confirmed via `git diff d7034fc`). Phase 9 did NOT introduce it; the line was present at commit `d7034fc`. Phase 9 explicitly preserved the `- run: just image` invariant and documented the PR-path cache gap, but WR-02 (the reviewer's finding) highlights that the pre-pull step also violates FOUND-12. Not blocking Phase 9 goal. |
| `.github/workflows/ci.yml` | 162–226 | Pre-existing: raw `sed`, `docker compose`, `curl`, `grep` commands in `compose-smoke` `run:` steps — violates FOUND-12 / D-10 | ⚠️ Warning | Also **pre-existing** (confirmed via `git diff d7034fc`). Phase 9 added `timeout-minutes`, `permissions`, and correct GHA cache scopes to this job. It did NOT introduce the FOUND-12 violations. The reviewer (WR-01) correctly identifies these as pre-existing violations that Phase 9 was an opportunity to fix. |
| `.github/workflows/release.yml` | 46, 85 | Pre-existing: `orhun/git-cliff-action@v4` and `softprops/action-gh-release@v2` use floating semver tags instead of full commit SHAs | ⚠️ Warning | Pre-existing (present at `d7034fc`). Phase 9's caching audit was the logical opportunity to pin these. The reviewer (WR-03) correctly flags this as a supply-chain hygiene gap in a workflow that has `contents: write` and `packages: write`. Not introduced by Phase 9, but not fixed by it either. |
| `scripts/update-project.sh` | ~582 | `HEAD@{1}` reflog reference in `generate_summary()` may produce inflated commit count on freshly-created branches | ℹ️ Info | Fallback `\|\| git --no-pager log --oneline -10` prevents hard failure. Misleading output only (inflated commit list). Does not affect any ecosystem update function. Reviewer WR-04 documents the fix (`STARTING_SHA=$(git rev-parse HEAD)` captured early). |

**Summary:** All four anti-pattern findings are either pre-existing (not introduced by Phase 9) or info-level. Phase 9 explicitly preserved the FOUND-12 invariant for its own changes. The pre-existing violations are correctly documented in `docs/CI_CACHING.md` (the FOUND-12 rationale section) for the deliberate gap case.

---

## Human Verification Required

### 1. `cleanup-cache.yml` fires on PR close

**Test:** Open a draft PR against `main`, push at least one commit so GHA creates a cache entry, then close the PR. Run `gh run list --workflow=cleanup-cache.yml` and inspect the run log.

**Expected:** Workflow run appears; log shows "Fetching list of cache keys for refs/pull/\<N\>/merge"; if no caches exist, run still exits 0 (the `set +e` path).

**Why human:** The `pull_request: closed` trigger fires exclusively on a live GitHub event. Cannot be simulated locally or via grep checks.

---

### 2. `cleanup-images.yml` dispatches and runs successfully

**Test:** `gh workflow run cleanup-images.yml`, then `gh run list --workflow=cleanup-images.yml --limit 5`.

**Expected:** Run appears with exit code 0; log summarises the retention policy applied to `ghcr.io/<owner>/cronduit` (keep-n-tagged: 2, older-than: 30 days). The run must target the real GHCR package (not a dry-run — this action does not have a built-in dry-run flag).

**Why human:** Requires a live GHCR package with at least one published image; requires runner auth to the `ghcr.io/<owner>` namespace. Cannot test locally.

---

### 3. CI cache hit rate on second push

**Test:** Open a PR and let CI run to full completion (first push warms the cache). Push a trivial no-op change (e.g., add a comment to a source file). Observe CI re-run.

**Expected:** `lint` and `test` jobs show "Cache restored from key:..." in the `Swatinem/rust-cache@v2` step; `compose-smoke` job shows Docker buildx cache import from `cronduit-ci-smoke` scope; `image` job does NOT show a cache restore log (expected — FOUND-12 deliberate gap). All jobs green.

**Why human:** GHA cache infrastructure only observable on GitHub-hosted runners. The cache hit rate cannot be confirmed from a local checkout.

---

## Gaps Summary

No gaps blocking phase goal achievement. All eight verifiable truths are VERIFIED. The ninth truth (live workflow execution and cache-hit confirmation) is HUMAN NEEDED by nature — it requires GitHub-hosted runner execution and a live GHCR package.

**Pre-existing FOUND-12 violations in `ci.yml`** (WR-01 and WR-02 in the code review) were present before this phase (`d7034fc` baseline) and are not regressions introduced by Phase 9. Phase 9 correctly preserved the documented `- run: just image` invariant and did not introduce any new FOUND-12 violations. The reviewer's suggestion to extract the `compose-smoke` and `test` pre-pull steps into `just` recipes is valid future work but does not block this phase's goal.

**Floating SHA tags in `release.yml`** (WR-03) are also pre-existing. The `scripts/update-project.sh` `update_gha_pins` function only processes SHA-pinned entries (by design — floating-tag actions are skipped). Pinning `orhun/git-cliff-action` and `softprops/action-gh-release` is valid follow-up work.

---

_Verified: 2026-04-13_
_Verifier: Claude (gsd-verifier)_
