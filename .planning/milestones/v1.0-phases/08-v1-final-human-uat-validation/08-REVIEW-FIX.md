---
phase: 08-v1-final-human-uat-validation
fixed_at: 2026-04-14T03:10:40Z
review_path: .planning/phases/08-v1-final-human-uat-validation/08-REVIEW.md
iteration: 1
findings_in_scope: 5
fixed: 5
skipped: 0
status: all_fixed
---

# Phase 8: Code Review Fix Report

**Fixed at:** 2026-04-14T03:10:40Z
**Source review:** .planning/phases/08-v1-final-human-uat-validation/08-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 5
- Fixed: 5
- Skipped: 0

## Fixed Issues

### WR-01: `set -eu` missing from /health polling loop in ci.yml

**Files modified:** `.github/workflows/ci.yml`
**Commit:** `2840ac0`
**Applied fix:** Wrapped the `body=$(curl -sSf …)` assignment in the "Assert /health body contains status:ok" step with an explicit `|| { echo "ERROR: /health curl failed with exit $?"; exit 1; }` guard so curl failures produce a clear diagnostic instead of a misleading grep failure.

---

### WR-02: Unquoted `${{ matrix.compose }}` in sed rewrite step

**Files modified:** `.github/workflows/ci.yml`
**Commit:** `58d041d`
**Applied fix:** Replaced the `grep -q 'image: cronduit:ci'` verification with an exact-count check (`grep -c … || true` + `if [ "$count" -ne 1 ]`) so the assertion fails if sed rewrote zero lines or more than one — catching partial rewrites in multi-service compose files like `docker-compose.secure.yml`.

---

### WR-03: Shared poll deadline allows first slow job to starve later jobs

**Files modified:** `.github/workflows/ci.yml`
**Commit:** `6c7febd`
**Applied fix:** Moved `deadline=$(( $(date +%s) + BUDGET_SECS ))` inside the `for name in $JOBS` loop so each job receives its own independent budget, eliminating the latent flakiness where a slow first job (e.g. cold `hello-world` pull) could exhaust time left for subsequent jobs.

---

### WR-04: `cronduit.toml` sets `bind = "0.0.0.0:8080"` — contradicts README and SPEC defaults

**Files modified:** `examples/cronduit.toml`
**Commit:** `b3b96b9`
**Applied fix:** Added a three-line inline comment directly on the `bind = "0.0.0.0:8080"` line explaining it is required for docker-compose port publishing and must never be used outside a reverse-proxy deployment. This resolves the direct contradiction with the comment block immediately above the `[server]` section.

---

### WR-05: `docker-compose.secure.yml` missing `group_add` — cronduit user cannot write to named volume

**Files modified:** `examples/docker-compose.secure.yml`
**Commit:** `f91ec24`
**Applied fix:** Added a three-line comment on the `cronduit:` service block explaining that `group_add` is intentionally absent: Docker API traffic flows through `dockerproxy` (no direct socket mount) and the cronduit user is UID 1000 matching the `/data` volume ownership set in the Dockerfile.

---

_Fixed: 2026-04-14T03:10:40Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_
