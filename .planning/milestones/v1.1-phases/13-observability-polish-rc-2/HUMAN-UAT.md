# Phase 13 — Human UAT runbook (v1.1.0-rc.2 tag cut)

**Owner:** Maintainer (requires local signing key + repo push access)
**Phase:** 13-observability-polish-rc-2
**Blocked-on:** Phase 13 PR merged to `main`
**Precedent:** Phase 12 plan 07 (`v1.1.0-rc.1`) — this doc mirrors that pattern

---

## Why this is a human checkpoint

Per Phase 12 **D-13** (carried forward to Phase 13 per **D-22**), the rc tag cut is a **maintainer-action**, NOT a Claude-action. The signing key lives on the maintainer's workstation, not in a GHA runner's identity. This preserves tag-as-trust-anchor even against a supply-chain compromise of GitHub Actions.

Claude has:

1. Landed Phase 13 plans 01..05 (observability features).
2. Added dual-backend EXPLAIN tests + timezone test + OBS-05 CI guard (plan 06 Tasks 1..3).
3. Flipped OBS-01..OBS-05 to `[x]` / `Complete` in `.planning/REQUIREMENTS.md` (Task 4).
4. Committed `13-06-SUMMARY.md`.

The **only remaining step** is Task 5 (this document): the maintainer cuts `v1.1.0-rc.2` locally.

---

## Gate signal

Do NOT start this runbook until:

- [ ] Phase 13 PR merged to `main` (all plan 01..06 commits landed).
- [ ] `gh run list --workflow=ci.yml --branch=main --limit=1` shows `completed/success`.
- [ ] `gh run list --workflow=compose-smoke.yml --branch=main --limit=1` shows `completed/success`.
- [ ] `git pull --ff-only origin main` on your local clone is clean.

If any gate is red, fix that first. This runbook trusts main is tag-ready.

---

## Tag format invariant (from `feedback_tag_release_version_match.md`)

The tag MUST be `v1.1.0-rc.2` — full semver with the dot before `rc.N`.

- Correct: `v1.1.0-rc.2`
- WRONG: `v1.1.0-rc2` (no dot)
- WRONG: `1.1.0-rc.2` (no leading `v`)

The `.github/workflows/release.yml` D-10 gate from Phase 12 uses `contains(github.ref, '-rc.')` — the dot is load-bearing. If you push `v1.1.0-rc2` by accident, `:latest` will be moved (catastrophic). Double-check before `git push`.

`Cargo.toml` is already at `1.1.0` (landed in Phase 10 FOUND-13). No Cargo.toml bump is required for rc.2.

---

## Runbook (follows `docs/release-rc.md` verbatim — do NOT deviate)

### Step 1 — Pull main

```bash
git checkout main
git pull --ff-only origin main
git log -1 --oneline
```

Confirm the SHA matches the Phase 13 PR merge commit.

### Step 2 — Pre-flight (mirrors `docs/release-rc.md` "Pre-flight checklist")

```bash
# Phase 12.1 pin verification: :latest must still equal v1.0.1 before tagging.
# If this script is missing, the rc.2 cut is blocked — restore from Phase 12.1 commits.
scripts/verify-latest-retag.sh 1.0.1
```

If the script exits non-zero, STOP. The `:latest` pin is the load-bearing invariant; do not proceed.

### Step 3 — Preview release notes

```bash
git fetch --tags
git cliff --unreleased --tag v1.1.0-rc.2 -o /tmp/rc2-preview.md
cat /tmp/rc2-preview.md
```

Confirm Phase 13 commits appear — expect conventional-commit groups for:

- `feat(13-01)`: percentile helper, format helper, CSS tokens.
- `feat(13-02)`: three DB queries (sparkline, duration, timeline).
- `feat(13-03)`: Duration card on job detail.
- `feat(13-04)`: dashboard sparkline + success-rate badge.
- `feat(13-05)`: `/timeline` page.
- `test(13-06)`: EXPLAIN + timezone tests.
- `ci(13-06)`: OBS-05 grep guard.
- `docs(13-06)`: REQUIREMENTS.md flip.

If the preview is missing a section or mis-categorizes a commit, stop and fix the commit messages on main BEFORE tagging (per Phase 12 D-12: `git-cliff` is authoritative; no hand-editing the Release body after publish).

### Step 4 — GPG pre-flight

```bash
git config --get user.signingkey
```

