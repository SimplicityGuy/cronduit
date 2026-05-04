---
phase: 22-job-tagging-schema-validators
verified: 2026-05-04T00:00:00Z
status: human_needed
score: 14/14 must-haves verified (autonomous surface)
overrides_applied: 0
re_verification: null
human_verification:
  - test: "22-HUMAN-UAT.md Scenario 1 — TOML→DB persistence spot-check"
    expected: "`just uat-tags-persist` prints `jobs.tags = '[\"backup\",\"prod\",\"weekly\"]'` in sorted-canonical order"
    why_human: "Eyeball validation of operator-readable shell output; per project memory feedback_uat_user_validates.md, Claude does NOT mark UAT passed."
  - test: "22-HUMAN-UAT.md Scenario 2 — validator error UX walk"
    expected: "`just uat-tags-validators` emits readable errors for charset, reserved, substring-pair, and 17-tag count cases."
    why_human: "Operator-readability of error messages requires human judgment; validator firing is covered by tests but message clarity is not."
  - test: "22-HUMAN-UAT.md Scenario 3 — TAG-03 dedup-collapse WARN"
    expected: "Same recipe shows a WARN line naming raw inputs `\"Backup\", \"backup \", \"BACKUP\"` collapsing to `\"backup\"`."
    why_human: "WARN-line readability + operator surface; structural emission already covered by unit + integration tests."
  - test: "22-HUMAN-UAT.md Scenario 4 — end-to-end webhook backfill (WH-09 closure)"
    expected: "`just uat-tags-webhook` chains webhook-mock → fire → verify and the captured POST body contains `\"tags\":[\"backup\",\"weekly\"]`."
    why_human: "Live-process webhook delivery exits the test harness — only a maintainer-driven run validates real wire payload."
---

# Phase 22: Job Tagging Schema + Validators — Verification Report

**Phase Goal:** Operators can attach normalized organizational tags to jobs in TOML config; tags persist to a new JSON column on `jobs`, validate against a strict charset + reserved-name list at config-load, and reject substring-collisions across the fleet. Phase 22 also closes the WH-09 webhook-payload `tags` placeholder shipped by Phase 18.

**Verified:** 2026-05-04
**Status:** READY FOR PR (PENDING MAINTAINER UAT)
**Re-verification:** No — initial verification.

---

## Goal Achievement

### Observable Truths (Requirement-Level)

