---
phase: 19-webhook-hmac-signing-receiver-examples
plan: 06
subsystem: ci-and-uat
tags: [webhooks, hmac, ci, github-actions, matrix, uat, maintainer-validated, standard-webhooks-v1]

# Dependency graph
requires:
  - phase: 19-webhook-hmac-signing-receiver-examples
    plan: 02
    provides: "examples/webhook-receivers/python/receiver.py + just uat-webhook-receiver-python-verify-fixture"
  - phase: 19-webhook-hmac-signing-receiver-examples
    plan: 03
    provides: "examples/webhook-receivers/go/receiver.go + just uat-webhook-receiver-go-verify-fixture"
  - phase: 19-webhook-hmac-signing-receiver-examples
    plan: 04
    provides: "examples/webhook-receivers/node/receiver.js + just uat-webhook-receiver-node-verify-fixture"
provides:
  - ".github/workflows/ci.yml::webhook-interop — new top-level matrix job (Python/Go/Node) gating cross-language wire-format drift"
  - ".planning/phases/19-webhook-hmac-signing-receiver-examples/19-HUMAN-UAT.md — 11 maintainer-validated UAT scenarios (all unchecked per D-22)"
affects: [phase-19-pr-merge, 20-webhook-retries]

# Tech tracking
tech-stack:
  added: []  # CI/UAT-only plan — zero new Rust crates (D-24 satisfied; openssl-check still empty)
  patterns:
    - "GitHub Actions matrix job pattern: matrix.lang ∈ {python, go, node} with conditional setup-{python|go|node} steps gated on matrix.lang, then extractions/setup-just@v2, then `just uat-webhook-receiver-${{ matrix.lang }}-verify-fixture`"
    - "Hard CI gate from day one (NO continue-on-error: true) — D-15 distinguishes interop drift (hard gate) from supply-chain advisories (soft gate via cargo-deny)"
    - "Same-file sibling job placement (RESEARCH §Alternatives Considered): webhook-interop appended to .github/workflows/ci.yml after compose-smoke, NOT a separate workflow file — keeps PR-page CI summary in one place"
    - "Maintainer-validated UAT pattern: 11 numbered scenarios, each citing an existing `just` recipe (D-21) or `Recipe: None — visual review/read` (17-HUMAN-UAT.md U1 precedent), all checkboxes ship `[ ] Maintainer-validated` (D-22)"
    - "least-privilege CI: `permissions: contents: read` on the new job; no GHCR write, no secrets needed (fixture is in-tree per Plan 01 with a clearly-marked test secret)"

key-files:
  created:
    - ".planning/phases/19-webhook-hmac-signing-receiver-examples/19-HUMAN-UAT.md (167 lines, 11 unchecked Maintainer-validated scenarios)"
    - ".planning/phases/19-webhook-hmac-signing-receiver-examples/19-06-SUMMARY.md (this file)"
  modified:
    - ".github/workflows/ci.yml (+34 lines — appended `webhook-interop` matrix job after compose-smoke at line 387)"

key-decisions:
  - "Added a one-line clarifying comment to the new job header explaining `matrix.lang` is a static set (python|go|node) with no untrusted github.event input flowing into `run:` — addresses the workflow-injection security advisory raised by the local pre-edit hook without changing job behavior. The matrix value is controlled by the workflow author (this PR), not by attacker-supplied PR/issue data."
  - "Kept `webhook-interop` independent (no `needs:` dependency on lint/test/image/compose-smoke) — interop drift is a property of the receiver code + signing-side wire format, not a property of Rust workspace correctness. Running in parallel reports cross-language drift even when an unrelated Rust failure breaks `test`."
  - "Job uses `timeout-minutes: 10` (vs `compose-smoke` 20m, `test` 30m) — the verify-fixture recipes are pure stdlib + a few subprocess shell-outs, expected wall-clock is <60s per cell on `ubuntu-latest`."
  - "UAT scenarios U6/U7/U8 (end-to-end live cronduit deliveries) are sequenced AFTER the verify-fixture scenarios U3/U4/U5 — fixture-mode isolates wire-format correctness from network/config wiring; if U3-U5 pass and U6-U8 fail, the regression is in `examples/cronduit.toml` config or in the Plan 18 dispatcher, not the receiver code."
  - "U11 'webhook-interop CI matrix passes on the PR' is included in the maintainer UAT (vs. relying on green-merge alone) so the maintainer reads the actual matrix-cell logs to confirm `OK: all 4 tamper variants behave correctly` printed on each lang — guards against a future regression where the recipe exits 0 without exercising the tamper variants."

