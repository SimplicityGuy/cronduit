# Phase 20 — v1.2.0-rc.1 Pre-Flight Checklist

**Phase:** 20 — Webhook SSRF/HTTPS Posture + Retry/Drain + Metrics — rc.1
**Tag target:** `v1.2.0-rc.1`
**Runbook:** [docs/release-rc.md](../../../docs/release-rc.md) — REUSED VERBATIM from v1.1; Phase 20 makes no edits (D-28 / D-30).
**Cargo.toml version:** `1.2.0` (set by Phase 15; the `-rc.1` is tag-only per `feedback_tag_release_version_match.md`).

> This artifact is a maintainer-validated checklist. Claude authored it; the maintainer runs through it and cuts the tag locally.

## 1. Phase 20 plans merged on `main`

- [ ] PR for Plan 01 (DLQ migration + helpers + Wave 0 stubs) merged.
- [ ] PR for Plan 02 (RetryingDispatcher + classification + DLQ writes) merged.
- [ ] PR for Plan 03 (HTTPS-required validator) merged.
- [ ] PR for Plan 04 (worker drain budget + queue_depth gauge) merged.
- [ ] PR for Plan 05 (metrics family migration) merged.
- [ ] PR for Plan 06 (config field + CLI wiring) merged.
- [ ] PR for Plan 07 (docs/WEBHOOKS.md extension) merged.
- [ ] PR for Plan 08 (UAT recipes + 20-HUMAN-UAT.md) merged.

Verification:
```bash
gh pr list --state merged --search "phase-20" --limit 30
git log --oneline main | head -20
```

## 2. CI matrix green on `main` for the merge commit

- [ ] `linux/amd64 × SQLite` lint+test green
- [ ] `linux/amd64 × Postgres` lint+test green
- [ ] `linux/arm64 × SQLite` lint+test green (cargo-zigbuild)
- [ ] `linux/arm64 × Postgres` lint+test green
- [ ] webhook-interop matrix (Python/Go/Node from P19) green
- [ ] cargo-deny CI job green (non-blocking on rc.1 per D-15 of P15; promotion to blocking is Phase 24)
- [ ] compose-smoke job green (the existing v1.1 healthcheck path still works)

Verification:
```bash
gh run list --branch main --limit 5
gh run view <RUN_ID>   # confirm all matrix legs green
```

## 3. rustls invariant intact

- [ ] `cargo tree -i openssl-sys` returns empty.

Verification:
```bash
cargo tree -i openssl-sys
# EXPECT: error: package ID specification `openssl-sys` did not match any packages
```

## 4. release.yml `:latest` gate logic intact (D-30 — no edits; T-20-06 visual gate)

- [ ] `.github/workflows/release.yml:134` still reads `enable=${{ !contains(github.ref, '-') }}` (the `:latest` skip-on-hyphen gate from P12 D-10 — this gate is what prevents `v1.2.0-rc.1` from accidentally promoting `:latest`).
- [ ] `.github/workflows/release.yml:135` still reads `enable=${{ contains(github.ref, '-rc.') }}` (the `:rc` rolling tag gate).
- [ ] No commits in the rc.1 PR set touch `release.yml`, `cliff.toml`, or `docs/release-rc.md` (D-30).

Verification:
```bash
grep -n "contains(github.ref" .github/workflows/release.yml
git log --oneline --name-only main..HEAD~10 | grep -E '(release\.yml|cliff\.toml|release-rc\.md)' || echo "OK: no edits to release engineering files"
```

## 5. git-cliff release-notes preview shows P15..P20 commits

- [ ] `git cliff --unreleased --tag v1.2.0-rc.1 | head -100` shows commits from Phases 15, 16, 17, 18, 19, 20.
- [ ] No commit appears in the wrong section (e.g., a feat commit landing under "fixes"). If any commit is mis-categorized, hotfix the conventional-commit prefix on `main` BEFORE tagging (precedent: v1.1 P12 discipline).

Verification:
```bash
git cliff --unreleased --tag v1.2.0-rc.1 | head -100
```

## 6. 20-HUMAN-UAT.md sign-off

- [ ] All maintainer checkboxes in `20-HUMAN-UAT.md` are ticked.
- [ ] The Sign-off block at the bottom of `20-HUMAN-UAT.md` is filled in (maintainer name + date).

## 7. Tag command (maintainer runs LOCALLY)

On a clean checkout of `main` at the merge commit:

```bash
git checkout main
git pull origin main
git tag -a -s v1.2.0-rc.1 -m "v1.2.0-rc.1 — webhook block (P15..P20)"
git push origin v1.2.0-rc.1
```

> Per D-13 + D-29: this is a maintainer-local action. Claude does NOT execute it. The `-s` flag requires the maintainer's GPG signing key.

## 8. Post-publish verification (T-20-06 detection gate)

After `release.yml` finishes (≈ 10-20 minutes for both archs):

- [ ] `gh release view v1.2.0-rc.1` shows the auto-generated git-cliff body (D-31 — do NOT hand-edit post-publish).
- [ ] `docker manifest inspect ghcr.io/SimplicityGuy/cronduit:v1.2.0-rc.1` returns 2 manifests (amd64 + arm64).
- [ ] **T-20-06 detection:** capture the digest of `ghcr.io/SimplicityGuy/cronduit:latest` AND the digest of `ghcr.io/SimplicityGuy/cronduit:1.1.0`; the two digests MUST be identical. The hyphen-gate at `release.yml:134` held; the rc.1 tag did NOT promote `:latest`.
- [ ] `docker manifest inspect ghcr.io/SimplicityGuy/cronduit:rc` digest matches `:v1.2.0-rc.1` (the `-rc.` rolling tag updates).

Verification:
```bash
gh release view v1.2.0-rc.1
docker manifest inspect ghcr.io/SimplicityGuy/cronduit:v1.2.0-rc.1
LATEST_DIGEST=$(docker manifest inspect ghcr.io/SimplicityGuy/cronduit:latest | sha256sum | awk '{print $1}')
V1_1_0_DIGEST=$(docker manifest inspect ghcr.io/SimplicityGuy/cronduit:1.1.0 | sha256sum | awk '{print $1}')
[[ "$LATEST_DIGEST" == "$V1_1_0_DIGEST" ]] && echo "OK: T-20-06 mitigation verified" || echo "FAIL: :latest was promoted to rc.1"
docker manifest inspect ghcr.io/SimplicityGuy/cronduit:rc | grep -A2 digest
```

## Sign-off

- [ ] All sections above ticked.
- [ ] Tag pushed: `v1.2.0-rc.1`.
- [ ] `:latest` STILL at v1.1.0 (verified post-publish — T-20-06 mitigation held).
- [ ] No regressions reported in the first 24h post-publish.

Maintainer: ____________________ Date: ____________________
Tag commit SHA: ____________________
GHCR amd64 digest: ____________________
GHCR arm64 digest: ____________________
GHCR :latest digest (must equal v1.1.0 digest): ____________________
GHCR :1.1.0 digest (for comparison): ____________________
