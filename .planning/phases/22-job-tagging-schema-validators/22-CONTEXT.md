# Phase 22: Job Tagging Schema + Validators - Context

**Gathered:** 2026-05-04
**Status:** Ready for planning

<domain>
## Phase Boundary

Operators can attach normalized organizational tags to jobs in TOML config; tags
persist to a new JSON column on `jobs`, validate against a strict charset +
reserved-name list at config-load, and reject substring-collisions across the
fleet. Phase 22 also closes the WH-09 webhook-payload `tags` placeholder
shipped by Phase 18 — receivers see real tag values the moment Phase 22 lands.
The change is config-and-data-layer only:

1. **Config schema** — `tags: Vec<String>` on `JobConfig` (TOML
   `tags = ["backup", "weekly"]`); explicitly NOT on `DefaultsConfig`
   (TAG-01 — per-job only; the `[defaults]` + per-job + `use_defaults = false`
   override pattern does NOT apply to tags, by design — would create the
   substring-collision detection problem on every config-load).
2. **Persistence** — new `jobs.tags TEXT NOT NULL DEFAULT '[]'` column
   (JSON-serialized array; structurally parity-friendly across SQLite + Postgres
   without JSONB ops). Single-file additive migration per backend; old jobs
   default to `'[]'` automatically on column add. NO three-file tightening.
3. **Validators (load-time, four new checks)** — TAG-03 normalization
   (lowercase + trim with WARN on dedup-collapse), TAG-04 charset
   (`^[a-z0-9][a-z0-9_-]{0,30}$`) + reserved-name list
   (`cronduit`, `system`, `internal`), TAG-05 substring-collision (fleet-level
   pass), and a per-job count cap (16 tags max — see D-06).
4. **Webhook payload backfill** — `WebhookPayload.tags` field (currently
   `vec![]` per `src/webhooks/payload.rs:88`) is wired to real values via
   `DbRunDetail.tags: Vec<String>` (sourced from the `jobs.tags` column at
   run-detail-fetch time). The placeholder test
   `payload_tags_empty_array_until_p22` is renamed and rewritten to assert real
   tag values round-trip into the payload.
5. **Tests** — schema-parity stays green via `tests/schema_parity.rs` (TEXT-
   family normalization); new integration test `tests/v12_tags_validators.rs`
   covers each rejection path; `tests/v12_tags_payload.rs` (or extends
   `webhooks::payload::tests`) covers the WH-09 backfill round-trip.
6. **Maintainer UAT** — `22-HUMAN-UAT.md` covers the operator-readable
   error UX for each validator, end-to-end TOML→DB persistence spot check, and
   end-to-end webhook delivery confirming real tag values land in the payload.

**Out of scope for Phase 22** (deferred — do not creep):
- Dashboard filter chips + AND semantics + URL state — TAG-06..08, Phase 23.
- Per-tag exposure as Prometheus label — explicitly rejected at requirements
  time (cardinality discipline; TAG explicitly NOT in `cronduit_*` metrics).
- `[defaults].tags` — rejected at requirements time (TAG-01 says per-job only).
- Tag participation in `compute_config_hash` — tags are organizational, not
  execution-input (D-01).
- Tag participation in `serialize_config_json` blob — single source of truth
  is the new `jobs.tags` column (D-02).
- Tag-based webhook routing keys — WH-09 carries tags in the payload but never
  AS a routing key (P18 D-17 lock).
- `release.yml` / `cliff.toml` / `docs/release-rc.md` modifications — no rc
  cut in this phase (rc.3 is Phase 23).

</domain>

<decisions>
## Implementation Decisions

### Tag participation in hashing + snapshot (Gray Area 1)

- **D-01:** **Exclude `tags` from `compute_config_hash`** (`src/config/hash.rs:16`).
  Tags are organizational metadata, not docker-execution input. Mirrors the
  webhook field's exclusion rationale (`src/config/mod.rs:158-161`). Operator
  consequence: a tag-only edit does NOT show as "config changed since last
  success" in the FCTX panel, and webhook receivers do NOT see `config_hash`
  churn from tag-only edits. Real tag values are still delivered via the
  payload's separate `tags: Vec<String>` field (D-04 / WH-09).

  **Rejected:** include in hash (mirrors labels precedent, `hash.rs:47-49`) —
  conflates organization with execution semantics; would make every tag rename
  show up as "config changed" in the FCTX panel which dilutes the signal.