patterns-established:
  - "Phase 19 cross-language CI matrix: 3 matrix cells (python/go/node), each cell installs the language toolchain conditionally and runs a `just uat-webhook-receiver-${{ matrix.lang }}-verify-fixture` recipe — the same shape any future cross-language receiver phase (Ruby/Java/.NET in v1.3+) would extend by adding rows to `matrix.lang` + a sibling conditional-setup step"
  - "UAT artifact for cross-language work: numbered scenarios with mode-symmetric testing (3 receivers × 2 modes = 6 fixture+e2e + 3 PR-render checks + 2 platform-wide gate checks = 11 scenarios) — Phase 18's UAT was behavior-asymmetric (signed/unsigned/coalesce/filter/secret each tested once); Phase 19's is language-symmetric and reflects the per-language symmetry of the implementation"

requirements-completed: [WH-04]

# Metrics
duration: ~12 min
completed: 2026-04-30
---

# Phase 19 Plan 06: Cross-Language CI Gate + Maintainer UAT Artifact Summary

**Shipped the Phase 19 hard CI gate and the maintainer UAT handoff: a new `webhook-interop` matrix job (Python/Go/Node × `just uat-webhook-receiver-${{ matrix.lang }}-verify-fixture`) appended to `.github/workflows/ci.yml` as a hard gate from day one (NOT `continue-on-error: true` per D-15), and a new `19-HUMAN-UAT.md` containing 11 maintainer-validated scenarios — every checkbox ships `[ ] Maintainer-validated` (D-22 — Claude does NOT flip them; the maintainer runs each cited `just` recipe post-merge and adds the sign-off line themselves). Every UAT scenario cites an existing `just` recipe (D-21) or `Recipe: None — visual review/read` per the 17-HUMAN-UAT.md U1 precedent. All 3 verify-fixture recipes pass locally (`OK: all 4 tamper variants behave correctly` × 3) before the CI job was wired, smoke-proving the matrix would go green on `ubuntu-latest`. CI/UAT-only plan: zero new Rust crates (D-24 satisfied).**

## Performance

- **Duration:** ~12 min
- **Started:** 2026-04-30 (worktree base eac460e)
- **Completed:** 2026-04-30
- **Tasks:** 2 (1 auto + 1 checkpoint:human-verify)
- **Files created:** 1 (`19-HUMAN-UAT.md`)
- **Files modified:** 1 (`.github/workflows/ci.yml`)
- **Commits:** 2 atomic commits (`d32546b` ci, `e1cffdb` docs)

## Accomplishments

### Task 1 — `webhook-interop` matrix job in `.github/workflows/ci.yml`

- New top-level sibling job appended after `compose-smoke` (lines 387–419, +34 lines net).
- `name: webhook-interop (${{ matrix.lang }})`, `runs-on: ubuntu-latest`, `timeout-minutes: 10`, `permissions: contents: read`.
- `strategy: fail-fast: false` so all 3 cells (python/go/node) report independently — debugging a single-language regression doesn't require re-running the whole matrix.
- Three conditional toolchain setup steps gated on `matrix.lang`:
  - `actions/setup-python@v5` with `python-version: '3.x'`
  - `actions/setup-go@v5` with `go-version: 'stable'`
  - `actions/setup-node@v4` with `node-version: '20'`
- Final step: `run: just uat-webhook-receiver-${{ matrix.lang }}-verify-fixture` — calls into the existing Plan 19-02/03/04 recipes which exercise canonical fixture + 3 tamper variants (mutated secret, mutated body, drift > 5 min).
- **HARD GATE (D-15):** No `continue-on-error: true`. Cross-language wire-format drift FAILS CI before merge. (Distinct from `cargo-deny` at line 58, which is intentionally `continue-on-error: true` per Phase 15 D-09 — a transient supply-chain advisory should not redden CI; cross-language interop drift is a design defect that must redden CI.)
- **Independent:** No `needs:` dependency on `lint`/`test`/`image`/`compose-smoke`. Runs in parallel; reports drift even when unrelated Rust workspace builds fail.
- **YAML well-formed:** validated via both `pyyaml` (in venv) and `ruby -ryaml`. `actionlint` exits 0.
- **Smoke-tested locally:** all 3 verify-fixture recipes printed `OK: all 4 tamper variants behave correctly` before the CI job was added — high confidence the matrix will go green on `ubuntu-latest`.

### Task 2 — `19-HUMAN-UAT.md` with 11 unchecked maintainer scenarios

