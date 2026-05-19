---
phase: 24
plan: 07
title: "Phase 24 — Milestone Close-Out — Human UAT (v1.2.0-rc.4)"
autonomous: false
maintainer_validated: true
created: 2026-05-16
# UAT-coverage marker (not requirement ownership): the 38 v1.2 feature REQ-IDs
# whose user-observable behavior this UAT runbook exercises. Phase 24 itself
# owns no REQ-IDs — those were shipped by Phases 15-23 and flipped to [x] by
# plan 24-02. FOUND-14..16 (cargo-deny + CI-hygiene) are NOT user-observable
# and are deliberately excluded from this list.
uat_coverage_requirements: [WH-01, WH-02, WH-03, WH-04, WH-05, WH-06, WH-07, WH-08, WH-09, WH-10, WH-11, LBL-01, LBL-02, LBL-03, LBL-04, LBL-05, LBL-06, FCTX-01, FCTX-02, FCTX-03, FCTX-04, FCTX-05, FCTX-06, FCTX-07, EXIT-01, EXIT-02, EXIT-03, EXIT-04, EXIT-05, EXIT-06, TAG-01, TAG-02, TAG-03, TAG-04, TAG-05, TAG-06, TAG-07, TAG-08]
rc_tag: v1.2.0-rc.4
status: pending
---

# Phase 24 — v1.2 Human UAT — Full Regression + Feature Smoke against `v1.2.0-rc.4`

> Maintainer-validated UAT runbook. Claude authored it; the MAINTAINER runs
> each scenario against the published `:v1.2.0-rc.4` image and ticks the
> sign-off boxes. Per project memory `feedback_uat_user_validates.md`, Claude
> does NOT mark UAT passed from its own runs. Per project memory
> `feedback_uat_use_just_commands.md`, every numbered step below references an
> existing `just` recipe — no ad-hoc `cargo` / `docker` / curl invocations in
> the runbook body. (Recipe internals legitimately wrap concrete commands —
> that is what `just` is for as a make-replacement.)
>
> **Image under test:** `ghcr.io/simplicityguy/cronduit:1.2.0-rc.4`
> (or `:1.2.0-rc.N` if iterated — re-run this runbook against the new rc tag
> and update the `RC tag UAT-validated` row in the Final sign-off block).
> Note: docker image tags drop the `v` prefix (semver-in-image convention;
> matches every prior tag — `1.1.0-rc.6`, `1.0.1`, etc.). The matching git
> tag is `v1.2.0-rc.4`. The `uat-quickstart` recipe accepts either form
> (`v1.2.0-rc.4` OR `1.2.0-rc.4`) and normalizes internally.
>
> **Six scenarios:**
>   1. `docker compose up` quickstart + dashboard renders without regression.
>   2. v1.0 / v1.1 surfaces intact (no regressions under the v1.2 codebase).
>   3. Webhooks end-to-end (Standard-Webhooks-v1 + HMAC-SHA256 + SSRF posture
>      + retry + drain + rustls).
>   4. Custom Docker labels merge + `cronduit.*` reserved-namespace error at
>      config-load.
>   5. FCTX panel + exit-code histogram render correctly (+ a11y observables).
>   6. Job tagging + dashboard filter chips (AND filter + URL state +
>      untagged-hidden + tags in webhook payload).
>
> Iterate from the top. If any scenario fails, fix in a follow-up close-out
> PR, cut rc.5 (or rc.N+1) per `docs/release-rc.md`, and re-run this runbook
> against the new rc image. The final `v1.2.0` always retags the **LAST
> passing-UAT** rc SHA (per plan 24-08 / Phase 14 D-16 "what was tested is
> what ships" discipline).

## Prerequisites

- [ ] Phase 24 close-out PR (plans 24-01..24-05) merged on `main`.
- [ ] `24-RC4-PREFLIGHT.md` sections 1–6 ticked (rc.4 image published to GHCR
      via `release.yml`).
- [ ] `just --list` shows the four plan-24-07 recipes:
      `uat-quickstart`, `uat-regression-v1x`, `uat-labels-merge`,
      `uat-labels-reserved-namespace-error` — plus the 23 pre-existing
      `uat-*` recipes referenced by Scenarios 3, 5, and 6.
- [ ] A web browser with DevTools is available (Chrome / Firefox / Safari).
- [ ] A screen reader is available for Scenario 5 step 8 (macOS VoiceOver via
      `Cmd+F5`; Windows NVDA; Linux Orca).

## Scenario 1 — `docker compose up` quickstart + dashboard renders against rc.4

**Goal:** Verify the `v1.2.0-rc.4` image boots healthy within 90s and the
dashboard renders without regression vs v1.1. Closes ROADMAP Phase 24 success
criterion #5 (docker compose up against quickstart with new v1.2.0 image
observes cronduit container reporting healthy + dashboard renders without
regression).

