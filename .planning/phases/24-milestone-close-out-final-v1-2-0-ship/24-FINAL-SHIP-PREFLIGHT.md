---
phase: 24
plan: 08
type: final-ship-preflight
autonomous: false
final_tag: v1.2.0
created: 2026-05-16
status: pending-maintainer-execution
---

# Phase 24 — Final v1.2.0 Ship Pre-Flight

**Phase:** 24 — Milestone Close-Out — final v1.2.0 ship
**Final tag target:** `v1.2.0`
**Runbook:** [docs/release-rc.md](../../../docs/release-rc.md) Step 2a (signed) or Step 2b (unsigned fallback) — REUSED VERBATIM (D-10 / D-16 informational). Phase 24 makes NO edits to this runbook.
**Cargo.toml version:** `1.2.0` (set in Phase 15; the final tag `v1.2.0` matches per `feedback_tag_release_version_match.md` and D-18 informational).

> This artifact is a maintainer-validated checklist. Claude authored it; the maintainer retags the rc.N SHA locally per D-10 + project memory `feedback_uat_user_validates.md`.
>
> **Retag-the-rc-SHA invariant (D-01 — mirrors v1.1 P14 D-16):** `v1.2.0` MUST retag the LAST PASSING-UAT rc.N SHA (rc.4 if UAT passed first time; rc.5 / rc.6 / etc. if iterated). Bit-identical image — what the maintainer UAT-validated is what ships. NO new commits between rc.N and v1.2.0.
>
> **Per D-14 (informational):** all changes land via PR on a feature branch; the v1.2.0 tag is pushed AFTER the close-out PR has merged AND the rc.N UAT has signed off in `24-HUMAN-UAT.md`.

---

## 1. rc.N UAT passed (final-tag prerequisite)

- [ ] `24-HUMAN-UAT.md` Scenarios 1-6 all ticked PASSED.
- [ ] Final sign-off block in `24-HUMAN-UAT.md` filled in with maintainer name + date.
- [ ] `RC tag UAT-validated:` field in `24-HUMAN-UAT.md` records the rc tag (`v1.2.0-rc.4` or iterated `v1.2.0-rc.N`).
- [ ] If iterated (rc.5+): each iteration's UAT also recorded in `24-HUMAN-UAT.md`; only the LAST passing rc.N SHA is retagged as `v1.2.0`.

Verification:

```bash
grep -E "(Scenario [1-6] passed|Final sign-off|Maintainer name|RC tag UAT-validated)" .planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-HUMAN-UAT.md
```

---

## 2. Identify rc.N SHA to retag

The `v1.2.0` tag points at the SAME commit SHA as the last-passing-UAT rc.N tag (bit-identical image — D-01).

```bash
RC_TAG=v1.2.0-rc.4   # or rc.5, rc.6, etc. — whichever the maintainer ran UAT against
RC_SHA=$(git rev-list -n 1 "$RC_TAG")
echo "Retagging $RC_SHA (currently $RC_TAG) as v1.2.0"
```

- [ ] `RC_SHA` captured.
- [ ] `git log -1 $RC_SHA --format=oneline` is the close-out PR merge commit (or a hotfix commit if UAT iterated).

---

## 3. Retag command — maintainer runs LOCALLY

Per [`docs/release-rc.md`](../../../docs/release-rc.md) **Step 2a (signed — preferred)**:

```bash
git tag -a -s v1.2.0 -m "v1.2 — Operator Integration & Insight" "$RC_SHA"
git push origin v1.2.0
```

OR **Step 2b (unsigned fallback — only if GPG is unavailable)**:

```bash
git tag -a v1.2.0 -m "v1.2 — Operator Integration & Insight" "$RC_SHA"
git push origin v1.2.0
```

- [ ] Tag pushed to `origin`.
- [ ] `gh release view v1.2.0` returns the published release within 10-15 minutes (release.yml multi-arch build latency).
- [ ] Release body is `git cliff --tag v1.2.0` output (no hand-edit per D-15 of P23 inherits to P24).

---

## 4. Post-publish verification: `:latest` hyphen-gate + four-tag equality

