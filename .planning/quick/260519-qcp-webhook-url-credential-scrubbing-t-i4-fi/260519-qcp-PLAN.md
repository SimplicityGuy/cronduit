---
phase: quick-260519-qcp
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - src/db/mod.rs
  - src/webhooks/dispatcher.rs
  - src/webhooks/retry.rs
  - examples/cronduit.toml
autonomous: true
requirements:
  - T-I4
must_haves:
  truths:
    - "A webhook URL containing userinfo (https://user:pass@host/path) never reaches webhook_deliveries.url with credentials intact"
    - "A reqwest/network error string that echoes the webhook URL is scrubbed before it lands in webhook_deliveries.last_error"
    - "Every tracing span/event in the webhook path that emits the webhook URL emits the scrubbed form, not the raw userinfo-bearing URL"
    - "strip_url_credentials returns userinfo-free and non-URL inputs unchanged (no false mangling)"
    - "examples/cronduit.toml header comments accurately describe all 8 active jobs"
  artifacts:
    - path: "src/db/mod.rs"
      provides: "pub fn strip_url_credentials + unit tests, mirroring strip_db_credentials"
      contains: "pub fn strip_url_credentials"
    - path: "src/webhooks/retry.rs"
      provides: "scrubbed url + scrubbed last_error written into WebhookDlqRow"
      contains: "strip_url_credentials"
    - path: "src/webhooks/dispatcher.rs"
      provides: "scrubbed url in tracing spans + scrubbed reqwest error in Network variant"
      contains: "strip_url_credentials"
    - path: "examples/cronduit.toml"
      provides: "header comment block listing all 8 active jobs"
      contains: "wh-example-unsigned"
  key_links:
    - from: "src/webhooks/retry.rs::write_dlq"
      to: "src/db::strip_url_credentials"
      via: "scrub cfg.url before building WebhookDlqRow.url"
      pattern: "strip_url_credentials"
    - from: "src/webhooks/dispatcher.rs"
      to: "src/db::strip_url_credentials"
      via: "scrub cfg.url in tracing spans + scrub reqwest error before Network()"
      pattern: "strip_url_credentials"
---

<objective>
Close THREAT_MODEL T-I4 (Information Disclosure): webhook URLs may embed credentials in the `userinfo` component (`https://user:pass@host/path`), and today those credentials can leak into `webhook_deliveries.url`, `webhook_deliveries.last_error`, and tracing spans. Add a `strip_url_credentials` helper mirroring the existing `strip_db_credentials`, then apply it at EVERY enumerated sink. Also fix a docs drift in `examples/cronduit.toml` (header says 6 jobs, file has 8).

Purpose: Remove the last unmitigated Information-Disclosure gap in the v1.2 STRIDE register before the v1.2.1 patch, and keep the shipped quickstart config self-consistent.

Output: A shared `strip_url_credentials` helper with unit tests, scrubbing applied at all webhook URL sinks (DB persist + tracing + reqwest error), and a corrected `examples/cronduit.toml` header comment.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@CLAUDE.md
@THREAT_MODEL.md

<interfaces>
<!-- Existing helper to mirror EXACTLY (style, doc-comment shape, placement, test conventions). -->
<!-- Extracted from src/db/mod.rs:350-361. `use url::Url;` is ALREADY imported at src/db/mod.rs:24 — no new import needed. -->

From src/db/mod.rs (the helper to mirror):
```rust
/// Strip username + password from a URL-style database connection string.
/// Robust against credentials containing `@` / `?` / `/` chars where a regex
/// would misparse. Falls back to "<unparseable>" on parse error.
pub fn strip_db_credentials(database_url: &str) -> String {
    Url::parse(database_url)
        .map(|mut u| {
            let _ = u.set_password(None);
            let _ = u.set_username("");
            u.to_string()
        })
        .unwrap_or_else(|_| "<unparseable>".into())
}
```

