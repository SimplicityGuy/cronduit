---
phase: 17
slug: custom-docker-labels-seed-001
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-28
---

# Phase 17 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` + `cargo nextest` (runtime: tokio) |
| **Config file** | `Cargo.toml` (no separate test runner config) |
| **Quick run command** | `just test-quick` (unit tests only — `cargo nextest run --lib`) |
| **Full suite command** | `just test` (full suite — `cargo nextest run --all-features`) |
| **Estimated runtime** | ~30s quick / ~3-5m full (testcontainers integration tests dominate) |

---

## Sampling Rate

- **After every task commit:** Run `just test-quick` (lib unit tests only — fast feedback)
- **After every plan wave:** Run `just test` (full suite including testcontainers integration tier)
- **Before `/gsd-verify-work`:** `just test` AND `just clippy` AND `just fmt-check` must all be green
- **Max feedback latency:** ~60s for unit tier, ~5m for integration tier

---

## Per-Task Verification Map

> Populated by the planner. Each task should map to one or more of the five
> ROADMAP success criteria (rows below) plus the four CONTEXT.md decisions
> (D-01..D-04; D-05 is process-only and verified at PR merge).

| Success Criterion | Requirement | Validation Type | Coverage |
|-------------------|-------------|-----------------|----------|
| **SC-1**: Operator label appears on container alongside `cronduit.run_id` / `cronduit.job_name` | LBL-01 | Integration (testcontainers + `bollard::container::inspect_container`) | `tests/v12_labels_merge.rs` |
| **SC-2**: `use_defaults = false` REPLACES; otherwise per-key merge with per-job-wins on collision | LBL-02 | Unit (mirrors `apply_defaults_use_defaults_false_disables_merge` at `src/config/defaults.rs:316`) + Integration (`tests/v12_labels_use_defaults_false.rs`) | Both |
| **SC-3**: `cronduit.*` rejected at config-LOAD time with offending-key error | LBL-03 | Unit (validator test in `src/config/validate.rs` test module — accept + reject paths) | Unit |
| **SC-4**: `labels` on `command`/`script` job rejected at config-LOAD time | LBL-04 | Unit (validator test mirroring `check_cmd_only_on_docker_jobs` test pattern) | Unit |
| **SC-5**: `${VAR}` interpolated in label values; key never; size > 4 KB / 32 KB rejected at LOAD | LBL-05, LBL-06 | Unit (interpolate test + size validator test) | Unit |

The planner is responsible for populating the full task-level verification map below
once PLAN.md files are generated.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| (TBD by planner) | | | | | | | | | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/v12_labels_merge.rs` — integration test (created by planner; testcontainers — bollard inspect)
- [ ] `tests/v12_labels_use_defaults_false.rs` — integration test (created by planner; testcontainers)
- [ ] `tests/v12_labels_validators.rs` — integration test (or in-file unit tests in `validate.rs`) covering all four LOAD-time rejection paths
- [ ] `examples/cronduit.toml` parse-check via `just check examples/cronduit.toml` — must succeed after labels added per CONTEXT.md D-03

*Existing infrastructure (cargo test, testcontainers harness from earlier v1.x phases, the just recipe set) is sufficient — no new test framework needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| README labels subsection renders correctly with mermaid merge-precedence diagram | D-04 (CONTEXT.md) | mermaid rendering correctness needs human eyes; CI catches markdown lint but not visual readability | After README edit lands: open README on GitHub or via `mdcat README.md`; confirm the mermaid diagram renders, the merge-semantics table is readable, and the code blocks for the worked example are correct. Use the `just docs-preview` recipe if it exists; otherwise document the manual step as `view-on-github` in `17-HUMAN-UAT.md`. |
| `examples/cronduit.toml` end-to-end via `docker-compose up` actually attaches operator labels visible to `docker inspect` on a real machine | LBL-01 (integration) | testcontainers integration covers the bollard layer; the docker-compose path is the operator-facing surface | `just docker-up` (or whatever the existing example-config recipe is named); then `docker inspect` the spawned container; confirm the Watchtower/Traefik/backup-tool labels are present alongside `cronduit.run_id` and `cronduit.job_name`. Document precise steps in `17-HUMAN-UAT.md`. |
| `.planning/seeds/SEED-001-custom-docker-labels.md` frontmatter shows `status: realized` after merge | D-05 (CONTEXT.md) | Process audit — establishes project's first realized-seed pattern | After Phase 17 PR merges to main: maintainer pulls, opens the seed file, confirms `status: realized`, `realized_in: phase-17`, `milestone: v1.2`, `realized_date: <ISO>`. Document the field expectations in `17-HUMAN-UAT.md`. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s for unit tier (full integration tier may exceed)
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
