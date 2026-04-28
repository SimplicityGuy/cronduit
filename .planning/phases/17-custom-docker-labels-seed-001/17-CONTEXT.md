# Phase 17: Custom Docker Labels (SEED-001) - Context

**Gathered:** 2026-04-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Operators can attach arbitrary Docker labels to cronduit-spawned containers
(`[defaults].labels` and per-`[[jobs]].labels`), with locked merge semantics, a
reserved `cronduit.*` namespace, type-gated validation, env-var interpolation in
values, and load-time enforced size limits. The change is purely additive:

1. **Config schema** — `labels: Option<HashMap<String, String>>` on
   `DefaultsConfig` and `JobConfig` (TOML inline-table or block-table; keys may
   contain dots — `com.centurylinklabs.watchtower.enable`).
2. **Merge** — `apply_defaults` extended so `use_defaults = false` REPLACES
   defaults; otherwise per-key merge with **per-job-wins** on collision.
3. **Validators (load-time, four new checks)** — reserved-namespace
   (`cronduit.*`), type-gate (docker-only), size limits (4 KB / 32 KB), key
   character convention.
4. **Bollard plumb-through** — operator labels merged into the existing
   `cronduit.run_id` / `cronduit.job_name` map at container-create time;
   internal labels remain intact (LBL-03 prevents operator from claiming
   `cronduit.*`).
5. **Docs + examples** — `examples/cronduit.toml` showcases three integration
   patterns; `README.md § Configuration` gains a full labels subsection.
6. **Seed close-out** — `.planning/seeds/SEED-001-custom-docker-labels.md`
   frontmatter promoted from `dormant` to `realized` (establishes the project's
   first realized-seed pattern).

**Out of scope for Phase 17** (deferred to other phases / future milestones — do
not creep): displaying operator labels in the Web UI run-detail or job-detail
pages; non-docker label-equivalents (systemd unit annotations, log tag
emission); label-based metric labels (Prometheus cardinality is intentionally
NOT operator-extensible per OBS posture); label-based webhook routing keys
(WH-09 includes tags in payload but never AS a routing key — same posture for
labels); generalizing to v1.3+ label-equivalent surfaces (deferred per
SEED-001 § Notes).

</domain>

<decisions>
## Implementation Decisions

### Validator error format (Area 1)

- **D-01:** **Match the existing `validate.rs` pattern.** All four new
  load-time validators emit `ConfigError { line: 0, col: 0, ... }` and
  produce **one `ConfigError` per job per violation type**. The error message
  enumerates ALL offending keys for that violation in a single line. No
  `toml::Spanned` plumbing — the existing
  `check_cmd_only_on_docker_jobs` (`src/config/validate.rs:89`) is the literal
  template. The aggregate-not-fail-fast posture is already in place via the
  per-job validator loop at `src/config/validate.rs:88-92` and the
  `errors: &mut Vec<ConfigError>` accumulator in `parse_and_validate`.

  **Example error shape (reserved-namespace):**
  ```
  [[jobs]] `nightly-backup`: labels under reserved namespace `cronduit.*` are
  not allowed: cronduit.foo, cronduit.bar. Remove these keys; the cronduit.*
  prefix is reserved for cronduit-internal labels.
  ```

  **Rejected:** real `line:col` via `toml::Spanned` (would force `Spanned`
  plumbing across `DefaultsConfig` + `JobConfig` and any downstream consumers
  — scope creep against the foundation pattern); one error per offending key
  (breaks the per-job-per-check aggregation idiom of every other validator
  in the file).

### Label key character validation (Area 2)

- **D-02:** **Strict ASCII key validation at LOAD time.** Keys must match
  `^[a-zA-Z0-9_][a-zA-Z0-9._-]*$` — leading char alphanumeric or underscore;
  subsequent chars alphanumeric, `.`, `-`, or `_`. Empty keys, non-ASCII,
  spaces, slashes, and characters outside this set are rejected at config-load
  with a clear cronduit-side error pointing at the offending key. This is a
  **fourth** new validator (in addition to LBL-03 reserved-namespace, LBL-04
  type-gate, LBL-06 size limits) — orthogonal to the other three.

  **Rationale:**
  - Aligns with the seed's "load-time, not runtime" philosophy. Without this
    check, operator typos surface as a confusing `dockerd` rejection at
    container-create time, far away from the cronduit log line.
  - Implicitly enforces LBL-05's "keys are NOT interpolated" intent — the
    pre-parse `interpolate::interpolate` pass at `src/config/interpolate.rs`
    treats keys and values uniformly, but any leftover `${`/`}` characters
    (or any other non-conforming char) are rejected by this validator. See
    "Specific Ideas" for the residual gap.
  - Aligns with Docker's documented label-key convention.

  **Rejected:** length-only ("the size limits already cover it") — leaves
  bollard error UX for the most common typo class; permissive ("only reject
  control chars + empty") — doesn't catch the high-frequency cases (spaces,
  slashes); regex per-key parse — adds compile-time const regex via
  `once_cell::sync::Lazy<Regex>` (already a project dep, see
  `src/config/interpolate.rs:22`), so this is a free idiom.

