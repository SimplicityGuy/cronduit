---
phase: 17
slug: custom-docker-labels-seed-001
status: draft
nyquist_compliant: true
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
| **Quick run command** | `just test` (full unit suite — `cargo nextest run --all-features`) — no quick-run-only just recipe exists; lib-only sub-step is `cargo nextest run --lib` (NOT a `just` recipe) |
| **Full suite command** | `just test` (full suite — `cargo nextest run --all-features`) — equivalent: `just nextest` |
| **Estimated runtime** | ~30s lib-only sub-step / ~3-5m full (testcontainers integration tests dominate) |

---

## Sampling Rate

- **After every task commit:** Run `just test` (full unit suite — fast feedback). For a faster lib-only sub-step, the executor MAY inline `cargo nextest run --lib` directly (NOT prefixed with `just` — no such recipe exists; cite this as an inline cargo step in the task notes, not as a `just` recipe per D-08).
- **After every plan wave:** Run `just test` (full suite including testcontainers integration tier behind `--ignored` per project convention; the planner notes that `cargo test --features integration -- --ignored` is the inline sub-step that exercises the `#[ignore]`-gated tests when needed — this is NOT a `just` recipe).
- **Before `/gsd-verify-work`:** `just test` AND `just clippy` AND `just fmt-check` must all be green.
- **Max feedback latency:** ~60s for unit tier, ~5m for integration tier.

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
- [ ] `examples/cronduit.toml` parse-check via `just check-config examples/cronduit.toml` — must succeed after labels added per CONTEXT.md D-03

*Existing infrastructure (cargo test, testcontainers harness from earlier v1.x phases, the just recipe set) is sufficient — no new test framework needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| README labels subsection renders correctly with mermaid merge-precedence diagram | D-04 (CONTEXT.md) | mermaid rendering correctness needs human eyes; CI catches markdown lint but not visual readability | After README edit lands: preview README on GitHub OR run `cat README.md` (no `just docs-preview` recipe exists; this is an inline manual step, NOT a `just` recipe). Confirm the mermaid diagram renders, the merge-semantics table is readable, and the code blocks for the worked example are correct. Document the manual step as `view-on-github` in `17-HUMAN-UAT.md`. |
| `examples/cronduit.toml` end-to-end via `docker-compose up` actually attaches operator labels visible to `docker inspect` on a real machine | LBL-01 (integration) | testcontainers integration covers the bollard layer; the docker-compose path is the operator-facing surface | `just docker-compose-up` (the verified existing recipe); then `docker inspect` the spawned container; confirm the Watchtower/Traefik/backup-tool labels are present alongside `cronduit.run_id` and `cronduit.job_name`. Tear down via inline `docker compose -f examples/docker-compose.yml down` (no `just` recipe for this; inline sub-step only). Document precise steps in `17-HUMAN-UAT.md`. |
| `.planning/seeds/SEED-001-custom-docker-labels.md` frontmatter shows `status: realized` after merge | D-05 (CONTEXT.md) | Process audit — establishes project's first realized-seed pattern | After Phase 17 PR merges to main: maintainer pulls, opens the seed file, confirms `status: realized`, `realized_in: phase-17`, `milestone: v1.2`, `realized_date: <ISO>`. Document the field expectations in `17-HUMAN-UAT.md`. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s for unit tier (full integration tier may exceed)
- [x] `nyquist_compliant: true` set in frontmatter (flipped to true after 17-03 lands the integration tests; see 17-06 acceptance criterion U6 for the maintainer-confirmed flip)

**Approval:** pending

> **Note on `nyquist_compliant: true`:** Set after Plan 17-03 lands `tests/v12_labels_*.rs` and Plans 17-01/17-02 land their unit tests. Maintainer (per 17-HUMAN-UAT.md U3) confirms `just nextest` (or `just test`) exits 0 with the new tests visible — that confirmation is the gate for the `true` value here.
