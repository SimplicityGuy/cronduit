# Phase 17 — Human UAT (SEED-001 Custom Docker Labels)

**Owner:** Maintainer (Robert)
**Validation rule (D-09):** Every checkbox below MUST be ticked by a human running the cited `just` recipe locally. Claude MUST NOT mark these steps complete from automated CI or its own ephemeral runs. The phase is NOT done until every box below is ticked by the maintainer.

**Recipe rule (D-08):** Every step cites an existing `just` recipe — no ad-hoc `cargo` / `docker` / `curl` invocations. If a step description appears to require an ad-hoc command, file it as a project-rule deviation and stop.

---

## UAT Checklist

- [x] **U1 — README labels subsection renders correctly on GitHub.**
  - **Recipe:** None — visual review of `README.md` after PR is open.
  - **Steps:**
    1. After the PR is opened, navigate to the PR's "Files changed" tab.
    2. Find the `README.md` diff and confirm the rendered preview shows the new `### Labels` subsection.
    3. Confirm the mermaid merge-precedence diagram renders as an SVG (not as raw fenced text).
    4. Confirm the 3-row merge-semantics table renders as a real markdown table (header row + separator + 3 data rows).
    5. Confirm the four code blocks (`cronduit.foo` example, `team = "ops"` example, `${DEPLOYMENT_ID}` example, `${TEAM}` example) render as syntax-highlighted TOML.
  - **Pass criteria:** Mermaid diagram renders; table renders; code blocks render. NO ASCII art anywhere (D-07).

- [x] **U2 — examples/cronduit.toml parses + validates clean.**
  - **Recipe:** `just check-config examples/cronduit.toml` (verified existing recipe; bare `just check` does NOT exist — `check-config` takes a `PATH` argument)
  - **Steps:**
    1. From a clean working tree on the `phase-17-custom-docker-labels` branch, run `just check-config examples/cronduit.toml`.
    2. Confirm the recipe exits 0.
    3. Visually scan `examples/cronduit.toml` for the three integration patterns: `[defaults].labels` Watchtower line, `hello-world` Traefik labels, NEW `isolated-batch` job with `use_defaults = false`.
  - **Pass criteria:** Recipe exits 0; all three integration patterns visible in the file.

- [x] **U3 — Full unit + integration test suite passes.**
  - **Recipe:** `just nextest` (faster) OR `just test` (standard)
  - **Steps:**
    1. Run `just nextest` (or `just test` if nextest isn't available locally).
    2. Confirm the recipe exits 0.
    3. Confirm the test count visibly reflects the new tests added in Plans 17-01 (parity, hash-differs, merge tests) and 17-02 (12+ validator tests).
  - **Pass criteria:** Recipe exits 0; new tests visible in output.

- [x] **U4 — Lint + format gates pass.**
  - **Recipe:** `just clippy && just fmt-check` (or `just ci` — the composite recipe).
  - **Steps:**
    1. Run `just clippy`.
    2. Run `just fmt-check`.
    3. Confirm both recipes exit 0.
  - **Pass criteria:** Both recipes exit 0.

- [x] **U5 — End-to-end docker labels spot-check.**
  - **Recipe:** `just docker-compose-up` (start cronduit + a docker job from `examples/cronduit.toml`)
  - **Steps:**
    1. Run `just docker-compose-up` to launch cronduit with the example config.
    2. Wait for the `hello-world` job to fire (`*/5 * * * *` schedule — up to 5 minutes; check the cronduit web UI at `http://localhost:8080` for run status).
    3. After the run starts, identify the spawned container ID via `docker ps --filter label=cronduit.job_name=hello-world` (this is a sub-step verification command, NOT a cited recipe — the cited recipe is `just docker-compose-up`).
    4. Run `docker inspect <container-id> | jq '.[0].Config.Labels'` and confirm:
       - `cronduit.run_id` and `cronduit.job_name` are present (cronduit-internal labels)
       - `com.centurylinklabs.watchtower.enable: "false"` is present (inherited from `[defaults].labels`)
       - `traefik.enable: "true"` is present (per-job label on hello-world)
       - `traefik.http.routers.hello.rule: "Host(`hello.local`)"` is present (per-job label, backticks preserved)
    5. Tear down with the inline sub-step `docker compose -f examples/docker-compose.yml down` (there is no `just docker-compose-down` recipe; the existing `docker-compose-up` recipe has no down-equivalent, so the down command is documented here as a literal `docker compose ... down` sub-step, NOT a `just` recipe — note `docker compose` is the modern hyphenless subcommand).
  - **Pass criteria:** All four label categories visible on the spawned container — confirms the LBL-01 / LBL-02 / SC-1 / SC-2 contract end-to-end.

- [x] **U6 — Reserved namespace rejection error message is operator-friendly.**
  - **Recipe:** `just check-config /tmp/cronduit-bad.toml` (the verified existing recipe takes a PATH argument; the doctored copy lives at `/tmp/cronduit-bad.toml`)
  - **Steps:**
    1. Make a temporary copy of `examples/cronduit.toml` (`cp examples/cronduit.toml /tmp/cronduit-bad.toml`).
    2. Edit the copy: add `labels = { "cronduit.foo" = "bar" }` to the `[[jobs]] hello-world` block (per-job, not [defaults]).
    3. Run `just check-config /tmp/cronduit-bad.toml` against the doctored copy.
    4. Confirm the error mentions: the offending key (`cronduit.foo`), the rule (`reserved namespace`), and the job name (`hello-world`).
    5. The in-tree `examples/cronduit.toml` was NOT modified (the test edits a /tmp copy); no restore needed.
  - **Pass criteria:** Error message contains all three pieces of context; readable without consulting source.

---

## After All Boxes Ticked

- The maintainer comments on the PR with `UAT passed` (or equivalent) once every box above is ticked.
- `gsd-execute-phase` (or the orchestrator) treats the phase as complete only after the human-validation comment lands.
- SEED-001's frontmatter is already at `status: realized` (Task 1 of this plan) — no further seed-state action needed.

**Validated by:** Maintainer (Robert) on 2026-04-29 — all 6 UAT items passed locally per D-09.