### Examples + README content (Area 3)

- **D-03:** **`examples/cronduit.toml` shows three integration patterns across
  existing + new jobs.**
  - **`[defaults].labels`** — single Watchtower-exclusion entry
    (`com.centurylinklabs.watchtower.enable = "false"`) demonstrating the
    "all docker jobs inherit unless overridden" pattern.
  - **Existing `hello-world` (docker, defaults-merge demo)** — adds a Traefik
    routing-style annotation (e.g.
    `traefik.http.routers.hello.rule = "Host(\`hello.local\`)"`) to demonstrate
    the per-job MERGE: `hello-world` ends up with both the inherited
    Watchtower label AND its own Traefik label.
  - **NEW job `isolated-batch`** (or similarly-named — planner picks) sets
    `use_defaults = false` and a single backup-tool-style label
    (`backup.exclude = "true"`) to demonstrate the WHOLE-SECTION REPLACE
    semantic. The operator can see in one diff "this job replaces, not
    merges."
  - **Existing `hello-world-container`** (docker, per-job override demo) —
    leave unchanged. Adding labels here would muddy its existing role
    demonstrating image / network / volume override.
  - Each new label line carries an inline comment cross-referencing the
    README subsection.

- **D-04:** **`README.md § Configuration` — full labels subsection.** ~30–40
  lines mirroring v1.0's `[defaults]` documentation depth. Sections:
  - Short prose intro (what it does, where it goes).
  - **Mermaid merge-precedence diagram** (per project rule D-14 — no ASCII).
    Order: `[defaults].labels → per-job [[jobs]].labels (merge or replace per use_defaults) → cronduit-internal labels (cronduit.run_id, cronduit.job_name) [overrides]`.
  - Merge-semantics table (3 rows: `use_defaults` unset/true/false × per-job
    label set/unset).
  - Reserved-namespace rule (`cronduit.*`) with single-line example.
  - Type-gate rule (docker-only) with single-line example.
  - Size limits (value ≤ 4 KB, total set ≤ 32 KB).
  - Env-var interpolation note (values yes, keys no).

### Seed lifecycle ceremony (Area 4)

- **D-05:** **Update `.planning/seeds/SEED-001-custom-docker-labels.md`
  frontmatter when the phase ships.** Edit in the LAST plan of Phase 17 (so
  the seed is closed only after every other deliverable is in place):
  - `status: dormant` → `status: realized`
  - Add `realized_in: phase-17`
  - Add `milestone: v1.2`
  - Add `realized_date: <ISO date of merge or close-out commit>`

  The seed file stays at `.planning/seeds/SEED-001-custom-docker-labels.md`
  (no physical move). The `17-SUMMARY.md` cross-references the seed path. The
  PR description for the final Phase 17 plan references the seed file
  inline. **This establishes the project's first realized-seed pattern** —
  future phases that close seeds inherit this shape.

  **Rejected:** physically moving the file to
  `.planning/seeds/realized/SEED-001-...md` (breaks any external reference;
  premature for the first realized seed); no formal close (loses the
  audit-trail observability — leaving a `dormant` status on a shipped seed
  is a freshness landmine for future maintainers).

### Project-rule reaffirmations (carried from prior phases)

- **D-06:** All Phase 17 changes land via PR on a feature branch. No direct
  commits to `main`. (Project rule, REQUIREMENTS.md and PROJECT.md.)
- **D-07:** All diagrams in any artifact — README, plan files, summary,
  PR description, code comments — are mermaid code blocks. The README labels
  merge-precedence diagram (D-04) is the load-bearing instance for this
  phase. (Project rule.)
