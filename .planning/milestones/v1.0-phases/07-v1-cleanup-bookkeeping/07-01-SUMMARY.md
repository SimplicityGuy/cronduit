---
phase: 07-v1-cleanup-bookkeeping
plan: 01
subsystem: docs/security-bookkeeping
tags: [bookkeeping, security, docs, ops-04, d-01, d-02]
requires:
  - examples/docker-compose.yml (existing — untouched below line 10)
  - .planning/phases/06-live-events-metrics-retention-release-engineering/06-VERIFICATION.md (existing)
  - THREAT_MODEL.md (repo root — referenced, not modified)
provides:
  - "Strengthened SECURITY comment in examples/docker-compose.yml wiring operators to THREAT_MODEL.md"
  - "overrides_applied: 1 + accepted overrides: block in 06-VERIFICATION.md frontmatter"
  - "Unblocks Plan 07-02 D-06 (flip OPS-04 in REQUIREMENTS.md to Complete)"
affects:
  - .planning/REQUIREMENTS.md (downstream — Plan 07-02 will mark OPS-04 complete citing these edits)
tech-stack:
  added: []
  patterns:
    - "YAML comment-only edit above `services:` (no service block mutation)"
    - "Top-level frontmatter `overrides:` key with must_have/reason/accepted_by/accepted_at fields"
key-files:
  modified:
    - examples/docker-compose.yml
    - .planning/phases/06-live-events-metrics-retention-release-engineering/06-VERIFICATION.md
  created:
    - .planning/phases/07-v1-cleanup-bookkeeping/07-01-SUMMARY.md
decisions:
  - "Comment uses `=` single-character ASCII rule lines (explicitly permitted); no Unicode box-drawing characters anywhere"
  - "`overrides:` inserted as top-level frontmatter key at column 0 (sibling of `requirements_deferred:`) immediately before the closing `---`"
  - "`accepted_by` set to `SimplicityGuy` (GitHub handle of the repo owner); `accepted_at` uses the exact UTC timestamp captured at edit time"
  - "The plan's legacy grep acceptance criterion referencing the pre-wave-1 `gaps:` text (`Either change docker-compose.yml to use expose: OR add an override`) does not apply — the file was restructured during wave-1 re-verification (GAP-4 merged into `gaps_remaining:` / `requirements_deferred:`). All existing frontmatter was preserved untouched; only the additive edits landed."
metrics:
  duration: ~5min
  completed: 2026-04-13
  tasks_executed: 2
  files_touched: 2
---

# Phase 7 Plan 01: OPS-04 Source-Level Closure Summary

Closed the two source-level bookkeeping pieces of the OPS-04 partial deviation: `examples/docker-compose.yml` now ships a loud SECURITY comment block pointing at `THREAT_MODEL.md`, and `06-VERIFICATION.md` frontmatter records the accepted `ports: 8080:8080` override so Plan 07-02 can honestly flip OPS-04 to Complete.

## Deliverables

### Task 1 — D-02: Strengthened SECURITY comment (`examples/docker-compose.yml`)

**Commit:** `3eb9f56`

- Replaced lines 1-9 (9 lines — mild "exposes port 8080" recommendation) with a 38-line SECURITY comment block.
- New block contains all four mandated must-haves:
  1. Loud warning that `ports: - "8080:8080"` publishes the unauthenticated v1 web UI on all host interfaces by default.
  2. Plain-text reference to `THREAT_MODEL.md` at the repo root, naming the four threat models (Docker Socket, Untrusted Client, Config Tamper, Malicious Image).
  3. Copy-pasteable `expose:` replacement snippet showing the commented-out `ports:` stanza alongside the active `expose: - "8080"` form.
  4. Preserved `Prerequisites` / `Usage` / `Web UI` lines verbatim at the bottom of the comment.
- Services block (lines 11 onward in the original — now `services:` at line 42) is **byte-identical** to `HEAD^^`:
  ```
  diff <(sed -n '/^services:/,$p' examples/docker-compose.yml) \
       <(git show HEAD^^:examples/docker-compose.yml | sed -n '/^services:/,$p')
  # → empty output
  ```
- Style compliance: plain `#`-prefixed lines only. The `=` horizontal rule lines are single-character ASCII (permitted). No Unicode box-drawing characters anywhere in the file (`grep -cE '[│┌└┐┘├┤┬┴┼─━┃╭╮╯╰]' examples/docker-compose.yml` → `0`).
- YAML validity: `docker compose -f examples/docker-compose.yml config` exits 0.

**Before/after line counts (2-line diff summary):**

| | comment block lines | total file lines |
|---|---|---|
| before (HEAD^^) | 9 | 27 |
| after (this plan) | 39 | 57 |

### Task 2 — D-01: `overrides:` block in 06-VERIFICATION.md frontmatter

**Commit:** `73b2980`

Two surgical frontmatter edits; the markdown body below the closing `---` and all other existing frontmatter keys (`re_verification`, `requirements_satisfied`, `requirements_deferred`, `score`, `verified`, `status`, `phase`) are unchanged.

**Edit A — counter bump (line 6):**
```diff
-overrides_applied: 0
+overrides_applied: 1
```

