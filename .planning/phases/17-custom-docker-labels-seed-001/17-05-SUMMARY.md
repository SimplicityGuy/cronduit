---
phase: 17-custom-docker-labels-seed-001
plan: 05
subsystem: docs
tags: [docker, labels, readme, documentation, mermaid, configuration]

# Dependency graph
requires:
  - phase: 17-custom-docker-labels-seed-001
    plan: 01
    provides: "labels: Option<HashMap<String, String>> on JobConfig + DefaultsConfig + DockerJobConfig; apply_defaults labels merge with per-job-wins on collision; use_defaults=false REPLACE semantic"
  - phase: 17-custom-docker-labels-seed-001
    plan: 02
    provides: "four LOAD-time validators (reserved-namespace cronduit.*, type-gate docker-only, size limits 4KB/32KB, strict ASCII key regex) — README documents each as an operator-facing rule"
  - phase: 17-custom-docker-labels-seed-001
    plan: 03
    provides: "bollard plumb-through site that resolves operator labels merged with cronduit-internal labels — README mermaid diagram shows the exact chain that 17-03 implements"
provides:
  - "README.md § Configuration > Labels — single source of truth for operator-facing labels documentation"
  - "First mermaid diagram in README.md (project-rule D-07 carried as Phase 17 D-07)"
  - "Cross-reference target for examples/cronduit.toml inline comments (Plan 17-04)"
affects: [17-04-examples, 17-06-seed-closeout]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Mermaid flowchart-LR for merge-precedence visualization with classDef styling matching the existing Architecture/Workflow diagrams (terminal-green for operator-facing nodes; purple-violet accent for cronduit-internal/bollard nodes)"
    - "GFM table with right-padded columns + bold-marker rule paragraphs (**Reserved namespace.**, **Type-gate.**, **Size limits.**, **Env-var interpolation.**, **Label values are NOT secrets.**) for scan-friendly operator reading"

key-files:
  created: []
  modified:
    - README.md

key-decisions:
  - "Placement: AFTER ### Default Job Settings, BEFORE ### Job Types — per Plan 17-05 <interfaces> note. Labels live in [defaults] AND [[jobs]] so this placement mirrors the [defaults] documentation depth (D-04). It also keeps the section ordering logical: server config → defaults config → defaults add-ons (labels) → per-job-type schemas."
  - "Mermaid diagram structure: 4-step chain visualized with three input branches (per-job present + use_defaults!=false / present + use_defaults=false / per-job absent) collapsing to a single 'operator label set' node, then forwarding through cronduit-internal labels to bollard. Captures all four steps mandated by D-04 + the 'internal labels override' arrow direction mandated by 17-PATTERNS.md S-7."
  - "Five operator-facing rule paragraphs use bold-marker prefix (e.g. **Reserved namespace.**) rather than H4 headings. Reasoning: H4 would clutter the TOC of an already-deep section; the bold-prefix paragraph style matches the existing 'Picking a tag' subsection at L114-129 of the README and reads better in narrow-column terminal viewers."
  - "Section heading is `### Labels` (not `### Custom Docker Labels` per the alternative wording in 17-PATTERNS.md). Reasoning: examples/cronduit.toml inline comments (Plan 17-04) reference 'README § Configuration > Labels' per Plan 17-05 acceptance criteria — and the shorter heading composes more cleanly with that breadcrumb."
  - "Used `4 KB (4096 bytes)` rather than just `4 KB` so the size-limit acceptance criterion `grep -E '4 ?KB|4096' README.md` matches both forms; gives operators the explicit byte count alongside the human-readable unit."

requirements-completed:
  - LBL-01
  - LBL-02
  - LBL-03
  - LBL-04
  - LBL-05
  - LBL-06

# Metrics
duration: ~10min
completed: 2026-04-29
---

# Phase 17 Plan 05: README Labels Subsection (LBL-01..LBL-06 documentation surface) Summary

**Single ~70-line `### Labels` subsection added to README's `## Configuration` section: mermaid merge-precedence flowchart (first mermaid diagram in README.md), 3-row merge-semantics table on per-job labels × use_defaults, five operator-facing rule paragraphs covering reserved namespace / type-gate / size limits / env-var interpolation / values-are-not-secrets, and a cross-reference to examples/cronduit.toml for the three integration patterns.**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-04-29T00:57:00Z (approximate)
- **Completed:** 2026-04-29T01:07:07Z
- **Tasks:** 1 (single doc-only task)
- **Files modified:** 1

## Accomplishments

