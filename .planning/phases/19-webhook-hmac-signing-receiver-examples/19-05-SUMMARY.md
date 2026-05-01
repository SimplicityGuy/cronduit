---
phase: 19-webhook-hmac-signing-receiver-examples
plan: 05
subsystem: documentation
tags: [webhooks, hmac, sha256, docs, operator-hub, standard-webhooks-v1, mermaid]

# Dependency graph
requires:
  - phase: 19-webhook-hmac-signing-receiver-examples
    plan: 02
    provides: "examples/webhook-receivers/python/{receiver.py,README.md} — Python receiver linked from §9"
  - phase: 19-webhook-hmac-signing-receiver-examples
    plan: 03
    provides: "examples/webhook-receivers/go/{receiver.go,README.md} — Go receiver linked from §9"
  - phase: 19-webhook-hmac-signing-receiver-examples
    plan: 04
    provides: "examples/webhook-receivers/node/{receiver.js,README.md} — Node receiver linked from §9 (Pitfall 2 length-guard guidance in §5)"
provides:
  - "docs/WEBHOOKS.md — 10-section operator-facing hub (288 lines, 3 mermaid diagrams)"
  - "docs/CONFIG.md ## Webhooks back-link section (D-07 no duplication)"
  - "README.md third pointer line under existing pointer block (D-08 no sprawl)"
  - "examples/cronduit.toml — 3 commented-out wh-example-receiver-{python,go,node} jobs (ports 9991/9992/9993)"
affects: [19-06-ci-matrix, 20-webhook-retries]

# Tech tracking
tech-stack:
  added: []  # Doc-only plan — zero new Rust crates (D-24 satisfied; openssl-check still empty)
  patterns:
    - "Operator-facing hub doc layout: ## Overview → ## 3 headers → ## SHA-256 only → ## Secret rotation → ## Constant-time compare → ## Anti-replay → ## Idempotency → ## Retry-aware codes → ## Receiver examples → ## Loopback Rust mock"
    - "Mermaid-only diagrams (D-19 / CLAUDE.md project rule): sequenceDiagram for delivery flow, flowchart TD for verify decision tree, stateDiagram-v2 for retry FSM"
    - "DRY cross-linking: docs do NOT duplicate receiver code — they link to examples/webhook-receivers/<lang>/README.md (single source of truth)"
    - "Per-language constant-time primitive table with stdlib documentation URLs (Pattern 3 from PATTERNS.md)"
    - "Verbatim retry contract reproduction: D-12 retry-aware response codes table appears identically in both PLAN/CONTEXT and §8 of WEBHOOKS.md (Phase 20 inherits unchanged)"

key-files:
  created:
    - "docs/WEBHOOKS.md (288 lines, 10 sections + TOC, 3 mermaid diagrams)"
    - ".planning/phases/19-webhook-hmac-signing-receiver-examples/19-05-SUMMARY.md (this file)"
  modified:
    - "docs/CONFIG.md (added ## Webhooks back-link section between § Validation and § Hot reload; TOC updated)"
    - "README.md (added third pointer line in existing blockquote pointer block, line 176)"
    - "examples/cronduit.toml (appended 3 commented-out wh-example-receiver-{python,go,node} jobs after Phase 18 templates)"

key-decisions:
  - "Updated CONFIG.md table-of-contents to include the new Webhooks section (renumbered Hot reload from §8 to §9, Patterns and recipes from §9 to §10) — keeps the TOC accurate after the inline insertion. Plan PLAN.md only specified inserting the section; updating the TOC is the obvious correctness corollary (Rule 1 plan internal consistency)."
  - "Receiver-example LOC range reported as ~80-300 in §9 (not the plan's hint of ~80-150) — Plan 02/03/04 SUMMARYs report Python at 205, Go at 244, Node at 284 LOC; the broader range matches the receivers actually shipped."
  - "TOC line in WEBHOOKS.md uses `## Table of contents` heading style (matches CONFIG.md precedent at line 7) — consistency with the sibling hub doc."