| #   | Requirement | Truth                                                                                                                                                                            | Status     | Evidence                                                                                                                                                                                                              |
| --- | ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | TAG-01      | `JobConfig.tags: Vec<String>` exists with `#[serde(default)]`; **NOT** present on `DefaultsConfig`.                                                                              | VERIFIED   | `src/config/mod.rs:170` `pub tags: Vec<String>`; `src/config/mod.rs:87-111` `DefaultsConfig` has no `tags` field. Defaults merge in `src/config/defaults.rs::apply_defaults` does not touch `tags` (verified by grep). |
| 2   | TAG-02      | `jobs.tags TEXT NOT NULL DEFAULT '[]'` migration exists on both backends; `upsert_job` writes `tags_json`; `get_run_by_id` projects `j.tags` into `DbRunDetail.tags`.            | VERIFIED   | `migrations/sqlite/20260504_000010_jobs_tags_add.up.sql` (line 19, no IF NOT EXISTS); `migrations/postgres/...` (line 17, with IF NOT EXISTS). `src/db/queries.rs:67-141` upsert; `:1407-1487` SELECT join with tags. |
| 3   | TAG-03      | Normalize (trim + lowercase) + dedup-collapse emits `tracing::warn!` naming raw inputs.                                                                                          | VERIFIED   | `src/config/validate.rs:425-545` `check_tag_charset_and_reserved` normalizes pre-charset; `:533` warn! references raw + canonical. Captured-MakeWriter unit test at `:2103`. Integration test at `tests/v12_tags_validators.rs:325`. |
| 4   | TAG-04      | Charset `^[a-z0-9][a-z0-9_-]{0,30}$` + `RESERVED_TAGS = ["cronduit","system","internal"]` + empty/whitespace rejection.                                                          | VERIFIED   | `src/config/validate.rs:27` `TAG_CHARSET_RE`; `:33` `RESERVED_TAGS` const slice with exactly the three entries; empty/whitespace rejected at `:454`. Integration tests cover charset, reserved, empty, and capital-normalizes paths. |
| 5   | TAG-05      | Fleet-level substring-collision uses `s1.contains(s2)` (NOT regex); 1 error per pair; identical tags across jobs produce 0 errors; three-way produces 3 errors.                  | VERIFIED   | `src/config/validate.rs:611-675` `check_tag_substring_collision` uses plain `str::contains`; error message at `:667-670` matches the spec wording verbatim. Integration tests `:222`, `:256`, `:281` cover the three cases. |
| 6   | D-08        | `MAX_TAGS_PER_JOB = 16` enforced after dedup via `check_tag_count_per_job`.                                                                                                      | VERIFIED   | `src/config/validate.rs:40` constant; `:549-578` validator. Integration test `tag_count_cap_17_rejected` (`:295`) fires exactly one error.                                                                              |
| 7   | D-04        | Validator order: charset+reserved + count-cap inside per-job loop; substring-collision AFTER per-job loop.                                                                       | VERIFIED   | `src/config/validate.rs:67-93` `run_all_checks` body — `check_tag_charset_and_reserved` + `check_tag_count_per_job` inside the per-job loop; `check_tag_substring_collision(&cfg.jobs, ...)` is called after the loop. |
| 8   | D-01        | Tags excluded from `compute_config_hash`; regression test locks behavior.                                                                                                        | VERIFIED   | `src/config/hash.rs:51-55` `// DO NOT include tags` comment; function body never inserts `tags`. Test `tags_excluded_from_hash` at `:340` verifies tag-only edits hash identically.                                  |
| 9   | D-02        | Tags NOT in `serialize_config_json`.                                                                                                                                              | VERIFIED   | `src/scheduler/sync.rs:67-107` `serialize_config_json` body — no `tags` insert. Comment cross-reference at `src/config/hash.rs:55`.                                                                                  |
| 10  | WH-09       | `WebhookPayload.tags = run.tags.clone()`; placeholder breadcrumb test gone; `payload_tags_carries_real_values` test exists and asserts wire JSON `"tags":["backup","weekly"]`.   | VERIFIED   | `src/webhooks/payload.rs:91` `tags: run.tags.clone()`; `:252-273` new test asserts the wire JSON. Grep for `until_p22` / `Empty.*until.*Phase 22` returns zero hits anywhere under `src/`.                          |

### Operator-Facing Scaffolding

| #  | Item                                                                                                                                                                                            | Status   | Evidence                                                                                                                                                                                                     |
| -- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 11 | Three `just uat-tags-*` recipes exist (`uat-tags-persist`, `uat-tags-validators`, `uat-tags-webhook`).                                                                                          | VERIFIED | `justfile:1182`, `:1234`, `:1370` all present and bodied.                                                                                                                                                    |
| 12 | `examples/cronduit.toml` shows `tags = [...]` on exactly one existing demo job.                                                                                                                  | VERIFIED | `examples/cronduit.toml:143` `tags = ["demo", "hello"]`. Grep count = 1 occurrence of `tags = [`.                                                                                                            |
| 13 | `tests/v12_tags_validators.rs` covers each rejection path + WARN + round-trip persistence.                                                                                                       | VERIFIED | 10 tests, all pass. Coverage: charset, reserved, capital-normalize, empty/whitespace, pair collision, three-way collision, identical-tags-no-error, count-cap, dedup WARN, full TOML→upsert→get_run_by_id round-trip. |
| 14 | `22-HUMAN-UAT.md` autonomous=false; four scenarios; references `just uat-tags-*` recipes; no checkboxes pre-ticked.                                                                              | VERIFIED | Frontmatter `autonomous: false`. Four scenarios at L26, L46, L73, L92. All `[ ]` (none `[x]`). Recipe references at L38, L52, L75, L94.                                                                       |

---

## Negative-Invariant Table (Out-of-Scope Absences)

