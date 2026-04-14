---
phase: 09-ci-cd-improvements
plan: 03
subsystem: ci-cd
tags: [ci-cd, scripts, dependency-updates, cargo, docker, github-actions, tailwind, pre-commit]
dependency_graph:
  requires:
    - "09-01 cleanup-cache.yml (workflow file must exist before update_gha_pins iterates)"
    - "09-02 cleanup-images.yml (ditto; also provides the first SHA-pinned action for the updater to exercise)"
  provides:
    - "One-command dependency updater: scripts/update-project.sh"
    - "just update-cargo recipe (lockfile-only Cargo refresh)"
    - "just update-hooks recipe (graceful no-op pre-commit autoupdater)"
    - "backups/ gitignore exclusion (so timestamped backup trees don't leak into git status)"
  affects:
    - "scripts/update-project.sh"
    - "justfile"
    - ".gitignore"
tech_stack:
  added: []
  patterns:
    - "Delegate every ecosystem command to a 'just' recipe so justfile stays the single source of truth (CLAUDE.md D-10 / FOUND-12)"
    - "Per-ecosystem atomic commits (cargo / docker / tailwind / gha / pre-commit) so the operator can cherry-pick"
    - "gh api repos/<o>/<r>/releases/latest → git/refs/tags/<tag> → .object.sha pattern for SHA-pinned Action updates (with annotated-tag dereference via git/tags/<sha>)"
    - "Pitfall 14 openssl-sys re-check after every cargo mutation (delegated to 'just openssl-check')"
    - "Bash 3.2 portability: no associative arrays, portable sed (GNU vs BSD), parallel simple-array caches"
    - "Emoji visual logging scoped exclusively to this one file (CLAUDE.md exception explicitly granted in CONTEXT.md Plan 3)"
key_files:
  created:
    - "scripts/update-project.sh (613 lines, 0755)"
    - ".planning/phases/09-ci-cd-improvements/09-03-SUMMARY.md (this file)"
  modified:
    - "justfile (+23 lines: update-cargo + update-hooks recipes)"
    - ".gitignore (+4 lines: backups/ exclusion with explanatory comment)"
decisions:
  - "Script refuses to run unless both (a) project-root markers are present and (b) current branch is NOT main (enforces CLAUDE.md no-direct-main policy)"
  - "Dockerfile base-image regex reduced from plan's 'FROM[^r]*rust:...' to plain 'rust:<ver>-slim-bookworm' because the plan's regex never matches the Cronduit Dockerfile (word 'platform' in 'FROM --platform=\$BUILDPLATFORM' contains a lowercase r)"
  - "Tailwind tag lookup uses 'gh api --paginate repos/tailwindlabs/tailwindcss/tags' because the unpaginated first page is all v4.x and the v3.x filter would silently match nothing"
  - "GHA pin updater uses parallel simple arrays instead of associative arrays to stay portable to bash 3.2 (system bash on macOS)"
  - "Tailwind tag filter defaults to '^v3\\.[0-9]+\\.[0-9]+$' (v3 only); --major widens it to any semver, honoring the existing 'Pinned to v3.4.17 -- v4 breaks tailwind.config.js' comment in justfile"
  - "Distroless runtime image (gcr.io/distroless/static-debian12:nonroot) is left version-floating — the :nonroot tag already pulls the latest nonroot variant on every docker build, so an updater rewrite would be a no-op"
metrics:
  duration: ~9min
  completed: 2026-04-14
  tasks: 5
  files: 3
  commits: 7
---

# Phase 09 Plan 03: scripts/update-project.sh Summary

One-command Cronduit dependency updater: Cargo lockfile, Dockerfile base image, GitHub Actions SHA pins, Tailwind standalone binary, and pre-commit hooks — each in its own atomic commit on a feature branch, with per-ecosystem dry-run support and a Pitfall 14 openssl-sys re-check baked into the cargo path.

## What Was Built

### scripts/update-project.sh (613 lines)

A single bash script that walks five ecosystems in order and commits each set of changes atomically on a feature branch. The script is a Cronduit adaptation of discogsography's `update-project.sh`, with all Python/Node/uv paths removed and the hard "no direct commits to main" enforcement added.

