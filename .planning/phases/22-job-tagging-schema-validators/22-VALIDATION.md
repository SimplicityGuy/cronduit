---
phase: 22
slug: job-tagging-schema-validators
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-04
---

# Phase 22 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` + `cargo nextest` (existing) |
| **Config file** | `Cargo.toml`, `.cargo/config.toml`, `nextest.toml` |
| **Quick run command** | `just test-unit` (or `cargo test --lib config::validate -- --quiet`) |
| **Full suite command** | `just test` (full `cargo nextest run --all-features`) |
| **Estimated runtime** | ~30s unit / ~3-4 min full (per existing P21 baseline) |

---

## Sampling Rate

- **After every task commit:** Run `just test-unit`
- **After every plan wave:** Run `just test`
- **Before `/gsd-verify-work`:** Full suite must be green AND `cargo clippy --all-targets --all-features -- -D warnings` clean AND `cargo fmt --all -- --check` clean.
- **Max feedback latency:** ~30s (unit) / ~240s (full)

---

## Per-Task Verification Map

> Plan IDs (`22-NN-...`) are tentative — the planner finalizes them. Filled values reflect the suggested 5-plan grouping from CONTEXT.md `<decisions>` § Claude's Discretion. Updated by planner if it collapses/expands plans.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 22-01-01 | 01 (schema+serde+migration) | 1 | TAG-01, TAG-02 | — | TOML `tags = [...]` deserializes; `[defaults]` lacks the field | unit | `cargo test --lib config -- tags` | ❌ W0 | ⬜ pending |
| 22-01-02 | 01 | 1 | TAG-02 | — | `jobs.tags TEXT NOT NULL DEFAULT '[]'` migration applies on both backends; old rows default to `[]` | integration | `cargo test --test schema_parity` | ✅ | ⬜ pending |
| 22-02-01 | 02 (validators) | 2 | TAG-04 | T-V12-TAG-04 | Charset regex `^[a-z0-9][a-z0-9_-]{0,30}$` rejects invalid; reserved (`cronduit`, `system`, `internal`) rejects | unit | `cargo test --lib config::validate -- tag_charset_and_reserved` | ❌ W0 | ⬜ pending |
| 22-02-02 | 02 | 2 | TAG-03 | T-V12-TAG-03 | Normalization (lowercase + trim); WARN on dedup-collapse names inputs | unit | `cargo test --lib config::validate -- tag_normalize_warn` | ❌ W0 | ⬜ pending |
| 22-02-03 | 02 | 2 | TAG-05 | T-V12-TAG-05 | Substring-collision pass `s1.contains(s2) where s1 != s2` over union; one ConfigError per pair | unit | `cargo test --lib config::validate -- tag_substring_collision` | ❌ W0 | ⬜ pending |
| 22-02-04 | 02 | 2 | D-08 | T-V12-TAG-06 | Per-job count cap of 16 (post-dedup) emits one ConfigError per offending job | unit | `cargo test --lib config::validate -- tag_count_cap` | ❌ W0 | ⬜ pending |
| 22-03-01 | 03 (DB plumbing) | 3 | TAG-02 | — | `upsert_job` binds `tags_json: &str`; round-trip TOML→DB→`Vec<String>` via `get_run_by_id` | integration | `cargo test --test v12_tags_validators` | ❌ W0 | ⬜ pending |
| 22-03-02 | 03 | 3 | D-01 | T-V12-TAG-07 | `compute_config_hash` does NOT include `tags`; tag-only edits produce identical hash | unit | `cargo test --lib config::hash -- tags_excluded_from_hash` | ❌ W0 | ⬜ pending |
| 22-04-01 | 04 (WH-09 backfill) | 4 | WH-09 | — | `WebhookPayload::build` reads `run.tags.clone()`; payload JSON contains real `tags` array | unit | `cargo test --lib webhooks::payload -- payload_tags_carries_real_values` | ❌ W0 | ⬜ pending |
| 22-04-02 | 04 | 4 | WH-09 | — | Old test `payload_tags_empty_array_until_p22` removed; new test asserts non-empty tags round-trip | unit | `cargo test --lib webhooks::payload -- payload_tags` | ❌ W0 | ⬜ pending |
| 22-05-01 | 05 (UAT recipes + examples) | 5 | TAG-01..05, WH-09 | — | Three new `just` recipes (`uat-tags-persist`, `uat-tags-validators`, `uat-tags-webhook`) execute end-to-end | manual | (UAT — see Manual section) | ❌ W0 | ⬜ pending |
| 22-05-02 | 05 | 5 | — | — | `examples/cronduit.toml` shows `tags = [...]` syntax in context | grep | `grep -E '^tags = \[' examples/cronduit.toml` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

