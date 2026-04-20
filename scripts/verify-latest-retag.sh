#!/usr/bin/env bash

# verify-latest-retag.sh — Assert :latest per-platform digests match :<stable>.
#
# Used by the OPS-09 retag workflow: after a maintainer runs
# `docker buildx imagetools create -t ghcr.io/simplicityguy/cronduit:latest \
#      ghcr.io/simplicityguy/cronduit:<stable>`, this script verifies that the
# resulting :latest tag's per-platform digests (linux/amd64 + linux/arm64)
# equal the source :<stable> tag's per-platform digests. Exits 0 on match,
# non-zero on divergence.
#
# Why per-platform and not the top-level INDEX digest: docker buildx
# imagetools create may re-canonicalize the top-level index JSON (different
# annotation key order, omitted empty arrays), producing a new top-level
# digest even when the image contents are bit-identical. The per-platform
# manifests — the actual data operators pull — are guaranteed identical when
# the retag succeeds. See RESEARCH.md § "imagetools create Mechanics" L145.
#
# Usage:
#   ./scripts/verify-latest-retag.sh              # default :stable-tag = 1.0.1
#   ./scripts/verify-latest-retag.sh 1.1.0        # after v1.1.0 ships
#   ./scripts/verify-latest-retag.sh --help       # show this block
#
# Prerequisites:
#   - docker with buildx plugin (Docker Desktop or Linux docker >= 20.10)
#   - jq (standard on most package managers; `brew install jq` / `apt install jq`)
#   - Read access to ghcr.io/simplicityguy/cronduit (public repo; no auth
#     needed for inspect)

set -euo pipefail

# -------------------- Parse args --------------------

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
  sed -n '3,24p' "$0"
  exit 0
fi

REPO="ghcr.io/simplicityguy/cronduit"
STABLE_TAG="${1:-1.0.1}"

# -------------------- Pre-flight --------------------

if ! command -v docker >/dev/null 2>&1; then
  echo "ERROR: docker not found on PATH" >&2
  exit 2
fi
if ! command -v jq >/dev/null 2>&1; then
  echo "ERROR: jq not found on PATH (install: brew install jq / apt install jq)" >&2
  exit 2
fi
if ! docker buildx version >/dev/null 2>&1; then
  echo "ERROR: docker buildx not available (update Docker or install buildx plugin)" >&2
  exit 2
fi

# -------------------- Inspect & diff --------------------

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

echo "=== expected per-platform digests ($REPO:$STABLE_TAG) ==="
docker buildx imagetools inspect "$REPO:$STABLE_TAG" --raw \
  | jq -r '.manifests[] | select(.platform.os == "linux") | "\(.platform.architecture)\t\(.digest)"' \
  | sort \
  | tee "$tmpdir/expected.txt"

echo ""
echo "=== observed per-platform digests ($REPO:latest) ==="
docker buildx imagetools inspect "$REPO:latest" --raw \
  | jq -r '.manifests[] | select(.platform.os == "linux") | "\(.platform.architecture)\t\(.digest)"' \
  | sort \
  | tee "$tmpdir/observed.txt"

echo ""

# -------------------- Assert --------------------

if ! diff -u "$tmpdir/expected.txt" "$tmpdir/observed.txt"; then
  echo "" >&2
  echo "::error:::latest per-platform digests do NOT match :$STABLE_TAG" >&2
  echo "If you just ran 'docker buildx imagetools create', this means the" >&2
  echo "retag either failed or targeted the wrong source. Re-run:" >&2
  echo "  docker buildx imagetools create -t $REPO:latest $REPO:$STABLE_TAG" >&2
  exit 1
fi

echo ""
echo "OK: :latest per-platform digests match :$STABLE_TAG — OPS-09 retag verified."
echo ""
echo "Platforms checked:"
awk '{print "  - " $1 " -> " $2}' "$tmpdir/expected.txt"
