---
phase: 09-ci-cd-improvements
plan: 02
subsystem: ci-cd
tags: [ci-cd, github-actions, ghcr, cleanup, retention]
dependency_graph:
  requires: []
  provides:
    - "Monthly GHCR image pruning for ghcr.io/<owner>/cronduit"
  affects:
    - ".github/workflows/cleanup-images.yml"
tech_stack:
  added:
    - "dataaxiom/ghcr-cleanup-action v1.0.16 (pinned by SHA cd0cdb900b5dbf3a6f2cc869f0dbb0b8211f50c4)"
  patterns:
    - "Pin third-party actions by 40-char SHA with # vX.Y.Z trailing comment"
    - "Least-privilege workflow permissions (top-level contents:read, job-level packages:write)"
    - "Explicit timeout-minutes on every job"
    - "Concurrency group keyed on github.ref with cancel-in-progress:false for cleanup workflows"
key_files:
  created:
    - ".github/workflows/cleanup-images.yml"
  modified: []
decisions:
  - "Pinned ghcr-cleanup-action v1.0.16 (SHA cd0cdb900b5dbf3a6f2cc869f0dbb0b8211f50c4), verified as the latest release via gh api repos/dataaxiom/ghcr-cleanup-action/releases/latest before writing the file."
  - "Collapsed discogsography's list-sub-projects + matrix strategy to a single flat job because Cronduit publishes exactly ONE image."
  - "Added a top-level `permissions: { contents: read }` block (discogsography relies on the default); Cronduit CONTEXT.md mandates explicit top-level permissions."
metrics:
  duration: ~4min
  completed: 2026-04-13
---

# Phase 09 Plan 02: Monthly GHCR Image Cleanup Workflow Summary

Port discogsography's monthly GHCR image pruner to Cronduit as a single flat job pinned by 40-char SHA, keeping the package page navigable by retaining the two most recent tagged releases and deleting untagged/partial/>30-day-old revisions.

## What Was Built

One new file, `.github/workflows/cleanup-images.yml` (44 lines), which wires `dataaxiom/ghcr-cleanup-action@cd0cdb900b5dbf3a6f2cc869f0dbb0b8211f50c4 # v1.0.16` to run on `workflow_dispatch` and on `schedule: cron "0 0 15 * *"` against the `cronduit` package under `${{ github.repository_owner }}`.

### Workflow structure

- **Triggers:** `workflow_dispatch` + `schedule.cron: "0 0 15 * *"` (15th of each month, 00:00 UTC)
- **Concurrency:** `cleanup-images-${{ github.ref }}`, `cancel-in-progress: false` (a cleanup in progress is never cancelled by a new dispatch)
- **Top-level permissions:** `contents: read`
- **Single job `cleanup`:**
  - `runs-on: ubuntu-latest`
  - `timeout-minutes: 30`
  - Job-level `permissions: { packages: write, contents: read }` (least privilege)
  - One step `Cleanup Docker Images` invoking the pinned action with `delete-partial-images: true`, `delete-untagged: true`, `keep-n-tagged: 2`, `older-than: 30 days`, `token: ${{ secrets.GITHUB_TOKEN }}`, `package: cronduit`, `owner: ${{ github.repository_owner }}`.

### Intentional differences from the discogsography source

1. **No `list-sub-projects` upstream job** — Cronduit publishes one image.
2. **No `strategy.matrix`** — removed the entire `strategy:` block.
3. **No `needs: [list-sub-projects]`** — single flat job.
4. **`package: cronduit`** as a literal string instead of `${{ matrix.package }}`.
5. **Explicit top-level `permissions: { contents: read }`** (discogsography relies on the default).
6. **No emoji** in the step name (Cronduit scopes the emoji exception to `scripts/update-project.sh` only).

## SHA Re-verification

CONTEXT.md and the plan both lock `dataaxiom/ghcr-cleanup-action@cd0cdb900b5dbf3a6f2cc869f0dbb0b8211f50c4 # v1.0.16`. Before writing the file I re-verified this against the live GitHub API:

```bash
gh api repos/dataaxiom/ghcr-cleanup-action/releases/latest --jq '{tag_name, name}'
#   -> {"name":"v1.0.16","tag_name":"v1.0.16"}

gh api repos/dataaxiom/ghcr-cleanup-action/git/refs/tags/v1.0.16 --jq '.object.sha, .object.type'
#   -> cd0cdb900b5dbf3a6f2cc869f0dbb0b8211f50c4
#   -> commit
```

v1.0.16 is still current; the locked SHA resolves to an annotated commit-type object with hash `cd0cdb900b5dbf3a6f2cc869f0dbb0b8211f50c4`. No SHA update was needed.

## Acceptance Criteria Results

| # | Criterion | Result |
|---|-----------|--------|
| 1 | `test -f .github/workflows/cleanup-images.yml` | PASS |
| 2 | `grep -c 'workflow_dispatch:'` returns 1 | PASS (1) |
| 3 | `grep -Fc 'cron: "0 0 15 * *"'` returns 1 | PASS (1) |
| 4 | `grep -Ec 'dataaxiom/ghcr-cleanup-action@[a-f0-9]{40} # v[0-9]+\.[0-9]+\.[0-9]+'` returns 1 | PASS (1) |
| 5 | `grep -c 'packages: write'` returns 1 | PASS (1) |
| 6 | `grep -c 'timeout-minutes: 30'` returns 1 | PASS (1) |
| 7 | `grep -c 'package: cronduit'` returns 1 | PASS (1) |
| 8 | `grep -c 'keep-n-tagged: 2'` returns 1 | PASS (1) |
| 9 | `grep -c 'older-than: 30 days'` returns 1 | PASS (1) |
| 10 | `grep -c 'delete-untagged: true'` returns 1 | PASS (1) |
| 11 | `grep -c 'delete-partial-images: true'` returns 1 | PASS (1) |
| 12 | `grep -c 'strategy:'` returns 0 | PASS (0) |
| 13 | `grep -c 'list-sub-projects'` returns 0 | PASS (0) |
| 14 | `grep -c 'matrix'` returns 0 | PASS (0) |
| 15 | No non-ASCII bytes in file | PASS (verified via Python byte scan; em-dash replaced with `--`) |
| 16 | Valid YAML | PASS (`yq e . <file>` succeeds; acceptance criterion specified `python3 -c 'import yaml'`, but the PyYAML module isn't installable under PEP-668 on this host; used `yq` + `yamllint` + `actionlint` as stronger equivalents) |

Additional validations run beyond the plan:

- `yamllint` (strict indentation + trailing whitespace): PASS
- `actionlint` (GitHub Actions semantics, shellcheck, expression validation): PASS

## Deviations from Plan

**None** that change file contents. One verification-command substitution:

- The plan's automated verify command `python3 -c 'import yaml; yaml.safe_load(...)'` relies on a PyYAML module that is not importable from the system Python on this host (PEP-668 externally-managed environment, `--break-system-packages` is a footgun and not acceptable on a developer workstation). I validated YAML with `yq e . <file>` (libyaml-backed), then added `yamllint` and `actionlint` as stricter equivalents — all three pass. The file semantics are identical to what `yaml.safe_load` would accept; this is a tooling-availability substitution, not a plan deviation.

- One micro-detail that a literal interpretation of the plan's code block would have caused: the plan's suggested top-of-file comment block repeated the exact acceptance-criterion strings (`keep-n-tagged: 2`, `older-than: 30 days`, `delete-untagged: true`, `delete-partial-images: true`) and the word `matrix`, which would have made `grep -c` return 2 (or 1) instead of the required 1 (or 0). I rewrote the comment prose to describe the retention policy in English without repeating those literal keyword strings. The resulting step inputs are byte-for-byte identical to the plan, only the file-top comments were rephrased.

## Files

| Path | Status | Lines |
|------|--------|-------|
| `.github/workflows/cleanup-images.yml` | created | 44 |

## Commits

- `eef3df3` — feat(09-02): add monthly GHCR image cleanup workflow

## Self-Check: PASSED

- File `.github/workflows/cleanup-images.yml` exists (verified via `test -f`).
- Commit `eef3df3` is present in `git log --oneline`.
- All 16 acceptance criteria pass (YAML parse verified via `yq` + `actionlint` stand-ins for the unavailable `python3 -c 'import yaml'`).
- No other files modified (`git status` clean after commit).
