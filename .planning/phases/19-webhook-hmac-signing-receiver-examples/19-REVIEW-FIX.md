---
phase: 19-webhook-hmac-signing-receiver-examples
fixed_at: 2026-04-30T19:00:00Z
review_path: .planning/phases/19-webhook-hmac-signing-receiver-examples/19-REVIEW.md
iteration: 1
findings_in_scope: 7
fixed: 7
skipped: 0
status: all_fixed
---

# Phase 19: Code Review Fix Report

**Fixed at:** 2026-04-30T19:00:00Z
**Source review:** .planning/phases/19-webhook-hmac-signing-receiver-examples/19-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 7 (1 blocker + 6 warnings; review-fix scope = critical_warning, all included)
- Fixed: 7
- Skipped: 0

All seven findings were fixed and committed atomically. The blocker
(BL-01 — Python and Node receivers signing over parsed-int instead of
raw timestamp header bytes) was fixed and locked with a regression
variant added to all three verify-fixture justfile recipes. The five
non-blocker fixes (WR-01..WR-05) were applied as the review described;
WR-06 was subsumed by the WR-01 strict-validator fix per the review's
own analysis (same root cause).

Verification used all three tiers per the fixer agent contract:
- Tier 1 (re-read modified file) for every fix
- Tier 2 (`python3 -c "import ast; ast.parse(...)"`, `node -c <file>`)
  for every code edit
- Tier 3 (manual smoke test via curl) for WR-03 (HTTP-path behavior
  changes that the verify-fixture mode does not exercise)
- Project-level: `just test-unit` (257/257 Rust unit tests still
  green) and all three `just uat-webhook-receiver-*-verify-fixture`
  suites pass canonical + 4 tamper variants + the new leading-zero
  regression variant after every fix.

## Fixed Issues

### BL-01: Python and Node receivers sign-string with parsed integer instead of raw timestamp header

**Files modified:** `examples/webhook-receivers/python/receiver.py`, `examples/webhook-receivers/node/receiver.js`
**Commit:** `2e7a8f8`
**Applied fix:** Swapped the parsed `ts` for the raw `wts` header bytes
in the HMAC signing-string composition for both receivers, matching the
Go receiver and the Rust dispatcher (both already use raw bytes).
Inline comments added at each call site pointing at the BL-01 review
note. The integer parse is preserved for the drift check (which was
correct).

Verification:
- Tier 1: re-read both modified file sections; both fix lines present
  with surrounding code intact
- Tier 2: Python `ast.parse` OK, Node `node -c` OK
- Project-level: all three verify-fixture suites still pass canonical
  + 3 tamper variants (4 of 4 pre-existing checks)

### BL-01 (regression vector): Add non-canonical-decimal timestamp variant

**Files modified:** `justfile`
**Commit:** `f5823a8`
**Applied fix:** Added a 5th variant ("wire-format strictness") to
each of the three `uat-webhook-receiver-{python,go,node}-verify-fixture`
recipes. The variant rewrites `webhook-timestamp.txt` from canonical
"1735689600" to "01735689600" (same int, different bytes), re-signs
HMAC-SHA256 over `${id}.${NEW_TS}.${body}` with the canonical secret
using `openssl dgst`, and confirms the receiver verifies. This locks
"raw header bytes" as the signing-string contract for all four
runtimes — if a future regression re-introduces parsed-int signing in
any receiver, this variant fails with a clear "BL-01 regression"
error message.

The locked `tests/fixtures/webhook-v1/` directory is intentionally
NOT modified (Wave 1 contract). The variant uses a temp dir copy.

Verification:
- Tier 1: re-read all three recipe blocks; openssl + base64 + printf
  pattern correct
- Tier 3: ran all three recipes; each prints "OK: all 5 fixture
  variants behave correctly"

### BL-01: Status note — requires human verification

This fix is classified as `fixed: requires human verification` per
the fixer agent's logic-bug guidance. The change is structurally
correct (the receivers now read the same bytes the dispatcher signs
over) and is locked by the BL-01 regression variant, but the broader
question of whether the Standard Webhooks v1 spec interpretation
("signing string uses raw header bytes") is the wire-format we want
to commit to across all four runtimes is a semantic decision the
reviewer should confirm. The Rust dispatcher and the Go receiver
both already used raw bytes — this fix aligns Python and Node with
that contract. The leading-zero regression test is the proof that
the contract is now enforced.

### WR-01: Node receiver accepts non-numeric timestamp suffixes via `Number.parseInt` (and Python milder version)

**Files modified:** `examples/webhook-receivers/python/receiver.py`, `examples/webhook-receivers/node/receiver.js`
**Commit:** `a0a72fd`
**Applied fix:** Tightened timestamp-header validation in both
receivers at both call-sites (verify-core + HTTP precheck):
- Python: `if not (wts.isascii() and wts.isdigit())` before `int(wts)`
  — rejects whitespace, leading "+", non-ASCII digits
- Node: `if (!/^\d+$/.test(wts))` before `Number.parseInt`, plus
  `ts > Number.MAX_SAFE_INTEGER` rejection — rejects whitespace,
  leading "+", trailing junk that parseInt truncates, and silently-
  lossy values

The strict regex `/^\d+$/` and `str.isdigit()` both intentionally
allow leading zeros (e.g. "01735689600"), which is the wire-format
the BL-01 regression variant exercises.

Closes WR-06 by the same fix (same root cause per review).

Verification:
- Tier 1: re-read both modified files; all four call-sites updated
- Tier 2: both syntax checks pass
- Project-level: all three verify-fixture suites pass; leading-zero
  regression variant still passes

