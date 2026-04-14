---
phase: 9
phase_slug: ci-cd-improvements
gathered: 2026-04-13
status: ready_for_planning
source: user_directed_scope (ROADMAP.md Phase 9 block)
---

# Phase 9: CI/CD Improvements — Context

<domain>
## Phase Boundary

This phase is pure DevOps hygiene. It ships **no runtime code changes** and adds **no new product features**. Every artifact lives in `.github/workflows/`, `scripts/`, `justfile`, `Dockerfile`, and contributor-facing docs. The user-visible Cronduit binary is untouched.

The phase exists to prevent operational debt from accumulating in a long-lived OSS project: stale PR caches, unbounded GHCR image storage, manual dependency drift, and untapped GitHub Actions caching lanes.

</domain>

<decisions>
## Implementation Decisions (LOCKED by user)

### Plan 1 — PR cache cleanup workflow

**Source of inspiration:** <https://github.com/SimplicityGuy/discogsography/blob/main/.github/workflows/cleanup-cache.yml>

- File: `.github/workflows/cleanup-cache.yml`
- Trigger: `pull_request: { types: [closed] }`
- Concurrency: `cleanup-cache-${{ github.event.pull_request.number }}`, `cancel-in-progress: true`
- Job permissions: `actions: write` (and only that — least privilege)
- Body: enumerate caches whose ref is `refs/pull/<PR_NUM>/merge` via `gh cache list --ref "$BRANCH" --limit 100 --json id --jq ".[].id"`, then `gh cache delete` each one inside `set +e` so a missing cache doesn't fail the workflow
- Runs-on: `ubuntu-latest`, `timeout-minutes: 10`
- Adaptation: discogsography's version is essentially correct as-is. No matrix needed (Cronduit is a single-crate repo). Port it verbatim with the project name updated in any comments.

### Plan 2 — GHCR image cleanup workflow

**Source of inspiration:** <https://github.com/SimplicityGuy/discogsography/blob/main/.github/workflows/cleanup-images.yml>

- File: `.github/workflows/cleanup-images.yml`
- Triggers: `workflow_dispatch` AND `schedule: cron: "0 0 15 * *"` (15th of each month, 00:00 UTC)
- Concurrency: `cleanup-images-${{ github.ref }}`, `cancel-in-progress: false`
- Job permissions: `packages: write`, `contents: read`
- Action: `dataaxiom/ghcr-cleanup-action` — **MUST be pinned by full SHA** (discogsography uses `cd0cdb900b5dbf3a6f2cc869f0dbb0b8211f50c4 # v1.0.16`). Look up the latest release at plan time, pin to its SHA, and add a `# vX.Y.Z` comment after the SHA.
- Action inputs:
  - `delete-partial-images: true`
  - `delete-untagged: true`
  - `keep-n-tagged: 2`
  - `older-than: 30 days`
  - `token: ${{ secrets.GITHUB_TOKEN }}`
  - `package: cronduit` (single image — Cronduit publishes ONE image, not a matrix; collapse the `list-sub-projects` matrix from discogsography to a flat single-job structure)
  - `owner: ${{ github.repository_owner }}`
- Adaptation: drop the `list-sub-projects` job entirely and the matrix strategy. Single job, single package.

### Plan 3 — `scripts/update-project.sh`

**Source of inspiration:** <https://github.com/SimplicityGuy/discogsography/blob/main/scripts/update-project.sh>

A Cronduit-flavored adapter of the discogsography updater. Cronduit's relevant ecosystems are different from discogsography (no Python, no Node), so the script handles:

- **Cargo dependencies** — `cargo update` for lockfile-only minor/patch refresh; `cargo upgrade --incompatible` (from `cargo-edit`) followed by `cargo update` when `--major` is passed. Verify with `cargo build` + `cargo test` unless `--skip-tests`.
- **GitHub Actions pin updates** — scan all `.github/workflows/*.yml` for `uses: <owner>/<repo>@<sha> # vX.Y.Z` lines, look up the latest release tag for each repo via `gh api repos/<owner>/<repo>/releases/latest`, fetch the SHA for that tag, rewrite the file. (This is the actionlint-recommended pin-by-SHA pattern.)
- **Dockerfile base image refresh** — find `FROM <image>:<tag>` lines, look up the newest tag (digest) for each via `docker manifest inspect` or `gh api`, update.
- **Tailwind standalone binary version** — `assets/vendor/` carries a vendored Tailwind binary; the script bumps its version-of-record (likely a `TAILWIND_VERSION` constant in the Dockerfile or justfile) and re-downloads.
- **pre-commit hooks** — only if a `.pre-commit-config.yaml` exists; run `pre-commit autoupdate`.
- **`cargo tree -i openssl-sys` guard** — after any cargo update, re-run the Pitfall 14 check (must be empty). Fail loud if it isn't.

