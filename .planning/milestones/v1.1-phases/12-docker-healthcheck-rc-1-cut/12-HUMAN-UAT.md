---
status: complete
phase: 12-docker-healthcheck-rc-1-cut
source: [12-VERIFICATION.md]
started: 2026-04-18T03:08:17Z
updated: 2026-04-19T22:30:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Maintainer cuts v1.1.0-rc.1 tag locally per docs/release-rc.md AFTER the Phase 12 PR merges to main
expected: Annotated (and signed if GPG configured) tag `v1.1.0-rc.1` pushed to origin; `release.yml` workflow runs green on the pushed tag.
why_human: Per Phase 12 D-13 the tag is the trust anchor — cut by the maintainer's signing key, explicitly NOT by `workflow_dispatch`. Per `feedback_uat_user_validates.md` Claude does not assert UAT pass.
result: pass
evidence: |
  Tag created locally + pushed 2026-04-19. Annotated, unsigned (no GPG configured —
  annotated-fallback path per runbook). Tag points at 1b39ac0 (main HEAD; includes
  Phase 12 + cliff.toml fix). release.yml run 24639622925 → completed success.
  Tag visible on origin: 52130046b0c73e7ceeed0e5d7c307b4c2cb50524 refs/tags/v1.1.0-rc.1

### 2. Post-push GHCR tag verification after rc.1 tag is pushed
expected: `docker manifest inspect` shows `:1.1.0-rc.1` + `:rc` present and multi-arch (amd64+arm64); `:latest` digest unchanged from v1.0.1; `:1` and `:1.1` digests unchanged; `gh release view` reports `isPrerelease=true`; release body matches `git-cliff --unreleased` preview.
why_human: Requires live GHCR registry state post-publish; cannot be programmatically asserted from the local repo. Per `feedback_uat_user_validates.md`, operator confirms each row in the runbook post-push verification table.
result: pass
evidence: |
  All 6 sub-checks passed against post-push GHCR state:
  - :1.1.0-rc.1 multi-arch (linux/amd64 + linux/arm64 + 2 attestation entries) ✓
  - :rc digest IDENTICAL to :1.1.0-rc.1 (sha256:8839352b43fbe…) ✓
  - release.yml metadata-action log shows :latest, :1, :1.1 all gated to enable=false (D-10 held) ✓
  - gh release view v1.1.0-rc.1 → isPrerelease=true ✓
  - Release body matches git-cliff --unreleased preview (only diff is changelog header strip)
  Caveat (NOT counted as a phase 12 issue, predates phase 12): :latest digest (d45549ab…)
  differs from :1.0.1 digest (dbc60b39…). :1, :1.0, :1.0.1 all agree on the v1.0.1 retag
  digest. The v1.0.1 retag never propagated to :latest. Worth a separate cleanup before
  the final v1.1.0 tag (which will overwrite :latest naturally).

### 3. compose-smoke GitHub Actions workflow runs green on the Phase 12 PR
expected: The `compose-smoke / compose-smoke` GHA check reports a green status on the feature-branch PR, exercising shipped-compose healthy-by-default, compose-override wins, and OPS-08 before/after assertions on ubuntu-latest.
why_human: Requires GitHub Actions runner execution (docker daemon + buildx + compose CLI) — confirmable only after the branch pushes and the PR is opened. The workflow file itself is verified present, well-formed, and YAML-valid locally.
result: pass
evidence: |
  - PR #29 compose-smoke run 24613345378 → completed success
  - PR #30 (cliff.toml fix) compose-smoke run also completed success — confirms fix didn't regress
  - All 3 assertions exercised on ubuntu-latest:
    1. Shipped-compose smoke (examples/docker-compose.yml) reached healthy within 90s
    2. Compose-override smoke (tests/compose-override.yml) — CMD-SHELL form won
    3. OPS-08 before/after — OLD wget HEALTHCHECK exercised; NEW cronduit health reached healthy
  - The MD-01 DATABASE_URL fix held up under real GHA runner conditions

## Summary

total: 3
passed: 3
issues: 0
pending: 0
skipped: 0
blocked: 0

## Gaps