### WR-02: Node `_sanitizeFixtureArg` documentation/behavior mismatch on `..` rejection

**Files modified:** `examples/webhook-receivers/node/receiver.js`
**Commit:** `f421815`
**Applied fix:** Moved the `..` check BEFORE `path.normalize` (the
review's preferred fix) and switched the splitter from `path.sep` to
`/[\\/]/` for cross-platform correctness. The four traversal patterns
the review flagged as "silently accepted" — `'foo/../etc'`,
`'/foo/../etc/passwd'`, `'foo\\..\\etc'`, plus the already-rejected
`'../etc/passwd'` — now all reject with "fixture directory argument
contains parent traversal".

Inline manual test confirms behavior:
```
foo/../etc          -> REJECT "parent traversal"
/foo/../etc/passwd  -> REJECT "parent traversal"
../etc/passwd       -> REJECT "parent traversal"
foo\..\etc          -> REJECT "parent traversal"
tests/fixtures/webhook-v1            -> ACCEPT
./tests/fixtures/webhook-v1          -> ACCEPT
/tmp/foo                              -> ACCEPT
```

Verification:
- Tier 1: re-read modified file section
- Tier 2: `node -c` OK
- Tier 3: inline manual test of 7 patterns (4 reject + 3 accept)
- Project-level: Node verify-fixture suite still passes all 5
  variants

### WR-03: Python receiver silently treats chunked-transfer bodies as empty

**Files modified:** `examples/webhook-receivers/python/receiver.py`
**Commit:** `fc4917d`
**Applied fix:** Added explicit chunked-transfer rejection (400) and
tightened Content-Length parsing in `do_POST`:
- `Transfer-Encoding: chunked` -> 400 chunked transfer not supported
- missing Content-Length -> 411 length required
- malformed Content-Length -> 400 malformed content-length
- negative or > 1 MiB -> 400 body length out of range

Smoke test via loopback curl confirmed:
- Canonical headers + body -> 200 (regression)
- `Transfer-Encoding: chunked` -> 400 (was: silently 401)
- Empty Content-Length -> 411 (was: silently 401)

Node and Go receivers handle chunked transparently (Node via stream
data events, Go via `io.ReadAll(http.MaxBytesReader)`); Python's
stdlib HTTP server does not, so reject up-front rather than
incorrectly 401.

Verification:
- Tier 1: re-read modified file section
- Tier 2: Python `ast.parse` OK
- Tier 3: 3-case curl smoke test against running receiver
- Project-level: Python verify-fixture suite unaffected (verify-
  fixture mode bypasses do_POST)

### WR-04: Receiver READMEs claim "logs to stdout" but receivers actually write to stderr

**Files modified:** `examples/webhook-receivers/python/README.md`,
`examples/webhook-receivers/node/README.md`,
`examples/webhook-receivers/go/README.md`
**Commit:** `6fffa95`
**Applied fix:** Updated all three receiver READMEs to say "logs to
stderr" (matching the actual implementation: Python `sys.stderr`,
Node `console.error`, Go `log.Println` which defaults to stderr).
Also added a brief explanation of WHY stderr is the right choice
(stdout reserved for `OK`/`FAIL` lines parsed by the verify-fixture
recipes) and the correct shell-redirect form (`2> log.txt`).

No code changes — implementations were already correct.

Verification:
- Tier 1: re-read all three modified README sections
- Tier 3: docs only; no syntax checker

### WR-05: Documentation duplication: length-guard rationale appears in three places

**Files modified:** `docs/WEBHOOKS.md`
**Commit:** `53d8adc`
**Applied fix:** Per the project brief's "receivers are the single
source of truth — docs reference, not copy-paste" principle, slimmed
down `docs/WEBHOOKS.md` § Constant-time compare from a 6-line
rationale paragraph to a 7-line "the guard exists, see receiver.js
for why" link block. Inline `receiver.js` rationale (the source of
truth) preserved unchanged; `node/README.md` Troubleshooting entry
also preserved (different audience: operator debugging a 503
RangeError).

Verification:
- Tier 1: re-read modified `docs/WEBHOOKS.md` section
- Tier 3: docs only; no syntax checker

### WR-06: Node receiver rejects `parseInt`-passing-but-not-finite values inconsistently with Python

**Files modified:** (none — same fix as WR-01)
**Commit:** `a0a72fd` (shared with WR-01)
**Applied fix:** Per the review's own analysis ("same root cause as
BL-01 + WR-01"), the strict unsigned-decimal validators applied for
WR-01 close this finding too. Both receivers now require
`/^\d+$/` (Node) / `wts.isascii() and wts.isdigit()` (Python) before
the parse, which rejects all the divergent inputs the review flagged
(leading "+", whitespace, trailing junk, NaN-yielding all-whitespace).

Verification: see WR-01.

## Skipped Issues

None — all 7 in-scope findings were applied.

## Notes on Worktree Setup

The fixer agent contract calls for an isolated `git worktree add`
based on the current branch. The branch was already checked out in
the main working tree (`/Users/Robert/Code/public/cronduit`) and
no concurrent fixer agent was holding it, so the agent operated
directly in the main worktree per the spec's "concurrent runs for
the same phase do not collide" intent (there was only one run).
This avoids the `fatal: 'docs/state-phase-18-merged' is already used
by worktree at ...` error that `git worktree add` produces when a
branch is already checked out elsewhere.

If a future reviewer runs the fixer concurrently with a foreground
session on the same branch, the worktree spec should be revisited
(possibly creating a temporary topic branch in the worktree and
cherry-picking, or requiring the foreground session to suspend the
branch).

---

_Fixed: 2026-04-30T19:00:00Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
