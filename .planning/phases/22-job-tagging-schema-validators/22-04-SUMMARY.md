---
phase: 22-job-tagging-schema-validators
plan: 04
subsystem: webhooks

tags: [webhooks, payload, backfill, wh-09, p18-cutover, tags, serde-json]

# Dependency graph
requires:
  - phase: 22-job-tagging-schema-validators
    provides: "DbRunDetail.tags: Vec<String> populated by get_run_by_id from jobs.tags JSON column (Plan 22-03)"
  - phase: 22-job-tagging-schema-validators
    provides: "jobs.tags JSON column written by upsert_job in sorted-canonical form (Plan 22-03)"
  - phase: 22-job-tagging-schema-validators
    provides: "Tag charset/length/count validators (Plan 22-02)"
  - phase: 22-job-tagging-schema-validators
    provides: "JobConfig.tags: Vec<String> with #[serde(default)] (Plan 22-01)"
  - phase: 18-webhooks-mvp
    provides: "WebhookPayload struct with tags placeholder (vec![]) and structural breadcrumb test"
provides:
  - "WH-09 closure: real tag values flow from DbRunDetail.tags into outbound webhook payload JSON"
  - "Sorted-canonical multi-tag round-trip test (payload_tags_carries_real_values) regression-locks D-05 + D-06.5"
  - "Backwards-compatible test fixture widening (fixture_run_detail_with_tags) — seven existing callers unchanged"
  - "Removal of until_p22 structural breadcrumb (test name + doc-comment language)"
affects: [phase-22-plan-05-uat, future-phase-webhook-receivers, future-phase-payload-v2]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Build-site: payload literal field reads `run.tags.clone()` — match canonical sibling pattern (image_digest.clone, config_hash.clone)"
    - "Test fixture widening Option B: keep narrow helper for existing callers, add wider sibling helper for new tests (avoids touching seven call sites)"
    - "Test assertion: `s.contains(r#\"\"tags\":[\"backup\",\"weekly\"]\"#)` — string-substring round-trip with raw-string for escape clarity"

key-files:
  created: []
  modified:
    - "src/webhooks/payload.rs (build-site cutover, doc-comment cleanup, fixture helper, test rename+rewrite)"

key-decisions:
  - "Build site reads `tags: run.tags.clone()` — single load-bearing functional change (D-05)"
  - "Old test `payload_tags_empty_array_until_p22` deleted, not edited — name was load-bearing structural breadcrumb (D-06.5)"
  - "Fixture widening: Option B (sibling helper `fixture_run_detail_with_tags`) chosen over Option A (widen existing 2-arg signature) — minimizes blast radius across the seven existing `fixture_run_detail(None, None)` callers"
  - "Test assertion uses sorted-canonical order `[\"backup\",\"weekly\"]` (NOT `[\"weekly\",\"backup\"]`) — locks the upstream Plan 22-03 normalize+sort+dedup contract"

patterns-established:
  - "Cutover discipline: when a placeholder ships with a structural-breadcrumb test, the test must be DELETED (not just edited) at cutover time so the breadcrumb cannot regress"
  - "Backwards-compatible fixture widening: prefer adding a sibling helper over rewriting an existing helper's signature"

requirements-completed: [WH-09]

# Metrics
duration: ~10min
completed: 2026-05-04
---

# Phase 22 Plan 04: WH-09 Webhook Payload Backfill Cutover Summary

**Real `tags` values now flow from `DbRunDetail.tags` into outbound webhook payload JSON — closing the WH-09 v1.2 commitment end-to-end (TOML → validators → DB column → DbRunDetail → WebhookPayload → wire JSON).**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-05-04T19:45:00Z
- **Completed:** 2026-05-04T19:55:34Z
- **Tasks:** 1 (decomposed into 2 atomic commits per user commit protocol)
- **Files modified:** 1 (`src/webhooks/payload.rs`)

## Accomplishments

