# Phase 14 HUMAN UAT — Bulk Enable/Disable + rc.3 → v1.1.0 Promotion

**Status:** awaiting user validation
**Owner:** Robert (maintainer)
**Prerequisites:** Phase 14 merged to `main`; `v1.1.0-rc.3` tag cut + GHCR image pushed
**Honors:** auto-memory `feedback_uat_user_validates.md` + `feedback_uat_use_just_commands.md` + `feedback_tag_release_version_match.md`

> Claude does NOT mark UAT passed. Each checkbox below must be ticked by a human
> running the command, verifying the expected outcome, and observing reality.
> Claude drafted this document; the maintainer executes it.

---

## How to Use This Document

1. Read the entire document end-to-end ONCE before starting Step 1, so the overall flow + the rc.3 → v1.1.0 promotion sequence are familiar.
2. Work through the Pre-UAT Checklist top-to-bottom; tick boxes only after personally observing the expected behavior.
3. Walk through Steps 1–8 in order; each step builds on prior state (Step 6 expects Step 3's selection still applied; Step 7 expects Step 6's clear already done).
4. Tick a step's validation checkbox ONLY after observing the expected behavior firsthand. Do NOT pre-tick boxes; do NOT tick a box because "it should work."
5. If any checkbox in any step fails, STOP. Document what you observed in `.planning/phases/14-bulk-enable-disable-rc-3-final-v1-1-0-ship/14-08-UAT-RESULTS.md` (or comment in this file), and follow the "If any box above is unticked, UAT FAILS" guidance in the Sign-Off section.
6. Once ALL boxes (Pre-UAT + 8 steps + Sign-Off) are ticked, proceed to the Post-UAT promotion sequence.

Estimated total time: 30–45 minutes assuming Step 3's running job uses a 30s `sleep`. Add 5+ minutes per silence-confirmation observation in Step 3.

---

## Pre-UAT Checklist

Run these BEFORE Step 1. If any check fails, do NOT proceed — fix the prerequisite, then re-run the failing check.