- **D-08:** All UAT items in `17-HUMAN-UAT.md` (if produced by the planner)
  reference an existing `just` recipe — never ad-hoc `cargo` / `docker` /
  curl-URL invocations. (Project rule.)
- **D-09:** UAT items are validated by the maintainer running them locally —
  never marked passed from Claude's own runs. (Project rule.)
- **D-10:** The git tag and `Cargo.toml` `version` field always match. Phase
  17 ships within the v1.2.0-rc.1 / rc.2 cycle; no version field changes
  in this phase (Plan 15-01 already bumped to `1.2.0`). (Project rule.)

### Claude's Discretion

The planner picks freely on each of the following — none of these were
discussed in the gray-area selection:

- **Plan count and grouping.** A natural split is (1) schema + merge in
  `mod.rs` + `defaults.rs`; (2) the four new validators in `validate.rs`;
  (3) bollard plumb-through in `docker.rs`; (4) examples + README; (5)
  integration tests; (6) seed close-out. Planner may collapse plans (e.g.,
  schema+merge+plumb-through into one) or expand them. Atomic-commit-per-plan
  per project convention.
- **Validator function names.** Suggested:
  `check_label_reserved_namespace` (LBL-03),
  `check_labels_only_on_docker_jobs` (LBL-04, mirrors
  `check_cmd_only_on_docker_jobs`), `check_label_size_limits` (LBL-06),
  `check_label_key_chars` (D-02). Planner may rename for symmetry.
- **Whether all four checks live in one function or four.** Four functions
  parallels the existing per-validator-per-concern shape; one combined
  function that walks each `(key, value)` once is more cache-friendly. Either
  is fine; the existing `validate.rs` style suggests four.
- **Const regex via `once_cell::sync::Lazy<Regex>` vs hand-rolled
  char-by-char match for D-02.** `once_cell` is already a project dep
  (`src/config/interpolate.rs:22`) so the regex is free. A hand-rolled match
  avoids a regex compile at process start. Either is acceptable; the regex
  shape is short enough that the choice is taste.
- **Whether `apply_defaults` extension lives inside the existing function or
  in a new helper `apply_label_defaults(...)`.** Either matches the existing
  shape; a helper improves testability if the labels merge grows complex.
- **`testcontainers` integration test naming.** Convention is
  `tests/v12_<feature>_<scenario>.rs`. Suggested: `tests/v12_labels_merge.rs`
  (defaults+per-job merge ends up on container per `docker inspect`),
  `tests/v12_labels_use_defaults_false.rs` (replace semantic),
  `tests/v12_labels_validators.rs` (config-load rejection paths). Planner
  picks count + names per feature-coverage need.
- **Whether `17-HUMAN-UAT.md` is produced.** The phase is largely
  CI-observable (validators emit deterministic errors, bollard plumb-through
  has a `docker inspect` integration test). Maintainer-facing UAT is
  worthwhile for the README-renders-correctly + examples/cronduit.toml-loads
  scenarios specifically. Planner decides scope.
- **Whether to add a fail-on-empty-string-value check.** TOML allows
  `key = ""`; bollard accepts it. Adding a "value must be non-empty" check
  is a small extension of the size validator; planner picks. Default
  recommendation: skip — empty-string values are valid Docker labels and
  rejecting them would surprise operators.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level

- `.planning/PROJECT.md` § Current Milestone (v1.2 scope, custom Docker
  labels listed as one of five features) and § Constraints (locked tech
  stack — `bollard`, no shelling out).
- `.planning/REQUIREMENTS.md` § Custom Docker Labels (LBL) — **LBL-01**
  through **LBL-06** are the canonical requirement IDs Phase 17 satisfies.
  T-V12-LBL-01..LBL-10 are the verification anchors.
- `.planning/ROADMAP.md` § "Phase 17: Custom Docker Labels (SEED-001)" —
  goal, five operator-observable success criteria, depends-on (Phase 15
  hygiene preamble; INDEPENDENT of Phase 16 FCTX work).
- `.planning/seeds/SEED-001-custom-docker-labels.md` — **MUST READ.**
  Pre-locked design (merge semantics, reserved namespace, type gating);
  scope estimate; breadcrumbs to source files; "Decisions LOCKED at seed
  time" table that the planner MUST NOT re-litigate. After this phase
  ships, the planner edits this file's frontmatter per D-05.
- `.planning/STATE.md` — current phase state (`current_phase: null`,
  ready-to-plan posture for Phase 17).

