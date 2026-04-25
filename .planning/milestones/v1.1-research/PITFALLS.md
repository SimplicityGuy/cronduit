# Cronduit v1.1 — Pitfalls Research

**Dimension:** Integration Pitfalls (subsequent milestone, not greenfield)
**Milestone:** v1.1 "Operator Quality of Life"
**Researched:** 2026-04-14
**Confidence:** HIGH — every pitfall is grounded in a direct read of the shipped v1.0.1 source tree. Line numbers and file paths included where nontrivial.

> This document adapts the default `research-project/PITFALLS.md` template. The standard template is greenfield-oriented ("what domain mistakes should we avoid?"); for v1.1 the interesting pitfalls are **integration pitfalls** — where newly added v1.1 code meets the existing v1.0.1 code surface. Each pitfall below is anchored to a specific file + line in the current tree, with a prevention strategy that can be transformed into a named test case by REQUIREMENTS.md.
>
> v1.0's domain pitfalls (`auto_remove` race, `container:<name>` target-gone, log back-pressure, orphan reconciliation, unauthenticated UI + docker.sock) are already mitigated in the shipped code; this document references them rather than re-flagging them. Where a v1.1 feature re-enters one of those hazard zones, the pitfall below calls it out explicitly as a **regression risk**.

---

## How to Read This File

Each feature section has the pitfalls ranked **Critical → Moderate → Minor** for that feature. Every pitfall has:

- **Where:** the file(s) and (where relevant) line numbers the pitfall touches
- **What goes wrong:** the failure mode in plain language
- **Why:** the specific v1.0 code shape or external ecosystem bug that causes it
- **Prevention:** an actionable fix — ideally a named test case or a specific code pattern
- **Test case name (T-xxx):** a stable identifier REQUIREMENTS.md can reference

A pitfall marked with 🔗 **v1.0-P#N** indicates it touches a hazard zone already documented in `.planning/milestones/v1.0-research/PITFALLS.md` — fixing it must not re-open that v1.0 pitfall.

---

## Feature 1 — Stop a Running Job (new `stopped` status)

### 1.1 CRITICAL — Cancellation-token identity collision (per-run vs shutdown)

**Where:** `src/scheduler/mod.rs` L98, L122, L166, L215 — every spawn site creates `let child_cancel = self.cancel.child_token();`. `src/scheduler/command.rs` L127–L140 and `src/scheduler/docker.rs` L338–L358 both match on `cancel.cancelled()` and return `RunStatus::Shutdown`. `src/scheduler/run.rs` L238–L244 maps `Shutdown → "cancelled"`.

**What goes wrong:** Today there is **one single path** from a cancelled token to a finalized status string: `Shutdown → "cancelled"`. If v1.1 adds `SchedulerCmd::Stop { run_id }` that simply calls `.cancel()` on a per-run child token, the executor has no way to distinguish "operator clicked Stop" from "SIGTERM arrived" — both paths reach the same `cancel.cancelled()` arm and both produce `status="cancelled"`. The user-visible effect: the Stop button writes `cancelled` rows that are indistinguishable from graceful-shutdown rows, the `/metrics` counter `cronduit_runs_total{status="stopped"}` stays flat forever, and the UI run-history badge shows the wrong label.

**Why:** `CancellationToken` carries no payload. Both `self.cancel` (graceful shutdown root) and every `child_token()` derived from it fire on `.cancelled()` with zero context. A naive `SchedulerCmd::Stop` handler that cancels a stored child token is **observationally identical to the shutdown path** from inside the executor.

**Prevention:** Adopt ARCHITECTURE.md §3.1 "Option A" — per-run `Arc<AtomicU8> stop_reason` set by the scheduler **before** calling `.cancel()`. The executor's cancelled-branch reads the reason **after** it fires and returns `RunStatus::Stopped { reason: Operator }` or `RunStatus::Shutdown` based on the atomic. Never infer the cause of cancellation from the token identity.

**Test case:**
- **T-V11-STOP-01:** Spawn a long-running command job, set `stop_reason = Operator`, cancel, assert `status="stopped"` in DB and `RunStatus::Stopped` returned from executor.
- **T-V11-STOP-02:** Same setup but instead cancel the **root** `self.cancel`; assert `status="cancelled"` is still produced (no regression for shutdown path).
- **T-V11-STOP-03:** Cancel with `stop_reason = Operator` **before** spawn completes (pre-insert); assert behavior is safe and the row either never exists or exists as `stopped`, never as `running`.

**Severity:** CRITICAL — breaks the entire feature signal.

---

### 1.2 CRITICAL — Stop-vs-natural-completion race (already called out in ARCHITECTURE.md §5.1)

**Where:** `src/scheduler/mod.rs::SchedulerLoop::run` — the main `tokio::select!` arm at L143 (`join_set.join_next()`) runs in the **same event loop** as the `cmd_rx.recv()` arm at L162. Without a dedicated per-run control map, a `SchedulerCmd::Stop { run_id: 42 }` arriving at T=0 can race a `JoinNext → RunResult { run_id: 42, status: "success" }` arriving at T=+1μs.

**What goes wrong:** Three bad outcomes are possible:

1. Stop handler cancels the token of an already-finished task (harmless but a race-condition-y API shape).
2. Stop handler overwrites a `success` / `failed` DB row to `stopped` — a **lie** about what actually happened.
3. The executor's `cancel.cancelled()` branch fires **after** the task has already moved past the `tokio::select!` and is waiting on `finalize_run`'s DB write, producing an inconsistent dual-write.

**Why:** The existing executor `tokio::select!` does not have a deterministic ordering between "child exited" and "cancel fired" — whichever arm yields first wins. Once one arm wins, the other is structurally ignored by `select!`. But the scheduler loop has no visibility into which arm won, so a Stop issued at the same wall-clock moment as natural exit is a TOCTOU.

**Prevention:** Pin the ordering in **the scheduler loop**, not in the executor. Concretely:

1. Every `run_job` spawn registers a `RunControl` into `running_handles: HashMap<i64, RunControl>` before the spawn returns (use a startup barrier).
2. `join_set.join_next() → RunResult` **removes** the entry from `running_handles` atomically before returning control to the main loop.
3. `SchedulerCmd::Stop { run_id }` acquires the map entry **once** — if present, sets `stop_reason = Operator` then cancels; if absent, returns `StopResult::AlreadyFinished { final_status }` by reading `job_runs` directly. **No DB write from the Stop handler ever**; only the executor's cancel branch writes `stopped`.
4. The executor's `stop_reason` read is the **single** authoritative source for "operator stopped this run"; any `success/failed/timeout/error` return path **wins** over a racing Stop because the executor never consults `stop_reason` on those paths.

**Test cases:**
- **T-V11-STOP-04:** Use `tokio::time::pause` to spawn a job that exits at T+1ms; send Stop at T+0. Assert the **natural** status wins (no `stopped` overwrite). Run 1000 iterations to catch order-of-operations bugs.
- **T-V11-STOP-05:** Stop for an unknown `run_id` → API handler returns 404; no DB touch; `running_handles` unchanged.
- **T-V11-STOP-06:** Stop for a `run_id` that completed 5 minutes ago → API handler returns `AlreadyFinished { final_status: "success" }`; no DB touch.

**Severity:** CRITICAL. This is the highest-risk integration pitfall in v1.1.

---

### 1.3 CRITICAL — `FEATURES.md` proposes `kill_on_drop(true)` for command/script — that is a REGRESSION

**Where:** `.planning/research/FEATURES.md` L93 literally says: *"the spawned process is killed via the existing tokio cancellation token + `kill_on_drop(true)` pattern. No process-group walk."*

**What goes wrong:** The current v1.0 code is **strictly better** than the proposed v1.1 pattern. `src/scheduler/command.rs` L203 spawns with `.process_group(0)` and L150–L167 kills the entire process group via `libc::kill(-pid, SIGKILL)`. Same for `src/scheduler/script.rs` L89. This kills **all grandchildren** of a shell pipeline like `sh -c 'curl … | tee log.txt'`. Switching to `kill_on_drop(true)` would:

