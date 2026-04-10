#!/usr/bin/env bash
# Test: justfile recipes conformance (01-06 acceptance criteria)
set -euo pipefail

FAIL=0
pass() { echo "  PASS: $1"; }
fail() { echo "  FAIL: $1"; FAIL=1; }

echo "=== Justfile Recipe Tests ==="

# 1. justfile exists
[ -f justfile ] && pass "justfile exists" || fail "justfile missing"

# 2. shell setting
grep -q 'set shell := \["bash", "-euo", "pipefail", "-c"\]' justfile 2>/dev/null \
  && pass "shell setting" || fail "shell setting missing"

# 3. ci chain
grep -q 'ci: fmt-check clippy openssl-check nextest schema-diff image' justfile 2>/dev/null \
  && pass "ci chain" || fail "ci chain missing"

# 4. install-targets recipe
grep -q '^install-targets:' justfile 2>/dev/null \
  && pass "install-targets recipe" || fail "install-targets recipe missing"

# 5. openssl-check depends on install-targets
grep -q 'openssl-check: install-targets' justfile 2>/dev/null \
  && pass "openssl-check depends on install-targets" || fail "openssl-check dependency missing"

# 6. openssl-check loops over targets
grep -q 'aarch64-unknown-linux-musl.*x86_64-unknown-linux-musl' justfile 2>/dev/null \
  && pass "openssl-check loops targets" || fail "openssl-check target loop missing"

# 7. grep -q . pattern
grep -q 'grep -q \.' justfile 2>/dev/null \
  && pass "grep -q . pattern" || fail "grep -q . pattern missing"

# 8. schema-diff recipe
grep -q 'cargo test --test schema_parity' justfile 2>/dev/null \
  && pass "schema-diff recipe" || fail "schema-diff recipe missing"

# 9. migrate: dev
grep -q '^migrate: dev' justfile 2>/dev/null \
  && pass "migrate alias" || fail "migrate alias missing"

# 10. D-01 deferral comment
grep -qi 'D-01.*deferred\|deferred.*D-01' justfile 2>/dev/null \
  && pass "D-01 deferral comment" || fail "D-01 deferral comment missing"

# 11. All expected recipes exist
for recipe in fmt fmt-check clippy test nextest install-targets image build build-release clean tailwind db-reset sqlx-prepare docker-compose-up; do
  grep -q "^${recipe}" justfile 2>/dev/null \
    && pass "recipe: $recipe" || fail "recipe missing: $recipe"
done

# 12. image-push and check-config (parameterized)
grep -q '^image-push' justfile 2>/dev/null \
  && pass "recipe: image-push" || fail "recipe missing: image-push"
grep -q '^check-config' justfile 2>/dev/null \
  && pass "recipe: check-config" || fail "recipe missing: check-config"

# 13. just --list should print >= 19 recipes
if [ -f justfile ]; then
  COUNT=$(just --list 2>/dev/null | grep -c '^\s' || echo 0)
  [ "$COUNT" -ge 19 ] && pass "just --list >= 19 recipes ($COUNT)" || fail "just --list only $COUNT recipes (need >= 19)"
fi

echo ""
if [ "$FAIL" -eq 0 ]; then
  echo "ALL TESTS PASSED"
else
  echo "SOME TESTS FAILED"
  exit 1
fi
