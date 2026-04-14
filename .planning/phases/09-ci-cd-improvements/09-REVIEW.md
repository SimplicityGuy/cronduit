---
phase: 09-ci-cd-improvements
reviewed: 2026-04-13T00:00:00Z
depth: standard
files_reviewed: 8
files_reviewed_list:
  - .github/workflows/cleanup-cache.yml
  - .github/workflows/cleanup-images.yml
  - .github/workflows/ci.yml
  - .github/workflows/release.yml
  - scripts/update-project.sh
  - justfile
  - .gitignore
  - docs/CI_CACHING.md
findings:
  critical: 0
  warning: 4
  info: 4
  total: 8
status: issues_found
---

# Phase 9: Code Review Report

**Reviewed:** 2026-04-13
**Depth:** standard
**Files Reviewed:** 8
**Status:** issues_found

## Summary

Phase 9 ships four artifacts: `cleanup-cache.yml` (PR cache cleanup), `cleanup-images.yml` (monthly GHCR pruning), `scripts/update-project.sh` (dependency updater), and caching audit documentation in `docs/CI_CACHING.md`. The two new workflow files are well-structured and correctly implement least-privilege permissions, timeouts, and concurrency controls.

The main concerns are:

1. **FOUND-12/D-10 invariant violations** in `ci.yml`: several `run:` steps in the `compose-smoke` job run raw `docker`, `curl`, and `sed` commands instead of delegating to `just` recipes. The `test` job's pre-pull step has the same problem. The invariant is documented in the `ci.yml` header as absolute ("exclusively"), and Phase 9 carries the explicit instruction to preserve it.

2. **Floating SHA tags on third-party actions** in `release.yml`: `orhun/git-cliff-action@v4` and `softprops/action-gh-release@v2` are not pinned by commit SHA. The Phase 9 CONTEXT.md cross-cutting decision states "every new third-party action MUST be pinned by full commit SHA." These actions existed before Phase 9 but the caching audit was the opportunity to address them.

3. **`generate_summary` function in `scripts/update-project.sh`** uses `HEAD@{1}` which is a reflog reference that will fail or produce wrong output on a freshly-created feature branch.

4. Minor consistency issue: `docs/CI_CACHING.md` documents the Tailwind binary as not cached by any CI workflow step, but the script's `update_tailwind_version` function can re-download the binary and commit it — if a CI step ever runs `just tailwind`, the doc must be updated, and the current wording is slightly misleading.

---

## Warnings

### WR-01: FOUND-12/D-10 violated — `compose-smoke` job uses raw inline commands in `run:` steps

**File:** `.github/workflows/ci.yml:162-226`

**Issue:** The `ci.yml` header explicitly states: *"Every `run:` step invokes `just <recipe>` exclusively (D-10 / FOUND-12). No inline `cargo` / `docker` / `rustup` / `sqlx` / `npm` / `npx` commands."* The `compose-smoke` job has seven `run:` steps that violate this invariant directly:

- Line 163: `run: |` block with `sed -i` and `grep` inline (rewrite compose image)
- Line 177: `run: docker compose -f docker-compose.yml up -d` (raw docker compose)
- Line 183: `run: |` block with `curl` and `sleep` loop
- Line 197: `run: |` block with `curl` and `grep`
- Line 206: `run: |` block with `curl` and `grep`
- Line 221: `run: docker compose -f examples/docker-compose.yml logs`
- Line 225: `run: docker compose -f docker-compose.yml down -v`

Phase 9 CONTEXT.md instructs that the caching audit must preserve the FOUND-12/D-10 invariant. It was also the natural opportunity to extract these steps into `just` recipes. The invariant is now documented as broken by this job, yet `docs/CI_CACHING.md` does not note it as an exception.

**Fix:** Extract the `compose-smoke` steps into `justfile` recipes that can be invoked from CI. For example:

```yaml
# In ci.yml compose-smoke job, replace raw steps with:
- name: Rewrite compose to use locally-built cronduit:ci image
  run: just compose-rewrite-ci-image

- name: Start compose stack
  working-directory: examples
  run: just compose-up

- name: Wait for /health and assert
  run: just compose-smoke-assert

- name: Dump compose logs on failure
  if: failure()
  run: just compose-logs

- name: Tear down compose stack
  if: always()
  working-directory: examples
  run: just compose-down
```

