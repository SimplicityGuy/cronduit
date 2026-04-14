# Phase 7: v1.0 Cleanup & Bookkeeping - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in `07-CONTEXT.md` — this log preserves the alternatives considered.

**Date:** 2026-04-12
**Phase:** 07-v1-cleanup-bookkeeping
**Areas discussed:** OPS-04 resolution direction, REQUIREMENTS.md update mechanism, 05-VERIFICATION.md handling, HX-Refresh / Reload card fix scope

---

## Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| OPS-04 resolution direction | ROADMAP allows revert-to-`expose:` OR `overrides:` block in 06-VERIFICATION.md | ✓ |
| REQUIREMENTS.md update mechanism | How rigorous to be flipping 81 requirement checkboxes to Complete | ✓ |
| 05-VERIFICATION.md handling | Re-run /gsd-verify-work 5 vs in-place re_verification annotation | ✓ |
| HX-Refresh / Reload card fix scope | Document, regression test, or browser UAT | ✓ |

**User selected all 4 areas.**

---

## OPS-04 Resolution Direction

### Q1: Which path for OPS-04?

| Option | Description | Selected |
|--------|-------------|----------|
| Accept D-12 via override | Add overrides: block to 06-VERIFICATION.md frontmatter; keep ports: in docker-compose.yml | ✓ |
| Revert to `expose:` | Change docker-compose.yml to use expose: 8080 (breaks the "open localhost:8080 immediately" UX) | |
| Both: revert + supplement | Switch to expose: AND ship a docker-compose.quickstart.yml | |

**User's choice:** Accept D-12 via override (Recommended).
**Rationale:** Reverting to `expose:` would break the 5-minute stranger quickstart promise (OPS-05). The override path is explicitly authorized by ROADMAP success criterion 1.

### Q2: Should the docker-compose.yml comment be strengthened as part of accepting the override?

| Option | Description | Selected |
|--------|-------------|----------|
| Yes — strengthen | Expand into a clear SECURITY block warning about publishing unauthenticated UI, link THREAT_MODEL.md, show expose: snippet | ✓ |
| Keep as-is | Leave the existing 3-line comment | |

**User's choice:** Yes — strengthen (Recommended).
**Rationale:** Accepting the override means we owe operators a clearer warning about what `ports:` actually does (publishes unauthenticated v1 UI on the host).

---

## REQUIREMENTS.md Update Mechanism

### Q1: How to determine which REQ-IDs to flip to Complete?

| Option | Description | Selected |
|--------|-------------|----------|
| Strict cross-check | For each REQ-ID, grep matching 0X-VERIFICATION.md and confirm SATISFIED before flipping | ✓ |
| Bulk-trust per-phase reports | Mark all 81 complete in one shot | |

**User's choice:** Strict cross-check (Recommended).
**Rationale:** Audit-grade traceability — the master table should be defensible against the per-phase reports.

### Q2: How should the flipped table cite evidence?

| Option | Description | Selected |
|--------|-------------|----------|
| Add evidence column | Extend table with 4th column linking each Complete row to its source verification file | ✓ |
| Footnote only | Keep 3-column format, add a single "see 0X-VERIFICATION.md" footnote | |
| No citation | Just flip the boxes | |

**User's choice:** Add evidence column (Recommended).

### Q3: What about REQ-IDs that are PARTIAL in their verification report?

| Option | Description | Selected |
|--------|-------------|----------|
| Leave Pending + footnote | PARTIAL items stay Pending in master table with a footnote pointing to the open issue | ✓ |
| Promote to Complete if remaining gap is human-only | Mark Complete now if Phase 8 will handle the visual signoff | |

**User's choice:** Leave Pending + footnote (Recommended).
**Rationale:** Honest accounting — don't pretend gaps are closed before they are.

---

## 05-VERIFICATION.md Handling

### Q1: How should 05-VERIFICATION.md be brought current?

| Option | Description | Selected |
|--------|-------------|----------|
| In-place re_verification annotation | Add re_verification: frontmatter block citing PR #9 file:line for each closed gap; preserve original audit trail | ✓ |
| Re-run /gsd-verify-work 5 | Spawn verifier agent to regenerate from scratch | |

**User's choice:** In-place re_verification annotation (Recommended).
**Rationale:** Regeneration would lose the original gap-found audit trail and would still fail because gap 3 is human-only.

### Q2: Should the new status field reflect that human UAT is still pending?

| Option | Description | Selected |
|--------|-------------|----------|
| code_complete, human_needed | Two-part status mirroring 06-VERIFICATION.md pattern | ✓ |
| passed | Mark passed and let Phase 8 track the human checkpoint separately | |
| partial | Single 'partial' status until Phase 8 closes the human checkpoint | |

**User's choice:** code_complete, human_needed (Recommended).

---

## HX-Refresh / Reload Card Fix Scope

### Q1: What's the scope for the HX-Refresh fix in Phase 7?

| Option | Description | Selected |
|--------|-------------|----------|
| Document only | Cite api.rs:177 in 05-VERIFICATION.md re_verification annotation; no test | (initially picked) |
| Document + add regression test | Add HTTP-level test asserting POST /api/reload returns HX-Refresh: true | |
| Defer browser UAT to Phase 8 | Document the fix and add a Phase 8 UAT item for browser-confirming behavior | |

### Q2: If we add a regression test, where should it live?

| Option | Description | Selected |
|--------|-------------|----------|
| Skip — not adding a test | (Moot per Q1) | |
| tests/reload_api.rs (new file) | New integration test file dedicated to /api/reload HTTP behavior | (initially picked — conflicted with Q1) |
| Extend existing reload test file | Add HTTP-level case to tests/reload_sighup.rs or similar | |

### Q3 (reconciliation): Test or document-only?

| Option | Description | Selected |
|--------|-------------|----------|
| Add regression test in tests/reload_api.rs | Combine documentation with active regression protection | ✓ |
| Document only — no test | Just cite api.rs:177; rely on Phase 8 browser UAT for regression catching | |

**User's final choice:** Add regression test in tests/reload_api.rs.
**Rationale:** Combines documentation with regression protection. Phase 7 ships test + cite + 05-VERIFICATION.md re_verification entry; browser UAT is Phase 8.

---

## Claude's Discretion (per CONTEXT.md)

- Plan ordering / wave assignment (only one real dependency: D-06 depends on D-01 + D-02)
- Test harness shape for `tests/reload_api.rs` — match existing `tests/reload_*.rs` patterns
- Exact wording of strengthened docker-compose.yml comment (must satisfy D-02 must-haves)
- Exact ISO timestamp for `re_verified_at`
- Whether to write a `07-VERIFICATION.md` (accept whatever GSD standard flow does)

## Deferred Ideas

- Browser UAT for reload card auto-refresh — Phase 8
- Visual checkpoint signoff for 05-VERIFICATION.md gap 3 — Phase 8
- OPS-05 5-minute stranger quickstart UAT — Phase 8
- Auto-generation script for the REQUIREMENTS.md traceability table — v1.1+
- THREAT_MODEL.md creation if missing — separate phase/task