`v1.2.0` contains NO hyphen → `release.yml`'s hyphen-gate from P12 D-10 fires → `:1.2.0` + `:1.2` + `:1` + `:latest` all publish on amd64 + arm64.

Verify each tag's digest matches the v1.2.0 multi-arch manifest:

```bash
for tag in 1.2.0 1.2 1 latest; do
  echo "== :$tag =="
  docker manifest inspect ghcr.io/simplicityguy/cronduit:$tag | jq '.manifests[] | { arch: .platform.architecture, digest: .digest }'
done
```

- [ ] amd64 digest of `:1.2.0` matches amd64 digest of `:1.2`, `:1`, AND `:latest`.
- [ ] arm64 digest of `:1.2.0` matches arm64 digest of `:1.2`, `:1`, AND `:latest`.
- [ ] Four-tag equality verified on BOTH architectures (ROADMAP Phase 24 success criterion #4).
- [ ] `:latest` digest CHANGED from the pre-tag digest (which was `:1.1.0`).
- [ ] `cronduit --version` from the published `:1.2.0` image returns `cronduit 1.2.0`.

Capture before/after digests:

```bash
# Before (record from rc.4 sign-off table in plan 24-06):
PREV_LATEST_DIGEST="<from plan 24-06 § Sign-off — the :1.1.0 digest>"

# After (run now):
NEW_LATEST_DIGEST=$(docker manifest inspect ghcr.io/simplicityguy/cronduit:latest | jq -r '.manifests[0].digest')
echo "Previous :latest digest: $PREV_LATEST_DIGEST"
echo "New :latest digest:      $NEW_LATEST_DIGEST"
test "$PREV_LATEST_DIGEST" != "$NEW_LATEST_DIGEST" && echo "ADVANCED" || echo "STUCK — investigate"
```

---

## 5. cargo-deny ERROR-gate on v1.2.0 tag CI run

FOUND-16 promoted in plan 24-05 (cargo-deny `continue-on-error: true` → blocking). Verify the v1.2.0 tag's CI run confirms this:

```bash
gh run list --workflow ci.yml --branch <ref> --commit "$RC_SHA" --limit 5
gh run view <run-id> --log | grep -E "(cargo-deny|just deny)" -A 3
```

- [ ] CI run for the v1.2.0 tag commit (== rc.N SHA) is green.
- [ ] `cargo-deny` step (`just deny`) is REQUIRED (no `continue-on-error: true` in the log).
- [ ] No `cargo deny check` advisories / license / duplicate / ban findings (or any present are `deny.toml`-allowlisted with documented expiry from plan 24-05).
- [ ] FOUND-16 considered fully closed at this point.

---

## 6. git-cliff cumulative release body

Per Phase 14 D-19 (v1.1 final ship cumulative release notes pattern), verify the v1.2.0 release body covers ALL v1.2 commits (not just the rc.4 → v1.2.0 delta):

```bash
git cliff v1.1.0..v1.2.0
```

- [ ] Output covers every commit from `v1.1.0..v1.2.0` (rc.1, rc.2, rc.3, rc.4, … final).
- [ ] Output landed verbatim as the GitHub Release body for `v1.2.0` (no hand-edit per D-15 of P23 inherits).
- [ ] Five v1.2 features visible in the release notes: webhooks / labels / FCTX / exit histogram / tags.

---

## 7. Update STATE.md + finalize MILESTONES.md SHIPPED date

After § 4-6 verify clean:

- [ ] Open `.planning/STATE.md`; flip `milestone: v1.2` `status: planning` (or whatever pre-ship state) to `status: shipped`; set `last_updated` to NOW; bump `progress.completed_phases` to 10 and `progress.percent` to 100; under `Current Position` set `Phase: 24` `Plan: 24-08 (SHIPPED)` `Status: Milestone v1.2 SHIPPED` with the actual ship date.
- [ ] Open `MILESTONES.md`; replace the `SHIPPED YYYY-MM-DD` placeholder in the v1.2 entry's H2 header with the actual ship date.
- [ ] Commit both file changes as `chore(24): finalize v1.2 ship — STATE.md + MILESTONES.md ship date`.

---

## 8. Run `/gsd-complete-milestone v1.2`

Per CONTEXT D-12, `/gsd-complete-milestone v1.2` is a SEPARATE post-final-tag command. It runs AFTER § 7's commit + tag push verifies clean. The command:

1. Archives `.planning/milestones/v1.2-ROADMAP.md` and `.planning/milestones/v1.2-REQUIREMENTS.md` (snapshots of the current `.planning/ROADMAP.md` v1.2 zone and `.planning/REQUIREMENTS.md`).
2. Rewrites the main `.planning/ROADMAP.md` with milestone groupings (mirrors the v1.0 + v1.1 archive moves).
3. Commits the archive.
4. Runs the PROJECT.md evolution review.
5. Offers to create the next milestone (v1.3) inline — maintainer decides whether to accept or defer.

- [ ] `/gsd-complete-milestone v1.2` invoked from a fresh Claude session.
- [ ] `.planning/milestones/v1.2-ROADMAP.md` + `v1.2-REQUIREMENTS.md` archive files created.
- [ ] Main `.planning/ROADMAP.md` updated with v1.2 grouped under "Milestones" section.
- [ ] PROJECT.md evolution review run (may or may not produce edits — maintainer's call).
- [ ] v1.3 milestone created OR deferred (maintainer's call).

---

## Sign-off

All sections above must be ticked `[x]` by the maintainer.

- [ ] All sections (§ 1 – § 8) above ticked.
- [ ] Tag pushed: `v1.2.0` (bit-identical to the last-passing-UAT rc.N SHA).
- [ ] `:latest` now equals `:1.2.0` digest (verified post-publish — `:latest` invariant FLIPPED from rc preflights).
- [ ] No regressions reported in the first 24 h post-publish.

| Field | Value |
|-------|-------|
| Maintainer signature | `__________________` |
| Date (UTC) | `__________________` |
| Final tag commit SHA (== rc.N SHA) | `__________________` |
| rc.N tag UAT-validated (== retagged SHA source) | `__________________` (e.g., `v1.2.0-rc.4`) |
| GHCR amd64 digest (`:1.2.0`) | `__________________` |
| GHCR arm64 digest (`:1.2.0`) | `__________________` |
| GHCR `:latest` digest (MUST NOW equal `:1.2.0` digest) | `__________________` |
| GHCR `:1.2` digest (MUST equal `:1.2.0` digest) | `__________________` |
| GHCR `:1` digest (MUST equal `:1.2.0` digest) | `__________________` |
| Previous `:latest` digest (was `:1.1.0`) | `__________________` |
| `/gsd-complete-milestone v1.2` completed | `__________________` (y/n) |

---

## Out of scope

- `release.yml` / `cliff.toml` / `docs/release-rc.md` modifications — reused verbatim per P12 D-10..D-12 / P24 D-10 / D-16 informational.
- `Cargo.toml` version bump — stays at `1.2.0` (matches the final tag prefix per `feedback_tag_release_version_match.md`).
- New code features — Phase 24 is paperwork + final-ship.
- `/gsd-complete-milestone v1.2` itself — invoked separately AFTER this runbook completes (D-12); the archive + ROADMAP rewrite + PROJECT.md evolution review happen in that command, not here.
- v1.3 milestone kickoff — happens inside `/gsd-complete-milestone v1.2` (offer accepted/deferred at maintainer's discretion).

---

## Cross-reference

This runbook mirrors v1.1 P14 D-16 (`v1.1.0 = retag the rc.3 SHA`) with the substitution:

- `v1.1.0` → `v1.2.0`
- `rc.3` → `last-passing-UAT rc.N` (rc.4 if UAT passed first time; rc.5+ if iterated)
- Tag message `"v1.1 — Operator Quality of Life"` → `"v1.2 — Operator Integration & Insight"`
- `:latest` invariant flipped: `:latest` now MUST equal `:1.2.0` digest (rather than staying at `:1.1.0` as it did during the rc cuts).

The sign-off table mirrors `21-RC2-PREFLIGHT.md` § Sign-off with the `:latest` invariant flipped + added rows for `:1.2` / `:1` four-tag equality verification.