And in `justfile`, define each recipe with the current inline logic. If extracting the full smoke-test logic is deferred, at minimum add a comment in `ci.yml` explicitly noting the `compose-smoke` job as a documented FOUND-12 exception (similar to the PR-path image cache gap documented in `docs/CI_CACHING.md`), and update `docs/CI_CACHING.md` accordingly.

---

### WR-02: FOUND-12/D-10 violated — `test` job pre-pull step uses raw `docker` commands

**File:** `.github/workflows/ci.yml:80-88`

**Issue:** The "Pre-pull testcontainers images via mirror.gcr.io" step (lines 80-88) runs a `run: |` block with raw `docker pull` and `docker tag` commands. This violates the same FOUND-12/D-10 invariant.

```yaml
- name: Pre-pull testcontainers images via mirror.gcr.io
  run: |
    set -euo pipefail
    for image in postgres:11-alpine alpine:latest; do
      echo "::group::Pre-pull ${image}"
      docker pull "mirror.gcr.io/library/${image}"
      docker tag "mirror.gcr.io/library/${image}" "${image}"
      echo "::endgroup::"
    done
```

**Fix:** Add a `just` recipe in `justfile` and delegate from CI:

```just
# Prefetch testcontainers images via mirror.gcr.io to avoid Docker Hub rate limits.
prepull-test-images:
    #!/usr/bin/env bash
    set -euo pipefail
    for image in postgres:11-alpine alpine:latest; do
        echo "::group::Pre-pull ${image}"
        docker pull "mirror.gcr.io/library/${image}"
        docker tag "mirror.gcr.io/library/${image}" "${image}"
        echo "::endgroup::"
    done
```

Then in `ci.yml`:
```yaml
- name: Pre-pull testcontainers images via mirror.gcr.io
  run: just prepull-test-images
```

---

### WR-03: Third-party actions in `release.yml` use floating tags instead of SHA pins

**File:** `.github/workflows/release.yml:46,85`

**Issue:** Two third-party actions in `release.yml` are referenced by floating semver tags:

- Line 46: `uses: orhun/git-cliff-action@v4`
- Line 85: `uses: softprops/action-gh-release@v2`

The Phase 9 CONTEXT.md cross-cutting decision states: *"Every new third-party action MUST be pinned by full commit SHA, with a `# vX.Y.Z` trailing comment. This matches discogsography's hygiene and is the actionlint-recommended pattern."*

These actions are not new in Phase 9, but the caching/workflow audit was the intended opportunity to bring `release.yml` into compliance. Floating tags are a supply-chain risk: a malicious or accidental tag override on the upstream repo can inject arbitrary code into the release pipeline, which has `contents: write` and `packages: write` permissions.

**Fix:** Resolve the current commit SHA for each action and pin them:

```bash
gh api repos/orhun/git-cliff-action/releases/latest --jq .tag_name
gh api repos/orhun/git-cliff-action/git/refs/tags/v4 --jq .object.sha

gh api repos/softprops/action-gh-release/releases/latest --jq .tag_name
gh api repos/softprops/action-gh-release/git/refs/tags/v2 --jq .object.sha
```

Then update `release.yml`:
```yaml
# Before:
uses: orhun/git-cliff-action@v4
# After (example — verify SHA before committing):
uses: orhun/git-cliff-action@<40-char-sha> # v4.x.y

# Before:
uses: softprops/action-gh-release@v2
# After:
uses: softprops/action-gh-release@<40-char-sha> # v2.x.y
```

Also add these actions to the `update_gha_pins` scan in `scripts/update-project.sh` by pinning them in place — the script already handles SHA-pinned entries, but floating-tag entries are explicitly skipped by the script's grep pattern.

---

### WR-04: `generate_summary` uses `HEAD@{1}` reflog reference — fails on freshly-created branches

**File:** `scripts/update-project.sh:582`

**Issue:** The summary function attempts to count commits added in the current session:

```bash
git --no-pager log --oneline "HEAD~$(git rev-list --count HEAD ^HEAD@{1} 2>/dev/null || echo 1)..HEAD" 2>/dev/null || \
    git --no-pager log --oneline -10
```

`HEAD@{1}` is a reflog reference meaning "where HEAD was before the most recent movement." On a freshly-created feature branch (which is the common case — the script creates `chore/update-deps-<TS>` from the current branch), `HEAD@{1}` may refer to the HEAD of the parent branch, not to the start of the update session. More critically, `git rev-list --count HEAD ^HEAD@{1}` will count commits from the branch point, not just the commits the script made. This produces an inflated count, then `HEAD~<big_number>` can silently error or show commits predating the update session.

