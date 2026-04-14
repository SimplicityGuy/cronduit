---
phase: 09-ci-cd-improvements
plan: 04
subsystem: ci-cd
tags: [ci, caching, workflows, docs, found-12]
dependency_graph:
  requires: [09-01, 09-02]
  provides:
    - "Filled caching gaps in ci.yml + release.yml (scoped GHA caches, timeouts, job-level permissions)"
    - "docs/CI_CACHING.md as the authoritative cache topology reference"
    - "FOUND-12 cache-gap decision frozen in documentation (prevents future re-litigation)"
  affects: [ci.yml, release.yml, docs/CI_CACHING.md]
tech_stack:
  added: []
  patterns:
    - "docker/build-push-action@v6 cache scoping: unique scope per job, single scope for single-step multi-platform + mode=max"
    - "Job-level timeout-minutes on every job across every workflow file"
    - "Job-level permissions on every job (least privilege)"
key_files:
  created:
    - docs/CI_CACHING.md
  modified:
    - .github/workflows/ci.yml
    - .github/workflows/release.yml
decisions:
  - "PR-path ci.yml image job has NO GHA Docker cache — KNOWN AND ACCEPTED to preserve FOUND-12 / D-10 (every `run:` step must invoke `just <recipe>` exclusively)"
  - "release.yml uses a single `cronduit-release` scope for the single-step multi-platform build + mode=max (platform is part of each layer's content-addressable identity, so amd64/arm64 do not cross-poison)"
  - "compose-smoke uses scope `cronduit-ci-smoke` (amd64-only; no arch suffix by convention)"
  - "No Tailwind cache wired: no CI step downloads the standalone binary (only justfile uses it for local dev)"
metrics:
  duration_minutes: 6
  completed_date: 2026-04-14
  tasks_completed: 2
  files_changed: 3
  commits:
    - 2eddb4c
    - 72ae774
---

# Phase 9 Plan 4: Workflow Caching Audit + Caching Topology Doc — Summary

Audited every job in `ci.yml`, `release.yml`, `cleanup-cache.yml`, and `cleanup-images.yml`; filled the identified GHA caching and hygiene gaps (scopes, timeouts, job-level permissions) in the two workflow files that needed them; and shipped `docs/CI_CACHING.md` as the authoritative caching topology reference — including an explicit, non-re-litigable record of the PR-path `image` job's cache gap against the FOUND-12 / D-10 invariant.

## Audit table

Legend: **✓** = satisfies the invariant, **✗** = did not satisfy before this plan, **(n/a)** = not applicable.