### Phase 15 precedent (immediate-prior v1.2 phase)

- `.planning/phases/15-foundation-preamble/15-CONTEXT.md` — D-13/D-14/D-15/
  D-16/D-17 carry-forwards (PR-only, mermaid, just-recipe UAT, version
  match, user-validated UAT). Phase 17 inherits these as D-06..D-10.
  Structural pattern for "v1.2 phase touching config + scheduler + docs."

### Research

- `.planning/research/STACK.md` — confirms no new crate is required for
  Phase 17. `HashMap` is std; `serde` already wired; `regex` +
  `once_cell::sync::Lazy` already used in `src/config/interpolate.rs`.
- `.planning/research/PITFALLS.md` — review for label-collision and
  silent-override pitfalls. Cronduit-internal-label collision is
  structurally prevented by D-02 + the LBL-03 validator (operator can never
  set `cronduit.*`).

### Source files the phase touches

- `src/config/mod.rs` `L76-85` (`DefaultsConfig`) and `L88-120`
  (`JobConfig`) — add the new `labels` field on both. Must use
  `Option<HashMap<String, String>>` per SEED-001. Field placement is
  planner discretion; suggested order is after `volumes` (the closest
  semantic peer).
- `src/config/defaults.rs` `L112+` (`apply_defaults`) — extend with the
  labels merge per LBL-02. Existing test
  `apply_defaults_use_defaults_false_disables_merge` at `L316` is the test
  pattern to mirror for the labels case.
- `src/config/validate.rs` `L88-92` (per-job validator loop) — register
  call sites for the four new validators (LBL-03 reserved-namespace,
  LBL-04 type-gate, LBL-06 size limits, D-02 key chars). The validator
  function template is `check_cmd_only_on_docker_jobs` at `L89`.
- `src/scheduler/docker.rs` `L146-149` — the labels HashMap currently
  contains `cronduit.run_id` and `cronduit.job_name`. Extend to merge in
  operator-defined labels BEFORE the internal labels are inserted (so
  internal labels structurally win on the impossible-due-to-validator
  collision case). The merged map then populates `Config::labels: Some(...)`
  at the existing `Config` build site.
- `src/scheduler/docker_orphan.rs` `L31` — consumer of `cronduit.run_id`
  for orphan reconciliation. Justifies the `cronduit.*` reserved namespace.
  No code change here — just the load-bearing reason the LBL-03 validator
  exists.
- `examples/cronduit.toml` — add labels to existing `[defaults]` block,
  existing `hello-world` job, and a NEW `use_defaults = false` job per D-03.
- `README.md` § Configuration — add full labels subsection per D-04.

### NEW files

- `tests/v12_labels_*.rs` — integration tests via `testcontainers`. Coverage
  per the LBL-* T-V12-LBL-01..LBL-10 verification anchors. Planner picks
  exact filenames + count.

### Cross-reference (for SEED-001 close-out per D-05)

- `.planning/seeds/SEED-001-custom-docker-labels.md` — frontmatter edit
  target in the LAST plan of Phase 17.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **Per-job validator loop** (`src/config/validate.rs:88-92`) — direct
  registration site for the four new label validators. Existing shape: a
  loop over `cfg.jobs` calling `check_one_of_job_type`,
  `check_cmd_only_on_docker_jobs`, `check_network_mode`, `check_schedule`.
  The new validators slot in alongside.
- **`check_cmd_only_on_docker_jobs`** (`src/config/validate.rs:89`) — the
  literal template for the type-gate validator (LBL-04). Same idiom: detect
  a non-docker job by checking `job.command.is_some() || job.script.is_some()`
  (or via the post-`apply_defaults` `image.is_none()` discriminant), emit
  one `ConfigError` per offending job.
- **`apply_defaults`** (`src/config/defaults.rs:112`) — direct extension
  point for the labels merge. The existing function already handles the
  `use_defaults = false` short-circuit at `L114-117`; the labels case slots
  into the per-field merge sequence below it. The precedent is the
  `volumes` merge at `L138-142` (Per-job REPLACES defaults for `volumes` —
  but for labels we want per-key MERGE on collision; refer to the SEED-001
  decision table for the difference).
- **`apply_defaults_use_defaults_false_disables_merge`**
  (`src/config/defaults.rs:316`) — the test template for the labels case.