**Steps:**

1. Run `just uat-quickstart 1.2.0-rc.4` from a fresh terminal. The recipe
   pulls the rc.4 image, brings up `examples/docker-compose.yml` with
   `CRONDUIT_IMAGE` set so the existing image line at L72 resolves to the rc
   tag (no edit to the compose file), waits 90s for the healthcheck, prints
   `docker compose ps`, and pauses for eyeball verification.
2. Open `http://localhost:8080` in a browser.
3. **Eyeball criterion (a):** Dashboard renders without errors (no 500 page;
   no red JS console errors in F12 → Console).
4. **Eyeball criterion (b):** `docker compose ps` output (printed by the
   recipe) shows the cronduit container with `health: healthy` (not
   `starting` or `unhealthy`).
5. Answer `y` at the recipe's `Did the dashboard render and the healthcheck
   go healthy? (y/n)` prompt. The recipe tears down the compose stack.

**Sign-off:**

- [ ] Scenario 1 passed: rc.4 image boots healthy in 90s; dashboard renders
      without regression vs v1.1.

## Scenario 2 — v1.0 + v1.1 surfaces intact (no regressions)

**Goal:** Verify that the v1.2 codebase has not regressed any v1.0 or v1.1
dashboard surface (filter / sort / Run Now / Stop / bulk toggle / timeline /
sparklines / settings overrides / healthcheck). First time the full v1.2
stack gets a single-session smoke test against v1.0/v1.1 surfaces.

**Steps:**

1. Bring cronduit back up. Run `just uat-quickstart 1.2.0-rc.4` from a
   fresh terminal and at the eyeball prompt LEAVE cronduit running (answer
   `n` to the tear-down prompt only after Scenario 6 — OR re-bring-up
   between scenarios per maintainer discretion. Either path is valid.)
2. Run `just uat-regression-v1x`. The recipe prompts the maintainer to
   verify the nine v1.0/v1.1 surfaces one-by-one on the running dashboard.
3. **Eyeball criteria (recipe-prompted):** all nine surfaces work without
   regression — the recipe enumerates the surfaces (a) Filter, (b) Sort,
   (c) Run Now, (d) Stop, (e) Bulk toggle, (f) Timeline, (g) Sparklines,
   (h) Settings overrides, (i) Healthcheck.
4. Answer `y` at the recipe's final prompt.

**Sign-off:**

- [ ] Scenario 2 passed: all nine v1.0/v1.1 surfaces intact under the v1.2
      codebase.

## Scenario 3 — Webhooks end-to-end (Standard-Webhooks-v1 + HMAC + retry + drain)

**Goal:** Verify outbound webhook delivery against the rc.4 binary:
payload conforms to Standard-Webhooks-v1; HMAC-SHA256 signature is present
and verifiable; retry fires on receiver-500 with full-jitter backoff; drain
completes in-flight deliveries on shutdown; rustls is the TLS provider;
HTTPS-required validator rejects plain-HTTP to non-loopback destinations.

**Steps:**

1. Run `just uat-webhook-mock` in terminal A (starts the local mock receiver
   fixture on `127.0.0.1:9999`; logs at `/tmp/cronduit-webhook-mock.log`).
2. In terminal B, run `just uat-webhook-fire <JOB_NAME>` against a job whose
   `webhook = { url = "http://127.0.0.1:9999/...", states = [...] }` config
   targets the mock receiver.
3. Run `just uat-webhook-verify`. The recipe inspects the mock's captured
   payload + headers.
4. **Eyeball criterion (a):** `webhook-signature` (or `Webhook-Signature`)
   header is present and HMAC-SHA256-formatted (base64-encoded digest of the
   ID + timestamp + payload — see `docs/WEBHOOKS.md` for the canonical
   Standard-Webhooks-v1 layout).
5. **Eyeball criterion (b):** payload JSON has `payload_version: "v1"` and
   includes job name + run ID + status + exit code + duration_ms + tags
   (WH-09 — tags-in-webhook-payload).
6. Run `just uat-webhook-mock-500` (mock returns 500 to every POST).
   Then run `just uat-webhook-retry <JOB_NAME>`. **Eyeball criterion (c):**
   3 delivery attempts at t≈0, t≈30s, t≈300s (each with full-jitter
   randomization; the recipe surfaces the precise timestamps from the mock
   receiver log).
7. Run `just uat-webhook-drain` (the recipe arranges an in-flight delivery
   then sends SIGTERM). **Eyeball criterion (d):** the in-flight delivery
   completes within the 30s drain window; logs show
   `webhook_drain_completed` (or equivalent).
8. Run `just uat-webhook-rustls-check`. **Eyeball criterion (e):** rustls
   is the TLS provider (`cargo tree -i openssl-sys` returns empty inside
   the running container; the recipe surfaces the verdict).
9. Run `just uat-webhook-https-required`. **Eyeball criterion (f):** the
   validator rejects `http://` URLs for non-loopback / non-RFC1918
   destinations at config-load.
