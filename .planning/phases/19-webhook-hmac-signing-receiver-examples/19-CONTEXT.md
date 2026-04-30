# Phase 19: Webhook HMAC Signing + Receiver Examples - Context

**Gathered:** 2026-04-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Operators verify webhook authenticity via HMAC-SHA256 using the Standard Webhooks v1 signing-string convention already implemented in Phase 18 (`src/webhooks/dispatcher.rs::sign_v1`). Phase 19 ships **reference receiver examples in Python, Go, and Node** as runnable mini-servers demonstrating constant-time HMAC compare, plus operator-facing integration documentation that explicitly notes v1.2 is **SHA-256 only** (no algorithm-agility cronduit-side; secret rotation is a receiver-side concern). A locked wire-format fixture is consumed by both cronduit's Rust unit tests AND a new per-language CI matrix so any signing-side or receiver-side drift fails CI.

**In scope (Phase 19):**
- `examples/webhook-receivers/{python,go,node}/` — three runnable mini-servers (stdlib only — no Flask, no gin, no Express); each uses its language's constant-time compare primitive (Python `hmac.compare_digest`, Go `hmac.Equal`, Node `crypto.timingSafeEqual`)
- Each receiver demonstrates: 3-header parse → HMAC verify → 5-minute timestamp-drift check (anti-replay) → retry-aware HTTP response codes (4xx for permanent errors so Phase 20's retry doesn't loop, 5xx for transient errors so it does); idempotency dedup is documented as a comment block, not implemented
- Per-receiver `README.md` (run-and-verify steps for that language)
- New `docs/WEBHOOKS.md` operator hub: wire format (defers to Standard Webhooks v1 spec), header semantics, SHA-256-only note, secret-rotation guidance (receiver-side dual-secret window), links to all 3 receiver examples; back-link from `docs/CONFIG.md` § webhook section
- `tests/fixtures/webhook-v1/` shared interop fixture (secret + webhook-id + webhook-timestamp + payload + expected signature)
- New CI job `webhook-interop` (matrix: python / go / node) verifying each receiver against the fixture; cronduit's `sign_v1` also asserts the fixture in a Rust unit test
- `just` recipes per language: `uat-webhook-receiver-{python,go,node}` (run against real cronduit delivery) AND `uat-webhook-receiver-{python,go,node}-verify-fixture` (CI-gateable fixture check)
- `19-HUMAN-UAT.md` maintainer scenarios for end-to-end run with cronduit
- README link to `docs/WEBHOOKS.md` (extending the existing webhook coverage from Phase 18)

**Out of scope (deferred to other phases):**
- Cronduit-side HMAC signing implementation — DONE in Phase 18 (`sign_v1` + tests at `src/webhooks/dispatcher.rs:138`, `:310`, `:330`, `:360`)
- Wire format / header semantics implementation in cronduit — Phase 18 D-09/D-10/D-11
- `unsigned = true` opt-out — Phase 18 D-05
- 3-attempt retry with full-jitter backoff — Phase 20 (WH-05)
- `webhook_deliveries` dead-letter table — Phase 20 (WH-05)
- HTTPS-required URL validation for non-loopback / non-RFC1918 — Phase 20 (WH-07)
- 30-second drain on shutdown — Phase 20 (WH-10)
- `cronduit_webhook_*` Prometheus metric family — Phase 20 (WH-11)
- THREAT_MODEL.md Threat Model 5 (Webhook Outbound) — Phase 20 (WH-08)
- SSRF allow/block-list filter — explicit accepted-risk per WH-08; deferred to v1.3
- Algorithm-agility (SHA-384 / SHA-512 / Ed25519) — locked OUT for v1.2; revisit only if receiver ecosystem demands it in v1.3+
- Cronduit-side multi-secret window for rotation — locked OUT; rotation is a receiver-side concern (dual-secret verify in receiver, swap-then-cronduit-rotate-then-drop)
- Pluggable signature schemes (e.g., GitHub `x-hub-signature` style) — NOT in scope; Standard Webhooks v1 is the locked wire format
- Working idempotency dedup implementation in receivers — comment-block only (in-memory dedup distracts from the HMAC focus and would need a TTL story; spec note suffices)