- **Pre-parse env-var interpolation** (`src/config/interpolate.rs:22`) —
  handles `${VAR}` in label VALUES for free; values pass through the
  regex-replace pipeline along with the rest of the TOML. No labels-specific
  interpolation code is needed. The "keys not interpolated" rule is enforced
  implicitly via the D-02 strict char regex (any leftover `${` or `}` in a
  key is rejected).
- **`once_cell::sync::Lazy<Regex>` idiom** (`src/config/interpolate.rs:22`) —
  pattern for the D-02 key-char regex. Compile-time const regex; no runtime
  cost beyond the first call.
- **`docker_orphan.rs` `cronduit.run_id` consumer** (`L31`, `L48`, `L50`) —
  load-bearing justification for the `cronduit.*` reserved namespace. The
  orphan reconciliation depends on `cronduit.run_id` and `cronduit.job_name`
  being structurally trustworthy.
- **Existing labels HashMap at container-create time**
  (`src/scheduler/docker.rs:146-148`) — the merge site. Currently inserts
  exactly two keys (`cronduit.run_id`, `cronduit.job_name`); extend to
  insert operator labels FIRST (so cronduit-internal labels structurally
  win on key collision — though the LBL-03 validator means there can never
  be a real conflict).
- **`ConfigError` struct + `byte_offset_to_line_col`** (`src/config/mod.rs`)
   — emit pattern. New label validators use `line: 0, col: 0` per D-01.
- **Integration test convention** — `tests/v12_<feature>_<scenario>.rs`
  with `testcontainers` for any docker-touching test. Phase 17 follows
  `tests/v12_labels_*.rs`.

### Established Patterns

- **Module-per-feature, file-per-concern.** `src/config/{mod,defaults,
  validate,interpolate,hash}.rs` is a flat structure; Phase 17 does NOT add
  new files inside `src/config/` (the four label validators live alongside
  the existing ones in `validate.rs`).
- **Errors collected, not fail-fast.** `parse_and_validate` accumulates ALL
  errors into `Vec<ConfigError>` before returning. D-01's per-job-per-check
  aggregation respects this.
- **`HashMap<String, String>` over `BTreeMap` for labels.** Bollard's
  `Config::labels` is `Option<HashMap<String, String>>`; matching the type
  avoids a conversion. Existing v1.0 `BTreeMap<String, SecretString>` for
  `env` is intentionally different (env values are secrets;
  label values are not).
- **Plan-per-atomic-commit.** Phase 15 plan ordering (`15-01` bump,
  `15-02` deny, `15-03..N` worker scaffold) is the structural precedent for
  Phase 17's plan split.
- **Test-via-`just` for UAT** (project rule D-08). Examples that need
  manual verification cite an existing `just` recipe such as `just check`
  (config validation) or `just docker-up` (end-to-end with the example
  config). No ad-hoc `docker inspect` curls in UAT step text.

### Integration Points

- **`DefaultsConfig` + `JobConfig` field additions** (`src/config/mod.rs`).
- **`apply_defaults` extension** (`src/config/defaults.rs:112+`) per LBL-02.
- **Four new validator registrations** in the per-job loop at
  `src/config/validate.rs:88-92` per LBL-03 + LBL-04 + LBL-06 + D-02.
- **Operator-labels merge into the existing labels HashMap** at
  `src/scheduler/docker.rs:146-149` per LBL-01.
- **`examples/cronduit.toml`** + **`README.md § Configuration`** per D-03 +
  D-04.
- **Frontmatter edit** of `.planning/seeds/SEED-001-custom-docker-labels.md`
  in the last plan per D-05.
- **No DB schema change.** No migration. Bollard's `Config::labels` already
  accepts the type. No new external dep.

</code_context>

<specifics>
## Specific Ideas

- **The seed's "Decisions LOCKED at seed time" table is binding.** Planner:
  do NOT re-litigate merge semantics, reserved namespace, or type gating in
  any plan file. Cite the SEED-001 row instead.
- **Three integration patterns must be visible in
  `examples/cronduit.toml`** (D-03): Watchtower exclusion in `[defaults]`,
  Traefik annotation per-job-merge on `hello-world`, backup-tool filter on
  the new `use_defaults = false` job. Operators reading the example file
  should be able to pattern-match each integration to a real homelab
  workflow without consulting the README.
- **The README mermaid diagram is load-bearing for D-04.** It is the
  visual proof of the merge-precedence chain (defaults → per-job →
  cronduit-internal-overrides). Operators who skim the README will see this
  diagram before the table; the diagram MUST capture all four steps and the
  "internal labels override" arrow direction.