patterns-established:
  - "Webhook docs hub at docs/WEBHOOKS.md sits next to docs/{QUICKSTART,CONFIG,SPEC}.md as the operator-facing topic hub for the v1.2 webhook subsystem"
  - "README.md pointer block uses `> **Verifying webhook deliveries?** ...` blockquote style mirroring the existing CONFIG.md and QUICKSTART.md pointer rows (D-08 — single line, no sprawl)"
  - "examples/cronduit.toml ships per-language receiver-target jobs commented-out — the smoke-clean default (D-05) lets `docker compose up` boot first-run operators without WEBHOOK_SECRET being set"

requirements-completed: [WH-04]

# Metrics
duration: ~6 min
completed: 2026-04-30
---

# Phase 19 Plan 05: Operator-Facing Documentation Hub Summary

**Shipped the operator-facing webhook documentation hub: a 10-section `docs/WEBHOOKS.md` (288 lines, 3 mermaid diagrams) covering the 3-header semantics, the verbatim SHA-256-only note (SC-3), per-language constant-time compare primitives, the 5-min anti-replay window, idempotency guidance, and the verbatim D-12 retry-aware response codes table — plus a back-link from `docs/CONFIG.md`, a one-line pointer in `README.md`, and 3 commented-out `wh-example-receiver-{python,go,node}` jobs in `examples/cronduit.toml` targeting ports 9991/9992/9993. Doc-only plan: zero new Rust crates (D-24 satisfied) and `just check-config` stays green.**

## Performance

- **Duration:** ~6 min
- **Started:** 2026-04-30 (worktree base 3eee73b)
- **Completed:** 2026-04-30
- **Tasks:** 3
- **Files created:** 1 (docs/WEBHOOKS.md)
- **Files modified:** 3 (docs/CONFIG.md, README.md, examples/cronduit.toml)

## Accomplishments

- **`docs/WEBHOOKS.md`** — 288 lines, 10 sections + TOC, 3 mermaid diagrams (System Architecture sequenceDiagram, Verify Decision Tree flowchart TD, Phase 20 Retry stateDiagram-v2). Zero ASCII art (D-19 / CLAUDE.md project rule).
- **SC-3 satisfied:** the verbatim `Cronduit v1.2 ships SHA-256 only.` sentence appears in §3, with rationale for no algorithm-agility and no multi-secret rotation cronduit-side, and a forward-pointer to receiver-side dual-secret rotation in §4.
- **Per-language constant-time primitive table in §5** with stdlib-doc URLs for `hmac.compare_digest` (Python), `hmac.Equal` (Go), and `crypto.timingSafeEqual` (Node) — including the mandatory Pitfall 2 length-guard guidance for the Node receiver.
- **§8 Retry-aware response codes** reproduces the D-12 table verbatim (5 rows: 400 missing/malformed, 400 drift, 401 mismatch, 200 success, 503 transient) — Phase 20's retry implementation MUST inherit this contract unchanged (CONTEXT D-12, T-19-26 mitigation).
- **§9 Receiver examples** cross-links all 3 receiver READMEs (`examples/webhook-receivers/{python,go,node}/README.md`) and tabulates the 6 `just uat-webhook-receiver-*` recipes (3 foreground UAT + 3 verify-fixture CI gates).
- **§10 Loopback Rust mock** points operators at the Phase 18 always-200 mock at `examples/webhook_mock_server.rs` for initial header inspection before switching to a verifying receiver.
- **`docs/CONFIG.md`** — new `## Webhooks` section between § Validation and § Hot reload: short TOML field cheat sheet + back-link to `docs/WEBHOOKS.md` (D-07 — receiver implementation guidance does NOT duplicate into CONFIG.md). TOC updated to list the new section.
- **`README.md`** — third pointer line in the existing blockquote pointer block (line 176): `> **Verifying webhook deliveries?** Receiver examples (Python/Go/Node) and the verify protocol live in **[docs/WEBHOOKS.md](./docs/WEBHOOKS.md)**.` (D-08 — no sprawl, single line matching the existing pointer style).
- **`examples/cronduit.toml`** — 3 new commented-out jobs appended after the existing Phase 18 webhook templates: `wh-example-receiver-python` (port 9991), `wh-example-receiver-go` (port 9992), `wh-example-receiver-node` (port 9993). Each with `command = "false"` (exercises failure firing path), `use_defaults = false`, `timeout = "5m"`, and `webhook = { url = ..., secret = "${WEBHOOK_SECRET}" }`. Block introduction comment cross-links to `docs/WEBHOOKS.md`.
- **`just check-config examples/cronduit.toml`** still exits 0 with `WEBHOOK_SECRET=test-secret-shh` (D-05 smoke preserved — the new templates ship commented-out so the uncommented `[[jobs]]` block count is unchanged at 7).
- **`just openssl-check`** still empty across native + arm64-musl + amd64-musl (D-24 — Plan 05 added zero Rust crates).

