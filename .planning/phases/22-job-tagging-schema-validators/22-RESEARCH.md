# Phase 22: Job Tagging Schema + Validators - Research

**Researched:** 2026-05-04
**Domain:** Rust + sqlx + serde — additive config field, JSON-in-TEXT column, validator family extension, webhook payload backfill
**Confidence:** HIGH (light pass — all decisions locked in CONTEXT.md; this research only confirms mechanics)
**Scope:** Light. CONTEXT.md fixes 16 D-decisions, exact file paths, and template precedents. This file confirms the six narrow mechanics flagged by the planner objective: serde_json round-trip behavior, the `Lazy<Regex>` idiom, the migration template, the substring-collision algorithm, the `DbRunDetail` row-mapping site, and a Validation Architecture map.

## Summary

Phase 22 is a **mechanically additive phase** — every gray-area decision is pre-locked, and the source-of-truth references are all in tree:

- **Migration template:** `migrations/{sqlite,postgres}/20260427_000005_image_digest_add.up.sql` (Phase 16). Same shape with `ALTER TABLE jobs ADD COLUMN tags TEXT NOT NULL DEFAULT '[]'`. Postgres uses `IF NOT EXISTS`; SQLite cannot. `tests/schema_parity.rs::normalize_type` already maps TEXT-family → `TEXT`, so the new column passes parity by construction with zero test edits.
- **Regex idiom:** `once_cell::sync::Lazy<Regex>` exactly per `src/config/validate.rs:10-13` and `src/config/interpolate.rs:34`. Crate already in tree (verified `Cargo.toml` `once_cell = "1"`, `regex = "1"`). Compile-on-first-call, zero runtime cost thereafter.
- **JSON round-trip:** `serde_json::to_string` / `from_str` on `Vec<String>` is round-trip safe and deterministic when input is sorted. `Vec<String>` matches the TOML deserialization shape (TOML arrays of strings → `Vec<String>` natively); writing `serde_json::to_string(&sorted_vec)` produces `["a","b"]`-style canonical output, which is what `serde_derive`'s `Serialize` for `Vec<String>` will emit on the WH-09 payload as well.
- **Substring-collision algorithm:** Plain `s1.contains(s2) where s1 != s2`, double-loop over the union of post-dedup tag sets. NOT regex. Deterministic when iteration is sorted. One `ConfigError` per pair, naming up to ~3 representative jobs per side.
- **DbRunDetail row-mapping site:** `src/db/queries.rs:1390` (`get_run_by_id`). Both SQLite (1414) and Postgres (1436) match arms construct `DbRunDetail` field-by-field; widen the SELECT to project `j.tags` from the joined `jobs` table, deserialize JSON → `Vec<String>` at the row mapping site (one-line `serde_json::from_str` per arm).
- **Validation Architecture:** Eight verification anchors map cleanly to two new test files (`tests/v12_tags_validators.rs` + extended `payload.rs::tests`). Schema parity green by construction.

**Primary recommendation:** Plan as 5 small, atomic-commit-per-plan slices: (1) migration + JobConfig field + serde_json sorted-canonical helper; (2) four validators in `validate.rs` + fleet-level pass; (3) `upsert_job` widening + `DbRunDetail.tags` + `get_run_by_id` row-map; (4) WH-09 payload backfill + test rename; (5) integration tests + `just` recipes + `examples/cronduit.toml` tag line. Each plan is a self-contained PR-ready commit.

## User Constraints (from CONTEXT.md)

### Locked Decisions

The CONTEXT.md `<decisions>` block locks 16 D-decisions. Reproduced here verbatim for the planner:

- **D-01:** Exclude `tags` from `compute_config_hash` (`src/config/hash.rs:16`). Add `// DO NOT include tags` comment mirroring `// DO NOT include env` at L50.
- **D-02:** Exclude `tags` from `serialize_config_json`. Single source of truth is the new `jobs.tags` column.
- **D-03:** Fleet-level substring-collision pass after the per-job loop. **One `ConfigError` per colliding pair**, message names BOTH tags AND the jobs that use them; multiple jobs sharing the same offending tag get listed up to ~3 with `(+N more)`.
- **D-04:** Validator order locked: **normalize → reject (charset + reserved) → dedup with WARN → per-job count cap (16) → fleet-level substring pass**.
- **D-05:** WH-09 backfill is in Phase 22 scope. The `tags: vec![]` placeholder at `src/webhooks/payload.rs:88` is replaced with `run.tags.clone()`.
- **D-06.5:** Rename `payload_tags_empty_array_until_p22` → `payload_tags_carries_real_values` and rewrite to assert real values round-trip.
- **D-07:** Read path: `DbRunDetail.tags: Vec<String>` sourced from `jobs.tags` JSON column at run-detail-fetch time. Symmetric with `image_digest`/`config_hash`. Implementation: extend `get_run_by_id` SELECT to project `j.tags`, deserialize JSON → `Vec<String>`.
- **D-08:** Hard cap of 16 tags per job. One `ConfigError` per job whose post-dedup count > 16: `[[jobs]] '<name>': has N tags; max is 16. Remove tags or split into multiple jobs.`
- **D-09:** Two new test files + extension of `payload.rs::tests`:
  - `tests/v12_tags_validators.rs`
  - `payload_tags_carries_real_values` test (replaces `payload_tags_empty_array_until_p22`)
  - `tests/schema_parity.rs` stays green automatically (TEXT-family normalization).
- **D-10:** `22-HUMAN-UAT.md` autonomous=false: persistence spot-check, validator error-UX walk, dedup-WARN, end-to-end webhook backfill.
- **D-11:** Three new `just` recipes — `uat-tags-persist`, `uat-tags-validators`, `uat-tags-webhook` — using the `recipe-calls-recipe` pattern.
- **D-12..D-17 [informational]:** PR-only branch state; mermaid-only diagrams; UAT via `just`; maintainer-validates UAT; `Cargo.toml` stays at `1.2.0` (no rc cut here); `cargo tree -i openssl-sys` empty; **zero new external crates** (`serde_json`, `once_cell`, `regex` all in tree).

### Claude's Discretion

The planner picks freely on:

- **Plan count and grouping** (5-plan suggested split above is not load-bearing).
- **Validator function names** (suggested: `check_tag_charset_and_reserved`, `check_tag_substring_collision`, `check_tag_count_per_job`).
- **Whether normalization lives in `validate.rs` or a sibling `tags` module** (one-liner — fits in the validator).
- **Migration filename** (suggested `20260504_000010_jobs_tags_add.up.sql`).
- **Sorted-canonical vs insert-order JSON form** — RECOMMENDATION: sorted canonical (rationale below in §3).
- **`examples/cronduit.toml`** — add `tags = [...]` to one or two demo jobs.
- **README subsection on tags** — optional.

### Deferred Ideas (OUT OF SCOPE)

- Dashboard filter chips + AND semantics + URL state (TAG-06..08, **Phase 23**).
- `[defaults].tags` (rejected at requirements time — TAG-01).
- Tag participation in `compute_config_hash` (D-01).
- Tag participation in `serialize_config_json` (D-02).
- Tag-based webhook routing keys (P18 D-17).
- Tag-based bulk operations (v1.3 candidate).
- Tags as Prometheus label (cardinality discipline).
- `release.yml` / `cliff.toml` / `docs/release-rc.md` modifications (rc.3 is Phase 23).

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TAG-01 | `tags: Vec<String>` on `JobConfig` (per-job only — NOT on `DefaultsConfig`) | §1 — `JobConfig` field add at `src/config/mod.rs:114-165`; serde derive shape known; `#[serde(default)]` handles unset case. |
| TAG-02 | `jobs.tags TEXT NOT NULL DEFAULT '[]'` JSON-serialized; single-file additive migration per backend; old jobs default to `'[]'` automatically | §2 — exact migration template at `migrations/{sqlite,postgres}/20260427_000005_image_digest_add.up.sql`; `tests/schema_parity.rs::normalize_type` (L57) already collapses TEXT-family. |
| TAG-03 | Lowercase + trim normalization; WARN on dedup-collapse | §4 — Validator order D-04: normalize first, then dedup with `tracing::warn!` line that names original inputs. |
| TAG-04 | Charset `^[a-z0-9][a-z0-9_-]{0,30}$`; reserved `cronduit` / `system` / `internal` | §1 + §4 — `Lazy<Regex>` idiom at `interpolate.rs:34`; reserved-list pattern at `validate.rs:24-31` (VALID_WEBHOOK_STATES); reject after normalization. |
| TAG-05 | Substring-collision rejection (fleet-level) | §5 — `s1.contains(s2) where s1 != s2`; one `ConfigError` per pair (D-03); fleet-level pass slots in alongside `check_duplicate_job_names` at `validate.rs:612`. |
| WH-09 | Webhook payload `tags` field carries real values (closes the Phase 18 placeholder) | §6 — `WebhookPayload::build` site `src/webhooks/payload.rs:88`; `DbRunDetail.tags` add; `get_run_by_id` row-map at `queries.rs:1390-1454`; rename `payload_tags_empty_array_until_p22` → `payload_tags_carries_real_values`. |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| TOML deserialization of `tags = [...]` | Config layer (`src/config/mod.rs`) | — | `serde::Deserialize` derive on `JobConfig`; `#[serde(default)]` for optional field. |
| Tag normalization + validation | Config validators (`src/config/validate.rs`) | — | Existing per-job loop (`L88-92`) + fleet-level pass site (analog at `L612`). All errors collected into `Vec<ConfigError>` (no fail-fast). |
| Tag persistence (write) | DB layer (`src/db/queries.rs::upsert_job`) | — | Widen INSERT/ON CONFLICT UPDATE to bind `tags_json: &str`. |
| Tag persistence schema | Migration files | `tests/schema_parity.rs` | Single ALTER TABLE per backend; parity test enforces no drift. |
| Tag read for run detail | DB layer (`get_run_by_id`) | — | Project `j.tags` from JOIN; `serde_json::from_str` at row-map. |
| Tag delivery in webhook payload | Webhook payload (`src/webhooks/payload.rs`) | DB layer (`DbRunDetail.tags`) | Read site already symmetric with `image_digest`/`config_hash`. |
| Operator-readable error UX | Config validators (error messages) | UAT recipes (`just`) | Phase 17 D-01 per-job-per-violation `ConfigError` shape inherited. |
| Static asset (example TOML) | `examples/cronduit.toml` | — | Demo only; planner discretion. |