**Option surface (5 flags, no --python):**

| Flag | Purpose |
|------|---------|
| `--dry-run` | Print intended changes, make no file mutations |
| `--major` | Allow major-version upgrades (requires `cargo-edit` for cargo; unlocks v3→v4 line for Tailwind) |
| `--no-backup` | Skip creating `backups/project-updates-<TS>/` |
| `--skip-tests` | Skip `just nextest` after updates |
| `--help`, `-h` | Print usage |

**Safety rails:**

1. Must be run from project root (requires `Cargo.toml` + `justfile` + `Dockerfile` + `src/main.rs` or `crates/`).
2. **Refuses to run on `main`** (CLAUDE.md: "all changes land via PR on a feature branch").
3. Requires `cargo`, `git`, `gh`, `jq`, `curl`, `just` on PATH (clear error message listing missing tools).
4. `--major` validates that `cargo upgrade` (from `cargo-edit`) is installed before proceeding.
5. Refuses to run with a dirty working tree.
6. Timestamped backups in `backups/project-updates-<TS>/` unless `--no-backup`.

**main() execution order:**

1. `update_cargo_deps` — `cargo upgrade --incompatible allow` (on `--major`), then `just update-cargo`, then `just openssl-check` (Pitfall 14 rustls-only guard).
2. `update_dockerfile_base` — looks up the newest `rust:N.M-slim-bookworm` tag on Docker Hub via `https://hub.docker.com/v2/repositories/library/rust/tags/` and rewrites Dockerfile in place.
3. `update_tailwind_version` — walks Tailwind tags via `gh api --paginate repos/tailwindlabs/tailwindcss/tags`, filters to `v3.*` (or any semver with `--major`), rewrites `justfile` and force-redownloads `./bin/tailwindcss` via `just tailwind`.
4. `update_gha_pins` — scans `.github/workflows/*.yml` for every `uses: <owner>/<repo>@<40-hex> # vX.Y.Z` line, resolves each to the current release SHA via `gh api repos/<o>/<r>/releases/latest` + `git/refs/tags/<tag>` (with annotated-tag dereference), rewrites the file. Floating-tag actions are intentionally ignored per CONTEXT.md cross-cutting rule.
5. `update_precommit_hooks` — delegates to `just update-hooks` (no-op if `.pre-commit-config.yaml` absent).
6. `run_tests_if_requested` — `just nextest` unless `--skip-tests` or no changes were made.
7. `generate_summary` — prints commits added, next-steps (push, `gh pr create`), and backup path.

Each ecosystem updater creates its own atomic commit (`chore(deps): update cargo dependencies`, `chore(deps): bump rust base image to <ver>`, etc.) so the operator can cherry-pick.

### justfile additions (2 new recipes, +23 lines)

```just
# -------------------- dependency updates (scripts/update-project.sh) --------------------

# Update Cargo.lock within existing Cargo.toml constraints (minor/patch only).
update-cargo:
    cargo update

# Update pre-commit hooks to their latest versions. No-op if pre-commit is not
# installed or .pre-commit-config.yaml is missing.
update-hooks:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ ! -f .pre-commit-config.yaml ]; then
        echo "No .pre-commit-config.yaml — skipping hook updates"
        exit 0
    fi
    if ! command -v pre-commit >/dev/null 2>&1; then
        echo "pre-commit not installed — skipping hook updates"
        exit 0
    fi
    pre-commit autoupdate
```

No existing recipes were renamed, reordered, or modified. `just --list` now enumerates both new recipes alongside the existing `ci`, `tailwind`, `openssl-check`, etc.

### .gitignore additions (+4 lines)

```gitignore
# scripts/update-project.sh creates timestamped backups/ trees before any
# dependency update. Never committed.
backups/
```

Added under the existing `# Cronduit` section, right after the `bin/tailwindcss` line.

## Deviations from Plan

Two auto-fixes were needed during Task 3 execution — both bugs in the regex/API patterns the plan specified verbatim. Discovered by running `./scripts/update-project.sh --dry-run` against the actual Cronduit repo.