10. Run `just uat-webhook-metrics-check`. **Eyeball criterion (g):**
    `cronduit_webhook_deliveries_total`, `cronduit_webhook_retries_total`,
    and `cronduit_webhook_drain_total` metrics expose under `/metrics`.

**Sign-off:**

- [ ] Scenario 3 passed: payload + HMAC + retry + drain + rustls +
      HTTPS-required + metrics all behave per spec.

## Scenario 4 — Custom Docker labels (merge precedence + reserved-namespace error)

**Goal:** Verify the v1.2 labels feature: `[defaults].labels` + per-job
`[[jobs]].labels` merge with per-job-wins on collision; `cronduit.*`
reserved-namespace validator surfaces a clear error at config-load.

**Steps:**

1. Run `just uat-labels-merge`. The recipe writes
   `.tmp/uat-labels-merge.toml` with overlapping `[defaults]` + per-job
   labels (defaults: `com.example.env=prod, com.example.owner=platform`;
   per-job: `com.example.owner=data-team, com.example.team=infra`), then
   invokes `just check-config` internally to validate the merge.
2. **Eyeball criterion (a):** Config parses without error (recipe's
   `just check-config` exits 0).
3. **Eyeball criterion (b):** Per-job label `com.example.owner=data-team`
   wins over defaults `platform` (per-job-wins on collision per LBL-03);
   new `com.example.team=infra` merges in. The recipe surfaces the
   effective merged label set via cronduit's startup logs (or by inspection
   of the spawned container labels at runtime).
4. Answer `y` at the recipe's prompt.
5. Run `just uat-labels-reserved-namespace-error`. The recipe writes
   `.tmp/uat-labels-reserved.toml` with a `cronduit.job-name` label
   (intentionally inside the reserved namespace), then invokes
   `just check-config` and asserts it FAILS with a `cronduit.*` error in
   the output (the recipe `grep`s the captured log and exits non-zero if
   the error string is missing).
6. **Eyeball criterion (c):** `just check-config` (invoked inside the
   recipe) FAILS with a clear `cronduit.*` reserved-namespace error
   message. The recipe `cat`s the error log for the maintainer to read.
7. Answer `y` at the recipe's prompt confirming the error UX clearly
   names the reserved namespace.

**Sign-off:**

- [ ] Scenario 4 passed: labels merge + reserved-namespace error work as
      documented.

## Scenario 5 — FCTX panel on run-detail + exit-code histogram card on job-detail

**Goal:** Verify the v1.2 run-detail FCTX panel renders the 5 P1 signals on
a failed or timed-out run; the job-detail exit-code histogram card renders
10 buckets from the last 100 runs with the documented classifier;
accessibility observable criteria (Tab focus order, screen-reader
`aria-expanded` announcement, no keyboard trap, reduced-motion) all hold.

**Steps:**

1. Run `just uat-fctx-panel`. The recipe seeds a 4-failing-run fixture
   (via `just db-reset` + raw sqlite3 inserts inside the recipe body) and
   prompts the maintainer to open the run-detail page in the browser.