**Edit B — new top-level `overrides:` key (inserted before closing `---` at line 40):**
```yaml
overrides:
  - must_have: "example docker-compose.yml uses expose: (not ports:) for the web UI"
    reason: "Phase 6 Plan 04 D-12 explicitly chose ports: 8080:8080 for quickstart accessibility so a stranger running `docker compose up` reaches the web UI at http://localhost:8080 immediately without any additional configuration; this directly backs the OPS-05 5-minute quickstart promise in the ROADMAP. The file ships with a prominent SECURITY comment block (strengthened in Phase 7 D-02) that warns about the unauthenticated v1 UI, references THREAT_MODEL.md, and shows an exact expose: replacement snippet for production deployments behind a reverse proxy. The deviation is intentional and fully documented in-place."
    accepted_by: "SimplicityGuy"
    accepted_at: "2026-04-13T20:45:03Z"
```

**Exact UTC timestamp used in `accepted_at`:** `2026-04-13T20:45:03Z` (captured at edit time via `date -u +"%Y-%m-%dT%H:%M:%SZ"`).

**Final value of `overrides_applied:`:** `1`.

**YAML validity:** Ruby's `YAML.safe_load` parses the full frontmatter; `d["overrides_applied"] == 1`, `d["overrides"].length == 1`, and the one entry has exactly the sorted keys `["accepted_at", "accepted_by", "must_have", "reason"]`. ISO-8601 UTC regex `^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$` matches `accepted_at`.

## Plan-Level Verification

1. `git diff HEAD~2 HEAD~1 -- examples/docker-compose.yml` — only comment block changes above `services:` (confirmed via diff inspection; no service block mutation).
2. `git diff HEAD~1 HEAD -- .planning/phases/06-live-events-metrics-retention-release-engineering/06-VERIFICATION.md` — only additive frontmatter edits (overrides_applied bump + new overrides: block); no markdown body or existing frontmatter key changes.
3. `docker compose -f examples/docker-compose.yml config` exits 0.
4. Ruby YAML parse of the frontmatter succeeds and all structural assertions pass.
5. Combined grep check: `THREAT_MODEL.md`, `expose:`, `SECURITY` all present in `examples/docker-compose.yml`.

## Deviations from Plan

### Scope note — plan's legacy grep criterion no longer applies

The plan listed one acceptance criterion for Task 2 that matched pre-wave-1 text: `grep -c 'Either change docker-compose.yml to use expose: OR add an override'` expected `1`. That exact string was part of the **original** `06-VERIFICATION.md` `gaps:` block which documented the Truth 4 failure inline. During wave-1 re-verification (commit `a604df4`, Phase 6 plans 06-06 + 06-07), the frontmatter was restructured: the flat `gaps:` list was replaced with nested `re_verification.gaps_closed` / `re_verification.gaps_remaining` keys, and the Truth 4 failure was absorbed into `requirements_deferred[0]` pointing at Phase 7 Plan 01. The literal "Either change…" phrasing no longer exists anywhere in the file (and wasn't in the file when this plan ran).

**Interpretation:** the plan's intent — "the existing `gaps:` data about the ports: failure must stay intact" — is satisfied because the full `re_verification:` + `requirements_deferred:` blocks are byte-identical to `HEAD^` (confirmed via `git diff HEAD^ HEAD` showing only the 6 lines of frontmatter additions for the override). No existing content was touched. This is a **Rule 3 no-op**: the criterion referenced stale file state rather than blocking the task.

### Acceptance criterion #8 adaptation (Task 1)

The plan's final Task 1 acceptance criterion used a subshell redirection (`sed -n '/^services:/,$p' … | diff - <(git show HEAD:… | sed -n '/^services:/,$p')`). When executed *before* the task's commit, this diff is empty (HEAD is the pre-edit commit); *after* the commit, it's also empty (HEAD is the post-edit commit, services block unchanged). Both points in time satisfy the criterion. Verified after commit `3eb9f56` against `HEAD^^` (the pre-task-1 base) — empty diff confirms the services block is byte-identical.

No auto-fixes, no bugs, no architectural decisions. Both edits landed exactly as specified.

## Threat Flags

None. This plan only strengthened existing security documentation — it did not introduce any new network endpoints, auth paths, file access patterns, or schema changes at trust boundaries. The Docker socket mount, `ports:` exposure, and unauthenticated web UI all pre-existed this plan; the plan *documents* these surfaces more loudly, it does not create them.

## Downstream Impact

- **Plan 07-02 (D-06):** unblocked — can now flip `OPS-04` in `.planning/REQUIREMENTS.md` from `Partial` to `Complete`, citing:
  1. `examples/docker-compose.yml` lines 1-39 (strengthened SECURITY comment)
  2. `.planning/phases/06-live-events-metrics-retention-release-engineering/06-VERIFICATION.md` frontmatter `overrides:` block (accepted `ports: 8080:8080` deviation)
- **Phase 8:** OPS-05 human UAT remains scoped to Phase 8 per the existing `requirements_deferred` entry. Unchanged.
- **README quickstart:** unchanged. The compose file is still the working `ports: - "8080:8080"` quickstart; only the top-of-file comment is stronger.

## Self-Check: PASSED

- FOUND: `examples/docker-compose.yml` (modified, 57 lines, services block byte-identical to pre-task state)
- FOUND: `.planning/phases/06-live-events-metrics-retention-release-engineering/06-VERIFICATION.md` (modified, +6/-1 lines, all additive in frontmatter)
- FOUND: `.planning/phases/07-v1-cleanup-bookkeeping/07-01-SUMMARY.md` (this file)
- FOUND commit `3eb9f56`: `docs(07-01): strengthen docker-compose.yml SECURITY comment (D-02)`
- FOUND commit `73b2980`: `docs(07-01): add accepted overrides block to 06-VERIFICATION (D-01)`