</domain>

<decisions>
## Implementation Decisions

### Receiver form factor (Gray Area 1)
- **D-01:** Each receiver is a **runnable mini-server** mirroring the shape of `examples/webhook_mock_server.rs` from Phase 18 — listens on `127.0.0.1:PORT`, parses 3 webhook headers, verifies HMAC with constant-time compare, returns appropriate HTTP status, logs the verdict. Operator runs `python receiver.py` / `go run receiver.go` / `node receiver.js` and validates against a real cronduit delivery without touching their existing framework first.
- **D-02:** **Stdlib only — no third-party dependencies.** Python uses `http.server` + `hmac` + `hashlib`; Go uses `net/http` + `crypto/hmac` + `crypto/sha256`; Node uses the `http` module + `crypto` (no Express, no Flask, no gin). Rationale: the WH-04 success criterion is constant-time HMAC compare, and all three constant-time primitives live in stdlib. Stdlib-only also means operators can run the examples without a `pip install` / `npm install` / `go mod download` step gating their first delivery.
- **D-03:** Per-receiver port: Python `9991`, Go `9992`, Node `9993`. Avoids Phase 18's `webhook_mock_server.rs` port `9999` so an operator can run multiple receivers simultaneously alongside the Rust mock during a fan-out validation.
- **D-04:** Each receiver file targets ~80–120 LOC including comments. The verify logic lives in a clearly-separated, copy-pasteable function (`verify_signature(secret, headers, body) -> bool` or local equivalent) at the top of the file, with a docstring. Operators who want to integrate into Flask/gin/Express can copy that function verbatim.