**Out of tier (intentional):** dashboard rendering, filter chips, URL state — these are Phase 23 (browser/SSR templating tier).

## Standard Stack

### Core (already in tree — D-17 lock: zero new external crates)

| Library | Version | Purpose | Why Standard | Verification |
|---------|---------|---------|--------------|--------------|
| `serde` | 1.x | Derive `Deserialize` on `JobConfig` | Universal; already used pervasively. | `[VERIFIED: Cargo.toml]` |
| `serde_json` | 1.x | JSON-in-TEXT column round-trip; payload serialization | Already used by `compute_config_hash` (`hash.rs:52`), `WebhookPayload` serialize, sqlx no-op. | `[VERIFIED: Cargo.toml]` `serde_json = "1"` |
| `once_cell` | 1.x | `Lazy<Regex>` static for charset regex | Already used at `interpolate.rs:34`, `validate.rs:10-15`. | `[VERIFIED: Cargo.toml]` `once_cell = "1"` |
| `regex` | 1.x | Charset matching | Already used; standard. | `[VERIFIED: Cargo.toml]` `regex = "1"` |
| `tracing` | 0.1.x | `tracing::warn!` for TAG-03 dedup collapse | Already pervasive; structured logs. | `[VERIFIED: pre-existing usage]` |
| `sqlx` | 0.8.x | DB layer (already in use) | Locked tech stack constraint. | `[VERIFIED: CLAUDE.md]` |

### Supporting

None. The phase's complete dependency surface is the existing `serde + serde_json + once_cell + regex + tracing + sqlx` stack. **No `Cargo.toml` edits expected.**

### Alternatives Considered

| Instead of | Could Use | Tradeoff | Decision |
|------------|-----------|----------|----------|
| `Vec<String>` for tags | `BTreeSet<String>` | Auto-dedup, sorted iteration | **Rejected** — TOML deserializes arrays into `Vec<String>` natively; converting introduces a layer. The validator handles dedup explicitly; the storage layer normalizes by sorting before `serde_json::to_string`. |
| Sorted-canonical JSON form | Insert-order JSON form | Determinism vs. user-visible order preservation | **Recommended sorted-canonical** — makes diffs deterministic, makes substring-collision pass stable, makes WH-09 payload deterministic across re-uploads. Operators rarely care about array order in `tags = [...]`. |
| Storing tags JSON in `config_json` blob | New `jobs.tags` column | Single source of truth | **Locked column** (D-02). Column = canonical; future P23 read site is the same column. |
| `regex::Regex::new` per-call | `Lazy<Regex>` static | Compile cost on every config-load | **Locked `Lazy<Regex>`** (D-17 idiom; pattern free since dep in tree). |
| Including tags in `compute_config_hash` | Excluding (D-01) | Tag-only edit triggers "config changed" FCTX signal | **Locked exclusion** — tags are organizational, not docker-execution input. |

## Architecture Patterns

### System Architecture Diagram

```mermaid
flowchart LR
    A[cronduit.toml<br/>tags = [...]] -->|1. interpolate ${VAR}| B[String]
    B -->|2. toml::from_str| C[JobConfig.tags: Vec<String>]
    C -->|3. per-job loop| D[Normalize<br/>trim + lowercase]
    D -->|4. charset + reserved| E[Reject<br/>ConfigError]
    D -->|5. dedup w/ WARN| F[Vec<String> sorted]
    F -->|6. count cap 16| G[ConfigError if > 16]
    F -->|7. fleet-level pass| H[Substring-collision<br/>ConfigError per pair]
    F -->|8. serde_json::to_string| I[upsert_job tags_json]
    I -->|9. SQLite/Postgres ALTER| J[(jobs.tags TEXT<br/>NOT NULL DEFAULT '[]')]
    J -->|10. SELECT j.tags<br/>get_run_by_id| K[serde_json::from_str]
    K -->|11. row-map| L[DbRunDetail.tags: Vec<String>]
    L -->|12. clone| M[WebhookPayload.tags]
    M -->|13. serde::Serialize| N[JSON wire payload<br/>tags: ['backup','weekly']]

    style A fill:#1f2937,color:#fff
    style J fill:#0c4a6e,color:#fff
    style N fill:#065f46,color:#fff
```

Steps 4–7 are config-load validators; rejection at any of 4/6/7 produces `ConfigError`s collected into the existing `Vec<ConfigError>` accumulator (no fail-fast). Step 5 emits `tracing::warn!` only — never blocks.

### Recommended File Layout (no new files except migrations/tests)

```
src/config/
├── mod.rs           # JobConfig.tags field add (L114-165)
├── validate.rs      # 3-4 new fns mirror check_label_* family (L185-435)
└── hash.rs          # comment-only edit (DO NOT include tags)

src/db/
└── queries.rs       # upsert_job widen, DbRunDetail.tags, get_run_by_id row-map

src/webhooks/
└── payload.rs       # one-liner replace + test rename

migrations/sqlite/    20260504_000010_jobs_tags_add.up.sql   (NEW — suggested)
migrations/postgres/  20260504_000010_jobs_tags_add.up.sql   (NEW — suggested)

tests/
└── v12_tags_validators.rs   (NEW)

examples/cronduit.toml       # add tags = [...] to one demo job

justfile                     # 3 new uat-tags-* recipes
```

### Pattern 1: Lazy<Regex> for charset validator

**What:** `once_cell::sync::Lazy<Regex>` static — compiled on first access, zero cost thereafter.
**When to use:** Any validator regex that runs in a hot loop or per-row.
**Example (verbatim from `src/config/validate.rs:10-15`):**

```rust
use once_cell::sync::Lazy;
use regex::Regex;

static LABEL_KEY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-zA-Z0-9_][a-zA-Z0-9._-]*$").unwrap()
});
```

**Phase 22 application — `check_tag_charset_and_reserved`:**

```rust
static TAG_CHARSET_RE: Lazy<Regex> = Lazy::new(|| {
    // TAG-04: must start with [a-z0-9]; body [a-z0-9_-]; total 1–31 chars.
    // Pattern is checked AGAINST the post-normalization (lowercase+trim) form
    // per D-04 step 2; uppercase input is normalized first and would only
    // fail charset if it contained chars outside [a-z0-9_-] post-lowercase.
    Regex::new(r"^[a-z0-9][a-z0-9_-]{0,30}$").unwrap()
});

const RESERVED_TAGS: &[&str] = &["cronduit", "system", "internal"];
```

`[CITED: src/config/validate.rs:15-20, src/config/interpolate.rs:34]`

### Pattern 2: Per-job-per-violation ConfigError aggregation (Phase 17 D-01)

**What:** Every validator pushes 0..N `ConfigError`s into the shared `&mut Vec<ConfigError>`. No fail-fast. Sorted-key iteration for determinism (HashMap iter is non-deterministic — `validate.rs:184` Pitfall comment).

