---
phase: 24-milestone-close-out-final-v1-2-0-ship
plan: 07
subsystem: docs+ci-paperwork
tags: [uat, runbook, milestone-close-out, v1.2, just-recipes]
dependency_graph:
  requires:
    - 24-06 (24-RC4-PREFLIGHT.md — rc.4 image published before UAT runs)
    - .planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-HUMAN-UAT.md (structural mirror target)
    - .planning/milestones/v1.1-phases/14-bulk-enable-disable-rc-3-final-v1-1-0-ship/14-CONTEXT.md (v1.1 final-ship UAT precedent)
    - justfile (host of the new uat-quickstart / uat-regression-v1x / uat-labels-* recipes)
  provides:
    - .planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-HUMAN-UAT.md (maintainer-EXECUTES six-scenario UAT runbook)
    - justfile new recipes: uat-quickstart RC_TAG, uat-regression-v1x, uat-labels-merge, uat-labels-reserved-namespace-error
  affects:
    - 24-08 (24-FINAL-SHIP-PREFLIGHT.md reads the Final sign-off block to confirm UAT passed before retagging the rc.N SHA as v1.2.0)
    - ROADMAP Phase 24 success criterion #5 (docker compose up healthy + dashboard renders + webhook delivers + no v1.0/v1.1 regressions)
tech_stack:
  added: []
  patterns:
    - "Composing `just check-config` inside recipe bodies rather than re-inlining `cargo run -- check`"
    - "Parameterized recipe `uat-quickstart RC_TAG` so iterated rc.N reuses the same recipe (Strategy a — CRONDUIT_IMAGE env var override; examples/docker-compose.yml:72 already consumes the var)"
    - "Mirror P22/P23 recipe shape (prompt + validate + maintainer y/n gate)"
    - "Mirror 23-HUMAN-UAT.md six-scenario shape (Goal / Steps / Eyeball criteria / Sign-off) with v1.2 substitution"
key_files:
  created:
    - .planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-HUMAN-UAT.md
    - .planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-07-SUMMARY.md
  modified:
    - justfile (added 180 lines at L1678+ — four new recipes; existing compose-up-rc3 at L921 left untouched)
decisions:
  - "Strategy (a) for uat-quickstart: parameterize CRONDUIT_IMAGE env var; no edit to examples/docker-compose.yml; existing compose-up-rc3 (v1.1 historical artifact) left untouched"
  - "Compose `just check-config` inside uat-labels-* recipe bodies rather than re-inlining `cargo run -- check` — keeps the abstraction layer consistent with the `feedback_uat_use_just_commands.md` rule at both runbook AND recipe-internal levels"
  - "uat_coverage_requirements list (38 v1.2 feature REQ-IDs) lives in 24-HUMAN-UAT.md frontmatter as a UAT-coverage marker, NOT in 24-07-PLAN.md frontmatter (per checker W6 — P24 itself owns no REQ-IDs)"
  - "FOUND-14..16 (cargo-deny + CI-hygiene) deliberately excluded from uat_coverage_requirements — those are not user-observable, so they cannot be UAT-exercised"
metrics:
  duration_seconds: 404
  completed: 2026-05-17T02:39:18Z
  tasks_completed: 2
  files_changed: 3
---

# Phase 24 Plan 07: 24-HUMAN-UAT.md — Maintainer Runbook for Full v1.2.0 Close-Out UAT Summary

Six-scenario maintainer-EXECUTES UAT runbook (`24-HUMAN-UAT.md`, 359 lines) covering full v1.2 regression smoke + all five v1.2 features end-to-end against the `:v1.2.0-rc.4` image, plus four new `just` recipes (`uat-quickstart RC_TAG`, `uat-regression-v1x`, `uat-labels-merge`, `uat-labels-reserved-namespace-error`) appended to `justfile` so every numbered runbook step invokes a `just` recipe per project memory `feedback_uat_use_just_commands.md`.

## What shipped

**Task 1 — `justfile` recipe additions (commit `bba11c5`):**

