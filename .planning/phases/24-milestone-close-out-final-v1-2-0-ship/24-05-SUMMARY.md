---
phase: 24-milestone-close-out-final-v1-2-0-ship
plan: 05
subsystem: infra
tags: [cargo-deny, ci, supply-chain, licenses, found-16]

# Dependency graph
requires:
  - phase: 15-foundation-preamble
    provides: cargo-deny CI step landed warn-only (continue-on-error: true) with embedded forecast comment for Phase 24 promotion (FOUND-16, D-09)
provides:
  - cargo-deny is the ERROR gate on all subsequent CI runs (rc.4 cut, close-out PR commits, final v1.2.0 tag)
  - License allowlist widened to include the five OSI/FSF licenses already present in the v1.0/v1.1 dep graph but tolerated under continue-on-error
  - FOUND-16 closed — last unticked v1.2 requirement that needed a workflow change rather than source
affects: [24-06-rc4-preflight, 24-07-human-uat, 24-08-final-ship-preflight, v1.3-milestone-close]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "License allowlist entries carry per-license rationale + carrier-dep list + `expires: YYYY-MM-DD — re-evaluate before <milestone>` soft-signal comment"
    - "Branch B remediation prefers `deny.toml` allowlist over `Cargo.lock` rev when carrier deps are deeply transitive (sqlx / metrics / rustls / notify) and convergence would risk regressions"

key-files:
  created:
    - .planning/phases/24-milestone-close-out-final-v1-2-0-ship/24-05-SUMMARY.md
  modified:
    - .github/workflows/ci.yml
    - deny.toml

key-decisions:
  - "Branch B remediation strategy: license-allowlist (deny.toml) over Cargo.lock rev — all five licenses are OSI-approved AND FSF Free/Libre per cargo-deny's own classification; transitive deps would be invasive/impossible to converge"
  - "Single atomic commit (workflow flip + license allowlist) per CONTEXT D-11 atomic-commit-per-plan — no separable dep-rev commit needed since Branch B yielded license-only findings"
  - "Comment block at ci.yml L47-53 rewritten in past tense per PATTERNS § Plan 24-05 preferred shape (full-line removal of continue-on-error vs flip-to-false)"

patterns-established:
  - "Pattern A: license-allowlist expiry-comment shape (rationale + carrier list + expires: date) — re-usable for future v1.3+ accumulated-advisory remediations"
  - "Pattern B: compound SPDX expressions (`(MIT OR Apache-2.0) AND Unicode-3.0` etc.) cannot be allowlisted as atomic entries in cargo-deny v0.19.x — must allowlist each atomic SPDX component; inline NOTE in deny.toml documents the workaround"

requirements-completed: []

# Metrics
duration: ~10min
completed: 2026-05-17
---

# Phase 24 Plan 05: cargo-deny WARN→ERROR Promotion (FOUND-16) Summary

**cargo-deny promoted to a blocking CI gate with a one-time license-allowlist remediation covering five OSI+FSF licenses (Unicode-3.0, Zlib, CDLA-Permissive-2.0, CC0-1.0, and the compound-via-component allow) that were already present in the v1.0/v1.1 dep graph but tolerated under `continue-on-error: true` since rc.1.**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-05-17T01:17:50Z (approximate — phase 24 execution start per STATE.md)
- **Completed:** 2026-05-17T01:26:05Z
- **Tasks:** 2 (Task 1 probe + Task 2 promotion + remediation)
- **Files modified:** 2 (`.github/workflows/ci.yml`, `deny.toml`)

## Accomplishments

- `.github/workflows/ci.yml` cargo-deny step has `continue-on-error: true` removed entirely (preferred shape per PATTERNS § Plan 24-05); preceding comment block rewritten in past tense ("PROMOTED TO BLOCKING in Phase 24 per the original FOUND-16 spec").
- `deny.toml` `[licenses].allow` widened by five SPDX identifiers (`Unicode-3.0`, `Zlib`, `CDLA-Permissive-2.0`, `CC0-1.0`) + an inline NOTE explaining why the compound expression `(MIT OR Apache-2.0) AND Unicode-3.0` cannot be allowlisted as an atomic entry but is satisfied automatically once its atomic components are present.
- Each new allowlist entry carries (a) per-license rationale, (b) carrier-dep list (which transitive deps surface that license), (c) `expires: 2026-12-31 — re-evaluate before v1.3 close` soft-signal comment.
- `deny.toml` header comment rewritten to past tense documenting the Phase 24 promotion.
- Local `just deny` returns `EXIT=0` (advisories ok, bans ok, licenses ok) post-remediation.
- Rustls invariant preserved (`cargo tree -i openssl-sys` returns "package ID specification did not match any packages").

## Task Commits

Each task was committed atomically:

1. **Tasks 1+2 (combined): probe + promotion + Branch B license-allowlist remediation** — `3bd1ed9` (ci) — `ci(24-05): promote cargo-deny to blocking (FOUND-16)`

**Plan metadata:** (this SUMMARY.md commit will be recorded after this Write completes)

