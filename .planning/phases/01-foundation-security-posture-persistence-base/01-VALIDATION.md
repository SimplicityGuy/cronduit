---
phase: 1
slug: foundation-security-posture-persistence-base
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-09
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Draft skeleton created by /gsd-plan-phase after research. Planner fills the
> per-task verification map as PLAN.md files are written.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test + cargo-nextest (Rust) |
| **Config file** | `Cargo.toml` + `.cargo/config.toml` (no separate test config) |
| **Quick run command** | `just test` (wraps `cargo test`) |
| **Full suite command** | `just ci` (fmt-check → clippy → openssl-check → nextest → schema-diff → image) |
| **Estimated runtime** | ~{TBD — planner to measure during Wave 0} seconds |

---

## Sampling Rate

- **After every task commit:** Run `just test`
- **After every plan wave:** Run `just nextest` (plus `just schema-diff` if the wave touched migrations)
- **Before `/gsd-verify-work`:** `just ci` must be green locally AND the GitHub Actions CI matrix must be green on the feature branch
- **Max feedback latency:** {TBD — planner to fill based on measured runtime; target < 120 s for `just test`, < 600 s for `just ci`}

---

## Per-Task Verification Map

*Planner fills this table while writing PLAN.md files. Every task referenced in
`*-PLAN.md` frontmatter must have a matching row here with an automated command
OR an explicit Wave 0 dependency.*

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | FOUND-01 | — | Cargo.toml compiles cleanly on stable 1.94.1 edition 2024 | unit | `just build` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Wave 0 must establish the test infrastructure before any implementation task
can be verified. For Phase 1 (greenfield), Wave 0 creates the entire test
surface from scratch:

- [ ] `Cargo.toml` + `rust-toolchain.toml` — so `cargo test` runs at all
- [ ] `justfile` with at minimum `build`, `test`, `nextest`, `fmt-check`, `clippy`, `openssl-check`, `schema-diff`, `ci` recipes
- [ ] `tests/schema_parity.rs` — structural parity harness (D-14)
- [ ] `tests/config_fixtures/` — TOML fixtures for `cronduit check` golden-output tests (valid + invalid cases for D-21)
- [ ] `.github/workflows/ci.yml` — required for success-criterion #4 to be verifiable
- [ ] `migrations/sqlite/` + `migrations/postgres/` — both must exist with the initial migration or `sqlx::migrate!` macros won't compile
- [ ] `.sqlx/` (committed, per open question #2 — recommended YES) so `query!`/`query_as!` work in offline mode on CI without a live DB

*Status: all items MISSING — Phase 1 is greenfield. Wave 0 is the entire
scaffolding effort and must land before any verification command produces
useful signal.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `cronduit` binary runs under Docker on a real host with the socket mounted | OPS-03 | Requires a Docker daemon + host filesystem — not reproducible inside a CI job without Docker-in-Docker complications | 1. `just image` to build the multi-arch image. 2. `docker run --rm -v /var/run/docker.sock:/var/run/docker.sock -v $(pwd)/examples/cronduit.toml:/etc/cronduit/config.toml:ro cronduit:dev cronduit check /etc/cronduit/config.toml` must exit 0. 3. `docker run ... cronduit run --config /etc/cronduit/config.toml` must emit the `cronduit.startup` JSON log line within 5 s and keep running until SIGTERM. |
| Loud non-loopback bind warning is visible and readable to a human operator | FOUND-05 / D-24 | The warning text must be scannable in a terminal, not just present in the log stream | Set `[server].bind = "0.0.0.0:8080"` in a test config. Run `cronduit run --config that.toml` in a terminal. Confirm the WARN line is visible in the scroll-back AND that `bind_warning: true` appears in the `cronduit.startup` JSON event. |
| README SECURITY section is intelligible to a stranger self-hosting for the first time | FOUND-10 / FOUND-11 | Documentation clarity is subjective | Ask a reviewer who has never seen Cronduit to read the README top-to-bottom and answer: "Would I run this on my LAN? Would I put it behind a reverse proxy? Do I understand the Docker-socket risk?" Reviewer comment recorded in PR. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags (no `cargo watch`, no `--watch`, no long-lived test processes in CI)
- [ ] Feedback latency < 120 s for `just test`; < 600 s for `just ci`
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending (planner to flip `nyquist_compliant: true` once the per-task verification map is filled and all Wave 0 items are assigned to plans)