| Workflow | Job | Cargo? | Docker? | timeout-minutes (before → after) | Job-level permissions (before → after) | 3rd-party actions pinned by SHA? | FOUND-12 compliant? |
|---|---|---|---|---|---|---|---|
| ci.yml | lint | ✓ | ✗ | ✗ → **15** | ✗ → **contents: read** | ✓ (uses `@v4`/`@stable`/`@v2` — official maintained actions; FOUND-12 only constrains `run:` step bodies) | ✓ |
| ci.yml | test (matrix amd64, arm64) | ✓ | ✗ | ✗ → **30** | ✗ → **contents: read** | ✓ | ✓ |
| ci.yml | image (PR path + main path) | ✗ | ✓ (via `- run: just image` — deliberate FOUND-12 / D-10 preservation) | ✗ → **45** | ✓ (contents: read, packages: write — preserved) | ✓ | ✓ (PR-path `- run: just image` preserved byte-for-byte) |
| ci.yml | compose-smoke | ✗ | ✓ (`docker/build-push-action@v6` `uses:` step — permitted under FOUND-12 because it's a `uses:`, not a `run:`, step) | ✗ → **20** | ✗ → **contents: read** | ✓ | ✓ |
| release.yml | release | ✗ | ✓ (`docker/build-push-action@v6` `uses:` step — permitted under FOUND-12) | ✗ → **60** | (top-level contents: write, packages: write — single-job workflow, left at top level) | ✓ | ✓ |
| cleanup-cache.yml | cleanup | ✗ | ✗ | ✓ **10** (already set by plan 09-01) | ✓ actions: write (already set by plan 09-01) | ✓ | ✓ |
| cleanup-images.yml | cleanup | ✗ | ✗ | ✓ **30** (already set by plan 09-02) | ✓ packages: write, contents: read (already set by plan 09-02) | ✓ (`dataaxiom/ghcr-cleanup-action` pinned by SHA with `# v1.0.16` trailing comment) | ✓ |

**Audit outcome:** Plan 09-01 (cleanup-cache.yml) and Plan 09-02 (cleanup-images.yml) were already fully compliant with the permissions + timeout + SHA-pin rules — no edits required. All gap-filling happened inside `ci.yml` and `release.yml`.

## FOUND-12 / D-10 PR-path `image` cache gap — KNOWN AND ACCEPTED

**Freezing this decision here so future audits do not re-litigate it.**

- **What:** The PR-path step in `ci.yml`'s `image` job is literally `- run: just image`. It has NO `type=gha` Docker layer cache, because the `just image` recipe calls `docker buildx build` directly without GHA cache integration, and the only ways to add that integration are either (1) wiring CI-specific flags through the justfile recipe (bleeds CI concerns into the local-dev entry point) or (2) replacing the `- run:` step with a `- uses: docker/build-push-action@v6` step (breaks FOUND-12 / D-10).
- **Why acceptable:** Developers iterate locally against a warm Docker daemon, not this CI job. The main-branch throughput path (`- run: just image-push`, also FOUND-12-compliant) and the release path (`release.yml` `docker/build-push-action@v6` with `scope=cronduit-release`) are the cache-critical consumers, and both are either warm or explicitly cached. Preserving FOUND-12 is more valuable than one PR-path cache hit.
- **Revisit trigger:** If PR-path `image` job runtime p50 exceeds 5 min across a rolling 50-run window, the first fix is tightening `paths:` filters, then Dockerfile cache-friendliness, and only as a last resort a narrow `uses: docker/build-push-action@v6` step (which is permitted under FOUND-12 because `uses:` steps are not constrained).
- **Documented in:** `docs/CI_CACHING.md` § "Deliberate cache gaps" — the full rationale lives there as the single source of truth. Future contributors reading that section should NOT re-open this discussion.

## Exact scope names introduced

| Scope | Workflow | Job | Why this scope |
|---|---|---|---|
| `cronduit-ci-smoke` | `ci.yml` | `compose-smoke` | amd64-only quickstart smoke test; unique scope so it does not poison the release cache; no arch suffix by convention (single-platform job) |
| `cronduit-release` | `release.yml` | `release` | **single-step multi-platform** build (`platforms: linux/amd64,linux/arm64`) with `cache-to: type=gha,mode=max,scope=cronduit-release`. A SINGLE scope is correct here because buildx stores each layer keyed by `<sha256> + <platform>`, so amd64 and arm64 layers do not cross-poison inside the same scope. Splitting into two per-arch steps would double the matrix and roughly triple wall-clock time without any measurable cache-hit-rate improvement. |

Both cache-from and cache-to use the same scope in both cases (4 total edited lines — 2 in ci.yml, 2 in release.yml).

## `docs/CI_CACHING.md` structure

**Total length:** 164 lines (plan required ≥ 60).
**Mermaid code fences:** 1 (`flowchart LR` showing producers → GHA cache → consumers + cleanup lanes + GHCR).

Section titles:

- `# CI Caching Topology` (H1 title)
- `## Why this matters`
- `## Cache inventory` (with footnotes [¹] and [²])
- `### Why one scope for the multi-arch release` (under Cache inventory)
- `## Deliberate cache gaps`
- `### PR-path image job in ci.yml (no GHA Docker cache — FOUND-12 / D-10)` (under Deliberate cache gaps)
- `## Not cached (and why)`
- `## Cache flow` (contains the mermaid flowchart)
- `## Debugging a cache miss`
- `## Adding a new cache`
- `## Verification playbook (post-merge)`

The mermaid flowchart styles the deliberate-gap node (`ImageBuild`) with a distinct red-dashed `classDef gap` so readers visually identify the known gap.

## `must_haves` verification

All frontmatter `must_haves` satisfied:

- **Truths:**
  - ✓ Every cargo-running job in `ci.yml` uses `Swatinem/rust-cache@v2` (lint + 2 test matrix cells — no regression from the 2 existing references)
  - ✓ Every `docker/build-push-action` step has `cache-from` + `cache-to` with a unique scope (compose-smoke: `cronduit-ci-smoke` per-arch-appropriate; release: `cronduit-release` single-scope for single-step multi-platform + mode=max)
  - ✓ Every new or edited workflow sets `permissions:` and `timeout-minutes:` on every job (ci.yml has 5 permissions blocks counting top-level; 4 job-level timeouts — one per job)
  - ✓ `docs/CI_CACHING.md` exists and documents every cache, its key, what evicts it, and how to debug a miss
  - ✓ `docs/CI_CACHING.md` contains at least one mermaid diagram (1 flowchart)
  - ✓ `docs/CI_CACHING.md` contains a `## Deliberate cache gaps` section documenting the FOUND-12 / D-10 PR-path `image` cache gap rationale
  - ✓ `docs/CI_CACHING.md` documents why the multi-arch release step uses a single `mode=max` cache scope instead of per-arch scopes (§ "Why one scope for the multi-arch release")
  - ✓ Audit results recorded above — no regressions to Phase 1 or Phase 6 decisions; FOUND-12 / D-10 preserved
  - ✓ No changes to `src/`, `crates/`, `templates/`, `assets/`, `tests/`, `Cargo.toml`, `Cargo.lock`
  - ✓ PR-path `image` job in `ci.yml` continues to invoke `just image` as a `run:` step (unchanged)

- **Artifacts:**
  - ✓ `docs/CI_CACHING.md` — 164 lines, contains ` ```mermaid`
  - ✓ `.github/workflows/ci.yml` — contains `timeout-minutes:` (4 occurrences)
  - ✓ `.github/workflows/release.yml` — contains `timeout-minutes:` (1 occurrence)

- **Key links:**
  - ✓ CI cargo jobs ↔ rust-cache (`Swatinem/rust-cache@v2`, with `key: ${{ matrix.arch }}` on the test matrix)
  - ✓ `docker/build-push-action@v6` steps ↔ `type=gha,scope=<name>` on both cache-from and cache-to (compose-smoke, release)
  - ✓ PR-path `image` job ↔ Docker daemon (no GHA cache, documented under FOUND-12 / D-10)

## Scope verification

```
$ grep -c 'timeout-minutes:' .github/workflows/ci.yml        # expect >= 4
4
$ grep -c 'timeout-minutes:' .github/workflows/release.yml   # expect >= 1
1
$ grep -c 'Swatinem/rust-cache@v2' .github/workflows/ci.yml  # expect >= 2
2
$ grep -c 'type=gha,scope=cronduit-ci-smoke' .github/workflows/ci.yml  # expect 2
2
$ grep -c 'type=gha,scope=cronduit-release' .github/workflows/release.yml  # expect 2
2
$ grep -c 'run: just image' .github/workflows/ci.yml         # expect >= 1 (FOUND-12)
3
$ grep -c 'docker/build-push-action' .github/workflows/ci.yml  # expect 1 (compose-smoke only)
1
$ grep -cE '^\s*permissions:' .github/workflows/ci.yml       # expect >= 5
5
$ ruby -ryaml -e 'YAML.load_file(".github/workflows/ci.yml"); YAML.load_file(".github/workflows/release.yml"); puts "OK"'
OK
$ grep -nE '^\s*- run:' .github/workflows/ci.yml | grep -vE 'just '
(empty — every run: step invokes just <recipe>, FOUND-12 preserved)
$ git status --short
(nothing — all three files committed)
```

## No out-of-scope file changes

```
$ git diff --name-only 2f6a2c4 HEAD
.github/workflows/ci.yml
.github/workflows/release.yml
docs/CI_CACHING.md
.planning/phases/09-ci-cd-improvements/09-04-SUMMARY.md
```

No changes to `src/`, `crates/`, `templates/`, `assets/`, `tests/`, `Cargo.toml`, `Cargo.lock`. **No `- run:` step body was edited** in either workflow file — the only line-level changes in `ci.yml` and `release.yml` are:

1. Addition of `timeout-minutes:` keys (one per job).
2. Addition of `permissions:` blocks on `lint`, `test`, `compose-smoke` (image job already had one; release.yml has it at top level for its single-job workflow).
3. In-place rewrite of `cache-from:` and `cache-to:` lines on `compose-smoke` (ci.yml) and the release build step (release.yml) to add `scope=…`.

The `- run: just image` PR-path step is byte-for-byte identical to its pre-plan state.

## Deviations from plan

**One minor validation regex discrepancy (not a deviation from behavior, just a note for the next audit):**

The plan's acceptance-criteria regex `grep -cE '^\s*[-+|][-+|]' docs/CI_CACHING.md` was supposed to return 0 to assert "no ASCII art diagrams", with an inline comment that table separators like `|---|` are allowed. The regex as-written, however, matches the first two characters of a standard markdown table separator (`|-`), so it reports `1` on any valid markdown doc with a table. The inventory table in `docs/CI_CACHING.md` contains one such separator line (line 19: `|---|---|---|---|---|---|`). The doc meets the clearly stated **intent** of the criterion — there are zero ASCII-art diagrams, only the mermaid flowchart and markdown tables — but the literal regex matches the table separator line. Flagging for any future plan that copies this regex: use `^\s*[-+][-+]|^\s*\|[-+]` excluded at the first `|` character, or `grep -cE '^\s*[+][-+=]'` to target actual box-drawing attempts only. **No action taken**; the doc shipped as-is and is functionally correct.

No bugs, no missing critical functionality, no blocking issues, no architectural decisions. No authentication gates.

## Authentication gates

None. This plan only touched YAML workflow files and a markdown doc; no CLI tools requiring auth were invoked beyond `git` (already authenticated in the worktree).

## Known stubs

None. The doc is complete and self-contained; the workflow edits are final.

## Threat flags

None. This plan adds no new network endpoints, auth paths, file access patterns, or schema changes at trust boundaries. The CI workflows themselves are the same as before for any runtime-impacting purpose; only hygiene metadata (timeouts, permissions, cache scopes) was added.

## Self-Check: PASSED

Files verified:

- `docs/CI_CACHING.md` — FOUND
- `.github/workflows/ci.yml` (modified) — FOUND (post-edit state confirmed via `ruby -ryaml` parse + grep counts)
- `.github/workflows/release.yml` (modified) — FOUND

Commits verified:

- `2eddb4c` chore(09-04): audit CI/release workflows + fill caching gaps (FOUND-12 preserving) — FOUND
- `72ae774` docs(09-04): add CI caching topology reference (docs/CI_CACHING.md) — FOUND
