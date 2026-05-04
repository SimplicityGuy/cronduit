# Phase 22: Job Tagging Schema + Validators - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-04
**Phase:** 22-job-tagging-schema-validators
**Areas discussed:** Tags in config_hash + config_json, Substring-collision error UX, WH-09 webhook payload backfill, Per-job tag cap + UAT shape

---

## Tags in config_hash + config_json

### Q1: `compute_config_hash` participation

| Option | Description | Selected |
|--------|-------------|----------|
| Exclude tags from config_hash | Tags are organizational metadata, not execution input. Mirrors webhook field's exclusion rationale. Tag-only edits invisible to FCTX `config_hash` delta and don't churn the webhook payload's hash. | ✓ |
| Include tags in config_hash (mirrors labels) | Treats tags like labels — every tag edit is a config change. FCTX panel shows 'config changed since last success' on tag-only edits; webhook receivers see hash churn. | |

**User's choice:** Exclude tags from config_hash.
**Captured as:** D-01.

### Q2: `serialize_config_json` blob participation

| Option | Description | Selected |
|--------|-------------|----------|
| Single source: column only | Tags live exclusively in `jobs.tags`. `config_json` stays unchanged — no parity invariant to maintain. Consistent with the 'exclude from hash' choice. | ✓ |
| Both: column + config_json (mirrors labels) | Tags ALSO go into `serialize_config_json` like labels do. Creates a parity invariant for no functional benefit. | |

**User's choice:** Single source: column only.
**Captured as:** D-02.

---

## Substring-collision error UX

### Q1: Error format and scope

| Option | Description | Selected |
|--------|-------------|----------|
| One fleet-level error per colliding pair | After all jobs are normalized, run a single fleet-level pass that emits one `ConfigError` per substring-colliding pair, naming both tags AND which jobs use them. Single error per pair = no spam when many jobs share the same tag. | ✓ |
| Per-job error at every offending site | Each job that uses a colliding tag gets its own `ConfigError`. Mirrors per-job-per-violation idiom from Phase 17 D-01 strictly, at the cost of error spam. | |
| One global error listing all colliding pairs | Single `ConfigError` enumerating every colliding pair found. Lowest spam but loses per-pair granularity — hard to scan. | |

**User's choice:** One fleet-level error per colliding pair.
**Captured as:** D-03 (with normalization order locked in D-04).

---

## WH-09 webhook payload backfill

### Q1: Phase 22 scope or defer

| Option | Description | Selected |
|--------|-------------|----------|
| Include in Phase 22 | Read `jobs.tags` into the payload build site so WH-09 receivers see real tag values the moment Phase 22 ships. The placeholder test (`payload_tags_empty_array_until_p22`) literally hints this is P22 scope. | ✓ |
| Defer to a follow-on phase | Keep Phase 22 narrowly schema/validators. WH-09 receivers continue to see `tags: []` until later. | |
| Include + harden the placeholder test | Same as 'Include in Phase 22' but explicitly: rename the test and add a fixture that asserts a multi-tag job round-trips. Locks the cutover. | (folded into D-06.5) |

**User's choice:** Include in Phase 22.
**Captured as:** D-05 + D-06.5 (test rename folded in by Claude).

### Q2: Read path for tags in webhook payload

| Option | Description | Selected |
|--------|-------------|----------|
| Add `tags: Vec<String>` to `DbRunDetail` | Read tags directly from `jobs.tags` JSON column when fetching run detail. One canonical source. Symmetric with `image_digest` and `config_hash` flow. | ✓ |
| Cache per-job in `Arc<HashMap<i64, Vec<String>>>` at bin layer | Mirrors WebhookConfig pattern. Faster but introduces a second source-of-truth and bin-layer plumbing change. | |

**User's choice:** Add `tags: Vec<String>` to `DbRunDetail`.
**Captured as:** D-07.

---

## Per-job tag cap + UAT shape

### Q1: Tag count cap

| Option | Description | Selected |
|--------|-------------|----------|
| Hard cap of 16 tags per job | Reject at config-load if a job declares >16 tags. Phase 23 chip UI stays readable; operators rarely need >16 dimensions; cap can be lifted later without migration. Mirrors LBL-06 posture. | ✓ |
| No cap (charset only) | Charset and substring-collision are the only bounds. An operator can attach 1000 tags per job. | |
| Soft cap of 16 with WARN | Don't reject; emit WARN. Invisible after first startup; P23 still has to handle unbounded case. | |
| Hard cap of 8 per job | Tighter cap. Safer for the chip UI but might bite operators with reasonable organizational density. | |

**User's choice:** Hard cap of 16 tags per job.
**Captured as:** D-08.

### Q2: HUMAN-UAT scope

| Option | Description | Selected |
|--------|-------------|----------|
| End-to-end + validator error UX | Maintainer scenarios: persistence spot-check, each invalid case eyeballed for operator readability, end-to-end webhook backfill confirms real tag values. Each step references an existing or new `just` recipe. | ✓ |
| Validator error UX only | Just hand-eyeball each rejection error for clarity. Skip end-to-end webhook scenario. | |
| Skip HUMAN-UAT entirely | Phase is small enough that integration tests cover everything. Risk: error-message UX goes un-eyeballed. | |

**User's choice:** End-to-end + validator error UX.
**Captured as:** D-10 + D-11 (three new `just` recipes for the UAT scenarios).

---

## Claude's Discretion

The planner picks freely on:
- Plan count and grouping (suggested 6-plan split documented in CONTEXT § Claude's Discretion).
- Validator function names (`check_tag_charset_and_reserved`, `check_tag_substring_collision`, `check_tag_count_per_job` suggested).
- Whether normalization is a sibling helper (`normalize_tags(...)`) or inline.
- `once_cell::sync::Lazy<Regex>` for the charset check (free idiom in tree).
- Migration filename + timestamp prefix (suggested `20260504_000010_jobs_tags_add.up.sql`).
- Sorted-canonical JSON form vs insert-order in the `jobs.tags` column (recommendation: sorted).
- `examples/cronduit.toml` tag examples (suggested: 1-2 demo jobs gain `tags = [...]`).
- Whether to ship a README configuration subsection on tags (Phase 17's labels subsection is the template if picked up).
- Validator order inside the per-job loop (locked in D-04: normalize → reject → dedup → cap).

## Deferred Ideas

Captured in CONTEXT.md `<deferred>`. Highlights:

- Tag-based bulk operations (v1.3 candidate per requirements).
- Tags as Prometheus label (cardinality discipline; explicit out-of-scope).
- `[defaults].tags` (rejected at requirements time).
- Tag-based webhook routing keys (WH-09 carries tags in payload but never AS a routing key).
- Per-job `Arc<HashMap<i64, Vec<String>>>` cache (rejected as alternative read path; v1.3 if scale demands).
- Tag participation in `compute_config_hash` / `serialize_config_json` (rejected this phase; revisitable in v1.3).
- Reserved-namespace prefix for tags (rejected as premature; finite list sufficient).
- Tag autocompletion in the dashboard (Phase 23 question, not Phase 22 surface).
- README configuration subsection on tags (Claude's discretion; could land in P22 or P23).