**Example template (`check_label_reserved_namespace` at `validate.rs:185-206`):**

```rust
fn check_tag_charset_and_reserved(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    if job.tags.is_empty() { return; }
    // 1. normalize (TAG-03)
    let normalized: Vec<String> = job.tags.iter().map(|t| t.trim().to_lowercase()).collect();
    // 2. find offenders (TAG-04 charset + reserved + empty-after-trim)
    let mut bad_charset: Vec<String> = normalized.iter()
        .filter(|t| !t.is_empty() && !TAG_CHARSET_RE.is_match(t))
        .cloned().collect();
    let mut empty_after_trim: Vec<String> = job.tags.iter()
        .filter(|t| t.trim().is_empty())
        .cloned().collect();
    let mut reserved_hits: Vec<&str> = normalized.iter()
        .filter(|t| RESERVED_TAGS.contains(&t.as_str()))
        .map(String::as_str).collect();
    bad_charset.sort(); empty_after_trim.sort(); reserved_hits.sort();
    // ... emit one ConfigError per category that fired ...
}
```

`[CITED: src/config/validate.rs:185-206, 298-335]`

**Note on T-V12-TAG-02/03 (empty / whitespace-only tag):** PITFALLS.md §51 prevention rule 3 mandates: `tags = [""]` and `tags = ["   "]` REJECT (not silently drop). The `empty_after_trim` arm above implements this — these inputs are caught BEFORE the charset regex (which would also reject them since `""` doesn't match `^[a-z0-9]`).

### Pattern 3: Fleet-level check (sibling of `check_duplicate_job_names`)

**What:** A free function called at `run_all_checks` top level, AFTER the per-job loop completes. Operates on `&[JobConfig]` (the whole fleet).

**Example template (`check_duplicate_job_names` at `validate.rs:612`):**

```rust
fn check_tag_substring_collision(jobs: &[JobConfig], path: &Path, errors: &mut Vec<ConfigError>) {
    // 1. Build (job_name, normalized_tag) pairs across the fleet.
    // 2. Build sorted unique tag list.
    // 3. Double loop: for each (i, j) where i < j, check tag_i.contains(tag_j) || tag_j.contains(tag_i).
    // 4. For each colliding pair: gather job names that use each tag, emit one ConfigError.
}
```

Registered in `run_all_checks` at `validate.rs:46-65` as a sibling of `check_duplicate_job_names(...)` (currently L46), AFTER the per-job loop (L47-65).

`[CITED: src/config/validate.rs:46, 612-654]`

### Anti-Patterns to Avoid

- **Hand-rolled regex compilation per call.** `regex::Regex::new("...")` on every config-load entry costs ~30µs; `Lazy<Regex>` amortizes to first-call cost only. (`[CITED: validate.rs:10-15 idiom]`)
- **HashMap iteration without `.sort()` before formatting error messages.** Iteration order is non-deterministic; tests that grep for specific error message text become flaky. Pattern: collect to `Vec<&str>`, `.sort()`, then `.join(", ")`. (`[CITED: validate.rs:184, 195, 308, 369]`)
- **Including `tags` in `compute_config_hash`.** D-01 lock; would make every tag rename trigger an FCTX "config changed since last success" signal — dilutes the operational signal that cares about docker-execution change.
- **Including `tags` in `serialize_config_json`.** D-02 lock; creates a parity invariant `tests/schema_parity.rs` doesn't enforce.
- **Persisting unsorted JSON.** Insert-order JSON makes the column value churn on every config re-upload of the same TOML. Sort before `serde_json::to_string` per Recommendation §3.
- **Three-file tightening migration pattern.** TAG-02 explicitly mandates one-file additive (`NOT NULL DEFAULT '[]'`). The Phase 11 `job_run_number` three-file shape (add nullable → backfill → tighten) is **not** the right precedent — Phase 16 `image_digest_add` is.
- **Forgetting to rename `payload_tags_empty_array_until_p22`.** The `_until_p22` suffix is a structural breadcrumb; if Phase 22 ships without renaming, the test name lies and future readers are confused. D-06.5 makes this load-bearing.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Regex compilation caching | Custom `OnceCell<Regex>` wrapper | `once_cell::sync::Lazy<Regex>` (in-tree at `interpolate.rs:34`) | Identical pattern; zero new code. |
| TOML array → `Vec<String>` parsing | Custom split + parse | `serde::Deserialize` derive on `Vec<String>` field | Native serde + toml support; one line. |
| JSON array round-trip | Custom encoder/decoder | `serde_json::to_string` / `from_str` | Already in tree; tested at scale; deterministic when input is sorted. |
| Sorted dedup | Manual `sort` + `dedup` loop | `let mut v = vec; v.sort(); v.dedup();` (stdlib) | Stdlib idiom; O(n log n); preserves chosen canonical form. |
| Substring check | regex | `s1.contains(s2)` (stdlib `str::contains`) | Plain string contains — D-03 spec. Regex would be wrong AND slower. |

**Key insight:** Phase 22 has zero "deceptively complex" subproblems. Every primitive (regex, JSON, TOML deserialize, sort/dedup, substring) has a stdlib or in-tree-crate solution that is exactly fit-for-purpose.

## Section 1 — `serde_json` Round-Trip for `Vec<String>`

**Confirmed behavior (HIGH confidence):**

`serde_json::to_string(&Vec<String>)` produces `[]` for empty, `["a"]` for single, `["a","b"]` for sorted-multi (no whitespace, no escapes for our charset since `[a-z0-9_-]` are all JSON-safe). `serde_json::from_str::<Vec<String>>(&s)` is the symmetric inverse.

**Round-trip test (already implicit in similar paths):**

```rust
// In tests/v12_tags_validators.rs
let original: Vec<String> = vec!["backup".to_string(), "weekly".to_string()];
let json = serde_json::to_string(&original).unwrap();
assert_eq!(json, r#"["backup","weekly"]"#);
let restored: Vec<String> = serde_json::from_str(&json).unwrap();
assert_eq!(original, restored);
```

**Sorted-canonical recommendation rationale (planner discretion D-09 sub-bullet):**

| Property | Sorted-canonical | Insert-order |
|----------|-----------------|---------------|
| Column value stable across re-uploads of the same TOML | YES | NO (operator reorder = column rewrite) |
| Substring-collision iteration is deterministic | YES | NO (need to sort anyway) |
| WH-09 payload bytes deterministic across re-uploads | YES | NO |
| Diff-friendly for ops eyeballing the DB | YES | NO |
| User-visible order preservation | NO (operator wrote `["weekly","backup"]`, sees `["backup","weekly"]`) | YES |
| Test fixtures simpler | YES (single canonical form) | NO (must match input order exactly) |

Five wins for sorted-canonical against one for insert-order; the lost property (user-visible order) is low-value because TOML arrays of organizational tags rarely have meaningful order. **Recommendation: sort before `serde_json::to_string`.** If insert-order is chosen, document it prominently in the migration comment AND adjust the substring-collision pass to sort-then-pair (otherwise iteration is non-deterministic).

**Edge cases to lock with tests:**

- `tags = []` → `'[]'` in column (matches the `DEFAULT '[]'` for old jobs — round-trip is null-equivalent).
- `tags = ["a"]` → `'["a"]'` (single-element array, no special handling).
- Tag containing internal hyphen `prod-east` → `'["prod-east"]'` (no escape; charset guarantees JSON-safe chars).

`[VERIFIED: serde_json existing usage at hash.rs:52, payload.rs serde derive]`
`[VERIFIED: Cargo.toml — serde_json = "1"]`

## Section 2 — `Lazy<Regex>` Idiom Confirmation

**Confirmed (HIGH confidence):** The pattern at `src/config/interpolate.rs:34` and `src/config/validate.rs:10-20` is the project's blessed idiom for static regex.

```rust
static TAG_CHARSET_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-z0-9][a-z0-9_-]{0,30}$").unwrap()
});
```

**Properties:**
- Compiled on first access (lazy) — zero cost during binary load.
- `unwrap()` is acceptable because the pattern is a compile-time constant and pattern-failure is a programmer bug caught by the unit test that constructs the regex implicitly on first match call.
- Thread-safe via `once_cell::sync::Lazy` (`std::sync::OnceLock` would also work but the project uses `once_cell` consistently).
- Zero runtime cost on subsequent calls beyond the regex match itself.

**Length math:** `^[a-z0-9][a-z0-9_-]{0,30}$` matches strings of length 1–31 (one anchored leading alphanumeric + 0..30 body chars). PITFALLS.md §52 (line 961) confirms: "Length 1–31 characters." Aligned.

**Reserved-name check:** `const RESERVED_TAGS: &[&str] = &["cronduit", "system", "internal"];` — finite slice; `RESERVED_TAGS.contains(&tag.as_str())` is `O(3)` linear scan, microseconds. No HashSet needed.

`[VERIFIED: src/config/interpolate.rs:34, src/config/validate.rs:10-20]`
`[VERIFIED: PITFALLS.md §52, lines 961-963]`

## Section 3 — One-File Additive Migration Shape

**Confirmed template (HIGH confidence):** `migrations/{sqlite,postgres}/20260427_000005_image_digest_add.up.sql` (Phase 16 — image_digest column).

### SQLite migration (suggested filename `20260504_000010_jobs_tags_add.up.sql`):

```sql
-- Phase 22: jobs.tags JSON column (TAG-01, TAG-02).
--
-- TEXT NOT NULL DEFAULT '[]', FOREVER (TAG-02): operators may attach
-- normalized organizational tags to any job in cronduit.toml; existing
-- pre-Phase-22 rows are auto-defaulted to '[]' on column add. Old rows
-- never need backfill — empty-array is a valid in-domain value.
--
-- Pairs with migrations/postgres/20260504_000010_jobs_tags_add.up.sql.
-- Any structural change MUST land in both files in the same PR;
-- tests/schema_parity.rs::normalize_type collapses TEXT-family types to
-- TEXT, so this column passes parity with zero test edits (RESEARCH §E
-- pattern carried from P16/image_digest).
--
-- Idempotency: sqlx _sqlx_migrations tracking. SQLite ALTER TABLE ADD
-- COLUMN does NOT support a conditional-existence guard clause
-- (Postgres pair uses one; SQLite cannot). Re-runs are guarded by
-- sqlx's migration ledger.

ALTER TABLE jobs ADD COLUMN tags TEXT NOT NULL DEFAULT '[]';
```

### Postgres migration (mirror):

```sql
-- Phase 22: jobs.tags JSON column (TAG-01, TAG-02).
--
-- (same comment as sqlite pair, with the IF NOT EXISTS rationale below)
--
-- Idempotency: Postgres `IF NOT EXISTS` provides re-run safety even if
-- sqlx's _sqlx_migrations ledger is somehow out of sync.

ALTER TABLE jobs ADD COLUMN IF NOT EXISTS tags TEXT NOT NULL DEFAULT '[]';
```

### Schema-parity reasoning (why no `tests/schema_parity.rs` edit needed):

`normalize_type` at `tests/schema_parity.rs:57` already returns `"TEXT"` for the input `"TEXT"`. SQLite reports `TEXT` via PRAGMA; Postgres reports `text` via `information_schema.data_type` (lowercase). Both round-trip to the normalized `"TEXT"` token. Parity-test passes by construction.

`NOT NULL DEFAULT '[]'`:
- SQLite: PRAGMA reports `notnull = 1`, default `'[]'`. The Column struct (`tests/schema_parity.rs:25-30`) records `not_null: true`.
- Postgres: `information_schema.is_nullable = 'NO'`. Column struct records `not_null: true`.
- The Column struct does **NOT** record default values, so the per-backend default-syntax difference (SQLite `DEFAULT '[]'` vs Postgres `DEFAULT '[]'::text`) is invisible to parity. Both happen to be the same string here anyway.

**Migration-sequence numbering:** Latest is `20260503_000009_scheduled_for_add.up.sql` (Phase 21). Phase 22 timestamp prefix should be ≥ `20260504` to maintain strictly-increasing ordering. Suggested `20260504_000010_jobs_tags_add.up.sql`.

`[VERIFIED: ls migrations/{sqlite,postgres}/ — last is _009 dated 20260503]`
`[VERIFIED: tests/schema_parity.rs:57 — TEXT-family branch]`
`[VERIFIED: tests/schema_parity.rs:283 — assert_eq!(normalize_type("TEXT"), "TEXT")]`

## Section 4 — Substring-Collision Algorithm

**Confirmed (HIGH confidence):** Per CONTEXT specifics §, the algorithm is **plain string `contains`**, not regex, with these semantics:

1. Operate on the **post-normalization, post-dedup, post-charset-validation** tag set across the WHOLE fleet (after the per-job loop completes).
2. Build a sorted (for determinism) `BTreeSet<String>` of unique tags AND a `HashMap<String, Vec<String>>` mapping `tag -> [job_names_that_use_it]`.
3. Double-loop with `i < j` over the sorted unique tags. For each pair `(t1, t2)` where `t1 != t2`:
   - If `t1.contains(&t2)` OR `t2.contains(&t1)`: emit one `ConfigError`.
   - Direction is symmetric (one pair per collision, not two — `back ↔ backup` is one error).
4. Identical tags across jobs (job A and B both have `backup`) are NOT collisions (not even iterated — `i < j` plus distinct values from the `BTreeSet` skips equal pairs).

**Reference algorithm (Rust):**

```rust
fn check_tag_substring_collision(jobs: &[JobConfig], path: &Path, errors: &mut Vec<ConfigError>) {
    use std::collections::{BTreeSet, HashMap};

    // Step 1: build {tag -> [jobs_using_it]} from the POST-NORMALIZATION view.
    // Use BTreeSet<String> (sorted) for the tag list to make iteration deterministic.
    let mut tag_to_jobs: HashMap<String, Vec<String>> = HashMap::new();
    for job in jobs {
        // Re-run normalization here OR (better) hold normalized-tags on the
        // job during validation. Planner picks the data flow.
        let normalized: BTreeSet<String> = job.tags.iter()
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty())  // already rejected, skip silently here
            .collect();
        for tag in normalized {
            tag_to_jobs.entry(tag).or_default().push(job.name.clone());
        }
    }
    // Stable sort of jobs per tag (for the `(+N more)` listing).
    for v in tag_to_jobs.values_mut() { v.sort(); v.dedup(); }

    // Step 2: collect sorted unique tags.
    let tags: Vec<&String> = {
        let mut v: Vec<&String> = tag_to_jobs.keys().collect();
        v.sort();
        v
    };

    // Step 3: O(n^2) pair check. n ≤ ~16*N_jobs at homelab scale → microseconds.
    for i in 0..tags.len() {
        for j in (i + 1)..tags.len() {
            let (a, b) = (tags[i], tags[j]);
            // Equal tags impossible here because BTreeSet uniqueness + i<j.
            if a.contains(b.as_str()) || b.contains(a.as_str()) {
                let jobs_a = &tag_to_jobs[a];
                let jobs_b = &tag_to_jobs[b];
                let preview_a = preview_jobs(jobs_a);  // up to 3 + (+N more)
                let preview_b = preview_jobs(jobs_b);
                errors.push(ConfigError {
                    file: path.into(),
                    line: 0,
                    col: 0,
                    message: format!(
                        "tag '{a}' (used by {preview_a}) is a substring of '{b}' (used by {preview_b}); rename or remove one to avoid SQL substring false-positives at filter time."
                    ),
                });
            }
        }
    }
}

fn preview_jobs(jobs: &[String]) -> String {
    if jobs.len() <= 3 {
        format!("'{}'", jobs.join("', '"))
    } else {
        format!("'{}', '{}', '{}' (+{} more)", jobs[0], jobs[1], jobs[2], jobs.len() - 3)
    }
}
```

**Complexity:** `O(T^2)` where `T` = unique tag count across fleet. At homelab scale (≤16 jobs × ≤16 tags = 256 max, typically 5×3 = 15) this is microseconds. The per-tag job list and the preview formatting are deterministic (sorted, deduped).

**Critical edge cases (test in `tests/v12_tags_validators.rs`):**

| Case | Expected |
|------|----------|
| `back` and `backup` in different jobs | One ConfigError per pair (D-03) |
| `back` and `backup` in the SAME job | Same one ConfigError (job listed once on each side) |
| `back` in 5 jobs, `backup` in 1 job | One ConfigError; preview shows `'a', 'b', 'c' (+2 more)` for `back`, `'d'` for `backup` |
| Two jobs both have `backup` only (no substring) | NO error (identical tags across jobs is allowed) |
| Three-way `bac`, `back`, `backup` | THREE ConfigErrors — pairs `(bac, back)`, `(bac, backup)`, `(back, backup)`. Operator must fix at least two to break all pairs. |

`[VERIFIED: CONTEXT specifics §, lines 530-537]`

## Section 5 — `DbRunDetail` Row-Mapping Site

**Confirmed (HIGH confidence):** The exact row-mapping site for `DbRunDetail` is at `src/db/queries.rs:1390-1454` in `get_run_by_id`. Both backends already construct `DbRunDetail` field-by-field from a JOIN'd SELECT.

### Current SELECT (lines 1391-1406):

```sql
-- SQLite (also Postgres modulo $1 vs ?1):
SELECT r.id, r.job_id, r.job_run_number, j.name AS job_name, r.status, r.trigger,
       r.start_time, r.end_time, r.duration_ms, r.exit_code, r.error_message,
       r.image_digest, r.config_hash, r.scheduled_for
FROM job_runs r
JOIN jobs j ON j.id = r.job_id
WHERE r.id = ?1
```

### Required edit (Phase 22):

Project `j.tags` into the SELECT (the JOIN already provides access to the `jobs` table):

```sql
SELECT r.id, r.job_id, r.job_run_number, j.name AS job_name, r.status, r.trigger,
       r.start_time, r.end_time, r.duration_ms, r.exit_code, r.error_message,
       r.image_digest, r.config_hash, r.scheduled_for,
       j.tags AS tags_json
FROM job_runs r
JOIN jobs j ON j.id = r.job_id
WHERE r.id = ?1
```

### Row-map deserialization (both arms — SQLite at L1414, Postgres at L1436):

```rust
Ok(row.map(|r| {
    let tags_json: String = r.get("tags_json");
    let tags: Vec<String> = serde_json::from_str(&tags_json)
        .unwrap_or_default();  // forgiving: corrupt JSON falls back to []; logs warn
    DbRunDetail {
        id: r.get("id"),
        job_id: r.get("job_id"),
        // ... existing fields ...
        scheduled_for: r.get("scheduled_for"),
        tags, // Phase 22 TAG-01 / WH-09
    }
}))
```

**Forgiving-deserialize rationale:** the column is `NOT NULL DEFAULT '[]'` so corruption is structurally impossible (every row has at minimum `'[]'` from the migration default); but if a future writer ever bugs and stores invalid JSON, the read path falls back to `Vec::new()` rather than panicking and breaking webhook delivery. Log via `tracing::warn!` if the planner picks tighter handling.

**`DbRunDetail` struct edit at `queries.rs:622-648`:** add field

```rust
/// Phase 22 TAG-01 / WH-09: tags from jobs.tags column (sorted-canonical
/// JSON). Empty Vec when the job has no tags; never None (column is
/// NOT NULL DEFAULT '[]').
pub tags: Vec<String>,
```

**`upsert_job` edit at `queries.rs:62-130`:** widen signature with `tags_json: &str` argument, bind in both INSERT VALUES list AND ON CONFLICT UPDATE SET clause. Both branches widen identically (only differ in `?N` vs `$N` placeholder syntax).

```rust
pub async fn upsert_job(
    pool: &DbPool,
    name: &str,
    schedule: &str,
    resolved_schedule: &str,
    job_type: &str,
    config_json: &str,
    config_hash: &str,
    timeout_secs: i64,
    tags_json: &str,  // NEW — Phase 22 TAG-01
) -> anyhow::Result<i64> { ... }
```

`[VERIFIED: src/db/queries.rs:62-130 (upsert_job), 622-648 (DbRunDetail struct), 1390-1454 (get_run_by_id row-map)]`

## Section 6 — WH-09 Webhook Payload Backfill

**Confirmed (HIGH confidence):** Single-line replacement at `src/webhooks/payload.rs:88` plus a test rename + rewrite.

### Field is already declared (no struct edit needed):

`src/webhooks/payload.rs:53` declares:
```rust
/// Empty `[]` until Phase 22 lights up real values. Schema-stable
/// — Phase 22 cutover does NOT break receivers.
pub tags: Vec<String>,
```

The Phase 22 task is to **update the comment** (remove "Empty `[]` until Phase 22") AND swap the runtime value source.

### Build-site edit (`payload.rs:88`):

```rust
// Before (line 88):
tags: vec![],

// After:
tags: run.tags.clone(),  // Phase 22 WH-09 / D-05
```

`run` here is the `&DbRunDetail` parameter at `payload.rs:68`. After the §5 edit, `run.tags: Vec<String>` is populated.

### Test rename + rewrite (`payload.rs:235-242`):

```rust
// REMOVE this entire test:
#[test]
fn payload_tags_empty_array_until_p22() {
    let event = fixture_event();
    let fctx = fixture_fctx();
    let run = fixture_run_detail(None, None);
    let p = WebhookPayload::build(&event, &fctx, &run, 1, "1.2.0");
    let s = serde_json::to_string(&p).unwrap();
    assert!(s.contains("\"tags\":[]"));
}

// REPLACE with:
#[test]
fn payload_tags_carries_real_values() {
    // Phase 22 WH-09 / D-06.5: the placeholder is gone; receivers see
    // real tag values from the jobs.tags column.
    let event = fixture_event();
    let fctx = fixture_fctx();
    // fixture_run_detail must be widened to take tags
    let run = fixture_run_detail_with_tags(
        None, None, vec!["backup".to_string(), "weekly".to_string()],
    );
    let p = WebhookPayload::build(&event, &fctx, &run, 1, "1.2.0");
    let s = serde_json::to_string(&p).unwrap();
    assert!(s.contains(r#""tags":["backup","weekly"]"#),
        "tags must round-trip into payload preserving sorted-canonical order: {s}");
}
```

**`fixture_run_detail` widening (`payload.rs:120`):** Add a `tags: Vec<String>` parameter or add a new `fixture_run_detail_with_tags` helper. Note: every existing test that calls `fixture_run_detail(None, None)` must also pass through (or default to `vec![]`) — `payload.rs:147, 155, 165, 195, 213, 228, 239, 249, 273` all use the helper. Recommendation: keep the existing 2-arg helper defaulting `tags: vec![]`, add a new 3-arg helper for the new test. Avoids touching seven existing tests.

`[VERIFIED: src/webhooks/payload.rs:53 (field decl), 88 (build site), 120-140 (fixture), 235-242 (test to rename)]`

## Common Pitfalls

### Pitfall 1: Forgetting to sort before `serde_json::to_string`

**What goes wrong:** Insert-order JSON makes the column value churn on re-uploads of the same TOML, breaks deterministic substring-collision iteration, and makes WH-09 payloads non-byte-stable across runs (HMAC compares would still pass since the receiver re-decodes, but log-grep gets noisy).

**Why it happens:** `Vec<String>` preserves insert order; `serde_json::to_string` emits in declaration order; nothing forces sort.

**How to avoid:** A single helper:
```rust
fn sort_canonical(mut tags: Vec<String>) -> Vec<String> {
    tags.sort();
    tags.dedup();
    tags
}
```
Apply right before `upsert_job` bind AND before substring-collision pass. Test fixtures assert `["backup","weekly"]` (alphabetical) regardless of input order.

**Warning signs:** A test like `assert!(json.contains(r#""tags":["weekly","backup"]"#))` passes locally but flakes in CI.

### Pitfall 2: Validator order drift

**What goes wrong:** If charset validation runs BEFORE normalization, the input `"Backup"` fails (capital `B` not in `[a-z0-9]`) — but TAG-03 says case-insensitive normalization should fix that to `"backup"`. The operator gets a confusing error.

**Why it happens:** Easy to write the validators in declaration order rather than D-04 spec order.

**How to avoid:** D-04 lock: **normalize → reject (charset + reserved) → dedup with WARN → per-job count cap → fleet pass**. Encode the order via test:
```rust
#[test]
fn capital_input_normalizes_then_passes_charset() {
    // input ["Backup"] should NOT produce a charset error
    let cfg = parse_config_with_tags(vec!["Backup"]);
    assert!(cfg.errors.is_empty(), "Backup should normalize to backup, charset OK");
    assert_eq!(cfg.jobs[0].tags, vec!["backup"]);
}
```

### Pitfall 3: HashMap iteration in error messages

**What goes wrong:** Sorting forgotten → tests grep for `"backup, weekly"` but get `"weekly, backup"` 50% of the time. Already encountered in `validate.rs:184` (Pitfall 2 comment).

**How to avoid:** Always `Vec::sort()` before `.join(", ")`. Pattern from `validate.rs:195, 308, 369`.

### Pitfall 4: Dropping the `until_p22` test rename

**What goes wrong:** Phase 22 ships, payload carries real values, but `payload_tags_empty_array_until_p22` still tests for empty array against a `fixture_run_detail` that defaults to `vec![]` — the test passes, but the test NAME lies. Future readers see "empty array until phase 22" in tree forever.

**How to avoid:** D-06.5 is a load-bearing acceptance criterion. The PR description should explicitly call out the rename.

### Pitfall 5: Postgres `IF NOT EXISTS` divergence

**What goes wrong:** Adding `IF NOT EXISTS` to the SQLite migration → SQLite errors at parse time (`ALTER TABLE ... ADD COLUMN` does not accept this clause).

**How to avoid:** Per-backend asymmetry is intentional and documented in the migration comment (see Phase 16 template comment). SQLite relies on sqlx's `_sqlx_migrations` ledger; Postgres adds belt-and-suspenders `IF NOT EXISTS`.

### Pitfall 6: Empty / whitespace-only tag silently passes

**What goes wrong:** PITFALLS.md §51 rule 3: `tags = [""]` and `tags = ["   "]` MUST reject — but if the validator is structured `normalized.iter().filter(|t| t.is_empty())` BEFORE checking, the empty case slides through silently.

**How to avoid:** Check `t.trim().is_empty()` BEFORE charset (charset would catch it too — `""` doesn't match `^[a-z0-9]` — but the error message should be specific: `"empty tag in job 'X'"`). Test cases T-V12-TAG-02 and T-V12-TAG-03.

## Code Examples

### Validator registration in `run_all_checks`

```rust
// src/config/validate.rs (extending L43-66)
pub fn run_all_checks(cfg: &Config, path: &Path, raw: &str, errors: &mut Vec<ConfigError>) {
    check_timezone(&cfg.server.timezone, path, errors);
    check_bind(&cfg.server.bind, path, errors);
    check_duplicate_job_names(&cfg.jobs, path, raw, errors);
    for job in &cfg.jobs {
        check_one_of_job_type(job, path, errors);
        check_cmd_only_on_docker_jobs(job, path, errors);
        check_network_mode(job, path, errors);
        check_schedule(job, path, errors);
        check_label_reserved_namespace(job, path, errors);
        check_labels_only_on_docker_jobs(job, /* ... */ );
        check_label_size_limits(job, path, errors);
        check_label_key_chars(job, path, errors);
        check_webhook_url(job, path, errors);
        check_webhook_block_completeness(job, path, errors);
        // Phase 22 / TAG-* — D-04 order: normalize → reject → dedup → cap.
        check_tag_charset_and_reserved(job, path, errors);   // TAG-03 + TAG-04
        check_tag_count_per_job(job, path, errors);          // D-08 — checks post-dedup
    }
    // Phase 22 / TAG-05 — fleet-level pass AFTER per-job loop (D-03).
    check_tag_substring_collision(&cfg.jobs, path, errors);
}
```

`[CITED: src/config/validate.rs:43-66]`

### Sorted-canonical helper (recommended placement: `src/config/mod.rs` or new `src/config/tags.rs`)

```rust
/// Phase 22 TAG-01: normalize a Vec<String> of tags to the canonical form
/// stored in `jobs.tags` and emitted in WH-09 payload `tags` field.
/// Lowercase + trim + sort + dedup. Empty entries are dropped (validator
/// already emitted ConfigError for them); this is the post-validation
/// canonical form.
pub fn normalize_tags(raw: &[String]) -> Vec<String> {
    let mut v: Vec<String> = raw.iter()
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    v.sort();
    v.dedup();
    v
}
```

### TAG-03 dedup-collapse WARN

```rust
fn check_tag_dedup_warn(job: &JobConfig) {
    let raw = &job.tags;
    if raw.is_empty() { return; }
    let normalized: Vec<String> = raw.iter().map(|t| t.trim().to_lowercase()).collect();
    let mut deduped = normalized.clone();
    deduped.sort();
    deduped.dedup();
    if deduped.len() < raw.len() {
        // Identify which raw inputs collapsed by grouping by canonical form.
        let mut groups: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for (raw_t, norm_t) in raw.iter().zip(normalized.iter()) {
            groups.entry(norm_t.clone()).or_default().push(raw_t.clone());
        }
        let collapses: Vec<String> = groups.iter()
            .filter(|(_, raws)| raws.len() > 1)
            .map(|(canon, raws)| format!("{:?} → {:?}", raws, vec![canon.clone()]))
            .collect();
        tracing::warn!(
            job = %job.name,
            "tags collapsed by case+whitespace normalization: {}",
            collapses.join("; ")
        );
    }
}
```

This emits exactly the WARN shape called out in CONTEXT specifics §:
> `WARN job 'nightly-backup': tags ["Backup", "backup ", "BACKUP"] collapsed to ["backup"] (case + whitespace normalization)`

`[CITED: CONTEXT.md specifics § lines 548-553]`

## State of the Art

This is an additive feature within a stable Rust backend. No "old vs new approach" diff applies — the libraries used are the same the project has been using throughout v1.0–v1.2.

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `tags: vec![]` placeholder in WH-09 payload | Real values from `jobs.tags` column via `DbRunDetail.tags` | Phase 22 (this phase) | WH-09 closes end-to-end; receivers see real tags. |

**Deprecated/outdated:** None. `serde_json`, `once_cell`, `regex`, `sqlx`, `tracing` are all current versions with no relevant deprecations.

## Assumptions Log

All claims in this research are either VERIFIED against in-tree files or CITED from CONTEXT.md. No `[ASSUMED]` knowledge is load-bearing.

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| — | (none) | — | All claims sourced from in-tree code or locked CONTEXT decisions. |

**Rationale:** Phase 22 is mechanically additive and CONTEXT.md is exhaustive. Every recommendation in this research is either (a) directly observed in source (queries.rs, validate.rs, payload.rs, migrations/, schema_parity.rs) or (b) reproduced from CONTEXT decisions. The planner does NOT need to confirm anything with the user before proceeding.

## Open Questions

1. **Storing tags JSON: where does sort happen?**
   - What we know: D-09 sub-bullet recommends sorted-canonical; planner discretion.
   - What's unclear: Does sort happen (a) at validator time (mutating the `Vec` on the JobConfig in-place), (b) at upsert_job bind site (sort-then-`to_string` just before binding), or (c) both?
   - Recommendation: (b) — sort in a `normalize_tags` helper called immediately before `upsert_job`. Keeps the `JobConfig` as a faithful representation of operator intent post-normalization but pre-canonical. Substring-collision pass also calls `normalize_tags` for consistency. The planner picks; document the choice in the `normalize_tags` doc-comment.

2. **Should `examples/cronduit.toml` get one or two tagged jobs?**
   - Planner discretion (CONTEXT.md). Recommendation: one — adds `tags = ["demo", "hello"]` to the existing hello-world job. Two would risk a substring-collision on a demo file that operators copy-paste-modify.

3. **Should `check_tag_dedup_warn` be a separate function or folded into `check_tag_charset_and_reserved`?**
   - Planner discretion. The WARN is `tracing::warn!`, not `errors.push(ConfigError)`, so it's structurally distinct from the rejection validators. Folding into the same function is fine; extracting is also fine. No correctness impact either way.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `serde_json` crate | TAG-02 round-trip; WH-09 payload | YES | 1.x | — |
| `once_cell` crate | TAG-04 charset regex | YES | 1.x | — |
| `regex` crate | TAG-04 charset regex | YES | 1.x | — |
| `tracing` crate | TAG-03 dedup WARN | YES | 0.1.x | — |
| `sqlx` (sqlite + postgres backends) | TAG-02 migration; queries widening | YES | 0.8.x | — |
| `testcontainers-modules` postgres | `tests/v12_tags_validators.rs` Postgres CI cell | YES | 0.15.x | — |
| `cargo-zigbuild` | (CI multi-arch only — no Phase 22 surface) | (CI runner) | n/a | — |
| `just` recipe runner | UAT recipes (D-11) | YES (existing 30+ recipes in justfile) | — | — |

**Missing dependencies with no fallback:** None.
**Missing dependencies with fallback:** None.

## Validation Architecture

> Per Nyquist convention. This section is consumed by `VALIDATION.md` derivation.

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` + `cargo nextest` (per CLAUDE.md) |
| Config file | `Cargo.toml` `[[test]]` entries (auto-discovered for `tests/*.rs`) |
| Quick run command | `cargo nextest run --test v12_tags_validators` |
| Full suite command | `cargo nextest run --all-features` |
| CI matrix | `linux/{amd64,arm64} × {SQLite, Postgres}` (existing convention) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TAG-01 | `JobConfig.tags` deserializes from TOML; `#[serde(default)]` for unset | unit | `cargo nextest run --test v12_tags_validators tags_deserialize_*` | NO — Wave 0 |
| TAG-01 (per-job only — NOT in defaults) | `[defaults] tags = [...]` in TOML must NOT compile-attach (no field on `DefaultsConfig`) | unit | `cargo nextest run --test v12_tags_validators tags_not_on_defaults` | NO — Wave 0 |
| TAG-02 | Schema parity green for `jobs.tags TEXT NOT NULL DEFAULT '[]'` | integration | `cargo nextest run --test schema_parity` (existing — green by construction) | YES — already passing |
| TAG-02 | TOML `tags = ["backup", "weekly"]` round-trips: TOML → DB → fetch → Vec<String> | integration | `cargo nextest run --test v12_tags_validators tags_roundtrip_persistence` | NO — Wave 0 |
| TAG-02 | Old jobs (pre-Phase-22 rows) default to `[]` on column add | integration | `cargo nextest run --test v12_tags_validators tags_default_empty_for_old_rows` | NO — Wave 0 |
| TAG-03 | `["Backup", "backup ", "BACKUP"]` collapse to `["backup"]` + emit WARN | unit | `cargo nextest run --test v12_tags_validators tags_normalization_collapse_warn` | NO — Wave 0 |
| TAG-03 | `tags = [""]` rejected (T-V12-TAG-02) | unit | `cargo nextest run --test v12_tags_validators tags_empty_string_rejected` | NO — Wave 0 |
| TAG-03 | `tags = ["  "]` (whitespace-only) rejected after trim (T-V12-TAG-03) | unit | `cargo nextest run --test v12_tags_validators tags_whitespace_only_rejected` | NO — Wave 0 |
| TAG-04 | Valid: `backup`, `daily`, `prod-east`, `cost_center_42` accepted (T-V12-TAG-04) | unit | `cargo nextest run --test v12_tags_validators tags_charset_valid_accepted` | NO — Wave 0 |
| TAG-04 | Invalid: `my tag`, `tag.with.dot`, `🎉`, `<script>`, `-leading`, 32-char-too-long rejected (T-V12-TAG-05) | unit | `cargo nextest run --test v12_tags_validators tags_charset_invalid_rejected` | NO — Wave 0 |
| TAG-04 | XSS regression: `<b>x</b>` rejected at validator; if it slipped through, askama auto-escapes (T-V12-TAG-06 — ASSERT REJECTED) | unit | `cargo nextest run --test v12_tags_validators tags_xss_input_rejected_by_charset` | NO — Wave 0 |
| TAG-04 | Reserved-name rejection: `cronduit`, `system`, `internal` (and case-variants like `Cronduit` — rejected post-normalization) | unit | `cargo nextest run --test v12_tags_validators tags_reserved_names_rejected` | NO — Wave 0 |
| TAG-05 | Substring collision `back ↔ backup` across jobs → one ConfigError per pair (T-V12-TAG-07) | unit | `cargo nextest run --test v12_tags_validators tags_substring_collision_one_error_per_pair` | NO — Wave 0 |
| TAG-05 | Substring collision in SAME job → one ConfigError (job listed once each side) | unit | `cargo nextest run --test v12_tags_validators tags_substring_collision_same_job` | NO — Wave 0 |
| TAG-05 | Identical tags across jobs (job A and B both have `backup`) → NOT an error | unit | `cargo nextest run --test v12_tags_validators tags_identical_across_jobs_no_error` | NO — Wave 0 |
| TAG-05 | Three-way `bac ⊂ back ⊂ backup` → THREE errors (all pairs) | unit | `cargo nextest run --test v12_tags_validators tags_substring_collision_triple` | NO — Wave 0 |
| D-08 | 17-tag job rejected: `[[jobs]] '<n>': has 17 tags; max is 16` | unit | `cargo nextest run --test v12_tags_validators tags_count_cap_16` | NO — Wave 0 |
| D-08 | 16-tag job accepted (boundary) | unit | `cargo nextest run --test v12_tags_validators tags_count_cap_16_accepted` | NO — Wave 0 |
| D-01 | `compute_config_hash` does NOT change when tags change | unit | `cargo nextest run hash::tests::hash_unchanged_on_tags_change` | NO — Wave 0 (extends `src/config/hash.rs::tests`) |
| D-02 | `serialize_config_json` does NOT include `tags` key | unit | `cargo nextest run --test v12_tags_validators tags_not_in_config_json_blob` | NO — Wave 0 |
| WH-09 | `WebhookPayload.tags` carries real values from `DbRunDetail.tags` (D-05/D-06.5) | unit | `cargo nextest run webhooks::payload::tests::payload_tags_carries_real_values` | NO — Wave 0 (rename of `payload_tags_empty_array_until_p22`) |
| WH-09 | Sorted-canonical order in payload: input `["weekly","backup"]` → payload `"tags":["backup","weekly"]` | unit | `cargo nextest run webhooks::payload::tests::payload_tags_sorted_canonical` | NO — Wave 0 |
| Schema-parity (TAG-02) | `tests/schema_parity.rs::sqlite_and_postgres_schemas_match_structurally` stays green | integration | `cargo nextest run --test schema_parity` | YES — green by construction (no edits to `normalize_type` whitelist needed; TEXT-family branch L57 covers the new column) |

### What Each Test Asserts (sketch)

- **`tags_roundtrip_persistence`** (Postgres + SQLite): write `JobConfig { tags: vec!["backup".into(), "weekly".into()], ... }` via `upsert_job`, fetch via `get_run_by_id`, assert `run.tags == vec!["backup", "weekly"]` (sorted-canonical).
- **`tags_default_empty_for_old_rows`** (Postgres + SQLite): start fresh DB → run all migrations through `_009_scheduled_for_add` only (skip `_010`) → insert a job → run `_010_jobs_tags_add` → SELECT `tags` from the inserted row → assert `'[]'`. (NOTE: `sqlx::migrate!` runs the whole suite; this scenario may need to be tested via migration-level assertion rather than runtime — planner adapts. Alternative: trust the `DEFAULT '[]'` clause is honored by both backends, drop this test, and rely on schema-parity for the column shape.)
- **`tags_normalization_collapse_warn`** (unit): use `tracing-test` or a custom subscriber to capture WARN events; configure `tags = ["Backup", "backup ", "BACKUP"]`; assert exactly one WARN line containing `'Backup'`, `'backup '`, `'BACKUP'`, `→ ["backup"]`.
- **`tags_substring_collision_one_error_per_pair`**: configure two jobs `A.tags = ["back"]`, `B.tags = ["backup"]`; assert exactly one `ConfigError` whose message contains both `'back'` and `'backup'` and both `'A'` and `'B'`.
- **`tags_count_cap_16`**: configure one job with 17 distinct valid tags; assert one `ConfigError` matching `has 17 tags; max is 16`.
- **`payload_tags_carries_real_values`**: build a `DbRunDetail` fixture with `tags: vec!["backup".into(), "weekly".into()]`; call `WebhookPayload::build`; serialize to JSON; assert `"tags":["backup","weekly"]` substring present.
- **`tags_not_in_config_json_blob`**: serialize a `JobConfig` with non-empty tags via the same path `serialize_config_json` uses; assert the resulting JSON does NOT contain `"tags"` (D-02).
- **`hash_unchanged_on_tags_change`** (extension to `hash.rs::tests`): build two `JobConfig` differing only in `tags`; assert `compute_config_hash(&a) == compute_config_hash(&b)` (D-01 — opposite of `hash_differs_on_labels_change` at `hash.rs:307`).

### Sampling Rate

- **Per task commit:** `cargo nextest run --test v12_tags_validators` (~5–10s on CI cell)
- **Per wave merge:** `cargo nextest run --all-features` (full suite, ~3–5min on CI cell)
- **Phase gate:** Full suite green across all 4 CI cells (`linux/{amd64,arm64} × {SQLite, Postgres}`) before `/gsd-verify-work`.

### Wave 0 Gaps

- [ ] `tests/v12_tags_validators.rs` — covers TAG-01..05 + D-08 + WH-09 round-trip persistence
- [ ] Test fixture helper for `DbRunDetail` with tags (extension to existing `fixture_run_detail` pattern in `payload.rs:120-140`)
- [ ] `src/config/hash.rs::tests` — add `hash_unchanged_on_tags_change` test
- [ ] `src/webhooks/payload.rs::tests` — RENAME `payload_tags_empty_array_until_p22` → `payload_tags_carries_real_values` AND add `payload_tags_sorted_canonical`
- [ ] No framework install needed — `cargo nextest`, `tracing-test` or equivalent already in tree

### Schema-Parity Green By Construction Reasoning

`tests/schema_parity.rs::normalize_type` whitelist (L57):
```rust
"TEXT" | "VARCHAR" | "CHARACTER VARYING" | "CHAR" | "CHARACTER" => "TEXT".to_string(),
```
Both SQLite (`PRAGMA table_info` → `"TEXT"`) and Postgres (`information_schema.data_type` → `"text"`) report TEXT-family for the new column. Both normalize to `"TEXT"`. The Column struct (L25-30) records `not_null: true` from `notnull != 0` (SQLite) and `nullable == "NO"` (Postgres). Both backends report `not_null = true` for `NOT NULL DEFAULT '[]'`. Default-value strings are NOT stored in the Column struct, so the `DEFAULT '[]'` clause has no parity surface area to test.

**Therefore: zero test edits needed for schema-parity.** The Phase 16 `image_digest_add` migration set this precedent and Phase 22 follows it identically.

`[VERIFIED: tests/schema_parity.rs:57, 25-30, 96-100, 156-160, 283]`

## Security Domain

> `security_enforcement` not explicitly disabled in config; included by default.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | NO | Phase 22 changes config + DB schema only — no auth surface. |
| V3 Session Management | NO | No new sessions. |
| V4 Access Control | NO | Tags are organizational, not authorization. |
| V5 Input Validation | YES | TAG-04 charset regex + reserved-name list reject malformed input at config-load. |
| V6 Cryptography | NO | No new crypto surface (HMAC signing of WH-09 payload is unchanged from Phase 18). |
| V7 Error Handling & Logging | YES | TAG-03 WARN line; ConfigError shape mirrors Phase 17 D-01 (no secret leaks since tags are operator-set non-secrets). |
| V8 Data Protection | NO | Tags are non-sensitive operator metadata; not subject to secret-handling discipline. |
| V13 API & Web Service | YES | WH-09 webhook payload field carries operator-set strings; askama_web auto-escape mitigates dashboard XSS at P23 (P22 just delivers raw bytes via JSON, which is safe). |

### Known Threat Patterns for tags-in-payload

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Tag containing `<script>` etc. (XSS into dashboard at P23, or downstream consumers) | Tampering | TAG-04 charset `^[a-z0-9][a-z0-9_-]{0,30}$` REJECTS all HTML-significant chars at config-load. Defense-in-depth: askama_web auto-escapes templated output (Phase 23). PITFALLS.md §52 (line 953) confirms HIGH confidence on auto-escape. |
| Tag containing JSON-significant chars (`"`, `\`, `\n`) | Tampering | TAG-04 charset rejects. Even if it slipped through, `serde_json::to_string` correctly escapes. |
| Substring collision causing SQL filter false-positives in P23 (`tags LIKE '%"' || ?tag || '"%'`) | Repudiation / Spoofing of filter results | TAG-05 collision rejection at config-load — operator gets a clear error pointing at the offending pair before bad state reaches the DB. The `SQL filter substring false-positives at filter time` is the exact rationale in the D-03 error message. |
| Reserved-tag impersonation (`cronduit` tag confusing operators about cronduit-internal vs operator-set) | Spoofing | RESERVED_TAGS list rejects `cronduit`, `system`, `internal` post-normalization. No future-reserved prefix needed (CONTEXT specifics — finite list is sufficient). |
| Resource exhaustion via thousands of tags per job | DoS | D-08 hard cap of 16 tags per job rejected at config-load. Per-fleet DoS requires per-job × N_jobs; at homelab scale (≤16 jobs × 16 tags = 256) the substring-collision pass remains microseconds. |
| Tag-only edit triggering unwanted FCTX "config changed since last success" signal | Functional drift | D-01: `compute_config_hash` excludes tags. Tag rename doesn't dilute the docker-execution-change signal. |
| `${VAR}` env-var interpolation injecting unsafe content into a tag | Tampering | The whole-file textual interpolation at `interpolate.rs::interpolate` runs BEFORE the validator. Post-interpolation, the charset regex catches any `${`/`}`/`/` chars from an unset env var. Same defense as `check_label_key_chars` at `validate.rs:359`. |

### Project Constraints (from CLAUDE.md)

| Constraint | Phase 22 honors? | How |
|------------|-----------------|-----|
| Tech stack locked: Rust + sqlx + bollard + askama_web + croner + TOML + rustls | YES | No stack changes; uses in-tree `serde_json` + `once_cell` + `regex`. |
| `cargo tree -i openssl-sys` empty | YES | No new deps added; rustls invariant unchanged. |
| Mermaid-only diagrams | YES | RESEARCH.md uses mermaid (§ Architecture); D-13 [informational] reaffirms across all phase artifacts. |
| PR-only branch state — no direct main commits | YES | D-12 [informational]; planner produces feature-branch PRs. |
| UAT via `just` recipes | YES | D-10 + D-11 lock 3 new `uat-tags-*` recipes following `recipe-calls-recipe` pattern. |
| Maintainer validates UAT (Claude does NOT mark passed) | YES | D-15 [informational]; `22-HUMAN-UAT.md` autonomous=false. |
| Tag and version match — `Cargo.toml` stays at 1.2.0 | YES | D-16 [informational]; rc.3 cut is Phase 23. |
| Web UI matches `design/DESIGN_SYSTEM.md` | N/A | Phase 22 ships zero UI; chips are Phase 23. |
| `examples/cronduit.toml` operator-readable | YES | Optional addition of `tags = [...]` line on demo job. |

## Sources

### Primary (HIGH confidence — VERIFIED in this session)

- `src/config/mod.rs:114-165` — JobConfig struct shape; field add site
- `src/config/validate.rs:10-20` — Lazy<Regex> + RESERVED list idiom
- `src/config/validate.rs:43-66` — `run_all_checks` per-job loop registration site
- `src/config/validate.rs:185-206` — `check_label_reserved_namespace` (template for charset+reserved validator)
- `src/config/validate.rs:298-335` — `check_label_size_limits` (template for count cap)
- `src/config/validate.rs:359-380` — `check_label_key_chars` (template for charset regex)
- `src/config/validate.rs:612-654` — `check_duplicate_job_names` (template for fleet-level pass)
- `src/config/hash.rs:16, 50` — `compute_config_hash` (D-01 exclusion site + comment style)
- `src/config/interpolate.rs:34` — Lazy<Regex> idiom (verbatim)
- `src/db/queries.rs:62-130` — `upsert_job` widening site
- `src/db/queries.rs:622-648` — `DbRunDetail` struct (extension point)
- `src/db/queries.rs:1390-1454` — `get_run_by_id` row-mapping site (both backends)
- `src/webhooks/payload.rs:53` — `tags` field declaration (already in tree, just comment update)
- `src/webhooks/payload.rs:88` — `tags: vec![]` placeholder (one-line replace)
- `src/webhooks/payload.rs:235-242` — `payload_tags_empty_array_until_p22` test (rename target)
- `migrations/sqlite/20260427_000005_image_digest_add.up.sql` — exact migration template (SQLite)
- `migrations/postgres/20260427_000005_image_digest_add.up.sql` — exact migration template (Postgres + IF NOT EXISTS)
- `tests/schema_parity.rs:25-30, 41-62, 96-100, 156-160, 283` — TEXT-family normalization green-by-construction proof
- `Cargo.toml` — `serde_json = "1"`, `once_cell = "1"`, `regex = "1"` all VERIFIED present
- `justfile` lines 267-1124 — existing `uat-*` recipe family (template for D-11)
- `.planning/REQUIREMENTS.md` lines 113-127 — TAG-01..05 verbatim + WH-09 line 53
- `.planning/research/PITFALLS.md` lines 880-1020 — Pitfall 51/52 (tags) + T-V12-TAG-01..11 anchors
- `.planning/STATE.md` line 136 — Tagging row of v1.2 locked decisions
- `.planning/phases/22-job-tagging-schema-validators/22-CONTEXT.md` — 16 D-decisions (locked)
- `.planning/phases/17-custom-docker-labels-seed-001/17-RESEARCH.md` lines 231-402 — validator family shape (precedent)
- `.planning/phases/18-webhook-payload-state-filter-coalescing/18-PATTERNS.md` lines 69, 76 — `tags: Vec<String> = vec![]` until-Phase-22 breadcrumb

### Secondary (MEDIUM confidence)

- (none required — Phase 22 is mechanically additive)

### Tertiary (LOW confidence)

- (none — no claims at this confidence level)

## Metadata

**Confidence breakdown:**

- Standard Stack: HIGH — `Cargo.toml` verified; idiom verified at `validate.rs:10-15`.
- Architecture: HIGH — every file path and line number verified by reading the file in this session.
- Pitfalls: HIGH — five of six pitfalls map to existing in-tree comments (`validate.rs:184` "HashMap iter is non-deterministic"; `payload.rs:11` "Pitfall B"; etc.).
- Migration shape: HIGH — Phase 16 `image_digest_add` is byte-for-byte the template.
- Substring algorithm: HIGH — CONTEXT specifics § (lines 530-537) is explicit.
- DbRunDetail row-map: HIGH — `queries.rs:1390-1454` read in full; both backend arms identical structure.

**Research date:** 2026-05-04
**Valid until:** 2026-06-04 (30 days; stable scope, locked stack)