Placement decision (document in SUMMARY): `strip_url_credentials` lives ALONGSIDE `strip_db_credentials` in `src/db/mod.rs`. Justification — (1) `url::Url` is already imported there; (2) it is the project's single existing precedent for credential-stripping; (3) it is already a `pub fn` reachable from `src/webhooks/` as `crate::db::strip_url_credentials`. A separate util module would add a file and an import for zero benefit. NOTE on naming: `strip_db_credentials` falls back to `"<unparseable>"` on parse error because a DB connection string is always expected to be a URL. A webhook URL field, by contrast, must pass through non-URL / userinfo-free inputs UNCHANGED (the task spec is explicit: "non-URL or userinfo-free inputs returned unchanged or safely passed through") — so `strip_url_credentials` must NOT reuse the `"<unparseable>"` fallback; on parse failure it returns the input owned-string verbatim.

From src/webhooks/dispatcher.rs — the three tracing sinks + the reqwest-error sink (file:line):
- dispatcher.rs:289 — `url = %cfg.url` in the "webhook delivered" debug! span (success path)
- dispatcher.rs:310 — `url = %cfg.url` in the "webhook non-2xx" warn! span
- dispatcher.rs:340 — `url = %cfg.url, kind, error = %e` in the "webhook network error" warn! span (BOTH the url field AND `error = %e` are sinks — the reqwest error Display can echo the full request URL incl. userinfo)
- dispatcher.rs:346 — `Err(WebhookError::Network(format!("{e}")))` — the reqwest error string flows into WebhookError::Network, which retry.rs turns into webhook_deliveries.last_error (persisted sink)

From src/webhooks/retry.rs — the persist sink (file:line):
- retry.rs:267-278 — `let url = ... cfg.url.clone()` in `write_dlq`; this populates `WebhookDlqRow.url` (column write to webhook_deliveries.url)
- retry.rs:280-290 — `WebhookDlqRow { url, ... last_error, ... }` constructed and handed to `queries::insert_webhook_dlq_row` (the SQL INSERT at src/db/queries.rs:1775)
- `last_error` reaching write_dlq originates from WebhookError::Network/Timeout/etc. (assigned at retry.rs:382-408). The Network variant is the credential-bearing one — scrubbing at the dispatcher boundary (Task 2) is the primary defense; the helper is idempotent so a second scrub here is harmless defense-in-depth if desired.

CONFIRMED NON-SINKS (do NOT add scrubbing; documented for completeness):
- NO web handler renders webhook url/last_error — `grep -rni "webhook" src/web/` returns EMPTY. There is no dashboard/DLQ view-model surfacing webhook_deliveries in v1.2.
- NO askama template renders webhook url/last_error — `grep -rni "webhook|deliveries|dlq|last_error" templates/` returns EMPTY.
- Prometheus metrics use `job` label only (no url) — already safe (T-20-05).
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Add strip_url_credentials helper + unit tests in src/db/mod.rs</name>
  <files>src/db/mod.rs</files>
  <behavior>
    - userinfo present: strip_url_credentials("https://user:pass@host/path?q=1") returns a string that does NOT contain "user" or "pass", DOES contain "host", "/path", and "q=1" (scheme/host/path/query intact)
    - password-only userinfo: "https://:secret@host/x" → output excludes "secret", retains "host" and "/x"
    - username-only userinfo: "https://user@host/x" → output excludes "user@" (no userinfo segment), retains "host"
    - no userinfo (passthrough): "https://host/path?q=1" → returned semantically unchanged (host/path/query preserved)
    - non-URL input (passthrough): "not a url" → returned unchanged verbatim (NOT "<unparseable>" — this is the deliberate difference from strip_db_credentials)
    - empty string: "" → returned unchanged
  </behavior>
  <action>Add a `pub fn strip_url_credentials(url: &str) -> String` directly below `strip_db_credentials` in src/db/mod.rs (around line 361, before the `#[cfg(test)] mod tests`). Mirror strip_db_credentials' doc-comment style and the `Url::parse(...).map(|mut u| { set_password(None); set_username(""); u.to_string() })` body. CRITICAL DIFFERENCE from strip_db_credentials: on parse failure, return the input UNCHANGED (`.unwrap_or_else(|_| url.to_string())`), NOT "<unparseable>" — a webhook URL field must pass non-URL/userinfo-free inputs through safely (T-I4 spec). Reuse the already-present `use url::Url;` at line 24 — add NO new imports. Write the unit tests in the existing `#[cfg(test)] mod tests` block at the bottom of src/db/mod.rs (same file, mirroring `strip_creds_postgres` / `strip_creds_sqlite_is_unchanged` naming and `assert!(!out.contains(...))` style). Cover every case in the behavior block above. Document the placement justification (helper home = alongside strip_db_credentials) in the eventual SUMMARY.</action>
  <verify>
    <automated>just test-unit 2>&1 | tail -20</automated>
  </verify>
  <done>`just test-unit` passes; new tests for strip_url_credentials are present and green; helper is `pub fn`, lives in src/db/mod.rs, adds no new imports, and passes non-URL/userinfo-free inputs through unchanged.</done>
