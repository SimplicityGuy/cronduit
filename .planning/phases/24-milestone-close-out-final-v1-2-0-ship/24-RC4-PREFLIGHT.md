---
phase: 24
plan: 06
type: rc-preflight
autonomous: false
rc_tag: v1.2.0-rc.4
created: 2026-05-16
status: pending-maintainer-execution
---

# Phase 24 — v1.2.0-rc.4 Tag Cut Pre-Flight

**Phase:** 24 — Milestone Close-Out — final v1.2.0 ship
**Tag target:** `v1.2.0-rc.4`
**Runbook:** [docs/release-rc.md](../../../docs/release-rc.md) — REUSED VERBATIM (D-10). Phase 24 makes NO edits to this runbook.
**Cargo.toml version:** `1.2.0` (set in Phase 15; the `-rc.4` is tag-only per `feedback_tag_release_version_match.md` and D-18).

> This artifact is a maintainer-validated checklist. Claude authored it; the maintainer runs through it and cuts the tag locally per D-10 + project memory `feedback_uat_user_validates.md`.
>
> **Per D-10 + D-16 (informational):** reuse `docs/release-rc.md` verbatim. NO modifications to `release.yml` / `cliff.toml` / `docs/release-rc.md`. The `git-cliff` output is authoritative for the GitHub Release body; no hand-edits post-publish.
>
> **Per D-14 (informational):** all changes land via PR on a feature branch; no direct commits to `main` (`feedback_no_direct_main_commits.md`). The rc.4 tag is cut from `main` AFTER the Phase 24 close-out PR has merged.

## 1. Phase 24 plans merged on `main`

- [ ] PR for Plan 01 (Threat model close-out — TM5 in-place rewrite + new TM6 + STRIDE rows T-S3/T-T4/T-I4/T-D4 + Changelog + README §Security link-back) merged.
- [ ] PR for Plan 02 (Milestone audit — `.planning/milestones/v1.2-MILESTONE-AUDIT.md` + REQUIREMENTS.md flips + ROADMAP drift cleanup) merged.
- [ ] PR for Plan 03 (MILESTONES.md v1.2 entry) merged.
- [ ] PR for Plan 04 (README v1.2 close-out — hero block + §Features pointer + §Configuration §Webhooks + MILESTONES cross-link) merged.
- [ ] PR for Plan 05 (cargo-deny WARN→ERROR promotion — FOUND-16 closed) merged.

> Note: per CONTEXT D-02, plans 24-01..24-05 may land as a single close-out PR (atomic-commit-per-plan inside one PR) OR as five separate PRs. Either shape satisfies this section — the gate is "all five plan changes are on `main`."

Verification:
```bash
gh pr list --state merged --search "phase-24" --limit 30
git log --oneline main | head -25
```

## 2. CI matrix green on `main` for the merge commit (no new top-level CI changes — except cargo-deny promotion landed in plan 24-05)

- [ ] `linux/amd64 × SQLite` lint+test green — `test amd64` job covers both backends via testcontainers
- [ ] `linux/amd64 × Postgres` lint+test green — same `test amd64` cell exercises Postgres via `testcontainers-modules::postgres`
- [ ] `linux/arm64 × SQLite` lint+test green (cargo-zigbuild) — `test arm64` job
- [ ] `linux/arm64 × Postgres` lint+test green — `test arm64` cell exercises both backends
- [ ] `compose-smoke` workflow green (the existing v1.1+v1.2-rc.3 healthcheck path still works)
- [ ] `cargo-deny` job green — **BLOCKING** (promoted in P24 plan 24-05 — FOUND-16 closed). Any RUSTSEC/license/duplicate/ban finding now reddens CI.

Verification:
```bash
gh run list --workflow ci.yml --branch main --limit 10
```

## 3. rustls invariant intact (D-23)

- [ ] `cargo tree -i openssl-sys` returns empty. — verify at preflight execution time: expect `error: package ID specification 'openssl-sys' did not match any packages` (rustls invariant from PROJECT.md + project-locked tech stack — same posture as rc.2 / rc.3).

Verification:
```bash
cargo tree -i openssl-sys
# EXPECT: error: package ID specification `openssl-sys` did not match any packages
```

The Phase 24 close-out PR makes NO TLS-surface edits; the only dep-rev risk would come from plan 24-05 Branch B (advisory remediation via `cargo update -p <crate>`). Plan 24-05 § Task 2 step D explicitly re-verifies `cargo tree -i openssl-sys` empty post-remediation, so rc.4's § 3 should report identical empty output to rc.3.

## 4. release.yml `:latest` gate logic intact (D-16 — no edits)

The hyphen-gate from Phase 12 D-10 is what prevents `v1.2.0-rc.4` from accidentally promoting `:latest`. Confirm it is unchanged.