> Wave 0 = file scaffolding that must exist before downstream waves can produce green tests. Most of these are NEW test files; the existing test infrastructure (cargo test runner, sqlx test fixtures, schema_parity machinery) covers everything else.

- [ ] `tests/v12_tags_validators.rs` — integration test scaffold; covers each rejection path + dedup WARN + count cap + round-trip persistence (TAG-01..05 + D-08)
- [ ] `src/webhooks/payload.rs::tests` extension — `payload_tags_carries_real_values` test (replaces `payload_tags_empty_array_until_p22`); updates `fixture_run_detail` helper to widen `DbRunDetail` with the new `tags: Vec<String>` field (WH-09)
- [ ] `src/config/hash.rs::tests` extension — `tags_excluded_from_hash` test asserting tag-only edits produce identical config hash (D-01)
- [ ] `src/config/validate.rs::tests` extensions — unit tests for `check_tag_charset_and_reserved`, `check_tag_normalize_warn`, `check_tag_substring_collision`, `check_tag_count_per_job` (TAG-03/04/05 + D-08)
- [ ] `migrations/sqlite/20260504_000010_jobs_tags_add.up.sql` + `migrations/postgres/20260504_000010_jobs_tags_add.up.sql` — migration pair (NOT a test file but a Wave 0 scaffold; `tests/schema_parity.rs::normalize_type` absorbs the new TEXT column with zero edits per RESEARCH §E)

*Existing infrastructure covers schema parity and CI matrix execution (`linux/{amd64,arm64} × {SQLite, Postgres}`).*

---

## Manual-Only Verifications

> Per project memory `feedback_uat_user_validates.md`: maintainer validates UAT — Claude does NOT mark UAT passed from its own runs. Per project memory `feedback_uat_use_just_commands.md`: every UAT step references an existing `just` recipe, not ad-hoc `cargo` / `docker` / curl-URL invocations.

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| TOML→DB persistence spot-check (3 tags on a job) round-trips into `jobs.tags` JSON column | TAG-02 | Eyes-on-DB confirmation that the column receives the expected JSON array on both backends | `just uat-tags-persist` (D-11; new recipe; orchestrates `dev-build` + `dev-run` + `db-shell` query) |
| Each validator error UX is operator-readable | TAG-03, TAG-04, TAG-05, D-08 | Subjective readability check for each rejection message (charset, reserved, substring, >16 cap) | `just uat-tags-validators` (D-11; new recipe; walks each invalid TOML fixture and surfaces stderr) |
| Dedup WARN line names the original inputs (not just canonical form) | TAG-03 | Subjective readability of WARN message body | `just uat-tags-validators` (covers WARN scenario alongside rejects) |
| End-to-end webhook delivery contains real tag values in payload | WH-09 | Cross-system assertion: TOML → DB → dispatcher → HTTP body — only the maintainer can wire a receiver and visually confirm | `just uat-tags-webhook` (D-11; new recipe; pattern-mirrors `just uat-webhook-*` family from P18-20) |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (5 scaffolds enumerated above)
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s for unit, < 240s for full suite
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
