---
phase: 21
plan: 11
type: rc-preflight
autonomous: false
rc_tag: v1.2.0-rc.2
created: 2026-05-02
status: pending-maintainer-execution
---

# Phase 21 — v1.2.0-rc.2 Tag Cut Pre-Flight

**Phase:** 21 — Failure-Context UI Panel + Exit-Code Histogram Card — rc.2
**Tag target:** `v1.2.0-rc.2`
**Runbook:** [docs/release-rc.md](../../../docs/release-rc.md) — REUSED VERBATIM (D-22). Phase 21 makes NO edits to this runbook.
**Cargo.toml version:** `1.2.0` (set in Phase 15; the `-rc.2` is tag-only per `feedback_tag_release_version_match.md` and D-31).

> This artifact is a maintainer-validated checklist. Claude authored it; the maintainer runs through it and cuts the tag locally per D-26 + project memory `feedback_uat_user_validates.md`.
>
> **Per D-22..D-26:** reuse `docs/release-rc.md` verbatim. NO modifications to `release.yml` / `cliff.toml` / `docs/release-rc.md` (D-24). The `git-cliff` output is authoritative for the GitHub Release body (D-25); no hand-edits post-publish.
>
> **Per D-27 [informational]:** all changes land via PR on a feature branch; no direct commits to `main` (`feedback_no_direct_main_commits.md`). The rc.2 tag is cut from `main` AFTER the Phase 21 PR has merged.

## 1. Phase 21 plans merged on `main`

- [ ] PR for Plan 01 (`scheduled_for` migration — sqlite + postgres mirror) merged.
- [ ] PR for Plan 02 (scheduler `insert_running_run` widening for fire-skew write) merged.
- [ ] PR for Plan 03 (`src/web/exit_buckets.rs` aggregator + 10-bucket classifier) merged.
- [ ] PR for Plan 04 (run-detail handler wire-up + askama panel template) merged.
- [ ] PR for Plan 05 (job-detail handler wire-up + askama histogram template) merged.
- [ ] PR for Plan 06 (CSS additions in `assets/src/app.css` `@layer components`) merged.
- [ ] PR for Plan 07 (`tests/v12_fctx_panel.rs`) merged.
- [ ] PR for Plan 08 (`tests/v12_exit_histogram.rs`) merged.
- [ ] PR for Plan 09 (`tests/v12_fctx_explain.rs` extension) merged.
- [ ] PR for Plan 10 (`uat-fctx-panel`, `uat-exit-histogram`, `uat-fire-skew`, `uat-fctx-a11y` recipes + `fire-skew-demo` example job) merged.

Verification:
```bash
gh pr list --state merged --search "phase-21" --limit 30
git log --oneline main | head -25
```

## 2. CI matrix green on `main` for the merge commit (D-21 — no new top-level CI changes)

- [ ] `linux/amd64 × SQLite` lint+test green
- [ ] `linux/amd64 × Postgres` lint+test green
- [ ] `linux/arm64 × SQLite` lint+test green (cargo-zigbuild)
- [ ] `linux/arm64 × Postgres` lint+test green
- [ ] `compose-smoke` workflow green (the existing v1.1+v1.2-rc.1 healthcheck path still works)
- [ ] `cargo-deny` job green (still non-blocking on rc.2 — promotion to blocking is Phase 24)

Verification:
```bash
gh run list --branch main --limit 5
gh run view <RUN_ID>   # confirm all matrix legs green
```

## 3. rustls invariant intact (D-32)

- [ ] `cargo tree -i openssl-sys` returns empty.

Verification:
```bash
cargo tree -i openssl-sys
# EXPECT: error: package ID specification `openssl-sys` did not match any packages
```

## 4. release.yml `:latest` gate logic intact (D-24 — no edits)

The hyphen-gate from Phase 12 D-10 is what prevents `v1.2.0-rc.2` from accidentally promoting `:latest`. Confirm it is unchanged.

- [ ] `.github/workflows/release.yml` still has `enable=${{ !contains(github.ref, '-') }}` for the `:latest` skip-on-hyphen gate.
- [ ] `.github/workflows/release.yml` still has `enable=${{ contains(github.ref, '-rc.') }}` for the `:rc` rolling tag gate.
- [ ] No commits in the rc.2 PR set touch `release.yml`, `cliff.toml`, or `docs/release-rc.md` (D-24).