The fallback `|| git --no-pager log --oneline -10` is a reasonable backstop, but the primary path can produce misleading output (showing the wrong commits) without printing an error.

**Fix:** Track the starting commit SHA explicitly before any commits are made, and use it in the summary:

```bash
# Near the top of main(), after safety checks:
STARTING_SHA=$(git rev-parse HEAD)

# In generate_summary():
git --no-pager log --oneline "${STARTING_SHA}..HEAD" 2>/dev/null || \
    git --no-pager log --oneline -10
```

---

## Info

### IN-01: `cleanup-cache.yml` — `contents: read` not explicit at job level (not a bug, but worth noting)

**File:** `.github/workflows/cleanup-cache.yml:32-33`

**Issue:** The job-level `permissions:` block only specifies `actions: write`:

```yaml
permissions:
  actions: write
```

The top-level `permissions: { contents: read }` correctly covers the gap: when a job-level block is present, GitHub applies only the job-level permissions (plus any elevated from top-level for unlisted scopes — actually GitHub merges them as "job permissions override top-level for scopes present in the job block, and inherit top-level for absent scopes"). The practical effect is correct. However, for clarity and self-documentation, consider adding `contents: read` explicitly to the job-level block to make the full permission surface visible without cross-referencing the top-level block.

**Fix:**
```yaml
permissions:
  actions: write
  contents: read
```

---

### IN-02: `docs/CI_CACHING.md` — Tailwind cache "not cached" note may become stale

**File:** `docs/CI_CACHING.md:88-89`

**Issue:** The document states: *"Cronduit's Docker build does not run `just tailwind` inside the Dockerfile — the `assets/static/app.css` file is expected to be already generated… No CI workflow step currently invokes `just tailwind` either, so there is nothing to cache."*

If a future CI step adds `- run: just tailwind`, the doc must be updated. This is a documentation maintenance note, not a bug. The current state is accurately described.

**Fix:** Add a forward-looking note to signal what key to use if the situation changes, which the doc already partially does ("If a future workflow adds `- run: just tailwind`, add an `actions/cache@v4` step..."). No change strictly required.

---

### IN-03: `scripts/update-project.sh` — `ls .github/workflows/*.yml` pattern is correct but unusual

**File:** `scripts/update-project.sh:391`

**Issue:** The glob check `if ! ls .github/workflows/*.yml >/dev/null 2>&1` relies on `ls` failing when the glob expands to nothing (bash with default `nullglob=false` passes the literal `*.yml` to `ls`, which errors). This works but is non-obvious to readers expecting explicit glob testing. On systems where `nullglob` is set in the user's profile, the glob would expand to nothing and `ls` would succeed (exit 0 with no output), causing the early-return branch to be skipped incorrectly.

**Fix:** Use an explicit glob test:
```bash
if ! compgen -G '.github/workflows/*.yml' >/dev/null 2>&1; then
    print_warning "No workflow files found — skipping"
    return
fi
```
Or use a conditional array fill:
```bash
local -a wf_files=(.github/workflows/*.yml)
if [[ ! -e "${wf_files[0]}" ]]; then
    print_warning "No workflow files found — skipping"
    return
fi
```

---

### IN-04: `justfile` — `ci` recipe includes `image` but not `compose-smoke`

**File:** `justfile:18`

**Issue:** The `ci` recipe (the "local `just ci` must predict CI exit code" entry point) is:

```just
ci: fmt-check clippy openssl-check nextest schema-diff image
```

The `compose-smoke` job in `ci.yml` is a full CI gate that exercises the quickstart Docker Compose file. Running `just ci` locally will not exercise the compose smoke test, so a breaking change to `examples/docker-compose.yml` or `examples/cronduit.toml` could pass `just ci` locally but fail CI. This is a semantic gap between `just ci` and the actual GitHub Actions CI.

**Fix:** If the compose smoke test can reasonably run locally (it only requires Docker), add a recipe and include it:
```just
compose-smoke: image
    # ... smoke test logic ...

ci: fmt-check clippy openssl-check nextest schema-diff image compose-smoke
```
If the smoke test is intentionally excluded from local `just ci` (e.g., too slow), add a comment to the `ci` recipe explaining the gap.

---

_Reviewed: 2026-04-13_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
