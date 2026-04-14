---
phase: 09-ci-cd-improvements
fixed_at: 2026-04-13T00:00:00Z
review_path: .planning/phases/09-ci-cd-improvements/09-REVIEW.md
iteration: 1
findings_in_scope: 4
fixed: 2
skipped: 2
status: partial
---

# Phase 9: Code Review Fix Report

**Fixed at:** 2026-04-13
**Source review:** .planning/phases/09-ci-cd-improvements/09-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 4 (WR-01, WR-02, WR-03, WR-04)
- Fixed: 2 (WR-03, WR-04)
- Skipped: 2 (WR-01, WR-02)

## Fixed Issues

### WR-03: Third-party actions in `release.yml` use floating tags instead of SHA pins

**Files modified:** `.github/workflows/release.yml`
**Commit:** 6a1a8b2
**Applied fix:** Resolved current commit SHAs for both floating-tag actions via GitHub API and pinned them with version comments:
- `orhun/git-cliff-action@v4` → `@c93ef52f3d0ddcdcc9bd5447d98d458a11cd4f72 # v4.7.1`
- `softprops/action-gh-release@v2` → `@3bb12739c298aeb8a4eeaf626c5b8d85266b0e65 # v2.6.2`

Both tag refs were confirmed as direct commit SHAs (not annotated tag objects requiring dereferencing). This closes the supply-chain risk on the release pipeline which holds `contents: write` and `packages: write` permissions.

### WR-04: `generate_summary` uses `HEAD@{1}` reflog reference — fails on freshly-created branches

**Files modified:** `scripts/update-project.sh`
**Commit:** 22ce0a8
**Applied fix:** Two-part change:
1. Added `STARTING_SHA=$(git rev-parse HEAD)` at the top of `main()`, after the mode/config print block and before any update functions run. Comment explains why it must be captured before commits.
2. Replaced the fragile `"HEAD~$(git rev-list --count HEAD ^HEAD@{1} 2>/dev/null || echo 1)..HEAD"` range with the stable `"${STARTING_SHA}..HEAD"` range in `generate_summary()`.

The new approach correctly shows only the commits made by the script in the current invocation, regardless of reflog state on freshly-created branches. The fallback `|| git --no-pager log --oneline -10` is retained as a backstop.

## Skipped Issues

### WR-01: FOUND-12/D-10 violated — `compose-smoke` job uses raw inline commands in `run:` steps

**File:** `.github/workflows/ci.yml:162-226`
**Reason:** Out of Phase 9 scope — pre-existing violation predating Phase 9 commit d7034fc. Phase 9 only added `timeout-minutes` and `permissions` to the `compose-smoke` job; it did not introduce the raw `docker`, `curl`, and `sed` `run:` steps. Migrating the seven compose-smoke steps to `just` recipes is a substantive refactor affecting both `ci.yml` and `justfile`, and would require smoke-testing the extracted recipes end-to-end. This work belongs in a dedicated follow-up phase rather than as a patch on the Phase 9 review cycle.

**Original issue:** Seven `run:` steps in the `compose-smoke` job use raw `docker compose`, `curl`, and `sed` commands instead of delegating to `just` recipes, violating the FOUND-12/D-10 invariant documented in the `ci.yml` header.

### WR-02: FOUND-12/D-10 violated — `test` job pre-pull step uses raw `docker` commands

**File:** `.github/workflows/ci.yml:80-88`
**Reason:** Out of Phase 9 scope — pre-existing violation predating Phase 9 commit d7034fc. Phase 9 only added `timeout-minutes` and `permissions` to the `test` job; it did not introduce the raw `docker pull` / `docker tag` pre-pull step. Extracting a `just prepull-test-images` recipe would change `ci.yml` and `justfile` in a way that warrants its own phase task with local validation. Bundling it into this fix cycle would risk breaking the integration test matrix without the time to verify it.

**Original issue:** The "Pre-pull testcontainers images via mirror.gcr.io" step runs raw `docker pull` and `docker tag` commands in a `run: |` block, violating the FOUND-12/D-10 invariant.

---

_Fixed: 2026-04-13_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