Verification:
```bash
grep -n "contains(github.ref" .github/workflows/release.yml
git log --oneline --name-only main..HEAD~10 | grep -E '(release\.yml|cliff\.toml|release-rc\.md)' || echo "OK: no edits to release engineering files"
```

> **Reminder (D-24):** Phase 21 does NOT modify `.github/workflows/release.yml`, `cliff.toml`, or `docs/release-rc.md`. If a maintainer-discovered runbook gap surfaces during the rc.2 cut, that becomes a hotfix PR landed BEFORE tagging (mirroring v1.1 P12 + v1.2 P20 discipline).

## 5. EXIT-06 cardinality discipline holds (out-of-scope verification)

- [ ] `grep -rn 'exit_code' src/telemetry.rs` returns empty — confirms Phase 21 did NOT add a per-job `exit_code` Prometheus label per EXIT-06 reasoning. (Plan 21-11 said `src/metrics.rs`, which does not exist in this repo; the metrics module lives at `src/telemetry.rs`. Same intent applies.)
- [ ] `grep -rn 'cronduit_runs_total.*exit_code' src/` returns empty — confirms the `cronduit_runs_total` counter family has only `{job, status}` labels.
- [ ] `grep -rn 'exit_code' src/web/handlers/metrics.rs` returns empty (vacuously satisfied — no such file).

Verification:
```bash
grep -rn 'exit_code' src/telemetry.rs              # MUST return empty
grep -rn 'cronduit_runs_total.*exit_code' src/    # MUST return empty
grep -rn 'exit_code' src/web/handlers/metrics.rs   # MUST return empty (no such file)
```

## 6. git-cliff release-notes preview shows P21 commits

- [ ] `git cliff --unreleased --tag v1.2.0-rc.2 | head -100` shows commits from Phase 21 (plus any post-rc.1 hotfixes that landed since `v1.2.0-rc.1`).
- [ ] No commit appears in the wrong section (e.g., a feat commit landing under "fixes"). If any commit is mis-categorized, hotfix the conventional-commit prefix on `main` BEFORE tagging — per D-25, `git-cliff` output is authoritative; do NOT hand-edit the GitHub Release body after publish.

Verification:
```bash
git fetch --tags
git cliff --unreleased --tag v1.2.0-rc.2 -o /tmp/release-rc2-preview.md
cat /tmp/release-rc2-preview.md
```

## 7. 21-HUMAN-UAT.md sign-off

- [ ] All maintainer checkboxes in `21-HUMAN-UAT.md` are ticked `[x]` (8 scenarios + the rustls spot check).
- [ ] The Sign-off block at the bottom of `21-HUMAN-UAT.md` is filled in (maintainer name + date + comment).

> Per D-26 + project memory `feedback_uat_user_validates.md`: Claude does NOT mark UAT passed; the maintainer flips every checkbox manually.

## 8. Tag command (maintainer runs LOCALLY) — D-23

On a clean checkout of `main` at the merge commit:

```bash
git checkout main
git pull --ff-only origin main
git tag -a -s v1.2.0-rc.2 -m "v1.2.0-rc.2 — FCTX UI panel + exit-code histogram (P21)"
git push origin v1.2.0-rc.2
```

> Per D-26 + project memory `feedback_no_direct_main_commits.md`: this is a maintainer-local action. Claude does NOT execute it. The `-s` flag requires the maintainer's GPG signing key (per `docs/release-rc.md` Step 2a). If the maintainer's git is not GPG-configured, fall back to `docs/release-rc.md` Step 2b (unsigned annotated tag) — the runbook covers both paths.

The literal tag command (copy-paste verbatim):

```bash
git tag -a -s v1.2.0-rc.2 -m "v1.2.0-rc.2 — FCTX UI panel + exit-code histogram (P21)"
```

## 9. Post-publish verification (per docs/release-rc.md § Post-push verification)

After `release.yml` finishes (≈ 10–20 minutes for both archs):