2. **Eyeball criterion (a):** FCTX panel is COLLAPSED by default on a
   failed run (per FCTX-02 — `<details>` element with no `open`).
3. **Eyeball criterion (b):** When the maintainer expands the panel, it
   shows the 5 P1 signals: timestamp, image digest, config hash,
   duration-vs-p50 delta, scheduler-fire-skew (per FCTX-03..07).
4. Answer `y` (or press Enter at the recipe's pause) confirming both
   observable criteria.
5. Run `just uat-exit-histogram`. The recipe seeds a fleet with exit-code
   variety (success + failed + stopped + 128..143 signal exits + timeout)
   and prompts the maintainer to open the job-detail page.
6. **Eyeball criterion (c):** Histogram card renders 10 buckets with the
   documented color coding (per EXIT-03 / UI-SPEC § Exit-code distribution).
7. **Eyeball criterion (d):** Top-3 exit codes section shows codes sorted
   by frequency with ties broken by code ASC (per Phase 21 D-08 / EXIT-04).
   Verifies the locked dual-classifier (status='stopped' + exit=137 →
   `BucketStopped`; status='failed' + exit=137 → `Bucket128to143`).
8. Run `just uat-fctx-a11y` and verify the four observable a11y criteria
   below in addition to the recipe's 4-phase walkthrough (mobile viewport,
   light mode, print mode, keyboard-only):
   - **Eyeball criterion (e1) — keyboard focus order:** Press Tab
     repeatedly from the top of the run-detail page. Tab MUST reach the
     FCTX panel expand/collapse summary element (focus ring visible —
     `outline` or `box-shadow` per `--cd-focus-ring` / `--cd-green-dim`
     token). Pressing Enter or Space MUST toggle the panel's expanded
     state.
   - **Eyeball criterion (e2) — screen-reader announcement:** With a
     screen reader active (macOS VoiceOver: `Cmd+F5`; Windows NVDA; Linux
     Orca), when focus reaches the FCTX expand button it MUST announce
     something equivalent to "Failure context, collapsed, button"
     (collapsed state) or "Failure context, expanded, button" (expanded
     state). The `<details>` element's implicit ARIA semantics (or any
     explicit `aria-expanded` attribute on the summary) MUST flip between
     `false` and `true` on toggle — verify by inspecting the element in
     DevTools → Accessibility tree.
   - **Eyeball criterion (e3) — no keyboard trap:** From inside the
     expanded panel, Tab MUST continue past the panel to the next
     focusable element on the page (no trap; Shift-Tab MUST move focus
     back past the panel).
   - **Eyeball criterion (e4) — reduced motion:** With OS-level "Reduce
     motion" enabled (macOS: System Settings → Accessibility → Display →
     Reduce motion; Windows: Settings → Accessibility → Visual effects →
     Animation effects = off), the panel expand/collapse MUST NOT animate
     (instant state change; no slide / fade transition). The site CSS
     honors `prefers-reduced-motion` per the design system tokens.
9. Answer `y` (or press Enter) at all recipe prompts confirming the
   eyeball criteria.

**Sign-off:**

- [ ] Scenario 5 passed: FCTX panel renders 5 P1 signals collapsed-by-default;
      exit-code histogram renders 10 buckets with status-discriminator-wins
      classifier + top-3 tie-break; a11y observables (e1-e4) all hold.

## Scenario 6 — Job tagging + dashboard filter chips (AND filter + URL state + untagged-hidden)

**Goal:** Verify tag persistence end-to-end (TAG-01..05); tag filter chips
render on the dashboard with alphabetical order + empty-state hiding
(TAG-06); AND filter semantics intersect (TAG-06); URL state via repeated
`?tag=` survives bookmark/share (TAG-07); untagged jobs hide when filter
active (TAG-07); tags carry into webhook payloads (WH-09 / TAG-08).

**Steps:**

1. Run `just uat-tags-persist`. The recipe validates tag persistence
   end-to-end through config-load → sorted-canonical JSON in the
   `jobs.tags` column → restart → tags retained.