_Note: Task 1 (probe) is read-only and produces no committable artifacts beyond the transient `.tmp/deny-check.log` (which the plan explicitly marks "transient — not committed"). Task 2 carries the entire diff. CONTEXT D-11 atomic-commit-per-plan + the plan's `<objective>` ("One commit (Branch A) OR two commits if Branch B yields a separable dep-rev commit") authorize the single-commit shape since Branch B yielded only a license-allowlist remediation (no separable dep-rev)._

## Files Created/Modified

- `.github/workflows/ci.yml` — cargo-deny step `continue-on-error: true` line removed; preceding comment block at L47-53 rewritten in past tense (single coherent block per PATTERNS § Plan 24-05 preferred shape).
- `deny.toml` — header comment updated to past tense; `[licenses].allow` extended by 4 new SPDX entries (`Unicode-3.0`, `Zlib`, `CDLA-Permissive-2.0`, `CC0-1.0`) + inline NOTE on compound-expression handling. Each new entry carries rationale + carrier-dep list + `expires: 2026-12-31` re-evaluate comment.

## Decisions Made

**Decision 1: Branch B classification — license findings only (no advisories, no bans failures).** The Task 1 probe surfaced `EXIT=4` with 24 individual `error[rejected]` license findings across 5 distinct SPDX identifiers. `advisories ok, bans ok, licenses FAILED`. No RUSTSEC advisories accumulated since rc.1; the duplicate-version warnings persist as designed (D-10 `bans.multiple-versions = "warn"`).

**Decision 2: license-allowlist over Cargo.lock rev for ALL five licenses.** Per CONTEXT § Claude's Discretion + plan task 2 § Branch B guidance:

| License | Carrier deps | Why allowlist (not dep-rev) |
|---|---|---|
| `Unicode-3.0` | icu_* family + transitive (idna → url → SSRF guard + sqlx URL parsing) | Successor SPDX to the already-allowed `Unicode-DFS-2016`; widely accepted in Rust ecosystem; pinning idna/url to pre-Unicode-3.0 versions is unrealistic. |
| `Zlib` | foldhash 0.1.5 + 0.2.0 (hashbrown → sqlx-core + hashbrown → metrics-util → metrics-exporter-prometheus) | One of the most permissive OSS licenses; pinning sqlx/metrics to older hashbrown is high regression risk. |
| `CDLA-Permissive-2.0` | webpki-roots 0.26.x + 1.0.x (rustls trust-anchor bundle) | Mozilla root CA list sourced from CCADB; CCADB redistribution requires CDLA-2.0. Required for rustls posture per PROJECT.md D-19. |
| `CC0-1.0` | notify 8.2.0 (file-watch config reload — Phase 5 hot reload) | Public-domain dedication; pinning notify is unnecessary risk for a stable file-watcher. |
| `(MIT OR Apache-2.0) AND Unicode-3.0` | unicode-ident (syn → every proc-macro) | Compound; satisfied automatically once atomic components allowlisted. No action required beyond the inline NOTE. |

All five are OSI-approved AND FSF Free/Libre per cargo-deny's own output ("OSI approved", "FSF Free/Libre" annotations in the error messages).

**Decision 3: Past-tense single-coherent comment block (preferred shape) over keep-line-flip-value alternative.** PATTERNS § Plan 24-05 explicitly states "the original comment described this as 'single-line removal of continue-on-error' — honor that spec." Comment block at L47-53 now reads coherently in past tense.

**Decision 4: NO Cargo.lock rev.** Confirmed by `git status --short` — only `ci.yml` and `deny.toml` modified. The dep graph is bit-identical to rc.3.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 — Blocking] cargo-deny rejects compound SPDX expressions in `[licenses].allow`**
- **Found during:** Task 2 (post-allowlist re-probe)
- **Issue:** Added `"(MIT OR Apache-2.0) AND Unicode-3.0"` (verbatim from `unicode-ident` 1.0.24 Cargo.toml) as an allowlist entry. cargo-deny v0.19.x rejected the config: `error[custom]: expected a <license> here`. cargo-deny's `[licenses].allow` accepts only atomic SPDX identifiers, not compound expressions.
- **Fix:** Removed the compound entry; replaced with an inline NOTE documenting why the compound is satisfied automatically (every atomic component — MIT, Apache-2.0, Unicode-3.0 — is individually allowlisted, so cargo-deny resolves the compound at evaluation time without needing an explicit entry).
- **Files modified:** `deny.toml` (single edit on the same Task 2 commit)
- **Verification:** `just deny` returned `EXIT=0` after the fix (cf. `EXIT=1` config-deserialization error before).
- **Committed in:** `3bd1ed9` (part of Task 2 commit — discovered + fixed before initial commit)

---

**Total deviations:** 1 auto-fixed (1 blocking — config-syntax issue)
**Impact on plan:** No scope creep. The plan's `<action>` block did not anticipate cargo-deny's atomic-only requirement; the fix is purely mechanical and stays within the plan's "deny.toml allowlist with timestamped expiry comment" remediation shape.

## Issues Encountered

**Issue 1: Initial bash command sequence captured an empty `EXIT=` value in `.tmp/deny-check.log`.**