## Task Commits

1. **Task 1: Create `docs/WEBHOOKS.md` (10-section operator hub, 3 mermaid diagrams, verbatim SHA-256-only + D-12 retry table)** — `41e0f87` (docs)
2. **Task 2: Add `## Webhooks` back-link in `docs/CONFIG.md` + one-line pointer in `README.md`** — `c06f06b` (docs)
3. **Task 3: Add 3 commented-out `wh-example-receiver-*` jobs to `examples/cronduit.toml`** — `13025ba` (docs)

_Final metadata commit happens at orchestrator level._

## Files Created/Modified

### Created

- `docs/WEBHOOKS.md` — 288 lines. Heading shape:
  - `# Cronduit Webhooks` (h1, title)
  - `## Table of contents` (numbered list of §1-§10)
  - `## Overview` — Standard Webhooks v1 spec link + 3-step operator implementation guide + sequenceDiagram
  - `## Three required headers` — table of `webhook-id` / `webhook-timestamp` / `webhook-signature` + multi-token note
  - `## SHA-256 only` — **verbatim SC-3 sentence** + v1.3 roadmap pointer
  - `## Secret rotation` — receiver-side dual-secret window (3-step rotation procedure)
  - `## Constant-time compare` — per-language primitive table + Node Pitfall 2 length guard + flowchart TD verify decision tree
  - `## Anti-replay window` — `MAX_TIMESTAMP_DRIFT_SECONDS = 300` constant + Standard Webhooks reference impl link
  - `## Idempotency` — in-memory TTL Set vs DB unique constraint guidance
  - `## Retry-aware response codes` — verbatim D-12 5-row table + stateDiagram-v2 retry FSM
  - `## Receiver examples` — cross-links to all 3 receiver READMEs + 6 just recipes table
  - `## Loopback Rust mock` — Phase 18 `examples/webhook_mock_server.rs` pointer + `just uat-webhook-mock` / `just uat-webhook-fire` / `just uat-webhook-verify` triplet

### Modified

- `docs/CONFIG.md` — inserted a new `## Webhooks` section (12 lines) between `## Validation` (line 542) and `## Hot reload` (line 575), and added it to the TOC (renumbered Hot reload from §8 to §9, Patterns and recipes from §9 to §10).
- `README.md` — inserted a third pointer line (1 line) in the existing pointer blockquote at line 176, immediately after the CONFIG.md pointer.
- `examples/cronduit.toml` — appended 31 lines after line 256 (the existing `wh-example-fire-every-zero` template): block-introduction comment + 3 commented-out `[[jobs]]` blocks (`wh-example-receiver-python`, `wh-example-receiver-go`, `wh-example-receiver-node`).

## Decisions Made