- **`uat-quickstart RC_TAG`** (`justfile:1693`) — parameterized compose-up against the rc.N image. Strategy (a) chosen from plan 24-07's three-strategy analysis: sets `CRONDUIT_IMAGE` env var which `examples/docker-compose.yml:72` already consumes as `image: ${CRONDUIT_IMAGE:-ghcr.io/simplicityguy/cronduit:latest}`. No edit to `examples/docker-compose.yml` required. Existing `compose-up-rc3` recipe at `justfile:921` left untouched (v1.1 historical artifact — preserves `git blame` lineage).
- **`uat-regression-v1x`** (`justfile:1735`) — operator-eyeball walkthrough harness for the nine v1.0/v1.1 surfaces (filter / sort / Run Now / Stop / bulk toggle / timeline / sparklines / settings overrides / healthcheck). No new fixtures; assumes cronduit running from `uat-quickstart`.
- **`uat-labels-merge`** (`justfile:1769`) — writes `.tmp/uat-labels-merge.toml` with overlapping `[defaults]` + per-job labels (defaults: `com.example.env=prod, com.example.owner=platform`; per-job: `com.example.owner=data-team, com.example.team=infra`), then invokes `just check-config` to validate. Asserts per-job-wins merge precedence (LBL-03).
- **`uat-labels-reserved-namespace-error`** (`justfile:1815`) — writes a fixture with `cronduit.job-name` label inside the reserved namespace, invokes `just check-config`, asserts it FAILS with a `cronduit.*` error string (auto-greps the log; exits non-zero if missing).

All four recipe BODIES compose `just check-config` (existing recipe at `justfile:877`) where applicable rather than re-inlining `cargo run -- check`. This keeps the `feedback_uat_use_just_commands.md` rule honored at both the runbook-step layer (the maintainer runs `just <recipe>`) AND the recipe-internal layer (the recipe wraps another `just` recipe instead of bare cargo invocations).

`just --list` confirms all four recipes parse and surface; existing `compose-up-rc3` recipe still present at L921 with the rc.3 pin intact.

**Task 2 — `.planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-HUMAN-UAT.md` (commit `a790ff2`):**

