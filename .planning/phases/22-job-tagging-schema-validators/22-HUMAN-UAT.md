---
phase: 22
plan: 06
title: "Phase 22 — Job Tagging Schema + Validators — Human UAT"
autonomous: false
maintainer_validated: true
created: 2026-05-04
requirements: [TAG-01, TAG-02, TAG-03, TAG-04, TAG-05]
status: pending
---

# Phase 22 — Maintainer UAT Runbook

> **`autonomous: false`** — Claude does NOT mark this UAT passed.
> Per project memory `feedback_uat_user_validates.md`: every UAT step
> requires maintainer execution + eyeball validation; Claude's
> automated tests cover the unit/integration surface (Plan 05), but
> the operator-readable error UX, the WARN-line shape, and the
> end-to-end webhook delivery require human judgment.
>
> Per project memory `feedback_uat_use_just_commands.md`: every
> scenario below references a `just` recipe (Plan 05 / D-11). NO
> ad-hoc `cargo` / `docker` / curl invocations.

## Prerequisites

- [ ] Plans 01–05 are merged (or applied locally on a feature branch).
- [ ] `cargo build` succeeds on the working tree.
- [ ] `cargo test --test v12_tags_validators` exits 0 (Plan 05 lock).
- [ ] `just --list` shows `uat-tags-persist`, `uat-tags-validators`, `uat-tags-webhook`.

## Scenario 1 — Persistence spot-check (TAG-02 / D-10 step 1)

**Goal:** Confirm a TOML with `tags = ["backup", "weekly", "prod"]` round-trips to the `jobs.tags` column as the sorted-canonical JSON `["backup","prod","weekly"]`.

**Steps:**

1. Run `just uat-tags-persist` from a fresh terminal.
2. Watch for the recipe's output of `SELECT name, tags FROM jobs WHERE name = 'uat-tags-persist-demo';`.
3. **Eyeball criterion:** the `tags` column shows the JSON string `["backup","prod","weekly"]` exactly (sorted-canonical; alphabetized).

**Sign-off:**

- [ ] Scenario 1 passed (column shows the expected sorted JSON array).

## Scenario 2 — Validator error UX walk (TAG-03 + TAG-04 + TAG-05 + D-08 / D-10 step 2)

**Goal:** Confirm each rejection produces an operator-readable error.

**Steps:**

1. Run `just uat-tags-validators`.
2. For each emitted error, confirm:
   - **Case 1 (charset, TAG-04):** input `tags = ["MyTag!"]`. Error message names the offending tag and the regex (`^[a-z0-9][a-z0-9_-]{0,30}$`). Example expected fragment: ``tags fail charset ... `mytag!` ``.
   - **Case 2 (reserved, TAG-04):** input `tags = ["cronduit"]`. Error message names the reserved list `["cronduit", "system", "internal"]`.
   - **Case 3 (substring-collision pair, TAG-05):** two jobs with `["back"]` and `["backup"]`. Exactly ONE error message of the shape: ``tag 'back' (used by '<job-a>') is a substring of 'backup' (used by '<job-b>'); rename or remove one to avoid SQL substring false-positives at filter time.``
   - **Case 4 (count cap, D-08):** a job with 17 tags. Error: ``[[jobs]] '<name>': has 17 tags; max is 16. Remove tags or split into multiple jobs.``

**Eyeball criteria for "operator-readable":**

- Each error names the offending VALUE (not just the field).
- Each error states the RULE violated (charset, reserved, collision, cap).
- Each error suggests a FIX ("Rename or remove these tags", "Split into multiple jobs").

**Sign-off:**

- [ ] Scenario 2 passed (all four cases produce operator-readable errors).

## Scenario 3 — Dedup-collapse WARN (TAG-03 / D-10 step 3)

**Goal:** Confirm the WARN line names the ORIGINAL inputs that collapsed.

**Steps:**

1. Run `just uat-tags-validators` (the dedup case is part of the same recipe).
2. For input `tags = ["Backup", "backup ", "BACKUP"]`, watch for a `tracing::warn!` line.

**Eyeball criterion:** the WARN line names ALL THREE original inputs (e.g., `tags ["Backup", "backup ", "BACKUP"] collapsed to ["backup"]`), NOT just the canonical form `["backup"]`. The operator must be able to tell that they wrote three things and cronduit treated them as one.

**Reference WARN shape** (from CONTEXT.md `<specifics>` lines 548-553):

> `WARN job 'nightly-backup': tags ["Backup", "backup ", "BACKUP"] collapsed to ["backup"] (case + whitespace normalization)`

**Sign-off:**

- [ ] Scenario 3 passed (WARN line names original inputs + canonical form).

## Scenario 4 — End-to-end webhook backfill (WH-09 / D-10 step 4)

**Goal:** Confirm a webhook configured on a tagged failing job delivers a payload containing real tag values. This is the WH-09 closure proof end-to-end.

**Steps:**

1. Run `just uat-tags-webhook` (chains `just uat-webhook-mock` from P18).
2. Run `just uat-webhook-verify` to surface the last 30 lines of `/tmp/cronduit-webhook-mock.log`.
3. Inspect the delivered POST body in the log.

**Eyeball criterion:** the JSON body contains the substring `"tags":["backup","weekly"]` (sorted-canonical order). NOT `"tags":[]` (the Phase 18 placeholder is gone). NOT `"tags":["weekly","backup"]` (insert-order would be a regression).

**Reference payload shape** (excerpt):

```json
{
  "payload_version": "v1",
  "event_type": "run_finalized",
  "job_name": "uat-tags-webhook-demo",
  "tags": ["backup", "weekly"],
  ...
}
```

**Sign-off:**

- [ ] Scenario 4 passed (delivered payload contains real tag values; WH-09 closed end-to-end).

## Final sign-off

When all four scenarios above are checked:

- [ ] **Maintainer:** I have run all four scenarios on a clean working tree against a feature branch with Plans 01–05 applied. Each scenario produced the expected operator-readable output. WH-09 is closed end-to-end. Phase 22 is UAT-complete and ready to merge.

Maintainer name: ________
Date: ________