</task>

<task type="auto">
  <name>Task 2: Apply strip_url_credentials at all webhook sinks (dispatcher tracing + reqwest error + DLQ persist)</name>
  <files>src/webhooks/dispatcher.rs, src/webhooks/retry.rs</files>
  <action>Apply `crate::db::strip_url_credentials` at every enumerated sink:

  In src/webhooks/dispatcher.rs:
  (a) Line 289 — the "webhook delivered" debug! span: replace `url = %cfg.url` with the scrubbed value (compute `let safe_url = crate::db::strip_url_credentials(&cfg.url);` once near the top of the relevant match arm/scope and emit `url = %safe_url`). Bind once and reuse if a single scope covers multiple emits; otherwise scrub per emit.
  (b) Line 310 — the "webhook non-2xx" warn! span: emit the scrubbed url.
  (c) Line 340 — the "webhook network error" warn! span: emit the scrubbed url for the `url` field. Additionally, the `error = %e` field AND the `Network(format!("{e}"))` construction at line 346 echo the reqwest error whose Display can contain the full request URL incl. userinfo. Scrub the rendered error string before it is logged AND before it is wrapped in `WebhookError::Network`: build `let err_str = crate::db::strip_url_credentials(&format!("{e}"));` (the helper passes non-URL text through unchanged, so a plain error message is unaffected — only an embedded `user:pass@host` substring inside a URL token is removed). Log `error = %err_str` and construct `Err(WebhookError::Network(err_str))`. Leave the `Timeout` branch (line 343) returning `WebhookError::Timeout` unchanged — it carries no string.

  In src/webhooks/retry.rs:
  (d) Lines 267-278 (`write_dlq`) — scrub the configured URL before it populates `WebhookDlqRow.url`: wrap the `cfg.url.clone()` result with `crate::db::strip_url_credentials(&cfg.url)` (the `Some(cfg)` arm). The `None` arm already stores `String::new()` — leave it. This guarantees webhook_deliveries.url never persists userinfo even if Task-2(c)'s upstream scrub is ever bypassed.
  (e) `last_error` (the `WebhookDlqRow { ... last_error, ... }` field): the credential-bearing source is the Network variant string, already scrubbed at construction in step (c). The helper is idempotent and passthrough-safe, so optionally re-scrub at retry.rs where `last_error = Some(truncate_error(msg))` for the Network arm (retry.rs:392) as defense-in-depth — apply `strip_url_credentials` before `truncate_error`. This is belt-and-suspenders; the primary fix is (c).

  Do NOT alter metrics, signing, payload, or control flow. Pure scrub insertions only. No new external crates (CLAUDE.md: stack locked).</action>
  <verify>
    <automated>just nextest 2>&1 | tail -25</automated>
  </verify>
  <done>`just nextest` passes; `grep -n strip_url_credentials src/webhooks/dispatcher.rs src/webhooks/retry.rs` shows scrubbing at the dispatcher tracing spans (289/310/340), the Network-error construction (346), and the write_dlq url assignment (267-278); no metrics/signing/control-flow changes; no new deps.</done>
</task>

