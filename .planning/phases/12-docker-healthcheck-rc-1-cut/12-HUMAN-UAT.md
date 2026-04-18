---
status: partial
phase: 12-docker-healthcheck-rc-1-cut
source: [12-VERIFICATION.md]
started: 2026-04-18T03:08:17Z
updated: 2026-04-18T03:08:17Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. Maintainer cuts v1.1.0-rc.1 tag locally per docs/release-rc.md AFTER the Phase 12 PR merges to main
expected: Annotated (and signed if GPG configured) tag `v1.1.0-rc.1` pushed to origin; `release.yml` workflow runs green on the pushed tag.
why_human: Per Phase 12 D-13 the tag is the trust anchor — cut by the maintainer's signing key, explicitly NOT by `workflow_dispatch`. Per `feedback_uat_user_validates.md` Claude does not assert UAT pass.
result: [pending]

### 2. Post-push GHCR tag verification after rc.1 tag is pushed
expected: `docker manifest inspect` shows `:1.1.0-rc.1` + `:rc` present and multi-arch (amd64+arm64); `:latest` digest unchanged from v1.0.1; `:1` and `:1.1` digests unchanged; `gh release view` reports `isPrerelease=true`; release body matches `git-cliff --unreleased` preview.
why_human: Requires live GHCR registry state post-publish; cannot be programmatically asserted from the local repo. Per `feedback_uat_user_validates.md`, operator confirms each row in the runbook post-push verification table.
result: [pending]

### 3. compose-smoke GitHub Actions workflow runs green on the Phase 12 PR
expected: The `compose-smoke / compose-smoke` GHA check reports a green status on the feature-branch PR, exercising shipped-compose healthy-by-default, compose-override wins, and OPS-08 before/after assertions on ubuntu-latest.
why_human: Requires GitHub Actions runner execution (docker daemon + buildx + compose CLI) — confirmable only after the branch pushes and the PR is opened. The workflow file itself is verified present, well-formed, and YAML-valid locally.
result: [pending]

## Summary

total: 3
passed: 0
issues: 0
pending: 3
skipped: 0
blocked: 0

## Gaps