- [ ] `gh release view v1.2.0-rc.2 --json isPrerelease --jq .isPrerelease` returns `true`.
- [ ] `gh release view v1.2.0-rc.2` shows the auto-generated git-cliff body (D-25 — do NOT hand-edit post-publish).
- [ ] `docker manifest inspect ghcr.io/simplicityguy/cronduit:v1.2.0-rc.2` returns 2 manifests (`linux/amd64` + `linux/arm64`).
- [ ] **`:latest` invariant detection (D-22 / Phase 12 D-10):** capture the digest of `ghcr.io/simplicityguy/cronduit:latest` AND the digest of `ghcr.io/simplicityguy/cronduit:1.1.0`; the two digests MUST be identical. The hyphen-gate at `release.yml` held; the rc.2 tag did NOT promote `:latest`.
- [ ] `docker manifest inspect ghcr.io/simplicityguy/cronduit:rc` digest matches the `:v1.2.0-rc.2` digest (the `:rc` rolling tag updated to rc.2).
- [ ] `:1` and `:1.1` are unchanged (still pointing at `v1.1.0`'s digest — they only update on the final non-rc ship).
- [ ] `docker run --rm ghcr.io/simplicityguy/cronduit:v1.2.0-rc.2 cronduit --version` returns `cronduit 1.2.0` (the unsuffixed in-source version per D-31).
- [ ] Healthy in the shipped compose stack: `docker compose -f examples/docker-compose.yml up -d` (with `image:` overridden to `:v1.2.0-rc.2`) → `docker compose ps` after 90 s shows `Up N seconds (healthy)`.

Verification:
```bash
gh release view v1.2.0-rc.2
docker manifest inspect ghcr.io/simplicityguy/cronduit:v1.2.0-rc.2
LATEST_DIGEST=$(docker manifest inspect ghcr.io/simplicityguy/cronduit:latest | sha256sum | awk '{print $1}')
V1_1_0_DIGEST=$(docker manifest inspect ghcr.io/simplicityguy/cronduit:1.1.0 | sha256sum | awk '{print $1}')
[[ "$LATEST_DIGEST" == "$V1_1_0_DIGEST" ]] && echo "OK: :latest invariant verified" || echo "FAIL: :latest was promoted to rc.2"
docker manifest inspect ghcr.io/simplicityguy/cronduit:rc | grep -A2 digest
```

## Out-of-scope (per D-24)

The following files are deliberately untouched by Phase 21 — they are reused verbatim from the v1.1+v1.2-rc.1 release machinery:

- DO NOT modify .github/workflows/release.yml — the hyphen-gate from P12 D-10 is what prevents rc.2 from promoting `:latest`.
- DO NOT modify cliff.toml — the git-cliff config is the canonical changelog grammar; per-rc tweaks would break commit grouping.
- DO NOT modify docs/release-rc.md — this runbook is the trust anchor for every rc cut; the rc.2 procedure REUSES it verbatim per D-22.
- DO NOT hand-edit the GitHub Release body post-publish (D-25) — the git-cliff output is authoritative.

If any maintainer-discovered runbook gap surfaces during the rc.2 cut, that becomes a hotfix PR landed BEFORE tagging (mirroring v1.1 P12 + v1.2 P20 discipline per D-24).

## What if UAT fails (per docs/release-rc.md § What if UAT fails)

The cardinal rule: **never force-push a tag, never delete-and-retag**. If a critical issue surfaces during rc.2 UAT:

1. Fix the issue on `main` via a normal feature branch + PR (per `feedback_no_direct_main_commits.md`).
2. Cut `v1.2.0-rc.3` following this same runbook from the top.
3. Leave `v1.2.0-rc.2` published. It stays as a historical artifact.
4. Optionally update the rc.2 GitHub Release body with a one-line `> ⚠️ Superseded by v1.2.0-rc.3 — see [link]` callout (the ONE acceptable hand-edit per `docs/release-rc.md`).

## Sign-off

All sections above must be ticked `[x]` by the maintainer.

- [ ] All sections above ticked.
- [ ] Tag pushed: `v1.2.0-rc.2`.
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
| GHCR `:rc` digest (must equal `v1.2.0-rc.2` digest) | `__________________` |

After all boxes are ticked and the sign-off table is filled in:

- The maintainer marks Plan 21-11 complete (orchestrator updates `.planning/STATE.md` + `.planning/ROADMAP.md` to reflect Phase 21 → SHIPPED at rc.2).
- Operators are notified via the GitHub Release page (the git-cliff body is authoritative per D-25).

---

**Cross-reference:** this runbook mirrors `.planning/phases/20-webhook-ssrf-https-posture-retry-drain-metrics-rc-1/20-RC1-PREFLIGHT.md` with `rc.1 → rc.2`, `P20 → P21`, and the EXIT-06 cardinality verification added per CONTEXT § Out of scope. The structural reuse is intentional per D-22: same runbook (`docs/release-rc.md`), same gate semantics, same `:latest` invariant detection. The only Phase-21-specific additions are the EXIT-06 grep verification (§5) and the FCTX-/EXIT-scoped tag message (§8).
