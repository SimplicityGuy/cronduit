# Phase 5: Config Reload & `@random` Resolver - Context

**Gathered:** 2026-04-11
**Status:** Ready for planning

<domain>
## Phase Boundary

Production-grade config reload via SIGHUP / `POST /api/reload` / debounced file-watch, the slot-based `@random` algorithm with feasibility checks, and the UI surfaces that make resolved schedules visible. All three reload triggers funnel through the existing `SchedulerCmd` channel (Phase 3, D-08). The `@random` resolver plugs into the existing sync engine placeholder at `sync.rs:96`.

</domain>

<decisions>
## Implementation Decisions

### Reload feedback UX
- **D-01:** Successful reload shows an HTMX toast notification with a diff summary ("Config reloaded: 2 jobs added, 1 updated, 1 disabled"). Toast auto-dismisses after 5s. Settings page updates to show last reload timestamp and summary.
- **D-02:** Failed reload (parse error, invalid TOML) shows a red error toast with parse error summary ("Reload failed: invalid TOML at line 42"). Error toasts persist until dismissed (no auto-dismiss). Full error in structured log. Settings page shows last failed attempt.
- **D-03:** `POST /api/reload` response includes full diff summary: `{"status": "ok", "added": 2, "updated": 1, "disabled": 1, "unchanged": 5}`. On failure: `{"status": "error", "message": "..."}`.

### @random re-roll cadence
- **D-04:** `@random` schedules are resolved only at sync time (startup or config reload). No daily re-roll. Once resolved, the value stays fixed until the next restart or config reload.
- **D-05:** If a job's raw `schedule` field is unchanged across a reload (same `config_hash`), its `resolved_schedule` is preserved from the DB. Re-randomization only happens when the schedule field changes, the job is newly added, or explicitly re-rolled via API/UI.
- **D-06:** Operators can force re-randomization of a specific job via `POST /api/jobs/{id}/reroll` endpoint AND a "Re-roll" button on the Job Detail page. Clears and re-resolves without editing config.

### @random UI presentation
- **D-07:** Job Detail page shows raw and resolved schedule inline: "Schedule: `@random 14 * * *` (resolved to `14 17 * * *`)". Resolved value in parentheses or muted secondary style.
- **D-08:** Dashboard job list shows a small `@random` badge/pill next to the schedule column in the terminal-green accent color. Subtle but scannable at a glance.

### Reload concurrency
- **D-09:** All three trigger sources (file watcher, SIGHUP, API) send through the same `SchedulerCmd` channel. File watcher has a 500ms debounce. SIGHUP and API execute immediately, but if a reload is already in-flight, the new request waits for completion then checks if another reload is needed (coalesce, not reject).
- **D-10:** File watcher is enabled by default. Operators can disable with `[server].watch_config = false`. Logs a startup message: "Watching cronduit.toml for changes".

### Claude's Discretion
- Exact debounce implementation (tokio::time::sleep vs notify's built-in debounce)
- Internal locking mechanism for reload serialization
- Slot algorithm implementation details for `random_min_gap` enforcement
- Re-roll button placement and styling on Job Detail page
- Toast animation and positioning (consistent with existing "Run Now" toast from Phase 3)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Config reload requirements
- `.planning/REQUIREMENTS.md` — RELOAD-01 through RELOAD-07: full acceptance criteria for config reload behavior
- `.planning/ROADMAP.md` Phase 5 section — success criteria, pitfalls addressed (Pitfall 6, 9, 16)

### @random scheduling requirements
- `.planning/REQUIREMENTS.md` — RAND-01 through RAND-06: full acceptance criteria for @random resolver and UI display

### Existing sync engine (Phase 5 integration point)
- `src/scheduler/sync.rs` line 96-97 — placeholder for @random resolution: `let resolved_schedule = job.schedule.clone();`
- `src/scheduler/mod.rs` — `SchedulerCmd` enum and `tokio::select!` loop (add `Reload` variant)

### Signal handling
- `src/shutdown.rs` — existing SIGINT/SIGTERM handlers (extend with SIGHUP)

### Web UI patterns
- `src/web/handlers/api.rs` — `run_now()` handler pattern (CSRF + channel command + HX-Trigger toast)
- `src/web/handlers/settings.rs` — `last_reload: "never"` placeholder to replace with real tracking

### Design system
- `design/DESIGN_SYSTEM.md` — terminal-green brand, badge/pill styling for @random indicator

### Config structure
- `src/config/mod.rs` — `ServerConfig` (needs `watch_config` field), `DefaultsConfig` (already has `random_min_gap`)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **Sync engine** (`src/scheduler/sync.rs`): `sync_config_to_db()` handles upsert via `config_hash`. Phase 5 plugs `@random` resolver into the placeholder at line 96.
- **Scheduler command channel** (`SchedulerCmd` enum): Add `Reload` variant following the `RunNow` pattern.
- **CSRF double-submit** (Phase 3, D-11): Already protects all POST endpoints. Apply to `/api/reload` and `/api/jobs/{id}/reroll`.
- **Toast pattern** (`run_now` handler): HX-Trigger response header for HTMX toast notifications. Reuse for reload success/error toasts.
- **Config parsing** (`src/config/mod.rs`): `parse_and_validate()` reusable for reload — parse new file, validate, then apply.

### Established Patterns
- **Channel-based commands**: Web handlers send `SchedulerCmd` through `cmd_tx`, scheduler receives in `tokio::select!` loop. Phase 5 follows this exactly.
- **Signal handling**: `shutdown.rs` spawns a task listening for signals. SIGHUP handler follows the same spawn-task pattern but sends `Reload` instead of triggering shutdown.
- **Binary heap scheduling**: Scheduler uses `BinaryHeap<Reverse<...>>` for next-fire tracking. After reload, rebuild the heap with new/updated jobs.

### Integration Points
- `src/scheduler/mod.rs` — Add 4th `tokio::select!` branch for `Reload` command
- `src/shutdown.rs` — Add SIGHUP listener task that sends `SchedulerCmd::Reload`
- `src/web/mod.rs` — Add routes: `POST /api/reload`, `POST /api/jobs/{id}/reroll`
- `src/web/handlers/api.rs` — Add `reload()` and `reroll()` handlers
- `src/web/handlers/settings.rs` — Replace hardcoded `last_reload: "never"` with actual tracking
- `src/config/mod.rs` — Add `watch_config: bool` to `ServerConfig`
- `AppState` — Add reload timestamp tracking (e.g., `last_reload: Arc<Mutex<Option<DateTime>>>`)
- `Cargo.toml` — Add `notify` crate for file watching

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches. Follow existing patterns established in Phases 1-4.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 05-config-reload-random-resolver*
*Context gathered: 2026-04-11*
