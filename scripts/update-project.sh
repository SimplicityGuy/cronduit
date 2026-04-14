#!/usr/bin/env bash

# update-project.sh — Cronduit dependency and version updater.
#
# This script provides a safe, one-command way to update:
#   - Cargo dependencies (Cargo.lock; with --major, also Cargo.toml via cargo-edit)
#   - Dockerfile base image tags (rust:*, gcr.io/distroless/static-debian12)
#   - GitHub Actions pin SHAs (every `uses: <owner>/<repo>@<sha> # vX.Y.Z` line)
#   - Tailwind standalone binary version (TAILWIND_VERSION in justfile)
#   - Pre-commit hooks (only if .pre-commit-config.yaml exists)
#
# Safety rails:
#   - Refuses to run outside the project root
#   - Refuses to run on the `main` branch (per CLAUDE.md "no direct commits to main")
#   - Creates a feature branch `chore/update-deps-<TS>` and one atomic commit per ecosystem
#   - Re-runs `just openssl-check` after any cargo update (Pitfall 14 — rustls-only guard)
#   - Timestamped backups in backups/project-updates-<TS>/ unless --no-backup
#
# Usage: ./scripts/update-project.sh [options]
#
# Options:
#   --dry-run      Print intended changes without modifying files
#   --major        Include major version upgrades (requires cargo-edit for cargo)
#   --no-backup    Skip creating backups/project-updates-<TS>/
#   --skip-tests   Skip `just nextest` verification after updates
#   --help, -h     Show this help message
#
# Tool invocations delegate to `just` recipes wherever possible so justfile
# remains the single source of truth for command definitions (per CLAUDE.md D-10).
#
# Inspired by: https://github.com/SimplicityGuy/discogsography/blob/main/scripts/update-project.sh
# (Cronduit-adapted: dropped Python/Node/uv paths, added openssl-sys guard, main-branch refusal.)

set -euo pipefail

# -------------------- Defaults --------------------

BACKUP=true
DRY_RUN=false
MAJOR_UPGRADES=false
SKIP_TESTS=false
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="backups/project-updates-${TIMESTAMP}"
FEATURE_BRANCH="chore/update-deps-${TIMESTAMP}"
CHANGES_MADE=false

# -------------------- Visual helpers (emoji logging — approved for this file only) --------------------

EMOJI_INFO="ℹ️"
EMOJI_SUCCESS="✅"
EMOJI_WARNING="⚠️"
EMOJI_ERROR="❌"
EMOJI_ROCKET="🚀"
EMOJI_PACKAGE="📦"
EMOJI_RUST="🦀"
EMOJI_DOCKER="🐳"
EMOJI_ACTIONS="🎬"
EMOJI_TAILWIND="🎨"
EMOJI_HOOKS="🪝"
EMOJI_TEST="🧪"
EMOJI_BACKUP="💾"
EMOJI_GIT="🔀"
EMOJI_VERIFY="🔍"

print_info()    { echo -e "\033[0;34m${EMOJI_INFO}  [INFO]\033[0m $1"; }
print_success() { echo -e "\033[0;32m${EMOJI_SUCCESS}  [SUCCESS]\033[0m $1"; }
print_warning() { echo -e "\033[1;33m${EMOJI_WARNING}  [WARNING]\033[0m $1"; }
print_error()   { echo -e "\033[0;31m${EMOJI_ERROR}  [ERROR]\033[0m $1"; }

print_section() {
    echo ""
    echo -e "\033[1;36m$1  $2\033[0m"
    echo -e "\033[1;36m$(printf '=%.0s' {1..60})\033[0m"
}

# -------------------- Help --------------------

show_help() {
    sed -n '3,30p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
}

# -------------------- Argument parsing --------------------

while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run)    DRY_RUN=true; shift ;;
        --major)      MAJOR_UPGRADES=true; shift ;;
        --no-backup)  BACKUP=false; shift ;;
        --skip-tests) SKIP_TESTS=true; shift ;;
        --help|-h)    show_help ;;
        *)
            print_error "Unknown option: $1"
            show_help
            ;;
    esac