- **D-02:** **Exclude `tags` from `serialize_config_json`** — the canonical
  source of truth for tag values is the new `jobs.tags` column. Storing in
  both `jobs.tags` AND `jobs.config_json` would create a parity invariant
  with no functional benefit (the column is the read site for the dashboard
  filter chips in P23 and the WH-09 backfill in this phase). Symmetric with
  D-01: tags are not a docker-execution surface, so not in the docker-execution
  snapshot.

  **Rejected:** include in `config_json` blob (mirrors labels) — adds an
  invariant `tests/schema_parity.rs` doesn't enforce; one source of truth is
  cheaper to maintain.

### Substring-collision check UX (Gray Area 2)

- **D-03:** **Fleet-level pass after normalization, one `ConfigError` per
  colliding pair.** TAG-05 runs after the per-job loop completes (so all tags
  are already lowercased + trimmed + charset-validated + dedup'd). For each
  colliding pair, emit a single `ConfigError { line: 0, col: 0 }` whose
  message names BOTH tags AND the jobs that use them:

  > `tag 'back' (used by 'cleanup-temp') is a substring of 'backup' (used by 'nightly-backup'); rename or remove one to avoid SQL substring false-positives at filter time.`

  When multiple jobs use the same offending tag, the message lists up to ~3
  representative job names then `(+N more)` to keep it scannable. Single error
  per pair = no spam when many jobs share the same tag.

  **Rejected:** per-job error at every offending site (Phase 17 D-01-style)
  — operator could see 10 errors for the same root-cause `back↔backup` pair;
  spammy and obscures which collision is actually new. **Rejected:** one global
  error listing every pair — loses per-pair granularity once the fleet has
  multiple collisions; harder to scan than one-per-pair.

- **D-04:** **Validator order locked: normalize → reject → dedup → fleet
  check.** Inside the per-job loop:
  1. `tags.iter().map(|t| t.trim().to_lowercase())` — TAG-03 normalization.
  2. Charset regex `^[a-z0-9][a-z0-9_-]{0,30}$` against the **normalized**
     form, plus reserved-name check (`cronduit`, `system`, `internal`).
     Rejection at this step emits per-job-per-violation per Phase 17 D-01.
  3. Dedup with WARN: when normalization causes collapse
     (`["Backup", "backup", "BACKUP"]` → `["backup"]`), emit a config-load
     `tracing::warn!` line per TAG-03 (NOT a ConfigError — WARN flags it so
     operators notice, not blocks).
  4. Per-job count cap of 16 (D-06) checked on the post-dedup list.

  Then, after the per-job loop, the fleet-level substring-collision pass
  (D-03) runs over the union of all post-dedup tag sets.

  **Why this order:** charset on normalized form means `"Backup"` becomes
  `"backup"` (passes) instead of failing for capital letters (which would
  surprise operators given TAG-03's WARN-on-collapse semantics). Reserved
  names checked AFTER normalization so `"Cronduit"` is also rejected. Dedup
  after rejection so we don't WARN about collapsing a value that was about
  to be rejected anyway. Cap on the post-dedup count so the operator's
  intent (16 distinct tags) is what's measured, not raw input length.

### WH-09 webhook payload backfill (Gray Area 3)

- **D-05:** **Include the WH-09 backfill in Phase 22 scope.** Phase 18
  shipped `tags: vec![]` placeholder (`src/webhooks/payload.rs:88`) with the
  test `payload_tags_empty_array_until_p22` (`payload.rs:235`) explicitly
  hinting Phase 22. Phase 22 wires real tag values through to webhook
  receivers — the moment the column lands, receivers see real tags. Closes
  the WH-09 v1.2 commitment end-to-end (TOML → validators → column → webhook
  payload) in a single phase boundary.

  **Rejected:** defer the backfill to a follow-on phase — the column is
  landing here, the `until_p22` test name is a structural breadcrumb, and
  splitting scope leaves WH-09 in placeholder state for an extra release.

- **D-06.5:** **Rename + harden the placeholder test.** Rename
  `payload_tags_empty_array_until_p22` → `payload_tags_carries_real_values`
  and rewrite it to assert a multi-tag fixture round-trips into the payload's
  `tags` field. Locks the cutover with a regression test that won't decay
  back to empty.

- **D-07:** **Read path: `DbRunDetail.tags: Vec<String>`** sourced from the
  `jobs.tags` JSON column at run-detail-fetch time. Symmetric with how
  `image_digest` and `config_hash` already flow through `DbRunDetail`
  (`src/webhooks/payload.rs:16` import + `payload.rs:86-87` field reads).
  Implementation: extend the SELECT in `get_run_by_id` (or equivalent
  fetch site) to project `jobs.tags`, deserialize JSON → `Vec<String>` in
  Rust at the row-mapping layer. `WebhookPayload::build` reads
  `run.tags.clone()` instead of `vec![]`.

  **Rejected:** per-job `Arc<HashMap<i64, Vec<String>>>` cache at the bin
  layer (mirrors WebhookConfig pattern noted in `mod.rs:158-161`) — would
  introduce a second source of truth alongside `jobs.tags`. The column is
  canonical; reading from it per-delivery is cheap at homelab scale (one
  small JSON parse) and removes a sync hazard. Worth revisiting in v1.3 if
  webhook delivery rate ever justifies the cache.

### Per-job tag count cap (Gray Area 4)

- **D-08:** **Hard cap of 16 tags per job, rejected at config-load.** The
  validator (call it `check_tag_count_per_job` or fold into
  `check_tags_validate`) emits one `ConfigError` per job whose post-dedup
  tag count exceeds 16:

  > `[[jobs]] 'nightly-backup': has 19 tags; max is 16. Remove tags or split into multiple jobs.`

  **Rationale:** (a) the Phase 23 chip UI on a single dashboard row stays
  readable — 16 chips is a reasonable upper bound on what fits next to a job
  name without wrapping or scrolling; (b) operators rarely need >16
  organizational dimensions on a single job; (c) the cap can be lifted later
  without a migration if it bites. Mirrors LBL-06's "limits enforced at
  config-load" posture.

  **Rejected:** no cap (charset only) — leaves Phase 23's chip UI to handle
  unbounded counts; operator typos like a forgotten newline turning a comment
  into 30 micro-tags would land in DB before being noticed.
  **Rejected:** soft cap of 16 with WARN — invisible after first startup;
  P23 still has to handle the unbounded case.
  **Rejected:** hard cap of 8 — too tight; operators with `backup,weekly,
  prod,critical,db,postgres,vpn,encrypted` already hit 8 with reasonable
  organization.

### Test + UAT shape (Gray Area 4 cont.)

- **D-09:** **Two new test files + extend webhooks tests:**
  - `tests/v12_tags_validators.rs` — covers each rejection path
    (charset reject, reserved-name reject, substring-collision pair, count cap),
    each WARN path (dedup collapse), and round-trip persistence (TOML
    `tags = ["backup", "weekly"]` → DB `["backup","weekly"]` JSON column →
    fetch returns the same `Vec<String>`). Tests run in the CI matrix
    (`linux/{amd64,arm64} × {SQLite, Postgres}`) per the existing convention.
  - Extend `src/webhooks/payload.rs::tests` with `payload_tags_carries_real_values`
    (replaces `payload_tags_empty_array_until_p22`). Asserts a fixture
    `DbRunDetail { tags: vec!["backup", "weekly"], ... }` produces a payload
    JSON containing `"tags":["backup","weekly"]` (preserves order; lowercase;
    no extra fields).
  - `tests/schema_parity.rs` stays green automatically — TEXT-family
    normalization absorbs the new column without test edits (RESEARCH §E
    pattern carried from P16).

- **D-10:** **`22-HUMAN-UAT.md` autonomous=false** scenarios:
  1. **Persistence spot-check** — write a TOML with 3 tags on a job, start
     cronduit, query the SQLite/Postgres `jobs.tags` column to confirm the
     expected JSON array. Cites `just dev-run` + `just db-shell` (or
     equivalent existing recipe per project memory
     `feedback_uat_use_just_commands.md`).
  2. **Each validator error UX** — write each invalid case (charset reject:
     `tags = ["MyTag!"]`; reserved reject: `tags = ["cronduit"]`; substring
     collision: two jobs with `["back"]` and `["backup"]`; >16 cap), start
     cronduit, eyeball each error message for operator readability per
     project memory `feedback_uat_user_validates.md`.
  3. **Dedup WARN** — write `tags = ["Backup", "backup ", "BACKUP"]`,
     confirm cronduit logs a WARN line that names which inputs collapsed,
     not just the canonical form.
  4. **End-to-end webhook backfill** — configure a webhook on a tagged
     failing job, trigger a fire (existing `just uat-webhook-*` recipe
     pattern from P18/P19/P20), confirm the delivered payload contains
     real tag values in the `tags` field. Closes WH-09 end-to-end.

- **D-11:** **Three new `just` recipes** mirroring the P18/P19/P20 family
  pattern (`uat-webhook-*`, `uat-fctx-*`):
  - `uat-tags-persist` — TOML → DB persistence spot-check (D-10 step 1).
  - `uat-tags-validators` — invalid-input UX walk (D-10 step 2).
  - `uat-tags-webhook` — end-to-end webhook backfill (D-10 step 4).
  Recipes follow the `recipe-calls-recipe` pattern (P18 D-25 precedent —
  each `uat-*` recipe orchestrates seed → run → walk → assert via existing
  recipes like `dev-build`, `dev-run`).

### Universal project constraints (carried forward)

> The decisions below are **[informational]** — repo-wide process constraints
> honored by absence (mermaid-only diagrams, PR-only branch state,
> maintainer-validated UAT, just-recipe UAT). They are not phase-implementation
> tasks.

- **D-12:** [informational] All Phase 22 changes land via PR on a feature
  branch. No direct commits to `main`. (Project memory
  `feedback_no_direct_main_commits.md`.)
- **D-13:** [informational] All diagrams in any Phase 22 artifact (PLAN,
  SUMMARY, README addition, PR description, code comments) are mermaid code
  blocks. No ASCII art. (Project memory `feedback_diagrams_mermaid.md`.)
- **D-14:** [informational] UAT recipes reference existing/new `just` commands
  per D-11; no ad-hoc `cargo` / `docker` / curl-URL invocations. (Project
  memory `feedback_uat_use_just_commands.md`.)
- **D-15:** [informational] Maintainer validates UAT — Claude does NOT mark
  UAT passed from its own runs. (Project memory
  `feedback_uat_user_validates.md`.)
- **D-16:** [informational] Tag and version match — `Cargo.toml` stays at
  `1.2.0`; Phase 22 ships within the rc.2 → rc.3 cycle (no in-source version
  changes). The rc.3 tag (`v1.2.0-rc.3`) is cut in Phase 23, not here.
  (Project memory `feedback_tag_release_version_match.md`.)
- **D-17:** [informational] `cargo tree -i openssl-sys` must remain empty.
  Phase 22 adds zero new external crates — `serde_json` is already a project
  dep; the regex idiom uses `once_cell::sync::Lazy<Regex>` already present at
  `src/config/interpolate.rs:22`. No new TLS/cross-compile surface.

### Claude's Discretion

The planner picks freely on each of the following — none of these were
discussed in the gray-area selection:

- **Plan count and grouping.** A natural split is (1) schema migration +
  serde field; (2) the four validators in `validate.rs`; (3) DB column
  read/write plumbing in `src/db/queries.rs` (including widening the
  `upsert_job` write path for the new column); (4) WH-09 payload backfill
  in `src/webhooks/payload.rs` + `DbRunDetail` extension; (5) integration
  tests + `just` recipes; (6) `examples/cronduit.toml` + README labels-style
  subsection on tags. Planner may collapse plans (e.g., schema+serde+
  upsert into one) or expand. Atomic-commit-per-plan per project convention.
- **Validator function names.** Suggested:
  `check_tag_charset_and_reserved` (TAG-04), `check_tag_substring_collision`
  (TAG-05, fleet-level after the per-job loop), `check_tag_count_per_job`
  (D-08). Planner may rename for symmetry with the existing
  `check_label_*` family.
- **Whether normalization lives in `validate.rs` or a sibling module.**
  TAG-03 normalization is a one-liner (`trim().to_lowercase()`) — fits
  inside the validator. Planner may extract a `normalize_tags(&[String]) ->
  Vec<String>` helper if reuse emerges (the same helper is the obvious
  read-side function for the dedup WARN path).
- **`once_cell::sync::Lazy<Regex>` for the charset check** — pattern is
  free since the dep is in tree (`src/config/interpolate.rs:22`).
- **Migration filename + timestamp prefix.** Phase 22 starts a new sequence
  after `20260503_000009`. Suggested: `20260504_000010_jobs_tags_add.up.sql`.
  Planner picks; the schema-parity test only cares about the per-backend
  pair existing.
- **Whether tags JSON is stored as a sorted-canonical form** (alphabetized
  before `serde_json::to_string`) or insert-order. Recommendation: sorted
  canonical form — makes diffs deterministic, makes the substring-collision
  check trivial, makes the WH-09 payload deterministic. Planner picks; if
  insert-order is chosen, the test fixtures must match.
- **`examples/cronduit.toml`** — add a `tags = [...]` line to one or two
  existing example jobs (e.g., `tags = ["backup", "weekly"]` on the
  hello-world docker job) so operators reading the example see the syntax
  in context. Documenting the validators in a new README subsection is
  optional in this phase; the README pattern from Phase 17 labels is the
  template if the planner picks it up.
- **Whether to ship a tags-vs-labels-vs-name `<details>` table in the
  README.** Phase 17 shipped a labels merge-precedence diagram; tags don't
  have merge semantics so a diagram is unnecessary. A short prose section
  or a "see Phase 23 dashboard chips" forward-reference suffices. Planner
  picks.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level

- `.planning/PROJECT.md` § Current Milestone (v1.2 scope; tagging is one of
  five v1.2 features) and § Constraints (locked tech stack — `sqlx`, TOML,
  rustls invariant).
- `.planning/REQUIREMENTS.md` § Job Tagging / Grouping (TAG) — **TAG-01**
  through **TAG-05** are the canonical requirement IDs Phase 22 satisfies;
  T-V12-TAG-01..TAG-07 are the verification anchors. Section also documents
  why `[defaults].tags` is out of scope (substring-collision detection
  problem on every config-load).
- `.planning/REQUIREMENTS.md` § Webhooks (WH) — **WH-09** is the schema
  promise that Phase 22 closes via D-05/D-07.
- `.planning/ROADMAP.md` § "Phase 22: Job Tagging Schema + Validators" —
  goal, four operator-observable success criteria, depends-on (Phase 15
  hygiene preamble; INDEPENDENT of Phase 16/17/18-21 work). Also Phase 23
  ("Job Tagging Dashboard Filter Chips — rc.3") for the downstream dependent
  scope (TAG-06..08).
- `.planning/STATE.md` — current phase state and v1.2 decisions inherited
  from research/requirements (LOCKED). Section "Tagging:" enumerates the
  pre-locked decisions.

### Phase 17 precedent (closest structural analog)

- `.planning/phases/17-custom-docker-labels-seed-001/17-CONTEXT.md` — the
  template for "config field + validators + reserved namespace at config-
  load". D-01 (per-job-per-violation `ConfigError` shape), D-06..D-10
  (project-rule reaffirmations) — Phase 22 inherits the same shape as
  D-12..D-16 here. Read for the validator-error format precedent.
- `.planning/phases/17-custom-docker-labels-seed-001/17-RESEARCH.md` —
  research depth for the validator family. Phase 22 does NOT need a deep
  research pass (the decisions are already locked); a light research file
  confirming `serde_json` round-trip behavior + the migration shape is
  sufficient.

### Phase 18 precedent (WH-09 placeholder)

- `.planning/phases/18-webhook-payload-state-filter-coalescing/18-03-PLAN.md`
  — D-07 lock for the `tags: []` placeholder; rename target for D-06.5 in
  this phase.
- `.planning/phases/18-webhook-payload-state-filter-coalescing/18-PATTERNS.md`
  L69 / L76 — `tags: Vec<String> = vec![]` per D-07; emit empty array (NOT
  omit). Phase 22 cutover preserves the schema (`tags: Vec<String>` non-
  optional) and just changes the source-of-values.

### Phase 16 precedent (one-file additive migration shape)

- `migrations/sqlite/20260427_000005_image_digest_add.up.sql` and the
  Postgres pair — exact template for the `jobs.tags` migration shape. Phase
  22 mirrors: `ALTER TABLE` + `IF NOT EXISTS` (postgres only; SQLite relies
  on sqlx ledger). Comment header explains the parity invariant + `tests/
  schema_parity.rs` integration.

### Research (already on disk; no new research pass required)

- `.planning/research/STACK.md` — confirms no new crate is required for
  Phase 22. `serde_json` already in tree; `regex` + `once_cell::sync::Lazy`
  already used.
- `.planning/research/PITFALLS.md` — review for tag-collision and
  silent-mutation pitfalls. T-V12-TAG-01..TAG-07 verification anchors.

### Source files the phase touches

- `src/config/mod.rs` `L114-165` (`JobConfig`) — add `tags: Vec<String>`
  field with `#[serde(default)]`. Field placement is planner discretion;
  suggested order is after `cmd` and before `webhook` (groups
  organizational metadata together).
- `src/config/validate.rs` `L88-92` (per-job validator loop) — register
  call sites for the new tag validators. The validator function template
  is the existing `check_label_*` family at `L185-435`. Add the fleet-level
  substring-collision pass AFTER the per-job loop (new structural site —
  the existing `check_duplicate_job_names` at `L612` is the closest analog
  for fleet-level checks).
- `src/config/hash.rs` `L16` (`compute_config_hash`) — **DO NOT add tags**
  per D-01. Add a comment line at the field-list site explaining the
  exclusion (mirrors the `// DO NOT include env` comment at L50).
- `src/db/queries.rs` `L62-130` (`upsert_job`) — extend the INSERT/UPSERT
  to bind a new `tags_json: &str` argument. Both SQLite and Postgres
  branches widen identically. Read sites that need the column (currently
  none on `Job` struct; future read paths in P23 will populate from this
  same column).
- `src/db/queries.rs` (`DbRunDetail` struct, used at `src/webhooks/payload.rs:68`)
  — add `pub tags: Vec<String>` field. Update the row-mapping site to
  deserialize `jobs.tags` JSON → `Vec<String>` (likely a join in
  `get_run_by_id` or equivalent).
- `src/webhooks/payload.rs` `L88` — replace `tags: vec![]` with
  `tags: run.tags.clone()`. `L235` test renamed + rewritten per D-06.5.
- `examples/cronduit.toml` — add `tags = [...]` to one or two demo jobs.

### Migration files (NEW)

- `migrations/sqlite/20260504_000010_jobs_tags_add.up.sql` (suggested
  filename — planner adjusts) — `ALTER TABLE jobs ADD COLUMN tags TEXT NOT
  NULL DEFAULT '[]';`. Header comment explains additive-forever shape +
  pairs-with-postgres invariant.
- `migrations/postgres/20260504_000010_jobs_tags_add.up.sql` (mirror) —
  same shape with `IF NOT EXISTS` guard per Postgres convention.

### NEW test files

- `tests/v12_tags_validators.rs` — integration test covering each rejection
  path + dedup WARN + per-job count cap + round-trip persistence. Runs in
  the existing CI matrix.
- (Extension only) `src/webhooks/payload.rs::tests` —
  `payload_tags_carries_real_values` replaces
  `payload_tags_empty_array_until_p22`.

### Cross-reference for WH-09 closure

- `.planning/REQUIREMENTS.md` TAG-* + WH-09 — Phase 22's SUMMARY should
  cross-reference both, marking WH-09's `tags` field commitment as fully
  realized end-to-end (TOML → validators → column → payload).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **Per-job validator loop** (`src/config/validate.rs:88-92`) — direct
  registration site for the new tag validators. Existing shape: a loop over
  `cfg.jobs` calling `check_one_of_job_type`, `check_cmd_only_on_docker_jobs`,
  `check_network_mode`, `check_schedule`, `check_label_*` (4 funcs),
  `check_webhook_*` (2 funcs). The new tag validators slot in alongside.
- **`check_label_reserved_namespace`** (`src/config/validate.rs:185`) — direct
  template for `check_tag_charset_and_reserved` (TAG-04). Same idiom: detect
  offending entries, accumulate, emit one `ConfigError` per job per violation
  type with all offending values listed.
- **`check_label_size_limits`** (`src/config/validate.rs:298`) — template for
  `check_tag_count_per_job` (D-08). Same idiom: count + threshold + reject.
- **`check_duplicate_job_names`** (`src/config/validate.rs:612`) — closest
  analog for the fleet-level substring-collision pass (D-03). Existing
  fleet-level check; new pass slots in alongside as a sibling top-level call
  in `parse_and_validate`.
- **`once_cell::sync::Lazy<Regex>` idiom** (`src/config/interpolate.rs:22`)
  — pattern for the TAG-04 charset regex. Compile-time const regex; no
  runtime cost beyond the first call.
- **`compute_config_hash`** (`src/config/hash.rs:16`) — read site for D-01.
  Phase 22 does NOT add `tags` to the BTreeMap. Add a `// DO NOT include
  tags` comment at the field-list site mirroring the existing
  `// DO NOT include env` (L50).
- **`upsert_job`** (`src/db/queries.rs:62-130`) — widen to bind a new
  `tags_json: &str` arg on both backends. The shape is identical for both
  branches (SQLite `?N`, Postgres `$N` placeholders); the parity-friendly
  pattern is already in place across the function.
- **`DbRunDetail`** (`src/db/queries.rs`, used at `src/webhooks/payload.rs:68`)
  — extension point for D-07. Add `pub tags: Vec<String>` field; update
  the row-mapping site (likely in `get_run_by_id`) to read + deserialize
  the JSON column.
- **`WebhookPayload::build`** (`src/webhooks/payload.rs:65-91`) — the
  one-line replacement site for D-05 + D-06.5. `tags: vec![]` →
  `tags: run.tags.clone()`.
- **`tests/schema_parity.rs::normalize_type`** — covers the new `TEXT`
  column automatically; no test edit needed (P16 image_digest precedent).

### Established Patterns

- **One-file additive migration with `DEFAULT '[]'`** — mirrors P16's
  `image_digest_add.up.sql` shape exactly. NULLABLE columns require a NULL
  default; tags is `NOT NULL DEFAULT '[]'` because empty-array is a valid
  JSON value and operators expect "no tags" to round-trip cleanly. Old jobs
  get the default automatically on column add.
- **Errors collected, not fail-fast.** `parse_and_validate` accumulates ALL
  errors into `Vec<ConfigError>` before returning. D-03's per-pair fleet
  errors and the per-job validator errors all flow through the same
  accumulator.
- **`Vec<String>` vs `BTreeSet<String>` for the in-memory tag list.**
  `Vec<String>` matches the TOML deserialization shape and serializes to
  `serde_json::to_string` deterministically when sorted (D-09 Claude's
  Discretion). Operators write tags as a list in TOML; round-trip through a
  Vec preserves user-visible authoring without losing dedup semantics
  (which happen at validator time).
- **Migration sequence** — strict timestamp prefix `2026XXXX_NNNNNNN_*`,
  next number is `_010` after `_009_scheduled_for_add`.
- **Test naming** — `tests/v12_<feature>_<scenario>.rs` per the existing
  v1.2 convention. `tests/v12_tags_validators.rs` follows the family.

### Integration Points

- **`JobConfig.tags`** (`src/config/mod.rs:114+`) — field add with
  `#[serde(default)]` so the field is optional in TOML.
- **Four new validator registrations** in the per-job loop at
  `src/config/validate.rs:88-92` per TAG-04 + TAG-05 + D-08 (and TAG-03
  normalization either as part of the rejection validator or as a sibling
  helper).
- **One fleet-level call** for the substring-collision pass after the
  per-job loop in `parse_and_validate`.
- **`upsert_job` widening** (`src/db/queries.rs:62`) — new `tags_json`
  argument bound at both INSERT and ON CONFLICT UPDATE sites.
- **`DbRunDetail.tags` field add** + row-mapping JSON deserialization at
  the read site.
- **Webhook payload one-liner** at `src/webhooks/payload.rs:88`.
- **One-file additive migration per backend** + parity-test green by
  construction.
- **No `webhooks/dispatcher.rs` change.** The dispatcher reads
  `WebhookPayload::build(...)` indirectly; widening `DbRunDetail` is the
  only payload-side change.
- **No `compute_config_hash` change** per D-01.
- **No `serialize_config_json` change** per D-02.

</code_context>

<specifics>
## Specific Ideas

- **The `until_p22` test name is a structural breadcrumb.** Phase 18 left
  `payload_tags_empty_array_until_p22` as a checkpoint. Phase 22 MUST rename
  + rewrite this test (D-06.5). If the planner forgets, the test name stays
  in tree like a stale TODO. Treat the rename as a load-bearing acceptance
  criterion of the phase, not a cosmetic cleanup.
- **Substring-collision check semantics: `s1.contains(s2)` where `s1 != s2`.**
  Plain string `contains`, not regex. Both directions are checked (so
  `back ↔ backup` is one collision, not two). Identical-tag pairs are NOT
  collisions — those would have been caught by per-job dedup or the
  fleet-level uniqueness... wait, tags ARE allowed to repeat across jobs
  (job A and B can both have `backup`). The check is specifically: a tag
  in any job is a non-equal substring of a tag in any (same or other) job.
- **Sorted-canonical JSON form** (planner discretion D-09 sub-bullet) is
  recommended — makes the column value stable across re-uploads of the same
  TOML, makes the substring-collision pass a stable iteration, makes the
  WH-09 payload deterministic. If the planner picks insert-order instead,
  the test fixtures and WH-09 payload tests must match (and the substring
  pass needs sort-then-pair).
- **Reserved-tag list is finite and short** (`cronduit`, `system`, `internal`).
  No future-reserved namespace prefix (unlike LBL-03's `cronduit.*` which
  reserves a whole prefix). Operators can use `cronduit2` or `cronduit-foo`
  freely. The list is a `const RESERVED: &[&str] = &["cronduit", "system",
  "internal"]` constant; expansion is a single-line edit if needed in v1.3.
- **TAG-03 WARN line shape.** When dedup collapses, the WARN should name
  the inputs (not just the canonical form) so operators can tell "you wrote
  three things, I treated them as one":

  > `WARN job 'nightly-backup': tags ["Backup", "backup ", "BACKUP"] collapsed to ["backup"] (case + whitespace normalization)`

- **Phase 22 is independent of Phase 18.** Phase 22 only depends on Phase 15
  (foundation preamble). Phase 18's webhook payload module is in tree, the
  placeholder test exists, but Phase 22 doesn't structurally need anything
  Phase 18 ships beyond the existing `WebhookPayload` struct + test. If
  Phase 18 PRs were still open this work would still slot in.
- **Phase 22 ships within rc.2 → rc.3 cycle.** No rc cut in this phase
  (rc.3 is Phase 23). The PR description should cross-reference rc.3
  readiness target so the milestone close-out audit (Phase 24) has a clear
  input.

</specifics>

<deferred>
## Deferred Ideas

- **Tag-based bulk operations (bulk enable/disable BY TAG)** — explicit v1.3
  candidate per `.planning/REQUIREMENTS.md` § Out of Scope. Not in v1.2.
- **Tags as Prometheus label** — explicit out-of-scope (cardinality
  discipline; same posture as labels and exit codes per EXIT-06).
- **`[defaults].tags`** — explicit out-of-scope (TAG-01; would create the
  substring-collision detection problem on every config-load and is
  semantically inconsistent with the per-job-only goal).
- **Tag-based webhook routing keys** — WH-09 carries tags in payload but
  never AS a routing key. Same posture as label-based routing. Out of v1.2.
- **Per-job `Arc<HashMap<i64, Vec<String>>>` cache** for fast in-memory tag
  lookup in the dispatcher hot path — rejected as alternative read path
  (D-07). Worth revisiting in v1.3 if webhook delivery rate ever justifies
  the cache (homelab scale doesn't).
- **Tag participation in `compute_config_hash`** — rejected (D-01); could
  be added in v1.3 if operators ask for "config-changed-since-last-success"
  to also fire on tag-only edits.
- **Tag participation in `serialize_config_json`** — rejected (D-02); same
  as above.
- **Sorted-canonical vs insert-order** — left to Claude's discretion in
  D-09, recommendation is sorted. If insert-order is chosen, document it
  prominently so operators reading the DB see what's there.
- **Reserved-namespace prefix** (`cronduit.*` style) for tags — rejected as
  premature; the finite reserved list is sufficient and a prefix has no
  current motivating use case.
- **Tag autocompletion in the dashboard** — Phase 23 UI question; not
  Phase 22's surface. Planner should not creep this in.
- **`docs/release-rc.md` modifications** — not in this phase; rc.3 is
  Phase 23 and reuses the runbook verbatim per P20 D-30 / P21 D-22.
- **README configuration subsection on tags** — Claude's discretion (see
  D-09 sub-bullet); if the planner picks it up the shape mirrors the labels
  subsection from Phase 17 D-04. If skipped, Phase 23 can add it once the
  filter UI lands and operators have a complete picture to read.

</deferred>

---

*Phase: 22-job-tagging-schema-validators*
*Context gathered: 2026-05-04*