- **Updated CONFIG.md TOC:** the plan only specified inserting the `## Webhooks` section before `## Hot reload`, but the existing TOC at lines 7-17 numbered all `## ` sections; correctness required adding "Webhooks" to the TOC and renumbering Hot reload + Patterns and recipes. This is plan internal consistency rather than scope creep — leaving the TOC stale would be a bug. Captured here so future PR reviewers can verify the TOC matches the section list.
- **Receiver-example LOC text broadened to "~80-300 LOC":** the plan template hinted at "~80-150 LOC" for the §9 receiver-examples narrative, but Plan 02/03/04 SUMMARYs report Python at 205, Go at 244, and Node at 284 LOC. Reporting the actual range avoids documenting a misleading lower bound. Behavior — stdlib-only and copy-pasteable verify_signature core — is unchanged.
- **No `(no `pip install`/`go mod download`/`npm install` required), ~80-300 LOC,` rewrite of §9 narrative:** kept the wording light and accurate; the heavy lifting (per-language install + run instructions) lives in each receiver's own README.md. Single source of truth (D-07).

## Deviations from Plan

None — plan executed exactly as written, with two minor consistency adjustments documented in **Decisions Made** above:

1. CONFIG.md TOC updated to include the new `## Webhooks` entry (otherwise the TOC would silently lie about the doc structure).
2. §9 receiver-example LOC range broadened to match the actual receivers shipped by Plans 02/03/04.

Both adjustments are correctness-only — no scope change, no new files, no removed content.

## Issues Encountered

None — all 3 tasks executed cleanly. The 3 mermaid diagrams from `19-RESEARCH.md` were reused verbatim (D-19 satisfied — zero ASCII box-drawing characters in `docs/WEBHOOKS.md`). The pre-existing `examples/cronduit.toml` validated with `just check-config` both before and after the Plan 05 edits (the new templates ship commented-out per D-05).

## Threat Flags

None — Plan 05 mitigations match the threat register exactly:
- T-19-24 (Information Disclosure / docs prose): the doc defers wire format to the upstream Standard Webhooks v1 spec (D-06; explicit "this document does NOT paraphrase the spec" line in §1); cronduit-specific deviations (1 secret only, SHA-256 only) are stated explicitly in §3 and §4.
- T-19-25 (Tampering / operator confusion in `examples/cronduit.toml`): new jobs ship commented-out (D-05); each has `command = "false"` (always-fails — exercises failure firing path); webhook URLs are loopback-only (`127.0.0.1:999X`); secret is `${WEBHOOK_SECRET}` env-var (no plaintext-checked-in secrets).
- T-19-26 (Repudiation / drift in retry contract): the D-12 table is reproduced verbatim in `docs/WEBHOOKS.md` §8; Phase 20's retry implementation MUST inherit unchanged.

No new security surface introduced beyond what the threat model already covers.

## Verification Receipts

- `test -f docs/WEBHOOKS.md` → exit 0
- `grep -q "Cronduit v1.2 ships SHA-256 only" docs/WEBHOOKS.md` → match (SC-3 verbatim)
- `grep -c '^```mermaid$' docs/WEBHOOKS.md` → `3` (3 mermaid code blocks)
- `grep -E '┌|└|├|┤' docs/WEBHOOKS.md` → empty (D-19 — no ASCII box-drawing)
- `grep -c '^## ' docs/WEBHOOKS.md` → `11` (10 sections + TOC)
- `wc -l docs/WEBHOOKS.md` → `288` (within 200-450 range)
- `grep -q "examples/webhook-receivers/python/README.md" docs/WEBHOOKS.md` → match
- `grep -q "examples/webhook-receivers/go/README.md" docs/WEBHOOKS.md` → match
- `grep -q "examples/webhook-receivers/node/README.md" docs/WEBHOOKS.md` → match
- `grep -q "MAX_TIMESTAMP_DRIFT_SECONDS = 300" docs/WEBHOOKS.md` → match
- `grep -q "webhook_mock_server.rs" docs/WEBHOOKS.md` → match
- `grep -q "hmac.compare_digest" docs/WEBHOOKS.md` → match
- `grep -q "hmac.Equal" docs/WEBHOOKS.md` → match
- `grep -q "crypto.timingSafeEqual" docs/WEBHOOKS.md` → match
- `grep -q "standard-webhooks/standard-webhooks" docs/WEBHOOKS.md` → match
- `grep -q "^## Webhooks" docs/CONFIG.md` → match
- `grep -q "WEBHOOKS\.md" docs/CONFIG.md` → match
- `! grep -q "verify_signature" docs/CONFIG.md` → empty (D-07 — receiver impl NOT in CONFIG.md)
- `grep -q "Verifying webhook deliveries" README.md` → match
- `grep -q "docs/WEBHOOKS\.md" README.md` → match
- `grep -cE '^# name = "wh-example-receiver-(python\|go\|node)"' examples/cronduit.toml` → `3`
- `grep -cE '127\.0\.0\.1:999[123]' examples/cronduit.toml` → `3`
- `grep -q "127.0.0.1:9991" examples/cronduit.toml` → match (Python)
- `grep -q "127.0.0.1:9992" examples/cronduit.toml` → match (Go)
- `grep -q "127.0.0.1:9993" examples/cronduit.toml` → match (Node)
- `grep -q "docs/WEBHOOKS.md" examples/cronduit.toml` → match (cross-link)
- `grep -cE '^\[\[jobs\]\]$' examples/cronduit.toml` → `7` (uncommented [[jobs]] count unchanged from Phase 18 baseline)
- `WEBHOOK_SECRET=test-secret-shh just check-config examples/cronduit.toml` → `ok: examples/cronduit.toml` (exit 0)
- `just openssl-check` → `OK: no openssl-sys in dep tree (native + arm64-musl + amd64-musl)` (D-24)