| Invariant                                                                                          | Status | Evidence                                                                                                                                                                                                                                                                |
| -------------------------------------------------------------------------------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `[defaults].tags` NOT supported (TAG-01 per-job only)                                              | PASS   | `src/config/mod.rs:87-111` `DefaultsConfig` has no `tags`. Adding `tags` to `[defaults]` TOML would deserialize-error. `apply_defaults` never references `job.tags`.                                                                                                  |
| Tags NOT a Prometheus metric label                                                                  | PASS   | Grep for `tag.*label` / `cronduit_.*tag` in `src/` returns zero hits in metrics code paths. Metric families unchanged.                                                                                                                                                  |
| No UI/dashboard chip work landed                                                                    | PASS   | No edits in `src/web/`, `src/templates/`, or `assets/` related to tag chips. Phase 23 still pending in ROADMAP.                                                                                                                                                          |
| Tags NOT in `compute_config_hash` (D-01)                                                            | PASS   | `src/config/hash.rs:16-66` body never inserts `tags`; comment at L51-55. Regression test `tags_excluded_from_hash` proves behavior.                                                                                                                                      |
| Tags NOT in `serialize_config_json` (D-02)                                                          | PASS   | `src/scheduler/sync.rs:67-107` body has no `tags` insert.                                                                                                                                                                                                                |
| No new external crates (D-17)                                                                       | PASS   | `cargo tree -i openssl-sys` returns "did not match any packages" (rustls invariant intact). No `Cargo.toml` adds in this phase (`serde_json`, `regex`, `once_cell` already in tree).                                                                                  |
| Placeholder test `payload_tags_empty_array_until_p22` removed                                       | PASS   | `grep -rn "until_p22"` in `src/` returns zero hits. Replaced by `payload_tags_carries_real_values` at `src/webhooks/payload.rs:252`.                                                                                                                                  |
| Three-file migration tightening NOT used                                                            | PASS   | Single `up.sql` per backend (no `tighten` / `null_to_default` / `add_not_null` triplet). Migration sequence number `_010` is the next slot after `_009_scheduled_for_add`.                                                                                              |

---

## Quality-Gate Table

| Gate                                                                                | Status | Output                                                                                                          |
| ----------------------------------------------------------------------------------- | ------ | --------------------------------------------------------------------------------------------------------------- |
| `cargo build`                                                                       | PASS   | `Finished dev profile [unoptimized + debuginfo] target(s) in 0.57s`                                             |
| `cargo fmt --all -- --check`                                                        | PASS   | (no output — clean)                                                                                             |
| `cargo clippy --all-targets --all-features -- -D warnings`                          | PASS   | `Finished dev profile [unoptimized + debuginfo] target(s) in 0.52s`                                             |
| `cargo test --lib --quiet`                                                          | PASS   | `test result: ok. 323 passed; 0 failed; 1 ignored; 0 measured`                                                  |
| `cargo test --test v12_tags_validators --quiet`                                     | PASS   | `test result: ok. 10 passed; 0 failed; 0 ignored`                                                               |
| `cargo test --test schema_parity --quiet` (SQLite leg)                              | PASS   | 2 of 3 cases passed; the 3rd (Postgres testcontainer) requires Docker — known local-skip; CI runs it.            |
| `cargo tree -i openssl-sys`                                                         | PASS   | `error: package ID specification 'openssl-sys' did not match any packages` — rustls invariant intact (D-17).    |

---

## Risk + Open Items Table

| Item                                                                                                                                  | Severity | Action                                                                                                                                                                          |
| ------------------------------------------------------------------------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Postgres-leg of `tests/schema_parity.rs` skipped locally (Docker socket absent).                                                       | INFO     | Expected; CI runs the testcontainer matrix per the project's CI invariant. Not a phase blocker.                                                                                  |
| Maintainer UAT not yet executed (per project memory `feedback_uat_user_validates.md`).                                                  | EXPECTED | Four scenarios in `22-HUMAN-UAT.md` await maintainer sign-off (`autonomous: false`). PR cannot be marked rc-ready until those four `[ ]` items are checked by the maintainer. |
| `examples/cronduit.toml` adds `tags = ["demo", "hello"]` on the hello-world job — operator copy-pasting may carry the demo tag values forward. | INFO     | Comment at L139-142 already calls this out. No action.                                                                                                                          |

No BLOCKER-class risks. No code-level gaps. All autonomous quality gates green.

---

## Final Verdict

**READY FOR PR (PENDING MAINTAINER UAT)**

All requirements (TAG-01, TAG-02, TAG-03, TAG-04, TAG-05, WH-09) and locked decisions (D-01, D-02, D-04, D-08, D-17) are satisfied in the codebase. Every observable success criterion from `.planning/ROADMAP.md` Phase 22 is verified by code + test evidence. Out-of-scope invariants hold by absence. Quality gates are green. The four scenarios in `22-HUMAN-UAT.md` remain `pending` per `autonomous: false` — those are the gating items between this PR and rc-ready status, and the maintainer (not Claude) validates them.

---

_Verified: 2026-05-04_
_Verifier: Claude (gsd-verifier)_