- **D-02's strict key validation partially enforces LBL-05's
  "keys are NOT interpolated."** Residual gap: the pre-parse
  `interpolate::interpolate` pass at `src/config/interpolate.rs:22` does NOT
  distinguish keys from values — it operates on raw TOML text. If an
  operator writes a label key like `${VAR}` and VAR is set to a regex-safe
  string (e.g. `traefik.enable`), the key resolves and passes the strict
  char regex. This is undocumented behavior and is flagged in **Deferred
  Ideas** as a v1.3+ tightening candidate. For Phase 17, the LBL-05
  contract is satisfied by:
  1. The strict char regex (most common case: leftover `${`/`}` chars are
     rejected).
  2. The README labels subsection explicitly documenting "keys are NOT
     interpolated" to set operator expectations.
- **Phase 17 is independent of Phase 16 (FCTX schema).** It depends only on
  Phase 15 (foundation preamble landing). Planner: do NOT introduce a
  structural dependency on `job_runs.image_digest` or any FCTX column.
  Labels are config-time only; no DB column.
- **Phase 17 ships with rc.1.** Roadmap places the "foundation block"
  rc.1 cut after Phase 17's deliverables land alongside Phase 15 + 16's. No
  rc-specific gating in this phase's plans, but the SUMMARY should
  cross-reference the rc.1 readiness target so the milestone close-out
  audit has a clear input.

</specifics>

<deferred>
## Deferred Ideas

- **Display operator labels in the Web UI run-detail / job-detail page.**
  Came up implicitly during the README depth discussion (the merge-precedence
  diagram naturally raises "could the UI show this?"). Out of scope for
  Phase 17 — labels are an operator-visible Docker concern, not a cronduit
  Web UI concern. If operators ask for it post-v1.2, file as a v1.3
  candidate.
- **Substring-after-interpolation key gap.** D-02's strict char regex
  catches the most common leftover-`${` case but not the
  fully-resolved-to-safe-chars case. Tightening would require either: (a)
  marking labels.keys as a region the pre-parse interpolation pass skips
  (would need spans), or (b) a post-parse check comparing each key against
  a copy of the raw TOML byte range. Both are scope creep against Phase
  17's "config + plumb-through + docs" shape. Flag for v1.3+ if the gap
  becomes a UX problem.
- **Generalizing the labels validator stack to non-docker label-equivalents
  (systemd unit annotations, log tag emission).** SEED-001 explicitly defers
  this. Re-evaluate at v1.3 milestone kickoff if scope warrants.
- **Label-based metric labels (Prometheus `cronduit_*` family).** Same
  posture as job tags (TAG category, Phase 22-23): UI-only, no metric-label
  side-effect. Adding labels as Prometheus dimensions would make cardinality
  unbounded — explicitly rejected at requirements time. Flag for v1.3+ ONLY
  if a structural cardinality cap (e.g., a configurable allowlist of label
  keys eligible for export) lands.
- **Label-based webhook routing keys.** WH-09 (Phase 18) includes
  tags-and-labels-equivalent in the payload but never AS a routing key.
  Same posture as labels in metrics. Out of v1.2 entirely.
- **`cronduit.*` namespace expansion.** Future cronduit-internal labels
  (e.g., `cronduit.job_run_number`, `cronduit.image_digest`) are reserved
  by D-A1 / LBL-03 but not added in Phase 17. Phase 16 already added
  `image_digest` as a `job_runs` column; whether to also expose it as a
  container label is a Phase 21 (failure-context UI) consideration, NOT a
  Phase 17 one.
- **`bans.skip` deny.toml entries for transitive duplicates introduced by
  Phase 17.** None expected — the phase introduces no new crate. If
  cargo-deny's warn output during Phase 17 implementation surfaces a new
  duplicate, Plan 15-02's posture (warn-only, allowlist empty) absorbs it
  without action.
- **Empty-string label values rejection.** Discussed under Claude's
  Discretion; default is to accept (TOML and bollard both accept). If
  operator feedback says empty values are footguns, revisit.
- **Physical move of realized seed files to
  `.planning/seeds/realized/`.** Discussed and rejected as premature for
  the first realized seed. Revisit if/when several seeds are in the
  `realized` state and the directory becomes hard to scan.

</deferred>

---

*Phase: 17-custom-docker-labels-seed-001*
*Context gathered: 2026-04-28*