- **Build-site cutover:** Replaced `tags: vec![]` at `src/webhooks/payload.rs:88` with `tags: run.tags.clone(), // Phase 22 WH-09 / D-05` — one-line load-bearing functional change.
- **Doc-comment cleanup:** Removed the placeholder language `Empty []` until Phase 22 lights up real values` at lines 51–53; replaced with real-tags behavior description and v1.2 schema-stability language.
- **Old test deletion:** `payload_tags_empty_array_until_p22` is GONE from the file (D-06.5 — the structural breadcrumb cannot regress to an empty array).
- **New test:** `payload_tags_carries_real_values` asserts a multi-tag fixture (`vec!["backup", "weekly"]`) round-trips into the wire JSON as `"tags":["backup","weekly"]` (sorted-canonical order).
- **Backwards-compatible fixture helper:** Added `fixture_run_detail_with_tags(image_digest, config_hash, tags)` as a 3-arg sibling to the existing `fixture_run_detail(image_digest, config_hash)`. The seven existing `fixture_run_detail(None, None)` callers are untouched and continue to compile.

## Cutover Diagram

```mermaid
flowchart LR
    subgraph Phase18["Phase 18 (legacy state)"]
        P18Build["WebhookPayload::build<br/>tags: vec![]"]
        P18Test["payload_tags_empty_array_until_p22<br/>asserts \"tags\":[]"]
        P18Doc["doc: Empty [] until Phase 22"]
    end

    subgraph Phase22["Phase 22 Plan 04 (cutover)"]
        P22Build["WebhookPayload::build<br/>tags: run.tags.clone()"]
        P22Test["payload_tags_carries_real_values<br/>asserts \"tags\":[\"backup\",\"weekly\"]"]
        P22Doc["doc: Real values from jobs.tags<br/>via DbRunDetail.tags"]
    end

    P18Build -->|"vec![] -> run.tags.clone()"| P22Build
    P18Test -->|"DELETED (D-06.5);<br/>replaced with sorted-canonical round-trip"| P22Test
    P18Doc -->|"placeholder language removed"| P22Doc

    subgraph DataFlow["End-to-end data flow (now closed)"]
        TOML["cronduit.toml<br/>tags = [\"weekly\", \"backup\"]"]
        Valid["validators (Plan 02)<br/>charset + length + count"]
        Upsert["upsert_job (Plan 03)<br/>normalize+sort+dedup<br/>writes JSON to jobs.tags"]
        DbRow["DbRunDetail.tags<br/>(Plan 03 reads jobs.tags)"]
        Wire["Wire JSON<br/>\"tags\":[\"backup\",\"weekly\"]"]
        TOML --> Valid --> Upsert --> DbRow --> P22Build --> Wire
    end
