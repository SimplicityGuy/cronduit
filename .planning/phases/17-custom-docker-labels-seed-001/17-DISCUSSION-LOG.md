# Phase 17: Custom Docker Labels (SEED-001) - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in `17-CONTEXT.md` — this log preserves the alternatives considered.

**Date:** 2026-04-28
**Phase:** 17-custom-docker-labels-seed-001
**Areas discussed:** Validator error UX & aggregation, Label key char-validation policy, Examples + README content, Seed lifecycle + summary cross-refs

---

## Validator error UX & aggregation

| Option | Description | Selected |
|--------|-------------|----------|
| Match existing pattern (line:0, col:0) | Stay consistent with `check_cmd_only_on_docker_jobs` and the rest of `validate.rs`. Each violation is one `ConfigError` per job, message lists ALL offending keys in a single line. No new `toml::Spanned` plumbing. Cheapest, most consistent. | ✓ |
| One error per offending key (still 0,0) | Aggregation OFF for reserved-namespace + size violations: each bad key = its own `ConfigError` row. Operator sees a flat list `cronduit.foo: reserved; cronduit.bar: reserved` instead of one combined message. Still no real line:col. | |
| Add real line:col via `toml::Spanned` | Wrap the labels HashMap value in `toml::Spanned` so each bad key reports its actual TOML line. Better UX (jump-to-line in editors) but adds `Spanned` plumbing across `DefaultsConfig` + `JobConfig` and downstream consumers. Scope creep risk. | |

**User's choice:** Match existing pattern (line:0, col:0)
**Notes:** D-01 in CONTEXT.md captures this. The aggregate-not-fail-fast posture is already in place via `parse_and_validate`'s `Vec<ConfigError>` accumulator and the per-job validator loop at `validate.rs:88-92`.

---

## Label key char-validation policy

| Option | Description | Selected |
|--------|-------------|----------|
| Strict: ASCII `[a-zA-Z0-9._-]`, no leading `.` or `-` | Validator rejects non-ASCII letters, spaces, slashes, etc. at load time with a clear cronduit error. Operator never sees a confusing dockerd rejection at runtime. Matches Docker's documented convention; aligns with the SEED-001 'load-time, not runtime' philosophy. | ✓ |
| Length-only (already covered by size limits) | Skip char validation. The 4 KB / 32 KB checks (LBL-06) are the only key-side enforcement. Anything dockerd rejects surfaces as a runtime container-create failure. Smallest scope; matches 'pass through what TOML accepts.' | |
| Permissive: only reject ASCII control chars + empty key | Middle ground. Reject `\0`-`\x1F` and empty string at load (these are obvious bugs); allow everything else through to bollard. Avoids over-policing exotic-but-valid label conventions some operators might use; still catches accidents. | |

**User's choice:** Strict ASCII validation
**Notes:** D-02 in CONTEXT.md captures this as a fourth load-time validator (in addition to LBL-03 reserved-namespace, LBL-04 type-gate, LBL-06 size limits). Regex shape: `^[a-zA-Z0-9_][a-zA-Z0-9._-]*$`. This also implicitly enforces LBL-05's "keys are NOT interpolated" intent — leftover `${` / `}` characters are rejected. Residual gap (fully-resolved-to-safe-chars interpolated keys) flagged in `<deferred>`.

---

## Examples + README content

### Where labels live in `examples/cronduit.toml`

| Option | Description | Selected |
|--------|-------------|----------|
| Add labels to existing hello-world jobs (defaults + override demo) | `[defaults].labels` demonstrates Watchtower exclusion. `hello-world` inherits. `hello-world-container` adds Traefik annotations on top to show per-job MERGE. One more comment block + one or two label lines per existing job. No new jobs. | |
| Add a dedicated labels-demo job | Net-new docker job e.g. `traefik-routed-canary` that exists purely to showcase labels. Adds clutter to a quickstart but reads more clearly. Doesn't reuse existing demos. | |
| Both: hello-world for merge + new job for `use_defaults = false` replace | Maximum coverage at cost of file size. `hello-world` demonstrates merge semantics; a new job (e.g., `isolated-batch`) sets `use_defaults = false` and its own minimal label set to show whole-section replace. Three integration patterns visible: Watchtower (defaults), Traefik (per-job merge), backup-tool (replace). | ✓ |

