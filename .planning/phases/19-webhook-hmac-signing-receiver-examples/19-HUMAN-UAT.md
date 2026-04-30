# Phase 19 Human UAT — Webhook HMAC Signing + Receiver Examples

> **Maintainer-validated only.** Per project memory `feedback_uat_user_validates.md`, Claude does NOT mark these scenarios passed — the maintainer runs each scenario and flips the `[ ]` to `[x]` themselves. Per `feedback_uat_use_just_commands.md`, every step references a `just` recipe — NEVER raw `curl`/`cargo`/`docker`.

## Prerequisites

| Prereq | Recipe | Notes |
|--------|--------|-------|
| Workspace builds clean | `just ci` | Full CI gate: fmt + clippy + openssl-check + nextest + schema-diff + image |
| rustls invariant holds | `just openssl-check` | `cargo tree -i openssl-sys` returns empty across native + arm64-musl + amd64-musl |
| Receiver toolchains available | `python3 --version && go version && node --version` | Plan 19-02/03/04 examples are stdlib only — no install step required, but the runtimes must be present |
| `wh-example-receiver-*` jobs enabled | edit `examples/cronduit.toml` | Scenarios U6-U8 below exercise the `wh-example-receiver-{python,go,node}` jobs, which ship commented-out per D-05 (keeps `docker compose up` smoke clean). Uncomment the 3 blocks BEFORE running `just dev` |
| Example config validates | `WEBHOOK_SECRET=test-secret-shh just check-config examples/cronduit.toml` | Set the env var first |

## Scenarios

### U1 — Workspace builds clean

**What this proves:** Plan 19-01..19-06 changes do NOT regress the existing CI gate.

- **Recipe:** `just ci`
- **Steps:**
  1. From a clean working tree on the `phase-19-webhook-hmac-receivers` branch, run `just ci`.
  2. Confirm the recipe exits 0.
  3. Confirm `just openssl-check` (called inside `just ci`) reports empty `cargo tree -i openssl-sys` for native + arm64-musl + amd64-musl (D-24 — zero new Rust crates).
- **Pass criteria:** `just ci` exits 0; `cargo tree -i openssl-sys` empty across all targets.

[ ] Maintainer-validated

### U2 — Rust fixture lock test green

**What this proves:** Plan 19-01 — `sign_v1` produces a stable signature against the locked fixture; cross-language CI matrix has a fixed wire-format target.

- **Recipe:** `just nextest` (specifically the `webhooks::dispatcher::tests::sign_v1_locks_interop_fixture` test)
- **Steps:**
  1. Run `just nextest`.
  2. Confirm the recipe exits 0.
  3. In the test output, confirm `webhooks::dispatcher::tests::sign_v1_locks_interop_fixture` appears as PASSED.
- **Pass criteria:** `just nextest` exits 0 and the fixture lock test is in the passed list.

[ ] Maintainer-validated

### U3 — Python verify-fixture recipe green (4 tamper variants)

**What this proves:** Plan 19-02 — Python receiver verifies cronduit's wire format and rejects all 3 tamper variants (mutated secret, mutated body, drift).

- **Recipe:** `just uat-webhook-receiver-python-verify-fixture`
- **Steps:**
  1. Run `just uat-webhook-receiver-python-verify-fixture`.
  2. Confirm the recipe exits 0.
  3. Confirm stdout contains `OK: all 4 tamper variants behave correctly`.
- **Pass criteria:** Recipe exits 0; the 4-variant OK line is printed.

[ ] Maintainer-validated

### U4 — Go verify-fixture recipe green (4 tamper variants)

**What this proves:** Plan 19-03 — Go receiver verifies cronduit's wire format and rejects all 3 tamper variants.

- **Recipe:** `just uat-webhook-receiver-go-verify-fixture`
- **Steps:**
  1. Run `just uat-webhook-receiver-go-verify-fixture`.
  2. Confirm the recipe exits 0.
  3. Confirm stdout contains `OK: all 4 tamper variants behave correctly`.
- **Pass criteria:** Recipe exits 0; the 4-variant OK line is printed.

[ ] Maintainer-validated

### U5 — Node verify-fixture recipe green (4 tamper variants)

**What this proves:** Plan 19-04 — Node receiver verifies cronduit's wire format and rejects all 3 tamper variants. Pitfall 2 length guard prevents `RangeError`.

- **Recipe:** `just uat-webhook-receiver-node-verify-fixture`
- **Steps:**
  1. Run `just uat-webhook-receiver-node-verify-fixture`.
  2. Confirm the recipe exits 0.
  3. Confirm stdout contains `OK: all 4 tamper variants behave correctly`.
- **Pass criteria:** Recipe exits 0; the 4-variant OK line is printed; no `RangeError` traceback in any subprocess output.

[ ] Maintainer-validated

### U6 — Python receiver end-to-end against real cronduit

**What this proves:** Plan 19-02 — the Python receiver verifies a live cronduit delivery, returns 200, and logs the `verified` line.

- **Recipe:** `just uat-webhook-receiver-python` (terminal A) + `just dev` (terminal B; with `wh-example-receiver-python` UNCOMMENTED in `examples/cronduit.toml` and `WEBHOOK_SECRET` exported) + `just uat-webhook-fire wh-example-receiver-python` (terminal C)
- **Steps:**
  1. Edit `examples/cronduit.toml`: uncomment the `wh-example-receiver-python` `[[jobs]]` block (lines added by Plan 19-05).
  2. In terminal B: `export WEBHOOK_SECRET=my-test-secret-shh`
  3. In terminal A: `just uat-webhook-receiver-python` (starts receiver on 127.0.0.1:9991)
  4. In terminal B: `just dev` (cronduit runs against examples/cronduit.toml; the `wh-example-receiver-python` job fires every minute and fails)
  5. In terminal C: `just uat-webhook-fire wh-example-receiver-python` (force an immediate run)
  6. In terminal A, watch for the `[python-receiver] verified webhook-id=<ULID> bytes=<N>` line.
