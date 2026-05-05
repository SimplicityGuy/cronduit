---
phase: 23
plan: 08
type: rc-preflight
autonomous: false
rc_tag: v1.2.0-rc.3
created: 2026-05-04
status: pending-maintainer-execution
---

# Phase 23 — v1.2.0-rc.3 Tag Cut Pre-Flight

**Phase:** 23 — Job Tagging Dashboard Filter Chips — rc.3
**Tag target:** `v1.2.0-rc.3`
**Runbook:** [docs/release-rc.md](../../../docs/release-rc.md) — REUSED VERBATIM (D-15). Phase 23 makes NO edits to this runbook.
**Cargo.toml version:** `1.2.0` (set in Phase 15; the `-rc.3` is tag-only per `feedback_tag_release_version_match.md` and D-22).

> This artifact is a maintainer-validated checklist. Claude authored it; the maintainer runs through it and cuts the tag locally per D-15 + project memory `feedback_uat_user_validates.md`.
>
> **Per D-15..D-16:** reuse `docs/release-rc.md` verbatim. NO modifications to `release.yml` / `cliff.toml` / `docs/release-rc.md` (D-16). The `git-cliff` output is authoritative for the GitHub Release body (D-15); no hand-edits post-publish.
>
> **Per D-18 [informational]:** all changes land via PR on a feature branch; no direct commits to `main` (`feedback_no_direct_main_commits.md`). The rc.3 tag is cut from `main` AFTER the Phase 23 PR has merged.

## 1. Phase 23 plans merged on `main`

- [ ] PR for Plan 01 (Wave-0 test scaffolding — tests/v12_tags_dashboard.rs + dashboard.rs::tests stubs) merged.
- [ ] PR for Plan 02 (DB layer — DashboardJob.tags + get_dashboard_jobs SELECT/WHERE widening for AND-tag filter) merged.
- [ ] PR for Plan 03 (handler — axum_extra::Query swap + DashboardParams.tags + fleet-tag fold + active-set sort/dedup/intersect) merged.
- [ ] PR for Plan 04 (CSS — cd-tag-chip-* family in @layer components + reduced-motion + print extensions) merged.
- [ ] PR for Plan 05 (template — chip strip insert + sort-header href widening + poll hx-include widening + OOB swap response composition) merged.
- [ ] PR for Plan 06 (UAT — 3 just uat-chips-* recipes + README Tag Filter Chips subsection) merged.
- [ ] PR for Plan 07 (HUMAN-UAT — 23-HUMAN-UAT.md autonomous=false maintainer plan) merged.

Verification:
```bash
gh pr list --state merged --search "phase-23" --limit 30
git log --oneline main | head -25
```

## 2. CI matrix green on `main` for the merge commit (no new top-level CI changes)

- [ ] `linux/amd64 × SQLite` lint+test green
- [ ] `linux/amd64 × Postgres` lint+test green
- [ ] `linux/arm64 × SQLite` lint+test green (cargo-zigbuild)
- [ ] `linux/arm64 × Postgres` lint+test green
- [ ] `compose-smoke` workflow green (the existing v1.1+v1.2-rc.2 healthcheck path still works)
- [ ] `cargo-deny` job green (still non-blocking on rc.3 — promotion to blocking is Phase 24)

Verification:
```bash
gh run list --branch main --limit 5
gh run view <RUN_ID>   # confirm all matrix legs green
```

## 3. rustls invariant intact (D-23)

- [ ] `cargo tree -i openssl-sys` returns empty.

Verification:
```bash
cargo tree -i openssl-sys
# EXPECT: error: package ID specification `openssl-sys` did not match any packages
```

## 4. release.yml `:latest` gate logic intact (D-16 — no edits)

The hyphen-gate from Phase 12 D-10 is what prevents `v1.2.0-rc.3` from accidentally promoting `:latest`. Confirm it is unchanged.

- [ ] `.github/workflows/release.yml` still has `enable=${{ !contains(github.ref, '-') }}` for the `:latest` skip-on-hyphen gate.
- [ ] `.github/workflows/release.yml` still has `enable=${{ contains(github.ref, '-rc.') }}` for the `:rc` rolling tag gate.
- [ ] No commits in the rc.3 PR set touch `release.yml`, `cliff.toml`, or `docs/release-rc.md` (D-16).

Verification:
```bash
grep -n "contains(github.ref" .github/workflows/release.yml
git log --oneline --name-only main..HEAD~10 | grep -E '(release\.yml|cliff\.toml|release-rc\.md)' || echo "OK: no edits to release engineering files"
```

> **Reminder (D-16):** Phase 23 does NOT modify `.github/workflows/release.yml`, `cliff.toml`, or `docs/release-rc.md`. If a maintainer-discovered runbook gap surfaces during the rc.3 cut, that becomes a hotfix PR landed BEFORE tagging (mirroring v1.1 P12 + v1.2 P20/P21 discipline).

## 5. Tags-as-Prometheus-label out-of-scope verification (per CONTEXT § deferred)