- [ ] `v1.1.0-rc.3` tag exists on `origin`: `git ls-remote --tags origin | grep v1.1.0-rc.3` returns one line.
- [ ] rc.3 image is pullable from GHCR: `docker manifest inspect ghcr.io/simplicityguy/cronduit:1.1.0-rc.3` returns a multi-arch manifest with two platforms (`linux/amd64` + `linux/arm64`).
- [ ] `:rc` rolling tag points at the same digest as `:1.1.0-rc.3`: `docker manifest inspect ghcr.io/simplicityguy/cronduit:rc` returns the same digest as `docker manifest inspect ghcr.io/simplicityguy/cronduit:1.1.0-rc.3` (D-10 release.yml gating).
- [ ] `:latest` still points at `v1.0.1` (D-10 gating must have skipped `:latest` on the rc.3 push): `docker manifest inspect ghcr.io/simplicityguy/cronduit:latest` digest matches the pre-rc.3 v1.0.1 digest. If `:latest` advanced on the rc.3 push, that is a release.yml regression — STOP and treat as a hotfix event, not a Phase 14 UAT.
- [ ] All four release-group `just` recipes are present and parsed cleanly: `just --list | grep -E 'compose-up-rc3|^[[:space:]]*reload|^[[:space:]]*health|^[[:space:]]*metrics-check'` lists FOUR distinct recipes (Plan 07 Warning #8 — Steps 1, 4, 7, 8 wrap their probes in `just` recipes per `feedback_uat_use_just_commands.md`).
- [ ] `examples/docker-compose.yml` honors `${CRONDUIT_IMAGE:-…}` so `just compose-up-rc3` can pin rc.3 via env var: `grep -F '${CRONDUIT_IMAGE' examples/docker-compose.yml` returns a match.
- [ ] No cronduit container running on the UAT host: `docker ps --filter name=cronduit --format '{{.Names}}'` is empty (so we start from a clean state).
- [ ] At least 3 jobs are configured in `examples/cronduit.toml`. If fewer, append two dummy `command` jobs (e.g., `command = "sleep 30"` on a `*/5 * * * *` schedule) so Step 3's "bulk-disable 3" precondition is satisfiable.

---

## UAT Steps

Each step is a `just` recipe (or a single browser action) plus an explicit expected outcome and one or more validation checkboxes. Tick a box ONLY after you have personally observed the expected behavior.

### Step 1 — Compose up pinned to rc.3

**Command:** `just compose-up-rc3`

**Expected:**
- Pulls `ghcr.io/simplicityguy/cronduit:1.1.0-rc.3` if not already present locally.
- Container starts in detached mode.
- Within 90 seconds, `docker ps --filter name=cronduit --format '{{.Names}} {{.Status}}'` reports `cronduit Up N seconds (healthy)`.
- `just health` (which wraps `curl -sf /health | jq -r '.status'` per Plan 07 Task 2 — Warning #8 mitigation) prints exactly `healthy` to stdout and exits 0.

**Validation:**
- [ ] Container reports `(healthy)` within 90s of `just compose-up-rc3` returning.
- [ ] `just health` prints `healthy` (NOT `starting`, NOT `unhealthy`, NOT empty).
- [ ] Image digest pulled matches the rc.3 manifest from the Pre-UAT checklist (compare `docker inspect cronduit --format '{{.Image}}'` to the manifest digest).

### Step 2 — Dashboard shows new bulk-select chrome

**Command:** Open `http://127.0.0.1:8080/` in a browser.

**Expected:**
- A new leftmost checkbox column is visible on every job row.
- The header row contains a select-all checkbox.
- No bulk action bar is visible while zero rows are checked.
- Click one row checkbox → a sticky action bar appears between the filter input and the table, showing `{N} selected` plus three buttons: `Disable selected`, `Enable selected`, `Clear`.
- Click the header select-all checkbox → all visible-filtered rows become checked; the bar's count updates to match.
- Type a substring into the filter box → select-all selects only the visible filtered rows (not the hidden ones).
- Scroll the page down past the table → the action bar stays attached to the viewport top (`position: sticky`).
- Wait at least 6 seconds with rows still ticked → the 3-second HTMX poll runs at least once; row checkboxes keep their checked state across the swap (`hx-preserve` + stable `id`).

**Validation:**
- [ ] Checkbox column visible on every job row (no row missing the column).
- [ ] Header select-all toggles every visible-filtered row (and only those).
- [ ] Action bar appears on first selection, hides when zero rows are ticked, and shows the correct `{N} selected` count.
- [ ] Action bar sticks to viewport top while scrolling.
- [ ] Row checkboxes survive the 3s HTMX poll without losing their checked state (wait 6+ seconds with selection active).
- [ ] Browser console is clean — no JS errors from the inline `__cdBulk*` helpers.

### Step 3 — Bulk-disable 3 jobs including one currently-running (ERG-02)

**Preparation:**
- Ensure at least 3 jobs are present (Pre-UAT step took care of this).
- Use the per-row "Run Now" button on a job whose command runs ~30 seconds (e.g., a `sleep 30` command job). Confirm the run is in `running` status before continuing.

**Command:** In the dashboard, check the row checkboxes for THREE jobs INCLUDING the currently-running one, then click `Disable selected` in the action bar.

**Expected:**
- Toast fires verbatim: `3 jobs disabled. 1 currently-running job will complete naturally.` (singular "job", NOT "jobs"; Copywriting Contract M==1 branch.)
- The two non-running jobs stop firing — wait at least 5 minutes past their next scheduled slot and confirm no new run rows appear for them on the dashboard.
- The currently-running job continues until its natural terminal status. When the run finishes, it lands as `success` / `failed` / `timeout` — NEVER `stopped` (bulk disable does NOT terminate running jobs; that is the literal ERG-02 promise).
- After the running job's natural termination, that third job also stops firing on subsequent cron slots.
- `just metrics-check` (Step 8 — preview now if convenient) shows NO new increments to `cronduit_runs_total{...,status="stopped"}` attributable to this bulk-disable action.

**Validation:**
- [ ] Toast copy matches verbatim: `3 jobs disabled. 1 currently-running job will complete naturally.` (no editorial drift; if M-count rendering reads "0 currently-running" or omits the second sentence, the verbose-toast variant did not fire and Step 3 FAILS).
- [ ] Running job's terminal status is `success` / `failed` / `timeout` — NOT `stopped`.
- [ ] Other 2 jobs stop firing (confirmed by a 5+ minute observation past their next scheduled slot).

### Step 4 — Reload preserves override (ERG-04)

**Command:** `just reload`

**Expected:**
- Recipe prints `SIGHUP sent to cronduit container` and exits 0.
- `docker logs cronduit --tail 20` shows a `config reload` event from the scheduler within a second of the recipe completing.
- All 3 bulk-disabled jobs from Step 3 STAY disabled after the reload — `upsert_job` does not touch `enabled_override` (T-V11-BULK-01 invariant).
- Dashboard reflects the still-disabled state within 3 seconds (next HTMX poll); the disabled jobs do NOT reappear on the active schedule.

**Validation:**
- [ ] Reload event visible in `docker logs cronduit --tail 20`.
- [ ] All 3 bulk-disabled jobs are still disabled after `just reload` completes.
- [ ] Dashboard does not show the disabled jobs returning to active state on the next poll cycle.

### Step 5 — Settings "Currently Overridden" audit (ERG-03)

**Command:** Open `http://127.0.0.1:8080/settings` in a browser.

**Expected:**
- A new "Currently Overridden" `<section>` appears below the existing 6-card status grid.
- The section is full-width and visually consistent with the dashboard table (same row hover, same spacing tokens).
- The section lists exactly the 3 jobs bulk-disabled in Step 3.
- Rows are ordered alphabetically by job name (D-10b stable ordering).
- Each row shows three columns: `Name | Override State | Clear`.
- The Override State column renders a `DISABLED` badge (terminal-yellow `--cd-status-disabled` token from Phase 10's design system; reused unchanged).
- Each row has an inline `Clear` button on the right.

**Validation:**
- [ ] "Currently Overridden" section is visible on `/settings` (not on any other page).
- [ ] Exactly 3 jobs listed (matches the Step 3 selection).
- [ ] Rows are alphabetical by name.
- [ ] DISABLED badge uses the terminal-yellow color token (matches Phase 10 disabled badge styling).
- [ ] Each row has a `Clear` button on the right.

### Step 6 — Per-row Clear button restores override (ERG-03)

**Command:** On the `/settings` page, click `Clear` on one of the 3 rows in the Currently Overridden section.

**Expected:**
- Toast fires verbatim: `1 job: override cleared.` (singular "job"; Copywriting Contract N==1 branch — the per-row Clear button uses the same multi-row formatter as the bulk bar with `rows_affected=1`, so operators see consistent wording everywhere).
- Reload `/settings` (browser refresh): the cleared job has dropped off the Currently Overridden list; only 2 jobs now remain in the section.
- Navigate to `/` (dashboard): the cleared job has returned to active scheduling and will fire on its next cron slot.

**Validation:**
- [ ] Toast appears with verbatim copy: `1 job: override cleared.` (singular "job"; trailing period; no plural drift).
- [ ] After `/settings` reload, only 2 jobs remain in Currently Overridden.
- [ ] On `/`, the cleared job is back in the active schedule (verify by waiting for its next cron slot OR by clicking `Run Now` and observing a successful run).

### Step 7 — Remove a still-disabled job from config + reload (ERG-04 symmetry)

**Command:**
1. Edit `examples/cronduit.toml` and DELETE one of the 2 still-disabled jobs (the ones still listed in Currently Overridden after Step 6). Leave the other 1 still-disabled job in place.
2. `just reload`

**Expected:**
- Removed job: `enabled_override` is cleared at the same time as `enabled = 0` (via `disable_missing_jobs` extension — D-13 / ERG-04 symmetry).
- Remaining still-disabled job: keeps `enabled_override = 0` after reload.
- Dashboard (`/`): the removed job disappears from the table entirely (not just disabled — fully gone).
- `/settings` Currently Overridden: the removed job is gone from the section; exactly 1 job is still listed.
- Re-add the removed job to `examples/cronduit.toml` and `just reload` again → it reappears on the dashboard as ACTIVE (no stale `enabled_override` row). This is the regression check for ERG-04 symmetry — proves `disable_missing_jobs` cleared the override along with `enabled = 0`.

**Validation:**
- [ ] Removed job disappears from BOTH the dashboard AND the `/settings` Currently Overridden list after `just reload`.
- [ ] Remaining still-disabled job continues to show DISABLED in the Currently Overridden section.
- [ ] Re-adding the removed job to config + `just reload` returns it to ACTIVE state on the dashboard (no stale `enabled_override = 0` carryover from the prior life).
- [ ] Re-added job confirmed running on its next cron slot OR via `Run Now` triggering a successful run.

### Step 8 — `/metrics` health

**Command:** `just metrics-check`

**Expected:**
- Recipe wraps the raw `/metrics` curl per Plan 07 Task 2 (Warning #8 mitigation — no raw HTTP calls in UAT).
- Output contains `cronduit_scheduler_up 1` (liveness gauge from observability stack).
- Output contains `cronduit_runs_total{...}` lines covering the runs that fired during this UAT.
- Output does NOT show NEW `cronduit_runs_total{...,status="stopped"}` increments attributable to this UAT's bulk-disable actions (Step 3 running jobs completed naturally; the `stopped` counter — if present at all — is unchanged from the pre-UAT baseline).
- Recipe exits 0.

**Validation:**
- [ ] `just metrics-check` exits 0 and prints both `cronduit_scheduler_up` and `cronduit_runs_total` lines.
- [ ] `cronduit_scheduler_up 1` is present (scheduler liveness confirmed).
- [ ] No NEW `cronduit_runs_total{...,status="stopped"}` increments attributable to the bulk-disable in Step 3 (compare against a baseline `just metrics-check` snapshot taken right after Step 1 if necessary).

---

## UAT Sign-Off

- [ ] All 8 step-level validation checklists are fully ticked.
- [ ] No observed regressions in existing Phase 10–13 behavior — Stop button still terminates a running job; timeline page still renders; sparkline still draws; `:rc` rolling tag still resolves; healthcheck still reports `healthy`.
- [ ] Browser console is clean — no JS errors from the inline `__cdBulk*` helpers across any page (`/`, `/settings`, job detail).
- [ ] All toast strings render verbatim per the Copywriting Contract — no editorial drift, no missing periods, no singular/plural mistakes.

**If any box above is unticked, UAT FAILS — do NOT promote rc.3 to v1.1.0.** Open a follow-up plan in this phase to fix the failing behavior, cut a `v1.1.0-rc.4` tag, and re-run this entire HUMAN-UAT against rc.4.

---

## Post-UAT: v1.1.0 Promotion Sequence (Maintainer Action)

Once ALL boxes above are ticked, execute the promotion commands VERBATIM from `docs/release-rc.md` (Phase 12 runbook — D-14 reuse). Reproduced here in the order Phase 14 D-16 / D-18 / D-19 expects:

```bash
# 1. Refresh local refs and confirm we are on the rc.3 merge commit
git fetch --tags
git checkout main && git pull --ff-only origin main
git log -1 --oneline   # sanity: should be the Phase 14 PR merge commit

# 2. Resolve the rc.3 SHA — v1.1.0 will retag the SAME commit per D-16
#    (guarantees byte-identical source between what UAT validated and what ships)
RC3_SHA=$(git rev-list -n 1 v1.1.0-rc.3)
echo "$RC3_SHA"        # sanity: should match HEAD if main is at the rc.3 merge commit

# 3. Preview cumulative release notes (D-19 — v1.0.1..v1.1.0 covers Phases 10/11/12/12.1/13/14)
git cliff --unreleased --tag v1.1.0 -o /tmp/v1.1.0-preview.md
cat /tmp/v1.1.0-preview.md
# Sanity-check: cumulative across Phases 10-14. NO hand-editing per D-19 — fix
# the cliff.toml or the commit messages on main if the grouping looks wrong.

# 4. Tag the rc.3 commit as v1.1.0 (signed preferred; annotated-fallback OK per
#    docs/release-rc.md Step 2a/2b)
git tag -a -s v1.1.0 -m "v1.1 — Operator Quality of Life" "$RC3_SHA"
git tag -v v1.1.0      # verify signature (or `git cat-file tag v1.1.0` if unsigned)

# 5. Push the tag — release.yml D-10 metadata-action gating auto-advances
#    :latest, :1, :1.1 because v1.1.0 has no hyphen (non-rc); :1.1.0 publishes
#    in parallel as the canonical version tag (D-18)
git push origin v1.1.0
gh run watch --exit-status   # follow release.yml; MUST complete green

# 6. Post-push verification (D-18) — :latest MUST advance, all stable tags MUST agree
./scripts/verify-latest-retag.sh 1.1.0
# MUST exit 0 — :latest digest must now equal :1.1.0 digest on BOTH amd64 + arm64.
docker manifest inspect ghcr.io/simplicityguy/cronduit:1.1.0
docker manifest inspect ghcr.io/simplicityguy/cronduit:1.1
docker manifest inspect ghcr.io/simplicityguy/cronduit:1
docker manifest inspect ghcr.io/simplicityguy/cronduit:latest
# All four MUST have IDENTICAL digests.
```

If `verify-latest-retag.sh` fails, `:latest` did NOT advance — that is a release.yml regression and is a hotfix event, NOT Phase 14 scope. Do not retroactively edit Phase 14 close-out commits to "fix" it; open a hotfix branch off `main` and address the gating bug there.

---

## Post-Promotion Close-Out (Plan 09 scope)

After `v1.1.0` is tagged and `:latest` has advanced, Plan 09 handles the milestone close-out commit:

- [ ] Plan 09 runs: `REQUIREMENTS.md` ERG-01..04 + DB-14 checkboxes flip `[ ] → [x]`.
- [ ] Plan 09 runs: `MILESTONES.md` v1.1 archive entry appended (follows the v1.0 entry shape per D-20).
- [ ] Plan 09 runs: `README.md` "Current State" paragraph updated to mark v1.1.0 as current stable.
- [ ] `/gsd-complete-milestone v1.1` invoked to archive the milestone artifacts (`.planning/milestones/v1.1-ROADMAP.md`, `.planning/milestones/v1.1-REQUIREMENTS.md`, `.planning/milestones/v1.1-MILESTONE-AUDIT.md`).

These items are NOT part of HUMAN-UAT — they happen automatically once Plan 09 runs. They are listed here so the maintainer knows what to expect after the v1.1.0 push.

---

*Doc created: by Phase 14 Plan 08 (autonomous: false — Claude drafts, human executes). Awaiting maintainer validation after rc.3 is tagged.*