## Next Phase Readiness

- **Plan 06 (CI matrix + Maintainer UAT) is unblocked.** The CI `webhook-interop` job can now reference `docs/WEBHOOKS.md` as the operator-facing source of truth for the wire format and retry contract; the Maintainer UAT scenarios (U1-U10) can cite the docs hub when validating that operators have a single landing page for receiver implementation.
- **Plan 06 mermaid render check (manual):** Maintainer should verify via the GitHub PR preview that the 3 mermaid diagrams in `docs/WEBHOOKS.md` render as SVG (not raw fenced text). If GitHub's mermaid renderer chokes on any of the diagrams, that's a Plan 06 follow-up — the plan-05 doc-only commit hashes (`41e0f87`, `c06f06b`, `13025ba`) can be amended in a PR fix-up commit if needed.
- **Phase 20 (webhook retries):** the verbatim D-12 retry-aware response codes table in `docs/WEBHOOKS.md` §8 is the contract the retry implementation MUST honor. Drift in the table or in cronduit's response-classification code constitutes a public-API break.

## Self-Check: PASSED

**Files exist:**
- `docs/WEBHOOKS.md` — FOUND (`test -f` exit 0)
- `docs/CONFIG.md` — FOUND with `## Webhooks` section
- `README.md` — FOUND with `Verifying webhook deliveries` pointer
- `examples/cronduit.toml` — FOUND with 3 new commented-out `wh-example-receiver-*` jobs
- `.planning/phases/19-webhook-hmac-signing-receiver-examples/19-05-SUMMARY.md` — FOUND (this file)

**Commits exist:**
- `41e0f87` — FOUND in `git log --oneline --all` (Task 1: docs/WEBHOOKS.md)
- `c06f06b` — FOUND in `git log --oneline --all` (Task 2: CONFIG.md back-link + README.md pointer)
- `13025ba` — FOUND in `git log --oneline --all` (Task 3: 3 commented-out jobs in cronduit.toml)

**SC-3 verbatim:** `grep -q "Cronduit v1.2 ships SHA-256 only" docs/WEBHOOKS.md` exits 0.
**D-19 mermaid-only:** `grep -E '┌|└|├|┤' docs/WEBHOOKS.md` returns empty; 3 mermaid diagrams confirmed.
**D-24 openssl:** `just openssl-check` returns the expected OK string.
**D-05 smoke:** `WEBHOOK_SECRET=test-secret-shh just check-config examples/cronduit.toml` exits 0; uncommented `[[jobs]]` count is 7 (unchanged from Phase 18 baseline).