done

# -------------------- Safety rails --------------------

# 1. Must be run from project root (Cargo.toml + Cronduit-specific marker).
if [[ ! -f "Cargo.toml" ]] || [[ ! -f "justfile" ]] || [[ ! -f "Dockerfile" ]]; then
    print_error "Must be run from the Cronduit project root (Cargo.toml + justfile + Dockerfile required)."
    exit 1
fi

# Extra Cronduit marker: either src/main.rs or crates/ must exist.
if [[ ! -f "src/main.rs" ]] && [[ ! -d "crates" ]]; then
    print_error "This does not look like a Cronduit checkout (no src/main.rs and no crates/ directory)."
    exit 1
fi

# 2. Must not be on main branch.
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [[ "$CURRENT_BRANCH" == "main" ]]; then
    print_error "Refusing to run on the 'main' branch. CLAUDE.md policy: all changes land via PR on a feature branch."
    print_info  "Rerun after creating/switching to a feature branch, or let this script create one for you:"
    print_info  "    git checkout -b ${FEATURE_BRANCH}"
    exit 1
fi

# 3. Required tools.
MISSING_TOOLS=()
for tool in cargo git gh jq curl just; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        MISSING_TOOLS+=("$tool")
    fi
done
if [[ ${#MISSING_TOOLS[@]} -gt 0 ]]; then
    print_error "Missing required tools: ${MISSING_TOOLS[*]}"
    print_info  "Install instructions:"
    print_info  "  cargo: https://rustup.rs/"
    print_info  "  gh:    https://cli.github.com/"
    print_info  "  jq, curl, just: platform package manager (brew / apt / dnf)"
    exit 1
fi

# 4. If --major, cargo-edit is required (script prints install instructions).
if [[ "$MAJOR_UPGRADES" == true ]]; then
    if ! cargo upgrade --help >/dev/null 2>&1; then
        print_error "--major requires cargo-edit (provides 'cargo upgrade')."
        print_info  "Install with: cargo install cargo-edit --locked"
        exit 1
    fi
fi

# -------------------- Working-tree cleanliness + feature branch --------------------

if [[ -n "$(git status --porcelain)" ]]; then
    print_warning "You have uncommitted changes. Please stash or commit before running this script."
    print_info    "  git stash push -m 'pre update-project'"
    exit 1
fi

# If the operator is NOT on main but IS on an already-named feature branch, use it as-is.
# Otherwise create the update-deps branch. (We already refused to run on main above.)
print_info "Current branch: ${CURRENT_BRANCH}"

# -------------------- Backup directory --------------------

if [[ "$BACKUP" == true ]] && [[ "$DRY_RUN" == false ]]; then
    mkdir -p "$BACKUP_DIR"
    print_info "${EMOJI_BACKUP} Creating backups in ${BACKUP_DIR}/"
fi

backup_file() {
    local file=$1
    if [[ "$BACKUP" == true ]] && [[ -f "$file" ]] && [[ "$DRY_RUN" == false ]]; then
        local backup_path
        backup_path="${BACKUP_DIR}/$(dirname "$file")"
        mkdir -p "$backup_path"
        cp "$file" "${backup_path}/$(basename "$file").backup"
    fi
}

# -------------------- Cargo --------------------

update_cargo_deps() {
    print_section "${EMOJI_RUST}" "Updating Cargo Dependencies"

    backup_file "Cargo.lock"
    backup_file "Cargo.toml"

    if [[ "$DRY_RUN" == true ]]; then
        if [[ "$MAJOR_UPGRADES" == true ]]; then
            print_info "[DRY RUN] Would run: cargo upgrade --incompatible allow (updates Cargo.toml constraints)"
        fi
        print_info "[DRY RUN] Would run: just update-cargo (refreshes Cargo.lock within constraints)"
        print_info "[DRY RUN] Would run: just openssl-check (Pitfall 14 — must stay empty)"
        return
    fi

    if [[ "$MAJOR_UPGRADES" == true ]]; then
        print_info "Running: cargo upgrade --incompatible allow"
        if cargo upgrade --incompatible allow; then
            print_success "Cargo.toml version requirements updated (major bumps applied)"
        else
            print_warning "cargo upgrade --incompatible failed — falling back to lockfile-only update"
        fi
    fi

    print_info "Running: just update-cargo"
    if just update-cargo; then
        print_success "Cargo.lock refreshed"
    else
        print_error "just update-cargo failed"
        return 1
    fi

    # Pitfall 14 — rustls-only guard. MUST be empty across native + amd64-musl + arm64-musl.
    print_info "Running: just openssl-check (Pitfall 14 rustls-only guard)"
    if just openssl-check; then
        print_success "openssl-sys not present in dep tree (rustls-only confirmed)"
    else
        print_error "FATAL: openssl-sys appeared in dep tree after cargo update. Reverting is up to you."
        print_info  "  Restore from: $BACKUP_DIR/Cargo.lock.backup"
        return 2
    fi

    # Atomic commit per ecosystem.
    if [[ -n "$(git status --porcelain Cargo.toml Cargo.lock 2>/dev/null)" ]]; then
        git add Cargo.toml Cargo.lock
        git commit -m "chore(deps): update cargo dependencies" >/dev/null
        CHANGES_MADE=true
        print_success "Committed: chore(deps): update cargo dependencies"
    else
        print_info "No cargo changes to commit"
    fi
}

# -------------------- Dockerfile base images --------------------

update_dockerfile_base() {
    print_section "${EMOJI_DOCKER}" "Updating Dockerfile Base Images"

    backup_file "Dockerfile"

    # Cronduit uses two base images:
    #   builder:  rust:<version>-slim-bookworm
    #   runtime:  gcr.io/distroless/static-debian12:nonroot
    #
    # The distroless tag is already version-floating (:nonroot pulls the latest
    # nonroot variant of debian12 on each docker build), so we only update the
    # rust:<N.M>-slim-bookworm tag by looking up the latest minor on Docker Hub.

    local current_rust
    current_rust=$(grep -Eo 'FROM[^r]*rust:[0-9]+\.[0-9]+(\.[0-9]+)?-slim-bookworm' Dockerfile | head -1 | grep -Eo 'rust:[0-9]+\.[0-9]+(\.[0-9]+)?' || true)
    if [[ -z "$current_rust" ]]; then
        print_warning "No 'rust:<version>-slim-bookworm' line found in Dockerfile — skipping"
        return
    fi
    print_info "Current builder image: $current_rust-slim-bookworm"

    # Look up latest rust tag matching 'N.M-slim-bookworm' on Docker Hub.
    local latest_rust
    latest_rust=$(curl -sf 'https://hub.docker.com/v2/repositories/library/rust/tags/?page_size=200&name=-slim-bookworm' \
        | jq -r '.results[].name' \
        | grep -E '^[0-9]+\.[0-9]+(\.[0-9]+)?-slim-bookworm$' \
        | sed 's/-slim-bookworm$//' \
        | sort -V \
        | tail -1 || true)
    if [[ -z "$latest_rust" ]]; then
        print_warning "Could not determine latest rust image tag from Docker Hub — skipping"
        return
    fi
    print_info "Latest builder image:  rust:${latest_rust}-slim-bookworm"

    if [[ "$current_rust" == "rust:${latest_rust}" ]]; then
        print_info "Dockerfile already uses the latest rust base — no change"
        return
    fi

    if [[ "$DRY_RUN" == true ]]; then
        print_info "[DRY RUN] Would rewrite ${current_rust}-slim-bookworm → rust:${latest_rust}-slim-bookworm in Dockerfile"
        return
    fi

    # Portable sed (GNU vs BSD).
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "s|${current_rust}-slim-bookworm|rust:${latest_rust}-slim-bookworm|g" Dockerfile
    else
        sed -i "s|${current_rust}-slim-bookworm|rust:${latest_rust}-slim-bookworm|g" Dockerfile
    fi

    # Verify sed applied.
    if grep -q "rust:${latest_rust}-slim-bookworm" Dockerfile; then
        print_success "Dockerfile updated: ${current_rust} → rust:${latest_rust}"
    else
        print_error "sed rewrite failed — Dockerfile not updated"
        return 1
    fi

    # Atomic commit.
    git add Dockerfile
    git commit -m "chore(deps): bump rust base image to ${latest_rust}" >/dev/null
    CHANGES_MADE=true
    print_success "Committed: chore(deps): bump rust base image to ${latest_rust}"
}

# -------------------- Tailwind standalone binary --------------------

update_tailwind_version() {
    print_section "${EMOJI_TAILWIND}" "Updating Tailwind Standalone Binary"

    if ! grep -q 'tailwindlabs/tailwindcss/releases/download/v' justfile; then
        print_warning "No Tailwind download line found in justfile — skipping"
        return
    fi

    local current_tw
    current_tw=$(grep -Eo 'tailwindlabs/tailwindcss/releases/download/v[0-9]+\.[0-9]+\.[0-9]+' justfile | head -1 | sed 's|.*/v||')
    print_info "Current Tailwind: v${current_tw}"

    # Cronduit is pinned to the Tailwind v3 line — v4 breaks tailwind.config.js format (see existing
    # justfile comment "Pinned to v3.4.17 -- v4 breaks tailwind.config.js format"). So we only look
    # at v3.x tags. --major can override this explicitly if the operator has updated the config.
    local filter
    if [[ "$MAJOR_UPGRADES" == true ]]; then
        filter='^v[0-9]+\.[0-9]+\.[0-9]+$'
    else
        filter='^v3\.[0-9]+\.[0-9]+$'
    fi

    local latest_tw
    latest_tw=$(gh api repos/tailwindlabs/tailwindcss/tags --jq '.[].name' 2>/dev/null \
        | grep -E "$filter" \
        | sed 's/^v//' \
        | sort -V \
        | tail -1 || true)
    if [[ -z "$latest_tw" ]]; then
        print_warning "Could not determine latest Tailwind tag — skipping"
        return
    fi
    print_info "Latest Tailwind:  v${latest_tw} (filter: ${filter})"

    if [[ "$current_tw" == "$latest_tw" ]]; then
        print_info "Tailwind already on v${current_tw} — no change"
        return
    fi

    if [[ "$DRY_RUN" == true ]]; then
        print_info "[DRY RUN] Would bump justfile Tailwind version v${current_tw} → v${latest_tw} and re-download binary"
        return
    fi

    backup_file "justfile"

    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "s|v${current_tw}|v${latest_tw}|g" justfile
    else
        sed -i "s|v${current_tw}|v${latest_tw}|g" justfile
    fi

    # Force re-download by deleting the cached binary.
    rm -f ./bin/tailwindcss

    if just tailwind >/dev/null 2>&1; then
        print_success "Tailwind re-downloaded at v${latest_tw}"
    else
        print_error "just tailwind failed after version bump"
        return 1
    fi

    git add justfile
    git commit -m "chore(deps): bump tailwind standalone to v${latest_tw}" >/dev/null
    CHANGES_MADE=true
    print_success "Committed: chore(deps): bump tailwind standalone to v${latest_tw}"
}

# -------------------- Main scaffold (functions added in subsequent tasks) --------------------

main() {
    print_section "${EMOJI_ROCKET}" "Cronduit Project Update"
    print_info "Mode:        $([[ $DRY_RUN == true ]] && echo DRY-RUN || echo LIVE)"
    print_info "Major bumps: $MAJOR_UPGRADES"
    print_info "Backups:     $BACKUP"
    print_info "Skip tests:  $SKIP_TESTS"

    update_cargo_deps
    update_dockerfile_base
    update_tailwind_version
    # update_gha_pins        # Added in Task 4
    # update_precommit_hooks # Added in Task 4
    # run_tests_if_requested # Added in Task 5
    # generate_summary       # Added in Task 5
}

main "$@"