<task type="auto">
  <name>Task 3: Correct examples/cronduit.toml header comments to reflect all 8 active jobs</name>
  <files>examples/cronduit.toml</files>
  <action>Update BOTH header comment blocks that claim "six example jobs" to accurately list all 8 active `[[jobs]]` blocks. This is COMMENT-ONLY — do not touch any `[[jobs]]` config, schedules, commands, webhook blocks, or the commented-out template jobs.

  Block 1 (lines 1-9): change "This file ships six example jobs covering every execution type:" to reflect 8, and extend the numbered list 1-6 to 1-8 by appending:
    7. wh-example-unsigned (command) - every 2 minutes, unsigned-webhook delivery demo (webhook fires on failed/stopped to a loopback receiver; `command = "false"` always fails to exercise the firing path)
    8. fire-skew-demo (docker) - every minute, FCTX-06 fire-skew demo (sleep 30 then echo; seeds `just uat-fire-skew`)

  Block 2 (lines 46-57, the "Quickstart jobs" comment): update its "Six jobs covering every execution type" line and its numbered 1-6 list the same way — append items 7 and 8 with matching descriptions.

  Use the exact job names/order/schedules read from the file (positions 7=`wh-example-unsigned` at schedule `*/2 * * * *`, 8=`fire-skew-demo` at schedule `* * * * *`). Keep the existing comment style/indentation. Diagrams are not introduced (no mermaid needed for a comment list). Do NOT bump any version.</action>
  <verify>
    <automated>just check-config examples/cronduit.toml 2>&1 | tail -10</automated>
  </verify>
  <done>Both header comment blocks list 8 jobs (1-8) with names matching the actual `[[jobs]]` blocks; `wh-example-unsigned` and `fire-skew-demo` appear in both numbered lists; `just check-config examples/cronduit.toml` exits 0 (no job config changed); no version bump.</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| operator config → process | `webhook.url` is operator-supplied and may embed `userinfo` credentials |
| process → database | scrubbed URL/last_error persisted to `webhook_deliveries` (SQLite/Postgres) |
| process → tracing/log sink | scrubbed URL emitted in webhook spans (Docker stdout / log shipper) |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-I4 | Information Disclosure | webhook.url userinfo → webhook_deliveries.url / last_error / tracing spans | mitigate | Add `strip_url_credentials` (src/db/mod.rs) and apply it at the DLQ persist (retry.rs write_dlq), the reqwest-error → Network construction (dispatcher.rs:346), and the three tracing spans (dispatcher.rs:289/310/340). Closes the documented T-I4 gap in THREAT_MODEL.md § Information Disclosure. |
| T-I4-SC | Tampering | new packages | accept | No new crates introduced — helper reuses already-imported `url::Url`; stack remains locked per CLAUDE.md. |
</threat_model>

<verification>
- `just test-unit` green (strip_url_credentials unit tests).
- `just nextest` green (full suite incl. webhook dispatcher/retry tests after scrub insertions).
- `just fmt-check` and `just clippy` clean (CI gates; no warnings).
- `grep -n strip_url_credentials src/db/mod.rs src/webhooks/dispatcher.rs src/webhooks/retry.rs` shows the helper definition + all sink applications.
- `grep -c '^\[\[jobs\]\]' examples/cronduit.toml` returns 8, and both header comment blocks enumerate 8 jobs.
- `just check-config examples/cronduit.toml` exits 0 (config still validates).
</verification>

<success_criteria>
- T-I4 is closed: no code path persists or logs webhook URL `userinfo`. The DLQ `url` column, the `last_error` column (via the Network-error string), and every webhook tracing span emit credential-free URLs.
- `strip_url_credentials` is a `pub fn` in src/db/mod.rs with unit tests covering userinfo-present, password-only, username-only, no-userinfo passthrough, non-URL passthrough, and empty-string; it adds no new imports and reuses the existing `url::Url`.
- `examples/cronduit.toml` header comments accurately describe all 8 active jobs; no job config or version string changed.
- `just fmt-check`, `just clippy`, and `just nextest` all pass.
</success_criteria>

<output>
Create `.planning/quick/260519-qcp-webhook-url-credential-scrubbing-t-i4-fi/260519-qcp-SUMMARY.md` when done. In the SUMMARY, record: (1) the helper-placement justification (alongside strip_db_credentials), (2) the exact file:line of each sink scrubbed, and (3) confirmation that no web/template sink exists in v1.2.
</output>