### README depth

| Option | Description | Selected |
|--------|-------------|----------|
| Brief subsection (one para + small example) | 5-8 lines summarizing: what it does, where it goes (defaults + per-job), reserved `cronduit.*` namespace, size limits as bullets. Defers detailed merge semantics to the inline-config comments. Easiest to keep fresh. | |
| Full subsection with merge-semantics table + size-limits + reserved-namespace + type-gate | Dedicated mini-section in `## Configuration` mirroring the v1.0 `[defaults]` documentation depth. Includes a small mermaid diagram of the merge precedence (operator labels then internal labels override) and one worked example. ~30-40 lines. | ✓ |

**User's choices:** Both (existing + new job) | Full subsection with mermaid diagram
**Notes:** D-03 + D-04 in CONTEXT.md capture these. The README's mermaid diagram is the visual proof of the merge-precedence chain — load-bearing per project rule D-14 (mermaid only). The new job is provisionally named `isolated-batch`; planner picks final name.

---

## Seed lifecycle + summary cross-refs

| Option | Description | Selected |
|--------|-------------|----------|
| Update status to `realized` + add `realized_in: phase-17, milestone v1.2` field | Within Phase 17's last plan, edit `.planning/seeds/SEED-001-custom-docker-labels.md` frontmatter: `status: dormant` → `realized`; add `realized_in: phase-17, milestone v1.2` (and optionally `realized_date`). Seed file stays in place as historical record. Establishes the project pattern for future realized seeds. Cross-reference seed path in `17-SUMMARY.md`. | ✓ |
| Move file to `.planning/seeds/realized/SEED-001-...md` and rewrite frontmatter | Physical move parallel to how completed phases are archived. Makes the dormant/realized split visible at a glance via directory. More invasive (any external reference to the path breaks). Probably overkill for a first realized seed. | |
| No formal close — just cross-reference seed path in `17-SUMMARY.md` and PR description | Leave the seed file untouched. Phase artifacts cite it. The 'is this seed shipped?' question is answered by checking REQUIREMENTS.md and the milestone close-out audit, not by the seed file itself. Lowest-ceremony option. | |

**User's choice:** Update status to `realized` + add `realized_in` / `milestone` / `realized_date` fields
**Notes:** D-05 in CONTEXT.md captures this. Establishes the project's first realized-seed pattern. Edit happens in the LAST plan of Phase 17 (so the seed is closed only after every other deliverable is in place). Physical move to `seeds/realized/` was rejected as premature.

---

## Claude's Discretion

The user did not ask about, and CONTEXT.md leaves to the planner:

- Plan count and grouping within Phase 17 (suggested 4-6 atomic plans).
- Validator function names (suggested: `check_label_reserved_namespace`,
  `check_labels_only_on_docker_jobs`, `check_label_size_limits`,
  `check_label_key_chars`).
- Whether the four checks live in one combined function or four separate
  functions.
- `once_cell::sync::Lazy<Regex>` vs hand-rolled char-by-char match for the
  D-02 key-char check.
- Whether `apply_defaults` extension lives inside the existing function or
  in a new helper.
- Integration test naming (suggested: `tests/v12_labels_*.rs`).
- Whether to produce `17-HUMAN-UAT.md`.
- Whether to add a fail-on-empty-string-value check (default
  recommendation: skip).

## Deferred Ideas

Captured in `17-CONTEXT.md` `<deferred>`:

- Display operator labels in the Web UI run-detail / job-detail page.
- Substring-after-interpolation key gap (interpolated keys that resolve to
  safe chars).
- Generalizing the labels validator stack to non-docker label-equivalents
  (systemd unit annotations, log tag emission).
- Label-based metric labels (Prometheus cardinality).
- Label-based webhook routing keys.
- `cronduit.*` namespace expansion (e.g., `cronduit.job_run_number`,
  `cronduit.image_digest`).
- `bans.skip` deny.toml entries for transitive duplicates.
- Empty-string label values rejection.
- Physical move of realized seed files to `.planning/seeds/realized/`.