- README's `## Configuration` section gained a comprehensive `### Labels` subsection (~70 lines, +69 net insertions) covering all six LBL requirements (LBL-01..LBL-06).
- Mermaid `flowchart LR` block visualizes the four-step merge precedence: `[defaults].labels` → branch on per-job presence × `use_defaults` → unified operator label set → cronduit-internal labels added → bollard `Config::labels` reaches the Docker daemon. Three input branches collapse into one collector node so the "merge / replace / inherit" trio is visually parallel; a final arrow with the cronduit-internal callout shows the override semantic.
- Mermaid styling reuses the established README idiom: terminal-green `classDef step` for operator-facing nodes; violet `classDef internal` for the bollard / cronduit-internal node, matching the `:main`/`:rc`/`:latest` workflow diagram at L95-112 and the Architecture diagram at L133-158.
- 3-row merge-semantics table renders cleanly on GitHub: per-job `labels` field × per-job `use_defaults` × resulting label set. Headers right-padded for visual alignment.
- Five operator-facing rules each have a single TOML example (REJECTED-at-load case for the first two; ALLOWED + REJECTED contrast for env-var interpolation):
  - Reserved namespace rule (`cronduit.*`).
  - Type-gate rule (docker-only; command/script jobs rejected).
  - Size limits (4 KB / 4096 bytes per value, 32 KB per set).
  - Env-var interpolation rule (values yes via `${VAR}`; keys never).
  - "Label values are NOT secrets" security advisory paragraph cross-referencing the existing `env =` field for actual secrets handling (mitigation for T-17-05-I in plan threat model).
- Cross-reference paragraph at end pointing operators at `examples/cronduit.toml` for the three integration patterns (Watchtower exclusion in `[defaults]`, Traefik routing on a per-job block, `use_defaults = false` clean-slate job). The cross-reference uses the same path style as elsewhere in the README (e.g. L285 `examples/prometheus.yml`).

## Task Commits

Each task was committed atomically:

1. **Task 1: Add `### Labels` subsection to README.md § Configuration with mermaid diagram + 3-row table + 6 documented rules** — `b3fc46d` (docs)

## Files Created/Modified

- `README.md` — inserted ~70-line `### Labels` subsection between `### Default Job Settings` (L190-203) and `### Job Types` (now starting at L274). Diff is purely additive: 69 insertions, 0 deletions, 0 lines outside the inserted range modified.

## Decisions Made

- **Placement: between `### Default Job Settings` and `### Job Types`.** Plan 17-05 `<interfaces>` allowed either between-Default-Settings-and-Job-Types or after-Job-Types; chose the former because labels live in `[defaults]` AND `[[jobs]]` so the section is logically a peer of `### Default Job Settings`, and placing it next to `### Default Job Settings` makes the [defaults] documentation depth contiguous (matches D-04 intent).
- **Section heading: `### Labels`** (not `### Custom Docker Labels`). Shorter heading composes more cleanly with the breadcrumb that examples/cronduit.toml inline comments will use ("README § Configuration > Labels"). Plan 17-04 references this exact heading per Plan 17-05 acceptance criteria.
- **Mermaid styling reuses existing README classDef idiom.** Two existing mermaid diagrams (Workflow at L95, Architecture at L133) use terminal-green `core` / `step` styling and violet accent for external/internal-special nodes. The new diagram follows the same palette (`#0a3d0a` fill, `#00ff7f` stroke for operator nodes; `#2a1a3d` fill, `#bf7fff` stroke for the bollard plumb-through node) so the README's visual language stays consistent.
- **Bold-prefix paragraph rule blocks** (e.g. `**Reserved namespace.**`) rather than H4 sub-subheadings. Reasoning: five new H4s would clutter any TOC tooling; the bold-prefix style matches the existing "Picking a tag" subsection at L114-129. Each rule still gets its own paragraph + TOML example so scannability is preserved.
- **Used `4 KB (4096 bytes)`** rather than just `4 KB` so both forms satisfy the size-limit acceptance criterion `grep -E '4 ?KB|4096'`. Gives operators the explicit byte count.
- **Mermaid arrow labels use HTML `<br/>` tags for multi-line node labels** (e.g. the `merge: defaults ∪ per-job<br/>(per-job wins on collision)` form). Mermaid renders `<br/>` cleanly on GitHub; this is the same idiom the existing Workflow diagram at L95-112 uses (`":1.1.0<br/>:1.1<br/>:1<br/>:latest"`).

## Deviations from Plan