- [ ] `.github/workflows/release.yml` still has `enable=${{ !contains(github.ref, '-') }}` for the `:latest` skip-on-hyphen gate (verified at lines 132–134 — covers `:latest`, `:major`, `:major.minor`).
- [ ] `.github/workflows/release.yml` still has `enable=${{ contains(github.ref, '-rc.') }}` for the `:rc` rolling tag gate (verified at line 135).
- [ ] No commits in the Phase 24 close-out PR set touch `release.yml`, `cliff.toml`, or `docs/release-rc.md` (D-16). — `git log --name-only <pr-base>..HEAD | grep -E '(release\.yml|cliff\.toml|release-rc\.md)'` returned empty.

Verification:
```bash
grep -n "contains(github.ref" .github/workflows/release.yml
git log --oneline --name-only main..HEAD~10 | grep -E '(release\.yml|cliff\.toml|release-rc\.md)' || echo "OK: no edits to release engineering files"
```

> **Reminder (D-16):** Phase 24 does NOT modify `.github/workflows/release.yml`, `cliff.toml`, or `docs/release-rc.md`. If a maintainer-discovered runbook gap surfaces during the rc.4 cut, that becomes a hotfix PR landed BEFORE tagging (mirroring v1.1 P12 + v1.2 P20/P21/P23 discipline).

## 5. v1.2 close-out audit-predicate verification (Pitfall 56 T-V12-XCUT-05/06/07)

The Phase 24 close-out delivered the threat-model close-out artifacts that Pitfall 56 audit predicates verify. This section gates the rc.4 cut on those predicates being TRUE on `main`.

- [ ] **T-V12-XCUT-05 (TM5 + TM6 sections):** `THREAT_MODEL.md` contains exactly one `## Threat Model 5: Webhook Outbound` (no `(SSRF Accepted Risk)` suffix) AND exactly one `## Threat Model 6: Operator-supplied Docker labels` (lowercase `labels`).
- [ ] **T-V12-XCUT-06 (STRIDE rows):** `THREAT_MODEL.md` STRIDE Summary tables contain rows `| T-S3 |`, `| T-T4 |`, `| T-I4 |`, `| T-D4 |`.
- [ ] **T-V12-XCUT-07 (README link-back):** `README.md` §Security paragraph contains anchor links `#threat-model-5-webhook-outbound` AND `#threat-model-6-operator-supplied-docker-labels`.
- [ ] **REQUIREMENTS flipped:** `grep -c '^- \[ \]' .planning/REQUIREMENTS.md` returns `0` (all 20 remaining v1.2 items flipped to `[x]` per plan 24-02).
- [ ] **Audit doc present + passed:** `.planning/milestones/v1.2-MILESTONE-AUDIT.md` exists with `status: passed` (or `tech_debt` with enumerated remediation per plan 24-02 branching).
- [ ] **MILESTONES.md v1.2 entry:** `MILESTONES.md` contains the v1.2 entry above the v1.1 entry (six-row shape per plan 24-03).
- [ ] **README hero + §Webhooks:** `README.md` contains `## What's New in v1.2` + `### Webhooks` (plan 24-04).

Verification:
```bash
grep -c "^## Threat Model 5: Webhook Outbound$" THREAT_MODEL.md         # expect 1
grep -c "^## Threat Model 6: Operator-supplied Docker labels$" THREAT_MODEL.md  # expect 1
grep -E "^\| T-(S3|T4|I4|D4) \|" THREAT_MODEL.md                         # expect 4 lines
grep -c "#threat-model-[56]-" README.md                                   # expect 2
grep -c "^- \[ \]" .planning/REQUIREMENTS.md                              # expect 0
test -f .planning/milestones/v1.2-MILESTONE-AUDIT.md && grep -q "passed\|tech_debt" .planning/milestones/v1.2-MILESTONE-AUDIT.md
grep -c "^## v1.2 — Operator Integration & Insight" MILESTONES.md         # expect 1
grep -c "^## What's New in v1.2$" README.md                               # expect 1
grep -c "^### Webhooks$" README.md                                        # expect 1
```

## 6. git-cliff release-notes preview

Preview the GitHub Release body that `release.yml` will publish on the rc.4 tag push:

```bash
git cliff --unreleased --tag v1.2.0-rc.4
```

- [ ] Output contains commits for plans 24-01..24-05 (the close-out PR commits).
- [ ] Output is small (3-5 commits expected per CONTEXT § Specifics — rc.4 is a clean docs+CI cut after rc.3, NOT a substantive code delta).
- [ ] No hand-edits to the output — `release.yml` uses `git-cliff` authoritatively per P21 D-25 / P23 D-15.

## 7. 24-HUMAN-UAT.md sign-off

> This section ticks AFTER the rc.4 image is published AND the maintainer has run through `24-HUMAN-UAT.md` (plan 24-07). It is intentionally blank at preflight time; updated when UAT completes.