### Auto-fixed Issues

**1. [Rule 1 - Bug] Dockerfile rust image regex never matched**

- **Found during:** Task 3 dry-run
- **Issue:** The plan's regex `'FROM[^r]*rust:[0-9]+\.[0-9]+(\.[0-9]+)?-slim-bookworm'` excludes any lowercase `r` between `FROM` and `rust:`. Cronduit's Dockerfile is `FROM --platform=$BUILDPLATFORM rust:1.94-slim-bookworm AS builder` — and the word **`platform`** contains a lowercase `r`. Result: `grep -Eo` returned empty, `current_rust` was empty, the whole function printed "No 'rust:<version>-slim-bookworm' line found — skipping" and bailed.
- **Fix:** Simplified to plain `'rust:[0-9]+\.[0-9]+(\.[0-9]+)?-slim-bookworm'` (no `FROM` anchor, no `[^r]*` exclusion).
- **Files modified:** `scripts/update-project.sh` (1 regex line + 3-line fix comment).
- **Commit:** `7a2633a`

**2. [Rule 1 - Bug] Tailwind tag lookup returned nothing (pagination)**

- **Found during:** Task 3 dry-run
- **Issue:** The plan's `gh api repos/tailwindlabs/tailwindcss/tags --jq '.[].name'` returns only the first 30 tags by default. In 2026 those are all v4.x. The script then filters to `^v3\.[0-9]+\.[0-9]+$`, matches nothing, and prints "Could not determine latest Tailwind tag — skipping". The updater was effectively dead on arrival.
- **Fix:** Added `--paginate` to the `gh api` call so the full tag history is walked.
- **Files modified:** `scripts/update-project.sh` (1 line + 4-line fix comment).
- **Commit:** `7a2633a`

**3. [Rule 1 - Bug] `local -A` (associative array) is bash 4+ only**

- **Found during:** Task 4 dry-run
- **Issue:** macOS ships `bash 3.2.57` as `/bin/bash` and `#!/usr/bin/env bash` picks it up. The plan's `update_gha_pins` function declares `local -A latest_sha_cache=()` and `local -A latest_tag_cache=()`, which errors out with `local: -A: invalid option`. The function aborted before even reaching the first SHA lookup. GitHub Actions `ubuntu-latest` ships bash 5.x so the bug would have been invisible on CI but fatal locally.
- **Fix:** Replaced associative arrays with parallel simple arrays (`cache_keys`, `cache_shas`, `cache_tags`) + a linear-probe lookup. Cache size is bounded by the number of distinct SHA-pinned actions (currently 1 — the dataaxiom ghcr-cleanup-action) so O(N) per probe is free.
- **Files modified:** `scripts/update-project.sh` (the cache setup block inside `update_gha_pins`).
- **Commit:** `eb88c15`

### Authentication Gates

None.

## Per-Task Acceptance Criteria Results

### Task 1 — justfile recipes + .gitignore

| Criterion | Result |
|---|---|
| `grep -c '^backups/$' .gitignore` returns 1 | PASS |
| `grep -c '^update-cargo:' justfile` returns 1 | PASS |
| `grep -c '^update-hooks:' justfile` returns 1 | PASS |
| `grep -c '^ci:' justfile` returns 1 (existing recipe untouched) | PASS |
| `grep -c '^tailwind:' justfile` returns 1 (existing recipe untouched) | PASS |
| `just --list` exits 0 and shows both new recipes | PASS |
| `git diff` shows ONLY additions | PASS |

### Task 2 — skeleton (arg parsing, safety rails, tool check)

| Criterion | Result |
|---|---|
| `test -x scripts/update-project.sh` | PASS |
| `bash -n scripts/update-project.sh` exits 0 | PASS |
| `--help` contains all 5 flag names | PASS |
| `--help` contains zero `--python` occurrences | PASS |
| `--dry-run` from project root exits 0 | PASS |
| `--dry-run` output contains "Mode:        DRY-RUN" | PASS |
| Running from /tmp without Cargo.toml: non-zero exit with "project root" in stderr | PASS (exit 1) |
| `grep -c 'set -euo pipefail'` returns 1 | PASS |
| `grep -c 'CURRENT_BRANCH'` returns ≥2 | PASS |
| `grep -c '"main"'` returns ≥1 | PASS |
| `grep -c 'backups/project-updates-'` returns ≥1 | PASS |
| `grep -cE 'for tool in cargo git gh jq curl'` returns 1 | PASS |
| `git diff` shows only additions | PASS |

