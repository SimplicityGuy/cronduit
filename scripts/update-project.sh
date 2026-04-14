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

# -------------------- Main scaffold (functions added in subsequent tasks) --------------------

main() {
    print_section "${EMOJI_ROCKET}" "Cronduit Project Update"
    print_info "Mode:        $([[ $DRY_RUN == true ]] && echo DRY-RUN || echo LIVE)"
    print_info "Major bumps: $MAJOR_UPGRADES"
    print_info "Backups:     $BACKUP"
    print_info "Skip tests:  $SKIP_TESTS"

    # Ecosystem functions (added in Tasks 3-4):
    # update_cargo_deps
    # update_dockerfile_base
    # update_gha_pins
    # update_tailwind_version
    # update_precommit_hooks

    # Post-update verification (added in Task 5):
    # run_openssl_guard
    # run_tests_if_requested
    # generate_summary

    print_success "Skeleton complete. Tasks 3-5 will wire the ecosystem updaters."
}

main "$@"