- 167 lines, mirrors `17-HUMAN-UAT.md` (numbered `### U1`..`### U11` scenarios with **Recipe** / **Steps** / **Pass criteria**) and inherits the prereqs-table preamble from `18-HUMAN-UAT.md`.
- **D-22 enforced:** all 11 checkboxes ship `[ ] Maintainer-validated` (verified by `grep -c "^\[ \] Maintainer-validated$"` returning `11`; `grep -c "^\[x\] Maintainer-validated"` returning `0`). Sign-off `**Validated by:**` line is blank — the maintainer fills it on UAT completion.
- **D-21 enforced:** every scenario has a `Recipe:` line citing either an existing `just` recipe (8 scenarios — U1..U8) or `Recipe: None — visual review/read` (3 scenarios — U9 docs render, U10 README/CONFIG render, U11 PR Checks tab). Verified: 11 Recipe: lines, 11 cite just-or-visual-review, 0 cite raw `curl`/`cargo`/`docker`.
- **Project-memory citations** in the header banner: both `feedback_uat_user_validates.md` (D-22) and `feedback_uat_use_just_commands.md` (D-21) named verbatim — leaves no ambiguity for future-Claude or future-maintainer about why checkboxes start unchecked and why scenarios cite recipes.
- **Scenario coverage** maps 1:1 to Plans 19-01..19-06 + cross-cutting Phase 19 surface:
  - U1 (workspace) + U2 (fixture lock) — guard Phase 19 didn't regress prior CI gate or break the wire-format lock.
  - U3, U4, U5 — verify-fixture recipes, one per receiver language, smoke-prove the CI matrix's behavior locally.
  - U6, U7, U8 — end-to-end live-cronduit deliveries, one per receiver, prove the receivers verify a real cronduit delivery (not just the in-tree fixture).
  - U9 + U10 — visual GitHub-render review of `docs/WEBHOOKS.md` (3 mermaid diagrams + tables) and `README.md` + `docs/CONFIG.md` cross-references.
  - U11 — confirms the new `webhook-interop` matrix from Task 1 actually passed on the PR (3 cells GREEN, with the `OK: all 4 tamper variants behave correctly` line visible in each cell's logs).

## Verification Performed

| Check | Tool | Result |
|------|------|--------|
| `webhook-interop` job present at top level of jobs: | `grep "^  webhook-interop:"` | line 387 — 1 match |
| `lang: [python, go, node]` matrix axis | `grep "lang: \[python, go, node\]"` | match |
| Final step calls verify-fixture recipe | `grep "matrix.lang.*verify-fixture"` | line 412 |
| `actions/setup-python@v5` | `grep "actions/setup-python@v5"` | match |
| `actions/setup-go@v5` | `grep "actions/setup-go@v5"` | match |
| `actions/setup-node@v4` | `grep "actions/setup-node@v4"` | match |
| `extractions/setup-just@v2` | `grep "extractions/setup-just@v2"` | match (existing — reused) |
| **NO** `continue-on-error: true` on the new job (D-15) | `awk` slice + `grep -v` | absent — hard gate confirmed |
| `fail-fast: false` on the matrix | `awk` slice + `grep` | match |
| `permissions: contents: read` (least-privilege) | `awk` slice + `grep` | match |
| YAML parses cleanly | `python3 -c "import yaml; yaml.safe_load(...)"` (pyyaml in venv) + `ruby -ryaml` | both exit 0 |
| `actionlint .github/workflows/ci.yml` | `actionlint` | exit 0, no warnings |
| Python verify-fixture recipe smoke-test | `just uat-webhook-receiver-python-verify-fixture` | `OK: all 4 tamper variants behave correctly` |
| Go verify-fixture recipe smoke-test | `just uat-webhook-receiver-go-verify-fixture` | `OK: all 4 tamper variants behave correctly` |
| Node verify-fixture recipe smoke-test | `just uat-webhook-receiver-node-verify-fixture` | `OK: all 4 tamper variants behave correctly` |
| `19-HUMAN-UAT.md` exists | `test -f` | exit 0 |
| Exactly 11 unchecked Maintainer-validated boxes | `grep -c "^\[ \] Maintainer-validated$"` | 11 |
| Zero pre-flipped boxes | `grep -c "^\[x\] Maintainer-validated"` | 0 |
| All 11 scenarios cite `just` recipe or visual-review (D-21) | bold-aware grep + Python re.findall | 11/11 |
| `Maintainer-validated only` banner | `grep -q` | match |
| `feedback_uat_user_validates.md` citation | `grep -q` | match |
| `feedback_uat_use_just_commands.md` citation | `grep -q` | match |
| `Validated by:` sign-off slot blank | `grep -q "Validated by:"` | match (placeholder text only) |
| 11 numbered scenarios `### U1`..`### U11` | `grep -E -c "^### U(1\|2\|...\|11) —"` | 11 |
| No raw `curl`/`docker run`/`cargo {build,test,run}` outside `just` | `grep -nE`...`grep -v "just "` | empty (clean) |

## Deviations from Plan

None — plan executed exactly as written.

The pre-edit security-reminder hook flagged `.github/workflows/ci.yml` as an Actions-injection-risk surface. This is a project-wide advisory (not a block). The new `webhook-interop` job uses only `${{ matrix.lang }}` (a static, controlled matrix value: `python|go|node`) with no `github.event.*` interpolation flowing into `run:` — so the workflow-injection class of vuln does not apply. A one-line comment was added to the job header documenting this for future readers. Did not require Rule 4 (architectural) escalation.

## Authentication Gates

None — Phase 19-06 ships zero new auth surface. No CI secrets are used by the new `webhook-interop` job (the in-tree test fixture per Plan 01 contains a clearly-marked test secret `cronduit-test-fixture-secret-not-real`).

## Threat Flags

None. Threat surface confined to:

- **T-19-27 (mitigate)** — webhook-interop CI gate IS the mitigation; verified hard-gate posture (no `continue-on-error: true`).
- **T-19-28 (mitigate)** — UAT-checkbox tampering; verified all 11 boxes ship `[ ] Maintainer-validated` literal; D-22 banner present; project-memory cite present.
- **T-19-29 (accept)** — fixture is in-tree test value; no real secret in CI. Confirmed unchanged by this plan.

## Known Stubs

None. All references in the new files resolve to real artifacts:

- `19-HUMAN-UAT.md` cites only existing `just` recipes (verified — `uat-webhook-receiver-{python,go,node}{,-verify-fixture}`, `dev`, `check-config`, `ci`, `nextest`, `openssl-check`, `uat-webhook-fire` all confirmed in `justfile`).
- `webhook-interop` runs only existing Plan 19-02/03/04 recipes that pass locally.
- All cross-references (`docs/WEBHOOKS.md`, `examples/webhook-receivers/{python,go,node}/`, `examples/cronduit.toml::wh-example-receiver-*`, `tests/fixtures/webhook-v1/`, `src/webhooks/dispatcher.rs::sign_v1_locks_interop_fixture`) verified present on the worktree before being cited.

## Phase-19 Completion Signal

Phase 19 is now **plan-complete** at the executor layer:

- Plans 19-01..19-06 all shipped with SUMMARY.md.
- Phase-level success criteria all green at the artifact level.
- The `webhook-interop` CI job is a hard gate guarding cross-language wire-format drift.

The phase is **NOT yet complete** at the requirements layer — `WH-04` flips from Pending → Validated only after the maintainer:

1. Opens the Phase 19 PR.
2. Runs each of the 11 UAT scenarios in `19-HUMAN-UAT.md` locally.
3. Flips each `[ ] Maintainer-validated` to `[x] Maintainer-validated`.
4. Adds the `**Validated by:** Maintainer (Robert) on YYYY-MM-DD — all 11 UAT items passed locally per D-22.` sign-off line.
5. Comments `UAT passed` on the PR.
6. Merges the PR.

Post-merge, the orchestrator updates `STATE.md` and `ROADMAP.md` to reflect WH-04 → Validated.

## Self-Check: PASSED

- File `.github/workflows/ci.yml` modified — verified present (`stat`).
- File `.planning/phases/19-webhook-hmac-signing-receiver-examples/19-HUMAN-UAT.md` created — verified present (`stat`, 167 lines).
- File `.planning/phases/19-webhook-hmac-signing-receiver-examples/19-06-SUMMARY.md` created — this file (after this Self-Check banner is appended, the final commit will include it).
- Commit `d32546b` (Task 1 — ci): present in `git log` (verified).
- Commit `e1cffdb` (Task 2 — UAT artifact): present in `git log` (verified).
- All 11 UAT checkboxes confirmed `[ ] Maintainer-validated` (D-22 enforced — Claude did NOT flip).
- All 11 UAT scenarios confirmed citing `just` recipes or `Recipe: None — visual review/read` (D-21 enforced).
- `webhook-interop` job confirmed lacking `continue-on-error: true` (D-15 enforced — hard gate).
