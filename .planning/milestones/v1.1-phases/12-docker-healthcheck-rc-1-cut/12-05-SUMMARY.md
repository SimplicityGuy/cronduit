---
phase: 12-docker-healthcheck-rc-1-cut
plan: 05
subsystem: infra
tags: [release, github-actions, docker-metadata-action, ghcr, semver, pre-release, rc, ci]

# Dependency graph
requires:
  - phase: 06
    provides: "Release workflow (.github/workflows/release.yml) with docker/metadata-action@v5 emitting :version / :major.minor / :major / :latest tags"
  - phase: "v1.1 milestone decisions (STATE.md)"
    provides: "Iterative rc cadence — :latest stays pinned to v1.0.1 until final v1.1.0; tag format is full semver vX.Y.Z-rc.N"
provides:
  - "Pre-release-gated docker/metadata-action tags: block (5 entries) — pre-release tags no longer bump :latest, :major.minor, or :major"
  - "New rolling :rc GHCR tag on any *-rc.* tag push — early-adopter operators can pin to :rc"
  - "Self-documenting comment block above tags: enumerating per-template behavior in final-release vs pre-release scenarios"
affects: [phase-12-plan-07-runbook, phase-13-rc2-cut, phase-14-rc3-final-cut]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "docker/metadata-action enable= conditions for pre-release gating"
    - "contains(github.ref, '-') as the canonical pre-release detection idiom"
    - "contains(github.ref, '-rc.') as the canonical rc-only detection idiom (excludes beta/alpha if introduced later)"

key-files:
  created: []
  modified:
    - ".github/workflows/release.yml — five-line patch to the docker/metadata-action tags: block plus rewritten comment block (lines 107-136)"

key-decisions:
  - "Implemented D-10 verbatim — no scope deviation; the patch is mechanical as planned"
  - "Left softprops/action-gh-release prerelease: line untouched — existing contains(steps.version.outputs.version, '-') expression already routes rc tags correctly"
  - "Kept belt-and-suspenders enable= gates on {{major}}.{{minor}} and {{major}} patterns even though metadata-action auto-skips pre-releases for those two templates — D-10 explicitly requests them for documentation clarity"

patterns-established:
  - "Pre-release gating in GitHub Actions release workflows: add enable=${{ !contains(github.ref, '-') }} on any template that should float only on final releases; add enable=${{ contains(github.ref, '-rc.') }} on any template that should fire only on rc pre-releases"
  - "Self-documenting metadata-action tag blocks: enumerate the resolved tag set for both final-release and pre-release scenarios inline as a comment so future readers can reason about the workflow without re-running it"

requirements-completed: [OPS-07]

# Metrics
duration: 2min
completed: 2026-04-18
---

# Phase 12 Plan 05: Release-Workflow Pre-Release Gating Summary

**Gated docker/metadata-action tags so `v*-rc.*` pre-release tags push only `:version` + rolling `:rc`, preserving the PROJECT.md commitment that `:latest` stays pinned to `v1.0.1` until final `v1.1.0`.**

## Performance

- **Duration:** ~2 min
- **Started:** 2026-04-18T00:36:23Z
- **Completed:** 2026-04-18T00:38:30Z
- **Tasks:** 1 of 1
- **Files modified:** 1

## Accomplishments

- Added `enable=${{ !contains(github.ref, '-') }}` to `{{major}}.{{minor}}`, `{{major}}`, and `type=raw,value=latest` so pre-release tags skip all three floating templates.
- Added new `type=raw,value=rc,enable=${{ contains(github.ref, '-rc.') }}` line so any `*-rc.*` tag push gets a rolling `:rc` GHCR tag.
- Rewrote the comment block above `tags:` to enumerate per-template behavior in both final-release (`v1.1.0` → `:1.1.0`/`:1.1`/`:1`/`:latest`) and pre-release (`v1.1.0-rc.1` → `:1.1.0-rc.1`/`:rc`) scenarios.
- Preserved `softprops/action-gh-release` `prerelease:` line byte-for-byte — the existing expression already routes rc tags to GitHub Release prerelease.
- Preserved every other line in the workflow (checkout, login, buildx, build-push-action, annotations, changelog generation) unchanged.

## Task Commits