1. Only kill the direct child (the `sh`), leaving `curl` and `tee` as orphans adopted by PID 1 (inside the container, that's `cronduit` itself — the parent).
2. Produce zombie processes on long-running shell pipelines — exactly the "zombie processes on local jobs" pitfall the question flagged.
3. Rely on `Drop` timing which is **not** deterministic when the child future is inside a `JoinSet`.

Also: `kill_on_drop(true)` must be set on the `Command` **before** spawn. You cannot retrofit it onto an existing `Child`. The existing code structure cannot absorb this change without rewriting both executors.

**Prevention:**
1. **Do NOT adopt `kill_on_drop(true)` for command/script executors.** The existing `.process_group(0)` + `libc::kill(-pid, SIGKILL)` pattern is already correct; the Stop feature just needs a distinct cancel path that reaches the existing `kill_process_group` call with a different `stop_reason`.
2. **Update FEATURES.md L93** to match the actual implementation pattern before phase planning begins. The architecture doc (§3.1) and the code both already use process-group kill — FEATURES.md is the outlier.
3. Document in code comments that the existing process-group kill also handles shell-pipeline grandchildren — prevents a future refactor from silently adopting `kill_on_drop`.

**Test case:**
- **T-V11-STOP-07:** Run a command `sh -c 'sleep 120 | cat | cat'`; issue Stop; assert **all three** of `sh`, `cat`, `cat` are reaped (check via `/proc/<pid>` inspection inside a test container, or use `ps --ppid` in an integration test that mounts `/proc`).
- **T-V11-STOP-08:** Same test against a script job body that launches a background process (`(sleep 300 &); echo started`); assert the background sleep is also killed by the process-group walk.

**Severity:** CRITICAL (latent regression — correct today, would be broken by FEATURES.md proposal).

---

### 1.4 MODERATE — Bollard `kill_container` race with natural completion

**Where:** `src/scheduler/docker.rs` L265–L358 — the `tokio::select!` over `wait_container`, `sleep(timeout)`, and `cancel.cancelled()`.

**What goes wrong:** Operator clicks Stop at T=0. Scheduler cancels per-run token. `cancel.cancelled()` arm fires and calls `docker.stop_container(t=10)`. Between the cancel firing and the `stop_container` request reaching the daemon, the container naturally exits. `stop_container` returns either:

- `BollardError::DockerResponseServerError { status_code: 304, message: "Container already stopped" }` — the expected case; we should treat it as success.
- `BollardError::DockerResponseServerError { status_code: 404, message: "No such container" }` — possible if the post-drain remove fired first from another code path (shouldn't happen today because `maybe_cleanup_container` runs **after** the select, but worth testing).
- Network error mid-request — genuine error.

The first case is **not** a failure but today's `cancel.cancelled()` arm calls `.await.ok()`-style on `stop_container` (L341) and ignores the result entirely. That's fine for graceful shutdown but the **Stop feature** needs to confirm the container was reaped before writing `status="stopped"`, otherwise you can get `stopped` rows whose containers are still running.

**Why:** The v1.0 cancel path assumes the container MUST be stopped because the whole process is exiting. The v1.1 Stop path needs stronger guarantees because the process continues. The daemon-side race is documented in moby#8441 (referenced in v1.0 PITFALLS.md Pitfall #3) and historical bollard issues where `stop_container` on an already-exited container returns 304.

**Prevention:**
1. On operator-initiated stop, after `stop_container` returns, **inspect the container** to confirm state transitioned to `exited` or `dead`. If `stop_container` returned 304/404, treat as success (container already reaped). Only a network/daemon error should surface as an executor `Error`.
2. Log the 304/404 case at `debug` level with a `cronduit.docker.stop_raced_natural_exit` target so operators can distinguish benign races in post-mortems.
3. **Do not re-introduce `auto_remove=true`.** The existing `maybe_cleanup_container` runs after the select and must remain the single removal point (🔗 **v1.0-P#3 `auto_remove` race**).

**Test cases:**
- **T-V11-STOP-09:** Stop a docker job whose container has just exited (race window <10ms); assert `status="stopped"`, `exit_code` is captured if available, no orphan left behind.
- **T-V11-STOP-10:** Stop a docker job whose daemon returns transport error on `stop_container`; assert `status="error"` with a distinct `error_message` (not `"stopped"`).
- **T-V11-STOP-11:** Verify `cargo tree -i openssl-sys` stays empty after adding the new Stop path (no new crate dep should pull OpenSSL).

**Severity:** MODERATE.

---

### 1.5 MODERATE — `stopped` status vs orphan reconciliation (§5.5 — already covered, pre-fixed)

**Where:** `src/scheduler/docker_orphan.rs::mark_run_orphaned` L114–L142.

**What goes wrong in theory:** Cronduit is killed mid-stop (operator clicks Stop, cronduit crashes before `finalize_run` writes `stopped`). Container is left with label `cronduit.run_id=42`; on next startup, orphan reconciler finds it, stops/removes it, and overwrites the DB row's status to `error` / `orphaned at restart`. The run was **explicitly stopped**, not orphaned.

**Why it is NOT a pitfall today:** `mark_run_orphaned` already includes `AND status = 'running'` in its WHERE clause (L120, L131). If the row has been finalized to `stopped` by the time reconciliation runs, the UPDATE is a no-op. The worry is inverted: the pitfall is only real if the **Stop path finalizes AFTER** the cancel fires, leaving the row briefly at `running` when cronduit crashes.

**Prevention:**
1. The Stop handler in the scheduler loop **must not** touch the DB. Only the executor's finalize path writes `stopped`. This keeps the atomicity story identical to the existing graceful-shutdown path.
2. Order of operations inside the executor on operator-stop: `kill container/process` → `wait for child/container exit` → `finalize_run(status="stopped")` → scheduler-loop removes `running_handles[run_id]`. If cronduit crashes at **any** step before `finalize_run`, the row stays at `running` and orphan reconciliation on next boot produces `error`/`orphaned at restart`. **This is acceptable** — a crashed stop becomes an orphan from the operator's perspective.
3. Make the `status = 'running'` guard a test invariant so future refactors don't drop it.

**Test cases:**
- **T-V11-STOP-12:** Pre-seed a `job_runs` row with `status='stopped'`, run `reconcile_orphans` against a matching container; assert row stays `stopped` (the existing guard catches this, but lock the invariant in v1.1).
- **T-V11-STOP-13:** Pre-seed a row with `status='cancelled'`; reconcile; assert row stays `cancelled`.
- **T-V11-STOP-14:** Pre-seed with `status='running'`; reconcile; assert row transitions to `error` (existing behavior preserved).

**Severity:** MODERATE (covered by existing guard; test lock prevents regression).

---

### 1.6 MODERATE — `classify_failure_reason` bounded-cardinality label break

**Where:** `src/scheduler/run.rs::classify_failure_reason` L298–L313. Metric: `cronduit_run_failures_total{reason}`.

**What goes wrong:** v1.0's run.rs L270 logs a `cronduit_run_failures_total` counter only when `status_str != "success"`. Adding `stopped` as a new status will, by default, flow through the `match status` branch at L299 and land in `_ => FailureReason::Unknown`, polluting the `reason="unknown"` bucket with every operator-stop. That corrupts the signal Prometheus alerts currently key off of.

Also relevant: the `metrics` facade + `metrics-exporter-prometheus` setup eagerly describes label values at boot (see v1.0 STACK.md). If a new label value `stopped` on `cronduit_runs_total{status}` is not pre-declared in the describe step, Prometheus scrapers may see intermittent label creation — tolerable in practice but worth double-checking.

**Why:** v1.0 explicitly chose bounded-cardinality enums (v1.0 Phase 6 decision) and any new status must be pre-declared; silently falling into `Unknown` also violates the "every failure is classified" discipline.

**Prevention:**
1. **Do NOT count operator-stopped runs in `cronduit_run_failures_total`.** A stopped run is a user action, not a failure. Exclude `status == "stopped"` from the failure-counter branch at `run.rs` L270.
2. **Do** add `stopped` to the `cronduit_runs_total{status}` counter — increment with `status="stopped"`. Ensure the exporter's describe step lists `stopped` as a possible value alongside `success/failed/timeout/error/cancelled`.
3. Optional: add a new metric `cronduit_runs_stopped_total{job}` (no labels beyond `job`) so operators can alert on "too many operator-stops" if they want. Keep it behind a documented decision; not required for v1.1.

**Test cases:**
- **T-V11-STOP-15:** Stop a running job; scrape `/metrics`; assert `cronduit_runs_total{status="stopped"}` incremented and `cronduit_run_failures_total` unchanged.
- **T-V11-STOP-16:** Enumerate the describe step and assert `stopped` is in the declared set for `cronduit_runs_total`.

**Severity:** MODERATE.

---

### 1.7 MINOR — Permission / authorization surface (documentation, not code)

**Where:** `THREAT_MODEL.md` at repo root.

**What goes wrong:** v1 has no auth. Anyone who can reach `:8080` can now kill any running job. This is **not a regression** — anyone who can reach `:8080` can already click "Run Now" on any job, and Run Now has a larger blast radius than Stop. But the threat model currently enumerates Run Now explicitly; Stop must be added to the same enumeration so an operator reading the threat model doesn't feel blindsided.

**Prevention:** Add a one-paragraph entry to `THREAT_MODEL.md` under the "Untrusted Client" section: *"An attacker with LAN access to the Cronduit UI can stop any running job. This is strictly less dangerous than the existing Run Now capability and does not expand the documented blast radius."*

**Test case:** N/A — documentation only. Phase-plan checklist item.

**Severity:** MINOR.

---

## Feature 2 — Log Line Ordering (live→static transition)

### 2.1 CRITICAL — Broadcast-before-insert means SSE id-less events cannot dedupe

**Where:** `src/scheduler/run.rs::log_writer_task` L320–L350. Key lines:

```rust
for line in &batch {
    let _ = broadcast_tx.send(line.clone());     // L334-L336: broadcast FIRST
}
...
if let Err(e) = insert_log_batch(&pool, run_id, &tuples).await { ... }   // L341: insert SECOND
```

And `src/scheduler/log_pipeline.rs::LogLine` L22–L29 has only `{stream, ts, line}` — **no id field**.

**What goes wrong:** ARCHITECTURE.md §3.3 proposes adding `id: Option<i64>` to `LogLine` and using it for SSE-backfill dedupe. But the current write path **broadcasts before insert**. At the moment a line reaches the SSE subscriber, the DB row does not yet exist — so no id is available. The architecture's proposed fix requires inverting this ordering: insert first, then broadcast with the assigned id.

Inverting the order has a second-order effect: if the DB insert is slow, SSE subscribers see a visible lag that doesn't exist today. The current broadcast-before-insert is a deliberate latency optimization for live-tail UX.

**Why:** `insert_log_batch` (`src/db/queries.rs` L365) uses a multi-row `INSERT` bulk statement without `RETURNING`. SQLite's `RETURNING` on bulk inserts returns **one row per inserted row** but requires changing the insert shape. Postgres supports it out of the box.

**Prevention:** Two options, pick one and commit:

**Option A (recommended) — Insert-then-broadcast.**
1. Change `insert_log_batch` to `INSERT … RETURNING id` on both backends, returning `Vec<i64>` matching the input slice order.
2. `log_writer_task` inserts the batch, populates `line.id = Some(returned_id)` on each line, THEN broadcasts.
3. Measure insert latency before/after — if above ~50ms on SQLite for 64-line batches, reduce batch size or keep the broadcast path concurrent but tag every broadcast line with a monotonic-per-run counter that does NOT collide with future DB ids (see Option B).

**Option B — Two-id scheme.**
1. Add a monotonic `seq: u64` to `LogLine`, assigned by the log writer in receive order. Do NOT pretend it's the DB id.
2. Broadcast carries `seq`; DB insert also writes `seq` into a new column.
3. Backfill query returns `(seq, stream, ts, line)`; JS dedupes by `seq`. The `job_logs.id` primary key stays as-is.

**Option A is simpler and matches the architecture doc; Option B avoids the latency hit but requires a schema change.** Recommendation: Option A, measure, fall back to Option B only if latency regression is user-visible.

**Test case:**
- **T-V11-LOG-01:** Produce a burst of 500 log lines; subscribe to SSE and concurrently paginate the static log viewer; assert every line appears exactly once (no duplicates, no gaps) in the SSE-then-static merge using `id`-based dedupe.
- **T-V11-LOG-02:** Measure log insert latency on SQLite before/after the Option A change; assert p95 stays under 50ms for a 64-line batch.

**Severity:** CRITICAL — blocks the entire §3.3 architecture plan unless resolved.

---

### 2.2 CRITICAL — Live→static swap target collision

**Where:** `templates/pages/run_detail.html` L64–L82 (per ARCHITECTURE.md §2 row), the `#log-lines` div that hosts both the live SSE stream and the static partial.

**What goes wrong:** When the SSE `run_complete` event fires today, the client swaps a static partial into the container. But the live SSE stream may still be mid-flight: the last 1–5 lines written just before `close()` are in the broadcast channel's buffer, not yet consumed, and arrive on the SSE socket **after** the static partial has been swapped in. The HTMX swap replaces `#log-lines` with the DB-backed full log, and the trailing SSE events then append inside the new static container — producing duplicates of the last handful of lines.

**Why:** HTMX OOB (out-of-band) swaps and `hx-swap="innerHTML"` target the DOM by selector. The SSE handler writes to `#log-lines` via its own append logic. The static swap writes to `#log-lines` via HTMX. The two writers have no ordering guarantee because SSE is a separate network stream from the HTMX request that fetched the static partial.

**Prevention:**
1. **Use id-based dedupe (Feature 2.1 Option A).** After the static swap lands, late-arriving SSE events whose id is `<= max_backfill_id` are dropped client-side. The `static_log_viewer.html` partial must carry `data-max-id="{{ max_id }}"` on the container so the SSE listener can read it.
2. **Alternatively**, close the SSE stream **before** fetching the static partial: listen for `run_complete`, call `sse.close()`, THEN fire `hx-get` for the static partial. Guarantees ordering but adds a blank-frame visual glitch on slow connections. Prefer id-based dedupe.
3. **Test with a log-flush timing probe:** spawn a job whose final 10 lines are emitted in the last 100ms before exit; assert no duplicates in the DOM after the run-complete swap.

**Test cases:**
- **T-V11-LOG-03:** Browser-level test (or hand-rolled Playwright) — job completes, SSE `run_complete` fires, static partial swaps in, assert no line appears twice.
- **T-V11-LOG-04:** Same test with artificial 200ms SSE delivery lag; assert dedupe still holds.

**Severity:** CRITICAL — the feature title says "log lines rendering out of order"; this is the canonical failure mode.

---

### 2.3 MODERATE — `broadcast::Sender` ring-buffer lag races static backfill

**Where:** `src/web/handlers/sse.rs` L48–L54. `tokio::sync::broadcast::error::RecvError::Lagged(n)` is handled today by emitting a `[skipped N lines -- reload page for full log]` marker.

**What goes wrong:** The broadcast channel has capacity 256 (`run.rs` L101). A subscriber that's slow for even 500ms during a log burst (network hiccup, browser hidden tab) gets `Lagged(n)` and loses `n` lines. Today's behavior surfaces a marker string. With the v1.1 static-backfill flow, the correct recovery is **different**: on `Lagged`, the client should stop the SSE stream, re-fetch the static partial (which now contains the missed lines from the DB), and **not** attach a fresh SSE subscription unless the run is still running.

Also: today's marker says *"reload page for full log"* — that's a manual step. v1.1 can do better.

**Prevention:**
1. Change the SSE handler's `Lagged` branch to emit a distinct `log_lag` event (not the current `log_line` with embedded HTML).
2. Client HTMX handler for `log_lag` fetches `/partials/runs/{id}/static-log` (the existing static-log partial handler at `run_detail.rs::static_log_partial`) and swaps it in, automatically recovering the missed lines.
3. Only emit the fallback "reload" marker if the static partial fetch fails.

**Test cases:**
- **T-V11-LOG-05:** Force a broadcast lag (stall a subscriber); assert the `log_lag` event fires and client auto-recovers via static fetch.
- **T-V11-LOG-06:** Assert the client never gets stuck in a lag-recovery loop (recovery path must be idempotent and must not re-subscribe to SSE if the run has since completed).

**Severity:** MODERATE — improves an existing signal rather than fixing a crash.

---

### 2.4 MINOR — Wall-clock timestamps are untrustworthy for ordering

**Where:** `src/scheduler/log_pipeline.rs::make_log_line` L178–L184 — `ts: chrono::Utc::now().to_rfc3339()`.

**What goes wrong:** Naive implementations use `ts` as the ordering key for merge-dedupe. Two lines captured in the **same millisecond** from a log burst get identical timestamps, so any dedupe-by-`(ts, line)` is broken. SSE reconnection adds to the hazard: a reconnected stream does not restart timestamps.

**Why:** RFC3339 strings are not monotonic and cannot be compared as strict-less-than for ordering.

**Prevention:** All ordering and dedupe must use the `id` (or `seq` per Feature 2.1 Option B). The timestamp stays in the UI as a display value only; it is never the dedupe key. Document this in a code comment on `LogLine`.

**Test case:**
- **T-V11-LOG-07:** Unit test that produces 100 lines in a tight loop; assert none share a `(ts, line)` pair in the output; assert every `id` is unique.

**Severity:** MINOR.

---

## Feature 3 — "Error Getting Logs" Transient

### 3.1 CRITICAL — Handler fires before `runs` row is committed (TOCTOU)

**Where:** `src/web/handlers/run_detail.rs::run_detail` L140–L144 queries `get_run_by_id`; the scheduler path is `src/scheduler/run.rs::run_job` L75–L90 which inserts the row FIRST, then dispatches. Combined with `src/scheduler/mod.rs::SchedulerCmd::RunNow` at L164–L187.

**What goes wrong:** Operator clicks "Run Now" in the dashboard. The API handler sends `SchedulerCmd::RunNow { job_id }` on an mpsc channel and returns immediately with a toast. The dashboard then auto-refreshes (HTMX polling). The new run appears in the list. Operator clicks into it. The run_detail handler queries `get_run_by_id(run_id)` — but the dashboard saw a stale copy from a previous poll, and the actual run_id from the most recent Run Now has not yet been inserted because the scheduler loop hasn't scheduled the task yet. Result: `Ok(None)` → 404.

This is the classic TOCTOU: dashboard sees "run 42", user navigates, database doesn't have "run 42" yet.

**Why:** The scheduler loop is single-threaded. Between `cmd_rx.recv()` at L162 and `join_set.spawn(run_job(…))` at L167, time passes. Inside `run_job`, `insert_running_run` at `run.rs` L76 is the commit point. A fast user clicking in under ~5ms can win the race.

**Prevention:**
1. **Insert the running row on the API thread** before returning from the `run_now` handler. This guarantees the row exists at the moment the dashboard ever sees the id. Pass the pre-inserted `run_id` in the `SchedulerCmd::RunNow` message (`SchedulerCmd::RunNow { job_id, run_id }`), and adjust `run_job` to accept an optional pre-inserted id instead of always calling `insert_running_run`.
2. Alternatively, **never expose a run_id to the client until the row is committed.** Make the `run_now` handler synchronous on the DB write — the handler awaits a `oneshot::Sender<i64>` reply from the scheduler loop carrying the `run_id`. The toast then includes the id and the dashboard can safely reference it.
3. As a defensive backstop, `run_detail` on `Ok(None)` should respond with a **retry-friendly 404** that the client transforms into a "still starting, please wait" HTMX partial with `hx-trigger="load delay:500ms"` instead of an error.

**Recommendation:** Option 1 (insert on the API thread) is the cleanest. The run row is trivial (`{job_id, status='running', trigger, start_time}`) and can be written from the web process without touching the scheduler mpsc path at all.

**Test cases:**
- **T-V11-LOG-08:** Rapid-click "Run Now" then immediately navigate to the run detail; assert no 404 in 1000 iterations (use `tokio::time::pause` + deterministic ordering).
- **T-V11-LOG-09:** `run_detail` on a non-existent run_id still returns 404 (the fix must not mask genuine 404s).

**Severity:** CRITICAL — root cause of the reported "error getting logs" bug.

---

### 3.2 MODERATE — `job_logs` empty at t=0, handler treats "no logs yet" as error

**Where:** `src/web/handlers/run_detail.rs::fetch_logs` L102–L111 wraps `get_log_lines` in `match`, returning `{items: vec![], total: 0}` on `Err`. At L104 it also **logs an error** via `tracing::error!`. The actual bug: the error log is loud, the empty-list case looks like the error case to a casual reader.

**What goes wrong:** When a brand-new running job has zero log lines yet (very common — script hasn't produced output in the first 200ms), the handler path is fine (returns empty vec). But if `get_log_lines` errors for **any** reason (read-pool exhausted, transient SQLite busy, Postgres network blip), the page shows empty logs and the server tracing shows a loud error. A busy operator interprets this as "the system lost my logs."

Additionally: there's no distinction between "no logs yet because the job just started" and "logs failed to load due to an error." The UI should make this clear.

**Prevention:**
1. Split the error path from the empty path. On `Err`, propagate a 500 (or a structured partial error) instead of an empty list. An empty list must **only** mean "no logs yet."
2. In the template, render a contextual message when `logs.is_empty() && is_running`: "Waiting for output…" — distinct from "No logs for this run" which is shown for completed runs with no output.
3. If the error is transient (e.g., pool exhausted), expose a retry via HTMX rather than a hard failure.

**Test cases:**
- **T-V11-LOG-10:** Brand-new running run with no log rows yet; assert `logs == []` is returned AND the template renders "Waiting for output…" (not an error).
- **T-V11-LOG-11:** Force `get_log_lines` to `Err`; assert the handler returns an error partial, not an empty-list success.

**Severity:** MODERATE.

---

### 3.3 MODERATE — HTMX `sseError` handler wipes content on transient reconnect

**Where:** v1.0 HTMX integration in `templates/pages/run_detail.html`. The existing `hx-on::sse-error` handler (or equivalent) needs review — if it currently `innerHTML`-replaces `#log-lines` with an error, any transient SSE disconnect silently wipes the loaded static logs.

**What goes wrong:** Browser briefly loses network. HTMX SSE extension fires the error event. If the handler clears `#log-lines` to show "connection error," the operator sees their existing logs disappear — even though they're safe in the DB and the next refresh will show them. Feels like a data-loss bug.

**Prevention:**
1. The SSE error handler must **append** a "reconnecting…" toast to a sibling `#log-status` banner, NEVER modify `#log-lines`.
2. On `sse-connect` (reconnect success), remove the toast without touching `#log-lines`.
3. If the error is permanent (run completed, server said close), fire the static-partial fetch flow (Feature 2.2).

**Test case:**
- **T-V11-LOG-12:** Simulate an SSE network hiccup while viewing a running job; assert existing log content in `#log-lines` is untouched; assert a transient banner appears and then clears on reconnect.

**Severity:** MODERATE.

---

### 3.4 MODERATE — SSE endpoint fails when the run transitioned to finished between handler spawn and subscribe

**Where:** `src/web/handlers/sse.rs::sse_logs` L34–L37:

```rust
let maybe_rx = {
    let active = state.active_runs.read().await;
    active.get(&run_id).map(|tx| tx.subscribe())
};
```

Followed by L40–L65 which emits `run_complete` immediately if `maybe_rx` is `None`.

**What goes wrong:** Page loads, renders `is_running=true`, template attaches SSE. Network latency is 100ms. Meanwhile the run completes on the server and `run.rs::run_job` L276 removes the entry from `active_runs`. The SSE handler acquires the read lock, finds nothing, and immediately sends `run_complete`. The client triggers its run_complete handler, which fetches the static partial. **This is actually the correct behavior today** — the page self-heals.

**However:** the v1.1 backfill flow (§3.3) needs to render the existing `logs` inside `#log-lines` on the initial page load. If the run is actually still running at page-render time but completes before the SSE subscribe, the sequence becomes: render static backfill → attach SSE → SSE immediately says `run_complete` → fetch static partial again → duplicate render. Without id-based dedupe (Feature 2.1), the operator briefly sees log lines rendered twice.

**Prevention:**
1. The static-partial fetch triggered by `run_complete` must be **idempotent on the client side** — replacing `#log-lines` innerHTML is safe because the static partial contains the full log. The flash of duplicate content lasts <1 frame but is visible.
2. Alternatively: if the SSE handler's first event is `run_complete` with no prior `log_line` events, the client should treat that as "already finished" and trigger the static fetch **without** any intermediate state.
3. Race test: deterministically ensure the transition doesn't produce a visible duplicate render.

**Test case:**
- **T-V11-LOG-13:** Start a short-lived job; navigate to its detail page; assert the page settles to the static state without visible duplication of any line.

**Severity:** MODERATE.

---

## Feature 4 — Log Backfill on Navigate-Back

### 4.1 CRITICAL — Gap between backfill and SSE subscribe

**Where:** The sequence in the proposed §3.3 flow:
1. `run_detail` handler queries `get_log_lines` → returns rows `[0..N]`
2. Handler returns HTML; browser parses; SSE client attaches to `/events/runs/{id}/logs`
3. SSE stream delivers lines `[M..]` where M is whatever was in the broadcast ring at subscribe time

**What goes wrong:** If steps 1→3 span 50ms and 10 lines are written in that window, they're persisted to DB (visible at step 1's `max_id + 1..max_id + 10`) but NOT in the broadcast buffer at step 3 (already shifted out). The client sees a gap: DB lines `[0..N]`, then SSE lines `[N+11..]`, missing `[N+1..N+10]`.

Alternatively: broadcast channel re-delivers lines `[N-5..]` (they're still in the ring); client sees 5 duplicates at the seam.

**Why:** `tokio::sync::broadcast` has no replay-from-id semantics. A new subscriber gets whatever happens to be in the ring buffer at subscribe time, bounded by capacity (256).

**Prevention:**
1. **Id-based dedupe and gap detection.** Client records `max_backfill_id` (see Feature 2.1). On every SSE event:
   - If `event.id <= max_backfill_id` → drop (duplicate).
   - If `event.id == last_seen_id + 1` → append.
   - If `event.id > last_seen_id + 1` → **gap detected**; re-fetch `/partials/runs/{id}/static-log?after={last_seen_id}` to fill the hole, then update `last_seen_id`.
2. The gap-fill partial handler is new — add `fetch_logs_after(run_id, after_id) -> Vec<LogLineView>` to the DB layer.
3. Log gap-detection events at `debug` level so operators can track how often they happen.

**Test cases:**
- **T-V11-BACK-01:** Navigate back to a running job in the middle of a 500-line burst; assert exactly 500 lines rendered at the end (no gaps, no duplicates). Run against both SQLite and Postgres.
- **T-V11-BACK-02:** Force a deliberate gap (subscribe 10ms after backfill, inject 50 DB-only lines in between); assert the client auto-fills via `?after=` refetch.

**Severity:** CRITICAL.

---

### 4.2 MODERATE — Duplicate lines on navigate-back (covered by Feature 2.1 + 4.1)

**Where:** Same as 4.1. Covered by id-based dedupe.

**Test case:**
- **T-V11-BACK-03:** Load run detail twice in quick succession (simulating back-button navigation); assert both loads show identical line counts and identical last-id.

**Severity:** MODERATE (covered by dedupe).

---

### 4.3 MINOR — Retention pruner race with backfill

**Where:** `src/scheduler/retention.rs` (daily pruner) + `src/db/queries.rs::get_log_lines`.

**What goes wrong:** Retention pruner runs at 03:00 and deletes old log rows. Operator opens a run-detail page at 03:00:00.500 for a run whose logs are about to be pruned. Backfill query runs at 03:00:00.600 during the prune batch, returns partial results, SSE attaches, and on refresh the static partial now shows 500 lines where the last page showed 1000.

**Why:** v1.0 retention is batched with WAL checkpoint (see v1.0 STACK.md), so an in-progress prune can partially overlap a concurrent read.

**Prevention:**
1. Retention is a **background maintenance** task, not a user-facing guarantee. Document in code comments that log-line visibility for pruned runs is best-effort.
2. If a job's start_time is within the retention window, its log rows must not be pruned. Double-check the prune query's WHERE clause.
3. Test retention pruning concurrent with a read flow; assert the reader either sees old lines or sees pruned-empty — never an inconsistent mid-state (this is inherent to SQLite MVCC read snapshots via the reader pool, but worth an explicit test).

**Test case:**
- **T-V11-BACK-04:** Concurrently run the retention pruner and `get_log_lines` against the same run (before its retention cutoff); assert the reader always sees a consistent snapshot.

**Severity:** MINOR.

---

## Feature 5 — Per-Job Run Numbers Migration

**This is the single highest-risk migration in v1.1.** Deep pitfall coverage follows.

### 5.1 CRITICAL — Migration step ordering (three-step split is mandatory)

**Where:** New migrations in `migrations/{sqlite,postgres}/20260415_00000{0,1,2}_job_run_number*.up.sql`.

**What goes wrong:** Combining "add column + backfill + add NOT NULL constraint" in a single migration creates multiple failure modes:

1. **SQLite `ALTER TABLE ADD COLUMN` with `NOT NULL DEFAULT`** requires a default expression. A literal `DEFAULT 0` is wrong (not a real run number). A subquery default is not supported. You must add as nullable first.
2. **Postgres DDL inside a transaction** can combine all three, but if the backfill fails mid-statement, the whole migration rolls back, leaving the column absent — which is the least bad outcome but still requires a re-run.
3. **Resume after crash:** if the process is killed between the backfill UPDATE and the `ALTER TABLE … SET NOT NULL`, the partially-applied schema is inconsistent on Postgres (column exists, some nulls, no constraint) and **SQLx migration tracking will mark the migration as applied** because it's all one file.

**Why:** SQLx applies each migration file atomically based on filename. An in-file crash leaves the tracking table in whatever state it was in at crash time — usually "not applied" if the transaction rolled back, but the failure mode depends on the backend.

**Prevention:** Split into **three separate migration files** per backend:

```
20260415_000000_job_run_number_add_column.up.sql       -- ALTER TABLE ADD COLUMN job_run_number INTEGER (nullable)
20260415_000001_job_run_number_backfill.up.sql         -- UPDATE … SET job_run_number = ROW_NUMBER() … WHERE job_run_number IS NULL
20260415_000002_job_run_number_not_null.up.sql         -- ALTER TABLE … SET NOT NULL (Postgres) / table-rewrite (SQLite)
```

**Rationale:**
- SQLx tracks each file separately. A crash in file 1 rolls back cleanly; re-run picks up where it left off.
- The backfill file is idempotent (`WHERE job_run_number IS NULL`) — rerunning is safe.
- The NOT NULL file can only succeed if the backfill completed; if not, the migration fails loudly and operators get a clear error instead of a silently corrupt schema.

**Test cases:**
- **T-V11-RUNNUM-01:** Start with a DB at migration file 0 (column exists, all nulls); run migrate; assert files 1+2 apply in order.
- **T-V11-RUNNUM-02:** Start with a DB at migration file 1 partially applied (some rows backfilled); run migrate; assert remaining rows get backfilled and file 2 succeeds.
- **T-V11-RUNNUM-03:** Start with a DB where file 1 was applied but file 2 failed mid-way; re-run migrate; assert the NOT NULL constraint applies cleanly.

**Severity:** CRITICAL.

---

### 5.2 CRITICAL — SQLite `ALTER TABLE` restrictions vs Postgres `UPDATE … FROM`

**Where:** The backfill migration SQL.

**What goes wrong:** SQLite 3.33+ supports `UPDATE … FROM` (which is what ARCHITECTURE.md §3.2 recommends), but:

1. **SQLite `ALTER TABLE … SET NOT NULL` does not exist.** The only way to add NOT NULL to an existing column is the 12-step table-rewrite pattern (`CREATE new table → INSERT SELECT → DROP old → RENAME`). This is documented in the SQLite "making other kinds of table schema changes" docs.
2. **`sqlx-sqlite` 0.8.6 bundles SQLite ~3.46.** 3.33 is the minimum for `UPDATE … FROM`, so the bundled version is fine — but verify in CI via `SELECT sqlite_version()` to lock the assumption.
3. **Postgres `UPDATE … FROM`** is standard and works cleanly; `ALTER TABLE … ALTER COLUMN … SET NOT NULL` is also standard.

**Why:** SQLite's `ALTER TABLE` is intentionally limited. The table-rewrite pattern is mechanical but easy to get wrong (`PRAGMA foreign_keys` must be disabled during the rewrite; indexes must be recreated on the new table; triggers must be preserved).

**Prevention:**
1. **SQLite NOT NULL migration must use the table-rewrite pattern verbatim from the SQLite docs:**
   ```sql
   PRAGMA foreign_keys=OFF;
   BEGIN TRANSACTION;
   CREATE TABLE job_runs_new ( ... job_run_number INTEGER NOT NULL ...);
   INSERT INTO job_runs_new SELECT id, job_id, status, trigger, start_time, end_time, duration_ms, exit_code, container_id, error_message, job_run_number FROM job_runs;
   DROP TABLE job_runs;
   ALTER TABLE job_runs_new RENAME TO job_runs;
   -- Recreate indexes
   CREATE INDEX idx_job_runs_job_id_start ON job_runs(job_id, start_time DESC);
   CREATE INDEX idx_job_runs_start_time ON job_runs(start_time);
   COMMIT;
   PRAGMA foreign_keys=ON;
   ```
2. **Assert SQLite version in a test** (`SELECT sqlite_version()`) so the `UPDATE … FROM` syntax requirement is locked.
3. **Recreate every index** listed in the initial migration (`idx_job_runs_job_id_start`, `idx_job_runs_start_time`). Missing an index here silently regresses dashboard query performance.
4. **Run the migration against a fixture database** with realistic data (≥100k rows) in CI before merging the migration.

**Test cases:**
- **T-V11-RUNNUM-04:** Unit test that runs the full migration chain against a seeded SQLite DB with 100k `job_runs` rows across 20 distinct `job_id`s; assert post-migration row count unchanged, all indexes present via `PRAGMA index_list`.
- **T-V11-RUNNUM-05:** Same test against `testcontainers` Postgres.
- **T-V11-RUNNUM-06:** Regression test that asserts `SELECT sqlite_version()` from the bundled sqlx-sqlite is `>= 3.33.0`.

**Severity:** CRITICAL.

---

### 5.3 CRITICAL — Long-running migration vs container healthcheck timeout

**Where:** `src/db/mod.rs::migrate` is called before `scheduler::spawn` — migrations run synchronously at startup. The Docker healthcheck (Feature 10) polls `/health` but the HTTP server isn't up until migrations complete.

**What goes wrong:** On a homelab SQLite DB with 2M `job_runs` rows and slow spinning-rust storage, the backfill migration's `UPDATE … FROM (ROW_NUMBER() …)` could take 30+ seconds. Meanwhile:

1. Docker healthcheck `start-period` defaults to `0s` unless explicitly set, so the container is marked `(unhealthy)` the moment the first check fires.
2. `docker compose up -d` with `depends_on: { cronduit: { condition: service_healthy } }` deadlocks other services.
3. SQLite's `UPDATE … FROM` with `ROW_NUMBER()` is effectively `O(N log N)` because the window function sorts. On 2M rows this is meaningful.

**Why:** v1.0 never had a long-running startup migration; the initial migration was empty-table-only. v1.1 introduces the first migration that can take non-trivial wall time proportional to existing data.

**Prevention:**
1. **Dockerfile `HEALTHCHECK` must set `start-period` generously** — e.g., `--start-period=60s` (or higher) to accommodate realistic backfill times. Document the exact number in the compose example with a comment explaining why.
2. **Chunk the backfill** if the table size exceeds a threshold. Instead of one giant UPDATE, loop in 10k-row batches with a `WHERE job_run_number IS NULL LIMIT 10000` clause. Preserves progress across crashes, reduces WAL pressure on SQLite, and keeps the write lock short on Postgres.
3. **Log progress at INFO level** during the backfill (`backfilled 10000/2000000 rows`). Operators watching `docker logs` see activity and don't assume the process is hung.
4. **Add a `cronduit_migration_progress` gauge** (bounded-cardinality, label `migration_name`) for Prometheus observability during upgrade windows.

**Test cases:**
- **T-V11-RUNNUM-07:** Seed a DB with 500k rows; run migration; assert it completes under 30s on CI hardware.
- **T-V11-RUNNUM-08:** Kill the process after 1s during backfill; restart; assert migration resumes cleanly without losing rows.
- **T-V11-RUNNUM-09:** Assert INFO log lines appear at regular intervals during a long backfill.

**Severity:** CRITICAL — a failed upgrade for an existing operator is the worst possible rc.1 signal.

---

### 5.4 MODERATE — Scheduler-startup race (pre-resolved, must be test-locked)

**Where:** `src/cli/run.rs` startup ordering: `DbPool::connect → pool.migrate() → reconcile_orphans → sync_config_to_db → scheduler::spawn`.

**What goes wrong:** ARCHITECTURE.md §5.2 confirms migrations run strictly before the scheduler spawns, so **no race exists**. But this is a structural invariant that could be easily broken by a future refactor — someone moving the migrate call inside `scheduler::spawn` would silently reintroduce the race.

**Prevention:**
1. Lock the invariant in a test that asserts `pool.migrate()` is called before `scheduler::spawn` in the startup path. Use a spy/mock if necessary.
2. Code comment in `src/cli/run.rs`: `// INVARIANT: pool.migrate() must complete before scheduler::spawn — the job_run_number backfill migration (v1.1) depends on this ordering.`
3. Document in `src/db/mod.rs::migrate` that callers must not spawn scheduler tasks concurrently.

**Test cases:**
- **T-V11-RUNNUM-10:** Integration test that asserts the migration observed zero concurrent writes during backfill (count rows at start of backfill and end, ensure delta is zero).

**Severity:** MODERATE.

---

### 5.5 MODERATE — Option B (dedicated counter on `jobs`) requires its own migration

**Where:** ARCHITECTURE.md §3.2 recommends Option B: a new column `jobs.next_run_number BIGINT NOT NULL DEFAULT 1`, with insert becoming `UPDATE … RETURNING next_run_number - 1`.

**What goes wrong:** The counter column needs its own migration **and must be initialized correctly for existing jobs**. `DEFAULT 1` means a job with existing runs would get the next id = 1, colliding with backfilled run numbers. The migration must:

1. Add the column with `DEFAULT 1`.
2. Backfill: `UPDATE jobs SET next_run_number = (SELECT COALESCE(MAX(job_run_number), 0) + 1 FROM job_runs WHERE job_runs.job_id = jobs.id)`.
3. Order relative to the `job_runs.job_run_number` migrations is critical: the `jobs.next_run_number` backfill must run **AFTER** `job_runs.job_run_number` is backfilled.

**Prevention:**
1. Order migration files so the `jobs.next_run_number` backfill is strictly after `job_runs.job_run_number` backfill. Use the filename timestamp to enforce.
2. Use a correlated subquery in the backfill as shown above.
3. Test that a fresh install (empty `job_runs`) results in `next_run_number = 1` for new jobs.

**Test case:**
- **T-V11-RUNNUM-11:** Insert 5 runs for a job, run full migration chain, assert `jobs.next_run_number = 6` for that job.
- **T-V11-RUNNUM-12:** Insert new run post-migration; assert `job_run_number = 6`; assert `jobs.next_run_number` was incremented to 7.

**Severity:** MODERATE.

---

### 5.6 MODERATE — URL stability: do NOT rekey `/jobs/{job_id}/runs/{run_id}` by `job_run_number`

**Where:** Routes in `src/web/mod.rs`, the `run_detail` handler, and the SSE handler.

**What goes wrong:** Tempting to expose nicer URLs like `/jobs/foo/runs/42` where `42` is the per-job number. But this collides with:

1. **Orphan reconciliation:** containers are labelled `cronduit.run_id=<global_id>`; the label is the global id, not the per-job number. The reconciler cannot look up a container by `(job_id, job_run_number)`.
2. **The SSE handler:** `active_runs` is keyed by global `run_id` (see `src/scheduler/mod.rs` L56). Changing the URL key to per-job number adds a lookup layer and a new 404 surface.
3. **Historical URLs:** bookmarks from v1.0 use the global id path. Rekeying silently breaks them.

**Prevention:**
1. URLs stay on the global id: `/jobs/{job_id}/runs/{run_id}` where `run_id` is the global `job_runs.id`. **Non-negotiable.**
2. The per-job number is a **display** value only, shown in breadcrumbs and titles as `Run #{{ run.job_run_number }}` while the URL and internal identifiers use the global id.
3. Code-review checklist item: no handler may accept a `job_run_number` path parameter in v1.1.

**Test case:**
- **T-V11-RUNNUM-13:** Assert every route matching `/runs/{id}` uses the global id via a macro or router introspection test.

**Severity:** MODERATE.

---

## Feature 6 — Run Timeline (Gantt)

### 6.1 CRITICAL — N+1 query on the timeline handler

**Where:** Proposed `src/db/queries.rs::get_timeline_runs` (new). ARCHITECTURE.md §3.4 recommends a single query with `WHERE end_time >= $since OR status = 'running'`.

**What goes wrong:** A naive implementation iterates over `get_enabled_jobs()` and queries runs for each job: `for job in jobs { get_job_runs(job.id, since, until) }`. For 50 jobs and a 7-day window this is 50 round-trips to SQLite, each requiring a read-pool acquire. Total latency: 50 × (5ms acquire + 10ms query) = ~750ms page load.

Also: the dashboard query (`get_dashboard_jobs` L474–L597 of `queries.rs`) is already a complex LEFT JOIN — reusing that shape verbatim for the timeline compounds the cost.

**Prevention:**
1. **Single SELECT** shape:
   ```sql
   SELECT jr.id, jr.job_id, j.name AS job_name, jr.status, jr.start_time, jr.end_time
   FROM job_runs jr
   JOIN jobs j ON j.id = jr.job_id
   WHERE (jr.end_time >= ?1 OR jr.status = 'running')
     AND j.enabled = 1
   ORDER BY j.name, jr.start_time
   LIMIT 10000;
   ```
2. **Limit the result set** with a hard cap (`LIMIT 10000`). A 7-day window for 50 jobs running every minute is ~500k runs — rendering the full set in HTML would OOM the page. Document the cap in the UI ("Showing first 10000 runs in this window").
3. **Index usage:** the query must hit `idx_job_runs_start_time` (already exists per the initial migration at `migrations/sqlite/20260410_000000_initial.up.sql` L46). Verify via `EXPLAIN QUERY PLAN` in a test.

**Test cases:**
- **T-V11-TIME-01:** Seed 10 jobs × 1000 runs each; query the timeline for the full window; assert single query executed (use sqlx query counter middleware), under 100ms on CI.
- **T-V11-TIME-02:** `EXPLAIN QUERY PLAN` on the timeline query contains `USING INDEX idx_job_runs_start_time`.

**Severity:** CRITICAL.

---

### 6.2 MODERATE — Rendering long-running "now" bars

**Where:** Template `templates/pages/timeline.html` (new).

**What goes wrong:** A run that started 8 hours ago and is still running has `end_time IS NULL`. The template must render its bar as extending to "now." Options:

1. **Server-side substitution**: handler computes `end_time_or_now` via `COALESCE(end_time, strftime('now'))`. Works but the rendered HTML is stale the moment the page is loaded — the bar doesn't extend as time passes.
2. **Client-side animation**: render a CSS `width: calc(... * (now - start))` with a JS ticker. Works but adds custom JS, violating the "no JS framework" rule. A tiny 10-line vanilla JS ticker is acceptable.
3. **Polling the partial**: re-fetch the timeline partial every 5s while the window contains running runs (ARCHITECTURE.md §3.4 already proposes this).

**Prevention:**
1. Adopt option 3 (polling) as the primary mechanism. The 5s cadence is visually sufficient for "now" bars and reuses the existing HTMX polling pattern.
2. Server computes `end_time_or_now` in Rust from `chrono::Utc::now()`, NOT from SQLite `strftime('now')` — this ensures timezone consistency with the rest of the UI (see 6.3).

**Test case:**
- **T-V11-TIME-03:** Render the timeline with a running run; assert `end_time_or_now` in the output equals `start_time + (now - start_time)` within a 1s tolerance.

**Severity:** MODERATE.

---

### 6.3 MODERATE — Timezone rendering consistency

**Where:** Timeline handler + template.

**What goes wrong:** Three candidate timezones:
1. **Server UTC** — consistent with `job_runs.start_time` storage format (RFC3339 TEXT, usually UTC).
2. **Operator timezone from config** — cronduit has a `tz` config field used by croner for schedule evaluation (`src/scheduler/mod.rs` L50). This is the "natural" operator timezone.
3. **Browser local** — rendered client-side via JS.

Mixing these is visible: "my job fired at 3am UTC" vs "my job fired at 10pm PDT" for the same run. The dashboard already renders in operator tz via Rust-side formatting; the timeline must match.

**Prevention:**
1. Use **the operator timezone from `self.tz`** throughout the timeline (matching the dashboard). Compute bar positions server-side in that tz.
2. Label the X axis with the tz abbreviation (`"PDT"`) so operators know what they're looking at.
3. Do NOT mix wall-clock labels from one tz with duration offsets computed in another.

**Test case:**
- **T-V11-TIME-04:** Set operator tz to `America/Los_Angeles`; seed a run at UTC 2026-04-14T10:00:00Z; assert the timeline label shows `03:00` (PDT) not `10:00`.

**Severity:** MODERATE.

---

### 6.4 MODERATE — Color-only status coding fails accessibility audit

**Where:** Timeline bar rendering in the template.

**What goes wrong:** Using only the `cd-status-*` CSS color tokens (success=green, failed=red, timeout=orange) to distinguish bars is colorblind-hostile. Roughly 8% of men and 0.5% of women have red-green colorblindness; green/red bars are indistinguishable.

**Prevention:**
1. Encode status with **pattern + color**: diagonal stripes for `failed`, solid for `success`, checkerboard for `timeout`, pulsing outline for `running`, cross-hatch for `stopped`. Implement via inline SVG `<pattern>` elements or CSS `background-image: repeating-linear-gradient(...)`.
2. Add ARIA labels to each bar: `aria-label="Backup job run #42: failed, lasted 3m 12s"`.
3. Design system reference: check `design/DESIGN_SYSTEM.md` for any existing pattern tokens before inventing new ones.

**Test case:**
- **T-V11-TIME-05:** Visual regression test that asserts distinct patterns for each status value (not just color). Screenshot-diff against a reference image.
- **T-V11-TIME-06:** Automated accessibility audit (axe-core or pa11y) run against the timeline page in CI.

**Severity:** MODERATE.

---

## Feature 7 — Sparkline + Success Rate

### 7.1 CRITICAL — Sample-size honesty

**Where:** Proposed `get_dashboard_job_sparks` in `queries.rs` and `dashboard.rs::to_view`.

**What goes wrong:** A job with exactly 1 run ever, which failed, renders "success rate: 0%". A job with 1 successful run renders "100%". Neither number is meaningful. Operators see "50% success rate" on a 2-run job and over-interpret.

**Prevention:**
1. **Minimum sample threshold**: N < 5 renders "—" or "n/a" for the success rate.
2. The threshold is a **constant** in code (`const MIN_SAMPLES_FOR_RATE: usize = 5;`) with a doc comment explaining the rationale.
3. Sparkline can still render with <5 samples (it's an individual-run view, not a rate), but pad the visual so 1 cell doesn't span the full sparkline width.
4. Tooltip on the badge shows the raw count: `"12 successes / 15 runs over last 20"` so operators can reason about signal strength.

**Test cases:**
- **T-V11-SPARK-01:** Job with 0 runs → sparkline renders empty, badge shows `"—"`.
- **T-V11-SPARK-02:** Job with 3 runs → badge shows `"—"` (below threshold).
- **T-V11-SPARK-03:** Job with 5 successful runs → badge shows `"100%"`.
- **T-V11-SPARK-04:** Job with 20 runs (15 success, 5 failed) → badge shows `"75%"`.

**Severity:** CRITICAL.

---

### 7.2 MODERATE — Rolling window boundary effects

**Where:** Same handler.

**What goes wrong:** A job that always fails at the top of the hour and succeeds otherwise will show 0% right after the hourly failure, then slowly climb to 95%, then drop to 0% again. The user sees oscillating numbers that look scary but aren't signal.

**Prevention:**
1. Rolling window of **last N runs** (e.g., N=20) not "last N hours." Makes the signal independent of the job's cadence.
2. Document the window size visibly in the UI (hover tooltip: "Last 20 runs").
3. Consider an EWMA (exponentially weighted moving average) if N-based looks choppy; but start with the simpler window and see if operators complain.

**Test case:**
- **T-V11-SPARK-05:** Job with 20 runs in a pattern `success*19 + failed*1`; assert sparkline shows 19 green + 1 red; assert rate = 95%.

**Severity:** MODERATE.

---

### 7.3 MINOR — SVG pixel snapping at small sizes

**Where:** Template sparkline rendering.

**What goes wrong:** An inline SVG sparkline rendered at 16px height with `<rect>` elements positioned by floating-point `x` values gets anti-aliased blur at non-integer pixel positions. Looks fuzzy at low DPI.

**Prevention:**
1. Use integer pixel positions (round x/width to the nearest pixel).
2. Or render the sparkline as a row of fixed-width `<span>` elements with CSS classes, not inline SVG. ARCHITECTURE.md §3.5 already recommends this approach.
3. Test the rendered output at 1x and 2x DPI.

**Test case:**
- **T-V11-SPARK-06:** Rendered sparkline HTML contains only integer coordinates (or fixed-width spans, no SVG).

**Severity:** MINOR.

---

## Feature 8 — Duration Trend p50/p95

### 8.1 CRITICAL — Percentile computation parity between SQLite and Postgres

**Where:** ARCHITECTURE.md §3.6 recommends computing in Rust for parity. Proposed `src/web/stats.rs::percentile`.

**What goes wrong:** A "compute in Rust" implementation is trivial but easy to get subtly wrong:

1. **Index rounding**: `samples[len * 0.95]` vs `samples[ceil(len * 0.95)]` vs `samples[floor(len * 0.95)]` — different choices produce different values for small N. Document the chosen rounding and match NumPy's `percentile` convention (`method='linear'`) for familiarity.
2. **Empty input**: `samples.is_empty()` → panic on index. Must return `None` / `"—"`.
3. **Single-element input**: both p50 and p95 collapse to the same value. Correct but the UI needs to communicate it.
4. **Unsorted input**: computing on an unsorted slice is wrong. The helper must sort (or assert sorted).

**Prevention:**
1. Write the `percentile(samples: &mut [i64], q: f64) -> Option<i64>` helper in a new `src/web/stats.rs` module with exhaustive unit tests covering empty/one/two/odd/even/boundary cases.
2. Document the rounding convention: "linear interpolation between adjacent samples, matching NumPy's default".
3. Make the helper take `&mut [i64]` (it sorts in place) to force callers to commit to the allocation.
4. NEVER use SQL-side percentile (even on Postgres) — the structural-parity constraint mandates Rust-side computation.

**Test cases:**
- **T-V11-DUR-01:** `percentile(&mut [], 0.5)` returns `None`.
- **T-V11-DUR-02:** `percentile(&mut [100], 0.5)` returns `Some(100)`.
- **T-V11-DUR-03:** `percentile(&mut [1,2,3,4,5,6,7,8,9,10], 0.5)` returns `Some(5)` or `Some(6)` depending on convention — lock the chosen value.
- **T-V11-DUR-04:** `percentile(&mut [1,2,3,...,100], 0.95)` returns a documented, stable value.

**Severity:** CRITICAL.

---

### 8.2 MODERATE — Minimum sample size for percentile meaningfulness

**Where:** `job_detail.rs` handler.

**What goes wrong:** p95 on 3 runs is meaningless (effectively the max). Operators see a scary spike after one slow run.

**Prevention:**
1. Same minimum-sample threshold as Feature 7.1 (`N >= 20`). Below that, render `"—"` with a tooltip `"Need at least 20 runs for percentile analysis"`.
2. Consider a distinct threshold for p95 vs p50: p50 can be meaningful at N=10, p95 needs N=20+.

**Test case:**
- **T-V11-DUR-05:** Job with 10 runs → p50 shown, p95 shown as `"—"` with tooltip.

**Severity:** MODERATE.

---

### 8.3 MODERATE — Outlier contamination of p95

**Where:** Same.

**What goes wrong:** A backup job normally runs in 5 seconds but had one bad day at 30 minutes. That single run dominates the p95 for the visible window. The chart is dominated by one outlier.

**Prevention:**
1. **Do not trim outliers.** The p95 is designed to expose them — that's the signal. Trimming hides the problem the metric exists to find.
2. **Use a logarithmic Y axis** for the duration trend chart so both 5s and 1800s are visible.
3. Add a separate "max in window" display so the outlier is called out explicitly.

**Test case:**
- **T-V11-DUR-06:** Chart rendering with outlier present doesn't crash the SVG viewbox computation.

**Severity:** MODERATE.

---

## Feature 9 — Bulk Enable/Disable

### 9.1 CRITICAL — Config reload unsets `enabled_override` by accident

**Where:** `src/scheduler/sync.rs::sync_config_to_db` L102–L216, `src/db/queries.rs::upsert_job` L57–L123 (hardcodes `enabled = 1` on conflict), `disable_missing_jobs` L129–L169.

**What goes wrong:** Feature 9's whole point is that `enabled_override` is **orthogonal** to config reload (ARCHITECTURE.md §3.7 Option B). But `upsert_job` currently runs `ON CONFLICT DO UPDATE … SET enabled = 1, …` on every config reload. If the new `enabled_override` column is added to the same UPDATE by accident, the override gets reset on every reload and bulk-disable is useless.

**Prevention:**
1. **Absolute rule:** `upsert_job` must NOT touch `enabled_override` in its SET clause. The override is only written by the new `set_enabled_override_bulk` API handler.
2. **Lock this with a code-search test**: grep the codebase for `enabled_override` and assert it only appears in specific allowed places (migration, bulk-toggle handler, query filters).
3. `disable_missing_jobs` must **clear** the override when disabling (per ARCHITECTURE.md §3.7 step 1 "one-line addition") so a later re-add doesn't leave a stale override. But for jobs that remain in the config, the override must be preserved across reloads.

**Test cases:**
- **T-V11-BULK-01:** Set `enabled_override = 0` on a job; trigger `SchedulerCmd::Reload`; assert override is still `0` after reload.
- **T-V11-BULK-02:** Delete a job from the config file; reload; assert the job's `enabled_override` is cleared (NULL) in addition to `enabled = 0`.
- **T-V11-BULK-03:** Re-add the deleted job to the config; reload; assert it's `enabled = 1, enabled_override = NULL` (clean slate).
- **T-V11-BULK-04:** Grep-based test: `enabled_override` appears only in the specific allowed modules/files.

**Severity:** CRITICAL.

---

### 9.2 CRITICAL — Heap + `enabled_override` filter must both live in the reload path

**Where:** `src/scheduler/reload.rs::do_reload` and `src/db/queries.rs::get_enabled_jobs`.

**What goes wrong:** `do_reload` rebuilds the in-memory heap from `get_enabled_jobs()`. If `get_enabled_jobs` doesn't include the `enabled_override` filter, a bulk-disabled job keeps firing from the heap until the process restarts — because the scheduler loop's view of the jobs is the heap, not the DB.

**Why:** v1.0's `get_enabled_jobs` filters on `WHERE enabled = 1` only. ARCHITECTURE.md §3.7 proposes changing it to `WHERE enabled = 1 AND (enabled_override IS NULL OR enabled_override = 1)`.

**Prevention:**
1. Change `get_enabled_jobs` filter in the same PR that adds the column.
2. `bulk_toggle` API handler MUST fire `SchedulerCmd::Reload` after updating the DB. Without this, the heap is stale.
3. Integration test: bulk-disable a job that's about to fire; assert it does NOT fire within 10s.

**Test cases:**
- **T-V11-BULK-05:** Seed a job with schedule `* * * * *`; bulk-disable via the API; `tokio::time::advance` 2 minutes; assert zero new runs for that job.
- **T-V11-BULK-06:** Bulk-enable a disabled job; assert reload fires; assert next scheduled fire produces a run.

**Severity:** CRITICAL.

---

### 9.3 MODERATE — Bulk-disable does NOT stop already-running runs

**Where:** ARCHITECTURE.md §5.4 documents this.

**What goes wrong:** Operator bulk-disables 5 jobs for maintenance; 2 of them are currently running. The operator expects "all these jobs stop now." Reality: bulk disable only affects **future** fires; the 2 running jobs complete naturally. If the operator wanted them stopped, they should use the Stop button from Feature 1.

**Prevention:**
1. UI toast after bulk-disable: `"3 jobs disabled; 2 currently-running jobs will complete normally. Use the Stop button to terminate them immediately."`
2. Surface the running-vs-disabled count in the toast so it's unambiguous.
3. Do NOT extend bulk-disable to also issue Stop — separation of concerns. Stop is per-run, bulk-disable is per-job future-fires.

**Test case:**
- **T-V11-BULK-07:** Start a long-running job; bulk-disable it; assert the run completes with its natural status; assert the next scheduled fire does NOT happen.

**Severity:** MODERATE.

---

### 9.4 MODERATE — Operator forgets jobs are disabled, discovers 3 months later

**Where:** UI discoverability.

**What goes wrong:** Operator disables 5 jobs for maintenance, forgets, 3 months later realizes their backups haven't run. The v1 UI surface must make the "currently disabled for non-config reason" set highly visible.

**Prevention:**
1. Settings page shows an explicit list: `"3 jobs forced-disabled: backup-postgres, backup-redis, backup-mongo"` with a prominent "Re-enable all" button.
2. Dashboard card for a bulk-disabled job shows a distinct badge: `"DISABLED (override)"` not just `"DISABLED"` — operators need to tell the two disabled-states apart.
3. Optional: `/metrics` exposes `cronduit_jobs_override_disabled{job}` gauge so Prometheus alerts can fire on "backup job disabled for >24h". Nice-to-have.

**Test cases:**
- **T-V11-BULK-08:** Settings page rendering contains the overridden-jobs list when any job has `enabled_override = 0`.
- **T-V11-BULK-09:** Dashboard card badge distinguishes "config-disabled" from "override-disabled".

**Severity:** MODERATE.

---

### 9.5 MODERATE — Bulk-disable race with config reload mid-selection

**Where:** The new `bulk_toggle` API handler and the config file-watcher.

**What goes wrong:** Operator selects 5 jobs in the UI at T=0. Between T=0 and T=+3s (user clicks "Disable"), the file-watcher detects a config edit and triggers `SchedulerCmd::Reload`. The reload runs `disable_missing_jobs`, which for Feature 9 also clears `enabled_override` for any job removed from the config. If one of the 5 selected jobs was simultaneously removed from the config file, the bulk-disable runs AFTER the reload, the override fires on a job whose row is already `enabled=0`, and the override is set on a disappeared-from-config job — confusing state.

**Prevention:**
1. `bulk_toggle` operates by `job_id`. If a `job_id` in the request no longer exists (or is already in the "missing from config" state), skip it and include that in the response for the operator: `"Updated 4 of 5 jobs; job 'foo' was removed from the config."`
2. Document the resolution in the API response so the UI can show it in the toast.
3. Integration test that exercises this exact race deterministically.

**Test case:**
- **T-V11-BULK-10:** Start a bulk toggle for 3 jobs; concurrently fire a config reload that removes one of them; assert the toggle affects the remaining 2 and reports the skipped one.

**Severity:** MODERATE.

---

## Feature 10 — Docker Healthcheck (NEW)

### 10.1 CRITICAL — Root-cause verification: Dockerfile has NO `HEALTHCHECK` today

**Where:** `/Users/Robert/Code/public/cronduit/Dockerfile` (verified — no `HEALTHCHECK` directive). `examples/docker-compose.yml` and `examples/docker-compose.secure.yml` (verified — no `healthcheck:` stanza).

**What goes wrong:** The reported `(unhealthy)` symptom **cannot** come from the shipped examples alone — the shipped images + compose files define no healthcheck at all. The operator's failing deployment is using an **operator-authored** healthcheck stanza (almost certainly `wget --spider` against `/health`). This changes the fix-path calculus:

1. **The shipped artifacts do not have the bug today.** They have no healthcheck at all, so the container reports `Up N hours` without a health suffix.
2. **The operator's problem is in their own compose file.** Fixing the shipped artifacts doesn't fix their deployment — they must update their compose file to use whatever v1.1 ships.
3. **Any v1.1 fix that adds a default HEALTHCHECK to the Dockerfile is a behavior change**: containers that used to report `Up N hours` will now report `Up N hours (healthy)` or `(unhealthy)`. Some operators may have tooling that keys off the current "no status suffix" shape.

**Why:** The original hypothesis in the scope question was "busybox wget chunked-encoding bug in the shipped compose". Grep confirms there is no wget healthcheck in the shipped compose. The bug is in operator-authored overrides that v1 never documented as a supported pattern.

**Prevention:**
1. **Reproduce the exact failing stanza from the operator's compose file before writing any code.** Ask the operator (or recover from the report if it has their compose) — do NOT guess the wget pattern.
2. **Ship a `cronduit health` subcommand** (per ARCHITECTURE.md §3.8) so the canonical fix is *"use `test: ['CMD', '/cronduit', 'health']` instead of whatever you had"*. This works regardless of the root cause of the wget bug.
3. **Add a default `HEALTHCHECK` to the Dockerfile** using `cronduit health` — conservative intervals documented below. This makes the default shipped image healthy without operator action.
4. **Leave compose examples `healthcheck`-free OR add a documented example that uses `cronduit health`.** Either works; both reinforce the canonical pattern.
5. **Verify the reported root cause** by running the operator's exact wget invocation against cronduit's `/health` in a reproducer; the chunked-encoding theory is plausible but unconfirmed.

**Test cases:**
- **T-V11-HEALTH-01:** Build the default Docker image; `docker run` it; `docker inspect` shows `Health.Status == healthy` within 30s.
- **T-V11-HEALTH-02:** Reproduce the reported busybox `wget --spider https://localhost:8080/health` against the shipped image; capture the exact exit code and stderr for the record (either confirms chunked-encoding theory or surfaces a different root cause).

**Severity:** CRITICAL (scope clarification — the fix still ships but the rationale changes).

---

### 10.2 CRITICAL — `cronduit health` subcommand must NOT share state with the running server

**Where:** New `src/cli/health.rs` module.

**What goes wrong:** A naive implementation imports `AppState`, reads the pool, and queries the DB directly. This is wrong for three reasons:

1. **Two connections to the same SQLite file from two processes is allowed, but the healthcheck running `SELECT 1` while the main server is mid-write can trip `SQLITE_BUSY` and return false-negative unhealthy.**
2. **The whole point of a healthcheck is to verify the running server is serving requests** — that's an HTTP-level check, not a DB check. A sub-process that queries the DB directly doesn't prove the server is alive.
3. **Sharing state pulls in every dependency** (bollard, sqlx, etc.) so the healthcheck binary is as big as the main binary. Fine since it IS the main binary (`/cronduit health`), but reinforces that it should make an HTTP call, not a DB call.

**Prevention:**
1. **Implementation:** `cronduit health` spawns a minimal HTTP client (`ureq` or hand-rolled `hyper::Client`), makes `GET http://$bind/health`, parses the JSON body, exits 0 if `status == "ok"`, exits 1 otherwise. That's it. ~60 LOC.
2. **Do NOT use reqwest** — it pulls a huge dep tree. Use `ureq` (pure Rust, rustls-compatible, tiny) or a hand-rolled `hyper::Client` (already a transitive dep via axum).
3. **Config discovery**: the subcommand needs to know the bind address. Options:
   - `--config /etc/cronduit/config.toml` (same as `run` subcommand) and re-parse TOML → picks up operator customizations.
   - `--bind http://127.0.0.1:8080` explicit flag → simpler, no TOML dependency.
   - Default to `127.0.0.1:8080` if neither is provided → matches the documented v1 default.
4. **Verify `cargo tree -i openssl-sys` stays empty** after adding the HTTP client dep (v1.0 security gate must hold).

**Test cases:**
- **T-V11-HEALTH-03:** Run `cronduit health` against a running server; assert exit 0.
- **T-V11-HEALTH-04:** Run `cronduit health` when no server is running; assert exit 1 within 5 seconds (no indefinite retry).
- **T-V11-HEALTH-05:** Run against a server whose `/health` returns `{"status": "degraded"}`; assert exit 1 (partial failure is failure for Docker healthcheck purposes).
- **T-V11-HEALTH-06:** `cargo tree -i openssl-sys` empty post-addition.

**Severity:** CRITICAL.

---

### 10.3 CRITICAL — `cronduit health` must fail fast, not retry indefinitely

**Where:** New `src/cli/health.rs`.

**What goes wrong:** A naive HTTP client with default timeouts might hang for 30+ seconds waiting for a response. Docker's healthcheck has its own `timeout=5s` policy; if `cronduit health` takes 30s to fail, Docker marks the check as "unhealthy" simultaneously from the outer timeout AND from the inner subprocess failure. The Docker daemon logs look weird.

**Prevention:**
1. **Hard connect timeout**: 2 seconds.
2. **Hard read timeout**: 2 seconds.
3. **No retries inside `cronduit health`**: one attempt only. Docker's `retries=3` is the retry policy.
4. **Exit fast** (<3 seconds total) whether success or failure.

**Test cases:**
- **T-V11-HEALTH-07:** Run `cronduit health` against an unreachable address (`--bind http://127.0.0.1:9`); assert exit 1 within 3 seconds.
- **T-V11-HEALTH-08:** Run against a hanging server (returns data extremely slowly); assert exit 1 within 3 seconds.

**Severity:** CRITICAL.

---

### 10.4 MODERATE — Dockerfile HEALTHCHECK interval vs start_period

**Where:** New `HEALTHCHECK` directive in `Dockerfile`.

**What goes wrong:** Aggressive healthcheck settings produce false negatives during startup (migration) or on slow hardware. Conservative settings produce slow unhealthy-detection.

**Prevention:** Conservative defaults derived from realistic cronduit startup times:

```dockerfile
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
  CMD ["/cronduit", "health"]
```

Rationale:
- `--interval=30s`: cronduit is a long-running scheduler, 30s health resolution is plenty.
- `--timeout=5s`: 2.5× the internal 2s connect+read timeout, gives slack for Docker's process-spawn overhead.
- `--start-period=60s`: covers the v1.1 migration backfill on mid-size DBs (§5.3). Document that operators with very large DBs (>1M runs) may need to override this.
- `--retries=3`: matches Docker defaults; with interval=30s this is a 90s window before unhealthy.

**Test cases:**
- **T-V11-HEALTH-09:** Build shipped image, run `docker compose up`, assert container reaches `healthy` within 90s.
- **T-V11-HEALTH-10:** Build image with a synthetic 120s migration delay; assert operators get a clear error message (or the start_period is sufficient — pick and document).

**Severity:** MODERATE.

---

### 10.5 MODERATE — Backward compat: operator-authored healthcheck overrides must still work

**Where:** Operator compose files with their own `healthcheck:` stanza.

**What goes wrong:** Docker compose YAML semantics: a `healthcheck:` stanza in the service definition **replaces** the Dockerfile's `HEALTHCHECK` entirely. So operators who already have their own healthcheck (even a broken one) will NOT pick up the new default. Their deployment continues to be broken until they update their compose file.

**Conversely:** operators who have NO healthcheck in their compose will pick up the new default automatically (via Dockerfile) — that's the win.

**Prevention:**
1. **Release notes MUST call out the new default explicitly** and show the recommended compose stanza for operators who want to override: `test: ["CMD", "/cronduit", "health"]`.
2. **Troubleshooting section in README**: "If your container reports unhealthy with busybox wget, update your compose healthcheck to use `cronduit health`."
3. **Do NOT silently break existing overrides** — the Dockerfile directive is additive. Operators who override continue to override.

**Test case:**
- **T-V11-HEALTH-11:** Start the shipped image with an operator compose override that uses wget (broken pattern); assert container starts but health is controlled by the broken check; assert release notes call this out as an operator action item.

**Severity:** MODERATE.

---

### 10.6 MINOR — Response Content-Length fix is a band-aid, not a fix

**Where:** `src/web/handlers/health.rs` L12–L28 — returns `(StatusCode, Json(...))`.

**What goes wrong:** One proposed fix (ARCHITECTURE.md §3.8 option 3) is to hand-build a Response with an explicit `Content-Length` header so chunked encoding is avoided. This is a **local fix for a specific client bug** and does not generalize — a different tool (netcat, curl with weird flags) could hit a different parse issue. The `cronduit health` subcommand is a superior fix because it owns the client side.

**Prevention:**
1. Do **not** muck with the `/health` handler's response shape. Leave it as the idiomatic axum `(StatusCode, Json(...))`.
2. The canonical health check is `cronduit health`; all other patterns are operator responsibility.
3. Document in `health.rs` that changing the response shape is not the fix path.

**Test case:**
- **T-V11-HEALTH-12:** `/health` endpoint response is unchanged from v1.0 (regression lock).

**Severity:** MINOR.

---

## Cross-Feature Pitfalls

### X.1 CRITICAL — Two migrations land in rc.1; one must precede the other

**Where:** Feature 5 (`job_run_number`, 3 files) + Feature 9 (`enabled_override`, 1 file) both land in v1.1. Feature 5 is in rc.1 (bug-fix block); Feature 9 is in rc.3 (ergonomics).

**What goes wrong:** If the Feature 9 migration file is authored with a timestamp that sorts **earlier** than the Feature 5 files, an operator upgrading from v1.0 to rc.3 directly gets the migrations in the wrong order: `enabled_override` is added first, but then the per-job-run-number migration tries to read `job_runs` rows in a way that assumed the v1.0 schema.

**Prevention:**
1. Migration filenames use `YYYYMMDD_HHMMSS_description` format. Feature 5 files should be `20260415_*` and Feature 9 file should be `20260420_*` or later. The rc build cadence naturally gives this ordering.
2. **Assert migration monotonicity in CI**: the CI job that runs migrations against a fixture DB must process them in filename order and all must succeed. Any out-of-order issue surfaces as a test failure.
3. Document the migration ordering requirement in `migrations/README.md` (create if it doesn't exist).

**Test cases:**
- **T-V11-MIG-01:** CI test: run full migration chain from empty DB; assert all migrations applied; verify monotonic filename ordering.
- **T-V11-MIG-02:** Run migration chain from a v1.0.1 DB snapshot fixture; assert all v1.1 migrations apply cleanly.

**Severity:** CRITICAL.

---

### X.2 CRITICAL — rc.1 ships stop-a-job WITHOUT the §2.1 log-writer ordering change

**Where:** Build-order in ARCHITECTURE.md §4 places Stop-a-Job before Log-Backfill in rc.1, but they share the `LogLine` struct change.

**What goes wrong:** If stop-a-job lands first and the engineer doesn't do the `LogLine.id` refactor proactively, the log-backfill feature (Feature 2) won't have the id field available and the dedupe path has to be bolted on later. That creates a second, redundant pass through the SSE handler.

**Prevention:**
1. Land the `LogLine.id` schema + writer-order change as its own micro-PR **before** either Feature 1 or Feature 3. It's mechanical, independent, and unblocks both downstream features.
2. Sequence: (pre-rc.1) `LogLine.id` refactor → (rc.1.a) Stop-a-Job → (rc.1.b) Log backfill → (rc.1.c) Run numbers → (rc.1.d) Healthcheck.

**Test case:**
- **T-V11-SEQ-01:** Commit graph / PR dependency test: the `LogLine.id` PR merges before any PR touching SSE backfill.

**Severity:** CRITICAL (process, not code).

---

### X.3 MODERATE — Testcontainers integration tests are expensive; keep them gated

**Where:** `tests/` directory (integration), `just test` wiring, CI config.

**What goes wrong:** Feature 1 (stop-a-job) integration tests spawn real alpine containers via bollard+testcontainers. Feature 5 (migration) integration tests seed realistic DBs. Running them on every local `cargo test` slows developer inner loop from ~5s to ~60s. Developers start skipping integration tests entirely.

**Prevention:**
1. Gate integration tests behind a feature flag `#[cfg(feature = "integration")]` (v1.0 already does this per STACK.md).
2. `just test` runs unit tests only; `just test-integration` runs both.
3. CI runs both in separate jobs so local iteration stays fast.

**Test case:** N/A — process.

**Severity:** MODERATE.

---

### X.4 MODERATE — `active_runs` map becomes `running_handles` map (rename risk)

**Where:** `src/scheduler/mod.rs` L56, `src/web/mod.rs` L40, `src/cli/run.rs` L133.

**What goes wrong:** ARCHITECTURE.md §3.1 introduces `running_handles: HashMap<i64, RunControl>` as a new field. But the existing `active_runs: HashMap<i64, broadcast::Sender<LogLine>>` serves a similar "map of in-flight runs" role. Tempting to merge the two into `RunControl { broadcast: broadcast::Sender<LogLine>, cancel: CancellationToken, stop_reason: Arc<AtomicU8>, container_id: Arc<RwLock<Option<String>>> }`. **Merging is fine**, but two separate maps kept in sync is a drift hazard.

**Prevention:**
1. **Merge** `active_runs` and `running_handles` into one `HashMap<i64, RunControl>` where `RunControl` contains both the broadcast sender (for SSE) and the cancel token + stop_reason (for Stop). Update `AppState` to expose `active_runs.read().get(&id).map(|rc| rc.broadcast.clone())`.
2. This is the cleanest design; ARCHITECTURE.md's two-map suggestion can be simplified at implementation time.
3. Test that adding/removing runs happens atomically (single map = single lock acquisition).

**Test case:**
- **T-V11-MAP-01:** Concurrent insert/remove/read on the active-runs map stress-test; assert no dropped entries.

**Severity:** MODERATE.

---

### X.5 MINOR — Clippy lint drift from new `async fn` signatures

**Where:** CI clippy gate (`cargo clippy -D warnings`).

**What goes wrong:** New code in `src/scheduler/control.rs`, `src/web/handlers/timeline.rs`, `src/cli/health.rs`, `src/web/stats.rs` may introduce lints that the existing codebase hasn't seen (e.g., `clippy::module_name_repetitions`, `clippy::doc_markdown`). The `-D warnings` gate fails the build on any new lint.

**Prevention:**
1. Run `cargo clippy --all-targets --all-features -- -D warnings` locally before pushing any of the new modules.
2. If a new lint fires, fix it (don't `#[allow]`).
3. If a lint is genuinely wrong for the project, add to a `clippy.toml` or an `allow` at the module level with a comment explaining why.

**Test case:** Covered by existing CI.

**Severity:** MINOR.

---

## Phase-Specific Warnings (for the Roadmapper)

| Phase | Likely Pitfall | Mitigation |
|-------|---------------|------------|
| rc.1 Spike (Stop) | 1.1, 1.2, 1.3 — cancellation-identity + race + FEATURES.md regression | Spike the `RunControl` + `stop_reason` design first; validate test T-V11-STOP-01..07 before promoting to rc.1 feature work. |
| rc.1 LogLine.id refactor | X.2, 2.1 — writer ordering blocks everything | Land as its own micro-PR **before** either stop-a-job or log-backfill PRs. |
| rc.1 Log backfill | 2.2, 4.1 — dedupe + gap-detection | Id-based dedupe is the single fix for multiple pitfalls; implement client-side once and reuse. |
| rc.1 Per-job run number migration | 5.1, 5.2, 5.3 — three-step split + SQLite table-rewrite + long migration | Biggest risk in v1.1. Ship as its own multi-file PR with a 100k-row fixture test. |
| rc.1 Healthcheck | 10.1 — verify operator's actual compose first | Reproduce operator's failing invocation against the shipped image before writing code. |
| rc.2 Timeline | 6.1, 6.3 — N+1 + tz consistency | Single-query design reviewed; tz matches dashboard. |
| rc.2 Duration trend | 8.1 — percentile rounding convention | Write `src/web/stats.rs::percentile` with exhaustive tests first, then plug into handler. |
| rc.2 Sparkline | 7.1 — sample-size honesty | Lock `MIN_SAMPLES_FOR_RATE` constant; show "—" below threshold. |
| rc.3 Bulk toggle | 9.1, 9.2 — override sync semantics | Code-search test for `enabled_override` usage; test `disable_missing_jobs` clears override. |

---

## Sources

- `/Users/Robert/Code/public/cronduit/src/scheduler/mod.rs` (L56, L98–L250)
- `/Users/Robert/Code/public/cronduit/src/scheduler/cmd.rs` (enum shape)
- `/Users/Robert/Code/public/cronduit/src/scheduler/run.rs` (L65–L350)
- `/Users/Robert/Code/public/cronduit/src/scheduler/command.rs` (L58–L218 — process-group kill pattern)
- `/Users/Robert/Code/public/cronduit/src/scheduler/script.rs` (L89 — matching process-group kill)
- `/Users/Robert/Code/public/cronduit/src/scheduler/docker.rs` (L250–L409)
- `/Users/Robert/Code/public/cronduit/src/scheduler/docker_orphan.rs` (entire file — `mark_run_orphaned` guard at L120, L131)
- `/Users/Robert/Code/public/cronduit/src/scheduler/log_pipeline.rs` (entire file — `LogLine` lacks `id`)
- `/Users/Robert/Code/public/cronduit/src/web/handlers/sse.rs` (entire file)
- `/Users/Robert/Code/public/cronduit/src/web/handlers/run_detail.rs` (entire file — `fetch_logs` L97–L132)
- `/Users/Robert/Code/public/cronduit/src/web/handlers/health.rs` (entire file — `(StatusCode, Json)` shape)
- `/Users/Robert/Code/public/cronduit/src/db/queries.rs` (L286–L313 `insert_running_run`, L365 `insert_log_batch`, L783 `get_log_lines`)
- `/Users/Robert/Code/public/cronduit/migrations/sqlite/20260410_000000_initial.up.sql` (schema baseline)
- `/Users/Robert/Code/public/cronduit/Dockerfile` (no HEALTHCHECK directive)
- `/Users/Robert/Code/public/cronduit/examples/docker-compose.yml` (no healthcheck stanza)
- `/Users/Robert/Code/public/cronduit/examples/docker-compose.secure.yml` (no healthcheck stanza)
- `/Users/Robert/Code/public/cronduit/.planning/research/ARCHITECTURE.md` (§3.1 — §5.6)
- `/Users/Robert/Code/public/cronduit/.planning/research/FEATURES.md` (L92–L94 — flags the `kill_on_drop` regression proposal)
- `/Users/Robert/Code/public/cronduit/.planning/milestones/v1.0-research/PITFALLS.md` (referenced as "v1.0-P#N" throughout)
- Docker/moby ecosystem: moby#8441 (auto_remove race, already cited in v1.0 research), moby#50326 (container:<name> ns race, v1.0 P#2), SQLite docs on "Making Other Kinds Of Table Schema Changes" (12-step table-rewrite pattern), docker-py#2655 (wait vs auto_remove), Docker healthcheck docs (`--start-period`, `--retries`).

---

## Confidence

| Area | Level | Basis |
|------|-------|-------|
| Stop-a-job pitfalls | HIGH | Verified against `mod.rs` main loop, `run.rs` run_job, `command.rs` + `docker.rs` cancel branches, `docker_orphan.rs` mark_run_orphaned. All line numbers checked. |
| Log ordering + backfill | HIGH | Confirmed `LogLine` has no id field; confirmed broadcast-before-insert ordering in `log_writer_task` L334–L341; confirmed `run_detail.html` has no backfill today. |
| TOCTOU on run detail | HIGH | Handler path in `run_detail.rs` L140–L144 returns 404 on `Ok(None)`; scheduler insert happens async via mpsc. |
| Per-job run number migration | HIGH | Three-step split grounded in SQLite docs on NOT NULL addition; bundled sqlite version needs a CI assertion. |
| Timeline pitfalls | HIGH | Existing indices + dashboard query shape both verified in `queries.rs` and the initial migration. |
| Bulk enable/disable | HIGH | `sync_config_to_db`, `disable_missing_jobs`, `upsert_job` all read; override-clear logic derived from existing sync. |
| Healthcheck scope correction | HIGH | Grep-verified: zero `HEALTHCHECK` in Dockerfile, zero `healthcheck:` in shipped compose files. |
| FEATURES.md `kill_on_drop` regression flag | HIGH | Direct read of `FEATURES.md` L93 and `command.rs` L203 — the two disagree, and the current code is correct. |
| Percentile rounding | MEDIUM-HIGH | Standard stats problem; convention choice is a decision not a risk. |

