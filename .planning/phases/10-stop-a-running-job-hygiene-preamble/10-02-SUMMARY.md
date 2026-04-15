---
phase: 10-stop-a-running-job-hygiene-preamble
plan: 02
subsystem: dependency-hygiene
tags: [rand, csrf, scheduler, random, FOUND-12, hygiene]
requires: []
provides:
  - rand 0.9 API surface available across the codebase
  - clean baseline for Wave 2 Stop spike (plan 10-03)
affects:
  - src/web/csrf.rs (CSRF token generator)
  - src/scheduler/sync.rs (@random batch resolution entrypoint)
  - src/scheduler/reload.rs (per-job re-randomize on reload)
  - src/scheduler/random.rs (@random slot picker)
tech-stack:
  added: []
  updated:
    - "rand 0.8 -> 0.9.2 (direct dep)"
  patterns:
    - "rand::rng() replaces rand::thread_rng() (thread-local CSPRNG accessor)"
    - "Rng::fill(&mut [u8]) replaces RngCore::fill_bytes"
    - "rng.random_range(min..=max) replaces rng.gen_range"
key-files:
  created: []
  modified:
    - Cargo.toml
    - Cargo.lock
    - src/web/csrf.rs
    - src/scheduler/sync.rs
    - src/scheduler/reload.rs
    - src/scheduler/random.rs
decisions:
  - "Stay on rand 0.9 (NOT 0.10) per D-13 to avoid the gen->random trait rename churn beyond the 0.9 diff"
metrics:
  duration: ~3 minutes
  completed: 2026-04-15
requirements:
  - FOUND-12
---

# Phase 10 Plan 02: rand 0.9 hygiene migration Summary

Mechanically migrated the `rand` crate from 0.8 to 0.9.2 across four source files and Cargo.toml. No behavioral change — same thread-local CSPRNG primitive (ChaCha-based) for both CSRF token generation and `@random` slot picking; only the API surface changed.

## What Shipped

### Cargo.toml
- Line 105-106: comment updated from "Random bytes for CSRF tokens (D-11)" to "Random bytes for CSRF tokens and @random cron slot picker (D-13/FOUND-12)"; dep bumped `rand = "0.8"` -> `rand = "0.9"`.

### src/web/csrf.rs
- Line 10: `use rand::RngCore;` -> `use rand::Rng;` (the `fill` method now lives on `Rng` in 0.9, not `RngCore`).
- Line 21: `rand::thread_rng().fill_bytes(&mut token);` -> `rand::rng().fill(&mut token[..]);`. The `[..]` slice reborrow is required because `Rng::fill` takes `&mut [T]`, not `&mut [u8; N]`. Token array remains `[0u8; 32]`; output byte format (64-char hex) unchanged; existing session cookies remain valid.

### src/scheduler/sync.rs
- Line 131: `let mut rng = rand::thread_rng();` -> `let mut rng = rand::rng();` inside `resolve_random_schedules_batch` entrypoint.

### src/scheduler/reload.rs
- Line 171: `let mut rng = rand::thread_rng();` -> `let mut rng = rand::rng();` inside per-job re-randomize path (RAND-03c).

### src/scheduler/random.rs
- Line 10: `use rand::Rng;` — UNCHANGED (still required and the import stays stable across 0.8/0.9).
- Line 97: `rng.gen_range(min..=max).to_string()` -> `rng.random_range(min..=max).to_string()` inside `resolve_fields`.
- Lines 268-269: `use rand::SeedableRng;` + `use rand::rngs::StdRng;` in the test module — UNCHANGED (both paths are stable across 0.8 and 0.9).

## Cargo.lock delta

Before: `rand = "0.8.5"` as our direct dep.
After: `rand = "0.9.2"` as our direct dep. A transitive `rand = "0.8.5"` entry remains, pulled by `sqlx-postgres 0.8.6` — not controlled by us and expected; will drop when sqlx upgrades its own rand.

```
cargo tree -i "rand@0.8.5" output:
rand v0.8.5
├── sqlx-postgres v0.8.6
│   └── sqlx v0.8.6
│       └── cronduit v1.1.0
└── sqlx-postgres v0.8.6
    └── sqlx-macros-core v0.8.6
        └── sqlx-macros v0.8.6 (proc-macro)
            └── sqlx v0.8.6 (*)
```

Our direct 0.9.2 dep is independent of the transitive 0.8.5; call sites all resolve against 0.9.2.

## Verification

| Gate | Command | Result |
|------|---------|--------|
| Build | `cargo build -p cronduit` | OK (Finished dev profile in 50.06s) |
| CSRF tests | `cargo test -p cronduit --lib csrf` | 7 passed; 0 failed |
| @random tests | `cargo test -p cronduit --lib random` | 15 passed; 0 failed |
| Clippy | `cargo clippy -p cronduit --all-targets -- -D warnings` | Finished with no warnings |
| Old API grep | `grep -rn 'thread_rng\|fill_bytes\|gen_range\|RngCore' src/` | Zero matches |
| rustls-only | `cargo tree -i openssl-sys` | Empty (no match — constraint satisfied) |
| Lockfile rand 0.9 | grep rand in Cargo.lock | `rand v0.9.2` present as direct |

All acceptance criteria from the plan satisfied:
- `grep -c 'rand = "0.9"' Cargo.toml` = 1
- `grep -c 'rand = "0.8"' Cargo.toml` = 0
- `grep -rn 'rand::thread_rng\|fill_bytes\|\.gen_range(' src/` = 0 matches
- `grep -c 'rand::rng()' src/web/csrf.rs` = 1
- `grep -c 'rand::rng()' src/scheduler/sync.rs` = 1
- `grep -c 'rand::rng()' src/scheduler/reload.rs` = 1
- `grep -c 'random_range' src/scheduler/random.rs` = 1
- `grep -c 'use rand::Rng' src/web/csrf.rs` = 1
- `grep -c 'use rand::RngCore' src/web/csrf.rs` = 0

## Deviations from Plan

None — plan executed exactly as written. Seven edits across five files, one atomic commit.

## Threat Model Review

All three threats from the plan's threat register are mitigated as designed:

- **T-10-02-01 (CSRF token predictability):** `rand::rng()` in 0.9 is the same thread-local ChaCha-based CSPRNG as `thread_rng()` in 0.8 (verified per plan via Context7). Token size unchanged (32 bytes -> 64 hex chars).
- **T-10-02-02 (rand transitive deps):** `cargo tree -i openssl-sys` still empty; rustls-only constraint preserved.
- **T-10-02-03 (migration missing a call site):** compiler + grep gates both pass; zero old-API tokens remain in `src/`.

No new threat surface introduced.

## Commits

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Bump rand + migrate call sites | d28a5db | Cargo.toml, Cargo.lock, src/web/csrf.rs, src/scheduler/sync.rs, src/scheduler/reload.rs, src/scheduler/random.rs |

## Self-Check: PASSED

- File `Cargo.toml` — FOUND
- File `src/web/csrf.rs` — FOUND (grep verifies `rand::rng().fill` + `use rand::Rng;`)
- File `src/scheduler/sync.rs` — FOUND (grep verifies `rand::rng()`)
- File `src/scheduler/reload.rs` — FOUND (grep verifies `rand::rng()`)
- File `src/scheduler/random.rs` — FOUND (grep verifies `random_range`)
- Commit d28a5db — FOUND in `git log`
- Build clean, clippy clean, tests green, lockfile shows rand 0.9.2