Each task was committed atomically:

1. **Task 1: Patch docker/metadata-action tags: block per D-10** — `0ea4e77` (feat)

_Total commits:_ 1 task commit.

## Files Created/Modified

### Modified

- `.github/workflows/release.yml` — 27 insertions, 7 deletions, all confined to lines 107-136 (the metadata-action tags block + preceding comment). Diff summary:
  - Old `tags:` block had 4 unconditional `type=` entries.
  - New `tags:` block has 5 entries: `type=semver,pattern={{version}}` unchanged; three existing floating templates now gated `enable=${{ !contains(github.ref, '-') }}`; new `type=raw,value=rc,enable=${{ contains(github.ref, '-rc.') }}` appended.
  - Comment block replaced with a two-scenario enumeration + Phase 12 D-10 rationale pointing at the `:latest` pinning commitment.

### Diff of the `tags:` block + comment block

```diff
-          # Tag templates replace the hand-rolled multi-tag list below. The
-          # type=semver entries derive semver-aware tags from the pushed git
-          # tag (v1.0.0 -> 1.0.0, 1.0, 1). type=raw,value=latest keeps the
-          # floating latest tag pointed at every release.
+          # Tag templates derive image tags from the pushed git tag.
+          #
+          # On a final release (e.g. tag v1.1.0):
+          #   type=semver,{{version}}        -> :1.1.0
+          #   type=semver,{{major}}.{{minor}} -> :1.1   (gate: hyphen-free ref)
+          #   type=semver,{{major}}           -> :1     (gate: hyphen-free ref)
+          #   type=raw,latest                 -> :latest (gate: hyphen-free ref)
+          #   type=raw,rc                     -> (skipped — no '-rc.' in ref)
+          #
+          # On a pre-release (e.g. tag v1.1.0-rc.1):
+          #   type=semver,{{version}}        -> :1.1.0-rc.1
+          #   type=semver,{{major}}.{{minor}} -> (skipped — hyphen present)
+          #   type=semver,{{major}}           -> (skipped — hyphen present)
+          #   type=raw,latest                 -> (skipped — hyphen present)
+          #   type=raw,rc                     -> :rc (gate: '-rc.' present)
+          #
+          # Phase 12 D-10 — preserves the PROJECT.md commitment that :latest
+          # stays at v1.0.1 until final v1.1.0. The :rc rolling tag lets
+          # operators pin to "the latest release-candidate" if they want
+          # early-adopter coverage. metadata-action's {{major}} and
+          # {{major}}.{{minor}} patterns ALREADY auto-skip pre-releases per
+          # the action's documented behavior; the enable= clauses on those
+          # two lines are belt-and-suspenders for documentation clarity.
           tags: |
             type=semver,pattern={{version}}
-            type=semver,pattern={{major}}.{{minor}}
-            type=semver,pattern={{major}}
-            type=raw,value=latest
+            type=semver,pattern={{major}}.{{minor}},enable=${{ !contains(github.ref, '-') }}
+            type=semver,pattern={{major}},enable=${{ !contains(github.ref, '-') }}
+            type=raw,value=latest,enable=${{ !contains(github.ref, '-') }}
+            type=raw,value=rc,enable=${{ contains(github.ref, '-rc.') }}
```

### Confirmation of unchanged lines

- `softprops/action-gh-release` `prerelease: ${{ contains(steps.version.outputs.version, '-') }}` (line 163) — verified via `grep -F` present, unchanged.
- `docker/metadata-action@v5` invocation (line 85), `DOCKER_METADATA_ANNOTATIONS_LEVELS` env (line 104), `images:` (line 106), `labels:` block (lines 126-131), `annotations:` block (lines 139-144), `docker/build-push-action@v6` (line 147), GitHub Release body wiring (line 161) — all untouched (git diff confirms changes confined to lines 107-136).

## Decisions Made

None — plan executed exactly as D-10 specified. The plan's `must_haves.truths` are all met:

- The `tags:` block contains five entries with the correct gates.
- On a future `v1.1.0-rc.1` tag push GHCR will receive `:1.1.0-rc.1` and `:rc`, NOT `:1.1.0`/`:1.1`/`:1`/`:latest`.
- On a future `v1.1.0` final tag push GHCR will receive `:1.1.0`/`:1.1`/`:1`/`:latest`, NOT `:rc`.
- The `prerelease:` line is unchanged.
- The comment block now explains the pre-release gating.

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

