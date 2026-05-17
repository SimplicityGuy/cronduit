---
phase: 24-milestone-close-out-final-v1-2-0-ship
reviewed: 2026-05-16T00:00:00Z
depth: standard
files_reviewed: 6
files_reviewed_list:
  - .github/workflows/ci.yml
  - MILESTONES.md
  - README.md
  - THREAT_MODEL.md
  - deny.toml
  - justfile
findings:
  critical: 1
  warning: 3
  info: 3
  total: 7
status: issues_found
---

# Phase 24: Code Review Report

**Reviewed:** 2026-05-16
**Depth:** standard
**Files Reviewed:** 6
**Status:** issues_found

## Summary

Phase 24 is an operational close-out: TM5/TM6 threat-model authoring, milestone audit, MILESTONES.md v1.2 entry, README v1.2 updates, and the cargo-deny WARN→ERROR promotion. The five documentation files (`THREAT_MODEL.md`, `MILESTONES.md`, `README.md`, `deny.toml`, `.github/workflows/ci.yml`) are well-executed — TM5/TM6 canonical structure matches the TM1–TM4 peer pattern, STRIDE rows T-S3/T-T4/T-I4/T-D4 are present, the cargo-deny gate promotion is correct, and the five Phase 24 license additions to `deny.toml` are legitimate SPDX-recognized identifiers with documented rationale and re-evaluate dates.

One **critical** finding in the `justfile`: the `uat-labels-merge` TOML fixture simultaneously sets both `image` and `command` on the same `[[jobs]]` block. The validator `check_one_of_job_type` rejects configs where `count != 1`, so `just check-config` will always exit non-zero — causing UAT Scenario 4a to fail immediately every time it is run. Three warnings cover a stale `deny.toml` comment that contradicts the file header, stale v1.1.0 references in the README Docker image tags section, and a parallel fixture defect in `uat-labels-reserved-namespace-error` that produces the right exit code but for a partially wrong reason.

---

## Critical Issues

### CR-01: `uat-labels-merge` TOML fixture sets both `image` and `command` — config always rejected

**File:** `justfile:1783-1789`
**Issue:** The TOML fixture written by `uat-labels-merge` declares `image = "alpine:latest"` and `command = "echo merged"` in the same `[[jobs]]` block. `src/config/validate.rs::check_one_of_job_type` counts `job.image.is_some() + job.command.is_some()` = 2 and emits the error:

> `[[jobs]] 'label-merge-test' must declare exactly one of 'command', 'script', or 'image' (found 2).`

The recipe body calls `just check-config .tmp/uat-labels-merge.toml` with `set -euo pipefail` active. Because `check-config` exits non-zero, the recipe aborts at that step. UAT Scenario 4a (custom Docker labels merge precedence) will **always fail** regardless of whether the labels feature itself works correctly.