- If it outputs a key → Step 5a (signed).
- If it outputs nothing → Step 5b (annotated, unsigned).

### Step 5a — Signed annotated tag

```bash
git tag -a -s v1.1.0-rc.2 -m "v1.1.0-rc.2 — release candidate (observability polish)"
```

### Step 5b — Unsigned annotated tag (fallback)

```bash
git tag -a v1.1.0-rc.2 -m "v1.1.0-rc.2 — release candidate (observability polish)"
```

### Step 6 — Push the tag

```bash
git push origin v1.1.0-rc.2
```

### Step 7 — Watch `release.yml`

```bash
gh run list --workflow=release.yml --branch=v1.1.0-rc.2 --limit=1
gh run watch --exit-status <run-id>
```

The workflow does:

1. Builds `linux/amd64 + linux/arm64` via cargo-zigbuild (not QEMU).
2. Pushes `ghcr.io/simplicityguy/cronduit:1.1.0-rc.2` as a multi-arch manifest.
3. Advances the `:rc` rolling tag to this digest.
4. **Skips** `:latest`, `:1`, `:1.1` updates (D-10 gate: `contains(github.ref, '-rc.')` wraps the metadata-action step).
5. Publishes a GitHub Release marked `prerelease = true` with the `git-cliff` body.

Expected runtime: ~5-10 min.

### Step 8 — Post-push verification

All checks must pass:

```bash
# 1. Multi-arch manifest published
docker buildx imagetools inspect ghcr.io/simplicityguy/cronduit:1.1.0-rc.2
# Expect: two platform entries (linux/amd64 + linux/arm64)

# 2. :rc rolling tag advanced to rc.2 digest
docker buildx imagetools inspect ghcr.io/simplicityguy/cronduit:rc
# Expect: digest equals :1.1.0-rc.2 digest

# 3. :latest STILL pinned to v1.0.1 (the load-bearing invariant)
scripts/verify-latest-retag.sh 1.0.1
# Expect: exit 0

# 4. Versioned permalink pulls
docker pull ghcr.io/simplicityguy/cronduit:1.1.0-rc.2
docker run --rm ghcr.io/simplicityguy/cronduit:1.1.0-rc.2 --version
# Expect: "cronduit 1.1.0"

# 5. GitHub Release flagged as pre-release
gh release view v1.1.0-rc.2 --json isPrerelease --jq .isPrerelease
# Expect: true

# 6. Compose-smoke against the rc.2 image
cd examples && docker compose up -d
sleep 90
docker compose ps   # Expect: Up N seconds (healthy)
docker compose down -v
```

If any check fails, do NOT delete-and-retag. Ship `v1.1.0-rc.3` instead (a fresh pre-release tag, not a force-push). This protects consumers who already pulled rc.2.

### Step 9 — Report back

Post the results in the following structured format so the orchestrator / verifier can route as human-validated:

```
rc2 tag pushed
Image digest (:1.1.0-rc.2): sha256:<64-hex-chars>
:rc digest === :1.1.0-rc.2 digest: yes
:latest digest: sha256:<64-hex-chars> (equal to v1.0.1 digest — UNCHANGED)
GitHub Release URL: https://github.com/simplicityguy/cronduit/releases/tag/v1.1.0-rc.2
Compose-smoke: PASS
```

---

## Rollback

There is no rollback for a pushed tag — tags are one-way in this project's policy.

If post-push verification fails:

1. Keep `v1.1.0-rc.2` in place (it is an observable artifact; deleting it damages consumer trust).
2. File a Phase 13.1 (or equivalent) hotfix plan.
3. Ship `v1.1.0-rc.3` after the fix lands.

`:latest` is protected by D-10 even if rc.2 is broken — consumers on `:latest` stay on v1.0.1.

---

## Cross-references

- Phase 12 D-10 / D-11 / D-12 / D-13 — release mechanics (reused verbatim per Phase 13 D-22).
- Phase 12.1 — `:latest` pin + `:main` rolling tag.
- `docs/release-rc.md` — this runbook's canonical source.
- `scripts/verify-latest-retag.sh` — `:latest` integrity check.
- `.github/workflows/release.yml` — multi-arch build + pre-release gating.
- `.planning/phases/12-docker-healthcheck-rc-1-cut/12-07-SUMMARY.md` — rc.1 precedent.

---

*Phase: 13-observability-polish-rc-2*
*Plan: 06 Task 5 (maintainer-action checkpoint)*
*Created: 2026-04-21*