The first probe used `just deny 2>&1 | tee .tmp/deny-check.log; DENY_EXIT=$?` — but `$?` captures the exit code of `tee`, not `just deny`. Re-ran with explicit `bash -c '... ; echo "BASH_EXIT_CODE=$?"'` capture, then stripped the marker line and appended the clean `EXIT=4`. Final log contains correct `EXIT=4` per checker W5 anti-no-op gate.

## Cargo-Deny Probe & Verification Output

### Initial probe (Task 1 — Branch classification)

```
EXIT=4
TIMESTAMP=2026-05-17T01:21:23Z
```

Branch B (eventful). License findings (24 total, 5 distinct SPDX identifiers):

| SPDX | Count | Distinct crates |
|---|---|---|
| `Unicode-3.0` | 18 | icu_collections, icu_locale_core, icu_normalizer, icu_normalizer_data, icu_properties, icu_properties_data, icu_provider, litemap, potential_utf, tinystr, writeable, yoke, yoke-derive, zerofrom, zerofrom-derive, zerotrie, zerovec, zerovec-derive |
| `Zlib` | 2 | foldhash 0.1.5, foldhash 0.2.0 |
| `CDLA-Permissive-2.0` | 2 | webpki-roots 0.26.11, webpki-roots 1.0.7 |
| `CC0-1.0` | 1 | notify 8.2.0 |
| `(MIT OR Apache-2.0) AND Unicode-3.0` | 1 | unicode-ident 1.0.24 |

`advisories ok, bans ok, licenses FAILED` — no advisories accumulated since rc.1; no bans-severity findings; duplicate-versions remain warn-only by design.

### Post-remediation probe (Task 2 verification)

```
$ just deny
... (duplicate-version warnings — by design per D-10) ...
advisories ok, bans ok, licenses ok
$ echo $?
0
```

### Rustls invariant (Task 2 acceptance criterion D)

```
$ cargo tree -i openssl-sys
error: package ID specification `openssl-sys` did not match any packages
```

Exit 0 (no carriers in dep graph). PROJECT.md D-19 invariant intact.

### Task 2 combined verification gate

```
$ (! grep -E "continue-on-error: true" .github/workflows/ci.yml | grep -i deny) \
    && grep -E "just deny" .github/workflows/ci.yml \
    && just deny >/dev/null 2>&1 \
    && (cargo tree -i openssl-sys 2>&1 | grep -E "did not match|not found" -q) \
    && echo "PASS" || (echo "FAIL"; exit 1)
      - run: just deny
PASS
```

## Threat Flags

None. This plan tightens supply-chain posture (T-24-05-SC mitigation per plan threat register) and preserves the rustls invariant (T-24-05-RUSTLS); it does not introduce new attack surface.

## Known Stubs

None.

## TDD Gate Compliance

Not applicable — plan `type` is `execute`, not `tdd`. No RED/GREEN/REFACTOR cycle.

## Next Phase Readiness

- cargo-deny is the ERROR gate from `3bd1ed9` forward. Every subsequent commit (close-out PR plans 24-01..04, rc.4 cut, final v1.2.0 tag CI run) is gated on `just deny` exit 0.
- FOUND-16 closed — last unticked v1.2 requirement requiring a workflow change.
- Plans 24-06 (rc.4 PREFLIGHT), 24-07 (HUMAN-UAT), 24-08 (FINAL-SHIP-PREFLIGHT) now run against a known-green CI gate per CONTEXT D-11.
- Future advisory remediations (between this commit and final v1.2.0 tag): per CONTEXT § Integration Points L267 — "if an advisory surfaces between rc.4 cut and final-tag, the fix is a HOTFIX PR per the project-wide PR-only policy (D-14)." This plan does NOT need to anticipate them.
- v1.3 close-out should re-evaluate the four new license allowlist entries (`expires: 2026-12-31` comments are the soft signal).

## Self-Check: PASSED

Verified before SUMMARY commit:

- `.github/workflows/ci.yml` modified — `continue-on-error: true` line removed; past-tense comment block at L47-53 in place. Verified via `grep -E "just deny" .github/workflows/ci.yml` returning `      - run: just deny` (and no `continue-on-error: true` neighbor).
- `deny.toml` modified — 4 new atomic SPDX entries (`Unicode-3.0`, `Zlib`, `CDLA-Permissive-2.0`, `CC0-1.0`) + inline NOTE on compound handling. Verified via `just deny` exit 0.
- Commit `3bd1ed9` exists — `git log --oneline -3` shows `3bd1ed9 ci(24-05): promote cargo-deny to blocking (FOUND-16)`.
- `.tmp/deny-check.log` contains `EXIT=4` marker line (checker W5 anti-no-op gate satisfied).
- `cargo tree -i openssl-sys` empty (rustls invariant preserved per PROJECT.md D-19).
- NO Cargo.lock changes (per Branch B license-allowlist-only decision).
- NO modifications to STATE.md or ROADMAP.md (per parallel-executor worktree mode invariant).

---

*Phase: 24-milestone-close-out-final-v1-2-0-ship*
*Plan: 05*
*Completed: 2026-05-17*