**Fix:** Remove `command = "echo merged"` from the fixture — a `docker` job is identified by having `image` set (the `command` on a docker job is the container entrypoint override, which is optional and a distinct field from the command-type job's `command`). To confirm the merge-precedence scenario via `check-config`, set only `image`:

```toml
[[jobs]]
name = "label-merge-test"
schedule = "0 0 * * *"
image = "alpine:latest"
labels = { "com.example.owner" = "data-team", "com.example.team" = "infra" }
```

---

## Warnings

### WR-01: `uat-labels-reserved-namespace-error` fixture also sets both `image` and `command` — test passes accidentally

**File:** `justfile:1826-1832`
**Issue:** The fixture for Scenario 4b has the same `image + command` defect as CR-01. Because `check-config` exits non-zero (two errors: "found 2" AND "cronduit.*"), the recipe's `if just check-config ... ; then echo FAIL; fi` correctly detects failure, and the subsequent `grep -q "cronduit\."` finds the reserved-namespace error text in the combined error output alongside the "found 2" message. The test passes, but for a partially wrong reason: the exit is driven by `check_one_of_job_type` firing first, not solely by `check_label_reserved_namespace`. If `check_one_of_job_type` were ever made non-fatal (e.g., returning only a warning), the reserved-namespace check would still fire — but as authored, the fixture is misleading and fragile.

**Fix:** Remove `command = "echo reserved"` from the fixture. The validator fires on label keys regardless of whether the job type is otherwise valid:

```toml
[[jobs]]
name = "reserved-ns-test"
schedule = "0 0 * * *"
image = "alpine:latest"
labels = { "cronduit.job-name" = "operator-supplied" }
```

### WR-02: `deny.toml` `[bans]` comment contradicts file header — stale "Phase 24 will promote" promise

**File:** `deny.toml:95-102`
**Issue:** The `[bans]` block carries an old rc.1 comment:

> `Phase 24 will promote to "deny" with a curated skip = [...] allowlist for transitive duplicates we accept (e.g., the windows-sys / windows_x86_64_msvc families).`

and:

> `Empty in rc.1; populate in Phase 24 with curated skips.`

Phase 24 plan 24-05 (D-11 / CONTEXT D-10) explicitly decided to keep `multiple-versions = "warn"` and NOT promote bans to deny (the `ci.yml` comment updated to match: "Pairs with deny.toml's `bans.multiple-versions = "warn"` for layered defense (D-10)"). The file header at L3 correctly states "stays at 'warn' through Phase 24 close-out". The `[bans]` body comment is an unfulfilled, contradictory promise that will confuse a future reader doing the v1.3 bans promotion.

**Fix:** Replace the stale `[bans]` comment with a past-tense record of the Phase 24 decision:

```toml
[bans]
# Phase 24 D-10 decision: keep multiple-versions at "warn" (not promoted to "deny").
# Rationale: the ci.yml ERROR-gate on license/advisory/ban violations is the
# blocking layer; non-fatal duplicate-version warnings are the informational layer.
# If a duplicate-version finding needs to become blocking, add an entry to
# `skip = [...]` for duplicates we accept and flip to "deny" at a future milestone.
multiple-versions = "warn"
wildcards = "warn"
skip = []
skip-tree = []
```

### WR-03: README `§Docker image tags` table and mermaid diagram show stale v1.1.0 references

**File:** `README.md:103-118`
**Issue:** The `§Docker image tags` table states:

> `:latest` | The most recent stable (non-rc) release — **currently `:1.1.0`**

and:

> `:rc` | The most recent release candidate — **currently `:1.1.0-rc.6`** (last rc before `v1.1.0` shipped)

The mermaid diagram at L114-118 uses `v1.1.0-rc.N` and `v1.1.0` as the illustrative tag values. These will be visibly incorrect at the moment `v1.2.0` ships: a new operator reading the quickstart will see `:latest` claimed to be `1.1.0` while `docker pull ghcr.io/simplicityguy/cronduit:latest` delivers the v1.2.0 image. Plan 24-04's scope (D-13) did not cover this section, but the close-out PR is the natural place to correct it since it already touches `README.md`.

**Fix:** Update the table "currently" column values and the mermaid diagram to use `v1.2.0`:

```markdown
| `:latest` | The most recent stable (non-rc) release — currently `:1.2.0` | ...
| `:rc` | The most recent release candidate — currently `:1.2.0-rc.4` (last rc before `v1.2.0` ships) | ...
```

Mermaid diagram: replace `v1.1.0-rc.N` with `v1.2.0-rc.N` and `v1.1.0` with `v1.2.0` in the node labels.

---

## Info

### IN-01: `MILESTONES.md` v1.1 entry references `v1.1-MILESTONE-AUDIT.md` — file does not exist

**File:** `MILESTONES.md:25`
**Issue:** The v1.1 entry's **Audit** row says:

> see `.planning/milestones/v1.1-ROADMAP.md`, `.planning/milestones/v1.1-REQUIREMENTS.md`, `.planning/milestones/v1.1-MILESTONE-AUDIT.md` (archived by `/gsd-complete-milestone v1.1`)

`v1.1-MILESTONE-AUDIT.md` was never created (acknowledged in `v1.2-MILESTONE-AUDIT.md` § Tech Debt Summary as a known gap). A reader clicking through this reference will hit a missing file. This is a pre-existing gap carried forward; the v1.2 audit doc records it as "deferred-not-blocker."

**Fix:** Remove the non-existent file reference from the v1.1 entry or add a parenthetical noting it was not produced at v1.1 close:

```markdown
**Audit:** see `.planning/milestones/v1.1-ROADMAP.md`, `.planning/milestones/v1.1-REQUIREMENTS.md`
(v1.1 milestone audit doc was not produced — see v1.2-MILESTONE-AUDIT.md § Tech Debt)
```

Alternatively, create the missing `v1.1-MILESTONE-AUDIT.md` retroactively — though this is explicitly out of scope for P24 per CONTEXT § Deferred.

### IN-02: `uat-quickstart RC_TAG` — operator-supplied tag interpolated without quoting in bash shebang recipe

**File:** `justfile:1699,1703`
**Issue:** In the `#!/usr/bin/env bash`-shebang recipe body, `just` textually substitutes `{{RC_TAG}}` before the shell sees the script. The resulting lines are:

```bash
docker pull ghcr.io/simplicityguy/cronduit:{{RC_TAG}}
CRONDUIT_IMAGE=ghcr.io/simplicityguy/cronduit:{{RC_TAG}} \
```

If `RC_TAG` contains shell-special characters (`;`, `|`, `&&`, backticks, `$(...)`, spaces), the expansion becomes a shell injection. For example `just uat-quickstart "v1.2.0-rc.4; rm -rf .tmp"` would execute `rm -rf .tmp` in a `set -euo pipefail` shell. The threat model is limited (the maintainer runs this recipe, not a CI bot or untrusted user), but it is still a code quality defect and a potential footgun when copy-pasting tag strings from a release notes page that may contain formatting characters.

**Fix:** Validate or quote `RC_TAG` at the top of the recipe:

```bash
# Validate RC_TAG matches the expected version-tag pattern before using it.
if [[ ! "{{RC_TAG}}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-rc\.[0-9]+)?$ ]]; then
    echo "ERROR: RC_TAG '{{RC_TAG}}' does not match expected pattern vX.Y.Z[-rc.N]"
    exit 1
fi
RC_TAG_SAFE="{{RC_TAG}}"   # safe to use after validation
docker pull "ghcr.io/simplicityguy/cronduit:${RC_TAG_SAFE}"
CRONDUIT_IMAGE="ghcr.io/simplicityguy/cronduit:${RC_TAG_SAFE}" \
    docker compose -f examples/docker-compose.yml up -d
```

### IN-03: `deny.toml` `[sources]` `unknown-registry` and `unknown-git` are `"warn"` not `"deny"`

**File:** `deny.toml:110-111`
**Issue:** The `[sources]` block has:

```toml
unknown-registry = "warn"
unknown-git = "warn"
```

For a project that explicitly restricts to crates.io only (`allow-registry = [...]`, `allow-git = []`), any crate from an unknown registry or git source should be a hard error, not a warning. An unknown-registry dep that slips into the tree under `"warn"` will pass the now-blocking `cargo deny check` and reach production. This is a pre-existing posture from rc.1 and was not changed in Phase 24's scope, but the promotion of cargo-deny to blocking makes this worth flagging.

**Fix:** Promote both to `"deny"` at the next opportunity (v1.3 milestone close or as a standalone hygiene PR):

```toml
unknown-registry = "deny"
unknown-git = "deny"
```

---

_Reviewed: 2026-05-16_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
