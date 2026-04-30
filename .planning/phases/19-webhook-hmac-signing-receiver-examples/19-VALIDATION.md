---
phase: 19
slug: webhook-hmac-signing-receiver-examples
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-29
---

# Phase 19 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo nextest` (Rust) + per-language stdlib runners (Python `unittest` invoked through `python -m`, Go `go test`/CLI verify, Node `node` direct) — no third-party Python/Go/Node test deps per D-02 |
| **Config file** | `Cargo.toml` (nextest), `justfile` (recipe orchestration), `.github/workflows/ci.yml` (matrix) |
| **Quick run command** | `cargo nextest run -p cronduit webhook --no-fail-fast` (filters Phase 19 fixture test) |
| **Full suite command** | `just ci` (existing) + `just uat-webhook-receiver-{python,go,node}-verify-fixture` per language |
| **Estimated runtime** | ~30s for the Rust fixture test; ~5s per per-language fixture-verify recipe; full webhook-interop matrix < 90s on CI |

---

## Sampling Rate

- **After every task commit:** Run `cargo nextest run -p cronduit webhook` (Rust) OR the corresponding `just uat-webhook-receiver-<lang>-verify-fixture` recipe (per-language receiver work)
- **After every plan wave:** Run `cargo nextest run --all-features` AND all 3 fixture-verify recipes
- **Before `/gsd-verify-work`:** Full suite green + per-language CI matrix green + Rust fixture test green + maintainer-validated `19-HUMAN-UAT.md` checkboxes
- **Max feedback latency:** ~30 seconds for the Rust fixture test (locks the wire format); ~5 seconds for any single per-language verify

---

## Per-Task Verification Map

> Filled by the planner (each plan's tasks reference the relevant entry). Skeleton below — the planner expands per-task with concrete commands. The fixture-verify recipes encode 4 outcomes per language (canonical / mutated-secret / mutated-body / drift) so a single recipe invocation samples all four wire-format invariants.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 19-01-XX | 01 | 1 | WH-04 | — | Rust `sign_v1` produces stable signature against locked fixture | unit | `cargo nextest run -p cronduit -- sign_v1_fixture` | ❌ W0 | ⬜ pending |
| 19-02-XX | 02 | 2 | WH-04 | — | Python receiver verifies fixture (4 outcomes) | integration | `just uat-webhook-receiver-python-verify-fixture` | ❌ W0 | ⬜ pending |
| 19-03-XX | 03 | 2 | WH-04 | — | Go receiver verifies fixture (4 outcomes) | integration | `just uat-webhook-receiver-go-verify-fixture` | ❌ W0 | ⬜ pending |
| 19-04-XX | 04 | 2 | WH-04 | — | Node receiver verifies fixture (4 outcomes) | integration | `just uat-webhook-receiver-node-verify-fixture` | ❌ W0 | ⬜ pending |
| 19-05-XX | 05 | 3 | WH-04 | — | docs/WEBHOOKS.md present + back-link present + README pointer present | doc grep | `grep -q "docs/WEBHOOKS.md" docs/CONFIG.md README.md` | ❌ W0 | ⬜ pending |
| 19-06-XX | 06 | 3 | WH-04 | — | webhook-interop matrix passes (Python/Go/Node) | CI | (CI-only, asserted via PR check) | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/fixtures/webhook-v1/secret.txt` — locked test secret (no trailing newline, plaintext, comment header in adjacent README)
- [ ] `tests/fixtures/webhook-v1/webhook-id.txt` — locked ULID (e.g., `01HXYZTESTFIXTURE0000000000`)
- [ ] `tests/fixtures/webhook-v1/webhook-timestamp.txt` — locked Unix-epoch seconds (e.g., `1735689600` = 2026-01-01T00:00:00Z)
- [ ] `tests/fixtures/webhook-v1/payload.json` — full v1 payload schema bytes for the canonical RunFinalized command-archetype event (matches `src/webhooks/payload.rs` output; tags=[], image_digest=null)
- [ ] `tests/fixtures/webhook-v1/expected-signature.txt` — `v1,<base64>` produced by cronduit `sign_v1` against the above
- [ ] `tests/fixtures/webhook-v1/README.md` — explains each file's role + the "no trailing newline in secret.txt" footgun
- [ ] Rust fixture test in `src/webhooks/dispatcher.rs` `#[cfg(test)] mod tests` (sign_v1 is `pub(crate)` per RESEARCH.md finding 1) — must re-derive `expected-signature.txt` AND assert `payload.json` byte-equals the encoded `WebhookPayload`
- [ ] `examples/webhook-receivers/python/`, `examples/webhook-receivers/go/`, `examples/webhook-receivers/node/` — per-language directories with receiver script + README
- [ ] `docs/WEBHOOKS.md` — operator-facing hub doc (10 sections per CONTEXT.md D-06)
- [ ] `19-HUMAN-UAT.md` — maintainer scenarios (per Phase 18 precedent; all checkboxes start `[ ] Maintainer-validated`)
- [ ] CI job `webhook-interop` in `.github/workflows/ci.yml` (matrix Python/Go/Node)
- [ ] 6 new just recipes: `uat-webhook-receiver-{python,go,node}` and `uat-webhook-receiver-{python,go,node}-verify-fixture`

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Receiver examples produce the documented log shape ("verified" line on success) on a real cronduit delivery | WH-04 | Requires running cronduit + receiver + triggering an actual job → end-to-end behavior cannot be asserted by fixture alone | Run `just uat-webhook-receiver-{python,go,node}` per language, observe the "verified" line, flip the `[ ] Maintainer-validated` checkbox in `19-HUMAN-UAT.md` |
| `docs/WEBHOOKS.md` prose accurately describes the wire format and operator workflow | WH-04 | LLM cannot evaluate prose accuracy automatically — content drift from Standard Webhooks v1 spec is operator-visible | Maintainer reviews `docs/WEBHOOKS.md` against the Standard Webhooks v1 spec link; manually flips the `[ ] Maintainer-validated` checkbox in `19-HUMAN-UAT.md` |
| Mermaid diagrams render correctly in GitHub markdown viewer | D-19 | Mermaid syntax errors don't fail any local check — only render time at GitHub | Maintainer opens the PR, visually confirms diagrams render, flips a `[ ] Maintainer-validated` checkbox |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies (the Rust fixture test + 3 fixture-verify recipes cover ALL receiver wire-format outcomes; doc/README tasks are grep-asserted)
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify (the fixture-verify recipes ride on every receiver-touching task)
- [ ] Wave 0 covers all MISSING references (fixture files, receiver dirs, docs/WEBHOOKS.md, just recipes, CI job — all listed above)
- [ ] No watch-mode flags (recipes use single-shot `just <recipe>` invocations)
- [ ] Feedback latency < 30s for fixture test, < 5s per fixture-verify recipe
- [ ] `nyquist_compliant: true` set in frontmatter (planner sets this once Wave 0 fields are resolved into specific tasks)

**Approval:** pending