- [ ] `grep -rn 'tags' src/telemetry.rs` returns empty — confirms Phase 23 did NOT add a per-job `tags` Prometheus label per CONTEXT § Out of scope ("Tags as Prometheus label — explicit out-of-scope; same cardinality posture as exit codes per EXIT-06"). (Plan 23-08 said `src/metrics.rs`, which does not exist in this repo; the metrics module lives at `src/telemetry.rs`. Same intent applies.)
- [ ] `grep -rn 'cronduit_runs_total.*tags' src/` returns empty — confirms the `cronduit_runs_total` counter family has only `{job, status}` labels (Phase 23's TAG additions did not extend the metric family).
- [ ] `grep -rn 'tags' src/web/handlers/metrics.rs` returns empty (vacuously satisfied — no such file in tree; metrics route lives in `src/telemetry.rs`).

Verification:
```bash
grep -rn 'tags' src/telemetry.rs              # MUST return empty
grep -rn 'cronduit_runs_total.*tags' src/    # MUST return empty
grep -rn 'tags' src/web/handlers/metrics.rs   # MUST return empty (no such file)
```

## 6. git-cliff release-notes preview shows P23 commits

- [ ] `git cliff --unreleased --tag v1.2.0-rc.3 | head -100` shows commits from Phase 23 (plus any post-rc.2 hotfixes that landed since `v1.2.0-rc.2`).
- [ ] No commit appears in the wrong section (e.g., a feat commit landing under "fixes"). If any commit is mis-categorized, hotfix the conventional-commit prefix on `main` BEFORE tagging — per D-15, `git-cliff` output is authoritative; do NOT hand-edit the GitHub Release body after publish.

Verification:
```bash
git fetch --tags
git cliff --unreleased --tag v1.2.0-rc.3 -o /tmp/release-rc3-preview.md
cat /tmp/release-rc3-preview.md
```

## 7. 23-HUMAN-UAT.md sign-off

- [ ] All maintainer checkboxes in `23-HUMAN-UAT.md` are ticked `[x]` (six scenarios + the rustls spot check).
- [ ] The Sign-off block at the bottom of `23-HUMAN-UAT.md` is filled in (maintainer name + date + comment).

> Per D-15 + project memory `feedback_uat_user_validates.md`: Claude does NOT mark UAT passed; the maintainer flips every checkbox manually.

## 8. Tag command (maintainer runs LOCALLY) — D-15

On a clean checkout of `main` at the merge commit:

```bash
git checkout main
git pull --ff-only origin main
git tag -a -s v1.2.0-rc.3 -m "v1.2.0-rc.3 — dashboard tag filter chips (P23)"
git push origin v1.2.0-rc.3
```

> Per D-15 + project memory `feedback_no_direct_main_commits.md`: this is a maintainer-local action. Claude does NOT execute it. The `-s` flag requires the maintainer's GPG signing key (per `docs/release-rc.md` Step 2a). If the maintainer's git is not GPG-configured, fall back to `docs/release-rc.md` Step 2b (unsigned annotated tag) — the runbook covers both paths.

The literal tag command (copy-paste verbatim):

```bash
git tag -a -s v1.2.0-rc.3 -m "v1.2.0-rc.3 — dashboard tag filter chips (P23)"
```

> **Tag-version invariant (D-22 + project memory `feedback_tag_release_version_match.md`):** the tag prefix `v1.2.0` MUST match `Cargo.toml`'s `version = "1.2.0"`. The `-rc.3` is tag-only suffix. Section 9 verifies `cronduit --version` returns `cronduit 1.2.0` from the published rc.3 image.

## 9. Post-publish verification (per docs/release-rc.md § Post-push verification)

After `release.yml` finishes (≈ 10–20 minutes for both archs):

- [ ] `gh release view v1.2.0-rc.3 --json isPrerelease --jq .isPrerelease` returns `true`.
- [ ] `gh release view v1.2.0-rc.3` shows the auto-generated git-cliff body (D-15 — do NOT hand-edit post-publish).
- [ ] `docker manifest inspect ghcr.io/simplicityguy/cronduit:v1.2.0-rc.3` returns 2 manifests (`linux/amd64` + `linux/arm64`).
- [ ] **`:latest` invariant detection (D-15 / Phase 12 D-10):** capture the digest of `ghcr.io/simplicityguy/cronduit:latest` AND the digest of `ghcr.io/simplicityguy/cronduit:1.1.0`; the two digests MUST be identical. The hyphen-gate at `release.yml` held; the rc.3 tag did NOT promote `:latest`.
- [ ] `docker manifest inspect ghcr.io/simplicityguy/cronduit:rc` digest matches the `:v1.2.0-rc.3` digest (the `:rc` rolling tag updated to rc.3).
- [ ] `:1` and `:1.1` are unchanged (still pointing at `v1.1.0`'s digest — they only update on the final non-rc ship).
- [ ] `docker run --rm ghcr.io/simplicityguy/cronduit:v1.2.0-rc.3 cronduit --version` returns `cronduit 1.2.0` (the unsuffixed in-source version per D-22).
- [ ] Healthy in the shipped compose stack: `docker compose -f examples/docker-compose.yml up -d` (with `image:` overridden to `:v1.2.0-rc.3`) → `docker compose ps` after 90 s shows `Up N seconds (healthy)`.

Verification:
```bash
gh release view v1.2.0-rc.3
docker manifest inspect ghcr.io/simplicityguy/cronduit:v1.2.0-rc.3
LATEST_DIGEST=$(docker manifest inspect ghcr.io/simplicityguy/cronduit:latest | sha256sum | awk '{print $1}')
V1_1_0_DIGEST=$(docker manifest inspect ghcr.io/simplicityguy/cronduit:1.1.0 | sha256sum | awk '{print $1}')
[[ "$LATEST_DIGEST" == "$V1_1_0_DIGEST" ]] && echo "OK: :latest invariant verified" || echo "FAIL: :latest was promoted to rc.3"
docker manifest inspect ghcr.io/simplicityguy/cronduit:rc | grep -A2 digest
```

## Out-of-scope (per D-16)

The following files are deliberately untouched by Phase 23 — they are reused verbatim from the v1.1+v1.2-rc.1+v1.2-rc.2 release machinery:

- DO NOT modify .github/workflows/release.yml — the hyphen-gate from P12 D-10 is what prevents rc.3 from promoting `:latest`.
- DO NOT modify cliff.toml — the git-cliff config is the canonical changelog grammar; per-rc tweaks would break commit grouping.
- DO NOT modify docs/release-rc.md — this runbook is the trust anchor for every rc cut; the rc.3 procedure REUSES it verbatim per D-15.
- DO NOT hand-edit the GitHub Release body post-publish (D-15) — the git-cliff output is authoritative.

If any maintainer-discovered runbook gap surfaces during the rc.3 cut, that becomes a hotfix PR landed BEFORE tagging (mirroring v1.1 P12 + v1.2 P20/P21 discipline per D-16).

## What if UAT fails (per docs/release-rc.md § What if UAT fails)

The cardinal rule: **never force-push a tag, never delete-and-retag**. If a critical issue surfaces during rc.3 UAT:

1. Fix the issue on `main` via a normal feature branch + PR (per `feedback_no_direct_main_commits.md`).
2. Cut `v1.2.0-rc.4` following this same runbook from the top.
3. Leave `v1.2.0-rc.3` published. It stays as a historical artifact.
4. Optionally update the rc.3 GitHub Release body with a one-line `> ⚠️ Superseded by v1.2.0-rc.4 — see [link]` callout (the ONE acceptable hand-edit per `docs/release-rc.md`).

## Sign-off

All sections above must be ticked `[x]` by the maintainer.

- [ ] All sections above ticked.
- [ ] Tag pushed: `v1.2.0-rc.3`.
- [ ] `:latest` STILL at `v1.1.0` (verified post-publish — `:latest` invariant held).
- [ ] No regressions reported in the first 24 h post-publish.

| Field | Value |
|-------|-------|
| Maintainer signature | `__________________` |
| Date (UTC) | `__________________` |
| Tag commit SHA | `__________________` |
| GHCR amd64 digest | `__________________` |
| GHCR arm64 digest | `__________________` |
| GHCR `:latest` digest (must equal `v1.1.0` digest) | `__________________` |
| GHCR `:1.1.0` digest (for comparison) | `__________________` |
| GHCR `:rc` digest (must equal `v1.2.0-rc.3` digest) | `__________________` |

After all boxes are ticked and the sign-off table is filled in:

- The maintainer marks Plan 23-08 complete (orchestrator updates `.planning/STATE.md` + `.planning/ROADMAP.md` to reflect Phase 23 → SHIPPED at rc.3).
- Operators are notified via the GitHub Release page (the git-cliff body is authoritative per D-15).

---

**Cross-reference:** this runbook mirrors `.planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-RC2-PREFLIGHT.md` with `rc.2 → rc.3`, `P21 → P23`, `FCTX UI panel + exit-code histogram → dashboard tag filter chips`, and the EXIT-06 cardinality verification swapped for the analogous P23 invariant (tags-as-Prometheus-label out-of-scope). The structural reuse is intentional per D-15: same runbook (`docs/release-rc.md`), same gate semantics, same `:latest` invariant detection. The only Phase-23-specific additions are the tags-cardinality grep verification (§5) and the tag-chip-scoped tag message (§8). The chain extends `.planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-RC1-PREFLIGHT.md → .planning/phases/21-failure-context-ui-panel-exit-code-histogram-card-rc-2/21-RC2-PREFLIGHT.md → .planning/phases/23-job-tagging-dashboard-filter-chips-rc-3/23-RC3-PREFLIGHT.md`, mirroring v1.1 P12 + v1.2 P20/P21 discipline.