- **Frontmatter:** `autonomous: false` + `maintainer_validated: true` + `rc_tag: v1.2.0-rc.4` + `uat_coverage_requirements:` listing all 38 v1.2 feature REQ-IDs (WH-01..11, LBL-01..06, FCTX-01..07, EXIT-01..06, TAG-01..08). FOUND-14..16 deliberately excluded (cargo-deny + CI-hygiene are not user-observable).
- **Preamble:** maintainer-validated framing + image-under-test reference + six-scenario summary + fail-iterate-to-rc.N policy + `feedback_uat_use_just_commands.md` + `feedback_uat_user_validates.md` enforcement.
- **Prerequisites:** Phase 24 close-out PR merged + `24-RC4-PREFLIGHT.md` sections 1-6 ticked + `just --list` shows the four new recipes + browser with DevTools available + screen reader available for Scenario 5.
- **Six scenarios** (each with canonical Goal / Steps / Eyeball criteria / Sign-off shape mirroring `23-HUMAN-UAT.md`):
  1. **Scenario 1 — `docker compose up` quickstart on rc.4 + dashboard renders.** `just uat-quickstart v1.2.0-rc.4` → 90s healthcheck → dashboard renders without errors. Closes ROADMAP P24 success criterion #5.
  2. **Scenario 2 — v1.0/v1.1 surfaces intact.** `just uat-regression-v1x` walks the nine surfaces. First time the full v1.2 stack receives a single-session smoke test against v1.0/v1.1 surfaces.
  3. **Scenario 3 — Webhooks end-to-end.** 10-step chain: `uat-webhook-mock` + `uat-webhook-fire` + `uat-webhook-verify` (Standard-Webhooks-v1 + HMAC-SHA256 payload) → `uat-webhook-mock-500` + `uat-webhook-retry` (3 attempts at t≈0/30s/300s full-jitter) → `uat-webhook-drain` (30s graceful drain) → `uat-webhook-rustls-check` (rustls verifier) → `uat-webhook-https-required` (plain-HTTP rejected for non-loopback) → `uat-webhook-metrics-check` (`cronduit_webhook_*` metrics on `/metrics`).
  4. **Scenario 4 — Custom Docker labels.** `uat-labels-merge` (per-job-wins merge precedence) + `uat-labels-reserved-namespace-error` (`cronduit.*` validator).
  5. **Scenario 5 — FCTX panel + exit histogram + a11y.** `uat-fctx-panel` (5 P1 signals collapsed-by-default) + `uat-exit-histogram` (10 buckets + status-discriminator-wins classifier + top-3 tie-break per Phase 21 D-08) + `uat-fctx-a11y` (recipe's 4-phase walkthrough: mobile / light mode / print / keyboard-only). Adds **four observable a11y criteria (e1-e4)** beyond the recipe: Tab focus order reaches the FCTX summary; `aria-expanded` flips on toggle (verified via DevTools Accessibility tree); no keyboard trap (Tab/Shift-Tab moves past the panel); `prefers-reduced-motion` honored (no animation when OS reduce-motion enabled). Closes checker W1.
  6. **Scenario 6 — Job tagging + filter chips.** Six-recipe chain: `uat-tags-persist` (sorted-canonical JSON column shape) + `uat-tags-validators` (charset / reserved / empty / per-job-cap reject) + `uat-chips-render` (alphabetical chip strip + empty-state hidden) + `uat-chips-and-filter` (AND across chips + AND with name-filter + untagged-hidden when filter active) + `uat-chips-share-url` (bookmarkable URL canonicalization + stale-tag silent-drop) + `uat-tags-webhook` (tags in webhook payload — WH-09 / TAG-08).
- **Failure-iteration block** (`## If UAT fails on any scenario`): documents the rc.N+1 iteration loop — capture finding → follow-up close-out PR on feature branch (per `feedback_no_direct_main_commits.md`) → cut next rc per `docs/release-rc.md` → re-run runbook → plan 24-08 retags LAST passing-UAT SHA per Phase 14 D-16.
- **Final sign-off block:** maintainer attestation paragraph covering both v1.2 features and v1.0/v1.1 regression surfaces + Maintainer name / Date / RC tag UAT-validated placeholders.

## How verified

Automated plan-spec verification (`24-07-PLAN.md` Task 2 `<verify><automated>`):
- File exists at the canonical path.
- `autonomous: false` in frontmatter.
- `rc_tag: v1.2.0-rc.4` reference present.
- `uat_coverage_requirements:` frontmatter key present.
- Six scenarios (`## Scenario 1` through `## Scenario 6`) all present.
- 20 numbered steps start with `Run \`just uat-...\`` (plan minimum: ≥ 8).
- Negative grep: zero numbered steps starting with `Run \`cargo\`` / `Run \`docker\`` / `Run \`curl\``.
- `## Final sign-off` block present.
- `aria-expanded` mention present (closes W1 a11y observable e2).

`just --list 2>&1 | grep -E "uat-quickstart|uat-regression-v1x|uat-labels-merge|uat-labels-reserved-namespace-error" | wc -l` returns `4` (plan Task 1 `<verify><automated>` passes).

Sanity: `just check-config /dev/null` exits non-zero with a typed error (confirms the composed-into recipe still resolves correctly post-changes).

## Decisions

- **Strategy (a) for `uat-quickstart`:** parameterize `CRONDUIT_IMAGE` env var rather than (b) generating a temporary compose override file or (c) renaming the existing `compose-up-rc3` recipe. `examples/docker-compose.yml:72` already consumes `${CRONDUIT_IMAGE:-…}` so the env-var path works with zero compose-file edits. Renaming `compose-up-rc3` would destroy the v1.1 P14 historical artifact `git blame` lineage (rejected). Generating a temporary compose override file adds unnecessary file I/O when the env var works (rejected).
- **Compose `just check-config` inside recipe bodies:** keeps the `feedback_uat_use_just_commands.md` rule honored at BOTH layers — the maintainer runs a `just` recipe at the runbook level, AND the recipe internally invokes another `just` recipe rather than bare `cargo run -- check`. Avoids `cargo` invocation drift if `check-config` ever gains new flags (single source of truth at `justfile:877`).
- **`uat_coverage_requirements` list in `24-HUMAN-UAT.md` frontmatter (not `24-07-PLAN.md`):** Phase 24 itself owns no REQ-IDs (those shipped in P15-23 and were flipped to `[x]` by plan 24-02). The 38-ID coverage list lives in the UAT runbook as a coverage marker per plan 24-07 checker W6.
- **FOUND-14..16 excluded from `uat_coverage_requirements`:** cargo-deny + CI-hygiene requirements have no user-observable surface; they cannot be UAT-exercised (closed by plan 24-05's CI workflow toggle, not by maintainer eyeballing).
- **Four observable a11y criteria (e1-e4) in Scenario 5 step 8:** Tab focus order, `aria-expanded` announcement (verified via DevTools Accessibility tree, not just `outerHTML` inspection), no keyboard trap (Tab/Shift-Tab past the panel), and `prefers-reduced-motion` honored. This closes plan 24-07 checker W1 (which flagged the previous draft's a11y verification as too vague).

## Deviations from Plan

None — plan executed exactly as written. The plan was unusually prescriptive (full recipe bodies + full scenario markdown were inlined into the `<action>` blocks), so the executor's work was almost entirely transcription + targeted micro-adjustments for readability (e.g., adding `[group('release')]` rather than `[group('uat')]` for the `uat-quickstart` recipe because it composes a release-flow primitive — `docker compose up` against the published GHCR image — which matches the existing `compose-up-rc3` recipe at `justfile:919-921` that also lives in `[group('release')]`).

The `uat-labels-merge` and `uat-labels-reserved-namespace-error` recipes were also assigned to `[group('release')]` so they cluster with `uat-quickstart` and `uat-regression-v1x` rather than splitting across `[group('uat')]` (which holds the per-feature P18-P23 recipes). All four plan-24-07 recipes are close-out / release-gate semantics, not per-feature UAT scaffolds — grouping them under `[group('release')]` matches the conceptual layer.

## Threat Flags

None. Plan was doc-authoring + four `just` recipes that wrap existing primitives (`docker pull` / `docker compose` / `just check-config`). The two new TOML fixtures (`uat-labels-merge.toml` and `uat-labels-reserved.toml`) contain NO secrets — only label key/value pairs and a single intentionally-failing label namespace. Fixtures land under `.tmp/` (gitignored). No new network surface, no new auth paths, no schema changes. Matches the plan's `<threat_model>` register (T-24-07-DOC: accept; T-24-07-SECRET: mitigate via gitignored `.tmp/`).

## Commits

| Task | Type | Hash | Files |
|------|------|------|-------|
| 1 | chore | `bba11c5` | `justfile` (+180 lines: 4 new recipes at L1678+) |
| 2 | docs | `a790ff2` | `.planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-HUMAN-UAT.md` (+359 lines, new file) |

## Self-Check: PASSED

- [x] `justfile` modified with `uat-quickstart RC_TAG` + `uat-regression-v1x` + `uat-labels-merge` + `uat-labels-reserved-namespace-error` (verified via `just --list`).
- [x] `compose-up-rc3` still present at `justfile:921` with rc.3 pin intact (verified via grep).
- [x] `24-HUMAN-UAT.md` exists at `.planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-HUMAN-UAT.md`, 359 lines.
- [x] Frontmatter has `autonomous: false`, `rc_tag: v1.2.0-rc.4`, `uat_coverage_requirements` with 38 REQ-IDs.
- [x] Six scenarios all present (`## Scenario 1` through `## Scenario 6`).
- [x] 20 numbered steps starting with `Run \`just uat-...\`` (well above plan minimum of 8).
- [x] Negative grep: zero numbered steps starting with `Run \`cargo\``/`Run \`docker\``/`Run \`curl\``.
- [x] Scenario 5 step 8 has four observable a11y criteria (e1-e4: Tab focus order, `aria-expanded` announcement, no keyboard trap, reduced motion) — closes checker W1.
- [x] `## Final sign-off` block present with Maintainer name / Date / RC tag UAT-validated placeholders.
- [x] Task 1 commit `bba11c5` found in `git log`.
- [x] Task 2 commit `a790ff2` found in `git log`.
- [x] No STATE.md or ROADMAP.md modifications (per parallel-executor brief — orchestrator owns those writes after the worktree merges back).