### Receiver layout & docs home (Gray Area 2)
- **D-05:** Receivers live under `examples/webhook-receivers/{python,go,node}/`. Each language gets its own subdirectory with `receiver.py`/`receiver.go`/`receiver.js` plus a focused `README.md` covering: install (none), run command, expected log output on first delivery, troubleshooting (signature mismatch usually = wrong secret), and a verbatim "v1.2 ships SHA-256 only" note.
- **D-06:** New `docs/WEBHOOKS.md` is the **operator-facing hub doc** — mirrors the established `docs/QUICKSTART.md` / `docs/CONFIG.md` / `docs/SPEC.md` pattern. Sections (target ~250–400 lines):
  1. Overview + Standard Webhooks v1 link (defer wire format to the spec, do NOT paraphrase)
  2. Three required headers and their semantics (`webhook-id`, `webhook-timestamp`, `webhook-signature: v1,<base64>`)
  3. SHA-256-only note (LOCKED for v1.2; algorithm-agility is explicit OUT scope)
  4. Secret-rotation guidance (receiver-side dual-secret verify; cronduit holds one secret per job)
  5. Constant-time compare requirement + the three primitive names
  6. Anti-replay: 5-minute timestamp drift window (matches receiver default)
  7. Idempotency: dedupe by `webhook-id` (production guidance)
  8. Retry-aware response codes (4xx vs 5xx — explains how cronduit's Phase 20 retry will interpret each)
  9. Links to the 3 receiver examples
  10. Pointer to `examples/webhook_mock_server.rs` (Rust loopback mock from Phase 18)
- **D-07:** `docs/CONFIG.md` § webhook section gets a back-link to `docs/WEBHOOKS.md` ("see docs/WEBHOOKS.md for receiver implementation guidance"). No content duplication; CONFIG.md stays focused on TOML field reference.
- **D-08:** Project `README.md` gets a one-line addition under the existing webhook coverage: "Receiver examples: see [`docs/WEBHOOKS.md`](docs/WEBHOOKS.md)." No README sprawl.

### Receiver scope beyond constant-time HMAC verify (Gray Area 3)
- **D-09:** **Verify + 5-minute timestamp drift check + retry-aware response codes** — the comprehensive production-shaped option. Each receiver implements:
  1. Parse 3 headers; reject early (400 — permanent) if any missing or malformed
  2. Reject (400 — permanent) if `|now() - webhook-timestamp| > 5 minutes` (Standard Webhooks v1 anti-replay default)
  3. Compute HMAC-SHA256 over `${webhook-id}.${webhook-timestamp}.${body-bytes}` using stdlib HMAC
  4. Constant-time compare against the base64-decoded value of `webhook-signature` (after stripping `v1,` prefix); reject (401 — permanent) on mismatch
  5. On success: log a one-line summary (`run_id`, `job_name`, `status`), return 200
  6. Catch-all: any unexpected exception returns 503 (transient — Phase 20's retry will redeliver)
- **D-10:** **Idempotency dedup is a comment block, not working code.** Each receiver's verify-success branch carries a clearly-marked comment explaining: "In production, dedupe by `webhook-id` (e.g., short-TTL set or DB unique constraint). Cronduit may redeliver on transient receiver failures (Phase 20)." Working dedup needs a TTL story and state management that distracts from the HMAC focus.
- **D-11:** **5-minute drift window is hard-coded with a clearly-named constant** (`MAX_TIMESTAMP_DRIFT_SECONDS = 300`). Operators who need a different window edit the constant. Configurability is not in Phase 19's scope (and not needed for any documented use case).
- **D-12:** **Retry semantics mapping (the 4xx/5xx contract):**
  | Receiver outcome | HTTP status | Cronduit (Phase 20) interpretation |
  |---|---|---|
  | Missing/malformed headers | 400 | Permanent — drop, no retry |
  | Timestamp drift > 5 min | 400 | Permanent — drop, no retry |
  | HMAC mismatch | 401 | Permanent — drop, no retry |
  | Verify success | 200 | Success — counter increment |
  | Unexpected exception | 503 | Transient — Phase 20 retries (3 attempts t=0/30s/300s) |
  This contract gets a verbatim section in `docs/WEBHOOKS.md` so Phase 20's retry implementation doesn't drift from receiver expectations.

### Interop CI verification (Gray Area 4)
- **D-13:** **Per-language CI matrix verifies each receiver against a shared fixture.** New `tests/fixtures/webhook-v1/` directory holds:
  - `secret.txt` — fixed test secret (plaintext, NOT a real secret; checked into git with a comment header)
  - `webhook-id.txt` — fixed ULID (e.g., `01HXYZTESTFIXTURE0000000000`)
  - `webhook-timestamp.txt` — fixed Unix-epoch seconds (a stable past timestamp)
  - `payload.json` — full v1 payload with all 16 fields populated (matches Phase 18 D-06's locked schema; tags = `[]`, image_digest = `null` for the command-job archetype)
  - `expected-signature.txt` — `v1,<base64>` produced by running cronduit's `sign_v1` against the above; locked once.
- **D-14:** Cronduit's Rust test suite gains a unit test `tests/webhook_signature_fixture.rs` (or extension to existing `dispatcher.rs` tests) that re-derives `expected-signature.txt` from the fixture inputs and asserts equality. This locks cronduit's signing side forever; if anyone changes `sign_v1` in a way that breaks the wire format, Rust CI fails before the per-language matrix even runs.
- **D-15:** New GHA workflow job `webhook-interop` (added to the existing `ci.yml` or as a sibling job in the same workflow file — researcher decides based on `ci.yml` structure):
  ```
  webhook-interop:
    strategy:
      matrix:
        lang: [python, go, node]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - setup-{python,go,node}@v* (per matrix entry)
      - run: just uat-webhook-receiver-${{ matrix.lang }}-verify-fixture
  ```
  Each `verify-fixture` recipe loads the fixture, asks the receiver's verify function to validate the signature, asserts a specific success/failure outcome (success on the canonical fixture; failure on a tamper variant). Exit 0 on pass, exit 1 on fail. CI gate from day one (NOT a soft warn-only gate like cargo-deny in Phase 15 — interop drift is more dangerous than dependency policy drift).
- **D-16:** New `just` recipe family per receiver:
  | Recipe | Purpose | Used by |
  |---|---|---|
  | `uat-webhook-receiver-python` | Run Python receiver against real cronduit delivery | Maintainer UAT |
  | `uat-webhook-receiver-go` | Run Go receiver against real cronduit delivery | Maintainer UAT |
  | `uat-webhook-receiver-node` | Run Node receiver against real cronduit delivery | Maintainer UAT |
  | `uat-webhook-receiver-python-verify-fixture` | Verify Python receiver against checked-in fixture | CI matrix + maintainer UAT |
  | `uat-webhook-receiver-go-verify-fixture` | Verify Go receiver against checked-in fixture | CI matrix + maintainer UAT |
  | `uat-webhook-receiver-node-verify-fixture` | Verify Node receiver against checked-in fixture | CI matrix + maintainer UAT |
- **D-17:** **Tamper variants** are part of the fixture-verify recipes — each verify recipe runs the canonical fixture (must pass), then re-runs with one byte mutated in the secret (must fail), then with one byte mutated in the body (must fail), then with the timestamp shifted past the 5-min drift window (must fail). All four outcomes encoded in the recipe so any drift in any of the four behaviors fails CI. Without tamper variants, an always-true `verify_signature` would pass CI.

### Universal Project Constraints (carried forward)

> The decisions below are **[informational]** — repo-wide process constraints honored by absence (no UI files, mermaid-only diagrams, PR-only branch state, maintainer-validated UAT). They are not phase-implementation tasks and do not need to be cited in any single plan's `must_haves`. Plans that introduce UAT files still gate D-21 / D-22 explicitly because they shape that plan's UAT structure.

- **D-18:** [informational] All changes land via PR on a feature branch. No direct commits to `main`. Working branch: `phase-19-webhook-hmac-receivers`.
- **D-19:** [informational] Diagrams in any Phase 19 artifact (PLAN, SUMMARY, README, code comments, `docs/WEBHOOKS.md`) are mermaid. No ASCII art.
- **D-20:** Tag and version match — Phase 19 does NOT cut an rc; first rc is Phase 20 (`v1.2.0-rc.1`). `Cargo.toml` stays at `1.2.0` (already bumped Phase 15).
- **D-21:** UAT recipes use existing `just` commands (project memory `feedback_uat_use_just_commands.md`). The 6 new `uat-webhook-receiver-*` recipes are the only `just`-callable surface for Phase 19 UAT (plus existing `dev`, `check-config`, `ci`, `openssl-check` from prior phases).
- **D-22:** [informational] Maintainer validates UAT — Claude does NOT mark UAT passed from its own runs (project memory `feedback_uat_user_validates.md`). All `19-HUMAN-UAT.md` checkboxes start `[ ] Maintainer-validated`.
- **D-23:** [informational] No UI surface in Phase 19. Webhook operator visibility on the dashboard is post-v1.2 territory; ROADMAP marks Phase 19 `UI hint: no`.
- **D-24:** [informational] Cronduit-side rustls invariant unchanged — receivers don't link cronduit, but Phase 19 adds zero new Rust crates (the fixture test reuses existing `hmac`, `sha2`, `base64`, `serde_json`). `cargo tree -i openssl-sys` must remain empty.

### Claude's Discretion
- Exact filenames inside each receiver dir (`receiver.py` vs `verify.py` vs `server.py` — Claude's call, follow language convention)
- Exact `just` recipe body shape (fixture-verify is research/planner territory — likely `python receiver.py --verify-fixture path/to/fixture` or similar)
- Whether the fixture-verify mode is invoked via a CLI flag in each receiver script, or via a sibling test harness file (`verify_fixture.py`/`verify_fixture.go`/`verify_fixture.js`) that imports the receiver's verify function — researcher and planner decide based on stdlib idiom per language
- Exact section ordering inside `docs/WEBHOOKS.md` beyond the 10 sections in D-06
- Whether the per-language CI matrix lives as a new top-level job in `ci.yml` or as a separate workflow file — researcher checks `ci.yml` size/shape
- Exact ULID value in `webhook-id.txt` and exact timestamp in `webhook-timestamp.txt` (any stable past values work; document the choice in a fixture README)
- Exact secret string in `secret.txt` — must be obviously-test (e.g., `cronduit-test-fixture-secret-not-real`) with a comment header in the file warning operators against using it
- The Phase 20 retry-respect contract (D-12 table) is the canonical source until Phase 20's CONTEXT supersedes it — Phase 20 inherits this verbatim and may extend (e.g., 429 handling) but MUST NOT contradict the existing 4xx-permanent / 5xx-transient split

</decisions>

<specifics>
## Specific Ideas

- **Receiver shape mirrors `examples/webhook_mock_server.rs`** — same `Connection: close` discipline (forces request-per-connection on cronduit's reqwest dispatcher; Phase 18 plan T-18-35 mitigation), same loopback-only loud comment header, same dual-log pattern (stdout + a `/tmp/*-receiver.log` file the verify recipe can `tail`).
- **Constant-time compare primitives — call them out by name in code AND docs:**
  | Language | Primitive | Where |
  |---|---|---|
  | Python | `hmac.compare_digest(a, b)` | `hmac` stdlib module |
  | Go | `hmac.Equal(macA, macB)` | `crypto/hmac` |
  | Node | `crypto.timingSafeEqual(bufA, bufB)` | `crypto` (built-in) |
  Each receiver's verify function has a `# constant-time compare per WH-04` comment immediately above the call.
- **Anti-replay window** — 5 minutes matches the Standard Webhooks v1 spec's recommended default. Mention this in `docs/WEBHOOKS.md` so the operator knows the value isn't arbitrary.
- **Idempotency comment template (verbatim across all 3 receivers):**
  ```
  # In production: dedupe by webhook-id to handle Phase 20 retries.
  # E.g., short-TTL Set/Map (in-memory) or DB unique constraint on webhook-id.
  # Cronduit may redeliver on transient receiver failures (5xx response → retry t=30s, t=300s).
  # First successful 2xx terminates the retry chain.
  ```
- **Fixture file format — boring is the goal:** plaintext files with a 1-line `# comment` header explaining the file's role. NOT JSON or YAML wrapping the fixture; the receivers each read the files directly with their stdlib file-read primitives. Smaller surface + zero parser-bug exposure.
- **Tamper variants are encoded as recipe-internal mutations**, not separate fixture files. Recipe loads `secret.txt`, runs verify (must pass), mutates the in-memory secret by 1 byte, runs verify (must fail), restores. Avoids fixture-file proliferation and makes the tamper logic visible at the recipe layer.
- **Phase 19 is README's first-class entry for webhook operators** — once `docs/WEBHOOKS.md` lands, the README's existing webhook section gets a "see docs/WEBHOOKS.md for receivers and verification" pointer.

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level locks
- `.planning/PROJECT.md` — core value, locked v1.2 webhook decisions (HMAC SHA-256 only, retry shape, payload schema, reload survival, drain), Tech Stack constraints (rustls everywhere, mermaid diagrams, PR-only workflow, just-recipe UAT)
- `.planning/REQUIREMENTS.md` § Webhooks — `WH-04` is Phase 19's requirement; `WH-03`/`WH-09` (Phase 18 — wire format + payload) are upstream prerequisites already met; `WH-05`/`WH-07`/`WH-08`/`WH-10`/`WH-11` are downstream Phase 20 dependents that this CONTEXT seeds (D-12 retry contract)
- `.planning/STATE.md` § Accumulated Context > Decisions — v1.2 webhook decisions inherited from research/requirements (HMAC SHA-256 only, retry shape, coalescing default, etc.)
- `.planning/ROADMAP.md` § Phase 19 — goal + 3 success criteria
- `./CLAUDE.md` — project conventions, locked tech stack, mermaid-only, PR-only workflow, GSD enforcement

### Standard Webhooks v1 spec (READ DIRECTLY — do NOT paraphrase)
- `https://github.com/standard-webhooks/standard-webhooks/blob/main/spec/standard-webhooks.md` — wire format spec; `webhook-id`/`webhook-timestamp`/`webhook-signature` headers; `v1,<base64>` signature format; HMAC-SHA256 over `id.timestamp.payload`; 5-minute timestamp-drift recommendation. **`docs/WEBHOOKS.md` defers to this spec for wire format; do not duplicate.**

### Phase 18 (the cronduit signing side — already implemented)
- `.planning/phases/18-webhook-payload-state-filter-coalescing/18-CONTEXT.md` — D-09/D-10/D-11 (header semantics, body framing); D-22 (HMAC implementation: `hmac` + `sha2`); D-05 (`unsigned = true` opt-out)
- `.planning/phases/18-webhook-payload-state-filter-coalescing/VERIFICATION.md` — Phase 18 sign-side verification record
- `src/webhooks/dispatcher.rs:138-176` — `sign_v1(secret, webhook_id, webhook_timestamp, body) -> String`; produces `base64::STANDARD`-encoded HMAC-SHA256 over `${id}.${ts}.${body}`. **The contract Phase 19 receivers must inter-operate with.**
- `src/webhooks/dispatcher.rs:253` — `webhook-signature: v1,<base64>` header construction (omitted when `cfg.unsigned`)
- `src/webhooks/dispatcher.rs:310, :330, :360` — Existing sign-side unit tests (`sign_v1_known_fixture`, `signature_uses_standard_base64_alphabet`, `signature_value_is_v1_comma_b64`); Phase 19 fixture test joins this family
- `src/webhooks/payload.rs` — JSON wire-format encoder for the locked v1 payload schema (16 fields including `payload_version: "v1"`); fixture `payload.json` mirrors output of this encoder for the canonical RunFinalized event
- `src/webhooks/dispatcher.rs:120-134` — `should_fire(fire_every, filter_position)` (Phase 18's coalesce decision; not Phase 19 territory but documents the dispatcher's complete decision flow that produces the signed delivery)
- `examples/webhook_mock_server.rs` — Rust loopback mock receiver from Phase 18; the form-factor template for the 3 stdlib receivers (`Connection: close`, header+body loop reader, dual-log)
- `examples/cronduit.toml` — `wh-example-signed`/`wh-example-unsigned`/`wh-example-fire-every-zero` job examples for end-to-end UAT against any receiver
- `.planning/phases/18-webhook-payload-state-filter-coalescing/18-HUMAN-UAT.md` — UAT pattern Phase 19 mirrors (just-recipes only, maintainer flips checkboxes)
- `.planning/phases/18-webhook-payload-state-filter-coalescing/18-06-SUMMARY.md` — `just uat-webhook-mock` / `uat-webhook-fire` / `uat-webhook-verify` / `api-run-now` recipe family Phase 19 extends

### Phase 17 (the LBL precedent for docs + just-recipe patterns)
- `.planning/phases/17-custom-docker-labels-seed-001/17-CONTEXT.md` — D-04 README structure; D-08 just-recipe UAT pattern
- `.planning/phases/17-custom-docker-labels-seed-001/17-HUMAN-UAT.md` — UAT artifact precedent (verify checkbox shape `[ ] Maintainer-validated`)

### Existing Cronduit infra to reuse
- `Cargo.toml` — `hmac`, `sha2`, `base64`, `serde_json` already present (Phase 18 added them); Phase 19 adds zero new Rust crates. `cargo tree -i openssl-sys` must remain empty.
- `justfile:267, :291, :312, :326, :337, :349` — existing `uat-fctx-bugfix-spot-check`, `api-run-now JOB_ID`, `api-job-id JOB_NAME`, `uat-webhook-mock`, `uat-webhook-fire JOB_NAME`, `uat-webhook-verify` recipes; Phase 19 adds 6 new `uat-webhook-receiver-*` recipes
- `.github/workflows/ci.yml` — Phase 19 adds new `webhook-interop` matrix job (Python/Go/Node); researcher determines whether to add as a sibling job in `ci.yml` or as a separate workflow file
- `docs/CONFIG.md` — existing operator config reference; Phase 19 adds a webhook back-link to `docs/WEBHOOKS.md`
- `docs/QUICKSTART.md`, `docs/SPEC.md` — established docs hub pattern that `docs/WEBHOOKS.md` joins
- `README.md` — existing webhook coverage; Phase 19 adds a one-line `docs/WEBHOOKS.md` pointer
- `tests/` — existing integration test layout; new fixture lives at `tests/fixtures/webhook-v1/` (sibling pattern to existing fixtures if any, or a new `fixtures/` subdirectory)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`sign_v1` from Phase 18** (`src/webhooks/dispatcher.rs:138`): produces the canonical HMAC over `${id}.${ts}.${body}`. The Phase 19 fixture is generated by calling this function once on a known input set; the resulting signature becomes `expected-signature.txt`. Receivers verify they re-derive the same signature from the same inputs.
- **Phase 18 `webhook_mock_server.rs`** (`examples/webhook_mock_server.rs`): the Rust loopback mock — pattern template for the 3 stdlib receivers. Reuse the `Connection: close` framing, the loop-based header+body reader, the safety cap on body size, and the dual-log pattern.
- **`hmac` 0.13 + `sha2` + `base64::STANDARD`**: already in `Cargo.toml` and used by `sign_v1`; the Rust fixture-verification test reuses these directly.
- **`SecretString` wrapper**: Phase 18 wraps the secret to prevent Debug/Display leakage; the Rust fixture test does the same when constructing the secret from `secret.txt`.
- **`just` recipe-calls-recipe pattern**: established in Phase 18 (`uat-webhook-fire` body delegates to `just api-run-now`); Phase 19's `uat-webhook-receiver-*` recipes follow the same pattern (e.g., `uat-webhook-receiver-python` body delegates to `just api-run-now wh-example-signed` after starting the Python receiver in the background).
- **Phase 18's `examples/cronduit.toml` webhook variants**: `wh-example-signed`, `wh-example-unsigned`, `wh-example-fire-every-zero` already exist; Phase 19's UAT recipes target these jobs against the Python/Go/Node receivers (each receiver listens on a different port; recipe selects which by overriding the webhook URL via env-var interpolation, OR Phase 19 adds 3 new wh-example-* jobs each pointed at a different receiver port — researcher decides the simpler shape).

### Established Patterns
- **`[defaults]` + per-job override + `use_defaults = false`**: locked LBL/webhook precedent. Phase 19 receivers don't touch config schema; the existing webhook block is sufficient.
- **LOAD-time validators with `ConfigError { line: 0, col: 0 }`**: Phase 17/18 — Phase 19 adds NO config validators (no schema changes).
- **`#[ignore]` integration tests** for Docker/HTTP path tests; non-`#[ignore]` for parse-pipeline tests — the Rust fixture test is non-`#[ignore]` (pure HMAC computation; no network).
- **CI matrix `linux/{amd64,arm64} × {SQLite, Postgres}`**: existing CI gate; Phase 19 adds a new matrix axis (Python/Go/Node) as a separate job — does NOT interact with the existing DB-backend matrix.
- **`use_defaults = false` + explicit `timeout = "5m"` on command-type jobs that demonstrate webhook config** — established in Phase 18 (avoids Watchtower label inheritance from `[defaults].labels` failing the LBL-04 validator). Phase 19's any new `wh-example-receiver-{python,go,node}` example jobs (if added) follow this pattern.
- **Maintainer UAT pattern**: 19-HUMAN-UAT.md scenarios all start `[ ] Maintainer-validated`; the maintainer flips them; Claude never marks UAT passed.

### Integration Points
- **Where the receivers integrate**: zero cronduit code changes for the three receiver implementations themselves — they live entirely under `examples/webhook-receivers/{python,go,node}/`. Cronduit's only Phase 19 code change is the new Rust fixture test and possibly the new `uat-webhook-receiver-*` justfile recipes (which are not Rust code).
- **Where the fixture test integrates**: new file `tests/webhook_signature_fixture.rs` (or extension to the existing `src/webhooks/dispatcher.rs` test module — researcher decides based on whether the test needs `tests/`-level fixture access patterns or can live in-module).
- **Where the CI matrix integrates**: new `webhook-interop` job in `.github/workflows/ci.yml` (or new sibling workflow file). Job sets up Python, Go, Node toolchains via setup-python@v5, setup-go@v5, setup-node@v4 actions; runs `just uat-webhook-receiver-${{ matrix.lang }}-verify-fixture` per matrix entry.
- **Where the docs integrate**: new `docs/WEBHOOKS.md` (sibling to `docs/CONFIG.md`/`docs/QUICKSTART.md`/`docs/SPEC.md`); back-link added to `docs/CONFIG.md` § webhook section; one-line README pointer.
- **Where the `just` recipes integrate**: 6 new `uat-webhook-receiver-*` recipes appended to `justfile` after the existing Phase 18 `uat-webhook-*` family; they follow the same `recipe-calls-recipe` pattern (delegating to `api-run-now`).

</code_context>

<deferred>
## Deferred Ideas

- **Working idempotency dedup in receivers** (in-memory Set with TTL or DB unique constraint): comment-block only in v1.2; if operators report production drift, revisit in v1.3.
- **Configurable timestamp-drift window**: hard-coded at 5 minutes (matches Standard Webhooks v1 default). Configurability adds a flag surface with no documented use case; revisit only if operators ask.
- **Frameworks examples** (Flask, gin, Express): not in scope. Stdlib-only keeps the run-without-install promise. Operators wanting framework-specific examples can copy the verify function into their framework directly (D-04 makes the function copy-pasteable).
- **Webhook UI** (delivery status, last-delivery timestamp, replay/retry button on dashboard): NOT in scope; Phase 21 owns FCTX UI panel; webhooks could light up there if Phase 21's discuss decides so. Roadmap candidate for v1.3.
- **Algorithm-agility for HMAC** (SHA-384 / SHA-512 / Ed25519 negotiation): WH-04 locked at SHA-256-only for v1.2; revisit for v1.3+ if receiver ecosystem demands it.
- **Pluggable signature schemes** (e.g., GitHub-style `x-hub-signature` instead of `webhook-signature`): NOT in scope. Standard Webhooks v1 is the locked wire format.
- **Cronduit-side multi-secret rotation window**: NOT in scope; rotation lives on the receiver side via dual-secret verify (documented in `docs/WEBHOOKS.md` § Secret rotation).
- **More than 3 receiver languages** (Ruby, Rust client-side, Java, .NET): not in scope for v1.2; Python/Go/Node cover the dominant homelab webhook-receiver ecosystem. Roadmap candidate for v1.3+ if community demand surfaces.
- **Per-receiver Docker images**: not in scope; receivers are stdlib scripts that run via `python receiver.py` / `go run receiver.go` / `node receiver.js`. A `docker-compose.webhook-receivers.yml` could be added in v1.3 if operators want multi-receiver fan-out staging without local toolchains.
- **Fixture interop with the Standard Webhooks reference test vectors**: opportunistically — IF the upstream spec ships canonical test vectors, the fixture-verify recipes can also assert against those for cross-implementation interop. NOT a blocker; researcher checks during plan phase.

</deferred>

---

*Phase: 19-webhook-hmac-signing-receiver-examples*
*Context gathered: 2026-04-29*