```

## Task Commits

| # | Subject | Type | SHA |
|---|---------|------|-----|
| 1 | Replace tags placeholder with real values (WH-09) | feat | `67e73fe` |
| 2 | Replace placeholder breadcrumb with real-tag round-trip | test | `194347b` |

**Plan metadata:** (this SUMMARY commit follows below)

### Commit 1 — `67e73fe` (feat)

- Replaced `tags: vec![]` at the build site with `tags: run.tags.clone()`.
- Updated the field doc-comment at lines 51–53.
- Deleted the `payload_tags_empty_array_until_p22` test (so this commit's CI is green — the deleted test's `s.contains("\"tags\":[]")` would otherwise fail after the cutover).

### Commit 2 — `194347b` (test)

- Added `fixture_run_detail_with_tags` 3-arg sibling helper (Option B widening).
- Added `payload_tags_carries_real_values` test with sorted-canonical multi-tag round-trip assertion.

## Code Diffs (key portions)

### Build-site (the load-bearing line)

**Before:**
```rust
image_digest: run.image_digest.clone(),
config_hash: run.config_hash.clone(),
tags: vec![],
cronduit_version,
```

**After:**
```rust
image_digest: run.image_digest.clone(),
config_hash: run.config_hash.clone(),
tags: run.tags.clone(), // Phase 22 WH-09 / D-05
cronduit_version,
```

### Doc-comment

**Before (lines 51–53):**
```rust
/// Empty `[]` until Phase 22 lights up real values. Schema-stable
/// — Phase 22 cutover does NOT break receivers.
pub tags: Vec<String>,
```

**After:**
```rust
/// Real values from `jobs.tags` column via `DbRunDetail.tags` (Phase 22
/// WH-09 / D-05). Sorted-canonical order. Always emitted (never omitted)
/// for schema stability; receivers can index without `KeyError`. Per
/// WH-09 the field is part of the locked v1.2.0 payload schema —
/// future additions are additive only.
pub tags: Vec<String>,
```

### Test (verbatim — the regression lock)

```rust
#[test]
fn payload_tags_carries_real_values() {
    // Phase 22 WH-09 / D-05 / D-06.5: the placeholder is gone.
    // Receivers see real tag values from the jobs.tags column,
    // round-tripped through DbRunDetail.tags into the wire JSON.
    // Sorted-canonical order is emitted (operator-written
    // ["weekly", "backup"] becomes ["backup", "weekly"] after the
    // upsert path's normalize+sort+dedup; this test asserts the
    // ORDER in the wire payload).
    let event = fixture_event();
    let fctx = fixture_fctx();
    let run = fixture_run_detail_with_tags(
        None,
        None,
        vec!["backup".to_string(), "weekly".to_string()],
    );
    let p = WebhookPayload::build(&event, &fctx, &run, 1, "1.2.0");
    let s = serde_json::to_string(&p).unwrap();
    assert!(
        s.contains(r#""tags":["backup","weekly"]"#),
        "tags must round-trip into payload preserving sorted-canonical order: {s}"
    );
}
```

### Fixture helper (Option B — backwards-compatible sibling)

```rust
/// Phase 22 WH-09 / D-05: 3-arg variant for tests that need to seed
/// non-empty tag values into `DbRunDetail.tags`. Backwards-compatible
/// with the seven existing `fixture_run_detail(None, None)` callers.
fn fixture_run_detail_with_tags(
    image_digest: Option<String>,
    config_hash: Option<String>,
    tags: Vec<String>,
) -> DbRunDetail {
    let mut r = fixture_run_detail(image_digest, config_hash);
    r.tags = tags;
    r
}
```

## Files Created/Modified

- `src/webhooks/payload.rs` — build-site cutover, doc-comment cleanup, fixture helper added, old test deleted, new test added.

## Decisions Made

- **Option B chosen for fixture widening** — keeping the existing `fixture_run_detail` narrow signature and adding a 3-arg sibling minimizes blast radius across the seven call sites that the plan explicitly enumerates (RESEARCH §6 lines 528–530).
- **Old test deleted (not edited)** — D-06.5 makes the test name itself load-bearing. An edit-in-place would leave `payload_tags_empty_array_until_p22` in the tree, which lies once Phase 22 ships real values.
- **Sorted-canonical assertion order** — the test asserts `["backup","weekly"]` not `["weekly","backup"]` because Plan 22-03's upsert path produces sorted-canonical form. This locks the upstream contract: if Plan 22-03 ever stops sorting, this test fails immediately.

## Deviations from Plan

None — plan executed exactly as written. Note: the plan said the existing fixture `tags: Vec::new()` line might be added by Plan 22-03 ("If Plan 03 widening already touched this fixture for `tags: Vec::new()`, this edit is a no-op for the existing helper"). Confirmed at start of execution: Plan 22-03 had already added `tags: Vec::new()` (line 142 with comment "Phase 22: defaulted; Plan 04 widens for real values"). I updated the trailing comment to reference the new sibling helper for clarity, but no functional change to the existing fixture.

## Issues Encountered

None.

## WH-09 End-to-End Closure

WH-09 v1.2 commitment now closes end-to-end. The data flow:

1. **TOML** — operator writes `tags = ["weekly", "backup"]` in `cronduit.toml` (Plan 22-01: `JobConfig.tags`).
2. **Validators** — charset/length/count gates at config-load (Plan 22-02).
3. **DB upsert** — `upsert_job` normalizes + sorts + dedups, writes `["backup","weekly"]` JSON to `jobs.tags` column (Plan 22-03).
4. **Read path** — `get_run_by_id` reads `jobs.tags` and populates `DbRunDetail.tags: Vec<String>` (Plan 22-03).
5. **Webhook payload** — `WebhookPayload::build` reads `run.tags.clone()` into `WebhookPayload.tags` (this plan).
6. **Wire JSON** — `serde::Serialize` emits `"tags":["backup","weekly"]` to the receiver (this plan, asserted by `payload_tags_carries_real_values`).

Plan 22-05 is expected to add the operator-readable end-to-end UAT recipe (`uat-tags-webhook`) that exercises this whole chain in a live container.

## Verification Gates

| Gate | Result |
|------|--------|
| `cargo build` | PASS |
| `cargo test --lib webhooks -- --quiet` (35 passed, 1 ignored) | PASS |
| `cargo test --lib -- --quiet` (323 passed, 1 ignored) | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| `cargo tree -i openssl-sys` empty (D-17) | PASS |
| `grep -F 'tags: vec![]' src/webhooks/payload.rs` (zero matches) | PASS |
| `grep -F 'until Phase 22' src/webhooks/payload.rs` (zero matches) | PASS |
| `grep -F 'until_p22' src/webhooks/payload.rs` (zero matches) | PASS |
| `grep -F 'payload_tags_empty_array_until_p22' src/webhooks/payload.rs` (zero matches) | PASS |
| `grep -F 'payload_tags_carries_real_values' src/webhooks/payload.rs` (≥1 match) | PASS |

## User Setup Required

None — pure code change inside the Rust binary; no external services or configuration touched.

## Next Phase Readiness

- **WH-09 closed end-to-end.** Receivers configured against tagged jobs will see real tag values in delivered payloads as soon as this branch lands.
- **Plan 22-05 hand-off:** the end-to-end UAT recipe (`uat-tags-webhook`) referenced in the plan's `<output>` section will exercise the whole TOML → wire JSON chain through `just` recipes and live container.
- **No blockers** for downstream phases. Payload schema is locked at v1.2.0; future tag-related additions remain additive.

## Self-Check: PASSED

- `src/webhooks/payload.rs` — modified (verified via `git log -1 --stat` and `git status`).
- Commit `67e73fe` exists in `git log --oneline`.
- Commit `194347b` exists in `git log --oneline`.
- All grep gates return expected results (PASS rows in Verification Gates table).
- All cargo gates green.

---

*Phase: 22-job-tagging-schema-validators*
*Plan: 04 (WH-09 webhook payload backfill cutover)*
*Completed: 2026-05-04*