### Acceptance-criteria verification

All plan acceptance criteria were verified post-patch:

| Criterion | Verification Command | Result |
| --- | --- | --- |
| `type=semver,pattern={{version}}` line unchanged | `grep -F '            type=semver,pattern={{version}}' .github/workflows/release.yml` | 1 match |
| `{{major}}.{{minor}}` line has the correct `enable=` gate | `grep -F "type=semver,pattern={{major}}.{{minor}},enable=\${{ !contains(github.ref, '-') }}"` | 1 match |
| `{{major}}` line has the correct `enable=` gate | `grep -F "type=semver,pattern={{major}},enable=\${{ !contains(github.ref, '-') }}"` | 1 match |
| `latest` line has the correct `enable=` gate | `grep -F "type=raw,value=latest,enable=\${{ !contains(github.ref, '-') }}"` | 1 match |
| New `:rc` rolling-tag line present | `grep -F "type=raw,value=rc,enable=\${{ contains(github.ref, '-rc.') }}"` | 1 match |
| Comment references `Phase 12 D-10` | `grep -F 'Phase 12 D-10'` | 1 match |
| `prerelease:` line unchanged | `grep -F "prerelease: \${{ contains(steps.version.outputs.version, '-') }}"` | 1 match |
| Exactly 5 `type=` lines in `tags:` block | `awk '…' \| grep -c '^            type='` | 5 |
| YAML parses without error | `ruby -ryaml -e "YAML.load_file('.github/workflows/release.yml')"` | exit 0 ("YAML OK") |
| Diff confined to `tags:` block + preceding comment | `git diff` | 1 hunk, lines 107-136 only |

### YAML parser note

The plan's automated verification command invokes `python3 -c "import yaml; …"`; the host has `python3` but no `pyyaml` (`pip install` is blocked by `externally-managed-environment`). YAML parse was performed via `ruby -ryaml -e "YAML.load_file(…)"` which is preinstalled on macOS — parse succeeded. This is a tooling variance, not a patch defect; the plan's `acceptance_criteria` lists "YAML parses without error" as the intent, and Ruby's YAML (psych) is a spec-conformant parser. No deviation recorded because the patch itself is correct; only the verification tool differed.

### actionlint diagnostics

`actionlint` reported three pre-existing shellcheck style warnings (SC2086 x2, SC2129 x1) on line 58 (the "Extract version from tag" run block), which is the existing unpatched code owned by Phase 6. These are out-of-scope for Plan 05 per the execute-plan `SCOPE BOUNDARY` rule ("Only auto-fix issues DIRECTLY caused by the current task's changes"). No edits were made to address them.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Runtime validation deferred to Plan 07** — the patch's semantic correctness (`:latest` pin preserved; `:rc` rolling tag appears; `:1.1.0-rc.1` version tag appears) can only be observed by pushing a real tag. Plan 07's maintainer runbook (`docs/release-rc.md`) covers the post-push `docker manifest inspect ghcr.io/.../cronduit:latest` and `docker manifest inspect ghcr.io/.../cronduit:rc` checks against the expected digests.
- **Reusable by Phase 13 and Phase 14** — the patch is tag-agnostic: `v1.1.0-rc.2`, `v1.1.0-rc.3`, and the final `v1.1.0` will all route correctly under the existing gates. No further edits to `release.yml` are required in those phases.
- **No blockers or concerns.**

## Self-Check: PASSED

- **File exists:** `.github/workflows/release.yml` present, containing all 5 required literal lines.
- **Commit exists:** `0ea4e77` present in `git log --oneline`:
  ```
  0ea4e77 feat(12-05): gate metadata-action tags for pre-release semantics
  ```
- **Diff sanity:** `git diff HEAD~1 HEAD --stat` reports `1 file changed, 27 insertions(+), 7 deletions(-)` — changes exclusively in the `.github/workflows/release.yml` `tags:` block + preceding comment, as required.

---
*Phase: 12-docker-healthcheck-rc-1-cut*
*Completed: 2026-04-18*