2. Answer `y` (or press Enter at the recipe's pause) at the recipe's
   prompt confirming persistence + sorted-canonical shape.
3. Run `just uat-tags-validators`. The recipe walks the maintainer through
   the four validator error cases (charset reject, reserved-name reject,
   empty / whitespace reject, per-job cap reject) via
   `just check-config` invocations against `.tmp/uat-tags-*.toml`
   fixtures.
4. Answer `y` (or press Enter) at the recipe's prompts confirming each
   error message is clear and names the offending input.
5. Run `just uat-chips-render`. The recipe seeds a multi-tag fleet and
   prompts the maintainer to verify the chip strip renders ABOVE the
   name-filter input with exactly THREE chips in alphabetical order
   (`backup` / `prod` / `weekly`); all inactive on first paint.
6. Answer `y` (or press Enter) at the recipe's prompt — both the
   populated-chip-strip state and the empty-state-hidden state.
7. Run `just uat-chips-and-filter`. The recipe walks the AND filter
   semantics: (a) click `backup` → 3 rows visible + untagged hidden;
   (b) also click `weekly` → 2 rows visible (AND across two chips);
   (c) type `prod` in the name-filter → 1 row visible (AND with
   name-filter); (d) deactivate `weekly` → 2 rows visible.
8. Answer `y` (or press Enter) at the recipe's prompt confirming AND
   semantics + untagged-hidden + name-filter composition.
9. Run `just uat-chips-share-url`. The recipe walks shareable URL state:
   click two chips → copy URL → paste in fresh tab → both chips active on
   first paint (no flash) + URL canonical alphabetical regardless of
   click order + stale-tag silent-drop on `?tag=ghost`.
10. Answer `y` (or press Enter) at the recipe's prompt confirming the
    four URL-round-trip eyeball criteria.
11. Run `just uat-tags-webhook`. The recipe chains
    `build → db-reset → uat-webhook-mock → uat-webhook-fire →
    uat-webhook-verify` to confirm tags appear in the `tags` field of the
    `run_finalized` webhook payload (closes WH-09 / TAG-08).
12. Answer `y` (or press Enter) at the recipe's prompt confirming the
    `tags: ["backup", "weekly"]` array in the captured payload.

**Sign-off:**

- [ ] Scenario 6 passed: tag persistence + validators + filter chips + AND
      filter + URL state + untagged-hidden + tags-in-webhook-payload all
      behave per spec.

## If UAT fails on any scenario

1. Document the failing scenario(s) under a `## Findings` H2 at the bottom
   of this file (add the section if not yet present). Capture: which
   scenario, which eyeball criterion, observed-vs-expected, screenshot or
   log excerpt.
2. Open a follow-up close-out PR with the fix on a feature branch (per
   project memory `feedback_no_direct_main_commits.md`).
3. Cut the next rc per `docs/release-rc.md` adapted to the new tag (e.g.,
   `rc.4 → rc.5`). The maintainer cuts the tag locally (Phase 12 D-13
   trust anchor).
4. Re-run THIS runbook against the new rc image. Update the `RC tag
   UAT-validated` row in the Final sign-off block to the new rc tag.
5. Plan 24-08 (`24-FINAL-SHIP-PREFLIGHT.md`) retags the **LAST
   passing-UAT** rc SHA as `v1.2.0` per Phase 14 D-16 ("what was tested
   is what ships" — bit-identical image).

## Final sign-off

When all six scenarios above are checked:

- [ ] **Maintainer:** I have run all six scenarios on a clean working tree
      against `v1.2.0-rc.4` (or `v1.2.0-rc.N` if iterated). Each scenario
      produced the expected operator-observable behavior. The full v1.2 stack
      (webhooks Standard-Webhooks-v1 + HMAC + retry + drain + rustls; custom
      Docker labels merge + reserved-namespace validator; FCTX panel with 5
      P1 signals; exit-code histogram with 10 buckets + top-3 tie-break;
      tag filter chips with AND filter + URL state + untagged-hidden + tags
      in webhook payload) PLUS the v1.0/v1.1 regression surfaces (filter /
      sort / Run Now / Stop / bulk toggle / timeline / sparklines / settings
      overrides / healthcheck) work end-to-end. Phase 24 is UAT-complete and
      ready for the final `v1.2.0` retag (plan 24-08).

Maintainer name: __________________
Date: __________________
RC tag UAT-validated: __________________ (e.g., `v1.2.0-rc.4`)