- [ ] Scenario 1 — `docker compose up` quickstart + dashboard renders — PASSED
- [ ] Scenario 2 — v1.0/v1.1 surfaces intact — PASSED
- [ ] Scenario 3 — Webhooks end-to-end — PASSED
- [ ] Scenario 4 — Custom Docker labels + reserved-namespace — PASSED
- [ ] Scenario 5 — FCTX panel + exit histogram — PASSED
- [ ] Scenario 6 — Tag filter chips — PASSED
- [ ] Final sign-off block in `24-HUMAN-UAT.md` filled in.

## 8. Tag command — maintainer runs LOCALLY (Phase 12 D-13 trust anchor)

Per `docs/release-rc.md` Step 2a (signed) or Step 2b (unsigned), on the maintainer's machine with the maintainer's GPG key:

```bash
git fetch origin main
git checkout main
git pull --ff-only origin main
git tag -a -s v1.2.0-rc.4 -m "v1.2.0-rc.4 — milestone close-out (P24)"
git push origin v1.2.0-rc.4
```

Or unsigned fallback (NOT preferred — only if GPG is unavailable):
```bash
git tag -a v1.2.0-rc.4 -m "v1.2.0-rc.4 — milestone close-out (P24)"
git push origin v1.2.0-rc.4
```

- [ ] Tag pushed to `origin`.
- [ ] `gh release view v1.2.0-rc.4` returns the published release within 5-10 minutes (release.yml workflow latency).
- [ ] Release body is `git cliff --tag v1.2.0-rc.4` output (no hand-edit).

## 9. Post-publish verification

After `release.yml` finishes (~10-15 min for the multi-arch build):

- [ ] `docker manifest inspect ghcr.io/simplicityguy/cronduit:v1.2.0-rc.4` returns amd64 + arm64 entries.
- [ ] `docker manifest inspect ghcr.io/simplicityguy/cronduit:rc` returns the same digest as `:v1.2.0-rc.4` (rolling `:rc` tag).
- [ ] `docker manifest inspect ghcr.io/simplicityguy/cronduit:latest` returns the SAME digest as `ghcr.io/simplicityguy/cronduit:1.1.0` — `:latest` does NOT promote yet (hyphen-gate from P12 D-10 — `v1.2.0-rc.4` contains a hyphen, so `:latest` / `:1.2` / `:1` do not advance until the final `v1.2.0` tag pushes).
- [ ] `cronduit --version` from the published `:v1.2.0-rc.4` image returns `cronduit 1.2.0` (tag prefix `v1.2.0` matches Cargo.toml `version = "1.2.0"` per `feedback_tag_release_version_match.md`).
- [ ] CI run for the rc.4 tag commit is green — `cargo-deny` runs as BLOCKING (no `continue-on-error: true`).

## Sign-off

| Field | Value |
|-------|-------|
| Maintainer signature | `__________________` |
| Date (UTC) | `__________________` |
| Tag commit SHA | `__________________` |
| GHCR amd64 digest | `__________________` |
| GHCR arm64 digest | `__________________` |
| GHCR `:latest` digest (must equal `v1.1.0` digest) | `__________________` |
| GHCR `:1.1.0` digest (for comparison) | `__________________` |
| GHCR `:rc` digest (must equal `v1.2.0-rc.4` digest) | `__________________` |

## Out of scope

- `release.yml` / `cliff.toml` / `docs/release-rc.md` modifications — these are reused verbatim per P12 D-10..D-12 and Phase 24 D-10 / D-16 (informational).
- `Cargo.toml` version bump — stays at `1.2.0` through rc.4 and final v1.2.0 tag.
- New code features — Phase 24 is paperwork + CI close-out; rc.4 is a clean post-close-out cut.
- `:latest` promotion — happens only on the non-hyphenated final `v1.2.0` tag (Phase 24 plan 24-08).

## Cross-reference

This runbook mirrors [`21-RC2-PREFLIGHT.md`](../21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-RC2-PREFLIGHT.md) and [`23-RC3-PREFLIGHT.md`](../23-job-tagging-dashboard-filter-chips-rc-3/23-RC3-PREFLIGHT.md) with the following substitutions:
- `rc.2` / `rc.3` → `rc.4`
- `P21` / `P23` → `P24`
- Plan list `01-10` / `01-07` → `01-05` (the close-out PR plans; plans 06/07/08 are autonomous=false maintainer runbooks not included in the merged-on-main gate of § 1)
- The EXIT-06 cardinality verification (P21) / tags-as-Prometheus-label out-of-scope verification (P23) is replaced by the v1.2 close-out audit-predicate verification (§ 5) — Pitfall 56 audit predicates T-V12-XCUT-05/06/07 + REQUIREMENTS flips + audit doc + MILESTONES entry + README hero.
- The `cargo-deny` row in § 2 reads BLOCKING (FOUND-16 closed per plan 24-05) — NOT the still-non-blocking language from rc.2/rc.3 preflights.
