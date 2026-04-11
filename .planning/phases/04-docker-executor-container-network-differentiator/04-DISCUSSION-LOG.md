# Phase 4: Docker Executor & container-network Differentiator - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-10
**Phase:** 04-docker-executor-container-network-differentiator
**Areas discussed:** Pull failure UX, Container lifecycle, Orphan reconciliation, Network pre-flight

---

## Pull failure UX

| Option | Description | Selected |
|--------|-------------|----------|
| Verbose progress | Each retry attempt logged with reason + backoff duration | ✓ |
| Summary only | Single log line after all retries exhausted | |
| You decide | Claude picks the approach | |

**User's choice:** Verbose progress
**Notes:** Operator sees exactly what happened during retries.

| Option | Description | Selected |
|--------|-------------|----------|
| Fail fast | Classify errors: network/timeout → retry, unauthorized/manifest-unknown → fail immediately | ✓ |
| Always retry 3x | Always do 3 attempts regardless of error type | |
| You decide | Claude picks based on bollard's error types | |

**User's choice:** Fail fast
**Notes:** No wasted time on unrecoverable errors.

| Option | Description | Selected |
|--------|-------------|----------|
| Log pull progress | Stream bollard pull events as log lines | |
| Silent on success | Only log final digest on success | ✓ |
| You decide | Claude decides | |

**User's choice:** Silent on success
**Notes:** Failures get verbose, success is quiet.

---

## Container lifecycle

| Option | Description | Selected |
|--------|-------------|----------|
| Graceful stop | SIGTERM via stop_container (10s grace), then kill | ✓ |
| Immediate kill | Call kill_container directly on timeout | |
| You decide | Claude picks | |

**User's choice:** Graceful stop
**Notes:** Matches Docker CLI behavior, lets containers clean up.

| Option | Description | Selected |
|--------|-------------|----------|
| Drain until EOF | Continue reading log stream until EOF after container exits | ✓ |
| Bounded drain (5s) | Wait up to 5 seconds for remaining logs | |
| You decide | Claude picks | |

**User's choice:** Drain until EOF
**Notes:** bollard's log stream closes naturally when container stops.

| Option | Description | Selected |
|--------|-------------|----------|
| Stop containers | Send stop_container to all in-flight Docker jobs during shutdown | ✓ |
| Leave running | Let containers finish on their own | |
| You decide | Claude picks | |

**User's choice:** Stop containers
**Notes:** Prevents orphans, consistent with command executor killing child processes.

---

## Orphan reconciliation

| Option | Description | Selected |
|--------|-------------|----------|
| Kill and mark error | Stop + remove still-running containers, mark DB as error='orphaned at restart' | ✓ |
| Mark DB only, leave running | Don't touch container, just mark DB row | |
| You decide | Claude picks | |

**User's choice:** Kill and mark error
**Notes:** Clean slate on every boot. Prevents resource leaks and duplicate runs.

| Option | Description | Selected |
|--------|-------------|----------|
| Remove stopped too | Remove stopped cronduit-labeled containers and update DB | ✓ |
| Only handle running | Only reconcile running containers | |
| You decide | Claude picks | |

**User's choice:** Remove stopped too
**Notes:** Full cleanup so no stale containers accumulate.

| Option | Description | Selected |
|--------|-------------|----------|
| Log each | WARN-level log per orphan with container/job/run details | ✓ |
| Summary only | Single INFO line with count | |
| You decide | Claude picks | |

**User's choice:** Log each
**Notes:** Operators see exactly what was cleaned up.

---

## Network pre-flight

| Option | Description | Selected |
|--------|-------------|----------|
| Exists AND running | Inspect target container, verify running state | ✓ |
| Exists only | Just check container exists by name | |
| You decide | Claude picks | |

**User's choice:** Exists AND running
**Notes:** Network namespace only works when target is running.

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, verify network exists | Call inspect_network before creating container | ✓ |
| No, let Docker fail | Only pre-flight container:<name> mode | |
| You decide | Claude picks | |

**User's choice:** Yes, verify network exists
**Notes:** Consistent pre-flight behavior across all network modes.

| Option | Description | Selected |
|--------|-------------|----------|
| Distinct error | Separate docker_unavailable, network_target_unavailable, network_not_found | ✓ |
| Same error bucket | All pre-flight failures as 'preflight_failed' with raw reason | |
| You decide | Claude picks | |

**User's choice:** Distinct error
**Notes:** Operators and metrics can distinguish infrastructure from configuration problems.

## Claude's Discretion

No areas deferred to Claude's discretion.

## Deferred Ideas

None — discussion stayed within phase scope.