- **Pass criteria:** Terminal A logs the `verified` line within ~2 seconds of the fire; terminal B's cronduit log shows a 200 response from `127.0.0.1:9991`.

[ ] Maintainer-validated

### U7 — Go receiver end-to-end against real cronduit

**What this proves:** Plan 19-03 — the Go receiver verifies a live cronduit delivery, returns 200, and logs the `verified` line.

- **Recipe:** `just uat-webhook-receiver-go` (terminal A) + `just dev` (terminal B; with `wh-example-receiver-go` UNCOMMENTED) + `just uat-webhook-fire wh-example-receiver-go` (terminal C)
- **Steps:** Identical to U6, but uncomment `wh-example-receiver-go` and use `just uat-webhook-receiver-go` + `just uat-webhook-fire wh-example-receiver-go`.
- **Pass criteria:** Terminal A logs `[go-receiver] verified webhook-id=<ULID> bytes=<N>`; terminal B's cronduit log shows 200 from 127.0.0.1:9992.

[ ] Maintainer-validated

### U8 — Node receiver end-to-end against real cronduit

**What this proves:** Plan 19-04 — the Node receiver verifies a live cronduit delivery, returns 200, and logs the `verified` line.

- **Recipe:** `just uat-webhook-receiver-node` (terminal A) + `just dev` (terminal B; with `wh-example-receiver-node` UNCOMMENTED) + `just uat-webhook-fire wh-example-receiver-node` (terminal C)
- **Steps:** Identical to U6, but uncomment `wh-example-receiver-node` and use `just uat-webhook-receiver-node` + `just uat-webhook-fire wh-example-receiver-node`.
- **Pass criteria:** Terminal A logs `[node-receiver] verified webhook-id=<ULID> bytes=<N>`; terminal B's cronduit log shows 200 from 127.0.0.1:9993.

[ ] Maintainer-validated

### U9 — `docs/WEBHOOKS.md` renders cleanly on GitHub

**What this proves:** Plan 19-05 — the new operator hub doc has correctly-rendering mermaid diagrams (D-19 — no ASCII art) and well-formed markdown.

- **Recipe:** None — visual review of the rendered Markdown after the PR is open.
- **Steps:**
  1. After the PR is opened, navigate to the PR's "Files changed" tab.
  2. Find the `docs/WEBHOOKS.md` diff and click it; GitHub renders the file.
  3. Confirm: 3 mermaid diagrams render as SVG (NOT raw fenced text). The diagrams are: System Architecture sequenceDiagram, Verify Decision Tree flowchart, Phase 20 Retry stateDiagram.
  4. Confirm: D-12 retry-aware response codes table renders as a real markdown table (header row + separator + 5 data rows).
  5. Confirm: Constant-time compare per-language table renders (3 data rows: Python, Go, Node).
  6. Confirm: SHA-256-only callout is visible and unambiguous.
  7. Confirm: NO ASCII-art diagrams anywhere (no `┌`, `└`, `├`, `┤`, `─` box-drawing characters).
- **Pass criteria:** All 3 mermaid diagrams render; tables render; SHA-256-only note visible; no ASCII art.

[ ] Maintainer-validated

### U10 — README + CONFIG.md cross-references render

**What this proves:** Plan 19-05 — `README.md` one-line pointer and `docs/CONFIG.md` back-link both link cleanly to `docs/WEBHOOKS.md`.

- **Recipe:** None — visual review of the rendered Markdown after the PR is open.
- **Steps:**
  1. On the PR "Files changed" tab, view the `README.md` diff. Confirm a new line "Verifying webhook deliveries? Receiver examples..." with a working link to `docs/WEBHOOKS.md`.
  2. View the `docs/CONFIG.md` diff. Confirm a new section/paragraph linking to `docs/WEBHOOKS.md`.
  3. Click both links and confirm they navigate to `docs/WEBHOOKS.md`.
- **Pass criteria:** Both links resolve; both renderings are clean (no broken markdown).

[ ] Maintainer-validated

### U11 — `webhook-interop` CI matrix passes on the PR

**What this proves:** Plan 19-06 — cross-language CI gate is green; per-language verify-fixture recipes pass on `ubuntu-latest` runners.

- **Recipe:** None — read the GitHub Actions check status on the PR.
- **Steps:**
  1. Open the PR's "Checks" tab.
  2. Confirm `webhook-interop (python)`, `webhook-interop (go)`, `webhook-interop (node)` are all GREEN.
  3. Click into one of them and confirm the final step (`just uat-webhook-receiver-<lang>-verify-fixture`) printed `OK: all 4 tamper variants behave correctly`.
- **Pass criteria:** All 3 matrix cells GREEN.

[ ] Maintainer-validated

## After All Boxes Ticked

- The maintainer comments on the PR with `UAT passed` (or equivalent) once every box above is ticked.
- `gsd-execute-phase` (or the orchestrator) treats the phase as complete only after the human-validation comment lands.
- Post-merge, `.planning/STATE.md` and `.planning/ROADMAP.md` are updated to reflect WH-04 → Validated.

**Validated by:** _[Maintainer flips this line on UAT completion: e.g., `Maintainer (Robert) on YYYY-MM-DD — all 11 UAT items passed locally per D-22.`]_