### Task 3 — cargo + dockerfile + tailwind updaters

| Criterion | Result |
|---|---|
| `bash -n` exits 0 | PASS |
| `grep -c '^update_cargo_deps()'` returns 1 | PASS |
| `grep -c '^update_dockerfile_base()'` returns 1 | PASS |
| `grep -c '^update_tailwind_version()'` returns 1 | PASS |
| `grep -c 'just openssl-check'` returns ≥1 | PASS (4) |
| `grep -c 'just update-cargo'` returns ≥1 | PASS (4) |
| `grep -c 'cargo upgrade --incompatible'` returns 1 | PASS (4 including docstring + fix comment) |
| `grep -c 'hub.docker.com/v2/repositories/library/rust'` returns 1 | PASS |
| `grep -c 'tailwindlabs/tailwindcss'` returns ≥1 | PASS (3) |
| `--dry-run` exits 0 | PASS |
| Output contains "Updating Cargo Dependencies", "Updating Dockerfile Base Images", "Updating Tailwind" | PASS |
| Output contains "[DRY RUN]" lines for each ecosystem | PASS |
| No mutations to tracked files after dry-run | PASS |

### Task 4 — GHA pins + pre-commit hooks

| Criterion | Result |
|---|---|
| `bash -n` exits 0 | PASS |
| `grep -c '^update_gha_pins()'` returns 1 | PASS |
| `grep -c '^update_precommit_hooks()'` returns 1 | PASS |
| `grep -c 'gh api.*releases/latest'` returns ≥1 | PASS |
| `grep -c 'gh api.*git/refs/tags'` returns ≥1 | PASS (2) |
| `grep -c 'just update-hooks'` returns ≥1 | PASS (4) |
| `--dry-run` exits 0 | PASS |
| Output contains "Updating GitHub Actions SHA Pins" and "Updating Pre-commit Hooks" | PASS |
| No mutations to .github/workflows/ after dry-run | PASS |
| main() calls all 5 update functions in order | PASS |

### Task 5 — tests runner + summary, final end-to-end dry-run

| Criterion | Result |
|---|---|
| `bash -n` exits 0 | PASS |
| `grep -c '^run_tests_if_requested()'` returns 1 | PASS |
| `grep -c '^generate_summary()'` returns 1 | PASS |
| `grep -c 'just nextest'` returns ≥1 | PASS (4) |
| `grep -c 'Skeleton complete'` returns 0 (placeholder removed) | PASS |
| `wc -l` shows ≥400 lines | PASS (613) |
| `--help` contains all 5 flags | PASS |
| `--help` contains zero `--python` | PASS |
| `--dry-run` exits 0 | PASS |
| Dry-run output contains section headers in order | PASS |
| Dry-run output contains "Dry-run complete. No files were modified." | PASS |
| main() has exactly 7 function calls in order | PASS |

## Dry-Run Transcript (final, after all tasks)

Running `./scripts/update-project.sh --dry-run` from the worktree root produced the following section sequence (all INFO lines preserved, exit code 0, no file mutations):