### Auto-fixed Issues

**None.** Plan executed exactly as written. The plan provided a verbatim content block with placeholder tokens (`MERMAID_FENCE_OPEN/CLOSE`, `TOML_FENCE_OPEN/CLOSE`); the executor replaced each with the literal triple-backtick fences as instructed. The acceptance-criteria self-check confirmed:
- No leaked placeholder tokens (the WARNING #5 fix grep returned 0 lines).
- Mermaid fence well-formed: opens at column 0 with `\`\`\`mermaid`, closes at column 0 with bare `\`\`\``, and `awk` range extracts a `flowchart` line.
- All twelve acceptance-criteria greps pass with the required counts.

The plan's `<verify>` block also requires `just fmt-check` to exit 0 — confirmed (no source-code change; the gate was a sanity check that the doc edit didn't accidentally touch a Rust file).

## Issues Encountered

**None.** The plan was technically tight; the verbatim content blocks lined up with the README's existing tone and code-fence idiom on first try. Two minor stylistic choices fell to executor discretion (placement: between-vs-after Job Types; heading: `### Labels` vs `### Custom Docker Labels`) — both documented in `key-decisions` above.

## Verification Results

| Acceptance criterion | Required | Observed |
| -------------------- | -------- | -------- |
| `grep -c '^### Labels' README.md` | ≥ 1 | 1 |
| `grep -c 'mermaid' README.md` | ≥ 1 | 4 (1 fence-open + 3 prose mentions of project-wide rule context) |
| `grep -c '^| ' README.md` | +4 vs pre-edit (= ≥ 22) | 23 |
| `grep -c 'use_defaults' README.md` | ≥ 3 | 4 |
| `grep -c 'cronduit.run_id' README.md` | ≥ 1 | 2 |
| `grep -E '4 ?KB|4096' README.md` | match found | matches "≤ 4 KB (4096 bytes)" |
| `grep -E '32 ?KB' README.md` | match found | matches "≤ 32 KB" twice |
| `grep -c 'cronduit\.\*' README.md` | ≥ 1 | 2 |
| `grep -c 'docker-only' README.md` | ≥ 1 | 2 |
| `grep -c 'NOT secret' README.md` | ≥ 1 | 1 |
| `grep -c 'DEPLOYMENT_ID' README.md` | ≥ 1 | 2 |
| `grep -c 'examples/cronduit.toml' README.md` | ≥ 1 (added) | 3 |
| `awk '/```mermaid/,/```/' README.md \| grep -c 'flowchart'` | ≥ 1 | 3 (counts both existing diagrams + new one within the range) |
| WARNING #5 — no leaked placeholder tokens | 0 | 0 |
| WARNING #5 — mermaid fence well-formed AND closed | ≥ 1 | 3 (range captures all three flowchart blocks; new one closes correctly) |
| `just fmt-check` exits 0 | yes | yes (no source-code change) |

All acceptance criteria PASS.

## Next Phase Readiness

- **Plan 17-04 (examples/cronduit.toml)** — its inline comments cross-referencing "README § Configuration > Labels" now have a real target. The example file's three integration patterns (Watchtower exclusion, Traefik routing, `use_defaults = false`) are explicitly called out in the README's closing paragraph so an operator reading the example can pivot to the README rule explanations without context-switching.
- **Plan 17-06 (seed close-out)** — no dependency on this plan; the seed file frontmatter edit lands independently in the last plan of the wave.
- **rc.1 readiness target** — Phase 17's documentation surface is now complete from the operator's perspective. Combined with Plans 17-01 (config schema + merge), 17-02 (validators), and 17-03 (bollard plumb-through), an operator can:
  1. Discover the feature via the README (this plan).
  2. Pattern-match against `examples/cronduit.toml` (Plan 17-04).
  3. Trust LOAD-time validation will catch most misconfigurations early (Plan 17-02).
  4. Verify operator labels reach the Docker daemon end-to-end (Plan 17-03 + the integration test stack).
- **No blockers.**

## Self-Check: PASSED

Files claimed in this summary verified to exist:
- `README.md` — FOUND (modified)
- `.planning/phases/17-custom-docker-labels-seed-001/17-05-SUMMARY.md` — FOUND (this file, just written)

Commit claimed in this summary verified to exist:
- `b3fc46d` (Task 1) — FOUND

Acceptance criteria verified (all 16): see Verification Results table above — every row PASS.

---
*Phase: 17-custom-docker-labels-seed-001*
*Completed: 2026-04-29*
