# Phase 20 Deferred Items

Discoveries made during Phase 20 execution that are out of scope for the current
plan(s) and have NOT been auto-fixed. These should be picked up by a follow-up
plan, the phase close-out, or a future hygiene pass.

## Pre-existing fmt drift in `src/db/queries.rs`

**Discovered during:** Plan 05 execution (Wave 3, agent-a4971088ba2b28ddf).

**Issue:** `cargo fmt --check` reports a single diff at `src/db/queries.rs:1535`:

```diff
-pub async fn insert_webhook_dlq_row(
-    pool: &DbPool,
-    row: WebhookDlqRow,
-) -> Result<(), sqlx::Error> {
+pub async fn insert_webhook_dlq_row(pool: &DbPool, row: WebhookDlqRow) -> Result<(), sqlx::Error> {
```

**Origin:** Introduced by Plan 01 commit `ba250b3` (`feat(20-01): add WebhookDlqRow + dual-dialect insert/delete helpers`). The function signature was hand-written across 4 lines but the rustfmt rule for fn-decls under 100 chars prefers a single line.

**Why deferred:** Pre-existing fmt drift in a file Plan 05 does NOT modify. Per
deviation rule scope boundary, only auto-fix issues DIRECTLY caused by the
current task's changes. The CI fmt gate would have caught this on Plan 01's PR
had it run, but the plan executor in that worktree apparently skipped the
`cargo fmt --check` step.

**Fix:** A one-line `cargo fmt -- src/db/queries.rs` collapses the function
signature to a single line. Trivially safe. Should land in the phase close-out
PR (Plan 09) or any follow-up plan that touches `src/db/queries.rs`.