```
Current branch: worktree-agent-a09cc95e

🚀  Cronduit Project Update
============================================================
Mode:        DRY-RUN
Major bumps: false
Backups:     true
Skip tests:  false

🦀  Updating Cargo Dependencies
============================================================
[DRY RUN] Would run: just update-cargo (refreshes Cargo.lock within constraints)
[DRY RUN] Would run: just openssl-check (Pitfall 14 — must stay empty)

🐳  Updating Dockerfile Base Images
============================================================
Current builder image: rust:1.94-slim-bookworm
Latest builder image:  rust:1.94.1-slim-bookworm
[DRY RUN] Would rewrite rust:1.94-slim-bookworm → rust:1.94.1-slim-bookworm in Dockerfile

🎨  Updating Tailwind Standalone Binary
============================================================
Current Tailwind: v3.4.17
Latest Tailwind:  v3.4.19 (filter: ^v3\.[0-9]+\.[0-9]+$)
[DRY RUN] Would bump justfile Tailwind version v3.4.17 → v3.4.19 and re-download binary

🎬  Updating GitHub Actions SHA Pins
============================================================
dataaxiom/ghcr-cleanup-action: already at v1.0.16 (cd0cdb9)

🪝  Updating Pre-commit Hooks
============================================================
No .pre-commit-config.yaml — skipping (Cronduit does not require pre-commit)
[DRY RUN] Would run: just nextest

🔍  Summary
============================================================
Dry-run complete. No files were modified.
```

Observations:

- The Dockerfile lookup correctly identified `rust:1.94-slim-bookworm` as current and `rust:1.94.1-slim-bookworm` as latest on Docker Hub.
- The Tailwind lookup correctly identified `v3.4.17` as current and `v3.4.19` as the newest v3.x tag (confirming that `--paginate` now walks past the first v4.x page).
- The GHA pin updater correctly discovered the only SHA-pinned action in Cronduit's workflows today (`dataaxiom/ghcr-cleanup-action` at `cd0cdb9 # v1.0.16`, from Plan 09-02) and confirmed it is already at the latest release.
- Pre-commit was correctly no-op'd because Cronduit has no `.pre-commit-config.yaml`.

## Plan-Level Verification

| # | Check | Result |
|---|---|---|
| 1 | `bash -n scripts/update-project.sh` exits 0 | PASS |
| 2 | `./scripts/update-project.sh --help` exits 0 and shows all 5 flags | PASS |
| 3 | `./scripts/update-project.sh --dry-run` exits 0 with clean git tree afterwards | PASS |
| 4 | `just --list` shows `update-cargo` and `update-hooks` recipes | PASS |
| 5 | `grep backups/ .gitignore` returns a match | PASS (line 91) |

## Success Criteria

- [x] `scripts/update-project.sh` exists, executable, 613 lines (≥400), dry-run passes
- [x] `justfile` has `update-cargo` and `update-hooks` recipes, no other changes
- [x] `.gitignore` has `backups/` entry
- [x] No changes to `src/`, `crates/`, `templates/`, `assets/`, `tests/`, `Cargo.toml` dependencies
- [x] Every must_have truth satisfied (verified individually in Task 5 final checks)

## Commits

| # | Hash | Scope | Message |
|---|------|-------|---------|
| 1 | `100042a` | Task 1 | `chore(09-03): add update-cargo/update-hooks recipes and backups/ to .gitignore` |
| 2 | `e43ea01` | Task 2 | `feat(09-03): add scripts/update-project.sh skeleton` |
| 3 | `735e3e8` | Task 3 | `feat(09-03): add cargo/dockerfile/tailwind ecosystem updaters` |
| 4 | `7a2633a` | Task 3 auto-fix | `fix(09-03): correct Dockerfile+Tailwind regex/pagination in updater` |
| 5 | `9a93acc` | Task 4 | `feat(09-03): add GHA pin updater + pre-commit hook updater` |
| 6 | `eb88c15` | Task 4 auto-fix | `fix(09-03): make GHA pin updater portable to bash 3.2 (macOS)` |
| 7 | `6a957eb` | Task 5 | `feat(09-03): add post-update test runner + summary generator` |

7 commits = 5 tasks + 2 Rule 1 auto-fixes, all on the `worktree-agent-a09cc95e` feature branch.

## Self-Check: PASSED

- `scripts/update-project.sh` — FOUND
- `justfile` — FOUND
- `.gitignore` — FOUND
- `.planning/phases/09-ci-cd-improvements/09-03-SUMMARY.md` — FOUND
- Commit `100042a` — FOUND
- Commit `e43ea01` — FOUND
- Commit `735e3e8` — FOUND
- Commit `7a2633a` — FOUND
- Commit `9a93acc` — FOUND
- Commit `eb88c15` — FOUND
- Commit `6a957eb` — FOUND