**Required option surface (must mirror discogsography):**
- `--dry-run` — print intended changes without modifying files
- `--major` — include major version upgrades
- `--no-backup` — skip the timestamped `backups/project-updates-<TS>/` directory
- `--skip-tests` — skip the post-update `cargo test` run
- `--help` / `-h`
- (Drop discogsography's `--python` flag entirely — Cronduit has no Python)

**Hard rules:**
- Refuses to run unless executed from the project root (check for `Cargo.toml` + a Cronduit-specific marker like `crates/` or the workspace root).
- Requires `cargo`, `git`, `gh`, `jq`, `curl` on PATH. `cargo upgrade` requires `cargo-edit` (script must check and print install instructions if missing).
- Delegates wherever a `just` recipe already exists. Per discogsography's pattern, `justfile` is the single source of truth for command definitions.
- **MUST commit to a feature branch, never to `main`.** Project memory captures "no direct commits to main" as a hard rule. The script either creates a `chore/update-deps-<TS>` branch upfront or refuses to run if currently on `main`. One atomic commit per ecosystem updated (cargo, actions, docker, tailwind, pre-commit) so the user can cherry-pick.
- Visual logging: discogsography uses emojis (🐍 🐳 ✅ etc.); Cronduit's CLAUDE.md says "Only use emojis if the user explicitly requests it." For this script, the **user is the script author** and the discogsography pattern explicitly uses emojis — emoji logging is approved for this one file.

**Out of scope (deferred / not applicable):**
- Python version updates — Cronduit has no Python
- Node/npm updates — Cronduit has no Node frontend
- `uv` version updates — N/A
- Security scan integration (`pip-audit`, OSV) — defer to a future "supply chain" phase

### Plan 4 — Workflow caching audit

**Existing workflows in this repo:** `.github/workflows/ci.yml`, `.github/workflows/release.yml` (only — verified at plan-phase time)

**Caching lanes that MUST be confirmed wired (or wired in this plan):**

| Lane | Tool | Key strategy | Notes |
|---|---|---|---|
| Cargo registry + index + target | `Swatinem/rust-cache@v2` | Default keying (Cargo.lock + rustc version + workflow + job + env) | Required on every job that runs `cargo` |
| Docker buildx layers | `docker/build-push-action@v6` with `cache-from: type=gha,scope=<job-name>` and `cache-to: type=gha,mode=max,scope=<job-name>` | Per-arch scope for parallel per-arch builds; single scope for single-step multi-platform builds with `mode=max` (rationale documented in `docs/CI_CACHING.md`) | Both `ci.yml` Docker build and `release.yml` |
| Tailwind standalone binary | `actions/cache@v4` | Key on `TAILWIND_VERSION` from justfile or Dockerfile | Avoid re-downloading 30 MB binary every run |
| `cargo-zigbuild` cross targets | `Swatinem/rust-cache@v2` per target triple | Key includes the target triple in the prefix | Only relevant on jobs that call `just build-arm64` |
| `cargo nextest` archive (if used) | `actions/cache` for `target/nextest/` | Optional, skip if nextest isn't used yet | Add only if it produces measurable speedup |

**Documentation deliverable:** new `docs/CI_CACHING.md` listing every cache, its key, what evicts it, and how to debug a cache miss. Linked from `CONTRIBUTING.md` (or `README.md` if no `CONTRIBUTING.md` exists).

**Out of scope:** rewriting workflows to a different runner type (still GitHub-hosted `ubuntu-latest`); changing the CI matrix (still `linux/amd64 + linux/arm64 × SQLite + Postgres`); migrating off `cargo-zigbuild`.

### Cross-cutting decisions

- **All workflow file edits MUST keep existing CI matrix shape and job names** unless the audit (Plan 4) explicitly identifies a job name change as required. Renaming jobs breaks branch protection rules.
- **Every new third-party action MUST be pinned by full commit SHA**, with a `# vX.Y.Z` trailing comment. This matches discogsography's hygiene and is the actionlint-recommended pattern.
- **Every new workflow MUST set `permissions:` at the job level** (least-privilege; never use the default token scope).
- **Every new workflow MUST set `timeout-minutes:`** on every job — a runaway gh API loop must not burn 6h of GHA budget.
- **No diagrams introduced in this phase use ASCII art.** Per project memory, every diagram is mermaid. Plan 4's `docs/CI_CACHING.md` likely needs a flow showing what triggers each cache invalidation — it must be mermaid.
- **All changes land on a feature branch via PR**, per project memory ("No direct commits to main").

### Claude's Discretion

- Exact wave structure for parallelization (planner decides)
- Whether `cleanup-cache.yml` and `cleanup-images.yml` ship in the same plan or separate plans (planner decides — they're independent, so probably separate)
- The exact set of `just` recipes to add/refactor in support of `update-project.sh`
- The format/structure of `docs/CI_CACHING.md` beyond the must-haves above
- Whether to bundle a smoke-test workflow that runs `update-project.sh --dry-run` on every PR to catch regressions in the script itself

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing CI/CD surface
- `.github/workflows/ci.yml` — current CI workflow; the caching audit (Plan 4) edits this in place
- `.github/workflows/release.yml` — current release workflow; the caching audit edits this in place
- `Dockerfile` — multi-stage build with `cargo-zigbuild` → distroless; Plan 3 reads the base image tag from here
- `justfile` — single source of truth for command definitions; Plan 3's script delegates to recipes here

### Project policy / decisions
- `CLAUDE.md` (project root) — Tech stack lock-ins, security posture, documentation rules (mermaid only), workflow rules (PR-only)
- `.planning/PROJECT.md` — locked decisions, security stance
- `THREAT_MODEL.md` (if present) — security context for any new workflow that touches secrets
- `.planning/STATE.md` — accumulated decisions from prior phases (especially the Phase 1 CI matrix decisions and the Phase 6 release-engineering decisions which Plan 4 must not regress)
- `.planning/MEMORY.md`-equivalent user feedback memories: "No direct commits to main", "Diagrams must be mermaid"

### Reference implementations (external — fetch with `gh api repos/SimplicityGuy/discogsography/contents/<path>` or via raw URL at plan time)
- `.github/workflows/cleanup-cache.yml` (discogsography) — Plan 1 verbatim source
- `.github/workflows/cleanup-images.yml` (discogsography) — Plan 2 source (de-matrix it)
- `scripts/update-project.sh` (discogsography) — Plan 3 source (drop Python/Node, keep structure)

### Pitfalls index (from research synthesis)
- `.planning/research/PITFALLS.md` — Pitfall 14 (`cargo tree -i openssl-sys` empty) is checked by Plan 3's update script post-update

</canonical_refs>

<specifics>
## Specific Ideas

- The discogsography `cleanup-cache.yml` has a clever `set +e` around the delete loop so a cache that's already gone (race with another delete or expiry) doesn't fail the whole workflow. **Preserve that.**
- The discogsography `cleanup-images.yml` keeps `keep-n-tagged: 2` and `older-than: 30 days`. These are sensible defaults for an OSS project that ships maybe 1-2 releases per month. Adopt them as-is.
- The discogsography `update-project.sh` uses `BACKUP_DIR="backups/project-updates-${TIMESTAMP}"`. Match this exactly so contributors who know the discogsography flow are immediately at home, and add `backups/` to `.gitignore` if it isn't already.
- The discogsography script's `print_section` uses ANSI `\033[1;36m` cyan — keep the visual style consistent.
- For pinning third-party actions by SHA, the standard tool is `pin-github-action` (npm) or doing it via `gh api`. The script should use `gh api` to avoid adding npm as a build dependency (Cronduit has none).

</specifics>

<deferred>
## Deferred Ideas (Out of Scope for Phase 9)

- **Supply-chain scanning** (`cargo audit`, `cargo deny`, `osv-scanner`) — worth its own phase later
- **Renovate / Dependabot integration** — `update-project.sh` is the manual lever; bot-driven updates are a v1.1+ decision
- **CodeQL / Semgrep workflow** — security-scanning is its own phase
- **Release signing (cosign / sigstore)** — release engineering, not CI hygiene
- **SBOM generation** — supply-chain phase
- **Workflow benchmarking dashboard** — interesting but not yet justified by build-time data
- **Switching off `ubuntu-latest` to a pinned `ubuntu-24.04`** — separate decision, would affect every workflow and deserves its own discussion

</deferred>

---

*Phase: 09-ci-cd-improvements*
*Context gathered: 2026-04-13 — direct user-directed scope, codified from ROADMAP.md Phase 9 block*
